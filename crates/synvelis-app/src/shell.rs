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
//!   respond_close`]. Only the last live window is guarded: the shell
//!   queries `syn.window.close_guard` synchronously and vetoes when
//!   the app reports unsaved changes or an active recording — an
//!   op error, missing result, or malformed payload fails closed to
//!   `Veto`. Earlier windows approve unconditionally because the
//!   shared session survives in the remaining windows.
//! - `Windowed { Ready }` → the boot app-state notify is delivered
//!   (once per window — `post_event` also buffers pre-ready traffic).
//! - `Windowed { Created | Ready | Resized | Moved | Focused |
//!   ScaleFactorChanged }` → forwarded to the workspace through the
//!   internal `syn.window.lifecycle` op so `WorkspaceManager` — the
//!   dock/window authority — sees every host-created window
//!   (`adopt_window` on first sight, never `create_window`). `Ready`
//!   additionally delivers the boot app-state notify once per window.
//! - `Windowed { Destroyed }` → [`UiRuntime::cancel_window_jobs`] +
//!   `syn.window.lifecycle{destroyed}` so the workspace applies the
//!   window's `WindowCloseDisposition` (`DetachViews` pools docked
//!   views — UI detach, never `syn.view.close`; `ReleaseViews`
//!   removes them) + `syn.view.close` replayed only for legacy
//!   (non-dock) views the shell recorded on that window + `exit()`
//!   when the last window dies.
//! - `syn.window.dock_prepare_new_window` pushes a [`WindowRequest`]
//!   the shell drains into `driver.create_window`; the new window's
//!   `Ready` attaches the pending view (the one `syn.view.attach`
//!   the app permits mid-saga) and commits the move — attach or
//!   commit failure aborts the move and destroys the orphan window.
//! - `syn.app.quit` sets the shared quit flag from inside the sealed
//!   service; the shell observes it and `request_close`s every live
//!   window (each passes the vetoable close flow — quit is never a
//!   silent destroy).
//!
//! The shell also tracks `syn.view.{attach,detach,close}` and
//! `syn.window.dock_{commit,reattach,detach,abort}` intents it
//! forwards and reconciles `view → window` on their successful
//! results, so `Destroyed` teardown targets exactly the views that
//! lived on the dead window.
//!
//! **Dependency direction:** GPUI/OS/DOM types never appear here —
//! only the `ruviform-ui` port types. **RT contract:** non-RT only —
//! driven on the UI thread by the driver's `run`/`pump` loop.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Value, json};

use ruviform_ui::driver::{DriverEvent, WindowDriver, WindowLifecycle, WindowSpec};
use ruviform_ui::ipc::{ErrorBody, Event, LifecycleState, codes};
use ruviform_ui::view::{AttachEpoch, AttachEpochAllocator, TAB_SELECT_ACTION};
use ruviform_ui::window::{CloseDecision, RoutedEvent, WindowId};
use ruviform_ui::{DataRegistry, UiError, UiRuntime};

use crate::app::SynvelisApp;
use crate::binding::{
    COMMAND_INTENT_OP, PROJECT_TARGET, VIEW_STATE_TARGET, decode_view_command,
    notify_to_view_event, project_action_op, view_command_dispatch,
};
use crate::screens::dock_view_info;
use crate::service::AppService;
use crate::ui_ops::{UiOutbox, WindowRequest};
use synvelis_workspace::controls_ui::CommandIntent;

/// `"w{n}"` → [`WindowId`]; anything else → `None`.
fn parse_window_id_str(s: &str) -> Option<WindowId> {
    WindowId::new(s.strip_prefix('w')?.parse().ok()?)
}

