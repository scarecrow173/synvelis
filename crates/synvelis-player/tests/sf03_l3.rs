//! SF-03 slice 3 acceptance: semantic chain processors connect through
//! the [`ProcessorHost`] boundary — `StaticProcessorHost` for the
//! headless fake path and `PluginHostProcessors` driving the real
//! `ruviform-plugin-host` lifecycle
//! (`Instantiate → Initialize → Activate → StartProcessing → attach`).
//!
//! Evidence under test: connect fault → typed [`RoutingFault`] +
//! last-good preservation, generation-scoped lease teardown (pending
//! displacement, RT-observed activation, engine drop), connect-failure
//! cleanup of already-minted leases, and offline fail-closed behavior.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::{
    PluginHostProcessors, ProcessorConnectRequest, ProcessorConnection, ProcessorHost,
    ProcessorLease, StaticProcessorHost,
};
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::resolver::{PreparedClip, prepare};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::routing_lower::GainProcessor;
use synvelis_player::{ImportOutcome, PlayReport};
use synvelis_project::{
    BuildBudget, BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command,
    CounterIdGenerator, MediaDuration, MediaTimeValue, PlaybackMapping, ProcessorBinding,
    ProcessorId, Project, ProjectController, ProjectId, ProjectOp, ProjectRevision,
    RoutingActivationState, RoutingBuildRequestId, RoutingCompileRequest, RoutingFault,
    RuntimeClockEpochRef, RuntimeInstanceId, SourceFrame, SourceId, SourceRange, StripId,
    TimeAnchor,
};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_reader::PrefetchConfig;
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventList,
    ParamInfo, Plugin, PluginDescriptor, PluginFactory, PluginProcessor, PrepareError,
    ProcessConfig, ProcessContext, ProcessResult, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::PluginHost;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf03l3-{}-{}-{}",
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
        Project::new(ProjectId::new(0x106), "sf03l3"),
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

fn import_stereo(
    c: &mut ProjectController,
    root: &Path,
    name: &str,
    frames: usize,
) -> ImportOutcome {
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

fn strip_of_track(project: &Project, track: synvelis_project::TrackId) -> StripId {
    project.routing.strip_of_track(track).map(|s| s.id).unwrap()
}

fn prepared_clips(project: &Project, media_root: &Path) -> Vec<PreparedClip> {
    project
        .clips
        .values()
        .map(|clip| prepare(project, clip.source_id, media_root, &prefetch()).unwrap())
        .collect()
}

fn deps_for<'a>(
    project: &Project,
    media_root: &Path,
    canonical: &'a dyn Fn(SourceId) -> Option<AudioFormat>,
) -> RoutingDeps<'a> {
    RoutingDeps {
        canonical_of: canonical,
        clips: prepared_clips(project, media_root),
        engine_config_generation: 1,
        source_format_generation: 1,
    }
}

fn deps_default(project: &Project, media_root: &Path) -> RoutingDeps<'static> {
    deps_for(project, media_root, &|_| Some(canonical_format()))
}

