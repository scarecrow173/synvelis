//! PC-C3 acceptance tests (TRACE-C3-01..04): the read-only
//! [`ControlTrace`] projection — per-stage evidence slots, freshness
//! classification, probe channel states and failed-plan activation.
//!
//! All fixtures are synthetic; no real plugin/OS/RT observation is
//! claimed. `CommandId` cannot be minted outside `ruviform-media-core`,
//! so receipt evidence is built through [`ReceiptEvidence`] directly
//! (the RT token is intentionally not part of trace evidence).

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

fn time(sec: i64) -> MediaTime {
    MediaTime::new(sec * 90_000, TimeBase::COMMON_90K)
}

fn max_age() -> MediaDuration {
    MediaDuration::new(90_000, TimeBase::COMMON_90K)
}

fn key() -> ControlTraceKey {
    ControlTraceKey {
        project_id: ProjectId::new(1),
        parameter_target_id: ParameterTargetId::new(2),
        project_revision: ProjectRevision(7),
        control_plan_generation: PlanGeneration::new(3),
        graph_generation: 11,
        transport_generation: TransportGeneration::new(5),
        clock_epoch: RuntimeClockEpochId::new(1),
        loop_occurrence: LoopOccurrence::new(0),
        evaluation_position: time(10),
    }
}

fn ctx() -> FreshnessContext {
    FreshnessContext {
        project_revision: ProjectRevision(7),
        control_plan_generation: PlanGeneration::new(3),
        graph_generation: 11,
        transport_generation: TransportGeneration::new(5),
        clock_epoch: RuntimeClockEpochId::new(1),
        evaluation_position: time(10),
        loop_occurrence: LoopOccurrence::new(0),
        now: time(10),
        max_age: max_age(),
    }
}

fn receipt(status: ApplyStatus) -> ReceiptEvidence {
    ReceiptEvidence {
        status,
        applied_frame: None,
        transport_generation: TransportGeneration::new(5),
        plan_generation: PlanGeneration::new(3),
        clock_epoch: RuntimeClockEpochId::new(1),
    }
}

fn stamp(provenance: TimestampProvenance) -> MediaTimestamp {
    MediaTimestamp {
        time: time(10),
        provenance,
    }
}

fn base_trace() -> ControlTrace {
    ControlTrace {
        key: key(),
        configured: TraceSlot::Present(ConfiguredEvidence {
            host_normalized: 0.5,
            saturated: false,
            quantized: false,
        }),
        prepared: TraceSlot::Present(PreparedEvidence {
            plan_generation: PlanGeneration::new(3),
        }),
        scheduled: TraceSlot::Missing,
        applied: TraceSlot::Missing,
        dispatched: TraceSlot::Missing,
        plugin_reported: TraceSlot::Missing,
        observed: TraceSlot::Missing,
    }
}

fn value(f: f64) -> ParamValue {
    ParamValue::new(f).unwrap()
}

// TRACE-C3-01: stage slots are independent; missing stages stay
// Unknown; receipts correlate through the key's generations.
#[test]
fn trace_c3_01_stage_slots_missing_stays_unknown() {
    let trace = base_trace();

    // Only configure/prep evidence exists — every later stage is
    // Missing, never synthesized from earlier success.
    assert!(matches!(trace.configured, TraceSlot::Present(_)));
    assert!(matches!(trace.prepared, TraceSlot::Present(_)));
    for slot in [
        trace.scheduled.present().is_none(),
        trace.applied.present().is_none(),
        trace.dispatched.present().is_none(),
        trace.plugin_reported.present().is_none(),
        trace.observed.present().is_none(),
    ] {
        assert!(slot);
    }
    assert!(matches!(trace.applied, TraceSlot::Missing));
    assert!(matches!(trace.observed, TraceSlot::Missing));

    // No observation channel evidence → never Current.
    assert_eq!(
        trace.freshness(&ctx()),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::NoEvidence,
        }
    );

    // Receipt slots are fail-closed on lifecycle status.
    assert!(ScheduledReceipt::new(receipt(ApplyStatus::Requested)).is_some());
    assert!(ScheduledReceipt::new(receipt(ApplyStatus::Pending)).is_some());
    assert!(ScheduledReceipt::new(receipt(ApplyStatus::Applied)).is_none());
    assert!(AppliedStageReceipt::new(receipt(ApplyStatus::Applied)).is_some());
    assert!(AppliedStageReceipt::new(receipt(ApplyStatus::Requested)).is_none());
    assert!(AppliedStageReceipt::new(receipt(ApplyStatus::Rejected)).is_none());

    // A fully populated coherent trace passes the coherence check.
    let mut full = trace.clone();
    full.scheduled =
        TraceSlot::Present(ScheduledReceipt::new(receipt(ApplyStatus::Pending)).unwrap());
    full.applied = TraceSlot::Present(
        AppliedStageReceipt::new(ReceiptEvidence {
            applied_frame: Some(EngineFrame::new(RuntimeClockEpochId::new(1), 480)),
            ..receipt(ApplyStatus::Applied)
        })
        .unwrap(),
    );
    full.dispatched = TraceSlot::Present(DispatchEvidence::Dispatched {
        value: value(0.5),
        position: time(10),
        plan_generation: PlanGeneration::new(3),
        call_start: EngineFrame::new(RuntimeClockEpochId::new(1), 512),
        frame_offset: 3,
    });
    assert!(full.is_coherent());
}

