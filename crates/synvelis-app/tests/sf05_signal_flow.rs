//! SF-05 slice 1 acceptance (synvelis#108): the app-side Signal Flow
//! surface — `syn.signal_flow.view`/`syn.signal_flow.visual` ops, the
//! per-view visual state, and honest engine-evidence status.
//!
//! - SF-UX-02: every visual op leaves project revision, project
//!   state, undo-relevant history, engine graph builds, and the
//!   activation receipt untouched while `view_generation` advances.
//! - SF-UX-05: `Published` (published generation, no observed active
//!   generation) is displayed distinctly from `Active` (RT-observed
//!   generation match); absent engine evidence reads
//!   `engine_attached = false` / `Unobserved`, never fake success.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_ui::BackendReport;
use serde_json::{Value, json};

use synvelis_app::{SessionEngineConfig, SessionRoutingEngine, SynvelisApp, main_view_id};
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::StaticProcessorHost;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::routing_lower::GainProcessor;
use synvelis_project::{
    BusKind, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, ConnectionKind, LaneId,
    MediaDuration, MediaTimeValue, PlaybackMapping, PortDirection, PortRole, ProcessorBinding,
    ProcessorId, Project, ProjectController, ProjectOp, ProjectRevision, RoutingActivationState,
    SemanticPortId, SendTap, SourceFrame, SourceId, SourceRange, StripId, TimeAnchor, TrackId,
};
use synvelis_workspace::{EnginePort, SignalFlowScope, WorkspaceSnapshot};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf05-{}-{}-{}",
        name,
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn apply(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
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

fn engine_config() -> synvelis_player::lowering::EngineConfig {
    synvelis_player::lowering::EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 256,
    }
}

fn prefetch() -> ruviform_media_reader::PrefetchConfig {
    ruviform_media_reader::PrefetchConfig {
        block_frames: 256,
        block_count: 8,
        channels: 2,
        sample_rate: 48_000,
    }
}

