//! Relation Inspector projection (Synvelis #102 PC-C5 slice 1,
//! UI-C5-01) — bidirectional exploration, roundtrip, and verbatim
//! coverage/state surfacing.
//!
//! Fixture-only mutations go straight to `ProjectSession::apply`;
//! the functions under test are `relation_rows`/`relation_rows_with`/
//! `RelationNav`/`RelationRow::neighbor`.

use synvelis_project::{
    AutomationLaneId, AutomationTimeScope, ControlEntityRef, ControlOwner, CounterIdGenerator,
    ModulationAssignmentId, ModulationSourceId, ParamMapping, ParameterRole, ParameterTargetId,
    Polarity, Project, ProjectController, ProjectId, ProjectOp, ProjectRevision, RelationCoverage,
    RelationDirection, RelationKind, RelationState, SourceKind, Transform, ValueKind,
    is_issuable_raw,
};
use synvelis_workspace::{
    ProjectSession, RelationNav, RelationViewOptions, relation_rows, relation_rows_with,
};

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

fn fresh_session() -> ProjectSession {
    let project = Project::new(ProjectId::new(0xC5), "rel-c5-test");
    ProjectSession::new(ProjectController::new(
        project,
        Box::new(CounterIdGenerator::new(1)),
    ))
}

fn apply(session: &mut ProjectSession, op: ProjectOp) -> ProjectRevision {
    let rev = session.revision();
    session.apply(op, rev).expect("fixture op")
}

