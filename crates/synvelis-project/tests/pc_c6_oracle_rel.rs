//! PC-C6 oracle acceptance tests (REL01..REL07): independent-oracle
//! verification of [`RelationIndex`] against Issue #103's fixed matrix.
//!
//! Unlike `pc_c2_relation.rs` (which asserts hand-picked expectations),
//! this file keeps a declarative [`FixtureModel`] — the same control
//! declarations that are inserted into `Project::control` — and derives
//! every expected [`RelationReply`] from it with an independent
//! implementation (`OracleIndex`). The oracle never reads
//! `RelationIndex` internals: edge derivation, state classification
//! (Enabled/Disabled/Unresolved incl. cycle detection), and the bounded
//! BFS are all re-implemented here from the documented contract, then
//! compared field-by-field (coverage, revision, ordered relations).
//!
//! Fixture projects are built by direct `control` map mutation (same
//! approach as `pc_c2_relation.rs`) so dangling references, disabled
//! cycles and duplicate bindings stay reachable. `ProjectRevision`
//! values are arbitrary tags; what matters is index/oracle agreement.
//! All results are headless fixture evidence — never claimed as real
//! GUI/plugin/OS success.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use synvelis_project::{
    AutomationLane, AutomationLaneId, AutomationTimeScope, BeatPoint, ControlEntityRef,
    ControlGraph, ControlOwner, LfoShape, ModulationAssignment, ModulationAssignmentId,
    ModulationSource, ModulationSourceId, ParamMapping, ParameterRole, ParameterTarget,
    ParameterTargetId, Polarity, Project, ProjectId, ProjectRevision, RelationCoverage,
    RelationDirection, RelationIndex, RelationKind, RelationQuery, RelationReply, RelationState,
    SourceKind, Transform, ValueKind,
};

// ---------------------------------------------------------------------------
// Fixture declaration model — the oracle's independent source of truth.
// ---------------------------------------------------------------------------

/// One declared assignment: the referenced ids may or may not exist in
/// the entity sets (dangling fixtures declare missing endpoints).
#[derive(Copy, Clone, Debug)]
struct AssignmentDecl {
    source: ModulationSourceId,
    target: ParameterTargetId,
    depth_source: Option<ParameterTargetId>,
    enabled: bool,
}

/// Declarative mirror of what was inserted into `Project::control`.
/// The oracle derives edges/states from this model only — it never
/// inspects the `Project` or the `RelationIndex` under test.
#[derive(Default)]
struct FixtureModel {
    targets: BTreeSet<ParameterTargetId>,
    lanes: BTreeMap<AutomationLaneId, ParameterTargetId>,
    sources: BTreeSet<ModulationSourceId>,
    assignments: BTreeMap<ModulationAssignmentId, AssignmentDecl>,
}

impl FixtureModel {
    /// Cycles require `DepthControlledBy` edges (the only edges whose
    /// target is an assignment while their source is a parameter).
    /// Without depth drivers the edge graph is a DAG by construction:
    /// sources have only outgoing edges, parameters only incoming.
    fn has_depth_driver(&self) -> bool {
        self.assignments
            .values()
            .any(|decl| decl.depth_source.is_some())
    }
}

/// One canonical edge as the oracle derives it (stored direction).
#[derive(Clone, Debug)]
struct OracleEdge {
    kind: RelationKind,
    source: ControlEntityRef,
    target: ControlEntityRef,
    assignment_id: Option<ModulationAssignmentId>,
    /// Final evidence state (cycle marking applied at build).
    state: RelationState,
}

impl OracleEdge {
    fn to_relation(&self) -> synvelis_project::ControlRelation {
        synvelis_project::ControlRelation {
            kind: self.kind,
            source: self.source,
            target: self.target,
            assignment_id: self.assignment_id,
            state: self.state,
        }
    }
}

/// Independent re-implementation of the documented index contract:
/// same edge model, same state rules, same bounded BFS — derived from
/// [`FixtureModel`], not from `RelationIndex`.
struct OracleIndex {
    revision: ProjectRevision,
    entities: BTreeSet<ControlEntityRef>,
    edges: Vec<OracleEdge>,
    outgoing: BTreeMap<ControlEntityRef, Vec<usize>>,
    incoming: BTreeMap<ControlEntityRef, Vec<usize>>,
}

