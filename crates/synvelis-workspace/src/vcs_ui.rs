//! VCS-04 History/Visual Diff read-only projection (synvelis#95).
//!
//! [`VcsHistoryViews`] is the per-view state registry the app layer
//! drives through `syn.vcs.*` ops; [`vcs_history_view`] projects one
//! view's state plus a [`VcsSession`] context into a display DTO.
//! Everything here is read-only: history listing, revision pinning,
//! and `ChangeSet` comparison never mutate the project, the undo
//! stack, transport, the engine, or the VCS repository — the only
//! shared mutable state is the bounded [`ChangeSetCache`], which is a
//! derived cache, never a source of truth.
//!
//! Like [`crate::signal_flow_ui`]'s visual state, `VcsHistoryState`
//! deliberately stays out of `WorkspaceSnapshot`/`ViewState`
//! (schema v2, `deny_unknown_fields`): it is session-only.

use std::collections::BTreeMap;
use std::sync::Arc;

use ruviform_ui::dock::ViewInstanceId;
use synvelis_project::diff::{
    ChangeSet, ChangeSetCache, ChangeSetSummary, ComparisonCompleteness, ComparisonEquality,
    ComparisonSpec, DiffError, SemanticChange,
};
use synvelis_project::snapshot::{
    BlobProbe, FixedSnapshotLocator, ImmutableSnapshot, IndexGeneration, SnapshotCancel,
    SnapshotError, SnapshotLocator, SnapshotResolver,
};
use synvelis_project::vcs::{
    FixedVcsRevision, HistoryNode, HistoryQuery, RevisionSelector, VcsCancel, VcsError,
    VcsHistoryBackend,
};
use synvelis_project::{ProjectController, ProjectId, ProjectRevision, ProjectStore};

// -- bounds -------------------------------------------------------------

/// Maximum history nodes retained per view. Exceeding it refuses the
/// page load (recorded in `last_error`) rather than silently dropping
/// the oldest entries.
pub const MAX_HISTORY_NODES: usize = 512;

/// Maximum change rows projected into one [`VcsDiffView`].
pub const MAX_DIFF_CHANGES: usize = 256;

/// Maximum unresolved-issue rows projected into one [`VcsDiffView`].
pub const MAX_DIFF_ISSUES: usize = 128;

// -- pins / selection ----------------------------------------------------

/// Which side of an A/B comparison a [`PinnedEndpoint`] occupies.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PinSide {
    /// The "before" endpoint.
    Before,
    /// The "after" endpoint.
    After,
}

/// Re-resolution recipe for one comparison endpoint.
///
/// The pin records *what* was resolved, not the bytes: Live/Saved/
/// Staged pins name the observed mutable reference (revision, HEAD
/// name, index generation), Committed pins keep the full
/// [`FixedVcsRevision`] because [`FixedSnapshotLocator::VcsCommitted`]
/// carries only the object id, not the provider binding context.
#[derive(Clone, Debug, PartialEq)]
pub enum EndpointPin {
    /// Live project at `live_revision`.
    Live {
        /// Project the pin belongs to.
        project_id: ProjectId,
        /// Live revision observed at pin time.
        live_revision: ProjectRevision,
    },
    /// Saved store snapshot at `head`.
    Saved {
        /// Project the pin belongs to.
        project_id: ProjectId,
        /// HEAD snapshot name observed at pin time.
        head: String,
    },
    /// Staged index set at `index_generation`.
    Staged {
        /// Provider-side repository identity.
        repository_id: String,
        /// Index generation stable across the staged capture window.
        index_generation: IndexGeneration,
    },
    /// Committed project bytes at `revision`.
    Committed {
        /// The fully-resolved fixed revision.
        revision: FixedVcsRevision,
    },
}

/// One resolved comparison endpoint: the re-resolution recipe plus
/// the immutable snapshot it produced.
#[derive(Clone, Debug)]
pub struct PinnedEndpoint {
    /// What was pinned.
    pub pin: EndpointPin,
    /// The immutable snapshot resolved at pin time. Its
    /// `identity` (digest, locator, schema versions) is what
    /// [`ComparisonSpec`] binds — later Live/index/Saved-HEAD movement
    /// cannot rewrite it.
    pub snapshot: ImmutableSnapshot,
}

impl PinnedEndpoint {
    /// The snapshot identity `ComparisonSpec::new` consumes.
    #[must_use]
    pub fn identity(&self) -> &synvelis_project::snapshot::SnapshotIdentity {
        &self.snapshot.identity
    }
}

/// A complete A/B selection — materialized on demand from a view's
/// `before`/`after` pins.
#[derive(Clone, Debug)]
pub struct HistorySelection {
    /// The "before" endpoint.
    pub before: PinnedEndpoint,
    /// The "after" endpoint.
    pub after: PinnedEndpoint,
    /// For merge-commit "after" endpoints, the parent lineage the
    /// comparison follows (first parent). Display metadata only —
    /// the `ChangeSet` itself is computed on the pinned snapshots.
    pub chosen_parent: Option<FixedVcsRevision>,
}

/// Display mode for the diff projection.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum VcsViewMode {
    /// Both endpoints' changes listed side by side.
    #[default]
    SideBySide,
    /// Only the confirmed change rows, issue-dense.
    ChangeFocus,
}

// -- per-view state ------------------------------------------------------

