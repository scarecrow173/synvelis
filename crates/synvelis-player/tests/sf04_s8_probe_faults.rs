//! SF-04 slice 8 (Issue #107, SF-PROBE-05): bounded fault fixtures
//! over the real `RoutingEngine` + `ProbeTraceBridge` — chunk-pool
//! exhaustion (`lost_blocks`/`LostBlocks`/`Lost` observation/
//! `Incomplete` segment), telemetry overflow (`lost_records` →
//! fail-closed coverage downgrade), bridge-level distinction between
//! measured silence / disabled / lost / missing, and the bounded
//! capture variants (`max_frames` budget vs explicit
//! `stop_capture`).
//!
//! Reachability notes (honest, not hidden):
//!
//! - `StatusKind::LostRecords` is declared and mapped end-to-end
//!   (`status_kind` → `ProbeStatusKind::LostRecords`, segment gap →
//!   `ObservationGapKind::LostRecords`, coverage →
//!   `ObservationGapKind::LostRecords`, slot update →
//!   `ProbeObservation::Lost`), but the deck never *emits* it —
//!   telemetry overflow only increments the `lost_records` counter.
//!   Evidence therefore stays counter + `lost_records`-floor based.
//! - `StatusKind::Unobserved { Stale | BorrowConflict }` is likewise
//!   mapped (`ProbeStatusKind::Unobserved`, segment gap
//!   `ObservationGapKind::Unobserved`, coverage
//!   `CoverageClass::Unobserved`) but unreachable through the public
//!   engine API on this build: unconnected inputs resolve
//!   `Unavailable` at install (no slot is ever bound), every bound
//!   slot of the static schedule is written each callback, and a
//!   `try_borrow` conflict is RT-internal. Mapping-only evidence —
//!   see `sf04_s8_unreachable_fault_statuses_are_unfaked`.
//! - `ProbeObservation::Missing` is never synthesized by the drain
//!   path: `StatusKind::Removed` arrives only after the binding's
//!   `runtime_probe_id` was already cleared, so it cannot be
//!   attributed. `Missing` is a session-layer state (no binding
//!   exists) produced above this layer — the drain's honest answer
//!   for a removed subscription is *no update* plus ledger `Stopped`.

use std::collections::BTreeMap;

use synvelis_player::driver::PumpPolicy;
use synvelis_player::lowering::EngineConfig;
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::{ProbeRequestError, ProbeTraceBridge};
use synvelis_project::probe::{
    AcousticObservation, CaptureCompleteness, CaptureSegmentEnd, ObservationGapKind,
    ProbeAlignPolicy, ProbeComparisonBasis, ProbeComparisonQuality, ProbeComparisonRequest,
    ProbeComparisonSide, ProbeIncomparableReason, ProbeMetricKind, ProbeMode, ProbeResolveState,
    ProbeStatusKind, SemanticProbePoint, SemanticSignalEntityRef, SignalBoundary, SubscriptionKind,
};
use synvelis_project::trace::{MeasuredValue, ProbeObservation};
use synvelis_project::{
    BuildBudget, BuildMode, Command, CounterIdGenerator, ProbeSubscriptionKey, ProbeTracePort,
    Project, ProjectController, ProjectId, ProjectOp, ProjectRevision, ProjectSessionId,
    RoutingBuildRequestId, RoutingCompileRequest, RuntimeClockEpochRef, RuntimeInstanceId, StripId,
    TrackId,
};

use ruviform_audio::typed::probe::{ProbeEvent, StatusKind};
use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};

// ---------------------------------------------------------------------------
// fixture (mirrors sf04_s7_probe_compare.rs — no clips needed:
// monitored/captured silence is still observed data)
// ---------------------------------------------------------------------------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(u128::from(0x107_u64)), "sf04-s8-faults"),
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

