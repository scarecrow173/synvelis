//! TIM-03 loop-boundary auto-derivation acceptance tests — Issue #127
//! slice (headless mock; spec `docs/specs/tim-03-loop-boundary.md`,
//! test spec rules R1–R9 in `docs/test-specs/tim-03-loop-boundary.md`).
//!
//! [`RoutingControlDispatch::boundary_loop_wraps`] derives the
//! loop-wrap [`BoundaryPoint`]s inside the upcoming physical window
//! from each node's scheduler-faithful [`WrapSim`] state — advanced
//! through every `observe_block` window by the same pending transport
//! commands the node scheduler applies — plus the scheduled/staged
//! command queues resolved against the window head. The derived
//! points drop straight into `boundary_plan_window` /
//! `boundary_stage_window`'s `boundaries` argument.
//!
//! R9 evidence: the recording plugin's `TransportInfo.position` is
//! the *real* `TransportScheduler`'s folded position at each physical
//! call head — the tests reconstruct the wrap frames that position
//! trajectory implies and require them to equal the derived offsets,
//! including across a mid-stream `SetLoop`/`Seek`/`ClearLoop` where
//! the lazy-fold mirror would diverge from scheduler `pos_at`.
//!
//! Node cursor domain: a generation's plugin nodes count frames from
//! their own attach — after `publish_and_run`'s single block the next
//! callback head is node frame 256, and each `eng.run(1)` advances it
//! by 256. `observe_block` must be called once per physical window,
//! in order; `boundary_loop_wraps` is a pure read called *before* the
//! window's `observe_block` while the sim still holds pre-window
//! state.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_core::{
    ClockEpochId, EngineFrame, InBlockTransportSupport, MediaDuration, MediaRange, MediaTime,
    ScheduleBudget, TimeBase, TransportCommand,
};
use ruviform_media_reader::PrefetchConfig;
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, Event,
    EventList, ParamFlags, ParamId, ParamInfo, Plugin, PluginDescriptor, PluginFactory,
    PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult, TransportInfo,
    erase, erase_sendable,
};
use ruviform_plugin_host::PluginHost;
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::PluginHostProcessors;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::process_boundary::BoundaryKind;
use synvelis_player::resolver::{PreparedClip, prepare};
use synvelis_player::routing_control::{
    BoundaryStageError, FastControlOutcome, FastControlWrite, ParamCatalog,
};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::{
    BoundaryDeclaration, BoundaryPoint, RoutingControlDispatch, TimingPrecision,
};
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
        "synvelis-tim03lb-{}-{}-{}",
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
        Project::new(ProjectId::new(0x127), "tim03-lb"),
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

/// The RT-observed active generation as the control gate consumes it.
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
) -> (
    synvelis_project::TrackId,
    synvelis_project::LaneId,
    synvelis_project::StripId,
) {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    (track, lane, strip)
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

/// Minimal project: one stereo import, one track strip, one clip,
/// `count` chain processors bound to the recording plugin. Returns
/// the processor ids in insertion order — only the first gets a
/// boundary declaration unless `declare` covers more.
fn plugin_project(
    root: &Path,
    processors: usize,
) -> (ProjectController, PathBuf, Vec<ProcessorId>) {
    let mut c = controller();
    let out = import_stereo(&mut c, root, "a.wav", 4_800);
    let (_track, lane, strip) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    let media = root.join("media");
    let mut ids = Vec::new();
    for index in 0..processors {
        apply(
            &mut c,
            ProjectOp::InsertProcessor {
                strip_id: strip,
                index,
                name: "rec".into(),
                binding: ProcessorBinding::Resolved(synvelis_project::PluginRef {
                    descriptor: REC_DESCRIPTOR.id.into(),
                    version: "0.1.0".into(),
                    capability_revision: 0,
                }),
                sidechain_input: false,
            },
        );
        ids.push(*c.live().routing.processors.keys().next_back().unwrap());
    }
    (c, media, ids)
}

/// A `ProcessorParam` target on `processor` declaring param 7 plus a
/// linear automation lane — the lane is unused (fast controls carry
/// the writes) but keeps the fixture identical to pc-c4-07's.
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
/// recording plugin's live `ParamInfo` enumeration per processor.
fn catalog(processors: &[ProcessorId]) -> ParamCatalog {
    processors
        .iter()
        .map(|p| (*p, REC_PARAMS.to_vec()))
        .collect::<BTreeMap<_, _>>()
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
        transport_generation: None,
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
    id: "org.synvelis.test.tim03-lb-rec",
    name: "TIM-03 LB Recorder",
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

/// One physical `process` call's scheduler evidence: the folded
/// transport position at the call head (`-1` when transport info was
/// absent) and the call's frame count. Calls partition each block at
/// transport-span boundaries — command applies and loop wraps — so
/// accumulating `frames` from a known head reconstructs the exact
/// frame of every wrap the real scheduler performed (R9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CallPos {
    position: i64,
    frames: u32,
}

type Positions = Arc<Mutex<Vec<CallPos>>>;

struct RecPlugin {
    positions: Positions,
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
        outputs: &mut [O],
        _events: EventList<'_>,
        transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: BufferView,
        O: BufferViewMut,
    {
        self.positions.lock().unwrap().push(CallPos {
            position: transport.map_or(-1, |t| t.position().value()),
            frames: outputs
                .first()
                .map_or(0, |b| u32::try_from(b.frames()).unwrap_or(u32::MAX)),
        });
        ProcessResult::CONTINUE
    }

    fn drain_output_reports(&mut self, _out: &mut Vec<Event>) {}
}

impl Plugin for RecPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        REC_DESCRIPTOR
    }
    fn params(&self) -> &[ParamInfo] {
        &REC_PARAMS
    }
    fn buses(&self) -> Option<&[BusDescriptor]> {
        Some(&STEREO_BUSES)
    }
}