/// Per-view History state — selection, pins, cursor, observed
/// revision. Session-only: never persisted into `WorkspaceSnapshot`.
#[derive(Clone, Debug, Default)]
pub struct VcsHistoryState {
    /// Loaded history nodes (bounded by [`MAX_HISTORY_NODES`]).
    pub nodes: Vec<HistoryNode>,
    /// Continuation cursor for the next page.
    pub next_cursor: Option<String>,
    /// `commit_oid` of the selected history node.
    pub selected: Option<String>,
    /// Pinned "before" endpoint.
    pub before: Option<PinnedEndpoint>,
    /// Pinned "after" endpoint.
    pub after: Option<PinnedEndpoint>,
    /// Merge-parent lineage metadata for the current selection.
    pub chosen_parent: Option<FixedVcsRevision>,
    /// The last computed `ChangeSet`, `Arc`-shared with the cache.
    pub current_diff: Option<Arc<ChangeSet>>,
    /// Live project revision this view last observed — the
    /// "new revision available" probe.
    pub observed_live_revision: Option<ProjectRevision>,
    /// Display mode for the diff projection.
    pub view_mode: VcsViewMode,
    /// Last failure, verbatim — cleared by the next successful op.
    pub last_error: Option<VcsUiError>,
}

impl VcsHistoryState {
    /// The materialized A/B selection when both endpoints are pinned.
    #[must_use]
    pub fn selection(&self) -> Option<HistorySelection> {
        match (&self.before, &self.after) {
            (Some(before), Some(after)) => Some(HistorySelection {
                before: before.clone(),
                after: after.clone(),
                chosen_parent: self.chosen_parent.clone(),
            }),
            _ => None,
        }
    }
}

/// View-scoped VCS history registry plus the shared `ChangeSet`
/// cache. Owned by `WorkspaceManager`; lifecycle follows the same
/// rules as [`crate::state::ViewStateRegistry`] — ensured on view
/// spawn/adopt/restore, removed on `remove_view` and
/// `WindowCloseDisposition::ReleaseViews`.
pub struct VcsHistoryViews {
    states: BTreeMap<ViewInstanceId, VcsHistoryState>,
    changesets: ChangeSetCache,
}

impl std::fmt::Debug for VcsHistoryViews {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VcsHistoryViews")
            .field("states", &self.states)
            .finish_non_exhaustive()
    }
}

impl Default for VcsHistoryViews {
    fn default() -> Self {
        Self::new()
    }
}

impl VcsHistoryViews {
    /// Default cache capacity — bounded, LRU-evicted.
    pub const DEFAULT_CACHE_CAPACITY: usize = 32;

    /// A registry with the default cache capacity.
    #[must_use]
    pub fn new() -> Self {
        Self {
            states: BTreeMap::new(),
            changesets: ChangeSetCache::new(Self::DEFAULT_CACHE_CAPACITY),
        }
    }

    /// Ensures `view` has a state entry (idempotent).
    pub fn ensure(&mut self, view: ViewInstanceId) {
        self.states.entry(view).or_default();
    }

    /// Removes `view`'s state entirely.
    pub fn remove(&mut self, view: ViewInstanceId) {
        self.states.remove(&view);
    }

    /// Whether `view` has state.
    #[must_use]
    pub fn contains(&self, view: ViewInstanceId) -> bool {
        self.states.contains_key(&view)
    }

    /// The view's state, if any.
    #[must_use]
    pub fn get(&self, view: ViewInstanceId) -> Option<&VcsHistoryState> {
        self.states.get(&view)
    }

    /// Mutable access for tests/advanced callers.
    #[must_use]
    pub fn get_mut(&mut self, view: ViewInstanceId) -> Option<&mut VcsHistoryState> {
        self.states.get_mut(&view)
    }

    // -- ops ---------------------------------------------------------------

    /// Replaces the loaded history with a fresh first page (or the
    /// given query's first page). Node selection/pins are preserved;
    /// a selected oid that left the page stays selected (display-only).
    ///
    /// # Errors
    /// [`VcsUiError::NoBackend`], [`VcsUiError::Vcs`].
    pub fn refresh_history(
        &mut self,
        view: ViewInstanceId,
        ctx: &VcsSession<'_>,
        query: Option<&HistoryQuery>,
        cancel: &VcsCancel,
    ) -> Result<(), VcsUiError> {
        let backend = ctx.backend.ok_or(VcsUiError::NoBackend)?;
        let mut q = query.cloned().unwrap_or_default();
        q.cursor = None;
        let page = match backend.list_history(&q, cancel) {
            Ok(page) => page,
            Err(err) => {
                let ui_err = VcsUiError::Vcs(err);
                self.states.entry(view).or_default().last_error = Some(ui_err.clone());
                return Err(ui_err);
            }
        };
        let state = self.states.entry(view).or_default();
        state.nodes = page.nodes;
        state.next_cursor = page.next_cursor;
        state.observed_live_revision = Some(ctx.controller.live_revision());
        state.last_error = None;
        Ok(())
    }

