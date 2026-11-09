//! Read-only `ControlTrace` projection over one [`ParameterTargetId`]
//! (issue #100, PC-C3 — TRACE-C3-01..04).
//!
//! [`ControlTrace`] correlates per-stage evidence — configured,
//! prepared, scheduled, applied, dispatched, plugin-reported and
//! probe-observed — under a single [`ControlTraceKey`]. Every stage
//! is an independent [`TraceSlot`]: a missing stage stays `Missing` /
//! `Unsupported` / `Dropped` / `Expired` and is never synthesized
//! from neighbouring stages' success.
//!
//! Honesty contract (mirrors the Ruviform dispatch ledger and the
//! workspace observation split):
//!
//! - `Dispatched` evidence is a *projection* — the event was covered
//!   by a plan/span on the dispatch path. It is not a plugin-side
//!   acknowledgment and says nothing about plugin-internal processing
//!   or output.
//! - `Applied` is the scheduler/RT application receipt; it is distinct
//!   from dispatch coverage and from any plugin-visible report.
//! - `PluginReported` and `Observed` (probe) are independent
//!   measurement channels; neither is implied by the other.
//! - A failed plan publish never displaces the last-good active plan
//!   — see [`PlanActivationState`].
//!
//! Freshness ([`ControlTrace::freshness`]) compares the trace key and
//! evidence stamps against a [`FreshnessContext`]; generation, epoch,
//! transport, loop, position and age mismatches classify as
//! [`ObservationFreshness::Stale`] or
//! [`ObservationFreshness::Unavailable`] — never `Current`.
//!
//! This module is a pure projection: it mutates no project state,
//! controller, undo history, evaluator or routing graph, and performs
//! no allocation on the freshness path.
//!
//! [`ParameterTargetId`]: crate::id::ParameterTargetId
//! [`Project::control`]: crate::project::Project::control

use ruviform_media_core::{
    AppliedReceipt, ApplyStatus, ClockEpochId, EngineFrame, LoopOccurrence, MediaDuration,
    MediaTime, MediaTimestamp, PlanGeneration, TimestampProvenance, TransportGeneration,
};
use ruviform_plugin_core::ParamValue;

use crate::{
    ProjectRevision,
    id::{ParameterTargetId, ProjectId},
    routing_build::{RoutingFault, RuntimeInstanceId},
};

/// Correlation key binding every stage slot of one [`ControlTrace`] to
/// an exact evidence context. Two traces with different keys describe
/// different worlds and must never share evidence.
///
/// `control_plan_generation == PlanGeneration::new(0)` means no
/// dispatch plan has been staged for this target yet.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ControlTraceKey {
    /// Owning project.
    pub project_id: ProjectId,
    /// Semantic parameter target — never a raw node/param id, so
    /// runtime instance or plugin generation drift cannot alias two
    /// targets onto one trace.
    pub parameter_target_id: ParameterTargetId,
    /// Committed project revision the evidence was produced under.
    pub project_revision: ProjectRevision,
    /// Generation of the staged dispatch plan.
    pub control_plan_generation: PlanGeneration,
    /// Graph build generation (SF-03 `RoutingSourceMap`-scoped).
    pub graph_generation: u64,
    /// Transport generation — bumped by Play/Pause/Stop/Seek.
    pub transport_generation: TransportGeneration,
    /// Runtime device clock epoch — bumped by `DeviceRestart`.
    pub clock_epoch: ClockEpochId,
    /// Loop occurrence — distinguishes passes through a loop region
    /// that share positions but are different evidence contexts.
    pub loop_occurrence: LoopOccurrence,
    /// Media position the trace evaluates at.
    pub evaluation_position: MediaTime,
}

/// One stage's evidence state. `Present` carries the evidence;
/// every other variant records *why* there is none — evidence is
/// never synthesized for a missing stage.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TraceSlot<T> {
    /// Stage evidence exists.
    Present(T),
    /// The stage never produced evidence for this target.
    Missing,
    /// The stage cannot produce evidence for this target.
    Unsupported,
    /// Evidence existed but was dropped before correlation.
    Dropped,
    /// Evidence existed but expired (superseded) before correlation.
    Expired,
}

