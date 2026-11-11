//! PC-C4-04 app composition acceptance (synvelis#101): the app
//! composition root owns the control surface end-to-end —
//! host-vended catalogs bind on publish, `ui_pump` drives one
//! control tick per cycle, fast-control writes reach the plugin
//! through the generation gate, and only `Reported`-stage
//! observations cross into the workspace control feed. A generation
//! swap proves stale surfaces and stale reports fail closed.
//!
//! Fixture mirrors `synvelis-player/tests/pc_c4_03_observation.rs`'s
//! reporting plugin, but every drive goes through the app surface:
//! `SessionRoutingEngine` (catalog cell + attach/sync + tick +
//! `fast_control` + observe/drain) and `SynvelisApp`
//! (`ui_pump`/`control_fast_control`/`feed_control_cycle`).

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

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
use ruviform_plugin_host::{PluginHandle, PluginHost};
use ruviform_ui::BackendReport;

use synvelis_app::{SessionEngineConfig, SessionRoutingEngine, SynvelisApp};
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::PluginHostProcessors;
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
        "synvelis-pcc404-{}-{}-{}",
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
/// linear automation lane — the lane exercises the tick's automation
/// evaluation path.
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
    id: "org.synvelis.test.pc-c4-04-rec",
    name: "PC-C4-04 Reporter",
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

/// A plugin that reports parameter observations back through
/// `drain_output_reports`: on its first `process` call it announces
/// `param 7 = 0.5` (plugin-initiated — distinct from any dispatched
/// value), an unbound `param 99` and one `Midi` report; on every
/// call that receives a `ParamValue` event it echoes the parameter
/// back at half value.
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

/// `instances.len()` after N generations is the per-generation
/// isolation evidence — each publish vends a fresh plugin instance.
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

// ---------------------------------------------------------------- fixture

struct Fixture {
    app: SynvelisApp,
    engine: SessionRoutingEngine,
    processor: ProcessorId,
    strip: StripId,
    target: ParameterTargetId,
    host: Rc<RefCell<PluginHost>>,
    instances: InstanceLogs,
}

/// App + session + engine + plugin host, with the project already
/// holding one imported clip, one rec-bound chain processor and one
/// param-7 target. Catalog vend is the *host* path under test:
/// `register_params` on `PluginHostProcessors`, the shared cell on
/// `SessionRoutingEngine::set_param_catalog` — no hand-built
/// `ParamCatalog` reaches the surface.
fn fixture(root: &Path) -> Fixture {
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let (processor, strip, target) = {
        let c = app.workspace_mut().session_mut().controller_mut();
        let out = import_stereo(c, root, "a.wav", 4_800);
        let (_track, strip, lane) = add_track_strip(c);
        add_clip(c, lane, out.commit.source_id, 0, 480, 0);
        let processor = insert_rec_processor(c, strip, "rec");
        let target = add_gain_target(c, processor);
        (processor, strip, target)
    };

    let mut host = PluginHost::new();
    let instances = Arc::new(Mutex::new(Vec::new()));
    host.register_factory(Box::new(ReportFactory {
        instances: Arc::clone(&instances),
    }));
    let mut processors = PluginHostProcessors::new(host);
    // The catalog vend: the composition root registers the plugin
    // descriptor's live `ParamInfo` enumeration; `connect` accumulates
    // it into the shared cell the engine reads at attach/sync.
    processors.register_params(REC_DESCRIPTOR.id, REC_PARAMS.to_vec());
    let cell = processors.param_catalog_cell();
    let host_rc = processors.host();

    let engine = SessionRoutingEngine::new(session_config(root.join("media")));
    engine.set_processor_host(Box::new(processors));
    engine.set_param_catalog(cell);
    app.attach_routing_engine(engine.clone());

    // First notification → prepare + publish → attach/sync.
    let rev = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "pcc404".into(),
            },
            rev,
        )
        .unwrap();

    Fixture {
        app,
        engine,
        processor,
        strip,
        target,
        host: host_rc,
        instances,
    }
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
/// node's first staged plan (`PlanGeneration(1)` — `ParamDispatch`'s
/// `next_generation` starts at 1 and each surface mints fresh).
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
) -> Option<synvelis_player::DispatchCycle> {
    engine.control_observe_cycle(
        processor,
        generation,
        &span(512),
        EngineFrame::new(ClockEpochId::new(0), 0),
        256,
    )
}

