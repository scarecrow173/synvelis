//! Read-only bidirectional relation index over [`Project::control`]
//! (issue #99, PC-C2).
//!
//! [`RelationIndex`] is a derived projection — never a second source of
//! truth. It is built from the four `BTreeMap`s of [`ControlModel`] at
//! one [`ProjectRevision`], answers bounded [`RelationQuery`]s in both
//! directions, and reports its own generation so a consumer can detect
//! staleness. Building or querying never mutates the project, the
//! evaluator, the compiled [`ControlGraph`], or undo history.
//!
//! Edge model (canonical direction is always stored; query direction
//! only chooses which adjacency is walked):
//!
//! | kind | source | target |
//! | --- | --- | --- |
//! | `AutomationDrives` | `AutomationLane(lane)` | `Parameter(lane.target)` |
//! | `ModulationDrives` | `ModulationSource(a.source)` | `Assignment(a)` |
//! | `ModulationDrives` | `Assignment(a)` | `Parameter(a.target)` |
//! | `DepthControlledBy` | `Parameter(a.depth_source)` | `Assignment(a)` |
//!
//! State is fixed at build time: a dangling endpoint is `Unresolved`, a
//! disabled assignment's edges are `Disabled`, an edge whose endpoints
//! share a non-trivial enabled-graph SCC is `Unresolved` (mirroring
//! [`ControlGraph::compile`]'s `ControlCycle` rejection), and everything
//! else is `Enabled`.
//!
//! [`Project::control`]: crate::project::Project::control
//! [`ControlModel`]: crate::control::ControlModel

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    ProjectRevision,
    id::{AutomationLaneId, ModulationAssignmentId, ModulationSourceId, ParameterTargetId},
    project::Project,
};

/// A node of the control relation graph — one of the four entity kinds
/// persisted in [`ControlModel`]. `Copy + Ord` keeps adjacency order
/// deterministic.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ControlEntityRef {
    /// A [`ParameterTarget`](crate::control::ParameterTarget).
    Parameter(ParameterTargetId),
    /// An [`AutomationLane`](crate::control::AutomationLane).
    AutomationLane(AutomationLaneId),
    /// A [`ModulationSource`](crate::control::ModulationSource).
    ModulationSource(ModulationSourceId),
    /// A [`ModulationAssignment`](crate::control::ModulationAssignment)
    /// — assignments are explicit nodes so duplicate source→target
    /// bindings stay distinguishable.
    Assignment(ModulationAssignmentId),
}

impl From<ParameterTargetId> for ControlEntityRef {
    fn from(id: ParameterTargetId) -> Self {
        Self::Parameter(id)
    }
}

impl From<AutomationLaneId> for ControlEntityRef {
    fn from(id: AutomationLaneId) -> Self {
        Self::AutomationLane(id)
    }
}

impl From<ModulationSourceId> for ControlEntityRef {
    fn from(id: ModulationSourceId) -> Self {
        Self::ModulationSource(id)
    }
}

impl From<ModulationAssignmentId> for ControlEntityRef {
    fn from(id: ModulationAssignmentId) -> Self {
        Self::Assignment(id)
    }
}

/// The semantic kind of a relation edge.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RelationKind {
    /// `AutomationLane → ParameterTarget`: the lane's series drives the
    /// parameter.
    AutomationDrives,
    /// `ModulationSource → Assignment` or `Assignment → ParameterTarget`:
    /// the assignment routes its source signal into the target's depth.
    ModulationDrives,
    /// `ParameterTarget(depth_source) → Assignment`: the parameter's
    /// evaluated value scales the assignment's depth.
    DepthControlledBy,
}

/// Which adjacency a [`RelationQuery`] walks. The emitted
/// [`ControlRelation`]s always keep their stored source→target
/// direction — this only selects traversal reachability.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RelationDirection {
    /// Walk edges against their stored direction (target → source).
    Incoming,
    /// Walk edges along their stored direction (source → target).
    Outgoing,
    /// Union of both — undirected reachability for "everything tied to
    /// this entity" exploration.
    Both,
}

/// Evidence state of one relation edge, fixed at index build time.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RelationState {
    /// Both endpoints resolve and the edge participates in no enabled
    /// cycle — contributes to evaluation.
    Enabled,
    /// The owning assignment is disabled — visible only when the query
    /// sets `include_disabled`, and never traversed otherwise (matches
    /// "disabled contributes nothing").
    Disabled,
    /// An endpoint reference dangles (`target`/`source`/`depth_source` not in
    /// the model) or the edge participates in an enabled cycle —
    /// reported, never silently fabricated or treated as live.
    Unresolved,
    /// Reserved for relation kinds this projection does not produce.
    Unsupported,
}

/// How complete a [`RelationReply`] is.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RelationCoverage {
    /// Every reachable edge within the query bounds was emitted.
    Complete,
    /// `max_edges` stopped the walk early — `relations` is a deterministic
    /// prefix; raise the budget or narrow the anchor/direction for more.
    Truncated,
    /// `expected_revision` did not match the index generation — no
    /// relations are returned rather than risking stale IDs.
    Stale,
    /// The anchor is not a known control entity — no relations.
    Unavailable,
}

