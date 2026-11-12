//! SF-04 slice 6 (Issue #107, SF-PROBE-07): multi-probe alignment
//! comparison — the `ProbeComparisonRequest → AcousticObservation`
//! consumer.
//!
//! [`probe_compare`] is a free, non-RT function: it takes the engine's
//! public read models (`probe_bindings()`/`probe_segments()`/
//! `probe_time_context()`/`probe_counts()` on
//! [`crate::routing_engine::RoutingEngine`]) plus the [`ProbeCoverage`]
//! evidence retained by the probe-trace drain path, builds one
//! [`ObservationSpan`] per side, and delegates the three-valued
//! verdict to Ruviform #309's [`align_observation_points`]. Results
//! are mapped into the `synvelis_project::probe` contract vocabulary
//! — runtime ids (`ProbeId`, `NodeId`, `PortId`) never cross it.
//!
//! Fail-closed rules owned here (everything else is the runtime's
//! own fail-closed semantics):
//!
//! - A semantic point with no binding → `PointUnresolved`; bindings
//!   without an installed runtime probe → `ProbeNotBound`; a resolved
//!   window with no `Data` records → `NoObservationData`. Multiple
//!   subscriptions on one point are tried in binding order — the
//!   first side that yields an honest span wins.
//! - In-window records disagreeing on channel count or sample rate →
//!   `InconsistentFormat`; channel counts without a canonical layout
//!   → `UnsupportedChannelLayout` (the runtime record carries no
//!   speaker mask — a layout is never invented).
//! - `observed_range` bounds the comparison absolutely: regions the
//!   request covers but no side observed land in `gap_mask` as
//!   `Unknown` and demote the result to `Conditional{PartialCoverage}`
//!   — never silently clipped into an `Aligned` claim.
//! - `max_bytes` bounds the retained evidence fed to alignment —
//!   exceeding it fails closed *before* `align_observation_points`.
//! - Telemetry loss is localized per probe from the drain-batch
//!   `lost_records` floor recorded with each event; a positive delta
//!   downgrades reconstructed coverage to `Unknown` (runtime rule),
//!   which surfaces as gaps/`CoverageEmpty`, never as validity.
//!
//! No PCM is correlated, resampled, or fabricated; no RT path is
//! touched — this module is control-plane only.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ruviform_audio::typed::probe::{
    AlignPolicy, Alignment, AlignmentBasis, AlignmentCondition, CoverageClass, CoverageEvent,
    CoverageInterval, FrameInterval, GenerationRange, IncomparableReason, LatencyKnowledge,
    ObservationCoverage, ObservationFormat, ObservationSpan, ProbeClockMark, ProbeCounts, ProbeId,
    ProbePoint, ProbePrecision, ProbeTimeContext, align_observation_points,
};
use ruviform_audio_core::ChannelLayout;

use synvelis_project::ProbeSubscriptionKey;
use synvelis_project::probe::{
    AcousticObservation, ClockContextRef, ObservationGapKind, ProbeAlignCondition,
    ProbeAlignPolicy, ProbeCaptureSegment, ProbeComparisonBasis, ProbeComparisonGap,
    ProbeComparisonProvenance, ProbeComparisonQuality, ProbeComparisonRequest, ProbeComparisonSide,
    ProbeFrameInterval, ProbeGenerationRange, ProbeIncomparableReason, SemanticProbeBinding,
    SemanticProbePoint,
};

use crate::probe::clock_ref;

/// Bound on retained coverage events per runtime probe — oldest are
/// evicted first. Sized for tens of seconds of per-block telemetry at
/// typical block sizes; eviction degrades coverage to honest `Unknown`
/// edges rather than fabricating validity.
pub const PROBE_COVERAGE_MAX_EVENTS: usize = 2048;

