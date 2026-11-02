//! Focus model — three separate concepts (WS-A scope 2):
//!
//! - *native window focus*: which OS window the host reports focused
//!   (host events are the only source of truth).
//! - *active view*: which view is the operation target in the focused
//!   window/session.
//! - *input context*: where keystrokes currently land — a text field,
//!   IME composition, plugin GUI or modal capture DAW commands.

use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::window::WindowId;
use serde::{Deserialize, Serialize};

/// Where input currently lands. Anything other than `View` suppresses
/// project/editing/transport commands (text editing, IME composition,
/// plugin GUI interaction and modal flows own the keyboard).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InputContext {
    /// A normal DAW view — commands route normally.
    #[default]
    View,
    /// A text input field is editing.
    TextInput,
    /// An IME composition is in progress.
    Ime,
    /// A plugin GUI owns input.
    PluginGui,
    /// A modal dialog owns input.
    Modal,
}

impl InputContext {
    /// Whether this context captures DAW-routed input.
    #[must_use]
    pub fn captures_input(self) -> bool {
        !matches!(self, Self::View)
    }
}

/// The session-wide focus snapshot — three independent concepts kept
/// side by side; none is derived from another.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusState {
    /// The window the host most recently reported focused.
    pub native_window: Option<WindowId>,
    /// The operation-target view (typically the active tab of the
    /// focused window, but an independent concept).
    pub active_view: Option<ViewInstanceId>,
    /// The session's current input context.
    pub input: InputContext,
}
