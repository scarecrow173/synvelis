//! PC-C4 drain composition acceptance (synvelis#101): the production
//! composition root (`compose_routing_engine` — the same function
//! `synvelis-native` calls) assembles the routing engine, catalog
//! vend, report drain, and control surface end-to-end. `ui_pump`
//! drives `drain_output_reports` on the control cadence; plugin
//! reports reach the workspace feed only through the `Reported`
//! stage; inactive surfaces gate the drain fail-closed; a saturated
//! fast-control queue answers typed `QueueFull`, never a false
//! success.
//!
//! Fixture mirrors `pc_c4_04_app_composition.rs`'s reporting plugin
//! and project shape, but the engine is composed through
//! [`compose_routing_engine`] — the fixture never names `PluginHost`
//! or builds `PluginHostProcessors` by hand, so a green run proves
//! the production wiring itself. Behavioral drives go through both
//! the app surface (`ui_pump`/`control_fast_control`/
//! `drain_control_reports`/`feed_control_cycle`) and the
//! `syn.control.*` op surface via `UiRuntime::dispatch_op`.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use ruviform_audio_core::ChannelLayout;
use ruviform_media_core::{
    ClockEpochId, EngineFrame, LoopOccurrence, MediaTime, MidiChannel, MidiEvent, MidiGroup,
    MidiKind, NoteKey, PlanGeneration, SpanContext, TempoMapRevision, TimeBase,
    TransportGeneration, TransportSpan, TransportStatus,
};
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, Event,
    EventKind, EventList, ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, ParamWriteMeans,
    Plugin, PluginDescriptor, PluginFactory, PluginProcessor, PrepareError, ProcessConfig,
    ProcessContext, ProcessResult, TransportInfo, erase, erase_sendable,
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
use synvelis_player::{
    DispatchCycle, FastControlBlockKind, FastControlOutcome, FastControlWrite, ReportDrainReport,
};
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
        "synvelis-pcc4dc-{}-{}-{}",
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
/// linear automation lane — identical to the `pc_c4_04`/`pc_c4_06` fixture so
/// the automation tick path stays exercised.
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
                flags_bits: ParamFlags::AUTOMATABLE.bits(),
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
    id: "org.synvelis.test.pc-c4-drain-rec",
    name: "PC-C4-Drain Reporter",
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
    flags: ParamFlags::AUTOMATABLE,
}];

/// The `pc_c4_04`/`pc_c4_06` reporting plugin verbatim: announces param 7 =
/// 0.5, unbound param 99 and one Midi report on its first `process`,
/// and echoes every received `ParamValue` back at half value.
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

/// `instances.len()` is the composition evidence — each publish
/// vends a fresh plugin instance through the production host path.
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

/// `AppService` behind a shared handle — the fixture reaches
/// `app_mut()`/`app()` (engine attach evidence, `ui_pump`,
/// `drain_control_reports`, `feed_control_cycle`) through the same
/// object `UiRuntime` dispatches into.
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
    instances: InstanceLogs,
}

/// App + session + engine composed through the **production**
/// [`compose_routing_engine`] — `PluginHostProcessors::product` over
/// a fresh `PluginHost`, the plugin's `ParamInfo` table registered
/// under its descriptor id, catalog cell + report drain wired, and
/// the engine attached as the session's `EnginePort`. The project
/// already holds one imported clip, one rec-bound chain processor
/// and one param-7 target. The fixture never constructs
/// `PluginHostProcessors` or names `PluginHost` itself.
fn fixture(root: &Path) -> Fixture {
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let (processor, _strip, target) = {
        let c = app.workspace_mut().session_mut().controller_mut();
        let out = import_stereo(c, root, "a.wav", 4_800);
        let (_track, strip, lane) = add_track_strip(c);
        add_clip(c, lane, out.commit.source_id, 0, 480, 0);
        let processor = insert_rec_processor(c, strip, "rec");
        let target = add_gain_target(c, processor);
        (processor, strip, target)
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
                name: "pcc4dc".into(),
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

/// RT-observed activation generation — the same value the dispatch
/// gates consume via `rt_status().active_generation`.
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
        means: ParamWriteMeans::TimedEvent,
        position: MediaTime::new(frame, frame_base()),
        project_revision: revision,
    }
}