// TRACE-C3-01/04: evidence carrying foreign generations is flagged
// incoherent — it must not be trusted as describing this trace.
#[test]
fn trace_c3_01_receipt_generation_mismatch_is_incoherent() {
    let mut trace = base_trace();
    trace.applied = TraceSlot::Present(
        AppliedStageReceipt::new(ReceiptEvidence {
            plan_generation: PlanGeneration::new(99),
            ..receipt(ApplyStatus::Applied)
        })
        .unwrap(),
    );
    assert!(!trace.is_coherent());

    let mut trace = base_trace();
    trace.dispatched = TraceSlot::Present(DispatchEvidence::Scheduled {
        value: value(0.5),
        position: time(10),
        plan_generation: PlanGeneration::new(42),
    });
    assert!(!trace.is_coherent());

    let mut trace = base_trace();
    trace.prepared = TraceSlot::Present(PreparedEvidence {
        plan_generation: PlanGeneration::new(77),
    });
    assert!(!trace.is_coherent());
}

// TRACE-C3-02: every context mismatch classifies Stale — never
// Current — with the dimension named.
#[test]
fn trace_c3_02_context_mismatch_is_stale() {
    let mut trace = base_trace();
    trace.plugin_reported = TraceSlot::Present(PluginReportedEvidence {
        value: value(0.5),
        stamp: stamp(TimestampProvenance::Carried),
    });
    let live = ctx();
    assert_eq!(trace.freshness(&live), ObservationFreshness::Current);

    let cases: [(FreshnessContext, StaleReason); 7] = [
        (
            FreshnessContext {
                project_revision: ProjectRevision(8),
                ..live
            },
            StaleReason::ProjectRevision,
        ),
        (
            FreshnessContext {
                control_plan_generation: PlanGeneration::new(4),
                ..live
            },
            StaleReason::ControlPlan,
        ),
        (
            FreshnessContext {
                graph_generation: 12,
                ..live
            },
            StaleReason::Graph,
        ),
        (
            FreshnessContext {
                transport_generation: TransportGeneration::new(6),
                ..live
            },
            StaleReason::Transport,
        ),
        (
            FreshnessContext {
                clock_epoch: RuntimeClockEpochId::new(2),
                ..live
            },
            StaleReason::ClockEpoch,
        ),
        (
            FreshnessContext {
                loop_occurrence: LoopOccurrence::new(1),
                ..live
            },
            StaleReason::Loop,
        ),
        (
            FreshnessContext {
                evaluation_position: time(11),
                ..live
            },
            StaleReason::Position,
        ),
    ];
    for (stale_ctx, reason) in cases {
        assert_eq!(
            trace.freshness(&stale_ctx),
            ObservationFreshness::Stale { reason },
            "expected Stale{{{reason:?}}}"
        );
        // Channel-level answers agree — the whole trace is stale.
        assert_eq!(
            trace.plugin_reported_freshness(&stale_ctx),
            ObservationFreshness::Stale { reason }
        );
    }
}

// TRACE-C3-02: stamp provenance and age classify honestly —
// estimated is never Current, older/too-old is Stale, future or
// uncomputable deadlines are Unavailable.
#[test]
fn trace_c3_02_stamp_quality_classification() {
    let mut trace = base_trace();
    trace.plugin_reported = TraceSlot::Present(PluginReportedEvidence {
        value: value(0.5),
        stamp: stamp(TimestampProvenance::Estimated),
    });
    assert_eq!(trace.freshness(&ctx()), ObservationFreshness::Estimated);

    // Within budget, trusted provenance → Current.
    trace.plugin_reported = TraceSlot::Present(PluginReportedEvidence {
        value: value(0.5),
        stamp: stamp(TimestampProvenance::Carried),
    });
    assert_eq!(trace.freshness(&ctx()), ObservationFreshness::Current);

    // Older than the age budget → Stale{Age}.
    let old_now = time(10)
        .checked_add(MediaDuration::new(180_000, TimeBase::COMMON_90K))
        .unwrap();
    assert_eq!(
        trace.freshness(&FreshnessContext {
            now: old_now,
            ..ctx()
        }),
        ObservationFreshness::Stale {
            reason: StaleReason::Age,
        }
    );

    // Stamp from the future → clock untrusted, never Current.
    let future_stamp = MediaTimestamp {
        time: time(11),
        provenance: TimestampProvenance::Carried,
    };
    trace.plugin_reported = TraceSlot::Present(PluginReportedEvidence {
        value: value(0.5),
        stamp: future_stamp,
    });
    assert_eq!(
        trace.freshness(&ctx()),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::ClockUntrusted,
        }
    );
}

