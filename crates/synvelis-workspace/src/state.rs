//! Per-view state: object selection, time selection, lane scope,
//! inspection focus, linked context, tool/grid/snap configuration and
//! viewport — deliberately separate concepts (WS-A-01 + ED-A-01).
//!
//! - [`crate::selection::ObjectSelections`] — the view-local *editing
//!   target*, per domain; never broadcast, never read by another view.
//! - [`crate::selection::TimeSelection`] / [`crate::selection::LaneScope`]
//!   — the view-local time range and lane reach, independent of the
//!   object selection.
//! - [`InspectionFocus`] — the view-local *inspected entity* with a
//!   Follow/Pinned mode.
//! - [`LinkedContext`] — link-derived highlight/context received from
//!   link-group peers; it is *not* an editing selection.
//! - [`Viewport`] — scroll/zoom display state, local to the view.

use std::collections::BTreeMap;

use ruviform_ui::dock::ViewInstanceId;
use serde::{Deserialize, Serialize};
use synvelis_project::{ClipId, LaneId, ProjectRevision, SourceId, TrackId};

use crate::gesture::Gesture;
use crate::selection::{EditBinding, LaneScope, ObjectSelections, TimeSelection};
use crate::snap::{GridConfig, SnapConfig};
use crate::tool::Tool;

/// A semantic project entity — the currency of selections, focus,
/// link events and command targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticTarget {
    /// A track.
    Track(TrackId),
    /// A lane.
    Lane(LaneId),
    /// A clip.
    Clip(ClipId),
    /// A media source.
    Source(SourceId),
}

/// The view-local editing selection. Ordered; the last pushed target
/// is the *primary* one (a single-target selection keeps a
/// one-element `targets`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditingSelection {
    /// Selected targets, in selection order.
    pub targets: Vec<SemanticTarget>,
}

impl EditingSelection {
    /// Selection of a single target.
    #[must_use]
    pub fn single(target: SemanticTarget) -> Self {
        Self {
            targets: vec![target],
        }
    }

    /// The primary (most recent) target, if any.
    #[must_use]
    pub fn primary(&self) -> Option<SemanticTarget> {
        self.targets.last().copied()
    }

    /// Whether nothing is selected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

/// Whether the inspector follows link-group context or stays pinned.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InspectionMode {
    /// Follow incoming link events (default).
    #[default]
    Follow,
    /// Keep the pinned target regardless of link events.
    Pinned,
}

/// The view-local inspection target.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectionFocus {
    /// Inspected entity, if any.
    pub target: Option<SemanticTarget>,
    /// Follow/Pinned mode.
    pub mode: InspectionMode,
}

/// Link-derived context delivered by a link-group peer. Distinct from
/// the editing selection: it is a highlight/context hint, not an edit
/// target.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedContext {
    /// Semantic target received via the link group.
    pub target: Option<SemanticTarget>,
    /// The view that published the last applied event.
    pub source: Option<ViewInstanceId>,
    /// The last applied event id (dedup evidence).
    pub last_event: Option<u64>,
}

/// Display state — scroll/zoom. Local to each view.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    /// Horizontal scroll offset (arbitrary units).
    pub scroll_x: f64,
    /// Vertical scroll offset (arbitrary units).
    pub scroll_y: f64,
    /// Horizontal zoom factor.
    pub zoom_x: f64,
    /// Vertical zoom factor.
    pub zoom_y: f64,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            scroll_x: 0.0,
            scroll_y: 0.0,
            zoom_x: 1.0,
            zoom_y: 1.0,
        }
    }
}

/// The full state bundle carried by one view.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewState {
    /// Object selection, kept per editing domain (view-local).
    #[serde(default)]
    pub object_selections: ObjectSelections,
    /// Time selection (view-local) — may coexist with the object
    /// selection and never implicitly triggers object commands.
    #[serde(default)]
    pub time_selection: Option<TimeSelection>,
    /// Lane scope for time-domain commands (view-local).
    #[serde(default)]
    pub lane_scope: LaneScope,
    /// Active editing tool (view-local).
    #[serde(default)]
    pub active_tool: Tool,
    /// Display grid configuration — cosmetic only.
    #[serde(default)]
    pub grid: GridConfig,
    /// Snap configuration — independent of the display grid.
    #[serde(default)]
    pub snap: SnapConfig,
    /// Editor display/edit binding — *not* a command target.
    #[serde(default)]
    pub edit_binding: Option<EditBinding>,
    /// The running gesture, if any. Session state only — never
    /// persisted.
    #[serde(skip)]
    pub gesture: Option<Gesture>,
    /// Inspection focus (view-local).
    pub inspection_focus: InspectionFocus,
    /// Link-derived context (view-local, delivered by peers).
    pub linked_context: LinkedContext,
    /// Viewport (view-local).
    pub viewport: Viewport,
    /// Project revisions delivered to this view — the observable
    /// proof that update distribution reached the view.
    #[serde(default)]
    pub revision_inbox: Vec<ProjectRevision>,
}

/// Registry of per-view state, keyed by view id. Independent of dock
/// placement: a detached or mid-transfer view keeps its state.
#[derive(Debug, Default)]
pub struct ViewStateRegistry {
    states: BTreeMap<ViewInstanceId, ViewState>,
}

impl ViewStateRegistry {
    /// Inserts a default state for `id` (no-op when present).
    pub fn ensure(&mut self, id: ViewInstanceId) -> &mut ViewState {
        self.states.entry(id).or_default()
    }

    /// Inserts `state` for `id`, replacing any existing one.
    pub fn insert(&mut self, id: ViewInstanceId, state: ViewState) {
        self.states.insert(id, state);
    }

    /// Removes `id`'s state.
    pub fn remove(&mut self, id: ViewInstanceId) -> Option<ViewState> {
        self.states.remove(&id)
    }

    /// The state for `id`.
    #[must_use]
    pub fn get(&self, id: ViewInstanceId) -> Option<&ViewState> {
        self.states.get(&id)
    }

    /// Mutable state for `id`.
    #[must_use]
    pub fn get_mut(&mut self, id: ViewInstanceId) -> Option<&mut ViewState> {
        self.states.get_mut(&id)
    }

    /// All ids holding state.
    #[must_use]
    pub fn ids(&self) -> Vec<ViewInstanceId> {
        self.states.keys().copied().collect()
    }
}