    /// Appends the next history page. Refuses (recorded in
    /// `last_error`, [`VcsUiError::LimitExceeded`]) rather than
    /// silently dropping oldest nodes when the bound would be
    /// exceeded; `next_cursor` is retained either way.
    ///
    /// # Errors
    /// [`VcsUiError::NoBackend`], [`VcsUiError::Vcs`],
    /// [`VcsUiError::LimitExceeded`].
    pub fn load_more_history(
        &mut self,
        view: ViewInstanceId,
        ctx: &VcsSession<'_>,
        cancel: &VcsCancel,
    ) -> Result<(), VcsUiError> {
        let backend = ctx.backend.ok_or(VcsUiError::NoBackend)?;
        let cursor = self
            .states
            .get(&view)
            .and_then(|s| s.next_cursor.clone())
            .ok_or_else(|| VcsUiError::InvalidRequest {
                detail: "no further history page".into(),
            })?;
        let q = HistoryQuery {
            cursor: Some(cursor),
            ..HistoryQuery::default()
        };
        let page = match backend.list_history(&q, cancel) {
            Ok(page) => page,
            Err(err) => {
                let ui_err = VcsUiError::Vcs(err);
                self.states.entry(view).or_default().last_error = Some(ui_err.clone());
                return Err(ui_err);
            }
        };
        let state = self.states.entry(view).or_default();
        if state.nodes.len() + page.nodes.len() > MAX_HISTORY_NODES {
            let err = VcsUiError::LimitExceeded {
                detail: format!(
                    "history exceeds {MAX_HISTORY_NODES} nodes ({} loaded)",
                    state.nodes.len()
                ),
            };
            state.last_error = Some(err.clone());
            state.observed_live_revision = Some(ctx.controller.live_revision());
            return Err(err);
        }
        state.nodes.extend(page.nodes);
        state.next_cursor = page.next_cursor;
        state.observed_live_revision = Some(ctx.controller.live_revision());
        state.last_error = None;
        Ok(())
    }

    /// Selects a loaded history node by index — display selection
    /// only; it never pins or mutates anything.
    ///
    /// # Errors
    /// [`VcsUiError::InvalidRequest`] when `index` is out of range.
    pub fn select_node(&mut self, view: ViewInstanceId, index: usize) -> Result<(), VcsUiError> {
        let state = self.states.entry(view).or_default();
        let node = state
            .nodes
            .get(index)
            .ok_or_else(|| VcsUiError::InvalidRequest {
                detail: format!("history node index {index} out of range"),
            })?;
        state.selected = Some(node.revision.commit_oid.clone());
        Ok(())
    }

    /// Pins `side` to `request`'s resolved snapshot. When both
    /// endpoints end up pinned, a fresh `ChangeSet` is computed
    /// through the shared cache. Failures — including cancellation —
    /// are recorded in `last_error` and returned; a successful pin
    /// clears it.
    ///
    /// # Errors
    /// [`VcsUiError::NoBackend`]/[`VcsUiError::NoStore`] when the
    /// request needs an absent dependency,
    /// [`VcsUiError::InvalidRequest`] for an unknown node index,
    /// [`VcsUiError::Vcs`]/[`VcsUiError::Snapshot`]/[`VcsUiError::Diff`]
    /// for provider/resolver/compare failures, and
    /// [`VcsUiError::Vcs`]`(`[`VcsError::Cancelled`]`)` on cancellation.
    pub fn pin_endpoint(
        &mut self,
        view: ViewInstanceId,
        ctx: &VcsSession<'_>,
        side: PinSide,
        request: &EndpointRequest,
        cancel: &VcsCancel,
    ) -> Result<(), VcsUiError> {
        if cancel.is_cancelled() {
            let err = VcsUiError::Vcs(VcsError::Cancelled);
            self.states.entry(view).or_default().last_error = Some(err.clone());
            return Err(err);
        }
        // Node requests read the loaded page — clone the node so the
        // state borrow ends before resolution.
        let node = match request {
            EndpointRequest::Node { index } => Some(
                self.states
                    .get(&view)
                    .and_then(|s| s.nodes.get(*index))
                    .cloned()
                    .ok_or_else(|| VcsUiError::InvalidRequest {
                        detail: format!("history node index {index} out of range"),
                    })?,
            ),
            _ => None,
        };
        let outcome = match resolve_pin(ctx, request, node.as_ref(), cancel) {
            Ok(outcome) => outcome,
            Err(err) => {
                self.states.entry(view).or_default().last_error = Some(err.clone());
                return Err(err);
            }
        };
        let state = self.states.entry(view).or_default();
        match side {
            PinSide::Before => state.before = Some(outcome.endpoint),
            PinSide::After => {
                state.after = Some(outcome.endpoint);
                state.chosen_parent = outcome.chosen_parent;
            }
        }
        state.observed_live_revision = Some(ctx.controller.live_revision());
        // Both endpoints pinned → refresh the shared ChangeSet.
        if let (Some(before_pin), Some(after_pin)) = (&state.before, &state.after) {
            let before = before_pin.snapshot.clone();
            let after = after_pin.snapshot.clone();
            let spec = ComparisonSpec::new(before.identity.clone(), after.identity.clone());
            let snap_cancel = SnapshotCancel::new();
            match self
                .changesets
                .get_or_compare(&before, &after, &spec, &snap_cancel)
            {
                Ok(diff) => {
                    state.current_diff = Some(diff);
                    state.last_error = None;
                    return Ok(());
                }
                Err(err) => {
                    let ui_err = VcsUiError::Diff(err);
                    state.last_error = Some(ui_err.clone());
                    return Err(ui_err);
                }
            }
        }
        state.last_error = None;
        Ok(())
    }

