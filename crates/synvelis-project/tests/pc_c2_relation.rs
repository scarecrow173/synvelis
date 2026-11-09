//! PC-C2 acceptance tests (REL-C2-01..05): the revision-tagged
//! bidirectional [`RelationIndex`] over `Project::control`.
//!
//! Fixture projects are built by direct `control` map mutation (same
//! approach as `pc_b_rt.rs`) so dangling references, disabled cycles
//! and duplicate bindings — states the op oracle would reject — are
//! reachable. `ProjectRevision` values are arbitrary tags; what matters
//! is index/query agreement.

use std::collections::BTreeSet;
use std::time::Instant;

use synvelis_project::{
    AutomationLane, AutomationLaneId, AutomationTimeScope, BeatPoint, ControlEntityRef,
    ControlGraph, ControlOwner, ModulationAssignment, ModulationAssignmentId, ModulationSource,
    ModulationSourceId, ParamMapping, ParameterRole, ParameterTarget, ParameterTargetId, Polarity,
    Project, ProjectId, ProjectRevision, RelationCoverage, RelationDirection, RelationIndex,
    RelationKind, RelationQuery, RelationState, SourceKind, Transform, ValueKind, evaluate_control,
};

fn bp(num: i64) -> BeatPoint {
    BeatPoint::new(num, 1).unwrap()
}

fn rev(raw: u64) -> ProjectRevision {
    ProjectRevision(raw)
}

fn param(id: u128) -> ParameterTarget {
    ParameterTarget {
        id: ParameterTargetId::new(id),
        owner: ControlOwner::Project,
        role: ParameterRole::Generic,
        time_scope: AutomationTimeScope::ProjectMusical,
        name: format!("p{id}"),
        kind: ValueKind::Continuous,
        mapping: ParamMapping::Linear,
        capability: None,
        base: 0.0,
    }
}

fn lane(id: u128, target: ParameterTargetId) -> AutomationLane {
    AutomationLane {
        id: AutomationLaneId::new(id),
        target,
        regions: Vec::new(),
    }
}

fn source(id: u128, kind: SourceKind) -> ModulationSource {
    ModulationSource {
        id: ModulationSourceId::new(id),
        owner: ControlOwner::Project,
        kind,
    }
}

fn assignment(
    id: u128,
    source: ModulationSourceId,
    target: ParameterTargetId,
    depth_source: Option<ParameterTargetId>,
    enabled: bool,
) -> ModulationAssignment {
    ModulationAssignment {
        id: ModulationAssignmentId::new(id),
        source,
        target,
        depth: 0.5,
        depth_source,
        polarity: Polarity::Bipolar,
        transform: Transform::Identity,
        enabled,
    }
}

fn add_target(p: &mut Project, id: u128) -> ParameterTargetId {
    let t = ParameterTargetId::new(id);
    p.control.targets.insert(t, param(id));
    t
}

fn add_lane(p: &mut Project, id: u128, target: ParameterTargetId) -> AutomationLaneId {
    let l = AutomationLaneId::new(id);
    p.control.lanes.insert(l, lane(id, target));
    l
}

fn add_source(p: &mut Project, id: u128, kind: SourceKind) -> ModulationSourceId {
    let s = ModulationSourceId::new(id);
    p.control.sources.insert(s, source(id, kind));
    s
}

fn add_assignment(
    p: &mut Project,
    id: u128,
    source: ModulationSourceId,
    target: ParameterTargetId,
    depth_source: Option<ParameterTargetId>,
    enabled: bool,
) -> ModulationAssignmentId {
    let a = ModulationAssignmentId::new(id);
    p.control
        .assignments
        .insert(a, assignment(id, source, target, depth_source, enabled));
    a
}

fn query(
    anchor: ControlEntityRef,
    direction: RelationDirection,
    max_depth: u16,
    max_edges: usize,
    include_disabled: bool,
    expected: ProjectRevision,
) -> RelationQuery {
    RelationQuery {
        anchor,
        direction,
        max_depth,
        max_edges,
        include_disabled,
        expected_revision: expected,
    }
}

