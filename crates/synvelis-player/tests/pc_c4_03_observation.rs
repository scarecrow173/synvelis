//! PC-C4-03 acceptance tests — reported-observation consumption:
//! `PluginHost::drain_output_reports` → session-binding `ParamId →
//! target` resolution → `ParamDispatch` ledger `Reported` /
//! `Unavailable` stages, all generation-gated on
//! [`RoutingControlDispatch`].
//!
//! Evidence under test:
//!
//! - A `Drained` answer's `EventKind::ParamValue` reports advance the
//!   node's ledger to `Reported` with the *plugin-reported* value —
//!   distinct from the host-resolved dispatched value.
//! - `ReportDrain::Unavailable` (and stale/failed handles) terminate
//!   pending entries honestly — never reported as success. Real L1
//!   slots answering `Unavailable` are `NotRun` at pin `b02942c2`
//!   (no wire report verb; RF#253 PR #564 pending) — the branch is
//!   exercised through the injected drain seam.
//! - Generation/active-generation mismatches gate the whole drain —
//!   foreign-generation reports are never attributed, and gated
//!   reports stay queued plugin-side for the next active drain.
//! - Unbound `ParamId`s, `Midi` reports and bound params with no
//!   eligible ledger entry are typed counters — never false
//!   observations.
//!
//! Headless only.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::DispatchStage;
use ruviform_media_core::{
    ClockEpochId, EngineFrame, LoopOccurrence, MediaTime, MidiChannel, MidiEvent, MidiGroup,
    MidiKind, NoteKey, PlanGeneration, TempoMapRevision, TimeBase, TransportGeneration,
    TransportSpan, TransportStatus,
};
use ruviform_media_reader::PrefetchConfig;
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, Event,
    EventKind, EventList, ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, Plugin,
    PluginDescriptor, PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext,
    ProcessResult, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::{HostError, PluginHandle, PluginHost, ReportDrain};
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::PluginHostProcessors;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::resolver::{PreparedClip, prepare};
use synvelis_player::routing_control::{
    FastControlOutcome, FastControlWrite, ParamCatalog, ReportDrainReport,
};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::{PluginStateEndpoint, RoutingControlDispatch};
use synvelis_project::{
    AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange, BuildBudget,
    BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, ConflictPolicy,
    ControlOwner, CounterIdGenerator, Interpolation, MediaDuration, MediaTimeValue, ParamMapping,
    ParameterRole, ParameterTargetId, PlaybackMapping, ProcessorBinding, ProcessorId, Project,
    ProjectController, ProjectId, ProjectOp, ProjectRevision, RoutingBuildRequestId,
    RoutingCompileRequest, RuntimeClockEpochRef, RuntimeInstanceId, SourceFrame, SourceId,
    SourceRange, TargetCapability, TimeAnchor, ValueKind,
};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-pcc403-{}-{}-{}",
        name,
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0x107), "pcc403"),
        Box::new(CounterIdGenerator::new(1)),
    )
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

fn engine() -> synvelis_player::lowering::EngineConfig {
    synvelis_player::lowering::EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 256,
    }
}

fn canonical_format() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn prefetch() -> PrefetchConfig {
    PrefetchConfig {
        block_frames: 256,
        block_count: 8,
        channels: 2,
        sample_rate: 48_000,
    }
}

fn frame_base() -> TimeBase {
    TimeBase::sample_rate(48_000).unwrap()
}

fn request(id: u64, revision: ProjectRevision) -> RoutingCompileRequest {
    RoutingCompileRequest {
        request_id: RoutingBuildRequestId(id),
        expected_project_revision: revision,
        mode: BuildMode::Realtime,
        resource_budget: BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
        runtime_instance: RuntimeInstanceId(1),
        device_clock_epoch: RuntimeClockEpochRef(1),
    }
}

fn new_engine() -> RoutingEngine {
    RoutingEngine::new(engine(), RuntimeInstanceId(1), RuntimeClockEpochRef(1))
}

