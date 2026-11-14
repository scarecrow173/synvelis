//! PC-C4 observe-engine-wiring acceptance (synvelis#101): the
//! production path `SessionRoutingEngine::run` → `RoutingEngine::
//! run_observed` drives `RoutingControlDispatch::observe_block` once
//! per processed block with engine-derived node-cursor windows —
//! the fixture never calls `observe_block`/`observe_cycle` itself.
//! Buffered [`DispatchCycle`]s drain through
//! `control_drain_observations`/`drain_control_observations` into
//! `feed_control_cycle`, and `syn.control.status.observe` exposes
//! the last projected `TransportSpan` plus its plan generation.
//!
//! Fixture mirrors `pc_c4_drain_composition.rs`: the same reporting
//! plugin, project shape and `compose_routing_engine` production
//! composition root.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use ruviform_audio_core::ChannelLayout;
use ruviform_media_core::{
    MediaTime, MidiChannel, MidiEvent, MidiGroup, MidiKind, NoteKey, TimeBase,
};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, Event,
    EventKind, EventList, ParamEvent, ParamId, ParamInfo, ParamValue, Plugin, PluginDescriptor,
    PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult,
    TransportInfo, erase, erase_sendable,
};
use ruviform_ui::ipc::{ErrorBody, Event as UiEvent};
use ruviform_ui::{
    BackendReport, CallOutcome, DataRegistry, OpInfo, Service, ServiceContext, UiRuntime,
};

use synvelis_app::{
    AppService, OPS, ProductPlugin, SessionEngineConfig, SessionRoutingEngine, SynvelisApp,
    compose_routing_engine,
};
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::{FastControlOutcome, FastControlWrite};
use synvelis_project::{
    AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange, BuildBudget,
    BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, ConflictPolicy,
    ControlOwner, Interpolation, LaneId, MediaDuration, MediaTimeValue, ParamMapping,
    ParameterRole, ParameterTargetId, PlaybackMapping, PluginRef, ProcessorBinding, ProcessorId,
    ProjectController, ProjectOp, ProjectRevision, RoutingActivationState, SourceFrame, SourceId,
    SourceRange, StripId, TargetCapability, TimeAnchor, TrackId, ValueKind,
};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-pcwe-{}-{}-{}",
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
        budget: BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
        mode: BuildMode::Realtime,
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

/// Track + strip + lane — returns `(track, strip, lane)`.
fn add_track_strip(c: &mut ProjectController) -> (TrackId, StripId, LaneId) {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
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

fn insert_rec_processor(c: &mut ProjectController, strip: StripId, name: &str) -> ProcessorId {
    apply(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: name.into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: REC_DESCRIPTOR.id.into(),
                version: "0.1.0".into(),
                capability_revision: 0,
            }),
            sidechain_input: false,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

/// A `ProcessorParam` target on `processor` declaring param 7 plus a
/// linear automation lane — identical to the `pc_c4_04`/`pc_c4_06`/`pc_c4_drain` fixture.
fn add_gain_target(c: &mut ProjectController, processor: ProcessorId) -> ParameterTargetId {
    apply(
        c,
        ProjectOp::AddParameterTarget {
            name: "gain".into(),
            owner: ControlOwner::Processor(processor),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(TargetCapability {
                param_id: 7,
                flags_bits: ruviform_plugin_core::ParamFlags::AUTOMATABLE.bits(),
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.0,
            }),
            base: 0.0,
        },
    );
    let target = c
        .live()
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Processor(processor))
        .unwrap()
        .id;
    apply(c, ProjectOp::AddAutomationLane { target });
    let lane = *c
        .live()
        .control
        .lanes
        .values()
        .find(|l| l.target == target)
        .map(|l| &l.id)
        .unwrap();
    apply(
        c,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(0, 1).unwrap(),
                    BeatPoint::new(5, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Linear,
                points: vec![
                    AutomationPoint {
                        pos: BeatPoint::new(0, 1).unwrap(),
                        value: 0.2,
                    },
                    AutomationPoint {
                        pos: BeatPoint::new(4, 1).unwrap(),
                        value: 0.6,
                    },
                ],
            },
            policy: ConflictPolicy::Replace,
        },
    );
    target
}