// ---------------------------------------------------------------------------
// REL-C2-01 — LFO/Macro/StepSeq → multiple targets, bidirectional lookup
// ---------------------------------------------------------------------------

#[test]
#[expect(clippy::too_many_lines)]
fn rel_c2_01_bidirectional_multi_source_multi_target() {
    let mut p = Project::new(ProjectId::new(0), "pc-c2-01");
    let t1 = add_target(&mut p, 1);
    let t2 = add_target(&mut p, 2);
    let t3 = add_target(&mut p, 3);
    let lfo = add_source(
        &mut p,
        1,
        SourceKind::Lfo {
            rate: bp(4),
            shape: synvelis_project::LfoShape::Sine,
            bipolar: true,
        },
    );
    let mac = add_source(&mut p, 2, SourceKind::Macro { value: 0.5 });
    let seq = add_source(
        &mut p,
        3,
        SourceKind::StepSeq {
            values: vec![-1.0, 1.0],
            step: bp(1),
        },
    );
    let a1 = add_assignment(&mut p, 1, lfo, t1, None, true);
    let a2 = add_assignment(&mut p, 2, lfo, t2, None, true);
    let a3 = add_assignment(&mut p, 3, mac, t3, None, true);
    let a4 = add_assignment(&mut p, 4, seq, t1, None, true);
    let lane1 = add_lane(&mut p, 1, t1);

    let index = RelationIndex::build(&p, rev(7));
    assert_eq!(index.revision(), rev(7));

    // Source → all of its assignments → all of their targets.
    let out = index.query(&query(
        ControlEntityRef::ModulationSource(lfo),
        RelationDirection::Outgoing,
        2,
        64,
        false,
        rev(7),
    ));
    assert_eq!(out.coverage, RelationCoverage::Complete);
    assert_eq!(out.project_revision, rev(7));
    assert_eq!(out.relations.len(), 4);
    for expected in [
        (lfo.into(), a1.into()),
        (lfo.into(), a2.into()),
        (a1.into(), t1.into()),
        (a2.into(), t2.into()),
    ] {
        assert!(
            out.relations.iter().any(|r| {
                r.kind == RelationKind::ModulationDrives
                    && r.source == expected.0
                    && r.target == expected.1
                    && r.state == RelationState::Enabled
            }),
            "missing {expected:?} in {:?}",
            out.relations
        );
    }

    // Lane anchor outgoing: AutomationDrives lane→target.
    let lane_out = index.query(&query(
        ControlEntityRef::AutomationLane(lane1),
        RelationDirection::Outgoing,
        2,
        64,
        false,
        rev(7),
    ));
    assert_eq!(lane_out.coverage, RelationCoverage::Complete);
    assert_eq!(lane_out.relations.len(), 1);
    assert_eq!(lane_out.relations[0].kind, RelationKind::AutomationDrives);

    // max_depth 0 anchors without expanding any edge.
    let shallow = index.query(&query(
        ControlEntityRef::Parameter(t1),
        RelationDirection::Both,
        0,
        64,
        false,
        rev(7),
    ));
    assert_eq!(shallow.coverage, RelationCoverage::Complete);
    assert_eq!(shallow.relations.len(), 0);

    // The Macro source reaches its own assignment and target t3.
    let mac_out = index.query(&query(
        ControlEntityRef::ModulationSource(mac),
        RelationDirection::Outgoing,
        2,
        64,
        false,
        rev(7),
    ));
    assert!(
        mac_out.relations.iter().any(|r| {
            r.assignment_id == Some(a3) && r.target == ControlEntityRef::Parameter(t3)
        })
    );

    // Target → every driving lane/source through the assignment hop.
    let incoming = index.query(&query(
        ControlEntityRef::Parameter(t1),
        RelationDirection::Incoming,
        3,
        64,
        false,
        rev(7),
    ));
    assert_eq!(incoming.coverage, RelationCoverage::Complete);
    // L1→T1, A1→T1, A4→T1, then S_lfo→A1 and S_step→A4.
    assert_eq!(incoming.relations.len(), 5);
    assert!(incoming.relations.iter().any(|r| {
        r.kind == RelationKind::AutomationDrives
            && r.source == ControlEntityRef::AutomationLane(lane1)
            && r.target == ControlEntityRef::Parameter(t1)
    }));
    assert!(incoming.relations.iter().any(|r| {
        r.kind == RelationKind::ModulationDrives
            && r.source == ControlEntityRef::ModulationSource(seq)
            && r.assignment_id == Some(a4)
    }));
    // The LFO source is reachable through assignment A1.
    assert!(incoming.relations.iter().any(|r| {
        r.source == ControlEntityRef::ModulationSource(lfo) && r.assignment_id == Some(a1)
    }));

    // Direction matters: Incoming from the LFO source sees nothing
    // (sources have no incoming edges), Outgoing from T1 sees nothing.
    let src_in = index.query(&query(
        ControlEntityRef::ModulationSource(lfo),
        RelationDirection::Incoming,
        3,
        64,
        false,
        rev(7),
    ));
    assert_eq!(src_in.coverage, RelationCoverage::Complete);
    assert_eq!(src_in.relations.len(), 0);
    let tgt_out = index.query(&query(
        ControlEntityRef::Parameter(t1),
        RelationDirection::Outgoing,
        3,
        64,
        false,
        rev(7),
    ));
    assert_eq!(tgt_out.coverage, RelationCoverage::Complete);
    assert_eq!(tgt_out.relations.len(), 0);

    // Both = undirected reachability without duplicates: beyond the 5
    // incoming edges it also reaches s_lfo→A2 through
    // t1←A1←s_lfo→A2 (canonical direction preserved on every edge).
    let both = index.query(&query(
        ControlEntityRef::Parameter(t1),
        RelationDirection::Both,
        3,
        64,
        false,
        rev(7),
    ));
    assert_eq!(both.coverage, RelationCoverage::Complete);
    assert_eq!(both.relations.len(), 6, "{:?}", both.relations);
    assert!(both.relations.iter().any(|r| {
        r.kind == RelationKind::ModulationDrives
            && r.source == ControlEntityRef::ModulationSource(lfo)
            && r.assignment_id == Some(a2)
    }));
    let unique: BTreeSet<_> = both
        .relations
        .iter()
        .map(|r| (r.kind, r.source, r.target, r.assignment_id))
        .collect();
    assert_eq!(unique.len(), both.relations.len(), "duplicate emission");
}