fn add_track_strip(c: &mut ProjectController, name: &str) -> (TrackId, StripId) {
    apply(c, ProjectOp::AddTrack { name: name.into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    (track, strip)
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

fn publish(engine: &mut RoutingEngine, c: &ProjectController, request_id: u64) -> u64 {
    let revision = c.live_revision();
    let canonical = |_: synvelis_project::SourceId| Some(canonical_format());
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

fn point(strip: StripId, boundary: SignalBoundary) -> SemanticProbePoint {
    SemanticProbePoint {
        entity: SemanticSignalEntityRef::Strip(strip),
        boundary,
        port: None,
    }
}

fn sub_key(session: u64, strip: StripId, boundary: SignalBoundary) -> ProbeSubscriptionKey {
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

fn subscribe(
    bridge: &mut ProbeTraceBridge,
    c: &ProjectController,
    session: u64,
    strip: StripId,
    boundary: SignalBoundary,
) {
    let key = sub_key(session, strip, boundary);
    bridge
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
}

/// Runs `blocks` engine blocks then drains probe telemetry through the
/// bridge — the only path that feeds retained coverage evidence.
fn run_drain(bridge: &mut ProbeTraceBridge, blocks: usize) {
    bridge
        .engine_mut()
        .run(blocks, PumpPolicy::Every(1))
        .unwrap();
    let _ = bridge.probe_drain();
}

fn incomparable_reason(obs: &AcousticObservation) -> &ProbeIncomparableReason {
    match &obs.quality {
        ProbeComparisonQuality::Incomparable { reason } => reason,
        other => panic!("expected Incomparable, got {other:?}"),
    }
}

/// Two-strip project, published, both output points monitored —
/// mirrors `monitored_pair` in `sf04_s7`.
fn monitored_pair() -> (
    ProbeTraceBridge,
    ProjectController,
    SemanticProbePoint,
    SemanticProbePoint,
) {
    let mut c = controller();
    let (_t1, strip_a) = add_track_strip(&mut c, "a");
    let (_t2, strip_b) = add_track_strip(&mut c, "b");
    let mut bridge = ProbeTraceBridge::new(
        RoutingEngine::new(engine(), RuntimeInstanceId(1), RuntimeClockEpochRef(1)),
        BTreeMap::new(),
    );
    publish(bridge.engine_mut(), &c, 1);
    let before = point(strip_a, SignalBoundary::Output);
    let after = point(strip_b, SignalBoundary::Output);
    subscribe(&mut bridge, &c, 1, strip_a, SignalBoundary::Output);
    subscribe(&mut bridge, &c, 2, strip_b, SignalBoundary::Output);
    (bridge, c, before, after)
}

// ---------------------------------------------------------------------------
// SF-PROBE-05: chunk-pool exhaustion → lost_blocks evidence chain
// ---------------------------------------------------------------------------

#[test]
fn sf04_s8_chunk_pool_exhaustion_records_lost_blocks() {
    let mut c = controller();
    let (_t, strip) = add_track_strip(&mut c, "t");
    let mut bridge = ProbeTraceBridge::new(
        RoutingEngine::new(engine(), RuntimeInstanceId(1), RuntimeClockEpochRef(1)),
        BTreeMap::new(),
    );
    publish(bridge.engine_mut(), &c, 1);

    let key = sub_key(1, strip, SignalBoundary::Output);
    subscribe(&mut bridge, &c, 1, strip, SignalBoundary::Output);
    run_drain(&mut bridge, 1);
    assert_eq!(
        bridge.probe_bindings()[&key].state,
        ProbeResolveState::BoundActive
    );

    // Explicit bounded capture, generous budget — the pool, not the
    // budget, is the fault under test. `pool_chunks == chunks_out`
    // capacity == 8: chunks recycle only when the collector drains,
    // so 12 undrained capture blocks exhaust the pool and the last
    // four fail `free.try_pop`.
    bridge.engine_mut().probe_capture(key, 65_536).unwrap();
    bridge.engine_mut().run(12, PumpPolicy::Every(1)).unwrap();

    let counts = bridge.engine().probe_counts();
    assert_eq!(
        counts.lost_blocks, 4,
        "8 pooled chunks cover 8 blocks; blocks 9-12 must count lost_blocks"
    );
    assert_eq!(
        counts.lost_records, 0,
        "telemetry never overflowed — the fault under test is isolated"
    );

    // Drain: four `LostBlocks` statuses + eight captured chunks.
    // The bridge surfaces each status as a `Lost` observation carrying
    // the cumulative counter, and the ledger records the gap kind.
    let updates = bridge.probe_drain();
    assert!(
        updates.iter().any(|u| {
            u.key == key && matches!(u.observation, ProbeObservation::Lost { count: 4 })
        }),
        "LostBlocks status must surface as ProbeObservation::Lost, got {updates:?}"
    );
    assert_eq!(
        bridge.probe_bindings()[&key]
            .last_status
            .as_ref()
            .map(|s| &s.kind),
        Some(&ProbeStatusKind::LostBlocks),
        "the latest RT status is the pool-exhaustion report"
    );

    // Explicit stop closes the segment: the recorded gaps make it
    // honestly Incomplete — never Complete, never Unknown-by-default.
    bridge.probe_stop_capture(key).unwrap();
    run_drain(&mut bridge, 1);

    let segments = bridge.probe_segments();
    assert_eq!(segments.len(), 1, "one capture session, one segment");
    let segment = &segments[0];
    assert_eq!(segment.ended_by, Some(CaptureSegmentEnd::Requested));
    assert_eq!(segment.completeness, CaptureCompleteness::Incomplete);
    assert!(
        !segment.discontinuities.is_empty()
            && segment
                .discontinuities
                .iter()
                .all(|gap| gap.kind == ObservationGapKind::LostBlocks),
        "every recorded gap is LostBlocks, got {:?}",
        segment.discontinuities
    );
    assert!(segment.frame_range.is_some(), "delivered PCM kept");

    // No implicit recapture: further blocks emit metrics only.
    run_drain(&mut bridge, 2);
    assert_eq!(bridge.probe_segments().len(), 1, "no auto-recapture");
}

// ---------------------------------------------------------------------------
// SF-PROBE-05: telemetry overflow → lost_records → coverage downgrade
// ---------------------------------------------------------------------------

#[test]
fn sf04_s8_telemetry_overflow_downgrades_coverage_fail_closed() {
    let (mut bridge, _c, before, after) = monitored_pair();

    // Baseline: a drained, loss-free window aligns on the shared
    // cursor — the downgrade below is caused by loss, not the fixture.
    run_drain(&mut bridge, 4);
    let baseline = bridge.compare_probes(&compare_request(before, after));
    assert!(
        matches!(
            baseline.quality,
            ProbeComparisonQuality::Aligned {
                basis: ProbeComparisonBasis::SharedCursor { .. },
                ..
            }
        ),
        "loss-free baseline must align, got {:?}",
        baseline.quality
    );

    // `telemetry_capacity` is 256 records and the collector only
    // drains on `probe_drain`: two monitor probes emit one record per
    // block each, so 140 undrained blocks push 280 records and the
    // newest ~24 are dropped with `lost_records` counted. Capture
    // chunks are an independent channel — this fault is telemetry-only.
    bridge.engine_mut().run(140, PumpPolicy::Every(1)).unwrap();
    let counts = bridge.engine().probe_counts();
    assert!(
        counts.lost_records > 0,
        "280 undrained telemetry records must overflow the 256-slot channel"
    );
    assert_eq!(
        counts.lost_blocks, 0,
        "no capture is active — chunk loss must stay zero"
    );

    // Drain once: events produced *before* this batch keep their
    // `lost_records == 0` floor while the counter now reads positive —
    // the delta localizes the loss inside the retained window.
    let _ = bridge.probe_drain();

    let obs = bridge.compare_probes(&compare_request(before, after));
    // Every retained interval reconstructs as `Unknown` (the counter
    // cannot localize the loss), so no jointly-valid window exists —
    // fail closed, never `Aligned`.
    assert_eq!(
        incomparable_reason(&obs),
        &ProbeIncomparableReason::CoverageEmpty,
        "loss-downgraded coverage must leave no valid joint window"
    );
    assert!(obs.interval.is_some(), "sides still resolve a window");
    for side in [ProbeComparisonSide::Before, ProbeComparisonSide::After] {
        assert!(
            obs.gap_mask
                .iter()
                .any(|gap| gap.side == side && gap.kind == ObservationGapKind::Unknown),
            "downgraded {side:?} coverage must surface as Unknown gaps, got {:?}",
            obs.gap_mask
        );
    }
}

// ---------------------------------------------------------------------------
// bounded capture variants: budget vs explicit stop vs removal
// ---------------------------------------------------------------------------

#[test]
fn sf04_s8_capture_variants_close_with_distinct_reasons() {
    let mut c = controller();
    let (_t, strip) = add_track_strip(&mut c, "t");
    let mut bridge = ProbeTraceBridge::new(
        RoutingEngine::new(engine(), RuntimeInstanceId(1), RuntimeClockEpochRef(1)),
        BTreeMap::new(),
    );
    publish(bridge.engine_mut(), &c, 1);

    let key = sub_key(1, strip, SignalBoundary::Output);
    subscribe(&mut bridge, &c, 1, strip, SignalBoundary::Output);
    run_drain(&mut bridge, 1);

    // (a) `max_frames` budget: 1024 frames = four 256-frame blocks —
    // the RT side auto-stops with `BudgetExhausted`.
    bridge.probe_capture(key, 1_024).unwrap();
    // A second capture request while the segment is open fails
    // closed — captures are explicit sessions, never nested.
    assert!(matches!(
        bridge.engine_mut().probe_capture(key, 1_024),
        Err(ProbeRequestError::AlreadyCapturing)
    ));
    run_drain(&mut bridge, 4);

    let segments = bridge.probe_segments();
    assert_eq!(segments.len(), 1);
    let budgeted = &segments[0];
    assert_eq!(budgeted.ended_by, Some(CaptureSegmentEnd::BudgetExhausted));
    assert_eq!(budgeted.completeness, CaptureCompleteness::Complete);
    assert_eq!(
        budgeted.frame_range.map(|r| r.end - r.start),
        Some(1_024),
        "the budget bounds exactly the captured frames"
    );
    let first_session = budgeted.capture_session_id;

    // (b) Explicit `stop_capture`: a distinct end reason, still a
    // Complete segment — requested stop is not a fault.
    bridge.probe_capture(key, 8_192).unwrap();
    run_drain(&mut bridge, 2);
    bridge.probe_stop_capture(key).unwrap();
    run_drain(&mut bridge, 1);

    let segments = bridge.probe_segments();
    assert_eq!(segments.len(), 2, "explicit recapture is a new segment");
    let stopped = &segments[1];
    assert_eq!(stopped.ended_by, Some(CaptureSegmentEnd::Requested));
    assert_eq!(stopped.completeness, CaptureCompleteness::Complete);
    assert_ne!(
        stopped.capture_session_id, first_session,
        "each explicit capture mints its own session id"
    );

    // (c) Removal while capturing: the ledger closes the open segment
    // itself with `Removed` — the RT status stays the confirmation.
    bridge.probe_capture(key, 8_192).unwrap();
    run_drain(&mut bridge, 1);
    bridge.probe_remove(key).unwrap();
    run_drain(&mut bridge, 1);

    let segments = bridge.probe_segments();
    assert_eq!(segments.len(), 3);
    assert_eq!(segments[2].ended_by, Some(CaptureSegmentEnd::Removed));
    assert_eq!(
        bridge.probe_bindings()[&key].state,
        ProbeResolveState::Stopped,
        "removal is lifecycle evidence, never a silent unbind"
    );
}

// ---------------------------------------------------------------------------
// bridge-level observation-state distinction
// ---------------------------------------------------------------------------

#[test]
fn sf04_s8_observation_states_stay_distinct() {
    let mut c = controller();
    let (_t, strip) = add_track_strip(&mut c, "t");
    let mut bridge = ProbeTraceBridge::new(
        RoutingEngine::new(engine(), RuntimeInstanceId(1), RuntimeClockEpochRef(1)),
        BTreeMap::new(),
    );
    publish(bridge.engine_mut(), &c, 1);

    let key = sub_key(1, strip, SignalBoundary::Output);
    subscribe(&mut bridge, &c, 1, strip, SignalBoundary::Output);
    bridge.engine_mut().run(2, PumpPolicy::Every(1)).unwrap();

    // Silence is *measured* data — `Measuring{Silence}`, distinct from
    // every absence/loss state below.
    let updates = bridge.probe_drain();
    assert!(
        updates.iter().any(|u| {
            u.key == key
                && matches!(
                    &u.observation,
                    ProbeObservation::Measuring {
                        value: MeasuredValue::Silence,
                        ..
                    }
                )
        }),
        "monitored silence must surface as Measuring{{Silence}}, got {updates:?}"
    );

    // Off → Disabled: the channel reports an explicit state, and the
    // probe emits nothing while Off.
    bridge.probe_set_mode(key, ProbeMode::Off).unwrap();
    bridge.engine_mut().run(1, PumpPolicy::Every(1)).unwrap();
    let updates = bridge.probe_drain();
    assert!(
        updates
            .iter()
            .any(|u| u.key == key && matches!(u.observation, ProbeObservation::Disabled)),
        "ModeChanged→Off must surface Disabled"
    );
    assert!(
        !updates.iter().any(|u| {
            u.key == key && matches!(u.observation, ProbeObservation::Measuring { .. })
        }),
        "an Off probe emits no measurements"
    );

    // Resume → measured silence again: coming back from Disabled is
    // observed data, never Missing.
    bridge.probe_set_mode(key, ProbeMode::Monitor).unwrap();
    bridge.engine_mut().run(1, PumpPolicy::Every(1)).unwrap();
    let updates = bridge.probe_drain();
    assert!(
        updates.iter().any(|u| {
            u.key == key
                && matches!(
                    &u.observation,
                    ProbeObservation::Measuring {
                        value: MeasuredValue::Silence,
                        ..
                    }
                )
        }),
        "resumed monitoring must measure silence again"
    );

    // A second subscription on the same point is independent —
    // removing one must not silence the other.
    let key_b = sub_key(2, strip, SignalBoundary::Output);
    subscribe(&mut bridge, &c, 2, strip, SignalBoundary::Output);
    run_drain(&mut bridge, 1);

    // Removal: the drain must NOT fabricate `Missing` — the RT
    // `Removed` status is unattributable (the binding's
    // `runtime_probe_id` was cleared synchronously), so the honest
    // evidence is *no update* plus ledger `Stopped`. `Missing` is a
    // session-layer state owned above this bridge.
    bridge.probe_remove(key).unwrap();
    bridge.engine_mut().run(1, PumpPolicy::Every(1)).unwrap();
    let updates = bridge.probe_drain();
    assert!(
        !updates
            .iter()
            .any(|u| u.key == key && matches!(u.observation, ProbeObservation::Missing)),
        "the drain must never synthesize Missing for a removed probe"
    );
    assert!(
        updates.iter().any(|u| {
            u.key == key_b
                && matches!(
                    &u.observation,
                    ProbeObservation::Measuring {
                        value: MeasuredValue::Silence,
                        ..
                    }
                )
        }),
        "the surviving subscription on the same point keeps measuring"
    );
    assert_eq!(
        bridge.probe_bindings()[&key].state,
        ProbeResolveState::Stopped
    );

    // With every subscription on the point removed, comparison fails
    // closed as `ProbeNotBound` — a stopped binding is not silence.
    bridge.probe_remove(key_b).unwrap();
    run_drain(&mut bridge, 1);
    let obs = bridge.compare_probes(&compare_request(
        point(strip, SignalBoundary::Output),
        point(strip, SignalBoundary::Output),
    ));
    assert!(
        matches!(
            incomparable_reason(&obs),
            ProbeIncomparableReason::ProbeNotBound {
                side: ProbeComparisonSide::Before
            }
        ),
        "removed subscriptions leave bindings without runtime probes"
    );
}

// ---------------------------------------------------------------------------
// honest reachability: statuses the deck cannot emit stay unfaked
// ---------------------------------------------------------------------------

#[test]
fn sf04_s8_unreachable_fault_statuses_are_unfaked() {
    let mut c = controller();
    let (_t1, strip_a) = add_track_strip(&mut c, "a");
    let (_t2, strip_b) = add_track_strip(&mut c, "b");
    let mut bridge = ProbeTraceBridge::new(
        RoutingEngine::new(engine(), RuntimeInstanceId(1), RuntimeClockEpochRef(1)),
        BTreeMap::new(),
    );
    publish(bridge.engine_mut(), &c, 1);

    let key_a = sub_key(1, strip_a, SignalBoundary::Output);
    let key_b = sub_key(2, strip_b, SignalBoundary::Output);
    subscribe(&mut bridge, &c, 1, strip_a, SignalBoundary::Output);
    subscribe(&mut bridge, &c, 2, strip_b, SignalBoundary::Output);

    // Drain through the engine directly (not the bridge port): this
    // test collects raw runtime `StatusKind`s, which the bridge
    // drain consumes internally for coverage/updates.
    let mut kinds = Vec::new();
    let mut run_collect = |bridge: &mut ProbeTraceBridge, blocks: usize| {
        bridge
            .engine_mut()
            .run(blocks, PumpPolicy::Every(1))
            .unwrap();
        bridge.engine_mut().probe_drain(|event| {
            if let ProbeEvent::Status(status) = event {
                kinds.push(status.kind);
            }
        });
    };
    run_collect(&mut bridge, 2);

    // Exercise the reachable fault surface: capture success + budget
    // stop, mode off/on, removal, and a generation rebind.
    bridge.probe_capture(key_a, 512).unwrap();
    run_collect(&mut bridge, 4);
    bridge.probe_set_mode(key_b, ProbeMode::Off).unwrap();
    run_collect(&mut bridge, 1);
    bridge.probe_set_mode(key_b, ProbeMode::Monitor).unwrap();
    add_track_strip(&mut c, "c");
    publish(bridge.engine_mut(), &c, 2);
    run_collect(&mut bridge, 4);
    assert!(
        kinds.contains(&StatusKind::Installed)
            && kinds.contains(&StatusKind::CaptureStarted { budget_frames: 512 })
            && kinds.iter().any(|k| matches!(
                k,
                StatusKind::CaptureStopped {
                    reason: ruviform_audio::typed::probe::StopReason::BudgetExhausted
                }
            )),
        "the reachable lifecycle statuses must be present, got {kinds:?}"
    );
    assert!(
        !kinds.iter().any(|k| matches!(k, StatusKind::LostRecords)),
        "the deck never emits LostRecords — only the counter moves"
    );
    assert!(
        !kinds
            .iter()
            .any(|k| matches!(k, StatusKind::Unobserved { .. })),
        "no public engine path produces Unobserved on this build"
    );

    // The counters agree: nothing was unobserved, and record loss is
    // only ever the telemetry counter — never a per-slot status.
    let counts = bridge.engine().probe_counts();
    assert_eq!(counts.unobserved, 0, "no stale/borrow-conflict slots");
    assert_eq!(counts.lost_blocks, 0, "budgeted capture never overflowed");

    // Segment gaps across the whole run contain no unreachable kinds
    // either — mapping exists, emission does not.
    for segment in bridge.probe_segments() {
        for gap in &segment.discontinuities {
            assert!(
                !matches!(
                    gap.kind,
                    ObservationGapKind::LostRecords | ObservationGapKind::Unobserved
                ),
                "unreachable gap kind must never be recorded: {gap:?}"
            );
        }
    }
}
