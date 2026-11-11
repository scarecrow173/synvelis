//! Issue #101 / PC-C4-05: persisted `StablePluginInstanceId` —
//! `RoutingControlDispatch` hydrates the instance identity from
//! `Project::plugin_instances` (schema v13) instead of minting a
//! runtime counter. The same authored processor/plugin pair carries
//! the same id across save→reload and engine restart; a missing,
//! plugin-mismatched or out-of-range entry fails closed into
//! `without_control` with zero delivery.
//!
//! Headless only: real OS/plugin/GUI paths are `NotRun`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::PluginHostProcessors;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::resolver::{PreparedClip, prepare};
use synvelis_player::routing_control::ParamCatalog;
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::{RoutingControlDispatch, StablePluginInstanceId};
use synvelis_project::store::InMemoryStore;
use synvelis_project::{
    ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, ControlOwner, CounterIdGenerator,
    LaneId, MediaDuration, MediaTimeValue, ParamMapping, ParameterRole, ParameterTargetId,
    PlaybackMapping, PluginInstanceId, ProcessorBinding, ProcessorId, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRevision, RoutingBuildRequestId, RoutingCompileRequest,
    RuntimeClockEpochRef, RuntimeInstanceId, SourceFrame, SourceId, SourceRange, StripId,
    TargetCapability, TimeAnchor, TrackId, ValueKind,
};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_reader::PrefetchConfig;
use ruviform_plugin_core::{
    BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventList, ParamFlags, ParamId, ParamInfo,
    Plugin, PluginDescriptor, PluginFactory, PluginProcessor, PrepareError, ProcessConfig,
    ProcessContext, ProcessResult, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::PluginHost;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-pc405-{}-{}-{}",
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
        Project::new(ProjectId::new(0x405), "pc405"),
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

fn plugin_ref() -> synvelis_project::PluginRef {
    synvelis_project::PluginRef {
        descriptor: REC_DESCRIPTOR.id.into(),
        version: "0.1.0".into(),
        capability_revision: 0,
    }
}

/// Minimal project: one stereo import, one track strip, one clip, one
/// resolved chain processor — plus a bindable plugin param target.
fn plugin_project(root: &Path) -> (ProjectController, PathBuf, ProcessorId, ParameterTargetId) {
    let mut c = controller();
    let out = import_stereo(&mut c, root, "a.wav", 4_800);
    let (_track, lane, strip) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "rec".into(),
            binding: ProcessorBinding::Resolved(plugin_ref()),
            sidechain_input: false,
        },
    );
    let processor_id = *c.live().routing.processors.keys().next_back().unwrap();
    apply(
        &mut c,
        ProjectOp::AddParameterTarget {
            name: "gain".into(),
            owner: ControlOwner::Processor(processor_id),
            role: ParameterRole::ProcessorParam,
            time_scope: synvelis_project::AutomationTimeScope::ProjectMusical,
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
        .find(|t| t.name == "gain")
        .unwrap()
        .id;
    (c, root.join("media"), processor_id, target)
}

fn catalog(processor: ProcessorId) -> ParamCatalog {
    BTreeMap::from([(processor, REC_PARAMS.to_vec())])
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
    id: "org.synvelis.test.pc405-rec",
    name: "PC405 Recorder",
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

struct RecPlugin;

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
        _events: EventList<'_>,
        _transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: ruviform_plugin_core::BufferView,
        O: ruviform_plugin_core::BufferViewMut,
    {
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

struct RecFactory;

impl PluginFactory for RecFactory {
    fn descriptor(&self) -> PluginDescriptor {
        REC_DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(RecPlugin)
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(RecPlugin))
    }
}

fn rec_host() -> PluginHostProcessors {
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory));
    PluginHostProcessors::new(host)
}

// ---------------------------------------------------------------- tests

