//! CTL-02 acceptance (issue #122): Host Parameter authority —
//! strip/send channel params are persisted `ParameterTarget`s whose
//! delivery is the lowering-time `base` projection (never a plugin
//! `ParamId`), plugin params bind through the verified two-stage
//! path, and generation disagreement classifies `Stale`/`Pending`
//! instead of delivering.
//!
//! Old-raw-`ParamId` misdelivery is zero by construction and pinned
//! here: `fast_control`/`tick` only ever resolve a
//! `ParameterTargetId` → verified [`PluginParameterBinding`], and
//! internal channel targets block `UnboundTarget` — no code path
//! accepts a bare plugin `ParamId`.
//!
//! Headless only: real OS/plugin/GUI paths are `NotRun`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::PluginHostProcessors;
use synvelis_player::host_param::binding_evidence;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::resolver::{PreparedClip, prepare};
use synvelis_player::routing_control::{FastControlOutcome, ParamCatalog};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::{
    FastControlBlockKind, FastControlWrite, HostParamDelivery, RoutingControlDispatch,
};
use synvelis_project::host_param::strip_channel_target_id;
use synvelis_project::{
    ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, ControlOwner, CounterIdGenerator,
    LaneId, MediaDuration, MediaTimeValue, ParamMapping, ParameterRole, ParameterTargetId,
    PlaybackMapping, ProcessorBinding, ProcessorId, Project, ProjectController, ProjectId,
    ProjectOp, ProjectRevision, RoutingBuildRequestId, RoutingCompileRequest, RuntimeClockEpochRef,
    RuntimeInstanceId, SourceFrame, SourceId, SourceRange, StripId, TargetCapability, TimeAnchor,
    TrackId, ValueKind,
};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_core::{MediaTime, TimeBase};
use ruviform_media_reader::PrefetchConfig;
use ruviform_plugin_core::{
    BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind, EventList, ParamFlags, ParamId,
    ParamInfo, Plugin, PluginDescriptor, PluginFactory, PluginProcessor, PrepareError,
    ProcessConfig, ProcessContext, ProcessResult, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::PluginHost;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-ctl02-{}-{}-{}",
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
        Project::new(ProjectId::new(0x122), "ctl02"),
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
        mode: synvelis_project::BuildMode::Realtime,
        resource_budget: synvelis_project::BuildBudget {
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
    let (_, lane, _) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    (c, root.join("media"))
}

/// `minimal_project` plus one chain processor bound to the param
/// recorder plugin.
fn plugin_project(root: &Path) -> (ProjectController, PathBuf, ProcessorId, StripId) {
    let (mut c, media) = minimal_project(root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
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
    (c, media, processor_id, strip)
}

/// Adds a `ProcessorParam` target on `processor` declaring plugin
/// param `param_id`. Returns the issued target id.
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

/// The caller-vended catalog the surface binds against — the
/// recorder plugin's live `ParamInfo` enumeration.
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
    id: "org.synvelis.test.ctl02-rec",
    name: "CTL02 Recorder",
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
        I: ruviform_plugin_core::BufferView,
        O: ruviform_plugin_core::BufferViewMut,
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

/// `instances[i]` is the i-th created plugin instance's delivery log.
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

// ---------------------------------------------------------------- tests

/// CTL-02-ACC-01: seeded strip channel params are the persisted Host
/// authority but are *internal* — `binding_evidence` reports
/// `Unbound` with no semantic or runtime plugin binding, and a
/// `fast_control` write naming one blocks `UnboundTarget` instead of
/// ever reaching a plugin `ParamId`. The plugin param on the same
/// strip reports the full two-stage binding and `Observed`.
#[test]
fn channel_params_are_internal_and_never_reach_plugin_params() {
    let root = scratch("channel");
    let (mut c, media, processor_id, strip) = plugin_project(&root);
    let plugin_target = add_plugin_param(&mut c, processor_id, "gain", 7);
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
    assert_eq!(active_generation(&eng), Some(1));

    let mut dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(processor_id)).unwrap();
    let map_generation = eng.source_map().map(|m| m.generation);

    // Every seeded channel role resolves to its persisted target and
    // classifies Unbound — internal delivery is the lowering-time
    // `base` projection, a different binding that never touches a
    // plugin ParamId.
    for role in [
        ParameterRole::Fader,
        ParameterRole::Mute,
        ParameterRole::Solo,
    ] {
        let target = strip_channel_target_id(c.live(), strip, role)
            .unwrap_or_else(|| panic!("strip lacks seeded {role:?}"));
        let evidence = binding_evidence(
            c.live(),
            &dispatch,
            map_generation,
            active_generation(&eng),
            target,
        )
        .unwrap();
        assert_eq!(evidence.delivery(), HostParamDelivery::Unbound, "{role:?}");
        assert!(
            evidence.semantic.is_none(),
            "{role:?} has no plugin binding"
        );
        assert!(
            evidence.runtime.is_none(),
            "{role:?} has no runtime binding"
        );
        assert!(dispatch.binding(target).is_none());

        // The old raw-param path cannot deliver it: `fast_control`
        // resolves `ParameterTargetId` → verified binding only, and
        // an internal target has none.
        let outcome = dispatch.fast_control(
            c.live(),
            fast_write(target, 300, 0.9, rev),
            active_generation(&eng),
        );
        assert_eq!(
            outcome,
            FastControlOutcome::Blocked(FastControlBlockKind::UnboundTarget),
            "{role:?} must never reach a plugin param"
        );
    }
    // Pan is deliberately not seeded — its canonical center is not
    // bit-exact with the legacy bypass, so it registers on first use.
    assert!(strip_channel_target_id(c.live(), strip, ParameterRole::Pan).is_none());

    // Contrast: the processor param carries both binding stages and
    // agrees on every generation axis → Observed.
    let evidence = binding_evidence(
        c.live(),
        &dispatch,
        map_generation,
        active_generation(&eng),
        plugin_target,
    )
    .unwrap();
    let semantic = evidence.semantic.as_ref().expect("stage-1 binding");
    assert_eq!(semantic.param_id, ParamId(7));
    assert_eq!(semantic.processor, processor_id);
    assert_eq!(semantic.plugin.descriptor, REC_DESCRIPTOR.id);
    assert_eq!(
        evidence.delivery(),
        HostParamDelivery::Observed { generation: 1 }
    );
    assert!(evidence.runtime.is_some(), "stage-2 verified binding");

    // Nothing internal ever reached the plugin — the delivery log is
    // empty until a real plugin-param write lands.
    eng.run(3, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    assert!(
        instances.lock().unwrap()[0].lock().unwrap().is_empty(),
        "internal channel params must never be delivered as plugin params"
    );
}

/// CTL-02-ACC-02: generation correlation — the published plan
/// generation (source map), the surface's pinned binding generation
/// and the RT-observed active generation must agree before a plugin
/// param is `Observed`; disagreement is `Stale` (reject, never
/// retarget) and a not-yet-active plan is `Pending` (gated).
#[test]
fn plugin_param_delivery_tracks_generation_axes() {
    let root = scratch("gen");
    let (mut c, media, processor_id, _strip) = plugin_project(&root);
    let target = add_plugin_param(&mut c, processor_id, "gain", 7);
    let rev = c.live_revision();
    let (adapter, _instances) = rec_host();
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
    let map_generation = eng.source_map().map(|m| m.generation);
    assert_eq!(map_generation, Some(1));

    // Pending: every pin agrees but the plan is not RT-active yet —
    // gated, never delivered early.
    let evidence = binding_evidence(c.live(), &dispatch, map_generation, None, target).unwrap();
    assert_eq!(
        evidence.delivery(),
        HostParamDelivery::Pending {
            generation: 1,
            active: None
        }
    );

    // Activate → all three axes name generation 1 → Observed.
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    let evidence = binding_evidence(
        c.live(),
        &dispatch,
        map_generation,
        active_generation(&eng),
        target,
    )
    .unwrap();
    assert_eq!(
        evidence.delivery(),
        HostParamDelivery::Observed { generation: 1 }
    );

    // A newer published plan the surface has not synced to makes the
    // pinned binding stale — reject, never retarget into a delivery.
    let evidence = binding_evidence(c.live(), &dispatch, Some(2), Some(1), target).unwrap();
    assert_eq!(
        evidence.delivery(),
        HostParamDelivery::Stale {
            pinned: 1,
            current: 2
        }
    );

    // And the gate itself: while inactive the same write queues;
    // after activation it delivers through the verified binding.
    let queued = dispatch.fast_control(c.live(), fast_write(target, 300, 0.7, rev), None);
    assert!(matches!(queued, FastControlOutcome::Queued { .. }));
    let delivered = dispatch.fast_control(
        c.live(),
        fast_write(target, 300, 0.8, rev),
        active_generation(&eng),
    );
    assert_eq!(delivered, FastControlOutcome::Delivered);
}

/// CTL-02-ACC-03: old-raw-`ParamId` misdelivery stays zero — a
/// target whose declared capability the plugin does not enumerate
/// (param 99 vs the catalog's param 7) binds nothing, blocks typed,
/// and delivers nothing; a non-existent target is `NotFound`. The
/// unresolved-processor stage-1 rejection is covered project-side
/// (`semantic_plugin_binding` is fail-closed there).
#[test]
fn unbound_and_foreign_targets_never_deliver() {
    let root = scratch("foreign");
    let (mut c, media, processor_id, _strip) = plugin_project(&root);
    let bound = add_plugin_param(&mut c, processor_id, "gain", 7);
    // Same processor, capability names a param the plugin does not
    // have — a raw-ParamId-shaped target that must never bind.
    let ghost_capability = add_plugin_param(&mut c, processor_id, "phantom", 99);

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
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(processor_id)).unwrap();
    let map_generation = eng.source_map().map(|m| m.generation);

    // Stage-1 fails closed: unresolved binding and foreign capability
    // have no semantic binding — never a numeric guess.
    let report = dispatch.report().unwrap();
    assert!(
        report.unbound.contains_key(&ghost_capability),
        "param-99 capability must not bind: {report:?}"
    );
    assert!(dispatch.binding(ghost_capability).is_none());

    // Stage-1 records the *authored* claim (param 99); stage-2's
    // live-catalog verification is what fails closed — no runtime
    // binding, `Unbound`, zero delivery.
    let evidence = binding_evidence(
        c.live(),
        &dispatch,
        map_generation,
        active_generation(&eng),
        ghost_capability,
    )
    .unwrap();
    assert_eq!(
        evidence.semantic.as_ref().map(|s| s.param_id),
        Some(ParamId(99)),
        "stage-1 names the authored param id only"
    );
    assert!(evidence.runtime.is_none());
    assert_eq!(evidence.delivery(), HostParamDelivery::Unbound);

    let outcome = dispatch.fast_control(
        c.live(),
        fast_write(ghost_capability, 300, 0.9, rev),
        active_generation(&eng),
    );
    assert!(
        matches!(
            outcome,
            FastControlOutcome::Blocked(
                FastControlBlockKind::UnboundTarget | FastControlBlockKind::Lowering
            )
        ),
        "phantom capability must never deliver: {outcome:?}"
    );

    // A target id that does not exist is a typed `NotFound`, not a
    // delivery.
    let ghost = ParameterTargetId::new(0xDEAD);
    assert!(binding_evidence(c.live(), &dispatch, map_generation, None, ghost).is_err());
    let blocked = dispatch.fast_control(c.live(), fast_write(ghost, 300, 0.5, rev), None);
    assert!(matches!(
        blocked,
        FastControlOutcome::Blocked(
            FastControlBlockKind::UnboundTarget | FastControlBlockKind::Lowering
        ) | FastControlOutcome::Queued { .. }
    ));

    // The bound target still delivers — proving the rejects above are
    // binding-scoped, not a dead surface.
    let delivered = dispatch.fast_control(
        c.live(),
        fast_write(bound, 300, 0.8, rev),
        active_generation(&eng),
    );
    assert_eq!(delivered, FastControlOutcome::Delivered);
    eng.run(2, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    let log = instances.lock().unwrap()[0].lock().unwrap().clone();
    assert_eq!(log.len(), 1, "only the bound target delivered: {log:?}");
    assert_eq!(log[0].param_id, 7);
    assert!((log[0].value - 0.8).abs() < 1e-9, "{log:?}");
}
