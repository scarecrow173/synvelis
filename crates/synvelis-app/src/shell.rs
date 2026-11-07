//! [`NativeShell`] — the driver-neutral product shell (SV-GPUI-01).
//!
//! The shell is the composition seam between the [`WindowDriver`] port
//! (any backend: GPUI via `ruviform-ui-gpui`, `FakeDriver` in tests)
//! and the sealed app: it owns the [`UiRuntime`] + [`AppService`]
//! pair that carries the single `WorkspaceManager → ProjectSession →
//! ProjectController` source of truth, plus the pre-registration
//! handles the host needs — the shared quit flag, the notify outbox,
//! the UI clock cell, and the boot app-state push.
//!
//! Routing contract (RF386 — mirrors `ruviform-ui::driver` module
//! docs):
//!
//! - `Windowed { Action(intent) }` → [`UiRuntime::dispatch_window_intent`]
//!   and every returned [`RoutedEvent`] is delivered back through
//!   [`WindowDriver::post_event`] — `Some(window)` to that window,
//!   `None` (unrouted) broadcast to every window the shell tracks.
//! - `Tick` → [`UiRuntime::drain_jobs_routed`] + UI-clock advance +
//!   outbox drain + quit-flag check.
//! - `Windowed { CloseRequested }` → a `lifecycle(Closing)` event to
//!   that window, then the close policy answers [`WindowDriver::
//!   respond_close`]. The initial slice's policy is `Approve`
//!   (unsaved-guard veto is the designed follow-up hook).
//! - `Windowed { Ready }` → the boot app-state notify is delivered
//!   (once per window — `post_event` also buffers pre-ready traffic).
//! - `Windowed { Destroyed }` → [`UiRuntime::cancel_window_jobs`] +
//!   `syn.view.close` replayed for every view the shell recorded on
//!   that window (the same teardown contract a view would run — never
//!   a parallel cleanup path) + `exit()` when the last window dies.
//! - `syn.app.quit` sets the shared quit flag from inside the sealed
//!   service; the shell observes it and `request_close`s every live
//!   window (each passes the vetoable close flow — quit is never a
//!   silent destroy).
//!
//! The shell also tracks `syn.view.{attach,detach,close}` intents it
//! forwards and reconciles `view → window` on their successful
//! results, so `Destroyed` teardown targets exactly the views that
//! lived on the dead window.
//!
//! **Dependency direction:** GPUI/OS/DOM types never appear here —
//! only the `ruviform-ui` port types. **RT contract:** non-RT only —
//! driven on the UI thread by the driver's `run`/`pump` loop.

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::json;

use ruviform_ui::driver::{DriverEvent, WindowDriver, WindowLifecycle, WindowSpec};
use ruviform_ui::ipc::{ErrorBody, Event, LifecycleState, codes};
use ruviform_ui::window::{CloseDecision, RoutedEvent, WindowId};
use ruviform_ui::{DataRegistry, UiError, UiRuntime};

use crate::app::SynvelisApp;
use crate::binding::{
    COMMAND_INTENT_OP, decode_view_command, notify_to_view_event, view_command_dispatch,
};
use crate::service::AppService;
use crate::ui_ops::UiOutbox;

/// A `syn.view.*` intent the shell is watching so it can reconcile
/// `view → window` when the result lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewOp {
    /// `syn.view.attach` — on success the view lives on the window.
    Attach,
    /// `syn.view.detach` — the window association is kept (the app's
    /// `ui.views` registry keeps it too; a re-attach elsewhere moves
    /// it, a window `Destroyed` still tears it down).
    Detach,
    /// `syn.view.close` — on success the view is gone.
    Close,
}

