//! Shared GW-04 test rig — the linked-MIDI demo project, gateway
//! wiring, and request builders used by `prepare.rs` and
//! `prepare_regressions.rs`.
//!
//! Each test binary compiles this module independently; helpers are
//! shared across the suite, so unused-here helpers stay.
#![allow(dead_code)]

//! GW-04 preparation tests — Issue #56 acceptance criteria.
//!
//! Each test names the contract it proves (GTC-005, G2, GT-005/006,
//! SEC-05/06/14): non-destructive preparation against a pinned base,
//! real effect derivation (linked MIDI vs explicit independent copy),
//! typed error taxonomy, GW-02 effect-aware authorization, and the
//! `ProjectController` single-writer/Undo integration seam.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use synvelis_gateway::commit::{JournalPayload, PolicyEvent};
use synvelis_gateway::fixture::FixtureAdapter;
use synvelis_gateway::journal;
use synvelis_gateway::snapshot::attributes;
use synvelis_gateway::*;
use synvelis_project::{
    Asset, Clip, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, CounterIdGenerator,
    DirectoryBundleStore, Lane, LaneId, MidiDivision, MidiNote, MidiSource, MidiTick,
    MusicalDuration, MusicalTick, NoteId, PlaybackMapping, Project, ProjectController, ProjectId,
    ProjectOp, ProjectRevision, ProjectStore, Source, SourceFrame, SourceId, SourceRange,
    SourceState, TimeAnchor, Track, TrackId,
};

// ── Fixture ids ──────────────────────────────────────────────

pub(crate) fn project() -> ProjectId {
    ProjectId::new(7)
}
pub(crate) fn clip11() -> ResourceRef {
    ResourceRef::Clip(ClipId::new(11))
}
pub(crate) fn clip12() -> ResourceRef {
    ResourceRef::Clip(ClipId::new(12))
}
pub(crate) fn clip13() -> ResourceRef {
    ResourceRef::Clip(ClipId::new(13))
}
pub(crate) fn lane4() -> ResourceRef {
    ResourceRef::Lane(LaneId::new(4))
}
pub(crate) fn source9() -> ResourceRef {
    ResourceRef::Source(SourceId::new(9))
}
pub(crate) fn source10() -> ResourceRef {
    ResourceRef::Source(SourceId::new(10))
}
pub(crate) fn note20() -> ResourceRef {
    ResourceRef::Note(NoteId::new(20))
}
pub(crate) fn note21() -> ResourceRef {
    ResourceRef::Note(NoteId::new(21))
}
pub(crate) fn project_ref() -> ResourceRef {
    ResourceRef::Project(project())
}

pub(crate) fn task_ref(tag: &str) -> TaskRef {
    TaskRef::new(tag).unwrap()
}
pub(crate) fn grant_ref(tag: &str) -> GrantRef {
    GrantRef::new(tag).unwrap()
}
pub(crate) fn cap_ref(id: &str) -> CapabilityRef {
    CapabilityRef {
        id: CapabilityId::new(id).unwrap(),
        version: CapabilityVersion::new(1, 0),
    }
}
pub(crate) fn tagged(r: &ResourceRef) -> String {
    match r {
        ResourceRef::Project(id) => id.to_tagged(),
        ResourceRef::Track(id) => id.to_tagged(),
        ResourceRef::Lane(id) => id.to_tagged(),
        ResourceRef::Clip(id) => id.to_tagged(),
        ResourceRef::Source(id) => id.to_tagged(),
        ResourceRef::Asset(id) => id.to_tagged(),
        ResourceRef::Note(id) => id.to_tagged(),
        _ => unimplemented!("fixture kinds"),
    }
}

pub(crate) fn adapter() -> FixtureAdapter {
    FixtureAdapter::for_testing()
}
pub(crate) fn agent_principal() -> Principal {
    Principal::new(PrincipalKind::Agent, "mcp-client-1").unwrap()
}
pub(crate) fn human_principal() -> Principal {
    Principal::new(PrincipalKind::Human, "user-1").unwrap()
}
pub(crate) fn agent() -> CallerIdentity {
    adapter().agent("mcp-client-1").unwrap()
}
/// A second agent — the "other principal" for binding/blinding cases.
pub(crate) fn other_agent() -> CallerIdentity {
    adapter().agent("mcp-client-2").unwrap()
}
pub(crate) fn human() -> CallerIdentity {
    adapter().human("user-1").unwrap()
}

// ── The linked-MIDI demo project ─────────────────────────────
//
// track3 ─ lane4 ─ clip11 → media source9 (asset5)
//               └ clip12 → midi source10 (notes 20/21, PPQN 480)
//         lane5 ─ clip13 → midi source10   ← linked: same S1