    /// Recomputes the `ChangeSet` for the current pins through the
    /// shared cache (cache hit returns the identical `Arc`).
    ///
    /// # Errors
    /// [`VcsUiError::InvalidRequest`] without both pins,
    /// [`VcsUiError::Diff`] on compare failure.
    pub fn compare_selection(
        &mut self,
        view: ViewInstanceId,
        ctx: &VcsSession<'_>,
        cancel: &VcsCancel,
    ) -> Result<(), VcsUiError> {
        if cancel.is_cancelled() {
            let err = VcsUiError::Vcs(VcsError::Cancelled);
            self.states.entry(view).or_default().last_error = Some(err.clone());
            return Err(err);
        }
        let state = self.states.entry(view).or_default();
        let (before, after) = match (&state.before, &state.after) {
            (Some(b), Some(a)) => (b.snapshot.clone(), a.snapshot.clone()),
            _ => {
                return Err(VcsUiError::InvalidRequest {
                    detail: "both endpoints must be pinned".into(),
                });
            }
        };
        let spec = ComparisonSpec::new(before.identity.clone(), after.identity.clone());
        let snap_cancel = SnapshotCancel::new();
        let diff = match self
            .changesets
            .get_or_compare(&before, &after, &spec, &snap_cancel)
        {
            Ok(diff) => diff,
            Err(err) => {
                let ui_err = VcsUiError::Diff(err);
                state.last_error = Some(ui_err.clone());
                return Err(ui_err);
            }
        };
        state.current_diff = Some(diff);
        state.observed_live_revision = Some(ctx.controller.live_revision());
        state.last_error = None;
        Ok(())
    }

    /// Sets the diff display mode — visual-only, no recompute.
    pub fn set_view_mode(&mut self, view: ViewInstanceId, mode: VcsViewMode) {
        self.states.entry(view).or_default().view_mode = mode;
    }

    /// Clears the A/B selection and current diff. History nodes and
    /// node selection are preserved.
    pub fn clear_selection(&mut self, view: ViewInstanceId) {
        let state = self.states.entry(view).or_default();
        state.before = None;
        state.after = None;
        state.chosen_parent = None;
        state.current_diff = None;
    }
}

// -- resolution ------------------------------------------------------------

/// What a pin request names — the `syn.vcs.visual` wire vocabulary.
#[derive(Clone, Debug)]
pub enum EndpointRequest {
    /// The live project at its current revision.
    Live,
    /// The saved store's HEAD, optionally pinned to `head`.
    Saved {
        /// Expected HEAD name; `None` resolves whatever HEAD is.
        head: Option<String>,
    },
    /// The currently staged index set.
    Staged,
    /// A provider revision expression resolved exactly once.
    Committed {
        /// Mutable selector — resolved to a [`FixedVcsRevision`] once.
        selector: RevisionSelector,
    },
    /// A loaded history node (its `FixedVcsRevision` is already
    /// pinned); `index` refers to the view's loaded `nodes`.
    Node {
        /// Index into the view's loaded history page.
        index: usize,
    },
}

/// The read sources one VCS view operation needs — composed by the
/// app layer from the session, store, and injected backend. Every
/// field is a shared reference; nothing here can mutate them.
pub struct VcsSession<'a> {
    /// The bound VCS history backend (`None` = not attached).
    pub backend: Option<&'a dyn VcsHistoryBackend>,
    /// The live project controller (read-only use).
    pub controller: &'a ProjectController,
    /// The project store for Saved endpoints (`None` = unavailable).
    pub store: Option<&'a dyn ProjectStore>,
    /// Optional asset blob probe for snapshot completeness.
    pub blobs: Option<&'a dyn BlobProbe>,
}

/// Failures the VCS view orchestration can report. Provider/resolver/
/// diff errors stay wrapped verbatim — display layers map them to
/// status without re-interpreting.
#[derive(Clone, Debug, PartialEq)]
pub enum VcsUiError {
    /// No VCS backend is attached to the session.
    NoBackend,
    /// A Saved endpoint was requested without a project store.
    NoStore,
    /// The request itself is malformed (bad index, missing pins).
    InvalidRequest {
        /// What was wrong.
        detail: String,
    },
    /// A declared bound would be exceeded (history node cap).
    LimitExceeded {
        /// What was bounded.
        detail: String,
    },
    /// A [`VcsError`] from the backend.
    Vcs(VcsError),
    /// A [`SnapshotError`] from resolution.
    Snapshot(SnapshotError),
    /// A [`DiffError`] from comparison.
    Diff(DiffError),
}

impl VcsUiError {
    /// `true` for cooperative-cancellation failures in any wrapped
    /// layer.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        matches!(
            self,
            Self::Vcs(VcsError::Cancelled)
                | Self::Snapshot(SnapshotError::Cancelled)
                | Self::Diff(DiffError::Cancelled)
        )
    }
}

impl std::fmt::Display for VcsUiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBackend => f.write_str("no VCS backend attached"),
            Self::NoStore => f.write_str("no project store attached"),
            Self::InvalidRequest { detail } | Self::LimitExceeded { detail } => f.write_str(detail),
            Self::Vcs(err) => write!(f, "vcs: {err}"),
            Self::Snapshot(err) => write!(f, "snapshot: {err}"),
            Self::Diff(err) => write!(f, "diff: {err}"),
        }
    }
}

impl std::error::Error for VcsUiError {}