// ---------------------------------------------------------------- plugins

const STEREO_BUSES: [BusDescriptor; 2] = [
    BusDescriptor {
        direction: BusDirection::Input,
        layout: ChannelLayout::Stereo,
        name: "Main In",
        role: BusRole::Main,
        enabled: true,
    },
    BusDescriptor {
        direction: BusDirection::Output,
        layout: ChannelLayout::Stereo,
        name: "Main Out",
        role: BusRole::Main,
        enabled: true,
    },
];

const REC_DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.pc-c4-observewire-rec",
    name: "PC-C4-ObserveWire Reporter",
    vendor: "Synvelis",
    version: "0.1.0",
    features: &[],
};

const REC_PARAMS: [ParamInfo; 1] = [ParamInfo {
    id: ParamId(7),
    name: "Gain",
    unit: "",
    min_value: 0.0,
    max_value: 1.0,
    default_value: 0.0,
    flags: ruviform_plugin_core::ParamFlags::AUTOMATABLE,
}];

/// The `pc_c4_04`/`pc_c4_06`/`pc_c4_drain` reporting plugin verbatim:
/// announces param 7 = 0.5, unbound param 99 and one Midi report on
/// its first `process`, and echoes every received `ParamValue` back
/// at half value.
struct ReportPlugin {
    pending: Vec<Event>,
    announce: bool,
}

impl PluginProcessor for ReportPlugin {
    fn prepare(
        &mut self,
        _config: &ProcessConfig,
        _context: &mut ProcessContext,
    ) -> Result<(), PrepareError> {
        Ok(())
    }

    fn process<I, O>(
        &mut self,
        _inputs: &[I],
        _outputs: &mut [O],
        events: EventList<'_>,
        _transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: BufferView,
        O: BufferViewMut,
    {
        let mut echoes = Vec::new();
        for ev in events.iter() {
            if let EventKind::ParamValue(pe) = ev.kind() {
                echoes.push(Event::new(
                    ev.frame_offset(),
                    EventKind::ParamValue(ParamEvent {
                        param_id: pe.param_id,
                        value: ParamValue::new(pe.value.get() * 0.5).unwrap(),
                    }),
                ));
            }
        }
        if self.announce {
            self.announce = false;
            self.pending.push(Event::new(
                0,
                EventKind::ParamValue(ParamEvent {
                    param_id: ParamId(7),
                    value: ParamValue::new(0.5).unwrap(),
                }),
            ));
            self.pending.push(Event::new(
                0,
                EventKind::ParamValue(ParamEvent {
                    param_id: ParamId(99),
                    value: ParamValue::new(0.25).unwrap(),
                }),
            ));
            self.pending.push(Event::new(
                0,
                EventKind::Midi(MidiEvent {
                    group: MidiGroup::ZERO,
                    channel: MidiChannel::new(0).unwrap(),
                    kind: MidiKind::NoteOn {
                        key: NoteKey::new(60).unwrap(),
                        velocity: 100,
                    },
                }),
            ));
        }
        self.pending.extend(echoes);
        ProcessResult::CONTINUE
    }

    fn drain_output_reports(&mut self, out: &mut Vec<Event>) {
        out.append(&mut self.pending);
    }
}

impl Plugin for ReportPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        REC_DESCRIPTOR
    }
    fn params(&self) -> &[ParamInfo] {
        &REC_PARAMS
    }
    fn buses(&self) -> &[BusDescriptor] {
        &STEREO_BUSES
    }
}

type InstanceLogs = Arc<Mutex<Vec<()>>>;

struct ReportFactory {
    instances: InstanceLogs,
}