/// Drains `generation`'s vended endpoints through the real
/// `PluginHost::drain_output_reports` — the same call the production
/// drain path composes.
fn drain(
    engine: &SessionRoutingEngine,
    host: &Rc<RefCell<PluginHost>>,
    generation: u64,
) -> synvelis_player::ReportDrainReport {
    let endpoints = engine.plugin_state_handles(generation);
    let active = active_generation(engine);
    engine
        .with_control_dispatch(|dispatch| {
            dispatch.drain_output_reports(
                &endpoints,
                generation,
                active,
                &mut |handle: PluginHandle, out: &mut Vec<Event>| {
                    host.borrow_mut().drain_output_reports(handle, out)
                },
            )
        })
        .expect("attached surface")
}

// ---------------------------------------------------------------- tests

/// Catalog vend → publish → attach: the host-accumulated
/// `ParamCatalog` cell is what binds the target — `bound_targets == 1`
/// with zero `unbound` is impossible without the vended `ParamInfo`
/// (a hand-built catalog never reaches this surface).
#[test]
fn catalog_vend_binds_on_publish() {
    let root = scratch("vend");
    let fx = fixture(&root);

    let generation = fx
        .engine
        .control_generation()
        .expect("surface attached at publish");
    assert_eq!(generation, 1, "first published generation");

    let report = fx.engine.control_report().expect("surface report");
    assert_eq!(report.generation, generation);
    assert_eq!(report.bound_targets, 1, "param-7 target bound via vend");
    assert!(
        report.unbound.is_empty(),
        "no bind rejections: {:?}",
        report.unbound
    );
    assert_eq!(report.controlled_processors, vec![fx.processor]);
    assert!(fx.engine.last_control_error().is_none());

    // The vended cell is the shared accumulation — the processor's
    // entry carries exactly the registered `ParamInfo` table.
    // (Read through the engine's cell is internal; the binding
    // evidence above is the observable contract.)
}

/// `ui_pump` drives one `tick` per cycle: automation evaluation at
/// the session's honest `(beat, media)` instant plus the queue drain,
/// ungated once RT observes the surface's generation.
#[test]
fn ui_pump_drives_control_tick() {
    let root = scratch("tick");
    let mut fx = fixture(&root);

    // Before RT observes the published generation the tick is gated —
    // automation self-corrects next tick; nothing is dispatched.
    fx.app.ui_pump();
    let gated = fx.app.last_control_tick().expect("tick ran");
    assert!(gated.attached);
    assert!(gated.gated, "no active generation before run");

    let generation = activate(&fx.engine);
    fx.app.ui_pump();
    let tick = fx.app.last_control_tick().expect("tick report");
    assert!(tick.attached);
    assert!(!tick.gated, "active generation matches surface");
    assert_eq!(tick.generation, generation);
    assert!(!tick.stale);
    assert!(tick.evaluated >= 1, "automation lane evaluated: {tick:?}");
}