/// A deliverable Playing span whose plan generation matches the
/// node's first staged plan (`PlanGeneration(1)` — these tests never
/// run `ui_pump` between attach and observe, so no automation plan
/// advances the counter; the same rule `pc_c4_04` pins).
fn span(end: u64) -> TransportSpan {
    TransportSpan {
        start: EngineFrame::new(ClockEpochId::new(0), 0),
        end: EngineFrame::new(ClockEpochId::new(0), end),
        ctx: SpanContext {
            status: TransportStatus::Playing,
            position_ticks: 0,
            position_base: frame_base(),
            loop_ticks: None,
            occurrence: LoopOccurrence::new(0),
            clock_epoch: ClockEpochId::new(0),
            transport_generation: TransportGeneration::new(0),
            plan_generation: PlanGeneration::new(1),
            tempo_revision: TempoMapRevision::new(0),
        },
        deliverable: true,
    }
}

fn observe(
    engine: &SessionRoutingEngine,
    processor: ProcessorId,
    generation: u64,
) -> Option<DispatchCycle> {
    engine.control_observe_cycle(
        processor,
        generation,
        &span(512),
        EngineFrame::new(ClockEpochId::new(0), 0),
        256,
    )
}

fn revision(fx: &Fixture) -> ProjectRevision {
    fx.svc.borrow().app().workspace().session().revision()
}

// ---------------------------------------------------------------- tests

/// The production composition root assembles the whole control
/// path: catalog vend binds on publish, the `syn.control.*` op
/// surface answers typed evidence, `ui_pump` drives the periodic
/// `drain_output_reports` pass, and the typed `ReportDrainReport`
/// reaches `syn.control.status.drain` verbatim — generation is the
/// surface's own, both gates clear once RT observes activation.
#[test]
fn composition_root_assembles_surface_and_periodic_drain() {
    let root = scratch("compose");
    let mut fx = fixture(&root);

    // Composition evidence: the catalog vended through
    // `ProductPlugin.params` bound the param-7 target on publish —
    // impossible without the registered `ParamInfo` table.
    let report = fx.engine.control_report().expect("surface report");
    assert_eq!(report.bound_targets, 1, "catalog vend bound target");
    assert!(report.unbound.is_empty(), "no bind rejections");

    let generation = fx
        .engine
        .control_generation()
        .expect("surface attached at publish");
    assert_eq!(generation, 1, "first published generation");

    // Before RT activation one explicit `ui_pump` drives the drain
    // pass — gated fail-closed on the unobserved active generation,
    // nothing drained (`syn.control.status` is read-only and does
    // not pump, so it reports the last recorded pass verbatim).
    fx.svc.borrow_mut().app_mut().ui_pump();
    let drain = fx.svc.borrow().app().last_control_drain();
    assert!(drain.attached);
    assert_eq!(drain.generation, 1);
    assert!(drain.inactive_gated, "unobserved active generation gates");
    assert!(!drain.generation_gated);
    assert_eq!(drain.drained, 0, "gated drain never calls the host");
    assert_eq!(drain.events, 0);

    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    assert_eq!(status["engine"], true);
    assert_eq!(status["generation"], 1);
    assert!(status["active_generation"].is_null());
    let drain = &status["drain"];
    assert_eq!(drain["attached"], true);
    assert_eq!(drain["generation"], 1);
    assert_eq!(drain["inactive_gated"], true);
    assert_eq!(drain["drained"], 0, "gated drain never calls the host");

    activate(&fx.engine);
    assert!(!fx.instances.lock().unwrap().is_empty());

    // The periodic cadence: one `ui_pump` runs one drain pass — the
    // plugin's announce reports (produced during `activate`'s RT
    // pump) are consumed and counted, never fabricated.
    fx.svc.borrow_mut().app_mut().ui_pump();
    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    let drain = &status["drain"];
    assert_eq!(drain["attached"], true);
    assert_eq!(drain["generation"], 1);
    assert_eq!(drain["generation_gated"], false);
    assert_eq!(drain["inactive_gated"], false);
    assert_eq!(drain["drained"], 1, "one endpoint drained");
    assert!(
        drain["param_reports"].as_u64().unwrap() >= 1,
        "plugin ParamValue reports consumed: {drain}"
    );

    // The op surface is reachable end-to-end through the composed
    // engine — a delivered fast write, not a stub error.
    let params = json!({
        "target": fx.target.to_tagged(),
        "value": 0.7,
        "means": "timed_event",
        "position_frames": 100,
        "project_revision": revision(&fx).0,
    });
    let outcome = ok(&mut fx.rt, "syn.control.fast_control", params);
    assert_eq!(outcome["outcome"], "delivered");
}

