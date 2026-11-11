//! TIM-03 session→driver/routing wiring acceptance tests — Issue #127
//! slice 3 (headless mock; spec `docs/specs/tim-03-session-driver-wiring.md`,
//! test spec rules SW-01..SW-10 in
//! `docs/test-specs/tim-03-session-driver-wiring.md`).
//!
//! Two composition paths under test:
//!
//! - **driver**: `AttachedNode.boundary` opens a per-activation
//!   [`ProcessBoundarySession`] whose provenance pins the
//!   driver-minted instance id, node id, capability generation `0`
//!   and the published plan generation; transport commands stage
//!   through `stage_window`; `attached_span` reports the
//!   mirror-projected (never fabricated) span context; forged
//!   provenance and queue floods refuse typed.
//! - **routing**: a registered [`BoundaryDeclaration`] flows
//!   `PluginHostProcessors::connect` → `ProcessorControl::boundary` →
//!   `vend_nodes` → per-generation session sharing the binding
//!   session's instance id; `boundary_plan_window`/
//!   `boundary_stage_window` gate on surface freshness and the
//!   RT-observed active generation.
//!
//! Real CLAP/VST3/OS/DAW/consumer validation stays `NotRun`
//! (separate gate); `tim03_request_plan.rs` regression is SW-09.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_audio_plugin_bridge::{AttachConfig, attach};
use ruviform_graph::{GraphBuilder, NodeDef, NodeId, NodeKind};
use ruviform_media_core::{
    BlockContract, DeliveryPolicy, InBlockTransportSupport, LoopOccurrence, MediaTime,
    ScheduleBudget, TimeBase, TransportCommand, TransportStatus,
};
use ruviform_media_reader::PrefetchConfig;
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, Plugin, PluginDescriptor,
    PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult,
    TimedParamEvent, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHost};
use synvelis_player::binding::StablePluginInstanceId;
use synvelis_player::driver::{ActivateError, PumpPolicy};
use synvelis_player::lowering::{EngineConfig, lower_with_inserts};
use synvelis_player::process_boundary::{
    BoundarySessionError, BoundarySessionSpec, PrecisionProvenance, ProcessBoundaryError,
    ProcessCapability, TimingPrecision,
};
use synvelis_player::routing_control::BoundaryStageError;
use synvelis_player::routing_engine::RoutingEngine;
use synvelis_player::{
    AttachedNode, BoundaryDeclaration, DegradedConsent, ExecutionMode, HeadlessDriver,
    ParamDispatch, PluginHostProcessors, RoutingControlDispatch,
};
use synvelis_project::{
    BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, ControlOwner,
    CounterIdGenerator, LaneId, MediaDuration, MediaTimeValue, ParamMapping, ParameterRole,
    ParameterTargetId, PlaybackMapping, ProcessorId, Project, ProjectController, ProjectId,
    ProjectOp, ProjectRevision, RoutingBuildRequestId, RoutingCompileRequest, RuntimeClockEpochRef,
    RuntimeInstanceId, SourceFrame, SourceId, SourceRange, StripId, TargetCapability, TimeAnchor,
    TrackId, ValueKind, port_capability_generation,
};

// ---------------------------------------------------------------------
// shared plugin fixture (pc_b / ctl_02 shape)
// ---------------------------------------------------------------------

const DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.tim03-sw",
    name: "TIM-03 SW Recorder",
    vendor: "Synvelis",
    version: "0.1.0",
    features: &[],
};

const BUSES: [BusDescriptor; 2] = [
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

const PARAMS: [ParamInfo; 1] = [ParamInfo {
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
        DESCRIPTOR
    }
    fn params(&self) -> &[ParamInfo] {
        &PARAMS
    }
    fn buses(&self) -> &[BusDescriptor] {
        &BUSES
    }
}

struct RecFactory {
    delivered: Arc<Mutex<Vec<Delivered>>>,
}

impl PluginFactory for RecFactory {
    fn descriptor(&self) -> PluginDescriptor {
        DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(RecPlugin {
            delivered: Arc::clone(&self.delivered),
        })
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(RecPlugin {
            delivered: Arc::clone(&self.delivered),
        }))
    }
}

fn fmt() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn frame_base() -> TimeBase {
    TimeBase::sample_rate(48_000).unwrap()
}

fn engine_config() -> EngineConfig {
    EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 128,
    }
}

