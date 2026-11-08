//! AUT-02 (issue #132): Region Replace 境界曲線保存 —
//! `cut_range_points` を含む op apply 経路で、切断範囲
//! `[c0,c1)` の外側の merged series 評価値が保存されることを検証する。
//!
//! 契約: 明示 point が boundary 位置を所有し、sentinel は明示 point
//! 非存在時のみ挿入される(同時刻競合は構造上発生しない)。例外:
//! E-1 caller point at c0、E-2 旧系列未到達、E-3 erase 左 Linear 接近。

use synvelis_project::*;

// ---------------------------------------------------------------------------
// fixture helpers
// ---------------------------------------------------------------------------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "aut-02"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    let expected_revision = c.live_revision();
    c.apply(Command {
        expected_revision,
        op,
    })
    .expect("fixture op")
}

fn try_apply(c: &mut ProjectController, op: ProjectOp) -> Result<ProjectRevision, Error> {
    let expected_revision = c.live_revision();
    c.apply(Command {
        expected_revision,
        op,
    })
}

fn bp(num: i64) -> BeatPoint {
    BeatPoint::new(num, 1).unwrap()
}

fn frac(num: i64, den: u32) -> BeatPoint {
    BeatPoint::new(num, den).unwrap()
}

fn pt(pos: BeatPoint, value: f64) -> AutomationPoint {
    AutomationPoint { pos, value }
}

fn span(s: BeatPoint, e: BeatPoint) -> BeatRange {
    Range::try_new(s, e).unwrap()
}

fn region(
    s: BeatPoint,
    e: BeatPoint,
    interpolation: Interpolation,
    points: Vec<AutomationPoint>,
) -> AutomationRegion {
    AutomationRegion {
        range: span(s, e),
        interpolation,
        points,
    }
}

fn add_target(c: &mut ProjectController, base: f64) -> ParameterTargetId {
    apply(
        c,
        ProjectOp::AddParameterTarget {
            name: "p".into(),
            owner: ControlOwner::Project,
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base,
        },
    );
    // The bootstrap master-fader target is well-known (never issued) —
    // pick the newest *issued* target.
    *c.live()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .unwrap()
}

fn add_lane(c: &mut ProjectController, target: ParameterTargetId) -> AutomationLaneId {
    apply(c, ProjectOp::AddAutomationLane { target });
    *c.live().control.lanes.keys().last().unwrap()
}

fn write(c: &mut ProjectController, lane: AutomationLaneId, region: AutomationRegion) {
    apply(
        c,
        ProjectOp::WriteAutomationRegion {
            lane,
            region,
            policy: ConflictPolicy::Reject,
        },
    );
}

/// Probe grid: every `1/den`-beat offset across `[0, upto]`, plus the
/// positions themselves — dense enough to chase every segment on both
/// sides of a cut.
fn probes(upto: i64, den: u32) -> Vec<BeatPoint> {
    (0..=(upto * i64::from(den)))
        .map(|k| frac(k, den))
        .collect()
}

/// The merged-series value at `pos` — `None` where the series is
/// unreached (falls back to base in evaluation).
fn value_at(p: &Project, target: ParameterTargetId, pos: BeatPoint) -> Option<f64> {
    lane_value(p, target, pos)
}

/// F1/F2 oracle: for every probe outside `cut`, old curve == new curve.
/// `except` marks contract-exception spans (E-1/E-3) that are skipped.
fn assert_outside_preserved(
    old: &Project,
    new: &Project,
    target: ParameterTargetId,
    cut: &BeatRange,
    probes: &[BeatPoint],
    except: &dyn Fn(BeatPoint) -> bool,
) {
    for &pos in probes {
        if cut.contains(pos) || except(pos) {
            continue;
        }
        let a = value_at(old, target, pos);
        let b = value_at(new, target, pos);
        match (a, b) {
            (None, None) => {}
            (Some(a), Some(b)) => assert!((a - b).abs() < 1e-12, "pos {pos}: old {a} != new {b}"),
            _ => panic!("pos {pos}: old {a:?} != new {b:?}"),
        }
    }
}