/// A `syn.view.*` intent the shell is watching so it can reconcile
/// `view → window` when the result lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewOp {
    /// `syn.view.attach` — on success the view lives on the window.
    Attach,
    /// `syn.view.detach` — on success a dock-domain view loses its
    /// window (pooled); a legacy view keeps the association (the
    /// app's `ui.views` registry keeps it too; a re-attach elsewhere
    /// moves it, a window `Destroyed` still tears it down).
    Detach,
    /// `syn.view.close` — on success the view is gone.
    Close,
    /// `syn.window.dock_commit` / `syn.window.dock_reattach` — on
    /// success `data.to` names the view's new owner window.
    DockCommit,
    /// `syn.window.dock_detach` — on success the view is pooled.
    DockDetach,
    /// `syn.window.dock_abort` — on success `data.owner` names the
    /// restored owner (`null` = detached pool).
    DockAbort,
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
    /// Shared window-request queue — `syn.window.dock_prepare_new_
    /// window` pushes here; the shell drains it into
    /// `driver.create_window` (the app→shell direction of the
    /// new-window dock saga).
    window_requests: Rc<RefCell<VecDeque<WindowRequest>>>,
    /// Session `AttachEpoch` allocator — shared cell captured before
    /// the app sealed, so mid-session host recreation still mints
    /// from the single allocator (RF387).
    attach_epochs: Rc<RefCell<AttachEpochAllocator>>,
    /// Boot app-state push — delivered to each window on `Ready`.
    initial_notify: Event,
    /// `Tick` cadence in ms — the clock advance per tick.
    tick_ms: u64,
    /// Tracked windows → ready flag. `Created` inserts, `Ready` marks,
    /// `Destroyed` removes.
    windows: BTreeMap<WindowId, bool>,
    /// `view_id → owner driver window` — reconciled from successful
    /// `syn.view.*` results so `Destroyed` teardown and view-command
    /// origin resolution are precise. The owner is the window the
    /// `attach` declares (`window_id` — the window the domain
    /// actually attaches the view to), never the channel the intent
    /// happened to arrive on.
    views: HashMap<String, WindowId>,
    /// `view_id → generation` adopted from `syn.view.attach` results —
    /// the authoritative epoch `DispatchContext.generation` must carry
    /// for view-translated commands.
    view_generation: HashMap<String, u32>,
    /// `intent.id → (view op, view_id, declared owner window)` in
    /// flight — `Some` only for `attach` when the declared
    /// `window_id` names a live tracked window.
    pending_view_ops: HashMap<String, (ViewOp, String, Option<WindowId>)>,
    /// Window title per tracked window — forwarded on `Created` so
    /// the workspace record carries the spec the shell created the
    /// window with (driver `Created` itself carries no payload).
    window_titles: HashMap<WindowId, String>,
    /// New-window saga destinations the shell created but that have
    /// not committed yet: `window → view_id`. On the window's `Ready`
    /// the shell attaches the view and commits the dock move; on
    /// `Destroyed` (or attach/commit failure) it aborts the move and
    /// destroys the orphan.
    saga_windows: HashMap<WindowId, String>,
    /// Shell-minted intent ids for teardown/lifecycle replays.
    teardown_seq: u64,
}

impl NativeShell {
    /// Composes the shell: captures the host-side handles from `app`,
    /// then seals the app into `AppService` on a fresh [`UiRuntime`].
    ///
    /// # Errors
    /// [`UiError`] if service registration fails (a malformed or
    /// duplicate `syn.*` op — a code defect, not a runtime condition).
    pub fn new(app: SynvelisApp, tick: Duration) -> Result<Self, UiError> {
        Self::new_with_data(app, tick, DataRegistry::new())
    }

    /// [`new`](Self::new) with a caller-supplied [`DataRegistry`] — the
    /// `WebView` host shares its registry with the `rvf://data`
    /// scheme handler, so the page's bounded pulls resolve against the
    /// same store the runtime's services write to.
    ///
    /// # Errors
    /// [`UiError`] if service registration fails (a malformed or
    /// duplicate `syn.*` op — a code defect, not a runtime condition).
    pub fn new_with_data(
        mut app: SynvelisApp,
        tick: Duration,
        data: DataRegistry,
    ) -> Result<Self, UiError> {
        let quit = app.quit_flag();
        let outbox = app.event_outbox();
        let clock = app.ui_clock();
        let window_requests = app.window_requests();
        let attach_epochs = app.attach_epoch_allocator();
        let initial_notify = app.app_state_notify();
        let mut rt = UiRuntime::new(data);
        rt.register_service(AppService::new(app))?;
        Ok(Self {
            rt,
            quit,
            outbox,
            clock,
            window_requests,
            attach_epochs,
            initial_notify,
            tick_ms: u64::try_from(tick.as_millis()).unwrap_or(u64::MAX),
            windows: BTreeMap::new(),
            views: HashMap::new(),
            view_generation: HashMap::new(),
            pending_view_ops: HashMap::new(),
            window_titles: HashMap::new(),
            saga_windows: HashMap::new(),
            teardown_seq: 0,
        })
    }

    /// The session `AttachEpoch` allocator — mints every `ViewHost`'s
    /// epoch from the one session-owned sequence. Reachable after the
    /// app sealed, so a view remount/host recreation under a live
    /// shell still takes the mandatory allocator path.
    #[must_use]
    pub fn attach_epoch_allocator(&self) -> Rc<RefCell<AttachEpochAllocator>> {
        Rc::clone(&self.attach_epochs)
    }

