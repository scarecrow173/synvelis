//! RCD-04 product E2E (synvelis#118 app slice) — headless `syn.*` op
//! evidence for Comp Transition / MIDI Phrase / Recovery Review.
//!
//! Every case drives the same `syn.*` string dispatch the product
//! host uses: `UiRuntime::dispatch_op` → `Service::call` →
//! `AppService` → `SynvelisApp`. Fixture mutation (comp/recording/
//! MIDI ops, staging writers, VCS backend injection) goes through
//! `AppService::app_mut()` on the same composed app — there is no
//! second project model and no test-only op surface.
//!
//! Boundary: this is headless evidence only. Real window/OS GUI,
//! real device, real OS filesystem durability, real RT-engine sample
//! equality, and `AudioLens` native-f64 measurement stay `NotRun` —
//! fake/mock fixtures here never claim those passes.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{Value, json};

use ruviform_ui::ipc::{ErrorBody, Event};
use ruviform_ui::{
    BackendReport, CallOutcome, DataRegistry, OpInfo, Service, ServiceContext, UiRuntime,
};

use synvelis_app::{AppService, SynvelisApp};
use synvelis_project::snapshot::{IndexGeneration, VcsBytesPort, VcsPortError};
use synvelis_project::vcs::{
    FixedVcsRevision, HistoryCompleteness, HistoryNode, HistoryPage, HistoryQuery, IndexEntry,
    IndexEntryMode, ProjectPath, ProjectRelativePath, ProjectScope, RevisionRelations,
    RevisionSelector, StagedSnapshot, VcsCancel, VcsCapabilities, VcsError, VcsFeature,
    VcsHistoryAdapter, VcsObjectFormat, VcsReadProvider, WorkingStatus, WorkspaceInfo,
};
use synvelis_project::{
    BlobRead, CaptureStreamId, ClipExtent, ClipPlacement, ClipPlayback, ClockEpochId,
    CompCandidateRef, CompId, CompOp, CompRegionId, CompTransitionId, FadeShape, LaneId,
    MidiCompRegionId, MidiCompSelection, MidiControllerEvent, MidiControllerKind,
    MidiControllerLane, MidiControllerLanes, MidiDivision, MidiPhraseRegion, MidiPlanAction,
    MidiPlaybackPlan, MidiRange, MidiTick, MusicalDuration, MusicalRange, MusicalTick, NoteId,
    PlaybackMapping, Project, ProjectOp, ProjectRange, ProjectRevision, RecordingSessionId,
    SCHEMA_VERSION, SessionFrame, SessionRange, SnapshotEnvelope, SourceFrame, SourceId,
    SourceRange, SourceState, TakeGroupId, TakeId, TimeAnchor, VcsFileRead, VcsLocator,
    compile_midi_comp,
};
use synvelis_recording::{
    CaptureBlockHeader, CaptureChunk, CaptureSpan, ChunkSink, CommitOutcome, DiscardCandidate,
    EngineEnds, FaultLatch, FaultWriter, FinalizedOutcome, MediaReferenceProbe, MockPcm,
    PublishFaults, RecordingOp, ReferenceCheck, ReviewAction, ReviewDecision, ReviewError,
    ReviewOutcome, ReviewRecord, ReviewState, StagingArea, StagingSessionConfig,
    StagingStreamConfig, StreamWriter, SynvelisOp, build_handoff, commit_session, recovery,
    review_stream, stream_paths, stream_stem,
};

// ---------- shared headless harness ----------

type Outbox = Rc<RefCell<VecDeque<Event>>>;

/// Shares one [`AppService`] between the headless [`UiRuntime`]
/// (real `syn.*` string dispatch) and the test (fixture mutation via
/// `app_mut`). `UiRuntime` never exposes a registered service back,
/// so the service is wrapped in `Rc<RefCell>` and re-registered
/// through a delegating [`Service`] impl.
struct SharedService(Rc<RefCell<AppService>>);

impl Service for SharedService {
    fn ops(&self) -> &'static [OpInfo] {
        self.0.borrow().ops()
    }

    fn call(
        &mut self,
        op: &str,
        params: &Value,
        ctx: &mut ServiceContext<'_>,
    ) -> Result<CallOutcome, ErrorBody> {
        self.0.borrow_mut().call(op, params, ctx)
    }
}

struct Fixture {
    rt: UiRuntime,
    svc: Rc<RefCell<AppService>>,
    outbox: Outbox,
}

fn fixture() -> Fixture {
    let app = SynvelisApp::new(BackendReport::available("webview2", "test-1.0".into()));
    let outbox = app.event_outbox();
    let svc = Rc::new(RefCell::new(AppService::new(app)));
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(SharedService(svc.clone()))
        .expect("register AppService");
    Fixture { rt, svc, outbox }
}

fn call(rt: &mut UiRuntime, op: &str, params: Value) -> (bool, Option<Value>, Option<ErrorBody>) {
    let events = rt.dispatch_op("test-1", op, params);
    let result = events.iter().find_map(|e| match e {
        Event::Result {
            ok, data, error, ..
        } => Some((*ok, data.clone(), error.clone())),
        _ => None,
    });
    result.expect("every intent yields exactly one result event")
}

fn ok(rt: &mut UiRuntime, op: &str, params: Value) -> Value {
    let (ok, data, error) = call(rt, op, params);
    assert!(ok, "{op} failed: {error:?}");
    data.expect("successful result carries data")
}

fn notifies(outbox: &Outbox, channel: &str) -> Vec<Value> {
    let mut matched = Vec::new();
    let mut rest = Vec::new();
    {
        let mut q = outbox.borrow_mut();
        while let Some(e) = q.pop_front() {
            match &e {
                Event::Notify { data, .. } if data["channel"] == channel => {
                    matched.push(data["data"].clone());
                }
                _ => rest.push(e),
            }
        }
        for e in rest.into_iter().rev() {
            q.push_front(e);
        }
    }
    matched
}

fn drain_all(outbox: &Outbox) {
    outbox.borrow_mut().clear();
}

// ---------- fixture mutation through the same app ----------

fn with_live<R>(f: &Fixture, g: impl FnOnce(&Project) -> R) -> R {
    let svc = f.svc.borrow();
    g(svc.app().workspace().session().controller().live())
}

fn session_rev(f: &Fixture) -> ProjectRevision {
    f.svc.borrow().app().workspace().session().revision()
}

fn apply_synvelis(f: &Fixture, op: SynvelisOp) -> ProjectRevision {
    let mut svc = f.svc.borrow_mut();
    let session = svc.app_mut().workspace_mut().session_mut();
    let rev = session.revision();
    session.apply(op, rev).expect("fixture op applies")
}

fn apply_project(f: &Fixture, op: ProjectOp) -> ProjectRevision {
    apply_synvelis(f, SynvelisOp::Project(op))
}

fn apply_recording(f: &Fixture, op: RecordingOp) -> ProjectRevision {
    apply_synvelis(f, SynvelisOp::Recording(op))
}

fn apply_comp(f: &Fixture, op: CompOp) -> ProjectRevision {
    apply_synvelis(f, SynvelisOp::Comp(op))
}

fn project_new(f: &mut Fixture, dir: &Path) -> Value {
    let bundle = dir.join("project");
    ok(
        &mut f.rt,
        "syn.project.new",
        json!({ "path": bundle.to_string_lossy(), "name": "rcd-04-e2e" }),
    )
}

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("synvelis-rcd04e2e-{}-{name}", std::process::id()));
    if d.exists() {
        fs::remove_dir_all(&d).unwrap();
    }
    fs::create_dir_all(&d).unwrap();
    d
}

// ---------- shared domain builders ----------

const RATE: u32 = 48_000;