impl OracleIndex {
    fn build(model: &FixtureModel, revision: ProjectRevision) -> Self {
        let mut entities = BTreeSet::new();
        for id in &model.targets {
            entities.insert(ControlEntityRef::Parameter(*id));
        }
        for id in model.lanes.keys() {
            entities.insert(ControlEntityRef::AutomationLane(*id));
        }
        for id in &model.sources {
            entities.insert(ControlEntityRef::ModulationSource(*id));
        }
        for id in model.assignments.keys() {
            entities.insert(ControlEntityRef::Assignment(*id));
        }

        let mut edges: Vec<OracleEdge> = Vec::new();
        let mut push = |kind: RelationKind,
                        source: ControlEntityRef,
                        target: ControlEntityRef,
                        assignment_id: Option<ModulationAssignmentId>,
                        owner_enabled: bool| {
            let resolved = entities.contains(&source) && entities.contains(&target);
            let state = if !resolved {
                RelationState::Unresolved
            } else if !owner_enabled {
                RelationState::Disabled
            } else {
                RelationState::Enabled
            };
            edges.push(OracleEdge {
                kind,
                source,
                target,
                assignment_id,
                state,
            });
        };

        // Canonical build order: lanes first (BTreeMap key order), then
        // assignments each emitting source→assignment, assignment→target,
        // and depth_source→assignment when present.
        for (lane_id, target) in &model.lanes {
            push(
                RelationKind::AutomationDrives,
                ControlEntityRef::AutomationLane(*lane_id),
                ControlEntityRef::Parameter(*target),
                None,
                true,
            );
        }
        for (assignment_id, decl) in &model.assignments {
            let asn = ControlEntityRef::Assignment(*assignment_id);
            push(
                RelationKind::ModulationDrives,
                ControlEntityRef::ModulationSource(decl.source),
                asn,
                Some(*assignment_id),
                decl.enabled,
            );
            push(
                RelationKind::ModulationDrives,
                asn,
                ControlEntityRef::Parameter(decl.target),
                Some(*assignment_id),
                decl.enabled,
            );
            if let Some(depth_source) = decl.depth_source {
                push(
                    RelationKind::DepthControlledBy,
                    ControlEntityRef::Parameter(depth_source),
                    asn,
                    Some(*assignment_id),
                    decl.enabled,
                );
            }
        }

        // Cycle marking: an Enabled edge is Unresolved when its endpoints
        // are mutually reachable through non-Disabled edges. Implemented
        // independently as per-edge reachability (small fixtures only —
        // skipped entirely when the model has no depth drivers, where the
        // edge graph is acyclic by construction).
        if model.has_depth_driver() {
            let cyclic = cyclic_edge_indices(&edges);
            for (index, edge) in edges.iter_mut().enumerate() {
                if cyclic.contains(&index) && edge.state == RelationState::Enabled {
                    edge.state = RelationState::Unresolved;
                }
            }
        }

        let mut outgoing: BTreeMap<ControlEntityRef, Vec<usize>> = BTreeMap::new();
        let mut incoming: BTreeMap<ControlEntityRef, Vec<usize>> = BTreeMap::new();
        for (index, edge) in edges.iter().enumerate() {
            outgoing.entry(edge.source).or_default().push(index);
            incoming.entry(edge.target).or_default().push(index);
        }

        Self {
            revision,
            entities,
            edges,
            outgoing,
            incoming,
        }
    }

    /// Independent bounded BFS — same documented traversal contract:
    /// deterministic adjacency order, FIFO queue, `Both` scans outgoing
    /// then incoming per node, disabled edges skipped unless
    /// `include_disabled`, `max_edges` yields a deterministic Truncated
    /// prefix.
    fn query(&self, query: &RelationQuery) -> RelationReply {
        let reply = |coverage, relations| RelationReply {
            project_revision: self.revision,
            relations,
            coverage,
        };
        if query.expected_revision != self.revision {
            return reply(RelationCoverage::Stale, Vec::new());
        }
        if !self.entities.contains(&query.anchor) {
            return reply(RelationCoverage::Unavailable, Vec::new());
        }
        if query.max_depth == 0 {
            return reply(RelationCoverage::Complete, Vec::new());
        }

        let sides: &[RelationDirection] = match query.direction {
            RelationDirection::Outgoing => &[RelationDirection::Outgoing],
            RelationDirection::Incoming => &[RelationDirection::Incoming],
            RelationDirection::Both => &[RelationDirection::Outgoing, RelationDirection::Incoming],
        };

        let mut relations = Vec::new();
        let mut visited_nodes = BTreeSet::from([query.anchor]);
        let mut visited_edges = BTreeSet::new();
        let mut queue = VecDeque::from([(query.anchor, 0u16)]);
        let mut truncated = false;

        'walk: while let Some((node, depth)) = queue.pop_front() {
            if depth >= query.max_depth {
                continue;
            }
            for &side in sides {
                let adjacent = match side {
                    RelationDirection::Outgoing => self.outgoing.get(&node),
                    RelationDirection::Incoming => self.incoming.get(&node),
                    RelationDirection::Both => unreachable!("sides never contains Both"),
                };
                let Some(indices) = adjacent else { continue };
                for &edge_index in indices {
                    if visited_edges.contains(&edge_index) {
                        continue;
                    }
                    let edge = &self.edges[edge_index];
                    if edge.state == RelationState::Disabled && !query.include_disabled {
                        continue;
                    }
                    if relations.len() >= query.max_edges {
                        truncated = true;
                        break 'walk;
                    }
                    visited_edges.insert(edge_index);
                    relations.push(edge.to_relation());
                    let far = match side {
                        RelationDirection::Outgoing => edge.target,
                        RelationDirection::Incoming => edge.source,
                        RelationDirection::Both => unreachable!(),
                    };
                    let next_depth = depth + 1;
                    if next_depth <= query.max_depth && visited_nodes.insert(far) {
                        queue.push_back((far, next_depth));
                    }
                }
            }
        }

        reply(
            if truncated {
                RelationCoverage::Truncated
            } else {
                RelationCoverage::Complete
            },
            relations,
        )
    }
}

