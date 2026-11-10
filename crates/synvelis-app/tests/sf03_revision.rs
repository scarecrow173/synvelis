//! SF-03 slice 8 acceptance (synvelis#106): the app/workspace
//! same-Revision fixture — every consumer-facing evidence surface
//! must point at the same committed project revision.
//!
//! - `SessionRoutingEngine` receipt `build_key.project_revision`,
//!   `source_map().project_revision`/`generation`, semantic probe
//!   bindings, and `SynvelisApp::signal_flow_view` status all agree
//!   on the revision the live plan was built from.
//! - Semantic node/port IDs in the source map resolve the same
//!   `SemanticNodeRef`s the Signal Flow view projects — no implicit
//!   conversion, no fabricated runtime identity.
//! - A viewstate-only op changes nothing outside `view_generation`;
//!   a semantic no-op revision is `Equivalent` at the engine (no new
//!   generation) while the projection honestly marks the older
//!   generation-scoped map `stale`.
//! - A controller-bypassed commit surfaces as reconcile drift
//!   (`ProjectRevision` mismatch + fault), never silent reuse.
//! - `save` publishes a snapshot HEAD — it is persistence evidence,
//!   never runtime activation evidence.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_ui::BackendReport;
use serde_json::json;

use synvelis_app::{SessionEngineConfig, SessionRoutingEngine, SynvelisApp, main_view_id};
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::routing_lower::GainProcessor;
use synvelis_player::{
    ProcessorConnectError, ProcessorConnectRequest, ProcessorConnection, ProcessorHost,
    ProcessorLease,
};
use synvelis_project::{
    ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, InMemoryStore, LaneId, MediaDuration,
    MediaTimeValue, PlaybackMapping, ProcessorBinding, ProcessorId, ProjectOp, ProjectRevision,
    RoutingActivationState, RoutingReconcileMismatch, SemanticNodeRef, SemanticProbePoint,
    SemanticSignalEntityRef, SignalBoundary, SourceFrame, SourceId, SourceRange, StripId,
    TimeAnchor, TrackId,
};
use synvelis_workspace::EnginePort;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf03s8-{}-{}-{}",
        name,
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn apply(c: &mut synvelis_project::ProjectController, op: ProjectOp) -> ProjectRevision {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .unwrap()
}

fn media_placement(start_frame: u64, end_frame: u64, rate: u32) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Absolute(
            MediaTimeValue::new(i64::try_from(start_frame).unwrap(), 1, rate).unwrap(),
        ),
        extent: ClipExtent::Absolute(
            MediaDuration::new(i64::try_from(end_frame - start_frame).unwrap(), 1, rate).unwrap(),
        ),
    }
}

fn natural_playback(offset: i64, frames: i64) -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::new(offset).unwrap(),
            SourceFrame::new(offset + frames).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

fn session_config(media_root: PathBuf) -> SessionEngineConfig {
    SessionEngineConfig {
        media_root,
        engine: synvelis_player::lowering::EngineConfig {
            sample_rate: 48_000,
            channels: 2,
            block_frames: 256,
        },
        prefetch: ruviform_media_reader::PrefetchConfig {
            block_frames: 256,
            block_count: 8,
            channels: 2,
            sample_rate: 48_000,
        },
        budget: synvelis_project::BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
        mode: synvelis_project::BuildMode::Realtime,
        runtime_instance: synvelis_project::RuntimeInstanceId(1),
        device_clock_epoch: synvelis_project::RuntimeClockEpochRef(1),
        engine_config_generation: 1,
    }
}

fn import_stereo(
    c: &mut synvelis_project::ProjectController,
    root: &Path,
    name: &str,
    frames: usize,
) -> synvelis_player::ImportOutcome {
    let wav = root.join(name);
    write_fixture(&wav, 2, 48_000, frames).unwrap();
    import_local(
        c,
        &ImportRequest {
            path: wav,
            staging_root: root.join("staging"),
            media_root: root.join("media"),
            resume: None,
        },
    )
    .unwrap()
}

/// Track + strip + lane — returns `(track, strip, lane)`.
fn add_track_strip(
    c: &mut synvelis_project::ProjectController,
    name: &str,
) -> (TrackId, StripId, LaneId) {
    apply(c, ProjectOp::AddTrack { name: name.into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    (track, strip, lane)
}

fn add_clip(
    c: &mut synvelis_project::ProjectController,
    lane: LaneId,
    source: SourceId,
    start: u64,
    end: u64,
    offset: i64,
) -> ClipId {
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: media_placement(start, end, 48_000),
            playback: natural_playback(offset, i64::try_from(end - start).unwrap()),
        },
    );
    *c.live().clips.keys().next_back().unwrap()
}