/// A hosted plugin attach — the `Attached { node, control, ports }`
/// triple the driver consumes.
struct Hosted {
    _host: PluginHost,
    attached: ruviform_audio_plugin_bridge::Attached,
    delivered: Arc<Mutex<Vec<Delivered>>>,
}

fn host_plugin() -> Hosted {
    let delivered = Arc::new(Mutex::new(Vec::new()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory {
        delivered: Arc::clone(&delivered),
    }));
    let HostReply::Instantiated { handle } = host
        .execute(HostCommand::Instantiate {
            plugin_id: DESCRIPTOR.id.to_owned(),
        })
        .unwrap()
    else {
        panic!("Instantiate")
    };
    host.execute(HostCommand::Initialize { handle }).unwrap();
    let process_config = ProcessConfig::new(SampleRate::new(48_000).unwrap(), 1, 512).unwrap();
    host.execute(HostCommand::Activate {
        handle,
        config: process_config,
    })
    .unwrap();
    host.execute(HostCommand::StartProcessing { handle })
        .unwrap();
    let attached = attach(
        &mut host,
        handle,
        &AttachConfig {
            buses: BUSES.to_vec(),
            port_format: fmt(),
            bounds: EndpointBounds {
                max_frames: 512,
                channels: 2,
            },
            process_config,
            frame_base: frame_base(),
            policy: DeliveryPolicy::Exact,
            budget: ScheduleBudget::default(),
            input: None,
        },
    )
    .expect("attach");
    Hosted {
        _host: host,
        attached,
        delivered,
    }
}

/// The attach-resolved boundary inputs a declared processor's
/// composition root would vend — Exact realtime delivery, no consent.
fn spec() -> BoundarySessionSpec {
    BoundarySessionSpec {
        capability: ProcessCapability::new(
            BlockContract::new(1, 512, InBlockTransportSupport::Unknown),
            TimingPrecision::FrameAccurateEvents,
        ),
        mode: ExecutionMode::Realtime,
        consent: None,
        budget: ScheduleBudget::default(),
    }
}

/// Lowered insert slots for `count` attached nodes on an empty
/// project — returns (lowered, node ids in slot order).
fn lowered_inserts(count: usize) -> (synvelis_player::lowering::Lowered, Vec<NodeId>) {
    let project = Project::new(ProjectId::new(0), "tim03-sw");
    let lowered = lower_with_inserts(
        &project,
        &|_: SourceId| -> Option<AudioFormat> { None },
        &engine_config(),
        count,
    );
    let ids = lowered.inserts.iter().map(|i| i.node_id).collect();
    (lowered, ids)
}

/// Mints a `NodeId` different from `except` — `NodeId::new` is
/// crate-private upstream, so candidates come from one throwaway
/// builder (sequential ids guarantee a mismatch within 16 tries).
fn foreign_node_id(except: NodeId) -> NodeId {
    let mut b = GraphBuilder::new();
    for _ in 0..16 {
        let (node, _ports) = b.add_node(&NodeDef {
            kind: NodeKind::Processor,
            ports: Vec::new(),
        });
        if node != except {
            return node;
        }
    }
    panic!("no foreign NodeId")
}

/// Absolute-frame param writes the session's `dispatch_params` gates.
fn writes() -> Vec<(ParameterTargetId, TimedParamEvent)> {
    let base = frame_base();
    let target = ParameterTargetId::new(1);
    [(64_i64, 0.25_f64), (128, 0.5), (192, 0.75), (320, 1.0)]
        .into_iter()
        .map(|(pos, v)| {
            (
                target,
                TimedParamEvent {
                    position: MediaTime::new(pos, base),
                    event: ParamEvent {
                        param_id: ParamId(7),
                        value: ParamValue::new(v).unwrap(),
                    },
                },
            )
        })
        .collect()
}

// ---------------------------------------------------------------------
// routing fixture (ctl_02 / sf03_l6 shape)
// ---------------------------------------------------------------------

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-tim03sw-{}-{}-{}",
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
        Project::new(ProjectId::new(0x127), "tim03-sw"),
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

