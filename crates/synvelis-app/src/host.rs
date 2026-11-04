//! [`ShellWindowHost`] — the single-shell-window [`WindowHandle`]
//! adapter handed to `WorkspaceManager`.
//!
//! The product shell owns exactly one window per app (the host window
//! `ruviform-ui-webview` created from `HostConfig`); a second window
//! is never synthesized here — `create_window` reports
//! [`UiError::BackendUnavailable`] so a stray `WorkspaceManager` view
//! spawn fails loudly instead of lying about a window that cannot
//! exist. `eval` is a no-op record because no views are mounted in
//! SV-GUI-01 (the single shell's own event routing goes through
//! `HostHandle`, not this port); SV-GUI-02 replaces this stub with the
//! real adapter when the dock/view layer comes online.

use ruviform_ui::UiError;
use ruviform_ui::window::{CloseDecision, MonitorInfo, WindowConfig, WindowHandle, WindowId};

/// The one [`WindowId`] the shell claims.
const SHELL_WINDOW: u64 = 1;

/// Single-window [`WindowHandle`] for the product shell.
#[derive(Debug)]
pub struct ShellWindowHost {
    window: WindowId,
    evals: Vec<String>,
    close_requested: bool,
    destroyed: bool,
    exited: bool,
}

impl ShellWindowHost {
    /// A fresh shell host claiming [`SHELL_WINDOW`].
    ///
    /// # Panics
    /// If `SHELL_WINDOW` were changed to a zero `u64` (the constant is a
    /// nonzero literal, so this cannot happen in practice).
    #[must_use]
    pub fn new() -> Self {
        Self {
            // SHELL_WINDOW is a nonzero literal — infallible.
            window: WindowId::new(SHELL_WINDOW).expect("SHELL_WINDOW must be nonzero"),
            evals: Vec::new(),
            close_requested: false,
            destroyed: false,
            exited: false,
        }
    }

    /// The [`WindowId`] this host manages.
    #[must_use]
    pub const fn window(&self) -> WindowId {
        self.window
    }

    /// Scripts queued via [`WindowHandle::eval`]. No real webview sits
    /// behind this port — the scripts are recorded so tests can assert
    /// what the workspace *would* have pushed.
    #[must_use]
    pub fn eval_log(&self) -> &[String] {
        &self.evals
    }

    /// Whether [`WindowHandle::request_close`] was invoked.
    #[must_use]
    pub const fn close_requested(&self) -> bool {
        self.close_requested
    }

    /// Whether [`WindowHandle::exit`] was invoked.
    #[must_use]
    pub const fn exited(&self) -> bool {
        self.exited
    }
}

impl Default for ShellWindowHost {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowHandle for ShellWindowHost {
    fn eval(&mut self, _window: WindowId, script: &str) {
        self.evals.push(script.to_string());
    }

    fn resize(&mut self, _window: WindowId, _width: u32, _height: u32) -> Result<(), UiError> {
        Ok(())
    }

    fn focus(&mut self, _window: WindowId, _focused: bool) -> Result<(), UiError> {
        Ok(())
    }

    fn create_window(&mut self, _config: &WindowConfig) -> Result<WindowId, UiError> {
        Err(UiError::backend_unavailable(
            "synvelis shell is single-window (SV-GUI-01); multi-window \
             support arrives with the SV-GUI-02 workspace wiring",
        ))
    }

    fn request_close(&mut self, _window: WindowId) {
        self.close_requested = true;
    }

    fn respond_close(
        &mut self,
        _window: WindowId,
        _decision: CloseDecision,
    ) -> Result<(), UiError> {
        Ok(())
    }

    fn destroy_window(&mut self, window: WindowId) -> Result<(), UiError> {
        if window == self.window {
            self.destroyed = true;
            Ok(())
        } else {
            Err(UiError::host(format!("unknown window: {window:?}")))
        }
    }

    fn windows(&self) -> Vec<WindowId> {
        if self.destroyed {
            Vec::new()
        } else {
            vec![self.window]
        }
    }

    fn monitors(&self) -> Vec<MonitorInfo> {
        // The host window owns monitor awareness; this port only
        // exposes what the workspace needs to name (none in SV-GUI-01).
        Vec::new()
    }

    fn exit(&mut self) {
        self.exited = true;
    }
}