/// Independent cycle detection: edge `u→v` is cyclic iff `v` can reach
/// `u` through non-Disabled edges (equivalent to sharing a non-trivial
/// SCC, since every edge crosses entity variants so no self-loops
/// exist). Per-edge reachability is O(E·(V+E)) — only used on small
/// fixtures that actually contain depth drivers.
fn cyclic_edge_indices(edges: &[OracleEdge]) -> BTreeSet<usize> {
    let mut out: BTreeMap<ControlEntityRef, Vec<ControlEntityRef>> = BTreeMap::new();
    for edge in edges.iter().filter(|e| e.state != RelationState::Disabled) {
        out.entry(edge.source).or_default().push(edge.target);
    }

    let reaches = |from: ControlEntityRef, to: ControlEntityRef| {
        let mut seen = BTreeSet::from([from]);
        let mut stack = vec![from];
        while let Some(node) = stack.pop() {
            if node == to {
                return true;
            }
            if let Some(next) = out.get(&node) {
                for &far in next {
                    if seen.insert(far) {
                        stack.push(far);
                    }
                }
            }
        }
        false
    };

    edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| {
            edge.state != RelationState::Disabled && reaches(edge.target, edge.source)
        })
        .map(|(index, _)| index)
        .collect()
}

// ---------------------------------------------------------------------------
// Fixture builder — keeps `Project::control` and `FixtureModel` in sync.
// ---------------------------------------------------------------------------

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

/// A fixture under test: the real `Project` (drives `RelationIndex`)
/// plus the declarative `FixtureModel` (drives `OracleIndex`).
struct Fixture {
    project: Project,
    model: FixtureModel,
}

impl Fixture {
    fn new(name: &str) -> Self {
        Self {
            project: Project::new(ProjectId::new(0), name),
            model: FixtureModel::default(),
        }
    }

    fn project(&self) -> &Project {
        &self.project
    }

    fn add_target(&mut self, id: u128) -> ParameterTargetId {
        let target = ParameterTargetId::new(id);
        self.project.control.targets.insert(target, param(id));
        self.model.targets.insert(target);
        target
    }

    /// `target` may dangle (not previously added) — the oracle sees the
    /// same declaration and marks the edge Unresolved.
    fn add_lane(&mut self, id: u128, target: ParameterTargetId) -> AutomationLaneId {
        let lane_id = AutomationLaneId::new(id);
        self.project.control.lanes.insert(lane_id, lane(id, target));
        self.model.lanes.insert(lane_id, target);
        lane_id
    }

    /// `source`/`target`/`depth_source` may dangle — direct map mutation
    /// bypasses op validation on purpose (same approach as
    /// `pc_c2_relation.rs`).
    fn add_source(&mut self, id: u128, kind: SourceKind) -> ModulationSourceId {
        let source_id = ModulationSourceId::new(id);
        self.project
            .control
            .sources
            .insert(source_id, source(id, kind));
        self.model.sources.insert(source_id);
        source_id
    }

    fn add_assignment(
        &mut self,
        id: u128,
        source: ModulationSourceId,
        target: ParameterTargetId,
        depth_source: Option<ParameterTargetId>,
        enabled: bool,
    ) -> ModulationAssignmentId {
        let assignment_id = ModulationAssignmentId::new(id);
        let decl = assignment(id, source, target, depth_source, enabled);
        self.project.control.assignments.insert(assignment_id, decl);
        self.model.assignments.insert(
            assignment_id,
            AssignmentDecl {
                source,
                target,
                depth_source,
                enabled,
            },
        );
        assignment_id
    }

    /// Removes the target from both the project and the oracle model —
    /// dependent edges become dangling (Unresolved) and the anchor
    /// itself becomes Unavailable.
    fn remove_target(&mut self, target: ParameterTargetId) {
        self.project.control.targets.remove(&target);
        self.model.targets.remove(&target);
    }

    fn remove_assignment(&mut self, assignment: ModulationAssignmentId) {
        self.project.control.assignments.remove(&assignment);
        self.model.assignments.remove(&assignment);
    }

    fn oracle(&self, revision: ProjectRevision) -> OracleIndex {
        OracleIndex::build(&self.model, revision)
    }
}