fn routing_engine() -> RoutingEngine {
    RoutingEngine::new(
        EngineConfig {
            sample_rate: 48_000,
            channels: 2,
            block_frames: 256,
        },
        RuntimeInstanceId(1),
        RuntimeClockEpochRef(1),
    )
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

fn request(id: u64, revision: ProjectRevision) -> RoutingCompileRequest {
    RoutingCompileRequest {
        request_id: RoutingBuildRequestId(id),
        expected_project_revision: revision,
        mode: BuildMode::Realtime,
        resource_budget: synvelis_project::BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
        runtime_instance: RuntimeInstanceId(1),
        device_clock_epoch: RuntimeClockEpochRef(1),
    }
}

fn active_generation(eng: &RoutingEngine) -> Option<u64> {
    let generation = eng.rt_status().active_generation;
    (generation != 0).then_some(generation)
}

fn write_fixture_wav(root: &Path, name: &str, frames: usize) {
    synvelis_player::fixture::write_fixture(&root.join(name), 2, 48_000, frames).unwrap();
}

fn import_stereo(
    c: &mut ProjectController,
    root: &Path,
    name: &str,
    frames: usize,
) -> synvelis_player::ImportOutcome {
    write_fixture_wav(root, name, frames);
    synvelis_player::import::import_local(
        c,
        &synvelis_player::import::ImportRequest {
            path: root.join(name),
            staging_root: root.join("staging"),
            media_root: root.join("media"),
            resume: None,
        },
    )
    .unwrap()
}

fn add_track_strip(c: &mut ProjectController) -> (TrackId, LaneId, StripId) {
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

fn prepared_clips(
    project: &Project,
    media_root: &Path,
) -> Vec<synvelis_player::resolver::PreparedClip> {
    project
        .clips
        .values()
        .map(|clip| {
            synvelis_player::resolver::prepare(project, clip.source_id, media_root, &prefetch())
                .unwrap()
        })
        .collect()
}

fn deps_default<'a>(
    project: &Project,
    media_root: &Path,
) -> synvelis_player::routing_engine::RoutingDeps<'a> {
    synvelis_player::routing_engine::RoutingDeps {
        canonical_of: &|_| Some(canonical_format()),
        clips: prepared_clips(project, media_root),
        engine_config_generation: 1,
        source_format_generation: 1,
    }
}

/// One imported clip + one track strip + the chain processor bound
/// to the recorder plugin — plus an optional second *undeclared*
/// processor on the same plugin for the legacy-contrast assertions.
fn plugin_project(
    root: &Path,
    second_processor: bool,
) -> (ProjectController, PathBuf, ProcessorIdPair) {
    let mut c = controller();
    let out = import_stereo(&mut c, root, "a.wav", 4_800);
    let (_track, lane, strip) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    let media = root.join("media");
    let first = insert_processor(&mut c, strip, 0);
    let second = second_processor.then(|| insert_processor(&mut c, strip, 1));
    (c, media, ProcessorIdPair { first, second })
}

struct ProcessorIdPair {
    first: ProcessorId,
    second: Option<ProcessorId>,
}

