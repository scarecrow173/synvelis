//! TIM-03 plugin process boundary — capability classification,
//! degraded-consent policy and offline-exact fail-closed evaluation
//! (issue #127, headless mock slice).
//!
//! Synvelis owns the *consumer-side* precision policy; Ruviform owns
//! the generic machinery this module consumes unchanged:
//! [`BlockContract`]/[`CallPartition`] legality
//! (`legal_span_len`/`partition`), the `TransportScheduler` span
//! splitting and Exact/Degraded apply semantics, and the
//! `PluginNode` physical-call path. This module is the non-RT
//! preflight oracle the composition root consults *before* choosing
//! `AttachConfig::policy`/`process_config` and before staging
//! transport commands:
//!
//! - [`ProcessCapability`] — one plugin instance's process contract
//!   (`min_frames`/`max_frames`/in-block transport support, derived
//!   from the activated [`ProcessConfig`]) plus its certified
//!   [`TimingPrecision`]. Unmeasured precision stays
//!   [`TimingPrecision::Unknown`] — never upgraded by assumption.
//! - [`ProcessBoundaryRequest`]/[`ProcessCapability::evaluate`] —
//!   splits a callback window at discontinuity offsets (loop wrap /
//!   seek / plan install), checks every resulting logical segment
//!   against the block contract, and returns a
//!   [`ProcessBoundaryPlan`] (policy, achieved precision, per-segment
//!   [`CallPartition`]s and deferred boundaries) or a typed
//!   [`ProcessBoundaryError`].
//! - [`DegradedConsent`] — the explicit consent token required before
//!   [`DeliveryPolicy::Degraded`] may be selected for realtime
//!   delivery. There is no `Default`; the only mint path is
//!   [`DegradedConsent::grant`], and the granted `max_deferral`
//!   bounds how far a discontinuity may move.
//!
//! Policy contract (issue #127 / architecture doc §14.2):
//!
//! 1. Exact delivery — every segment legally partitionable — runs
//!    [`DeliveryPolicy::Exact`]. Offline exact render additionally
//!    requires a *certified* exact-class precision on the capability;
//!    `Unknown`/`BlockBoundaryOnly`/`DegradedWithConsent`/
//!    `Unsupported` fail closed with
//!    [`ProcessBoundaryError::OfflineExact`] or
//!    [`ProcessBoundaryError::OfflineUncertified`].
//! 2. An illegal segment under realtime is never silently quantized:
//!    without [`DegradedConsent`] the request is
//!    [`ProcessBoundaryError::ConsentRequired`]; with consent the
//!    boundary defers to the next frame where both adjacent segments
//!    are legal (bounded by `min(consent.max_deferral,
//!    budget.max_deferral)`), recorded as [`BoundaryDegradation`]. No
//!    legal deferred frame → [`ProcessBoundaryError::DeferralExceeded`].
//! 3. A capability certified [`TimingPrecision::Unsupported`] refuses
//!    delivery outright — typed, never a fake success.
//! 4. Zero illegal process lengths is a *verification* property, not
//!    a plan property: tests drive the real scheduler/node and assert
//!    every physical call satisfies `min_frames <= frames <=
//!    max_frames`.
//!
//! RT contract: [`ProcessCapability::evaluate`] is non-RT
//! orchestration (allocates the plan). Nothing in this module runs on
//! the audio path; the RT delivery path is upstream and covered by
//! `rt_` contract tests.

use std::fmt;
use std::sync::Arc;

use ruviform_audio_plugin_bridge::{FrameBaseMismatch, InstallPlanError, PluginNodeControl};
use ruviform_graph::NodeId;
use ruviform_media_core::{
    BlockContract, CallPartition, DeliveryPolicy, InBlockTransportSupport, MidiEvent,
    ScheduleBudget, TransportCommand, TransportPlan,
};
use ruviform_plugin_core::{
    EventKind, ProcessConfig, TimedParamEvent, contract_from_process_config,
};
use synvelis_project::ParameterTargetId;

use crate::binding::{AttachClockContext, StablePluginInstanceId};
use crate::dispatch::{DispatchError, ParamDispatch};

/// Issue #127 timing-precision classification — the vocabulary the
/// capability matrix and this policy share.
///
/// `ExactWithinVerifiedCapability` and `FrameAccurateEvents` are the
/// *certified exact* classes: only they satisfy an
/// [`ExecutionMode::OfflineExact`] request. `DegradedWithConsent` is
/// a delivery mode, not a precision claim — it is produced only by a
/// consented degraded plan. `Unknown` is the honest state of an
/// unmeasured capability and never upgrades itself.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TimingPrecision {
    /// Delivery is exact within what the plugin's verified
    /// capability supports (verified in-block transport path).
    ExactWithinVerifiedCapability,
    /// Events are frame-accurate (`EventList` offsets) and transport
    /// context is exact at every physical call boundary.
    FrameAccurateEvents,
    /// Only block-boundary granularity is certified.
    BlockBoundaryOnly,
    /// Degraded delivery — precision loss is explicit and consented.
    DegradedWithConsent,
    /// The capability cannot support the requested delivery.
    Unsupported,
    /// Precision not measured — never treated as supported.
    Unknown,
}

impl TimingPrecision {
    /// Whether this class certifies exact (non-degraded) delivery for
    /// [`ExecutionMode::OfflineExact`].
    #[must_use]
    pub const fn certified_exact(self) -> bool {
        matches!(
            self,
            Self::ExactWithinVerifiedCapability | Self::FrameAccurateEvents
        )
    }
}

/// Execution intent for a process-boundary request.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ExecutionMode {
    /// Live/realtime delivery — degraded precision is permitted only
    /// with explicit [`DegradedConsent`].
    Realtime,
    /// Offline exact render — inexact or uncertified delivery fails
    /// closed; degraded is never accepted.
    OfflineExact,
}