/// One retained coverage event plus the telemetry-loss counter value
/// observed *before* the drain batch that produced it — the floor for
/// the loss delta covering this event's retained window.
#[derive(Copy, Clone)]
struct RetainedEvent {
    loss_floor: u64,
    event: CoverageEvent,
}

/// Per-probe retained coverage series.
#[derive(Default)]
struct SeriesCoverage {
    /// Broker-issued id recovered from the first retained event —
    /// `ProbeId` has no public constructor, so it is echoed back out
    /// of the evidence itself.
    probe: Option<ProbeId>,
    /// Events in drain order (oldest first).
    events: VecDeque<RetainedEvent>,
}

impl SeriesCoverage {
    /// Records `event`, evicting the oldest entries past
    /// [`PROBE_COVERAGE_MAX_EVENTS`].
    fn push(&mut self, probe: ProbeId, loss_floor: u64, event: CoverageEvent) {
        self.probe = Some(probe);
        self.events.push_back(RetainedEvent { loss_floor, event });
        while self.events.len() > PROBE_COVERAGE_MAX_EVENTS {
            self.events.pop_front();
        }
    }
}

/// Bounded per-probe [`CoverageEvent`] evidence retained across
/// drains — the coverage-reconstruction input for [`probe_compare`].
///
/// Fed by the probe-trace drain path (the single
/// `ProbeTracePort::probe_drain` owner); direct
/// `RoutingEngine::probe_drain` calls bypass it by design.
#[derive(Default)]
pub struct ProbeCoverage {
    /// Raw broker probe id → retained series.
    series: BTreeMap<u64, SeriesCoverage>,
}

impl ProbeCoverage {
    /// Records one drained coverage event. `loss_floor` is the
    /// `ProbeCounts::lost_records` value sampled *before* the drain
    /// batch that produced `event`.
    pub fn record(&mut self, event: CoverageEvent, loss_floor: u64) {
        let probe = match event {
            CoverageEvent::Data(meta) => meta.probe,
            CoverageEvent::Status(status) => status.probe,
            // Future non-coverage variants carry no span evidence.
            _ => return,
        };
        self.series
            .entry(probe.get())
            .or_default()
            .push(probe, loss_floor, event);
    }

    /// Retained events for raw probe `probe` in drain order.
    pub fn events(&self, probe: u64) -> impl Iterator<Item = CoverageEvent> + '_ {
        self.series
            .get(&probe)
            .into_iter()
            .flat_map(|series| series.events.iter())
            .map(|retained| retained.event)
    }

    /// Number of retained events for raw probe `probe`.
    #[must_use]
    pub fn retained_len(&self, probe: u64) -> usize {
        self.series.get(&probe).map_or(0, |s| s.events.len())
    }

    /// Retained series keys (raw broker probe ids) — diagnostic
    /// surface for tests and future session reporting.
    pub fn probes(&self) -> impl Iterator<Item = u64> + '_ {
        self.series.keys().copied()
    }

    /// Drops every series whose raw probe id fails `keep` — invoked
    /// after each drain so removed or rebound-away probes stop
    /// accumulating stale evidence.
    pub fn retain(&mut self, keep: impl Fn(u64) -> bool) {
        self.series.retain(|probe, _| keep(*probe));
    }

    /// Whether no series are retained.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.series.is_empty()
    }

    /// Reconstructs [`ObservationCoverage`] for `probe` plus the
    /// broker id and retained-event count the span needs. `counts` is
    /// the *current* collector snapshot — the loss delta is measured
    /// from the oldest retained event's drain-batch floor, so losses
    /// anywhere inside the retained window downgrade coverage to
    /// `Unknown` (never a guessed validity).
    fn reconstruct(
        &self,
        probe: u64,
        counts: ProbeCounts,
    ) -> Option<(ProbeId, ObservationCoverage, usize)> {
        let series = self.series.get(&probe)?;
        let probe_id = series.probe?;
        let retained = series.events.len();
        if retained == 0 {
            return None;
        }
        let floor = series.events.front().map_or(0, |r| r.loss_floor);
        let lost = counts.lost_records.saturating_sub(floor);
        let events = series.events.iter().map(|r| r.event);
        let coverage = ObservationCoverage::reconstruct(probe_id, lost, events);
        Some((probe_id, coverage, retained))
    }
}

