//! SF-03 slice 6 acceptance: SF-ACT-06 headless consumer —
//! generation-scoped automation→`TimedParamEvent` wiring, the
//! fast-control active-generation gate, and the bounded
//! re-validated queue, plus the cross-consumer same-revision/
//! generation evidence fixture.
//!
//! Evidence under test ([`RoutingControlDispatch`]):
//!
//! - Automation evaluation lowers through the *active* generation's
//!   verified [`PluginBindingSession`] pins — a published-but-not-
//!   RT-observed generation dispatches nothing.
//! - Fast controls deliver only while `rt_status().active_generation`
//!   equals the surface generation; otherwise they queue (bounded,
//!   `QueueFull` rejects) and replay re-resolves through the current
//!   surface's bindings — never a stale `(node, param)` pair.
//! - Binding `graph_generation` pins, the engine's published
//!   [`RoutingSourceMap`] and the activation receipt all name the
//!   same `(project_revision, generation)` — the cross-consumer
//!   correlation contract SF-04 relies on.
//!
//! Headless only: real OS/plugin/GUI paths are `NotRun` (see the slice
//! spec's deferred concerns).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use synvelis_player::RoutingControlDispatch;
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::{PluginHostProcessors, StaticProcessorHost};
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::resolver::{PreparedClip, prepare};
use synvelis_player::routing_control::{FastControlOutcome, FastControlWrite, ParamCatalog};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine, RoutingPrepare};
use synvelis_player::routing_lower::GainProcessor;
use synvelis_project::{
    AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange, BuildBudget,
    BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, ConflictPolicy,
    ControlOwner, CounterIdGenerator, Interpolation, MediaDuration, MediaTimeValue, ParamMapping,
    ParameterRole, ParameterTargetId, PlaybackMapping, ProcessorBinding, ProcessorId, Project,
    ProjectController, ProjectId, ProjectOp, ProjectRevision, RoutingBuildRequestId,
    RoutingCompileRequest, RuntimeClockEpochRef, RuntimeInstanceId, SourceFrame, SourceId,
    SourceRange, TargetCapability, TimeAnchor, ValueKind,
};

use ruviform_audio::typed::{GraphProcessor, NodeIo, OfflineRenderSupport};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_core::{MediaTime, TimeBase};
use ruviform_media_reader::PrefetchConfig;
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamFlags, ParamId, ParamInfo, Plugin, PluginDescriptor, PluginFactory,
    PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult, TransportInfo,
    erase, erase_sendable,
};
use ruviform_plugin_host::PluginHost;
use ruviform_rt::RtContext;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf03l6-{}-{}-{}",
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
        Project::new(ProjectId::new(0x106), "sf03l6"),
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

/// `minimal_project` plus one chain processor bound to the param
/// recorder plugin.
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
/// (the recorder's only parameter) plus a linear automation lane
/// 0.2 → 0.6 over beats [0, 4). Returns `(target, lane)`.
fn add_gain_automation(
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

/// The caller-vended catalog the surface binds against — the recorder
/// plugin's live `ParamInfo` enumeration.
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
    id: "org.synvelis.test.sf03-l6-rec",
    name: "SF03-L6 Recorder",
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

struct RecPlugin {
    delivered: Arc<Mutex<Vec<Delivered>>>,
}

impl PluginProcessor for RecPlugin {
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
        for ev in events.iter() {
            if let EventKind::ParamValue(pe) = ev.kind() {
                #[allow(clippy::cast_sign_loss)]
                self.delivered.lock().unwrap().push(Delivered {
                    abs_frame: (pos + i64::from(ev.frame_offset())) as u64,
                    param_id: pe.param_id.0,
                    value: pe.value.get(),
                });
            }
        }
        ProcessResult::CONTINUE
    }
}

impl Plugin for RecPlugin {
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

struct RecFactory {
    instances: InstanceLogs,
}

impl PluginFactory for RecFactory {
    fn descriptor(&self) -> PluginDescriptor {
        REC_DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        let delivered = Arc::new(Mutex::new(Vec::new()));
        self.instances.lock().unwrap().push(Arc::clone(&delivered));
        erase(RecPlugin { delivered })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        let delivered = Arc::new(Mutex::new(Vec::new()));
        self.instances.lock().unwrap().push(Arc::clone(&delivered));
        Some(erase_sendable(RecPlugin { delivered }))
    }
}

fn rec_host() -> (PluginHostProcessors, InstanceLogs) {
    let instances = Arc::new(Mutex::new(Vec::new()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory {
        instances: Arc::clone(&instances),
    }));
    (PluginHostProcessors::new(host), instances)
}

/// Test-scoped `OfflineRenderSupport::Verified` declaration — same
/// rationale as `sf03_l2`: the upstream `GainProcessor` keeps the
/// honest `Unknown` default.
struct VerifiedGain(f32);

impl GraphProcessor for VerifiedGain {
    fn process(&mut self, io: &mut NodeIo<'_>, ctx: &RtContext) {
        GainProcessor::new(self.0).process(io, ctx);
    }

