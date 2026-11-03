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
use std::sync::Arc;

use serde_json::{Value, json};
use synvelis_gateway::fixture::FixtureAdapter;
use synvelis_gateway::snapshot::attributes;
use synvelis_gateway::*;
use synvelis_project::{
    Asset, Clip, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Lane, LaneId, MidiDivision,
    MidiNote, MidiSource, MidiTick, MusicalDuration, MusicalTick, NoteId, PlaybackMapping, Project,
    ProjectId, ProjectRevision, Source, SourceFrame, SourceId, SourceRange, SourceState,
    TimeAnchor, Track, TrackId,
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
    pub(crate) source: Arc<MemorySnapshotSource>,
    pub(crate) engine: Arc<PrepareEngine>,
    pub(crate) gw: Gateway,
}

pub(crate) fn fixture_rig() -> Rig {
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
    let engine = Arc::new(PrepareEngine::new(
        ctx,
        Arc::new(PreparePolicy::new(store.clone(), clock.clone())),
        PrepareBudget::default(),
    ));
    let mut gw = Gateway::with_evaluator(Box::new(GrantPolicy::new(
        store.clone(),
        clock.clone(),
        oracle,
    )));
    engine.install(&mut gw).unwrap();
    Rig {
        store,
        source,
        engine,
        gw,
    }
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

pub(crate) fn issue_agent(store: &InMemoryPolicyStore, tag: &str, grant: Grant) {
    issue_task(store, tag);
    store.issue_grant(grant);
}

// ── Request builders ─────────────────────────────────────────

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