fn mr(start: i64, end: i64) -> ProjectRange {
    ProjectRange::Musical(
        MusicalRange::try_new(MusicalTick::new(start), MusicalTick::new(end)).unwrap(),
    )
}

fn session_frames(start: i64, end: i64) -> SessionRange {
    SessionRange::try_new(
        SessionFrame::new(start).unwrap(),
        SessionFrame::new(end).unwrap(),
    )
    .unwrap()
}

fn source_frames(start: i64, end: i64) -> SourceRange {
    SourceRange::try_new(
        SourceFrame::new(start).unwrap(),
        SourceFrame::new(end).unwrap(),
    )
    .unwrap()
}

/// Track + `lane_count` lanes + clock epoch + committed media
/// source. Returns `(lane_ids, source_id)`.
fn seed_media_substrate(f: &Fixture, lane_count: usize) -> (Vec<LaneId>, SourceId) {
    apply_project(
        f,
        ProjectOp::AddTrack {
            name: "rcd04".into(),
        },
    );
    let track = with_live(f, |p| *p.tracks.keys().next().unwrap());
    let mut lanes = Vec::new();
    for _ in 0..lane_count {
        apply_project(f, ProjectOp::AddLane { track_id: track });
        lanes.push(with_live(f, |p| *p.lanes.keys().last().unwrap()));
    }
    apply_project(
        f,
        ProjectOp::RegisterClockEpoch {
            description: "mock".into(),
        },
    );
    apply_project(
        f,
        ProjectOp::RegisterAsset {
            digest: "aa".into(),
        },
    );
    let asset = with_live(f, |p| *p.assets.keys().next().unwrap());
    apply_project(
        f,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "aa".into(),
        },
    );
    let source = with_live(f, |p| *p.sources.keys().next().unwrap());
    // Imported → Verified → Committed (forward-only chain).
    for state in [SourceState::Verified, SourceState::Committed] {
        apply_project(
            f,
            ProjectOp::SetSourceState {
                source_id: source,
                state,
            },
        );
    }
    (lanes, source)
}

/// One finalized recording session on `lane` producing `take_count`
/// verified takes over distinct segments — the MD-A comp substrate,
/// built entirely through `RecordingOp`s (undoable, CAS-checked).
/// Returns `(session_id, take_ids, stream_id)`.
fn seed_verified_takes(
    f: &Fixture,
    lane: LaneId,
    source: SourceId,
    take_count: usize,
) -> (RecordingSessionId, Vec<TakeId>, CaptureStreamId) {
    let epoch = with_live(f, |p| *p.clock_epochs.keys().next().unwrap());
    apply_recording(
        f,
        RecordingOp::CreateSession {
            name: "rcd04-session".into(),
        },
    );
    let session_id = with_live(f, |p| *p.recording.sessions.keys().next().unwrap());
    apply_recording(f, RecordingOp::OpenSession { session_id });
    apply_recording(
        f,
        RecordingOp::RegisterPass {
            session_id,
            index: 0,
        },
    );
    let pass_id = with_live(f, |p| *p.recording.passes.keys().next().unwrap());
    apply_recording(
        f,
        RecordingOp::RegisterStream {
            session_id,
            lane_id: lane,
            clock_epoch_id: epoch,
        },
    );
    let stream_id = with_live(f, |p| *p.recording.streams.keys().next().unwrap());
    let mut takes = Vec::new();
    for i in 0..take_count {
        let lo = i64::try_from(i).unwrap() * 40_000;
        let hi = lo + 30_000;
        apply_recording(
            f,
            RecordingOp::RegisterSegment {
                stream_id,
                pass_id,
                session_range: session_frames(lo, hi),
                source_range: source_frames(lo, hi),
                source_id: source,
            },
        );
        let seg = with_live(f, |p| *p.recording.segments.keys().last().unwrap());
        apply_recording(
            f,
            RecordingOp::RegisterTake {
                session_id,
                pass_id,
                stream_id,
                lane_id: lane,
                source_id: source,
                segment_ids: vec![seg],
            },
        );
        let take = with_live(f, |p| *p.recording.takes.keys().last().unwrap());
        apply_recording(f, RecordingOp::MarkTakeVerified { take_id: take });
        takes.push(take);
    }
    apply_recording(f, RecordingOp::CloseSession { session_id });
    apply_recording(f, RecordingOp::FinalizeSession { session_id });
    (session_id, takes, stream_id)
}

/// group → comp → regions → active → transition, mirroring the
/// `md_a`/`rcd_04_comp_cas` fixture geometry (musical ranges, Natural
/// mapping, EqualPower/Linear boundary fades).
fn build_comp_fixture(
    f: &Fixture,
    lane: LaneId,
    source: SourceId,
    takes: &[TakeId],
) -> (TakeGroupId, CompId, Vec<CompRegionId>, CompTransitionId) {
    assert!(takes.len() >= 2, "comp fixture needs two takes");
    apply_comp(
        f,
        CompOp::RegisterTakeGroup {
            lane_id: lane,
            target_range: mr(0, 2_000),
            take_ids: takes.to_vec(),
            source_rate: RATE,
        },
    );
    let group = with_live(f, |p| *p.comp.take_groups.keys().next().unwrap());
    apply_comp(
        f,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = with_live(f, |p| *p.comp.comps.keys().next().unwrap());
    for (range, candidate) in [
        (
            mr(0, 960),
            CompCandidateRef::Take {
                take_id: takes[0],
                source_offset: SourceFrame::ZERO,
            },
        ),
        (
            mr(960, 1_920),
            CompCandidateRef::ExistingSource {
                source_id: source,
                handle: source_frames(0, 96_000),
                play_from: SourceFrame::new(2_000).unwrap(),
            },
        ),
        (mr(1_920, 2_000), CompCandidateRef::Silence),
    ] {
        apply_comp(
            f,
            CompOp::AddCompRegion {
                comp_id: comp,
                project_range: range,
                candidate,
                mapping: PlaybackMapping::Natural,
            },
        );
    }
    apply_comp(
        f,
        CompOp::SetActiveComp {
            take_group_id: group,
            comp_id: Some(comp),
        },
    );
    let region_ids = with_live(f, |p| p.comp.comps[&comp].region_ids.clone());
    apply_comp(
        f,
        CompOp::AddCompTransition {
            left_region_id: region_ids[0],
            right_region_id: region_ids[1],
            overlap_range: mr(900, 1_020),
            fade_out: FadeShape::EqualPower,
            fade_in: FadeShape::Linear,
        },
    );
    let transition = with_live(f, |p| *p.comp.transitions.keys().next().unwrap());
    (group, comp, region_ids, transition)
}

fn comp_ids(f: &Fixture) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    with_live(f, |p| {
        (
            p.comp.take_groups.keys().map(|id| id.to_tagged()).collect(),
            p.comp.comps.keys().map(|id| id.to_tagged()).collect(),
            p.comp.regions.keys().map(|id| id.to_tagged()).collect(),
            p.comp.transitions.keys().map(|id| id.to_tagged()).collect(),
        )
    })
}

fn recording_snapshot(f: &Fixture) -> Value {
    with_live(f, |p| {
        serde_json::to_value(&p.recording).expect("recording serializes")
    })
}

fn media_snapshot(f: &Fixture) -> Value {
    with_live(f, |p| {
        json!({
            "sources": serde_json::to_value(&p.sources).unwrap(),
            "assets": serde_json::to_value(&p.assets).unwrap(),
        })
    })
}

// ---------- fake VCS provider (vcs04_history.rs conventions) ----------

const REPO: &str = "repo-rcd04";
const PROJECT_PATH: &str = "project/main.json";

fn oid(ch: char) -> String {
    ch.to_string().repeat(40)
}

