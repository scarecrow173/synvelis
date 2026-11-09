//! Relation read model (Synvelis #102 PC-C5 slice 1, UI-C5-01) — the
//! bidirectional control-relation projection behind the Inspector's
//! relation table.
//!
//! [`relation_rows`] builds a fresh
//! [`synvelis_project::RelationIndex`] from `session`'s live project
//! at the live [`ProjectRevision`] and answers one bounded
//! [`RelationQuery`], mapping the [`RelationReply`] into display
//! DTOs. It is read-side only: no domain ops, no mutation of the
//! project, the evaluator, the compiled graph, or undo history — the
//! same contract [`crate::arrangement_rows`] follows.
//!
//! **Coverage honesty:** [`RelationView::coverage`] carries the
//! [`RelationCoverage`] verbatim (`Complete`/`Truncated`/`Stale`/
//! `Unavailable`) and [`RelationRow::state`] carries
//! [`RelationState`] verbatim (`Enabled`/`Disabled`/`Unresolved`).
//! An empty `rows` list is never the only signal that the
//! neighborhood could not be fully resolved — `Disabled` edges are
//! included by default so they surface as `Disabled`, not vanish.
//!
//! **Exploration model:** the default query is depth-1 in
//! [`RelationDirection::Both`] — the anchor's direct relations.
//! Every emitted edge touches the anchor, so
//! [`RelationRow::neighbor`] yields the entity on the far side and
//! [`RelationNav`] composes hops (`Parameter → Assignment → Source →
//! Assignment → affected Parameter`) plus pop-based return to the
//! origin anchor. Deeper one-shot expansion is deliberately not the
//! default: rows that do not touch the anchor would make the
//! row→next-anchor mapping ambiguous.
//!
//! **RT contract:** non-RT only.

use synvelis_project::{
    ControlEntityRef, ModulationAssignmentId, ProjectRevision, RelationCoverage, RelationDirection,
    RelationIndex, RelationKind, RelationQuery, RelationState,
};

use crate::session::ProjectSession;

/// Default hop bound for [`relation_rows`] — direct relations of the
/// anchor only; multi-hop exploration composes depth-1 queries
/// through [`RelationNav`].
pub const RELATION_VIEW_MAX_DEPTH: u16 = 1;

/// Default emitted-edge budget for [`relation_rows`] — a bounded
/// Inspector table; exceeding it reports `Truncated` rather than
/// silently cutting rows.
pub const RELATION_VIEW_MAX_EDGES: usize = 32;

/// Default `include_disabled` for [`relation_rows`] — the Inspector
/// surfaces disabled edges as `Disabled` rows instead of dropping
/// them ("Disabled を潰さない").
pub const RELATION_VIEW_INCLUDE_DISABLED: bool = true;

/// Bounds and caller pins for [`relation_rows_with`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelationViewOptions {
    /// Maximum hop count from the anchor (`0` = anchor only).
    pub max_depth: u16,
    /// Maximum emitted relations; exceeding it yields `Truncated`.
    pub max_edges: usize,
    /// Whether `Disabled` edges are emitted (and traversed).
    pub include_disabled: bool,
    /// The revision the caller believes is live. `None` queries at
    /// the session's live revision; `Some(stale)` reproduces the
    /// index's `Stale` coverage path for staleness handling.
    pub expected_revision: Option<ProjectRevision>,
}

impl Default for RelationViewOptions {
    fn default() -> Self {
        Self {
            max_depth: RELATION_VIEW_MAX_DEPTH,
            max_edges: RELATION_VIEW_MAX_EDGES,
            include_disabled: RELATION_VIEW_INCLUDE_DISABLED,
            expected_revision: None,
        }
    }
}

/// One relation edge as a display/navigation row — the
/// [`synvelis_project::ControlRelation`] fields needed by the
/// Inspector, kept verbatim (no state/kind collapsing).
#[derive(Clone, Debug, PartialEq)]
pub struct RelationRow {
    /// Stored-direction source entity.
    pub source: ControlEntityRef,
    /// Stored-direction target entity.
    pub target: ControlEntityRef,
    /// Edge semantics — verbatim [`RelationKind`].
    pub kind: RelationKind,
    /// Build-time evidence state — verbatim [`RelationState`];
    /// `Disabled`/`Unresolved` are surfaced, never folded into
    /// `Enabled` or dropped.
    pub state: RelationState,
    /// Owning assignment for modulation/depth edges; `None` for
    /// automation edges.
    pub assignment: Option<ModulationAssignmentId>,
}

impl RelationRow {
    /// Tagged id of the stored-direction source entity
    /// (`param-…`/`automation-lane-…`/`mod-source-…`/`mod-assign-…`).
    #[must_use]
    pub fn source_label(&self) -> String {
        entity_label(self.source)
    }

    /// Tagged id of the stored-direction target entity.
    #[must_use]
    pub fn target_label(&self) -> String {
        entity_label(self.target)
    }

    /// The entity on the far side of this edge from `anchor` — `Some`
    /// only when the edge touches the anchor (always true for
    /// depth-1 query rows; deeper BFS rows that do not touch the
    /// anchor return `None` and are display-only).
    #[must_use]
    pub fn neighbor(&self, anchor: ControlEntityRef) -> Option<ControlEntityRef> {
        if self.source == anchor {
            Some(self.target)
        } else if self.target == anchor {
            Some(self.source)
        } else {
            None
        }
    }
}

