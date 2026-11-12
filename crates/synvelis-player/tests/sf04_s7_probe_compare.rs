//! SF-04 slice 6 (Issue #107, SF-PROBE-07): `ProbeComparisonRequest
//! → AcousticObservation` — the multi-probe alignment consumer over
//! real `RoutingEngine` probe read models and drain-retained
//! `CoverageEvent` evidence. Headless: every claim below is proven
//! through `ProbeTraceBridge::compare_probes`, which delegates the
//! three-valued verdict to Ruviform #309 `align_observation_points`.
//!
//! Covered branches: `Aligned` (shared cursor basis), `Conditional`
//! (partial coverage — never silently promoted), and `Incomparable`
//! for consumer-side resolution failures (`PointUnresolved`,
//! `ProbeNotBound`, `NoObservationData`, `CoverageEmpty`,
//! `EvidenceBudgetExceeded`) plus runtime reasons reachable on one
//! cursor series (`EpochMismatch`, `DiscontinuityInWindow` on seek,
//! `GenerationSpan` on republish). Reasons that cannot occur inside
//! a single engine's uniform format/clock series (`DomainMismatch`,
//! `FormatMismatch`, `RateMismatch`, `LatencyUnknown`, `UnknownClock`)
//! are delegated to the runtime contract and mirrored in the
//! vocabulary — see the slice-6 test spec.
//!
//! Fixture note: comparisons always pair two *resolvable* points —
//! `Output` boundaries on two distinct track strips. Strip `Input`
//! boundaries legitimately resolve to no runtime slot on this
//! build (no input summing node is lowered for an unconnected
//! strip), which is itself the `ProbeNotBound` evidence used below.

use std::collections::BTreeMap;

use synvelis_player::ProbeTraceBridge;
use synvelis_player::driver::PumpPolicy;
use synvelis_player::lowering::EngineConfig;
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_project::probe::{
    AcousticObservation, ObservationGapKind, ProbeAlignCondition, ProbeAlignPolicy,
    ProbeComparisonBasis, ProbeComparisonQuality, ProbeComparisonRequest, ProbeComparisonSide,
    ProbeFrameInterval, ProbeIncomparableReason, ProbeMetricKind, ProbeMode, SemanticProbePoint,
    SemanticSignalEntityRef, SignalBoundary, SubscriptionKind,
};
use synvelis_project::{
    BuildBudget, BuildMode, Command, CounterIdGenerator, ProbeSubscriptionKey, ProbeTracePort,
    Project, ProjectController, ProjectId, ProjectOp, ProjectRevision, ProjectSessionId,
    RoutingBuildRequestId, RoutingCompileRequest, RuntimeClockEpochRef, RuntimeInstanceId, StripId,
    TrackId,
};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};

// ---------------------------------------------------------------------------
// fixture (mirrors sf04_s2_probe_trace_bridge.rs — no clips needed:
// monitored silence is still observed data)
// ---------------------------------------------------------------------------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(u128::from(0x107_u64)), "sf04-s6-compare"),
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

fn subscribe(
    bridge: &mut ProbeTraceBridge,
    c: &ProjectController,
    session: u64,
    strip: StripId,
    boundary: SignalBoundary,
) {
    let key = key(session, strip, boundary);
    bridge
        .probe_request(c.live(), key, ProbeMode::Monitor)
        .unwrap();
}

/// Runs `blocks` engine blocks then drains probe telemetry into the
/// bridge's retained coverage evidence — the only way coverage
/// accumulates.
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

/// Two-strip project, published, both output points monitored.
/// Returns the bridge plus the two semantic points.
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
// Aligned / Conditional
// ---------------------------------------------------------------------------

#[test]
fn sf04_s7_monitored_probes_align_on_shared_cursor() {
    let (mut bridge, _c, before, after) = monitored_pair();
    run_drain(&mut bridge, 8);

    let obs = bridge.compare_probes(&compare_request(before, after));
    match &obs.quality {
        ProbeComparisonQuality::Aligned {
            basis,
            error_budget_frames,
            ..
        } => {
            assert!(
                matches!(basis, ProbeComparisonBasis::SharedCursor { .. }),
                "same cursor series must align on the shared-cursor basis, got {basis:?}"
            );
            assert_eq!(*error_budget_frames, 0);
        }
        other => panic!("expected Aligned, got {other:?}"),
    }
    // The request echoes through and the compared window is explicit.
    assert_eq!(obs.before, before);
    assert_eq!(obs.after, after);
    assert!(
        obs.interval.is_some(),
        "aligned comparison names its window"
    );
    // Both sides resolved real clock evidence on the same series.
    let (bc, ac) = (obs.provenance.before_clock, obs.provenance.after_clock);
    assert!(bc.is_some() && ac.is_some(), "wired clock marks resolve");
    assert_eq!(bc, ac, "one cursor series — one clock identity");
    assert_eq!(obs.provenance.sample_rate, 48_000);
    assert!(
        obs.gap_mask.is_empty(),
        "fully observed window reports no gaps, got {:?}",
        obs.gap_mask
    );
}