/// Inserts a chain processor with a fixture-resolvable binding.
fn insert_processor(
    c: &mut synvelis_project::ProjectController,
    strip: StripId,
    index: usize,
    name: &str,
) -> ProcessorId {
    apply(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index,
            name: name.into(),
            binding: ProcessorBinding::Resolved(synvelis_project::PluginRef {
                descriptor: format!("vendor.{name}"),
                version: "1.0".into(),
                capability_revision: 1,
            }),
            sidechain_input: false,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

/// A processor host that vends a fresh `GainProcessor` per connect.
/// `StaticProcessorHost` consumes its instance on `connect`, which
/// only supports a single semantic build — this fixture drives
/// several (initial build, post-bypass rebuild), so it needs a host
/// that can vend repeatedly like a real plugin host.
struct FreshGainHost;

impl ProcessorHost for FreshGainHost {
    fn connect(
        &mut self,
        _request: ProcessorConnectRequest,
    ) -> Result<ProcessorConnection, ProcessorConnectError> {
        Ok(ProcessorConnection {
            processor: Box::new(GainProcessor::new(0.5)),
            lease: ProcessorLease::new(|| {}),
            control: None,
            state_handle: None,
        })
    }
}

/// App + engine + a committed routing fixture: one imported stereo
/// source, one track strip with a clip and one chain processor
/// (track strips route to the master bus implicitly). Setup
/// mutations go through the controller directly
/// (pre-session-observation); the engine's first build is the
/// `session.apply` after attach.
/// Returns `(app, engine, track_strip, processor)`.
fn app_with_engine(root: &Path) -> (SynvelisApp, SessionRoutingEngine, StripId, ProcessorId) {
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let (track_strip, processor) = {
        let c = app.workspace_mut().session_mut().controller_mut();
        let out = import_stereo(c, root, "a.wav", 4_800);
        let (_track, track_strip, lane) = add_track_strip(c, "vox");
        add_clip(c, lane, out.commit.source_id, 0, 480, 0);
        let processor = insert_processor(c, track_strip, 0, "gain");
        (track_strip, processor)
    };
    let engine = SessionRoutingEngine::new(session_config(root.join("media")));
    engine.set_processor_host(Box::new(FreshGainHost));
    app.attach_routing_engine(engine.clone());
    // First notification → prepare + publish at the live revision.
    let rev = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "sf03s8".into(),
            },
            rev,
        )
        .unwrap();
    (app, engine, track_strip, processor)
}

fn activate(engine: &SessionRoutingEngine) -> u64 {
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    match engine.receipt().expect("receipt").state {
        RoutingActivationState::Active { generation, .. } => generation,
        ref other => panic!("expected Active, got {other:?}"),
    }
}

fn master_strip(app: &SynvelisApp) -> StripId {
    let project = app.workspace().session().project();
    project.routing.buses[&project.routing.master].strip
}

fn probe_target(app: &SynvelisApp) -> String {
    app.workspace()
        .session()
        .project()
        .control
        .targets
        .keys()
        .next()
        .expect("control target")
        .to_tagged()
}

fn request_probe(app: &mut SynvelisApp, strip: StripId) {
    let point = serde_json::to_value(SemanticProbePoint {
        entity: SemanticSignalEntityRef::Strip(strip),
        boundary: SignalBoundary::PostFader,
        port: None,
    })
    .unwrap();
    app.op_probe_control(&json!({
        "action": "request",
        "target": probe_target(app),
        "point": point,
        "kind": "metrics",
        "mode": "monitor",
    }))
    .expect("probe request");
}