/// PC-C4-05-06: `vend_nodes` hydrates the persisted instance id — the
/// binding record's `instance_id` equals the id the project store
/// issued at `InsertProcessor` time (id 1 for the first resolved
/// processor).
#[test]
fn hydration_uses_persisted_instance_id() {
    let root = scratch("hydrate");
    let (c, media, processor, target) = plugin_project(&root);
    assert_eq!(c.live().plugin_instances.instances[&processor].id.get(), 1);
    let rev = c.live_revision();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(rec_host()));
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

    let dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, c.live(), &catalog(processor)).unwrap();
    let report = dispatch.report().unwrap();
    assert!(
        report.without_control.is_empty(),
        "processor must hydrate: {report:?}"
    );
    let binding = dispatch.binding(target).expect("bound target");
    assert_eq!(binding.instance_id, StablePluginInstanceId::new(1));
}

/// PC-C4-05-01/06: save→load restores the identical persisted
/// identity, and a rebuilt engine+dispatch hydrates the same
/// `StablePluginInstanceId` — restart-stable by construction.
#[test]
fn reload_restores_same_instance_id() {
    let root = scratch("reload");
    let (mut c, media, processor, target) = plugin_project(&root);

    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c.live(), c2.live());

    // Rebuild the runtime against the reloaded project — same
    // persisted identity must hydrate.
    let rev2 = c2.live_revision();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(rec_host()));
    let p1 = eng
        .prepare(
            &request(1, rev2),
            c2.live(),
            rev2,
            deps_default(c2.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(p1, c2.live(), rev2).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    let dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, c2.live(), &catalog(processor)).unwrap();
    let binding = dispatch.binding(target).expect("bound target");
    assert_eq!(binding.instance_id, StablePluginInstanceId::new(1));
}

/// PC-C4-05-06: every identity-store defect fails closed — a missing
/// entry, a plugin-pinned mismatch and an out-of-range id each land
/// the processor in `without_control` with no session and no binding.
#[test]
fn missing_or_mismatched_identity_fails_closed() {
    let root = scratch("reject");
    let (c, media, processor, target) = plugin_project(&root);
    let rev = c.live_revision();

    // Baseline engine parts are rebuilt per scenario —
    // `take_processor_control` is one-shot per published generation.
    let build = |project: &Project| {
        let mut eng = new_engine();
        eng.set_processor_host(Box::new(rec_host()));
        let p = eng
            .prepare(
                &request(1, rev),
                project,
                rev,
                deps_default(project, &media),
            )
            .unwrap()
            .expect_built();
        eng.publish(p, project, rev).unwrap();
        eng.run(1, PumpPolicy::Every(1)).unwrap();
        eng
    };

    // (a) Missing entry — the store simply has no identity for the
    // processor (e.g. a state that bypassed ops).
    let mut missing = c.live().clone();
    missing.plugin_instances.instances.clear();
    let mut eng = build(&missing);
    let dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, &missing, &catalog(processor)).unwrap();
    let report = dispatch.report().unwrap();
    assert!(report.without_control.contains(&processor), "{report:?}");
    assert!(dispatch.binding(target).is_none());

    // (b) Plugin mismatch — the entry pins a different authored
    // PluginRef than the instance was connected as.
    let mut mismatched = c.live().clone();
    mismatched
        .plugin_instances
        .instances
        .get_mut(&processor)
        .unwrap()
        .plugin
        .descriptor = "org.other.plugin".into();
    let mut eng = build(&mismatched);
    let dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, &mismatched, &catalog(processor)).unwrap();
    let report = dispatch.report().unwrap();
    assert!(report.without_control.contains(&processor), "{report:?}");
    assert!(dispatch.binding(target).is_none());

    // (c) Stale/out-of-range id — the entry id lies at/past the
    // watermark, i.e. outside the issued range.
    let mut stale = c.live().clone();
    stale
        .plugin_instances
        .instances
        .get_mut(&processor)
        .unwrap()
        .id = PluginInstanceId::new(99);
    let mut eng = build(&stale);
    let dispatch =
        RoutingControlDispatch::attach(&mut eng, 1, &stale, &catalog(processor)).unwrap();
    let report = dispatch.report().unwrap();
    assert!(report.without_control.contains(&processor), "{report:?}");
    assert!(dispatch.binding(target).is_none());
}