impl<T> TraceSlot<T> {
    /// The evidence when `Present`, else `None`.
    #[must_use]
    pub const fn present(&self) -> Option<&T> {
        match self {
            Self::Present(evidence) => Some(evidence),
            _ => None,
        }
    }

    /// Whether the slot holds evidence.
    #[must_use]
    pub const fn is_present(&self) -> bool {
        matches!(self, Self::Present(_))
    }
}

/// Host-computed configured value evidence — what the project says the
/// parameter should be at the key's evaluation position. This is a
/// *computed* value, not a plugin-applied or observed one.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ConfiguredEvidence {
    /// Host-normalized value (`0.0..=1.0`) at `evaluation_position`.
    pub host_normalized: f64,
    /// The raw total saturated the parameter range.
    pub saturated: bool,
    /// The value was quantized for dispatch.
    pub quantized: bool,
}

/// Prepared-plan evidence — a control/dispatch plan was compiled and
/// staged under `plan_generation` but not yet installed.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PreparedEvidence {
    /// Generation of the prepared plan — must agree with
    /// [`ControlTraceKey::control_plan_generation`] for coherence.
    pub plan_generation: PlanGeneration,
}

/// Receipt evidence projected from an RT-minted [`AppliedReceipt`].
/// The scheduler's `CommandId` correlation token is intentionally
/// dropped — it is meaningful only inside the issuing scheduler —
/// while the lifecycle status, applied frame and post-receipt
/// generations/epoch are preserved for stage correlation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ReceiptEvidence {
    /// Lifecycle status at capture time.
    pub status: ApplyStatus,
    /// Frame the command was actually applied at (`Applied` only).
    pub applied_frame: Option<EngineFrame>,
    /// Transport generation after the receipt was produced.
    pub transport_generation: TransportGeneration,
    /// Plan generation after the receipt was produced.
    pub plan_generation: PlanGeneration,
    /// Clock epoch after the receipt was produced.
    pub clock_epoch: ClockEpochId,
}

impl ReceiptEvidence {
    /// Projects a scheduler/RT [`AppliedReceipt`] into trace evidence.
    #[must_use]
    pub fn from_applied_receipt(receipt: &AppliedReceipt) -> Self {
        Self {
            status: receipt.status,
            applied_frame: receipt.applied_frame,
            transport_generation: receipt.transport_generation,
            plan_generation: receipt.plan_generation,
            clock_epoch: receipt.clock_epoch,
        }
    }
}

/// Scheduled-stage receipt evidence — a [`ReceiptEvidence`] still in
/// `Requested`/`Pending`. Construction is fail-closed: evidence in
/// any other status cannot occupy this slot.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ScheduledReceipt {
    receipt: ReceiptEvidence,
}

impl ScheduledReceipt {
    /// Wraps `receipt` when its status is `Requested` or `Pending`.
    #[must_use]
    pub fn new(receipt: ReceiptEvidence) -> Option<Self> {
        match receipt.status {
            ApplyStatus::Requested | ApplyStatus::Pending => Some(Self { receipt }),
            _ => None,
        }
    }

    /// The wrapped evidence.
    #[must_use]
    pub const fn receipt(&self) -> ReceiptEvidence {
        self.receipt
    }
}

/// Applied-stage receipt evidence — a [`ReceiptEvidence`] whose
/// status is `Applied`: the scheduler/RT side confirmed application
/// at `applied_frame`. This is *not* proof the plugin processed the
/// value internally or that any output was measured.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct AppliedStageReceipt {
    receipt: ReceiptEvidence,
}

impl AppliedStageReceipt {
    /// Wraps `receipt` when its status is `Applied`.
    #[must_use]
    pub fn new(receipt: ReceiptEvidence) -> Option<Self> {
        match receipt.status {
            ApplyStatus::Applied => Some(Self { receipt }),
            _ => None,
        }
    }

    /// The wrapped evidence.
    #[must_use]
    pub const fn receipt(&self) -> ReceiptEvidence {
        self.receipt
    }
}