/// Every evidence surface names the same committed revision: the
/// activation receipt's build key, the generation-scoped source
/// map, the semantic probe binding, and the Signal Flow view's
/// status block — plus per-node `Resolved` runtime scoped to that
/// same generation.
#[test]
fn same_committed_revision_across_receipt_map_probe_and_view() {
    let root = scratch("same-rev");
    let (mut app, engine, track_strip, processor) = app_with_engine(&root);
    let generation = activate(&engine);
    let rev = app.workspace().session().revision();

    // Engine-side evidence: receipt key and source map agree on the
    // committed revision the plan was built from.
    let receipt = engine.receipt().expect("receipt");
    let key = receipt.build_key.clone().expect("build key");
    assert_eq!(key.project_revision, rev);
    assert_eq!(receipt.published_generation, Some(generation));
    let map = engine.source_map().expect("source map");
    assert_eq!(map.project_revision, rev);
    assert_eq!(map.generation, generation);
    assert_eq!(map.build_key.project_revision, rev);
    // Semantic IDs are the project's own — no runtime-local aliases.
    assert!(
        map.semantic_nodes
            .contains_key(&SemanticNodeRef::Strip(track_strip))
    );
    assert!(
        map.semantic_nodes
            .contains_key(&SemanticNodeRef::Processor(processor))
    );

    // Probe evidence binds the same revision and generation.
    request_probe(&mut app, track_strip);
    let bindings = engine.probe_bindings();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].project_revision, rev);
    assert_eq!(bindings[0].active_plan_generation, generation);

    // Processor nodes only project under Expanded scope.
    app.op_signal_flow_visual(&json!({"action": "set_scope", "scope": "expanded"}))
        .expect("set_scope");

    // App projection: live revision, source-map revision, probe
    // revision, and activation state all point at `rev`.
    let view = app.signal_flow_view(&main_view_id());
    assert_eq!(view.project_revision, rev);
    assert_eq!(view.status.project_revision, rev);
    assert!(view.status.engine_attached);
    assert!(!view.status.stale);
    assert_eq!(view.status.source_map_revision, Some(rev));
    assert_eq!(view.status.source_map_generation, Some(generation));
    assert!(matches!(
        view.status.activation,
        Some(RoutingActivationState::Active {
            project_revision,
            generation: g,
            ..
        }) if project_revision == rev && g == generation
    ));
    assert_eq!(view.status.probes.len(), 1);
    assert_eq!(view.status.probes[0].project_revision, rev);
    assert_eq!(view.status.probes[0].generation, generation);

    // Nodes the project routing model owns resolve through the same
    // generation-scoped map — Resolved carries the map's generation,
    // never a fabricated identity.
    for (node_id, semantic) in [
        (
            synvelis_workspace::SignalFlowNodeId::Strip(track_strip),
            SemanticNodeRef::Strip(track_strip),
        ),
        (
            synvelis_workspace::SignalFlowNodeId::Processor(processor),
            SemanticNodeRef::Processor(processor),
        ),
        (
            synvelis_workspace::SignalFlowNodeId::Strip(master_strip(&app)),
            SemanticNodeRef::Strip(master_strip(&app)),
        ),
    ] {
        let node = view
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .unwrap_or_else(|| panic!("view must project {semantic:?}"));
        assert!(
            map.semantic_nodes.contains_key(&semantic),
            "source map must key {semantic:?}"
        );
        assert_eq!(
            node.runtime,
            synvelis_workspace::SignalFlowNodeRuntime::Resolved { generation },
            "{semantic:?} must resolve at generation {generation}"
        );
    }
}

/// Viewstate ops never touch domain truth; a semantic no-op
/// revision is engine-`Equivalent` (same active generation) while
/// the projection marks the older generation-scoped map `stale` —
/// equivalence is not silently rewritten into fresh evidence.
#[test]
fn viewstate_and_semantic_noop_keep_revision_axes_honest() {
    let root = scratch("noop");
    let (mut app, engine, track_strip, _processor) = app_with_engine(&root);
    let generation = activate(&engine);
    request_probe(&mut app, track_strip);
    let built_rev = app.workspace().session().revision();

    // Viewstate-only op: revision, project bytes, build count and
    // receipt are untouched; only view_generation advances.
    let receipt_before = format!("{:?}", engine.receipt());
    let ack = app
        .op_signal_flow_visual(&json!({"action": "zoom", "zoom": 1.5}))
        .expect("visual op");
    assert_eq!(ack["ok"], json!(true));
    {
        let session = app.workspace().session();
        assert_eq!(session.revision(), built_rev);
    }
    assert_eq!(engine.graph_builds(), 1);
    assert_eq!(format!("{:?}", engine.receipt()), receipt_before);

    // Semantic no-op: rename commits a new revision, the engine is
    // notified but resolves Equivalent — no new generation, the
    // receipt still describes the plan built from `built_rev`.
    let live_before = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "renamed".into(),
            },
            live_before,
        )
        .unwrap();
    let live_rev = app.workspace().session().revision();
    assert_ne!(live_rev, built_rev);
    assert_eq!(engine.graph_builds(), 2);
    let receipt = engine.receipt().expect("receipt");
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Active { generation: g, .. } if g == generation
    ));
    assert_eq!(
        receipt.build_key.as_ref().unwrap().project_revision,
        built_rev
    );
    assert_eq!(engine.source_map().unwrap().project_revision, built_rev);

    // The probe binding keeps the revision it was requested against.
    assert_eq!(engine.probe_bindings()[0].project_revision, built_rev);

    // The projection is honest: live revision moved, generation-
    // scoped evidence still names `built_rev`, so it reads stale —
    // never presented as current.
    let view = app.signal_flow_view(&main_view_id());
    assert_eq!(view.project_revision, live_rev);
    assert_eq!(view.status.project_revision, live_rev);
    assert_eq!(view.status.source_map_revision, Some(built_rev));
    assert!(view.status.stale);
    assert!(
        view.nodes
            .iter()
            .all(|n| n.runtime == synvelis_workspace::SignalFlowNodeRuntime::Stale)
    );
    assert_eq!(view.status.probes[0].project_revision, built_rev);
}