/// The driver-neutral product shell — one instance per app process.
pub struct NativeShell {
    rt: UiRuntime,
    /// Shared quit flag — `syn.app.quit` sets it from inside the
    /// sealed service; the shell drains it into `request_close`s.
    quit: Arc<AtomicBool>,
    /// Shared notify outbox — UI ops pump receipts/projections/frames
    /// into it; the shell broadcasts them to live windows.
    outbox: UiOutbox,
    /// Shared UI clock cell — advanced per `Tick`; `ui_pump` re-syncs
    /// the gateway clock from it inside every op.
    clock: Rc<Cell<u64>>,
    /// Boot app-state push — delivered to each window on `Ready`.
    initial_notify: Event,
    /// `Tick` cadence in ms — the clock advance per tick.
    tick_ms: u64,
    /// Tracked windows → ready flag. `Created` inserts, `Ready` marks,
    /// `Destroyed` removes.
    windows: BTreeMap<WindowId, bool>,
    /// `view_id → origin driver window` — reconciled from successful
    /// `syn.view.*` results so `Destroyed` teardown is precise.
    views: HashMap<String, WindowId>,
    /// `view_id → generation` adopted from `syn.view.attach` results —
    /// the authoritative epoch `DispatchContext.generation` must carry
    /// for view-translated commands.
    view_generation: HashMap<String, u32>,
    /// `intent.id → (view op, view_id, origin window)` in flight.
    pending_view_ops: HashMap<String, (ViewOp, String, WindowId)>,
    /// Shell-minted intent ids for teardown replays.
    teardown_seq: u64,
}

impl NativeShell {
    /// Composes the shell: captures the host-side handles from `app`,
    /// then seals the app into `AppService` on a fresh [`UiRuntime`].
    ///
    /// # Errors
    /// [`UiError`] if service registration fails (a malformed or
    /// duplicate `syn.*` op — a code defect, not a runtime condition).
    pub fn new(mut app: SynvelisApp, tick: Duration) -> Result<Self, UiError> {
        let quit = app.quit_flag();
        let outbox = app.event_outbox();
        let clock = app.ui_clock();
        let initial_notify = app.app_state_notify();
        let mut rt = UiRuntime::new(DataRegistry::new());
        rt.register_service(AppService::new(app))?;
        Ok(Self {
            rt,
            quit,
            outbox,
            clock,
            initial_notify,
            tick_ms: u64::try_from(tick.as_millis()).unwrap_or(u64::MAX),
            windows: BTreeMap::new(),
            views: HashMap::new(),
            view_generation: HashMap::new(),
            pending_view_ops: HashMap::new(),
            teardown_seq: 0,
        })
    }

    /// Creates the product's main window through the driver. Call
    /// before the driver's `run`/`pump` loop starts draining events —
    /// the `Created`/`Ready` lifecycle arrives through `handle`.
    ///
    /// # Errors
    /// [`UiError`] from `create_window` (backend unavailable,
    /// unsupported, host failure).
    pub fn open_main(&mut self, driver: &mut dyn WindowDriver) -> Result<WindowId, UiError> {
        let spec = WindowSpec {
            title: "Synvelis".to_string(),
            ..WindowSpec::default()
        };
        driver.create_window(&spec)
    }

    /// The view→window association learned from successful
    /// `syn.view.attach` results — exposed for verification.
    #[must_use]
    pub fn view_window(&self, view: &str) -> Option<WindowId> {
        self.views.get(view).copied()
    }

    /// Whether `window` reached `Ready` — exposed for verification.
    #[must_use]
    pub fn is_ready(&self, window: WindowId) -> bool {
        self.windows.get(&window).copied().unwrap_or(false)
    }

    /// Consumes one [`DriverEvent`] — the body the driver loop calls
    /// (`GpuiDriver::run`'s `on_event`, `FakeDriver::pump`'s closure).
    pub fn handle(&mut self, event: DriverEvent, driver: &mut dyn WindowDriver) {
        match event {
            DriverEvent::Tick => self.on_tick(driver),
            DriverEvent::Windowed { window, event } => self.on_windowed(window, event, driver),
            // `DriverEvent` is non-exhaustive — a variant added by a
            // newer port carries no semantics this shell knows.
            _ => {}
        }
        // Every handled event can push into the outbox (op results do,
        // teardown replays do) — drain it into the windows before the
        // quit check so a `syn.app.quit`'s trailing notifies still land.
        self.drain_outbox(driver);
        if self.quit.swap(false, Ordering::AcqRel) {
            for window in driver.windows() {
                driver.request_close(window);
            }
        }
    }

    /// `Tick`: job drain, UI-clock advance. The outbox drain and quit
    /// check happen in [`handle`] for every event, ticks included.
    fn on_tick(&mut self, driver: &mut dyn WindowDriver) {
        for re in self.rt.drain_jobs_routed() {
            self.post_routed(&re, driver);
        }
        self.clock
            .set(self.clock.get().saturating_add(self.tick_ms));
    }