/// Reported→Observed through the app-owned drain path:
/// fast-control Delivered → RT echo → `drain_control_reports` (the
/// registered `PluginReportDrain`, not a fixture-side host borrow)
/// → `Reported` observation → `feed_control_cycle` crosses exactly
/// one row into `ControlSession::feed_plugin_report`.
#[test]
fn drain_reports_reach_observed_feed() {
    let root = scratch("feed");
    let fx = fixture(&root);
    let generation = activate(&fx.engine);

    // Delivered fast write (plan gen 1 — no pump ran, so the
    // automation lane never staged an extra plan).
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

    // RT applies the staged plan; the plugin echoes param 7 at half
    // value plus its plugin-initiated announce.
    fx.engine.run(2, PumpPolicy::Every(1)).unwrap();

    // First projection: `Dispatched` — host-resolved, never fed.
    let cycle = observe(&fx.engine, fx.processor, generation).expect("cycle");
    assert!(
        cycle.observations.iter().any(|(_, obs)| {
            matches!(
                obs.stage,
                ruviform_audio_plugin_bridge::DispatchStage::Dispatched { .. }
            )
        }),
        "dispatched observation present: {:?}",
        cycle.observations
    );
    assert_eq!(
        fx.svc.borrow_mut().app_mut().feed_control_cycle(&cycle),
        0,
        "host-resolved stages never cross into the plugin-report feed"
    );

    // The app-owned periodic drain path: one explicit pass through
    // the composition-registered `PluginReportDrain`.
    fx.svc.borrow_mut().app_mut().drain_control_reports();
    let drain: ReportDrainReport = fx.svc.borrow().app().last_control_drain();
    assert!(drain.attached);
    assert_eq!(drain.generation, generation);
    assert_eq!(drain.drained, 1, "one endpoint drained");
    assert!(drain.param_reports >= 1);
    assert!(drain.reported >= 1, "a report advanced the ledger");
    assert!(!drain.generation_gated && !drain.inactive_gated);

    // Second projection: the entry is `Reported` — plugin evidence.
    let cycle = observe(&fx.engine, fx.processor, generation).expect("cycle");
    let reported = cycle
        .observations
        .iter()
        .find(|(_, obs)| obs.stage == ruviform_audio_plugin_bridge::DispatchStage::Reported)
        .map(|(_, obs)| obs)
        .expect("reported observation after drain");
    let reported_value = reported.value.expect("reported value").get();
    assert!(
        (0.0..=1.0).contains(&reported_value),
        "plugin-reported normalized value"
    );
    assert_ne!(
        reported_value, 0.7,
        "the fed value is plugin-reported (echo/announce), never the \
         host-resolved dispatched value relabeled"
    );
    assert_eq!(
        fx.svc.borrow_mut().app_mut().feed_control_cycle(&cycle),
        1,
        "one Reported fed"
    );

    // Workspace evidence: the plugin-report channel holds the value
    // under an `Estimated` stamp → `Stale`, never `Live`/`Captured`.
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
    assert_eq!(data.value, reported_value);
    assert_eq!(
        data.stamp.provenance,
        ruviform_media_core::TimestampProvenance::Estimated,
        "app-fed reports are backend-estimated evidence"
    );
}