// ---------------------------------------------------------------------------
// REL-C2-02 — duplicate assignments, depth driver, disabled semantics
// ---------------------------------------------------------------------------

#[test]
fn rel_c2_02_duplicate_assignments_depth_driver_disabled() {
    let mut p = Project::new(ProjectId::new(0), "pc-c2-02");
    let t1 = add_target(&mut p, 1);
    let t2 = add_target(&mut p, 2);
    let s1 = add_source(&mut p, 1, SourceKind::Macro { value: 0.25 });
    // Two assignments, same source→target pair, distinct IDs.
    let a1 = add_assignment(&mut p, 1, s1, t1, None, true);
    let a2 = add_assignment(&mut p, 2, s1, t1, None, true);
    // Disabled assignment to t2.
    let a3 = add_assignment(&mut p, 3, s1, t2, None, false);
    // Depth driver: t2's evaluated value scales a4's depth on t1.
    let a4 = add_assignment(&mut p, 4, s1, t1, Some(t2), true);

    let index = RelationIndex::build(&p, rev(3));

    // Duplicate bindings are separate edges keyed by assignment id.
    let out = index.query(&query(
        ControlEntityRef::ModulationSource(s1),
        RelationDirection::Outgoing,
        1,
        64,
        false,
        rev(3),
    ));
    assert_eq!(out.coverage, RelationCoverage::Complete);
    // depth 1: source→assignment edges only (a3's are Disabled → skipped).
    let src_edges: Vec<_> = out
        .relations
        .iter()
        .filter(|r| r.source == ControlEntityRef::ModulationSource(s1))
        .collect();
    assert_eq!(src_edges.len(), 3, "{:?}", out.relations);
    assert!(src_edges.iter().all(|r| r.assignment_id != Some(a3)));
    assert!(src_edges.iter().any(|r| r.assignment_id == Some(a1)));
    assert!(src_edges.iter().any(|r| r.assignment_id == Some(a2)));

    // include_disabled surfaces the disabled edges with Disabled state.
    let out_all = index.query(&query(
        ControlEntityRef::ModulationSource(s1),
        RelationDirection::Outgoing,
        2,
        64,
        true,
        rev(3),
    ));
    assert_eq!(out_all.coverage, RelationCoverage::Complete);
    let disabled: Vec<_> = out_all
        .relations
        .iter()
        .filter(|r| r.assignment_id == Some(a3))
        .collect();
    assert_eq!(disabled.len(), 2, "{:?}", out_all.relations);
    assert!(disabled.iter().all(|r| r.state == RelationState::Disabled));

    // Depth driver edge: Parameter(t2) →Assignment(a4), kind
    // DepthControlledBy, reachable outgoing from t2.
    let drv = index.query(&query(
        ControlEntityRef::Parameter(t2),
        RelationDirection::Outgoing,
        2,
        64,
        false,
        rev(3),
    ));
    assert_eq!(drv.coverage, RelationCoverage::Complete);
    assert!(drv.relations.iter().any(|r| {
        r.kind == RelationKind::DepthControlledBy
            && r.source == ControlEntityRef::Parameter(t2)
            && r.target == ControlEntityRef::Assignment(a4)
            && r.assignment_id == Some(a4)
            && r.state == RelationState::Enabled
    }));
    // …and onward to a4's target t1.
    assert!(drv.relations.iter().any(|r| {
        r.kind == RelationKind::ModulationDrives
            && r.source == ControlEntityRef::Assignment(a4)
            && r.target == ControlEntityRef::Parameter(t1)
    }));

    // Disabled non-contribution: removing the disabled assignment must
    // not change evaluation of its target.
    let before = evaluate_control(&p, t2, bp(0), None).unwrap();
    let mut p2 = p.clone();
    p2.control.assignments.remove(&a3);
    let after = evaluate_control(&p2, t2, bp(0), None).unwrap();
    assert_eq!(before, after, "disabled assignment must not contribute");
}