/// Explicit user consent to degraded delivery precision.
///
/// Minted only by [`Self::grant`]: the type cannot be defaulted,
/// forged from a literal, or produced accidentally — selecting
/// `DeliveryPolicy::Degraded` requires presenting one to
/// [`ProcessCapability::evaluate`]. `max_deferral` is the consented
/// bound on how many frames a discontinuity may move; the effective
/// bound is `min(consent.max_deferral, budget.max_deferral)`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DegradedConsent {
    max_deferral: u32,
}

impl DegradedConsent {
    /// Records explicit consent to degraded delivery within
    /// `max_deferral` frames of boundary displacement.
    #[must_use]
    pub const fn grant(max_deferral: u32) -> Self {
        Self { max_deferral }
    }

    /// The consented deferral bound in frames.
    #[must_use]
    pub const fn max_deferral(self) -> u32 {
        self.max_deferral
    }
}

/// The kind of transport discontinuity a boundary offset carries —
/// scenario metadata for plans/receipts; legality is identical for
/// every kind (a boundary is a half-open span split).
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BoundaryKind {
    /// Loop wrap — position returns to the loop start, occurrence +1.
    LoopWrap,
    /// Seek apply — position jumps; transport generation +1.
    Seek,
    /// Plan install — new plan/tempo-map revision boundary.
    PlanInstall,
    /// Any other transport command boundary (play/pause/stop/loop set).
    Other,
}

/// One requested discontinuity at `offset` frames inside the callback
/// window. `offset == 0` applies at the callback head (no span is
/// emitted for it); `offset` must be `< window_frames`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BoundaryPoint {
    /// Window-relative frame offset of the discontinuity.
    pub offset: u32,
    /// What kind of discontinuity this is.
    pub kind: BoundaryKind,
}

/// A process-boundary evaluation request: one callback window, its
/// discontinuities, the execution intent and the consent/budget the
/// policy may use.
#[derive(Clone, Debug)]
pub struct ProcessBoundaryRequest {
    /// Callback window length in frames (e.g. 100/128/256).
    pub window_frames: u32,
    /// Discontinuity points inside the window. Same-offset points
    /// coalesce into one boundary (matching the scheduler's
    /// same-frame apply semantics); order is normalized by
    /// [`ProcessCapability::evaluate`].
    pub boundaries: Vec<BoundaryPoint>,
    /// Realtime vs offline-exact intent.
    pub mode: ExecutionMode,
    /// Explicit degraded consent — required iff any segment is
    /// illegal under the block contract.
    pub consent: Option<DegradedConsent>,
    /// Bounded-work budget; `max_deferral` caps degraded boundary
    /// displacement.
    pub budget: ScheduleBudget,
}

/// One logical segment of the evaluated window and its legal
/// physical-call partition. Under a degraded plan `start`/`end` are
/// the *applied* (post-deferral) boundaries.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SegmentPlan {
    /// Segment start, window-relative.
    pub start: u32,
    /// Segment end (exclusive), window-relative.
    pub end: u32,
    /// The legal physical-call partition for `[start, end)` —
    /// guaranteed present in a returned plan (illegal segments are
    /// errors, never silent).
    pub partition: CallPartition,
}

/// A boundary displaced by degraded delivery: `requested` is the
/// discontinuity offset, `applied` the legal frame it moved to.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BoundaryDegradation {
    /// The requested boundary offset.
    pub requested: u32,
    /// The frame the boundary was actually applied at.
    pub applied: u32,
    /// Boundary kind (first kind at the offset wins).
    pub kind: BoundaryKind,
}

/// An evaluated delivery plan: the policy the runtime must run, the
/// precision it delivers, the deduplicated/applied boundary offsets
/// and every segment's legal call partition.
#[derive(Clone, Debug)]
pub struct ProcessBoundaryPlan {
    /// [`DeliveryPolicy::Exact`] or [`DeliveryPolicy::Degraded`].
    pub policy: DeliveryPolicy,
    /// The precision this plan delivers:
    /// [`TimingPrecision::ExactWithinVerifiedCapability`] when the
    /// contract's in-block transport support is `Verified`,
    /// [`TimingPrecision::FrameAccurateEvents`] for ordinary exact
    /// delivery, [`TimingPrecision::DegradedWithConsent`] for a
    /// consented degraded plan.
    pub precision: TimingPrecision,
    /// Applied boundary offsets (window-relative, sorted, deduped).
    /// Under degraded these are post-deferral frames.
    pub boundaries: Vec<u32>,
    /// Degradation records — empty under exact delivery.
    pub degradations: Vec<BoundaryDegradation>,
    /// Logical segments with legal partitions, in window order.
    pub segments: Vec<SegmentPlan>,
}

/// The attach-time delivery decision projected from a
/// [`ProcessCapability`] and the build's execution intent — the
/// `AttachConfig::{policy, budget}` pair the composition root
/// installs on the node's scheduler (TIM-03 wiring slice).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct AttachDelivery {
    /// [`DeliveryPolicy::Exact`] or [`DeliveryPolicy::Degraded`].
    pub policy: DeliveryPolicy,
    /// The budget to install. Under consented degraded delivery
    /// `max_deferral` is clamped to
    /// `min(consent.max_deferral, budget.max_deferral)` — the same
    /// bound [`ProcessCapability::evaluate`] applies to deferred
    /// boundaries.
    pub budget: ScheduleBudget,
}