fn assert_well_formed(p: &Project) {
    assert!(
        detect_conflicts(p).is_empty(),
        "conflicts: {:?}",
        detect_conflicts(p)
    );
    for lane in p.control.lanes.values() {
        for r in &lane.regions {
            r.validate().unwrap_or_else(|e| panic!("region {r:?}: {e}"));
        }
    }
}

fn replace(
    c: &mut ProjectController,
    lane: AutomationLaneId,
    region: AutomationRegion,
) -> ProjectRevision {
    apply(
        c,
        ProjectOp::WriteAutomationRegion {
            lane,
            region,
            policy: ConflictPolicy::Replace,
        },
    )
}

fn standard(c: &mut ProjectController) -> (ParameterTargetId, AutomationLaneId) {
    let target = add_target(c, 0.5);
    let lane = add_lane(c, target);
    (target, lane)
}

// ---------------------------------------------------------------------------
// F1: 範囲外曲線保存 — Step / Linear
// ---------------------------------------------------------------------------

#[test]
fn aut_02_step_mid_range_replace_preserves_outside() {
    let mut c = controller();
    let (target, lane) = standard(&mut c);
    write(
        &mut c,
        lane,
        region(
            bp(0),
            bp(10),
            Interpolation::Step,
            vec![pt(bp(1), 0.3), pt(bp(4), 0.7), pt(bp(8), 0.9)],
        ),
    );
    let old = c.live().clone();
    replace(
        &mut c,
        lane,
        region(bp(3), bp(6), Interpolation::Step, vec![pt(bp(3), 0.5)]),
    );
    let new = c.live();
    let cut = span(bp(3), bp(6));
    let grid = probes(12, 2);
    assert_outside_preserved(&old, new, target, &cut, &grid, &|_| false);
    assert_well_formed(new);
    // Step chase-forward: the sentinel holds the pre-edit value at
    // cut.end until the next kept point.
    assert_eq!(value_at(new, target, bp(6)), Some(0.7));
    assert_eq!(value_at(new, target, bp(7)), Some(0.7));
    assert_eq!(value_at(new, target, bp(8)), Some(0.9));
}

#[test]
fn aut_02_linear_mid_range_replace_preserves_outside() {
    let mut c = controller();
    let (target, lane) = standard(&mut c);
    write(
        &mut c,
        lane,
        region(
            bp(0),
            bp(10),
            Interpolation::Linear,
            vec![pt(bp(1), 0.2), pt(bp(5), 0.8), pt(bp(9), 0.4)],
        ),
    );
    let old = c.live().clone();
    replace(
        &mut c,
        lane,
        region(bp(3), bp(7), Interpolation::Linear, vec![pt(bp(3), 0.5)]),
    );
    let new = c.live();
    let cut = span(bp(3), bp(7));
    let grid = probes(12, 4);
    assert_outside_preserved(&old, new, target, &cut, &grid, &|_| false);
    assert_well_formed(new);
    // Left approach and right resume reconnect exactly.
    assert_eq!(value_at(new, target, bp(10)), Some(0.4));
}

#[test]
fn aut_02_point_at_boundaries_owns_anchor_position() {
    let mut c = controller();
    let (target, lane) = standard(&mut c);
    write(
        &mut c,
        lane,
        region(
            bp(0),
            bp(8),
            Interpolation::Step,
            vec![pt(bp(2), 0.4), pt(bp(6), 0.9)],
        ),
    );
    let old = c.live().clone();
    // Cut on the old points themselves: the kept point at c1 owns the
    // boundary — no sentinel inserted there.
    replace(
        &mut c,
        lane,
        region(bp(2), bp(6), Interpolation::Step, vec![pt(bp(4), 0.5)]),
    );
    let new = c.live();
    let grid = probes(10, 2);
    // E-1: the caller region has no point at c0, so the left anchor
    // (2, 0.4) is folded in — the approach value at the boundary is
    // preserved even though the kept point at c1 owns its boundary.
    assert_outside_preserved(&old, new, target, &span(bp(2), bp(6)), &grid, &|_| false);
    assert_eq!(value_at(new, target, bp(1)), None);
    assert_eq!(value_at(new, target, bp(6)), Some(0.9));
    assert_eq!(value_at(new, target, bp(7)), Some(0.9));
    assert_well_formed(new);
    let pts: Vec<_> = new.control.lanes[&lane]
        .regions
        .iter()
        .flat_map(|r| r.points.iter())
        .collect();
    // No coincident point at c1 — the kept point is the only one.
    assert_eq!(pts.iter().filter(|p| p.pos == bp(6)).count(), 1);
}