pub(crate) fn demo_project() -> Project {
    let mut p = Project::new(project(), "demo");
    p.id_watermark = 1000;
    let track = TrackId::new(3);
    let lane = LaneId::new(4);
    let lane5 = LaneId::new(5);
    let src9 = SourceId::new(9);
    let src10 = SourceId::new(10);
    p.tracks.insert(
        track,
        Track {
            id: track,
            name: "t3".to_owned(),
            lane_ids: vec![lane, lane5],
        },
    );
    p.lanes.insert(
        lane,
        Lane {
            id: lane,
            track_id: track,
            clip_ids: vec![ClipId::new(11), ClipId::new(12)],
        },
    );
    p.lanes.insert(
        lane5,
        Lane {
            id: lane5,
            track_id: track,
            clip_ids: vec![ClipId::new(13)],
        },
    );
    p.assets.insert(
        synvelis_project::AssetId::new(5),
        Asset {
            id: synvelis_project::AssetId::new(5),
            digest: "sha256:asset5".to_owned(),
        },
    );
    p.sources.insert(
        src9,
        Source {
            id: src9,
            asset_id: synvelis_project::AssetId::new(5),
            content_digest: "sha256:media9".to_owned(),
            state: SourceState::Verified,
        },
    );
    let mut notes = BTreeMap::new();
    for (id, key, start, len) in [(20u128, 60u8, 0i64, 240i64), (21, 64, 480, 240)] {
        notes.insert(
            NoteId::new(id),
            MidiNote {
                id: NoteId::new(id),
                key,
                velocity: 100,
                start: MidiTick::new(start).unwrap(),
                len: MidiTick::new(len).unwrap(),
            },
        );
    }
    p.midi_sources.insert(
        src10,
        MidiSource {
            id: src10,
            division: MidiDivision::ppqn(480).unwrap(),
            notes,
        },
    );
    for (clip, lane, source) in [
        (ClipId::new(11), lane, src9),
        (ClipId::new(12), lane, src10),
        (ClipId::new(13), lane5, src10),
    ] {
        p.clips.insert(
            clip,
            Clip {
                id: clip,
                lane_id: lane,
                source_id: source,
                placement: ClipPlacement {
                    anchor: TimeAnchor::Musical(MusicalTick::new(120)),
                    extent: ClipExtent::Musical(MusicalDuration::new(240).unwrap()),
                },
                playback: ClipPlayback {
                    source_region: SourceRange::try_new(
                        SourceFrame::new(0).unwrap(),
                        SourceFrame::new(48_000).unwrap(),
                    )
                    .unwrap(),
                    mapping: PlaybackMapping::Natural,
                },
                muted: false,
            },
        );
    }
    p
}

// ── Rig ──────────────────────────────────────────────────────

pub(crate) struct LatestOracle {
    pub(crate) source: Arc<MemorySnapshotSource>,
}

impl ResourceOracle for LatestOracle {
    fn facts(&self, project: ProjectId, resource: &ResourceRef) -> ResourceFacts {
        let Some(rev) = self.source.latest_revision(project) else {
            return ResourceFacts::Unresolved;
        };
        let Some(state) = self.source.pin(project, rev) else {
            return ResourceFacts::Unresolved;
        };
        match attributes(&state, resource) {
            Some(attrs) => ResourceFacts::Known(attrs),
            None => ResourceFacts::Unresolved,
        }
    }
}

pub(crate) struct StaticSelection;

impl SelectionSource for StaticSelection {
    fn selection(&self, _project: ProjectId, _view: &SelectionViewRef) -> SelectionFacts {
        SelectionFacts::Unknown
    }
}

pub(crate) struct Rig {
    pub(crate) store: Arc<InMemoryPolicyStore>,
    pub(crate) clock: Arc<ManualClock>,
    pub(crate) source: Arc<MemorySnapshotSource>,
    pub(crate) proposals: Arc<ProposalStore>,
    pub(crate) engine: Arc<PrepareEngine>,
    pub(crate) proposal_engine: Arc<ProposalEngine>,
    pub(crate) gw: Gateway,
}

/// Proposal/approval TTLs used by the rig — far enough ahead of the
/// manual clock's start (`1_000`) that expiry is opt-in per test.
pub(crate) const PROPOSAL_TTL: u64 = 60_000;
pub(crate) const APPROVAL_TTL: u64 = 30_000;

pub(crate) fn fixture_rig() -> Rig {
    fixture_rig_with_cap(PrepareBudget::default().max_proposals)
}

/// Same wiring with a bounded proposal store — eviction-order tests
/// pick a small cap instead of pushing hundreds of proposals.
pub(crate) fn fixture_rig_with_cap(max_entries: usize) -> Rig {
    let store = Arc::new(InMemoryPolicyStore::new());
    let clock = Arc::new(ManualClock::new(PolicyInstant::new(1_000)));
    let source = Arc::new(MemorySnapshotSource::new());
    source.commit(project(), ProjectRevision(1), demo_project());
    let oracle: Arc<dyn ResourceOracle> = Arc::new(LatestOracle {
        source: source.clone(),
    });
    let ctx = Arc::new(ContextProjectionEngine::new(
        source.clone(),
        Arc::new(StaticSelection),
        Arc::new(GrantPolicy::new(
            store.clone(),
            clock.clone(),
            oracle.clone(),
        )),
        clock.clone(),
        ContextBudget::default(),
    ));
    let proposals = Arc::new(ProposalStore::new(
        clock.clone(),
        PROPOSAL_TTL,
        APPROVAL_TTL,
        max_entries,
    ));
    let engine = Arc::new(PrepareEngine::new(
        ctx.clone(),
        Arc::new(PreparePolicy::new(store.clone(), clock.clone())),
        PrepareBudget::default(),
        proposals.clone(),
    ));
    let proposal_engine = Arc::new(ProposalEngine::new(
        proposals.clone(),
        source.clone(),
        store.clone(),
        Arc::new(PreparePolicy::new(store.clone(), clock.clone())),
    ));
    // The GW-05 apply gate wraps the GW-02 evaluator: everything the
    // base policy decides still decides, and the apply stage
    // additionally asks the receipt question.
    let mut gw = Gateway::with_evaluator(Box::new(ProposalGate::new(
        proposals.clone(),
        Box::new(GrantPolicy::new(store.clone(), clock.clone(), oracle)),
    )));
    engine.install(&mut gw).unwrap();
    proposal_engine.install(&mut gw).unwrap();
    // The context-query capability rides the same pinned snapshot
    // source — the rig exposes the full GW-03/04/05 surface.
    ctx.install(&mut gw).unwrap();
    Rig {
        store,
        clock,
        source,
        proposals,
        engine,
        proposal_engine,
        gw,
    }
}