/// One side's resolved comparison evidence — everything
/// [`ObservationSpan`] needs plus the retained-event count for the
/// byte-budget check.
struct SideEvidence {
    /// Emitting probe.
    probe: ProbeId,
    /// Observed runtime point.
    point: ProbePoint,
    /// Clock mark covering the compared window.
    clock_mark: ProbeClockMark,
    /// Compared cursor-frame window.
    window: FrameInterval,
    /// Plan generations the in-window records covered.
    generation_range: GenerationRange,
    /// Declared format (consistent across in-window data records).
    format: ObservationFormat,
    /// Latency knowledge across in-window data records.
    latency: LatencyKnowledge,
    /// Reconstructed coverage for the whole retained series.
    coverage: ObservationCoverage,
    /// Retained event count (budget estimate input).
    retained_events: usize,
}

/// Half-open overlap test in cursor coordinates.
fn overlaps(start: u64, end: u64, window: &FrameInterval) -> bool {
    start < window.end && end > window.start
}

/// The `(start, end)` a coverage event occupies — data spans its
/// frames, status events mark one position.
fn event_extent(event: &CoverageEvent) -> Option<(u64, u64)> {
    match *event {
        CoverageEvent::Data(meta) => Some((
            meta.sample_position,
            meta.sample_position.saturating_add(u64::from(meta.frames)),
        )),
        CoverageEvent::Status(status) => Some((
            status.sample_position,
            status.sample_position.saturating_add(1),
        )),
        _ => None,
    }
}

/// Evidence extent `[min_start, max_end)` — reconstructed coverage
/// intervals unioned with matching capture segments' frame ranges.
/// Segment ranges corroborate: they persist after coverage eviction
/// and may legitimately extend the extent (uncovered parts then
/// classify as `Unknown` gaps, never as validity).
fn evidence_extent(
    coverage: &ObservationCoverage,
    point: &SemanticProbePoint,
    segments: &[ProbeCaptureSegment],
) -> Option<(u64, u64)> {
    let mut lo = u64::MAX;
    let mut hi = 0_u64;
    let mut any = false;
    for interval in &coverage.intervals {
        any = true;
        lo = lo.min(interval.start);
        hi = hi.max(interval.end);
    }
    for segment in segments.iter().filter(|s| s.point == *point) {
        if let Some(range) = segment.frame_range {
            any = true;
            lo = lo.min(range.start);
            hi = hi.max(range.end);
        }
    }
    any.then_some((lo, hi))
}

/// Clock mark for `window`: the first coverage interval overlapping
/// it (mixed marks inside the window are the runtime's
/// `DiscontinuityInWindow` evidence), else the temporally nearest
/// retained interval's mark — the series' best-known identity near
/// an unobserved window. `UNKNOWN` only when no interval exists at
/// all (fail-closed: the mark is then genuinely unnameable).
fn mark_for_window(coverage: &ObservationCoverage, window: &FrameInterval) -> ProbeClockMark {
    if let Some(mark) = coverage
        .intervals
        .iter()
        .find(|iv| overlaps(iv.start, iv.end, window))
        .map(|iv| iv.mark)
    {
        return mark;
    }
    coverage
        .intervals
        .iter()
        .min_by_key(|iv| {
            let distance = if iv.end <= window.start {
                window.start - iv.end
            } else {
                iv.start.saturating_sub(window.end)
            };
            (distance, iv.start)
        })
        .map_or(ProbeClockMark::UNKNOWN, |iv| iv.mark)
}