struct RecFactory {
    positions: Positions,
}

impl PluginFactory for RecFactory {
    fn descriptor(&self) -> PluginDescriptor {
        REC_DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(RecPlugin {
            positions: Arc::clone(&self.positions),
        })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(RecPlugin {
            positions: Arc::clone(&self.positions),
        }))
    }
}

fn rec_host() -> (PluginHostProcessors, Positions) {
    let positions = Arc::new(Mutex::new(Vec::new()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory {
        positions: Arc::clone(&positions),
    }));
    (PluginHostProcessors::new(host), positions)
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

/// TIM-03 boundary declaration letting `boundary_stage_window` stage
/// transport commands on every processor in `processors`.
fn boundary_decls(processors: &[ProcessorId]) -> Vec<(ProcessorId, BoundaryDeclaration)> {
    processors
        .iter()
        .map(|p| {
            (
                *p,
                BoundaryDeclaration {
                    precision: TimingPrecision::FrameAccurateEvents,
                    in_block: InBlockTransportSupport::Unknown,
                    consent: None,
                    budget: ScheduleBudget::new(64, 256, 16, 4096),
                },
            )
        })
        .collect()
}

/// Runs one 256-frame block and observes the window it covered —
/// the contiguous per-block observation the run loop performs. `head`
/// is the node cursor the block started at; this also advances the
/// node's wrap sim through the same window.
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

/// Pure read: the loop-wrap points `boundary_loop_wraps` derives for
/// the window `[head, head + frames)` — call before that window's
/// `observe_block`.
fn derive(
    dispatch: &RoutingControlDispatch,
    c: &ProjectController,
    processor: ProcessorId,
    head: u64,
    frames: u32,
) -> Vec<BoundaryPoint> {
    dispatch
        .boundary_loop_wraps(processor, c.live(), ef(head), frames)
        .expect("boundary_loop_wraps")
}

fn offsets(points: &[BoundaryPoint]) -> Vec<u32> {
    points.iter().map(|p| p.offset).collect()
}

fn loop_cmd(start: i64, end: i64) -> TransportCommand {
    TransportCommand::SetLoop(MediaRange::new(
        MediaTime::new(start, frame_base()),
        MediaDuration::new(end - start, frame_base()),
    ))
}

fn seek_cmd(position: i64) -> TransportCommand {
    TransportCommand::Seek(MediaTime::new(position, frame_base()))
}

/// The plugin-observed calls recorded since the last drain, in call
/// order — one entry per transport span the scheduler delivered.
fn drain_positions(positions: &Positions) -> Vec<CallPos> {
    std::mem::take(&mut *positions.lock().unwrap())
}

/// Folds recorded calls into `(absolute_start_frame, position,
/// frames)` — the cursor starts at `first_head` (the node frame the
/// first recorded call covered) and each call advances it by its
/// frame count, so call starts land exactly on transport-span
/// boundaries: command applies and loop wraps.
fn call_starts(records: &[CallPos], first_head: u64) -> Vec<(u64, i64, u32)> {
    let mut frame = first_head;
    records
        .iter()
        .map(|r| {
            let start = frame;
            frame += u64::from(r.frames);
            (start, r.position, r.frames)
        })
        .collect()
}

/// Window-interior span boundaries `(offset, position)` the plugin
/// observed inside `[head, head + frames)` — command applies and loop
/// wraps both split spans, so this is the union of both; subtract
/// known apply offsets to isolate the wraps.
fn interior_boundaries(calls: &[(u64, i64, u32)], head: u64, frames: u64) -> Vec<(u32, i64)> {
    calls
        .iter()
        .filter(|(start, _, _)| *start > head && *start < head + frames)
        .map(|(start, pos, _)| (u32::try_from(start - head).unwrap_or(u32::MAX), *pos))
        .collect()
}

type PlayingSurface = (
    ProjectController,
    PathBuf,
    Vec<ProcessorId>,
    ParameterTargetId,
    RoutingEngine,
    RoutingControlDispatch,
    Positions,
);

/// Standard drive to a playing, mirror-observed node: publish gen 1,
/// attach (boundary declarations registered on every processor), one
/// delivered fast write (stages `Play` + plan install at node head
/// 256), run that block, observe it — returns with the node cursor at
/// 512 and the scheduler `Playing` at position 0 anchored at frame
/// 256. The wrap sim has likewise walked [256,512).
fn playing_surface(root: &Path, processors: usize) -> PlayingSurface {
    let (mut c, media, ids) = plugin_project(root, processors);
    let (target, _lane) = add_gain_target(&mut c, ids[0]);
    let (mut adapter, positions) = rec_host();
    // Only the first processor carries a boundary declaration —
    // siblings stay undeclared so tests can contrast sessioned and
    // sessionless nodes.
    for (processor, decl) in boundary_decls(&ids[..1]) {
        adapter.register_boundary(processor, decl);
    }
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    publish_and_run(&mut eng, &c, &media, 1);
    assert_eq!(active_generation(&eng), Some(1));
    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(&ids)).unwrap();

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
    (c, media, ids, target, eng, dispatch, positions)
}