/// Why a scheduled update never reached dispatch — semantic mirror of
/// the Ruviform bridge `DropReason`. Every drop is attributed; nothing
/// disappears silently.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DispatchDropReason {
    /// Per-call event scratch could not hold the window.
    EventScratchOverflow,
    /// `PluginEndpoint::process` returned an error — payload is the
    /// report's coarse error category.
    EndpointError {
        /// Coarse error category.
        kind: u8,
    },
    /// The span could not be partitioned into legal physical calls.
    UndeliverableSpan,
    /// A scheduler fault (stale generation, undeliverable apply, …).
    ScheduleFault,
    /// A newer plan replaced this one before delivery.
    PlanReplaced,
    /// The plan never installed — the plan vault was full.
    VaultExceeded,
}

/// Dispatched-stage evidence — semantic mirror of the bridge
/// `DispatchStage`. `Dispatched` is a deterministic projection onto a
/// physical call window: *sent*, never a plugin-side ACK.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum DispatchEvidence {
    /// In the installed plan; no physical call has covered its
    /// position yet — *sent-planned*.
    Scheduled {
        /// Host-resolved value in flight.
        value: ParamValue,
        /// Scheduled media position.
        position: MediaTime,
        /// Plan generation the event belongs to.
        plan_generation: PlanGeneration,
    },
    /// Projected onto physical call `call_start` + `frame_offset` —
    /// *sent*. Projection, not acknowledgment.
    Dispatched {
        /// Host-resolved value in flight.
        value: ParamValue,
        /// Scheduled media position.
        position: MediaTime,
        /// Plan generation the event belongs to.
        plan_generation: PlanGeneration,
        /// Engine frame the physical call starts at.
        call_start: EngineFrame,
        /// Offset of the event inside that call.
        frame_offset: u32,
    },
    /// Dropped on the way — see [`DispatchDropReason`].
    Dropped {
        /// Scheduled media position.
        position: MediaTime,
        /// Plan generation the event belonged to.
        plan_generation: PlanGeneration,
        /// Why the update did not reach dispatch.
        reason: DispatchDropReason,
    },
    /// A plugin-reported value exists on the dispatch path — the
    /// plugin's *claim*, kept here for ledger fidelity. The dedicated
    /// [`ControlTrace::plugin_reported`] slot carries the observation
    /// channel's stamped evidence.
    Reported {
        /// Plugin-claimed value.
        value: ParamValue,
        /// Scheduled media position.
        position: MediaTime,
    },
    /// No plugin→host reporting channel exists — the observable
    /// trail ends at `Dispatched`; *not* an error.
    Unavailable {
        /// Scheduled media position.
        position: MediaTime,
    },
}

/// Plugin-reported observation — the value the plugin *claims* for
/// the parameter, with the media timestamp and its provenance. The
/// provenance gates freshness: an `Estimated` stamp never yields
/// [`ObservationFreshness::Current`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PluginReportedEvidence {
    /// Plugin-claimed normalized value.
    pub value: ParamValue,
    /// Measurement stamp — time plus provenance.
    pub stamp: MediaTimestamp,
}

/// What a probe measurement actually observed — `Silence` is a real
/// measurement and must never be conflated with a missing, lost or
/// disabled probe channel.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MeasuredValue {
    /// A measured signal value (normalized).
    Signal(ParamValue),
    /// Measured silence — the probe ran and observed silence.
    Silence,
}

/// Probe-observed channel state for one trace. `Missing`/`Disabled`/
/// `Lost` are channel states, not measurements; only `Measuring`
/// carries a stamp and a [`MeasuredValue`].
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ProbeObservation {
    /// No probe binding exists for the point.
    Missing,
    /// A binding exists but observation is disabled.
    Disabled,
    /// Observation data was lost — `count` blocks/records dropped.
    Lost {
        /// Number of lost blocks/records.
        count: u64,
    },
    /// The probe is measuring — stamp plus measured value.
    Measuring {
        /// Measurement stamp — time plus provenance.
        stamp: MediaTimestamp,
        /// What was measured.
        value: MeasuredValue,
    },
}

