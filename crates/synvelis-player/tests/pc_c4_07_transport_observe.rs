//! PC-C4-07 acceptance tests — routing-path transport mirror +
//! `observe_block` automatic span supply.
//!
//! Evidence under test:
//!
//! - [`RoutingControlDispatch::observe_block`] derives each node's
//!   [`TransportSpan`] from the per-node `TransportMirror` — the same
//!   commanded-state projection the driver path uses — instead of
//!   demanding caller-fabricated transport evidence. The physical call
//!   window stays caller-supplied, in the node's own cursor domain.
//! - `ParamDispatch::dispatch`'s internal `Play` and
//!   `boundary_stage_window` commands are mirrored through the same
//!   receipt-correction model as `driver.rs`: optimistic commanded
//!   state, generation axes corrected by drained receipts.
//! - Commanded transport changes (`Stop`/`Seek`/`Pause`/`SetLoop`/
//!   `ClearLoop`) staged through `boundary_stage_window` fold into
//!   the mirror at their absolute apply frame; applies strictly
//!   inside a window split the projection exactly like the node
//!   scheduler's own span partition.
//! - Generation gating stays fail-closed: a foreign surface
//!   generation returns `None`, and a generation swap mints a fresh
//!   mirror on the new node instances.
//!
//! Headless only. Node cursor domain: a generation's plugin nodes
//! count frames from their own attach — after `publish_and_run`'s
//! single block the next callback head is node frame 256, and each
//! `eng.run(1)` advances it by 256. `observe_block` must be called
//! once per physical window, in order — the same contract the RT run
//! loop follows — so staged `due_frames` resolve against the true
//! next callback head.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{DispatchStage, ParamValueObservation};
use ruviform_media_core::{
    ClockEpochId, EngineFrame, InBlockTransportSupport, LoopOccurrence, MediaDuration, MediaRange,
    MediaTime, PlanGeneration, ScheduleBudget, TempoMapRevision, TimeBase, TransportCommand,
    TransportGeneration, TransportStatus,
};
use ruviform_media_reader::PrefetchConfig;
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, Event,
    EventKind, EventList, ParamFlags, ParamId, ParamInfo, Plugin, PluginDescriptor, PluginFactory,
    PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult, TransportInfo,
    erase, erase_sendable,
};
use ruviform_plugin_host::PluginHost;
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::PluginHostProcessors;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::resolver::{PreparedClip, prepare};
use synvelis_player::routing_control::{FastControlOutcome, FastControlWrite, ParamCatalog};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::{BoundaryDeclaration, RoutingControlDispatch, TimingPrecision};
use synvelis_project::{
    AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange, BuildBudget,
    BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, ConflictPolicy,
    ControlOwner, CounterIdGenerator, Interpolation, MediaTimeValue, ParamMapping, ParameterRole,
    ParameterTargetId, PlaybackMapping, ProcessorBinding, ProcessorId, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRevision, RoutingBuildRequestId, RoutingCompileRequest,
    RuntimeClockEpochRef, RuntimeInstanceId, SourceFrame, SourceId, SourceRange, TargetCapability,
    TimeAnchor, ValueKind,
};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-pcc407-{}-{}-{}",
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
        Project::new(ProjectId::new(0x107), "pcc407"),
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
            synvelis_project::MediaDuration::new(
                i64::try_from(end_frame - start_frame).unwrap(),
                1,
                rate,
            )
            .unwrap(),
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

