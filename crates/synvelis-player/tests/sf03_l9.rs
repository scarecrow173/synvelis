//! SF-03 slice 9 acceptance: host-side capability generation folded
//! into the engine's capability pin — `PluginCapabilityChanged` and
//! reconcile `CapabilityGeneration` fire *live* on host registration
//! drift, no longer shadowed by `SpeculativeMismatch`.
//!
//! Evidence under test:
//! - `ProcessorHost::capability_generation` tracks every registration
//!   axis of `PluginHostProcessors`: adapter-owned boundary/catalog
//!   epochs and factory/route drift visible through the composite
//!   host's audio `Scan` (including drift registered through the
//!   shared `PluginHost` handle after the adapter is boxed).
//! - `publish` re-validation rejects a prepared bundle whose pinned
//!   host generation moved — with the project fingerprint untouched,
//!   so before slice 9 this publish succeeded silently.
//! - `reconcile` reports `CapabilityGeneration` /
//!   `PluginCapabilityChanged` on host drift while the last-good
//!   plan stays live.
//! - Host drift defeats the `NoRebuild` equivalence probe: a
//!   semantically identical revision must rebuild, not shortcut onto
//!   stale capability evidence.
//! - Swapping the processor host between `prepare` and `publish` is
//!   capability drift.
//!
//! `ActivationNotObserved`, `RetireOverflow` and reconcile
//! `DeviceEpoch` stay defensive-only in this slice: the engine's
//! epoch/instance are pinned at construction with no transition API,
//! the retire channel overflow needs a Ruviform-side fault the
//! headless harness cannot inject deterministically, and a published
//! plan's RT activation cannot be withheld without a fault-injection
//! seam — all recorded `NotRun` in the slice-9 IR/matrix.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use synvelis_player::ImportOutcome;
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::{
    BoundaryDeclaration, PluginHostProcessors, ProcessorHost, StaticProcessorHost,
};
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::process_boundary::TimingPrecision;
use synvelis_player::resolver::{PreparedClip, prepare};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine, RoutingPrepare};
use synvelis_project::{
    BuildBudget, BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command,
    CounterIdGenerator, MediaDuration, MediaTimeValue, PlaybackMapping, ProcessorBinding,
    ProcessorId, Project, ProjectController, ProjectId, ProjectOp, ProjectRevision,
    RoutingActivationState, RoutingBuildRequestId, RoutingCompileRequest, RoutingFault,
    RoutingReconcileMismatch, RuntimeClockEpochRef, RuntimeInstanceId, SourceFrame, SourceId,
    SourceRange, TimeAnchor,
};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_core::{InBlockTransportSupport, ScheduleBudget};
use ruviform_media_reader::PrefetchConfig;
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventList,
    ParamFlags, ParamId, ParamInfo, Plugin, PluginDescriptor, PluginFactory, PluginProcessor,
    PrepareError, ProcessConfig, ProcessContext, ProcessResult, TransportInfo, erase,
    erase_sendable,
};
use ruviform_plugin_host::PluginHost;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf03l9-{}-{}-{}",
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
        Project::new(ProjectId::new(0x106), "sf03l9"),
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

fn deps_default(project: &Project, media_root: &Path) -> RoutingDeps<'static> {
    RoutingDeps {
        canonical_of: &|_| Some(canonical_format()),
        clips: prepared_clips(project, media_root),
        engine_config_generation: 1,
        source_format_generation: 1,
    }
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