// ---------------------------------------------------------------- tests

/// LB-01 (R1/R5/R9): a `SetLoop` applied at the window head derives
/// the wrap cascade the real scheduler then performs — the head wrap
/// records offset 0, interior wraps land at their true offsets, a
/// wrap at the window end is excluded, and the plugin's folded
/// `TransportInfo.position` at each subsequent head matches the sim's
/// trajectory exactly.
#[test]
fn tim03_lb_01_steady_loop_derives_offsets_matching_scheduler() {
    let root = scratch("lb01");
    let (c, _media, ids, _target, mut eng, mut dispatch, positions) = playing_surface(&root, 1);
    let processor = ids[0];

    // Playing position at head 512 is 256 (Play anchored 0 at 256).
    // Stage SetLoop [0,100): it applies at the head — the position
    // already sits past the loop end, so the scheduler wraps
    // immediately (offset 0) and the cascade lands the anchor at 56.
    dispatch
        .boundary_stage_window(
            processor,
            c.live(),
            256,
            vec![],
            &[(loop_cmd(0, 100), 0)],
            active_generation(&eng),
        )
        .expect("set_loop staged");

    // Preview the window BEFORE observing it — the sim still holds
    // the pre-window state. Smaller `call_frames` previews of the
    // same head derive the same leading wraps (pure read: the sim is
    // not advanced by derivation).
    let points = derive(&dispatch, &c, processor, 512, 256);
    assert_eq!(
        offsets(&points),
        vec![0, 44, 144, 244],
        "head wrap + wraps at 556/656/756; 856 belongs to the next window"
    );
    assert!(
        points
            .iter()
            .all(|p| p.kind == BoundaryKind::LoopWrap && u64::from(p.offset) < 256)
    );
    assert_eq!(
        offsets(&derive(&dispatch, &c, processor, 512, 100)),
        vec![0, 44]
    );
    assert_eq!(
        offsets(&derive(&dispatch, &c, processor, 512, 128)),
        vec![0, 44]
    );

    // Run the three windows; the real scheduler performs the same
    // wraps — each wrap splits the transport span, so the plugin's
    // per-call records land exactly on the derived wrap frames.
    let _ = drain_positions(&positions);
    run_and_observe(&mut eng, &mut dispatch, 1, 512);
    let points_next = derive(&dispatch, &c, processor, 768, 256);
    assert_eq!(offsets(&points_next), vec![88, 188]);
    run_and_observe(&mut eng, &mut dispatch, 1, 768);
    let points_third = derive(&dispatch, &c, processor, 1024, 256);
    assert_eq!(offsets(&points_third), vec![32, 132, 232]);
    run_and_observe(&mut eng, &mut dispatch, 1, 1024);

    // Scheduler truth (R9): `(position, frames)` per delivered call,
    // reconstructed to absolute call-start frames from head 512.
    // Positions are the folded values at each span start — 56 after
    // the head cascade, then the 100-tick loop cycling through 0.
    let records = drain_positions(&positions);
    let expected: &[(i64, u32)] = &[
        (56, 44),
        (0, 100),
        (0, 100),
        (0, 12), // [512,768): spans cut at wraps 556/656/756
        (12, 88),
        (0, 100),
        (0, 68), // [768,1024): wraps at 856/956
        (68, 32),
        (0, 100),
        (0, 100),
        (0, 24), // [1024,1280): wraps at 1056/1156/1256
    ];
    let got: Vec<(i64, u32)> = records.iter().map(|r| (r.position, r.frames)).collect();
    assert_eq!(got, expected, "plugin-observed call positions/frames");

    // The interior span boundaries the scheduler really cut at are
    // exactly the derived wrap offsets — no command applies exist
    // inside these windows, so every interior boundary is a wrap.
    let calls = call_starts(&records, 512);
    for (head, derived) in [
        (512_u64, vec![0_u32, 44, 144, 244]),
        (768, vec![88, 188]),
        (1024, vec![32, 132, 232]),
    ] {
        let interior: Vec<u32> = interior_boundaries(&calls, head, 256)
            .into_iter()
            .map(|(off, _)| off)
            .collect();
        let expected_wraps: Vec<u32> = derived.iter().copied().filter(|o| *o != 0).collect();
        assert_eq!(interior, expected_wraps, "wrap frames at head {head}");
    }
}