#[test]
fn aut_02_replace_exact_region_extent_sentinel_region_created() {
    let mut c = controller();
    let (target, lane) = standard(&mut c);
    write(
        &mut c,
        lane,
        region(
            bp(0),
            bp(8),
            Interpolation::Step,
            vec![pt(bp(1), 0.3), pt(bp(6), 0.9)],
        ),
    );
    let old = c.live().clone();
    // Replace the whole region extent — no right piece, no host: a
    // dedicated sentinel region must carry the resume value.
    replace(
        &mut c,
        lane,
        region(bp(0), bp(8), Interpolation::Step, vec![pt(bp(4), 0.5)]),
    );
    let new = c.live();
    let grid = probes(12, 2);
    assert_outside_preserved(&old, new, target, &span(bp(0), bp(8)), &grid, &|_| false);
    assert_eq!(value_at(new, target, bp(8)), Some(0.9));
    assert_eq!(value_at(new, target, bp(20)), Some(0.9));
    assert_well_formed(new);
}

#[test]
fn aut_02_single_point_and_empty_cut_regions() {
    let mut c = controller();
    let (target, lane) = standard(&mut c);
    write(
        &mut c,
        lane,
        region(bp(0), bp(4), Interpolation::Step, vec![pt(bp(2), 0.6)]),
    );
    let old = c.live().clone();
    replace(
        &mut c,
        lane,
        region(bp(1), bp(3), Interpolation::Step, vec![pt(bp(1), 0.2)]),
    );
    let new = c.live();
    let grid = probes(8, 2);
    assert_outside_preserved(&old, new, target, &span(bp(1), bp(3)), &grid, &|_| false);
    assert_eq!(value_at(new, target, bp(3)), Some(0.6));
    assert_eq!(value_at(new, target, bp(10)), Some(0.6));
    assert_well_formed(new);
}

#[test]
fn aut_02_multi_lane_merged_boundary() {
    let mut c = controller();
    let target = add_target(&mut c, 0.5);
    let lane_a = add_lane(&mut c, target);
    let lane_b = add_lane(&mut c, target);
    write(
        &mut c,
        lane_a,
        region(
            bp(0),
            bp(8),
            Interpolation::Step,
            vec![pt(bp(2), 0.3), pt(bp(6), 0.7)],
        ),
    );
    write(
        &mut c,
        lane_b,
        region(bp(8), bp(12), Interpolation::Linear, vec![pt(bp(10), 0.5)]),
    );
    let old = c.live().clone();
    // Cut spans lane_a's region and the start of lane_b's — every lane
    // of the target loses the overlapping span; the resume anchor is
    // computed on the merged series.
    replace(
        &mut c,
        lane_a,
        region(bp(4), bp(9), Interpolation::Step, vec![pt(bp(4), 0.5)]),
    );
    let new = c.live();
    let grid = probes(14, 2);
    assert_outside_preserved(&old, new, target, &span(bp(4), bp(9)), &grid, &|_| false);
    // q_S was lane_a's Step point at 6 — the anchor holds 0.7 until
    // lane_b's point at 10 (interp mismatch → dedicated sentinel).
    assert_eq!(value_at(new, target, bp(9)), Some(0.7));
    assert_eq!(value_at(new, target, frac(19, 2)), Some(0.7));
    assert_eq!(value_at(new, target, bp(10)), Some(0.5));
    assert_well_formed(new);
}