// ---------------------------------------------------------------------------
// REL-C2-03 — revision delta, deletion, stale/unresolved/unavailable
// ---------------------------------------------------------------------------

#[test]
fn rel_c2_03_revision_delta_and_unresolved() {
    let mut p = Project::new(ProjectId::new(0), "pc-c2-03");
    let t1 = add_target(&mut p, 1);
    let s1 = add_source(&mut p, 1, SourceKind::Macro { value: 0.5 });
    let a1 = add_assignment(&mut p, 1, s1, t1, None, true);

    let index = RelationIndex::build(&p, rev(10));

    // Stale expected revision → Stale coverage, no relations.
    let stale = index.query(&query(
        ControlEntityRef::Parameter(t1),
        RelationDirection::Incoming,
        3,
        64,
        false,
        rev(11),
    ));
    assert_eq!(stale.coverage, RelationCoverage::Stale);
    assert_eq!(stale.relations.len(), 0);
    // The reply still reports the index's own generation.
    assert_eq!(stale.project_revision, rev(10));

    // Unknown anchor → Unavailable, no relations.
    let missing = index.query(&query(
        ControlEntityRef::Parameter(ParameterTargetId::new(4242)),
        RelationDirection::Both,
        3,
        64,
        false,
        rev(10),
    ));
    assert_eq!(missing.coverage, RelationCoverage::Unavailable);
    assert_eq!(missing.relations.len(), 0);

    // A lone target with no relations → Complete + empty.
    let lone = add_target(&mut p, 2);
    let index2 = RelationIndex::build(&p, rev(11));
    let empty = index2.query(&query(
        ControlEntityRef::Parameter(lone),
        RelationDirection::Both,
        3,
        64,
        false,
        rev(11),
    ));
    assert_eq!(empty.coverage, RelationCoverage::Complete);
    assert_eq!(empty.relations.len(), 0);

    // Dangling references → Unresolved, never fabricated links.
    // Direct construction bypasses op validation on purpose.
    let mut bad = Project::new(ProjectId::new(0), "pc-c2-03-bad");
    let bt = add_target(&mut bad, 1);
    let dangling_source = ModulationSourceId::new(999);
    let ba = ModulationAssignmentId::new(1);
    bad.control
        .assignments
        .insert(ba, assignment(1, dangling_source, bt, None, true));
    let bad_index = RelationIndex::build(&bad, rev(1));
    let dangling = bad_index.query(&query(
        ControlEntityRef::Assignment(ba),
        RelationDirection::Incoming,
        2,
        64,
        false,
        rev(1),
    ));
    assert_eq!(dangling.coverage, RelationCoverage::Complete);
    assert_eq!(dangling.relations.len(), 1);
    assert_eq!(dangling.relations[0].state, RelationState::Unresolved);
    assert_eq!(
        dangling.relations[0].source,
        ControlEntityRef::ModulationSource(dangling_source)
    );

    // Deletion is reflected on rebuild: removing the assignment removes
    // its edges; the old index goes Stale against the new revision.
    let mut p3 = p.clone();
    p3.control.assignments.remove(&a1);
    let index3 = RelationIndex::build(&p3, rev(12));
    let after_delete = index3.query(&query(
        ControlEntityRef::Parameter(t1),
        RelationDirection::Incoming,
        3,
        64,
        false,
        rev(12),
    ));
    assert_eq!(after_delete.coverage, RelationCoverage::Complete);
    assert_eq!(after_delete.relations.len(), 0);
    // The previous generation still answers — as Stale.
    let old_again = index.query(&query(
        ControlEntityRef::Parameter(t1),
        RelationDirection::Incoming,
        3,
        64,
        false,
        rev(12),
    ));
    assert_eq!(old_again.coverage, RelationCoverage::Stale);

    // Target disappearance turns dependent edges Unresolved.
    let mut p4 = p.clone();
    p4.control.targets.remove(&t1);
    let index4 = RelationIndex::build(&p4, rev(13));
    let vanished = index4.query(&query(
        ControlEntityRef::Assignment(a1),
        RelationDirection::Outgoing,
        2,
        64,
        false,
        rev(13),
    ));
    assert!(
        vanished
            .relations
            .iter()
            .any(|r| r.state == RelationState::Unresolved)
    );
}