/// Minimal project: one stereo import, one track strip, one clip —
/// returns the controller and media root.
fn minimal_project(root: &Path) -> (ProjectController, PathBuf) {
    let mut c = controller();
    let out = import_stereo(&mut c, root, "a.wav", 4_800);
    let (_, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    (c, root.join("media"))
}

/// Inserts a chain processor bound to `binding` at `index` of the
/// track strip's chain; returns its semantic id.
fn insert_processor(
    c: &mut ProjectController,
    track: synvelis_project::TrackId,
    index: usize,
    name: &str,
    binding: ProcessorBinding,
    sidechain_input: bool,
) -> ProcessorId {
    let strip = strip_of_track(c.live(), track);
    apply(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index,
            name: name.into(),
            binding,
            sidechain_input,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

fn resolved_binding(descriptor: &str) -> ProcessorBinding {
    ProcessorBinding::Resolved(synvelis_project::PluginRef {
        descriptor: descriptor.into(),
        version: "0.1.0".into(),
        capability_revision: 0,
    })
}

fn unresolved_binding() -> ProcessorBinding {
    ProcessorBinding::Unresolved {
        descriptor: "test.unresolved".into(),
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

/// Reported bus table deliberately disagreeing with the semantic
/// declaration (mono main input vs the engine's stereo ports).
const MONO_IN_BUSES: [BusDescriptor; 2] = [
    BusDescriptor {
        direction: BusDirection::Input,
        layout: ChannelLayout::Mono,
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

const GAIN_DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.sf03-l3-gain",
    name: "SF03-L3 Gain",
    vendor: "Synvelis",
    version: "0.1.0",
    features: &[],
};

const BAD_BUS_DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.sf03-l3-badbus",
    name: "SF03-L3 BadBus",
    vendor: "Synvelis",
    version: "0.1.0",
    features: &[],
};

const FAIL_DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.sf03-l3-fail",
    name: "SF03-L3 Fail",
    vendor: "Synvelis",
    version: "0.1.0",
    features: &[],
};

/// Shared observability for hosted fixture plugins: `created` counts
/// factory instantiations, `activated` counts `prepare` calls (the
/// host's `Activate` hook), `drops` counts plugin destruction (the
/// host's `Destroy` command frees the instance slot).
#[derive(Clone, Default)]
struct PluginTally {
    created: Arc<AtomicUsize>,
    activated: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}

impl PluginTally {
    fn new() -> Self {
        Self::default()
    }
}

/// A gain plugin: copies input bus 0 to output bus 0 scaled by
/// `gain`. `Drop` reports instance teardown.
struct GainPlugin {
    gain: f32,
    fail_prepare: bool,
    buses: &'static [BusDescriptor],
    descriptor: PluginDescriptor,
    tally: PluginTally,
}

impl PluginProcessor for GainPlugin {
    fn prepare(
        &mut self,
        _config: &ProcessConfig,
        _context: &mut ProcessContext,
    ) -> Result<(), PrepareError> {
        self.tally.activated.fetch_add(1, Ordering::Relaxed);
        if self.fail_prepare {
            return Err(PrepareError::ActivationFailed);
        }
        Ok(())
    }

    fn process<I, O>(
        &mut self,
        inputs: &[I],
        outputs: &mut [O],
        _events: EventList<'_>,
        _transport: Option<TransportInfo>,
        _context: &mut ProcessContext,
    ) -> ProcessResult
    where
        I: BufferView,
        O: BufferViewMut,
    {
        let Some(input) = inputs.first() else {
            return ProcessResult::CONTINUE;
        };
        for output in outputs.iter_mut() {
            for c in 0..output.channels() {
                let Some(dst) = output.channel_mut(c) else {
                    continue;
                };
                match input.channel(c) {
                    Some(src) => {
                        for (d, s) in dst.iter_mut().zip(src.iter()) {
                            *d = s * self.gain;
                        }
                    }
                    None => dst.fill(0.0),
                }
            }
        }
        ProcessResult::CONTINUE
    }
}

impl Drop for GainPlugin {
    fn drop(&mut self) {
        self.tally.drops.fetch_add(1, Ordering::Relaxed);
    }
}

impl Plugin for GainPlugin {
    fn descriptor(&self) -> PluginDescriptor {
        self.descriptor
    }
    fn params(&self) -> &[ParamInfo] {
        &[]
    }
    fn buses(&self) -> &[BusDescriptor] {
        self.buses
    }
}

/// Factory producing `GainPlugin`s; `create`/`create_sendable` share
/// the same tally so L0 and sendable paths are both observed.
struct GainFactory {
    gain: f32,
    fail_prepare: bool,
    buses: &'static [BusDescriptor],
    descriptor: PluginDescriptor,
    tally: PluginTally,
}

impl GainFactory {
    fn plugin(&self) -> GainPlugin {
        self.tally.created.fetch_add(1, Ordering::Relaxed);
        GainPlugin {
            gain: self.gain,
            fail_prepare: self.fail_prepare,
            buses: self.buses,
            descriptor: self.descriptor,
            tally: self.tally.clone(),
        }
    }
}

impl PluginFactory for GainFactory {
    fn descriptor(&self) -> PluginDescriptor {
        self.descriptor
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(self.plugin())
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(self.plugin()))
    }
}

/// A `PluginHost` with one registered gain factory (0.5×, stereo
/// buses matching the semantic declaration) — the real-host fixture.
fn gain_host() -> (PluginHostProcessors, PluginTally) {
    let tally = PluginTally::new();
    let mut host = PluginHost::new();
    host.register_factory(Box::new(GainFactory {
        gain: 0.5,
        fail_prepare: false,
        buses: &STEREO_BUSES,
        descriptor: GAIN_DESCRIPTOR,
        tally: tally.clone(),
    }));
    (PluginHostProcessors::new(host), tally)
}

/// Runs prepare→publish→`blocks` RT blocks for `project@rev` on
/// `engine` with `deps`; returns the play report.
fn run_semantic(
    engine: &mut RoutingEngine,
    c: &ProjectController,
    media: &Path,
    request_id: u64,
    blocks: usize,
) -> PlayReport {
    let revision = c.live_revision();
    let deps = deps_default(c.live(), media);
    let prepared = engine
        .prepare(&request(request_id, revision), c.live(), revision, deps)
        .unwrap();
    engine.publish(prepared, c.live(), revision).unwrap();
    engine.run(blocks, PumpPolicy::Every(1)).unwrap()
}

/// Reference render: the same project shape with no chain processor.
fn reference_full(root: &Path) -> PlayReport {
    let (c_ref, media_ref) = {
        let mut c = controller();
        let out = import_stereo(&mut c, root, "a.wav", 4_800);
        let (_, lane) = add_track_strip(&mut c);
        add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
        (c, root.join("media"))
    };
    let mut eng = new_engine();
    run_semantic(&mut eng, &c_ref, &media_ref, 1, 4)
}

fn assert_halved(halved: &PlayReport, full: &PlayReport) {
    assert_eq!(halved.pcm.len(), full.pcm.len());
    for (h, f) in halved.pcm.iter().zip(full.pcm.iter()) {
        for (hs, fs) in h.iter().zip(f.iter()) {
            assert!(
                (*hs - fs * 0.5).abs() < 1e-6,
                "expected plugin gain 0.5, got {hs} vs {fs}"
            );
        }
    }
}

// ---------------------------------------------------------------- tests

/// Static boundary: a registered instance connects, is consumed by the
/// build, and processes audio; a second build finds the host empty and
/// fails `PluginUnavailable` — proving provisioning goes through the
/// host, not a retained map.
#[test]
fn sf03_l3_01_static_host_connect_consumes_instance() {
    let root = scratch("l301");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    let processor_id = insert_processor(&mut c, track, 0, "gain", unresolved_binding(), false);

    let mut eng = new_engine();
    let mut host = StaticProcessorHost::new();
    host.insert(processor_id, Box::new(GainProcessor::new(0.5)));
    eng.set_processor_host(Box::new(host));

    let rev = c.live_revision();
    let deps = deps_default(c.live(), &media);
    let prepared = eng.prepare(&request(1, rev), c.live(), rev, deps).unwrap();
    eng.publish(prepared, c.live(), rev).unwrap();
    let halved = eng.run(4, PumpPolicy::Every(1)).unwrap();
    assert_halved(&halved, &reference_full(&root));

    // The instance was consumed by the first connect — bump the
    // revision and the next build has nothing to connect.
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    let source = c.live().clips.values().next().unwrap().source_id;
    add_clip(&mut c, lane, source, 480, 960, 0);
    let rev2 = c.live_revision();
    let deps2 = deps_default(c.live(), &media);
    assert_eq!(
        eng.prepare(&request(2, rev2), c.live(), rev2, deps2)
            .err()
            .unwrap(),
        RoutingFault::PluginUnavailable
    );
}

/// Connect failure mints the typed fault and leaves the last-good
/// plan live: an active generation keeps rendering after a failed
/// prepare.
#[test]
fn sf03_l3_02_connect_failure_preserves_last_good() {
    let root = scratch("l302");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    let processor_id = insert_processor(&mut c, track, 0, "gain", unresolved_binding(), false);

    let mut eng = new_engine();
    let mut host = StaticProcessorHost::new();
    host.insert(processor_id, Box::new(GainProcessor::new(0.5)));
    eng.set_processor_host(Box::new(host));

    // Gen 1: build, publish, activate.
    let rev = c.live_revision();
    let deps = deps_default(c.live(), &media);
    let prepared = eng.prepare(&request(1, rev), c.live(), rev, deps).unwrap();
    eng.publish(prepared, c.live(), rev).unwrap();
    let first = eng.run(4, PumpPolicy::Every(1)).unwrap();
    assert!(matches!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Active { .. }
    ));

    // Swap to an empty host and add a second (unresolvable) processor
    // — the next prepare must fail closed.
    eng.set_processor_host(Box::new(StaticProcessorHost::new()));
    insert_processor(&mut c, track, 1, "missing", unresolved_binding(), false);
    let rev2 = c.live_revision();
    let deps2 = deps_default(c.live(), &media);
    assert_eq!(
        eng.prepare(&request(2, rev2), c.live(), rev2, deps2)
            .err()
            .unwrap(),
        RoutingFault::PluginUnavailable
    );
    let receipt = eng.receipt().unwrap();
    assert_eq!(receipt.last_good_generation, Some(1));
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Failed {
            fault: RoutingFault::PluginUnavailable,
            ..
        }
    ));

    // Last-good still renders: rewind and the same halved audio
    // comes back — the failed prepare touched nothing live.
    eng.seek(0);
    let again = eng.run(4, PumpPolicy::Every(1)).unwrap();
    assert_halved(&again, &reference_full(&root));
    assert_eq!(again.pcm, first.pcm);
}

