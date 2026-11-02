//! Workspace snapshot persistence (WS-A scope 6).
//!
//! [`WorkspaceSnapshot`] is the Synvelis-side envelope wrapping
//! `ruviform-ui`'s `LayoutSnapshot`: it adds the view descriptors +
//! per-view state, link-group assignments, and window titles —
//! everything the dock layer deliberately does not know. Layout
//! structure stays inside `LayoutSnapshot` so validation, monitor
//! matching and safe-default recovery come from `ruviform-ui`'s
//! `restore`/`restore_or_default` unchanged.
//!
//! Window identity is positional: `LayoutSnapshot::windows` is
//! capture-ordered, so `window_titles[i]` pairs with `windows[i]` —
//! restored windows get fresh `WindowId`s in that order. View
//! identity is logical (`ViewInstanceId` raw values) and survives.

use ruviform_ui::dock::LayoutSnapshot;
use serde::{Deserialize, Serialize};

use crate::error::WorkspaceError;
use crate::state::ViewState;
use crate::view::ViewType;

/// Envelope schema version — bump on incompatible changes.
pub const WORKSPACE_SCHEMA: u32 = 1;

/// One persisted view: identity + state. Position lives in the
/// `LayoutSnapshot` dock trees (or the detached pool).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewRecord {
    /// `ViewInstanceId` raw value.
    pub id: u64,
    /// Surface kind.
    pub kind: ViewType,
    /// Display title.
    pub title: String,
    /// Full per-view state bundle.
    pub state: ViewState,
}

/// One persisted link assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRecord {
    /// `ViewInstanceId` raw value.
    pub view: u64,
    /// `LinkGroupId` raw value.
    pub group: u64,
}

/// The persisted workspace layout envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSnapshot {
    /// Envelope schema — must equal [`WORKSPACE_SCHEMA`].
    pub schema: u32,
    /// Dock/window layout (validated by `ruviform-ui` on restore).
    pub layout: LayoutSnapshot,
    /// Window titles in `layout.windows` order.
    #[serde(default)]
    pub window_titles: Vec<String>,
    /// All views (docked + detached).
    #[serde(default)]
    pub views: Vec<ViewRecord>,
    /// Link-group assignments.
    #[serde(default)]
    pub links: Vec<LinkRecord>,
}

impl WorkspaceSnapshot {
    /// Serializes to JSON.
    ///
    /// # Errors
    /// [`WorkspaceError::Snapshot`] on serialization failure.
    pub fn to_json(&self) -> Result<String, WorkspaceError> {
        serde_json::to_string(self)
            .map_err(|e| WorkspaceError::Snapshot(format!("cannot serialize: {e}")))
    }

    /// Parses a snapshot — syntax only. `schema` mismatches are
    /// rejected here; layout-level corruption is left for
    /// `LayoutSnapshot::restore_or_default` during apply.
    ///
    /// # Errors
    /// [`WorkspaceError::Snapshot`] on malformed JSON or wrong schema.
    pub fn from_json(json: &str) -> Result<Self, WorkspaceError> {
        let snap: Self = serde_json::from_str(json)
            .map_err(|e| WorkspaceError::Snapshot(format!("malformed snapshot: {e}")))?;
        if snap.schema != WORKSPACE_SCHEMA {
            return Err(WorkspaceError::Snapshot(format!(
                "unsupported schema {} (expected {WORKSPACE_SCHEMA})",
                snap.schema
            )));
        }
        Ok(snap)
    }
}
