//! PC-C6 oracle acceptance tests (TR01..TR08): independent-oracle
//! verification of [`ControlTrace`] freshness/coherence contracts
//! against Issue #103's fixed matrix.
//!
//! Every expected [`ObservationFreshness`] / coherence verdict is
//! derived by `oracle_*` functions re-implemented from the documented
//! contract — context-dimension check order, stamp age/provenance
//! rules, per-channel slot mapping, best-channel combination and
//! receipt-generation coherence — never by calling the methods under
//! test. All fixtures are synthetic headless values; no real
//! plugin/OS/RT observation is claimed. The `synvelis-player`
//! dispatch-ledger path and bounded history retention (TRACE-C3-05)
//! are out of scope and recorded as `NotRun` in the task docs.

use ruviform_media_core::{
    ApplyStatus, ClockEpochId as RuntimeClockEpochId, EngineFrame, LoopOccurrence, MediaDuration,
    MediaTime, MediaTimestamp, PlanGeneration, TimeBase, TimestampProvenance, TransportGeneration,
};
use ruviform_plugin_core::ParamValue;
use synvelis_project::{
    ActivePlan, AppliedStageReceipt, ConfiguredEvidence, ControlTrace, ControlTraceKey,
    DispatchEvidence, FailedPlanAttempt, FreshnessContext, MeasuredValue, ObservationFreshness,
    ParameterTargetId, PlanActivationState, PluginReportedEvidence, PreparedEvidence,
    ProbeObservation, ProjectId, ProjectRevision, ReceiptEvidence, RoutingFault, RuntimeInstanceId,
    ScheduledReceipt, StaleReason, TraceSlot, UnavailableReason,
};

// ---------------------------------------------------------------------------
// Synthetic fixtures — deliberately different ids/positions from
// `trace_c3.rs` so this oracle does not inherit its fixture choices.
// ---------------------------------------------------------------------------

fn time(sec: i64) -> MediaTime {
    MediaTime::new(sec * 90_000, TimeBase::COMMON_90K)
}

fn max_age() -> MediaDuration {
    MediaDuration::new(180_000, TimeBase::COMMON_90K) // 2 s at 90 kHz
}

fn key() -> ControlTraceKey {
    ControlTraceKey {
        project_id: ProjectId::new(9),
        parameter_target_id: ParameterTargetId::new(33),
        project_revision: ProjectRevision(21),
        control_plan_generation: PlanGeneration::new(8),
        graph_generation: 23,
        transport_generation: TransportGeneration::new(13),
        clock_epoch: RuntimeClockEpochId::new(4),
        loop_occurrence: LoopOccurrence::new(2),
        evaluation_position: time(20),
    }
}

fn ctx() -> FreshnessContext {
    FreshnessContext {
        project_revision: ProjectRevision(21),
        control_plan_generation: PlanGeneration::new(8),
        graph_generation: 23,
        transport_generation: TransportGeneration::new(13),
        clock_epoch: RuntimeClockEpochId::new(4),
        evaluation_position: time(20),
        loop_occurrence: LoopOccurrence::new(2),
        now: time(20),
        max_age: max_age(),
    }
}

fn receipt(status: ApplyStatus) -> ReceiptEvidence {
    ReceiptEvidence {
        status,
        applied_frame: None,
        transport_generation: TransportGeneration::new(13),
        plan_generation: PlanGeneration::new(8),
        clock_epoch: RuntimeClockEpochId::new(4),
    }
}

fn stamp_at(when: MediaTime, provenance: TimestampProvenance) -> MediaTimestamp {
    MediaTimestamp {
        time: when,
        provenance,
    }
}

fn value(f: f64) -> ParamValue {
    ParamValue::new(f).unwrap()
}

/// A coherent trace with only configure/prepared evidence — every
/// observation channel empty.
fn base_trace() -> ControlTrace {
    ControlTrace {
        key: key(),
        configured: TraceSlot::Present(ConfiguredEvidence {
            host_normalized: 0.25,
            saturated: false,
            quantized: true,
        }),
        prepared: TraceSlot::Present(PreparedEvidence {
            plan_generation: PlanGeneration::new(8),
        }),
        scheduled: TraceSlot::Missing,
        applied: TraceSlot::Missing,
        dispatched: TraceSlot::Missing,
        plugin_reported: TraceSlot::Missing,
        observed: TraceSlot::Missing,
    }
}