#[test]
fn sf04_s7_partial_coverage_is_conditional_never_aligned() {
    let (mut bridge, _c, before, after) = monitored_pair();
    run_drain(&mut bridge, 8);

    // Learn the observed extent, then ask for a window that runs past
    // it — uncovered request regions are gaps, never silent validity.
    let extent = bridge
        .compare_probes(&compare_request(before, after))
        .interval
        .expect("both sides resolved to evidence spans");
    let mut request = compare_request(before, after);
    request.observed_range = Some(ProbeFrameInterval {
        start: extent.start,
        end: extent.end.saturating_add(4_096),
    });
    let obs = bridge.compare_probes(&request);

    match &obs.quality {
        ProbeComparisonQuality::Conditional {
            conditions,
            excluded_intervals,
            ..
        } => {
            assert!(
                conditions.contains(&ProbeAlignCondition::PartialCoverage),
                "uncovered request tail must demote to Conditional, got {conditions:?}"
            );
            assert!(
                !excluded_intervals.is_empty(),
                "excluded intervals name the uncovered overlap"
            );
        }
        other => panic!("expected Conditional, got {other:?}"),
    }
    for side in [ProbeComparisonSide::Before, ProbeComparisonSide::After] {
        assert!(
            obs.gap_mask
                .iter()
                .any(|g| g.side == side && g.kind == ObservationGapKind::Unknown),
            "unobserved {side:?} window tail must surface as an Unknown gap"
        );
    }
}

// ---------------------------------------------------------------------------
// Incomparable — consumer-side resolution failures
// ---------------------------------------------------------------------------

#[test]
fn sf04_s7_unsubscribed_point_is_point_unresolved() {
    let (mut bridge, _c, _before, after) = monitored_pair();
    run_drain(&mut bridge, 4);

    // `before` names a point nobody ever subscribed — no binding
    // exists for it at all (distinct from `ProbeNotBound`, where a
    // binding exists but never installed a runtime probe).
    let before = point(StripId::new(0xDEAD_BEEF), SignalBoundary::Output);
    let obs = bridge.compare_probes(&compare_request(before, after));
    assert_eq!(
        incomparable_reason(&obs),
        &ProbeIncomparableReason::PointUnresolved {
            side: ProbeComparisonSide::Before
        }
    );
    assert!(obs.interval.is_none(), "no alignment was attempted");
}

#[test]
fn sf04_s7_binding_without_runtime_probe_is_not_bound() {
    let (mut bridge, c, before, _after) = monitored_pair();
    run_drain(&mut bridge, 4);

    // A strip id that does not exist in the project: the subscription
    // is accepted but resolves to no runtime probe — the binding
    // exists while `runtime_probe_id` stays `None`.
    let ghost = SemanticProbePoint {
        entity: SemanticSignalEntityRef::Strip(StripId::new(0xFFFF_FFFF)),
        boundary: SignalBoundary::Output,
        port: None,
    };
    let ghost_key = ProbeSubscriptionKey {
        session_id: ProjectSessionId(9),
        point: ghost,
        kind: SubscriptionKind::Metrics,
    };
    bridge
        .probe_request(c.live(), ghost_key, ProbeMode::Monitor)
        .unwrap();
    assert_eq!(
        bridge.probe_bindings()[&ghost_key].runtime_probe_id,
        None,
        "an unresolvable point never gets a runtime probe id"
    );

    let obs = bridge.compare_probes(&compare_request(before, ghost));
    assert_eq!(
        incomparable_reason(&obs),
        &ProbeIncomparableReason::ProbeNotBound {
            side: ProbeComparisonSide::After
        }
    );
}

#[test]
fn sf04_s7_bound_probe_without_records_has_no_observation_data() {
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
    // No run, no drain — the broker issued probe ids at request time
    // but no coverage evidence exists yet.

    let obs = bridge.compare_probes(&compare_request(before, after));
    assert_eq!(
        incomparable_reason(&obs),
        &ProbeIncomparableReason::NoObservationData {
            side: ProbeComparisonSide::Before
        }
    );
}