/// Canonical layout for observed channel counts. The runtime record
/// carries no speaker mask, so only unambiguous counts map; anything
/// else fails closed rather than inventing a layout.
fn layout_for_channels(channels: u16) -> Option<ChannelLayout> {
    match channels {
        1 => Some(ChannelLayout::Mono),
        2 => Some(ChannelLayout::Stereo),
        4 => Some(ChannelLayout::Quad),
        6 => Some(ChannelLayout::FivePointOne),
        8 => Some(ChannelLayout::SevenPointOne),
        _ => None,
    }
}

/// One resolved runtime probe's retained evidence: the series id,
/// its reconstructed coverage, the retained event count feeding the
/// byte budget, and the raw retained events driving the in-window
/// format/latency/generation scan.
struct ResolvedProbe<'a> {
    probe: ProbeId,
    coverage: &'a ObservationCoverage,
    retained_events: usize,
    events: &'a [CoverageEvent],
}

/// Builds one side's [`SideEvidence`] from a resolved probe's
/// retained events — or the consumer-side reason it cannot be built
/// honestly.
fn build_side(
    resolved: &ResolvedProbe<'_>,
    observed_range: Option<ProbeFrameInterval>,
    segments: &[ProbeCaptureSegment],
    point: &SemanticProbePoint,
    side: ProbeComparisonSide,
) -> Result<SideEvidence, ProbeIncomparableReason> {
    // Compared window: the requested range verbatim (uncovered parts
    // become gaps, not silent clipping), else the evidence extent.
    let window = match observed_range {
        Some(range) => FrameInterval::new(range.start, range.end),
        None => evidence_extent(resolved.coverage, point, segments).map_or_else(
            || FrameInterval::new(0, 0),
            |(lo, hi)| FrameInterval::new(lo, hi),
        ),
    };
    if window.is_empty() {
        return Err(ProbeIncomparableReason::CoverageEmpty);
    }

    // In-window evidence: data records drive format/latency/point;
    // every event drives the generation range.
    let mut has_data = false;
    let mut channels = BTreeSet::new();
    let mut rates = BTreeSet::new();
    let mut latencies = BTreeSet::new();
    let mut generations = Vec::new();
    let mut rt_point = None;
    for event in resolved.events {
        let Some((start, end)) = event_extent(event) else {
            continue;
        };
        if !overlaps(start, end, &window) {
            continue;
        }
        match *event {
            CoverageEvent::Data(meta) => {
                has_data = true;
                channels.insert(meta.channels);
                rates.insert(meta.sample_rate);
                latencies.insert(meta.effective_latency);
                rt_point.get_or_insert(ProbePoint {
                    node: meta.node,
                    port: meta.port,
                });
                generations.push(meta.generation);
            }
            CoverageEvent::Status(status) => generations.push(status.generation),
            _ => {}
        }
    }
    if !has_data {
        return Err(ProbeIncomparableReason::NoObservationData { side });
    }
    let Some(rt_point) = rt_point else {
        return Err(ProbeIncomparableReason::NoObservationData { side });
    };
    let (channel_count, sample_rate) = match (channels.iter().next(), rates.iter().next()) {
        (Some(&c), Some(&r)) if channels.len() == 1 && rates.len() == 1 => (c, r),
        _ => {
            return Err(ProbeIncomparableReason::InconsistentFormat { side });
        }
    };
    let channel_layout = layout_for_channels(channel_count).ok_or(
        ProbeIncomparableReason::UnsupportedChannelLayout {
            side,
            channels: channel_count,
        },
    )?;
    let latency = if latencies.len() == 1 {
        LatencyKnowledge::Known {
            frames: u64::from(latencies.iter().next().copied().unwrap_or_default()),
        }
    } else {
        // Latency changed inside the window — no single honest value.
        LatencyKnowledge::Unknown
    };
    let first = generations.iter().copied().min().unwrap_or(0);
    let last = generations.iter().copied().max().unwrap_or(0);
    Ok(SideEvidence {
        probe: resolved.probe,
        point: rt_point,
        clock_mark: mark_for_window(resolved.coverage, &window),
        window,
        generation_range: GenerationRange { first, last },
        format: ObservationFormat {
            channels: channel_count,
            sample_rate,
            channel_layout,
        },
        latency,
        coverage: resolved.coverage.clone(),
        retained_events: resolved.retained_events,
    })
}