fn fresh_reported() -> TraceSlot<PluginReportedEvidence> {
    TraceSlot::Present(PluginReportedEvidence {
        value: value(0.5),
        stamp: stamp_at(time(20), TimestampProvenance::Carried),
    })
}

fn measuring(value_kind: MeasuredValue) -> TraceSlot<ProbeObservation> {
    TraceSlot::Present(ProbeObservation::Measuring {
        stamp: stamp_at(time(20), TimestampProvenance::Carried),
        value: value_kind,
    })
}

// ---------------------------------------------------------------------------
// Independent oracle — re-implements the documented contract. None of
// these functions call `ControlTrace` methods.
// ---------------------------------------------------------------------------

#[derive(Copy, Clone)]
enum OracleChannel {
    PluginReported,
    Observed,
}

/// Freshness rank — `Current` is the only live verdict; higher rank is
/// "less trustworthy". `non_exhaustive` forces a wildcard; unknown
/// future variants rank worst so they can never look live.
fn oracle_rank(freshness: &ObservationFreshness) -> u8 {
    match freshness {
        ObservationFreshness::Current => 0,
        ObservationFreshness::Estimated => 1,
        ObservationFreshness::Stale { .. } => 2,
        ObservationFreshness::DataLoss { .. } => 3,
        _ => 4, // Unavailable and any future variant
    }
}

/// First context mismatch in the documented deterministic order:
/// project revision, plan generation, graph generation, transport
/// generation, clock epoch, loop occurrence, evaluation position.
fn oracle_context_stale(
    trace: &ControlTrace,
    context: &FreshnessContext,
) -> Option<ObservationFreshness> {
    let key = &trace.key;
    if key.project_revision != context.project_revision {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::ProjectRevision,
        });
    }
    if key.control_plan_generation != context.control_plan_generation {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::ControlPlan,
        });
    }
    if key.graph_generation != context.graph_generation {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::Graph,
        });
    }
    if key.transport_generation != context.transport_generation {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::Transport,
        });
    }
    if key.clock_epoch != context.clock_epoch {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::ClockEpoch,
        });
    }
    if key.loop_occurrence != context.loop_occurrence {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::Loop,
        });
    }
    if key.evaluation_position != context.evaluation_position {
        return Some(ObservationFreshness::Stale {
            reason: StaleReason::Position,
        });
    }
    None
}

/// Non-`Present` slots map to their recorded absence reason. `Present`
/// is handled by callers; unknown future slot variants degrade to the
/// conservative `NoEvidence`.
fn oracle_slot_unavailable<T>(slot: &TraceSlot<T>) -> ObservationFreshness {
    let reason = match slot {
        TraceSlot::Unsupported => UnavailableReason::Unsupported,
        TraceSlot::Dropped => UnavailableReason::Dropped,
        TraceSlot::Expired => UnavailableReason::Expired,
        // Missing and any unknown future variant → conservative
        // NoEvidence (Present is handled by callers).
        _ => UnavailableReason::NoEvidence,
    };
    ObservationFreshness::Unavailable { reason }
}

/// Stamp evaluation: uncomputable deadline or future stamp →
/// `ClockUntrusted`; older than the age budget → `Stale{Age}` (age is
/// checked before provenance); estimated provenance → Estimated;
/// otherwise Current. `now == deadline` and `now == stamp.time` are
/// *not* stale (strict comparisons).
fn oracle_stamp_freshness(
    stamp: &MediaTimestamp,
    context: &FreshnessContext,
) -> ObservationFreshness {
    let Ok(deadline) = stamp.time.checked_add(context.max_age) else {
        return ObservationFreshness::Unavailable {
            reason: UnavailableReason::ClockUntrusted,
        };
    };
    if context.now > deadline {
        return ObservationFreshness::Stale {
            reason: StaleReason::Age,
        };
    }
    if context.now < stamp.time {
        return ObservationFreshness::Unavailable {
            reason: UnavailableReason::ClockUntrusted,
        };
    }
    if stamp.provenance == TimestampProvenance::Estimated {
        return ObservationFreshness::Estimated;
    }
    ObservationFreshness::Current
}