fn add_target(session: &mut ProjectSession, name: &str) -> ParameterTargetId {
    apply(
        session,
        ProjectOp::AddParameterTarget {
            name: name.into(),
            owner: ControlOwner::Project,
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    // Newest issuable target — the bootstrap master-strip fader is a
    // well-known (non-issuable) id and never matches the filter.
    *session
        .project()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .expect("param target")
}

fn add_lane(session: &mut ProjectSession, target: ParameterTargetId) -> AutomationLaneId {
    apply(session, ProjectOp::AddAutomationLane { target });
    *session
        .project()
        .control
        .lanes
        .keys()
        .next_back()
        .expect("automation lane")
}

fn add_source(session: &mut ProjectSession, value: f64) -> ModulationSourceId {
    apply(
        session,
        ProjectOp::AddModulationSource {
            owner: ControlOwner::Project,
            kind: SourceKind::Macro { value },
        },
    );
    *session
        .project()
        .control
        .sources
        .keys()
        .next_back()
        .expect("modulation source")
}

fn add_assignment(
    session: &mut ProjectSession,
    source: ModulationSourceId,
    target: ParameterTargetId,
    depth_source: Option<ParameterTargetId>,
    enabled: bool,
) -> ModulationAssignmentId {
    apply(
        session,
        ProjectOp::AddModulationAssignment {
            source,
            target,
            depth: 0.25,
            depth_source,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled,
        },
    );
    *session
        .project()
        .control
        .assignments
        .keys()
        .next_back()
        .expect("modulation assignment")
}

fn param(id: ParameterTargetId) -> ControlEntityRef {
    ControlEntityRef::Parameter(id)
}

fn lane(id: AutomationLaneId) -> ControlEntityRef {
    ControlEntityRef::AutomationLane(id)
}

fn source(id: ModulationSourceId) -> ControlEntityRef {
    ControlEntityRef::ModulationSource(id)
}

fn assign(id: ModulationAssignmentId) -> ControlEntityRef {
    ControlEntityRef::Assignment(id)
}

/// The shared neighborhood (all edges `Enabled` except `a3`'s, which
/// are `Disabled`):
///
/// ```text
///            lane ──AutomationDrives──▶ p1 ◀──ModulationDrives── a1 ◀── s1
///              ▲                        │  ▲                      ▲
///              │                        │  └──ModulationDrives── a3 ◀── s3 (disabled)
///              │                        │ DepthControlledBy        ▲
///              │                        └──────────────▶ a2 ◀─────┘
///              │                          ▲
///              └──── ModulationDrives ────┘──▶ p3   (a2: s2→p3, depth_source=p1)
/// ```
struct Fixture {
    p1: ParameterTargetId,
    p3: ParameterTargetId,
    lane: AutomationLaneId,
    s1: ModulationSourceId,
    a1: ModulationAssignmentId,
    a2: ModulationAssignmentId,
    a3: ModulationAssignmentId,
}

fn fixture() -> (ProjectSession, Fixture) {
    let mut session = fresh_session();
    let p1 = add_target(&mut session, "drive");
    let p3 = add_target(&mut session, "modulated");
    let lane = add_lane(&mut session, p1);
    let s1 = add_source(&mut session, 0.5);
    let s2 = add_source(&mut session, 0.25);
    let s3 = add_source(&mut session, -0.5);
    let a1 = add_assignment(&mut session, s1, p1, None, true);
    // a2 modulates p3 and is depth-driven by p1 → `p1 → a2` edge.
    let a2 = add_assignment(&mut session, s2, p3, Some(p1), true);
    // a3 is disabled → both of its edges are `Disabled`.
    let a3 = add_assignment(&mut session, s3, p1, None, false);
    (
        session,
        Fixture {
            p1,
            p3,
            lane,
            s1,
            a1,
            a2,
            a3,
        },
    )
}

// ---------------------------------------------------------------------------
// UI-C5-01: bidirectional rows + verbatim surfacing
// ---------------------------------------------------------------------------

#[test]
fn parameter_rows_surface_direct_relations_verbatim() {
    let (session, f) = fixture();
    let view = relation_rows(&session, param(f.p1), RelationDirection::Both);
    assert_eq!(view.anchor, param(f.p1));
    assert_eq!(view.direction, RelationDirection::Both);
    assert_eq!(view.project_revision, session.revision());
    assert_eq!(view.coverage, RelationCoverage::Complete);
    assert_eq!(view.anchor_label(), f.p1.to_tagged());

    // `Both` walks outgoing adjacency before incoming per node, and
    // each side keeps ascending build order: the depth edge first,
    // then lane / a1 / a3 (a3's edge is `Disabled`, included by
    // default).
    let shape: Vec<(
        RelationKind,
        ControlEntityRef,
        ControlEntityRef,
        RelationState,
    )> = view
        .rows
        .iter()
        .map(|r| (r.kind, r.source, r.target, r.state))
        .collect();
    assert_eq!(
        shape,
        vec![
            (
                RelationKind::DepthControlledBy,
                param(f.p1),
                assign(f.a2),
                RelationState::Enabled,
            ),
            (
                RelationKind::AutomationDrives,
                lane(f.lane),
                param(f.p1),
                RelationState::Enabled,
            ),
            (
                RelationKind::ModulationDrives,
                assign(f.a1),
                param(f.p1),
                RelationState::Enabled,
            ),
            (
                RelationKind::ModulationDrives,
                assign(f.a3),
                param(f.p1),
                RelationState::Disabled,
            ),
        ]
    );
    // Labels are the canonical tagged ids.
    assert_eq!(view.rows[1].source_label(), f.lane.to_tagged());
    assert_eq!(view.rows[1].target_label(), f.p1.to_tagged());
    // Automation edges carry no assignment; modulation/depth edges do.
    assert_eq!(view.rows[0].assignment, Some(f.a2));
    assert_eq!(view.rows[1].assignment, None);
}

#[test]
fn parameter_reaches_lane_source_and_assignment_by_hops() {
    let (session, f) = fixture();
    let view = relation_rows(&session, param(f.p1), RelationDirection::Both);

    // Parameter → AutomationLane: the lane row's far side is the lane,
    // and the lane's only relation leads straight back to p1.
    let lane_row = view
        .rows
        .iter()
        .find(|r| r.source == lane(f.lane))
        .expect("lane edge");
    assert_eq!(lane_row.kind, RelationKind::AutomationDrives);
    assert_eq!(lane_row.neighbor(param(f.p1)), Some(lane(f.lane)));
    let lane_view = relation_rows(&session, lane(f.lane), RelationDirection::Both);
    assert_eq!(lane_view.rows.len(), 1);
    assert_eq!(lane_view.rows[0].neighbor(lane(f.lane)), Some(param(f.p1)));

    // Parameter → Assignment → ModulationSource.
    let a1_row = view
        .rows
        .iter()
        .find(|r| r.source == assign(f.a1))
        .expect("a1 edge");
    let a1 = a1_row.neighbor(param(f.p1)).expect("a1 neighbor");
    assert_eq!(a1, assign(f.a1));
    let a1_view = relation_rows(&session, a1, RelationDirection::Both);
    let s1_row = a1_view
        .rows
        .iter()
        .find(|r| r.source == source(f.s1))
        .expect("source edge");
    assert_eq!(s1_row.kind, RelationKind::ModulationDrives);
    assert_eq!(s1_row.neighbor(a1), Some(source(f.s1)));
    // The source's only relation is back to its assignment.
    let s1_view = relation_rows(&session, source(f.s1), RelationDirection::Both);
    assert_eq!(s1_view.rows.len(), 1);
    assert_eq!(s1_view.rows[0].neighbor(source(f.s1)), Some(assign(f.a1)));
}

#[test]
fn assignment_and_depth_hops_reach_affected_parameter() {
    let (session, f) = fixture();
    let view = relation_rows(&session, param(f.p1), RelationDirection::Both);

    // p1 depth-drives a2 (`DepthControlledBy`); a2 modulates p3 — the
    // affected parameter one hop further.
    let depth_row = view
        .rows
        .iter()
        .find(|r| r.target == assign(f.a2))
        .expect("depth edge");
    assert_eq!(depth_row.kind, RelationKind::DepthControlledBy);
    assert_eq!(depth_row.neighbor(param(f.p1)), Some(assign(f.a2)));

    let a2_view = relation_rows(&session, assign(f.a2), RelationDirection::Both);
    let drive_row = a2_view
        .rows
        .iter()
        .find(|r| r.source == assign(f.a2) && r.target == param(f.p3))
        .expect("a2→p3 edge");
    assert_eq!(drive_row.kind, RelationKind::ModulationDrives);
    assert_eq!(drive_row.neighbor(assign(f.a2)), Some(param(f.p3)));

    // p3's only relation leads back to a2 — the roundtrip is closed.
    let p3_view = relation_rows(&session, param(f.p3), RelationDirection::Both);
    assert_eq!(p3_view.rows.len(), 1);
    assert_eq!(p3_view.rows[0].neighbor(param(f.p3)), Some(assign(f.a2)));
}

#[test]
fn nav_stack_roundtrips_back_to_origin_anchor() {
    let (session, f) = fixture();
    let mut nav = RelationNav::new();
    assert!(nav.is_empty());
    assert_eq!(nav.current(), None);

    // Descend: p1 → a2 (depth edge) → p3 (affected parameter) → a2 → p1.
    nav.push(param(f.p1));
    let v1 = relation_rows(&session, nav.current().unwrap(), RelationDirection::Both);
    let depth_row = v1.rows.iter().find(|r| r.target == assign(f.a2)).unwrap();
    nav.push(depth_row.neighbor(param(f.p1)).unwrap());
    assert_eq!(nav.current(), Some(assign(f.a2)));
    assert!(nav.can_go_back());

    let v2 = relation_rows(&session, nav.current().unwrap(), RelationDirection::Both);
    let drive_row = v2
        .rows
        .iter()
        .find(|r| r.source == assign(f.a2) && r.target == param(f.p3))
        .unwrap();
    nav.push(drive_row.neighbor(assign(f.a2)).unwrap());
    assert_eq!(nav.current(), Some(param(f.p3)));

    // Return toward the origin — 元画面へ戻る.
    assert_eq!(nav.pop(), Some(assign(f.a2)));
    assert_eq!(nav.pop(), Some(param(f.p1)));
    assert_eq!(nav.current(), Some(param(f.p1)));
    assert!(!nav.can_go_back());

    // Pushing the current anchor is a no-op (no wedge).
    nav.push(param(f.p1));
    assert_eq!(nav.len(), 1);
    nav.clear();
    assert!(nav.is_empty());
}

// ---------------------------------------------------------------------------
// coverage / state honesty
// ---------------------------------------------------------------------------

#[test]
fn stale_expected_revision_surfaces_stale_coverage() {
    let (mut session, f) = fixture();
    let pinned = session.revision();
    // Advance the project — the pinned revision no longer matches the
    // index generation.
    add_target(&mut session, "later");

    let view = relation_rows_with(
        &session,
        param(f.p1),
        RelationDirection::Both,
        RelationViewOptions {
            expected_revision: Some(pinned),
            ..RelationViewOptions::default()
        },
    );
    assert_eq!(view.coverage, RelationCoverage::Stale);
    assert_eq!(view.rows.len(), 0);
    // The reply still reports the live index generation honestly.
    assert_eq!(view.project_revision, session.revision());
    assert_ne!(view.project_revision, pinned);

    // A view captured before the mutation reports itself stale.
    let before = relation_rows(&session, param(f.p1), RelationDirection::Both);
    assert!(!before.is_stale(&session));
    add_target(&mut session, "even-later");
    assert!(before.is_stale(&session));
}

#[test]
fn unknown_anchor_surfaces_unavailable_coverage() {
    let (session, _f) = fixture();
    let ghost = ParameterTargetId::new(0xDEAD_BEEF);
    let view = relation_rows(&session, param(ghost), RelationDirection::Both);
    assert_eq!(view.coverage, RelationCoverage::Unavailable);
    assert_eq!(view.rows.len(), 0);
}

#[test]
fn edge_budget_surfaces_truncated_coverage() {
    let (session, f) = fixture();
    let full = relation_rows(&session, param(f.p1), RelationDirection::Both);
    assert!(full.rows.len() > 2);

    let view = relation_rows_with(
        &session,
        param(f.p1),
        RelationDirection::Both,
        RelationViewOptions {
            max_edges: 2,
            ..RelationViewOptions::default()
        },
    );
    assert_eq!(view.coverage, RelationCoverage::Truncated);
    // The emitted rows are a deterministic prefix of the full reply.
    assert_eq!(view.rows, full.rows[..2]);
}

#[test]
fn include_disabled_gates_disabled_edges() {
    let (session, f) = fixture();

    // Default (`include_disabled: true`): a3's edge is present and
    // marked `Disabled` — surfaced, not dropped.
    let view = relation_rows(&session, param(f.p1), RelationDirection::Both);
    let disabled = view
        .rows
        .iter()
        .find(|r| r.assignment == Some(f.a3))
        .expect("disabled edge row");
    assert_eq!(disabled.state, RelationState::Disabled);

    // `include_disabled: false`: the disabled edges are neither
    // emitted nor traversed.
    let view = relation_rows_with(
        &session,
        param(f.p1),
        RelationDirection::Both,
        RelationViewOptions {
            include_disabled: false,
            ..RelationViewOptions::default()
        },
    );
    assert!(view.rows.iter().all(|r| r.assignment != Some(f.a3)));
    assert!(view.rows.iter().all(|r| r.state != RelationState::Disabled));

    // From the disabled assignment's own anchor the same gate applies:
    // both of its edges are disabled, so nothing is emitted.
    let gated = relation_rows_with(
        &session,
        assign(f.a3),
        RelationDirection::Both,
        RelationViewOptions {
            include_disabled: false,
            ..RelationViewOptions::default()
        },
    );
    assert_eq!(gated.coverage, RelationCoverage::Complete);
    assert_eq!(gated.rows.len(), 0);
    let open = relation_rows(&session, assign(f.a3), RelationDirection::Both);
    assert_eq!(open.rows.len(), 2);
    assert!(open.rows.iter().all(|r| r.state == RelationState::Disabled));
}

#[test]
fn cycle_edges_surface_unresolved_without_nuking_enabled_ones() {
    let mut session = fresh_session();
    // a4: s→p2 depth_source=p3; a5: s→p3 depth_source=p2 — the
    // enabled graph closes p2→a5→p3→a4→p2, an SCC the index marks
    // `Unresolved` (ops/oracle allow the shape; `ControlGraph::
    // compile` is the layer that rejects it at evaluation).
    let p2 = add_target(&mut session, "cyc-a");
    let p3 = add_target(&mut session, "cyc-b");
    let s = add_source(&mut session, 0.5);
    let a4 = add_assignment(&mut session, s, p2, Some(p3), true);
    let a5 = add_assignment(&mut session, s, p3, Some(p2), true);

    let view = relation_rows(&session, param(p2), RelationDirection::Both);
    assert_eq!(view.coverage, RelationCoverage::Complete);
    assert_ne!(view.rows.len(), 0);
    assert!(
        view.rows
            .iter()
            .all(|r| r.state == RelationState::Unresolved)
    );
    assert!(
        view.rows
            .iter()
            .any(|r| r.kind == RelationKind::DepthControlledBy
                && r.source == param(p2)
                && r.target == assign(a5))
    );
    assert!(
        view.rows
            .iter()
            .any(|r| r.kind == RelationKind::ModulationDrives
                && r.source == assign(a4)
                && r.target == param(p2))
    );

    // Source→assignment edges sit outside the SCC and stay `Enabled`.
    let sv = relation_rows(&session, source(s), RelationDirection::Both);
    assert_eq!(sv.rows.len(), 2);
    assert!(sv.rows.iter().all(|r| r.state == RelationState::Enabled));
}

#[test]
fn deeper_hops_mark_non_anchor_edges_as_non_navigable() {
    let (session, f) = fixture();
    let deep = relation_rows_with(
        &session,
        param(f.p1),
        RelationDirection::Both,
        RelationViewOptions {
            max_depth: 2,
            ..RelationViewOptions::default()
        },
    );
    assert_eq!(deep.coverage, RelationCoverage::Complete);
    // s1→a1 does not touch the anchor — display-only, `neighbor` is
    // honestly `None` rather than guessing a direction.
    let s1a1 = deep
        .rows
        .iter()
        .find(|r| r.source == source(f.s1) && r.target == assign(f.a1))
        .expect("depth-2 source edge");
    assert_eq!(s1a1.neighbor(param(f.p1)), None);
    // Anchor-touching rows remain navigable at any depth.
    let direct = deep
        .rows
        .iter()
        .find(|r| r.source == assign(f.a1) && r.target == param(f.p1))
        .expect("direct edge");
    assert_eq!(direct.neighbor(param(f.p1)), Some(assign(f.a1)));
}

// ---------------------------------------------------------------------------
// non-destructive + determinism
// ---------------------------------------------------------------------------

#[test]
fn projection_is_read_only_and_deterministic() {
    let (session, f) = fixture();
    let rev = session.revision();
    let targets = session.project().control.targets.len();
    let lanes = session.project().control.lanes.len();
    let sources = session.project().control.sources.len();
    let assignments = session.project().control.assignments.len();

    let first = relation_rows(&session, param(f.p1), RelationDirection::Both);
    let second = relation_rows(&session, param(f.p1), RelationDirection::Both);
    assert_eq!(first, second, "queries are deterministic");

    assert_eq!(session.revision(), rev, "revision must not move");
    assert_eq!(session.project().control.targets.len(), targets);
    assert_eq!(session.project().control.lanes.len(), lanes);
    assert_eq!(session.project().control.sources.len(), sources);
    assert_eq!(session.project().control.assignments.len(), assignments);
    assert!(!first.is_stale(&session));
}