/// The per-stage evidence of one [`ControlTraceKey`]. Fields are
/// public so consumers can inspect any stage independently; use the
/// `*_freshness` / `is_coherent` methods for classified answers.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlTrace {
    /// The correlation key every slot belongs to.
    pub key: ControlTraceKey,
    /// Configure-stage evidence (host-computed value).
    pub configured: TraceSlot<ConfiguredEvidence>,
    /// Prepare-stage evidence (staged plan generation).
    pub prepared: TraceSlot<PreparedEvidence>,
    /// Install/Applied-pending receipt (`Requested`/`Pending`).
    pub scheduled: TraceSlot<ScheduledReceipt>,
    /// RT/scheduler application receipt (`Applied` only).
    pub applied: TraceSlot<AppliedStageReceipt>,
    /// Dispatch-path projection — *sent*, never plugin-internal proof.
    pub dispatched: TraceSlot<DispatchEvidence>,
    /// Plugin-visible report channel (stamped claim).
    pub plugin_reported: TraceSlot<PluginReportedEvidence>,
    /// Probe measurement channel.
    pub observed: TraceSlot<ProbeObservation>,
}

impl ControlTrace {
    /// Freshness of the plugin-reported channel alone.
    #[must_use]
    pub fn plugin_reported_freshness(&self, ctx: &FreshnessContext) -> ObservationFreshness {
        self.freshness_for(ctx, Channel::PluginReported)
    }

    /// Freshness of the probe-observed channel alone.
    #[must_use]
    pub fn observed_freshness(&self, ctx: &FreshnessContext) -> ObservationFreshness {
        self.freshness_for(ctx, Channel::Observed)
    }

    /// Combined observation freshness — the *best* trustworthy answer
    /// across the plugin-reported and probe-observed channels.
    /// Ordering: `Current` < `Estimated` < `Stale` < `DataLoss` <
    /// `Unavailable`; equal ranks prefer the channel whose slot
    /// actually holds evidence (a present-but-untrusted stamp is more
    /// informative than absent evidence), then the probe channel as
    /// the higher authority. Per-channel detail stays available via
    /// [`Self::plugin_reported_freshness`] /
    /// [`Self::observed_freshness`].
    ///
    /// Generation/epoch/transport/loop/position coherence between the
    /// trace key and `ctx` is checked first — any mismatch makes the
    /// whole trace `Stale` regardless of channel.
    #[must_use]
    pub fn freshness(&self, ctx: &FreshnessContext) -> ObservationFreshness {
        let observed = self.observed_freshness(ctx);
        let reported = self.plugin_reported_freshness(ctx);
        // Lower rank wins; on equal rank a Present slot (evidence
        // exists, even if untrusted) beats an absent one; a full tie
        // prefers the probe channel.
        let observed_score = (observed.rank(), !self.observed.is_present());
        let reported_score = (reported.rank(), !self.plugin_reported.is_present());
        if observed_score <= reported_score {
            observed
        } else {
            reported
        }
    }

    /// Whether every generation embedded in present stage evidence
    /// agrees with [`Self::key`]. `false` means at least one receipt
    /// or dispatch record belongs to a different context — consumers
    /// must not trust the mismatched stage as describing this trace.
    #[must_use]
    pub fn is_coherent(&self) -> bool {
        let key = &self.key;
        if let TraceSlot::Present(evidence) = &self.prepared
            && evidence.plan_generation != key.control_plan_generation
        {
            return false;
        }
        let receipt_coherent = |receipt: ReceiptEvidence| {
            receipt.plan_generation == key.control_plan_generation
                && receipt.transport_generation == key.transport_generation
                && receipt.clock_epoch == key.clock_epoch
        };
        if let TraceSlot::Present(receipt) = &self.scheduled
            && !receipt_coherent(receipt.receipt())
        {
            return false;
        }
        if let TraceSlot::Present(receipt) = &self.applied
            && !receipt_coherent(receipt.receipt())
        {
            return false;
        }
        if let TraceSlot::Present(evidence) = &self.dispatched {
            let plan_generation = match evidence {
                DispatchEvidence::Scheduled {
                    plan_generation, ..
                }
                | DispatchEvidence::Dispatched {
                    plan_generation, ..
                }
                | DispatchEvidence::Dropped {
                    plan_generation, ..
                } => Some(*plan_generation),
                DispatchEvidence::Reported { .. } | DispatchEvidence::Unavailable { .. } => None,
            };
            if let Some(generation) = plan_generation
                && generation != key.control_plan_generation
            {
                return false;
            }
        }
        true
    }