#[test]
fn aut_02_bidirectional_overlap_two_regions() {
    let mut c = controller();
    let (target, lane) = standard(&mut c);
    write(
        &mut c,
        lane,
        region(
            bp(0),
            bp(6),
            Interpolation::Step,
            vec![pt(bp(1), 0.2), pt(bp(5), 0.6)],
        ),
    );
    write(
        &mut c,
        lane,
        region(
            bp(6),
            bp(12),
            Interpolation::Linear,
            vec![pt(bp(7), 0.4), pt(bp(11), 0.8)],
        ),
    );
    let old = c.live().clone();
    // The cut overlaps both regions on both sides.
    replace(
        &mut c,
        lane,
        region(bp(4), bp(8), Interpolation::Step, vec![pt(bp(4), 0.9)]),
    );
    let new = c.live();
    let grid = probes(14, 4);
    assert_outside_preserved(&old, new, target, &span(bp(4), bp(8)), &grid, &|_| false);
    assert_well_formed(new);
}

#[test]
fn aut_02_e1_caller_point_owns_left_boundary() {
    let mut c = controller();
    let (target, lane) = standard(&mut c);
    write(
        &mut c,
        lane,
        region(
            bp(0),
            bp(10),
            Interpolation::Linear,
            vec![pt(bp(1), 0.0), pt(bp(9), 1.0)],
        ),
    );
    let old = c.live().clone();
    // The caller places its first point exactly at cut.start: it owns
    // the boundary value — no anchor is prepended, and the Linear
    // approach from `pa` redirects toward the caller value (E-1).
    replace(
        &mut c,
        lane,
        region(bp(4), bp(8), Interpolation::Linear, vec![pt(bp(4), 0.5)]),
    );
    let new = c.live();
    let new_pts: Vec<AutomationPoint> = new.control.lanes[&lane]
        .regions
        .iter()
        .flat_map(|r| r.points.iter().copied())
        .collect();
    assert_eq!(
        new_pts.iter().filter(|p| p.pos == bp(4)).count(),
        1,
        "caller point at c0 is the single owner"
    );
    assert_eq!(
        new_pts.iter().find(|p| p.pos == bp(4)).map(|p| p.value),
        Some(0.5)
    );
    let grid = probes(12, 4);
    // Everything outside is preserved except the E-1 approach span
    // (pa, c0) = (1, 4), where the line now aims at the caller value.
    assert_outside_preserved(&old, new, target, &span(bp(4), bp(8)), &grid, &|pos| {
        pos > bp(1) && pos < bp(4)
    });
    // Right side still resumes exactly.
    assert_eq!(value_at(new, target, bp(10)), Some(1.0));
    assert_well_formed(new);
}

// ---------------------------------------------------------------------------
// F1: 不正区間 — Reverse/empty は検証で拒否され live/revision 不変
// ---------------------------------------------------------------------------

#[test]
fn aut_02_invalid_ranges_rejected_atomically() {
    let mut c = controller();
    let (_target, lane) = standard(&mut c);
    write(
        &mut c,
        lane,
        region(bp(0), bp(4), Interpolation::Step, vec![pt(bp(1), 0.5)]),
    );
    let rev = c.live_revision();
    let live = c.live().clone();
    // Reversed ranges cannot be constructed (half-open legality is
    // enforced by `BeatRange` itself).
    assert!(Range::try_new(bp(5), bp(3)).is_err());
    // Zero-width ranges are constructible but rejected by the op
    // validation arm — live project and revision stay untouched.
    assert!(span(bp(3), bp(3)).is_empty());
    assert!(matches!(
        try_apply(
            &mut c,
            ProjectOp::EraseAutomationRange {
                lane,
                range: span(bp(3), bp(3)),
            },
        ),
        Err(Error::Validation(_))
    ));
    // A region that fails `validate()` is rejected at the op boundary
    // — live project and revision stay untouched.
    let bad_region = AutomationRegion {
        range: span(bp(10), bp(14)),
        interpolation: Interpolation::Step,
        points: vec![pt(bp(5), 0.5)], // point outside its own range
    };
    assert!(bad_region.validate().is_err());
    assert!(matches!(
        try_apply(
            &mut c,
            ProjectOp::WriteAutomationRegion {
                lane,
                region: bad_region,
                policy: ConflictPolicy::Replace,
            },
        ),
        Err(Error::Validation(_))
    ));
    assert_eq!(c.live_revision(), rev);
    assert_eq!(c.live().control, live.control);
}

// ---------------------------------------------------------------------------
// F2: Exact Beat fixture — 有理座標が厳密に保持される
// ---------------------------------------------------------------------------