fn insert_processor(c: &mut ProjectController, strip: StripId, index: usize) -> ProcessorId {
    apply(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index,
            name: "rec".into(),
            binding: synvelis_project::ProcessorBinding::Resolved(synvelis_project::PluginRef {
                descriptor: DESCRIPTOR.id.into(),
                version: "0.1.0".into(),
                capability_revision: 0,
            }),
            sidechain_input: false,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

/// The lane/source a semantic change needs for the gen-2 rebuild.
fn lane_and_source(c: &ProjectController) -> (LaneId, SourceId) {
    let lane = *c
        .live()
        .tracks
        .values()
        .next()
        .unwrap()
        .lane_ids
        .first()
        .unwrap();
    let source = c.live().clips.values().next().unwrap().source_id;
    (lane, source)
}

fn add_plugin_param(
    c: &mut ProjectController,
    processor: ProcessorId,
    name: &str,
    param_id: u32,
) -> ParameterTargetId {
    apply(
        c,
        ProjectOp::AddParameterTarget {
            name: name.into(),
            owner: ControlOwner::Processor(processor),
            role: ParameterRole::ProcessorParam,
            time_scope: synvelis_project::AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(TargetCapability {
                param_id,
                flags_bits: ParamFlags::AUTOMATABLE.bits(),
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.0,
            }),
            base: 0.0,
        },
    );
    c.live()
        .control
        .targets
        .values()
        .find(|t| t.name == name)
        .unwrap()
        .id
}

/// Caller-vended param catalogs — the recorder's live `ParamInfo`
/// enumeration per processor.
fn catalog(processors: &[ProcessorId]) -> synvelis_player::routing_control::ParamCatalog {
    processors
        .iter()
        .map(|p| (*p, PARAMS.to_vec()))
        .collect::<BTreeMap<_, _>>()
}

/// `PluginHostProcessors` over the recorder factory.
fn rec_host() -> PluginHostProcessors {
    let delivered = Arc::new(Mutex::new(Vec::new()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory { delivered }));
    PluginHostProcessors::new(host)
}

fn declaration(
    precision: TimingPrecision,
    consent: Option<DegradedConsent>,
) -> BoundaryDeclaration {
    BoundaryDeclaration {
        precision,
        in_block: InBlockTransportSupport::Unknown,
        consent,
        budget: ScheduleBudget::new(64, 256, 16, 4096),
    }
}

// ---------------------------------------------------------------------
// SW-01 / SW-09 — driver session open + provenance + legacy contrast
// ---------------------------------------------------------------------

/// `AttachedNode.boundary = Some(spec)` opens a session at activation
/// pinned to (driver-minted instance, node, capability gen 0, plan
/// generation); `boundary: None` nodes expose no session and keep the
/// legacy path.
#[test]
fn tim03_sw_01_driver_session_open_and_provenance() {
    let (lowered, ids) = lowered_inserts(2);
    let a = host_plugin();
    let b = host_plugin();
    let mut driver = HeadlessDriver::new(engine_config());
    driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![
                AttachedNode {
                    node: a.attached.node,
                    dispatch: ParamDispatch::new(a.attached.control, ids[0], frame_base()).unwrap(),
                    boundary: Some(spec()),
                },
                AttachedNode {
                    node: b.attached.node,
                    dispatch: ParamDispatch::new(b.attached.control, ids[1], frame_base()).unwrap(),
                    boundary: None,
                },
            ],
        )
        .expect("activate");

    let (session, _dispatch) = driver.attached_boundary(ids[0]).expect("sessioned node");
    let pin = session.provenance();
    assert_eq!(pin.instance(), StablePluginInstanceId::new(1));
    assert_eq!(pin.node(), ids[0]);
    assert_eq!(pin.capability_generation(), 0);
    assert_eq!(pin.graph_generation(), 1);

    // The undeclared sibling has no session — legacy lenient path.
    assert!(driver.attached_boundary(ids[1]).is_none());
    // No block has run yet — no span evidence.
    assert!(driver.attached_span(ids[0]).is_none());
}

// ---------------------------------------------------------------------
// SW-03 / SW-05 / SW-10 — driver span honesty through transport commands
// ---------------------------------------------------------------------

/// The mirror-fed span context tracks commanded transport state:
/// `set_loop` installs `loop_ticks`, wraps fold `position_ticks`
/// below the loop end and bump `occurrence`, `panic` stops at
/// position 0, `resume` re-anchors — and `transport_generation` is
/// receipt-derived (the anchor's Seek+Play bumps), never the old
/// fabricated `0`.
#[test]
fn tim03_sw_05_driver_span_context_is_honest() {
    let (lowered, ids) = lowered_inserts(1);
    let hosted = host_plugin();
    let mut driver = HeadlessDriver::new(engine_config());
    driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![AttachedNode {
                node: hosted.attached.node,
                dispatch: ParamDispatch::new(hosted.attached.control, ids[0], frame_base())
                    .unwrap(),
                boundary: Some(spec()),
            }],
        )
        .expect("activate");

    // Block 0: anchor Seek+Play applied — status Playing, position 0,
    // generation already past the scheduler's initial 0 (two bumps).
    driver.run(1, PumpPolicy::Every(1)).expect("run");
    let span = driver.attached_span(ids[0]).expect("span");
    assert_eq!(span.ctx.status, TransportStatus::Playing);
    assert_eq!(span.ctx.position_ticks, 0);
    assert_eq!(span.ctx.loop_ticks, None);
    assert_eq!(span.ctx.occurrence, LoopOccurrence::new(0));
    assert!(
        span.ctx.transport_generation.get() >= 2,
        "receipt-derived generation, got {:?}",
        span.ctx.transport_generation
    );

    // Loop [100, 200) staged at cursor 128 — applies at head 128.
    driver.set_loop(100, 200).expect("set_loop");
    driver.run(2, PumpPolicy::Every(1)).expect("run");
    // Last observed block starts at 256: raw position 256 wraps once
    // (200 → 100 + 56) — position 156, occurrence 1, loop recorded.
    let span = driver.attached_span(ids[0]).expect("span");
    assert_eq!(span.ctx.status, TransportStatus::Playing);
    assert_eq!(span.ctx.loop_ticks, Some((100, 200)));
    assert_eq!(span.ctx.position_ticks, 156);
    assert_eq!(span.ctx.occurrence, LoopOccurrence::new(1));

    // Panic: Stop applies at head 384 — status Stopped, position 0.
    driver.panic().expect("panic");
    driver.run(1, PumpPolicy::Every(1)).expect("run");
    let span = driver.attached_span(ids[0]).expect("span");
    assert_eq!(span.ctx.status, TransportStatus::Stopped);
    assert_eq!(span.ctx.position_ticks, 0);
    assert!(
        span.ctx.transport_generation.get() > 2,
        "Stop bumped the receipt-derived generation"
    );

    // Resume re-anchors Seek+Play at cursor 512: position 512 folds
    // through the still-active loop (512 → 4 wraps → 112), occurrence
    // continues from the pre-panic count.
    driver.resume_attached_transport().expect("resume");
    driver.run(1, PumpPolicy::Every(1)).expect("run");
    let span = driver.attached_span(ids[0]).expect("span");
    assert_eq!(span.ctx.status, TransportStatus::Playing);
    assert_eq!(span.ctx.loop_ticks, Some((100, 200)));
    assert_eq!(span.ctx.position_ticks, 112);
    assert!(span.ctx.occurrence.get() >= 5, "{:?}", span.ctx.occurrence);
}