fn oracle_channel_freshness(
    trace: &ControlTrace,
    context: &FreshnessContext,
    channel: OracleChannel,
) -> ObservationFreshness {
    if let Some(stale) = oracle_context_stale(trace, context) {
        return stale;
    }
    match channel {
        OracleChannel::PluginReported => match trace.plugin_reported.present() {
            Some(evidence) => oracle_stamp_freshness(&evidence.stamp, context),
            None => oracle_slot_unavailable(&trace.plugin_reported),
        },
        OracleChannel::Observed => match trace.observed.present() {
            Some(ProbeObservation::Measuring { stamp, .. }) => {
                oracle_stamp_freshness(stamp, context)
            }
            Some(ProbeObservation::Lost { count }) => {
                ObservationFreshness::DataLoss { count: *count }
            }
            Some(ProbeObservation::Missing) => ObservationFreshness::Unavailable {
                reason: UnavailableReason::ProbeMissing,
            },
            Some(ProbeObservation::Disabled) => ObservationFreshness::Unavailable {
                reason: UnavailableReason::ProbeDisabled,
            },
            // Unknown future observation kinds fail closed.
            Some(_) => ObservationFreshness::Unavailable {
                reason: UnavailableReason::NoEvidence,
            },
            None => oracle_slot_unavailable(&trace.observed),
        },
    }
}

/// Combined freshness — the *best* (lowest-rank) channel answer; equal
/// rank prefers a slot that actually holds evidence, then the probe
/// channel.
fn oracle_freshness(trace: &ControlTrace, context: &FreshnessContext) -> ObservationFreshness {
    let observed = oracle_channel_freshness(trace, context, OracleChannel::Observed);
    let reported = oracle_channel_freshness(trace, context, OracleChannel::PluginReported);
    let observed_score = (oracle_rank(&observed), !trace.observed.is_present());
    let reported_score = (oracle_rank(&reported), !trace.plugin_reported.is_present());
    if observed_score <= reported_score {
        observed
    } else {
        reported
    }
}

/// Receipt-generation coherence, re-derived: prepared plan generation
/// must equal the key's; scheduled/applied receipts must match plan,
/// transport and clock epoch; dispatched Scheduled/Dispatched/Dropped
/// must match plan generation (Reported/Unavailable carry none).
fn oracle_is_coherent(trace: &ControlTrace) -> bool {
    let key = &trace.key;
    if let Some(evidence) = trace.prepared.present()
        && evidence.plan_generation != key.control_plan_generation
    {
        return false;
    }
    let receipt_coherent = |receipt: ReceiptEvidence| {
        receipt.plan_generation == key.control_plan_generation
            && receipt.transport_generation == key.transport_generation
            && receipt.clock_epoch == key.clock_epoch
    };
    if let Some(receipt) = trace.scheduled.present()
        && !receipt_coherent(receipt.receipt())
    {
        return false;
    }
    if let Some(receipt) = trace.applied.present()
        && !receipt_coherent(receipt.receipt())
    {
        return false;
    }
    if let Some(evidence) = trace.dispatched.present() {
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
            // Unknown future variants cannot be verified — fail closed.
            _ => return false,
        };
        if let Some(generation) = plan_generation
            && generation != key.control_plan_generation
        {
            return false;
        }
    }
    true
}

/// The core oracle assertion: all three freshness entry points and the
/// coherence verdict must equal the independently derived answers.
fn assert_trace_agrees(trace: &ControlTrace, context: &FreshnessContext) {
    assert_eq!(
        trace.freshness(context),
        oracle_freshness(trace, context),
        "freshness oracle mismatch"
    );
    assert_eq!(
        trace.plugin_reported_freshness(context),
        oracle_channel_freshness(trace, context, OracleChannel::PluginReported),
        "plugin_reported freshness oracle mismatch"
    );
    assert_eq!(
        trace.observed_freshness(context),
        oracle_channel_freshness(trace, context, OracleChannel::Observed),
        "observed freshness oracle mismatch"
    );
    assert_eq!(
        trace.is_coherent(),
        oracle_is_coherent(trace),
        "coherence oracle mismatch"
    );
}

// ---------------------------------------------------------------------------
// TR01 — per-stage independence: no stage is inferred from another.
// ---------------------------------------------------------------------------