// ---------------------------------------------------------------------------
// REL-C2-04 — bounds, truncation, cycles, malformed safety, budget
// ---------------------------------------------------------------------------

#[test]
#[expect(clippy::too_many_lines)]
fn rel_c2_04_bounds_truncation_and_cycles() {
    let mut p = Project::new(ProjectId::new(0), "pc-c2-04");
    let s1 = add_source(&mut p, 1, SourceKind::Macro { value: 0.5 });
    // Fan-out: one source → 8 assignments → 8 distinct targets.
    let mut targets = Vec::new();
    for i in 0..8u128 {
        let t = add_target(&mut p, 10 + i);
        targets.push(t);
        add_assignment(&mut p, 10 + i, s1, t, None, true);
    }
    let index = RelationIndex::build(&p, rev(1));

    // Depth bound: depth 1 sees only source→assignment edges.
    let d1 = index.query(&query(
        ControlEntityRef::ModulationSource(s1),
        RelationDirection::Outgoing,
        1,
        64,
        false,
        rev(1),
    ));
    assert_eq!(d1.coverage, RelationCoverage::Complete);
    assert_eq!(d1.relations.len(), 8);
    assert!(
        d1.relations
            .iter()
            .all(|r| matches!(r.target, ControlEntityRef::Assignment(_)))
    );

    // Edge budget: deterministic prefix + Truncated.
    let trunc = index.query(&query(
        ControlEntityRef::ModulationSource(s1),
        RelationDirection::Outgoing,
        2,
        5,
        false,
        rev(1),
    ));
    assert_eq!(trunc.coverage, RelationCoverage::Truncated);
    assert_eq!(trunc.relations.len(), 5);
    // Same query → identical prefix (deterministic truncation).
    let trunc2 = index.query(&query(
        ControlEntityRef::ModulationSource(s1),
        RelationDirection::Outgoing,
        2,
        5,
        false,
        rev(1),
    ));
    assert_eq!(trunc, trunc2);

    // Enabled depth cycle: t_a ↔ t_b via two assignments. Compile must
    // keep rejecting it; the index marks cycle edges Unresolved.
    let mut cyc = Project::new(ProjectId::new(0), "pc-c2-04-cyc");
    let ta = add_target(&mut cyc, 1);
    let tb = add_target(&mut cyc, 2);
    let cs = add_source(&mut cyc, 1, SourceKind::Macro { value: 0.5 });
    let ca1 = add_assignment(&mut cyc, 1, cs, ta, Some(tb), true);
    let ca2 = add_assignment(&mut cyc, 2, cs, tb, Some(ta), true);
    assert!(matches!(
        ControlGraph::compile(&cyc),
        Err(synvelis_project::Error::ControlCycle { .. })
    ));
    let cyc_index = RelationIndex::build(&cyc, rev(1));
    let cyc_reply = cyc_index.query(&query(
        ControlEntityRef::Parameter(ta),
        RelationDirection::Both,
        4,
        64,
        false,
        rev(1),
    ));
    assert_eq!(cyc_reply.coverage, RelationCoverage::Complete);
    // Cycle-participating edges are Unresolved; source→assignment edges
    // outside the cycle stay Enabled.
    assert!(
        cyc_reply
            .relations
            .iter()
            .any(|r| r.state == RelationState::Unresolved)
    );
    assert!(cyc_reply.relations.iter().any(|r| {
        r.kind == RelationKind::ModulationDrives
            && r.source == ControlEntityRef::ModulationSource(cs)
            && r.state == RelationState::Enabled
    }));
    // The depth edges into the cycle are the Unresolved ones.
    assert!(cyc_reply.relations.iter().any(|r| {
        r.kind == RelationKind::DepthControlledBy
            && r.state == RelationState::Unresolved
            && (r.assignment_id == Some(ca1) || r.assignment_id == Some(ca2))
    }));

    // Disabled cycle: same shape with one leg disabled — not an active
    // control cycle; compile accepts, edges stay Disabled/Enabled.
    let mut dcyc = Project::new(ProjectId::new(0), "pc-c2-04-dcyc");
    let da = add_target(&mut dcyc, 1);
    let db = add_target(&mut dcyc, 2);
    let ds = add_source(&mut dcyc, 1, SourceKind::Macro { value: 0.5 });
    add_assignment(&mut dcyc, 1, ds, da, Some(db), true);
    add_assignment(&mut dcyc, 2, ds, db, Some(da), false);
    assert!(ControlGraph::compile(&dcyc).is_ok());
    let dcyc_index = RelationIndex::build(&dcyc, rev(1));
    let dcyc_reply = dcyc_index.query(&query(
        ControlEntityRef::Parameter(da),
        RelationDirection::Both,
        4,
        64,
        true,
        rev(1),
    ));
    assert!(
        dcyc_reply
            .relations
            .iter()
            .all(|r| r.state != RelationState::Unresolved)
    );
    assert!(
        dcyc_reply
            .relations
            .iter()
            .any(|r| r.state == RelationState::Disabled)
    );

    // Self depth cycle (depth_source == target) — malformed but safe.
    let mut self_cyc = Project::new(ProjectId::new(0), "pc-c2-04-self");
    let st = add_target(&mut self_cyc, 1);
    let ss = add_source(&mut self_cyc, 1, SourceKind::Macro { value: 0.5 });
    add_assignment(&mut self_cyc, 1, ss, st, Some(st), true);
    assert!(ControlGraph::compile(&self_cyc).is_err());
    let self_index = RelationIndex::build(&self_cyc, rev(1));
    let self_reply = self_index.query(&query(
        ControlEntityRef::Parameter(st),
        RelationDirection::Both,
        4,
        64,
        false,
        rev(1),
    ));
    assert_eq!(self_reply.coverage, RelationCoverage::Complete);
    assert!(
        self_reply
            .relations
            .iter()
            .any(|r| r.state == RelationState::Unresolved)
    );

    // Fixture-scale budget: ~200 edges build+query must stay cheap.
    let mut big = Project::new(ProjectId::new(0), "pc-c2-04-big");
    let mut big_sources = Vec::new();
    for i in 0..20u128 {
        big_sources.push(add_source(&mut big, i, SourceKind::Macro { value: 0.1 }));
    }
    for i in 0..40u128 {
        let t = add_target(&mut big, 100 + i);
        for (j, &s) in big_sources.iter().enumerate().take(5) {
            add_assignment(&mut big, 1000 + i * 8 + j as u128, s, t, None, true);
        }
    }
    let t0 = Instant::now();
    let big_index = RelationIndex::build(&big, rev(1));
    let build_elapsed = t0.elapsed();
    let t1 = Instant::now();
    let big_reply = big_index.query(&query(
        ControlEntityRef::ModulationSource(big_sources[0]),
        RelationDirection::Both,
        3,
        10_000,
        false,
        rev(1),
    ));
    let query_elapsed = t1.elapsed();
    assert_eq!(big_reply.coverage, RelationCoverage::Complete);
    eprintln!(
        "REL-C2-04 fixture: {} edges, build {:?}, query {:?} ({} relations)",
        big_index_edge_count(&big),
        build_elapsed,
        query_elapsed,
        big_reply.relations.len()
    );
    // Sanity bound — fixture scale must build+query far under a second.
    assert!(build_elapsed.as_secs() < 1 && query_elapsed.as_secs() < 1);
}

