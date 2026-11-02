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