/// Resolves one semantic point to comparison evidence: finds the
/// subscription bindings for `point`, picks the first installed
/// runtime probe whose retained coverage yields an honest span, and
/// reports the first consumer-side failure when none does.
fn resolve_side(
    point: &SemanticProbePoint,
    observed_range: Option<ProbeFrameInterval>,
    segments: &[ProbeCaptureSegment],
    bindings: &BTreeMap<ProbeSubscriptionKey, SemanticProbeBinding>,
    coverage: &ProbeCoverage,
    counts: ProbeCounts,
    side: ProbeComparisonSide,
) -> Result<SideEvidence, ProbeIncomparableReason> {
    let mut matched = false;
    let mut any_bound = false;
    let mut first_failure = None;
    for binding in bindings.values().filter(|b| b.point == *point) {
        matched = true;
        let Some(raw) = binding.runtime_probe_id.map(|id| id.0) else {
            continue;
        };
        any_bound = true;
        let events: Vec<CoverageEvent> = coverage.events(raw).collect();
        let Some((probe_id, reconstructed, retained)) = coverage.reconstruct(raw, counts) else {
            first_failure.get_or_insert(ProbeIncomparableReason::NoObservationData { side });
            continue;
        };
        match build_side(
            &ResolvedProbe {
                probe: probe_id,
                coverage: &reconstructed,
                retained_events: retained,
                events: &events,
            },
            observed_range,
            segments,
            point,
            side,
        ) {
            Ok(evidence) => return Ok(evidence),
            Err(reason) => {
                first_failure.get_or_insert(reason);
            }
        }
    }
    if !matched {
        return Err(ProbeIncomparableReason::PointUnresolved { side });
    }
    if !any_bound {
        return Err(ProbeIncomparableReason::ProbeNotBound { side });
    }
    Err(first_failure.unwrap_or(ProbeIncomparableReason::NoObservationData { side }))
}

/// `CoverageClass` → contract [`ObservationGapKind`] — future runtime
/// variants land on `Unknown`, never on a guessed cause.
fn gap_kind(class: CoverageClass) -> ObservationGapKind {
    match class {
        CoverageClass::Unobserved { .. } => ObservationGapKind::Unobserved,
        CoverageClass::LostBlocks => ObservationGapKind::LostBlocks,
        CoverageClass::LostRecords => ObservationGapKind::LostRecords,
        CoverageClass::GenerationBoundary => ObservationGapKind::GenerationBoundary,
        CoverageClass::RebindDropped => ObservationGapKind::RebindDropped,
        CoverageClass::Off => ObservationGapKind::Off,
        CoverageClass::Unsupported => ObservationGapKind::Unsupported,
        CoverageClass::Unavailable => ObservationGapKind::Unavailable,
        _ => ObservationGapKind::Unknown,
    }
}