/// The core oracle assertion: `RelationIndex::query` must equal the
/// independently derived reply — coverage, generation and the full
/// ordered relation list.
fn assert_query_agrees(index: &RelationIndex, oracle: &OracleIndex, query: &RelationQuery) {
    let expected = oracle.query(query);
    let actual = index.query(query);
    assert_eq!(
        actual, expected,
        "oracle mismatch for query {query:?}\nactual:   {actual:#?}\nexpected: {expected:#?}"
    );
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

fn macro_source(value: f64) -> SourceKind {
    SourceKind::Macro { value }
}

fn lfo_source(id_rate: i64) -> SourceKind {
    SourceKind::Lfo {
        rate: bp(id_rate),
        shape: LfoShape::Sine,
        bipolar: true,
    }
}

fn stepseq_source() -> SourceKind {
    SourceKind::StepSeq {
        values: vec![-1.0, 0.0, 1.0],
        step: bp(1),
    }
}

// ---------------------------------------------------------------------------
// REL01 — Incoming/Outgoing/Both direction agreement on a mixed fixture
// ---------------------------------------------------------------------------

#[test]
fn rel01_directions_agree_with_oracle() {
    let mut f = Fixture::new("pc-c6-rel01");
    let t1 = f.add_target(1);
    let t2 = f.add_target(2);
    let t3 = f.add_target(3);
    let s_lfo = f.add_source(1, lfo_source(4));
    let s_mac = f.add_source(2, macro_source(0.5));
    let s_seq = f.add_source(3, stepseq_source());
    let a1 = f.add_assignment(1, s_lfo, t1, None, true);
    let a2 = f.add_assignment(2, s_lfo, t2, None, true);
    let a3 = f.add_assignment(3, s_mac, t3, None, true);
    let a4 = f.add_assignment(4, s_seq, t1, None, true);
    let lane1 = f.add_lane(1, t1);
    let _ = (a1, a2, a3, a4, lane1);

    let index = RelationIndex::build(f.project(), rev(11));
    let oracle = f.oracle(rev(11));

    let anchors = [
        ControlEntityRef::Parameter(t1),
        ControlEntityRef::Parameter(t2),
        ControlEntityRef::Parameter(t3),
        ControlEntityRef::ModulationSource(s_lfo),
        ControlEntityRef::ModulationSource(s_mac),
        ControlEntityRef::ModulationSource(s_seq),
        ControlEntityRef::Assignment(a1),
        ControlEntityRef::Assignment(a4),
        ControlEntityRef::AutomationLane(lane1),
    ];
    for anchor in anchors {
        for direction in [
            RelationDirection::Incoming,
            RelationDirection::Outgoing,
            RelationDirection::Both,
        ] {
            for max_depth in [0u16, 1, 2, 3, 8] {
                let q = query(anchor, direction, max_depth, 256, false, rev(11));
                assert_query_agrees(&index, &oracle, &q);
            }
        }
    }

    // Spot-check oracle-derived facts (not implementation re-assertion):
    // t1 Incoming reaches lane + two assignments' targets + both sources.
    let t1_in = oracle.query(&query(
        ControlEntityRef::Parameter(t1),
        RelationDirection::Incoming,
        4,
        256,
        false,
        rev(11),
    ));
    assert_eq!(t1_in.coverage, RelationCoverage::Complete);
    assert!(t1_in.relations.iter().any(|r| {
        r.kind == RelationKind::AutomationDrives
            && r.source == ControlEntityRef::AutomationLane(lane1)
    }));
    assert!(t1_in.relations.iter().any(|r| {
        r.source == ControlEntityRef::ModulationSource(s_lfo) && r.assignment_id == Some(a1)
    }));
    assert!(t1_in.relations.iter().any(|r| {
        r.source == ControlEntityRef::ModulationSource(s_seq) && r.assignment_id == Some(a4)
    }));
}

// ---------------------------------------------------------------------------
// REL02 — LFO/Macro/StepSeq sources and DepthControlledBy driver edges
// ---------------------------------------------------------------------------

#[test]
fn rel02_source_kinds_and_depth_driver() {
    let mut f = Fixture::new("pc-c6-rel02");
    let driver = f.add_target(1); // depth driver for a4
    let t1 = f.add_target(2);
    let t2 = f.add_target(3);
    let s_lfo = f.add_source(1, lfo_source(2));
    let s_mac = f.add_source(2, macro_source(-0.25));
    let s_seq = f.add_source(3, stepseq_source());
    // Each source kind drives a distinct target; a4 is depth-driven by `driver`.
    let a1 = f.add_assignment(1, s_lfo, t1, None, true);
    let a2 = f.add_assignment(2, s_mac, t2, None, true);
    let a3 = f.add_assignment(3, s_seq, t1, Some(driver), true);
    let _ = (a1, a2, a3);

    let index = RelationIndex::build(f.project(), rev(5));
    let oracle = f.oracle(rev(5));

    for anchor in [
        ControlEntityRef::ModulationSource(s_lfo),
        ControlEntityRef::ModulationSource(s_mac),
        ControlEntityRef::ModulationSource(s_seq),
        ControlEntityRef::Parameter(driver),
        ControlEntityRef::Parameter(t1),
        ControlEntityRef::Parameter(t2),
        ControlEntityRef::Assignment(a3),
    ] {
        for direction in [
            RelationDirection::Incoming,
            RelationDirection::Outgoing,
            RelationDirection::Both,
        ] {
            let q = query(anchor, direction, 4, 128, false, rev(5));
            assert_query_agrees(&index, &oracle, &q);
        }
    }

    // Oracle-derived facts: the depth edge is Parameter(driver) →
    // Assignment(a3) with kind DepthControlledBy, reachable outgoing
    // from the driver and onward to a3's target t1.
    let drv = oracle.query(&query(
        ControlEntityRef::Parameter(driver),
        RelationDirection::Outgoing,
        3,
        128,
        false,
        rev(5),
    ));
    assert!(drv.relations.iter().any(|r| {
        r.kind == RelationKind::DepthControlledBy
            && r.source == ControlEntityRef::Parameter(driver)
            && r.target == ControlEntityRef::Assignment(a3)
            && r.assignment_id == Some(a3)
            && r.state == RelationState::Enabled
    }));
    assert!(drv.relations.iter().any(|r| {
        r.kind == RelationKind::ModulationDrives
            && r.source == ControlEntityRef::Assignment(a3)
            && r.target == ControlEntityRef::Parameter(t1)
    }));
    // No cycle: driver→a3→t1 does not return to driver.
    assert!(
        drv.relations
            .iter()
            .all(|r| r.state != RelationState::Unresolved)
    );
}

// ---------------------------------------------------------------------------
// REL03 — duplicate assignments on one source→target pair + disabled semantics
// ---------------------------------------------------------------------------

#[test]
fn rel03_duplicate_assignments_and_disabled() {
    let mut f = Fixture::new("pc-c6-rel03");
    let t1 = f.add_target(1);
    let t2 = f.add_target(2);
    let s1 = f.add_source(1, macro_source(0.25));
    // Two enabled assignments on the same (source, target) pair.
    let a1 = f.add_assignment(1, s1, t1, None, true);
    let a2 = f.add_assignment(2, s1, t1, None, true);
    // Disabled assignment to t2.
    let a3 = f.add_assignment(3, s1, t2, None, false);
    // Depth driver on t1 via t2 (t2→a4 edge; a3 disabled so no cycle).
    let a4 = f.add_assignment(4, s1, t1, Some(t2), true);

    let index = RelationIndex::build(f.project(), rev(3));
    let oracle = f.oracle(rev(3));

    for anchor in [
        ControlEntityRef::ModulationSource(s1),
        ControlEntityRef::Parameter(t1),
        ControlEntityRef::Parameter(t2),
        ControlEntityRef::Assignment(a1),
        ControlEntityRef::Assignment(a2),
        ControlEntityRef::Assignment(a3),
        ControlEntityRef::Assignment(a4),
    ] {
        for include_disabled in [false, true] {
            for max_depth in [1u16, 2, 4] {
                let q = query(
                    anchor,
                    RelationDirection::Both,
                    max_depth,
                    256,
                    include_disabled,
                    rev(3),
                );
                assert_query_agrees(&index, &oracle, &q);
            }
        }
    }

    // Oracle-derived facts: duplicate bindings are distinct edges keyed
    // by assignment id; disabled edges appear only with
    // include_disabled and carry Disabled state.
    let out_all = oracle.query(&query(
        ControlEntityRef::ModulationSource(s1),
        RelationDirection::Outgoing,
        2,
        256,
        true,
        rev(3),
    ));
    let a3_edges: Vec<_> = out_all
        .relations
        .iter()
        .filter(|r| r.assignment_id == Some(a3))
        .collect();
    assert_eq!(a3_edges.len(), 2, "{:?}", out_all.relations);
    assert!(a3_edges.iter().all(|r| r.state == RelationState::Disabled));
    let out_default = oracle.query(&query(
        ControlEntityRef::ModulationSource(s1),
        RelationDirection::Outgoing,
        2,
        256,
        false,
        rev(3),
    ));
    assert!(
        out_default
            .relations
            .iter()
            .all(|r| r.assignment_id != Some(a3))
    );
    // Duplicates stay distinguishable by assignment_id.
    let dup_ids: BTreeSet<_> = out_default
        .relations
        .iter()
        .filter(|r| r.target == ControlEntityRef::Parameter(t1))
        .filter_map(|r| r.assignment_id)
        .collect();
    assert!(dup_ids.contains(&a1) && dup_ids.contains(&a2));
}

// ---------------------------------------------------------------------------
// REL04 — cycles, disabled cycles, dangling refs, Unsupported vocabulary
// ---------------------------------------------------------------------------

#[test]
#[expect(clippy::too_many_lines)]
fn rel04_cycles_dangling_and_unsupported() {
    // Enabled depth cycle: t_a ↔ t_b via two assignments. Compile keeps
    // rejecting it; the oracle marks cycle-participating edges
    // Unresolved while non-cycle edges stay Enabled.
    let mut f = Fixture::new("pc-c6-rel04-cycle");
    let ta = f.add_target(1);
    let tb = f.add_target(2);
    let cs = f.add_source(1, macro_source(0.5));
    let ca1 = f.add_assignment(1, cs, ta, Some(tb), true);
    let ca2 = f.add_assignment(2, cs, tb, Some(ta), true);
    assert!(matches!(
        ControlGraph::compile(f.project()),
        Err(synvelis_project::Error::ControlCycle { .. })
    ));

    let index = RelationIndex::build(f.project(), rev(1));
    let oracle = f.oracle(rev(1));
    for anchor in [
        ControlEntityRef::Parameter(ta),
        ControlEntityRef::Parameter(tb),
        ControlEntityRef::ModulationSource(cs),
        ControlEntityRef::Assignment(ca1),
        ControlEntityRef::Assignment(ca2),
    ] {
        for direction in [
            RelationDirection::Incoming,
            RelationDirection::Outgoing,
            RelationDirection::Both,
        ] {
            let q = query(anchor, direction, 5, 128, true, rev(1));
            assert_query_agrees(&index, &oracle, &q);
        }
    }

    let cyc = oracle.query(&query(
        ControlEntityRef::Parameter(ta),
        RelationDirection::Both,
        5,
        128,
        true,
        rev(1),
    ));
    // Cycle-participating edges are Unresolved; source→assignment legs
    // outside the cycle stay Enabled.
    assert!(
        cyc.relations
            .iter()
            .any(|r| r.state == RelationState::Unresolved)
    );
    assert!(cyc.relations.iter().any(|r| {
        r.kind == RelationKind::ModulationDrives
            && r.source == ControlEntityRef::ModulationSource(cs)
            && r.state == RelationState::Enabled
    }));
    assert!(cyc.relations.iter().any(|r| {
        r.kind == RelationKind::DepthControlledBy && r.state == RelationState::Unresolved
    }));

    // Disabled cycle: same shape with one leg disabled — not an active
    // cycle; compile accepts and edges stay Disabled/Enabled.
    let mut d = Fixture::new("pc-c6-rel04-dcyc");
    let da = d.add_target(1);
    let db = d.add_target(2);
    let ds = d.add_source(1, macro_source(0.5));
    d.add_assignment(1, ds, da, Some(db), true);
    d.add_assignment(2, ds, db, Some(da), false);
    assert!(ControlGraph::compile(d.project()).is_ok());
    let d_index = RelationIndex::build(d.project(), rev(1));
    let d_oracle = d.oracle(rev(1));
    for anchor in [
        ControlEntityRef::Parameter(da),
        ControlEntityRef::Parameter(db),
        ControlEntityRef::ModulationSource(ds),
    ] {
        for include_disabled in [false, true] {
            let q = query(
                anchor,
                RelationDirection::Both,
                5,
                128,
                include_disabled,
                rev(1),
            );
            assert_query_agrees(&d_index, &d_oracle, &q);
        }
    }
    let d_reply = d_oracle.query(&query(
        ControlEntityRef::Parameter(da),
        RelationDirection::Both,
        5,
        128,
        true,
        rev(1),
    ));
    assert!(
        d_reply
            .relations
            .iter()
            .all(|r| r.state != RelationState::Unresolved)
    );
    assert!(
        d_reply
            .relations
            .iter()
            .any(|r| r.state == RelationState::Disabled)
    );

    // Dangling references: direct construction bypasses op validation.
    // A disabled assignment with a dangling source is Unresolved (the
    // dangling check precedes the disabled classification).
    let mut bad = Fixture::new("pc-c6-rel04-dangling");
    let bt = bad.add_target(1);
    let dangling_source = ModulationSourceId::new(999);
    let ba = bad.add_assignment(1, dangling_source, bt, None, false);
    // Lane pointing at a target that was never added.
    let ghost_target = ParameterTargetId::new(777);
    bad.add_lane(1, ghost_target);

    let bad_index = RelationIndex::build(bad.project(), rev(1));
    let bad_oracle = bad.oracle(rev(1));
    for anchor in [
        ControlEntityRef::Assignment(ba),
        ControlEntityRef::AutomationLane(AutomationLaneId::new(1)),
        ControlEntityRef::ModulationSource(dangling_source),
        ControlEntityRef::Parameter(ghost_target),
    ] {
        for direction in [
            RelationDirection::Incoming,
            RelationDirection::Outgoing,
            RelationDirection::Both,
        ] {
            let q = query(anchor, direction, 3, 64, true, rev(1));
            assert_query_agrees(&bad_index, &bad_oracle, &q);
        }
    }
    let dangling = bad_oracle.query(&query(
        ControlEntityRef::Assignment(ba),
        RelationDirection::Incoming,
        2,
        64,
        true,
        rev(1),
    ));
    assert_eq!(dangling.relations.len(), 1);
    assert_eq!(dangling.relations[0].state, RelationState::Unresolved);
    // The ghost lane's edge is Unresolved and the ghost target anchor
    // is Unavailable (it is not an entity).
    let ghost = bad_oracle.query(&query(
        ControlEntityRef::Parameter(ghost_target),
        RelationDirection::Both,
        3,
        64,
        true,
        rev(1),
    ));
    assert_eq!(ghost.coverage, RelationCoverage::Unavailable);

    // Unsupported is a reserved vocabulary: no fixture can produce it,
    // and no query result may contain it.
    for reply in [
        cyc,
        d_reply,
        dangling,
        ghost,
        bad_oracle.query(&query(
            ControlEntityRef::Parameter(bt),
            RelationDirection::Both,
            4,
            64,
            true,
            rev(1),
        )),
    ] {
        assert!(
            reply
                .relations
                .iter()
                .all(|r| r.state != RelationState::Unsupported),
            "Unsupported must never be emitted: {reply:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// REL05 — bounded traversal: depth/edge budgets, deterministic truncation
// ---------------------------------------------------------------------------

#[test]
fn rel05_bounds_and_truncation() {
    let mut f = Fixture::new("pc-c6-rel05");
    let s1 = f.add_source(1, macro_source(0.5));
    // Fan-out: one source → 8 assignments → 8 distinct targets.
    for i in 0..8u128 {
        let t = f.add_target(10 + i);
        f.add_assignment(10 + i, s1, t, None, true);
    }

    let index = RelationIndex::build(f.project(), rev(1));
    let oracle = f.oracle(rev(1));

    for max_depth in [0u16, 1, 2, 3] {
        for max_edges in [0usize, 1, 3, 5, 7, 64] {
            let q = query(
                ControlEntityRef::ModulationSource(s1),
                RelationDirection::Outgoing,
                max_depth,
                max_edges,
                false,
                rev(1),
            );
            assert_query_agrees(&index, &oracle, &q);
        }
    }
    // Incoming direction with the same budgets.
    for max_edges in [0usize, 4, 64] {
        let q = query(
            ControlEntityRef::ModulationSource(s1),
            RelationDirection::Incoming,
            2,
            max_edges,
            false,
            rev(1),
        );
        assert_query_agrees(&index, &oracle, &q);
    }

    // Oracle-derived facts: depth 1 sees only source→assignment edges;
    // max_edges=5 truncates deterministically; repeating the query gives
    // an identical reply.
    let d1 = oracle.query(&query(
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

    let trunc = oracle.query(&query(
        ControlEntityRef::ModulationSource(s1),
        RelationDirection::Outgoing,
        2,
        5,
        false,
        rev(1),
    ));
    assert_eq!(trunc.coverage, RelationCoverage::Truncated);
    assert_eq!(trunc.relations.len(), 5);
    let trunc_again = oracle.query(&query(
        ControlEntityRef::ModulationSource(s1),
        RelationDirection::Outgoing,
        2,
        5,
        false,
        rev(1),
    ));
    assert_eq!(trunc, trunc_again, "truncation must be deterministic");

    // max_edges = 0 with reachable edges → Truncated + empty prefix.
    let zero = oracle.query(&query(
        ControlEntityRef::ModulationSource(s1),
        RelationDirection::Outgoing,
        2,
        0,
        false,
        rev(1),
    ));
    assert_eq!(zero.coverage, RelationCoverage::Truncated);
    assert_eq!(zero.relations.len(), 0);
    // …but an anchor with no reachable edges stays Complete even at 0.
    let lone = f.add_target(999);
    let index2 = RelationIndex::build(f.project(), rev(2));
    let oracle2 = f.oracle(rev(2));
    let q0 = query(
        ControlEntityRef::Parameter(lone),
        RelationDirection::Both,
        3,
        0,
        false,
        rev(2),
    );
    assert_query_agrees(&index2, &oracle2, &q0);
    assert_eq!(oracle2.query(&q0).coverage, RelationCoverage::Complete);
}

// ---------------------------------------------------------------------------
// REL06 — stale revision, unavailable anchor, revision delta & deletion
// ---------------------------------------------------------------------------

#[test]
#[expect(clippy::too_many_lines)]
fn rel06_stale_unavailable_and_revision_delta() {
    let mut f = Fixture::new("pc-c6-rel06");
    let t1 = f.add_target(1);
    let t2 = f.add_target(2);
    let s1 = f.add_source(1, macro_source(0.5));
    let a1 = f.add_assignment(1, s1, t1, Some(t2), true);

    let index = RelationIndex::build(f.project(), rev(10));
    let oracle = f.oracle(rev(10));

    // Stale expected revision → Stale coverage, no relations, and the
    // reply still reports the index's own generation.
    let stale_q = query(
        ControlEntityRef::Parameter(t1),
        RelationDirection::Incoming,
        3,
        64,
        false,
        rev(11),
    );
    assert_query_agrees(&index, &oracle, &stale_q);
    assert_eq!(index.query(&stale_q).coverage, RelationCoverage::Stale);
    assert_eq!(index.query(&stale_q).project_revision, rev(10));

    // Unknown anchor → Unavailable; known anchor without relations →
    // Complete + empty. Both agree with the oracle.
    let ghost = ParameterTargetId::new(4242);
    for anchor in [
        ControlEntityRef::Parameter(ghost),
        ControlEntityRef::ModulationSource(ModulationSourceId::new(77)),
        ControlEntityRef::Assignment(ModulationAssignmentId::new(88)),
        ControlEntityRef::AutomationLane(AutomationLaneId::new(99)),
    ] {
        let q = query(anchor, RelationDirection::Both, 3, 64, false, rev(10));
        assert_query_agrees(&index, &oracle, &q);
        assert_eq!(index.query(&q).coverage, RelationCoverage::Unavailable);
    }

    // Deletion is reflected on rebuild: removing the assignment removes
    // its edges; queries against the old revision go Stale.
    f.remove_assignment(a1);
    let index2 = RelationIndex::build(f.project(), rev(12));
    let oracle2 = f.oracle(rev(12));
    let q = query(
        ControlEntityRef::Parameter(t1),
        RelationDirection::Incoming,
        4,
        64,
        false,
        rev(12),
    );
    assert_query_agrees(&index2, &oracle2, &q);
    assert_eq!(oracle2.query(&q).relations.len(), 0);
    // The previous generation still answers — as Stale.
    let old_again = query(
        ControlEntityRef::Parameter(t1),
        RelationDirection::Incoming,
        4,
        64,
        false,
        rev(12),
    );
    let stale_again = index.query(&old_again);
    assert_eq!(stale_again.coverage, RelationCoverage::Stale);
    assert_query_agrees(&index, &oracle, &old_again);

    // Target disappearance turns dependent edges Unresolved and makes
    // the vanished anchor Unavailable.
    let mut g = Fixture::new("pc-c6-rel06-gone");
    let gt = g.add_target(1);
    let gs = g.add_source(1, macro_source(0.5));
    let ga = g.add_assignment(1, gs, gt, None, true);
    g.add_lane(1, gt);
    g.remove_target(gt);
    let g_index = RelationIndex::build(g.project(), rev(13));
    let g_oracle = g.oracle(rev(13));
    for anchor in [
        ControlEntityRef::Assignment(ga),
        ControlEntityRef::AutomationLane(AutomationLaneId::new(1)),
        ControlEntityRef::Parameter(gt),
        ControlEntityRef::ModulationSource(gs),
    ] {
        for direction in [
            RelationDirection::Incoming,
            RelationDirection::Outgoing,
            RelationDirection::Both,
        ] {
            let q = query(anchor, direction, 4, 64, false, rev(13));
            assert_query_agrees(&g_index, &g_oracle, &q);
        }
    }
    let vanished = g_oracle.query(&query(
        ControlEntityRef::Assignment(ga),
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
    assert_eq!(
        g_oracle
            .query(&query(
                ControlEntityRef::Parameter(gt),
                RelationDirection::Both,
                3,
                64,
                false,
                rev(13),
            ))
            .coverage,
        RelationCoverage::Unavailable
    );
}

// ---------------------------------------------------------------------------
// REL07 — large graph (tens of thousands of nodes/edges) build+query
// measurement with oracle agreement and boundedness.
// ---------------------------------------------------------------------------

#[test]
#[expect(clippy::too_many_lines)]
fn rel07_large_graph_measurement() {
    // ~32k nodes / ~32k edges: 200 sources × 75 assignments → 15k
    // assignments (30k ModulationDrives edges) + 2k automation lanes.
    // No depth drivers → the edge graph is acyclic by construction, so
    // the oracle's cycle pass is legitimately skipped.
    const SOURCES: usize = 200;
    const ASSIGNMENTS_PER_SOURCE: usize = 75;
    const LANES: usize = 2_000;
    const TARGETS: usize = SOURCES * ASSIGNMENTS_PER_SOURCE;
    // Sanity bound only — the Issue defers real perf thresholds until
    // baselines are measured; we record actuals and fail only on
    // pathological non-termination.
    const SANITY: Duration = Duration::from_secs(30);

    let mut f = Fixture::new("pc-c6-rel07-big");
    let mut source_ids = Vec::new();
    for i in 0..SOURCES {
        source_ids.push(f.add_source(i as u128, macro_source(0.1)));
    }
    let mut target_ids = Vec::new();
    for i in 0..TARGETS {
        target_ids.push(f.add_target(1_000_000 + i as u128));
    }
    let mut assignment_ids = Vec::new();
    for (i, &s) in source_ids.iter().enumerate() {
        for j in 0..ASSIGNMENTS_PER_SOURCE {
            let idx = i * ASSIGNMENTS_PER_SOURCE + j;
            assignment_ids.push(f.add_assignment(
                2_000_000 + idx as u128,
                s,
                target_ids[idx],
                None,
                true,
            ));
        }
    }
    for i in 0..LANES {
        f.add_lane(500_000 + i as u128, target_ids[i % target_ids.len()]);
    }

    let edge_count = f.model.lanes.len()
        + f.model
            .assignments
            .values()
            .map(|a| 2 + usize::from(a.depth_source.is_some()))
            .sum::<usize>();
    let node_count = f.model.targets.len()
        + f.model.lanes.len()
        + f.model.sources.len()
        + f.model.assignments.len();
    eprintln!(
        "REL07 fixture: {node_count} nodes, {edge_count} edges \
         ({SOURCES} sources × {ASSIGNMENTS_PER_SOURCE} assignments, {LANES} lanes)"
    );

    let t0 = Instant::now();
    let index = RelationIndex::build(f.project(), rev(1));
    let build_elapsed = t0.elapsed();
    eprintln!("REL07 RelationIndex::build: {build_elapsed:?}");
    assert!(build_elapsed < SANITY, "build exceeded sanity bound");

    let t_oracle = Instant::now();
    let oracle = f.oracle(rev(1));
    let oracle_elapsed = t_oracle.elapsed();
    eprintln!("REL07 OracleIndex::build: {oracle_elapsed:?}");

    // Query battery: shallow fan-out, deep Both traversal, bounded
    // truncation, incoming lookup — each checked against the oracle.
    let cases: &[(ControlEntityRef, RelationDirection, u16, usize)] = &[
        (
            ControlEntityRef::ModulationSource(source_ids[0]),
            RelationDirection::Outgoing,
            1,
            1_000,
        ),
        (
            ControlEntityRef::ModulationSource(source_ids[0]),
            RelationDirection::Outgoing,
            2,
            1_000,
        ),
        (
            ControlEntityRef::ModulationSource(source_ids[199]),
            RelationDirection::Both,
            3,
            100_000,
        ),
        (
            ControlEntityRef::Parameter(target_ids[7_500]),
            RelationDirection::Incoming,
            4,
            100_000,
        ),
        (
            ControlEntityRef::AutomationLane(AutomationLaneId::new(500_123)),
            RelationDirection::Outgoing,
            2,
            100_000,
        ),
        (
            ControlEntityRef::Assignment(assignment_ids[3_333]),
            RelationDirection::Both,
            4,
            100_000,
        ),
        (
            ControlEntityRef::ModulationSource(source_ids[0]),
            RelationDirection::Outgoing,
            2,
            100,
        ), // truncation
        (
            ControlEntityRef::ModulationSource(source_ids[0]),
            RelationDirection::Outgoing,
            2,
            0,
        ), // zero budget → Truncated
    ];

    let mut measured = 0usize;
    for &(anchor, direction, max_depth, max_edges) in cases {
        let q = query(anchor, direction, max_depth, max_edges, false, rev(1));
        let t = Instant::now();
        let actual = index.query(&q);
        let elapsed = t.elapsed();
        measured += 1;
        eprintln!(
            "REL07 query {measured}: anchor={anchor:?} dir={direction:?} \
             depth={max_depth} max_edges={max_edges} → {} relations, \
             coverage={:?}, {elapsed:?}",
            actual.relations.len(),
            actual.coverage
        );
        assert!(elapsed < SANITY, "query exceeded sanity bound");
        assert_query_agrees(&index, &oracle, &q);
    }

    // Boundedness evidence: the widest traversal emits a bounded,
    // deterministic prefix; repeating it is identical.
    let wide = query(
        ControlEntityRef::ModulationSource(source_ids[199]),
        RelationDirection::Both,
        3,
        100_000,
        false,
        rev(1),
    );
    let t_wide = Instant::now();
    let wide_reply = index.query(&wide);
    let wide_elapsed = t_wide.elapsed();
    assert!(wide_elapsed < SANITY, "wide query exceeded sanity bound");
    assert_eq!(wide_reply.coverage, RelationCoverage::Complete);
    let wide_again = index.query(&wide);
    assert_eq!(wide_reply, wide_again, "large query must be deterministic");
    eprintln!(
        "REL07 widest query: {} relations in {wide_elapsed:?} (repeat identical)",
        wide_reply.relations.len()
    );

    // Stale guard still applies at scale.
    let stale = query(
        ControlEntityRef::ModulationSource(source_ids[0]),
        RelationDirection::Outgoing,
        2,
        100,
        false,
        rev(2),
    );
    assert_query_agrees(&index, &oracle, &stale);
    assert_eq!(index.query(&stale).coverage, RelationCoverage::Stale);
}
