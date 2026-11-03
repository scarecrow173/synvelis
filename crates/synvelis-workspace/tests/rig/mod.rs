//! Shared GW-09 test rig — the full GW-01..06 gateway stack plus the
//! `CommitGate` commit boundary plus an optional job engine, wrapped
//! in a [`GatewayUi`] bound to a trusted human caller, over a real
//! `WorkspaceManager` session.
//!
//! Each test binary compiles this module independently; helpers are
//! shared across the suite, so unused-here helpers stay.
#![allow(dead_code)]
//!
//! The rig exists to prove the GW-09 headless contract (issue #60):
//! the UI transmits trusted human intent and explicit ids; GW-02/05/06
//! own authorization, approval, and commit truth; workspace editing
//! never regresses when AI/MCP is off.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};
use synvelis_gateway::commit::CommitGate;
use synvelis_gateway::fixture::FixtureAdapter;
use synvelis_gateway::snapshot::attributes;
use synvelis_gateway::*;
use synvelis_project::{
    Asset, Clip, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, CounterIdGenerator,
    DirectoryBundleStore, Lane, LaneId, MidiDivision, MidiNote, MidiSource, MidiTick,
    MusicalDuration, MusicalTick, NoteId, PlaybackMapping, Project, ProjectController, ProjectId,
    ProjectOp, ProjectRevision, ProjectStore, Source, SourceFrame, SourceId, SourceRange,
    SourceState, TimeAnchor, Track, TrackId,
};
use synvelis_workspace::{
    EditingSelection, FakeWindowHost, GatewayUi, GatewayUiDeps, ProjectSession, SemanticTarget,
    WorkspaceManager,
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

pub(crate) fn task_ref(tag: &str) -> TaskRef {
    TaskRef::new(tag).unwrap()
}
pub(crate) fn grant_ref(tag: &str) -> GrantRef {
    GrantRef::new(tag).unwrap()
}
pub(crate) fn proposal_ref(tag: &str) -> ProposalRef {
    ProposalRef::new(tag).unwrap()
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

// ── seams ────────────────────────────────────────────────────

/// The commit-boundary and host-administration seams, backed by the
/// real `CommitGate` — an `Accepted` apply drives a real fenced
/// commit, and the journal is the durable verdict `reconcile` reads.
///
/// `fail_revoke_grants` is the scripted-failure knob SYS-05's
/// "confirmation failed" case needs: while positive, `revoke_grant`
/// refuses instead of confirming.
pub(crate) struct GatePorts {
    pub(crate) gate: Arc<CommitGate>,
    pub(crate) source: Arc<MemorySnapshotSource>,
    pub(crate) project: ProjectId,
    pub(crate) fail_revoke_grants: AtomicUsize,
}

impl GatePorts {
    /// Schedules `n` scripted `revoke_grant` refusals.
    pub(crate) fn fail_next_revoke_grants(&self, n: usize) {
        self.fail_revoke_grants.store(n, Ordering::Relaxed);
    }
}

impl CommitPort for GatePorts {
    fn commit(&self, proposal: &ProposalRef) -> Result<CommitReceipt, ErrorEnvelope> {
        let receipt = self.gate.commit(proposal)?;
        // A committed mutation advances the snapshot seam so the next
        // prepare/apply sees the new live revision (SEC-11).
        let live = self.gate.inspect(|c| c.live().clone());
        self.source.commit(self.project, receipt.revision, live);
        Ok(receipt)
    }

    fn reconcile(&self, operation: OperationId) -> RetryOutcome {
        self.gate.reconcile(operation)
    }

    fn cancel(&self, proposal: &ProposalRef, _reason: &str) -> Result<bool, ErrorEnvelope> {
        self.gate.cancel_apply(proposal)
    }
}

impl AdminPort for GatePorts {
    fn issue_task(&self, task: AuthorizedTask) -> Result<(), ErrorEnvelope> {
        self.gate.issue_task(&task)
    }

    fn issue_grant(&self, grant: Grant) -> Result<(), ErrorEnvelope> {
        self.gate.issue_grant(&grant)
    }

    fn revoke_grant(&self, id: &GrantRef) -> Result<(), ErrorEnvelope> {
        // Decrement the scripted-failure counter, if armed.
        let mut armed = self.fail_revoke_grants.load(Ordering::Relaxed);
        while armed > 0 {
            match self.fail_revoke_grants.compare_exchange_weak(
                armed,
                armed - 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    return Err(GatewayError::Internal {
                        reason: "scripted revoke failure".to_owned(),
                    }
                    .into());
                }
                Err(actual) => armed = actual,
            }
        }
        self.gate.revoke_grant(id)
    }

    fn revoke_task(&self, id: &TaskRef) -> Result<(), ErrorEnvelope> {
        self.gate.revoke_task(id)
    }

    fn live_revision(&self, project: ProjectId) -> Option<ProjectRevision> {
        (project == self.project).then(|| self.gate.live_revision())
    }
}

// ── project store + gate boot ────────────────────────────────

/// Per-test temp dir under the OS temp root — unique per call so
/// parallel tests never share a bundle/journal path.
static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn tmpdir(tag: &str) -> PathBuf {
    let n = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("synvelis-gw09-{tag}-{n}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Publishes the demo project at revision 1 — the same base the
/// snapshot seam pins — so `expected_revision: 1` commits land.
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

// ── authority builders ───────────────────────────────────────

/// A user-issued task record for `tag`.
pub(crate) fn task(tag: &str) -> AuthorizedTask {
    AuthorizedTask {
        task: task_ref(tag),
        principal: agent_principal(),
        project: project(),
        activation: Activation::UserIssued,
        validity: Validity::default(),
    }
}

/// A prepare+apply grant for the rig's agent — capabilities =
/// `synvelis.operation.prepare` + `synvelis.proposal.*` + `ops`,
/// scope/policies as given.
pub(crate) fn agent_grant(
    tag: &str,
    task_tag: &str,
    ops: &[&str],
    scope: ResourceScope,
    policy: ApprovalPolicy,
) -> Grant {
    let mut caps = BTreeSet::from([
        cap_ref("synvelis.operation.prepare"),
        cap_ref(PREVIEW_ID),
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
            PermissionKind::ReadContext,
        ]),
        data_access: DataAccessSet::of([
            DataAccessClass::MetadataRead,
            DataAccessClass::StructuredContentRead,
            DataAccessClass::ExternalDisclosure,
        ]),
        scope,
        create_within: None,
        approval_policy: policy,
        limits: GrantLimits::default(),
        validity: Validity::default(),
        epoch_ceiling: None,
        issued_at_revision: Some(ProjectRevision(1)),
    }
}

/// A job-capable grant — the three job capabilities + the
/// prepare/apply axis + `RunJob` + provider-transfer, scoped to the
/// demo resources (asset5 included so `media.register` admits it).
pub(crate) fn job_grant(tag: &str, task_tag: &str) -> Grant {
    let mut grant = agent_grant(
        tag,
        task_tag,
        &[JOB_SUBMIT_ID, JOB_GET_ID, JOB_CANCEL_ID, MEDIA_REGISTER_ID],
        ResourceScope::Exact {
            resources: BTreeSet::from([
                clip11(),
                clip12(),
                source9(),
                source10(),
                ResourceRef::Asset(synvelis_project::AssetId::new(5)),
            ]),
        },
        ApprovalPolicy::DelegatedApply,
    );
    grant.permissions = PermissionSet::of([
        PermissionKind::MutateProject,
        PermissionKind::ReadContext,
        PermissionKind::RunJob,
    ]);
    grant.data_access = DataAccessSet::of([
        DataAccessClass::MetadataRead,
        DataAccessClass::StructuredContentRead,
        DataAccessClass::ExternalDisclosure,
        DataAccessClass::ProviderTransfer,
    ]);
    grant.create_within = Some(CreateWithin {
        parents: OwnerConstraint::Any,
        kinds: BTreeSet::from([ResourceKind::Source]),
        placement: None,
    });
    grant
}

// ── request builders ─────────────────────────────────────────

/// Request-id counter — every wire request gets a distinct id so the
/// journal/ledger can never conflate two calls.
static NEXT_REQ: AtomicUsize = AtomicUsize::new(0);

/// A wire request against a capability — the same shape the MCP
/// adapter produces: explicit ids only, no caller/authority fields.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn wire(
    capability: &str,
    args: Value,
    task: Option<&str>,
    grant: Option<&str>,
    consistency: Value,
) -> Value {
    let n = NEXT_REQ.fetch_add(1, Ordering::Relaxed);
    let mut req = json!({
        "request_id": format!("agent-req-{n}-{capability}"),
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

pub(crate) fn expected(rev: u64) -> Value {
    json!({"kind": "expected-revision", "revision": rev})
}

pub(crate) fn unconditional() -> Value {
    json!({"kind": "unconditional"})
}

/// One `synvelis.<op>` operation inside a prepare request.
pub(crate) fn op(capability: &str, target: Value, args: &Value) -> Value {
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

/// `synvelis.operation.prepare` for `operation` under task+grant.
pub(crate) fn prepare_req(
    task: Option<&str>,
    grant: Option<&str>,
    operation: &Value,
    consistency: Value,
) -> Value {
    wire(
        "synvelis.operation.prepare",
        json!({"operation": operation}),
        task,
        grant,
        consistency,
    )
}

/// `synvelis.proposal.apply` for `(proposal, digest[, approval])`.
pub(crate) fn apply_req(
    proposal: &str,
    digest: &str,
    approval: Option<&str>,
    task: Option<&str>,
    grant: Option<&str>,
    revision: u64,
) -> Value {
    let mut args = json!({"proposal": proposal, "digest": digest});
    if let Some(a) = approval {
        args["approval"] = json!(a);
    }
    wire(APPLY_ID, args, task, grant, expected(revision))
}

/// `synvelis.job.submit` — a generation job over `inputs`.
pub(crate) fn submit_req(task: &str, grant: &str, inputs: &[&ResourceRef]) -> Value {
    wire(
        JOB_SUBMIT_ID,
        json!({
            "provider": "gen-1",
            "kind": "generation",
            "output": "media",
            "inputs": inputs.iter().map(|r| tagged(r)).collect::<Vec<_>>(),
            "prompt": "generate something",
        }),
        Some(task),
        Some(grant),
        unconditional(),
    )
}

pub(crate) fn complete(outcome: GatewayOutcome<Value>) -> Value {
    match outcome {
        GatewayOutcome::Completed { result } => result,
        other => panic!("expected Completed, got {other:?}"),
    }
}

/// Prepares the standard clip-11 mute as the agent under
/// `task`/`grant`; returns `(proposal, digest)`.
pub(crate) fn prepare_mute_agent(ui: &mut GatewayUi, task: &str, grant: &str) -> (String, String) {
    let result = complete(
        ui.gateway_mut()
            .handle(
                &prepare_req(
                    Some(task),
                    Some(grant),
                    &op(
                        "synvelis.clip.mute",
                        res_target("clip", &clip11()),
                        &json!({"muted": true}),
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

/// Prepares the clip-12→source10 note edit — the linked-source effect
/// case: the write lands on `source10`, referenced by clip12 *and*
/// clip13. Returns `(proposal, digest)`.
pub(crate) fn prepare_note_edit_agent(
    ui: &mut GatewayUi,
    task: &str,
    grant: &str,
) -> (String, String) {
    let result = complete(
        ui.gateway_mut()
            .handle(
                &prepare_req(
                    Some(task),
                    Some(grant),
                    &op(
                        "synvelis.midi.note.add",
                        res_target("source", &source10()),
                        &json!({
                            "key": 67,
                            "velocity": 90,
                            "range": {"domain": "source-native", "start": 720, "len": 240},
                        }),
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

// ── the fixture ──────────────────────────────────────────────

/// The GW-09 fixture: the workspace session, the bound `GatewayUi`,
/// and the authority seams tests poke directly.
pub(crate) struct UiFixture {
    /// The workspace session (manual editing side).
    pub(crate) ws: WorkspaceManager<FakeWindowHost>,
    /// The headless GW-09 session under test.
    pub(crate) ui: GatewayUi,
    /// The policy store (wire + gate admin share it).
    pub(crate) store: Arc<InMemoryPolicyStore>,
    /// The manual policy clock.
    pub(crate) clock: Arc<ManualClock>,
    /// The committed-snapshot seam.
    pub(crate) source: Arc<MemorySnapshotSource>,
    /// The proposal/approval/apply store.
    pub(crate) proposals: Arc<ProposalStore>,
    /// The commit gate.
    pub(crate) gate: Arc<CommitGate>,
    /// The seam wrapper — `CommitPort` + `AdminPort`.
    pub(crate) ports: Arc<GatePorts>,
    /// The durable project store (save/publication checks).
    pub(crate) project_store: DirectoryBundleStore,
    /// The job engine, when the fixture boots with jobs.
    pub(crate) jobs: Option<Arc<JobEngine>>,
    /// The provider stub, when jobs are wired.
    pub(crate) provider: Option<Arc<ProviderStub>>,
    /// The fixture adapter that binds callers.
    pub(crate) adapter: FixtureAdapter,
    /// The fixture's scratch dir.
    pub(crate) dir: PathBuf,
}

const PROPOSAL_TTL: u64 = 60_000;
const APPROVAL_TTL: u64 = 30_000;

struct LatestOracle {
    source: Arc<MemorySnapshotSource>,
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

struct StaticSelection;

impl SelectionSource for StaticSelection {
    fn selection(&self, _project: ProjectId, _view: &SelectionViewRef) -> SelectionFacts {
        SelectionFacts::Unknown
    }
}

/// Boots the fixture. `jobs: Some(mode)` wires a job engine over one
/// `gen-1` provider stub answering cancels with `mode`; `None` leaves
/// the job seam absent (the AI-off regression's shape).
#[allow(clippy::too_many_lines)] // one linear fixture assembly
pub(crate) fn boot(jobs: Option<CancelMode>) -> UiFixture {
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
        PrepareBudget::default().max_proposals,
    ));
    let (jobs_engine, provider, book) = match jobs {
        Some(mode) => {
            let content = StubContent::shared();
            let stub = ProviderStub::generation(content.clone()).cancel_mode(mode);
            let provider = Arc::new(stub);
            let mut ports: BTreeMap<String, Arc<dyn JobPort>> = BTreeMap::new();
            ports.insert("gen-1".to_owned(), provider.clone());
            let book = MediaBook::shared();
            let engine = Arc::new(JobEngine::new(
                ports,
                Arc::new(MediaStub::new(content)),
                book.clone(),
                source.clone(),
                store.clone(),
                Arc::new(GrantPolicy::new(
                    store.clone(),
                    clock.clone(),
                    oracle.clone(),
                )),
            ));
            (Some(engine), Some(provider), Some(book))
        }
        None => (None, None, None),
    };
    let engine = Arc::new(PrepareEngine::with_media(
        ctx.clone(),
        Arc::new(PreparePolicy::new(store.clone(), clock.clone())),
        PrepareBudget::default(),
        proposals.clone(),
        book.map(|b| b as Arc<dyn MediaGate>),
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
    ctx.install(&mut gw).unwrap();
    if let Some(jobs_engine) = &jobs_engine {
        jobs_engine.install(&mut gw).unwrap();
    }

    // The commit gate — GW-06 owns the project-write boundary; the
    // store seeds revision 1 so `expected_revision: 1` lands.
    let dir = tmpdir("boot");
    let store_dir = dir.join("project");
    let mut project_store = seed_store(&store_dir);
    let apply_capability = gw
        .registry()
        .describe(&cap_ref(APPLY_ID))
        .expect("apply capability registered")
        .clone();
    let gate_oracle: Arc<dyn ResourceOracle> = Arc::new(LatestOracle {
        source: source.clone(),
    });
    let (gate, _report) = CommitGate::open(
        demo_project(),
        Box::new(CounterIdGenerator::new(9_000)),
        &mut project_store,
        dir.join("commit.journal"),
        GateDeps {
            policy: store.clone(),
            admin: store.clone(),
            evaluator: GrantPolicy::new(store.clone(), clock.clone(), gate_oracle),
            proposals: proposals.clone(),
            apply_capability,
        },
        MutexFence::new(),
    )
    .unwrap();
    let gate = Arc::new(gate);
    // The snapshot seam must see the gate's live tip — `save` may have
    // published rev 1 while the in-memory commit pinned rev 1.
    let live = gate.inspect(|c| c.live().clone());
    source.commit(project(), gate.live_revision(), live);

    let ports = Arc::new(GatePorts {
        gate: gate.clone(),
        source: source.clone(),
        project: project(),
        fail_revoke_grants: AtomicUsize::new(0),
    });

    let adapter = adapter();
    let caller = adapter.human("user-1").unwrap();
    let ui = GatewayUi::new(
        caller,
        gw,
        GatewayUiDeps {
            policy: store.clone(),
            admin: ports.clone(),
            permits: Box::new(MemoryPermitStore::new()),
            proposals: proposals.clone(),
            proposal_engine,
            committer: ports.clone(),
            jobs: jobs_engine.clone(),
            clock: clock.clone(),
        },
    );

    // The workspace session — manual editing over the same logical
    // project, on its own controller (the UI side never writes the
    // gate's project directly).
    let session_controller =
        ProjectController::new(demo_project(), Box::new(CounterIdGenerator::new(9_000)));
    let ws = WorkspaceManager::new(
        FakeWindowHost::with_monitors(vec![]),
        ProjectSession::new(session_controller),
    );

    UiFixture {
        ws,
        ui,
        store,
        clock,
        source,
        proposals,
        gate,
        ports,
        project_store,
        jobs: jobs_engine,
        provider,
        adapter,
        dir,
    }
}

/// An editing selection of `SemanticTarget`s — the selection-assist
/// input `capture_scope` consumes.
pub(crate) fn selection_of(targets: &[SemanticTarget]) -> EditingSelection {
    EditingSelection {
        targets: targets.to_vec(),
    }
}