fn session_config(media_root: PathBuf) -> SessionEngineConfig {
    SessionEngineConfig {
        media_root,
        engine: engine_config(),
        prefetch: prefetch(),
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
    c: &mut ProjectController,
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

fn add_track(c: &mut ProjectController, name: &str) -> TrackId {
    apply(c, ProjectOp::AddTrack { name: name.into() });
    *c.live().tracks.keys().next_back().unwrap()
}

/// Track + strip + lane — returns (track, strip, lane).
fn add_track_strip(c: &mut ProjectController, name: &str) -> (TrackId, StripId, LaneId) {
    let track = add_track(c, name);
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    (track, strip, lane)
}

fn add_clip(
    c: &mut ProjectController,
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

fn strip_main_port(p: &Project, strip: StripId, direction: PortDirection) -> SemanticPortId {
    p.routing.strips[&strip]
        .port_ids
        .iter()
        .find(|id| {
            let port = &p.routing.ports[id];
            port.direction == direction && port.role == PortRole::Main
        })
        .copied()
        .expect("strip main port")
}

fn bus_strip(p: &Project, bus: synvelis_project::BusId) -> StripId {
    p.routing.buses[&bus].strip
}

/// Adds a bus and returns `(bus id, its strip)`.
fn add_bus(
    c: &mut ProjectController,
    kind: BusKind,
    name: &str,
) -> (synvelis_project::BusId, StripId) {
    apply(
        c,
        ProjectOp::AddBus {
            kind,
            name: name.into(),
        },
    );
    let bus = c
        .live()
        .routing
        .buses
        .values()
        .find(|b| b.name == name)
        .unwrap()
        .id;
    (bus, bus_strip(c.live(), bus))
}

/// Main-kind connection between two strips' main ports.
fn connect_strips(
    c: &mut ProjectController,
    source: StripId,
    destination: StripId,
    kind: ConnectionKind,
) {
    connect(
        c,
        strip_main_port(c.live(), source, PortDirection::Output),
        strip_main_port(c.live(), destination, PortDirection::Input),
        kind,
    );
}

fn connect(
    c: &mut ProjectController,
    source: SemanticPortId,
    destination: SemanticPortId,
    kind: ConnectionKind,
) {
    apply(
        c,
        ProjectOp::AddConnection {
            source,
            destination,
            kind,
            channel_map: None,
        },
    );
}

/// Inserts a chain processor with a fixture-resolvable binding and
/// returns its semantic id.
fn insert_processor(
    c: &mut ProjectController,
    strip: StripId,
    index: usize,
    name: &str,
    sidechain_input: bool,
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
            sidechain_input,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

fn processor_port(
    c: &ProjectController,
    processor: ProcessorId,
    direction: PortDirection,
    role: PortRole,
) -> SemanticPortId {
    c.live().routing.processors[&processor]
        .port_ids
        .iter()
        .find(|id| {
            let p = &c.live().routing.ports[id];
            p.direction == direction && p.role == role
        })
        .copied()
        .expect("processor port")
}

/// The Issue #108 routing fixture on the app's live project —
/// Bass→Music Bus→Master, Kick→Sidechain, Pre/Post sends→Return,
/// a two-processor chain — all through `ProjectOp` on the session's
/// controller (pre-engine setup, so the first engine observation is
/// the first `session.apply` after attach).
fn seed_routing(c: &mut ProjectController) {
    let (_bass, bass_strip, _bass_lane) = add_track_strip(c, "bass");
    let (_kick, kick_strip, _kick_lane) = add_track_strip(c, "kick");
    let (_vox, vox_strip, _vox_lane) = add_track_strip(c, "vox");

    let (_music_bus, music_strip) = add_bus(c, BusKind::Group, "music");
    let (_verb_bus, verb_strip) = add_bus(c, BusKind::Return, "verb");
    let master_strip = bus_strip(c.live(), c.live().routing.master);

    // Bass → Music → Master.
    connect_strips(c, bass_strip, music_strip, ConnectionKind::Main);
    connect_strips(c, music_strip, master_strip, ConnectionKind::Main);

    // Kick → sidechain of a processor on the music strip.
    let sc_proc = insert_processor(c, music_strip, 0, "comp", true);
    let sc_port = processor_port(c, sc_proc, PortDirection::Input, PortRole::Sidechain);
    connect(
        c,
        strip_main_port(c.live(), kick_strip, PortDirection::Output),
        sc_port,
        ConnectionKind::Sidechain,
    );

    // Pre/Post sends → Return bus.
    let verb_in = strip_main_port(c.live(), verb_strip, PortDirection::Input);
    for (source, tap) in [
        (vox_strip, SendTap::PreFader),
        (bass_strip, SendTap::PostFader),
    ] {
        apply(
            c,
            ProjectOp::AddSend {
                source: strip_main_port(c.live(), source, PortDirection::Output),
                destination: verb_in,
                tap,
                gain_target: None,
            },
        );
    }

    // Two-processor chain on the bass strip.
    insert_processor(c, bass_strip, 0, "eq", false);
    insert_processor(c, bass_strip, 1, "comp2", false);
}

/// App + engine + seeded routing/media fixture. Setup mutations go
/// through the controller directly (pre-session-observation); the
/// engine's first build is the `session.apply` inside
/// [`app_with_engine`].
fn app_with_engine(root: &Path) -> (SynvelisApp, SessionRoutingEngine) {
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    {
        let c = app.workspace_mut().session_mut().controller_mut();
        let out = import_stereo(c, root, "a.wav", 4_800);
        let source = out.commit.source_id;
        seed_routing(c);
        // Clips on every lane so the plan has real playbacks.
        let lanes: Vec<LaneId> = c
            .live()
            .tracks
            .values()
            .flat_map(|t| t.lane_ids.iter().copied())
            .collect();
        for lane in lanes {
            add_clip(c, lane, source, 0, 480, 0);
        }
    }
    let engine = SessionRoutingEngine::new(session_config(root.join("media")));
    // Headless fixture: every seeded chain processor binds a plain
    // `GainProcessor` through the static host (connect consumes the
    // instance, which is fine — the fixture performs exactly one
    // semantic build).
    {
        let mut host = StaticProcessorHost::new();
        for id in app
            .workspace()
            .session()
            .project()
            .routing
            .processors
            .keys()
        {
            host.insert(*id, Box::new(GainProcessor::new(0.5)));
        }
        engine.set_processor_host(Box::new(host));
    }
    app.attach_routing_engine(engine.clone());
    // First notification → prepare + publish.
    let rev = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "sf05".into(),
            },
            rev,
        )
        .unwrap();
    (app, engine)
}

fn entity(kind: &str, id: &str) -> Value {
    json!({ "kind": kind, "id": id })
}

fn strip_entity(strip: StripId) -> Value {
    entity("strip", &strip.to_tagged())
}

// ---------------------------------------------------------------------------
// SF-UX-02: visual ops never touch domain/engine truth
// ---------------------------------------------------------------------------

#[test]
fn visual_ops_leave_project_revision_undo_engine_and_receipt_untouched() {
    let root = scratch("visual");
    let (mut app, engine) = app_with_engine(&root);

    let (rev_before, project_before, builds_before, receipt_before) = {
        let session = app.workspace().session();
        (
            session.revision(),
            serde_json::to_value(session.project()).unwrap(),
            engine.graph_builds(),
            format!("{:?}", engine.receipt()),
        )
    };

    let strip = {
        let s = app.workspace().session();
        *s.project().routing.strips.keys().next().unwrap()
    };
    let track = {
        let s = app.workspace().session();
        *s.project().tracks.keys().next().unwrap()
    };

    let ops: Vec<Value> = vec![
        json!({"action": "move_node",
               "entity": strip_entity(strip), "x": 10.0, "y": -4.0}),
        json!({"action": "set_group", "group_id": 7,
               "label": "drums", "members": [entity("track", &track.to_tagged())],
               "collapsed": false}),
        json!({"action": "fold", "entity": strip_entity(strip)}),
        json!({"action": "unfold", "entity": strip_entity(strip)}),
        json!({"action": "zoom", "zoom": 1.25}),
        json!({"action": "pan", "x": 3.0, "y": 4.0}),
        json!({"action": "link_focus",
               "entity": entity("track", &track.to_tagged())}),
        json!({"action": "link_focus", "entity": Value::Null}),
        json!({"action": "set_scope", "scope": "expanded"}),
    ];

    let mut last_generation = 0u64;
    for params in &ops {
        let ack = app.op_signal_flow_visual(params).expect("visual op");
        assert_eq!(ack["ok"], json!(true));
        assert_eq!(ack["view"], json!(main_view_id()));
        let generation = ack["view_generation"].as_u64().unwrap();
        assert!(generation > last_generation, "view_generation must advance");
        last_generation = generation;
    }

    // Domain truth: revision, serialized project (which carries the
    // undo-relevant command history inputs), engine build count, and
    // the activation receipt are all untouched.
    {
        let session = app.workspace().session();
        assert_eq!(session.revision(), rev_before);
        assert_eq!(
            serde_json::to_value(session.project()).unwrap(),
            project_before
        );
    }
    assert_eq!(engine.graph_builds(), builds_before);
    assert_eq!(format!("{:?}", engine.receipt()), receipt_before);

    // The visual state did take effect: the view now projects the
    // Expanded scope chosen by `set_scope`.
    let view_json = app.op_signal_flow_view(&json!({})).unwrap();
    assert_eq!(view_json["scope"], json!("Expanded"));
    assert_eq!(view_json["view_generation"], json!(last_generation));
}

// ---------------------------------------------------------------------------
// SF-UX-05: Published ≠ Active, honest unobserved state
// ---------------------------------------------------------------------------

#[test]
fn published_without_rt_observation_is_not_displayed_as_active() {
    let root = scratch("publish");
    let (app, engine) = app_with_engine(&root);

    // The first build published but nothing has pumped the runtime —
    // the receipt must read Published, never Active.
    let receipt = engine.receipt().expect("receipt after first build");
    let published = match receipt.state {
        RoutingActivationState::Published { generation, .. } => generation,
        ref other => panic!("expected Published, got {other:?}"),
    };
    assert_ne!(receipt.observed_active_generation, Some(published));

    let view = app.signal_flow_view(&main_view_id());
    assert!(matches!(
        view.status.activation,
        Some(RoutingActivationState::Published { .. })
    ));
    assert!(view.status.engine_attached);
    assert_eq!(view.status.published_generation, Some(published));
    assert_ne!(view.status.observed_active_generation, Some(published));
    assert_eq!(
        view.status.source_map_revision,
        Some(app.workspace().session().revision())
    );
    assert!(!view.status.stale);
    // The published source map resolves nodes — nothing claims
    // Unobserved once real evidence exists.
    assert!(view.nodes.iter().any(|n| !matches!(
        n.runtime,
        synvelis_workspace::SignalFlowNodeRuntime::Unobserved
    )));

    // RT observation flips the same projection to Active.
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    let view = app.signal_flow_view(&main_view_id());
    assert!(matches!(
        view.status.activation,
        Some(RoutingActivationState::Active { .. })
    ));
    assert_eq!(view.status.observed_active_generation, Some(published));
}

#[test]
fn absent_engine_reports_unattached_and_unobserved() {
    let app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let view = app.signal_flow_view(&main_view_id());

    assert!(!view.status.engine_attached);
    assert_eq!(view.status.activation, None);
    assert_eq!(view.status.published_generation, None);
    assert!(!view.status.stale);
    assert!(view.nodes.iter().all(|n| matches!(
        n.runtime,
        synvelis_workspace::SignalFlowNodeRuntime::Unobserved
    )));

    // The op surface reports the same honest state.
    let json = app.op_signal_flow_view(&json!({})).unwrap();
    assert_eq!(json["status"]["engine_attached"], json!(false));
    assert_eq!(json["status"]["activation"], Value::Null);
    assert_eq!(json["coverage"], json!("Complete"));
}

#[test]
fn view_op_projects_hierarchy_and_stale_revision_honestly() {
    let root = scratch("viewop");
    let (app, _engine) = app_with_engine(&root);
    let live = app.workspace().session().revision();

    let json = app.op_signal_flow_view(&json!({})).unwrap();
    assert_eq!(json["coverage"], json!("Complete"));
    assert_eq!(json["project_revision"], json!(live.0));
    // Strip nodes for every routing strip; edges 1:1 with persisted
    // connections (send+config never duplicates).
    let routing = &app.workspace().session().project().routing;
    let strip_nodes = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["id"].as_str().unwrap().starts_with("Strip"))
        .count();
    assert_eq!(strip_nodes, routing.strips.len());
    let connection_edges = json["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["id"].as_str().unwrap().starts_with("Connection"))
        .count();
    assert_eq!(connection_edges, routing.connections.len());
    let send_edges = json["edges"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["role"] == "Send")
        .count();
    assert_eq!(send_edges, routing.sends.len());

    // Expanded scope exposes processor nodes.
    let expanded = app
        .op_signal_flow_view(&json!({"scope": "expanded"}))
        .unwrap();
    assert!(
        expanded["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["id"].as_str().unwrap().starts_with("Processor"))
    );

    // Stale expected_revision → Stale coverage, empty graph, live
    // revision still reported.
    let stale = app
        .op_signal_flow_view(&json!({"expected_revision": live.0 + 1}))
        .unwrap();
    assert_eq!(stale["coverage"], json!("Stale"));
    assert_eq!(stale["nodes"].as_array().unwrap().len(), 0);
    assert_eq!(stale["project_revision"], json!(live.0));
}

#[test]
fn visual_set_scope_local_and_role_filter_round_trip() {
    let root = scratch("scopes");
    let (mut app, _engine) = app_with_engine(&root);

    // set_scope local needs a root entity — grab a track strip.
    let strip = {
        let s = app.workspace().session();
        *s.project().routing.strips.keys().next().unwrap()
    };
    let ack = app
        .op_signal_flow_visual(&json!({
            "action": "set_scope", "scope": "local",
            "root": strip_entity(strip),
        }))
        .unwrap();
    assert_eq!(ack["ok"], json!(true));

    let local = app.op_signal_flow_view(&json!({})).unwrap();
    assert_eq!(local["coverage"], json!("Complete"));
    assert!(local["scope"].as_str().unwrap().starts_with("Local"));

    // Role filter through the view op params.
    let sends_only = app
        .op_signal_flow_view(&json!({"scope": "overview", "roles": ["send"]}))
        .unwrap();
    assert!(
        sends_only["edges"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["role"] == "Send")
    );

    // History scope is honestly Unsupported.
    let history = app
        .op_signal_flow_view(&json!({"scope": "history"}))
        .unwrap();
    assert_eq!(history["coverage"], json!("Unsupported"));

    // Unknown action is rejected, not silently accepted.
    assert!(
        app.op_signal_flow_visual(&json!({
            "action": "delete_project",
        }))
        .is_err()
    );
}

// ---------------------------------------------------------------------------
// SF-UX-03: port-drag routing gesture wire surface
// ---------------------------------------------------------------------------

/// The strip owning `track_name`'s main in/out ports.
fn strip_of_track_name(app: &SynvelisApp, name: &str) -> StripId {
    let s = app.workspace().session();
    let p = s.project();
    let track = p
        .tracks
        .values()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("track {name} missing"));
    p.routing.strip_of_track(track.id).unwrap().id
}

/// A fresh stereo Main route pair: vox output → kick input (the
/// seeded fixture leaves kick's main input unconnected).
fn gesture_port_pair(app: &SynvelisApp) -> (SemanticPortId, SemanticPortId) {
    let s = app.workspace().session();
    let p = s.project();
    (
        strip_main_port(p, strip_of_track_name(app, "vox"), PortDirection::Output),
        strip_main_port(p, strip_of_track_name(app, "kick"), PortDirection::Input),
    )
}

fn connection_count(app: &SynvelisApp) -> usize {
    app.workspace()
        .session()
        .project()
        .routing
        .connections
        .len()
}

#[test]
fn signal_flow_routing_gesture_commits_one_atomic_undo() {
    let root = scratch("sf-routing");
    let (mut app, _engine) = app_with_engine(&root);
    let (source, target) = gesture_port_pair(&app);
    let connections_before = connection_count(&app);
    let revision_before = app.workspace().session().revision();

    // begin — the fixation echoes the fixated view, source port, and
    // live project revision.
    let begin = app
        .op_signal_flow_routing(&json!({
            "action": "begin",
            "port": source.to_tagged(),
        }))
        .unwrap();
    assert_eq!(begin["ok"], json!(true));
    assert_eq!(begin["fixation"]["view"], json!(main_view_id()));
    assert_eq!(begin["fixation"]["source"], json!(source.to_tagged()));
    assert_eq!(begin["fixation"]["revision"], json!(revision_before.0));

    // state — the running gesture is observable.
    let state = app
        .op_signal_flow_routing(&json!({ "action": "state" }))
        .unwrap();
    assert_eq!(
        state["gesture"]["fixation"]["source"],
        json!(source.to_tagged())
    );

    // hover — dry-run assessment only; the project is untouched.
    let hover = app
        .op_signal_flow_routing(&json!({
            "action": "hover",
            "port": target.to_tagged(),
        }))
        .unwrap();
    assert_eq!(hover["assessment"]["verdict"], json!("compatible"));
    assert_eq!(connection_count(&app), connections_before);
    assert_eq!(app.workspace().session().revision(), revision_before);

    // drop fixes the candidate; confirm commits one routing op.
    let drop = app
        .op_signal_flow_routing(&json!({ "action": "drop" }))
        .unwrap();
    assert_eq!(drop["dropped"], json!(target.to_tagged()));
    let confirm = app
        .op_signal_flow_routing(&json!({ "action": "confirm" }))
        .unwrap();
    assert_eq!(confirm["outcome"]["outcome"], json!("applied"));
    assert_eq!(
        confirm["outcome"]["revision"],
        json!(revision_before.0 + 1)
    );
    assert_eq!(connection_count(&app), connections_before + 1);

    // The gesture is gone and exactly one undo boundary reverts the
    // whole commit (live revision is monotonic — undo bumps it).
    let state = app
        .op_signal_flow_routing(&json!({ "action": "state" }))
        .unwrap();
    assert_eq!(state["gesture"], Value::Null);
    assert!(app.workspace_mut().session_mut().undo().unwrap());
    assert_eq!(connection_count(&app), connections_before);
}

#[test]
fn signal_flow_routing_cancel_and_stale_revision_are_project_safe() {
    let root = scratch("sf-routing-stale");
    let (mut app, _engine) = app_with_engine(&root);
    let (source, target) = gesture_port_pair(&app);
    let connections_before = connection_count(&app);
    let revision_before = app.workspace().session().revision();

    // begin → hover → cancel: the preview is discarded and nothing
    // reaches the project.
    app.op_signal_flow_routing(&json!({
        "action": "begin",
        "port": source.to_tagged(),
    }))
    .unwrap();
    app.op_signal_flow_routing(&json!({
        "action": "hover",
        "port": target.to_tagged(),
    }))
    .unwrap();
    let cancel = app
        .op_signal_flow_routing(&json!({
            "action": "cancel",
            "reason": "escape",
        }))
        .unwrap();
    assert_eq!(cancel["outcome"]["outcome"], json!("cancelled"));
    assert_eq!(cancel["outcome"]["reason"], json!("escape"));
    assert_eq!(connection_count(&app), connections_before);
    assert_eq!(app.workspace().session().revision(), revision_before);

    // A second gesture whose project revision goes stale mid-flight
    // (concurrent window mutation) drops `stale` at confirm — never
    // a retarget onto the newer revision.
    app.op_signal_flow_routing(&json!({
        "action": "begin",
        "port": source.to_tagged(),
    }))
    .unwrap();
    app.op_signal_flow_routing(&json!({
        "action": "hover",
        "port": target.to_tagged(),
    }))
    .unwrap();
    app.op_signal_flow_routing(&json!({ "action": "drop" }))
        .unwrap();
    let live = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "renamed".into(),
            },
            live,
        )
        .unwrap();
    let confirm = app
        .op_signal_flow_routing(&json!({ "action": "confirm" }))
        .unwrap();
    assert_eq!(confirm["outcome"]["outcome"], json!("stale"));
    assert_eq!(connection_count(&app), connections_before);
    let state = app
        .op_signal_flow_routing(&json!({ "action": "state" }))
        .unwrap();
    assert_eq!(state["gesture"], Value::Null);
}