/// LB-02 (R4): no loop, `Paused`, and `Stopped` all derive empty —
/// wraps exist only while `Playing` under an installed region.
#[test]
fn tim03_lb_02_no_loop_and_not_playing_is_empty() {
    let root = scratch("lb02");
    let (c, _media, ids, _target, mut eng, mut dispatch, _positions) = playing_surface(&root, 1);
    let processor = ids[0];

    // Playing with no loop installed → empty.
    assert!(
        derive(&dispatch, &c, processor, 512, 256).is_empty(),
        "expected no derived wraps"
    );

    // Pause applies at head 512 → the window derives nothing, and
    // the following paused window stays empty.
    dispatch
        .boundary_stage_window(
            processor,
            c.live(),
            256,
            vec![],
            &[(TransportCommand::Pause, 0)],
            active_generation(&eng),
        )
        .expect("pause staged");
    assert!(
        derive(&dispatch, &c, processor, 512, 256).is_empty(),
        "expected no derived wraps"
    );
    run_and_observe(&mut eng, &mut dispatch, 1, 512);
    assert!(
        derive(&dispatch, &c, processor, 768, 256).is_empty(),
        "expected no derived wraps"
    );
    run_and_observe(&mut eng, &mut dispatch, 1, 768);
    assert!(
        derive(&dispatch, &c, processor, 1024, 256).is_empty(),
        "expected no derived wraps"
    );

    // Stop applies at head 1024 → still empty while stopped.
    dispatch
        .boundary_stage_window(
            processor,
            c.live(),
            256,
            vec![],
            &[(TransportCommand::Stop, 0)],
            active_generation(&eng),
        )
        .expect("stop staged");
    assert!(
        derive(&dispatch, &c, processor, 1024, 256).is_empty(),
        "expected no derived wraps"
    );
}

