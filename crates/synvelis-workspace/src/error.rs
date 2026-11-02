//! Unified error type for the workspace domain.

use std::fmt;

use ruviform_ui::UiError;
use ruviform_ui::dock::{DockError, ViewInstanceId};
use ruviform_ui::window::WindowId;

/// All fallible workspace operations return this error.
#[derive(Debug)]
pub enum WorkspaceError {
    /// Dock/workspace contract failure from `ruviform-ui`.
    Dock(DockError),
    /// Window-host contract failure from `ruviform-ui`.
    Ui(UiError),
    /// Project-domain failure from `synvelis-project`.
    Project(synvelis_project::Error),
    /// The view is not registered in this workspace.
    UnknownView(ViewInstanceId),
    /// The window is not registered in this workspace.
    UnknownWindow(WindowId),
    /// The view is mid-transfer; editing it is locked.
    ViewBusy(ViewInstanceId),
    /// No command is registered under this id.
    UnknownCommand(String),
    /// The workspace snapshot could not be decoded or applied.
    Snapshot(String),
    /// One selection combined targets from different editing domains.
    MixedDomains(String),
    /// A gesture's targets span musical and media time anchors with
    /// no declared common domain — rejected, never converted.
    MixedTimeDomains(String),
    /// The resolved operation has no modelled commit semantics.
    UnsupportedOperation(&'static str),
    /// A gesture is already running on this view.
    GestureActive(ViewInstanceId),
    /// No gesture is running on this view.
    NoGesture(ViewInstanceId),
    /// The gesture state itself is inconsistent.
    InvalidGesture(String),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dock(e) => write!(f, "dock: {e}"),
            Self::Ui(e) => write!(f, "ui: {e}"),
            Self::Project(e) => write!(f, "project: {e}"),
            Self::UnknownView(v) => write!(f, "unknown view {v}"),
            Self::UnknownWindow(w) => write!(f, "unknown window {w}"),
            Self::ViewBusy(v) => write!(f, "view {v} is mid-transfer"),
            Self::UnknownCommand(id) => write!(f, "unknown command: {id}"),
            Self::Snapshot(msg) => write!(f, "snapshot: {msg}"),
            Self::MixedDomains(msg) => write!(f, "mixed selection domains: {msg}"),
            Self::MixedTimeDomains(msg) => write!(f, "mixed time domains: {msg}"),
            Self::UnsupportedOperation(op) => write!(f, "unsupported operation: {op}"),
            Self::GestureActive(v) => write!(f, "gesture already active on view {v}"),
            Self::NoGesture(v) => write!(f, "no active gesture on view {v}"),
            Self::InvalidGesture(msg) => write!(f, "invalid gesture: {msg}"),
        }
    }
}

impl std::error::Error for WorkspaceError {}

impl From<DockError> for WorkspaceError {
    fn from(e: DockError) -> Self {
        Self::Dock(e)
    }
}

impl From<UiError> for WorkspaceError {
    fn from(e: UiError) -> Self {
        Self::Ui(e)
    }
}

impl From<synvelis_project::Error> for WorkspaceError {
    fn from(e: synvelis_project::Error) -> Self {
        Self::Project(e)
    }
}