/// The RT-observed active generation as the control gate consumes it:
/// `0` means "never observed" → `None` (always gated).
fn active_generation(eng: &RoutingEngine) -> Option<u64> {
    let generation = eng.rt_status().active_generation;
    (generation != 0).then_some(generation)
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

fn add_track_strip(
    c: &mut ProjectController,
) -> (synvelis_project::TrackId, synvelis_project::LaneId) {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    (track, lane)
}

fn add_clip(
    c: &mut ProjectController,
    lane: synvelis_project::LaneId,
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

fn lane_and_source(c: &ProjectController) -> (synvelis_project::LaneId, SourceId) {
    let track = *c.live().tracks.keys().next_back().unwrap();
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    let source = c.live().clips.values().next().unwrap().source_id;
    (lane, source)
}

fn strip_of_track(
    project: &Project,
    track: synvelis_project::TrackId,
) -> synvelis_project::StripId {
    project.routing.strip_of_track(track).map(|s| s.id).unwrap()
}

fn prepared_clips(project: &Project, media_root: &Path) -> Vec<PreparedClip> {
    project
        .clips
        .values()
        .map(|clip| prepare(project, clip.source_id, media_root, &prefetch()).unwrap())
        .collect()
}

fn deps_default<'a>(project: &Project, media_root: &Path) -> RoutingDeps<'a> {
    RoutingDeps {
        canonical_of: &|_| Some(canonical_format()),
        clips: prepared_clips(project, media_root),
        engine_config_generation: 1,
        source_format_generation: 1,
    }
}

/// Minimal project: one stereo import, one track strip, one clip.
fn minimal_project(root: &Path) -> (ProjectController, PathBuf) {
    let mut c = controller();
    let out = import_stereo(&mut c, root, "a.wav", 4_800);
    let (_, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    (c, root.join("media"))
}

/// `minimal_project` plus one chain processor bound to the reporting
/// plugin.
fn plugin_project(root: &Path) -> (ProjectController, PathBuf, ProcessorId) {
    let (mut c, media) = minimal_project(root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    let strip = strip_of_track(c.live(), track);
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "rec".into(),
            binding: ProcessorBinding::Resolved(synvelis_project::PluginRef {
                descriptor: REC_DESCRIPTOR.id.into(),
                version: "0.1.0".into(),
                capability_revision: 0,
            }),
            sidechain_input: false,
        },
    );
    let processor_id = *c.live().routing.processors.keys().next_back().unwrap();
    (c, media, processor_id)
}

/// Adds a `ProcessorParam` target on `processor` declaring param 7
/// plus a linear automation lane — the lane is unused by these tests
/// (fast controls carry the writes) but keeps the fixture identical
/// to `sf03_l6`'s bound surface.
fn add_gain_target(
    c: &mut ProjectController,
    processor: ProcessorId,
) -> (ParameterTargetId, synvelis_project::AutomationLaneId) {
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
    (target, lane)
}

/// The caller-vended catalog the surface binds against — the
/// reporting plugin's live `ParamInfo` enumeration.
fn catalog(processor: ProcessorId) -> ParamCatalog {
    BTreeMap::from([(processor, REC_PARAMS.to_vec())])
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
    id: "org.synvelis.test.pc-c4-03-rec",
    name: "PC-C4-03 Reporter",
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

/// One delivered parameter event at its absolute engine frame.
#[derive(Debug, Clone, PartialEq)]
struct Delivered {
    abs_frame: u64,
    param_id: u32,
    value: f64,
}

/// A plugin that reports parameter observations back through
/// `drain_output_reports`: on its first `process` call it announces
/// `param 7 = 0.5` (plugin-initiated — distinct from any dispatched
/// value), an unbound `param 99` and one `Midi` report; on every
/// call that receives a `ParamValue` event it echoes the parameter
/// back at half value. Pending reports accumulate until the host
/// drains them — the bounded-buffer contract is upstream's.
struct ReportPlugin {
    delivered: Arc<Mutex<Vec<Delivered>>>,
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
        transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: BufferView,
        O: BufferViewMut,
    {
        let pos = transport.map_or(0, |t| t.position().value());
        let mut echoes = Vec::new();
        for ev in events.iter() {
            if let EventKind::ParamValue(pe) = ev.kind() {
                #[allow(clippy::cast_sign_loss)]
                self.delivered.lock().unwrap().push(Delivered {
                    abs_frame: (pos + i64::from(ev.frame_offset())) as u64,
                    param_id: pe.param_id.0,
                    value: pe.value.get(),
                });
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

/// `instances[i]` is the i-th created plugin instance's delivery log —
/// per-generation isolation evidence.
type InstanceLogs = Arc<Mutex<Vec<Arc<Mutex<Vec<Delivered>>>>>>;

struct ReportFactory {
    instances: InstanceLogs,
}

impl PluginFactory for ReportFactory {
    fn descriptor(&self) -> PluginDescriptor {
        REC_DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        let delivered = Arc::new(Mutex::new(Vec::new()));
        self.instances.lock().unwrap().push(Arc::clone(&delivered));
        erase(ReportPlugin {
            delivered,
            pending: Vec::new(),
            announce: true,
        })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        let delivered = Arc::new(Mutex::new(Vec::new()));
        self.instances.lock().unwrap().push(Arc::clone(&delivered));
        Some(erase_sendable(ReportPlugin {
            delivered,
            pending: Vec::new(),
            announce: true,
        }))
    }
}

fn report_host() -> (PluginHostProcessors, InstanceLogs) {
    let instances = Arc::new(Mutex::new(Vec::new()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(ReportFactory {
        instances: Arc::clone(&instances),
    }));
    (PluginHostProcessors::new(host), instances)
}

// ---------------------------------------------------------------- drives

/// Builds + publishes generation `id` for `c`/`media` on `eng` and
/// runs one block so RT observes the activation.
fn publish_and_run(eng: &mut RoutingEngine, c: &ProjectController, media: &Path, id: u64) {
    let rev = c.live_revision();
    let prepared = eng
        .prepare(
            &request(id, rev),
            c.live(),
            rev,
            deps_default(c.live(), media),
        )
        .unwrap()
        .expect_built();
    eng.publish(prepared, c.live(), rev).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();
}

/// Attaches the control surface for `generation`.
fn attach_surface(
    eng: &mut RoutingEngine,
    c: &ProjectController,
    processor: ProcessorId,
    generation: u64,
) -> RoutingControlDispatch {
    RoutingControlDispatch::attach(eng, generation, c.live(), &catalog(processor)).unwrap()
}

/// Drains `generation`'s vended endpoints through the real
/// `PluginHost::drain_output_reports`.
fn drain_host(
    dispatch: &mut RoutingControlDispatch,
    eng: &RoutingEngine,
    host: &Rc<RefCell<PluginHost>>,
    generation: u64,
) -> ReportDrainReport {
    let endpoints = eng.plugin_state_handles(generation);
    drain_with(
        dispatch,
        &endpoints,
        generation,
        active_generation(eng),
        &mut |handle, out| host.borrow_mut().drain_output_reports(handle, out),
    )
}

/// The injected drain seam — same shape as
/// `RoutingControlDispatch::drain_output_reports`' `drain` parameter.
type DrainFn<'a> =
    &'a mut dyn FnMut(PluginHandle, &mut Vec<Event>) -> Result<ReportDrain, HostError>;

/// The generic drain driver — `endpoints`/`active`/`drain` are all
/// caller-injected so gate and error branches are testable without
/// an L1 worker.
fn drain_with(
    dispatch: &mut RoutingControlDispatch,
    endpoints: &[PluginStateEndpoint],
    generation: u64,
    active: Option<u64>,
    drain: DrainFn<'_>,
) -> ReportDrainReport {
    dispatch.drain_output_reports(endpoints, generation, active, drain)
}

/// A deliverable Playing span whose plan generation matches the
/// node's first staged plan (`PlanGeneration(1)`).
fn span(end: u64) -> TransportSpan {
    TransportSpan {
        start: EngineFrame::new(ClockEpochId::new(0), 0),
        end: EngineFrame::new(ClockEpochId::new(0), end),
        ctx: ruviform_media_core::SpanContext {
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

/// The observation for `target` in a `DispatchCycle`, if present.
fn observation_of(
    cycle: &synvelis_player::DispatchCycle,
    target: ParameterTargetId,
) -> Option<ruviform_audio_plugin_bridge::ParamValueObservation> {
    cycle
        .observations
        .iter()
        .find(|(t, _)| *t == target)
        .map(|(_, obs)| *obs)
}

// ---------------------------------------------------------------- tests

/// PC-C4-03-01: drain → `record_reported` → `observe_cycle` →
/// `Reported` projection end-to-end through the real host. The
/// announced `param 7 = 0.5` report lands on the pending write's
/// ledger entry — the observation carries the *plugin-reported*
/// value, not the dispatched 0.8.
#[test]
fn pc_c4_03_01_reported_value_advances_ledger_to_reported() {
    let root = scratch("c40301");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (target, _lane) = add_gain_target(&mut c, processor_id);
    let (adapter, _instances) = report_host();
    let host = adapter.host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    publish_and_run(&mut eng, &c, &media, 1);
    assert_eq!(active_generation(&eng), Some(1));

    let mut dispatch = attach_surface(&mut eng, &c, processor_id, 1);

    // Point-in-time write → `Scheduled` ledger entry + staged plan.
    let rev = c.live_revision();
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 300, 0.8, rev),
        active_generation(&eng),
    );
    assert!(matches!(outcome, FastControlOutcome::Delivered));

    // RT delivers the event; the plugin echoes param 7 at half value.
    eng.run(2, PumpPolicy::Every(1)).unwrap();

    // Scheduler-side projection: `Scheduled` → `Dispatched`.
    let cycle = dispatch
        .observe_cycle(
            processor_id,
            1,
            &span(384),
            EngineFrame::new(ClockEpochId::new(0), 0),
            384,
        )
        .expect("gen-1 node observed");
    let obs = observation_of(&cycle, target).expect("target observed");
    assert!(
        matches!(obs.stage, DispatchStage::Dispatched { .. }),
        "pre-drain stage is Dispatched: {obs:?}"
    );

    // Host-side drain → `record_reported` → `Reported`.
    let report = drain_host(&mut dispatch, &eng, &host, 1);
    assert!(report.attached);
    assert_eq!(report.generation, 1);
    assert!(!report.generation_gated && !report.inactive_gated);
    assert_eq!(report.drained, 1, "one endpoint drained: {report:?}");
    // announce batch (3 events: param7, param99, midi) + one echo.
    assert_eq!(report.events, 4, "{report:?}");
    assert_eq!(report.param_reports, 3, "{report:?}");
    assert_eq!(
        report.reported, 1,
        "announced param7 → Reported: {report:?}"
    );
    assert_eq!(report.unbound_param, 1, "param 99 unbound: {report:?}");
    assert_eq!(report.no_pending, 1, "echo after Reported: {report:?}");
    assert_eq!(report.errors.len(), 0, "{report:?}");

    // The ledger now reports the plugin's value — 0.5 announced,
    // never the dispatched 0.8 nor the 0.4 echo.
    let cycle = dispatch
        .observe_cycle(
            processor_id,
            1,
            &span(384),
            EngineFrame::new(ClockEpochId::new(0), 0),
            384,
        )
        .expect("gen-1 node observed");
    let obs = observation_of(&cycle, target).expect("target observed");
    assert!(
        matches!(obs.stage, DispatchStage::Reported),
        "post-drain stage is Reported: {obs:?}"
    );
    assert_eq!(
        obs.value.map(ParamValue::get),
        Some(0.5),
        "plugin-reported value wins: {obs:?}"
    );
}

/// PC-C4-03-02: `ReportDrain::Unavailable` terminates pending ledger
/// entries honestly — `Unavailable` stage, no value, no success
/// pretense. (Real L1 `Unavailable` is `NotRun` at this pin; the
/// injected seam exercises the identical branch.)
#[test]
fn pc_c4_03_02_unavailable_terminates_pending_entries() {
    let root = scratch("c40302");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (target, _lane) = add_gain_target(&mut c, processor_id);
    let (adapter, _instances) = report_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    publish_and_run(&mut eng, &c, &media, 1);
    let mut dispatch = attach_surface(&mut eng, &c, processor_id, 1);

    let rev = c.live_revision();
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 300, 0.8, rev),
        active_generation(&eng),
    );
    assert!(matches!(outcome, FastControlOutcome::Delivered));
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let endpoints = eng.plugin_state_handles(1);
    assert_eq!(endpoints.len(), 1, "one plugin endpoint vended");
    let report = drain_with(
        &mut dispatch,
        &endpoints,
        1,
        active_generation(&eng),
        &mut |_handle, _out| Ok(ReportDrain::Unavailable),
    );
    assert_eq!(report.unavailable, 1, "{report:?}");
    assert_eq!(report.drained, 0);
    assert_eq!(report.failed, 0);

    let cycle = dispatch
        .observe_cycle(
            processor_id,
            1,
            &span(384),
            EngineFrame::new(ClockEpochId::new(0), 0),
            384,
        )
        .expect("gen-1 node observed");
    let obs = observation_of(&cycle, target).expect("target observed");
    assert!(
        matches!(obs.stage, DispatchStage::Unavailable),
        "honest terminal — no report channel: {obs:?}"
    );
    assert_eq!(obs.value, None, "Unavailable carries no value");
}

/// PC-C4-03-03: generation gates — a drain attesting the wrong
/// generation refuses every endpoint before any host call; reports
/// stay queued plugin-side and a later correct drain still sees
/// them. `observe_cycle` fails closed the same way.
#[test]
fn pc_c4_03_03_generation_gate_refuses_foreign_reports() {
    let root = scratch("c40303");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (target, _lane) = add_gain_target(&mut c, processor_id);
    let (adapter, _instances) = report_host();
    let host = adapter.host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    publish_and_run(&mut eng, &c, &media, 1);
    let mut dispatch = attach_surface(&mut eng, &c, processor_id, 1);

    let rev = c.live_revision();
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 300, 0.8, rev),
        active_generation(&eng),
    );
    assert!(matches!(outcome, FastControlOutcome::Delivered));
    eng.run(2, PumpPolicy::Every(1)).unwrap();

    let endpoints = eng.plugin_state_handles(1);

    // Wrong attested generation → gated before any drain runs.
    let gated = drain_with(
        &mut dispatch,
        &endpoints,
        2,
        active_generation(&eng),
        &mut |handle, out| host.borrow_mut().drain_output_reports(handle, out),
    );
    assert!(gated.generation_gated, "{gated:?}");
    assert_eq!(gated.drained, 0);
    assert_eq!(gated.events, 0);

    // `observe_cycle` fails closed on the same mismatch.
    assert!(
        dispatch
            .observe_cycle(
                processor_id,
                2,
                &span(384),
                EngineFrame::new(ClockEpochId::new(0), 0),
                384,
            )
            .is_none(),
        "foreign generation window projects nothing"
    );

    // Reports were not consumed — the correct drain still sees them.
    let report = drain_with(
        &mut dispatch,
        &endpoints,
        1,
        active_generation(&eng),
        &mut |handle, out| host.borrow_mut().drain_output_reports(handle, out),
    );
    assert_eq!(report.reported, 1, "pending reports survived: {report:?}");
}

/// PC-C4-03-04: the active-generation gate — a published surface
/// whose generation RT has not observed drains nothing; reports
/// stay queued plugin-side and land on the first active drain.
#[test]
fn pc_c4_03_04_inactive_gate_keeps_reports_queued() {
    let root = scratch("c40304");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (target, _lane) = add_gain_target(&mut c, processor_id);
    let (adapter, _instances) = report_host();
    let host = adapter.host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    // Publish WITHOUT running — RT has observed no activation.
    let rev = c.live_revision();
    let prepared = eng
        .prepare(
            &request(1, rev),
            c.live(),
            rev,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(prepared, c.live(), rev).unwrap();
    assert_eq!(active_generation(&eng), None);

    let mut dispatch = attach_surface(&mut eng, &c, processor_id, 1);

    // Write queues (inactive) — no delivery yet.
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 300, 0.8, rev),
        active_generation(&eng),
    );
    assert!(matches!(
        outcome,
        FastControlOutcome::Queued { .. } | FastControlOutcome::Delivered
    ));

    // Run so the plugin announces + (maybe) receives the write.
    eng.run(2, PumpPolicy::Every(1)).unwrap();
    assert_eq!(active_generation(&eng), Some(1));

    // Drain attesting "not active" → gated, nothing consumed.
    let endpoints = eng.plugin_state_handles(1);
    let gated = drain_with(
        &mut dispatch,
        &endpoints,
        1,
        None,
        &mut |handle, out| host.borrow_mut().drain_output_reports(handle, out),
    );
    assert!(gated.inactive_gated, "{gated:?}");
    assert_eq!(gated.drained, 0);
    assert_eq!(gated.events, 0);

    // First active drain sees everything still queued plugin-side.
    let report = drain_with(
        &mut dispatch,
        &endpoints,
        1,
        active_generation(&eng),
        &mut |handle, out| host.borrow_mut().drain_output_reports(handle, out),
    );
    assert!(!report.inactive_gated);
    assert!(report.drained >= 1, "{report:?}");
    assert!(report.events > 0, "queued reports landed: {report:?}");
}

/// PC-C4-03-05: typed non-ledger outcomes — unbound `ParamId`,
/// `Midi` reports and bound params with no eligible entry are
/// counters, never false observations.
#[test]
fn pc_c4_03_05_unbound_and_non_param_reports_stay_out_of_ledger() {
    let root = scratch("c40305");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (_target, _lane) = add_gain_target(&mut c, processor_id);
    let (adapter, _instances) = report_host();
    let host = adapter.host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    publish_and_run(&mut eng, &c, &media, 1);
    let mut dispatch = attach_surface(&mut eng, &c, processor_id, 1);
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    // Drain BEFORE any write: announce batch only — param 7 has no
    // eligible ledger entry, param 99 is unbound, midi is ignored.
    let report = drain_host(&mut dispatch, &eng, &host, 1);
    assert_eq!(report.drained, 1, "{report:?}");
    assert_eq!(report.events, 3, "announce batch: {report:?}");
    assert_eq!(report.param_reports, 2, "{report:?}");
    assert_eq!(report.reported, 0, "nothing eligible: {report:?}");
    assert_eq!(report.no_pending, 1, "bound param, no entry: {report:?}");
    assert_eq!(report.unbound_param, 1, "param 99: {report:?}");

    // No observation was minted for the unbound param — the ledger
    // only tracks dispatched targets.
    let cycle = dispatch
        .observe_cycle(
            processor_id,
            1,
            &span(384),
            EngineFrame::new(ClockEpochId::new(0), 0),
            384,
        )
        .expect("gen-1 node observed");
    assert!(
        cycle.observations.is_empty(),
        "no write → no observations: {:?}",
        cycle.observations
    );
}

/// PC-C4-03-06: host errors are typed evidence — `InvalidHandle`
/// (stale/dead instance) terminates pending entries `Unavailable`;
/// a non-terminal `HostError` leaves the ledger untouched.
#[test]
fn pc_c4_03_06_host_errors_are_typed() {
    let root = scratch("c40306");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (target, _lane) = add_gain_target(&mut c, processor_id);
    let (adapter, _instances) = report_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    publish_and_run(&mut eng, &c, &media, 1);
    let mut dispatch = attach_surface(&mut eng, &c, processor_id, 1);

    let rev = c.live_revision();
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 300, 0.8, rev),
        active_generation(&eng),
    );
    assert!(matches!(outcome, FastControlOutcome::Delivered));
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let endpoints = eng.plugin_state_handles(1);

    // Non-terminal error → typed evidence, ledger untouched.
    let failed = drain_with(
        &mut dispatch,
        &endpoints,
        1,
        active_generation(&eng),
        &mut |_handle, _out| Err(HostError::Ipc),
    );
    assert_eq!(failed.failed, 1, "{failed:?}");
    assert_eq!(failed.unavailable, 0);
    assert_eq!(failed.errors.len(), 1);
    assert_eq!(failed.errors[0].0, processor_id);
    assert_eq!(failed.errors[0].1, HostError::Ipc);

    let cycle = dispatch
        .observe_cycle(
            processor_id,
            1,
            &span(384),
            EngineFrame::new(ClockEpochId::new(0), 0),
            384,
        )
        .expect("gen-1 node observed");
    let obs = observation_of(&cycle, target).expect("target observed");
    assert!(
        matches!(obs.stage, DispatchStage::Dispatched { .. }),
        "non-terminal error leaves Dispatched: {obs:?}"
    );

    // Stale/failed handle → terminal: pending entries → Unavailable.
    let stale = drain_with(
        &mut dispatch,
        &endpoints,
        1,
        active_generation(&eng),
        &mut |_handle, _out| Err(HostError::InvalidHandle),
    );
    assert_eq!(stale.unavailable, 1, "{stale:?}");
    assert_eq!(stale.errors.len(), 1);

    let cycle = dispatch
        .observe_cycle(
            processor_id,
            1,
            &span(384),
            EngineFrame::new(ClockEpochId::new(0), 0),
            384,
        )
        .expect("gen-1 node observed");
    let obs = observation_of(&cycle, target).expect("target observed");
    assert!(
        matches!(obs.stage, DispatchStage::Unavailable),
        "dead instance terminates honestly: {obs:?}"
    );
}

/// PC-C4-03-07: drain-once semantics — a second drain sees an empty
/// pending buffer (`Drained` with zero events), never re-applying
/// reports; scheduler (`observe_cycle`) and host drains own
/// disjoint channels.
#[test]
fn pc_c4_03_07_repeated_drains_do_not_duplicate() {
    let root = scratch("c40307");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (target, _lane) = add_gain_target(&mut c, processor_id);
    let (adapter, _instances) = report_host();
    let host = adapter.host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    publish_and_run(&mut eng, &c, &media, 1);
    let mut dispatch = attach_surface(&mut eng, &c, processor_id, 1);

    let rev = c.live_revision();
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 300, 0.8, rev),
        active_generation(&eng),
    );
    assert!(matches!(outcome, FastControlOutcome::Delivered));
    eng.run(2, PumpPolicy::Every(1)).unwrap();

    let first = drain_host(&mut dispatch, &eng, &host, 1);
    assert_eq!(first.reported, 1, "{first:?}");
    assert!(first.events > 0);

    let second = drain_host(&mut dispatch, &eng, &host, 1);
    assert_eq!(second.drained, 1, "still Drained: {second:?}");
    assert_eq!(second.events, 0, "pending buffer consumed once");
    assert_eq!(second.param_reports, 0);
    assert_eq!(second.reported, 0);
    assert_eq!(second.no_pending, 0);
}