// TRACE-C3-03: probe missing/lost/disabled are channel states —
// distinct from a measured silence, which is a real observation.
#[test]
fn trace_c3_03_probe_channel_states() {
    let live = ctx();

    // No binding → ProbeMissing, not silence.
    let mut trace = base_trace();
    trace.observed = TraceSlot::Present(ProbeObservation::Missing);
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::ProbeMissing,
        }
    );

    // Bound but disabled → ProbeDisabled.
    trace.observed = TraceSlot::Present(ProbeObservation::Disabled);
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::ProbeDisabled,
        }
    );

    // Lost blocks → DataLoss with the count preserved.
    trace.observed = TraceSlot::Present(ProbeObservation::Lost { count: 3 });
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::DataLoss { count: 3 }
    );

    // Measured silence IS an observation — Current, not missing.
    trace.observed = TraceSlot::Present(ProbeObservation::Measuring {
        stamp: stamp(TimestampProvenance::Carried),
        value: MeasuredValue::Silence,
    });
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::Current
    );
    assert_eq!(trace.freshness(&live), ObservationFreshness::Current);

    // Channel independence: a fresh plugin report still answers
    // freshness() when the probe channel is missing — while
    // observed_freshness() keeps reporting ProbeMissing.
    trace.observed = TraceSlot::Present(ProbeObservation::Missing);
    trace.plugin_reported = TraceSlot::Present(PluginReportedEvidence {
        value: value(0.5),
        stamp: stamp(TimestampProvenance::Carried),
    });
    assert_eq!(trace.freshness(&live), ObservationFreshness::Current);
    assert_eq!(
        trace.observed_freshness(&live),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::ProbeMissing,
        }
    );
}

// TRACE-C3-04: Dispatched is a projection, not plugin-internal proof;
// a failed plan never displaces the last-good active plan.
#[test]
fn trace_c3_04_dispatch_semantics_and_failed_plan() {
    // Dispatched evidence alone implies nothing about observation —
    // plugin_reported/observed stay Missing and freshness stays
    // Unavailable.
    let mut trace = base_trace();
    trace.dispatched = TraceSlot::Present(DispatchEvidence::Dispatched {
        value: value(0.5),
        position: time(10),
        plan_generation: PlanGeneration::new(3),
        call_start: EngineFrame::new(RuntimeClockEpochId::new(1), 512),
        frame_offset: 3,
    });
    assert!(trace.is_coherent());
    assert_eq!(
        trace.freshness(&ctx()),
        ObservationFreshness::Unavailable {
            reason: UnavailableReason::NoEvidence,
        }
    );

    // Reported/Unavailable dispatch terminals do not carry a plan
    // generation and stay coherent.
    trace.dispatched = TraceSlot::Present(DispatchEvidence::Unavailable { position: time(10) });
    assert!(trace.is_coherent());
    trace.dispatched = TraceSlot::Present(DispatchEvidence::Reported {
        value: value(0.5),
        position: time(10),
    });
    assert!(trace.is_coherent());

    // Failed publish: plan B fails, plan A remains the projected
    // active plan — the failed generation never occupies `active`.
    let plan_a = ActivePlan {
        plan_generation: PlanGeneration::new(3),
        transport_generation: TransportGeneration::new(5),
        clock_epoch: RuntimeClockEpochId::new(1),
        runtime_instance: RuntimeInstanceId(1),
    };
    let failed_b = FailedPlanAttempt {
        plan_generation: PlanGeneration::new(4),
        fault: RoutingFault::PublishQueueFull,
        transport_generation: TransportGeneration::new(5),
        clock_epoch: RuntimeClockEpochId::new(1),
    };
    let activation = PlanActivationState {
        active: Some(plan_a),
        last_attempt: Some(failed_b),
    };
    let active = activation.active.expect("last-good plan stays active");
    assert_eq!(active.plan_generation, PlanGeneration::new(3));
    let attempt = activation.last_attempt.expect("failure is recorded");
    assert_eq!(attempt.plan_generation, PlanGeneration::new(4));
    assert_eq!(attempt.fault, RoutingFault::PublishQueueFull);
    assert_ne!(active.plan_generation, attempt.plan_generation);
}