/// LB-03 (R2/E5): a mid-window `SetLoop` replaces the active region —
/// wraps already passed keep their frames, later wraps evaluate
/// under the new region, and the plugin's positions confirm the
/// scheduler performed the same re-anchoring.
#[test]
fn tim03_lb_03_mid_window_set_loop_replaces_region() {
    let root = scratch("lb03");
    let (c, _media, ids, _target, mut eng, mut dispatch, positions) = playing_surface(&root, 1);
    let processor = ids[0];

    // Window [512,768): SetLoop [0,100) at the head (position 256 →
    // immediate head wrap, anchor 56), then SetLoop [200,260) due 128
    // (frame 640, position 84 — below the new start). Wraps at 556
    // fire under the OLD region; from 640 on the NEW region governs.
    dispatch
        .boundary_stage_window(
            processor,
            c.live(),
            256,
            vec![],
            &[(loop_cmd(0, 100), 0), (loop_cmd(200, 260), 128)],
            active_generation(&eng),
        )
        .expect("set_loops staged");

    let points = derive(&dispatch, &c, processor, 512, 256);
    assert_eq!(
        offsets(&points),
        vec![0, 44],
        "head wrap + old-region wrap at 556; the new region's first \
         wrap lands at 816 — outside this window"
    );

    let _ = drain_positions(&positions);
    run_and_observe(&mut eng, &mut dispatch, 1, 512);

    // Next window under loop [200,260): position at head 768 is 212
    // (anchor 0 at 556 + 212 frames — SetLoop never re-anchored), so
    // wraps land every 60 ticks from 816: offsets 48/108/168/228.
    let points = derive(&dispatch, &c, processor, 768, 256);
    assert_eq!(offsets(&points), vec![48, 108, 168, 228]);
    run_and_observe(&mut eng, &mut dispatch, 1, 768);

    // Scheduler truth: spans cut at the head wrap (512), the old-
    // region wrap (556), the SetLoop apply (640), then the new
    // region's wraps at 816/876/936/996 — positions folded per the
    // region active at each span.
    let records = drain_positions(&positions);
    let got: Vec<(i64, u32)> = records.iter().map(|r| (r.position, r.frames)).collect();
    assert_eq!(
        got,
        vec![
            (56, 44),
            (0, 84),
            (84, 128), // [512,768): cuts at 556/640
            (212, 48),
            (200, 60),
            (200, 60),
            (200, 60),
            (200, 28), // [768,1024): wraps at 816/876/936/996
        ],
        "plugin-observed call positions/frames"
    );
    let calls = call_starts(&records, 512);
    // Window [512,768): interior boundaries {44 (wrap), 128 (SetLoop
    // apply)} — the derived wrap offsets plus the known apply frame.
    let interior: Vec<u32> = interior_boundaries(&calls, 512, 256)
        .into_iter()
        .map(|(off, _)| off)
        .collect();
    assert_eq!(interior, vec![44, 128]);
    // Window [768,1024): every interior boundary is a new-region wrap.
    let interior: Vec<u32> = interior_boundaries(&calls, 768, 256)
        .into_iter()
        .map(|(off, _)| off)
        .collect();
    assert_eq!(interior, vec![48, 108, 168, 228]);
}

/// LB-04 (R2/E6): a mid-window `Seek` preempts the wrap the old
/// trajectory predicted — the apply wins its frame, the position
/// re-anchors, and later wraps evaluate under the new position. The
/// plugin positions confirm the scheduler took the same path.
#[test]
fn tim03_lb_04_mid_window_seek_preempts_predicted_wrap() {
    let root = scratch("lb04");
    let (c, _media, ids, _target, mut eng, mut dispatch, positions) = playing_surface(&root, 1);
    let processor = ids[0];

    // Same head SetLoop [0,100) as LB-01, plus Seek to position 1000
    // due 128 (frame 640). Without the seek the trajectory would wrap
    // at 556 then 656; the seek applies at 640 first — position 1000
    // is past the loop end, so a wrap fires at 640 itself (offset
    // 128), then every 100 ticks: 740 → offset 228.
    dispatch
        .boundary_stage_window(
            processor,
            c.live(),
            256,
            vec![],
            &[(loop_cmd(0, 100), 0), (seek_cmd(1_000), 128)],
            active_generation(&eng),
        )
        .expect("set_loop+seek staged");

    let points = derive(&dispatch, &c, processor, 512, 256);
    assert_eq!(
        offsets(&points),
        vec![0, 44, 128, 228],
        "656 never wraps — the seek at 640 repositions first"
    );

    let _ = drain_positions(&positions);
    run_and_observe(&mut eng, &mut dispatch, 1, 512);
    let points_next = derive(&dispatch, &c, processor, 768, 256);
    assert_eq!(offsets(&points_next), vec![72, 172]);
    run_and_observe(&mut eng, &mut dispatch, 1, 768);

    // Scheduler truth: the seek apply at 640 splits the span AND the
    // wrap fires on the same frame (position 1000 sits past the loop
    // end) — one boundary, offset 128; the next wrap lands at 740.
    // Window [768,1024) wraps at 840/940 under the re-anchored loop.
    let records = drain_positions(&positions);
    let got: Vec<(i64, u32)> = records.iter().map(|r| (r.position, r.frames)).collect();
    assert_eq!(
        got,
        vec![
            (56, 44),
            (0, 84),
            (0, 100),
            (0, 28), // [512,768): cuts at 556/640/740
            (28, 72),
            (0, 100),
            (0, 84), // [768,1024): wraps at 840/940
        ],
        "plugin-observed call positions/frames"
    );
    let calls = call_starts(&records, 512);
    let interior: Vec<u32> = interior_boundaries(&calls, 512, 256)
        .into_iter()
        .map(|(off, _)| off)
        .collect();
    assert_eq!(interior, vec![44, 128, 228]);
    let interior: Vec<u32> = interior_boundaries(&calls, 768, 256)
        .into_iter()
        .map(|(off, _)| off)
        .collect();
    assert_eq!(interior, vec![72, 172]);
}