#[test]
fn aut_02_exact_rational_beat_anchors() {
    let mut c = controller();
    let (target, lane) = standard(&mut c);
    write(
        &mut c,
        lane,
        region(
            bp(0),
            bp(2),
            Interpolation::Linear,
            vec![
                pt(frac(1, 3), 0.2),
                pt(frac(4, 5), 0.8),
                pt(frac(9, 10), 0.5),
            ],
        ),
    );
    let old = c.live().clone();
    replace(
        &mut c,
        lane,
        region(
            frac(1, 2),
            frac(2, 3),
            Interpolation::Linear,
            vec![pt(frac(3, 5), 0.55)],
        ),
    );
    let new = c.live();
    // The synthesized anchors sit on the exact rational cut bounds —
    // no tick round-trip, no binary grid.
    let positions: Vec<BeatPoint> = new.control.lanes[&lane]
        .regions
        .iter()
        .flat_map(|r| r.points.iter().map(|p| p.pos))
        .collect();
    assert!(
        positions.contains(&frac(1, 2)),
        "left anchor at 1/2: {positions:?}"
    );
    assert!(
        positions.contains(&frac(2, 3)),
        "right anchor at 2/3: {positions:?}"
    );
    // No coincident duplicates.
    let mut sorted = positions.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), positions.len());
    // Outside the cut the merged curve is preserved on a dense
    // twelfth-beat probe grid.
    let grid = probes(2, 12);
    assert_outside_preserved(
        &old,
        new,
        target,
        &span(frac(1, 2), frac(2, 3)),
        &grid,
        &|_| false,
    );
    assert_well_formed(new);
}

// ---------------------------------------------------------------------------
// F3: 原子性 — 1 Undo・CAS・失敗不変
// ---------------------------------------------------------------------------

#[test]
fn aut_02_one_undo_restores_complete_pre_state() {
    let mut c = controller();
    let (target, lane) = standard(&mut c);
    write(
        &mut c,
        lane,
        region(
            bp(0),
            bp(10),
            Interpolation::Linear,
            vec![pt(bp(1), 0.2), pt(bp(5), 0.8), pt(bp(9), 0.4)],
        ),
    );
    let before = c.live().clone();
    let rev_before = c.live_revision();
    replace(
        &mut c,
        lane,
        region(bp(3), bp(7), Interpolation::Step, vec![pt(bp(3), 0.6)]),
    );
    assert_ne!(c.live().control, before.control);
    // Exactly one undo step reverts the whole command — new region,
    // sentinels and host splits together.
    assert!(c.undo().unwrap());
    assert_eq!(c.live().control, before.control);
    assert_eq!(c.live_revision(), ProjectRevision(rev_before.0 + 2));
    let grid = probes(12, 4);
    for pos in grid {
        assert_eq!(
            value_at(c.live(), target, pos),
            value_at(&before, target, pos),
            "undo restored curve differs at {pos}"
        );
    }
}

#[test]
fn aut_02_reject_and_stale_revision_leave_everything_untouched() {
    let mut c = controller();
    let (_target, lane) = standard(&mut c);
    write(
        &mut c,
        lane,
        region(bp(0), bp(4), Interpolation::Step, vec![pt(bp(1), 0.5)]),
    );
    let rev = c.live_revision();
    let live = c.live().clone();
    // Reject on overlap.
    assert!(matches!(
        try_apply(
            &mut c,
            ProjectOp::WriteAutomationRegion {
                lane,
                region: region(bp(2), bp(6), Interpolation::Step, vec![pt(bp(2), 0.9)]),
                policy: ConflictPolicy::Reject,
            },
        ),
        Err(Error::AutomationConflict { .. })
    ));
    // Stale CAS revision.
    let stale = Command {
        expected_revision: ProjectRevision(rev.0 + 5),
        op: ProjectOp::WriteAutomationRegion {
            lane,
            region: region(bp(6), bp(8), Interpolation::Step, vec![pt(bp(6), 0.1)]),
            policy: ConflictPolicy::Replace,
        },
    };
    assert!(matches!(c.apply(stale), Err(Error::StaleRevision { .. })));
    assert_eq!(c.live_revision(), rev);
    assert_eq!(c.live().control, live.control);
}