#[test]
fn sf04_s7_empty_observed_range_is_coverage_empty() {
    let (mut bridge, _c, before, after) = monitored_pair();
    run_drain(&mut bridge, 4);

    for range in [
        ProbeFrameInterval { start: 8, end: 8 },
        ProbeFrameInterval { start: 9, end: 8 },
    ] {
        let mut request = compare_request(before, after);
        request.observed_range = Some(range);
        let obs = bridge.compare_probes(&request);
        assert_eq!(
            incomparable_reason(&obs),
            &ProbeIncomparableReason::CoverageEmpty,
            "degenerate range {range:?} fails closed before evidence work"
        );
        assert!(obs.interval.is_none());
    }
}

#[test]
fn sf04_s7_evidence_budget_is_bounded() {
    let (mut bridge, _c, before, after) = monitored_pair();
    run_drain(&mut bridge, 8);

    // Both sides resolve — the budget check runs after resolution and
    // before alignment, and still reports the resolved window/gaps.
    let mut request = compare_request(before, after);
    request.max_bytes = 1;
    let obs = bridge.compare_probes(&request);
    assert_eq!(
        incomparable_reason(&obs),
        &ProbeIncomparableReason::EvidenceBudgetExceeded
    );
    assert!(obs.interval.is_some(), "resolved sides still report window");
}

// ---------------------------------------------------------------------------
// Incomparable — discontinuities on the cursor series
// ---------------------------------------------------------------------------

#[test]
fn sf04_s7_window_spanning_seek_is_discontinuity_in_window() {
    let (mut bridge, _c, before, after) = monitored_pair();
    run_drain(&mut bridge, 4);

    // Seek is a declared cursor-series discontinuity: the clock epoch
    // bumps and post-seek records carry it. A comparison window that
    // spans the boundary mixes clock marks — incomparable, never a
    // valid alignment.
    bridge.engine_mut().seek(1_024);
    run_drain(&mut bridge, 4);

    let obs = bridge.compare_probes(&compare_request(before, after));
    assert_eq!(
        incomparable_reason(&obs),
        &ProbeIncomparableReason::DiscontinuityInWindow,
        "a window spanning a seek must never align"
    );
}

#[test]
fn sf04_s7_sides_on_different_epochs_are_epoch_mismatch() {
    let mut c = controller();
    let (_t1, strip_a) = add_track_strip(&mut c, "a");
    let (_t2, strip_b) = add_track_strip(&mut c, "b");
    let mut bridge = ProbeTraceBridge::new(
        RoutingEngine::new(engine(), RuntimeInstanceId(1), RuntimeClockEpochRef(1)),
        BTreeMap::new(),
    );
    publish(bridge.engine_mut(), &c, 1);

    // `before` observes across the seek (its retained coverage
    // resolves to the first/oldest mark — the pre-seek epoch);
    // `after` is bound only after the seek and sees just the new
    // epoch. Same domain, different epochs, no anchor evidence →
    // EpochMismatch.
    let before = point(strip_a, SignalBoundary::Output);
    subscribe(&mut bridge, &c, 1, strip_a, SignalBoundary::Output);
    run_drain(&mut bridge, 4);

    bridge.engine_mut().seek(1_024);

    let after = point(strip_b, SignalBoundary::Output);
    subscribe(&mut bridge, &c, 2, strip_b, SignalBoundary::Output);
    run_drain(&mut bridge, 4);

    let obs = bridge.compare_probes(&compare_request(before, after));
    assert_eq!(
        incomparable_reason(&obs),
        &ProbeIncomparableReason::EpochMismatch,
        "pre-seek vs post-seek evidence cannot share a cursor basis"
    );
}

#[test]
fn sf04_s7_window_spanning_republish_is_generation_span() {
    let (mut bridge, c, before, after) = monitored_pair();
    run_drain(&mut bridge, 4);

    // Same project, explicit rebuild → a newer generation. The
    // semantic points do not move, so the broker reuses the runtime
    // probes and retained coverage legitimately spans both
    // generations.
    let binding_key = bridge
        .probe_bindings()
        .iter()
        .find(|(_, b)| b.point == before)
        .map(|(k, _)| *k)
        .expect("before point is bound");
    let id_before = bridge.probe_bindings()[&binding_key].runtime_probe_id;
    let generation = publish(bridge.engine_mut(), &c, 2);
    assert!(generation > 1, "republish mints a newer generation");
    assert_eq!(
        bridge.probe_bindings()[&binding_key].runtime_probe_id,
        id_before,
        "an unmoved point keeps its runtime probe across rebind"
    );
    run_drain(&mut bridge, 8);

    let obs = bridge.compare_probes(&compare_request(before, after));
    assert_eq!(
        incomparable_reason(&obs),
        &ProbeIncomparableReason::GenerationSpan,
        "a window crossing a plan-generation boundary is incomparable \
         under the fail-closed same-generation policy"
    );
}