#[test]
fn tr01_stage_independence() {
    let live = ctx();

    // Only configure/prepared evidence: every observation channel is
    // empty → Unavailable{NoEvidence}, never synthesized.
    let trace = base_trace();
    assert_trace_agrees(&trace, &live);
    assert_eq!(
        trace.freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::NoEvidence,
        }
    );

    // Filling non-observation stages one at a time must not light up
    // either observation channel.
    let mut trace = base_trace();
    trace.scheduled =
        TraceSlot::Present(ScheduledReceipt::new(receipt(ApplyStatus::Pending)).unwrap());
    assert_trace_agrees(&trace, &live);
    assert_eq!(
        trace.freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::NoEvidence,
        }
    );
    assert!(trace.is_coherent());
    assert!(oracle_is_coherent(&trace));

    trace.applied = TraceSlot::Present(
        AppliedStageReceipt::new(ReceiptEvidence {
            applied_frame: Some(EngineFrame::new(RuntimeClockEpochId::new(4), 960)),
            ..receipt(ApplyStatus::Applied)
        })
        .unwrap(),
    );
    trace.dispatched = TraceSlot::Present(DispatchEvidence::Dispatched {
        value: value(0.5),
        position: time(20),
        plan_generation: PlanGeneration::new(8),
        call_start: EngineFrame::new(RuntimeClockEpochId::new(4), 1024),
        frame_offset: 5,
    });
    assert_trace_agrees(&trace, &live);
    // Dispatched is a projection — it is not plugin acknowledgment and
    // implies nothing about observation channels.
    assert_eq!(
        trace.freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::NoEvidence,
        }
    );
    assert!(trace.is_coherent());

    // Each non-Present slot variant maps to its own absence reason —
    // checked per channel, per variant.
    for slot in [
        TraceSlot::Missing,
        TraceSlot::Unsupported,
        TraceSlot::Dropped,
        TraceSlot::Expired,
    ] {
        let mut t = base_trace();
        t.plugin_reported = slot;
        assert_trace_agrees(&t, &live);
        let expected = match slot {
            TraceSlot::Missing => UnavailableReason::NoEvidence,
            TraceSlot::Unsupported => UnavailableReason::Unsupported,
            TraceSlot::Dropped => UnavailableReason::Dropped,
            TraceSlot::Expired => UnavailableReason::Expired,
            _ => unreachable!(),
        };
        assert_eq!(
            t.plugin_reported_freshness(&live),
            ObservationFreshness::Unavailable { reason: expected }
        );
    }
}

// ---------------------------------------------------------------------------
// TR02 — every context mismatch dimension classifies Stale, named.
// ---------------------------------------------------------------------------

#[test]
fn tr02_context_mismatch_dimensions() {
    let mut trace = base_trace();
    trace.plugin_reported = fresh_reported();
    let live = ctx();
    assert_trace_agrees(&trace, &live);
    assert_eq!(trace.freshness(&live), ObservationFreshness::Current);

    // (mutated context, expected stale reason) — the oracle derives the
    // reason independently; the literal pins the documented order.
    let cases: [(FreshnessContext, StaleReason); 7] = [
        (
            FreshnessContext {
                project_revision: ProjectRevision(22),
                ..live
            },
            StaleReason::ProjectRevision,
        ),
        (
            FreshnessContext {
                control_plan_generation: PlanGeneration::new(9),
                ..live
            },
            StaleReason::ControlPlan,
        ),
        (
            FreshnessContext {
                graph_generation: 24,
                ..live
            },
            StaleReason::Graph,
        ),
        (
            FreshnessContext {
                transport_generation: TransportGeneration::new(14),
                ..live
            },
            StaleReason::Transport,
        ),
        (
            FreshnessContext {
                clock_epoch: RuntimeClockEpochId::new(5),
                ..live
            },
            StaleReason::ClockEpoch,
        ),
        (
            FreshnessContext {
                loop_occurrence: LoopOccurrence::new(3),
                ..live
            },
            StaleReason::Loop,
        ),
        (
            FreshnessContext {
                evaluation_position: time(21),
                ..live
            },
            StaleReason::Position,
        ),
    ];
    for (stale_ctx, reason) in cases {
        let expected = ObservationFreshness::Stale { reason };
        // Oracle derives the same verdict from the documented rules…
        assert_eq!(oracle_freshness(&trace, &stale_ctx), expected);
        // …and the implementation agrees with the oracle on every
        // freshness entry point.
        assert_trace_agrees(&trace, &stale_ctx);
        assert_eq!(trace.freshness(&stale_ctx), expected);
    }

    // Multiple simultaneous mismatches resolve to the first dimension
    // in check order (project revision beats everything).
    let everything = FreshnessContext {
        project_revision: ProjectRevision(99),
        control_plan_generation: PlanGeneration::new(99),
        graph_generation: 99,
        transport_generation: TransportGeneration::new(99),
        clock_epoch: RuntimeClockEpochId::new(99),
        evaluation_position: time(99),
        loop_occurrence: LoopOccurrence::new(99),
        ..live
    };
    assert_eq!(
        trace.freshness(&everything),
        ObservationFreshness::Stale {
            reason: StaleReason::ProjectRevision,
        }
    );
    assert_trace_agrees(&trace, &everything);
}