#[test]
fn signal_flow_routing_rejects_bad_params_and_unknown_actions() {
    let root = scratch("sf-routing-bad");
    let (mut app, _engine) = app_with_engine(&root);
    let (source, _target) = gesture_port_pair(&app);

    // Malformed port id and unknown action are both loud errors.
    assert!(
        app.op_signal_flow_routing(&json!({
            "action": "begin",
            "port": "not-a-port",
        }))
        .is_err()
    );
    assert!(
        app.op_signal_flow_routing(&json!({
            "action": "teleport",
        }))
        .is_err()
    );
    // No gesture running → gesture ops report NoGesture.
    assert!(
        app.op_signal_flow_routing(&json!({ "action": "drop" }))
            .is_err()
    );

    // A begin on a real port succeeds; a second begin is refused
    // (one gesture per view).
    app.op_signal_flow_routing(&json!({
        "action": "begin",
        "port": source.to_tagged(),
    }))
    .unwrap();
    assert!(
        app.op_signal_flow_routing(&json!({
            "action": "begin",
            "port": source.to_tagged(),
        }))
        .is_err()
    );
    app.op_signal_flow_routing(&json!({ "action": "cancel" }))
        .unwrap();
}

// ---------------------------------------------------------------------------
// SF-UX-04: inspection focus wire surface
// ---------------------------------------------------------------------------