/// Typed failures of [`ProcessCapability::evaluate`]. Nothing is
/// converted to a fake success; every rejection names what could not
/// be delivered and why.
#[non_exhaustive]
#[derive(Debug)]
pub enum ProcessBoundaryError {
    /// The request itself is malformed: empty window or a boundary
    /// offset outside `[0, window_frames)`.
    InvalidRequest {
        /// What was invalid.
        reason: &'static str,
    },
    /// The capability is certified [`TimingPrecision::Unsupported`]
    /// or carries a degenerate contract (`max_frames == 0`) —
    /// delivery is refused outright.
    Unsupported {
        /// Why delivery is unsupported.
        reason: &'static str,
    },
    /// [`ExecutionMode::OfflineExact`] cannot accept inexact
    /// delivery: a segment has no legal partition. `boundary`/`len`
    /// name the offending segment.
    OfflineExact {
        /// Segment start offset.
        boundary: u32,
        /// Segment length in frames.
        len: u32,
    },
    /// [`ExecutionMode::OfflineExact`] requires a certified
    /// exact-class [`TimingPrecision`]; the capability's certified
    /// precision does not provide one.
    OfflineUncertified {
        /// The capability's certified precision.
        precision: TimingPrecision,
    },
    /// A segment is illegal and the request is realtime without
    /// [`DegradedConsent`] — degraded delivery was not agreed to.
    /// `boundaries` lists the illegal segment start offsets.
    ConsentRequired {
        /// Illegal segment start offsets, sorted.
        boundaries: Vec<u32>,
    },
    /// Degraded delivery found no legal deferred frame within
    /// `min(consent.max_deferral, budget.max_deferral)` of the
    /// requested boundary.
    DeferralExceeded {
        /// The requested boundary offset that could not be placed.
        boundary: u32,
    },
}

impl fmt::Display for ProcessBoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest { reason } => write!(f, "invalid request: {reason}"),
            Self::Unsupported { reason } => write!(f, "unsupported capability: {reason}"),
            Self::OfflineExact { boundary, len } => {
                write!(
                    f,
                    "offline exact: segment at {boundary} ({len} frames) is inexact"
                )
            }
            Self::OfflineUncertified { precision } => {
                write!(
                    f,
                    "offline exact: certified precision {precision:?} is not exact"
                )
            }
            Self::ConsentRequired { boundaries } => {
                write!(
                    f,
                    "realtime delivery needs degraded consent for segments at {boundaries:?}"
                )
            }
            Self::DeferralExceeded { boundary } => {
                write!(
                    f,
                    "no legal deferred boundary within deferral bound at {boundary}"
                )
            }
        }
    }
}

impl std::error::Error for ProcessBoundaryError {}

/// One plugin instance's process capability: the legal physical-call
/// contract plus the certified timing precision.
///
/// `contract` is derived from the *activated* [`ProcessConfig`] via
/// `contract_from_process_config` — the same `BlockContract` the
/// node's scheduler enforces — so evaluator legality can never
/// diverge from runtime legality. `precision` is the certified
/// claim for this instance (`Unknown` until measured; see
/// [`Self::unmeasured`]).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ProcessCapability {
    contract: BlockContract,
    precision: TimingPrecision,
}

impl ProcessCapability {
    /// Builds a capability from the activated [`ProcessConfig`], the
    /// adapter-attested in-block transport support and the certified
    /// [`TimingPrecision`]. Only
    /// [`InBlockTransportSupport::Verified`] may be passed for
    /// `in_block` — `Unknown`/`Declared` are never treated as
    /// supported (upstream contract).
    #[must_use]
    pub fn from_process_config(
        config: &ProcessConfig,
        in_block: InBlockTransportSupport,
        precision: TimingPrecision,
    ) -> Self {
        Self {
            contract: contract_from_process_config(config, in_block),
            precision,
        }
    }

    /// An unmeasured capability over an explicit [`BlockContract`] —
    /// certified precision [`TimingPrecision::Unknown`]. Block bounds
    /// are contractual (always known); only the precision *claim* is
    /// unmeasured.
    #[must_use]
    pub const fn unmeasured(contract: BlockContract) -> Self {
        Self {
            contract,
            precision: TimingPrecision::Unknown,
        }
    }

    /// A capability with an explicitly supplied contract and
    /// certified precision.
    #[must_use]
    pub const fn new(contract: BlockContract, precision: TimingPrecision) -> Self {
        Self {
            contract,
            precision,
        }
    }

    /// The legal physical-call contract (`min_frames`/`max_frames`/
    /// in-block transport support).
    #[must_use]
    pub const fn contract(&self) -> BlockContract {
        self.contract
    }

    /// The certified timing precision (`Unknown` until measured).
    #[must_use]
    pub const fn precision(&self) -> TimingPrecision {
        self.precision
    }

    /// Evaluates `request` against this capability — the non-RT
    /// preflight oracle described in the module docs.
    ///
    /// # Errors
    /// [`ProcessBoundaryError`] — see the variant docs; every failure
    /// is typed and leaves no partial plan.
    pub fn evaluate(
        &self,
        request: &ProcessBoundaryRequest,
    ) -> Result<ProcessBoundaryPlan, ProcessBoundaryError> {
        if request.window_frames == 0 {
            return Err(ProcessBoundaryError::InvalidRequest {
                reason: "window_frames is 0",
            });
        }
        if self.contract.max_frames == 0 {
            return Err(ProcessBoundaryError::Unsupported {
                reason: "degenerate block contract (max_frames == 0)",
            });
        }
        if self.precision == TimingPrecision::Unsupported {
            return Err(ProcessBoundaryError::Unsupported {
                reason: "capability certified unsupported",
            });
        }

        // Normalize: sort by offset (stable — request order breaks
        // ties) and coalesce same-offset points into one boundary,
        // matching the scheduler's same-frame apply semantics.
        let mut points = request.boundaries.clone();
        points.sort_by_key(|p| p.offset);
        points.dedup_by_key(|p| p.offset);
        if points.iter().any(|p| p.offset >= request.window_frames) {
            return Err(ProcessBoundaryError::InvalidRequest {
                reason: "boundary offset outside window",
            });
        }

        let window = request.window_frames;
        // Interior cuts only — an offset-0 boundary applies at the
        // callback head and splits no span.
        let cuts: Vec<u32> = points.iter().map(|p| p.offset).filter(|o| *o > 0).collect();

        // Exact feasibility: every segment [prev, cut) plus the tail
        // [last, window) must be legally partitionable.
        let mut illegal: Vec<u32> = Vec::new();
        let mut prev = 0_u32;
        for &cut in cuts.iter().chain(std::iter::once(&window)) {
            let len = cut - prev;
            if len > 0 && !self.contract.legal_span_len(len) {
                illegal.push(prev);
            }
            prev = cut;
        }
        if illegal.is_empty() {
            return self.exact_plan(request, &cuts, window);
        }
        match request.mode {
            ExecutionMode::OfflineExact => {
                let first = illegal[0];
                Err(ProcessBoundaryError::OfflineExact {
                    boundary: first,
                    len: segment_len(&cuts, window, first),
                })
            }
            ExecutionMode::Realtime => {
                let consent =
                    request
                        .consent
                        .ok_or_else(|| ProcessBoundaryError::ConsentRequired {
                            boundaries: illegal.clone(),
                        })?;
                self.degraded_plan(request, &points, &cuts, window, consent)
            }
        }
    }