/// Fast-control write → Delivered while active → plugin echoes a
/// report → drain → `Reported` observation → `feed_control_cycle`
/// crosses only that stage into `ControlSession::feed_plugin_report`.
#[test]
fn reported_observation_reaches_control_feed() {
    let root = scratch("feed");
    let mut fx = fixture(&root);
    let generation = activate(&fx.engine);
    let revision = fx.app.workspace().session().revision();

    // Delivered immediately — the surface's generation is RT-active.
    let outcome = fx
        .app
        .control_fast_control(fast_write(fx.target, 100, 0.7, revision))
        .expect("attached surface");
    assert!(
        matches!(outcome, FastControlOutcome::Delivered),
        "active surface delivers: {outcome:?}"
    );

    // RT delivers the staged plan; the plugin echoes param 7 at half
    // value plus its plugin-initiated announce.
    fx.engine.run(2, PumpPolicy::Every(1)).unwrap();

    // First projection: the entry is `Dispatched` — host-resolved,
    // NOT a plugin report. The feed must refuse it.
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
        fx.app.feed_control_cycle(&cycle),
        0,
        "host-resolved stages never cross into the plugin-report feed"
    );

    // Drain the plugin's pending reports through the real host, then
    // project again — the entry is now `Reported`.
    let drain_report = drain(&fx.engine, &fx.host, generation);
    assert_eq!(drain_report.drained, 1, "one endpoint drained");
    assert!(drain_report.param_reports >= 1);
    assert!(!drain_report.generation_gated && !drain_report.inactive_gated);

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

    assert_eq!(fx.app.feed_control_cycle(&cycle), 1, "one Reported fed");

    // Workspace evidence: the plugin-report channel holds the value
    // under an `Estimated` stamp → `Stale`, never `Live`/`Captured`.
    let observation = fx
        .app
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
    assert_eq!(
        fx.app
            .workspace()
            .session()
            .control()
            .observation_state(fx.target),
        synvelis_workspace::ObservationState::Stale
    );
}

/// Generation swap: publish gen 2 → the surface re-syncs → observe/
/// drain calls naming gen 1 fail closed, gen-2 calls work, and the
/// feed only ever sees the live surface's reports.
#[test]
fn generation_swap_gates_stale_surface() {
    let root = scratch("swap");
    let mut fx = fixture(&root);
    let generation = activate(&fx.engine);
    assert_eq!(generation, 1);

    // A second plugin-bound processor changes the routing fingerprint
    // → the next session apply publishes generation 2.
    let rev = fx.app.workspace().session().revision();
    fx.app
        .workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::InsertProcessor {
                strip_id: fx.strip,
                index: 0,
                name: "rec2".into(),
                binding: ProcessorBinding::Resolved(PluginRef {
                    descriptor: REC_DESCRIPTOR.id.into(),
                    version: "0.1.0".into(),
                    capability_revision: 0,
                }),
                sidechain_input: false,
            },
            rev,
        )
        .unwrap();
    let generation2 = fx
        .engine
        .control_generation()
        .expect("surface re-synced at publish");
    assert_eq!(generation2, 2, "generation swap published");
    // Generation 1 vendored one instance (rec); the swap rebuilds the
    // whole graph so generation 2 vends fresh instances for both
    // plugin processors — per-generation isolation, never reuse.
    assert!(
        fx.instances.lock().unwrap().len() >= 2,
        "fresh instances per generation"
    );

    // Stale generation: observe and drain both refuse before any
    // work — a gen-1 report can never be attributed to gen 2.
    assert!(
        observe(&fx.engine, fx.processor, generation).is_none(),
        "observe_cycle generation-gates"
    );
    let stale = drain(&fx.engine, &fx.host, generation);
    assert!(stale.generation_gated, "drain generation-gates");
    assert_eq!(stale.drained, 0);

    // The live generation works end-to-end: fast_control → Delivered
    // → run → drain → Reported → feed.
    activate(&fx.engine);
    let revision = fx.app.workspace().session().revision();
    let outcome = fx
        .app
        .control_fast_control(fast_write(fx.target, 100, 0.7, revision))
        .expect("attached surface");
    assert!(matches!(outcome, FastControlOutcome::Delivered));
    fx.engine.run(2, PumpPolicy::Every(1)).unwrap();

    let live = drain(&fx.engine, &fx.host, generation2);
    assert!(!live.generation_gated);
    assert!(live.drained >= 1, "live generation endpoints drain");

    let cycle = observe(&fx.engine, fx.processor, generation2).expect("cycle");
    assert!(
        cycle
            .observations
            .iter()
            .any(|(_, obs)| { obs.stage == ruviform_audio_plugin_bridge::DispatchStage::Reported })
    );
    assert!(fx.app.feed_control_cycle(&cycle) >= 1);
}
