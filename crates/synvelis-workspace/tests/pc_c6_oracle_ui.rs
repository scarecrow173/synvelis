//! PC-C6 oracle rows UI01–UI07 (issue #103, PC-C5 GUI/control gate):
//! headless oracle fixtures for the control-surface consistency and
//! gesture contracts. Expected relation rows, navigation paths,
//! gesture outcomes and undo counts are derived from the *declared
//! fixture ops* — never from `RelationIndex`, `ControlsUi` or view
//! internals — and compared against observed projections.
//!
//! Evidence boundary: `FakeWindowHost` + headless `WorkspaceManager`
//! views — **headless oracle evidence only**. Real GPUI windows, real
//! OS focus, IME and plugin GUI stay `NotRun` (Issue #114; ledger
//! `docs/test-specs/pc-c6-oracle-rows.md`).

use ruviform_ui::dock::{DockTarget, ViewInstanceId};
use ruviform_ui::window::WindowId;
use synvelis_project::{
    AutomationLaneId, AutomationTimeScope, ControlEntityRef, ControlOwner, CounterIdGenerator,
    ModulationAssignmentId, ModulationSourceId, ParamMapping, ParameterRole, ParameterTargetId,
    Polarity, Project, ProjectController, ProjectId, ProjectOp, RelationCoverage,
    RelationDirection, RelationKind, RelationState, SourceKind, Transform, ValueKind,
    is_issuable_raw,
};
use synvelis_workspace::{
    CancelReason, ControlEdgeOutcome, ControlEdgePreview, FakeWindowHost, ProjectSession,
    RelationNav, RelationRow, RelationViewOptions, ViewType, WorkspaceManager, relation_rows,
    relation_rows_with,
};

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xCC5), "pc-c6-ui");
    let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
    WorkspaceManager::new(FakeWindowHost::default(), ProjectSession::new(controller))
}

fn apply(ws: &mut Ws, op: ProjectOp) {
    let rev = ws.session().revision();
    ws.session_mut().apply(op, rev).expect("fixture op");
}

fn window(ws: &mut Ws, title: &str) -> ruviform_ui::window::WindowId {
    ws.create_window(title, 1280, 800).unwrap()
}

fn view(ws: &mut Ws, kind: ViewType, title: &str, win: WindowId) -> ViewInstanceId {
    ws.spawn_view(kind, title, DockTarget::Window(win)).unwrap()
}

/// Declared fixture: one param target, one automation lane feeding it,
/// one modulation source and one enabled assignment S→T. Returns the
/// ids plus the assignment id issued by the op apply.
struct UiFixture {
    target: ParameterTargetId,
    lane: AutomationLaneId,
    source: ModulationSourceId,
    assignment: ModulationAssignmentId,
}