// ---------------------------------------------------------------------------
// TR03 — seek / loop wrap / pause-restart: generation & epoch moves make
// old evidence Stale; coherence is key-internal and stays true.
// ---------------------------------------------------------------------------

#[test]
fn tr03_transport_seek_loop_restart() {
    let mut trace = base_trace();
    trace.plugin_reported = fresh_reported();
    trace.observed = measuring(MeasuredValue::Signal(value(0.5)));
    let live = ctx();
    assert_trace_agrees(&trace, &live);
    assert_eq!(trace.freshness(&live), ObservationFreshness::Current);
    assert!(trace.is_coherent());

    // Scenario table: each transport-ish event bumps exactly the
    // documented context dimension; the old trace must go Stale with
    // that dimension named — never silently Current.
    let scenarios: [(FreshnessContext, StaleReason, &str); 6] = [
        (
            FreshnessContext {
                transport_generation: TransportGeneration::new(14),
                evaluation_position: time(21),
                ..live
            },
            StaleReason::Transport,
            "seek",
        ),
        (
            FreshnessContext {
                transport_generation: TransportGeneration::new(15),
                ..live
            },
            StaleReason::Transport,
            "pause/resume",
        ),
        (
            FreshnessContext {
                loop_occurrence: LoopOccurrence::new(3),
                ..live
            },
            StaleReason::Loop,
            "loop wrap",
        ),
        (
            FreshnessContext {
                clock_epoch: RuntimeClockEpochId::new(5),
                ..live
            },
            StaleReason::ClockEpoch,
            "device restart",
        ),
        (
            FreshnessContext {
                control_plan_generation: PlanGeneration::new(9),
                ..live
            },
            StaleReason::ControlPlan,
            "plan republish",
        ),
        (
            FreshnessContext {
                project_revision: ProjectRevision(22),
                ..live
            },
            StaleReason::ProjectRevision,
            "project edit",
        ),
    ];
    for (moved, reason, label) in scenarios {
        assert_eq!(
            oracle_freshness(&trace, &moved),
            ObservationFreshness::Stale { reason },
            "oracle: {label}"
        );
        assert_trace_agrees(&trace, &moved);
        assert_eq!(
            trace.freshness(&moved),
            ObservationFreshness::Stale { reason },
            "{label} must stale the trace"
        );
        // Coherence is key-internal: the trace still describes itself
        // coherently — it is simply stale against the live context.
        assert!(trace.is_coherent(), "{label}: coherence is ctx-free");
        assert!(oracle_is_coherent(&trace));
    }

    // A receipt carrying a *foreign* generation is incoherent — the
    // oracle agrees it must not be trusted as describing this trace.
    let mut foreign = base_trace();
    foreign.applied = TraceSlot::Present(
        AppliedStageReceipt::new(ReceiptEvidence {
            plan_generation: PlanGeneration::new(77),
            ..receipt(ApplyStatus::Applied)
        })
        .unwrap(),
    );
    assert!(!foreign.is_coherent());
    assert!(!oracle_is_coherent(&foreign));
    assert_trace_agrees(&foreign, &live);
}

// ---------------------------------------------------------------------------
// TR04 — plugin report absent/unknown vs probe states; dispatch
// terminals are coherent but carry no observation.
// ---------------------------------------------------------------------------