    /// The all-legal plan: [`DeliveryPolicy::Exact`] with per-segment
    /// partitions. Offline exact additionally requires certified
    /// exact precision.
    fn exact_plan(
        &self,
        request: &ProcessBoundaryRequest,
        cuts: &[u32],
        window: u32,
    ) -> Result<ProcessBoundaryPlan, ProcessBoundaryError> {
        if request.mode == ExecutionMode::OfflineExact && !self.precision.certified_exact() {
            return Err(ProcessBoundaryError::OfflineUncertified {
                precision: self.precision,
            });
        }
        Ok(ProcessBoundaryPlan {
            policy: DeliveryPolicy::Exact,
            precision: if self.contract.in_block == InBlockTransportSupport::Verified {
                TimingPrecision::ExactWithinVerifiedCapability
            } else {
                TimingPrecision::FrameAccurateEvents
            },
            boundaries: cuts.to_vec(),
            degradations: Vec::new(),
            segments: segments_for(&self.contract, cuts, window),
        })
    }

    /// The consented degraded plan: displaces each illegal boundary to
    /// the next frame where both adjacent segments are legal, bounded
    /// by `min(consent.max_deferral, budget.max_deferral)`.
    ///
    /// Mirrors the scheduler's `find_legal_boundary`: the deferral
    /// scan is bounded by the next *apply-kind* discontinuity (or the
    /// window end) — loop wraps are position-driven and do not bound
    /// it — and every requested boundary at or before an applied
    /// frame coalesces into that frame (upstream applies dues
    /// `<= prev` at the span head and re-derives wraps from position).
    fn degraded_plan(
        &self,
        request: &ProcessBoundaryRequest,
        points: &[BoundaryPoint],
        cuts: &[u32],
        window: u32,
        consent: DegradedConsent,
    ) -> Result<ProcessBoundaryPlan, ProcessBoundaryError> {
        let limit = consent.max_deferral.min(request.budget.max_deferral);
        // Apply-kind cuts bound the deferral scan; `LoopWrap` points
        // are position-driven and never bound it (upstream
        // `find_legal_boundary` sees only pending apply dues).
        let apply_bounds: Vec<u32> = cuts
            .iter()
            .copied()
            .filter(|&c| kind_at(points, c) != BoundaryKind::LoopWrap)
            .collect();
        let mut applied: Vec<u32> = Vec::with_capacity(cuts.len());
        let mut degradations: Vec<BoundaryDegradation> = Vec::new();
        let mut prev = 0_u32;
        let mut ai = 0_usize;
        for &b in cuts {
            if b <= prev {
                // Coalesced into an earlier applied frame — the
                // discontinuity still takes effect there; it is not
                // a separate span boundary.
                continue;
            }
            let len = b - prev;
            if self.contract.legal_span_len(len) {
                applied.push(b);
                prev = b;
                continue;
            }
            // Illegal segment [prev, b): defer the boundary at `b`.
            while ai < apply_bounds.len() && apply_bounds[ai] <= b {
                ai += 1;
            }
            let next_bound = apply_bounds.get(ai).copied().unwrap_or(window);
            let cand = self
                .find_legal_boundary(prev, b, next_bound, limit)
                .ok_or(ProcessBoundaryError::DeferralExceeded { boundary: b })?;
            degradations.push(BoundaryDegradation {
                requested: b,
                applied: cand,
                kind: kind_at(points, b),
            });
            applied.push(cand);
            prev = cand;
        }
        // The tail [prev, window) must also be legal — deferral can
        // leave an illegal remainder just like a requested boundary.
        let tail = window - prev;
        if tail > 0 && !self.contract.legal_span_len(tail) {
            return Err(ProcessBoundaryError::DeferralExceeded {
                boundary: *applied.last().unwrap_or(&0),
            });
        }
        let segments = segments_for(&self.contract, &applied, window);
        Ok(ProcessBoundaryPlan {
            policy: DeliveryPolicy::Degraded,
            precision: TimingPrecision::DegradedWithConsent,
            boundaries: applied,
            degradations,
            segments,
        })
    }

    /// Scans `cand` in `[b, min(b + max_deferral, next_bound))` for a
    /// frame where `[prev, cand)` and `[cand, next_bound)` are both
    /// legally partitionable — the same rule the scheduler's
    /// `find_legal_boundary` applies (bounded integer math on the
    /// shared [`BlockContract`]).
    fn find_legal_boundary(
        &self,
        prev: u32,
        b: u32,
        next_bound: u32,
        max_deferral: u32,
    ) -> Option<u32> {
        let limit = b.saturating_add(max_deferral).min(next_bound);
        let mut cand = b;
        while cand < limit {
            let left = cand - prev;
            let right = next_bound - cand;
            if self.contract.legal_span_len(left) && self.contract.legal_span_len(right) {
                return Some(cand);
            }
            cand += 1;
        }
        None
    }