    /// First generation/position mismatch between the trace key and
    /// the live context, in deterministic check order.
    fn context_stale(&self, ctx: &FreshnessContext) -> Option<ObservationFreshness> {
        let key = &self.key;
        if key.project_revision != ctx.project_revision {
            return Some(ObservationFreshness::Stale {
                reason: StaleReason::ProjectRevision,
            });
        }
        if key.control_plan_generation != ctx.control_plan_generation {
            return Some(ObservationFreshness::Stale {
                reason: StaleReason::ControlPlan,
            });
        }
        if key.graph_generation != ctx.graph_generation {
            return Some(ObservationFreshness::Stale {
                reason: StaleReason::Graph,
            });
        }
        if key.transport_generation != ctx.transport_generation {
            return Some(ObservationFreshness::Stale {
                reason: StaleReason::Transport,
            });
        }
        if key.clock_epoch != ctx.clock_epoch {
            return Some(ObservationFreshness::Stale {
                reason: StaleReason::ClockEpoch,
            });
        }
        if key.loop_occurrence != ctx.loop_occurrence {
            return Some(ObservationFreshness::Stale {
                reason: StaleReason::Loop,
            });
        }
        if key.evaluation_position != ctx.evaluation_position {
            return Some(ObservationFreshness::Stale {
                reason: StaleReason::Position,
            });
        }
        None
    }

    fn freshness_for(&self, ctx: &FreshnessContext, channel: Channel) -> ObservationFreshness {
        if let Some(stale) = self.context_stale(ctx) {
            return stale;
        }
        self.channel_freshness(ctx, channel)
    }

    fn channel_freshness(&self, ctx: &FreshnessContext, channel: Channel) -> ObservationFreshness {
        match channel {
            Channel::PluginReported => match &self.plugin_reported {
                TraceSlot::Present(evidence) => stamp_freshness(&evidence.stamp, ctx),
                slot => slot_unavailable(slot),
            },
            Channel::Observed => match &self.observed {
                TraceSlot::Present(observation) => match observation {
                    ProbeObservation::Measuring { stamp, .. } => stamp_freshness(stamp, ctx),
                    ProbeObservation::Lost { count } => {
                        ObservationFreshness::DataLoss { count: *count }
                    }
                    ProbeObservation::Missing => ObservationFreshness::Unavailable {
                        reason: UnavailableReason::ProbeMissing,
                    },
                    ProbeObservation::Disabled => ObservationFreshness::Unavailable {
                        reason: UnavailableReason::ProbeDisabled,
                    },
                },
                slot => slot_unavailable(slot),
            },
        }
    }
}

#[derive(Copy, Clone)]
enum Channel {
    PluginReported,
    Observed,
}

fn slot_unavailable<T>(slot: &TraceSlot<T>) -> ObservationFreshness {
    let reason = match slot {
        TraceSlot::Missing => UnavailableReason::NoEvidence,
        TraceSlot::Unsupported => UnavailableReason::Unsupported,
        TraceSlot::Dropped => UnavailableReason::Dropped,
        TraceSlot::Expired => UnavailableReason::Expired,
        TraceSlot::Present(_) => unreachable!("Present handled by caller"),
    };
    ObservationFreshness::Unavailable { reason }
}

/// Evaluates one measurement stamp against the freshness context.
/// Age is checked before provenance so a too-old measurement reports
/// `Stale{Age}` rather than the weaker `Estimated`.
fn stamp_freshness(stamp: &MediaTimestamp, ctx: &FreshnessContext) -> ObservationFreshness {
    let Ok(deadline) = stamp.time.checked_add(ctx.max_age) else {
        return ObservationFreshness::Unavailable {
            reason: UnavailableReason::ClockUntrusted,
        };
    };
    if ctx.now > deadline {
        return ObservationFreshness::Stale {
            reason: StaleReason::Age,
        };
    }
    if ctx.now < stamp.time {
        return ObservationFreshness::Unavailable {
            reason: UnavailableReason::ClockUntrusted,
        };
    }
    if stamp.provenance == TimestampProvenance::Estimated {
        return ObservationFreshness::Estimated;
    }
    ObservationFreshness::Current
}