    /// Mints the next attach epoch — convenience over
    /// [`Self::attach_epoch_allocator`].
    #[must_use]
    pub fn mint_attach_epoch(&self) -> AttachEpoch {
        self.attach_epochs.borrow_mut().mint()
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
        let window = driver.create_window(&spec)?;
        self.window_titles.insert(window, spec.title);
        Ok(window)
    }

    /// Attaches a mounted view (`ViewHost` already given to the
    /// driver) to `window` and subscribes its binding targets — the
    /// composition-root half of the RF387 mount seam.
    ///
    /// The window itself is created by the driver first
    /// (`GpuiDriver::create_view_window` on the GPUI lane; any
    /// `create_window` on a fake), because the host lives inside the
    /// driver's render object — the shell never holds `ViewHost`.
    /// This method then mints the `syn.view.attach` +
    /// `syn.control.subscribe` ops through the same runtime dispatch
    /// an `Action`-channel intent would take, so shell bookkeeping
    /// (`views`, `view_generation`) reconciles on the exact same path
    /// and `Destroyed` teardown covers the view uniformly.
    ///
    /// `view_id` is the wire identity (`"view-{ViewInstanceId}"` —
    /// [`crate::screens::main_view_id`]); the `AttachEpoch` used to
    /// build the host is a separate runtime guard minted by
    /// [`SynvelisApp::mint_attach_epoch`].
    ///
    /// Returns the `syn.view.attach` result payload (`generation`
    /// included) so callers/tests can assert the adopted epoch.
    ///
    /// # Errors
    /// [`UiError::Internal`] when the attach op reports a failure —
    /// a mounted view that cannot attach is a composition defect, so
    /// the error is propagated rather than leaving the host silently
    /// bound to nothing.
    pub fn attach_view(
        &mut self,
        driver: &mut dyn WindowDriver,
        window: WindowId,
        view_id: &str,
        targets: &[String],
    ) -> Result<Value, UiError> {
        let intent = ruviform_ui::ipc::Intent::new(
            format!("shell-attach-{view_id}"),
            "syn.view.attach",
            json!({ "view_id": view_id, "window_id": window.to_string() }),
        );
        self.track_view_op(&intent);
        let mut attach_data = Value::Null;
        let mut failure: Option<String> = None;
        for re in self.rt.dispatch_window_intent(window, intent) {
            if let Event::Result {
                ok, data, error, ..
            } = &re.event
            {
                if *ok {
                    attach_data = data.clone().unwrap_or(Value::Null);
                } else {
                    failure = Some(
                        error
                            .as_ref()
                            .map_or_else(|| "attach rejected".to_string(), |e| e.message.clone()),
                    );
                }
            }
            self.observe_result(&re);
            self.post_routed(&re, driver);
        }
        if let Some(message) = failure {
            return Err(UiError::Internal {
                message: format!("syn.view.attach for {view_id} failed: {message}"),
            });
        }
        for target in targets {
            let sub = ruviform_ui::ipc::Intent::new(
                format!("shell-sub-{view_id}-{target}"),
                "syn.control.subscribe",
                json!({ "view_id": view_id, "target": target }),
            );
            for re in self.rt.dispatch_window_intent(window, sub) {
                if let Event::Result {
                    ok: false, error, ..
                } = &re.event
                {
                    tracing::warn!(
                        view_id,
                        target,
                        error = ?error,
                        "view subscription refused"
                    );
                }
                self.observe_result(&re);
                self.post_routed(&re, driver);
            }
        }
        // The dispatched ops pumped receipts/initial projections into
        // the outbox — drain it into the windows exactly as `handle`
        // does after every event, so the subscription's first
        // `syn.control.projection` row lands without waiting for the
        // next unrelated event.
        self.drain_outbox(driver);
        Ok(attach_data)
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
        self.drain_window_requests(driver);
        if self.quit.swap(false, Ordering::AcqRel) {
            for window in driver.windows() {
                driver.request_close(window);
            }
        }
    }

