//! SF-03 slice 4 acceptance: `NoRebuild` equivalence, generation-scoped
//! processor-control vending, and the SF-ACT-05 headless contract
//! fixed in one place.
//!
//! Evidence under test:
//!
//! - [`RoutingPrepare::Equivalent`] — a committed revision whose
//!   semantic content matches the published plan's
//!   [`RoutingBuildKey`] skips lowering/host-connect/compile entirely:
//!   no generation is reserved, no receipt is recorded, and the live
//!   plan's `Active`/`Published` evidence is untouched.
//! - [`RoutingEngine::take_processor_control`] — one-shot vending of a
//!   published generation's [`PluginNodeControl`]; the channel is
//!   dedicated to that generation's plugin instance, so staged param
//!   writes can never reach another generation's plugin
//!   (wrong-plugin 0 by construction).
//! - SF-ACT-05 headless regression — rapid R1/R2/R3 publishes,
//!   `Superseded` rejection, `PublishQueueFull` last-good preservation,
//!   stale-counter `EventStale` reconcile, `DeviceEpochChanged` and
//!   fresh retry.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::host::{PluginHostProcessors, StaticProcessorHost};
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::resolver::{PreparedClip, prepare};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine, RoutingPrepare};
use synvelis_player::routing_lower::GainProcessor;
use synvelis_player::{ImportOutcome, ParamDispatch};
use synvelis_project::{
    BuildBudget, BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command,
    CounterIdGenerator, MediaDuration, MediaTimeValue, ParameterTargetId, PlaybackMapping,
    ProcessorBinding, ProcessorId, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, RoutingActivationState, RoutingBuildRequestId, RoutingCompileRequest,
    RoutingFault, RoutingReconcileMismatch, RuntimeClockEpochRef, RuntimeInstanceId, SourceFrame,
    SourceId, SourceRange, TimeAnchor,
};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_core::{MediaTime, TimeBase};
use ruviform_media_reader::PrefetchConfig;
use ruviform_plugin_core::{
    BufferView, BufferViewMut, BusDescriptor, BusDirection, BusRole, ErasedPlugin, EventKind,
    EventList, ParamEvent, ParamFlags, ParamId, ParamInfo, ParamValue, Plugin, PluginDescriptor,
    PluginFactory, PluginProcessor, PrepareError, ProcessConfig, ProcessContext, ProcessResult,
    TimedParamEvent, TransportInfo, erase, erase_sendable,
};
use ruviform_plugin_host::PluginHost;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf03l4-{}-{}-{}",
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
        Project::new(ProjectId::new(0x106), "sf03l4"),
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

/// The track's first lane and first clip source — the re-derivation
/// every semantic-change helper needs after `minimal_project`.
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

/// Minimal project: one stereo import, one track strip, one clip.
fn minimal_project(root: &Path) -> (ProjectController, PathBuf) {
    let mut c = controller();
    let out = import_stereo(&mut c, root, "a.wav", 4_800);
    let (_, lane) = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);
    (c, root.join("media"))
}

/// `minimal_project` plus one chain processor bound to the param
/// recorder plugin — the fixture every control-vending test needs.
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