/// LB-05 (R2): `ClearLoop` removes every later wrap — wraps before
/// the apply keep their offsets, the window after derives empty, and
/// the position unfolds (the plugin sees raw advancing ticks, not a
/// folded loop position).
#[test]
fn tim03_lb_05_clear_loop_removes_later_wraps() {
    let root = scratch("lb05");
    let (c, _media, ids, _target, mut eng, mut dispatch, positions) = playing_surface(&root, 1);
    let processor = ids[0];

    dispatch
        .boundary_stage_window(
            processor,
            c.live(),
            256,
            vec![],
            &[(loop_cmd(0, 100), 0), (TransportCommand::ClearLoop, 128)],
            active_generation(&eng),
        )
        .expect("set_loop+clear staged");

    let points = derive(&dispatch, &c, processor, 512, 256);
    assert_eq!(offsets(&points), vec![0, 44], "wraps stop at 640");

    let _ = drain_positions(&positions);
    run_and_observe(&mut eng, &mut dispatch, 1, 512);

    // No loop → no wraps; position keeps unfolding past the old end.
    assert!(
        derive(&dispatch, &c, processor, 768, 256).is_empty(),
        "expected no derived wraps"
    );
    run_and_observe(&mut eng, &mut dispatch, 1, 768);

    // Scheduler truth: after ClearLoop the position is raw — 84 at
    // the apply frame, 212 at the next head, no further span cuts.
    let records = drain_positions(&positions);
    let got: Vec<(i64, u32)> = records.iter().map(|r| (r.position, r.frames)).collect();
    assert_eq!(
        got,
        vec![(56, 44), (0, 84), (84, 128), (212, 256)],
        "plugin-observed call positions/frames"
    );
    let calls = call_starts(&records, 512);
    // Window [512,768): interior boundaries {44 (wrap), 128 (ClearLoop
    // apply)} — the derived wrap plus the known apply frame.
    let interior: Vec<u32> = interior_boundaries(&calls, 512, 256)
        .into_iter()
        .map(|(off, _)| off)
        .collect();
    assert_eq!(interior, vec![44, 128]);
    // Window [768,1024): a single unsplit span — no boundaries at all.
    assert!(
        interior_boundaries(&calls, 768, 256).is_empty(),
        "expected a single unsplit span after ClearLoop"
    );
}