/// Per-side classified non-valid coverage inside the side's window:
/// reconstructed non-`Valid` intervals plus `Unknown` for window
/// regions outside all retained coverage (the request bounds the
/// comparison; unobserved request regions are gaps, not silence).
fn side_gaps(evidence: &SideEvidence, side: ProbeComparisonSide) -> Vec<ProbeComparisonGap> {
    let window = evidence.window;
    let mut gaps = Vec::new();
    let mut covered: Vec<(u64, u64)> = Vec::new();
    for interval in &evidence.coverage.intervals {
        let start = interval.start.max(window.start);
        let end = interval.end.min(window.end);
        if start >= end {
            continue;
        }
        covered.push((start, end));
        if interval.class != CoverageClass::Valid {
            gaps.push(ProbeComparisonGap {
                side,
                range: ProbeFrameInterval { start, end },
                kind: gap_kind(interval.class),
            });
        }
    }
    covered.sort_unstable();
    let mut cursor = window.start;
    for (start, end) in covered {
        if start > cursor {
            gaps.push(ProbeComparisonGap {
                side,
                range: ProbeFrameInterval {
                    start: cursor,
                    end: start.min(window.end),
                },
                kind: ObservationGapKind::Unknown,
            });
        }
        cursor = cursor.max(end);
        if cursor >= window.end {
            break;
        }
    }
    if cursor < window.end {
        gaps.push(ProbeComparisonGap {
            side,
            range: ProbeFrameInterval {
                start: cursor,
                end: window.end,
            },
            kind: ObservationGapKind::Unknown,
        });
    }
    gaps
}

/// Runtime [`AlignmentBasis`] → contract [`ProbeComparisonBasis`].
fn map_basis(basis: &AlignmentBasis) -> ProbeComparisonBasis {
    match *basis {
        AlignmentBasis::SharedCursor { domain, epoch } => ProbeComparisonBasis::SharedCursor {
            clock: ClockContextRef { domain, epoch },
        },
        AlignmentBasis::EngineFrame { epoch } => ProbeComparisonBasis::EngineFrame { epoch },
        _ => ProbeComparisonBasis::Unknown,
    }
}

/// Runtime [`AlignmentCondition`] → contract [`ProbeAlignCondition`].
fn map_condition(condition: &AlignmentCondition) -> ProbeAlignCondition {
    match *condition {
        AlignmentCondition::LatencyBounded { bound_frames } => {
            ProbeAlignCondition::LatencyBounded { bound_frames }
        }
        AlignmentCondition::ProvenanceEstimated => ProbeAlignCondition::ProvenanceEstimated,
        AlignmentCondition::PartialCoverage => ProbeAlignCondition::PartialCoverage,
        AlignmentCondition::CrossGeneration => ProbeAlignCondition::CrossGeneration,
        AlignmentCondition::MixedPrecision => ProbeAlignCondition::MixedPrecision,
        _ => ProbeAlignCondition::Unknown,
    }
}

/// Runtime [`IncomparableReason`] → contract [`ProbeIncomparableReason`]
/// — the `AnchorRejected` payload stays on the runtime result.
fn map_reason(reason: &IncomparableReason) -> ProbeIncomparableReason {
    match *reason {
        IncomparableReason::UnknownClock => ProbeIncomparableReason::UnknownClock,
        IncomparableReason::DomainMismatch => ProbeIncomparableReason::DomainMismatch,
        IncomparableReason::EpochMismatch => ProbeIncomparableReason::EpochMismatch,
        IncomparableReason::SeriesMismatch => ProbeIncomparableReason::SeriesMismatch,
        IncomparableReason::RateMismatch { a, b } => ProbeIncomparableReason::RateMismatch { a, b },
        IncomparableReason::FormatMismatch => ProbeIncomparableReason::FormatMismatch,
        IncomparableReason::LatencyUnknown => ProbeIncomparableReason::LatencyUnknown,
        IncomparableReason::CoverageEmpty => ProbeIncomparableReason::CoverageEmpty,
        IncomparableReason::DiscontinuityInWindow => ProbeIncomparableReason::DiscontinuityInWindow,
        IncomparableReason::GenerationSpan => ProbeIncomparableReason::GenerationSpan,
        IncomparableReason::AnchorMissing => ProbeIncomparableReason::AnchorMissing,
        IncomparableReason::AnchorRejected { .. } => ProbeIncomparableReason::AnchorRejected,
        IncomparableReason::Overflow => ProbeIncomparableReason::Overflow,
        IncomparableReason::ErrorBudgetExceeded => ProbeIncomparableReason::ErrorBudgetExceeded,
        IncomparableReason::EstimatedProvenance => ProbeIncomparableReason::EstimatedProvenance,
        _ => ProbeIncomparableReason::Unknown,
    }
}