#[test]
fn signal_flow_inspection_modes_history_and_follow_source() {
    let root = scratch("sf-inspect");
    let (mut app, _engine) = app_with_engine(&root);
    let (bass_track, kick_track) = {
        let s = app.workspace().session();
        let p = s.project();
        (
            p.tracks.values().find(|t| t.name == "bass").unwrap().id,
            p.tracks.values().find(|t| t.name == "kick").unwrap().id,
        )
    };

    // inspect → Explore mode with the entity as target.
    let r = app
        .op_signal_flow_inspection(&json!({
            "action": "inspect",
            "entity": entity("track", &bass_track.to_tagged()),
        }))
        .unwrap();
    assert_eq!(r["inspection"]["mode"], json!("explore"));
    assert_eq!(r["inspection"]["target"]["kind"], json!("track"));
    assert_eq!(
        r["inspection"]["target"]["id"],
        json!(bass_track.to_tagged())
    );
    assert_eq!(r["inspection"]["explore_back"], json!(0));

    // A second inspect pushes the previous target onto the
    // session-only back stack.
    let r = app
        .op_signal_flow_inspection(&json!({
            "action": "inspect",
            "entity": entity("track", &kick_track.to_tagged()),
        }))
        .unwrap();
    assert_eq!(r["inspection"]["explore_back"], json!(1));

    // back/forward walk the history without touching selection.
    let r = app
        .op_signal_flow_inspection(&json!({ "action": "back" }))
        .unwrap();
    assert_eq!(
        r["inspection"]["target"]["id"],
        json!(bass_track.to_tagged())
    );
    assert_eq!(r["inspection"]["explore_fwd"], json!(1));
    let r = app
        .op_signal_flow_inspection(&json!({ "action": "forward" }))
        .unwrap();
    assert_eq!(
        r["inspection"]["target"]["id"],
        json!(kick_track.to_tagged())
    );

    // Leaving Explore clears the history; Pinned + a Follow-source
    // filter round-trip through the wire.
    let r = app
        .op_signal_flow_inspection(&json!({
            "action": "set_mode",
            "mode": "pinned",
        }))
        .unwrap();
    assert_eq!(r["inspection"]["mode"], json!("pinned"));
    assert_eq!(r["inspection"]["explore_back"], json!(0));
    assert_eq!(r["inspection"]["explore_fwd"], json!(0));

    let r = app
        .op_signal_flow_inspection(&json!({
            "action": "set_mode",
            "mode": "follow",
        }))
        .unwrap();
    assert_eq!(r["inspection"]["mode"], json!("follow"));
    let r = app
        .op_signal_flow_inspection(&json!({
            "action": "set_follow_source",
            "follow_source": { "view": "view-2" },
        }))
        .unwrap();
    assert_eq!(
        r["inspection"]["follow_source"],
        json!({ "view": "view-2" })
    );
    let r = app
        .op_signal_flow_inspection(&json!({
            "action": "set_follow_source",
            "follow_source": Value::Null,
        }))
        .unwrap();
    assert_eq!(r["inspection"]["follow_source"], Value::Null);

    // Unknown action/mode are loud errors.
    assert!(
        app.op_signal_flow_inspection(&json!({
            "action": "set_mode",
            "mode": "sideways",
        }))
        .is_err()
    );
    assert!(
        app.op_signal_flow_inspection(&json!({ "action": "explode" }))
            .is_err()
    );
}