    /// Drains app→shell [`WindowRequest`]s (pushed by
    /// `syn.window.dock_prepare_new_window`) into
    /// `driver.create_window`. A creation failure aborts the pending
    /// move app-side (`dock_abort{failed}`) so the view keeps its
    /// source ownership; a success registers the window as a saga
    /// destination — its `Ready` attaches + commits, its
    /// `Destroyed` aborts.
    fn drain_window_requests(&mut self, driver: &mut dyn WindowDriver) {
        let mut pending = Vec::new();
        {
            let mut queue = self.window_requests.borrow_mut();
            while let Some(request) = queue.pop_front() {
                pending.push(request);
            }
        }
        for request in pending {
            match driver.create_window(&request.spec) {
                Ok(window) => {
                    self.window_titles
                        .insert(window, request.spec.title.clone());
                    self.saga_windows.insert(window, request.view_id.clone());
                    tracing::info!(
                        window = %window,
                        view_id = %request.view_id,
                        "dock new-window request → window created"
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        view_id = %request.view_id,
                        error = %e,
                        "dock new-window creation failed — aborting move"
                    );
                    self.dispatch_dock_abort(
                        &request.view_id,
                        &format!("window creation failed: {e}"),
                        driver,
                    );
                }
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
                self.forward_lifecycle(
                    "created",
                    json!({
                        "window_id": window.to_string(),
                        "title": self.window_titles.get(&window),
                        "dock_view": self.saga_windows.get(&window),
                    }),
                    driver,
                );
            }
            WindowLifecycle::Ready => {
                // First `Ready` only — the boot app-state push is
                // once per window.
                let ready = self.windows.entry(window).or_insert(false);
                let first_ready = !*ready;
                *ready = true;
                // Adopt-if-unknown: fixtures that inject `Ready`
                // without `Created` still register the window.
                self.forward_lifecycle(
                    "ready",
                    json!({
                        "window_id": window.to_string(),
                        "title": self.window_titles.get(&window),
                        "dock_view": self.saga_windows.get(&window),
                    }),
                    driver,
                );
                if first_ready {
                    driver.post_event(window, &self.initial_notify.clone());
                }
                self.drive_saga_ready(window, driver);
            }
            WindowLifecycle::Resized { width, height } => {
                self.forward_lifecycle(
                    "resized",
                    json!({
                        "window_id": window.to_string(),
                        "width": width,
                        "height": height,
                    }),
                    driver,
                );
            }
            WindowLifecycle::Moved { x, y } => {
                self.forward_lifecycle(
                    "moved",
                    json!({
                        "window_id": window.to_string(),
                        "x": x,
                        "y": y,
                    }),
                    driver,
                );
            }
            WindowLifecycle::Focused(focused) => {
                self.forward_lifecycle(
                    "focused",
                    json!({
                        "window_id": window.to_string(),
                        "focused": focused,
                    }),
                    driver,
                );
            }
            WindowLifecycle::ScaleFactorChanged { factor } => {
                // DPI semantics belong to the #388/#389/#114 slice —
                // forwarded for the record only (`recorded: false`).
                self.forward_lifecycle(
                    "scale_factor",
                    json!({
                        "window_id": window.to_string(),
                        "factor": factor,
                    }),
                    driver,
                );
            }
            WindowLifecycle::Action(intent) => {
                if intent.op == COMMAND_INTENT_OP {
                    self.on_view_command(window, intent, driver);
                } else {
                    self.track_view_op(&intent);
                    for re in self.rt.dispatch_window_intent(window, intent) {
                        self.observe_result(&re);
                        self.post_routed(&re, driver);
                    }
                }
            }
            WindowLifecycle::CloseRequested => {
                // Mirror `UiRuntime::handle_window`'s closing push,
                // then apply the close policy — only the last live
                // window consults the app's unsaved/recording guard.
                driver.post_event(
                    window,
                    &Event::Lifecycle {
                        state: LifecycleState::Closing,
                    },
                );
                let decision = self.close_decision(window, driver);
                if let Err(e) = driver.respond_close(window, decision) {
                    tracing::debug!(window = %window, error = %e, "respond_close failed");
                }
            }
            WindowLifecycle::Destroyed => self.on_destroyed(window, driver),
            // DriverEvent/WindowLifecycle are non-exhaustive — a
            // variant added by a newer port carries no semantics this
            // shell knows.
            _ => {}
        }
    }

    /// Forwards one window-lifecycle fact to the workspace through
    /// the internal `syn.window.lifecycle` op — the sealed app is
    /// unreachable directly, so this op is the shell's only path
    /// into window adoption/disposition state. Failures are logged,
    /// never fatal: a missed `Resized` must not wedge the loop.
    fn forward_lifecycle(&mut self, event: &str, params: Value, driver: &mut dyn WindowDriver) {
        self.teardown_seq += 1;
        let id = format!("shell-lifecycle-{}", self.teardown_seq);
        let mut params = params;
        params["event"] = json!(event);
        for routed in self.rt.dispatch_op(id, "syn.window.lifecycle", params) {
            self.post_unrouted(&routed, driver);
        }
    }