/// `boundary_plan_window`-equivalent evaluation through the session:
/// every returned segment is legally partitionable inside the
/// declared contract (SW-10), and an oracle-illegal window refuses
/// typed before anything stages.
#[test]
fn tim03_sw_10_driver_plan_window_segments_legal() {
    let (lowered, ids) = lowered_inserts(1);
    let hosted = host_plugin();
    let mut driver = HeadlessDriver::new(engine_config());
    driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![AttachedNode {
                node: hosted.attached.node,
                dispatch: ParamDispatch::new(hosted.attached.control, ids[0], frame_base())
                    .unwrap(),
                boundary: Some(spec()),
            }],
        )
        .expect("activate");

    let (session, _dispatch) = driver.attached_boundary(ids[0]).expect("session");
    let pin = session.provenance();
    let plan = session.plan_window(&pin, 128, vec![]).expect("window plan");
    assert_eq!(plan.policy, DeliveryPolicy::Exact);
    // Segments tile the window contiguously, each non-empty and
    // inside the [1, 512] contract — the oracle never emits a 0-length
    // or oversized call.
    let mut covered = 0_u32;
    for seg in &plan.segments {
        assert!(seg.start < seg.end, "empty segment {seg:?}");
        assert_eq!(seg.start, covered, "segments must tile the window");
        assert!(seg.end - seg.start <= 512);
        covered = seg.end;
    }
    assert_eq!(covered, 128);

    // A zero-length window is an oracle refusal — typed, nothing staged.
    let err = session.plan_window(&pin, 0, vec![]).unwrap_err();
    assert!(matches!(
        err,
        BoundarySessionError::Evaluate(ProcessBoundaryError::InvalidRequest { .. })
    ));
}

// ---------------------------------------------------------------------
// SW-04 / SW-06 — driver dispatch gating + stale provenance
// ---------------------------------------------------------------------