// ── GW-07 job rig ────────────────────────────────────────────
//
// The fixture rig plus the job engine over the deterministic
// provider/media stubs; the media book doubles as the
// `synvelis.media.register` gate (same wiring `with_media` expects).

pub(crate) struct JobRig {
    pub(crate) rig: Rig,
    pub(crate) jobs: Arc<JobEngine>,
    pub(crate) providers: BTreeMap<String, Arc<ProviderStub>>,
    pub(crate) content: Arc<StubContent>,
    pub(crate) book: Arc<MediaBook>,
}

/// Builds `(provider-key → ProviderStub)` ports from constructor
/// arguments so tests keep the typed stub handles.
#[allow(clippy::needless_pass_by_value)] // stubs move into shared ports
pub(crate) fn job_rig(providers: Vec<(String, ProviderStub)>) -> JobRig {
    let store = Arc::new(InMemoryPolicyStore::new());
    let clock = Arc::new(ManualClock::new(PolicyInstant::new(1_000)));
    let source = Arc::new(MemorySnapshotSource::new());
    source.commit(project(), ProjectRevision(1), demo_project());
    let oracle: Arc<dyn ResourceOracle> = Arc::new(LatestOracle {
        source: source.clone(),
    });
    // The media stub must read the same content store the providers
    // write declared outputs into — derive it from the first stub
    // (all providers in a rig share one store by construction).
    let content = providers
        .first()
        .map_or_else(StubContent::shared, |(_, s)| s.content());
    let book = MediaBook::shared();
    let mut ports: BTreeMap<String, Arc<dyn JobPort>> = BTreeMap::new();
    let mut stubs: BTreeMap<String, Arc<ProviderStub>> = BTreeMap::new();
    for (key, stub) in providers {
        debug_assert!(
            Arc::ptr_eq(&stub.content(), &content),
            "every provider stub shares the rig's content store"
        );
        let stub = Arc::new(stub);
        ports.insert(key.clone(), stub.clone());
        stubs.insert(key, stub);
    }
    let jobs = Arc::new(JobEngine::new(
        ports,
        Arc::new(MediaStub::new(content.clone())),
        book.clone(),
        source.clone(),
        store.clone(),
        Arc::new(GrantPolicy::new(
            store.clone(),
            clock.clone(),
            oracle.clone(),
        )),
    ));
    let ctx = Arc::new(ContextProjectionEngine::new(
        source.clone(),
        Arc::new(StaticSelection),
        Arc::new(GrantPolicy::new(
            store.clone(),
            clock.clone(),
            oracle.clone(),
        )),
        clock.clone(),
        ContextBudget::default(),
    ));
    let proposals = Arc::new(ProposalStore::new(
        clock.clone(),
        PROPOSAL_TTL,
        APPROVAL_TTL,
        PrepareBudget::default().max_proposals,
    ));
    let engine = Arc::new(PrepareEngine::with_media(
        ctx,
        Arc::new(PreparePolicy::new(store.clone(), clock.clone())),
        PrepareBudget::default(),
        proposals.clone(),
        Some(book.clone()),
    ));
    let proposal_engine = Arc::new(ProposalEngine::new(
        proposals.clone(),
        source.clone(),
        store.clone(),
        Arc::new(PreparePolicy::new(store.clone(), clock.clone())),
    ));
    let mut gw = Gateway::with_evaluator(Box::new(ProposalGate::new(
        proposals.clone(),
        Box::new(GrantPolicy::new(store.clone(), clock.clone(), oracle)),
    )));
    engine.install(&mut gw).unwrap();
    proposal_engine.install(&mut gw).unwrap();
    jobs.install(&mut gw).unwrap();
    JobRig {
        rig: Rig {
            store,
            clock,
            source,
            proposals,
            engine,
            proposal_engine,
            gw,
        },
        jobs,
        providers: stubs,
        content,
        book,
    }
}

/// A job grant: the three job capabilities plus
/// `synvelis.media.register` plus the prepare/apply axis, the
/// `RunJob`/`MutateProject`/`ReadContext` permissions, and the
/// provider-transfer + disclosure classes the staged flow needs.
pub(crate) fn job_grant(tag: &str, task_tag: &str, ops: &[&str]) -> Grant {
    let mut caps = BTreeSet::from([
        cap_ref("synvelis.operation.prepare"),
        cap_ref(JOB_SUBMIT_ID),
        cap_ref(JOB_GET_ID),
        cap_ref(JOB_CANCEL_ID),
        cap_ref(MEDIA_REGISTER_ID),
        cap_ref(APPLY_ID),
    ]);
    caps.extend(ops.iter().map(|id| cap_ref(id)));
    Grant {
        id: grant_ref(tag),
        issued_by: human_principal(),
        principal: agent_principal(),
        project: project(),
        binding: GrantBinding::Task {
            task: task_ref(task_tag),
        },
        capabilities: caps,
        permissions: PermissionSet::of([
            PermissionKind::MutateProject,
            PermissionKind::RunJob,
            PermissionKind::ReadContext,
        ]),
        data_access: DataAccessSet::of([
            DataAccessClass::MetadataRead,
            DataAccessClass::ExternalDisclosure,
            DataAccessClass::ProviderTransfer,
        ]),
        scope: ResourceScope::Exact {
            // asset5 is the `media.register` create parent — the
            // target check needs it in scope; create_within bounds
            // the *kind* axis.
            resources: BTreeSet::from([
                clip11(),
                clip12(),
                source9(),
                source10(),
                ResourceRef::Asset(synvelis_project::AssetId::new(5)),
            ]),
        },
        create_within: Some(CreateWithin {
            parents: OwnerConstraint::Any,
            kinds: BTreeSet::from([ResourceKind::Source]),
            placement: None,
        }),
        approval_policy: ApprovalPolicy::DelegatedApply,
        limits: GrantLimits::default(),
        validity: Validity::default(),
        epoch_ceiling: None,
        issued_at_revision: Some(ProjectRevision(1)),
    }
}

