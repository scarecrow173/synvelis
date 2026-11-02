//! `synvelis-workspace` — headless Workspace foundation (Synvelis WS-A,
//! issue #36) plus the agreed editing model (ED-A, issues #11/#38).
//!
//! Owns the multi-window/multi-view workspace model on top of
//! `ruviform-ui`'s dock/window contracts and delegates all project
//! mutations to the single `synvelis-project` `ProjectController`.
//! The editing model — per-domain object selection, time selection,
//! lane scope, tool resolution, gesture fixation/preview, snap and
//! linked-context separation — lives in the `selection`, `tool`,
//! `snap`, `gesture` and `edit` modules.
//!
//! Everything here is non-RT and GUI-agnostic: windows are driven
//! through the [`ruviform_ui::window::WindowHandle`] port (a fake host
//! is provided for tests), so no OS window, webview, GPU or IME is
//! touched.

pub mod command;
pub mod control;
pub mod edit;
pub mod error;
pub mod focus;
pub mod gesture;
pub mod host;
pub mod link;
pub mod persist;
pub mod selection;
pub mod session;
pub mod snap;
pub mod state;
pub mod tool;
pub mod view;
pub mod workspace;

pub use command::{CommandContext, CommandKind, CommandOutcome};
pub use control::{
    AssignmentView, AutomationMode, ControlGesture, ControlObservation, ControlSession, GestureEnd,
    InspectorBreakdown, Lowered, ObservationData, ObservationState, PlaybackState, PlaybackStatus,
    RecordingOutcome, RecordingTransaction, SealReason, TransportEffect, WriteArm,
};
pub use edit::CommitEffect;
pub use error::WorkspaceError;
pub use focus::{FocusState, InputContext};
pub use gesture::{CancelReason, Gesture, GestureFixation, GestureOutcome, GesturePreview};
pub use host::FakeWindowHost;
pub use link::{LinkEvent, LinkGroupId, LinkGroupManager};
pub use persist::{WORKSPACE_SCHEMA, WorkspaceSnapshot};
pub use selection::{
    EditBinding, EditTimeDomain, LaneScope, ObjectSelections, SelectionDomain, TimeSelection,
};
pub use session::{EngineStub, ProjectSession, TransportState};
pub use snap::{
    GridConfig, SnapCandidate, SnapConfig, SnapEnvironment, SnapSource, SnapUnit, SnapWinner,
    resolve_snap,
};
pub use state::{
    EditingSelection, InspectionFocus, InspectionMode, LinkedContext, SemanticTarget, ViewState,
    ViewStateRegistry, Viewport,
};
pub use tool::{
    EditOperation, HandleKind, HitCandidate, HitProbe, HitZone, Tool, ToolResolution, resolve_tool,
};
pub use view::{HeadlessView, ViewDescriptor, ViewFactory, ViewRegistry, ViewType};
pub use workspace::{PointerOutcome, WindowRecord, WorkspaceManager};