/// One bounded relation query against a [`RelationIndex`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationQuery {
    /// Entity the walk starts from.
    pub anchor: ControlEntityRef,
    /// Which adjacency to traverse.
    pub direction: RelationDirection,
    /// Maximum hop count from the anchor (0 = anchor only, no edges).
    pub max_depth: u16,
    /// Maximum emitted relations; exceeding it yields `Truncated`.
    pub max_edges: usize,
    /// When false, `Disabled` edges are neither emitted nor traversed.
    pub include_disabled: bool,
    /// The revision the caller believes the index was built from.
    pub expected_revision: ProjectRevision,
}

/// One relation edge as reported by a query — always in stored
/// canonical `source → target` direction regardless of query
/// direction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlRelation {
    /// Edge semantics.
    pub kind: RelationKind,
    /// Stored-direction source entity.
    pub source: ControlEntityRef,
    /// Stored-direction target entity.
    pub target: ControlEntityRef,
    /// Owning assignment for modulation/depth edges; `None` for
    /// automation edges.
    pub assignment_id: Option<ModulationAssignmentId>,
    /// Build-time evidence state.
    pub state: RelationState,
}

/// The result of one [`RelationIndex::query`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationReply {
    /// The index generation that produced this reply — compare with the
    /// live [`ProjectRevision`] to detect drift even on `Complete`.
    pub project_revision: ProjectRevision,
    /// Emitted edges in deterministic BFS order (≤ `max_edges`).
    pub relations: Vec<ControlRelation>,
    /// Completeness evidence.
    pub coverage: RelationCoverage,
}

/// One canonical edge of the projection, stored in build order.
#[derive(Clone, Debug)]
struct RelationEdge {
    kind: RelationKind,
    source: ControlEntityRef,
    target: ControlEntityRef,
    assignment_id: Option<ModulationAssignmentId>,
    state: RelationState,
    cyclic: bool,
}

impl RelationEdge {
    fn to_relation(&self) -> ControlRelation {
        ControlRelation {
            kind: self.kind,
            source: self.source,
            target: self.target,
            assignment_id: self.assignment_id,
            state: self.state,
        }
    }
}

/// A revision-tagged, read-only bidirectional index over
/// [`Project::control`]. Build once per project revision; queries are
/// bounded BFS walks over precomputed adjacency.
pub struct RelationIndex {
    revision: ProjectRevision,
    entities: BTreeSet<ControlEntityRef>,
    edges: Vec<RelationEdge>,
    /// `ControlEntityRef →` indices into `edges`, ascending = the
    /// deterministic build order.
    outgoing: BTreeMap<ControlEntityRef, Vec<usize>>,
    incoming: BTreeMap<ControlEntityRef, Vec<usize>>,
}