impl From<VcsError> for VcsUiError {
    fn from(err: VcsError) -> Self {
        Self::Vcs(err)
    }
}

impl From<SnapshotError> for VcsUiError {
    fn from(err: SnapshotError) -> Self {
        Self::Snapshot(err)
    }
}

impl From<DiffError> for VcsUiError {
    fn from(err: DiffError) -> Self {
        Self::Diff(err)
    }
}

/// A resolved pin plus merge-parent metadata.
struct PinOutcome {
    endpoint: PinnedEndpoint,
    chosen_parent: Option<FixedVcsRevision>,
}

/// Builds the [`EndpointPin`] recipe from the resolved snapshot's
/// locator. Committed endpoints are the exception: their locator
/// carries only the object id, so the caller-supplied
/// [`FixedVcsRevision`] (which keeps the provider binding context) is
/// used instead.
fn pin_from_snapshot(
    snapshot: &ImmutableSnapshot,
    committed: Option<&FixedVcsRevision>,
) -> EndpointPin {
    match &snapshot.identity.locator {
        FixedSnapshotLocator::Live {
            project_id,
            live_revision,
        } => EndpointPin::Live {
            project_id: *project_id,
            live_revision: *live_revision,
        },
        FixedSnapshotLocator::Saved { project_id, head } => EndpointPin::Saved {
            project_id: *project_id,
            head: head.clone(),
        },
        FixedSnapshotLocator::VcsStaged {
            repository_id,
            index_generation,
            ..
        } => EndpointPin::Staged {
            repository_id: repository_id.clone(),
            index_generation: index_generation.clone(),
        },
        FixedSnapshotLocator::VcsCommitted { .. } => EndpointPin::Committed {
            revision: committed
                .expect("committed snapshot always carries its revision")
                .clone(),
        },
        // Recovery endpoints are not part of this slice's vocabulary.
        FixedSnapshotLocator::Recovery { .. } => {
            unreachable!("recovery snapshots are never resolved here")
        }
    }
}

/// Resolves `request` to a [`PinnedEndpoint`] plus the merge-parent
/// hint recorded on an After pin.
fn resolve_pin(
    ctx: &VcsSession<'_>,
    request: &EndpointRequest,
    node: Option<&HistoryNode>,
    cancel: &VcsCancel,
) -> Result<PinOutcome, VcsUiError> {
    let resolver = SnapshotResolver::new();
    let snap_cancel = SnapshotCancel::new();
    let check = |cancel: &VcsCancel| {
        if cancel.is_cancelled() {
            Err(VcsUiError::Vcs(VcsError::Cancelled))
        } else {
            Ok(())
        }
    };
    match request {
        EndpointRequest::Live => {
            check(cancel)?;
            let project_id = ctx.controller.live().project_id;
            let live_revision = ctx.controller.live_revision();
            let snapshot = resolver
                .capture_live(ctx.controller, live_revision, &snap_cancel, ctx.blobs)
                .map_err(VcsUiError::Snapshot)?;
            Ok(PinOutcome {
                endpoint: PinnedEndpoint {
                    pin: EndpointPin::Live {
                        project_id,
                        live_revision,
                    },
                    snapshot,
                },
                chosen_parent: None,
            })
        }
        EndpointRequest::Saved { head } => {
            check(cancel)?;
            let store = ctx.store.ok_or(VcsUiError::NoStore)?;
            let project_id = ctx.controller.live().project_id;
            let snapshot = resolver
                .resolve_saved(
                    store,
                    &SnapshotLocator::Saved {
                        project_id,
                        head_ref: head.clone(),
                    },
                    &snap_cancel,
                    ctx.blobs,
                )
                .map_err(VcsUiError::Snapshot)?;
            let pin = pin_from_snapshot(&snapshot, None);
            Ok(PinOutcome {
                endpoint: PinnedEndpoint { pin, snapshot },
                chosen_parent: None,
            })
        }
        EndpointRequest::Staged => {
            check(cancel)?;
            let backend = ctx.backend.ok_or(VcsUiError::NoBackend)?;
            let snapshot = backend
                .resolve_staged(&snap_cancel)
                .map_err(VcsUiError::Snapshot)?;
            let pin = pin_from_snapshot(&snapshot, None);
            Ok(PinOutcome {
                endpoint: PinnedEndpoint { pin, snapshot },
                chosen_parent: None,
            })
        }
        EndpointRequest::Committed { selector } => {
            check(cancel)?;
            let backend = ctx.backend.ok_or(VcsUiError::NoBackend)?;
            let revision = backend
                .resolve_fixed_revision(selector, cancel)
                .map_err(VcsUiError::Vcs)?;
            committed_pin(backend, revision, None, cancel)
        }
        EndpointRequest::Node { .. } => {
            check(cancel)?;
            let backend = ctx.backend.ok_or(VcsUiError::NoBackend)?;
            let node = node.expect("node request always carries its node");
            let chosen_parent = (node.parents.len() > 1).then(|| node.parents[0].clone());
            committed_pin(backend, node.revision.clone(), chosen_parent, cancel)
        }
    }
}

