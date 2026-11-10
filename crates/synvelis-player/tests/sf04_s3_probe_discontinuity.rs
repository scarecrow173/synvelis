//! SF-04 slice 3 (Issue #107): cursor clock-epoch discontinuity on
//! transport seek — epoch evidence on drained records, capture
//! segment cut at `ClockEpoch` (never spanning, never auto-recaptured),
//! probe modes leaving master output bit-identical (SF-PROBE-02), and
//! bounded fault injection — probe-table capacity and a mid-rebind
//! command-channel-full that must stay atomic and recover on the next
//! publish (SF-PROBE-04/05 headless evidence).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_player::ProbeRequestError;
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::lowering::EngineConfig;
use synvelis_player::resolver::prepare;
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_project::probe::{
    CaptureCompleteness, CaptureSegmentEnd, ProbeMode, ProbeResolveState, SemanticProbePoint,
    SemanticSignalEntityRef, SignalBoundary, SubscriptionKind,
};
use synvelis_project::{
    BuildBudget, BuildMode, ClipExtent, ClipPlacement, ClipPlayback, Command, CounterIdGenerator,
    LaneId, MediaDuration, MediaTimeValue, PlaybackMapping, ProbeSubscriptionKey, Project,
    ProjectController, ProjectId, ProjectOp, ProjectRevision, ProjectSessionId,
    RoutingBuildRequestId, RoutingCompileRequest, RuntimeClockEpochRef, RuntimeInstanceId,
    SourceFrame, SourceId, SourceRange, StripId, TimeAnchor, TrackId,
};

use ruviform_audio::typed::probe::{ProbeError, ProbeEvent};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_reader::PrefetchConfig;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf04s3-{}-{}-{}",
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
        Project::new(ProjectId::new(0x107), "sf04s3"),
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

fn add_track_strip(c: &mut ProjectController) -> (TrackId, LaneId, StripId) {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    (track, lane, strip)
}

fn add_clip(c: &mut ProjectController, lane: LaneId, source: SourceId) {
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: media_placement(0, 2_048, 48_000),
            playback: natural_playback(0, 2_048),
        },
    );
}

fn import_stereo(c: &mut ProjectController, root: &Path, name: &str, frames: usize) -> SourceId {
    let wav = root.join(name);
    write_fixture(&wav, 2, 48_000, frames).unwrap();
    let out = import_local(
        c,
        &ImportRequest {
            path: wav,
            staging_root: root.join("staging"),
            media_root: root.join("media"),
            resume: None,
        },
    )
    .unwrap();
    out.commit.source_id
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

/// Clip-bearing project: track strip + lane + one 2048-frame clip.
fn clip_project(c: &mut ProjectController, root: &Path) -> (TrackId, StripId) {
    let source = import_stereo(c, root, "a.wav", 4_800);
    let (track, lane, strip) = add_track_strip(c);
    add_clip(c, lane, source);
    (track, strip)
}

/// prepare + publish the live project — returns the published
/// generation.
fn publish(
    engine: &mut RoutingEngine,
    c: &ProjectController,
    request_id: u64,
    media: &Path,
) -> u64 {
    let revision = c.live_revision();
    let canonical = |_: SourceId| Some(canonical_format());
    let deps = RoutingDeps {
        canonical_of: &canonical,
        clips: c
            .live()
            .clips
            .values()
            .map(|clip| prepare(c.live(), clip.source_id, media, &prefetch()).unwrap())
            .collect(),
        engine_config_generation: 1,
        source_format_generation: 1,
    };
    let prepared = engine
        .prepare(&request(request_id, revision), c.live(), revision, deps)
        .unwrap()
        .expect_built();
    let generation = prepared.compiled.plan.generation;
    engine.publish(prepared, c.live(), revision).unwrap();
    generation
}

fn drain(engine: &mut RoutingEngine) -> usize {
    let mut events = 0usize;
    engine.probe_drain(|_| events += 1);
    events
}

fn strip_key(strip: StripId, boundary: SignalBoundary) -> ProbeSubscriptionKey {
    ProbeSubscriptionKey {
        session_id: ProjectSessionId(1),
        point: SemanticProbePoint {
            entity: SemanticSignalEntityRef::Strip(strip),
            boundary,
            port: None,
        },
        kind: SubscriptionKind::Metrics,
    }
}

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

fn binding<'a>(
    engine: &'a RoutingEngine,
    key: &ProbeSubscriptionKey,
) -> &'a synvelis_project::SemanticProbeBinding {
    &engine.probe_bindings()[key]
}

// ── clock epoch on transport discontinuity ──────────────────────