impl RelationIndex {
    /// Builds the index from `project.control` at `revision`.
    ///
    /// O(N) in the control model size plus one SCC pass over the
    /// non-disabled edge graph. Never mutates `project`.
    #[must_use]
    pub fn build(project: &Project, revision: ProjectRevision) -> Self {
        let control = &project.control;
        let mut entities = BTreeSet::new();
        for id in control.targets.keys() {
            entities.insert(ControlEntityRef::Parameter(*id));
        }
        for id in control.lanes.keys() {
            entities.insert(ControlEntityRef::AutomationLane(*id));
        }
        for id in control.sources.keys() {
            entities.insert(ControlEntityRef::ModulationSource(*id));
        }
        for id in control.assignments.keys() {
            entities.insert(ControlEntityRef::Assignment(*id));
        }

        let mut edges: Vec<RelationEdge> = Vec::new();
        let mut push = |kind: RelationKind,
                        source: ControlEntityRef,
                        target: ControlEntityRef,
                        assignment_id: Option<ModulationAssignmentId>,
                        dangling: bool,
                        enabled: bool| {
            let state = if dangling {
                RelationState::Unresolved
            } else if !enabled {
                RelationState::Disabled
            } else {
                RelationState::Enabled
            };
            edges.push(RelationEdge {
                kind,
                source,
                target,
                assignment_id,
                state,
                cyclic: false,
            });
        };

        for lane in control.lanes.values() {
            push(
                RelationKind::AutomationDrives,
                ControlEntityRef::AutomationLane(lane.id),
                ControlEntityRef::Parameter(lane.target),
                None,
                !control.targets.contains_key(&lane.target),
                true,
            );
        }
        for assignment in control.assignments.values() {
            let asn = ControlEntityRef::Assignment(assignment.id);
            push(
                RelationKind::ModulationDrives,
                ControlEntityRef::ModulationSource(assignment.source),
                asn,
                Some(assignment.id),
                !control.sources.contains_key(&assignment.source),
                assignment.enabled,
            );
            push(
                RelationKind::ModulationDrives,
                asn,
                ControlEntityRef::Parameter(assignment.target),
                Some(assignment.id),
                !control.targets.contains_key(&assignment.target),
                assignment.enabled,
            );
            if let Some(depth_source) = assignment.depth_source {
                push(
                    RelationKind::DepthControlledBy,
                    ControlEntityRef::Parameter(depth_source),
                    asn,
                    Some(assignment.id),
                    !control.targets.contains_key(&depth_source),
                    assignment.enabled,
                );
            }
        }

        // Enabled-graph SCC (non-disabled edges only): endpoints sharing
        // a non-trivial component are cycle participants → Unresolved,
        // mirroring `ControlGraph::compile`'s enabled-only `ControlCycle`
        // rejection without changing its contract.
        mark_cyclic(&mut edges);

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

    /// The project revision this index was built from.
    #[must_use]
    pub fn revision(&self) -> ProjectRevision {
        self.revision
    }

    /// Answers a bounded bidirectional query.
    ///
    /// Never fails and never mutates: stale revisions and unknown
    /// anchors return empty relation lists with explicit coverage.
    #[must_use]
    pub fn query(&self, query: &RelationQuery) -> RelationReply {
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

        // Traversal sides in scan order: Both walks outgoing first,
        // then incoming, per node.
        let sides: &[Side] = match query.direction {
            RelationDirection::Outgoing => &[Side::Outgoing],
            RelationDirection::Incoming => &[Side::Incoming],
            RelationDirection::Both => &[Side::Outgoing, Side::Incoming],
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
                    Side::Outgoing => self.outgoing.get(&node),
                    Side::Incoming => self.incoming.get(&node),
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
                        Side::Outgoing => edge.target,
                        Side::Incoming => edge.source,
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

/// Which adjacency a traversal step reads.
#[derive(Copy, Clone)]
enum Side {
    Outgoing,
    Incoming,
}

/// Finds edges whose endpoints share a non-trivial SCC of the
/// non-disabled edge graph (iterative Kosaraju — explicit stacks, no
/// recursion depth risk on malformed input).
fn cyclic_edge_indices(edges: &[RelationEdge]) -> BTreeSet<usize> {
    let mut out: BTreeMap<ControlEntityRef, Vec<ControlEntityRef>> = BTreeMap::new();
    let mut inn: BTreeMap<ControlEntityRef, Vec<ControlEntityRef>> = BTreeMap::new();
    for edge in edges.iter().filter(|e| e.state != RelationState::Disabled) {
        out.entry(edge.source).or_default().push(edge.target);
        inn.entry(edge.target).or_default().push(edge.source);
    }
    if out.is_empty() {
        return BTreeSet::new();
    }

    // Pass 1: finishing order on the forward graph.
    let mut visited = BTreeSet::new();
    let mut finish_order = Vec::with_capacity(out.len());
    for &start in out.keys() {
        if !visited.insert(start) {
            continue;
        }
        let mut stack = vec![(start, 0usize)];
        while let Some(&(node, index)) = stack.last() {
            let neighbors = out.get(&node).map_or(&[][..], Vec::as_slice);
            if index < neighbors.len() {
                let next = neighbors[index];
                if let Some(top) = stack.last_mut() {
                    top.1 += 1;
                }
                if visited.insert(next) {
                    stack.push((next, 0));
                }
            } else {
                finish_order.push(node);
                stack.pop();
            }
        }
    }

    // Pass 2: component assignment on the reversed graph.
    let mut component = BTreeMap::new();
    let mut component_size: BTreeMap<usize, usize> = BTreeMap::new();
    let mut next_component = 0usize;
    for &start in finish_order.iter().rev() {
        if component.contains_key(&start) {
            continue;
        }
        let id = next_component;
        next_component += 1;
        let mut stack = vec![start];
        component.insert(start, id);
        *component_size.entry(id).or_default() += 1;
        while let Some(node) = stack.pop() {
            for &prev in inn.get(&node).map_or(&[][..], Vec::as_slice) {
                if let std::collections::btree_map::Entry::Vacant(entry) = component.entry(prev) {
                    entry.insert(id);
                    *component_size.entry(id).or_default() += 1;
                    stack.push(prev);
                }
            }
        }
    }

    // An edge is cyclic when both endpoints sit in one non-trivial
    // component. Self-loops cannot occur (every edge crosses entity
    // variants), so size > 1 is the only cycle shape.
    edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| {
            if edge.state == RelationState::Disabled {
                return false;
            }
            match (component.get(&edge.source), component.get(&edge.target)) {
                (Some(&a), Some(&b)) => a == b && component_size.get(&a).copied().unwrap_or(0) > 1,
                _ => false,
            }
        })
        .map(|(index, _)| index)
        .collect()
}

/// Marks cyclic edges `Unresolved` in place.
fn mark_cyclic(edges: &mut [RelationEdge]) {
    let cyclic = cyclic_edge_indices(edges);
    for (index, edge) in edges.iter_mut().enumerate() {
        if cyclic.contains(&index) {
            edge.cyclic = true;
            if edge.state == RelationState::Enabled {
                edge.state = RelationState::Unresolved;
            }
        }
    }
}