/// `session.dispatch_params` delegates to the same `ParamDispatch`
/// seam: events deliver at absolute frames, and a forged provenance
/// pin refuses before any write resolves.
#[test]
fn tim03_sw_04_06_driver_dispatch_params_and_stale_provenance() {
    let (lowered, ids) = lowered_inserts(1);
    let hosted = host_plugin();
    let mut driver = HeadlessDriver::new(engine_config());
    driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![AttachedNode {
                node: hosted.attached.node,
                dispatch: ParamDispatch::new(hosted.attached.control, ids[0], frame_base())
                    .unwrap(),
                boundary: Some(spec()),
            }],
        )
        .expect("activate");

    let target = ParameterTargetId::new(1);
    {
        let (session, dispatch) = driver.attached_boundary(ids[0]).expect("session");
        dispatch.bind(target, ParamId(7), &PARAMS).unwrap();
        // Honest pin passes; forged pins refuse on every axis.
        session
            .dispatch_params(dispatch, &session.provenance(), &writes())
            .expect("dispatch_params");
        let staged_gen = dispatch.last_plan_generation();
        for forged in [
            PrecisionProvenance::new(StablePluginInstanceId::new(99), ids[0], 0, 1),
            PrecisionProvenance::new(StablePluginInstanceId::new(1), ids[0], 0, 2),
            PrecisionProvenance::new(StablePluginInstanceId::new(1), ids[0], 7, 1),
        ] {
            let err = session
                .dispatch_params(dispatch, &forged, &writes())
                .unwrap_err();
            assert!(
                matches!(
                    err,
                    BoundarySessionError::InstanceMismatch { .. }
                        | BoundarySessionError::GraphGeneration { .. }
                        | BoundarySessionError::CapabilityGeneration { .. }
                ),
                "forged pin must refuse typed, got {err:?}"
            );
        }
        // Refusals staged nothing — the plan generation is unchanged.
        assert_eq!(dispatch.last_plan_generation(), staged_gen);
    }

    driver.run(4, PumpPolicy::Every(1)).expect("run");
    let expected: Vec<Delivered> = writes()
        .iter()
        .map(|(_, e)| Delivered {
            abs_frame: u64::try_from(e.position.value()).unwrap(),
            param_id: 7,
            value: e.event.value.get(),
        })
        .collect();
    assert_eq!(
        *hosted.delivered.lock().unwrap(),
        expected,
        "session-gated dispatch delivered through the existing seam"
    );
}

/// A foreign node id in the presented provenance is a `NodeMismatch`
/// — the session pin is per-node, not per-driver.
#[test]
fn tim03_sw_06_driver_node_mismatch() {
    let (lowered, ids) = lowered_inserts(1);
    let hosted = host_plugin();
    let mut driver = HeadlessDriver::new(engine_config());
    driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![AttachedNode {
                node: hosted.attached.node,
                dispatch: ParamDispatch::new(hosted.attached.control, ids[0], frame_base())
                    .unwrap(),
                boundary: Some(spec()),
            }],
        )
        .expect("activate");

    let (session, _dispatch) = driver.attached_boundary(ids[0]).expect("session");
    let pin = session.provenance();
    let forged = PrecisionProvenance::new(
        pin.instance(),
        foreign_node_id(pin.node()),
        pin.capability_generation(),
        pin.graph_generation(),
    );
    let err = session.plan_window(&forged, 128, vec![]).unwrap_err();
    assert!(matches!(err, BoundarySessionError::NodeMismatch { .. }));
}

// ---------------------------------------------------------------------
// SW-07 — QueueFull is typed on the sessioned staging path
// ---------------------------------------------------------------------

/// Flooding the node's bounded control queue makes transport staging
/// refuse `BoundarySessionError::QueueFull` — never a silent drop.
#[test]
fn tim03_sw_07_driver_queue_full_typed() {
    let (lowered, ids) = lowered_inserts(1);
    let hosted = host_plugin();
    let mut driver = HeadlessDriver::new(engine_config());
    driver
        .activate_with_attached(
            &lowered,
            Vec::new(),
            vec![AttachedNode {
                node: hosted.attached.node,
                dispatch: ParamDispatch::new(hosted.attached.control, ids[0], frame_base())
                    .unwrap(),
                boundary: Some(spec()),
            }],
        )
        .expect("activate");

    // Fill the bounded channel without running blocks (nothing
    // drains). Raw pushes are the flood; the driver's own staging is
    // what must refuse typed afterwards.
    {
        let (_session, dispatch) = driver.attached_boundary(ids[0]).expect("session");
        let mut flooded = false;
        for _ in 0..16_384 {
            if dispatch
                .control()
                .request(TransportCommand::Play, 0)
                .is_err()
            {
                flooded = true;
                break;
            }
        }
        assert!(flooded, "control queue never filled");
    }

    let err = driver.set_loop(0, 64).unwrap_err();
    assert!(
        matches!(err, BoundarySessionError::QueueFull),
        "expected QueueFull, got {err:?}"
    );
}

// ---------------------------------------------------------------------
// SW-08 — capability refusals fail activation before publish
// ---------------------------------------------------------------------

