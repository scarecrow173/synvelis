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

use crate::control_edge::ControlEdgeGesture;
use crate::gesture::Gesture;
use crate::routing_gesture::RoutingGesture;
use crate::selection::{EditBinding, LaneScope, ObjectSelections, TimeSelection};
use crate::signal_flow_ui::SignalFlowViewState;
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

/// Whether the inspector follows link-group context, explores freely,
/// or stays pinned (Issue #5 semantics — Inspector-owned, never an
/// editing-selection concern).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InspectionMode {
    /// Follow incoming link events (default).
    #[default]
    Follow,
    /// Inspect targets explicitly without link-driven retargeting;
    /// back/forward walk the Inspector's own explore history.
    Explore,
    /// Keep the pinned target regardless of link events.
    Pinned,
}

/// What a Follow-mode inspector tracks. `None` on
/// [`InspectionFocus::follow_source`] keeps the legacy behaviour —
/// follow every event the view's own link group delivers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FollowSource {
    /// Follow events published by this view.
    View(ViewInstanceId),
    /// Follow events published inside this link group.
    Group(crate::link::LinkGroupId),
}

/// The view-local inspection target.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectionFocus {
    /// Inspected entity, if any.
    pub target: Option<SemanticTarget>,
    /// Follow/Explore/Pinned mode.
    pub mode: InspectionMode,
    /// Optional narrowing of Follow mode to one source view or link
    /// group. `None` follows whatever the view's own group delivers.
    #[serde(default)]
    pub follow_source: Option<FollowSource>,
    /// Explore back-stack — session state only, never persisted or
    /// carried into presets (Issue #5: explore history resets).
    #[serde(default, skip)]
    pub explore_back: Vec<SemanticTarget>,
    /// Explore forward-stack — session state only.
    #[serde(default, skip)]
    pub explore_fwd: Vec<SemanticTarget>,
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
    /// The running Audio Routing gesture, if any. Session state
    /// only — never persisted.
    #[serde(skip)]
    pub gesture: Option<Gesture>,
    /// The running Control Layer edge gesture, if any. Session
    /// state only — never persisted, and a different type from
    /// [`Gesture`] so the two gesture families cannot be confused.
    #[serde(skip)]
    pub control_edge_gesture: Option<ControlEdgeGesture>,
    /// The running Signal Flow routing gesture, if any. Session
    /// state only — never persisted, and a different type from both
    /// [`Gesture`] and [`ControlEdgeGesture`] so the three gesture
    /// families cannot be confused (SF-UX-03).
    #[serde(skip)]
    pub routing_gesture: Option<RoutingGesture>,
    /// Inspection focus (view-local).
    pub inspection_focus: InspectionFocus,
    /// Link-derived context (view-local, delivered by peers).
    pub linked_context: LinkedContext,
    /// Per-view Signal Flow state — visual arrangement plus routing
    /// inspection focus (SF-UX-04/07). `None` for views that never
    /// hosted a Signal Flow surface; absent in pre-v3 snapshots and
    /// defaulted on migrate. A malformed persisted payload degrades
    /// to `None` (safe recovery) instead of rejecting the snapshot.
    #[serde(default, deserialize_with = "deserialize_signal_flow")]
    pub signal_flow: Option<SignalFlowViewState>,
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

/// Tolerant per-view Signal Flow state decode (SF-UX-07 safe
/// recovery): a malformed persisted payload degrades to `None`
/// instead of rejecting the whole workspace snapshot. Well-formed
/// payloads — including explicit `null` — round-trip normally, and a
/// payload written by a newer schema (unknown fields) is dropped
/// rather than corrupting the restore.
fn deserialize_signal_flow<'de, D>(deserializer: D) -> Result<Option<SignalFlowViewState>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|v| serde_json::from_value(v).ok()))
}