/// A wire request against a job capability — the same request shape
/// `proposal_req` builds, with the unconditional consistency the
/// async-operation domain requires.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn job_req(capability: &str, args: Value, task: &str, grant: &str) -> Value {
    proposal_req(
        capability,
        args,
        Some(task),
        Some(grant),
        json!({"kind": "unconditional"}),
    )
}

/// The submit wire request for a generation job.
pub(crate) fn submit_req(task: &str, grant: &str, args: Value) -> Value {
    job_req(JOB_SUBMIT_ID, args, task, grant)
}

// ── Authority builders ───────────────────────────────────────

pub(crate) fn issue_task(store: &InMemoryPolicyStore, tag: &str) {
    store.issue_task(AuthorizedTask {
        task: task_ref(tag),
        principal: agent_principal(),
        project: project(),
        activation: Activation::UserIssued,
        validity: Validity::default(),
    });
}

/// A prepare grant: inner operation capabilities + mutate permission
/// + disclosure classes + scope (+ optional create axis).
pub(crate) fn prep_grant(
    tag: &str,
    task_tag: &str,
    ops: &[&str],
    scope: ResourceScope,
    create_within: Option<CreateWithin>,
    classes: &[DataAccessClass],
) -> Grant {
    let mut caps = BTreeSet::from([cap_ref("synvelis.operation.prepare")]);
    caps.extend(ops.iter().map(|id| cap_ref(id)));
    Grant {
        id: grant_ref(tag),
        issued_by: human_principal(),
        principal: agent_principal(),
        project: project(),
        binding: GrantBinding::Task {
            task: task_ref(task_tag),
        },
        capabilities: caps,
        permissions: PermissionSet::of([PermissionKind::MutateProject]),
        data_access: DataAccessSet::of(classes.iter().copied()),
        scope,
        create_within,
        approval_policy: ApprovalPolicy::DelegatedApply,
        limits: GrantLimits::default(),
        validity: Validity::default(),
        epoch_ceiling: None,
        issued_at_revision: Some(ProjectRevision(1)),
    }
}

pub(crate) fn mutate_classes() -> Vec<DataAccessClass> {
    vec![
        DataAccessClass::MetadataRead,
        DataAccessClass::ExternalDisclosure,
    ]
}

pub(crate) fn content_classes() -> Vec<DataAccessClass> {
    vec![
        DataAccessClass::MetadataRead,
        DataAccessClass::StructuredContentRead,
        DataAccessClass::ExternalDisclosure,
    ]
}

/// String forms matching `mutate_classes`/`content_classes` — the
/// oracle's fact vocabulary (independent of the typed enum).
pub(crate) fn mutate_classes_str() -> Vec<&'static str> {
    vec!["MetadataRead", "ExternalDisclosure"]
}
pub(crate) fn content_classes_str() -> Vec<&'static str> {
    vec![
        "MetadataRead",
        "StructuredContentRead",
        "ExternalDisclosure",
    ]
}
pub(crate) fn metadata_only_str() -> Vec<&'static str> {
    vec!["MetadataRead"]
}

pub(crate) fn issue_agent(store: &InMemoryPolicyStore, tag: &str, grant: Grant) {
    issue_task(store, tag);
    store.issue_grant(grant);
}

// ── Request builders ─────────────────────────────────────────

/// A wire request against one of the GW-05 proposal capabilities.
// The `Value` args are serialized into the request, not consumed.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn proposal_req(
    capability: &str,
    args: Value,
    task: Option<&str>,
    grant: Option<&str>,
    consistency: Value,
) -> Value {
    let mut req = json!({
        "request_id": "req-prop",
        "capability": {"id": capability, "version": {"major": 1, "minor": 0}},
        "schema": {"name": format!("{capability}.input"), "version": {"major": 1, "minor": 0}},
        "project": project().to_tagged(),
        "args": args,
        "consistency": consistency,
    });
    if let Some(t) = task {
        req["task"] = json!(t);
    }
    if let Some(g) = grant {
        req["grant"] = json!(g);
    }
    req
}

pub(crate) fn preview_req(
    proposal: &str,
    digest: Option<&str>,
    task: Option<&str>,
    grant: Option<&str>,
) -> Value {
    let mut args = json!({"proposal": proposal});
    if let Some(d) = digest {
        args["digest"] = json!(d);
    }
    proposal_req(PREVIEW_ID, args, task, grant, expected(1))
}

pub(crate) fn approve_req(proposal: &str, digest: &str) -> Value {
    proposal_req(
        APPROVE_ID,
        json!({"proposal": proposal, "digest": digest}),
        None,
        None,
        expected(1),
    )
}

pub(crate) fn apply_req(
    proposal: &str,
    digest: &str,
    approval: Option<&str>,
    task: Option<&str>,
    grant: Option<&str>,
) -> Value {
    let mut args = json!({"proposal": proposal, "digest": digest});
    if let Some(a) = approval {
        args["approval"] = json!(a);
    }
    proposal_req(APPLY_ID, args, task, grant, expected(1))
}