/// PC-C4-03-08: generation swap — reports vended for the retired
/// generation are refused by the surface gate; the new generation's
/// bindings are never touched by foreign reports.
#[test]
fn pc_c4_03_08_generation_swap_gates_old_reports() {
    let root = scratch("c40308");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (_target, _lane) = add_gain_target(&mut c, processor_id);
    let (adapter, instances) = report_host();
    let host = adapter.host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    publish_and_run(&mut eng, &c, &media, 1);
    let mut dispatch = attach_surface(&mut eng, &c, processor_id, 1);

    // Semantic change → generation 2.
    let (lane, source) = lane_and_source(&c);
    add_clip(&mut c, lane, source, 480, 960, 0);
    let rev2 = c.live_revision();
    let prepared = eng
        .prepare(
            &request(2, rev2),
            c.live(),
            rev2,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(prepared, c.live(), rev2).unwrap();
    eng.run(2, PumpPolicy::Every(1)).unwrap();
    assert_eq!(active_generation(&eng), Some(2));

    // Surface swaps to gen 2 — fresh sessions/instance ids.
    let outcome = dispatch
        .sync(&mut eng, 2, c.live(), &catalog(processor_id))
        .unwrap();
    assert!(matches!(outcome, synvelis_player::SyncOutcome::Swapped(_)));

    // Gen-1 endpoints attesting gen 1 → gated; gen-2 surface ledger
    // untouched by the retired generation's reports.
    let gen1_endpoints = eng.plugin_state_handles(1);
    let gated = drain_with(
        &mut dispatch,
        &gen1_endpoints,
        1,
        active_generation(&eng),
        &mut |handle, out| host.borrow_mut().drain_output_reports(handle, out),
    );
    assert!(gated.generation_gated, "{gated:?}");
    assert_eq!(gated.drained, 0);

    // Gen-2 endpoints drain against gen-2 bindings normally.
    let gen2_endpoints = eng.plugin_state_handles(2);
    assert!(!gen2_endpoints.is_empty(), "gen-2 endpoints vended");
    let report = drain_with(
        &mut dispatch,
        &gen2_endpoints,
        2,
        active_generation(&eng),
        &mut |handle, out| host.borrow_mut().drain_output_reports(handle, out),
    );
    assert!(
        !report.generation_gated && !report.inactive_gated,
        "{report:?}"
    );
    assert!(report.drained >= 1, "{report:?}");

    // The gen-2 instance announced its own param-7 report — with no
    // gen-2 write pending it lands `no_pending`, never `reported`
    // against a stale binding.
    assert_eq!(report.reported, 0, "no gen-2 write → no Reported");
    assert!(report.no_pending >= 1, "{report:?}");

    // Per-generation instance isolation: two plugin instances exist.
    assert_eq!(instances.lock().unwrap().len(), 2);
}