impl PluginFactory for ReportFactory {
    fn descriptor(&self) -> PluginDescriptor {
        REC_DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        self.instances.lock().unwrap().push(());
        erase(ReportPlugin {
            pending: Vec::new(),
            announce: true,
        })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        self.instances.lock().unwrap().push(());
        Some(erase_sendable(ReportPlugin {
            pending: Vec::new(),
            announce: true,
        }))
    }
}

// ---------------------------------------------------------------- service fixture

struct SharedService(Rc<RefCell<AppService>>);

impl Service for SharedService {
    fn ops(&self) -> &'static [OpInfo] {
        OPS
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
    svc: Rc<RefCell<AppService>>,
    rt: UiRuntime,
    engine: SessionRoutingEngine,
    processor: ProcessorId,
    target: ParameterTargetId,
    lane: LaneId,
    source: SourceId,
    instances: InstanceLogs,
}

/// App + session + engine composed through the **production**
/// [`compose_routing_engine`]. The project holds one imported clip,
/// one rec-bound chain processor and one param-7 target; `lane`/
/// `source` are kept so a test can commit a second clip and force a
/// generation swap.
fn fixture(root: &Path) -> Fixture {
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let (processor, lane, source, target) = {
        let c = app.workspace_mut().session_mut().controller_mut();
        let out = import_stereo(c, root, "a.wav", 4_800);
        let (_track, strip, lane) = add_track_strip(c);
        let clip_source = out.commit.source_id;
        add_clip(c, lane, clip_source, 0, 480, 0);
        let processor = insert_rec_processor(c, strip, "rec");
        let target = add_gain_target(c, processor);
        (processor, lane, clip_source, target)
    };

    let instances = Arc::new(Mutex::new(Vec::new()));
    let engine = compose_routing_engine(
        &mut app,
        session_config(root.join("media")),
        vec![ProductPlugin {
            factory: Box::new(ReportFactory {
                instances: Arc::clone(&instances),
            }),
            params: REC_PARAMS.to_vec(),
        }],
    );

    // First notification → prepare + publish → attach/sync.
    let rev = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "pcwe".into(),
            },
            rev,
        )
        .unwrap();

    let svc = Rc::new(RefCell::new(AppService::new(app)));
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(SharedService(Rc::clone(&svc)))
        .expect("register AppService");

    Fixture {
        svc,
        rt,
        engine,
        processor,
        target,
        lane,
        source,
        instances,
    }
}