fn capabilities() -> VcsCapabilities {
    VcsCapabilities {
        object_formats: BTreeSet::from([VcsObjectFormat::Sha1]),
        history: VcsFeature::Supported,
        working_status: VcsFeature::Supported,
        staged_index: VcsFeature::Supported,
        lfs_objects: VcsFeature::Supported,
        submodule_entries: VcsFeature::Supported,
        cancellation: VcsFeature::Supported,
        resource_bounds: VcsFeature::Supported,
    }
}

fn workspace_info() -> WorkspaceInfo {
    WorkspaceInfo {
        repository_id: REPO.into(),
        object_format: VcsObjectFormat::Sha1,
        worktree_root: "/tmp/wt".into(),
        git_dir: "/tmp/wt/.git".into(),
        git_common_dir: "/tmp/wt/.git".into(),
        project_root: String::new(),
        head_commit: Some(oid('b')),
        capabilities: capabilities(),
    }
}

fn fixed_rev(commit_oid: &str) -> FixedVcsRevision {
    FixedVcsRevision {
        repository_id: REPO.into(),
        object_format: VcsObjectFormat::Sha1,
        commit_oid: commit_oid.into(),
        project_root: String::new(),
        context: "ctx".into(),
    }
}

/// Two-commit history: `a` (older) → `b` (HEAD). `exprs` maps
/// revision expressions to commit oids.
struct FakeProvider {
    files: BTreeMap<String, Vec<u8>>,
    exprs: BTreeMap<String, String>,
}

impl FakeProvider {
    fn new(exprs: BTreeMap<String, String>) -> Self {
        Self {
            files: BTreeMap::new(),
            exprs,
        }
    }

    fn put_committed(&mut self, commit_oid: &str, bytes: Vec<u8>) {
        self.files
            .insert(format!("committed:{commit_oid}:{PROJECT_PATH}"), bytes);
    }
}

impl VcsReadProvider for FakeProvider {
    fn detect_workspace(_root: &ProjectPath) -> Result<WorkspaceInfo, VcsError> {
        Ok(workspace_info())
    }

    fn capabilities(&self) -> VcsCapabilities {
        capabilities()
    }

    fn working_status(
        &self,
        _scope: &ProjectScope,
        cancel: &VcsCancel,
    ) -> Result<WorkingStatus, VcsError> {
        if cancel.is_cancelled() {
            return Err(VcsError::Cancelled);
        }
        Ok(WorkingStatus {
            entries: Vec::new(),
            index_generation: IndexGeneration("gen-1".into()),
            head_commit: Some(oid('b')),
        })
    }

    fn list_history(
        &self,
        _query: &HistoryQuery,
        _cancel: &VcsCancel,
    ) -> Result<HistoryPage, VcsError> {
        Ok(HistoryPage {
            nodes: vec![
                HistoryNode {
                    revision: fixed_rev(&oid('b')),
                    parents: vec![fixed_rev(&oid('a'))],
                    committed_at_unix: Some(1_700_000_001),
                    author: Some("tester".into()),
                    message: Some("post".into()),
                    completeness: HistoryCompleteness::Complete,
                },
                HistoryNode {
                    revision: fixed_rev(&oid('a')),
                    parents: vec![],
                    committed_at_unix: Some(1_700_000_000),
                    author: Some("tester".into()),
                    message: Some("pre".into()),
                    completeness: HistoryCompleteness::Complete,
                },
            ],
            next_cursor: None,
        })
    }

    fn resolve_fixed_revision(
        &self,
        selector: &RevisionSelector,
        cancel: &VcsCancel,
    ) -> Result<FixedVcsRevision, VcsError> {
        if cancel.is_cancelled() {
            return Err(VcsError::Cancelled);
        }
        let commit = match selector {
            RevisionSelector::Head => oid('b'),
            RevisionSelector::Expr(expr) => {
                self.exprs
                    .get(expr)
                    .cloned()
                    .ok_or_else(|| VcsError::InvalidRequest {
                        detail: format!("unknown revision expression {expr}"),
                    })?
            }
        };
        Ok(fixed_rev(&commit))
    }

    fn revision_relations(
        &self,
        fixed: &FixedVcsRevision,
        cancel: &VcsCancel,
    ) -> Result<RevisionRelations, VcsError> {
        if cancel.is_cancelled() {
            return Err(VcsError::Cancelled);
        }
        let parents = if fixed.commit_oid == oid('b') {
            vec![fixed_rev(&oid('a'))]
        } else {
            Vec::new()
        };
        Ok(RevisionRelations {
            revision: fixed.clone(),
            parents,
        })
    }

    fn read_revision_file(
        &self,
        _fixed: &FixedVcsRevision,
        _path: &ProjectRelativePath,
        _cancel: &VcsCancel,
    ) -> Result<Vec<u8>, VcsError> {
        Ok(Vec::new())
    }

    fn resolve_staged_snapshot(&self, cancel: &VcsCancel) -> Result<StagedSnapshot, VcsError> {
        if cancel.is_cancelled() {
            return Err(VcsError::Cancelled);
        }
        Ok(StagedSnapshot {
            index_generation: IndexGeneration("gen-1".into()),
            entries: vec![IndexEntry {
                path: PROJECT_PATH.into(),
                stage: 0,
                mode: IndexEntryMode::Regular,
                object_id: oid('b'),
            }],
            conflicted_paths: Vec::new(),
            complete: true,
        })
    }
}

impl VcsBytesPort for FakeProvider {
    fn index_generation(&self, repository_id: &str) -> Result<IndexGeneration, VcsPortError> {
        if repository_id != REPO {
            return Err(VcsPortError::NotFound {
                detail: "unknown repository_id".into(),
            });
        }
        Ok(IndexGeneration("gen-1".into()))
    }

    fn read_file(&self, locator: &VcsLocator, path: &str) -> Result<VcsFileRead, VcsPortError> {
        let key = match locator {
            VcsLocator::Staged { .. } => format!("staged:{path}"),
            VcsLocator::Committed { fixed_revision, .. } => {
                format!("committed:{fixed_revision}:{path}")
            }
        };
        Ok(match self.files.get(&key) {
            Some(bytes) => VcsFileRead::Present(bytes.clone()),
            None => VcsFileRead::Absent,
        })
    }

    fn read_blob(&self, locator: &VcsLocator, digest: &str) -> Result<BlobRead, VcsPortError> {
        let key = match locator {
            VcsLocator::Staged { .. } => format!("staged:{digest}"),
            VcsLocator::Committed { fixed_revision, .. } => {
                format!("committed:{fixed_revision}:{digest}")
            }
        };
        Ok(match self.files.get(&key) {
            Some(bytes) => BlobRead::Present(bytes.clone()),
            None => BlobRead::Absent,
        })
    }
}

fn envelope_bytes(c: &synvelis_project::ProjectController) -> Vec<u8> {
    let mut project = c.live().clone();
    project.id_watermark = c.snapshot_id_watermark();
    serde_json::to_vec(&SnapshotEnvelope {
        schema_version: SCHEMA_VERSION,
        project_id: project.project_id,
        revision: c.live_revision(),
        project,
    })
    .expect("envelope serialize")
}

fn envelope_at(f: &Fixture) -> Vec<u8> {
    let svc = f.svc.borrow();
    envelope_bytes(svc.app().workspace().session().controller())
}

/// Installs the fake VCS backend: `pre` → commit `a`,
/// `post`/`HEAD` → commit `b`.
fn install_vcs(f: &mut Fixture, pre: Vec<u8>, post: Vec<u8>) {
    let mut provider = FakeProvider::new(BTreeMap::from([
        ("pre".to_string(), oid('a')),
        ("post".to_string(), oid('b')),
        ("HEAD".to_string(), oid('b')),
    ]));
    provider.put_committed(&oid('a'), pre);
    provider.put_committed(&oid('b'), post);
    let info = workspace_info();
    let adapter = VcsHistoryAdapter::new(provider, &info, PROJECT_PATH).expect("adapter");
    f.svc
        .borrow_mut()
        .app_mut()
        .set_vcs_backend(Arc::new(adapter));
}