fn ef(frame: u64) -> EngineFrame {
    EngineFrame::new(ClockEpochId::new(0), frame)
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
/// plus a linear automation lane — the lane is unused (fast controls
/// carry the writes) but keeps the fixture identical to pc-c4-03's.
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
    id: "org.synvelis.test.pc-c4-07-rec",
    name: "PC-C4-07 Reporter",
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

/// One delivered parameter event at its absolute engine frame —
/// `transport.position + frame_offset`, the plugin-side evidence that
/// a projected `Dispatched` frame really reached `process`.
#[derive(Debug, Clone, PartialEq)]
struct Delivered {
    abs_frame: u64,
    param_id: u32,
    value: f64,
}

struct ReportPlugin {
    delivered: Arc<Mutex<Vec<Delivered>>>,
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

    fn drain_output_reports(&mut self, _out: &mut Vec<Event>) {}
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
        erase(ReportPlugin { delivered })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        let delivered = Arc::new(Mutex::new(Vec::new()));
        self.instances.lock().unwrap().push(Arc::clone(&delivered));
        Some(erase_sendable(ReportPlugin { delivered }))
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
/// runs one block so RT observes the activation. After this call the
/// generation's plugin nodes have processed exactly one 256-frame
/// block — their cursor domain starts at frame 0 and the next
/// callback head is node frame 256.
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

/// TIM-03 boundary declaration letting `boundary_stage_window` stage
/// transport commands on the processor.
fn boundary_decl() -> BoundaryDeclaration {
    BoundaryDeclaration {
        precision: TimingPrecision::FrameAccurateEvents,
        in_block: InBlockTransportSupport::Unknown,
        consent: None,
        budget: ScheduleBudget::new(64, 256, 16, 4096),
    }
}

/// Runs one 256-frame block and observes the window it covered — the
/// contiguous per-block observation the run loop performs. `head` is
/// the node cursor the block started at.
fn run_and_observe(
    eng: &mut RoutingEngine,
    dispatch: &mut RoutingControlDispatch,
    generation: u64,
    head: u64,
) {
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    dispatch
        .observe_block(generation, ef(head), 256)
        .unwrap_or_else(|| panic!("gen-{generation} surface observed"));
}

/// The observation for `target` in a node's merged `DispatchCycle`.
fn observation_of(
    cycle: &synvelis_player::DispatchCycle,
    target: ParameterTargetId,
) -> Option<ParamValueObservation> {
    cycle
        .observations
        .iter()
        .find(|(t, _)| *t == target)
        .map(|(_, obs)| *obs)
}

/// Standard drive to a playing, mirror-observed node: publish gen 1,
/// attach, one delivered fast write (stages `Play` + plan install at
/// node head 256), run that block, observe it — returns with the node
/// cursor at 512 and the mirror anchored `Playing` at position 0,
/// frame 256. `boundary` registers the TIM-03 declaration so
/// `boundary_stage_window` accepts transport commands.
#[allow(clippy::type_complexity)]
fn playing_surface(
    root: &Path,
    boundary: bool,
) -> (
    ProjectController,
    PathBuf,
    ProcessorId,
    ParameterTargetId,
    RoutingEngine,
    RoutingControlDispatch,
    InstanceLogs,
) {
    let (mut c, media, processor_id) = plugin_project(root);
    let (target, _lane) = add_gain_target(&mut c, processor_id);
    let (mut adapter, instances) = report_host();
    if boundary {
        adapter.register_boundary(processor_id, boundary_decl());
    }
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    publish_and_run(&mut eng, &c, &media, 1);
    assert_eq!(active_generation(&eng), Some(1));
    let mut dispatch = attach_surface(&mut eng, &c, processor_id, 1);

    // One delivered write → `Play` + plan install staged (due 0 at
    // the next head, node frame 256).
    let rev = c.live_revision();
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 300, 0.8, rev),
        active_generation(&eng),
    );
    assert!(matches!(outcome, FastControlOutcome::Delivered));
    run_and_observe(&mut eng, &mut dispatch, 1, 256);
    (c, media, processor_id, target, eng, dispatch, instances)
}

// ---------------------------------------------------------------- tests