/// Real host path: `PluginHostProcessors` drives the full plugin
/// lifecycle and the attached `PluginNode` applies the plugin's gain
/// to the rendered audio.
#[test]
fn sf03_l3_03_real_host_gain_processes_audio() {
    let root = scratch("l303");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    insert_processor(
        &mut c,
        track,
        0,
        "gain",
        resolved_binding(GAIN_DESCRIPTOR.id),
        false,
    );

    let (adapter, tally) = gain_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    let rev = c.live_revision();
    let deps = deps_default(c.live(), &media);
    let prepared = eng.prepare(&request(1, rev), c.live(), rev, deps).unwrap();
    eng.publish(prepared, c.live(), rev).unwrap();
    let halved = eng.run(4, PumpPolicy::Every(1)).unwrap();

    assert_halved(&halved, &reference_full(&root));
    // Lifecycle evidence: the plugin was instantiated, its `prepare`
    // (host `Activate`) ran, and nothing was torn down while the
    // generation is live.
    assert_eq!(tally.created.load(Ordering::Relaxed), 1);
    assert_eq!(tally.activated.load(Ordering::Relaxed), 1);
    assert_eq!(tally.drops.load(Ordering::Relaxed), 0);
    assert!(matches!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Active { .. }
    ));
}

/// A `Resolved` binding whose plugin id the host does not know fails
/// `PluginUnavailable` before any instance exists.
#[test]
fn sf03_l3_04_real_host_unknown_plugin_unavailable() {
    let root = scratch("l304");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    insert_processor(
        &mut c,
        track,
        0,
        "gain",
        resolved_binding("org.synvelis.test.not-registered"),
        false,
    );

    let (adapter, _tally) = gain_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    let rev = c.live_revision();
    let deps = deps_default(c.live(), &media);
    assert_eq!(
        eng.prepare(&request(1, rev), c.live(), rev, deps)
            .err()
            .unwrap(),
        RoutingFault::PluginUnavailable
    );
}