    /// New-window saga `Ready`: attach the pending view to the fresh
    /// window (the single `syn.view.attach` the app permits mid-saga)
    /// then commit the dock move. Any failure aborts the move
    /// (`dock_abort{failed}`) and destroys the orphan window — the
    /// source view keeps its ownership and UI binding throughout.
    fn drive_saga_ready(&mut self, window: WindowId, driver: &mut dyn WindowDriver) {
        let Some(view_id) = self.saga_windows.get(&window).cloned() else {
            return;
        };
        let Some(info) = dock_view_info(&view_id) else {
            self.saga_windows.remove(&window);
            self.abort_saga(driver, &view_id, window, "view has no dock metadata");
            return;
        };
        match self.attach_view(driver, window, &view_id, &info.targets) {
            Ok(_) => {
                self.teardown_seq += 1;
                let id = format!("shell-dock-commit-{}", self.teardown_seq);
                let mut committed = false;
                let mut failure: Option<String> = None;
                for event in
                    self.rt
                        .dispatch_op(id, "syn.window.dock_commit", json!({ "view_id": view_id }))
                {
                    if let Event::Result { ok, error, .. } = &event {
                        committed |= *ok;
                        if !*ok {
                            failure = Some(error.as_ref().map_or_else(
                                || "commit rejected".to_string(),
                                |e| e.message.clone(),
                            ));
                        }
                    }
                    self.post_unrouted(&event, driver);
                }
                if committed {
                    self.saga_windows.remove(&window);
                    tracing::info!(window = %window, view_id = %view_id, "dock move committed");
                } else {
                    self.abort_saga(
                        driver,
                        &view_id,
                        window,
                        &failure.unwrap_or_else(|| "commit produced no result".to_string()),
                    );
                }
            }
            Err(e) => {
                self.abort_saga(driver, &view_id, window, &e.to_string());
            }
        }
    }

    /// Aborts a failed new-window saga: `dock_abort{failed}` restores
    /// the source binding app-side, then the orphan window is
    /// destroyed (its `Destroyed` finds no saga entry — already
    /// removed — and no docked views).
    fn abort_saga(
        &mut self,
        driver: &mut dyn WindowDriver,
        view_id: &str,
        window: WindowId,
        detail: &str,
    ) {
        self.saga_windows.remove(&window);
        tracing::warn!(window = %window, view_id = %view_id, detail, "dock saga aborted");
        self.dispatch_dock_abort(view_id, detail, driver);
        if let Err(e) = driver.destroy_window(window) {
            tracing::debug!(window = %window, error = %e, "saga orphan destroy failed");
        }
    }

    /// Dispatches `syn.window.dock_abort{failed}` for `view_id` and
    /// mirrors the restored ownership into shell bookkeeping — the
    /// op goes through `dispatch_op` (unrouted, internal), so
    /// `observe_result` never sees its result and the `views` /
    /// `view_generation` maps would otherwise keep pointing at the
    /// dead destination.
    fn dispatch_dock_abort(&mut self, view_id: &str, detail: &str, driver: &mut dyn WindowDriver) {
        self.teardown_seq += 1;
        let id = format!("shell-dock-abort-{}", self.teardown_seq);
        for event in self.rt.dispatch_op(
            id,
            "syn.window.dock_abort",
            json!({ "view_id": view_id, "failed": true, "detail": detail }),
        ) {
            if let Event::Result {
                ok: true,
                data: Some(d),
                ..
            } = &event
            {
                match d["owner"].as_str().and_then(parse_window_id_str) {
                    Some(owner) => {
                        self.views.insert(view_id.to_string(), owner);
                    }
                    None => {
                        self.views.remove(view_id);
                    }
                }
                match d["generation"].as_u64() {
                    Some(g) => {
                        self.view_generation
                            .insert(view_id.to_string(), u32::try_from(g).unwrap_or(u32::MAX));
                    }
                    None => {
                        self.view_generation.remove(view_id);
                    }
                }
            }
            self.post_unrouted(&event, driver);
        }
    }

    /// Close policy for `CloseRequested`. Only the *last* live
    /// window can strand the shared project/session/recording state,
    /// so only its close is guarded — with survivors the state stays
    /// reachable and earlier windows approve unconditionally.
    ///
    /// The guard is the app-global `syn.window.close_guard` query
    /// dispatched synchronously through the same window-intent path
    /// `attach_view` uses; its routed events flow through the normal
    /// observe/post route like any other op result. A well-formed
    /// empty `reasons` approves; non-empty reasons, an op error, a
    /// missing result, or a malformed payload all fail closed to
    /// `Veto` — an unanswerable guard must never silently destroy
    /// the session's last window.
    fn close_decision(&mut self, window: WindowId, driver: &mut dyn WindowDriver) -> CloseDecision {
        let live = driver.windows();
        if live.len() != 1 || live[0] != window {
            return CloseDecision::Approve;
        }
        let intent_id = format!("shell-close-guard-{window}");
        let intent =
            ruviform_ui::ipc::Intent::new(intent_id.clone(), "syn.window.close_guard", json!({}));
        // Fail-closed default: stays `true` until the guard op
        // answers with a well-formed empty `reasons` array.
        let mut guarded = true;
        for re in self.rt.dispatch_window_intent(window, intent) {
            if let Event::Result { id, ok, data, .. } = &re.event
                && id == &intent_id
            {
                guarded = !(*ok
                    && data
                        .as_ref()
                        .and_then(|d| d.get("reasons"))
                        .and_then(Value::as_array)
                        .is_some_and(Vec::is_empty));
            }
            self.observe_result(&re);
            self.post_routed(&re, driver);
        }
        if guarded {
            tracing::info!(window = %window, "window close vetoed by guard");
            CloseDecision::Veto
        } else {
            CloseDecision::Approve
        }
    }