fn big_index_edge_count(p: &Project) -> usize {
    p.control.lanes.len()
        + p.control
            .assignments
            .values()
            .map(|a| 2 + usize::from(a.depth_source.is_some()))
            .sum::<usize>()
}

// ---------------------------------------------------------------------------
// REL-C2-05 — read-only invariance, same IDs/revision, determinism
// ---------------------------------------------------------------------------

#[test]
fn rel_c2_05_read_only_and_deterministic() {
    let mut p = Project::new(ProjectId::new(0), "pc-c2-05");
    let t1 = add_target(&mut p, 1);
    let t2 = add_target(&mut p, 2);
    let s1 = add_source(&mut p, 1, SourceKind::Macro { value: 0.5 });
    let a1 = add_assignment(&mut p, 1, s1, t1, Some(t2), true);
    let lane1 = add_lane(&mut p, 1, t1);

    let before = p.clone();
    let plan_before = ControlGraph::compile(&p).unwrap();
    let index = RelationIndex::build(&p, rev(42));

    for direction in [
        RelationDirection::Incoming,
        RelationDirection::Outgoing,
        RelationDirection::Both,
    ] {
        for anchor in [
            ControlEntityRef::Parameter(t1),
            ControlEntityRef::Parameter(t2),
            ControlEntityRef::ModulationSource(s1),
            ControlEntityRef::Assignment(a1),
            ControlEntityRef::AutomationLane(lane1),
        ] {
            let _ = index.query(&query(anchor, direction, 4, 128, true, rev(42)));
        }
    }

    // The project — including control model, evaluation inputs and
    // undo-visible state — is bit-identical after build+queries.
    assert_eq!(p, before, "relation index must be read-only");
    let plan_after = ControlGraph::compile(&p).unwrap();
    assert_eq!(plan_before, plan_after, "compile inputs unchanged");

    // Same inputs → identical replies (deterministic ordering).
    let i2 = RelationIndex::build(&p, rev(42));
    let q = query(
        ControlEntityRef::Parameter(t1),
        RelationDirection::Both,
        4,
        128,
        true,
        rev(42),
    );
    assert_eq!(index.query(&q), i2.query(&q));
}