/// Runtime [`FrameInterval`] → contract [`ProbeFrameInterval`].
fn contract_interval(interval: FrameInterval) -> ProbeFrameInterval {
    ProbeFrameInterval {
        start: interval.start,
        end: interval.end,
    }
}

/// Runtime [`AlignPolicy`] mirror → the real runtime policy.
fn runtime_policy(policy: &ProbeAlignPolicy) -> AlignPolicy {
    AlignPolicy {
        require_same_generation: policy.require_same_generation,
        require_known_latency: policy.require_known_latency,
        allow_rate_conversion: policy.allow_rate_conversion,
        unknown_latency_bound_frames: policy.unknown_latency_bound_frames,
        max_error_frames: policy.max_error_frames,
        allow_conditional: policy.allow_conditional,
        prefer_engine_frame: policy.prefer_engine_frame,
    }
}

/// Builds the two [`ObservationSpan`]s and delegates the verdict to
/// Ruviform [`align_observation_points`], mapping the runtime
/// [`Alignment`] onto the contract [`ProbeComparisonQuality`].
fn align_sides(
    before: &SideEvidence,
    after: &SideEvidence,
    request: &ProbeComparisonRequest,
    time_context: &ProbeTimeContext,
) -> ProbeComparisonQuality {
    let span_before = ObservationSpan {
        probe: before.probe,
        point: before.point,
        clock_mark: before.clock_mark,
        interval: before.window,
        generation_range: before.generation_range,
        format: before.format,
        latency: before.latency,
        coverage: &before.coverage,
        context: Some(time_context),
        precision: ProbePrecision::F32Rt,
    };
    let span_after = ObservationSpan {
        probe: after.probe,
        point: after.point,
        clock_mark: after.clock_mark,
        interval: after.window,
        generation_range: after.generation_range,
        format: after.format,
        latency: after.latency,
        coverage: &after.coverage,
        context: Some(time_context),
        precision: ProbePrecision::F32Rt,
    };
    match align_observation_points(
        &span_before,
        &span_after,
        &runtime_policy(&request.alignment_policy),
    ) {
        Alignment::Aligned {
            basis,
            offset,
            error_budget,
            ..
        } => ProbeComparisonQuality::Aligned {
            basis: map_basis(&basis),
            offset_frames: offset,
            error_budget_frames: error_budget,
        },
        Alignment::Conditional {
            basis,
            offset,
            error_budget,
            conditions,
            excluded_intervals,
        } => ProbeComparisonQuality::Conditional {
            basis: map_basis(&basis),
            offset_frames: offset,
            error_budget_frames: error_budget,
            conditions: conditions.iter().map(map_condition).collect(),
            excluded_intervals: excluded_intervals
                .iter()
                .map(|iv| contract_interval(*iv))
                .collect(),
        },
        Alignment::Incomparable { reason } => ProbeComparisonQuality::Incomparable {
            reason: map_reason(&reason),
        },
        // Future runtime variants fail closed rather than being
        // silently promoted.
        _ => ProbeComparisonQuality::Incomparable {
            reason: ProbeIncomparableReason::Unknown,
        },
    }
}