/// PC-C4-07-01: before any dispatch the mirror reports the
/// scheduler-initial `Stopped` state; after `fast_control`'s internal
/// `Play` applies at node head 256, `observe_block` derives a
/// `Playing` span whose context carries scheduler-truth generations —
/// transport 1 (Play's bump, receipt-confirmed), plan 1 (the staged
/// install), epoch 0, position 0 at the apply head — over the
/// caller-supplied physical window, never a fabricated span.
#[test]
fn pc_c4_07_01_observe_block_derives_transport_span() {
    let root = scratch("c40701");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (target, _lane) = add_gain_target(&mut c, processor_id);
    let (adapter, _instances) = report_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    publish_and_run(&mut eng, &c, &media, 1);
    let mut dispatch = attach_surface(&mut eng, &c, processor_id, 1);

    // No dispatch yet — the node never played: mirror = scheduler
    // initial state, span covers the caller's window verbatim. The
    // window [256,512) has not run yet; observing it early is legal —
    // no staged commands exist to misresolve.
    let cycles = dispatch
        .observe_block(1, ef(256), 256)
        .expect("gen-1 surface observed");
    assert!(cycles.contains_key(&processor_id));
    let span = dispatch
        .last_observed_span(processor_id)
        .expect("span recorded");
    assert_eq!(span.start, ef(256));
    assert_eq!(span.end, ef(512));
    assert!(span.deliverable);
    assert_eq!(span.ctx.status, TransportStatus::Stopped);
    assert_eq!(span.ctx.position_ticks, 0);
    assert_eq!(span.ctx.position_base, frame_base());
    assert_eq!(span.ctx.loop_ticks, None);
    assert_eq!(span.ctx.occurrence, LoopOccurrence::new(0));
    assert_eq!(span.ctx.clock_epoch, ClockEpochId::new(0));
    assert_eq!(span.ctx.transport_generation, TransportGeneration::new(0));
    assert_eq!(span.ctx.plan_generation, PlanGeneration::new(0));
    assert_eq!(span.ctx.tempo_revision, TempoMapRevision::new(0));

    // Delivered write → Play + install staged; the next block applies
    // both at node head 256.
    let rev = c.live_revision();
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 300, 0.8, rev),
        active_generation(&eng),
    );
    assert!(matches!(outcome, FastControlOutcome::Delivered));
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let cycles = dispatch
        .observe_block(1, ef(256), 256)
        .expect("post-play observation");
    let cycle = cycles.get(&processor_id).expect("node cycle");
    assert!(
        !cycle.receipts.is_empty(),
        "scheduler receipts drained: {cycle:?}"
    );
    let span = dispatch
        .last_observed_span(processor_id)
        .expect("span recorded");
    assert_eq!(span.ctx.status, TransportStatus::Playing);
    // Play anchored position 0 at frame 256 — position at the head is 0.
    assert_eq!(span.ctx.position_ticks, 0);
    // Play bumped transport generation once; the drained receipt
    // confirms it (pending prediction cleared by scheduler truth).
    assert_eq!(span.ctx.transport_generation, TransportGeneration::new(1));
    // The staged plan install reports plan generation 1.
    assert_eq!(span.ctx.plan_generation, PlanGeneration::new(1));
    // Event @300 fires at node frame 556 — outside [256,512), so the
    // ledger entry is still Scheduled, never a fabricated Dispatched.
    let obs = observation_of(cycle, target).expect("target observed");
    assert!(
        matches!(obs.stage, DispatchStage::Scheduled),
        "pre-window event stays Scheduled: {obs:?}"
    );
}

/// PC-C4-07-02: the mirror's position projection advances with the
/// node cursor — `position_ticks` at each observed head equals the
/// scheduler's `pos_at` — and an in-window event projects `Dispatched`
/// at the exact offset the plugin really delivered it.
#[test]
fn pc_c4_07_02_position_advances_and_event_dispatches() {
    let root = scratch("c40702");
    let (_c, _media, processor_id, target, mut eng, mut dispatch, instances) =
        playing_surface(&root, false);

    // The block that applied Play was [256,512); run and observe the
    // next one — position at head 512 is 256.
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    let cycles = dispatch
        .observe_block(1, ef(512), 256)
        .expect("gen-1 observation");
    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(span.ctx.position_ticks, 256);
    let cycle = cycles.get(&processor_id).unwrap();
    let obs = observation_of(cycle, target).expect("target observed");
    // Event @300 → node frame 556 → offset 44 inside [512,768).
    assert!(
        matches!(
            obs.stage,
            DispatchStage::Dispatched {
                call_start: 512,
                frame_offset: 44
            }
        ),
        "event projects at its real frame: {obs:?}"
    );

    // The plugin really received it: position 256 at call start +
    // offset 44 = absolute position 300 — the stamped event position.
    let delivered = instances.lock().unwrap()[0].lock().unwrap().clone();
    assert!(
        delivered
            .iter()
            .any(|d| d.param_id == 7 && d.abs_frame == 300),
        "plugin delivery matches projection: {delivered:?}"
    );

    // Next block: position keeps advancing — no fabrication, no reset.
    run_and_observe(&mut eng, &mut dispatch, 1, 768);
    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(span.ctx.position_ticks, 512);
    assert_eq!(span.ctx.status, TransportStatus::Playing);
}