fn resolved_binding(descriptor: &str) -> ProcessorBinding {
    ProcessorBinding::Resolved(synvelis_project::PluginRef {
        descriptor: descriptor.into(),
        version: "0.1.0".into(),
        capability_revision: 0,
    })
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
    id: "org.synvelis.test.sf03-l4-rec",
    name: "SF03-L4 Recorder",
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

/// One delivered parameter event at its absolute engine frame — the
/// per-instance evidence a wrong-plugin delivery would corrupt.
#[derive(Debug, Clone, PartialEq)]
struct Delivered {
    abs_frame: u64,
    param_id: u32,
    value: f64,
}

/// Records every `ParamValue` event the plugin's `process` sees.
/// Each factory instantiation gets its own `delivered` log so two
/// generations' instances are distinguishable.
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

/// Per-instance delivery logs shared between the factory and the
/// test: `instances[i]` is the i-th created plugin instance's log.
type InstanceLogs = Arc<Mutex<Vec<Arc<Mutex<Vec<Delivered>>>>>>;

/// Factory handing every instantiated plugin a fresh delivery log;
/// `instances[i]` is the i-th created instance in connect order.
struct RecFactory {
    instances: InstanceLogs,
}

impl RecFactory {
    fn plugin(&self) -> RecPlugin {
        let delivered = Arc::new(Mutex::new(Vec::new()));
        self.instances.lock().unwrap().push(Arc::clone(&delivered));
        RecPlugin { delivered }
    }
}

impl PluginFactory for RecFactory {
    fn descriptor(&self) -> PluginDescriptor {
        REC_DESCRIPTOR
    }
    fn create(&self) -> Box<dyn ErasedPlugin> {
        erase(self.plugin())
    }
    fn create_sendable(&self) -> Option<Box<dyn ErasedPlugin + Send>> {
        Some(erase_sendable(self.plugin()))
    }
}

/// A `PluginHost` with the recorder factory — returns the engine-side
/// adapter plus the shared per-instance delivery logs.
fn rec_host() -> (PluginHostProcessors, InstanceLogs) {
    let instances = Arc::new(Mutex::new(Vec::new()));
    let mut host = PluginHost::new();
    host.register_factory(Box::new(RecFactory {
        instances: Arc::clone(&instances),
    }));
    (PluginHostProcessors::new(host), instances)
}

/// `(target, event)` writes at absolute `frame_base` positions.
fn writes_at(base: TimeBase, entries: &[(i64, f64)]) -> Vec<(ParameterTargetId, TimedParamEvent)> {
    let target = ParameterTargetId::new(1);
    entries
        .iter()
        .map(|&(pos, v)| {
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

// ---------------------------------------------------------------- tests

/// Rule 1/2/Example 1 — a name-only commit returns
/// `ExecutionEquivalence`: no generation reserved, no host connect,
/// no receipt update, and the live plan's evidence still names the
/// revision it was built from.
#[test]
fn sf03_l4_01_name_only_revision_is_equivalent() {
    let root = scratch("l401");
    let (mut c, media, _processor_id) = plugin_project(&root);
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
    assert!(matches!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Active { generation: 1, .. }
    ));
    assert_eq!(instances.lock().unwrap().len(), 1);

    // Name-only commit: new revision, identical semantic content.
    apply(
        &mut c,
        ProjectOp::SetProjectName {
            name: "renamed".into(),
        },
    );
    let rev2 = c.live_revision();
    assert_ne!(rev1, rev2);

    let prepared = eng.prepare(
        &request(2, rev2),
        c.live(),
        rev2,
        deps_default(c.live(), &media),
    );
    let RoutingPrepare::Equivalent(eq) = prepared.unwrap() else {
        panic!("name-only revision must be ExecutionEquivalence");
    };
    assert_eq!(eq.configured_project_revision, rev2);
    assert_eq!(eq.active_built_from_revision, rev1);
    assert_eq!(eq.plan_generation, 1);
    assert_eq!(
        eq.matching_semantic_build_key.project_revision, rev1,
        "evidence names the live plan's build revision, not the new one"
    );
    assert_eq!(
        eq.matching_semantic_build_key.routing_fingerprint,
        synvelis_project::routing_fingerprint(c.live()),
        "the live key's semantic content matches the committed snapshot"
    );

    // No side effects: no second plugin instance, no new generation,
    // receipt still describes gen-1 active at rev1 — never rev2.
    assert_eq!(instances.lock().unwrap().len(), 1, "no host reconnect");
    let receipt = eng.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Active {
            project_revision,
            generation: 1,
            ..
        } if project_revision == rev1
    ));
    assert_eq!(receipt.published_generation, Some(1));

    // No generation was reserved: the next *semantic* change builds
    // as generation 2, not 3.
    let (lane, source) = lane_and_source(&c);
    add_clip(&mut c, lane, source, 480, 960, 0);
    let rev3 = c.live_revision();
    let p2 = eng
        .prepare(
            &request(3, rev3),
            c.live(),
            rev3,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    assert_eq!(p2.generation, 2, "equivalence consumed no generation");
}

/// Rule 1/Example 2 — undo restores semantically identical content at
/// a *new* revision → `Equivalent`; redo restores the divergent
/// content → `Built`.
#[test]
fn sf03_l4_02_undo_returns_equivalent_redo_rebuilds() {
    let root = scratch("l402");
    let (mut c, media) = minimal_project(&root);
    let mut eng = new_engine();

    let rev1 = c.live_revision();
    let fp1 = synvelis_project::routing_fingerprint(c.live());
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

    // Divergent edit → rev2 builds (not yet published).
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
    assert_eq!(p2.generation, 2);

    // Undo → rev3 whose content is identical to rev1 → Equivalent
    // against the still-live gen-1 plan (gen 2 was never published).
    assert!(c.undo().unwrap());
    let rev3 = c.live_revision();
    assert_ne!(rev3, rev1);
    assert_eq!(
        synvelis_project::routing_fingerprint(c.live()),
        fp1,
        "undo restored rev1's semantic content at a new revision"
    );
    let prepared = eng.prepare(
        &request(3, rev3),
        c.live(),
        rev3,
        deps_default(c.live(), &media),
    );
    let RoutingPrepare::Equivalent(eq) = prepared.unwrap() else {
        panic!("undo-restored content must be ExecutionEquivalent");
    };
    assert_eq!(eq.configured_project_revision, rev3);
    assert_eq!(eq.active_built_from_revision, rev1);
    assert_eq!(eq.plan_generation, 1);

    // Redo → rev4 restores the divergent rev2 content → Built.
    assert!(c.redo().unwrap());
    let rev4 = c.live_revision();
    let p4 = eng
        .prepare(
            &request(4, rev4),
            c.live(),
            rev4,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    assert_eq!(p4.generation, 3, "equivalence reserved no generation");
}

/// Rule 1/Example 3 — re-preparing at the *same* committed revision
/// always builds: it is the explicit rebuild path, never equivalence.
#[test]
fn sf03_l4_03_same_revision_always_builds() {
    let root = scratch("l403");
    let (c, media) = minimal_project(&root);
    let mut eng = new_engine();

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

    let p2 = eng
        .prepare(
            &request(2, rev),
            c.live(),
            rev,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    assert_eq!(p2.generation, 2, "same-revision prepare is a rebuild");
}

/// Rule 3/Examples 4 — every non-revision key field defeats
/// equivalence: engine/source-format generations, build mode, and a
/// wrong device epoch or runtime instance (the latter rejected as
/// `DeviceEpochChanged` before equivalence is even considered).
/// `capability_generation`/`lowering_policy_version` cannot drift
/// without the fingerprint drifting too (bindings are fingerprinted)
/// or without a code change — `semantic_eq` requires *all* fields, so
/// any single-field drift lands here as `Built`/fault regardless.
#[test]
fn sf03_l4_04_non_equivalence_axes_build_or_fault() {
    let root = scratch("l404");
    let (mut c, media) = minimal_project(&root);
    let mut eng = new_engine();

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

    apply(&mut c, ProjectOp::SetProjectName { name: "v2".into() });
    let rev2 = c.live_revision();

    // Engine-geometry generation drift → fresh build.
    let mut deps = deps_default(c.live(), &media);
    deps.engine_config_generation = 2;
    let built = eng
        .prepare(&request(2, rev2), c.live(), rev2, deps)
        .unwrap()
        .expect_built();
    assert!(built.generation > 1);

    // Source-format generation drift → fresh build.
    let mut deps = deps_default(c.live(), &media);
    deps.source_format_generation = 7;
    let built = eng
        .prepare(&request(3, rev2), c.live(), rev2, deps)
        .unwrap()
        .expect_built();
    assert!(built.generation > 2);

    // Build-mode drift (offline intent) → fresh build.
    let offline = RoutingCompileRequest {
        mode: BuildMode::Offline,
        ..request(4, rev2)
    };
    let built = eng
        .prepare(&offline, c.live(), rev2, deps_default(c.live(), &media))
        .unwrap()
        .expect_built();
    assert!(built.generation > 3);

    // Wrong epoch / wrong runtime instance → DeviceEpochChanged,
    // never equivalence — checked_key runs before the equivalence
    // probe.
    let wrong_epoch = RoutingCompileRequest {
        device_clock_epoch: RuntimeClockEpochRef(2),
        ..request(5, rev2)
    };
    assert_eq!(
        eng.prepare(&wrong_epoch, c.live(), rev2, deps_default(c.live(), &media))
            .err()
            .unwrap(),
        RoutingFault::DeviceEpochChanged
    );
    let wrong_instance = RoutingCompileRequest {
        runtime_instance: RuntimeInstanceId(2),
        ..request(6, rev2)
    };
    assert_eq!(
        eng.prepare(
            &wrong_instance,
            c.live(),
            rev2,
            deps_default(c.live(), &media)
        )
        .err()
        .unwrap(),
        RoutingFault::DeviceEpochChanged
    );

    // The live plan is untouched throughout: gen-1 is still the
    // RT-observed active generation and last-good — the epoch faults
    // recorded `Failed` receipts but displaced nothing.
    assert_eq!(eng.rt_status().active_generation, 1);
    let receipt = eng.receipt().unwrap();
    assert_eq!(receipt.last_good_generation, Some(1));
    assert_eq!(receipt.fault, Some(RoutingFault::DeviceEpochChanged));
}

/// Rule 4/Example 5 — `reconcile` suppresses `ProjectRevision` only
/// when the live key is semantically equivalent; real semantic drift
/// still reports `StaleProjectSnapshot`.
#[test]
fn sf03_l4_05_reconcile_revision_only_vs_semantic_drift() {
    let root = scratch("l405");
    let (mut c, media) = minimal_project(&root);
    let mut eng = new_engine();

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

    // Revision-only drift → clean reconcile.
    apply(
        &mut c,
        ProjectOp::SetProjectName {
            name: "moved".into(),
        },
    );
    let rev2 = c.live_revision();
    let clean = eng.reconcile(c.live(), rev2, 1, 1);
    assert!(
        !clean
            .mismatches
            .contains(&RoutingReconcileMismatch::ProjectRevision)
    );
    assert_eq!(clean.fault, None);

    // Semantic drift on top → StaleProjectSnapshot.
    let clip = *c.live().clips.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::SetClipMuted {
            clip_id: clip,
            muted: true,
        },
    );
    let rev3 = c.live_revision();
    let drift = eng.reconcile(c.live(), rev3, 1, 1);
    assert!(
        drift
            .mismatches
            .contains(&RoutingReconcileMismatch::ProjectRevision)
    );
    assert_eq!(drift.fault, Some(RoutingFault::StaleProjectSnapshot));
    assert_eq!(eng.receipt().unwrap().last_good_generation, Some(1));
}

/// Rules 5/6/Examples 6–7 — control vending is one-shot, rejects
/// unknown generations/processors and non-plugin connections, and a
/// vended channel delivers only to its own generation's instance.
#[test]
fn sf03_l4_06_control_vending_one_shot_and_scoped() {
    let root = scratch("l406");
    let (c, media, processor_id) = plugin_project(&root);
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
    assert_eq!(instances.lock().unwrap().len(), 1);

    // Unknown generation / unknown processor / second take → None.
    assert!(eng.take_processor_control(99, processor_id).is_none());
    assert!(
        eng.take_processor_control(1, ProcessorId::new(0xDEAD))
            .is_none()
    );
    let control = eng
        .take_processor_control(1, processor_id)
        .expect("published plugin generation must vend control");
    assert!(
        eng.take_processor_control(1, processor_id).is_none(),
        "control vending is one-shot"
    );

    // The vended control drives ParamDispatch against *this*
    // generation's instance: writes at frames 64..320 land in
    // instance 0's log after two blocks (512 frames).
    let mut dispatch = ParamDispatch::new(control.control, control.node, control.frame_base);
    let target = ParameterTargetId::new(1);
    dispatch.bind(target, ParamId(7), &REC_PARAMS).unwrap();
    dispatch
        .dispatch(&writes_at(control.frame_base, &[(64, 0.25), (320, 0.5)]))
        .unwrap();
    eng.run(2, PumpPolicy::Every(1)).unwrap();
    let log0 = instances.lock().unwrap()[0].lock().unwrap().clone();
    assert_eq!(log0.len(), 2, "gen-1 instance received its writes");
    assert!(log0.iter().all(|d| d.param_id == 7));
    assert!(log0.iter().any(|d| (d.value - 0.25).abs() < 1e-9));
    assert!(log0.iter().any(|d| (d.value - 0.5).abs() < 1e-9));
}

/// Rules 5/6/Example 7 — after a newer generation activates, the old
/// generation's vended control cannot reach the new instance: staged
/// writes through the stale channel produce zero deliveries on the
/// live plugin (wrong-plugin 0), and the retired generation no longer
/// vends.
#[test]
fn sf03_l4_07_stale_control_never_reaches_new_generation() {
    let root = scratch("l407");
    let (mut c, media, processor_id) = plugin_project(&root);
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
    let control1 = eng
        .take_processor_control(1, processor_id)
        .expect("gen-1 control");

    // Semantic change → gen 2 builds, publishes, activates; gen-1's
    // instance is torn down and its connection ledger entry retires.
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
    eng.run(2, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    assert!(matches!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Active { generation: 2, .. }
    ));
    assert_eq!(instances.lock().unwrap().len(), 2, "gen-2 instance created");

    // The retired generation no longer vends; the live one does.
    assert!(eng.take_processor_control(1, processor_id).is_none());
    let control2 = eng
        .take_processor_control(2, processor_id)
        .expect("gen-2 control");

    // Gen-2 writes at frame 1280 (cursor is at 768 after three
    // blocks), value 0.5 — delivered on the next blocks.
    let mut dispatch2 = ParamDispatch::new(control2.control, control2.node, control2.frame_base);
    dispatch2
        .bind(ParameterTargetId::new(1), ParamId(7), &REC_PARAMS)
        .unwrap();
    dispatch2
        .dispatch(&writes_at(control2.frame_base, &[(1280, 0.5)]))
        .unwrap();

    // The stale gen-1 control still exists (caller-owned) — stage a
    // write through it. Its node is destroyed: the push either
    // overflows the dead channel or buffers undelivered; either way
    // it must never reach instance 2.
    let mut dispatch1 = ParamDispatch::new(control1.control, control1.node, control1.frame_base);
    dispatch1
        .bind(ParameterTargetId::new(1), ParamId(7), &REC_PARAMS)
        .unwrap();
    let _ = dispatch1.dispatch(&writes_at(control1.frame_base, &[(1400, 0.9)]));

    eng.run(8, PumpPolicy::Every(1)).unwrap();
    eng.collect();

    let instances = instances.lock().unwrap();
    let log1 = instances[0].lock().unwrap().clone();
    let log2 = instances[1].lock().unwrap().clone();
    assert!(
        log2.iter().all(|d| (d.value - 0.5).abs() < 1e-9),
        "gen-2 instance saw only its own writes: {log2:?}"
    );
    assert_eq!(log2.len(), 1, "gen-2 instance received exactly its write");
    assert!(
        log1.iter().all(|d| (d.value - 0.9).abs() > 1e-9),
        "dead gen-1 channel delivered nowhere: {log1:?}"
    );
}

/// Rule 5 — `StaticProcessorHost` connections carry no plugin control
/// half: vending returns `None` even for a published generation.
#[test]
fn sf03_l4_08_static_host_vends_no_control() {
    let root = scratch("l408");
    let (mut c, media) = minimal_project(&root);
    let track = *c.live().tracks.keys().next_back().unwrap();
    let strip = strip_of_track(c.live(), track);
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "gain".into(),
            binding: resolved_binding("test.unused"),
            sidechain_input: false,
        },
    );
    let processor_id = *c.live().routing.processors.keys().next_back().unwrap();

    let mut eng = new_engine();
    let mut host = StaticProcessorHost::new();
    host.insert(processor_id, Box::new(GainProcessor::new(0.5)));
    eng.set_processor_host(Box::new(host));

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

    assert!(
        eng.take_processor_control(1, processor_id).is_none(),
        "static (non-plugin) connections vend no control"
    );
}

/// Rule 8/Examples 8–9 — SF-ACT-05 headless: rapid R1/R2/R3 publishes
/// record `Published` receipts without activation, the newest pending
/// generation wins, `Superseded` rejects an older prepared plan,
/// `PublishQueueFull` preserves last-good, and stale install counters
/// surface as `EventStale` on reconcile.
#[test]
fn sf03_l4_09_rapid_publishes_superseded_and_queue_full() {
    let root = scratch("l409");
    let (mut c, media) = minimal_project(&root);
    let mut eng = new_engine();

    // R1/R2/R3: three semantic revisions prepared and published
    // back-to-back with no RT run — each displaces the still-pending
    // predecessor.
    let mut revisions = Vec::new();
    for id in 1..=3_u64 {
        let rev = c.live_revision();
        revisions.push(rev);
        let prepared = eng
            .prepare(
                &request(id, rev),
                c.live(),
                rev,
                deps_default(c.live(), &media),
            )
            .unwrap()
            .expect_built();
        let receipt = eng.publish(prepared, c.live(), rev).unwrap();
        assert!(matches!(
            receipt.state,
            RoutingActivationState::Published { generation, .. } if generation == id
        ));
        assert_eq!(receipt.last_good_generation, None, "never activated yet");
        if id < 3 {
            let (lane, source) = lane_and_source(&c);
            add_clip(&mut c, lane, source, id * 480, id * 480 + 480, 0);
        }
    }

    // RT observes only the newest generation; last-good catches up.
    eng.run(2, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    let receipt = eng.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Active { generation: 3, .. }
    ));
    assert_eq!(receipt.last_good_generation, Some(3));

    // Superseded: two same-revision prepares mint generations 4 and
    // 5; publishing the older one after the newer is rejected.
    let rev = c.live_revision();
    let p_old = eng
        .prepare(
            &request(10, rev),
            c.live(),
            rev,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    let p_new = eng
        .prepare(
            &request(11, rev),
            c.live(),
            rev,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(p_new, c.live(), rev).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    assert_eq!(
        eng.publish(p_old, c.live(), rev).err().unwrap(),
        RoutingFault::Superseded
    );

    // Queue-full: repeated publishes of fresh same-revision builds
    // eventually overflow the bounded install channel; last-good
    // (now the gen-5 plan once active) is preserved.
    let mut last_err = None;
    for id in 20..400_u64 {
        let prepared = eng
            .prepare(
                &request(id, rev),
                c.live(),
                rev,
                deps_default(c.live(), &media),
            )
            .unwrap()
            .expect_built();
        match eng.publish(prepared, c.live(), rev) {
            Ok(_) => {}
            Err(fault) => {
                last_err = Some(fault);
                break;
            }
        }
    }
    assert_eq!(last_err, Some(RoutingFault::PublishQueueFull));
    assert!(eng.receipt().unwrap().last_good_generation.is_some());

    // Stale install counters from the displaced pending generations
    // reconcile as EventStale — the published plan stays live.
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    eng.collect();
    let status = eng.rt_status();
    assert!(status.stale_commands > 0 || status.stale_events > 0);
    let report = eng.reconcile(c.live(), rev, 1, 1);
    assert!(
        report
            .mismatches
            .contains(&RoutingReconcileMismatch::RuntimeCounters)
    );
    assert_eq!(report.fault, Some(RoutingFault::EventStale));
}

/// Rule 8 — `DeviceEpochChanged` rejects a plan built for the wrong
/// runtime epoch/instance, and the fresh retry on the correct epoch
/// builds, publishes and activates normally.
#[test]
fn sf03_l4_10_epoch_fault_then_fresh_retry() {
    let root = scratch("l410");
    let (c, media) = minimal_project(&root);
    let mut eng = new_engine();

    let rev = c.live_revision();
    let wrong_epoch = RoutingCompileRequest {
        device_clock_epoch: RuntimeClockEpochRef(9),
        ..request(1, rev)
    };
    assert_eq!(
        eng.prepare(&wrong_epoch, c.live(), rev, deps_default(c.live(), &media))
            .err()
            .unwrap(),
        RoutingFault::DeviceEpochChanged
    );

    // Fresh retry against the committed snapshot on the right epoch.
    let prepared = eng
        .prepare(
            &request(2, rev),
            c.live(),
            rev,
            deps_default(c.live(), &media),
        )
        .unwrap()
        .expect_built();
    eng.publish(prepared, c.live(), rev).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    assert!(matches!(
        eng.receipt().unwrap().state,
        RoutingActivationState::Active { generation: 1, .. }
    ));
}