/// A spec whose capability is certified `Unsupported`, or whose
/// offline-exact intent lacks certified precision, refuses at session
/// open — `ActivateError::BoundarySession` before anything publishes.
#[test]
fn tim03_sw_08_driver_unsupported_spec_fails_activation() {
    for (capability, mode) in [
        (
            ProcessCapability::new(
                BlockContract::new(1, 512, InBlockTransportSupport::Unknown),
                TimingPrecision::Unsupported,
            ),
            ExecutionMode::Realtime,
        ),
        (
            ProcessCapability::new(
                BlockContract::new(1, 512, InBlockTransportSupport::Unknown),
                TimingPrecision::Unknown,
            ),
            ExecutionMode::OfflineExact,
        ),
    ] {
        let (lowered, ids) = lowered_inserts(1);
        let hosted = host_plugin();
        let mut driver = HeadlessDriver::new(engine_config());
        let err = driver
            .activate_with_attached(
                &lowered,
                Vec::new(),
                vec![AttachedNode {
                    node: hosted.attached.node,
                    dispatch: ParamDispatch::new(hosted.attached.control, ids[0], frame_base())
                        .unwrap(),
                    boundary: Some(BoundarySessionSpec {
                        capability,
                        mode,
                        consent: None,
                        budget: ScheduleBudget::default(),
                    }),
                }],
            )
            .unwrap_err();
        assert!(
            matches!(
                err,
                ActivateError::BoundarySession(BoundarySessionError::Capability(_))
            ),
            "expected BoundarySession(Capability), got {err:?}"
        );
    }
}

// ---------------------------------------------------------------------
// SW-02 / SW-03 / SW-06 — routing vend + scoped staging + swaps
// ---------------------------------------------------------------------

/// A declared processor's surface opens a session sharing the
/// binding session's instance id and pinning the surface's
/// capability/graph generations; an undeclared sibling reports
/// `NoSession`.
#[test]
fn tim03_sw_02_routing_session_vend_and_provenance() {
    let root = scratch("vend");
    let (mut c, media, ids) = plugin_project(&root, true);
    let target = add_plugin_param(&mut c, ids.first, "gain", 7);
    let rev = c.live_revision();
    let mut adapter = rec_host();
    adapter.register_boundary(
        ids.first,
        declaration(TimingPrecision::FrameAccurateEvents, None),
    );
    let mut eng = routing_engine();
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
    assert_eq!(active_generation(&eng), Some(1));

    let processors = [ids.first, ids.second.unwrap()];
    let dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(&processors)).unwrap();

    // Declared: session + pin — the same instance the binding session
    // minted, the live capability generation, the surface generation.
    let session = dispatch.boundary_session(ids.first).expect("session");
    let pin = session.provenance();
    let binding = dispatch.binding(target).expect("bound target");
    assert_eq!(pin.instance(), binding.instance_id);
    assert_eq!(pin.node(), binding.node);
    assert_eq!(pin.capability_generation(), binding.capability_generation);
    assert_eq!(pin.graph_generation(), binding.graph_generation);
    assert_eq!(pin.graph_generation(), 1);
    assert_eq!(
        pin.capability_generation(),
        port_capability_generation(c.live())
    );

    // Undeclared sibling: no session — legacy attach, `NoSession`.
    assert!(dispatch.boundary_session(ids.second.unwrap()).is_none());
    let err = dispatch
        .boundary_plan_window(ids.second.unwrap(), c.live(), 256, vec![])
        .unwrap_err();
    assert!(matches!(err, BoundaryStageError::NoSession(_)));
}

/// Scoped window evaluation/staging: pure plan eval needs no
/// activation evidence; staging gates on the RT-observed generation
/// and refuses oracle-illegal windows before pushing anything.
#[test]
fn tim03_sw_03_routing_plan_and_stage_window() {
    let root = scratch("stage");
    let (mut c, media, ids) = plugin_project(&root, false);
    add_plugin_param(&mut c, ids.first, "gain", 7);
    let rev = c.live_revision();
    let mut adapter = rec_host();
    adapter.register_boundary(
        ids.first,
        declaration(TimingPrecision::FrameAccurateEvents, None),
    );
    let mut eng = routing_engine();
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
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(&[ids.first])).unwrap();

    // Pure evaluation — no active-generation gate.
    let plan = dispatch
        .boundary_plan_window(ids.first, c.live(), 256, vec![])
        .expect("plan_window");
    assert_eq!(plan.policy, DeliveryPolicy::Exact);
    assert_eq!(plan.segments.last().unwrap().end, 256);

    // A Seek inside the window contributes its boundary offset.
    let staged = dispatch
        .boundary_stage_window(
            ids.first,
            c.live(),
            256,
            vec![],
            &[(TransportCommand::Seek(MediaTime::new(64, frame_base())), 32)],
            Some(1),
        )
        .expect("stage_window");
    assert!(staged.boundaries.contains(&32), "{:?}", staged.boundaries);

    // Stale/inactive generation evidence refuses typed.
    for active in [None, Some(2)] {
        let err = dispatch
            .boundary_stage_window(ids.first, c.live(), 256, vec![], &[], active)
            .unwrap_err();
        assert!(
            matches!(
                err,
                BoundaryStageError::InactiveGeneration { surface: 1, .. }
            ),
            "active {active:?}: {err:?}"
        );
    }

    // Oracle-illegal window refuses before staging anything.
    let err = dispatch
        .boundary_stage_window(ids.first, c.live(), 0, vec![], &[], Some(1))
        .unwrap_err();
    assert!(matches!(
        err,
        BoundaryStageError::Session(BoundarySessionError::Evaluate(
            ProcessBoundaryError::InvalidRequest { .. }
        ))
    ));
}