/// The relation neighborhood of one anchor — rows plus the verbatim
/// coverage/generation evidence of the query that produced them.
#[derive(Clone, Debug, PartialEq)]
pub struct RelationView {
    /// The anchor the query walked from.
    pub anchor: ControlEntityRef,
    /// Which adjacency the query walked.
    pub direction: RelationDirection,
    /// Index generation (`ProjectRevision`) that produced `rows` —
    /// compare with the live revision to detect drift.
    pub project_revision: ProjectRevision,
    /// Verbatim [`RelationCoverage`]. `Stale`/`Unavailable`/
    /// `Truncated` are carried as-is; `rows` may be empty *because*
    /// of them, never instead of them.
    pub coverage: RelationCoverage,
    /// Emitted edges in deterministic query order (≤ `max_edges`).
    pub rows: Vec<RelationRow>,
}

impl RelationView {
    /// Tagged id of the anchor entity.
    #[must_use]
    pub fn anchor_label(&self) -> String {
        entity_label(self.anchor)
    }

    /// Whether `project_revision` still matches `session`'s live
    /// revision — `false` means these rows describe a stale project
    /// state and must not be presented as current.
    #[must_use]
    pub fn is_stale(&self, session: &ProjectSession) -> bool {
        self.project_revision != session.revision()
    }
}

/// Roundtrip anchor navigation for relation exploration (UI-C5-01):
/// a last-in-first-out stack of [`ControlEntityRef`] anchors. Push the
/// [`RelationRow::neighbor`] of the row being followed to descend;
/// pop returns toward the origin anchor — "元画面へ戻る" without
/// losing the path. Pure value type: it stores no project data and
/// never mutates the session; freshness is judged per query through
/// [`RelationView`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RelationNav {
    /// Anchor history — the last entry is the current anchor.
    anchors: Vec<ControlEntityRef>,
}

impl RelationNav {
    /// An empty stack — no anchor selected yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The current anchor — the top of the stack, `None` when empty.
    #[must_use]
    pub fn current(&self) -> Option<ControlEntityRef> {
        self.anchors.last().copied()
    }

    /// Whether a "back" target exists (more than the current anchor).
    #[must_use]
    pub fn can_go_back(&self) -> bool {
        self.anchors.len() > 1
    }

    /// Number of anchors on the stack.
    #[must_use]
    pub fn len(&self) -> usize {
        self.anchors.len()
    }

    /// Whether the stack holds no anchor.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.anchors.is_empty()
    }

    /// Descend: push `anchor` as the new current. Pushing the current
    /// anchor is a no-op so re-selecting the same entity cannot wedge
    /// the stack.
    pub fn push(&mut self, anchor: ControlEntityRef) {
        if self.current() != Some(anchor) {
            self.anchors.push(anchor);
        }
    }

    /// Return toward the origin: pops the current anchor and returns
    /// the restored one (`None` when the stack empties).
    pub fn pop(&mut self) -> Option<ControlEntityRef> {
        self.anchors.pop()?;
        self.current()
    }

    /// Clears the whole stack — back to "no anchor".
    pub fn clear(&mut self) {
        self.anchors.clear();
    }
}

/// Queries the control-relation neighborhood of `anchor` against
/// `session`'s live project at the live revision, with the default
/// [`RelationViewOptions`].
///
/// Read-side only — see the module docs for the coverage-honesty and
/// exploration contracts.
#[must_use]
pub fn relation_rows(
    session: &ProjectSession,
    anchor: ControlEntityRef,
    direction: RelationDirection,
) -> RelationView {
    relation_rows_with(session, anchor, direction, RelationViewOptions::default())
}

/// [`relation_rows`] with explicit bounds/flags — the entry point
/// tests and later slices use to exercise `Truncated`,
/// `include_disabled`, deeper hops, or a pinned
/// `expected_revision` (`Stale` path).
#[must_use]
pub fn relation_rows_with(
    session: &ProjectSession,
    anchor: ControlEntityRef,
    direction: RelationDirection,
    options: RelationViewOptions,
) -> RelationView {
    let live = session.revision();
    let index = RelationIndex::build(session.project(), live);
    let reply = index.query(&RelationQuery {
        anchor,
        direction,
        max_depth: options.max_depth,
        max_edges: options.max_edges,
        include_disabled: options.include_disabled,
        expected_revision: options.expected_revision.unwrap_or(live),
    });
    RelationView {
        anchor,
        direction,
        project_revision: reply.project_revision,
        coverage: reply.coverage,
        rows: reply
            .relations
            .into_iter()
            .map(|relation| RelationRow {
                source: relation.source,
                target: relation.target,
                kind: relation.kind,
                state: relation.state,
                assignment: relation.assignment_id,
            })
            .collect(),
    }
}

/// Tagged display id of a control entity — delegates to each ID's
/// canonical `"<kind>-<32hex>"` form.
fn entity_label(entity: ControlEntityRef) -> String {
    match entity {
        ControlEntityRef::Parameter(id) => id.to_tagged(),
        ControlEntityRef::AutomationLane(id) => id.to_tagged(),
        ControlEntityRef::ModulationSource(id) => id.to_tagged(),
        ControlEntityRef::Assignment(id) => id.to_tagged(),
    }
}