/// Fail-closed evidence on both gates the app path owns: before RT
/// observes the active generation the drain reports
/// `inactive_gated` with zero host calls; the fast-control queue
/// answers `Queued` up to capacity then typed `QueueFull` — never a
/// `Delivered`/`Queued` false success.
#[test]
fn inactive_gate_and_queue_saturation_fail_closed() {
    let root = scratch("gates");
    let fx = fixture(&root);

    // Surface attached at publish, RT never ran → active generation
    // unobserved. One explicit drain pass must gate fail-closed.
    fx.svc.borrow_mut().app_mut().drain_control_reports();
    let drain = fx.svc.borrow().app().last_control_drain();
    assert!(drain.attached);
    assert_eq!(drain.generation, 1);
    assert!(drain.inactive_gated, "unobserved active generation gates");
    assert!(!drain.generation_gated);
    assert_eq!(drain.drained, 0, "gated drain never calls the host");
    assert_eq!(drain.events, 0);

    // Queue saturation while inactive: writes queue up to the
    // bounded capacity, then refuse typed `QueueFull`. Values vary
    // so no dedup path can mask the overflow.
    let rev = revision(&fx);
    let mut queued = 0usize;
    let mut blocked = 0usize;
    let mut saw_full = false;
    for i in 0..80i32 {
        let outcome = fx
            .svc
            .borrow_mut()
            .app_mut()
            .control_fast_control(fast_write(
                fx.target,
                100 + i64::from(i),
                0.1 + f64::from(i) * 0.001,
                rev,
            ))
            .expect("attached surface");
        match outcome {
            FastControlOutcome::Delivered => {
                panic!("inactive surface must never report Delivered: {outcome:?}")
            }
            FastControlOutcome::Queued { .. } => {
                assert!(!saw_full, "no write may queue after QueueFull");
                queued += 1;
            }
            FastControlOutcome::Blocked(FastControlBlockKind::QueueFull) => {
                saw_full = true;
                blocked += 1;
            }
            other @ FastControlOutcome::Blocked(_) => {
                panic!("unexpected block kind past capacity: {other:?}")
            }
        }
    }
    assert!(queued >= 1, "bounded queue accepted writes");
    assert!(blocked >= 1, "overflow refused typed QueueFull");
    assert_eq!(queued + blocked, 80, "every write accounted for");

    // Once RT activates the surface the same drain path ungates —
    // the queued reports stay honest evidence, not stale success.
    activate(&fx.engine);
    fx.svc.borrow_mut().app_mut().drain_control_reports();
    let drain = fx.svc.borrow().app().last_control_drain();
    assert!(!drain.inactive_gated);
    assert!(!drain.generation_gated);
    assert_eq!(drain.drained, 1);
}

/// Session recreate is the drain's hard boundary: `project.new`
/// destroys the session, so the engine handle, the registered
/// `PluginReportDrain`, and the recorded `ReportDrainReport` all
/// reset together — `syn.control.status` can never pair
/// `engine: false` with a stale `drain.attached: true` describing
/// the dead session's surface.
#[test]
fn project_recreate_clears_drain_and_its_evidence() {
    let root = scratch("recreate");
    let mut fx = fixture(&root);
    activate(&fx.engine);

    // Record real drain evidence against the live session first.
    fx.svc.borrow_mut().app_mut().drain_control_reports();
    let drain = fx.svc.borrow().app().last_control_drain();
    assert!(drain.attached);
    assert_eq!(drain.drained, 1, "activated surface drained once");

    // Recreate the session — the composed engine and its drain
    // handle belonged to the destroyed project.
    let bundle = root.join("next").to_string_lossy().into_owned();
    fx.svc
        .borrow_mut()
        .app_mut()
        .project_new(&bundle, Some("next"))
        .expect("project.new");

    // Evidence reset to the honest default — nothing carried over.
    let drain = fx.svc.borrow().app().last_control_drain();
    assert!(!drain.attached, "dead-session drain evidence cleared");
    assert_eq!(drain.drained, 0);

    // An explicit drain pass afterwards is a no-op: no engine, no
    // registered drain — the report stays at its honest default
    // rather than reviving the destroyed session's counters.
    fx.svc.borrow_mut().app_mut().drain_control_reports();
    let drain = fx.svc.borrow().app().last_control_drain();
    assert!(!drain.attached);
    assert_eq!(drain.drained, 0);

    // The status surface agrees: no engine, no surface, and the
    // drain block reports the honest unattached default.
    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    assert_eq!(status["engine"], false);
    assert!(status["generation"].is_null());
    assert_eq!(status["drain"]["attached"], false);
    assert_eq!(status["drain"]["drained"], 0);
}