/// Minimal project plus one gain chain processor resolved to
/// [`GAIN_DESCRIPTOR`] — the fixture every engine-level test shares.
fn gain_project(root: &Path) -> (ProjectController, PathBuf) {
    let (mut c, media) = minimal_project(root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    insert_processor(
        &mut c,
        track,
        0,
        "gain",
        resolved_binding(GAIN_DESCRIPTOR.id),
        false,
    );
    (c, media)
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

const GAIN_DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.sf03-l9-gain",
    name: "SF03-L9 Gain",
    vendor: "Synvelis",
    version: "0.1.0",
    features: &[],
};

/// A second registered plugin — registering it after `prepare`
/// changes the host's `Scan` table without touching the project.
const GAIN_B_DESCRIPTOR: PluginDescriptor = PluginDescriptor {
    id: "org.synvelis.test.sf03-l9-gain-b",
    name: "SF03-L9 Gain B",
    vendor: "Synvelis",
    version: "0.1.0",
    features: &[],
};

/// Shared observability for hosted fixture plugins.
#[derive(Clone, Default)]
struct PluginTally {
    created: Arc<AtomicUsize>,
    activated: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
}

/// A gain plugin: copies input bus 0 to output bus 0 scaled by
/// `gain`. `Drop` reports instance teardown.
struct GainPlugin {
    gain: f32,
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

/// Factory producing `GainPlugin`s.
struct GainFactory {
    gain: f32,
    buses: &'static [BusDescriptor],
    descriptor: PluginDescriptor,
    tally: PluginTally,
}

impl GainFactory {
    fn plugin(&self) -> GainPlugin {
        self.tally.created.fetch_add(1, Ordering::Relaxed);
        GainPlugin {
            gain: self.gain,
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

/// A `PluginHost` with one registered gain factory plus the shared
/// handle the composition root keeps for post-boxing registrations.
fn gain_host() -> (PluginHostProcessors, PluginTally, Rc<RefCell<PluginHost>>) {
    let tally = PluginTally::default();
    let mut host = PluginHost::new();
    host.register_factory(Box::new(GainFactory {
        gain: 0.5,
        buses: &STEREO_BUSES,
        descriptor: GAIN_DESCRIPTOR,
        tally: tally.clone(),
    }));
    let processors = PluginHostProcessors::new(host);
    let host_rc = processors.host();
    (processors, tally, host_rc)
}

/// Registers the second gain factory through the shared
/// `PluginHost` handle — factory drift invisible to `Project`.
fn register_gain_b(host_rc: &Rc<RefCell<PluginHost>>) {
    host_rc.borrow_mut().register_factory(Box::new(GainFactory {
        gain: 0.25,
        buses: &STEREO_BUSES,
        descriptor: GAIN_B_DESCRIPTOR,
        tally: PluginTally::default(),
    }));
}

fn boundary_declaration() -> BoundaryDeclaration {
    BoundaryDeclaration {
        precision: TimingPrecision::Unknown,
        in_block: InBlockTransportSupport::Unknown,
        consent: None,
        budget: ScheduleBudget::new(64, 256, 16, 4096),
    }
}

fn gain_param() -> ParamInfo {
    ParamInfo {
        id: ParamId(0),
        name: "Gain",
        unit: "dB",
        min_value: -60.0,
        max_value: 12.0,
        default_value: 0.0,
        flags: ParamFlags::AUTOMATABLE,
    }
}

// ---------------------------------------------------------------- tests

/// The adapter's capability generation is a deterministic change
/// detector over every registration axis: stable absent mutation,
/// moved by param-catalog registration, boundary declaration
/// registration, and factory drift through the shared `PluginHost`
/// handle.
#[test]
fn sf03_l9_01_host_generation_tracks_registrations() {
    let (mut processors, _tally, host_rc) = gain_host();
    let baseline = processors.capability_generation();
    assert_eq!(processors.capability_generation(), baseline);

    // Param catalog registration moves the generation.
    processors.register_params(GAIN_DESCRIPTOR.id, vec![gain_param()]);
    let after_params = processors.capability_generation();
    assert_ne!(after_params, baseline);
    assert_eq!(processors.capability_generation(), after_params);

    // Boundary declaration registration moves it again.
    processors.register_boundary(ProcessorId::new(1), boundary_declaration());
    let after_boundary = processors.capability_generation();
    assert_ne!(after_boundary, after_params);

    // Factory drift through the shared handle — the only mutation
    // channel left once the adapter is boxed into the engine.
    register_gain_b(&host_rc);
    let after_factory = processors.capability_generation();
    assert_ne!(after_factory, after_boundary);
    assert_eq!(processors.capability_generation(), after_factory);
}

/// Publish re-validation fires `PluginCapabilityChanged` *live*:
/// the prepared bundle pinned the host generation at `prepare`,
/// factory drift moved it before `publish`, and the project
/// fingerprint — blind to host state — still matches. The rejected
/// publish records `Blocked` and keeps last-good live.
#[test]
fn sf03_l9_02_publish_rejects_factory_drift() {
    let root = scratch("l902");
    let (c, media) = gain_project(&root);
    let (processors, _tally, host_rc) = gain_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(processors));

    // Gen 1: build, publish, RT-activate — last-good exists.
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
    eng.run(2, PumpPolicy::Every(1)).unwrap();
    assert_eq!(eng.receipt().unwrap().last_good_generation, Some(1));

    // Gen 2 prepare pins the current host generation.
    let prepared = eng
        .prepare(
            &request(2, rev),
            c.live(),
            rev,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();

    // Factory drift — the project model is untouched.
    register_gain_b(&host_rc);

    let err = eng.publish(prepared, c.live(), rev).err().unwrap();
    assert_eq!(err, RoutingFault::PluginCapabilityChanged);
    assert!(matches!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Blocked {
            fault: RoutingFault::PluginCapabilityChanged,
            last_good_generation: Some(1),
        }
    ));
    // Last-good plan still owns the runtime.
    assert_eq!(eng.rt_status().active_generation, 1);

    // A fresh prepare under the drifted generation publishes cleanly
    // — the fault is drift, not a poisoned host.
    let prepared = eng
        .prepare(
            &request(3, rev),
            c.live(),
            rev,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    let generation = prepared.generation;
    eng.publish(prepared, c.live(), rev).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    assert_eq!(eng.rt_status().active_generation, generation);
}

/// `reconcile` reports `CapabilityGeneration` with fault
/// `PluginCapabilityChanged` when the host generation moved after
/// publish — while revision, fingerprint and every other aspect
/// stay clean, proving the mismatch is host-side, not project-side.
#[test]
fn sf03_l9_03_reconcile_reports_host_capability_drift() {
    let root = scratch("l903");
    let (c, media) = gain_project(&root);
    let (processors, _tally, host_rc) = gain_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(processors));

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
    eng.run(2, PumpPolicy::Every(1)).unwrap();
    assert!(matches!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Active { .. }
    ));

    // Clean reconcile before drift — baseline.
    let clean = eng.reconcile(c.live(), rev, 1, 1);
    assert_eq!(clean.mismatches, Vec::new());
    assert_eq!(clean.fault, None);

    // Host drift after activation.
    register_gain_b(&host_rc);

    let report = eng.reconcile(c.live(), rev, 1, 1);
    assert_eq!(report.plan_key.unwrap().project_revision, rev);
    assert_eq!(
        report.mismatches,
        vec![RoutingReconcileMismatch::CapabilityGeneration]
    );
    assert_eq!(report.fault, Some(RoutingFault::PluginCapabilityChanged));
    assert!(matches!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Blocked {
            fault: RoutingFault::PluginCapabilityChanged,
            last_good_generation: Some(1),
        }
    ));
    // The published plan stays live — drift is reported, not applied.
    assert_eq!(eng.rt_status().active_generation, 1);
    let after = eng.run(1, PumpPolicy::Every(1)).unwrap();
    assert!(after.frames > 0);
}

/// Host drift defeats `NoRebuild` equivalence: a revision that only
/// renames the project is semantically identical, but the live plan
/// was built under a different host capability generation — prepare
/// must build fresh, and the fresh publish succeeds under the
/// drifted generation.
#[test]
fn sf03_l9_04_host_drift_defeats_equivalence() {
    let root = scratch("l904");
    let (mut c, media) = gain_project(&root);
    let (processors, _tally, host_rc) = gain_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(processors));

    let rev1 = c.live_revision();
    let prepared = eng
        .prepare(
            &request(1, rev1),
            c.live(),
            rev1,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(prepared, c.live(), rev1).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();

    // Rename only — semantically identical revision, fingerprint
    // untouched. Engine A's host then drifts before its prepare.
    apply(
        &mut c,
        ProjectOp::SetProjectName {
            name: "renamed".into(),
        },
    );
    let rev2 = c.live_revision();

    // --- engine A: drift before the rename prepare → Built, not
    // Equivalent. -------------------------------------------------
    register_gain_b(&host_rc);
    let outcome = eng
        .prepare(
            &request(2, rev2),
            c.live(),
            rev2,
            deps_default(c.live(), &media),
        )
        .unwrap();
    assert!(
        matches!(outcome, RoutingPrepare::Built(_)),
        "host drift must defeat NoRebuild equivalence"
    );
    let prepared = outcome.expect_built();
    eng.publish(prepared, c.live(), rev2).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    assert!(matches!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Active { .. }
    ));

    // --- engine B: same rename without drift → Equivalent. --------
    let (mut c2, media2) = gain_project(&scratch("l904b"));
    let (processors2, _t2, _h2) = gain_host();
    let mut eng2 = new_engine();
    eng2.set_processor_host(Box::new(processors2));
    let r1 = c2.live_revision();
    let p = eng2
        .prepare(
            &request(1, r1),
            c2.live(),
            r1,
            deps_default(c2.live(), &media2),
        )
        .unwrap()
        .expect_built();
    eng2.publish(p, c2.live(), r1).unwrap();
    eng2.run(1, PumpPolicy::Every(1)).unwrap();
    apply(
        &mut c2,
        ProjectOp::SetProjectName {
            name: "renamed".into(),
        },
    );
    let r2 = c2.live_revision();
    let outcome2 = eng2
        .prepare(
            &request(2, r2),
            c2.live(),
            r2,
            deps_default(c2.live(), &media2),
        )
        .unwrap();
    assert!(
        matches!(outcome2, RoutingPrepare::Equivalent(_)),
        "without drift the rename stays on the NoRebuild path"
    );
}

/// Swapping the processor host between `prepare` and `publish` is
/// capability drift: the prepared bundle pinned the old host's
/// generation and the new host reports a different one.
#[test]
fn sf03_l9_05_host_swap_is_capability_drift() {
    let root = scratch("l905");
    let (c, media) = gain_project(&root);
    let (processors, _tally, _host_rc) = gain_host();
    let mut eng = new_engine();
    eng.set_processor_host(Box::new(processors));

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

    // Swap to a host with different capability state before publish.
    eng.set_processor_host(Box::new(StaticProcessorHost::new()));

    let err = eng.publish(prepared, c.live(), rev).err().unwrap();
    assert_eq!(err, RoutingFault::PluginCapabilityChanged);
}
