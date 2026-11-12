//! PC-C4-07 capability-drift → republish → rebind junction tests.
//!
//! Evidence under test — the headless half of Issue #101's
//! capability-binding contract:
//!
//! - `PluginHostProcessors::register_params` / `register_boundary`
//!   drift bumps the host registration epoch, moving
//!   [`ProcessorHost::capability_generation`] — publish re-validation
//!   then refuses the prepared plan with
//!   `PluginCapabilityChanged` instead of letting stale capability
//!   evidence reach RT.
//! - `RoutingEngine::reconcile` reports the same drift as typed
//!   `CapabilityGeneration` evidence.
//! - A fresh `prepare` pins the *new* host capability generation;
//!   publish succeeds, RT observes the new generation, and
//!   [`RoutingControlDispatch::sync`] rebinds the surface — fresh
//!   session, fresh plugin instance id, `graph_generation` equal to
//!   the new routing plan generation.
//! - Stale-generation evidence stays gated: `observe_block` returns
//!   `None` for foreign generations, `fast_control` under a stale
//!   `active_generation` only queues, `boundary_stage_window` refuses
//!   typed `InactiveGeneration`, and `drain_output_reports` gates on
//!   generation before touching any endpoint.
//!
//! Headless only. The `SharedAdapter` wrapper keeps
//! `register_params`/`register_boundary` reachable after the engine
//! takes ownership of the host — the same `&mut` registration surface
//! a composition root would hold before handing the host over.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_core::{
    ClockEpochId, EngineFrame, InBlockTransportSupport, MediaTime, ScheduleBudget, TimeBase,
    TransportCommand,
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
use synvelis_player::process_boundary::BoundarySessionSpec;
use synvelis_player::resolver::{PreparedClip, prepare};
use synvelis_player::routing_control::{
    BoundaryStageError, FastControlOutcome, FastControlWrite, ParamCatalog,
};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::{
    BoundaryDeclaration, ProcessorConnectError, ProcessorConnectRequest, ProcessorConnection,
    ProcessorHost, RoutingControlDispatch, SyncOutcome, TimingPrecision,
};
use synvelis_project::{
    AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange, BuildBudget,
    BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, ConflictPolicy,
    ControlOwner, CounterIdGenerator, Interpolation, MediaTimeValue, ParamMapping, ParameterRole,
    ParameterTargetId, PlaybackMapping, ProcessorBinding, ProcessorId, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRevision, RoutingBuildRequestId, RoutingCompileRequest,
    RoutingFault, RoutingReconcileMismatch, RuntimeClockEpochRef, RuntimeInstanceId, SourceFrame,
    SourceId, SourceRange, TargetCapability, TimeAnchor, ValueKind,
};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-pcc407rb-{}-{}-{}",
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
        Project::new(ProjectId::new(0x107), "pcc407rb"),
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
/// plus a linear automation lane — same shape as the pc-c4-03/07
/// fixtures.
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
    id: "org.synvelis.test.pc-c4-07rb-rec",
    name: "PC-C4-07 Rebind Reporter",
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

/// Records delivered parameter events — kept for parity with the
/// sibling fixtures; the rebind evidence lives in bindings/gens.
#[derive(Debug)]
struct Delivered {
    #[allow(dead_code)]
    abs_frame: u64,
    #[allow(dead_code)]
    param_id: u32,
    #[allow(dead_code)]
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

// ---------------------------------------------------------------- host wrapper

/// `ProcessorHost` over a shared `PluginHostProcessors` — keeps
/// `register_params`/`register_boundary` reachable after the engine
/// takes ownership, so capability drift can be injected between
/// `prepare` and `publish` exactly where a composition root's
/// registration surface would move it.
struct SharedAdapter(Rc<RefCell<PluginHostProcessors>>);

impl ProcessorHost for SharedAdapter {
    fn connect(
        &mut self,
        request: ProcessorConnectRequest,
    ) -> Result<ProcessorConnection, ProcessorConnectError> {
        self.0.borrow_mut().connect(request)
    }

    fn boundary_attach(&self, request: &ProcessorConnectRequest) -> Option<BoundarySessionSpec> {
        self.0.borrow().boundary_attach(request)
    }

    fn capability_generation(&self) -> u64 {
        self.0.borrow().capability_generation()
    }
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

/// TIM-03 boundary declaration — registered on the shared adapter so
/// `boundary_stage_window` gating is exercisable post-rebind.
fn boundary_decl() -> BoundaryDeclaration {
    BoundaryDeclaration {
        precision: TimingPrecision::FrameAccurateEvents,
        in_block: InBlockTransportSupport::Unknown,
        consent: None,
        budget: ScheduleBudget::new(64, 256, 16, 4096),
    }
}

// ---------------------------------------------------------------- tests

/// PC-C4-07-RB-01: `register_params` drift between `prepare` and
/// `publish` blocks the publish with `PluginCapabilityChanged`,
/// `reconcile` reports `CapabilityGeneration`, a fresh `prepare`
/// republishes under the new host capability pin, `sync` rebinds the
/// surface to the new generation (fresh instance id, `graph_generation`
/// = new plan generation), and every stale-generation write/observe
/// path stays gated.
#[test]
fn pc_c4_07_rebind_params_drift_junction() {
    let root = scratch("params");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (target, _lane) = add_gain_target(&mut c, processor_id);
    let (adapter, instances) = report_host();
    let shared = Rc::new(RefCell::new(adapter));
    shared
        .borrow_mut()
        .register_boundary(processor_id, boundary_decl());
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(SharedAdapter(Rc::clone(&shared))));