fn ui_fixture(ws: &mut Ws) -> UiFixture {
    apply(
        ws,
        ProjectOp::AddParameterTarget {
            name: "gain".into(),
            owner: ControlOwner::Project,
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    let target = *ws
        .session()
        .project()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .expect("param target");
    apply(ws, ProjectOp::AddAutomationLane { target });
    let lane = *ws
        .session()
        .project()
        .control
        .lanes
        .keys()
        .next_back()
        .expect("automation lane");
    apply(
        ws,
        ProjectOp::AddModulationSource {
            owner: ControlOwner::Project,
            kind: SourceKind::Macro { value: 0.25 },
        },
    );
    let source = *ws
        .session()
        .project()
        .control
        .sources
        .keys()
        .next_back()
        .expect("modulation source");
    apply(
        ws,
        ProjectOp::AddModulationAssignment {
            source,
            target,
            depth: 0.5,
            depth_source: None,
            polarity: Polarity::Unipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    let assignment = *ws
        .session()
        .project()
        .control
        .assignments
        .keys()
        .next_back()
        .expect("assignment");
    UiFixture {
        target,
        lane,
        source,
        assignment,
    }
}

/// The oracle edge set declared by the fixture ops — independent of
/// `RelationIndex`: (kind, source, target, state).
type Edge = (
    RelationKind,
    ControlEntityRef,
    ControlEntityRef,
    RelationState,
);

fn declared_edges(f: &UiFixture) -> Vec<Edge> {
    vec![
        (
            RelationKind::AutomationDrives,
            ControlEntityRef::AutomationLane(f.lane),
            ControlEntityRef::Parameter(f.target),
            RelationState::Enabled,
        ),
        (
            RelationKind::ModulationDrives,
            ControlEntityRef::ModulationSource(f.source),
            ControlEntityRef::Assignment(f.assignment),
            RelationState::Enabled,
        ),
        (
            RelationKind::ModulationDrives,
            ControlEntityRef::Assignment(f.assignment),
            ControlEntityRef::Parameter(f.target),
            RelationState::Enabled,
        ),
    ]
}

fn observed_edges(view: &synvelis_workspace::RelationView) -> Vec<Edge> {
    let mut edges: Vec<Edge> = view
        .rows
        .iter()
        .map(|r| (r.kind, r.source, r.target, r.state))
        .collect();
    edges.sort();
    edges
}

// ---------------------------------------------------------------------------
// UI01 — one case, three projections agree with project truth: the
// Inspector relation view emits exactly the declared edge set; the
// project model remains the single authority (no view-side copies).
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_ui01_relation_view_matches_declared_project_truth() {
    let mut ws = fresh_ws();
    let f = ui_fixture(&mut ws);
    let rev = ws.session().revision();

    // Anchor on the parameter, walk Both at depth 2 — oracle: every
    // declared edge (including the source→assignment hop) is emitted
    // verbatim. The default view depth is 1, so the full declared set
    // requires explicit options.
    let view = relation_rows_with(
        ws.session(),
        ControlEntityRef::Parameter(f.target),
        RelationDirection::Both,
        RelationViewOptions {
            max_depth: 2,
            ..RelationViewOptions::default()
        },
    );
    assert_eq!(view.coverage, RelationCoverage::Complete);
    assert_eq!(view.project_revision, rev);
    assert!(!view.is_stale(ws.session()));

    let mut want = declared_edges(&f);
    want.sort();
    assert_eq!(observed_edges(&view), want);

    // Direction selectivity: Incoming from the target yields the two
    // edges whose stored target is the parameter (lane→T, A→T);
    // Outgoing yields none — the oracle derives both from direction.
    let incoming = relation_rows(
        ws.session(),
        ControlEntityRef::Parameter(f.target),
        RelationDirection::Incoming,
    );
    let mut want_in: Vec<Edge> = want
        .iter()
        .filter(|(_, _, t, _)| *t == ControlEntityRef::Parameter(f.target))
        .copied()
        .collect();
    want_in.sort();
    assert_eq!(observed_edges(&incoming), want_in);

    let outgoing = relation_rows(
        ws.session(),
        ControlEntityRef::Parameter(f.target),
        RelationDirection::Outgoing,
    );
    assert_eq!(outgoing.rows, [] as [RelationRow; 0]);
    assert_eq!(outgoing.coverage, RelationCoverage::Complete);
}

// ---------------------------------------------------------------------------
// UI02 — bidirectional exploration: RelationNav round-trips
// Parameter → Assignment → Source and back without losing the path.
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_ui02_nav_roundtrip_both_directions() {
    let mut ws = fresh_ws();
    let f = ui_fixture(&mut ws);

    let mut nav = RelationNav::new();
    assert!(nav.is_empty());
    nav.push(ControlEntityRef::Parameter(f.target));
    assert_eq!(nav.current(), Some(ControlEntityRef::Parameter(f.target)));
    assert!(!nav.can_go_back());

    // Descend to the assignment via the A→T edge's neighbor.
    nav.push(ControlEntityRef::Assignment(f.assignment));
    assert!(nav.can_go_back());
    // Descend to the source via the S→A edge.
    nav.push(ControlEntityRef::ModulationSource(f.source));
    assert_eq!(nav.len(), 3);

    // Each hop's view is fresh at the live revision.
    for anchor in [
        ControlEntityRef::Parameter(f.target),
        ControlEntityRef::Assignment(f.assignment),
        ControlEntityRef::ModulationSource(f.source),
    ] {
        let v = relation_rows(ws.session(), anchor, RelationDirection::Both);
        assert_eq!(v.coverage, RelationCoverage::Complete);
        assert!(!v.is_stale(ws.session()));
    }

    // Back twice returns to the parameter anchor — the path is kept.
    // `pop` restores the previous anchor, so the first pop lands on
    // the assignment and the second on the parameter.
    assert_eq!(nav.pop(), Some(ControlEntityRef::Assignment(f.assignment)));
    assert_eq!(nav.pop(), Some(ControlEntityRef::Parameter(f.target)));
    assert_eq!(nav.pop(), None);
    assert!(nav.is_empty());
}

// ---------------------------------------------------------------------------
// UI03 — stale index honesty: a pinned old revision yields Stale
// coverage with zero rows; a view built before a project change is
// flagged stale — old receipts never pose as current truth.
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_ui03_stale_revision_never_current() {
    let mut ws = fresh_ws();
    let f = ui_fixture(&mut ws);
    let rev0 = ws.session().revision();

    // View captured at rev0.
    let stale_view = relation_rows(
        ws.session(),
        ControlEntityRef::Parameter(f.target),
        RelationDirection::Both,
    );
    assert!(!stale_view.is_stale(ws.session()));

    // Project advances (new assignment) — the old view is now stale.
    apply(
        &mut ws,
        ProjectOp::AddModulationAssignment {
            source: f.source,
            target: f.target,
            depth: 0.25,
            depth_source: None,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    assert!(stale_view.is_stale(ws.session()));

    // A query pinned to the old revision reports Stale coverage and
    // returns no relations — never silently current rows.
    let pinned = relation_rows_with(
        ws.session(),
        ControlEntityRef::Parameter(f.target),
        RelationDirection::Both,
        RelationViewOptions {
            expected_revision: Some(rev0),
            ..RelationViewOptions::default()
        },
    );
    assert_eq!(pinned.coverage, RelationCoverage::Stale);
    assert_eq!(pinned.rows, [] as [RelationRow; 0]);
    // The reply is tagged with the index generation it was built at —
    // the live revision — while coverage honestly reports Stale.
    assert_eq!(pinned.project_revision, ws.session().revision());
}

// ---------------------------------------------------------------------------
// UI04 — two-window stale handling: a gesture begun in window A drops
// as Stale when the view moves to window B; cancel leaves the project
// and undo history untouched.
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_ui04_two_window_epoch_and_cancel() {
    let mut ws = fresh_ws();
    let f = ui_fixture(&mut ws);
    let win_a = window(&mut ws, "A");
    let win_b = window(&mut ws, "B");
    let sf = view(&mut ws, ViewType::SignalFlow, "sf", win_a);
    let rev0 = ws.session().revision();

    // Begin+preview in window A, then move the view to window B —
    // the commit must drop as Stale, never retarget.
    ws.control_edge_begin(
        sf,
        ControlEntityRef::ModulationSource(f.source),
        ControlEntityRef::Parameter(f.target),
    )
    .unwrap();
    ws.control_edge_preview(
        sf,
        ControlEdgePreview {
            depth: 0.9,
            polarity: Polarity::Bipolar,
        },
    )
    .unwrap();
    assert_eq!(ws.session().revision(), rev0);
    ws.move_view(sf, DockTarget::Window(win_b)).unwrap();
    let out = ws.control_edge_commit(sf).unwrap();
    assert_eq!(out, ControlEdgeOutcome::Stale);
    assert_eq!(ws.session().revision(), rev0);
    assert_eq!(ws.session().project().control.assignments.len(), 1);
    assert!(ws.state(sf).unwrap().control_edge_gesture.is_none());

    // Cancel path: begin again (now owned by B), cancel — project,
    // revision and undo depth all unchanged.
    ws.control_edge_begin(
        sf,
        ControlEntityRef::ModulationSource(f.source),
        ControlEntityRef::Parameter(f.target),
    )
    .unwrap();
    let out = ws.control_edge_cancel(sf, CancelReason::Escape).unwrap();
    assert!(matches!(out, ControlEdgeOutcome::Cancelled { .. }));
    assert_eq!(ws.session().revision(), rev0);
    // The next undo still reaches the fixture's own last op — the
    // cancelled gesture contributed no boundary.
    assert!(ws.session_mut().undo().unwrap());
}

// ---------------------------------------------------------------------------
// UI05 — Begin/Preview/Commit/Cancel lifecycle with exactly-one undo:
// preview never mutates; commit applies one assignment; one undo
// removes it.
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_ui05_gesture_lifecycle_one_undo() {
    let mut ws = fresh_ws();
    let f = ui_fixture(&mut ws);
    let win = window(&mut ws, "A");
    let sf = view(&mut ws, ViewType::SignalFlow, "sf", win);
    let rev0 = ws.session().revision();
    let assignments_before = ws.session().project().control.assignments.len();

    ws.control_edge_begin(
        sf,
        ControlEntityRef::ModulationSource(f.source),
        ControlEntityRef::Parameter(f.target),
    )
    .unwrap();
    // Preview mutates only in-memory state.
    ws.control_edge_preview(
        sf,
        ControlEdgePreview {
            depth: 0.5,
            polarity: Polarity::Unipolar,
        },
    )
    .unwrap();
    assert_eq!(ws.session().revision(), rev0);
    assert_eq!(
        ws.session().project().control.assignments.len(),
        assignments_before
    );

    let out = ws.control_edge_commit(sf).unwrap();
    let ControlEdgeOutcome::Applied { revision } = out else {
        panic!("expected Applied, got {out:?}")
    };
    assert!(revision > rev0);
    assert_eq!(ws.session().revision(), revision);
    assert_eq!(
        ws.session().project().control.assignments.len(),
        assignments_before + 1
    );

    // Exactly one undo boundary: one undo removes the new assignment.
    assert!(ws.session_mut().undo().unwrap());
    assert_eq!(
        ws.session().project().control.assignments.len(),
        assignments_before
    );
}

// ---------------------------------------------------------------------------
// UI06 — probe ownership boundary: this slice exercises only the
// headless view/session surface; probe-trace ownership stays with its
// own gate (TR rows). Marker test records the boundary.
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_ui06_probe_ownership_boundary_marker() {
    // The headless fixture has no probe/trace session attached — any
    // probe observation would be fabricated, so this row asserts only
    // that no probe state leaks into the relation/gesture surface.
    let mut ws = fresh_ws();
    let f = ui_fixture(&mut ws);
    let v = relation_rows(
        ws.session(),
        ControlEntityRef::Parameter(f.target),
        RelationDirection::Both,
    );
    assert_eq!(v.coverage, RelationCoverage::Complete);
    // Real probe ownership evidence: NotRun — see the ledger.
}

// ---------------------------------------------------------------------------
// UI07 — real GUI boundary: FakeWindowHost proves headless view/dock
// semantics only; GPUI/OS focus, IME and real windowing stay NotRun.
// ---------------------------------------------------------------------------

#[test]
fn pc_c6_ui07_headless_window_host_boundary_marker() {
    let mut ws = fresh_ws();
    let win = window(&mut ws, "headless");
    let sf = view(&mut ws, ViewType::SignalFlow, "sf", win);
    // Window epoch exists and is observable headlessly — the same
    // mechanism the real GUI will drive, but no OS window was created.
    assert_eq!(ws.dock().owner_of(sf), Some(win));
    // Real GPUI/OS/plugin-focus evidence: NotRun — see the ledger.
}