#[test]
fn tr04_plugin_unknown_vs_probe_states() {
    let live = ctx();

    // Plugin channel empty + probe measuring → probe alone answers
    // Current; the plugin channel still reports its own absence.
    let mut trace = base_trace();
    trace.observed = measuring(MeasuredValue::Signal(value(0.5)));
    assert_trace_agrees(&trace, &live);
    assert_eq!(trace.freshness(&live), ObservationFreshness::Current);
    assert_eq!(
        trace.plugin_reported_freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::NoEvidence,
        }
    );

    // Fresh plugin claim + lost probe blocks → combined freshness is
    // the plugin's Current; the probe channel keeps reporting DataLoss.
    trace.plugin_reported = fresh_reported();
    trace.observed = TraceSlot::Present(ProbeObservation::Lost { count: 7 });
    assert_trace_agrees(&trace, &live);
    assert_eq!(trace.freshness(&live), ObservationFreshness::Current);
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::DataLoss { count: 7 }
    );

    // Degraded on both channels: Estimated report vs DataLoss probe —
    // the best (lowest-rank) answer wins, and it is still not Current.
    trace.plugin_reported = TraceSlot::Present(PluginReportedEvidence {
        value: value(0.5),
        stamp: stamp_at(time(20), TimestampProvenance::Estimated),
    });
    assert_trace_agrees(&trace, &live);
    assert_eq!(trace.freshness(&live), ObservationFreshness::Estimated);
    assert_eq!(
        trace.plugin_reported_freshness(&live),
        ObservationFreshness::Estimated
    );
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::DataLoss { count: 7 }
    );

    // Dispatch terminals Reported/Unavailable are coherent but imply
    // nothing about observation — freshness stays Unavailable.
    let mut dispatch_only = base_trace();
    dispatch_only.dispatched = TraceSlot::Present(DispatchEvidence::Reported {
        value: value(0.5),
        position: time(20),
    });
    assert_trace_agrees(&dispatch_only, &live);
    assert!(dispatch_only.is_coherent());
    assert_eq!(
        dispatch_only.freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::NoEvidence,
        }
    );
    dispatch_only.dispatched =
        TraceSlot::Present(DispatchEvidence::Unavailable { position: time(20) });
    assert_trace_agrees(&dispatch_only, &live);
    assert!(dispatch_only.is_coherent());

    // A Dropped dispatch record with a foreign plan generation is
    // incoherent — attributed drops still must describe this trace.
    let mut dropped_foreign = base_trace();
    dropped_foreign.dispatched = TraceSlot::Present(DispatchEvidence::Dropped {
        position: time(20),
        plan_generation: PlanGeneration::new(55),
        reason: synvelis_project::DispatchDropReason::PlanReplaced,
    });
    assert!(!dropped_foreign.is_coherent());
    assert!(!oracle_is_coherent(&dropped_foreign));
    assert_trace_agrees(&dropped_foreign, &live);
}

// ---------------------------------------------------------------------------
// TR05 — stamp provenance/age matrix, including strict boundaries.
// ---------------------------------------------------------------------------