    fn offline_support(&self) -> OfflineRenderSupport {
        OfflineRenderSupport::Verified
    }
}

// ---------------------------------------------------------------- tests

/// Example: automation evaluates through the active generation's
/// verified binding — `eval → lower_event → session.dispatch →
/// install_plan` delivers the lane's value (0.4 at beat 2 of the
/// 0.2→0.6 ramp) to the plugin at the stamped frame. Also carries the
/// cross-consumer fixture: source map, receipt and binding pins all
/// name the same `(revision, generation)`.
#[test]
fn sf03_l6_01_automation_tick_delivers_on_active_generation() {
    let root = scratch("l601");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (target, _lane) = add_gain_automation(&mut c, processor_id);
    let (adapter, instances) = rec_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    let rev = c.live_revision();
    let p1 = eng
        .prepare(
            &request(1, rev),
            c.live(),
            rev,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(p1, c.live(), rev).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    assert_eq!(active_generation(&eng), Some(1));

    // Cross-consumer evidence: the published source map, the active
    // receipt and the control surface's binding pins agree on
    // (project_revision, generation).
    let map = eng.source_map().unwrap();
    assert_eq!(map.project_revision, rev);
    assert_eq!(map.generation, 1);
    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(processor_id)).unwrap();
    assert_eq!(dispatch.generation(), Some(1));
    let report = dispatch.report().unwrap();
    assert_eq!(
        report.bound_targets, 1,
        "capability target bound: {report:?}"
    );
    assert!(report.unbound.is_empty(), "unbound: {:?}", report.unbound);
    let binding = dispatch.binding(target).expect("verified binding record");
    assert_eq!(binding.graph_generation, 1);
    assert_eq!(binding.synvelis_target, target);
    assert_eq!(binding.plugin_param, ParamId(7));

    // Automation tick at beat 2 (→ 0.4), event stamped at frame 300 —
    // inside the next 256-frame block (cursor is at 256).
    let tick = dispatch.tick(
        c.live(),
        BeatPoint::new(2, 1).unwrap(),
        MediaTime::new(300, frame_base()),
        active_generation(&eng),
    );
    assert!(!tick.gated && !tick.stale, "{tick:?}");
    assert_eq!(tick.evaluated, 1);
    assert_eq!(tick.events, 1);
    assert_eq!(tick.nodes_dispatched, 1);
    assert_eq!(tick.dispatch_errors, 0);

    // Three blocks: the staged plan applies and the frame-300 event
    // lands inside a processed block's window.
    eng.run(3, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    let log = instances.lock().unwrap()[0].lock().unwrap().clone();
    assert_eq!(log.len(), 1, "one automation event delivered: {log:?}");
    assert_eq!(log[0].param_id, 7);
    assert!(
        (log[0].value - 0.4).abs() < 1e-9,
        "beat-2 ramp value: {log:?}"
    );
    assert_eq!(log[0].abs_frame, 300);
}

/// Gate evidence: before `RtRuntime::process` observes the generation
/// the surface reports `gated` and fast controls queue; after
/// activation the same calls deliver.
#[test]
fn sf03_l6_02_gate_holds_until_rt_observes_activation() {
    let root = scratch("l602");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (target, _lane) = add_gain_automation(&mut c, processor_id);
    let rev = c.live_revision();
    let (adapter, instances) = rec_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    let p1 = eng
        .prepare(
            &request(1, rev),
            c.live(),
            rev,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(p1, c.live(), rev).unwrap();
    // Published but never processed → RT has observed nothing.
    assert_eq!(eng.rt_status().active_generation, 0);

    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(processor_id)).unwrap();
    let gated_tick = dispatch.tick(
        c.live(),
        BeatPoint::new(2, 1).unwrap(),
        MediaTime::new(64, frame_base()),
        active_generation(&eng),
    );
    assert!(gated_tick.gated, "{gated_tick:?}");
    assert_eq!(gated_tick.events, 0);

    let queued = dispatch.fast_control(
        c.live(),
        fast_write(target, 320, 0.9, rev),
        active_generation(&eng),
    );
    assert!(
        matches!(queued, FastControlOutcome::Queued { depth: 1 }),
        "{queued:?}"
    );
    assert_eq!(dispatch.queue_depth(), 1);

    // Activation observed → automation + queued fast control land.
    eng.run(2, PumpPolicy::Every(1)).unwrap();
    assert_eq!(active_generation(&eng), Some(1));
    let tick = dispatch.tick(
        c.live(),
        BeatPoint::new(2, 1).unwrap(),
        MediaTime::new(600, frame_base()),
        active_generation(&eng),
    );
    assert!(!tick.gated, "{tick:?}");
    assert_eq!(tick.queue_delivered, 1, "queued write drained: {tick:?}");
    assert_eq!(tick.events, 2, "automation + fast write: {tick:?}");
    assert_eq!(tick.nodes_dispatched, 1, "{tick:?}");
    assert_eq!(tick.dispatch_errors, 0, "{tick:?}");

    eng.run(4, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    let log = instances.lock().unwrap()[0].lock().unwrap().clone();
    assert_eq!(log.len(), 2, "automation + queued fast write: {log:?}");
    assert!(log.iter().any(|d| (d.value - 0.4).abs() < 1e-9));
    assert!(log.iter().any(|d| (d.value - 0.9).abs() < 1e-9));
}

/// Fast-control contract: immediate delivery while active, typed
/// blocks for unbound targets and unsupported means, and the bounded
/// queue rejecting overflow without evicting older writes.
#[test]
fn sf03_l6_03_fast_control_gate_queue_and_typed_blocks() {
    let root = scratch("l603");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (target, _lane) = add_gain_automation(&mut c, processor_id);
    let rev = c.live_revision();
    let (adapter, instances) = rec_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    let p1 = eng
        .prepare(
            &request(1, rev),
            c.live(),
            rev,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(p1, c.live(), rev).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(processor_id))
            .unwrap()
            .with_queue_capacity(2);

    // Active → immediate delivery; let RT observe the staged plan
    // before the next control call (plan-replace semantics).
    let delivered = dispatch.fast_control(
        c.live(),
        fast_write(target, 300, 0.8, rev),
        active_generation(&eng),
    );
    assert_eq!(delivered, FastControlOutcome::Delivered);
    // Two blocks: the staged plan activates and the frame-300 event
    // delivers before the next control call can replace the plan.
    eng.run(2, PumpPolicy::Every(1)).unwrap();

    // Unbound target (no such target id) → typed block, not a queue.
    let ghost = ParameterTargetId::new(0xDEAD);
    let blocked = dispatch.fast_control(
        c.live(),
        fast_write(ghost, 300, 0.5, rev),
        active_generation(&eng),
    );
    assert!(matches!(
        blocked,
        FastControlOutcome::Blocked(synvelis_player::FastControlBlockKind::UnboundTarget)
    ));
    assert!(matches!(
        dispatch.last_block(),
        Some(synvelis_player::FastControlBlock::UnboundTarget)
    ));

    // Unsupported means → Lowering block through lower_event's gate.
    let mut immediate = fast_write(target, 300, 0.5, rev);
    immediate.means = ruviform_plugin_core::ParamWriteMeans::Immediate;
    let blocked = dispatch.fast_control(c.live(), immediate, active_generation(&eng));
    assert!(matches!(
        blocked,
        FastControlOutcome::Blocked(synvelis_player::FastControlBlockKind::Lowering)
    ));

    // Queue bound: capacity 2 → third write while *inactive* rejects.
    // `Some(0)` simulates "no RT-observed activation for this
    // surface" → writes queue; capacity 2 → third is QueueFull.
    // Positions stay ahead of the cursor (768): past-position events
    // beyond the scheduler's deliverable window drop as undeliverable.
    let inactive = Some(0);
    let q1 = dispatch.fast_control(c.live(), fast_write(target, 900, 0.1, rev), inactive);
    let q2 = dispatch.fast_control(c.live(), fast_write(target, 1_000, 0.2, rev), inactive);
    let q3 = dispatch.fast_control(c.live(), fast_write(target, 1_100, 0.33, rev), inactive);
    assert!(matches!(q1, FastControlOutcome::Queued { depth: 1 }));
    assert!(matches!(q2, FastControlOutcome::Queued { depth: 2 }));
    assert!(matches!(
        q3,
        FastControlOutcome::Blocked(synvelis_player::FastControlBlockKind::QueueFull)
    ));
    assert_eq!(dispatch.queue_depth(), 2, "overflow rejected, none evicted");

    // Reactivate → tick drains FIFO: both queued writes + the
    // automation event deliver (positions 900/1000/1050 all ahead of
    // the 768-frame cursor).
    let tick = dispatch.tick(
        c.live(),
        BeatPoint::new(1, 1).unwrap(),
        MediaTime::new(1_050, frame_base()),
        active_generation(&eng),
    );
    assert_eq!(tick.queue_delivered, 2, "{tick:?}");
    assert_eq!(tick.queue_dropped, 0);
    assert_eq!(tick.events, 3, "{tick:?}");
    assert_eq!(tick.dispatch_errors, 0, "{tick:?}");

    eng.run(4, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    let log = instances.lock().unwrap()[0].lock().unwrap().clone();
    assert!(log.iter().any(|d| (d.value - 0.8).abs() < 1e-9), "{log:?}");
    assert!(log.iter().any(|d| (d.value - 0.1).abs() < 1e-9), "{log:?}");
    assert!(log.iter().any(|d| (d.value - 0.2).abs() < 1e-9), "{log:?}");
    assert!(
        !log.iter().any(|d| (d.value - 0.33).abs() < 1e-9),
        "overflowed write never delivered: {log:?}"
    );
}

/// Generation swap: the queue survives `sync` and replays through the
/// *new* generation's verified bindings — the same semantic target
/// reaches the new plugin instance's param, never the retired one.
/// Dropped-when-unbound evidence rides along.
#[test]
fn sf03_l6_04_queue_replays_through_new_generation_bindings() {
    let root = scratch("l604");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (target, _lane) = add_gain_automation(&mut c, processor_id);
    let (adapter, instances) = rec_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    let rev1 = c.live_revision();
    let p1 = eng
        .prepare(
            &request(1, rev1),
            c.live(),
            rev1,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(p1, c.live(), rev1).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(processor_id)).unwrap();
    let binding1 = dispatch.binding(target).unwrap();
    assert_eq!(binding1.graph_generation, 1);

    // Semantic change → gen 2 builds/publishes; RT still observes gen 1.
    let (lane, source) = lane_and_source(&c);
    add_clip(&mut c, lane, source, 480, 960, 0);
    let rev2 = c.live_revision();
    let p2 = eng
        .prepare(
            &request(2, rev2),
            c.live(),
            rev2,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(p2, c.live(), rev2).unwrap();
    assert_eq!(
        eng.rt_status().active_generation,
        1,
        "gen-2 not yet observed"
    );

    // Swap the surface to the published gen-2 controls.
    let synced = dispatch
        .sync(&mut eng, 2, c.live(), &catalog(processor_id))
        .unwrap();
    assert!(matches!(synced, synvelis_player::SyncOutcome::Swapped(_)));
    assert_eq!(dispatch.generation(), Some(2));
    let binding2 = dispatch.binding(target).unwrap();
    assert_eq!(binding2.graph_generation, 2);
    // PC-C4-05: `instance_id` is the persisted authored identity —
    // stable across generations for the same processor/plugin pair.
    // Freshness lives in the generation pins (asserted above) and the
    // per-generation session/node, so the records still differ.
    assert_eq!(
        binding2.instance_id, binding1.instance_id,
        "persisted instance identity is generation-stable"
    );
    assert_ne!(binding2, binding1, "gen-2 binding is a fresh record");

    // While RT still runs gen 1, writes for the gen-2 surface queue —
    // they must not leak into the still-live gen-1 plugin.
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 900, 0.7, rev2),
        active_generation(&eng),
    );
    assert!(matches!(outcome, FastControlOutcome::Queued { depth: 1 }));

    eng.run(2, PumpPolicy::Every(1)).unwrap();
    assert_eq!(active_generation(&eng), Some(2));

    // Drain on the now-active gen-2 surface → re-resolved binding →
    // delivered to instance[1] (gen-2's plugin), never instance[0].
    let tick = dispatch.tick(
        c.live(),
        BeatPoint::new(2, 1).unwrap(),
        MediaTime::new(1_100, frame_base()),
        active_generation(&eng),
    );
    assert_eq!(tick.queue_delivered, 1, "{tick:?}");
    assert_eq!(tick.queue_dropped, 0);
    assert_eq!(tick.nodes_dispatched, 1, "{tick:?}");
    assert_eq!(tick.dispatch_errors, 0, "{tick:?}");

    eng.run(4, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    let instances = instances.lock().unwrap();
    assert_eq!(instances.len(), 2, "one plugin instance per generation");
    let log1 = instances[0].lock().unwrap().clone();
    let log2 = instances[1].lock().unwrap().clone();
    assert!(
        log2.iter().any(|d| (d.value - 0.7).abs() < 1e-9),
        "queued write replayed on gen-2 instance: gen1={log1:?} gen2={log2:?}"
    );
    assert!(
        !log1.iter().any(|d| (d.value - 0.7).abs() < 1e-9),
        "retired gen-1 instance never saw the replayed write: {log1:?}"
    );
}

/// Equivalent path: a name-only revision keeps the generation —
/// `sync` reports `Current`, sessions persist (controls are
/// one-shot), and a control-model change (new automation point)
/// refreshes the plan and re-evaluates without a rebuild.
#[test]
fn sf03_l6_05_equivalent_revision_refreshes_control_surface() {
    let root = scratch("l605");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (target, lane) = add_gain_automation(&mut c, processor_id);
    let (adapter, instances) = rec_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    let rev1 = c.live_revision();
    let p1 = eng
        .prepare(
            &request(1, rev1),
            c.live(),
            rev1,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(p1, c.live(), rev1).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(processor_id)).unwrap();

    // Control-model-only change: rewrite the lane's ramp to 0.8→1.0.
    // The routing fingerprint does not cover the control model, so
    // prepare reports Equivalent — same generation, same controls.
    apply(
        &mut c,
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
                        value: 0.8,
                    },
                    AutomationPoint {
                        pos: BeatPoint::new(4, 1).unwrap(),
                        value: 1.0,
                    },
                ],
            },
            policy: ConflictPolicy::Replace,
        },
    );
    let rev2 = c.live_revision();
    let prepared = eng.prepare(
        &request(2, rev2),
        c.live(),
        rev2,
        deps_default(c.live(), &media),
    );
    let RoutingPrepare::Equivalent(_) = prepared.unwrap() else {
        panic!("control-only change must not rebuild the graph");
    };