/// `Unresolved` bindings never reach the host — they fail
/// `PluginUnavailable` deterministically.
#[test]
fn sf03_l3_05_unresolved_binding_unavailable() {
    let root = scratch("l305");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    insert_processor(&mut c, track, 0, "gain", unresolved_binding(), false);

    let (adapter, tally) = gain_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    let rev = c.live_revision();
    let deps = deps_default(c.live(), &media);
    assert_eq!(
        eng.prepare(&request(1, rev), c.live(), rev, deps)
            .err()
            .unwrap(),
        RoutingFault::PluginUnavailable
    );
    assert_eq!(tally.created.load(Ordering::Relaxed), 0);
}

/// A plugin whose reported bus table disagrees with the semantic
/// declaration fails `UnsupportedFormatOrTimeMap`, and the instance it
/// already started is torn down.
#[test]
fn sf03_l3_06_real_host_bus_mismatch_incompatible() {
    let root = scratch("l306");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    insert_processor(
        &mut c,
        track,
        0,
        "badbus",
        resolved_binding(BAD_BUS_DESCRIPTOR.id),
        false,
    );

    let tally = PluginTally::new();
    let mut host = PluginHost::new();
    host.register_factory(Box::new(GainFactory {
        gain: 0.5,
        fail_prepare: false,
        buses: &MONO_IN_BUSES,
        descriptor: BAD_BUS_DESCRIPTOR,
        tally: tally.clone(),
    }));
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(PluginHostProcessors::new(host)));

    let rev = c.live_revision();
    let deps = deps_default(c.live(), &media);
    assert_eq!(
        eng.prepare(&request(1, rev), c.live(), rev, deps)
            .err()
            .unwrap(),
        RoutingFault::UnsupportedFormatOrTimeMap
    );
    // Instantiate→Initialize→Activate→StartProcessing ran before the
    // bus check; the failure path must have destroyed the instance.
    assert_eq!(tally.created.load(Ordering::Relaxed), 1);
    assert_eq!(tally.drops.load(Ordering::Relaxed), 1);
}