/// Live evaluation context for [`ControlTrace::freshness`] — the
/// current generations, epoch, position and clock against which the
/// trace key and evidence stamps are compared.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FreshnessContext {
    /// Currently committed project revision.
    pub project_revision: ProjectRevision,
    /// Currently staged dispatch plan generation.
    pub control_plan_generation: PlanGeneration,
    /// Currently active graph build generation.
    pub graph_generation: u64,
    /// Current transport generation.
    pub transport_generation: TransportGeneration,
    /// Current runtime device clock epoch.
    pub clock_epoch: ClockEpochId,
    /// Current evaluation position.
    pub evaluation_position: MediaTime,
    /// Current loop occurrence.
    pub loop_occurrence: LoopOccurrence,
    /// Current playhead — the reference for stamp age.
    pub now: MediaTime,
    /// Maximum acceptable measurement age.
    pub max_age: MediaDuration,
}

/// Why evidence is `Stale` — the mismatched context dimension.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum StaleReason {
    /// Project revision moved past the trace key's.
    ProjectRevision,
    /// Control plan generation moved on.
    ControlPlan,
    /// Graph build generation moved on.
    Graph,
    /// Transport generation moved on (Play/Pause/Stop/Seek).
    Transport,
    /// Device clock epoch changed (restart/re-anchor).
    ClockEpoch,
    /// Loop occurrence advanced — same positions, different pass.
    Loop,
    /// Evaluation position moved.
    Position,
    /// Measurement older than the age budget.
    Age,
}

/// Why observation evidence is `Unavailable`.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum UnavailableReason {
    /// The stage never produced evidence (`TraceSlot::Missing`).
    NoEvidence,
    /// The stage cannot produce evidence for this target.
    Unsupported,
    /// Evidence was dropped before correlation.
    Dropped,
    /// Evidence expired before correlation.
    Expired,
    /// The measurement clock cannot be trusted — stamp is from the
    /// future or the age deadline could not be computed.
    ClockUntrusted,
    /// No probe binding exists for the point.
    ProbeMissing,
    /// The probe binding exists but observation is disabled.
    ProbeDisabled,
    /// The measurement position is outside the observable range.
    OutOfRange,
}

/// Classified freshness of observation evidence. `Current` is the
/// only "live" verdict; every uncertainty resolves to `Estimated`,
/// `Stale`, `DataLoss` or `Unavailable` — never silently live.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ObservationFreshness {
    /// Evidence context matches and the stamp is within budget with
    /// trusted provenance.
    Current,
    /// Context matches but the stamp provenance is `Estimated` — the
    /// value is kept but not asserted as observed.
    Estimated,
    /// Evidence describes a superseded or too-old context.
    Stale {
        /// Which dimension mismatched.
        reason: StaleReason,
    },
    /// Observation data was lost in transit.
    DataLoss {
        /// Number of lost blocks/records, when known.
        count: u64,
    },
    /// No trustworthy observation exists.
    Unavailable {
        /// Why.
        reason: UnavailableReason,
    },
}

impl ObservationFreshness {
    fn rank(&self) -> u8 {
        match self {
            Self::Current => 0,
            Self::Estimated => 1,
            Self::Stale { .. } => 2,
            Self::DataLoss { .. } => 3,
            Self::Unavailable { .. } => 4,
        }
    }
}

/// The plan currently projected as active plus the most recent failed
/// attempt. `active` retains the last-good plan across a failed
/// publish — a failed plan can never occupy `active`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanActivationState {
    /// The plan projected as active (last-good semantics).
    pub active: Option<ActivePlan>,
    /// The most recent failed publish attempt — recorded, never
    /// projected as active.
    pub last_attempt: Option<FailedPlanAttempt>,
}

/// A plan confirmed active on a runtime instance.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ActivePlan {
    /// Active plan generation.
    pub plan_generation: PlanGeneration,
    /// Transport generation at activation.
    pub transport_generation: TransportGeneration,
    /// Clock epoch at activation.
    pub clock_epoch: ClockEpochId,
    /// Runtime instance the plan is active on — generations are only
    /// meaningful within their issuing instance.
    pub runtime_instance: RuntimeInstanceId,
}

/// A failed plan publish attempt — the fault taxonomy reuses
/// [`RoutingFault`] so activation failures stay in the existing
/// 18-variant vocabulary.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FailedPlanAttempt {
    /// The plan generation that failed.
    pub plan_generation: PlanGeneration,
    /// Why the publish/activation failed.
    pub fault: RoutingFault,
    /// Transport generation at the attempt.
    pub transport_generation: TransportGeneration,
    /// Clock epoch at the attempt.
    pub clock_epoch: ClockEpochId,
}