    // Same-generation sync refreshes the stale EvalPlan and rebinds —
    // no controls re-vended (one-shot), sessions persist.
    let synced = dispatch
        .sync(&mut eng, 1, c.live(), &catalog(processor_id))
        .unwrap();
    assert!(matches!(synced, synvelis_player::SyncOutcome::Current(_)));
    assert_eq!(dispatch.generation(), Some(1));

    // Beat 2 of the new 0.8→1.0 ramp → 0.9.
    let tick = dispatch.tick(
        c.live(),
        BeatPoint::new(2, 1).unwrap(),
        MediaTime::new(500, frame_base()),
        active_generation(&eng),
    );
    assert_eq!(tick.events, 1, "{tick:?}");
    eng.run(2, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    let log = instances.lock().unwrap()[0].lock().unwrap().clone();
    assert!(
        log.iter().any(|d| (d.value - 0.9).abs() < 1e-9),
        "refreshed plan evaluated the new lane: {log:?}"
    );
    let _ = target;
}

/// Offline-mode fixture: an offline-built generation attaches the
/// same control surface contract — with a static (non-plugin) host
/// there is no control half, so the surface reports
/// `without_control` and no delivery is ever faked.
#[test]
fn sf03_l6_06_offline_build_attaches_without_plugin_controls() {
    let root = scratch("l606");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    let strip = strip_of_track(c.live(), track);
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "gain".into(),
            binding: ProcessorBinding::Unresolved {
                descriptor: "test.gain".into(),
            },
            sidechain_input: false,
        },
    );
    let processor_id = *c.live().routing.processors.keys().next_back().unwrap();

    let mut eng = new_engine();
    let mut host = StaticProcessorHost::new();
    host.insert(processor_id, Box::new(VerifiedGain(0.5)));
    eng.set_processor_host(Box::new(host));

    let rev = c.live_revision();
    let offline = RoutingCompileRequest {
        mode: BuildMode::Offline,
        ..request(1, rev)
    };
    let p1 = eng
        .prepare(&offline, c.live(), rev, deps_default(c.live(), &media))
        .unwrap()
        .expect_built();
    eng.publish(p1, c.live(), rev).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    // Static connections vend no control — the surface attaches with
    // the processor listed under `without_control`, honestly empty.
    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(processor_id)).unwrap();
    let report = dispatch.report().unwrap();
    assert_eq!(report.controlled_processors, Vec::new());
    assert_eq!(report.without_control, vec![processor_id]);
    assert_eq!(report.bound_targets, 0);

    // No plugin param surface exists → a fast control can never
    // deliver; while inactive it queues, and the drain drops it typed
    // rather than fabricating a target.
    let ghost = ParameterTargetId::new(1);
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(ghost, 64, 0.5, rev),
        active_generation(&eng),
    );
    // `ghost` has no project target at all → UnboundTarget while the
    // (empty) surface is active.
    assert!(matches!(
        outcome,
        FastControlOutcome::Blocked(synvelis_player::FastControlBlockKind::UnboundTarget)
    ));
}