/// A plugin rejecting `Activate` (its `prepare` hook fails) maps to
/// `PluginUnavailable` — a lifecycle rejection, not a format fault.
#[test]
fn sf03_l3_07_real_host_lifecycle_failure() {
    let root = scratch("l307");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    insert_processor(
        &mut c,
        track,
        0,
        "fail",
        resolved_binding(FAIL_DESCRIPTOR.id),
        false,
    );

    let tally = PluginTally::new();
    let mut host = PluginHost::new();
    host.register_factory(Box::new(GainFactory {
        gain: 0.5,
        fail_prepare: true,
        buses: &STEREO_BUSES,
        descriptor: FAIL_DESCRIPTOR,
        tally: tally.clone(),
    }));
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(PluginHostProcessors::new(host)));

    let rev = c.live_revision();
    let deps = deps_default(c.live(), &media);
    assert_eq!(
        eng.prepare(&request(1, rev), c.live(), rev, deps)
            .err()
            .unwrap(),
        RoutingFault::PluginUnavailable
    );
    assert_eq!(tally.created.load(Ordering::Relaxed), 1);
    assert_eq!(tally.drops.load(Ordering::Relaxed), 1);
}

/// Two chain processors where the second cannot connect: the first
/// instance's lease must tear down — every created instance ends up
/// destroyed (`created == drops`), regardless of connect order.
#[test]
fn sf03_l3_08_connect_failure_tears_down_prior_leases() {
    let root = scratch("l308");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    insert_processor(
        &mut c,
        track,
        0,
        "gain",
        resolved_binding(GAIN_DESCRIPTOR.id),
        false,
    );
    insert_processor(
        &mut c,
        track,
        1,
        "missing",
        resolved_binding("org.synvelis.test.not-registered"),
        false,
    );

    let (adapter, tally) = gain_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    let rev = c.live_revision();
    let deps = deps_default(c.live(), &media);
    assert_eq!(
        eng.prepare(&request(1, rev), c.live(), rev, deps)
            .err()
            .unwrap(),
        RoutingFault::PluginUnavailable
    );
    assert_eq!(
        tally.created.load(Ordering::Relaxed),
        tally.drops.load(Ordering::Relaxed),
        "every instantiated plugin must be destroyed on failure"
    );
}

