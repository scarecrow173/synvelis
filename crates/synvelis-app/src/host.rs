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
//!
//! [`HeadlessWindowHost`] is the multi-window counterpart added for
//! SV#113 (SV-GUI-04 slice1 precursor): an in-memory [`WindowHandle`]
//! that issues and tracks arbitrary [`WindowId`]s so the
//! `WorkspaceManager` dock contract and the `NativeShell` view-origin
//! rules can be verified headlessly. It does not replace
//! [`ShellWindowHost`] — the product shell stays single-window until
//! the real GPUI backend lands — it is the reference implementation
//! of the multi-window semantics that backend must honour.

use std::collections::BTreeSet;

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

/// Multi-window [`WindowHandle`] for headless SV#113 verification.
///
/// Unlike [`ShellWindowHost`] — which honestly reports
/// [`UiError::BackendUnavailable`] from `create_window` because the
/// product shell owns exactly one OS window today — this host tracks
/// an arbitrary set of window identities in memory, so the
/// `WorkspaceManager` dock contract (create / spawn / detach /
/// reattach / transfer / close / destroy) can be exercised end to end
/// without an OS window. It is the headless reference implementation
/// of the multi-window semantics the real GPUI backend must honour;
/// nothing here reaches a platform.
///
/// `WindowId`s are issued from a monotonic counter starting at `1`
/// and are never reused after destroy, per the [`WindowId`] contract.
/// Host calls are recorded in the public fields so tests can assert
/// what the workspace asked the host to do; fire-and-forget calls
/// (`eval`, `request_close`) record only live destinations, matching
/// the [`WindowHandle`] delivery-safety contract.
#[derive(Debug, Default)]
pub struct HeadlessWindowHost {
    /// Next raw id to issue — monotonic, never reused.
    next: u64,
    /// Currently living windows.
    live: BTreeSet<WindowId>,
    /// When `true`, `create_window` fails — failure-injection for
    /// rollback-path tests (`detach_to_new_window` cleanup etc.).
    pub fail_create: bool,
    /// `create_window` configs, in creation order.
    pub created: Vec<(WindowId, WindowConfig)>,
    /// `eval` calls that reached a live window, in order. Messages to
    /// unknown or destroyed windows are silently dropped per the
    /// [`WindowHandle`] delivery-safety contract, so they never appear
    /// here.
    pub evals: Vec<(WindowId, String)>,
    /// `resize` calls, in order.
    pub resizes: Vec<(WindowId, u32, u32)>,
    /// `focus` calls, in order.
    pub focuses: Vec<(WindowId, bool)>,
    /// `request_close` calls, in order.
    pub close_requests: Vec<WindowId>,
    /// `respond_close` decisions, in order.
    pub close_decisions: Vec<(WindowId, CloseDecision)>,
    /// Window removals (`destroy_window` or approved close), in order.
    pub destroys: Vec<WindowId>,
    /// Monitors reported by `monitors()`.
    pub monitors: Vec<MonitorInfo>,
    /// Whether `exit` was called.
    pub exited: bool,
}

impl HeadlessWindowHost {
    /// A host reporting `monitors` from [`WindowHandle::monitors`].
    #[must_use]
    pub fn with_monitors(monitors: Vec<MonitorInfo>) -> Self {
        Self {
            monitors,
            ..Self::default()
        }
    }
}

impl WindowHandle for HeadlessWindowHost {
    fn eval(&mut self, window: WindowId, script: &str) {
        if self.live.contains(&window) {
            self.evals.push((window, script.to_string()));
        }
    }

    fn resize(&mut self, window: WindowId, width: u32, height: u32) -> Result<(), UiError> {
        if !self.live.contains(&window) {
            return Err(UiError::host(format!("unknown window {window}")));
        }
        self.resizes.push((window, width, height));
        Ok(())
    }

    fn focus(&mut self, window: WindowId, focused: bool) -> Result<(), UiError> {
        if !self.live.contains(&window) {
            return Err(UiError::host(format!("unknown window {window}")));
        }
        self.focuses.push((window, focused));
        Ok(())
    }

    fn create_window(&mut self, config: &WindowConfig) -> Result<WindowId, UiError> {
        if self.fail_create {
            return Err(UiError::BackendUnavailable {
                message: format!("injected create failure: {}", config.title),
            });
        }
        self.next += 1;
        let id = WindowId::new(self.next)
            .ok_or_else(|| UiError::internal("window id counter wrapped"))?;
        self.live.insert(id);
        self.created.push((id, config.clone()));
        Ok(id)
    }

    fn request_close(&mut self, window: WindowId) {
        if self.live.contains(&window) {
            self.close_requests.push(window);
        }
    }

    fn respond_close(&mut self, window: WindowId, decision: CloseDecision) -> Result<(), UiError> {
        if !self.live.contains(&window) {
            return Err(UiError::host(format!("unknown window {window}")));
        }
        self.close_decisions.push((window, decision));
        if decision == CloseDecision::Approve {
            self.live.remove(&window);
            self.destroys.push(window);
        }
        Ok(())
    }

    fn destroy_window(&mut self, window: WindowId) -> Result<(), UiError> {
        if !self.live.remove(&window) {
            return Err(UiError::host(format!("unknown window {window}")));
        }
        self.destroys.push(window);
        Ok(())
    }

    fn windows(&self) -> Vec<WindowId> {
        self.live.iter().copied().collect()
    }

    fn monitors(&self) -> Vec<MonitorInfo> {
        self.monitors.clone()
    }

    fn exit(&mut self) {
        self.exited = true;
    }
}
