//! Command routing (WS-A scope 4).
//!
//! Commands are classified into five kinds — `Application`,
//! `Project`, `Transport`, `View`, `Editing`. A [`CommandContext`]
//! carries *where the command came from* (window, view, input
//! context), an optional explicit target list (context-menu
//! invocation), a selection snapshot, and the expected project
//! revision.
//!
//! Resolution order for entity-targeted commands:
//! explicit targets → the origin view's own selection snapshot →
//! [`CommandOutcome::Unresolved`]. The router *never* falls back to
//! another window's or view's stale selection, and unresolved
//! destructive commands never execute.
//!
//! Input contexts that capture the keyboard (text input, IME,
//! plugin GUI, modal) suppress `Project`/`Transport`/`Editing`
//! commands before any target resolution.

use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::window::WindowId;
use synvelis_project::{ProjectOp, ProjectRevision};

use crate::error::WorkspaceError;
use crate::focus::InputContext;
use crate::state::{EditingSelection, SemanticTarget};

/// Command classification — determines the routing lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// Application-level (quit, about…): never touches the project.
    Application,
    /// Project-level mutation not bound to an entity target
    /// (rename, tempo map, …).
    Project,
    /// Transport control (play/stop/seek): engine-facing, not a
    /// project mutation.
    Transport,
    /// View-local operation (activate tab, zoom, …).
    View,
    /// Editing operation against a project entity (remove clip, …):
    /// requires a resolved target and goes through the controller.
    Editing,
}

/// The issue-time context a command travels with.
#[derive(Debug, Clone)]
pub struct CommandContext {
    /// Registered command id (for example `"clip.remove"`).
    pub id: String,
    /// The window the command originated in.
    pub origin_window: Option<WindowId>,
    /// The view the command originated in.
    pub origin_view: Option<ViewInstanceId>,
    /// Input context at issue time.
    pub input: InputContext,
    /// Explicit targets (context-menu / scripted invocation). When
    /// non-empty this wins over any selection.
    pub explicit_targets: Vec<SemanticTarget>,
    /// Optional snapshot of the origin view's selection captured at
    /// issue time. When absent the router reads the origin view's
    /// *current* selection — still never another view's.
    pub selection: Option<EditingSelection>,
    /// Expected live revision (`None` = caller accepts current).
    pub expected_revision: Option<ProjectRevision>,
    /// Free-form argument (project rename payload, …).
    pub arg: Option<String>,
}

impl CommandContext {
    /// A minimal context: `id` issued from `view` in normal input.
    #[must_use]
    pub fn from_view(id: &str, view: ViewInstanceId) -> Self {
        Self {
            id: id.to_string(),
            origin_window: None,
            origin_view: Some(view),
            input: InputContext::View,
            explicit_targets: Vec::new(),
            selection: None,
            expected_revision: None,
            arg: None,
        }
    }
}

/// The routing verdict.
#[derive(Debug)]
pub enum CommandOutcome {
    /// The command executed. `revision` is the new live revision for
    /// project-mutating commands, `None` otherwise.
    Applied {
        /// New live project revision, when the command mutated it.
        revision: Option<ProjectRevision>,
    },
    /// The input context captured the command — nothing ran.
    Suppressed {
        /// Why it was suppressed.
        reason: &'static str,
    },
    /// No legal target could be resolved — nothing ran. This is how
    /// ambiguous destructive commands are refused.
    Unresolved {
        /// Why resolution failed.
        reason: String,
    },
    /// Resolution succeeded but execution failed (validation,
    /// stale revision, busy view, unknown command, …).
    Rejected {
        /// The failure.
        error: WorkspaceError,
    },
}

/// Static metadata for one registered command.
#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    /// Routing lane.
    pub kind: CommandKind,
    /// Whether the command needs a resolved entity target.
    pub needs_target: bool,
}

/// The WS-A command vocabulary.
///
/// - `clip.remove` / `clip.mute` — editing, destructive / flag-set.
/// - `project.rename` — project mutation, no entity target.
/// - `transport.play` / `transport.stop` — transport.
/// - `view.activate` — view-local tab activation.
/// - `app.quit` — application.
#[must_use]
pub fn spec_for(id: &str) -> Option<CommandSpec> {
    let (kind, needs_target) = match id {
        "clip.remove" | "clip.mute" => (CommandKind::Editing, true),
        "project.rename" => (CommandKind::Project, false),
        "transport.play" | "transport.stop" => (CommandKind::Transport, false),
        "view.activate" => (CommandKind::View, true),
        "app.quit" => (CommandKind::Application, false),
        _ => return None,
    };
    Some(CommandSpec { kind, needs_target })
}

/// Builds the [`ProjectOp`] for an entity-targeted command. Kind
/// mismatch is reported as `None` — the caller turns it into
/// `Unresolved`, never a partial execution.
#[must_use]
pub fn build_project_op(id: &str, target: SemanticTarget) -> Option<ProjectOp> {
    match (id, target) {
        ("clip.remove", SemanticTarget::Clip(clip_id)) => Some(ProjectOp::RemoveClip { clip_id }),
        ("clip.mute", SemanticTarget::Clip(clip_id)) => Some(ProjectOp::SetClipMuted {
            clip_id,
            muted: true,
        }),
        _ => None,
    }
}
