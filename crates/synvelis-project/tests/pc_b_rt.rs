//! PC-B RT 契約証拠 (PC-B-03): control-rate 評価/dispatch の成功パスが
//! heap 割当ても dealloc も行わないことを `stats_alloc` で計測する
//! (ruviform の RT 契約テスト手法に倣う — 準備・warmup は測定窓外)。
//!
//! 測定対象は `EvalPlan::eval_rt` + `lower_event` + 事前確保済み
//! scratch へのイベント push の一周 — エンジンの control-rate
//! 評価→イベント産出の最小単位。`cargo test` の並列スレッドと
//! グローバルアロケータ共有のため計測は単一テストに集約する。

use std::alloc::System;

use ruviform_media_core::{MediaTime, TimeBase};
use ruviform_plugin_core::ParamWriteMeans;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use synvelis_project::{
    AutomationLaneId, AutomationPoint, AutomationRegion, BeatPoint, BeatRange, ControlGraph,
    ControlScope, Interpolation, ModulationAssignmentId, ModulationSourceId, ParamMapping,
    ParameterTargetId, Polarity, Project, SourceKind, TargetCapability, Transform, ValueKind,
    lower_event,
};

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

fn bp(num: i64) -> BeatPoint {
    BeatPoint::new(num, 1).unwrap()
}

/// Project with a driver target (macro-fed depth source), a dependent
/// automation-driven target, and a note-free track scope — all domains
/// always resolvable so the measured path is the success path.
fn fixture() -> (Project, ParameterTargetId) {
    let mut p = Project::new(synvelis_project::ProjectId::new(0), "pc-b-rt");
    let track = synvelis_project::TrackId::new(1);
    p.tracks.insert(
        track,
        synvelis_project::Track {
            id: track,
            name: "t".into(),
            lane_ids: Vec::new(),
        },
    );
    let driver = ParameterTargetId::new(1);
    let target = ParameterTargetId::new(2);
    for (id, base) in [(driver, 0.5), (target, 0.2)] {
        p.control.targets.insert(
            id,
            synvelis_project::ParameterTarget {
                id,
                name: "p".into(),
                owner: ControlScope::Track(track),
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Linear,
                capability: Some(TargetCapability {
                    param_id: 7,
                    flags_bits: ruviform_plugin_core::ParamFlags::AUTOMATABLE.bits(),
                    min_value: 0.0,
                    max_value: 1.0,
                    default_value: 0.0,
                }),
                base,
            },
        );
    }
    let lane = AutomationLaneId::new(1);
    p.control.lanes.insert(
        lane,
        synvelis_project::AutomationLane {
            id: lane,
            target,
            regions: vec![AutomationRegion {
                range: BeatRange::try_new(bp(0), bp(8)).unwrap(),
                interpolation: Interpolation::Linear,
                points: vec![
                    AutomationPoint {
                        pos: bp(0),
                        value: 0.2,
                    },
                    AutomationPoint {
                        pos: bp(4),
                        value: 0.8,
                    },
                ],
            }],
        },
    );
    let src = ModulationSourceId::new(1);
    p.control.sources.insert(
        src,
        synvelis_project::ModulationSource {
            id: src,
            owner: ControlScope::Track(track),
            kind: SourceKind::Macro { value: 0.25 },
        },
    );
    let asg = ModulationAssignmentId::new(1);
    p.control.assignments.insert(
        asg,
        synvelis_project::ModulationAssignment {
            id: asg,
            source: src,
            target,
            depth: 0.1,
            depth_source: Some(driver),
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    (p, target)
}

#[test]
fn rt_control_eval_and_lower_alloc_free() {
    let (project, target) = fixture();
    let plan = ControlGraph::compile(&project).unwrap();
    let mut scratch = plan.scratch();
    let base = TimeBase::COMMON_90K;

    // Equivalence first — the RT path agrees with the rich
    // `evaluate_control` breakdown on the same target/positions
    // (same semantics, not a simplified copy).
    for num in 0..8 {
        let pos = BeatPoint::new(num * 2, 3).unwrap_or(bp(num));
        let rich = synvelis_project::evaluate_control(&project, target, pos, None)
            .unwrap()
            .host_normalized;
        let fast = plan
            .eval_rt(&project, target, pos, None, &mut scratch)
            .unwrap();
        assert!(
            (rich - fast).abs() < 1e-12,
            "pos {pos:?}: rich {rich} != rt {fast}"
        );
    }

    // Pre-reserved event buffer — push must never grow inside the window.
    let mut events = Vec::with_capacity(64);
    // Warm-up: lazy init, first-touch caches — outside the window.
    let warm = plan
        .eval_rt(&project, target, bp(1), None, &mut scratch)
        .unwrap();
    let t = &project.control.targets[&target];
    let _ = lower_event(
        t,
        warm,
        MediaTime::new(48_000, base),
        ParamWriteMeans::TimedEvent,
    )
    .unwrap();
    {
        let region = Region::new(ALLOC);
        for i in 0..64 {
            let pos = bp(i64::from(i % 8));
            let value = plan
                .eval_rt(&project, target, pos, None, &mut scratch)
                .unwrap();
            let event = lower_event(
                t,
                value,
                MediaTime::new(48_000 + i64::from(i), base),
                ParamWriteMeans::TimedEvent,
            )
            .unwrap();
            events.push(event);
        }
        let stats = region.change();
        assert_eq!(stats.allocations, 0, "allocations: {stats:?}");
        assert_eq!(stats.deallocations, 0, "deallocations: {stats:?}");
    }
    assert_eq!(events.len(), 64);
}