#[test]
fn sf04_s3_seek_bumps_cursor_clock_epoch_on_records() {
    let root = scratch("seek-epoch");
    let media = root.join("media");
    let mut c = controller();
    let (_track, strip) = clip_project(&mut c, &root);
    let mut engine = new_engine();
    publish(&mut engine, &c, 1, &media);

    let key = strip_key(strip, SignalBoundary::Output);
    engine
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);
    assert_eq!(binding(&engine, &key).state, ProbeResolveState::BoundActive);

    // Pre-seek metric records carry the initial epoch.
    let epoch0 = engine.probe_time_context().epoch;
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    let mut pre_run = Vec::new();
    engine.probe_drain(|e| {
        if let ProbeEvent::Metric(m) = e {
            pre_run.push((m.meta.sample_position, m.meta.clock));
        }
    });
    assert!(!pre_run.is_empty(), "monitor metrics flow");
    assert!(
        pre_run
            .iter()
            .all(|(_, mark)| mark.epoch == epoch0 && !mark.is_unknown()),
        "pre-seek records share the initial epoch"
    );

    // Seek is a cursor-series discontinuity: the epoch advances and
    // post-seek records carry it — positions stay monotonic (the RT
    // frame cursor counts processed frames), so epoch is the only
    // evidence separating the spans.
    engine.seek(1_024);
    let epoch1 = engine.probe_time_context().epoch;
    assert_ne!(epoch1, epoch0, "seek bumps the probe clock epoch");

    engine.run(2, PumpPolicy::Every(1)).unwrap();
    let mut post = Vec::new();
    engine.probe_drain(|e| {
        if let ProbeEvent::Metric(m) = e {
            post.push((m.meta.sample_position, m.meta.clock));
        }
    });
    assert!(
        post.iter().any(|(_, mark)| mark.epoch == epoch1),
        "post-seek records carry the new epoch"
    );
    assert!(
        post.iter()
            .all(|(_, mark)| mark.epoch == epoch0 || mark.epoch == epoch1),
        "no third epoch appears"
    );
    // Positions remain monotonic across the boundary — the epoch,
    // not the position, marks the discontinuity.
    let positions: Vec<u64> = post.iter().map(|(p, _)| *p).collect();
    assert!(
        positions.windows(2).all(|w| w[0] < w[1]),
        "cursor positions stay monotonic across seek"
    );
}

#[test]
fn sf04_s3_capture_segment_cuts_at_clock_epoch() {
    let root = scratch("epoch-cut");
    let media = root.join("media");
    let mut c = controller();
    let (_track, strip) = clip_project(&mut c, &root);
    let mut engine = new_engine();
    publish(&mut engine, &c, 1, &media);

    let key = strip_key(strip, SignalBoundary::Output);
    engine
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);
    assert_eq!(binding(&engine, &key).state, ProbeResolveState::BoundActive);

    let epoch0 = engine.probe_time_context().epoch;
    engine.probe_capture(key, 8_192).unwrap();
    engine.run(4, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);

    // Seek mid-capture: the epoch bump lands on the next process;
    // the first new-epoch chunk cuts the segment — pre-seek chunks
    // already drained keep their old epoch inside it.
    engine.seek(1_024);
    let epoch1 = engine.probe_time_context().epoch;
    assert_ne!(epoch1, epoch0);
    engine.run(4, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);

    let first_session = {
        let segments = engine.probe_segments();
        assert_eq!(segments.len(), 1, "one segment, cut at the epoch");
        let segment = &segments[0];
        assert_eq!(segment.ended_by, Some(CaptureSegmentEnd::ClockEpoch));
        assert_eq!(
            segment.clock_context.map(|clock| clock.epoch),
            Some(epoch0),
            "the closed segment keeps its old-epoch clock context"
        );
        assert_eq!(segment.completeness, CaptureCompleteness::Complete);
        assert!(segment.frame_range.is_some(), "pre-seek PCM kept");
        segment.capture_session_id
    };

    // Post-cut chunks carry the new epoch and are discarded — the RT
    // capture runs on to its budget but never re-attributes data and
    // never starts a new segment implicitly.
    engine.run(4, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);
    assert_eq!(engine.probe_segments().len(), 1, "no auto-recapture");

    // An explicit new capture is a new session under the new epoch.
    engine.probe_capture(key, 2_048).unwrap();
    engine.run(9, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);

    let segments = engine.probe_segments();
    assert_eq!(segments.len(), 2);
    let second = &segments[1];
    assert_eq!(second.ended_by, Some(CaptureSegmentEnd::BudgetExhausted));
    assert_eq!(
        second.clock_context.map(|clock| clock.epoch),
        Some(epoch1),
        "the new segment records the post-seek epoch"
    );
    assert_ne!(
        second.capture_session_id, first_session,
        "explicit recapture mints a new capture session"
    );
}