/// PC-C4-07-03: generation gating stays fail-closed — a foreign
/// surface generation returns `None` for the whole block while the
/// matching generation observes every surface node.
#[test]
fn pc_c4_07_03_generation_gating_fail_closed() {
    let root = scratch("c40703");
    let (_c, _media, processor_id, _target, mut eng, mut dispatch, _instances) =
        playing_surface(&root, false);

    // Foreign generations — published-but-not-synced and never
    // published alike — mint nothing.
    assert!(dispatch.observe_block(2, ef(512), 256).is_none());
    assert!(dispatch.observe_block(0, ef(512), 256).is_none());

    // Matching generation observes every surface node.
    let cycles = dispatch
        .observe_block(1, ef(512), 256)
        .expect("gen-1 surface");
    assert_eq!(cycles.len(), 1);
    assert!(cycles.contains_key(&processor_id));
    let _ = &mut eng;
}

/// PC-C4-07-04: a commanded `Stop` through `boundary_stage_window`
/// folds into the mirror at its apply head — the next span reports
/// `Stopped` at position 0 with the bumped transport generation, and
/// position stays frozen while stopped.
#[test]
fn pc_c4_07_04_commanded_stop_projects_stopped() {
    let root = scratch("c40704");
    let (c, _media, processor_id, _target, mut eng, mut dispatch, _instances) =
        playing_surface(&root, true);

    // Playing state confirmed at head 512, then stage Stop due 0 —
    // it applies at the next head, node frame 768.
    run_and_observe(&mut eng, &mut dispatch, 1, 512);
    assert_eq!(
        dispatch
            .last_observed_span(processor_id)
            .unwrap()
            .ctx
            .status,
        TransportStatus::Playing
    );
    dispatch
        .boundary_stage_window(
            processor_id,
            c.live(),
            256,
            vec![],
            &[(TransportCommand::Stop, 0)],
            active_generation(&eng),
        )
        .expect("stop staged");
    run_and_observe(&mut eng, &mut dispatch, 1, 768);

    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(span.ctx.status, TransportStatus::Stopped);
    assert_eq!(span.ctx.position_ticks, 0);
    // Play bumped tg to 1; Stop bumps it to 2 — receipt-confirmed.
    assert_eq!(span.ctx.transport_generation, TransportGeneration::new(2));

    // Stopped position holds at 0 — the mirror does not invent drift.
    run_and_observe(&mut eng, &mut dispatch, 1, 1024);
    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(span.ctx.status, TransportStatus::Stopped);
    assert_eq!(span.ctx.position_ticks, 0);
}

/// PC-C4-07-05: a commanded `Seek` re-anchors the mirror's position —
/// the span context reports the new position from the apply head and
/// keeps advancing from there.
#[test]
fn pc_c4_07_05_commanded_seek_reanchors_position() {
    let root = scratch("c40705");
    let (c, _media, processor_id, _target, mut eng, mut dispatch, _instances) =
        playing_surface(&root, true);

    // Advance two observed blocks — position at head 1024 is 768.
    run_and_observe(&mut eng, &mut dispatch, 1, 512);
    run_and_observe(&mut eng, &mut dispatch, 1, 768);
    assert_eq!(
        dispatch
            .last_observed_span(processor_id)
            .unwrap()
            .ctx
            .position_ticks,
        512
    );

    // Seek to position 5000 due 0 → applies at the next head, 1024.
    dispatch
        .boundary_stage_window(
            processor_id,
            c.live(),
            256,
            vec![],
            &[(
                TransportCommand::Seek(MediaTime::new(5_000, frame_base())),
                0,
            )],
            active_generation(&eng),
        )
        .expect("seek staged");
    run_and_observe(&mut eng, &mut dispatch, 1, 1024);

    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(span.ctx.status, TransportStatus::Playing);
    assert_eq!(span.ctx.position_ticks, 5_000);
    // Play(1) + Seek(2) transport bumps.
    assert_eq!(span.ctx.transport_generation, TransportGeneration::new(2));

    // Position keeps advancing from the new anchor: at head 1280 it
    // is 5000 + 256 = 5256.
    run_and_observe(&mut eng, &mut dispatch, 1, 1280);
    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(span.ctx.position_ticks, 5_256);
}