/// A prepare+apply grant for the rig's agent — the same grant must
/// cover preparation and the apply gate so the receipt's grant/task
/// binding matches the apply request.
pub(crate) fn apply_grant(
    tag: &str,
    task_tag: &str,
    ops: &[&str],
    policy: ApprovalPolicy,
) -> Grant {
    let mut grant = prep_grant(
        tag,
        task_tag,
        ops,
        ResourceScope::Exact {
            resources: BTreeSet::from([clip11(), clip12(), clip13(), source10()]),
        },
        None,
        &content_classes(),
    );
    let mut caps = grant.capabilities.clone();
    caps.insert(cap_ref(PREVIEW_ID));
    caps.insert(cap_ref(APPLY_ID));
    grant.capabilities = caps;
    // Preview reads context; apply mutates — the grant covers both.
    grant.permissions =
        PermissionSet::of([PermissionKind::MutateProject, PermissionKind::ReadContext]);
    grant.approval_policy = policy;
    grant
}

/// Prepares the standard clip-11 mute as `caller`; returns
/// (proposal, digest).
pub(crate) fn prepare_mute(rig: &mut Rig, caller: CallerIdentity) -> (String, String) {
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.clip.mute",
                        res_target("clip", &clip11()),
                        json!({"muted": true}),
                    ),
                    expected(1),
                ),
                caller,
            )
            .unwrap(),
    );
    (
        result["proposal"].as_str().unwrap().to_owned(),
        result["digest"].as_str().unwrap().to_owned(),
    )
}

/// Prepares as the rig's agent under task+grant; returns
/// (proposal, digest).
pub(crate) fn prepare_mute_agent(rig: &mut Rig, task: &str, grant: &str) -> (String, String) {
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    Some(task),
                    Some(grant),
                    op(
                        "synvelis.clip.mute",
                        res_target("clip", &clip11()),
                        json!({"muted": true}),
                    ),
                    expected(1),
                ),
                agent(),
            )
            .unwrap(),
    );
    (
        result["proposal"].as_str().unwrap().to_owned(),
        result["digest"].as_str().unwrap().to_owned(),
    )
}

/// The preview result `proposal` produces for the trusted user.
pub(crate) fn preview_value_of(rig: &mut Rig, proposal: &str) -> Value {
    complete(
        rig.gw
            .handle(&preview_req(proposal, None, None, None), human())
            .unwrap(),
    )
}

// The `Value` args are serialized into the request, not consumed.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn prepare_req(
    task: Option<&str>,
    grant: Option<&str>,
    op: Value,
    consistency: Value,
) -> Value {
    let mut req = json!({
        "request_id": "req-prep",
        "capability": {"id": "synvelis.operation.prepare", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.operation.prepare.input", "version": {"major": 1, "minor": 0}},
        "project": project().to_tagged(),
        "args": {"operation": op},
        "consistency": consistency,
    });
    if let Some(t) = task {
        req["task"] = json!(t);
    }
    if let Some(g) = grant {
        req["grant"] = json!(g);
    }
    req
}

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn op(capability: &str, target: Value, args: Value) -> Value {
    let mut v = json!({
        "capability": {"id": capability, "version": {"major": 1, "minor": 0}},
        "args": args,
    });
    if !target.is_null() {
        v["target"] = target;
    }
    v
}

pub(crate) fn res_target(kind: &str, r: &ResourceRef) -> Value {
    json!({"shape": "resource", "kind": kind, "id": tagged(r)})
}

pub(crate) fn create_target(parent_kind: &str, parent: &ResourceRef, creates: &str) -> Value {
    json!({"shape": "create", "parent_kind": parent_kind, "parent": tagged(parent), "creates": creates})
}

pub(crate) fn expected(rev: u64) -> Value {
    json!({"kind": "expected-revision", "revision": rev})
}
pub(crate) fn read_at(rev: u64) -> Value {
    json!({"kind": "read-at", "revision": rev})
}

pub(crate) fn complete(outcome: GatewayOutcome<Value>) -> Value {
    match outcome {
        GatewayOutcome::Completed { result } => result,
        other => panic!("expected Completed, got {other:?}"),
    }
}

pub(crate) fn record_for(rig: &Rig, result: &Value) -> Arc<PreparedRecord> {
    let proposal = ProposalRef::new(result["proposal"].as_str().unwrap()).unwrap();
    rig.engine.prepared(&proposal).expect("proposal retained")
}

pub(crate) fn effect<'a>(
    record: &'a PreparedRecord,
    kind: EffectKind,
    resource: &ResourceRef,
) -> &'a DerivedEffect {
    record
        .prepared
        .effects
        .iter()
        .find(|e| e.kind == kind && e.resource == *resource)
        .unwrap_or_else(|| panic!("expected {kind:?} effect on {resource:?}"))
}

// ── GW-10 harness seam ──────────────────────────────────────
//
// Shared persistence/recovery helpers so every GW-10 suite boots the
// same real `CommitGate` + `DirectoryBundleStore` + framed journal
// triad. Deterministic scheduling (`ScheduledFence`) and crash
// injection (`CrashHook`) live in `synvelis_gateway::schedule` —
// production seams the suites enumerate, not test-only doubles.

/// Per-test temp dir under the OS temp root.
pub(crate) fn tmpdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("synvelis-gw10-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub(crate) fn apply_desc(rig: &Rig) -> CapabilityDescriptor {
    rig.gw
        .registry()
        .describe(&cap_ref(APPLY_ID))
        .expect("apply capability registered")
        .clone()
}