/// Dispatches `op` and extracts the single `result` event payload.
fn call(rt: &mut UiRuntime, op: &str, params: Value) -> (bool, Option<Value>, Option<ErrorBody>) {
    let events = rt.dispatch_op("test-1", op, params);
    let result = events.iter().find_map(|e| match e {
        UiEvent::Result {
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

/// RT-observed activation generation.
fn active_generation(engine: &SessionRoutingEngine) -> Option<u64> {
    let receipt = engine.receipt().expect("receipt");
    match receipt.state {
        RoutingActivationState::Active { generation, .. } => Some(generation),
        _ => None,
    }
}

fn activate(engine: &SessionRoutingEngine) -> u64 {
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    active_generation(engine).expect("Active receipt after run")
}

fn frame_base() -> TimeBase {
    TimeBase::sample_rate(48_000).unwrap()
}

fn fast_write(
    target: ParameterTargetId,
    frame: i64,
    value: f64,
    revision: ProjectRevision,
) -> FastControlWrite {
    FastControlWrite {
        target,
        value,
        means: ruviform_plugin_core::ParamWriteMeans::TimedEvent,
        position: MediaTime::new(frame, frame_base()),
        project_revision: revision,
    }
}

fn revision(fx: &Fixture) -> ProjectRevision {
    fx.svc.borrow().app().workspace().session().revision()
}

/// The `observe` section's per-processor span row, if present.
fn observed_span<'a>(status: &'a Value, tag: &str) -> Option<&'a Value> {
    status["observe"]["last_observed_span"]
        .get(tag)
        .filter(|v| !v.is_null())
}

// ---------------------------------------------------------------- tests

/// `SessionRoutingEngine::run` drives `observe_block` once per
/// processed block: two 256-frame blocks buffer exactly two merged
/// cycles, each carrying the bound target's observation — the
/// fixture never calls `observe_block`/`observe_cycle` itself.
#[test]
fn run_buffers_one_cycle_per_block() {
    let root = scratch("cycles");
    let fx = fixture(&root);

    // Nothing ran yet → nothing buffered.
    assert!(fx.engine.control_drain_observations().is_empty());

    activate(&fx.engine);

    let cycles = fx.engine.control_drain_observations();
    assert_eq!(cycles.len(), 2, "one merged cycle per processed block");
    // Pre-staging blocks honestly report no observations — no
    // control event was ever scheduled, so nothing is projected.
    // The per-block *count* is the contract under test here; window
    // correctness is pinned by the status/generation tests below.
    for cycle in &cycles {
        assert!(
            cycle.observations.is_empty(),
            "no staged control → honest empty observations: {cycle:?}"
        );
    }
    // Drained buffer stays empty until the next run.
    assert!(fx.engine.control_drain_observations().is_empty());
}

/// Reported→workspace feed through the engine-driven path alone:
/// fast-control Delivered → `run` buffers observe cycles →
/// `drain_control_reports` moves host reports into the ledger → the
/// next `run`'s observation projects `Reported` →
/// `drain_control_observations` feeds it into
/// `ControlSession::feed_plugin_report`. No test-side
/// `observe_cycle`/`observe_block` call participates.
#[test]
fn engine_observations_feed_reported_to_workspace() {
    let root = scratch("feed");
    let fx = fixture(&root);
    activate(&fx.engine);
    let _ = fx.engine.control_drain_observations();

    // Delivered fast write stages Play + plan install at the next
    // node head; the plugin echoes param 7 at half value.
    let rev = revision(&fx);
    let outcome = fx
        .svc
        .borrow_mut()
        .app_mut()
        .control_fast_control(fast_write(fx.target, 100, 0.7, rev))
        .expect("attached surface");
    assert!(
        matches!(outcome, FastControlOutcome::Delivered),
        "active surface delivers: {outcome:?}"
    );

    // Two blocks: the staged commands apply at the first head and
    // the event projects `Dispatched`; the plugin's echo report is
    // produced during RT processing.
    fx.engine.run(2, PumpPolicy::Every(1)).unwrap();
    // Host reports → dispatch ledger (`Reported` stage evidence).
    fx.svc.borrow_mut().app_mut().drain_control_reports();
    // One more block re-projects the ledger entry as `Reported`.
    fx.engine.run(1, PumpPolicy::Every(1)).unwrap();

    let fed = fx.svc.borrow_mut().app_mut().drain_control_observations();
    assert!(fed >= 1, "engine-observed Reported reached the feed");

    // Workspace evidence: plugin-reported value under an
    // `Estimated` stamp — never the host-resolved 0.7 relabeled.
    let observation = fx
        .svc
        .borrow()
        .app()
        .workspace()
        .session()
        .control()
        .observation(fx.target);
    let data = observation
        .plugin_reported
        .expect("plugin report reached ControlSession");
    assert!(
        (0.0..=1.0).contains(&data.value) && (data.value - 0.7).abs() > f64::EPSILON,
        "plugin-reported normalized value, not the host-resolved one: {}",
        data.value
    );
    assert_eq!(
        data.stamp.provenance,
        ruviform_media_core::TimestampProvenance::Estimated,
        "app-fed reports are backend-estimated evidence"
    );
}

/// `ui_pump` runs the observe drain on the control cadence: cycles
/// buffered by `SessionRoutingEngine::run` feed `Reported`
/// observations without any explicit drain call.
#[test]
fn ui_pump_drives_observation_feed() {
    let root = scratch("pump");
    let fx = fixture(&root);
    activate(&fx.engine);
    let _ = fx.engine.control_drain_observations();

    let rev = revision(&fx);
    let outcome = fx
        .svc
        .borrow_mut()
        .app_mut()
        .control_fast_control(fast_write(fx.target, 100, 0.7, rev))
        .expect("attached surface");
    assert!(matches!(outcome, FastControlOutcome::Delivered));

    // Pump 1: engine run is *not* inside ui_pump — the fixture drives
    // `run` explicitly (caller cadence), then the pump drains host
    // reports and feeds whatever cycles are buffered.
    fx.engine.run(2, PumpPolicy::Every(1)).unwrap();
    fx.svc.borrow_mut().app_mut().ui_pump();
    // The buffered cycles were observed before the drain moved
    // reports into the ledger — still `Dispatched`, fed = 0.
    let observation = fx
        .svc
        .borrow()
        .app()
        .workspace()
        .session()
        .control()
        .observation(fx.target);
    assert!(
        observation.plugin_reported.is_none(),
        "pre-drain observations stay Dispatched — no fabricated Reported"
    );

    // Pump 2 path: one more engine block re-projects `Reported`, and
    // `ui_pump`'s own drain tail feeds it — no explicit call.
    fx.engine.run(1, PumpPolicy::Every(1)).unwrap();
    fx.svc.borrow_mut().app_mut().ui_pump();
    let observation = fx
        .svc
        .borrow()
        .app()
        .workspace()
        .session()
        .control()
        .observation(fx.target);
    assert!(
        observation.plugin_reported.is_some(),
        "ui_pump fed the engine-observed Reported observation"
    );
}

/// `syn.control.status.observe` exposes the last projected span and
/// its plan generation per controlled processor — and a generation
/// swap proves the node-cursor domain: the new generation's plugin
/// nodes restart at frame 0 even though the engine cursor keeps
/// counting.
#[test]
fn status_observe_section_tracks_node_cursor_domain() {
    let root = scratch("status");
    let mut fx = fixture(&root);
    activate(&fx.engine);
    let _ = fx.engine.control_drain_observations();

    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    let tag = fx.processor.to_tagged();
    let span =
        observed_span(&status, &tag).unwrap_or_else(|| panic!("observed span for {tag}: {status}"));
    // After `activate`'s two blocks the last window is [256, 512).
    assert_eq!(span["start"]["frame"], 256);
    assert_eq!(span["end"]["frame"], 512);
    assert_eq!(span["start"]["epoch"], 0);
    assert!(span["deliverable"].is_boolean());
    // plan_generation mirrors the span ctx verbatim — consistent,
    // never independently fabricated.
    let plan_generation = status["observe"]["plan_generation"]
        .get(tag.as_str())
        .expect("plan_generation row");
    assert_eq!(plan_generation, &span["plan_generation"]);

    // Generation swap: a second clip republishes → surface syncs to
    // generation 2 → the activation block observes the new nodes'
    // cursor domain restarting at 0 (engine cursor is already past
    // 512 — the span proves the node domain, not the engine cursor).
    let instances_before = fx.instances.lock().unwrap().len();
    let rev = fx.svc.borrow().app().workspace().session().revision();
    fx.svc
        .borrow_mut()
        .app_mut()
        .workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::AddClip {
                lane_id: fx.lane,
                source_id: fx.source,
                placement: media_placement(480, 960, 48_000),
                playback: natural_playback(0, 480),
            },
            rev,
        )
        .unwrap();
    assert_eq!(
        fx.engine.control_generation(),
        Some(2),
        "surface synced to the republished generation"
    );
    assert!(
        fx.instances.lock().unwrap().len() > instances_before,
        "gen-2 publish vended a fresh plugin instance"
    );
    let _ = fx.engine.control_drain_observations();
    fx.engine.run(1, PumpPolicy::Every(1)).unwrap();

    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    let span = observed_span(&status, &tag)
        .unwrap_or_else(|| panic!("observed span for {tag} after swap: {status}"));
    assert_eq!(
        span["start"]["frame"], 0,
        "gen-2 nodes' cursor domain restarts at 0: {span}"
    );
    assert_eq!(span["end"]["frame"], 256);
}
