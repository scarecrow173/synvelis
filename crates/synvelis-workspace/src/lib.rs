//! `synvelis-workspace` — headless Workspace foundation (Synvelis WS-A,
//! issue #36).
//!
//! Owns the multi-window/multi-view workspace model on top of
//! `ruviform-ui`'s dock/window contracts and delegates all project
//! mutations to the single `synvelis-project` `ProjectController`.
//! Everything here is non-RT and GUI-agnostic: windows are driven
//! through the [`ruviform_ui::window::WindowHandle`] port (a fake host
//! is provided for tests), so no OS window, webview, GPU or IME is
//! touched.

pub mod command;
pub mod error;
pub mod focus;
pub mod host;
pub mod link;
pub mod persist;
pub mod session;
pub mod state;
pub mod view;
pub mod workspace;

pub use command::{CommandContext, CommandKind, CommandOutcome};
pub use error::WorkspaceError;
pub use focus::{FocusState, InputContext};
pub use host::FakeWindowHost;
pub use link::{LinkEvent, LinkGroupId, LinkGroupManager};
pub use persist::{WORKSPACE_SCHEMA, WorkspaceSnapshot};
pub use session::{EngineStub, ProjectSession, TransportState};
pub use state::{
    EditingSelection, InspectionFocus, InspectionMode, LinkedContext, SemanticTarget, ViewState,
    ViewStateRegistry, Viewport,
};
pub use view::{HeadlessView, ViewDescriptor, ViewFactory, ViewRegistry, ViewType};
pub use workspace::{WindowRecord, WorkspaceManager};