/// PC-C4-07-06: an apply strictly inside the window splits the
/// projection — the pre-apply segment keeps the old position line and
/// the post-apply segment carries the seek's re-anchored context, so
/// an event landing after the apply projects at its true frame.
#[test]
fn pc_c4_07_06_mid_window_apply_splits_projection() {
    let root = scratch("c40706");
    let (c, _media, processor_id, target, mut eng, mut dispatch, _instances) =
        playing_surface(&root, true);

    // Stage a Seek to position 9000 due 64 frames into the next
    // 256-frame window (head 512 → apply at 576), then a fast write
    // stamped past the seek position — its plan installs at head 512
    // (Play due 0) and its event @9050 can only fire on the post-seek
    // line at frame 576 + (9050 - 9000) = 626.
    dispatch
        .boundary_stage_window(
            processor_id,
            c.live(),
            256,
            vec![],
            &[(
                TransportCommand::Seek(MediaTime::new(9_000, frame_base())),
                64,
            )],
            active_generation(&eng),
        )
        .expect("mid-window seek staged");
    let rev = c.live_revision();
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 9_050, 0.7, rev),
        active_generation(&eng),
    );
    assert!(matches!(outcome, FastControlOutcome::Delivered));
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let cycles = dispatch
        .observe_block(1, ef(512), 256)
        .expect("split-window observation");
    let cycle = cycles.get(&processor_id).unwrap();
    let obs = observation_of(cycle, target).expect("target observed");
    // The event fired inside segment [576,768) at offset 50 — the
    // merged cycle reports the later segment's projection.
    assert!(
        matches!(
            obs.stage,
            DispatchStage::Dispatched {
                call_start: 576,
                frame_offset: 50
            }
        ),
        "mid-window apply splits the projection: {obs:?}"
    );

    // last_span is the first segment — pre-seek context at 512.
    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(span.start, ef(512));
    assert_eq!(span.end, ef(576));
    assert_eq!(span.ctx.position_ticks, 256);
}

/// PC-C4-07-07: `Pause` freezes the position line exactly like the
/// scheduler — status reports `Paused`, `position_ticks` stops
/// advancing, and a later `Play` resumes from the frozen position.
#[test]
fn pc_c4_07_07_pause_freezes_then_play_resumes() {
    let root = scratch("c40707");
    let (c, _media, processor_id, _target, mut eng, mut dispatch, _instances) =
        playing_surface(&root, true);

    // Position at head 512 is 256; pause applies at head 768 freezing
    // position 512.
    run_and_observe(&mut eng, &mut dispatch, 1, 512);
    dispatch
        .boundary_stage_window(
            processor_id,
            c.live(),
            256,
            vec![],
            &[(TransportCommand::Pause, 0)],
            active_generation(&eng),
        )
        .expect("pause staged");
    run_and_observe(&mut eng, &mut dispatch, 1, 768);
    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(span.ctx.status, TransportStatus::Paused);
    assert_eq!(span.ctx.position_ticks, 512);

    // Frozen: another block changes nothing.
    run_and_observe(&mut eng, &mut dispatch, 1, 1024);
    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(span.ctx.status, TransportStatus::Paused);
    assert_eq!(span.ctx.position_ticks, 512);

    // Play resumes from the frozen position — position at the apply
    // head is still 512, then advances again.
    dispatch
        .boundary_stage_window(
            processor_id,
            c.live(),
            256,
            vec![],
            &[(TransportCommand::Play, 0)],
            active_generation(&eng),
        )
        .expect("play staged");
    run_and_observe(&mut eng, &mut dispatch, 1, 1280);
    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(span.ctx.status, TransportStatus::Playing);
    assert_eq!(span.ctx.position_ticks, 512);
    run_and_observe(&mut eng, &mut dispatch, 1, 1536);
    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(span.ctx.position_ticks, 768);
}