    fn on_windowed(
        &mut self,
        window: WindowId,
        event: WindowLifecycle,
        driver: &mut dyn WindowDriver,
    ) {
        match event {
            WindowLifecycle::Created => {
                self.windows.entry(window).or_insert(false);
            }
            WindowLifecycle::Ready => {
                // First `Ready` only — the boot app-state push is
                // once per window.
                let ready = self.windows.entry(window).or_insert(false);
                if !*ready {
                    *ready = true;
                    driver.post_event(window, &self.initial_notify.clone());
                }
            }
            WindowLifecycle::Action(intent) => {
                if intent.op == COMMAND_INTENT_OP {
                    self.on_view_command(window, intent, driver);
                } else {
                    self.track_view_op(window, &intent);
                    for re in self.rt.dispatch_window_intent(window, intent) {
                        self.observe_result(&re);
                        self.post_routed(&re, driver);
                    }
                }
            }
            WindowLifecycle::CloseRequested => {
                // Mirror `UiRuntime::handle_window`'s closing push,
                // then apply the close policy — Approve for the
                // initial slice (unsaved-guard veto is the designed
                // follow-up hook in the SV175 chain).
                driver.post_event(
                    window,
                    &Event::Lifecycle {
                        state: LifecycleState::Closing,
                    },
                );
                if let Err(e) = driver.respond_close(window, CloseDecision::Approve) {
                    tracing::debug!(window = %window, error = %e, "respond_close failed");
                }
            }
            WindowLifecycle::Destroyed => self.on_destroyed(window, driver),
            // Resized / Moved / Focused / ScaleFactorChanged carry no
            // runtime semantics — consumer UI state (RF386 contract).
            _ => {}
        }
    }

    /// Handles a view-minted `rvf.ui.command.v1` intent: strictly
    /// decodes it into the typed command vocabulary, resolves the view
    /// (`origin_view`, else the window's bound view), fills
    /// `DispatchContext` with the attach-adopted generation + clock,
    /// and dispatches the `syn.control.dispatch` through the runtime.
    /// The ok-ack is not delivered to the view — the authoritative
    /// receipt arrives via the `syn.control.receipt` → `Result`
    /// translation; a failed ack is forwarded as the rejected receipt.
    fn on_view_command(
        &mut self,
        window: WindowId,
        intent: ruviform_ui::ipc::Intent,
        driver: &mut dyn WindowDriver,
    ) {
        let cmd = match decode_view_command(&intent.params) {
            Ok(cmd) => cmd,
            Err(e) => {
                driver.post_event(
                    window,
                    &Event::result_err(
                        intent.id,
                        ErrorBody::new(codes::INVALID_PARAMS, e.to_string()),
                    ),
                );
                return;
            }
        };
        let view = cmd.origin_view.clone().or_else(|| {
            self.views
                .iter()
                .find(|(_, w)| **w == window)
                .map(|(v, _)| v.clone())
        });
        let Some(view) = view else {
            driver.post_event(
                window,
                &Event::result_err(
                    intent.id,
                    ErrorBody::new(
                        codes::INVALID_PARAMS,
                        "view command carries no origin_view and window has no bound view",
                    ),
                ),
            );
            return;
        };
        let generation = self.view_generation.get(&view).copied().unwrap_or(0);
        let dispatch = view_command_dispatch(
            &cmd,
            &view,
            &window.to_string(),
            generation,
            self.clock.get(),
        );
        for re in self.rt.dispatch_window_intent(window, dispatch) {
            self.observe_result(&re);
            if matches!(&re.event, Event::Result { ok: true, .. }) {
                continue;
            }
            self.post_routed(&re, driver);
        }
    }

    /// Records `syn.view.*` intents so [`Self::observe_result`] can
    /// reconcile the view→window map on success.
    fn track_view_op(&mut self, window: WindowId, intent: &ruviform_ui::ipc::Intent) {
        let op = match intent.op.as_str() {
            "syn.view.attach" => ViewOp::Attach,
            "syn.view.detach" => ViewOp::Detach,
            "syn.view.close" => ViewOp::Close,
            _ => return,
        };
        let Some(view) = intent.params["view_id"].as_str() else {
            return;
        };
        self.pending_view_ops
            .insert(intent.id.clone(), (op, view.to_string(), window));
    }