    /// Projects the consumer delivery policy onto the attach-time
    /// `AttachConfig::{policy, budget}` pair — the composition-root
    /// half of the oracle. Per-window legality stays with
    /// [`Self::evaluate`]; this decides only which delivery policy
    /// and budget the node's scheduler is attached with.
    ///
    /// - A certified [`TimingPrecision::Unsupported`] capability or
    ///   a degenerate contract (`max_frames == 0`) is refused with
    ///   [`ProcessBoundaryError::Unsupported`].
    /// - [`ExecutionMode::OfflineExact`] requires certified
    ///   exact-class precision — anything else fails closed with
    ///   [`ProcessBoundaryError::OfflineUncertified`]. Offline
    ///   delivery is always [`DeliveryPolicy::Exact`]; `consent` is
    ///   ignored because degraded is never accepted offline.
    /// - [`ExecutionMode::Realtime`] with [`DegradedConsent`] attaches
    ///   [`DeliveryPolicy::Degraded`] — consent *permits* degraded
    ///   delivery within `min(consent.max_deferral,
    ///   budget.max_deferral)`; legal boundaries still apply exactly.
    ///   Without consent the attach is [`DeliveryPolicy::Exact`].
    ///
    /// # Errors
    /// [`ProcessBoundaryError::Unsupported`] or
    /// [`ProcessBoundaryError::OfflineUncertified`] — the only
    /// variants this projection can produce.
    pub fn attach_delivery(
        &self,
        mode: ExecutionMode,
        consent: Option<DegradedConsent>,
        budget: ScheduleBudget,
    ) -> Result<AttachDelivery, ProcessBoundaryError> {
        if self.contract.max_frames == 0 {
            return Err(ProcessBoundaryError::Unsupported {
                reason: "degenerate block contract (max_frames == 0)",
            });
        }
        if self.precision == TimingPrecision::Unsupported {
            return Err(ProcessBoundaryError::Unsupported {
                reason: "capability certified unsupported",
            });
        }
        if mode == ExecutionMode::OfflineExact {
            if !self.precision.certified_exact() {
                return Err(ProcessBoundaryError::OfflineUncertified {
                    precision: self.precision,
                });
            }
            return Ok(AttachDelivery {
                policy: DeliveryPolicy::Exact,
                budget,
            });
        }
        match consent {
            Some(consent) => Ok(AttachDelivery {
                policy: DeliveryPolicy::Degraded,
                budget: ScheduleBudget::new(
                    budget.max_spans_per_callback,
                    budget.max_events_per_span,
                    budget.max_pending_applies,
                    consent.max_deferral().min(budget.max_deferral),
                ),
            }),
            None => Ok(AttachDelivery {
                policy: DeliveryPolicy::Exact,
                budget,
            }),
        }
    }
}

/// Length of the segment starting at `start` under `cuts`/`window`.
fn segment_len(cuts: &[u32], window: u32, start: u32) -> u32 {
    cuts.iter()
        .chain(std::iter::once(&window))
        .find(|&&c| c > start)
        .map_or(window - start, |c| c - start)
}

/// Builds [`SegmentPlan`]s for `cuts` inside `window` — every segment
/// is legal by construction of the caller.
fn segments_for(contract: &BlockContract, cuts: &[u32], window: u32) -> Vec<SegmentPlan> {
    let mut out = Vec::with_capacity(cuts.len() + 1);
    let mut prev = 0_u32;
    for &cut in cuts.iter().chain(std::iter::once(&window)) {
        let len = cut - prev;
        if len > 0
            && let Some(partition) = contract.partition(len)
        {
            out.push(SegmentPlan {
                start: prev,
                end: cut,
                partition,
            });
        }
        prev = cut;
    }
    out
}

/// The kind recorded at offset `b` — first matching point wins
/// (dedup already coalesced same-offset points).
fn kind_at(points: &[BoundaryPoint], b: u32) -> BoundaryKind {
    points
        .iter()
        .find(|p| p.offset == b)
        .map_or(BoundaryKind::Other, |p| p.kind)
}

// ---------------------------------------------------------------------------
// Per-request plan wiring + precision provenance (TIM-03 wiring slice)
// ---------------------------------------------------------------------------

/// Generation pins proving one attached node's delivery context is
/// still current — the precision-provenance record every
/// [`ProcessBoundarySession`] call re-checks (issue #127).
///
/// The composition root mints this once per attach and re-presents it
/// as `current` on every session call. A pin that no longer matches
/// live truth — plugin re-attach/restart (new
/// [`StablePluginInstanceId`]), capability re-enumeration after a
/// plugin-state restore, or a routing/control-plan swap (new graph
/// generation) — turns every subsequent stage into a typed
/// [`BoundarySessionError`] refusal: stale context delivers nothing.
///
/// Clock mapping is a separate axis verified through
/// [`AttachClockContext`] (the attach-time frame base, Ruviform #307):
/// event positions authored against a different base are refused by
/// [`ProcessBoundarySession::dispatch_params`] /
/// [`ProcessBoundarySession::dispatch_midi`] before any plan is
/// staged. Transport/plan generations are event-scoped axes owned by
/// the upstream scheduler — [`ParamDispatch`]'s receipt-gated
/// observation already faults stale-generation delivery, so they are
/// deliberately not duplicated here.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PrecisionProvenance {
    /// Runtime plugin-instance identity (re-attach/restart mints a
    /// new id).
    instance: StablePluginInstanceId,
    /// The graph node the session drives.
    node: NodeId,
    /// Capability/parameter-surface generation (port capability
    /// generation / plugin-state binding revision).
    capability_generation: u64,
    /// Routing/control-plan generation the node belongs to.
    graph_generation: u64,
}

