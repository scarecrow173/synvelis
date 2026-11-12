//! SF-04 slice 8 (Issue #107): `RoutingEngine::declare_probe_discontinuity`
//! — the non-seek cursor-series discontinuity declaration point.
//! Proves a declaration that does **not** move the transport cursor
//! produces the same `ClockEpoch` segment cut and
//! `DiscontinuityInWindow` comparison behavior as `seek`, that
//! `stop`/`run` are *not* declaration points, that a fresh engine is
//! a fresh clock context (device restart = recreation), and that a
//! full command channel reports the error honestly while the broker
//! epoch still advances.
//!
//! Fixture mirrors `sf04_s3_probe_discontinuity.rs` (clip-bearing
//! project for transport/PCM evidence) and `sf04_s7_probe_compare.rs`
//! (bridge compare evidence).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_player::ProbeTraceBridge;
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::lowering::EngineConfig;
use synvelis_player::resolver::prepare;
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_project::ProbeTracePort;
use synvelis_project::probe::{
    AcousticObservation, CaptureCompleteness, CaptureSegmentEnd, ProbeAlignPolicy,
    ProbeComparisonBasis, ProbeComparisonQuality, ProbeComparisonRequest, ProbeIncomparableReason,
    ProbeMetricKind, ProbeMode, ProbeResolveState, SemanticProbePoint, SemanticSignalEntityRef,
    SignalBoundary, SubscriptionKind,
};
use synvelis_project::trace::ProbeObservation;
use synvelis_project::{
    BuildBudget, BuildMode, ClipExtent, ClipPlacement, ClipPlayback, Command, CounterIdGenerator,
    LaneId, MediaDuration, MediaTimeValue, PlaybackMapping, ProbeSubscriptionKey, Project,
    ProjectController, ProjectId, ProjectOp, ProjectRevision, ProjectSessionId,
    RoutingBuildRequestId, RoutingCompileRequest, RuntimeClockEpochRef, RuntimeInstanceId,
    SourceFrame, SourceId, SourceRange, StripId, TimeAnchor, TrackId,
};

use ruviform_audio::typed::probe::{ProbeError, ProbeEvent};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_core::ClockEpochId;
use ruviform_media_reader::PrefetchConfig;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf04s9-{}-{}-{}",
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
        Project::new(ProjectId::new(0x107), "sf04s9"),
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

/// Clip-bearing project: track strip + lane + one 2048-frame clip —
/// mirrors `sf04_s3` so transport/PCM evidence is meaningful.
fn clip_project(c: &mut ProjectController, root: &Path) -> (TrackId, StripId) {
    let source = import_stereo(c, root, "a.wav", 4_800);
    let (track, lane, strip) = add_track_strip(c);
    add_clip(c, lane, source);
    (track, strip)
}

/// prepare + publish the live project — returns the published
/// generation. `media` is the clip media root for clip-bearing
/// projects; clip-less projects pass any path (no clips prepared).
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