/// Assembles the contract result. `interval`/`gap_mask` are present
/// iff both sides resolved to evidence spans (alignment was
/// attempted); provenance fields are `None` for unresolved sides.
fn observation(
    request: &ProbeComparisonRequest,
    time_context: &ProbeTimeContext,
    before: Option<&SideEvidence>,
    after: Option<&SideEvidence>,
    quality: ProbeComparisonQuality,
) -> AcousticObservation {
    let mut gap_mask = Vec::new();
    if let Some(evidence) = before {
        gap_mask.extend(side_gaps(evidence, ProbeComparisonSide::Before));
    }
    if let Some(evidence) = after {
        gap_mask.extend(side_gaps(evidence, ProbeComparisonSide::After));
    }
    let generation_range = |evidence: Option<&SideEvidence>| {
        evidence.map(|e| ProbeGenerationRange {
            first: e.generation_range.first,
            last: e.generation_range.last,
        })
    };
    AcousticObservation {
        before: request.before,
        after: request.after,
        quality,
        interval: after.map(|e| contract_interval(e.window)),
        gap_mask,
        provenance: ProbeComparisonProvenance {
            before_clock: before.and_then(|e| clock_ref(e.clock_mark)),
            after_clock: after.and_then(|e| clock_ref(e.clock_mark)),
            before_generations: generation_range(before),
            after_generations: generation_range(after),
            runtime_instance: time_context.runtime_instance,
            sample_rate: time_context.sample_rate,
        },
    }
}

/// Compares two semantic probe points — the SF-PROBE-07 headless
/// consumer. Non-RT, bounded by [`PROBE_COVERAGE_MAX_EVENTS`] per
/// probe plus the request's `max_bytes` evidence budget; never
/// resamples, never correlates PCM, never treats unknown clocks,
/// latency, epochs, generations or coverage as aligned.
///
/// `bindings`/`segments`/`time_context`/`counts` are the engine's
/// public read models; `coverage` is the [`ProbeCoverage`] retained
/// by the drain path. The result is the contract
/// [`AcousticObservation`] — `Aligned`/`Conditional`/`Incomparable`
/// with basis, offset, error budget, gap mask and provenance, or an
/// explicit consumer-side reason when a side cannot be resolved.
#[must_use]
pub fn probe_compare(
    request: &ProbeComparisonRequest,
    bindings: &BTreeMap<ProbeSubscriptionKey, SemanticProbeBinding>,
    segments: &[ProbeCaptureSegment],
    time_context: &ProbeTimeContext,
    coverage: &ProbeCoverage,
    counts: ProbeCounts,
) -> AcousticObservation {
    // A degenerate requested window fails closed before any evidence
    // work — an empty span can never be aligned.
    if let Some(range) = request.observed_range
        && range.start >= range.end
    {
        return observation(
            request,
            time_context,
            None,
            None,
            ProbeComparisonQuality::Incomparable {
                reason: ProbeIncomparableReason::CoverageEmpty,
            },
        );
    }

    let before = resolve_side(
        &request.before,
        request.observed_range,
        segments,
        bindings,
        coverage,
        counts,
        ProbeComparisonSide::Before,
    );
    let after = resolve_side(
        &request.after,
        request.observed_range,
        segments,
        bindings,
        coverage,
        counts,
        ProbeComparisonSide::After,
    );
    let (before, after) = match (before, after) {
        (Ok(b), Ok(a)) => (b, a),
        // Request order decides which side's failure is reported.
        (Err(reason), _) | (_, Err(reason)) => {
            return observation(
                request,
                time_context,
                None,
                None,
                ProbeComparisonQuality::Incomparable { reason },
            );
        }
    };

    // Evidence budget: retained events plus reconstructed intervals
    // fed to alignment — checked before align runs.
    let estimate = (before.retained_events + after.retained_events)
        .saturating_mul(std::mem::size_of::<CoverageEvent>())
        .saturating_add(
            (before.coverage.intervals.len() + after.coverage.intervals.len())
                .saturating_mul(std::mem::size_of::<CoverageInterval>()),
        );
    if u64::try_from(estimate).unwrap_or(u64::MAX) > request.max_bytes {
        return observation(
            request,
            time_context,
            Some(&before),
            Some(&after),
            ProbeComparisonQuality::Incomparable {
                reason: ProbeIncomparableReason::EvidenceBudgetExceeded,
            },
        );
    }

    let quality = align_sides(&before, &after, request, time_context);
    observation(request, time_context, Some(&before), Some(&after), quality)
}