impl PrecisionProvenance {
    /// Mints the pin record for one attached node.
    #[must_use]
    pub const fn new(
        instance: StablePluginInstanceId,
        node: NodeId,
        capability_generation: u64,
        graph_generation: u64,
    ) -> Self {
        Self {
            instance,
            node,
            capability_generation,
            graph_generation,
        }
    }

    /// The pinned plugin-instance identity.
    #[must_use]
    pub const fn instance(&self) -> StablePluginInstanceId {
        self.instance
    }

    /// The pinned graph node.
    #[must_use]
    pub const fn node(&self) -> NodeId {
        self.node
    }

    /// The pinned capability generation.
    #[must_use]
    pub const fn capability_generation(&self) -> u64 {
        self.capability_generation
    }

    /// The pinned graph generation.
    #[must_use]
    pub const fn graph_generation(&self) -> u64 {
        self.graph_generation
    }
}

/// Typed failures of [`ProcessBoundarySession`]. Every refusal is
/// named and leaves nothing staged — a rejected request never reaches
/// the node's control queue.
#[non_exhaustive]
#[derive(Debug)]
pub enum BoundarySessionError {
    /// [`ProcessCapability::attach_delivery`] refused the
    /// capability/delivery intent at [`ProcessBoundarySession::open`]
    /// — `Unsupported`/`OfflineUncertified` (the same typed refusal
    /// `PluginHostProcessors::connect` maps).
    Capability(ProcessBoundaryError),
    /// The per-request oracle refused the window — the typed
    /// [`ProcessBoundaryError`] is preserved verbatim and nothing was
    /// staged.
    Evaluate(ProcessBoundaryError),
    /// The presented provenance's plugin instance differs from the
    /// session pin — a re-attach or restart minted a new identity.
    InstanceMismatch {
        /// The session's pinned instance.
        expected: StablePluginInstanceId,
        /// The presented instance.
        actual: StablePluginInstanceId,
    },
    /// The presented provenance's node (or the dispatch's node)
    /// differs from the session pin.
    NodeMismatch {
        /// The session's pinned node.
        expected: NodeId,
        /// The presented node.
        actual: NodeId,
    },
    /// The capability generation drifted — plugin-state restore or
    /// capability re-enumeration invalidated the pin.
    CapabilityGeneration {
        /// The session's pinned generation.
        expected: u64,
        /// The presented generation.
        actual: u64,
    },
    /// The graph generation drifted — a routing/control-plan swap
    /// invalidated the pin.
    GraphGeneration {
        /// The session's pinned generation.
        expected: u64,
        /// The presented generation.
        actual: u64,
    },
    /// The dispatch's frame base differs from the verified attach
    /// clock context — the clock mapping the event positions were
    /// authored against is stale (Ruviform #307).
    FrameBaseMismatch(FrameBaseMismatch),
    /// A staged transport command hit the node's bounded control
    /// queue. Commands pushed before the failure remain staged —
    /// they were all covered by the evaluated plan; retry policy is
    /// the caller's.
    QueueFull,
    /// `install_plan` was rejected by the node control — upstream
    /// [`InstallPlanError`] preserved (`FrameBaseMismatch` /
    /// `QueueFull`).
    InstallPlan(InstallPlanError),
    /// [`ParamDispatch`] refused the write batch — the upstream
    /// [`DispatchError`] is preserved verbatim (binding rejection,
    /// `Empty`, `QueueFull`, …).
    Dispatch(DispatchError),
}