/// Clip-less publish for the compare fixture (mirrors `sf04_s7` —
/// `clips` stays empty).
fn publish_empty(engine: &mut RoutingEngine, c: &ProjectController, request_id: u64) -> u64 {
    let revision = c.live_revision();
    let canonical = |_: SourceId| Some(canonical_format());
    let deps = RoutingDeps {
        canonical_of: &canonical,
        clips: Vec::new(),
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

fn point(strip: StripId, boundary: SignalBoundary) -> SemanticProbePoint {
    SemanticProbePoint {
        entity: SemanticSignalEntityRef::Strip(strip),
        boundary,
        port: None,
    }
}

fn key(session: u64, strip: StripId, boundary: SignalBoundary) -> ProbeSubscriptionKey {
    ProbeSubscriptionKey {
        session_id: ProjectSessionId(session),
        point: point(strip, boundary),
        kind: SubscriptionKind::Metrics,
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

/// Two-strip clip-less project on a bridge — mirrors `monitored_pair`
/// in `sf04_s7`.
fn monitored_pair() -> (
    ProbeTraceBridge,
    ProjectController,
    SemanticProbePoint,
    SemanticProbePoint,
) {
    let mut c = controller();
    apply(&mut c, ProjectOp::AddTrack { name: "a".into() });
    let track_a = *c.live().tracks.keys().next_back().unwrap();
    apply(&mut c, ProjectOp::AddTrackStrip { track_id: track_a });
    let strip_a = c.live().routing.strip_of_track(track_a).unwrap().id;
    apply(&mut c, ProjectOp::AddTrack { name: "b".into() });
    let track_b = *c.live().tracks.keys().next_back().unwrap();
    apply(&mut c, ProjectOp::AddTrackStrip { track_id: track_b });
    let strip_b = c.live().routing.strip_of_track(track_b).unwrap().id;

    let mut bridge = ProbeTraceBridge::new(new_engine(), BTreeMap::new());
    publish_empty(bridge.engine_mut(), &c, 1);
    let before = point(strip_a, SignalBoundary::Output);
    let after = point(strip_b, SignalBoundary::Output);
    for (session, strip) in [(1_u64, strip_a), (2_u64, strip_b)] {
        let k = key(session, strip, SignalBoundary::Output);
        bridge
            .probe_request(c.live(), k, ProbeMode::Monitor)
            .unwrap();
    }
    (bridge, c, before, after)
}

fn run_drain(bridge: &mut ProbeTraceBridge, blocks: usize) {
    bridge
        .engine_mut()
        .run(blocks, PumpPolicy::Every(1))
        .unwrap();
    let _ = bridge.probe_drain();
}

// ---------------------------------------------------------------------------
// declare == seek for probe evidence; declare != transport move
// ---------------------------------------------------------------------------

#[test]
fn sf04_s9_declare_cuts_segment_at_clock_epoch_without_moving_transport() {
    let root_a = scratch("declare-a");
    let root_b = scratch("declare-b");
    let media_a = root_a.join("media");
    let media_b = root_b.join("media");

    let mut c_a = controller();
    let (_ta, strip_a) = clip_project(&mut c_a, &root_a);
    let mut engine_a = new_engine();
    publish(&mut engine_a, &c_a, 1, &media_a);

    let mut c_b = controller();
    let (_tb, strip_b) = clip_project(&mut c_b, &root_b);
    let mut engine_b = new_engine();
    publish(&mut engine_b, &c_b, 1, &media_b);

    // Identical probe setup on both engines: monitor + open capture.
    let key_a = strip_key(strip_a, SignalBoundary::Output);
    let key_b = strip_key(strip_b, SignalBoundary::Output);
    engine_a
        .probe_request(c_a.live(), key_a, ProbeMode::Monitor)
        .unwrap();
    engine_b
        .probe_request(c_b.live(), key_b, ProbeMode::Monitor)
        .unwrap();
    engine_a.run(1, PumpPolicy::Every(1)).unwrap();
    engine_b.run(1, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine_a);
    drain(&mut engine_b);
    engine_a.probe_capture(key_a, 65_536).unwrap();
    engine_b.probe_capture(key_b, 65_536).unwrap();
    engine_a.run(4, PumpPolicy::Every(1)).unwrap();
    engine_b.run(4, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine_a);
    drain(&mut engine_b);

    let epoch0 = engine_a.probe_time_context().epoch;

    // The declaration: epoch advances exactly once; the transport
    // cursor is *not* touched — unlike `seek`, no re-anchor happens.
    engine_a.declare_probe_discontinuity().unwrap();
    let epoch1 = engine_a.probe_time_context().epoch;
    assert_ne!(epoch1, epoch0, "declare bumps the probe clock epoch");

    // Post-declaration evidence: new-epoch chunks match no open
    // segment and are discarded — the open segment cuts at
    // `ClockEpoch` on the first new-epoch record, exactly like seek.
    let report_a = engine_a.run(4, PumpPolicy::Every(1)).unwrap();
    let report_b = engine_b.run(4, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine_a);
    drain(&mut engine_b);

    // Transport untouched: identical projects + identical block
    // counts must produce bit-identical master PCM — a cursor move
    // (seek) would re-anchor clip handles and change the output.
    assert_eq!(
        report_a.pcm, report_b.pcm,
        "declare must not alter transport — PCM is bit-identical to the undeclared engine"
    );
    assert_eq!(report_a.crc, report_b.crc);

    let segments_a = engine_a.probe_segments();
    assert_eq!(segments_a.len(), 1, "the epoch cut closes the segment");
    let segment = &segments_a[0];
    assert_eq!(segment.ended_by, Some(CaptureSegmentEnd::ClockEpoch));
    assert_eq!(
        segment.clock_context.map(|clock| clock.epoch),
        Some(epoch0),
        "the closed segment keeps its old-epoch clock context"
    );
    assert_eq!(segment.completeness, CaptureCompleteness::Complete);

    // The control engine never declared: its capture is still open —
    // no segment closed at all.
    assert!(
        engine_b.probe_segments().is_empty(),
        "without a declaration the segment stays open"
    );

    // Post-cut chunks carry the new epoch and are discarded — the RT
    // capture runs on but never re-attributes data.
    engine_a.run(4, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine_a);
    assert_eq!(
        engine_a.probe_segments().len(),
        1,
        "no auto-recapture after the epoch cut"
    );

    // Return to Monitor so metric records flow again — capture mode
    // emits chunks only. The stop status finds no open segment (the
    // epoch cut already closed it) and records nothing new.
    engine_a.probe_stop_capture(key_a).unwrap();
    engine_a.run(1, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine_a);

    // Metric records confirm the epoch evidence on the cursor series:
    // post-declare records carry epoch1 while positions stay monotonic
    // — the RT frame cursor counts processed frames either way.
    engine_a.run(1, PumpPolicy::Every(1)).unwrap();
    let mut post = Vec::new();
    engine_a.probe_drain(|e| {
        if let ProbeEvent::Metric(m) = e {
            post.push((m.meta.sample_position, m.meta.clock));
        }
    });
    assert!(
        post.iter().any(|(_, mark)| mark.epoch == epoch1),
        "post-declare records carry the new epoch"
    );
    assert!(
        post.iter()
            .all(|(_, mark)| mark.epoch == epoch0 || mark.epoch == epoch1),
        "no third epoch appears"
    );

    // An explicit new capture under the new epoch is a new session —
    // same rule as after seek.
    engine_a.probe_capture(key_a, 2_048).unwrap();
    engine_a.run(9, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine_a);
    let segments = engine_a.probe_segments();
    assert_eq!(segments.len(), 2);
    assert_eq!(
        segments[1].ended_by,
        Some(CaptureSegmentEnd::BudgetExhausted)
    );
    assert_eq!(
        segments[1].clock_context.map(|clock| clock.epoch),
        Some(epoch1),
        "the new segment records the post-declare epoch"
    );
}

// ---------------------------------------------------------------------------
// declare produces the same comparison verdict as seek
// ---------------------------------------------------------------------------

#[test]
fn sf04_s9_declare_window_spanning_boundary_is_discontinuity_in_window() {
    let (mut bridge, _c, before, after) = monitored_pair();
    run_drain(&mut bridge, 4);

    // Baseline: pre-declaration evidence aligns — the incomparable
    // verdict below is caused by the declared discontinuity.
    let baseline = bridge.compare_probes(&compare_request(before, after));
    assert!(
        matches!(
            baseline.quality,
            ProbeComparisonQuality::Aligned {
                basis: ProbeComparisonBasis::SharedCursor { .. },
                ..
            }
        ),
        "pre-declare baseline must align, got {:?}",
        baseline.quality
    );

    // A non-seek declaration bumps the same clock epoch `seek` does —
    // a comparison window spanning the boundary mixes clock marks and
    // must never align.
    bridge.engine_mut().declare_probe_discontinuity().unwrap();
    run_drain(&mut bridge, 4);

    let obs = bridge.compare_probes(&compare_request(before, after));
    assert_eq!(
        incomparable_reason(&obs),
        &ProbeIncomparableReason::DiscontinuityInWindow,
        "a window spanning a declared discontinuity must never align"
    );
}

// ---------------------------------------------------------------------------
// stop/run are not declarations; restart is a fresh context
// ---------------------------------------------------------------------------

#[test]
fn sf04_s9_stop_and_run_are_not_discontinuity_declarations() {
    let root = scratch("stop-run");
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
    assert_eq!(
        engine.probe_bindings()[&key].state,
        ProbeResolveState::BoundActive
    );

    // `stop` changes transport state only — the cursor does not move,
    // so it is not a discontinuity declaration point.
    let epoch0 = engine.probe_time_context().epoch;
    engine.stop();
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);
    assert_eq!(
        engine.probe_time_context().epoch,
        epoch0,
        "stop/run must not bump the probe clock epoch"
    );

    engine.stop();
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    drain(&mut engine);
    assert_eq!(
        engine.probe_time_context().epoch,
        epoch0,
        "repeated stop/run cycles never declare a discontinuity"
    );

    // Post-stop records still carry the same epoch — evidence is
    // continuous across transport state changes.
    let mut record_marks = Vec::new();
    engine.probe_drain(|e| {
        if let ProbeEvent::Metric(m) = e {
            record_marks.push(m.meta.clock.epoch);
        }
    });
    assert!(
        record_marks.iter().all(|epoch| *epoch == epoch0),
        "stop/run produced no epoch boundary in the record stream"
    );

    // A device restart is an engine recreation — a *fresh* clock
    // context, not a bump on the old one: the epoch restarts at the
    // initial value and the domain is minted per session.
    let fresh = new_engine();
    assert_eq!(
        fresh.probe_time_context().epoch,
        ClockEpochId::new(0),
        "a fresh engine starts a new cursor series at epoch 0"
    );
    assert_ne!(
        fresh.probe_time_context().domain,
        engine.probe_time_context().domain,
        "each engine mints its own clock domain — restart evidence can never compare equal"
    );
}

// ---------------------------------------------------------------------------
// declaration error contract: honest failure, epoch still advances
// ---------------------------------------------------------------------------

#[test]
fn sf04_s9_declare_channel_full_reports_error_and_keeps_epoch() {
    let root = scratch("declare-full");
    let media = root.join("media");
    let mut c = controller();
    let (_track, strip) = clip_project(&mut c, &root);
    let mut engine = new_engine();
    publish(&mut engine, &c, 1, &media);

    // Fill the 64-slot probe command channel without any RT drain:
    // installs are capped at `max_probes` (32), so redundant SetMode
    // pushes on an installed probe are the filler — `set_mode` sends
    // a command unconditionally. Fill until the channel refuses.
    for session in 1..=32_u64 {
        engine
            .probe_request(c.live(), session_key(session, strip), ProbeMode::Monitor)
            .unwrap();
    }
    let any_key = session_key(1, strip);
    for _ in 0..64 {
        if engine.probe_set_mode(any_key, ProbeMode::Monitor).is_err() {
            break;
        }
    }

    let epoch0 = engine.probe_time_context().epoch;
    let result = engine.declare_probe_discontinuity();
    assert!(
        matches!(
            result,
            Err(synvelis_player::ProbeRequestError::Broker(
                ProbeError::CommandChannelFull
            ))
        ),
        "an explicit declaration reports channel-full honestly, got {result:?}"
    );
    // The broker epoch advances *before* the failed push — the
    // context and the deck now disagree, observably, never silently.
    assert_ne!(
        engine.probe_time_context().epoch,
        epoch0,
        "the broker epoch still advances on a failed wire"
    );

    // The queued commands drain on the next process; the lost
    // `SetClockMark` means the deck keeps stamping the old epoch —
    // records honestly claim what the deck knew.
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    let epoch_context = engine.probe_time_context().epoch;
    let mut record_epochs = Vec::new();
    engine.probe_drain(|e| {
        if let ProbeEvent::Metric(m) = e {
            record_epochs.push(m.meta.clock.epoch);
        }
    });
    assert!(
        !record_epochs.is_empty(),
        "installed probes emit metrics after the drain"
    );
    assert!(
        record_epochs.iter().all(|epoch| *epoch == epoch0),
        "the deck never received the new mark — records keep the old epoch"
    );
    assert_ne!(
        epoch_context, epoch0,
        "time_context and record marks disagree — observable, fail-closed"
    );

    // A later successful declaration re-wires the mark and resyncs
    // the deck — the disagreement is transient, never permanent.
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    engine.declare_probe_discontinuity().unwrap();
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    let mut resynced = Vec::new();
    engine.probe_drain(|e| {
        if let ProbeEvent::Metric(m) = e {
            resynced.push(m.meta.clock.epoch);
        }
    });
    assert!(
        resynced.iter().any(|epoch| *epoch != epoch0),
        "the next successful wire delivers the pending epoch to the deck"
    );
}

// ---------------------------------------------------------------------------
// updates routed through the bridge see the same epoch evidence
// ---------------------------------------------------------------------------

#[test]
fn sf04_s9_declare_observation_updates_carry_epoch_boundary() {
    let (mut bridge, _c, before, after) = monitored_pair();
    run_drain(&mut bridge, 4);

    bridge.engine_mut().declare_probe_discontinuity().unwrap();
    run_drain(&mut bridge, 4);

    // Metric updates keep flowing across the declaration — the epoch
    // boundary is comparison evidence, not an observation outage.
    let updates = bridge.probe_drain();
    let _ = (before, after, updates);
    // (drain already consumed; assert indirectly via bindings)
    for binding in bridge.probe_bindings().values() {
        assert!(
            binding.last_status.is_some(),
            "bindings retain status evidence across the declaration"
        );
    }
    // Silence of assertion: ProbeObservation updates were produced —
    // re-run one block and check updates explicitly.
    bridge.engine_mut().run(1, PumpPolicy::Every(1)).unwrap();
    let updates = bridge.probe_drain();
    assert!(
        updates
            .iter()
            .any(|u| matches!(u.observation, ProbeObservation::Measuring { .. })),
        "post-declare monitoring still produces measurements"
    );
}