// ---------------------------------------------------------------------------
// F5: Erase 境界処理 — 右側保存・residue 無し・merge skip
// ---------------------------------------------------------------------------

#[test]
fn aut_02_erase_preserves_right_side() {
    let mut c = controller();
    let (target, lane) = standard(&mut c);
    write(
        &mut c,
        lane,
        region(
            bp(0),
            bp(8),
            Interpolation::Step,
            vec![pt(bp(1), 0.3), pt(bp(3), 0.7), pt(bp(6), 0.9)],
        ),
    );
    let old = c.live().clone();
    apply(
        &mut c,
        ProjectOp::EraseAutomationRange {
            lane,
            range: span(bp(2), bp(4)),
        },
    );
    let new = c.live();
    // Right side preserved by the boundary anchor at c1.
    assert_eq!(value_at(new, target, bp(4)), Some(0.7));
    assert_eq!(value_at(new, target, bp(5)), Some(0.7));
    assert_eq!(value_at(new, target, bp(6)), Some(0.9));
    // No residue inside the erased range: every persisted point lies
    // outside it and every region's range avoids the erased interior
    // except the c1-touching sentinel boundary itself.
    let erased = span(bp(2), bp(4));
    for lane in new.control.lanes.values() {
        for r in &lane.regions {
            for p in &r.points {
                assert!(!erased.contains(p.pos), "residue point {p:?}");
            }
        }
    }
    let grid = probes(10, 2);
    // E-3: the left Linear approach can redirect — this fixture is
    // Step, which preserves exactly, so no exception range is needed.
    assert_outside_preserved(&old, new, target, &erased, &grid, &|_| false);
    assert_well_formed(new);
}

#[test]
fn aut_02_erase_merge_skip_when_chase_source_survives() {
    let mut c = controller();
    let target = add_target(&mut c, 0.5);
    let lane_a = add_lane(&mut c, target);
    let lane_b = add_lane(&mut c, target);
    write(
        &mut c,
        lane_a,
        region(bp(0), bp(4), Interpolation::Step, vec![pt(bp(1), 0.3)]),
    );
    write(
        &mut c,
        lane_b,
        region(bp(4), bp(8), Interpolation::Step, vec![pt(bp(5), 0.9)]),
    );
    let before_lanes: usize = c
        .live()
        .control
        .lanes
        .values()
        .map(|l| l.regions.len())
        .sum();
    // Erasing lane_a's empty span leaves the merged chase source at
    // c1 (lane_b's point at 5) untouched — no anchor is inserted.
    apply(
        &mut c,
        ProjectOp::EraseAutomationRange {
            lane: lane_a,
            range: span(bp(6), bp(7)),
        },
    );
    let new = c.live();
    let after_lanes: usize = new.control.lanes.values().map(|l| l.regions.len()).sum();
    assert_eq!(before_lanes, after_lanes);
    assert!(
        new.control.lanes[&lane_b]
            .regions
            .iter()
            .all(|r| r.points.len() == 1)
    );
}

// ---------------------------------------------------------------------------
// F6: RT/Offline eval parity on a post-replace series
// ---------------------------------------------------------------------------

#[test]
fn aut_02_rt_eval_matches_offline_after_replace() {
    let mut c = controller();
    let (target, lane) = standard(&mut c);
    write(
        &mut c,
        lane,
        region(
            bp(0),
            bp(10),
            Interpolation::Linear,
            vec![pt(bp(1), 0.2), pt(bp(5), 0.8), pt(bp(9), 0.4)],
        ),
    );
    replace(
        &mut c,
        lane,
        region(bp(3), bp(7), Interpolation::Step, vec![pt(bp(3), 0.6)]),
    );
    let project = c.live();
    let plan = ControlGraph::compile(project).unwrap();
    let mut scratch = plan.scratch();
    for pos in probes(11, 2) {
        let rich = evaluate_control(project, target, pos, None)
            .unwrap()
            .host_normalized;
        let fast = plan
            .eval_rt(project, target, pos, None, &mut scratch)
            .unwrap();
        assert!(
            (rich - fast).abs() < 1e-12,
            "pos {pos}: offline {rich} != rt {fast}"
        );
    }
}