impl fmt::Display for BoundarySessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capability(e) => write!(f, "capability refused at open: {e}"),
            Self::Evaluate(e) => write!(f, "boundary plan refused: {e}"),
            Self::InstanceMismatch { expected, actual } => {
                write!(f, "stale provenance: instance {expected:?} != {actual:?}")
            }
            Self::NodeMismatch { expected, actual } => {
                write!(f, "stale provenance: node {expected:?} != {actual:?}")
            }
            Self::CapabilityGeneration { expected, actual } => {
                write!(
                    f,
                    "stale provenance: capability generation {expected} != {actual}"
                )
            }
            Self::GraphGeneration { expected, actual } => {
                write!(
                    f,
                    "stale provenance: graph generation {expected} != {actual}"
                )
            }
            Self::FrameBaseMismatch(e) => write!(f, "{e}"),
            Self::QueueFull => write!(f, "plugin-node control queue full"),
            Self::InstallPlan(e) => write!(f, "plan install rejected: {e}"),
            Self::Dispatch(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BoundarySessionError {}

/// The per-request process-boundary gate for one attached plugin
/// node — the runtime wiring half of the TIM-03 oracle.
///
/// [`ProcessCapability::evaluate`] stays a pure non-RT oracle; this
/// session binds it to the node's verified attach context
/// ([`AttachClockContext`], Ruviform #307), the attach-time
/// [`AttachDelivery`] decision (the same `mode`/`consent`/`budget`
/// inputs `PluginHostProcessors::connect` projects onto
/// `AttachConfig::{policy, budget}`), and a [`PrecisionProvenance`]
/// pin record. Every method evaluates the upcoming window *before*
/// touching the control channel, so an illegal or unauthorized
/// boundary — and any stale provenance — is a typed refusal with
/// nothing staged: never a silent quantization, never a
/// wrong-target delivery.
///
/// Window contract: `window_frames`/`boundaries` describe the
/// *upcoming* callback window the node will run (e.g. the engine
/// block length and the discontinuities expected inside it —
/// position-derived loop wraps are caller-supplied; tempo/loop offset
/// auto-derivation is a documented non-goal). Staged `due_frames` are
/// window-relative: a command or plan install due inside the window
/// is itself a boundary and is legality-checked at its offset;
/// anything due at/past the window end belongs to a later request.
///
/// # RT contract
/// Non-RT orchestration only — `evaluate` allocates the plan and the
/// stage methods push bounded control messages. Nothing here runs on
/// the audio callback; the RT path stays upstream
/// (`TransportScheduler`/`PluginNode`).
#[derive(Debug)]
pub struct ProcessBoundarySession {
    capability: ProcessCapability,
    clock: AttachClockContext,
    mode: ExecutionMode,
    consent: Option<DegradedConsent>,
    budget: ScheduleBudget,
    attach: AttachDelivery,
    provenance: PrecisionProvenance,
}

impl ProcessBoundarySession {
    /// Opens a session for the node behind `control`.
    ///
    /// `capability`/`mode`/`consent`/`budget` must be the same inputs
    /// the composition root attached the node with — `attach_delivery`
    /// is re-evaluated here so an unattachable capability
    /// (`Unsupported`, uncertified `OfflineExact`) fails closed before
    /// a session exists, and the resulting [`AttachDelivery`] records
    /// the policy/budget the scheduler actually runs. Because every
    /// [`Self::plan_window`] uses the same inputs, an exact plan is
    /// always honored (a `Degraded` attach delivers legal boundaries
    /// identically) and a degraded plan can only arise under a
    /// consented `Degraded` attach — the oracle and the scheduler can
    /// never disagree about the delivery policy.
    ///
    /// # Errors
    /// [`BoundarySessionError::Capability`] when `attach_delivery`
    /// refuses the capability/delivery intent.
    pub fn open(
        control: &PluginNodeControl,
        capability: ProcessCapability,
        mode: ExecutionMode,
        consent: Option<DegradedConsent>,
        budget: ScheduleBudget,
        provenance: PrecisionProvenance,
    ) -> Result<Self, BoundarySessionError> {
        let attach = capability
            .attach_delivery(mode, consent, budget)
            .map_err(BoundarySessionError::Capability)?;
        Ok(Self {
            capability,
            clock: AttachClockContext::of(control),
            mode,
            consent,
            budget,
            attach,
            provenance,
        })
    }

    /// The capability this session evaluates against.
    #[must_use]
    pub const fn capability(&self) -> ProcessCapability {
        self.capability
    }

    /// The attach-time delivery decision the node's scheduler runs —
    /// the same `AttachConfig::{policy, budget}` projection
    /// `PluginHostProcessors::connect` installed.
    #[must_use]
    pub const fn attach_delivery(&self) -> AttachDelivery {
        self.attach
    }

    /// The provenance pins this session was opened with.
    #[must_use]
    pub const fn provenance(&self) -> PrecisionProvenance {
        self.provenance
    }

    /// Evaluates one upcoming window — provenance check, then the
    /// non-RT oracle with the session's fixed mode/consent/budget.
    /// Pure: nothing is staged.
    ///
    /// # Errors
    /// [`BoundarySessionError::InstanceMismatch`] /
    /// [`NodeMismatch`] / [`CapabilityGeneration`] /
    /// [`GraphGeneration`] on stale provenance;
    /// [`BoundarySessionError::Evaluate`] on oracle refusal
    /// (`Unsupported`, `ConsentRequired`, `OfflineExact`,
    /// `OfflineUncertified`, `DeferralExceeded`, `InvalidRequest`).
    pub fn plan_window(
        &self,
        current: &PrecisionProvenance,
        window_frames: u32,
        boundaries: Vec<BoundaryPoint>,
    ) -> Result<ProcessBoundaryPlan, BoundarySessionError> {
        self.check_provenance(current)?;
        let request = ProcessBoundaryRequest {
            window_frames,
            boundaries,
            mode: self.mode,
            consent: self.consent,
            budget: self.budget,
        };
        self.capability
            .evaluate(&request)
            .map_err(BoundarySessionError::Evaluate)
    }

    /// [`Self::plan_window`] plus staging `commands` on `control` —
    /// the per-request wiring of transport discontinuities.
    ///
    /// Each command whose `due_frames` lands inside the window
    /// contributes its own [`BoundaryPoint`] (a `Seek` maps to
    /// [`BoundaryKind::Seek`], every other command to
    /// [`BoundaryKind::Other`]) on top of the caller-supplied
    /// `boundaries` (position-derived loop wraps etc.), so the oracle
    /// checks exactly the discontinuities being staged. The plan is
    /// computed *first*: an oracle refusal stages nothing. On success
    /// commands push in order; a [`BoundarySessionError::QueueFull`]
    /// aborts the remainder with earlier commands still staged (they
    /// were covered by the evaluated plan — bounded-queue reality,
    /// same contract as `PluginNodeControl::request`).
    ///
    /// # Errors
    /// As [`Self::plan_window`], plus [`BoundarySessionError::QueueFull`].
    pub fn stage_window(
        &self,
        control: &mut PluginNodeControl,
        current: &PrecisionProvenance,
        window_frames: u32,
        boundaries: Vec<BoundaryPoint>,
        commands: &[(TransportCommand, u64)],
    ) -> Result<ProcessBoundaryPlan, BoundarySessionError> {
        let mut points = boundaries;
        for (cmd, due) in commands {
            if let Some(point) = command_boundary(*cmd, *due, window_frames) {
                points.push(point);
            }
        }
        let plan = self.plan_window(current, window_frames, points)?;
        for (cmd, due) in commands {
            control
                .request(*cmd, *due)
                .map_err(|_| BoundarySessionError::QueueFull)?;
        }
        Ok(plan)
    }

    /// [`Self::plan_window`] plus installing `plan` at `due_frames` —
    /// the per-request wiring of event/tempo plan installs.
    ///
    /// An install due inside the window contributes a
    /// [`BoundaryKind::PlanInstall`] point at its offset (a plan
    /// install bumps `plan_generation`, splits spans and releases/
    /// chases notes upstream — it is a real discontinuity). Due at or
    /// past the window end it applies at a later callback head and is
    /// not this window's boundary.
    ///
    /// # Errors
    /// As [`Self::plan_window`], plus [`BoundarySessionError::InstallPlan`].
    pub fn stage_plan(
        &self,
        control: &mut PluginNodeControl,
        current: &PrecisionProvenance,
        window_frames: u32,
        boundaries: Vec<BoundaryPoint>,
        plan: Arc<TransportPlan<EventKind>>,
        due_frames: u64,
    ) -> Result<ProcessBoundaryPlan, BoundarySessionError> {
        let mut points = boundaries;
        if let Some(offset) = window_offset(due_frames, window_frames) {
            points.push(BoundaryPoint {
                offset,
                kind: BoundaryKind::PlanInstall,
            });
        }
        let evaluated = self.plan_window(current, window_frames, points)?;
        control
            .install_plan(plan, due_frames)
            .map_err(BoundarySessionError::InstallPlan)?;
        Ok(evaluated)
    }

    /// Generation-gated param dispatch — the precision-provenance
    /// half of the param path.
    ///
    /// Verifies `current` against the session pins *and* `dispatch`
    /// against the pinned node and verified clock context, then
    /// delegates to [`ParamDispatch::dispatch`] — which keeps its own
    /// generation-tagged plan minting, ledger and receipt-gated
    /// observation (the `dispatch_midi` pattern). A stale instance /
    /// node / capability generation / graph generation / frame base
    /// is a typed refusal before any write resolves: wrong-target or
    /// stale param delivery is structurally impossible through this
    /// path.
    ///
    /// The staged plan installs at the callback head (`due = 0`), an
    /// offset-0 apply that splits no span — head installs cannot
    /// create an illegal interior boundary, so no window evaluation
    /// is needed here; window legality is [`Self::plan_window`]'s
    /// concern for in-window discontinuities.
    ///
    /// # Errors
    /// [`BoundarySessionError::InstanceMismatch`] /
    /// [`NodeMismatch`] / [`CapabilityGeneration`] /
    /// [`GraphGeneration`] / [`FrameBaseMismatch`] on stale context;
    /// [`BoundarySessionError::Dispatch`] on upstream refusal.
    pub fn dispatch_params(
        &self,
        dispatch: &mut ParamDispatch,
        current: &PrecisionProvenance,
        writes: &[(ParameterTargetId, TimedParamEvent)],
    ) -> Result<Arc<TransportPlan<EventKind>>, BoundarySessionError> {
        self.check_dispatch(dispatch, current)?;
        dispatch
            .dispatch(writes)
            .map_err(BoundarySessionError::Dispatch)
    }

    /// Generation-gated MIDI dispatch — identical gating to
    /// [`Self::dispatch_params`] over [`ParamDispatch::dispatch_midi`].
    ///
    /// # Errors
    /// As [`Self::dispatch_params`].
    pub fn dispatch_midi(
        &self,
        dispatch: &mut ParamDispatch,
        current: &PrecisionProvenance,
        events: &[(i64, MidiEvent)],
    ) -> Result<Arc<TransportPlan<EventKind>>, BoundarySessionError> {
        self.check_dispatch(dispatch, current)?;
        dispatch
            .dispatch_midi(events)
            .map_err(BoundarySessionError::Dispatch)
    }

    /// Compares `current` against the pinned provenance — first
    /// mismatched axis wins, named by the typed variant.
    fn check_provenance(&self, current: &PrecisionProvenance) -> Result<(), BoundarySessionError> {
        let pinned = self.provenance;
        if current.instance != pinned.instance {
            return Err(BoundarySessionError::InstanceMismatch {
                expected: pinned.instance,
                actual: current.instance,
            });
        }
        if current.node != pinned.node {
            return Err(BoundarySessionError::NodeMismatch {
                expected: pinned.node,
                actual: current.node,
            });
        }
        if current.capability_generation != pinned.capability_generation {
            return Err(BoundarySessionError::CapabilityGeneration {
                expected: pinned.capability_generation,
                actual: current.capability_generation,
            });
        }
        if current.graph_generation != pinned.graph_generation {
            return Err(BoundarySessionError::GraphGeneration {
                expected: pinned.graph_generation,
                actual: current.graph_generation,
            });
        }
        Ok(())
    }

    /// Provenance plus the dispatch-side pins: the dispatch must drive
    /// the session's node and build positions in the verified attach
    /// frame base — a foreign `ParamDispatch` (different node or a
    /// stale clock mapping) is refused before any write resolves.
    fn check_dispatch(
        &self,
        dispatch: &ParamDispatch,
        current: &PrecisionProvenance,
    ) -> Result<(), BoundarySessionError> {
        self.check_provenance(current)?;
        let pinned = self.provenance.node();
        if dispatch.node() != pinned {
            return Err(BoundarySessionError::NodeMismatch {
                expected: pinned,
                actual: dispatch.node(),
            });
        }
        self.clock
            .check_frame_base(dispatch.frame_base())
            .map_err(BoundarySessionError::FrameBaseMismatch)
    }
}

/// Maps a staged command's `due_frames` onto a window-relative
/// [`BoundaryPoint`] — `None` when the apply lands at or beyond the
/// window end (a later window's boundary, evaluated by that window's
/// request).
fn command_boundary(cmd: TransportCommand, due: u64, window: u32) -> Option<BoundaryPoint> {
    let offset = window_offset(due, window)?;
    Some(BoundaryPoint {
        offset,
        kind: match cmd {
            TransportCommand::Seek(_) => BoundaryKind::Seek,
            _ => BoundaryKind::Other,
        },
    })
}

/// `due_frames` → window offset when the apply lands strictly inside
/// `[0, window)`; `None` at or past the window end.
fn window_offset(due: u64, window: u32) -> Option<u32> {
    u32::try_from(due).ok().filter(|d| *d < window)
}