/// PC-C4-07-08: `SetLoop`/`ClearLoop` project `loop_ticks` into the
/// span context — the mirror tracks the region the scheduler
/// installed, and clearing returns the context to no-loop.
#[test]
fn pc_c4_07_08_loop_region_projects_into_context() {
    let root = scratch("c40708");
    let (c, _media, processor_id, _target, mut eng, mut dispatch, _instances) =
        playing_surface(&root, true);

    // Loop [300,400) sits ahead of the current position (256 at head
    // 512) so no wrap fold disturbs the assertions.
    let loop_region = MediaRange::new(
        MediaTime::new(300, frame_base()),
        MediaDuration::new(100, frame_base()),
    );
    dispatch
        .boundary_stage_window(
            processor_id,
            c.live(),
            256,
            vec![],
            &[(TransportCommand::SetLoop(loop_region), 0)],
            active_generation(&eng),
        )
        .expect("set_loop staged");
    run_and_observe(&mut eng, &mut dispatch, 1, 512);
    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(span.ctx.loop_ticks, Some((300, 400)));
    assert_eq!(span.ctx.occurrence, LoopOccurrence::new(0));

    dispatch
        .boundary_stage_window(
            processor_id,
            c.live(),
            256,
            vec![],
            &[(TransportCommand::ClearLoop, 0)],
            active_generation(&eng),
        )
        .expect("clear_loop staged");
    run_and_observe(&mut eng, &mut dispatch, 1, 768);
    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(span.ctx.loop_ticks, None);
}

/// PC-C4-07-09: a generation swap mints a fresh mirror per new node
/// instance — the gen-2 surface's span starts `Stopped` (the new
/// scheduler's initial state) until its own dispatch plays it, and
/// the old generation's `observe_block` is gated out entirely.
#[test]
fn pc_c4_07_09_generation_swap_mints_fresh_mirror() {
    let root = scratch("c40709");
    let (mut c, media, processor_id, _target, mut eng, mut dispatch, _instances) =
        playing_surface(&root, false);

    // Semantic change → gen 2; the new generation's nodes count their
    // own cursor from attach — one processed block → next head 256.
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
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    assert_eq!(active_generation(&eng), Some(2));

    let synced = dispatch
        .sync(&mut eng, 2, c.live(), &catalog(processor_id))
        .unwrap();
    assert!(matches!(synced, synvelis_player::SyncOutcome::Swapped(_)));

    // Gen-1 observation is gated; gen-2 observes the NEW node's
    // cursor domain.
    assert!(dispatch.observe_block(1, ef(256), 256).is_none());
    let cycles = dispatch
        .observe_block(2, ef(256), 256)
        .expect("gen-2 surface");
    assert!(cycles.contains_key(&processor_id));
    let span = dispatch.last_observed_span(processor_id).unwrap();
    // Fresh node instance → fresh scheduler → Stopped until the gen-2
    // surface dispatches its own Play.
    assert_eq!(span.ctx.status, TransportStatus::Stopped);
    assert_eq!(span.ctx.position_ticks, 0);
    assert_eq!(span.ctx.transport_generation, TransportGeneration::new(0));
}

/// PC-C4-07-10: receipts are the mirror's generation-axis truth —
/// after `observe_block` drains them, the next span's
/// `transport_generation`/`plan_generation` equal the last drained
/// receipt's values (pending prediction cleared by scheduler truth).
#[test]
fn pc_c4_07_10_receipts_confirm_mirror_generations() {
    let root = scratch("c40710");
    let (c, _media, processor_id, target, mut eng, mut dispatch, _instances) =
        playing_surface(&root, false);

    // A second write stages plan gen 2 + another Play — receipts for
    // both applies drain at the next observation.
    let rev = c.live_revision();
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 600, 0.4, rev),
        active_generation(&eng),
    );
    assert!(matches!(outcome, FastControlOutcome::Delivered));
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    let cycles = dispatch
        .observe_block(1, ef(512), 256)
        .expect("observation");
    let cycle = cycles.get(&processor_id).unwrap();
    let last_receipt = cycle.receipts.last().expect("receipts drained");
    assert_eq!(last_receipt.plan_generation, PlanGeneration::new(2));

    // Next observation: no new commands — the span context reports
    // the receipt-confirmed transport generation (Play×2 → 2) and
    // the live plan generation 2.
    run_and_observe(&mut eng, &mut dispatch, 1, 768);
    let span = dispatch.last_observed_span(processor_id).unwrap();
    assert_eq!(
        span.ctx.transport_generation, last_receipt.transport_generation,
        "mirror adopts the last drained receipt's generation"
    );
    assert_eq!(span.ctx.plan_generation, PlanGeneration::new(2));
}