    /// Applies a dispatch result to the `views` map before the event
    /// is forwarded — teardown bookkeeping only, never intercepting
    /// the result itself.
    fn observe_result(&mut self, re: &RoutedEvent) {
        let Event::Result { id, ok, data, .. } = &re.event else {
            return;
        };
        // Consume the tracking entry on *any* result — a failed op
        // must not leak it (and a later colliding id must not
        // reconcile against a stale entry).
        let Some((op, view, window)) = self.pending_view_ops.remove(id) else {
            return;
        };
        if !*ok {
            return;
        }
        match op {
            ViewOp::Attach => {
                self.views.insert(view.clone(), window);
                let generation = data
                    .as_ref()
                    .and_then(|d| d["generation"].as_u64())
                    .map(|g| u32::try_from(g.min(u64::from(u32::MAX))).unwrap_or(u32::MAX));
                if let Some(g) = generation {
                    self.view_generation.insert(view, g);
                }
            }
            ViewOp::Detach => {}
            ViewOp::Close => {
                self.views.remove(&view);
                self.view_generation.remove(&view);
            }
        }
    }

    /// `Destroyed`: cancel the window's jobs, replay `syn.view.close`
    /// for every view that lived on it (contractual teardown —
    /// gesture cancel, generation bump, subscription release through
    /// the single `syn.*` contract), drop bookkeeping, and exit the
    /// loop when no window remains (app-exit is consumer policy).
    fn on_destroyed(&mut self, window: WindowId, driver: &mut dyn WindowDriver) {
        let cancelled = self.rt.cancel_window_jobs(window);
        if cancelled > 0 {
            tracing::info!(window = %window, cancelled, "cancelled window jobs");
        }
        self.windows.remove(&window);
        let orphans: Vec<String> = self
            .views
            .iter()
            .filter(|(_, w)| **w == window)
            .map(|(v, _)| v.clone())
            .collect();
        for view in orphans {
            self.teardown_seq += 1;
            let id = format!("shell-teardown-{}", self.teardown_seq);
            for event in self
                .rt
                .dispatch_op(id, "syn.view.close", json!({ "view_id": view }))
            {
                // Teardown acks are unrouted — broadcast to whatever
                // windows remain (a `syn.view.close` result also
                // pumps the outbox, drained by `handle` next).
                self.post_unrouted(&event, driver);
            }
            self.views.remove(&view);
            self.view_generation.remove(&view);
        }
        if driver.windows().is_empty() {
            driver.exit();
        }
    }

    /// Delivers a routed event: `Some(window)` to that window, `None`
    /// broadcast to every tracked window (the consumer's unrouted
    /// sink policy — `post_event` buffers not-yet-ready windows).
    fn post_routed(&mut self, re: &RoutedEvent, driver: &mut dyn WindowDriver) {
        match re.window {
            Some(window) => driver.post_event(window, &re.event),
            None => self.post_unrouted(&re.event, driver),
        }
    }

    /// Broadcasts one event to every tracked window.
    fn post_unrouted(&mut self, event: &Event, driver: &mut dyn WindowDriver) {
        for window in self.windows.keys() {
            driver.post_event(*window, event);
        }
    }

    /// Drains the shared notify outbox — receipts, projections, viz
    /// frames, view state — broadcast to every window; each view's
    /// `ViewBinding` demuxes by `view_id`.
    fn drain_outbox(&mut self, driver: &mut dyn WindowDriver) {
        let mut pending = Vec::new();
        {
            let mut queue = self.outbox.borrow_mut();
            while let Some(event) = queue.pop_front() {
                pending.push(event);
            }
        }
        for event in &pending {
            // Mounted views (RF387 `ViewHost`) consume `Result`/
            // `Notify` correlated by `command_id`/`control_id`, not
            // `syn.control.*` notifies — translate those channels
            // alongside the broadcast, to the view's window when the
            // association is known.
            if let Some((view, translated)) = notify_to_view_event(event) {
                match self.views.get(&view) {
                    Some(w) => driver.post_event(*w, &translated),
                    None => self.post_unrouted(&translated, driver),
                }
            }
            self.post_unrouted(event, driver);
        }
    }
}