/// Shared Committed/Node resolution: pin the fixed revision, resolve
/// its snapshot, and record the merge-parent hint.
fn committed_pin(
    backend: &dyn VcsHistoryBackend,
    revision: FixedVcsRevision,
    chosen_parent: Option<FixedVcsRevision>,
    cancel: &VcsCancel,
) -> Result<PinOutcome, VcsUiError> {
    if cancel.is_cancelled() {
        return Err(VcsUiError::Vcs(VcsError::Cancelled));
    }
    let snap_cancel = SnapshotCancel::new();
    let snapshot = backend
        .resolve_committed(&revision, &snap_cancel)
        .map_err(VcsUiError::Snapshot)?;
    let pin = EndpointPin::Committed { revision };
    Ok(PinOutcome {
        endpoint: PinnedEndpoint { pin, snapshot },
        chosen_parent,
    })
}

// -- freshness -------------------------------------------------------------

/// Whether a pinned endpoint still describes the observed mutable
/// reference. Committed pins are immutable by construction — always
/// `Fresh`; resolution failures surface as errors, not staleness.
#[derive(Clone, Debug, PartialEq)]
pub enum EndpointFreshness {
    /// The pinned reference still matches the observed value.
    Fresh,
    /// The observed value moved — the pinned snapshot is unchanged
    /// but no longer current.
    Stale {
        /// What moved.
        reason: String,
    },
    /// Freshness could not be determined (missing store/backend or a
    /// failed probe). Never reported as `Fresh`.
    Unknown {
        /// Why it could not be determined.
        reason: String,
    },
}

/// Evaluates one pin against the current observed references.
#[must_use]
pub fn endpoint_freshness(pin: &EndpointPin, ctx: &VcsSession<'_>) -> EndpointFreshness {
    match pin {
        EndpointPin::Committed { .. } => EndpointFreshness::Fresh,
        EndpointPin::Live {
            project_id,
            live_revision,
        } => {
            let live = ctx.controller.live();
            if live.project_id != *project_id {
                return EndpointFreshness::Stale {
                    reason: "project identity changed".into(),
                };
            }
            let current = ctx.controller.live_revision();
            if current == *live_revision {
                EndpointFreshness::Fresh
            } else {
                EndpointFreshness::Stale {
                    reason: format!("live revision advanced to r{}", current.0),
                }
            }
        }
        EndpointPin::Saved { project_id, head } => {
            if ctx.controller.live().project_id != *project_id {
                return EndpointFreshness::Stale {
                    reason: "project identity changed".into(),
                };
            }
            let Some(store) = ctx.store else {
                return EndpointFreshness::Unknown {
                    reason: "no project store".into(),
                };
            };
            match store.head() {
                Ok(Some(current)) if current == *head => EndpointFreshness::Fresh,
                Ok(Some(current)) => EndpointFreshness::Stale {
                    reason: format!("store HEAD moved to {current}"),
                },
                Ok(None) => EndpointFreshness::Stale {
                    reason: "store HEAD cleared".into(),
                },
                Err(err) => EndpointFreshness::Unknown {
                    reason: format!("store head probe failed: {err}"),
                },
            }
        }
        EndpointPin::Staged {
            repository_id,
            index_generation,
        } => {
            let Some(backend) = ctx.backend else {
                return EndpointFreshness::Unknown {
                    reason: "no VCS backend".into(),
                };
            };
            if backend.repository_id() != repository_id {
                return EndpointFreshness::Stale {
                    reason: "repository binding changed".into(),
                };
            }
            match backend.index_generation() {
                Ok(current) if current == *index_generation => EndpointFreshness::Fresh,
                Ok(_) => EndpointFreshness::Stale {
                    reason: "staged index generation moved".into(),
                },
                Err(err) => EndpointFreshness::Unknown {
                    reason: format!("index generation probe failed: {err:?}"),
                },
            }
        }
    }
}

// -- read model ------------------------------------------------------------

/// Coarse view status for the History surface.
#[derive(Clone, Debug, PartialEq)]
pub enum VcsViewStatus {
    /// Content is available.
    Ready,
    /// No VCS backend is attached (history listing impossible;
    /// Live/Saved endpoints may still resolve).
    NoBackend,
    /// Backend attached but nothing loaded yet.
    Empty,
    /// The last operation failed — `detail` is the verbatim error.
    Failed {
        /// Verbatim failure detail.
        detail: String,
    },
    /// The last operation was cancelled.
    Cancelled,
}

/// One history row — display-ready, verbatim provider metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct VcsHistoryRow {
    /// Full commit object id.
    pub commit_oid: String,
    /// Short display form (first 12 chars).
    pub short_oid: String,
    /// Parent commit oids (all merge parents preserved).
    pub parents: Vec<String>,
    /// `true` for merge commits (`parents.len() > 1`).
    pub is_merge: bool,
    /// Committer timestamp (unix seconds), when reported.
    pub committed_at_unix: Option<i64>,
    /// Author identity text, when reported.
    pub author: Option<String>,
    /// First line of the commit message, when reported.
    pub message_headline: Option<String>,
    /// Provider-reported neighborhood completeness.
    pub completeness: synvelis_project::vcs::HistoryCompleteness,
    /// Whether this row is the selected node.
    pub selected: bool,
}

/// Endpoint kind label for display.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EndpointKind {
    /// Live project.
    Live,
    /// Saved store snapshot.
    Saved,
    /// Staged index set.
    Staged,
    /// Committed revision.
    Committed,
}

/// One endpoint's display projection.
#[derive(Clone, Debug, PartialEq)]
pub struct VcsEndpointView {
    /// Endpoint family.
    pub kind: EndpointKind,
    /// Human label (oid short / head name / revision).
    pub label: String,
    /// Freshness against the currently observed references.
    pub freshness: EndpointFreshness,
    /// Content digest of the pinned snapshot (`sha256:...`).
    pub digest: String,
}