#[test]
fn tr05_stamp_provenance_and_age_matrix() {
    let live = ctx();

    // (stamp, now offset seconds, expected freshness) — expected values
    // are asserted through the oracle *and* pinned literally.
    let cases: &[(MediaTimestamp, i64, ObservationFreshness)] = &[
        // Estimated provenance never reaches Current.
        (
            stamp_at(time(20), TimestampProvenance::Estimated),
            0,
            ObservationFreshness::Estimated,
        ),
        // Trusted stamp at evaluation position → Current.
        (
            stamp_at(time(20), TimestampProvenance::Carried),
            0,
            ObservationFreshness::Current,
        ),
        // now == stamp.time exactly (stamp one second in the past
        // relative to `now`) → Current; the future check is strict.
        (
            stamp_at(time(19), TimestampProvenance::Carried),
            -1,
            ObservationFreshness::Current,
        ),
        // now == deadline (stamp + max_age = time(22)) → still Current.
        (
            stamp_at(time(20), TimestampProvenance::Carried),
            2,
            ObservationFreshness::Current,
        ),
        // One tick past the deadline → Stale{Age}.
        (
            stamp_at(time(20), TimestampProvenance::Carried),
            3,
            ObservationFreshness::Stale {
                reason: StaleReason::Age,
            },
        ),
        // Future stamp → clock untrusted, never Current.
        (
            stamp_at(time(21), TimestampProvenance::Carried),
            0,
            ObservationFreshness::Unavailable {
                reason: UnavailableReason::ClockUntrusted,
            },
        ),
        // Estimated + too old → Stale{Age} wins over Estimated
        // (age is checked before provenance).
        (
            stamp_at(time(20), TimestampProvenance::Estimated),
            3,
            ObservationFreshness::Stale {
                reason: StaleReason::Age,
            },
        ),
    ];

    for &(stamp, now_sec, expected) in cases {
        let mut trace = base_trace();
        trace.plugin_reported = TraceSlot::Present(PluginReportedEvidence {
            value: value(0.5),
            stamp,
        });
        let context = FreshnessContext {
            now: time(20 + now_sec),
            ..live
        };
        // The oracle derives the verdict independently…
        assert_eq!(
            oracle_channel_freshness(&trace, &context, OracleChannel::PluginReported),
            expected,
            "oracle: stamp={stamp:?} now=+{now_sec}s"
        );
        assert_trace_agrees(&trace, &context);
        // …and the implementation matches it.
        assert_eq!(
            trace.plugin_reported_freshness(&context),
            expected,
            "impl: stamp={stamp:?} now=+{now_sec}s"
        );
    }

    // A stamp whose age deadline cannot be computed (time near i64::MAX)
    // is ClockUntrusted — never Current.
    let mut trace = base_trace();
    trace.plugin_reported = TraceSlot::Present(PluginReportedEvidence {
        value: value(0.5),
        stamp: stamp_at(
            MediaTime::new(i64::MAX, TimeBase::COMMON_90K),
            TimestampProvenance::Carried,
        ),
    });
    assert_trace_agrees(&trace, &live);
    assert_eq!(
        trace.plugin_reported_freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::ClockUntrusted,
        }
    );
}

// ---------------------------------------------------------------------------
// TR06 — measured silence is an observation; missing/disabled/lost are
// channel states, never conflated.
// ---------------------------------------------------------------------------

#[test]
fn tr06_silence_vs_unobserved() {
    let live = ctx();

    // Measured silence IS evidence — Current, distinct from every
    // non-measurement state.
    let mut trace = base_trace();
    trace.observed = measuring(MeasuredValue::Silence);
    assert_trace_agrees(&trace, &live);
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::Current
    );
    assert_eq!(trace.freshness(&live), ObservationFreshness::Current);

    // Channel states — each a different honest answer.
    let states: &[(TraceSlot<ProbeObservation>, ObservationFreshness)] = &[
        (
            TraceSlot::Present(ProbeObservation::Missing),
            ObservationFreshness::Unavailable {
                reason: UnavailableReason::ProbeMissing,
            },
        ),
        (
            TraceSlot::Present(ProbeObservation::Disabled),
            ObservationFreshness::Unavailable {
                reason: UnavailableReason::ProbeDisabled,
            },
        ),
        (
            TraceSlot::Present(ProbeObservation::Lost { count: 2 }),
            ObservationFreshness::DataLoss { count: 2 },
        ),
        (
            TraceSlot::Missing,
            ObservationFreshness::Unavailable {
                reason: UnavailableReason::NoEvidence,
            },
        ),
        (
            TraceSlot::Expired,
            ObservationFreshness::Unavailable {
                reason: UnavailableReason::Expired,
            },
        ),
    ];
    for (slot, expected) in states {
        let mut t = base_trace();
        t.observed = *slot;
        assert_trace_agrees(&t, &live);
        assert_eq!(
            t.observed_freshness(&live),
            *expected,
            "observed slot {slot:?}"
        );
        // A measured value is never implied by a channel state.
        assert_ne!(
            t.observed_freshness(&live),
            ObservationFreshness::Current,
            "{slot:?} must not read as a measurement"
        );
    }
}

// ---------------------------------------------------------------------------
// TR07 — shared probe evidence across two trace contexts: freshness is
// per-trace, never a global property of the observation.
// ---------------------------------------------------------------------------