#[allow(clippy::needless_pass_by_value)] // callers hand an owned descriptor for clarity
pub(crate) fn gate_deps(
    store: Arc<InMemoryPolicyStore>,
    clock: Arc<ManualClock>,
    source: Arc<MemorySnapshotSource>,
    proposals: Arc<ProposalStore>,
    apply_capability: CapabilityDescriptor,
) -> GateDeps {
    let oracle: Arc<dyn ResourceOracle> = Arc::new(LatestOracle {
        source: source.clone(),
    });
    GateDeps {
        policy: store.clone(),
        admin: store.clone(),
        evaluator: GrantPolicy::new(store, clock, oracle),
        proposals,
        apply_capability,
    }
}

pub(crate) fn rig_deps(rig: &Rig) -> GateDeps {
    gate_deps(
        rig.store.clone(),
        rig.clock.clone(),
        rig.source.clone(),
        rig.proposals.clone(),
        apply_desc(rig),
    )
}

/// Publishes the demo project at revision 1 — the same base the rig's
/// snapshot source pins — so `expected_revision: 1` commits land.
pub(crate) fn seed_store(dir: &Path) -> DirectoryBundleStore {
    let mut store = DirectoryBundleStore::create(dir).unwrap();
    if store.head().unwrap().is_none() {
        let mut controller =
            ProjectController::new(demo_project(), Box::new(CounterIdGenerator::new(9_000)));
        controller
            .apply(Command {
                expected_revision: controller.live_revision(),
                op: ProjectOp::SetProjectName {
                    name: "demo".to_owned(),
                },
            })
            .unwrap();
        controller.save(&mut store).unwrap();
    }
    store
}

/// First-boot gate: seeded store, fresh journal, rig-wired deps.
pub(crate) fn boot_with<F: CommitFence>(
    rig: &Rig,
    dir: &Path,
    fence: F,
) -> (CommitGate<F>, RecoveryReport, DirectoryBundleStore) {
    let store_dir = dir.join("project");
    let mut store = seed_store(&store_dir);
    let (gate, report) = CommitGate::open(
        demo_project(),
        Box::new(CounterIdGenerator::new(9_000)),
        &mut store,
        dir.join("commit.journal"),
        rig_deps(rig),
        fence,
    )
    .expect("gate opens");
    (gate, report, store)
}

pub(crate) fn boot(rig: &Rig, dir: &Path) -> (CommitGate, RecoveryReport, DirectoryBundleStore) {
    boot_with(rig, dir, MutexFence::new())
}

/// Restart with the A-model recovery view: a *fresh* policy store —
/// every grant/revocation/consumption the journal carries is what
/// policy knows. The proposal store is likewise fresh: nothing in
/// memory survived the process boundary.
#[allow(clippy::type_complexity)]
pub(crate) fn restart(
    rig: &Rig,
    dir: &Path,
) -> (
    CommitGate,
    RecoveryReport,
    Arc<InMemoryPolicyStore>,
    DirectoryBundleStore,
) {
    let store_dir = dir.join("project");
    let mut store = DirectoryBundleStore::create(&store_dir).unwrap();
    let policy = Arc::new(InMemoryPolicyStore::new());
    let proposals = Arc::new(ProposalStore::new(
        rig.clock.clone(),
        PROPOSAL_TTL,
        APPROVAL_TTL,
        PrepareBudget::default().max_proposals,
    ));
    let deps = gate_deps(
        policy.clone(),
        rig.clock.clone(),
        rig.source.clone(),
        proposals,
        apply_desc(rig),
    );
    let (gate, report) = CommitGate::open(
        demo_project(),
        Box::new(CounterIdGenerator::new(9_000)),
        &mut store,
        dir.join("commit.journal"),
        deps,
        MutexFence::new(),
    )
    .expect("gate reopens");
    (gate, report, policy, store)
}

#[allow(clippy::needless_pass_by_value)] // callers hand an owned grant for clarity
pub(crate) fn issue_via_gate<F: CommitFence>(gate: &CommitGate<F>, task: &str, grant: Grant) {
    gate.issue_task(&AuthorizedTask {
        task: task_ref(task),
        principal: agent_principal(),
        project: project(),
        activation: Activation::UserIssued,
        validity: Validity::default(),
    })
    .unwrap();
    gate.issue_grant(&grant).unwrap();
}

/// Delegated-apply grant covering the standard mute, with a one-unit
/// budget so consumption is observable.
pub(crate) fn delegated_grant(tag: &str, task: &str) -> Grant {
    let mut grant = apply_grant(
        tag,
        task,
        &["synvelis.clip.mute"],
        ApprovalPolicy::DelegatedApply,
    );
    grant.limits.apply_budget = Some(1);
    grant
}

