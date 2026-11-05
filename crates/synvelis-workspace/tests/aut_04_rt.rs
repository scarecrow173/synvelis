//! AUT-04 RT-path tests — the real-time ingress contract (issue #134):
//! `record_event_rt` is the only record-write path callable from an
//! RT callback and must be bounded, wait-free and **allocation-free**
//! — no unbounded alloc/IPC/IO on the audio thread. Verified with
//! `stats_alloc` instrumentation (`pc_b_rt.rs` の手法に倣う); the queue
//! itself is the `ruviform_rt` fixed-capacity SPSC whose own RT
//! guarantees are tested upstream.
//!
//! `cargo test` の並列スレッドとグローバルアロケータ共有のため計測は
//! 単一テストに集約する。

use std::alloc::System;

use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use synvelis_project::{
    AutomationTimeScope, BeatPoint, ControlOwner, CounterIdGenerator, ParamMapping, ParameterRole,
    ParameterTargetId, Project, ProjectController, ProjectId, ProjectOp, TrackId, ValueKind,
};
use synvelis_workspace::{
    AutomationMode, FakeWindowHost, ProjectSession, RECORD_EVENT_QUEUE_CAP, RecordPushError,
    WorkspaceManager, WriteArm,
};

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

type Ws = WorkspaceManager<FakeWindowHost>;

fn bp(num: i64) -> BeatPoint {
    BeatPoint::new(num, 1).unwrap()
}

/// A session with an armed write target on a rolling transport and
/// an open recording transaction — the state an RT callback sees.
fn armed_ws() -> (Ws, ParameterTargetId) {
    let project = Project::new(ProjectId::new(0xA05), "aut-04-rt");
    let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
    let mut ws = WorkspaceManager::new(FakeWindowHost::default(), ProjectSession::new(controller));
    let rev = ws.session().revision();
    ws.session_mut()
        .apply(ProjectOp::AddTrack { name: "t".into() }, rev)
        .unwrap();
    let track: TrackId = *ws.session().project().tracks.keys().last().unwrap();
    let rev = ws.session().revision();
    ws.session_mut()
        .apply(
            ProjectOp::AddParameterTarget {
                name: "p".into(),
                owner: ControlOwner::Track(track),
                role: ParameterRole::Generic,
                time_scope: AutomationTimeScope::ProjectMusical,
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Linear,
                capability: None,
                base: 0.5,
            },
            rev,
        )
        .unwrap();
    let target = *ws
        .session()
        .project()
        .control
        .targets
        .keys()
        .last()
        .unwrap();
    let rev = ws.session().revision();
    ws.session_mut()
        .apply(ProjectOp::AddAutomationLane { target }, rev)
        .unwrap();
    let lane = *ws.session().project().control.lanes.keys().last().unwrap();
    ws.session_mut()
        .control_with(|c, p| c.bind_write_lane(p, target, lane))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Write))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_arm(p, target, WriteArm::Armed))
        .unwrap();
    ws.session_mut().control_mut().set_record_enabled(true);
    ws.session_mut()
        .control_with(synvelis_workspace::ControlSession::transport_play);
    let rev = ws.session().revision();
    ws.session_mut()
        .control_with(|c, p| c.record_start(p, rev))
        .unwrap();
    (ws, target)
}

#[test]
fn record_event_rt_alloc_free_bounded_and_typed() {
    let (mut ws, target) = armed_ws();
    // Warm the one-time allocations the measurement must not see.
    let _ = ws
        .session_mut()
        .control_mut()
        .record_event_rt(target, bp(0), 0.5);
    {
        let reg = Region::new(ALLOC);
        for i in 1..=64 {
            ws.session_mut()
                .control_mut()
                .record_event_rt(target, bp(i), 0.5)
                .unwrap();
        }
        let stats = reg.change();
        assert_eq!(stats.allocations, 0, "allocations: {stats:?}");
        assert_eq!(stats.deallocations, 0, "deallocations: {stats:?}");
    }
    // The invalid-value rejection is also alloc-free — the gate check
    // happens before any health accounting.
    {
        let reg = Region::new(ALLOC);
        let err = ws
            .session_mut()
            .control_mut()
            .record_event_rt(target, bp(1), f64::NAN)
            .unwrap_err();
        assert_eq!(err, RecordPushError::Invalid);
        assert_eq!(reg.change().allocations, 0);
    }
    // The queue stays bounded: pushing past capacity is a typed
    // QueueFull, and the per-target health row is one bounded entry
    // (never an unbounded structure on the error path).
    let mut full = false;
    for i in 0..RECORD_EVENT_QUEUE_CAP + 8 {
        let pos = BeatPoint::new(i64::try_from(i).unwrap(), 1).unwrap();
        if ws
            .session_mut()
            .control_mut()
            .record_event_rt(target, pos, 0.5)
            .is_err()
        {
            full = true;
            break;
        }
    }
    assert!(full, "bounded queue must fill");
    let _ = ws
        .session_mut()
        .control_mut()
        .record_event_rt(target, bp(999_999), 0.5);
    let health = ws.session().control().record_health(target).unwrap();
    assert_eq!(health.queue_fulls, 2);
}