#[test]
fn tr07_shared_probe_two_contexts() {
    // Two traces for different targets sharing the *same* probe
    // observation evidence — one coherent with the live context, one
    // behind on transport generation.
    let shared_observation = ProbeObservation::Measuring {
        stamp: stamp_at(time(20), TimestampProvenance::Carried),
        value: MeasuredValue::Signal(value(0.5)),
    };

    let mut live_trace = base_trace();
    live_trace.observed = TraceSlot::Present(shared_observation);
    let mut stale_trace = base_trace();
    stale_trace.key.transport_generation = TransportGeneration::new(12);
    stale_trace.observed = TraceSlot::Present(shared_observation);

    let live = ctx();
    let moved = FreshnessContext {
        transport_generation: TransportGeneration::new(12),
        ..live
    };

    // Same evidence bytes, opposite verdicts — the probe channel does
    // not conflate the two inspectors/targets.
    assert_trace_agrees(&live_trace, &live);
    assert_trace_agrees(&stale_trace, &moved);
    assert_trace_agrees(&live_trace, &moved);
    assert_trace_agrees(&stale_trace, &live);

    assert_eq!(
        live_trace.observed_freshness(&live),
        ObservationFreshness::Current
    );
    assert_eq!(
        stale_trace.observed_freshness(&live),
        ObservationFreshness::Stale {
            reason: StaleReason::Transport,
        }
    );
    // And the stale trace is Current under *its own* context.
    assert_eq!(
        stale_trace.observed_freshness(&moved),
        ObservationFreshness::Current
    );
}

// ---------------------------------------------------------------------------
// TR08 — deterministic repeated evaluation + fixed-size bounded trace.
// Real bounded-history/RT retention is deferred (TRACE-C3-05).
// ---------------------------------------------------------------------------

#[test]
fn tr08_determinism_and_bounded_size() {
    let live = ctx();
    let mut trace = base_trace();
    trace.plugin_reported = fresh_reported();
    trace.observed = measuring(MeasuredValue::Silence);
    trace.dispatched = TraceSlot::Present(DispatchEvidence::Dispatched {
        value: value(0.5),
        position: time(20),
        plan_generation: PlanGeneration::new(8),
        call_start: EngineFrame::new(RuntimeClockEpochId::new(4), 2048),
        frame_offset: 9,
    });

    // Repeated evaluation is pure: identical verdicts every time.
    let first = (
        trace.freshness(&live),
        trace.plugin_reported_freshness(&live),
        trace.observed_freshness(&live),
        trace.is_coherent(),
    );
    for _ in 0..256 {
        assert_eq!(
            (
                trace.freshness(&live),
                trace.plugin_reported_freshness(&live),
                trace.observed_freshness(&live),
                trace.is_coherent(),
            ),
            first,
            "freshness/coherence must be deterministic"
        );
    }

    // The trace is a fixed-size projection — bounded by construction,
    // not by a retention policy. Sanity bound only; real bounded
    // history lives in the (deferred) dispatch ledger.
    assert!(
        std::mem::size_of::<ControlTrace>() <= 1024,
        "ControlTrace grew beyond the fixed-slot bound: {} bytes",
        std::mem::size_of::<ControlTrace>()
    );
    assert!(std::mem::size_of::<ControlTraceKey>() <= 256);

    // Failed-plan last-good semantics (TRACE-C3-04 tail): a failed
    // publish is recorded but never displaces the active plan.
    let plan_a = ActivePlan {
        plan_generation: PlanGeneration::new(8),
        transport_generation: TransportGeneration::new(13),
        clock_epoch: RuntimeClockEpochId::new(4),
        runtime_instance: RuntimeInstanceId(1),
    };
    let failed_b = FailedPlanAttempt {
        plan_generation: PlanGeneration::new(9),
        fault: RoutingFault::PublishQueueFull,
        transport_generation: TransportGeneration::new(13),
        clock_epoch: RuntimeClockEpochId::new(4),
    };
    let activation = PlanActivationState {
        active: Some(plan_a),
        last_attempt: Some(failed_b),
    };
    let active = activation.active.expect("last-good plan stays active");
    assert_eq!(active.plan_generation, PlanGeneration::new(8));
    let attempt = activation.last_attempt.expect("failure is recorded");
    assert_eq!(attempt.plan_generation, PlanGeneration::new(9));
    assert_ne!(active.plan_generation, attempt.plan_generation);
}
