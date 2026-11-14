//! SF-04 SF-PROBE-07 loop leg (Issue #107, slice
//! `sf-04-loop-discontinuity`): loop-wrap and `SetLoop`/`ClearLoop`
//! apply detection at the `RoutingControlDispatch::observe_block`
//! boundary, and the probe clock-epoch declaration the app run loop
//! performs for each detected discontinuity.
//!
//! `observe_block` never declares — it only reports
//! [`RoutingControlDispatch::observed_loop_discontinuity`] for the
//! window it just projected (detection stays in the player layer;
//! declaration is the composition root's job). The fixture mirrors
//! `SessionRoutingEngine::run`'s contract by hand — player tests
//! cannot reach the app crate — by declaring through
//! [`RoutingEngine::declare_probe_discontinuity`] after each
//! discontinuity-reporting block, so post-wrap evidence carries the
//! new epoch and comparisons spanning the boundary fail closed.
//!
//! Covered: ordinary playing windows never declare; every wrap
//! window declares (epoch cut per block, cursor untouched, compare
//! `DiscontinuityInWindow`); `SetLoop`/`ClearLoop` applies declare
//! even when the window derives no wrap; an open capture segment
//! cuts at `ClockEpoch` on the declared boundary; a failed
//! declaration (`CommandChannelFull`) still advances the broker
//! epoch so broker/deck divergence stays observable; and
//! generation-mismatched `observe_block` calls fail closed without
//! leaking a stale flag.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_audio::typed::probe::{ProbeError, ProbeEvent};
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
use synvelis_player::lowering::EngineConfig;
use synvelis_player::resolver::{PreparedClip, prepare};
use synvelis_player::routing_control::ParamCatalog;
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::{
    BoundaryDeclaration, ProbeTraceBridge, RoutingControlDispatch, TimingPrecision,
};
use synvelis_project::probe::{
    AcousticObservation, CaptureSegmentEnd, ProbeAlignPolicy, ProbeComparisonBasis,
    ProbeComparisonQuality, ProbeComparisonRequest, ProbeIncomparableReason, ProbeMetricKind,
    ProbeMode, ProbeResolveState, SemanticProbePoint, SemanticSignalEntityRef, SignalBoundary,
    SubscriptionKind,
};
use synvelis_project::{
    BuildBudget, BuildMode, ClipExtent, ClipPlacement, ClipPlayback, Command, CounterIdGenerator,
    LaneId, MediaTimeValue, PlaybackMapping, ProbeSubscriptionKey, ProbeTracePort,
    ProcessorBinding, ProcessorId, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, ProjectSessionId, RoutingBuildRequestId, RoutingCompileRequest,
    RuntimeClockEpochRef, RuntimeInstanceId, SourceFrame, SourceId, SourceRange, StripId,
    TimeAnchor, TrackId,
};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf04loop-{}-{}-{}",
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
        Project::new(ProjectId::new(0x107), "sf04-loop"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply(c: &mut ProjectController, op: ProjectOp) {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .unwrap();
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

fn engine() -> EngineConfig {
    EngineConfig {
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

fn import_stereo(c: &mut ProjectController, root: &Path, name: &str, frames: usize) -> SourceId {
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
    .commit
    .source_id
}

/// Track + strip (no lane) — the strip-level probe/compare partner.
fn add_track_strip(c: &mut ProjectController, name: &str) -> (TrackId, StripId) {
    apply(c, ProjectOp::AddTrack { name: name.into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    (track, strip)
}

fn add_clip(c: &mut ProjectController, lane: LaneId, source: SourceId) {
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: media_placement(0, 480, 48_000),
            playback: natural_playback(0, 480),
        },
    );
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

/// Builds + publishes generation `id` and runs one block so RT
/// observes the activation. The block covers node frames `[0, 256)`
/// with no observer attached — the first *observed* window starts at
/// node cursor 256, the same convention `tim03_loop_boundary.rs`
/// documents. Returns the published generation.
fn publish_and_run(eng: &mut RoutingEngine, c: &ProjectController, media: &Path, id: u64) -> u64 {
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
    let generation = prepared.compiled.plan.generation;
    eng.publish(prepared, c.live(), rev).unwrap();
    eng.run(1, PumpPolicy::Every(1)).unwrap();
    generation
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
    id: "org.synvelis.test.sf04-loop-rec",
    name: "SF-04 Loop Recorder",
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

/// Minimal pass-through plugin — the control node only needs to
/// exist so `observe_block` projects its transport window; position
/// evidence is TIM-03's concern, not this slice's.
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
        I: BufferView,
        O: BufferViewMut,
    {
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

/// The caller-vended catalog the surface binds against.
fn catalog(processors: &[ProcessorId]) -> ParamCatalog {
    processors
        .iter()
        .map(|p| (*p, REC_PARAMS.to_vec()))
        .collect::<BTreeMap<_, _>>()
}

fn loop_cmd(start: i64, end: i64) -> TransportCommand {
    TransportCommand::SetLoop(MediaRange::new(
        MediaTime::new(start, frame_base()),
        MediaDuration::new(end - start, frame_base()),
    ))
}

// ---------------------------------------------------------------- fixture

/// Playing, boundary-declared plugin surface plus a probe bridge —
/// the player-layer stand-in for `SessionRoutingEngine`'s run loop.
struct LoopFixture {
    bridge: ProbeTraceBridge,
    c: ProjectController,
    dispatch: RoutingControlDispatch,
    processor: ProcessorId,
    strip_a: StripId,
    strip_b: StripId,
    generation: u64,
    /// Node-cursor frame the next `observe_block` window starts at.
    head: u64,
}

/// Strip A carries the clip + plugin processor (the observed
/// transport); strip B is a plain strip — the second probe point
/// comparisons need. Published, host-attached, surface attached;
/// transport is *not* playing yet — stage `Play` first.
fn loop_fixture(root: &Path) -> LoopFixture {
    let mut c = controller();
    let source = import_stereo(&mut c, root, "a.wav", 4_800);
    let (track_a, strip_a) = add_track_strip(&mut c, "a");
    apply(&mut c, ProjectOp::AddLane { track_id: track_a });
    let lane = *c.live().tracks[&track_a].lane_ids.last().unwrap();
    add_clip(&mut c, lane, source);
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: strip_a,
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
    let processor = *c.live().routing.processors.keys().next_back().unwrap();
    let (_track_b, strip_b) = add_track_strip(&mut c, "b");
    let media = root.join("media");

    let mut adapter = rec_host();
    adapter.register_boundary(
        processor,
        BoundaryDeclaration {
            precision: TimingPrecision::FrameAccurateEvents,
            in_block: InBlockTransportSupport::Unknown,
            consent: None,
            budget: ScheduleBudget::new(64, 256, 16, 4096),
        },
    );

    let mut bridge = ProbeTraceBridge::new(new_engine(), BTreeMap::new());
    bridge.engine_mut().set_processor_host(Box::new(adapter));
    let generation = publish_and_run(bridge.engine_mut(), &c, &media, 1);
    assert_eq!(
        active_generation(bridge.engine()),
        Some(generation),
        "published generation must be RT-observed"
    );
    let dispatch = RoutingControlDispatch::attach(
        bridge.engine_mut(),
        generation,
        c.live(),
        &catalog(&[processor]),
    )
    .expect("control surface attaches");
    LoopFixture {
        bridge,
        c,
        dispatch,
        processor,
        strip_a,
        strip_b,
        generation,
        head: 256,
    }
}

impl LoopFixture {
    /// Stages transport commands due `due` frames after the next
    /// observed head — the same supply `boundary_stage_window`
    /// callers use in TIM-03.
    fn stage(&mut self, commands: &[(TransportCommand, u64)]) {
        self.dispatch
            .boundary_stage_window(
                self.processor,
                self.c.live(),
                256,
                vec![],
                commands,
                Some(self.generation),
            )
            .expect("commands staged");
    }

    /// Runs one 256-frame block and observes its node-cursor window,
    /// returning the discontinuity flag — the exact read
    /// `SessionRoutingEngine::run` performs before declaring. Does
    /// **not** declare.
    fn run_block_detect(&mut self) -> bool {
        let head = self.head;
        self.bridge
            .engine_mut()
            .run(1, PumpPolicy::Every(1))
            .expect("block runs");
        self.dispatch
            .observe_block(self.generation, ef(head), 256)
            .unwrap_or_else(|| panic!("gen-{} surface observed", self.generation));
        let discontinuity = self.dispatch.observed_loop_discontinuity();
        self.head += 256;
        discontinuity
    }

    /// `SessionRoutingEngine::run`'s contract: detect, then declare
    /// the probe epoch cut before the next block's records.
    fn run_block(&mut self) {
        if self.run_block_detect() {
            self.bridge
                .engine_mut()
                .declare_probe_discontinuity()
                .expect("declaration lands");
        }
    }
}

// ---------------------------------------------------------------- probes

fn session_key(session: u64, strip: StripId) -> ProbeSubscriptionKey {
    ProbeSubscriptionKey {
        session_id: ProjectSessionId(session),
        point: SemanticProbePoint {
            entity: SemanticSignalEntityRef::Strip(strip),
            boundary: SignalBoundary::Output,
            port: None,
        },
        kind: SubscriptionKind::Metrics,
    }
}

fn point(strip: StripId) -> SemanticProbePoint {
    SemanticProbePoint {
        entity: SemanticSignalEntityRef::Strip(strip),
        boundary: SignalBoundary::Output,
        port: None,
    }
}

fn compare_request(
    before: SemanticProbePoint,
    after: SemanticProbePoint,
) -> ProbeComparisonRequest {
    ProbeComparisonRequest {
        before,
        after,
        observed_range: None,
        alignment_policy: ProbeAlignPolicy::default(),
        metric_set: vec![ProbeMetricKind::PeakRms],
        max_bytes: 1 << 20,
    }
}

fn incomparable_reason(obs: &AcousticObservation) -> &ProbeIncomparableReason {
    match &obs.quality {
        ProbeComparisonQuality::Incomparable { reason } => reason,
        other => panic!("expected Incomparable, got {other:?}"),
    }
}

fn subscribe(f: &mut LoopFixture, session: u64, strip: StripId) {
    f.bridge
        .probe_request(f.c.live(), session_key(session, strip), ProbeMode::Monitor)
        .expect("probe request accepted");
}

fn drain(f: &mut LoopFixture) {
    let _ = f.bridge.probe_drain();
}

// ---------------------------------------------------------------- tests

/// SF-PROBE-07 loop leg, wrap path: ordinary playing windows never
/// declare; every loop-wrap window declares a probe epoch cut that
/// leaves the engine cursor untouched; and a comparison whose window
/// spans the declared boundary fails closed instead of aligning.
#[test]
fn sf04_loop_wrap_windows_declare_and_close_compare() {
    let root = scratch("wrap");
    let mut f = loop_fixture(&root);

    let (strip_a, strip_b) = (f.strip_a, f.strip_b);
    subscribe(&mut f, 1, strip_a);
    subscribe(&mut f, 2, strip_b);
    f.stage(&[(TransportCommand::Play, 0)]);

    // Ordinary playing progression — no loop installed — must never
    // report a discontinuity (no false declarations).
    for _ in 0..6 {
        assert!(
            !f.run_block_detect(),
            "ordinary playing window reports no discontinuity"
        );
    }
    drain(&mut f);
    let epoch0 = f.bridge.engine().probe_time_context().epoch;
    assert_eq!(epoch0, ClockEpochId::new(0), "no declaration yet");

    // Baseline: both strips monitored on the same cursor series
    // align — the incomparable verdict below is caused by the
    // declared wrap discontinuities, not missing evidence.
    let before = point(f.strip_a);
    let after = point(f.strip_b);
    let baseline = f.bridge.compare_probes(&compare_request(before, after));
    assert!(
        matches!(
            baseline.quality,
            ProbeComparisonQuality::Aligned {
                basis: ProbeComparisonBasis::SharedCursor { .. },
                ..
            }
        ),
        "pre-wrap baseline must align, got {:?}",
        baseline.quality
    );

    // Install loop [0,100): the playing position is already past the
    // loop end at the next head, so the window reports a head wrap
    // plus interior wraps — and every following window keeps wrapping
    // while the loop cycles.
    f.stage(&[(loop_cmd(0, 100), 0)]);

    let mut last_epoch = epoch0;
    for block in 0..3 {
        let discontinuity = f.run_block_detect();
        assert!(
            discontinuity,
            "wrap window {block} must report a loop discontinuity"
        );
        // The declaration the app loop performs: cursor untouched,
        // epoch cut before the next block's records.
        let cursor = f.bridge.engine().cursor();
        f.bridge
            .engine_mut()
            .declare_probe_discontinuity()
            .expect("wrap declaration lands");
        assert_eq!(
            f.bridge.engine().cursor(),
            cursor,
            "declare must not move the engine cursor"
        );
        let epoch = f.bridge.engine().probe_time_context().epoch;
        assert_ne!(
            epoch, last_epoch,
            "each wrap block cuts the probe clock epoch"
        );
        last_epoch = epoch;
        drain(&mut f);
    }

    // The comparison window spans pre-wrap and post-wrap clock marks
    // — it must never align across the declared discontinuity.
    let obs = f.bridge.compare_probes(&compare_request(before, after));
    assert_eq!(
        incomparable_reason(&obs),
        &ProbeIncomparableReason::DiscontinuityInWindow,
        "a window spanning loop-wrap declarations must never align"
    );
}

/// `SetLoop`/`ClearLoop` applies are discontinuities even when the
/// window derives no wrap: the region change alters the position
/// domain going forward. An open capture segment cuts at `ClockEpoch`
/// on the first post-declaration evidence — never re-captured.
#[test]
fn sf04_loop_set_and_clear_apply_declare_without_wrap() {
    let root = scratch("apply");
    let mut f = loop_fixture(&root);

    let strip_a = f.strip_a;
    let key = session_key(1, strip_a);
    subscribe(&mut f, 1, strip_a);
    f.stage(&[(TransportCommand::Play, 0)]);
    f.run_block();
    f.run_block();
    drain(&mut f);
    assert_eq!(
        f.bridge.engine().probe_bindings()[&key].state,
        ProbeResolveState::BoundActive,
        "monitor probe bound before capture"
    );
    f.bridge
        .engine_mut()
        .probe_capture(key, 65_536)
        .expect("capture opens");
    f.run_block();
    drain(&mut f);
    assert!(
        f.bridge.engine().probe_segments().is_empty(),
        "the capture segment is still open"
    );

    let epoch0 = f.bridge.engine().probe_time_context().epoch;

    // SetLoop far ahead of the playing position: it applies inside
    // the next window but derives no wrap — the apply alone is the
    // discontinuity.
    f.stage(&[(loop_cmd(2_000, 2_100), 0)]);
    let head = f.head;
    let derived = f
        .dispatch
        .boundary_loop_wraps(f.processor, f.c.live(), ef(head), 256)
        .expect("wrap derivation");
    assert!(
        derived.is_empty(),
        "an ahead loop derives no wraps: {derived:?}"
    );
    assert!(
        f.run_block_detect(),
        "SetLoop apply alone reports a discontinuity"
    );
    f.bridge
        .engine_mut()
        .declare_probe_discontinuity()
        .expect("apply declaration lands");
    let epoch1 = f.bridge.engine().probe_time_context().epoch;
    assert_ne!(epoch1, epoch0, "the apply cut the probe epoch");

    // The epoch cut closes the open segment on the first
    // new-epoch evidence — same lazy-close rule as seek/declare.
    f.run_block();
    drain(&mut f);
    let segments = f.bridge.engine().probe_segments();
    assert_eq!(segments.len(), 1, "the epoch cut closes the segment");
    assert_eq!(segments[0].ended_by, Some(CaptureSegmentEnd::ClockEpoch));
    assert_eq!(
        segments[0].clock_context.map(|clock| clock.epoch),
        Some(epoch0),
        "the closed segment keeps its pre-apply epoch"
    );

    // ClearLoop applies the same way — a discontinuity with no wrap.
    f.stage(&[(TransportCommand::ClearLoop, 0)]);
    let head = f.head;
    let derived = f
        .dispatch
        .boundary_loop_wraps(f.processor, f.c.live(), ef(head), 256)
        .expect("wrap derivation");
    assert!(derived.is_empty(), "no wraps remain: {derived:?}");
    assert!(
        f.run_block_detect(),
        "ClearLoop apply reports a discontinuity"
    );
    f.bridge
        .engine_mut()
        .declare_probe_discontinuity()
        .expect("clear declaration lands");
    let epoch2 = f.bridge.engine().probe_time_context().epoch;
    assert_ne!(epoch2, epoch1, "the clear cut the probe epoch again");

    // With the loop gone and no applies pending, ordinary playback
    // reports nothing — the flag cannot stick.
    assert!(
        !f.run_block_detect(),
        "post-clear ordinary window reports no discontinuity"
    );
}

/// A declaration that fails on a full probe command channel is still
/// fail-closed: the broker epoch advances before the wire push, so
/// `probe_time_context` and the deck's clock mark observably diverge
/// until a later successful declaration resyncs — the same contract
/// `seek`'s best-effort bump documents.
#[test]
fn sf04_loop_declare_channel_full_keeps_observable_divergence() {
    let root = scratch("full");
    let mut f = loop_fixture(&root);

    // Bound probes so command traffic exists, then establish the
    // playing surface far enough to wrap.
    let strip_b = f.strip_b;
    for session in 1..=4_u64 {
        subscribe(&mut f, session, strip_b);
    }
    f.stage(&[(TransportCommand::Play, 0)]);
    for _ in 0..4 {
        f.run_block();
    }
    drain(&mut f);

    // Install the wrap loop and run into a wrap window — detection
    // fires, but do not declare yet.
    f.stage(&[(loop_cmd(0, 100), 0)]);
    assert!(
        f.run_block_detect(),
        "wrap window detected before the failed declaration"
    );

    // Fill the 64-slot probe command channel *after* the block's
    // process drained it — redundant SetMode pushes on bound probes
    // are the filler (`set_mode` sends unconditionally).
    let any_key = session_key(1, f.strip_b);
    for _ in 0..64 {
        if f.bridge
            .probe_set_mode(any_key, ProbeMode::Monitor)
            .is_err()
        {
            break;
        }
    }

    let epoch0 = f.bridge.engine().probe_time_context().epoch;
    let result = f.bridge.engine_mut().declare_probe_discontinuity();
    assert!(
        matches!(
            result,
            Err(synvelis_player::ProbeRequestError::Broker(
                ProbeError::CommandChannelFull
            ))
        ),
        "channel-full declaration reports honestly, got {result:?}"
    );
    assert_ne!(
        f.bridge.engine().probe_time_context().epoch,
        epoch0,
        "the broker epoch still advances on the failed wire"
    );

    // The queued commands drain on the next process; the lost
    // `SetClockMark` means the deck keeps stamping the old epoch —
    // records honestly claim what the deck knew.
    f.run_block_detect();
    let mut record_epochs = Vec::new();
    f.bridge.engine_mut().probe_drain(|event| {
        if let ProbeEvent::Metric(record) = event {
            record_epochs.push(record.meta.clock.epoch);
        }
    });
    assert!(
        !record_epochs.is_empty(),
        "bound probes emit metrics after the drain"
    );
    assert!(
        record_epochs.iter().all(|epoch| *epoch == epoch0),
        "the deck never received the new mark — records keep the old epoch"
    );
    assert_ne!(
        f.bridge.engine().probe_time_context().epoch,
        epoch0,
        "context and record marks disagree — observable, fail-closed"
    );

    // A later successful declaration re-wires the mark and resyncs
    // the deck — the divergence is transient, never permanent.
    f.run_block_detect();
    f.bridge
        .engine_mut()
        .declare_probe_discontinuity()
        .expect("post-drain declaration lands");
    f.run_block_detect();
    let mut resynced = Vec::new();
    f.bridge.engine_mut().probe_drain(|event| {
        if let ProbeEvent::Metric(record) = event {
            resynced.push(record.meta.clock.epoch);
        }
    });
    assert!(
        resynced.iter().any(|epoch| *epoch != epoch0),
        "the next successful wire delivers the pending epoch to the deck"
    );
}

/// `observe_block` fails closed on a generation the surface does not
/// own: `None`, and the discontinuity flag is cleared rather than
/// leaking a stale `true` into the next window.
#[test]
fn sf04_loop_observe_block_stale_generation_clears_flag() {
    let root = scratch("stale");
    let mut f = loop_fixture(&root);

    assert!(
        !f.dispatch.observed_loop_discontinuity(),
        "the flag starts clear"
    );

    // A foreign generation observes nothing and cannot set the flag.
    assert!(
        f.dispatch
            .observe_block(f.generation + 1, ef(f.head), 256)
            .is_none(),
        "stale generation refuses observation"
    );
    assert!(!f.dispatch.observed_loop_discontinuity());

    // Drive into a real wrap window so the flag latches `true`.
    f.stage(&[(TransportCommand::Play, 0)]);
    for _ in 0..2 {
        f.run_block();
    }
    f.stage(&[(loop_cmd(0, 100), 0)]);
    assert!(f.run_block_detect(), "wrap window latches the flag");

    // A stale call between windows clears the flag — the next real
    // observation recomputes it, never inherits it.
    assert!(
        f.dispatch
            .observe_block(f.generation + 1, ef(f.head), 256)
            .is_none()
    );
    assert!(
        !f.dispatch.observed_loop_discontinuity(),
        "stale observation clears a latched flag"
    );
}