    /// Handles a view-minted `rvf.ui.command.v1` intent: strictly
    /// decodes it into the typed command vocabulary, resolves the
    /// producing view against live ownership, fills `DispatchContext`
    /// with the attach-adopted generation + clock, and dispatches the
    /// `syn.control.dispatch` through the runtime.
    ///
    /// Ownership rules — `ctx.window_id` is forwarded unchecked by
    /// `ControlsUi`, so the shell is the only boundary that can refuse
    /// a stale or foreign channel before stamping the live epoch:
    ///
    /// - `origin_view` must name the view *attached to this window*
    ///   (a reattach moves ownership; the old channel must not mint
    ///   commands under the new generation).
    /// - Without `origin_view` the window's bound view applies — the
    ///   current 1-window/1-view contract resolves it uniquely;
    ///   multiple bound views are ambiguous and fail closed rather
    ///   than picking one.
    /// - The epoch is the generation adopted from the view's last
    ///   successful attach — an unestablished one is never fabricated
    ///   (`unwrap_or(0)` would mint an op for an epoch nobody owns).
    ///
    /// Every refusal answers with an `invalid_params` `Result` on the
    /// arriving channel. The ok-ack is not delivered to the view —
    /// the authoritative receipt arrives via the `syn.control.receipt`
    /// → `Result` translation; a failed ack is forwarded as the
    /// rejected receipt.
    fn on_view_command(
        &mut self,
        window: WindowId,
        intent: ruviform_ui::ipc::Intent,
        driver: &mut dyn WindowDriver,
    ) {
        let cmd = match decode_view_command(&intent.params) {
            Ok(cmd) => cmd,
            Err(e) => {
                Self::reject_view_command(driver, window, intent.id, e.to_string());
                return;
            }
        };
        let resolved = if let Some(view) = &cmd.origin_view {
            match self.views.get(view) {
                Some(owner) if *owner == window => Ok(view.clone()),
                Some(owner) => Err(format!(
                    "view command origin_view {view} is attached to window {owner}, not this one"
                )),
                None => Err(format!("view command origin_view {view} is not attached")),
            }
        } else {
            let mut bound = self.views.iter().filter(|(_, w)| **w == window);
            match (bound.next(), bound.next()) {
                (Some((view, _)), None) => Ok(view.clone()),
                (None, _) => Err(
                    "view command carries no origin_view and window has no bound view".to_string(),
                ),
                _ => Err(
                    "view command carries no origin_view and window has multiple bound views"
                        .to_string(),
                ),
            }
        };
        let view = match resolved {
            Ok(view) => view,
            Err(message) => {
                Self::reject_view_command(driver, window, intent.id, message);
                return;
            }
        };
        if let CommandIntent::Action { target, action, .. } = &cmd.intent {
            if target == PROJECT_TARGET {
                self.dispatch_project_action(window, &cmd.command_id, action, driver);
                return;
            }
            if target == VIEW_STATE_TARGET {
                Self::answer_view_state_action(driver, window, &cmd.command_id, action);
                return;
            }
        }
        let Some(generation) = self.view_generation.get(&view).copied() else {
            Self::reject_view_command(
                driver,
                window,
                intent.id,
                format!("view {view} has no attach-adopted generation"),
            );
            return;
        };
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

    /// Runs a `project` Button action as its service op and answers the
    /// command with the op's own `Result` (ok or failed) on `window`, so
    /// the pressing Button resolves its receipt. Project actions are not
    /// control gestures, so no `syn.control.receipt` exists for them.
    fn dispatch_project_action(
        &mut self,
        window: WindowId,
        command_id: &str,
        action: &str,
        driver: &mut dyn WindowDriver,
    ) {
        let Some(op) = project_action_op(action) else {
            Self::reject_view_command(
                driver,
                window,
                command_id.to_string(),
                format!("unknown project action {action}"),
            );
            return;
        };
        let intent = ruviform_ui::ipc::Intent::new(command_id, op, json!({}));
        for re in self.rt.dispatch_window_intent(window, intent) {
            self.observe_result(&re);
            self.post_routed(&re, driver);
        }
    }

    /// Acknowledges a view-local action. The Tabs strip moves its display
    /// optimistically and settles on this receipt, so an accepted
    /// `tab-select` keeps the tab and a refused one restores the previous.
    fn answer_view_state_action(
        driver: &mut dyn WindowDriver,
        window: WindowId,
        command_id: &str,
        action: &str,
    ) {
        if action == TAB_SELECT_ACTION {
            driver.post_event(window, &Event::result_ok(command_id, json!({})));
        } else {
            Self::reject_view_command(
                driver,
                window,
                command_id.to_string(),
                format!("unknown view action {action}"),
            );
        }
    }

    /// Answers `id` with an `invalid_params` `Result` on `window` —
    /// every rejected view command gets a typed failure on the
    /// channel it arrived on, never a silent drop.
    fn reject_view_command(
        driver: &mut dyn WindowDriver,
        window: WindowId,
        id: String,
        message: String,
    ) {
        driver.post_event(
            window,
            &Event::result_err(id, ErrorBody::new(codes::INVALID_PARAMS, message)),
        );
    }

    /// Records `syn.view.*` intents so [`Self::observe_result`] can
    /// reconcile the view→window map on success. Only `attach`
    /// carries an owner: the declared `window_id` — the window the
    /// domain actually attaches the view to — resolved against the
    /// live tracked windows, never the channel the intent arrived
    /// on. A declared id no live window has (and every non-attach
    /// op) records `None`, so the view stays ownerless.
    fn track_view_op(&mut self, intent: &ruviform_ui::ipc::Intent) {
        let op = match intent.op.as_str() {
            "syn.view.attach" => ViewOp::Attach,
            "syn.view.detach" => ViewOp::Detach,
            "syn.view.close" => ViewOp::Close,
            "syn.window.dock_commit" | "syn.window.dock_reattach" => ViewOp::DockCommit,
            "syn.window.dock_detach" => ViewOp::DockDetach,
            "syn.window.dock_abort" => ViewOp::DockAbort,
            _ => return,
        };
        let Some(view) = intent.params["view_id"].as_str() else {
            return;
        };
        let owner = match op {
            // The owner must be a *live* window the shell tracks —
            // a declared id no window has is kept as `None`.
            ViewOp::Attach => intent.params["window_id"].as_str().and_then(|declared| {
                self.windows
                    .keys()
                    .find(|w| w.to_string() == declared)
                    .copied()
            }),
            _ => None,
        };
        self.pending_view_ops
            .insert(intent.id.clone(), (op, view.to_string(), owner));
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
        let Some((op, view, declared)) = self.pending_view_ops.remove(id) else {
            return;
        };
        if !*ok {
            return;
        }
        match op {
            // The declared window is the owner the domain attached
            // the view to — mirror it exactly. A declared `window_id`
            // that cannot name a live window leaves the view
            // ownerless — no channel may mint its commands or a
            // stale epoch.
            ViewOp::Attach => {
                if let Some(owner) = declared {
                    self.views.insert(view.clone(), owner);
                    let generation = data
                        .as_ref()
                        .and_then(|d| d["generation"].as_u64())
                        .map(|g| u32::try_from(g).unwrap_or(u32::MAX));
                    if let Some(g) = generation {
                        self.view_generation.insert(view, g);
                    } else {
                        // A generation-less attach establishes no
                        // epoch — drop any adopted one so commands
                        // fail closed at resolution.
                        self.view_generation.remove(&view);
                    }
                } else {
                    self.views.remove(&view);
                    self.view_generation.remove(&view);
                }
            }
            ViewOp::Detach => {
                // A dock-domain detach pools the view — it no longer
                // lives on any window, so the owner association must
                // go (a later `Destroyed` must not tear it down). A
                // legacy view keeps the old contract: the association
                // survives detach so `Destroyed` still tears it down.
                if dock_view_info(&view).is_some() {
                    self.views.remove(&view);
                    self.view_generation.remove(&view);
                }
            }
            ViewOp::Close | ViewOp::DockDetach => {
                self.views.remove(&view);
                self.view_generation.remove(&view);
            }
            ViewOp::DockCommit => {
                // `data.to` names the committed destination — the
                // dock already moved; mirror the new owner.
                if let Some(owner) = data
                    .as_ref()
                    .and_then(|d| d["to"].as_str())
                    .and_then(parse_window_id_str)
                {
                    self.views.insert(view.clone(), owner);
                } else {
                    self.views.remove(&view);
                }
                let generation = data
                    .as_ref()
                    .and_then(|d| d["generation"].as_u64())
                    .map(|g| u32::try_from(g).unwrap_or(u32::MAX));
                match generation {
                    Some(g) => {
                        self.view_generation.insert(view, g);
                    }
                    None => {
                        self.view_generation.remove(&view);
                    }
                }
            }
            ViewOp::DockAbort => {
                // `data.owner` is the restored owner (`"w{n}"` or
                // JSON null for the detached pool).
                match data
                    .as_ref()
                    .and_then(|d| d["owner"].as_str())
                    .and_then(parse_window_id_str)
                {
                    Some(owner) => {
                        self.views.insert(view.clone(), owner);
                    }
                    None => {
                        self.views.remove(&view);
                    }
                }
                let generation = data
                    .as_ref()
                    .and_then(|d| d["generation"].as_u64())
                    .map(|g| u32::try_from(g).unwrap_or(u32::MAX));
                match generation {
                    Some(g) => {
                        self.view_generation.insert(view, g);
                    }
                    None => {
                        self.view_generation.remove(&view);
                    }
                }
            }
        }
    }

    /// `Destroyed`: cancel the window's jobs, forward the lifecycle
    /// fact so the workspace applies the recorded
    /// `WindowCloseDisposition` (the app tears docked views down per
    /// disposition — `DetachViews` pools them UI-side, `ReleaseViews`
    /// closes them), replay `syn.view.close` only for *legacy*
    /// (non-dock) views the shell recorded on the window, drop
    /// bookkeeping, and exit the loop when no window remains
    /// (app-exit is consumer policy).
    fn on_destroyed(&mut self, window: WindowId, driver: &mut dyn WindowDriver) {
        let cancelled = self.rt.cancel_window_jobs(window);
        if cancelled > 0 {
            tracing::info!(window = %window, cancelled, "cancelled window jobs");
        }
        self.windows.remove(&window);
        self.window_titles.remove(&window);

        // A saga destination dying before commit aborts the pending
        // move — the source view keeps ownership and UI binding. The
        // abort result restores `views`/`view_generation` to the
        // source owner, so the orphan scan below no longer sees the
        // view on the dead window.
        if let Some(view_id) = self.saga_windows.remove(&window) {
            self.dispatch_dock_abort(&view_id, "destination window destroyed", driver);
        }

        // Forward `destroyed` — the workspace removes the window with
        // its disposition; the app applies the per-view UI teardown
        // inside the op. The result names the docked views it held.
        let mut dock_views: Vec<String> = Vec::new();
        let mut lifecycle_failed = false;
        self.teardown_seq += 1;
        let id = format!("shell-lifecycle-{}", self.teardown_seq);
        for event in self.rt.dispatch_op(
            id,
            "syn.window.lifecycle",
            json!({ "window_id": window.to_string(), "event": "destroyed" }),
        ) {
            if let Event::Result { ok, data, .. } = &event {
                if *ok {
                    if let Some(views) = data.as_ref().and_then(|d| d["views"].as_array()) {
                        dock_views = views
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect();
                    }
                } else {
                    lifecycle_failed = true;
                }
            }
            self.post_unrouted(&event, driver);
        }

        // Views the dead window held: dock-domain views were already
        // torn down app-side per disposition — drop shell bookkeeping
        // only. On a lifecycle failure fall back to the safe default
        // (`DetachViews`): dock-domain views get a `syn.view.detach`
        // replay (pool, nothing lost) instead of a blind close.
        let mut orphans: Vec<String> = self
            .views
            .iter()
            .filter(|(_, w)| **w == window)
            .map(|(v, _)| v.clone())
            .collect();
        for view in orphans.drain(..) {
            let docked = dock_views.iter().any(|v| v == &view)
                || (!lifecycle_failed && dock_view_info(&view).is_some());
            if docked {
                self.views.remove(&view);
                self.view_generation.remove(&view);
                if lifecycle_failed {
                    // Fail-safe pool — never lose a dock view to a
                    // teardown path that could not read the
                    // disposition.
                    self.teardown_seq += 1;
                    let id = format!("shell-teardown-{}", self.teardown_seq);
                    for event in self.rt.dispatch_op(
                        id,
                        "syn.view.detach",
                        json!({ "view_id": view, "reason": "unmount" }),
                    ) {
                        self.post_unrouted(&event, driver);
                    }
                }
            } else {
                // Legacy view — the contractual close replay.
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