/// Tri-state equality without the issue payload (issues are listed
/// separately) — `Unknown` is never collapsed to `Equal`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VcsEquality {
    /// Zero confirmed changes, zero unresolved issues.
    Equal,
    /// At least one confirmed change.
    Different,
    /// Equality cannot be claimed (unresolved issues).
    Unknown,
}

/// One confirmed change row.
#[derive(Clone, Debug, PartialEq)]
pub struct VcsChangeRow {
    /// Deterministic change id.
    pub id: String,
    /// Semantic domain.
    pub domain: synvelis_project::diff::ComparisonDomain,
    /// Object kind.
    pub object_kind: synvelis_project::diff::ObjectKind,
    /// Stable object id.
    pub stable_id: String,
    /// Change classification.
    pub kind: synvelis_project::diff::ChangeKind,
    /// Field-level evidence count.
    pub field_count: usize,
    /// Whether an affected time range exists.
    pub has_range: bool,
}

/// One unresolved issue row.
#[derive(Clone, Debug, PartialEq)]
pub struct VcsIssueRow {
    /// Issue tag (`unsupported-schema`, `missing-asset`, ...).
    pub kind: String,
    /// Verbatim detail.
    pub detail: String,
}

/// The diff projection of the current selection.
#[derive(Clone, Debug, PartialEq)]
pub struct VcsDiffView {
    /// Comparison-key digest (`sha256:...`).
    pub key_digest: String,
    /// Tri-state equality.
    pub overall: VcsEquality,
    /// Coverage completeness.
    pub completeness: ComparisonCompleteness,
    /// Confirmed-change counts.
    pub summary: ChangeSetSummary,
    /// Bounded change rows.
    pub changes: Vec<VcsChangeRow>,
    /// Bounded issue rows.
    pub issues: Vec<VcsIssueRow>,
    /// `true` when rows were truncated by the display bounds.
    pub truncated: bool,
}

/// The materialized selection's display projection.
#[derive(Clone, Debug, PartialEq)]
pub struct VcsSelectionView {
    /// Before endpoint.
    pub before: VcsEndpointView,
    /// After endpoint.
    pub after: VcsEndpointView,
    /// Merge-parent lineage label, when recorded.
    pub chosen_parent: Option<String>,
    /// Display mode.
    pub view_mode: VcsViewMode,
    /// Diff projection, when computed.
    pub diff: Option<VcsDiffView>,
}

/// The full read-model projection for one History view.
#[derive(Clone, Debug, PartialEq)]
pub struct VcsHistoryView {
    /// Live project revision at projection time.
    pub project_revision: ProjectRevision,
    /// Coarse status.
    pub status: VcsViewStatus,
    /// Whether a VCS backend is attached.
    pub backend_attached: bool,
    /// History rows in provider order.
    pub rows: Vec<VcsHistoryRow>,
    /// Continuation cursor, when more pages exist.
    pub next_cursor: Option<String>,
    /// Selected node oid.
    pub selected: Option<String>,
    /// A/B selection projection, when both endpoints are pinned.
    pub selection: Option<VcsSelectionView>,
    /// `true` when the live revision advanced past
    /// `observed_live_revision` — fixed pins are unaffected.
    pub new_revision_available: bool,
    /// Last operation failure, verbatim.
    pub last_error: Option<String>,
    /// Bound repository identity, when a backend is attached.
    pub repository_id: Option<String>,
}

fn short_oid(oid: &str) -> String {
    oid.chars().take(12).collect()
}

fn endpoint_kind(pin: &EndpointPin) -> EndpointKind {
    match pin {
        EndpointPin::Live { .. } => EndpointKind::Live,
        EndpointPin::Saved { .. } => EndpointKind::Saved,
        EndpointPin::Staged { .. } => EndpointKind::Staged,
        EndpointPin::Committed { .. } => EndpointKind::Committed,
    }
}

fn endpoint_label(pin: &EndpointPin) -> String {
    match pin {
        EndpointPin::Live { live_revision, .. } => format!("live r{}", live_revision.0),
        EndpointPin::Saved { head, .. } => format!("saved {head}"),
        EndpointPin::Staged {
            index_generation, ..
        } => format!("staged@{}", index_generation.0),
        EndpointPin::Committed { revision } => short_oid(&revision.commit_oid),
    }
}

fn endpoint_view(
    pin: &EndpointPin,
    snapshot: &ImmutableSnapshot,
    ctx: &VcsSession<'_>,
) -> VcsEndpointView {
    VcsEndpointView {
        kind: endpoint_kind(pin),
        label: endpoint_label(pin),
        freshness: endpoint_freshness(pin, ctx),
        digest: snapshot.identity.digest.to_string(),
    }
}

fn issue_kind(issue: &synvelis_project::diff::ComparisonIssue) -> &'static str {
    use synvelis_project::diff::ComparisonIssue as I;
    match issue {
        I::UnsupportedSchema { .. } => "unsupported-schema",
        I::MissingAsset { .. } => "missing-asset",
        I::MissingOpaqueState { .. } => "missing-opaque-state",
        I::AmbiguousIdentity { .. } => "ambiguous-identity",
        I::PartialProjection { .. } => "partial-projection",
        I::UnresolvedSource { .. } => "unresolved-source",
        I::InvalidReference { .. } => "invalid-reference",
    }
}

