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
    AutomationLaneId, AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange,
    ClipExtent, ClipPlacement, ClipPlayback, ControlGraph, ControlOwner, Interpolation, LaneId,
    ModulationAssignmentId, ModulationSourceId, MusicalDuration, MusicalTick, NoteId, ParamMapping,
    ParameterRole, ParameterTargetId, PlaybackMapping, Polarity, Project, SourceFrame, SourceId,
    SourceKind, SourceRange, TargetCapability, TimeAnchor, Transform, ValueKind, lower_event,
};

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

fn bp(num: i64) -> BeatPoint {
    BeatPoint::new(num, 1).unwrap()
}

/// Project with a driver target (macro-fed depth source), a dependent
/// automation-driven target on track scope, and a note-scoped target —
/// all domains always resolvable so the measured path is the success
/// path, including the clip→note scope-resolution arms.
#[expect(clippy::too_many_lines)]
fn fixture() -> (Project, ParameterTargetId, ParameterTargetId) {
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
                owner: ControlOwner::Track(track),
                role: ParameterRole::Generic,
                time_scope: AutomationTimeScope::ProjectMusical,
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
            owner: ControlOwner::Track(track),
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
    // Lane -> clip (beats 0..8) -> note (clip-rel 0..8) -> note-scoped
    // target: exercises the Clip+Note arms of scope resolution.
    let lane_id = LaneId::new(1);
    let clip_id = synvelis_project::ClipId::new(1);
    let note_id = NoteId::new(1);
    let note_target = ParameterTargetId::new(3);
    p.lanes.insert(
        lane_id,
        synvelis_project::Lane {
            id: lane_id,
            track_id: track,
            clip_ids: vec![clip_id],
        },
    );
    p.clips.insert(
        clip_id,
        synvelis_project::Clip {
            id: clip_id,
            lane_id,
            source_id: SourceId::new(1),
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(0)),
                extent: ClipExtent::Musical(MusicalDuration::new(8 * 960).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(1_000_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
            muted: false,
        },
    );
    p.notes.insert(
        note_id,
        synvelis_project::Note {
            id: note_id,
            clip: clip_id,
            key: 60,
            start: bp(0),
            len: bp(8),
        },
    );
    p.control.targets.insert(
        note_target,
        synvelis_project::ParameterTarget {
            id: note_target,
            name: "pn".into(),
            owner: ControlOwner::Note(note_id),
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::NoteLocal,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(TargetCapability {
                param_id: 9,
                flags_bits: ruviform_plugin_core::ParamFlags::AUTOMATABLE.bits(),
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.0,
            }),
            base: 0.4,
        },
    );
    (p, target, note_target)
}

#[test]
fn rt_control_eval_and_lower_alloc_free() {
    let (project, target, note_target) = fixture();
    let plan = ControlGraph::compile(&project).unwrap();
    let mut scratch = plan.scratch();
    let base = TimeBase::COMMON_90K;

    // Equivalence first — the RT path agrees with the rich
    // `evaluate_control` breakdown on the same target/positions
    // (same semantics, not a simplified copy), track and note scope.
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
        let rich_n = synvelis_project::evaluate_control(&project, note_target, pos, None)
            .unwrap()
            .host_normalized;
        let fast_n = plan
            .eval_rt(&project, note_target, pos, None, &mut scratch)
            .unwrap();
        assert!(
            (rich_n - fast_n).abs() < 1e-12,
            "note pos {pos:?}: rich {rich_n} != rt {fast_n}"
        );
    }

    // Pre-reserved event buffer — push must never grow inside the window.
    let mut events = Vec::with_capacity(64);
    // Warm-up: lazy init, first-touch caches — outside the window.
    let warm = plan
        .eval_rt(&project, target, bp(1), None, &mut scratch)
        .unwrap();
    let _ = plan.eval_rt(&project, note_target, bp(1), None, &mut scratch);
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
            // Alternate between the track-scoped and note-scoped target
            // so both scope-resolution arms run inside the window.
            let id = if i % 2 == 0 { target } else { note_target };
            let value = plan.eval_rt(&project, id, pos, None, &mut scratch).unwrap();
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