// ── probe modes never alter DSP output (SF-PROBE-02) ────────────

#[test]
fn sf04_s3_probe_modes_leave_master_output_identical() {
    let root_a = scratch("dsp-a");
    let root_b = scratch("dsp-b");
    let media_a = root_a.join("media");
    let media_b = root_b.join("media");

    let mut c_a = controller();
    let (_ta, _strip_a) = clip_project(&mut c_a, &root_a);
    let mut engine_a = new_engine();
    publish(&mut engine_a, &c_a, 1, &media_a);

    let mut c_b = controller();
    let (_tb, strip_b) = clip_project(&mut c_b, &root_b);
    let mut engine_b = new_engine();
    publish(&mut engine_b, &c_b, 1, &media_b);

    // Engine B observes the same strip at two boundaries — Monitor
    // metrics plus a bounded PCM capture — while engine A stays Off.
    let in_key = strip_key(strip_b, SignalBoundary::Input);
    let out_key = strip_key(strip_b, SignalBoundary::Output);
    engine_b
        .probe_request(c_b.live(), in_key, ProbeMode::Monitor)
        .unwrap();
    engine_b
        .probe_request(c_b.live(), out_key, ProbeMode::Monitor)
        .unwrap();
    engine_b.probe_capture(out_key, 4_096).unwrap();

    let report_a = engine_a.run(6, PumpPolicy::Every(1)).unwrap();
    let report_b = engine_b.run(6, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine_b);

    assert_eq!(
        report_a.pcm, report_b.pcm,
        "probe Monitor/Capture must not alter master PCM"
    );
    assert_eq!(report_a.crc, report_b.crc);
    assert!(
        report_a
            .pcm
            .iter()
            .any(|channel| channel.iter().any(|s| *s != 0.0)),
        "fixture produces non-silent PCM — the comparison is meaningful"
    );
    assert_eq!(
        binding(&engine_b, &out_key).state,
        ProbeResolveState::BoundActive
    );
}

// ── bounded fault injection ─────────────────────────────────────

#[test]
fn sf04_s3_probe_table_capacity_is_bounded() {
    let root = scratch("capacity");
    let media = root.join("media");
    let mut c = controller();
    let (_track, strip) = clip_project(&mut c, &root);
    let mut engine = new_engine();
    publish(&mut engine, &c, 1, &media);

    // `ProbeConfig::max_probes` is 32 — distinct subscription keys on
    // the same point each install their own broker probe.
    for session in 1..=32_u64 {
        engine
            .probe_request(c.live(), session_key(session, strip), ProbeMode::Monitor)
            .unwrap_or_else(|e| panic!("probe {session} must install: {e}"));
    }
    assert!(
        matches!(
            engine.probe_request(c.live(), session_key(33, strip), ProbeMode::Monitor),
            Err(ProbeRequestError::Broker(
                ProbeError::CapacityExceeded { .. }
            ))
        ),
        "the 33rd probe fails closed on the bounded table"
    );
}

#[test]
fn sf04_s3_rebind_channel_full_stays_atomic_and_recovers() {
    let root = scratch("rebind-full");
    let media = root.join("media");
    let mut c = controller();
    let (_track, strip) = clip_project(&mut c, &root);
    let mut engine = new_engine();
    publish(&mut engine, &c, 1, &media);

    // Fill the probe command channel without any RT drain: the
    // generation-1 on_publish pair plus 32 install binds leave 30
    // slots; generation 2's Begin+32 Stage+Commit needs 34 — the
    // commit can never land, so the rebind must stay atomic.
    for session in 1..=32_u64 {
        engine
            .probe_request(c.live(), session_key(session, strip), ProbeMode::Monitor)
            .unwrap();
    }
    add_track_strip(&mut c);
    publish(&mut engine, &c, 2, &media);

    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);

    // The dropped commit means no RT binding ever installed: every
    // subscription stays honestly queued — never falsely confirmed.
    assert!(engine.rt_status().stale_commands > 0);
    for session in 1..=32_u64 {
        let key = session_key(session, strip);
        assert_eq!(
            binding(&engine, &key).state,
            ProbeResolveState::QueuedForBinding,
            "session {session} must not claim RT installation"
        );
    }

    // The next publish re-runs the whole staged rebind on a drained
    // channel — atomic commit, then RT-confirmed BoundActive.
    add_track_strip(&mut c);
    publish(&mut engine, &c, 3, &media);
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);

    for session in 1..=32_u64 {
        let key = session_key(session, strip);
        let b = binding(&engine, &key);
        assert_eq!(b.state, ProbeResolveState::BoundActive, "session {session}");
        assert_eq!(b.active_plan_generation, 3);
    }
}
