//! `FakeWindowHost` — a deterministic in-memory [`WindowHandle`] for
//! headless tests. It issues sequential [`WindowId`]s, records every
//! host call, and supports failure injection (`fail_create`) so move
//! rollback paths are exercised without any OS window.

use std::collections::BTreeSet;

use ruviform_ui::UiError;
use ruviform_ui::window::{CloseDecision, MonitorInfo, WindowConfig, WindowHandle, WindowId};

/// Deterministic fake multi-window host.
#[derive(Debug, Default)]
pub struct FakeWindowHost {
    next: u64,
    live: BTreeSet<WindowId>,
    /// When set, `create_window` fails with this error once cleared or
    /// while `true` — set to `true` for permanent failure injection.
    pub fail_create: bool,
    /// `eval` calls, in order.
    pub evals: Vec<(WindowId, String)>,
    /// `resize` calls, in order.
    pub resizes: Vec<(WindowId, u32, u32)>,
    /// `focus` calls, in order.
    pub focuses: Vec<(WindowId, bool)>,
    /// `request_close` calls, in order.
    pub close_requests: Vec<WindowId>,
    /// `respond_close` decisions, in order.
    pub close_decisions: Vec<(WindowId, CloseDecision)>,
    /// `destroy_window` calls, in order.
    pub destroys: Vec<WindowId>,
    /// Monitors reported by `monitors()`.
    pub monitors: Vec<MonitorInfo>,
    /// Whether `exit` was called.
    pub exited: bool,
}

impl FakeWindowHost {
    /// A host with `monitor` reported attached (empty by default).
    #[must_use]
    pub fn with_monitors(monitors: Vec<MonitorInfo>) -> Self {
        Self {
            monitors,
            ..Self::default()
        }
    }
}

impl WindowHandle for FakeWindowHost {
    fn eval(&mut self, window: WindowId, script: &str) {
        self.evals.push((window, script.to_string()));
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