/// Rapid generations: a still-pending generation's leases release the
/// moment a newer publish displaces it; once a generation is
/// RT-observed active, every older generation's instances tear down;
/// engine drop tears down the survivor.
#[test]
fn sf03_l3_09_generation_teardown_lifecycle() {
    let root = scratch("l309");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    let source = c.live().clips.values().next().unwrap().source_id;
    insert_processor(
        &mut c,
        track,
        0,
        "gain",
        resolved_binding(GAIN_DESCRIPTOR.id),
        false,
    );

    let (adapter, tally) = gain_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    // Gen 1 published but never activated, then displaced by gen 2 —
    // its lease tears down (host `Destroy`) at the second publish.
    // The plugin *object* is released one phase later: the detached
    // endpoint inside gen-1's still-queued install holds a `SendCore`
    // share until the RT side discards the stale command.
    let rev1 = c.live_revision();
    let deps1 = deps_default(c.live(), &media);
    let p1 = eng
        .prepare(&request(1, rev1), c.live(), rev1, deps1)
        .unwrap();
    eng.publish(p1, c.live(), rev1).unwrap();
    assert_eq!(tally.created.load(Ordering::Relaxed), 1);
    assert_eq!(tally.drops.load(Ordering::Relaxed), 0);

    add_clip(&mut c, lane, source, 480, 960, 0);
    let rev2 = c.live_revision();
    let deps2 = deps_default(c.live(), &media);
    let p2 = eng
        .prepare(&request(2, rev2), c.live(), rev2, deps2)
        .unwrap();
    eng.publish(p2, c.live(), rev2).unwrap();
    assert_eq!(tally.created.load(Ordering::Relaxed), 2);

    // Gen 2 activates — gen-1's stale install is discarded and its
    // endpoint released, so the destroyed instance's object drops.
    eng.run(4, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    assert!(matches!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Active { generation: 2, .. }
    ));
    assert_eq!(
        tally.drops.load(Ordering::Relaxed),
        1,
        "displaced pending generation must fully tear down"
    );

    // Gen 3 displaces the *active* gen 2: gen 2's leases stay until
    // gen 3 is RT-observed, then release.
    add_clip(&mut c, lane, source, 960, 1_440, 0);
    let rev3 = c.live_revision();
    let deps3 = deps_default(c.live(), &media);
    let p3 = eng
        .prepare(&request(3, rev3), c.live(), rev3, deps3)
        .unwrap();
    eng.publish(p3, c.live(), rev3).unwrap();
    assert_eq!(
        tally.drops.load(Ordering::Relaxed),
        1,
        "still-active generation must keep its instances until swap"
    );
    eng.run(4, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    assert!(matches!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Active { generation: 3, .. }
    ));
    assert_eq!(
        tally.drops.load(Ordering::Relaxed),
        2,
        "superseded active generation must tear down on activation"
    );

    // Engine drop tears down the surviving generation's instance.
    drop(eng);
    assert_eq!(tally.drops.load(Ordering::Relaxed), 3);
}