/// Opens a VCS history view through `syn.vcs.visual` and returns its
/// wire `view_id`.
fn vcs_open(rt: &mut UiRuntime) -> String {
    let v = ok(rt, "syn.vcs.visual", json!({ "action": "open" }));
    v["view_id"].as_str().expect("view_id").to_string()
}

fn vcs_action(rt: &mut UiRuntime, view: &str, action: Value) {
    let mut params = action;
    params["view_id"] = json!(view);
    ok(rt, "syn.vcs.visual", params);
}

fn vcs_pin(rt: &mut UiRuntime, view: &str, side: &str, endpoint: &Value) {
    vcs_action(
        rt,
        view,
        json!({ "action": "pin", "side": side, "endpoint": endpoint }),
    );
}

fn vcs_view(rt: &mut UiRuntime, view: &str) -> Value {
    ok(rt, "syn.vcs.view", json!({ "view_id": view }))
}

/// `(kind, stable_id)` pairs for one `object_kind` in the current
/// selection diff.
fn diff_changes(view: &Value, object_kind: &str) -> Vec<(String, String)> {
    view["selection"]["diff"]["changes"]
        .as_array()
        .expect("diff changes")
        .iter()
        .filter(|c| c["object_kind"] == object_kind)
        .map(|c| {
            (
                c["kind"].as_str().unwrap().to_string(),
                c["stable_id"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

// =====================================================================
// E2E-1: Comp Transition — product op path
// =====================================================================

#[test]
#[allow(clippy::too_many_lines)] // sequential e2e scenario
fn rcd04_comp_transition_product_e2e() {
    let dir = tmp_dir("comp");
    let mut f = fixture();
    project_new(&mut f, &dir);

    // -- substrate: track/2 lanes/epoch/source + finalized session
    //    with two verified takes on lane_a (all through synvelis ops)
    let (lanes, source) = seed_media_substrate(&f, 2);
    let lane_a = lanes[0];
    let (_session, takes, _stream) = seed_verified_takes(&f, lane_a, source, 2);

    // committed(pre-comp) envelope for the later VCS diff.
    let env_pre_comp = envelope_at(&f);

    // recording projection shows the committed take registry.
    let info = ok(&mut f.rt, "syn.recording.info", json!({}));
    assert_eq!(info["counts"]["takes"], 2);
    assert_eq!(info["counts"]["sessions"], 1);
    assert_eq!(info["sessions"][0]["state"], "finalized");
    assert_eq!(info["takes"][0]["state"], "verified");

    // raw media/take/source/asset snapshot before comp ops.
    let rec_before = recording_snapshot(&f);
    let media_before = media_snapshot(&f);
    let rev_before_comp = session_rev(&f);

    // -- comp ops: 7 undoable mutations --
    let (group, comp, regions, transition) = build_comp_fixture(&f, lane_a, source, &takes);
    let expected = (
        vec![group.to_tagged()],
        vec![comp.to_tagged()],
        regions.iter().map(|r| r.to_tagged()).collect::<Vec<_>>(),
        vec![transition.to_tagged()],
    );
    assert_eq!(comp_ids(&f), expected);

    // Comp ops never touch takes/sources/assets/segments.
    assert_eq!(recording_snapshot(&f), rec_before);
    assert_eq!(media_snapshot(&f), media_before);

    // -- CAS: stale expected_revision is rejected atomically --
    let stale = {
        let mut svc = f.svc.borrow_mut();
        let session = svc.app_mut().workspace_mut().session_mut();
        let rev = session.revision();
        session
            .apply(
                SynvelisOp::Comp(CompOp::CreateComp {
                    take_group_id: group,
                }),
                ProjectRevision(rev.0 + 1_000),
            )
            .expect_err("stale revision must be rejected")
    };
    assert!(matches!(
        stale,
        synvelis_project::Error::StaleRevision { .. }
    ));
    // project unchanged: still exactly the seeded comp entities.
    assert_eq!(comp_ids(&f), expected);
    assert_eq!(session_rev(&f).0, rev_before_comp.0 + 7);

    // -- undo/redo through syn.project.* ops: 7 comp ops --
    for _ in 0..7 {
        let v = ok(&mut f.rt, "syn.project.undo", json!({}));
        assert_eq!(v["undone"], true);
    }
    // comp registry empty; takes/recording untouched.
    assert_eq!(
        comp_ids(&f),
        (
            Vec::<String>::new(),
            Vec::<String>::new(),
            Vec::<String>::new(),
            Vec::<String>::new()
        )
    );
    assert_eq!(recording_snapshot(&f), rec_before);
    for _ in 0..7 {
        let v = ok(&mut f.rt, "syn.project.redo", json!({}));
        assert_eq!(v["redone"], true);
    }
    // identical stable IDs after undo→redo (no ID churn).
    assert_eq!(comp_ids(&f), expected);
    assert_eq!(recording_snapshot(&f), rec_before);
    assert_eq!(media_snapshot(&f), media_before);

    // -- save / reload: stable IDs survive the bundle round-trip --
    ok(&mut f.rt, "syn.project.save", json!({}));
    ok(&mut f.rt, "syn.project.reload", json!({}));
    assert_eq!(comp_ids(&f), expected);
    let info = ok(&mut f.rt, "syn.recording.info", json!({}));
    assert_eq!(info["counts"]["takes"], 2);

    // -- VCS diff: committed(pre-comp) vs committed(post-comp) shows
    //    every comp entity Added under its stable ID; committed(post)
    //    vs live is equal — undo/redo/save/reload kept identity --
    let env_post = envelope_at(&f);
    install_vcs(&mut f, env_pre_comp, env_post);
    let view = vcs_open(&mut f.rt);
    vcs_action(&mut f.rt, &view, json!({ "action": "refresh" }));

    vcs_pin(
        &mut f.rt,
        &view,
        "before",
        &json!({ "kind": "committed", "expr": "pre" }),
    );
    vcs_pin(
        &mut f.rt,
        &view,
        "after",
        &json!({ "kind": "committed", "expr": "post" }),
    );
    let v = vcs_view(&mut f.rt, &view);
    for (kind_obj, want) in [
        ("TakeGroup", &expected.0[..]),
        ("Comp", &expected.1[..]),
        ("CompRegion", &expected.2[..]),
        ("CompTransition", &expected.3[..]),
    ] {
        let got: Vec<String> = diff_changes(&v, kind_obj)
            .into_iter()
            .filter(|(kind, _)| kind == "Added")
            .map(|(_, id)| id)
            .collect();
        assert_eq!(got, want, "{kind_obj} stable IDs in diff");
    }

    // read-only toward the project: revision unchanged by vcs ops.
    let rev_after_diff = session_rev(&f);
    vcs_action(&mut f.rt, &view, json!({ "action": "clear" }));
    vcs_pin(
        &mut f.rt,
        &view,
        "before",
        &json!({ "kind": "committed", "expr": "post" }),
    );
    vcs_pin(&mut f.rt, &view, "after", &json!({ "kind": "live" }));
    let v = vcs_view(&mut f.rt, &view);
    let d = &v["selection"]["diff"];
    // undo/redo/save/reload kept every stable ID: zero entity-level
    // changes between the committed(post) snapshot and live.
    assert_eq!(d["summary"]["total"], 0, "diff summary: {d}");
    assert_eq!(d["changes"].as_array().unwrap().len(), 0);
    // the fake asset digest keeps content resolution `unknown` — the
    // honest tri-state, never coerced to `equal` or `different`.
    assert_ne!(d["overall"], "different");
    assert_eq!(session_rev(&f), rev_after_diff);

    fs::remove_dir_all(&dir).ok();
}

// =====================================================================
// E2E-2: MIDI Phrase — plan-level boundary safety through product ops
// =====================================================================

fn mt(tick: i64) -> MidiTick {
    MidiTick::new(tick).unwrap()
}

fn midi_clip() -> (ClipPlacement, ClipPlayback) {
    (
        ClipPlacement {
            anchor: TimeAnchor::Musical(MusicalTick::ZERO),
            extent: ClipExtent::Musical(MusicalDuration::new(8 * 960).unwrap()),
        },
        ClipPlayback {
            source_region: source_frames(0, 8 * 960),
            mapping: PlaybackMapping::Natural,
        },
    )
}

fn add_midi_note(f: &Fixture, source: SourceId, key: u8, vel: u8, start: i64, len: i64) {
    apply_project(
        f,
        ProjectOp::AddMidiNote {
            source_id: source,
            key,
            velocity: vel,
            start: mt(start),
            len: mt(len),
        },
    );
}

fn midi_selection(source: SourceId) -> MidiCompSelection {
    MidiCompSelection {
        regions: vec![
            MidiPhraseRegion {
                id: MidiCompRegionId::new(101),
                source_id: source,
                phrase: MidiRange::try_new(mt(480), mt(2400)).unwrap(),
                project_range: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(3_840))
                    .unwrap(),
            },
            MidiPhraseRegion {
                id: MidiCompRegionId::new(102),
                source_id: source,
                phrase: MidiRange::try_new(mt(2400), mt(3840)).unwrap(),
                project_range: MusicalRange::try_new(
                    MusicalTick::new(3_840),
                    MusicalTick::new(6_720),
                )
                .unwrap(),
            },
        ],
    }
}

fn midi_lanes(source: SourceId) -> MidiControllerLanes {
    let ev = |tick: i64, kind: MidiControllerKind, value: i32| MidiControllerEvent {
        tick: mt(tick),
        channel: 0,
        kind,
        value,
    };
    MidiControllerLanes {
        lanes: vec![MidiControllerLane {
            source_id: source,
            events: vec![
                ev(100, MidiControllerKind::Cc(64), 127),
                ev(600, MidiControllerKind::Cc(1), 64),
                ev(700, MidiControllerKind::PitchBend, 10_000),
                ev(2_000, MidiControllerKind::Cc(64), 64),
                ev(2_500, MidiControllerKind::Cc(10), 32),
                ev(3_000, MidiControllerKind::Cc(64), 0),
            ],
        }],
    }
}

/// Boundary oracle: every `NoteOn` pairs with exactly one `NoteOff` at a
/// tick >= the on for the same `(source, note)` identity; the plan
/// ends with `AllNotesOff` and an empty active set.
fn assert_plan_boundary_safe(plan: &MidiPlaybackPlan) {
    let mut active: Vec<(SourceId, NoteId)> = Vec::new();
    let mut ons = 0usize;
    let mut offs = 0usize;
    for ev in &plan.events {
        match &ev.action {
            MidiPlanAction::NoteOn { .. } => {
                let id = ev.note.expect("note-on carries identity");
                assert!(!active.contains(&id), "note {id:?} double-triggered");
                active.push(id);
                ons += 1;
            }
            MidiPlanAction::NoteOff { .. } => {
                let id = ev.note.expect("note-off carries identity");
                let pos = active
                    .iter()
                    .position(|a| *a == id)
                    .unwrap_or_else(|| panic!("note-off without note-on: {id:?}"));
                active.remove(pos);
                offs += 1;
            }
            _ => {}
        }
    }
    assert_eq!(ons, offs, "every on must pair with an off");
    assert!(active.is_empty(), "hanging notes at plan end: {active:?}");
    let last = plan.events.last().expect("plan has events");
    assert_eq!(last.action, MidiPlanAction::AllNotesOff);
}

fn compile_plan(f: &Fixture) -> MidiPlaybackPlan {
    with_live(f, |p| {
        let source = *p.midi_sources.keys().next().expect("midi source");
        compile_midi_comp(p, &midi_selection(source), &midi_lanes(source))
            .expect("midi comp compiles")
    })
}

fn note_ids(f: &Fixture, source: SourceId) -> Vec<(u8, String)> {
    with_live(f, |p| {
        let mut rows: Vec<(u8, String)> = p.midi_sources[&source]
            .notes
            .values()
            .map(|n| (n.key, n.id.to_tagged()))
            .collect();
        rows.sort();
        rows
    })
}

#[test]
#[allow(clippy::too_many_lines)] // sequential e2e scenario
fn rcd04_midi_phrase_product_e2e() {
    let dir = tmp_dir("midi");
    let mut f = fixture();
    project_new(&mut f, &dir);

    // substrate: track → lane → MIDI source (division 480, project
    // PPQN 960 → native delta ×2 = project tick, exact) → clip.
    apply_project(
        &f,
        ProjectOp::AddTrack {
            name: "midi".into(),
        },
    );
    let track = with_live(&f, |p| *p.tracks.keys().next().unwrap());
    apply_project(&f, ProjectOp::AddLane { track_id: track });
    let lane = with_live(&f, |p| *p.lanes.keys().next().unwrap());
    apply_project(
        &f,
        ProjectOp::AddMidiSource {
            division: MidiDivision::ppqn(480).unwrap(),
        },
    );
    let source = with_live(&f, |p| *p.midi_sources.keys().next().unwrap());
    let (placement, playback) = midi_clip();
    apply_project(
        &f,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement,
            playback,
        },
    );

    // committed(pre-notes) envelope for the later VCS diff.
    let env_pre_notes = envelope_at(&f);

    // notes on both sides of the phrase boundary (native ticks).
    for (key, vel, start, len) in [
        (60u8, 100u8, 480i64, 480i64),
        (64, 90, 960, 1800),
        (67, 80, 1500, 300),
        (62, 60, 100, 50),   // onset before phrase A — excluded
        (63, 75, 2399, 10),  // boundary-straddling tiny note
        (65, 85, 2400, 480), // onset exactly at phrase B start
        (72, 70, 2500, 500),
    ] {
        add_midi_note(&f, source, key, vel, start, len);
    }

    let plan = compile_plan(&f);
    assert_plan_boundary_safe(&plan);
    // phrase reports: 2 regions echoing their stable selection ids.
    assert_eq!(plan.regions.len(), 2);
    assert_eq!(plan.regions[0].region, MidiCompRegionId::new(101));
    assert_eq!(plan.regions[1].region, MidiCompRegionId::new(102));
    // virtual offs cut boundary-straddling notes (no hanging note).
    assert!(
        plan.regions.iter().any(|r| !r.virtual_offs.is_empty()),
        "expected at least one virtual off at a phrase boundary"
    );
    // controller chase covers the declared kind union per region.
    for region in &plan.regions {
        assert!(
            !region.chase.is_empty(),
            "region {} has no chase",
            region.region
        );
    }

    // stable note identities before undo.
    let ids_before = note_ids(&f, source);
    assert_eq!(ids_before.len(), 7);

    // undo the last AddMidiNote → plan loses exactly that note's
    // events; redo restores the same NoteIds (stable across history).
    let undone_key = 72u8;
    let v = ok(&mut f.rt, "syn.project.undo", json!({}));
    assert_eq!(v["undone"], true);
    let plan_after_undo = compile_plan(&f);
    assert_plan_boundary_safe(&plan_after_undo);
    assert!(
        plan_after_undo.events.iter().all(|e| {
            e.action
                != MidiPlanAction::NoteOn {
                    key: undone_key,
                    velocity: 0,
                }
                && !(matches!(&e.action, MidiPlanAction::NoteOn { key, .. } if *key == undone_key))
        }),
        "undone note must leave the plan"
    );
    let v = ok(&mut f.rt, "syn.project.redo", json!({}));
    assert_eq!(v["redone"], true);
    assert_eq!(
        note_ids(&f, source),
        ids_before,
        "note IDs stable across undo/redo"
    );
    let plan_redone = compile_plan(&f);
    assert_eq!(plan_redone, plan, "plan deterministic across undo/redo");

    // save / reload: MIDI source + note stable IDs persist.
    ok(&mut f.rt, "syn.project.save", json!({}));
    ok(&mut f.rt, "syn.project.reload", json!({}));
    assert_eq!(note_ids(&f, source), ids_before);
    let source_reloaded = with_live(&f, |p| *p.midi_sources.keys().next().unwrap());
    assert_eq!(source_reloaded, source);
    assert_plan_boundary_safe(&compile_plan(&f));

    // -- VCS diff: committed(pre-notes) vs committed(post-notes)
    //    shows the 7 MidiNote additions under their stable IDs;
    //    committed(post) vs live is equal after reload --
    let env_post = envelope_at(&f);
    install_vcs(&mut f, env_pre_notes, env_post);
    let view = vcs_open(&mut f.rt);
    vcs_action(&mut f.rt, &view, json!({ "action": "refresh" }));

    vcs_pin(
        &mut f.rt,
        &view,
        "before",
        &json!({ "kind": "committed", "expr": "pre" }),
    );
    vcs_pin(
        &mut f.rt,
        &view,
        "after",
        &json!({ "kind": "committed", "expr": "post" }),
    );
    let v = vcs_view(&mut f.rt, &view);
    let note_changes = diff_changes(&v, "MidiNote");
    let added_ids: Vec<String> = note_changes
        .iter()
        .filter(|(kind, _)| kind == "Added")
        .map(|(_, id)| id.clone())
        .collect();
    let mut want_ids: Vec<String> = ids_before.iter().map(|(_, id)| id.clone()).collect();
    want_ids.sort();
    let mut got_ids = added_ids.clone();
    got_ids.sort();
    assert_eq!(got_ids, want_ids, "MidiNote stable IDs in diff");
    assert_eq!(added_ids.len(), 7);

    vcs_action(&mut f.rt, &view, json!({ "action": "clear" }));
    vcs_pin(
        &mut f.rt,
        &view,
        "before",
        &json!({ "kind": "committed", "expr": "post" }),
    );
    vcs_pin(&mut f.rt, &view, "after", &json!({ "kind": "live" }));
    let v = vcs_view(&mut f.rt, &view);
    assert_eq!(v["selection"]["diff"]["overall"], "equal");

    // MIDI comp persistence is plan-level only — deferred to
    // synvelis #126 (event model). The selection document is not a
    // project entity: no comp/midi-comp registry may appear.
    with_live(&f, |p| {
        assert!(p.comp.take_groups.is_empty());
        assert!(p.comp.comps.is_empty());
    });

    fs::remove_dir_all(&dir).ok();
}

// =====================================================================
// E2E-3: Recovery Review — partial + cancel(Hold) + retry(Adopt) →
// product commit + read-only projection
// =====================================================================

const REC_RATE: u32 = 48_000;
const REC_CHANNELS: u32 = 2;

/// Closed recording session pre-registered in the project —
/// `commit_session` resolves each staged stream's
/// `capture_stream_id`/`lane_id` against this registry. Returns the
/// project-issued `(session_id, [(capture_stream_id, lane_id)])`
/// tagged pairs the staging manifest must mirror.
fn seed_commit_session(
    f: &Fixture,
    lanes: &[LaneId],
) -> (RecordingSessionId, Vec<(String, String)>) {
    let epoch = with_live(f, |p| *p.clock_epochs.keys().next().unwrap());
    apply_recording(
        f,
        RecordingOp::CreateSession {
            name: "rcd04-recovery".into(),
        },
    );
    let session_id = with_live(f, |p| *p.recording.sessions.keys().next().unwrap());
    apply_recording(f, RecordingOp::OpenSession { session_id });
    let mut pairs = Vec::new();
    for &lane in lanes {
        apply_recording(
            f,
            RecordingOp::RegisterStream {
                session_id,
                lane_id: lane,
                clock_epoch_id: epoch,
            },
        );
        let stream_id = with_live(f, |p| *p.recording.streams.keys().last().unwrap());
        pairs.push((stream_id.to_tagged(), lane.to_tagged()));
    }
    apply_recording(f, RecordingOp::CloseSession { session_id });
    (session_id, pairs)
}

fn rec_session_dir(root: &Path, session: RecordingSessionId) -> PathBuf {
    root.join(session.to_tagged())
}

fn rec_media_path(root: &Path, session: RecordingSessionId, i: u16) -> PathBuf {
    rec_session_dir(root, session).join(format!("streams/{}.pcm", stream_stem(i)))
}

fn rec_journal_path(root: &Path, session: RecordingSessionId, i: u16) -> PathBuf {
    rec_session_dir(root, session).join(format!("streams/{}.journal", stream_stem(i)))
}

fn rec_finalized_path(root: &Path, session: RecordingSessionId, i: u16) -> PathBuf {
    rec_session_dir(root, session).join(format!("streams/{}.finalized.json", stream_stem(i)))
}

fn rec_review_path(root: &Path, session: RecordingSessionId, i: u16) -> PathBuf {
    rec_session_dir(root, session).join(format!("streams/{}.review.json", stream_stem(i)))
}

fn rec_stream_cfgs(pairs: &[(String, String)]) -> Vec<StagingStreamConfig> {
    pairs
        .iter()
        .enumerate()
        .map(|(i, (capture_stream_id, lane_id))| StagingStreamConfig {
            stream_index: u16::try_from(i).unwrap(),
            capture_stream_id: capture_stream_id.clone(),
            lane_id: lane_id.clone(),
            channels: REC_CHANNELS,
        })
        .collect()
}

fn rec_staging_area(
    root: &Path,
    session: RecordingSessionId,
    pairs: &[(String, String)],
) -> StagingArea {
    StagingArea::create(
        root,
        &StagingSessionConfig {
            session_id: session,
            sample_rate: REC_RATE,
            streams: rec_stream_cfgs(pairs),
        },
    )
    .unwrap()
}

/// crash 相当: finalize せず writer を落とした stream 用の手組み
/// writer（`rcd_04_recovery_partial` と同一の構成）。
fn rec_manual_writer_at(area: &StagingArea, i: u16) -> (StreamWriter, EngineEnds) {
    let (mut engine_ends, writer_ends) = build_handoff(0, 512, REC_CHANNELS, 4, 8, 4);
    while engine_ends.free_rx.try_pop().is_ok() {}
    let fault = FaultLatch::new();
    let m = area
        .manifest
        .streams
        .iter()
        .find(|s| s.stream_index == i)
        .unwrap();
    let cfg = StagingStreamConfig {
        stream_index: m.stream_index,
        capture_stream_id: m.capture_stream_id.clone(),
        lane_id: m.lane_id.clone(),
        channels: m.channels,
    };
    let paths = stream_paths(area, i);
    let media = FaultWriter::new(OpenOptions::new().append(true).open(&paths.media).unwrap());
    let journal = FaultWriter::new(
        OpenOptions::new()
            .append(true)
            .open(&paths.journal)
            .unwrap(),
    );
    (
        StreamWriter::with_writers(
            cfg,
            paths,
            media,
            journal,
            writer_ends,
            fault,
            area.manifest.capabilities.clone(),
        ),
        engine_ends,
    )
}

fn rec_valid_chunk(id: u32, stream_index: u16, start: i64, frames: u32, seed: u32) -> CaptureChunk {
    let pcm = MockPcm { seed };
    let mut c = CaptureChunk::new(id, stream_index, frames.max(64), REC_CHANNELS, 4);
    c.header = CaptureBlockHeader {
        clock_epoch: ClockEpochId::new(22),
        session_start: SessionFrame::new(start).unwrap(),
        first_input_frame: u64::try_from(start).unwrap(),
        block_frames: frames,
        has_capture: true,
    };
    c.spans
        .push(CaptureSpan {
            pass_index: 0,
            session_start: SessionFrame::new(start).unwrap(),
            frame_count: frames,
            packed_pcm_offset: 0,
            source_start: SourceFrame::new(start).unwrap(),
            project_in: MusicalTick::new(start),
            project_out: MusicalTick::new(start + i64::from(frames)),
        })
        .unwrap();
    c.stored_frames = frames;
    c.stored_samples = frames as usize * REC_CHANNELS as usize;
    for f in 0..frames {
        for ch in 0..REC_CHANNELS {
            c.pcm[f as usize * REC_CHANNELS as usize + ch as usize] = pcm.sample(u64::from(f), ch);
        }
    }
    c
}

fn rec_push_and_pump(
    w: &mut StreamWriter,
    ends: &mut EngineEnds,
    stream_index: u16,
    i: u32,
    start: i64,
) {
    assert!(
        ends.filled_tx
            .try_push(rec_valid_chunk(i, stream_index, start, 64, 0))
            .is_ok()
    );
    w.pump();
}

fn rec_append_unverified_media(path: &Path, bytes: usize) {
    let mut m = OpenOptions::new().append(true).open(path).unwrap();
    std::io::Write::write_all(&mut m, &vec![0xCDu8; bytes]).unwrap();
    m.sync_data().unwrap();
}

/// checkpoint 済み prefix + journaled tail の partial stream
/// （戻り値は検証済み frame 数）。
fn rec_partial_stream(
    area: &StagingArea,
    root: &Path,
    session: RecordingSessionId,
    stream: u16,
) -> u64 {
    let (mut w, mut ends) = rec_manual_writer_at(area, stream);
    for i in 0..3u32 {
        rec_push_and_pump(&mut w, &mut ends, stream, i, i64::from(i) * 64);
    }
    w.pump();
    w.checkpoint().unwrap();
    for i in 3..5u32 {
        rec_push_and_pump(&mut w, &mut ends, stream, i, i64::from(i) * 64);
    }
    drop(w);
    rec_append_unverified_media(&rec_media_path(root, session, stream), 512);
    3 * 64
}

/// clean finalize（Complete）した stream — commit 対象。
fn rec_complete_stream(area: &StagingArea, root: &Path, session: RecordingSessionId, stream: u16) {
    let (mut w, mut ends) = rec_manual_writer_at(area, stream);
    for i in 0..4u32 {
        rec_push_and_pump(&mut w, &mut ends, stream, i, i64::from(i) * 64);
    }
    w.pump();
    w.checkpoint().unwrap();
    let r = w
        .finalize(&session.to_tagged(), REC_RATE)
        .expect("finalize");
    assert_eq!(r.outcome, FinalizedOutcome::Complete);
    drop(w);
    assert!(rec_finalized_path(root, session, stream).exists());
}

struct ClearProbe;
impl MediaReferenceProbe for ClearProbe {
    fn check(&self, _c: &DiscardCandidate) -> ReferenceCheck {
        ReferenceCheck::Clear
    }
}

fn rec_review_record(root: &Path, session: RecordingSessionId, i: u16) -> Option<ReviewRecord> {
    fs::read(rec_review_path(root, session, i))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}

#[test]
fn rcd04_recovery_review_product_e2e() {
    let dir = tmp_dir("recovery");
    let mut f = fixture();
    project_new(&mut f, &dir);

    // product substrate: track + lanes + a Closed recording session
    // whose capture streams the staging manifest mirrors (commit
    // resolves `capture_stream_id`/`lane_id` against the registry).
    let (lanes, _source) = seed_media_substrate(&f, 2);
    let (session, pairs) = seed_commit_session(&f, &lanes);
    let rev_before_recovery = session_rev(&f);

    // staging: stream0 = clean finalize (commit target),
    //          stream1 = crashed partial (unverified tail).
    let root = dir.join("staging");
    fs::create_dir_all(&root).unwrap();
    let area = rec_staging_area(&root, session, &pairs);
    rec_complete_stream(&area, &root, session, 0);
    let prefix_frames = rec_partial_stream(&area, &root, session, 1);

    // scan reports both streams honestly.
    let report = recovery::scan(&root).expect("scan");
    assert_eq!(report.sessions.len(), 1);
    assert_eq!(report.sessions[0].streams.len(), 2);

    // --- cancel (Hold) on the partial stream: files byte-identical,
    //     durable review decision, project untouched ---
    let media_before = fs::read(rec_media_path(&root, session, 1)).unwrap();
    let journal_before = fs::read(rec_journal_path(&root, session, 1)).unwrap();
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 1, ReviewAction::Hold, None, &mut faults).unwrap();
    assert_eq!(out, ReviewOutcome::Held);
    assert_eq!(
        fs::read(rec_media_path(&root, session, 1)).unwrap(),
        media_before
    );
    assert_eq!(
        fs::read(rec_journal_path(&root, session, 1)).unwrap(),
        journal_before
    );
    assert!(!rec_finalized_path(&root, session, 1).exists());
    let rec = rec_review_record(&root, session, 1).expect("review record");
    assert_eq!(rec.decision, ReviewDecision::Held);
    // recovery review never mutates the project: revision identical.
    assert_eq!(session_rev(&f), rev_before_recovery);

    // --- retry (Adopt): verified prefix only → Partial finalized,
    //     never promoted to Complete ---
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 1, ReviewAction::Adopt, None, &mut faults).unwrap();
    let ReviewOutcome::Adopted(r) = out else {
        panic!("expected Adopted")
    };
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    assert_eq!(r.source_frames, prefix_frames);
    assert!(rec_finalized_path(&root, session, 1).exists());

    // idempotent re-Adopt: same digest, no double finalize.
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 1, ReviewAction::Adopt, None, &mut faults).unwrap();
    let ReviewOutcome::Adopted(r2) = out else {
        panic!("expected Adopted")
    };
    assert_eq!(r2.digest, r.digest);

    // --- product commit: only the Complete stream registers; the
    //     Partial stream is separated as recoverable (no partial
    //     group commit) ---
    let report = {
        let mut svc = f.svc.borrow_mut();
        let controller = svc.app_mut().workspace_mut().session_mut().controller_mut();
        commit_session(controller, &area).expect("commit_session")
    };
    assert!(
        matches!(report.outcome, CommitOutcome::Applied { .. }),
        "commit outcome: {:?} (recoverable: {:?})",
        report.outcome,
        report.recoverable
    );
    assert_eq!(
        report.takes_committed, 1,
        "one take from the complete stream"
    );
    assert_eq!(report.clips_committed, 1);
    assert_eq!(report.recoverable.len(), 1);
    assert_eq!(report.recoverable[0].stream_index, 1);
    assert_eq!(report.recoverable[0].reason, "partial-finalize");

    // syn.recording.info projects the committed entities. Both
    // lanes' streams were pre-registered; only stream0 received
    // committed media (stream1 stays recoverable, uncommitted).
    let info = ok(&mut f.rt, "syn.recording.info", json!({}));
    assert_eq!(info["counts"]["sessions"], 1);
    assert_eq!(info["counts"]["takes"], 1);
    assert_eq!(info["counts"]["streams"], 2);
    assert_eq!(info["counts"]["segments"], 1);
    assert_eq!(info["sessions"][0]["state"], "finalized");
    assert_eq!(info["takes"][0]["state"], "verified");
    let committed_session = info["sessions"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(committed_session, session.to_tagged());

    // undo removes the whole commit as one step — the pre-registered
    // session/streams stay (they predate the commit), committed
    // takes/segments/sources/clips are gone; redo restores them.
    let v = ok(&mut f.rt, "syn.project.undo", json!({}));
    assert_eq!(v["undone"], true);
    let info = ok(&mut f.rt, "syn.recording.info", json!({}));
    assert_eq!(info["counts"]["takes"], 0);
    assert_eq!(info["counts"]["segments"], 0);
    assert_eq!(info["counts"]["sessions"], 1);
    assert_eq!(info["counts"]["streams"], 2);
    assert_eq!(info["sessions"][0]["state"], "closed");
    let v = ok(&mut f.rt, "syn.project.redo", json!({}));
    assert_eq!(v["redone"], true);
    let info = ok(&mut f.rt, "syn.recording.info", json!({}));
    assert_eq!(info["counts"]["takes"], 1);
    assert_eq!(info["counts"]["segments"], 1);
    assert_eq!(info["sessions"][0]["state"], "finalized");

    // save / reload keeps the committed recording entities + ids.
    ok(&mut f.rt, "syn.project.save", json!({}));
    ok(&mut f.rt, "syn.project.reload", json!({}));
    let info = ok(&mut f.rt, "syn.recording.info", json!({}));
    assert_eq!(info["counts"]["takes"], 1);
    assert_eq!(info["sessions"][0]["id"], committed_session);

    // held stream's files remain for later review (Hold preserved).
    assert!(rec_media_path(&root, session, 1).exists());
    assert!(rec_journal_path(&root, session, 1).exists());

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn rcd04_recovery_corrupt_and_discard_guards_product_e2e() {
    let dir = tmp_dir("recovery-corrupt");
    let mut f = fixture();
    project_new(&mut f, &dir);
    let (lanes, _source) = seed_media_substrate(&f, 1);
    let rev_before = session_rev(&f);

    // review-only fixture: no commit is attempted, so the manifest
    // ids need not resolve against the project registry.
    let session = RecordingSessionId::new(331);
    let pairs = vec![("capture-stream-review".to_string(), lanes[0].to_tagged())];

    let root = dir.join("staging");
    fs::create_dir_all(&root).unwrap();
    let area = rec_staging_area(&root, session, &pairs);
    let (mut w, mut ends) = rec_manual_writer_at(&area, 0);
    rec_push_and_pump(&mut w, &mut ends, 0, 0, 0);
    w.pump();
    w.checkpoint().unwrap();
    drop(w);

    // Discard without a clear probe → fail-closed refusal; media kept.
    let mut faults = PublishFaults::new();
    let err = review_stream(&area, 0, ReviewAction::Discard, None, &mut faults)
        .expect_err("discard without probe must refuse");
    assert!(matches!(err, ReviewError::Refused(_)));
    assert!(rec_media_path(&root, session, 0).exists());
    assert_eq!(session_rev(&f), rev_before);

    // Corrupt the journal → scan must not report Finalized; review
    // actions fail closed or finalize only the verified prefix.
    let jp = rec_journal_path(&root, session, 0);
    let mut jbytes = fs::read(&jp).unwrap();
    let mid = jbytes.len() / 2;
    jbytes[mid] ^= 0xFF;
    fs::write(&jp, &jbytes).unwrap();

    let report = recovery::scan(&root).expect("scan");
    let s = &report.sessions[0].streams[0];
    assert_ne!(s.review_state, ReviewState::Finalized);
    if s.review_state == ReviewState::Unreadable {
        for action in [
            ReviewAction::Adopt,
            ReviewAction::Hold,
            ReviewAction::Discard,
        ] {
            let mut faults = PublishFaults::new();
            assert!(
                review_stream(&area, 0, action, Some(&ClearProbe), &mut faults).is_err(),
                "{action:?} must fail closed on unreadable stream"
            );
        }
    }
    // project revision and raw media untouched by every rejection.
    assert_eq!(session_rev(&f), rev_before);
    assert!(rec_media_path(&root, session, 0).exists());

    fs::remove_dir_all(&dir).ok();
}

// =====================================================================
// E2E-4: stale gesture + read-only history through syn.* ops
// =====================================================================

fn attach(rt: &mut UiRuntime, view: &str, window: &str) -> Value {
    ok(
        rt,
        "syn.view.attach",
        json!({ "view_id": view, "window_id": window }),
    )
}

fn dispatch(rt: &mut UiRuntime, view: &str, cmd: &str, intent: &Value, generation: u64) -> Value {
    ok(
        rt,
        "syn.control.dispatch",
        json!({
            "intent": intent.clone(),
            "ctx": {
                "commandId": cmd,
                "viewId": view,
                "generation": generation,
                "issuedAt": 0u64,
            },
        }),
    )
}

fn transport_action(action: &str) -> Value {
    json!({
        "kind": "action",
        "target": "transport",
        "controlId": "transport-1",
        "sequence": 1u64,
        "action": action,
    })
}

#[test]
fn rcd04_stale_gesture_and_readonly_history_product_e2e() {
    let dir = tmp_dir("stale");
    let mut f = fixture();
    project_new(&mut f, &dir);
    let (lanes, source) = seed_media_substrate(&f, 1);
    let (_s, takes, _st) = seed_verified_takes(&f, lanes[0], source, 2);
    let (_group, _comp, _regions, _transition) = build_comp_fixture(&f, lanes[0], source, &takes);

    // two logical views share the one project session (the same
    // single-writer session two windows would share).
    attach(&mut f.rt, "console-a", "win-1");
    attach(&mut f.rt, "console-b", "win-2");
    drain_all(&f.outbox);

    // a stale-generation gesture is dropped — no receipt is minted,
    // and the project revision does not move.
    let rev = session_rev(&f);
    dispatch(
        &mut f.rt,
        "console-a",
        "cmd-stale",
        &transport_action("play"),
        0,
    );
    assert_eq!(
        notifies(&f.outbox, "syn.control.receipt"),
        Vec::<Value>::new(),
        "stale generation must not mint a receipt"
    );
    assert_eq!(session_rev(&f), rev);

    // the owning view's current-generation command still applies.
    dispatch(
        &mut f.rt,
        "console-a",
        "cmd-ok",
        &transport_action("play"),
        1,
    );
    let receipts = notifies(&f.outbox, "syn.control.receipt");
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0]["receipt"]["status"], "applied");

    // VCS history view is read-only: open/refresh/select/pin/compare
    // never advance the project revision.
    let rev_before_vcs = session_rev(&f);
    let env = envelope_at(&f);
    install_vcs(&mut f, env.clone(), env);
    let view = vcs_open(&mut f.rt);
    vcs_action(&mut f.rt, &view, json!({ "action": "refresh" }));
    vcs_action(
        &mut f.rt,
        &view,
        json!({ "action": "select_node", "index": 1 }),
    );
    vcs_pin(&mut f.rt, &view, "before", &json!({ "kind": "live" }));
    vcs_pin(&mut f.rt, &view, "after", &json!({ "kind": "live" }));
    vcs_action(&mut f.rt, &view, json!({ "action": "compare" }));
    let v = vcs_view(&mut f.rt, &view);
    assert_eq!(v["status"], "ready");
    assert_eq!(session_rev(&f), rev_before_vcs, "vcs ops are read-only");
    // comp entities still present — nothing was mutated away.
    assert!(!with_live(&f, |p| p.comp.take_groups.is_empty()));

    fs::remove_dir_all(&dir).ok();
}