// ---------------------------------------------------------------------------
// SF-UX-07: per-view state separation + snapshot persistence
// ---------------------------------------------------------------------------

#[test]
fn signal_flow_state_is_per_view_and_survives_snapshot_round_trip() {
    let root = scratch("sf-persist");
    let (mut app, _engine) = app_with_engine(&root);

    // view-1 (default) → Expanded; view-3 → Local rooted at a strip.
    // Each visual op lands in that view's workspace ViewState.
    app.op_signal_flow_visual(&json!({
        "action": "set_scope",
        "scope": "expanded",
    }))
    .unwrap();
    let strip = {
        let s = app.workspace().session();
        *s.project().routing.strips.keys().next().unwrap()
    };
    app.op_signal_flow_visual(&json!({
        "view_id": "view-3",
        "action": "set_scope",
        "scope": "local",
        "root": strip_entity(strip),
    }))
    .unwrap();

    // The view op reads each view's own persisted options.
    let v1 = app.op_signal_flow_view(&json!({})).unwrap();
    assert!(v1["scope"].as_str().unwrap().starts_with("Expanded"));
    let v3 = app
        .op_signal_flow_view(&json!({ "view_id": "view-3" }))
        .unwrap();
    assert!(v3["scope"].as_str().unwrap().starts_with("Local"));

    // Workspace capture → JSON → parse preserves both views'
    // signal_flow state (schema v3).
    let snap = app.workspace().capture().unwrap();
    let text = snap.to_json().unwrap();
    let restored = WorkspaceSnapshot::from_json(&text).unwrap();
    assert_eq!(restored.schema, synvelis_workspace::WORKSPACE_SCHEMA);
    let rec1 = restored.views.iter().find(|v| v.id == 1).unwrap();
    let rec3 = restored.views.iter().find(|v| v.id == 3).unwrap();
    assert_eq!(
        rec1.state.signal_flow.as_ref().unwrap().visual.scope,
        SignalFlowScope::Expanded
    );
    assert!(matches!(
        rec3.state.signal_flow.as_ref().unwrap().visual.scope,
        SignalFlowScope::Local { .. }
    ));
}