/// Forged provenance presented to the vended session refuses typed on
/// every pin axis; a generation swap mints fresh pins and the old
/// surface's staging gate rejects the stale active generation.
#[test]
fn tim03_sw_06_routing_stale_provenance_and_generation_swap() {
    let root = scratch("swap");
    let (mut c, media, ids) = plugin_project(&root, false);
    add_plugin_param(&mut c, ids.first, "gain", 7);
    let rev1 = c.live_revision();
    let mut adapter = rec_host();
    adapter.register_boundary(
        ids.first,
        declaration(TimingPrecision::FrameAccurateEvents, None),
    );
    let mut eng = routing_engine();
    eng.set_processor_host(Box::new(adapter));

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
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(&[ids.first])).unwrap();
    let pin1 = dispatch.boundary_provenance(ids.first).expect("pin1");

    // Forged pins refuse — instance, node, capability and graph axes.
    let session = dispatch.boundary_session(ids.first).expect("session");
    for forged in [
        PrecisionProvenance::new(StablePluginInstanceId::new(99), pin1.node(), 0, 1),
        PrecisionProvenance::new(
            StablePluginInstanceId::new(1),
            foreign_node_id(pin1.node()),
            0,
            1,
        ),
        PrecisionProvenance::new(pin1.instance(), pin1.node(), 7, 1),
        PrecisionProvenance::new(pin1.instance(), pin1.node(), 0, 2),
    ] {
        let err = session.plan_window(&forged, 256, vec![]).unwrap_err();
        assert!(
            matches!(
                err,
                BoundarySessionError::InstanceMismatch { .. }
                    | BoundarySessionError::NodeMismatch { .. }
                    | BoundarySessionError::CapabilityGeneration { .. }
                    | BoundarySessionError::GraphGeneration { .. }
            ),
            "forged pin must refuse typed, got {err:?}"
        );
    }

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
    assert_eq!(active_generation(&eng), Some(1));

    let synced = dispatch
        .sync(&mut eng, 2, c.live(), &catalog(&[ids.first]))
        .unwrap();
    assert!(matches!(synced, synvelis_player::SyncOutcome::Swapped(_)));

    // Fresh surface → fresh session → fresh pins. PC-C4-05: the
    // instance id is the persisted authored identity — stable across
    // generations for the same processor/plugin pair; freshness lives
    // in the graph generation pin and the per-generation session.
    let pin2 = dispatch.boundary_provenance(ids.first).expect("pin2");
    assert_eq!(pin2.graph_generation(), 2);
    assert_eq!(
        pin2.instance(),
        pin1.instance(),
        "persisted instance identity is generation-stable"
    );
    assert_ne!(pin2, pin1, "gen-2 pin is a fresh record");

    // The gen-1 active evidence no longer matches the surface.
    let err = dispatch
        .boundary_stage_window(ids.first, c.live(), 256, vec![], &[], Some(1))
        .unwrap_err();
    assert!(matches!(
        err,
        BoundaryStageError::InactiveGeneration {
            surface: 2,
            active: Some(1)
        }
    ));

    // Once RT observes gen 2 the same call stages.
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    assert_eq!(active_generation(&eng), Some(2));
    dispatch
        .boundary_stage_window(ids.first, c.live(), 256, vec![], &[], Some(2))
        .expect("gen-2 stage_window");
}