    publish_and_run(&mut eng, &c, &media, 1);
    let mut dispatch = attach_surface(&mut eng, &c, processor_id, 1);
    let _gen1_binding = dispatch.binding(target).expect("gen-1 binding");

    // Semantic change → prepare gen 2 pins the CURRENT host
    // capability generation.
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

    // Host-side drift: `register_params` bumps the registration epoch
    // → `capability_generation` moves → publish refuses the stale pin.
    shared
        .borrow_mut()
        .register_params(REC_DESCRIPTOR.id, REC_PARAMS.to_vec());
    let err = eng.publish(prepared, c.live(), rev2).unwrap_err();
    assert_eq!(err, RoutingFault::PluginCapabilityChanged);

    // `reconcile` surfaces the same drift as typed evidence (the
    // project-side revision/fingerprint mismatches coexist — the
    // semantic change is real; capability drift is the host half).
    let report = eng.reconcile(c.live(), rev2, 1, 1);
    assert!(
        report
            .mismatches
            .contains(&RoutingReconcileMismatch::CapabilityGeneration),
        "reconcile reports capability drift: {:?}",
        report.mismatches
    );

    // Republish: a fresh prepare pins the NEW host capability
    // generation → publish succeeds. Gen 2's generation reservation
    // was consumed by the blocked publish, so this builds gen 3.
    let prepared = eng
        .prepare(
            &request(3, rev2),
            c.live(),
            rev2,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(prepared, c.live(), rev2).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    assert_eq!(active_generation(&eng), Some(3));

    // `sync` rebinds the surface to gen 3 — `graph_generation` pinned
    // to the new plan generation. `StablePluginInstanceId`/`NodeId`
    // are stable *by design* across generations, so the fresh-instance
    // half of the rebind is evidenced by the instance log: each
    // `prepare`'s `stage_build` connected a new plugin instance
    // (gen 1, the blocked gen 2, and gen 3 → three total).
    let synced = dispatch
        .sync(&mut eng, 3, c.live(), &catalog(processor_id))
        .unwrap();
    assert!(matches!(synced, SyncOutcome::Swapped(_)));
    let binding = dispatch.binding(target).expect("rebound target");
    assert_eq!(binding.graph_generation, 3);
    assert_eq!(
        instances.lock().unwrap().len(),
        3,
        "each prepare minted a fresh plugin instance"
    );

    // Stale-generation observation is gated fail-closed: gens 1 and 2
    // mint nothing; gen 3 observes its own node's cursor domain (one
    // processed block → next head 256).
    assert!(dispatch.observe_block(1, ef(256), 256).is_none());
    assert!(dispatch.observe_block(2, ef(256), 256).is_none());
    let cycles = dispatch
        .observe_block(3, ef(256), 256)
        .expect("gen-3 surface");
    assert!(cycles.contains_key(&processor_id));

    // A write gated on a stale `active_generation` only queues — it
    // can never deliver to the gen-3 node under gen-1 evidence.
    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(target, 100, 0.5, c.live_revision()),
        Some(1),
    );
    assert!(matches!(outcome, FastControlOutcome::Queued { .. }));

    // Boundary staging under a stale active generation is refused
    // typed — `InactiveGeneration`, nothing staged.
    let err = dispatch
        .boundary_stage_window(
            processor_id,
            c.live(),
            256,
            vec![],
            &[(TransportCommand::Stop, 0)],
            Some(1),
        )
        .expect_err("stale active generation refused");
    assert!(matches!(
        err,
        BoundaryStageError::InactiveGeneration {
            surface: 3,
            active: Some(1)
        }
    ));

    // A foreign generation's report drain is gated before any
    // endpoint drains.
    let drain = dispatch.drain_output_reports(&[], 1, active_generation(&eng), |_, _| {
        unreachable!("generation-gated drain never reaches endpoints")
    });
    assert!(drain.attached);
    assert!(drain.generation_gated);
    assert_eq!(drain.generation, 3);
}

/// PC-C4-07-RB-02: `register_boundary` drift moves the same host
/// capability generation — the publish block and reconcile evidence
/// are identical; only the registration surface differs.
#[test]
fn pc_c4_07_rebind_boundary_drift_blocks_publish() {
    let root = scratch("boundary");
    let (mut c, media, processor_id) = plugin_project(&root);
    let (_target, _lane) = add_gain_target(&mut c, processor_id);
    let (adapter, _instances) = report_host();
    let shared = Rc::new(RefCell::new(adapter));
    shared
        .borrow_mut()
        .register_boundary(processor_id, boundary_decl());
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(SharedAdapter(Rc::clone(&shared))));

    publish_and_run(&mut eng, &c, &media, 1);

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

    // Re-registering the boundary declaration bumps the registration
    // epoch unconditionally — capability evidence moved even though
    // the declaration content is identical.
    shared
        .borrow_mut()
        .register_boundary(processor_id, boundary_decl());
    let err = eng.publish(prepared, c.live(), rev2).unwrap_err();
    assert_eq!(err, RoutingFault::PluginCapabilityChanged);

    let report = eng.reconcile(c.live(), rev2, 1, 1);
    assert!(
        report
            .mismatches
            .contains(&RoutingReconcileMismatch::CapabilityGeneration),
        "reconcile reports capability drift: {:?}",
        report.mismatches
    );

    // The last-good gen-1 plan is still live — drift never displaced
    // it.
    assert_eq!(active_generation(&eng), Some(1));
}