/// A commit that bypasses the session (direct controller apply)
/// advances the live revision without an engine notification —
/// reconcile reports the drift and the view marks evidence stale
/// until the next session apply rebuilds at the live revision.
#[test]
fn controller_bypass_surfaces_as_reconcile_drift_then_converges() {
    let root = scratch("bypass");
    let (mut app, engine, _track_strip, _processor) = app_with_engine(&root);
    activate(&engine);
    let built_rev = app.workspace().session().revision();
    let builds_before = engine.graph_builds();

    // Bypass: commit a semantic change straight through the
    // controller — the EnginePort is never notified.
    let (lane, source) = {
        let project = app.workspace().session().project();
        let lane = *project
            .tracks
            .values()
            .next()
            .unwrap()
            .lane_ids
            .last()
            .unwrap();
        let source = project.clips.values().next().unwrap().source_id;
        (lane, source)
    };
    let bypassed = app
        .workspace_mut()
        .session_mut()
        .controller_mut()
        .apply(Command {
            expected_revision: built_rev,
            op: ProjectOp::AddClip {
                lane_id: lane,
                source_id: source,
                placement: media_placement(480, 960, 48_000),
                playback: natural_playback(0, 480),
            },
        })
        .unwrap();
    assert_ne!(bypassed, built_rev);
    assert_eq!(engine.graph_builds(), builds_before);

    // Reconcile sees the committed revision move past the published
    // key — typed drift, not silent reuse.
    let report = engine.reconcile(app.workspace().session().project(), bypassed);
    assert!(
        report
            .mismatches
            .contains(&RoutingReconcileMismatch::ProjectRevision)
    );
    assert!(report.fault.is_some());
    assert_eq!(report.plan_key.unwrap().project_revision, built_rev);

    // The view still describes the live revision but marks the
    // older generation-scoped map stale.
    let view = app.signal_flow_view(&main_view_id());
    assert_eq!(view.status.project_revision, bypassed);
    assert_eq!(view.status.source_map_revision, Some(built_rev));
    assert!(view.status.stale);

    // The next session.apply rebuilds at the live revision and
    // republishes — evidence converges back to current.
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "converged".into(),
            },
            bypassed,
        )
        .unwrap();
    let generation = activate(&engine);
    let live = app.workspace().session().revision();
    let receipt = engine.receipt().unwrap();
    assert_eq!(receipt.build_key.unwrap().project_revision, live);
    assert_eq!(engine.source_map().unwrap().project_revision, live);
    assert_eq!(engine.source_map().unwrap().generation, generation);
    let view = app.signal_flow_view(&main_view_id());
    assert_eq!(view.status.source_map_revision, Some(live));
    assert!(!view.status.stale);
}

/// `save` publishes an immutable snapshot HEAD — persistence
/// evidence only. It never implies RT activation: a save between
/// publish and `run` leaves the receipt `Published` with no
/// observed active generation, and the saved revision is the same
/// committed revision the build key names.
#[test]
fn save_is_persistence_evidence_not_activation() {
    let root = scratch("save");
    let (mut app, engine, _track_strip, _processor) = app_with_engine(&root);
    let rev = app.workspace().session().revision();
    let published = match engine.receipt().expect("receipt").state {
        RoutingActivationState::Published { generation, .. } => generation,
        ref other => panic!("expected Published, got {other:?}"),
    };

    // Save before any RT observation.
    let mut store = InMemoryStore::new();
    app.workspace_mut()
        .session_mut()
        .controller_mut()
        .save(&mut store)
        .unwrap();
    let saved = app
        .workspace()
        .session()
        .controller()
        .saved()
        .expect("saved state")
        .clone();
    assert_eq!(saved.revision, rev);

    // The save changed nothing on the activation axis: still
    // Published, still no observed active generation, same build
    // count, same revision.
    let receipt = engine.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Published { .. }
    ));
    assert_ne!(receipt.observed_active_generation, Some(published));
    assert_eq!(receipt.build_key.unwrap().project_revision, rev);
    assert_eq!(engine.graph_builds(), 1);
    assert_eq!(app.workspace().session().revision(), rev);
    let view = app.signal_flow_view(&main_view_id());
    assert!(matches!(
        view.status.activation,
        Some(RoutingActivationState::Published { .. })
    ));
    assert_ne!(view.status.observed_active_generation, Some(published));

    // RT observation — and only RT observation — flips Active.
    activate(&engine);
    let receipt = engine.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Active { .. }
    ));
    assert_eq!(receipt.observed_active_generation, Some(published));
    // The earlier save still names the same committed revision.
    assert_eq!(saved.revision, rev);
    assert_eq!(receipt.build_key.unwrap().project_revision, rev);
}