/// Presents an apply request for `proposal`/`digest`; returns the
/// accepted operation id.
pub(crate) fn accept_apply(
    rig: &mut Rig,
    proposal: &str,
    digest: &str,
    task: &str,
    grant: &str,
) -> OperationId {
    let outcome = rig
        .gw
        .handle(
            &apply_req(proposal, digest, None, Some(task), Some(grant)),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::Accepted { operation } = outcome else {
        panic!("expected Accepted, got {outcome:?}");
    };
    operation
}

/// The full GW-05 path: prepare → apply → `CommitRequested`.
pub(crate) fn accepted(rig: &mut Rig, task: &str, grant: &str) -> (ProposalRef, OperationId) {
    let (proposal, digest) = prepare_mute_agent(rig, task, grant);
    let operation = accept_apply(rig, &proposal, &digest, task, grant);
    (ProposalRef::new(&proposal).unwrap(), operation)
}

/// Same as `accepted` but with a wire `idempotency_key`.
pub(crate) fn accepted_keyed(
    rig: &mut Rig,
    task: &str,
    grant: &str,
    key: &str,
) -> (ProposalRef, OperationId) {
    let (proposal, digest) = prepare_mute_agent(rig, task, grant);
    let mut req = apply_req(&proposal, &digest, None, Some(task), Some(grant));
    req["idempotency_key"] = json!(key);
    let outcome = rig.gw.handle(&req, agent()).unwrap();
    let GatewayOutcome::Accepted { operation } = outcome else {
        panic!("expected Accepted, got {outcome:?}");
    };
    (ProposalRef::new(&proposal).unwrap(), operation)
}

/// `ApproveToApply` path: apply → `ApprovalRequired` → human grants →
/// apply presenting the receipt → `CommitRequested`.
pub(crate) fn accepted_via_receipt(
    rig: &mut Rig,
    task: &str,
    grant: &str,
) -> (ProposalRef, OperationId) {
    let (proposal, digest) = prepare_mute_agent(rig, task, grant);
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some(task), Some(grant)),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::ApprovalRequired { approval } = outcome else {
        panic!("expected ApprovalRequired, got {outcome:?}");
    };
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );
    let outcome = rig
        .gw
        .handle(
            &apply_req(
                &proposal,
                &digest,
                Some(approval.as_str()),
                Some(task),
                Some(grant),
            ),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::Accepted { operation } = outcome else {
        panic!("expected Accepted, got {outcome:?}");
    };
    (ProposalRef::new(&proposal).unwrap(), operation)
}

/// Replays the framed journal at `path`; returns the durable payloads.
pub(crate) fn journal_payloads(path: &Path) -> Vec<JournalPayload> {
    let bytes = fs::read(path).unwrap();
    journal::replay(&bytes)
        .records
        .iter()
        .map(|r| serde_json::from_slice::<JournalPayload>(&r.payload).unwrap())
        .collect()
}

/// Whether clip11 is muted under the gate's live controller.
pub(crate) fn clip11_muted<F: CommitFence>(gate: &CommitGate<F>) -> bool {
    gate.inspect(|c| c.live().clips[&ClipId::new(11)].muted)
}

/// Maps a gateway `handle` result to the oracle's decision class —
/// the comparison level the evidence records judge at.
pub(crate) fn verdict_of(outcome: &Result<GatewayOutcome<Value>, ErrorEnvelope>) -> Expected {
    match outcome {
        Ok(
            GatewayOutcome::Completed { .. }
            | GatewayOutcome::Accepted { .. }
            | GatewayOutcome::ProposalReady { .. },
        ) => Expected::Permit,
        Ok(GatewayOutcome::ApprovalRequired { .. }) => Expected::ApprovalRequired,
        Err(env) => verdict_of_err(&Err::<(), _>(env.clone())),
    }
}

/// Maps a `Result` carrying an [`ErrorEnvelope`] (e.g. `gate.commit`)
/// to the oracle's decision class.
pub(crate) fn verdict_of_err<T>(outcome: &Result<T, ErrorEnvelope>) -> Expected {
    match outcome {
        Ok(_) => Expected::Permit,
        Err(env) => match &env.code {
            GatewayError::RevisionConflict { .. } => Expected::Conflict,
            GatewayError::Indeterminate { .. } => Expected::Unknown,
            _ => Expected::Deny,
        },
    }
}

/// An oracle carrying the rig's standard authority facts: task `task`
/// issued to the rig's agent on the demo project, grant `grant`
/// bound to it. Scenario mutations (revocation, expiry, budgets) are
/// applied by the caller on the returned oracle/grant pair.
#[allow(clippy::too_many_arguments)]
pub(crate) fn oracle_grant(oracle: &mut SpecOracle, task: &str, grant: &str, fact: OracleGrant) {
    oracle.with_task(
        task,
        OracleTask {
            principal: agent_principal().id().to_owned(),
            project: project().to_tagged(),
            revoked: false,
        },
    );
    oracle.with_grant(grant, fact);
    oracle.with_revision(&project().to_tagged(), 1);
}

/// The oracle facts matching `apply_grant` — capabilities
/// prepare+apply (+ inner ops), mutate+read permissions, disclosure
/// classes, exact scope, approval policy.
pub(crate) fn oracle_apply_facts(task: &str, ops: &[&str], policy: &str) -> OracleGrant {
    let mut capabilities = BTreeSet::from([
        "synvelis.operation.prepare".to_owned(),
        APPLY_ID.to_owned(),
        PREVIEW_ID.to_owned(),
    ]);
    capabilities.extend(ops.iter().map(|s| (*s).to_owned()));
    OracleGrant {
        principal: agent_principal().id().to_owned(),
        project: project().to_tagged(),
        task: task.to_owned(),
        capabilities,
        permissions: BTreeSet::from(["MutateProject".to_owned(), "ReadContext".to_owned()]),
        data_access: BTreeSet::from([
            "MetadataRead".to_owned(),
            "StructuredContentRead".to_owned(),
            "ExternalDisclosure".to_owned(),
        ]),
        scope: BTreeSet::from([
            tagged(&clip11()),
            tagged(&clip12()),
            tagged(&clip13()),
            tagged(&source10()),
        ]),
        policy: policy.to_owned(),
        budget: None,
        revoked: false,
        expired: false,
    }
}