/// Offline builds fail closed on host-connected processors: a real
/// `PluginNode` reports `OfflineRenderSupport::Unknown`, so the
/// connected instance gates the build as
/// `OfflineCapabilityMismatch` and is torn down.
#[test]
fn sf03_l3_10_offline_real_host_fail_closed() {
    let root = scratch("l310");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    insert_processor(
        &mut c,
        track,
        0,
        "gain",
        resolved_binding(GAIN_DESCRIPTOR.id),
        false,
    );

    let (adapter, tally) = gain_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(adapter));

    let rev = c.live_revision();
    let deps = deps_default(c.live(), &media);
    let offline_req = RoutingCompileRequest {
        mode: BuildMode::Offline,
        ..request(1, rev)
    };
    assert_eq!(
        eng.prepare(&offline_req, c.live(), rev, deps)
            .err()
            .unwrap(),
        RoutingFault::OfflineCapabilityMismatch
    );
    assert_eq!(tally.created.load(Ordering::Relaxed), 1);
    assert_eq!(tally.drops.load(Ordering::Relaxed), 1);
}

/// A custom `ProcessorHost` proves the boundary contract itself:
/// `connect` receives the semantic processor/ports/generation and the
/// returned lease's drop is the teardown signal the engine drives.
struct RecordingHost {
    seen: Arc<AtomicUsize>,
    lease_drops: Arc<AtomicUsize>,
}

impl ProcessorHost for RecordingHost {
    fn connect(
        &mut self,
        request: ProcessorConnectRequest,
    ) -> Result<ProcessorConnection, synvelis_player::ProcessorConnectError> {
        self.seen.fetch_add(1, Ordering::Relaxed);
        assert_eq!(request.mode, BuildMode::Realtime);
        assert!(request.generation >= 1);
        assert!(!request.ports.is_empty());
        let drops = Arc::clone(&self.lease_drops);
        Ok(ProcessorConnection {
            processor: Box::new(GainProcessor::new(0.5)),
            lease: ProcessorLease::new(move || {
                drops.fetch_add(1, Ordering::Relaxed);
            }),
        })
    }
}

#[test]
fn sf03_l3_11_host_boundary_contract_and_lease_teardown() {
    let root = scratch("l311");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    insert_processor(&mut c, track, 0, "gain", unresolved_binding(), false);

    let seen = Arc::new(AtomicUsize::new(0));
    let lease_drops = Arc::new(AtomicUsize::new(0));
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(RecordingHost {
        seen: Arc::clone(&seen),
        lease_drops: Arc::clone(&lease_drops),
    }));

    let rev = c.live_revision();
    let deps = deps_default(c.live(), &media);
    let prepared = eng.prepare(&request(1, rev), c.live(), rev, deps).unwrap();
    eng.publish(prepared, c.live(), rev).unwrap();
    eng.run(2, PumpPolicy::Every(1)).unwrap();
    assert_eq!(seen.load(Ordering::Relaxed), 1);
    assert_eq!(lease_drops.load(Ordering::Relaxed), 0);

    // A second generation displaces the first only after the first is
    // active — activation already happened, so publishing gen 2 keeps
    // gen 1's lease until gen 2 activates.
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    let source = c.live().clips.values().next().unwrap().source_id;
    add_clip(&mut c, lane, source, 480, 960, 0);
    let rev2 = c.live_revision();
    let deps2 = deps_default(c.live(), &media);
    let p2 = eng
        .prepare(&request(2, rev2), c.live(), rev2, deps2)
        .unwrap();
    eng.publish(p2, c.live(), rev2).unwrap();
    eng.run(2, PumpPolicy::Every(1)).unwrap();
    assert_eq!(seen.load(Ordering::Relaxed), 2);
    assert_eq!(
        lease_drops.load(Ordering::Relaxed),
        1,
        "gen-1 lease must drop once gen-2 is observed active"
    );
}