/// LB-06 (R7): derived points drop straight into the oracle —
/// `boundary_plan_window`/`boundary_stage_window` accept them as the
/// `boundaries` supply and report the `LoopWrap` offsets in the plan,
/// deduplicated against command-derived points.
#[test]
fn tim03_lb_06_derived_points_feed_boundary_oracle() {
    let root = scratch("lb06");
    let (c, _media, ids, _target, eng, mut dispatch, _positions) = playing_surface(&root, 1);
    let processor = ids[0];

    dispatch
        .boundary_stage_window(
            processor,
            c.live(),
            256,
            vec![],
            &[(loop_cmd(0, 100), 0)],
            active_generation(&eng),
        )
        .expect("set_loop staged");

    let points = derive(&dispatch, &c, processor, 512, 256);
    assert_eq!(offsets(&points), vec![0, 44, 144, 244]);

    // Pure evaluation consumes the derived supply verbatim. The
    // oracle normalizes the offset-0 head point away (the window
    // start is implicit — no cut needed) and keeps the interior
    // LoopWrap offsets.
    let plan = dispatch
        .boundary_plan_window(processor, c.live(), 256, points.clone())
        .expect("plan_window accepts derived points");
    for offset in [44_u32, 144, 244] {
        assert!(
            plan.boundaries.contains(&offset),
            "plan boundaries {offset}: {:?}",
            plan.boundaries
        );
    }

    // Staging a second window's worth of commands with the derived
    // points as the boundary supply succeeds the same way — the
    // session dedups command-derived offsets on top.
    let staged = dispatch
        .boundary_stage_window(
            processor,
            c.live(),
            256,
            points,
            &[(seek_cmd(50), 200)],
            active_generation(&eng),
        )
        .expect("stage_window accepts derived points");
    assert!(staged.boundaries.contains(&200), "{:?}", staged.boundaries);
    assert!(staged.boundaries.contains(&44), "{:?}", staged.boundaries);
}

/// LB-07 (R8 + Guarantees): surface gating stays typed — an unknown
/// processor reports `NoSession`, a project whose capability
/// generation drifted from the surface pins reports `StaleSurface`,
/// and a node *without* a boundary session still derives (the wrap
/// sim is maintained for every vended control node).
#[test]
fn tim03_lb_07_provenance_gating_and_sessionless_nodes() {
    let root = scratch("lb07");
    let (c, _media, ids, _target, _eng, dispatch, _positions) = playing_surface(&root, 2);
    let processor = ids[0];
    let undeclared = ids[1];

    // The undeclared sibling has a control node (and therefore a wrap
    // sim) but no ProcessBoundarySession — derivation still works.
    assert!(dispatch.boundary_session(undeclared).is_none());
    let sibling = derive(&dispatch, &c, undeclared, 512, 256);
    assert!(
        sibling.is_empty(),
        "undeclared node derives empty: {sibling:?}"
    );

    // A processor with no vended control node refuses typed.
    let err = dispatch
        .boundary_loop_wraps(ProcessorId::new(9_999), c.live(), ef(512), 256)
        .unwrap_err();
    assert!(matches!(err, BoundaryStageError::NoSession(_)), "{err:?}");

    // Drift the project's capability generation away from the
    // surface pins — the surface is stale, not silently re-anchored.
    let mut stale = c.live().clone();
    let node = stale
        .routing
        .processors
        .get_mut(&processor)
        .expect("processor");
    let ProcessorBinding::Resolved(reference) = &mut node.binding else {
        panic!("resolved binding")
    };
    reference.capability_revision += 1;
    let err = dispatch
        .boundary_loop_wraps(processor, &stale, ef(512), 256)
        .unwrap_err();
    assert!(matches!(err, BoundaryStageError::StaleSurface), "{err:?}");

    // The current project still derives fine afterwards — the stale
    // probe mutated only the clone.
    assert!(
        derive(&dispatch, &c, processor, 512, 256).is_empty(),
        "expected no derived wraps"
    );
}

// ---------------------------------------------------------------------
// Geometry matrix expansion — physical 100/128/256-frame routing windows
// ---------------------------------------------------------------------

fn engine_for(block: usize) -> synvelis_player::lowering::EngineConfig {
    synvelis_player::lowering::EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: block,
    }
}

fn prefetch_for(block: usize) -> PrefetchConfig {
    PrefetchConfig {
        block_frames: block,
        block_count: 8,
        channels: 2,
        sample_rate: 48_000,
    }
}

fn prepared_clips_for(project: &Project, media_root: &Path, block: usize) -> Vec<PreparedClip> {
    project
        .clips
        .values()
        .map(|clip| prepare(project, clip.source_id, media_root, &prefetch_for(block)).unwrap())
        .collect()
}

fn deps_for<'a>(project: &Project, media_root: &Path, block: usize) -> RoutingDeps<'a> {
    RoutingDeps {
        canonical_of: &|_| Some(canonical_format()),
        clips: prepared_clips_for(project, media_root, block),
        engine_config_generation: 1,
        source_format_generation: 1,
    }
}