/// The oracle facts matching `job_grant` — the job capabilities +
/// media register + prepare/apply, run/mutate/read permissions,
/// provider-transfer + disclosure classes, job scope.
pub(crate) fn oracle_job_facts(task: &str, ops: &[&str]) -> OracleGrant {
    let mut capabilities = BTreeSet::from([
        "synvelis.operation.prepare".to_owned(),
        JOB_SUBMIT_ID.to_owned(),
        JOB_GET_ID.to_owned(),
        JOB_CANCEL_ID.to_owned(),
        MEDIA_REGISTER_ID.to_owned(),
        APPLY_ID.to_owned(),
    ]);
    capabilities.extend(ops.iter().map(|s| (*s).to_owned()));
    OracleGrant {
        principal: agent_principal().id().to_owned(),
        project: project().to_tagged(),
        task: task.to_owned(),
        capabilities,
        permissions: BTreeSet::from([
            "MutateProject".to_owned(),
            "RunJob".to_owned(),
            "ReadContext".to_owned(),
        ]),
        data_access: BTreeSet::from([
            "MetadataRead".to_owned(),
            "ExternalDisclosure".to_owned(),
            "ProviderTransfer".to_owned(),
        ]),
        scope: BTreeSet::from([
            tagged(&clip11()),
            tagged(&clip12()),
            tagged(&source9()),
            tagged(&source10()),
            tagged(&ResourceRef::Asset(synvelis_project::AssetId::new(5))),
        ]),
        policy: "DelegatedApply".to_owned(),
        budget: None,
        revoked: false,
        expired: false,
    }
}

/// The request view matching a standard `prepare`/`apply` wire
/// request for `task`/`grant` under `capability`.
pub(crate) fn req_view(task: &str, grant: &str, capability: &str) -> ReqView {
    ReqView {
        principal: agent_principal().id().to_owned(),
        task: Some(task.to_owned()),
        grant: Some(grant.to_owned()),
        capability: capability.to_owned(),
        permission: Some("MutateProject"),
        data_class: None,
        targets: vec![tagged(&clip11())],
        expected_revision: Some(1),
        approval: None,
        digest: None,
    }
}

// ── Context-query builders (GW-03 surface) ──────────────────

/// A context grant: `classes` data-access + `resources` exact scope.
pub(crate) fn ctx_grant(
    tag: &str,
    task_tag: &str,
    classes: &[DataAccessClass],
    resources: &[ResourceRef],
) -> Grant {
    Grant {
        id: grant_ref(tag),
        issued_by: human_principal(),
        principal: agent_principal(),
        project: project(),
        binding: GrantBinding::Task {
            task: task_ref(task_tag),
        },
        capabilities: BTreeSet::from([cap_ref("synvelis.context.query")]),
        permissions: PermissionSet::of([
            PermissionKind::ReadContext,
            PermissionKind::DiscloseContext,
        ]),
        data_access: DataAccessSet::of(classes.iter().copied()),
        scope: ResourceScope::Exact {
            resources: resources.iter().copied().collect(),
        },
        create_within: None,
        approval_policy: ApprovalPolicy::DelegatedApply,
        limits: GrantLimits::default(),
        validity: Validity::default(),
        epoch_ceiling: None,
        issued_at_revision: Some(ProjectRevision(1)),
    }
}

/// A `synvelis.context.query` wire request.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn query_req(
    task: &str,
    grant: &str,
    args: impl Into<Value>,
    consistency: impl Into<Value>,
) -> Value {
    json!({
        "request_id": "req-ctx",
        "capability": {"id": "synvelis.context.query", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.context.query.input", "version": {"major": 1, "minor": 0}},
        "project": project().to_tagged(),
        "args": args.into(),
        "consistency": consistency.into(),
        "task": task,
        "grant": grant,
    })
}

pub(crate) fn latest() -> Value {
    json!({"kind": "latest"})
}
pub(crate) fn selector(args: impl Into<Value>) -> Value {
    json!({"selector": args.into()})
}
pub(crate) fn resources(refs: &[ResourceRef]) -> Value {
    json!({"resources": refs.iter().map(tagged).collect::<Vec<_>>()})
}

/// The oracle facts matching `ctx_grant` — context-query capability,
/// read+disclose permissions, the granted classes, exact scope.
pub(crate) fn oracle_ctx_facts(
    task: &str,
    classes: &[&str],
    resources: &[ResourceRef],
) -> OracleGrant {
    OracleGrant {
        principal: agent_principal().id().to_owned(),
        project: project().to_tagged(),
        task: task.to_owned(),
        capabilities: BTreeSet::from(["synvelis.context.query".to_owned()]),
        permissions: BTreeSet::from(["ReadContext".to_owned(), "DiscloseContext".to_owned()]),
        data_access: classes.iter().map(|s| (*s).to_owned()).collect(),
        scope: resources.iter().map(tagged).collect(),
        policy: "DelegatedApply".to_owned(),
        budget: None,
        revoked: false,
        expired: false,
    }
}

/// The durable `PolicyEvent` positions inside the journal.
pub(crate) fn policy_positions(payloads: &[JournalPayload], kind: &str) -> Vec<usize> {
    payloads
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let matched = matches!(
                (kind, p),
                (
                    "revoke-grant",
                    JournalPayload::Policy(PolicyEvent::RevokeGrant(_))
                ) | (
                    "issue-grant",
                    JournalPayload::Policy(PolicyEvent::IssueGrant(_))
                ) | (
                    "issue-task",
                    JournalPayload::Policy(PolicyEvent::IssueTask(_))
                )
            );
            matched.then_some(i)
        })
        .collect()
}