fn issue_detail(issue: &synvelis_project::diff::ComparisonIssue) -> String {
    use synvelis_project::diff::ComparisonIssue as I;
    match issue {
        I::UnsupportedSchema { found, side } => {
            format!("schema {found} unsupported ({side:?})")
        }
        I::MissingAsset {
            asset_id,
            side,
            status,
        } => format!("asset {asset_id} {status:?} ({side:?})"),
        I::MissingOpaqueState {
            processor_id,
            side,
            status,
        } => format!("processor {processor_id} opaque state {status:?} ({side:?})"),
        I::AmbiguousIdentity { object, reason } => {
            format!("{} identity ambiguous: {reason:?}", object.stable_id)
        }
        I::PartialProjection { domain, detail } => {
            format!("{domain:?} partial projection: {detail}")
        }
        I::UnresolvedSource { asset_id, side } => {
            format!("asset {asset_id} source unresolved ({side:?})")
        }
        I::InvalidReference { detail } => detail.clone(),
    }
}

fn change_row(change: &SemanticChange) -> VcsChangeRow {
    VcsChangeRow {
        id: change.id.0.clone(),
        domain: change.object.kind.domain(),
        object_kind: change.object.kind,
        stable_id: change.object.stable_id.clone(),
        kind: change.kind,
        field_count: change.evidence.fields.len(),
        has_range: change.affected_range.is_some(),
    }
}

fn diff_view(diff: &ChangeSet) -> VcsDiffView {
    let truncated =
        diff.changes.len() > MAX_DIFF_CHANGES || diff.unresolved.len() > MAX_DIFF_ISSUES;
    VcsDiffView {
        key_digest: diff.key.digest().to_string(),
        overall: match &diff.overall {
            ComparisonEquality::Equal => VcsEquality::Equal,
            ComparisonEquality::Different => VcsEquality::Different,
            ComparisonEquality::Unknown(_) => VcsEquality::Unknown,
        },
        completeness: diff.completeness,
        summary: diff.summary,
        changes: diff
            .changes
            .iter()
            .take(MAX_DIFF_CHANGES)
            .map(change_row)
            .collect(),
        issues: diff
            .unresolved
            .iter()
            .take(MAX_DIFF_ISSUES)
            .map(|issue| VcsIssueRow {
                kind: issue_kind(issue).to_string(),
                detail: issue_detail(issue),
            })
            .collect(),
        truncated,
    }
}

fn selection_view(state: &VcsHistoryState, ctx: &VcsSession<'_>) -> Option<VcsSelectionView> {
    let selection = state.selection()?;
    Some(VcsSelectionView {
        before: endpoint_view(&selection.before.pin, &selection.before.snapshot, ctx),
        after: endpoint_view(&selection.after.pin, &selection.after.snapshot, ctx),
        chosen_parent: state
            .chosen_parent
            .as_ref()
            .map(|r| short_oid(&r.commit_oid)),
        view_mode: state.view_mode,
        diff: state.current_diff.as_deref().map(diff_view),
    })
}

/// Projects one view's VCS state into the display DTO. Pure read —
/// freshness probes (`store.head()`, `index_generation()`) are the
/// only backend touches and they are read-only by contract.
#[must_use]
pub fn vcs_history_view(
    views: &VcsHistoryViews,
    view: ViewInstanceId,
    ctx: &VcsSession<'_>,
) -> VcsHistoryView {
    let state = views.get(view);
    let empty = VcsHistoryState::default();
    let state = state.unwrap_or(&empty);

    let status = if let Some(err) = &state.last_error {
        if err.is_cancelled() {
            VcsViewStatus::Cancelled
        } else {
            VcsViewStatus::Failed {
                detail: err.to_string(),
            }
        }
    } else if state.nodes.is_empty() && state.selection().is_none() {
        if ctx.backend.is_none() {
            VcsViewStatus::NoBackend
        } else {
            VcsViewStatus::Empty
        }
    } else {
        VcsViewStatus::Ready
    };

    let live_revision = ctx.controller.live_revision();
    VcsHistoryView {
        project_revision: live_revision,
        status,
        backend_attached: ctx.backend.is_some(),
        rows: state
            .nodes
            .iter()
            .map(|node| {
                let oid = &node.revision.commit_oid;
                VcsHistoryRow {
                    commit_oid: oid.clone(),
                    short_oid: short_oid(oid),
                    parents: node.parents.iter().map(|p| p.commit_oid.clone()).collect(),
                    is_merge: node.parents.len() > 1,
                    committed_at_unix: node.committed_at_unix,
                    author: node.author.clone(),
                    message_headline: node
                        .message
                        .as_deref()
                        .and_then(|m| m.lines().next())
                        .map(str::to_string),
                    completeness: node.completeness,
                    selected: state.selected.as_deref() == Some(oid.as_str()),
                }
            })
            .collect(),
        next_cursor: state.next_cursor.clone(),
        selected: state.selected.clone(),
        selection: selection_view(state, ctx),
        new_revision_available: state
            .observed_live_revision
            .is_some_and(|observed| observed != live_revision),
        last_error: state.last_error.as_ref().map(ToString::to_string),
        repository_id: ctx.backend.map(|b| b.repository_id().to_string()),
    }
}