fn publish_and_run_for(
    eng: &mut RoutingEngine,
    c: &ProjectController,
    media: &Path,
    id: u64,
    block: usize,
) {
    let rev = c.live_revision();
    let prepared = eng
        .prepare(
            &request(id, rev),
            c.live(),
            rev,
            deps_for(c.live(), media, block),
        )
        .unwrap()
        .expect_built();
    eng.publish(prepared, c.live(), rev).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();
}

fn run_and_observe_for(
    eng: &mut RoutingEngine,
    dispatch: &mut RoutingControlDispatch,
    generation: u64,
    head: u64,
    block: u32,
) {
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    dispatch
        .observe_block(generation, ef(head), block)
        .unwrap_or_else(|| panic!("gen-{generation} surface observed"));
}

fn playing_surface_for(root: &Path, processors: usize, block: usize) -> PlayingSurface {
    let (mut c, media, ids) = plugin_project(root, processors);
    let (target, _lane) = add_gain_target(&mut c, ids[0]);
    let (mut adapter, positions) = rec_host();
    for (processor, decl) in boundary_decls(&ids[..1]) {
        adapter.register_boundary(processor, decl);
    }
    let mut eng = RoutingEngine::new(
        engine_for(block),
        RuntimeInstanceId(1),
        RuntimeClockEpochRef(1),
    );
    eng.set_processor_host(Box::new(adapter));

    publish_and_run_for(&mut eng, &c, &media, 1, block);
    assert_eq!(active_generation(&eng), Some(1));
    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(&ids)).unwrap();
    let rev = c.live_revision();
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 300, 0.8, rev),
        active_generation(&eng),
    );
    assert!(matches!(outcome, FastControlOutcome::Delivered));
    run_and_observe_for(
        &mut eng,
        &mut dispatch,
        1,
        u64::try_from(block).unwrap(),
        u32::try_from(block).unwrap(),
    );
    (c, media, ids, target, eng, dispatch, positions)
}

/// Routing-side loop-wrap derivation across physical window geometries
/// 100/128/256. After the two setup windows the next head is `2W` and
/// the playing position is `W`; `SetLoop` [100,200) therefore exercises
/// end-aligned, interior, and same-head cascade cases respectively.
#[test]
fn tim03_lb_geometry_matrix_loop_wrap_derivation() {
    let mut rows = Vec::new();
    for block in [100_usize, 128, 256] {
        let root = scratch(&format!("lbgeom{block}"));
        let (c, _media, ids, _target, mut eng, mut dispatch, positions) =
            playing_surface_for(&root, 1, block);
        let processor = ids[0];
        let head = u64::try_from(2 * block).unwrap();
        let frames = u32::try_from(block).unwrap();
        dispatch
            .boundary_stage_window(
                processor,
                c.live(),
                frames,
                vec![],
                &[(loop_cmd(100, 200), 0)],
                active_generation(&eng),
            )
            .expect("set_loop staged");
        let points = derive(&dispatch, &c, processor, head, frames);
        let derived = offsets(&points);
        let _ = drain_positions(&positions);
        run_and_observe_for(&mut eng, &mut dispatch, 1, head, frames);
        let records = drain_positions(&positions);
        let observed: Vec<(i64, u32)> = records.iter().map(|r| (r.position, r.frames)).collect();
        let (expected_derived, expected_observed): (Vec<u32>, Vec<(i64, u32)>) = match block {
            100 => (vec![], vec![(100, 100)]),
            128 => (vec![72], vec![(128, 72), (100, 56)]),
            256 => (
                vec![0, 44, 144, 244],
                vec![(156, 44), (100, 100), (100, 100), (100, 12)],
            ),
            _ => unreachable!(),
        };
        assert_eq!(derived, expected_derived, "block={block}");
        assert_eq!(observed, expected_observed, "block={block}");
        rows.push(serde_json::json!({
            "window_frames": block,
            "loop_range_frames": [100,200],
            "derived_loop_wrap_offsets_including_head": derived,
            "observed_position_length_calls": observed,
            "head_wrap_explicit_not_interior_cut": derived.first() == Some(&0),
            "window_end_wrap_excluded": block == 100,
            "verdict": "pass"
        }));
    }
    println!(
        "tim03-routing-loop-derivation-matrix {}",
        serde_json::json!({
            "schema":"tim03-frame-boundary-matrix/routing-loop-derivation/v1",
            "rows": rows,
        })
    );
}
