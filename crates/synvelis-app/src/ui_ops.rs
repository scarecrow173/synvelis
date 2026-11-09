//! `syn.view.*` / `syn.control.*` / `syn.viz.*` — the SV-GUI-02 op
//! surface that wires [`ControlsUi`]/[`VizUi`] to the frontend over
//! `rvf.ipc.v1`.
//!
//! Op results are minimal acks — receipts, projections, viz frames,
//! and view lifecycle pushes are `notify` events on
//! [`SynvelisApp::event_outbox`], routed by `data.channel` on the
//! frontend. The outbox is a shared `Rc<RefCell<VecDeque<Event>>>`
//! (single-threaded app) drained by the webview host loop and by
//! headless drivers identically.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use ruviform_ui::dock::{
    DockEvent, DockTarget, MoveToken, ViewInstanceId, WindowCloseDisposition, WindowGeometry,
};
use ruviform_ui::driver::WindowSpec;
use ruviform_ui::event::HostEvent;
use ruviform_ui::ipc::{ErrorBody, Event, codes};
use ruviform_ui::window::WindowId;
use serde_json::{Value, json};

use synvelis_project::{TickTimeMapping, is_issuable_raw};
use synvelis_workspace::VizSourceId;
use synvelis_workspace::controls_ui::{
    CommandIntent, ControlsUi, DispatchContext, GestureCancelReason, ParamProjection,
    TRANSPORT_POSITION_TARGET, TRANSPORT_TARGET,
};
use synvelis_workspace::viz_ui::{
    ManualMeter, ManualSpectrum, MemoryWaveform, MeterPort, SpectrumPort, VizUi, VizViewEvent,
    WaveformPort,
};

use crate::app::SynvelisApp;
use crate::screens::{
    DockViewInfo, InspectorRow, InspectorScreenState, TransportScreenState, dock_view_info,
    main_view_id,
};

/// Outbox depth cap — receipts/projections are never silently
/// unbounded; on overflow the oldest entry drops and
/// [`SynvelisApp::ui_drops`] counts it.
const OUTBOX_CAP: usize = 4096;
/// `syn.control.targets` row cap.
const TARGET_CAP: usize = 256;
/// Total samples a `syn.viz.register_waveform` may allocate.
const MAX_WAVEFORM_SAMPLES: usize = 1_000_000;
/// Default waveform plot width when `plot_px` is omitted.
const DEFAULT_PLOT_PX: u32 = 1024;

/// Shared notify outbox — `host.eval` drains this every tick.
pub type UiOutbox = Rc<RefCell<VecDeque<Event>>>;

/// Manually fed viz ports kept alive so `syn.viz.push_frame` /
/// `syn.viz.set_waveform_data` can drive them (test + boot fixture
/// path; real engine ports register on the same `VizUi` registry).
pub(crate) enum ManualPort {
    /// Manual meter port (push [`MeterFrame`]).
    Meter(Arc<ManualMeter>),
    /// Manual spectrum port (push [`SpectrumFrame`]).
    Spectrum(Arc<ManualSpectrum>),
}

/// Per-view registry entry — the app's view-side mirror of the
/// `ViewLink` attach/detach contract. `window` is `None` while the
/// view sits in the workspace's detached pool (dock-detached, or its
/// host window was destroyed under the `DetachViews` disposition) —
/// the entry stays so `syn.view.list` can distinguish attached from
/// pooled views.
pub(crate) struct ViewEntry {
    window: Option<String>,
}

/// In-flight dock move keyed by wire `view_id` — the op layer owns
/// the `MoveToken`; it never crosses the wire.
pub(crate) enum DockMove {
    /// `begin_transfer` issued against an existing adopted window.
    Prepared {
        /// Two-phase move token (app-private).
        token: MoveToken,
        /// Declared destination window.
        target: WindowId,
    },
    /// New-window saga: a window-creation request is in flight and
    /// the destination is not committed yet. `window` records the
    /// destination once the saga's UI-first attach lands — the only
    /// attach `syn.view.attach` permits while awaiting.
    AwaitingWindow {
        /// Window title requested for the driver spec — surfaced in
        /// `syn.window.list` pending moves for observability.
        title: String,
        /// Destination once the exception attach names it.
        window: Option<WindowId>,
    },
}

/// A driver-window creation request pushed by
/// `syn.window.dock_prepare_new_window` — the app→shell direction of
/// the new-window dock saga, shared through the same
/// `Rc<RefCell>` pattern as [`UiOutbox`]/`clock`/`quit`.
pub struct WindowRequest {
    /// Wire view id the move belongs to (`"view-{n}"`).
    pub view_id: String,
    /// Correlation id echoed in the op result.
    pub request_id: u64,
    /// Driver spec the shell creates — built app-side so the shell
    /// never invents product window parameters.
    pub spec: WindowSpec,
}

/// SV-GUI-02 additions to [`SynvelisApp`] state — kept in one struct so
/// `app.rs` stays the composition shape.
pub struct UiOpsState {
    /// Controls gateway (receipts/projections/gesture lifecycle).
    pub(crate) controls: ControlsUi,
    /// Viz surface owner (bounded queries/frames/events).
    pub(crate) viz: VizUi,
    /// Shared notify outbox.
    pub(crate) outbox: UiOutbox,
    /// Attached views (`view_id` → window) — app-side registry.
    pub(crate) views: HashMap<String, ViewEntry>,
    /// Live control subscriptions `(view, target)` — the bounded set
    /// of bindings whose rows are pushed after each pump.
    pub(crate) subscriptions: HashSet<(String, String)>,
    /// Last pushed meter/spectrum frame sequence per `(view, surface)`
    /// — latest-wins push dedup.
    pub(crate) frame_seq: HashMap<(String, String), u64>,
    /// Bound viz surfaces `(view, surface)` — app-side registry for
    /// frame pushes (the `VizUi` surface map is its private state).
    pub(crate) surfaces: HashSet<(String, String)>,
    /// Manually driven viz ports by source id.
    pub(crate) manual_ports: HashMap<String, ManualPort>,
    /// UI clock (ms) — shared cell so the driver-side shell can
    /// advance it per `Tick` while the app is sealed inside
    /// `UiRuntime`; deterministic, never wall-clock. [`SynvelisApp::
    /// set_ui_now`] and headless drivers write the same cell, and
    /// `ui_pump` re-syncs the gateway clock from it.
    pub(crate) clock: Rc<Cell<u64>>,
    /// Outbox drop counter (overflow evidence, reported in app info).
    pub(crate) ui_drops: u64,
    /// Notify sequence shared with `app_state_notify` — owned by app.
    pub(crate) notify_seq: u64,
    /// In-flight dock moves keyed by wire `view_id`.
    pub(crate) dock_moves: HashMap<String, DockMove>,
    /// Pending app→shell window-creation requests (dock new-window
    /// saga) — drained by `NativeShell`.
    pub(crate) window_requests: Rc<RefCell<VecDeque<WindowRequest>>>,
    /// Monotonic id for `dock_prepare_new_window` requests.
    pub(crate) window_request_seq: u64,
}

impl UiOpsState {
    /// Fresh state with contract-default timeouts.
    #[must_use]
    pub fn new() -> Self {
        Self {
            controls: ControlsUi::new(),
            viz: VizUi::new(),
            outbox: Rc::new(RefCell::new(VecDeque::new())),
            views: HashMap::new(),
            subscriptions: HashSet::new(),
            frame_seq: HashMap::new(),
            surfaces: HashSet::new(),
            manual_ports: HashMap::new(),
            clock: Rc::new(Cell::new(0)),
            ui_drops: 0,
            notify_seq: 0,
            dock_moves: HashMap::new(),
            window_requests: Rc::new(RefCell::new(VecDeque::new())),
            window_request_seq: 0,
        }
    }
}

impl Default for UiOpsState {
    fn default() -> Self {
        Self::new()
    }
}

fn invalid(msg: impl Into<String>) -> ErrorBody {
    ErrorBody::new(codes::INVALID_PARAMS, msg.into())
}

fn internal(msg: impl Into<String>) -> ErrorBody {
    ErrorBody::new(codes::SERVICE_ERROR, msg.into())
}

fn str_param<'a>(params: &'a Value, key: &str) -> Result<&'a str, ErrorBody> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(format!("params.{key} (string) required")))
}

fn u64_param(params: &Value, key: &str) -> Result<u64, ErrorBody> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid(format!("params.{key} (u64) required")))
}

fn u32_param(params: &Value, key: &str) -> Result<u32, ErrorBody> {
    u32::try_from(u64_param(params, key)?).map_err(|_| invalid(format!("params.{key} exceeds u32")))
}

fn usize_param(params: &Value, key: &str) -> Result<usize, ErrorBody> {
    usize::try_from(u64_param(params, key)?)
        .map_err(|_| invalid(format!("params.{key} exceeds usize")))
}

fn f64_param(params: &Value, key: &str) -> Result<f64, ErrorBody> {
    params
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid(format!("params.{key} (number) required")))
}

fn json_param<T: serde::de::DeserializeOwned>(params: &Value, key: &str) -> Result<T, ErrorBody> {
    params
        .get(key)
        .cloned()
        .ok_or_else(|| invalid(format!("params.{key} required")))
        .and_then(|v| serde_json::from_value(v).map_err(|e| invalid(format!("params.{key}: {e}"))))
}

fn viz_err(e: &synvelis_workspace::VizError) -> ErrorBody {
    internal(format!("viz: {e}"))
}

fn dock_err(e: &synvelis_workspace::WorkspaceError) -> ErrorBody {
    internal(format!("dock: {e}"))
}

/// `"w{n}"` → [`WindowId`]; `0`/malformed → `None`.
fn parse_window_id(s: &str) -> Option<WindowId> {
    WindowId::new(s.strip_prefix('w')?.parse().ok()?)
}

/// `"view-{n}"` → [`ViewInstanceId`]; `0`/malformed → `None`.
fn parse_view_instance(s: &str) -> Option<ViewInstanceId> {
    ViewInstanceId::new(s.strip_prefix("view-")?.parse().ok()?)
}

/// Dock-domain resolution for `syn.view.*`: `Ok(Some)` for the
/// composition root's registered views, `Ok(None)` for ids outside
/// the dock domain (legacy UI-only path), `Err` for `view-{n}`-shaped
/// ids with no registered metadata — those must never reach "UI
/// attached but dock unregistered".
fn dock_domain_view(view: &str) -> Result<Option<DockViewInfo>, ErrorBody> {
    if parse_view_instance(view).is_none() {
        return Ok(None);
    }
    dock_view_info(view).map(Some).ok_or_else(|| {
        invalid(format!(
            "view {view} has no dock metadata — register it in screens.rs"
        ))
    })
}

/// Flattens a [`ParamProjection`]'s `InspectorBreakdown` into display
/// rows — the agreed breakdown fields (evaluation, observation, mode,
/// arm, writer state, capability) plus the resolved read-only flag.
fn inspector_rows(p: &ParamProjection) -> Vec<InspectorRow> {
    let b = &p.breakdown;
    let e = &b.evaluation;
    let row = |field: &str, value: String| InspectorRow {
        field: field.to_string(),
        value,
    };
    vec![
        row("plain", format!("{}", e.host_plain)),
        row("normalized", format!("{}", e.host_normalized)),
        row("mode", format!("{:?}", b.mode)),
        row("arm", format!("{:?}", b.arm)),
        row("observation", format!("{:?}", b.observation)),
        row("writer", format!("{:?}", b.writer_state)),
        row("read_only", p.read_only.to_string()),
        row("capability", b.capability.is_some().to_string()),
    ]
}

impl SynvelisApp {
    /// Shared outbox handle — cloned by the host loop before the
    /// service moves into `UiRuntime`.
    #[must_use]
    pub fn event_outbox(&self) -> UiOutbox {
        Rc::clone(&self.ui.outbox)
    }

    /// Shared window-request queue — the shell drains it for the
    /// dock new-window saga (same hand-out pattern as
    /// [`Self::event_outbox`]).
    #[must_use]
    pub fn window_requests(&self) -> Rc<RefCell<VecDeque<WindowRequest>>> {
        Rc::clone(&self.ui.window_requests)
    }

    /// Outbox drop counter — evidence for verification reports.
    #[must_use]
    pub const fn ui_drops(&self) -> u64 {
        self.ui.ui_drops
    }

    /// Shared UI-clock cell — handed to the native shell before the
    /// service moves into `UiRuntime`; the shell advances it per
    /// driver `Tick` and `ui_pump` applies it to the gateways.
    #[must_use]
    pub fn ui_clock(&self) -> Rc<Cell<u64>> {
        Rc::clone(&self.ui.clock)
    }

    /// Injects the UI clock (ms). Called by the webview host each tick
    /// and by headless drivers for deterministic timeout/stale tests.
    /// Writes the shared cell [`Self::ui_clock`] exposes.
    pub fn set_ui_now(&mut self, now_ms: u64) {
        self.ui.clock.set(now_ms);
        self.ui.controls.set_clock(now_ms);
    }

    /// Gates a read op on a live view — detached/unknown views never
    /// receive fresh truth.
    fn require_attached_view(&self, view: &str) -> Result<(), ErrorBody> {
        if self.ui.viz.is_attached(view) {
            Ok(())
        } else {
            Err(internal(format!("view {view} not attached")))
        }
    }

    fn push_notify(&mut self, channel: &str, data: Value) {
        self.ui.notify_seq += 1;
        let seq = self.ui.notify_seq;
        let mut q = self.ui.outbox.borrow_mut();
        if q.len() >= OUTBOX_CAP {
            q.pop_front();
            self.ui.ui_drops += 1;
        }
        // `Map::insert` consumes `data` — the payload is moved onto
        // the wire, never re-borrowed.
        let mut body = serde_json::Map::new();
        body.insert("channel".to_owned(), Value::from(channel));
        body.insert("data".to_owned(), data);
        q.push_back(Event::Notify {
            seq,
            data: Value::Object(body),
        });
    }

    /// Pumps both UI gateways, drains receipts/events into the outbox,
    /// and pushes bound subscription rows + changed meter/spectrum
    /// frames. Called by every UI op at its tail — the outbox always
    /// carries the full consequence of the op.
    pub fn ui_pump(&mut self) {
        let now = self.ui.clock.get();
        // The shared cell is the single clock source — the shell may
        // have advanced it since the last op, so re-sync the gateway
        // here rather than trusting the `set_ui_now` writer path.
        self.ui.controls.set_clock(now);
        // `pump` would consume the wire itself — drain first so the
        // frontend sees every receipt, then `deliver` each back into
        // the owning binding (the same work `pump` performs).
        for wr in self.ui.controls.drain_wire() {
            let view = wr.view.clone();
            let target = wr.target.clone();
            let receipt = wr.receipt.clone();
            self.push_notify(
                "syn.control.receipt",
                json!({
                    "view_id": view,
                    "target": target,
                    "receipt": receipt,
                }),
            );
            self.ui.controls.deliver(&view, &target, receipt);
        }
        self.ui.viz.pump(now);
        for ev in self.ui.viz.drain_events() {
            match ev {
                VizViewEvent::Viewport { .. } | VizViewEvent::Select { .. } => {
                    self.push_notify("syn.viz.event", json!({ "event": ev }));
                }
            }
        }
        self.push_subscription_rows();
        self.push_surface_frames();
        self.capture_screen_state();
    }

    /// Captures the main view's display state out of the `ControlsUi`
    /// projection rows into the shared [`MainScreenState`] cell — the
    /// transport readout (`TransportRow.observed`) and the inspector
    /// breakdown (`StripRow.observed` of the first subscribed param
    /// target, target-sorted for determinism). Runs at the tail of
    /// every `ui_pump`, after subscriptions have re-projected, so the
    /// cell always mirrors the last landed truth — never a request or
    /// a preview (synvelis#112 slice 1).
    pub(crate) fn capture_screen_state(&self) {
        let view = main_view_id();
        let wants_transport =
            self.ui.subscriptions.iter().any(|(v, t)| {
                v == &view && (t == TRANSPORT_TARGET || t == TRANSPORT_POSITION_TARGET)
            });
        let mut transport = TransportScreenState::default();
        if wants_transport
            && let Some(observed) = self
                .ui
                .controls
                .transport_row(&view)
                .and_then(|row| row.observed)
        {
            // Same seconds conversion the seek's `display_value` uses —
            // beat position under the project tempo map.
            let tempo = TickTimeMapping::new(&self.workspace.session().project().tempo_map).ok();
            transport.status = Some(format!("{:?}", observed.status));
            transport.position_seconds = tempo
                .as_ref()
                .and_then(|t| t.beat_to_seconds(observed.position.to_beat()).ok());
            transport.loop_enabled = observed.loop_range.is_some();
        }
        let mut inspector = InspectorScreenState::default();
        let mut param_targets: Vec<String> = self
            .ui
            .subscriptions
            .iter()
            .filter(|(v, t)| v == &view && t != TRANSPORT_TARGET && t != TRANSPORT_POSITION_TARGET)
            .map(|(_, t)| t.clone())
            .collect();
        param_targets.sort();
        for target in param_targets {
            let Some(observed) = self
                .ui
                .controls
                .strip_row(&view, &target)
                .and_then(|row| row.observed)
            else {
                continue;
            };
            inspector.target = Some(target);
            inspector.rows = inspector_rows(&observed);
            break;
        }
        let mut screen = self.main_screen.borrow_mut();
        screen.transport = transport;
        screen.inspector = inspector;
    }

    /// Bound subscription rows — the projection channel. `observed`
    /// carries the upstream `StateProjection` shape (`revision` +
    /// `value` + `provenance`) so the frontend's `observe` can apply
    /// its own revision guard without a second format.
    fn push_subscription_rows(&mut self) {
        let subs: Vec<(String, String)> = self.ui.subscriptions.iter().cloned().collect();
        let tempo = TickTimeMapping::new(&self.workspace.session().project().tempo_map).ok();
        for (view, target) in subs {
            let Some(snap) = self.ui.controls.binding(&view, &target) else {
                continue;
            };
            let kind = if target == TRANSPORT_TARGET || target == TRANSPORT_POSITION_TARGET {
                "transport"
            } else {
                "param"
            };
            // The transport seekbar's controlled value is seconds (the
            // beat position converted under the project tempo map); the
            // raw transport object stays in `observed` for other readers.
            // Param projections carry the multi-key evaluation object,
            // which `projection_value_input` cannot decode — the
            // controlled display contract is the normalized domain, so
            // the adapter emits it as a single-key `{"normalized": …}`
            // (the same unit gesture commits write back).
            let display_value = match snap.last_observed.as_ref().map(|o| &o.value) {
                Some(synvelis_workspace::controls_ui::ProjectionValue::Transport(p))
                    if target == TRANSPORT_POSITION_TARGET =>
                {
                    tempo
                        .as_ref()
                        .and_then(|t| t.beat_to_seconds(p.position.to_beat()).ok())
                        .map(Value::from)
                }
                Some(synvelis_workspace::controls_ui::ProjectionValue::Parameter(p)) => {
                    Some(json!({ "normalized": p.breakdown.evaluation.host_normalized }))
                }
                _ => None,
            };
            let observed = snap.last_observed.as_ref().map(|o| {
                let value = match &o.value {
                    synvelis_workspace::controls_ui::ProjectionValue::Parameter(p) => {
                        let e = &p.breakdown.evaluation;
                        json!({
                            "plain": e.host_plain,
                            "normalized": e.host_normalized,
                            "read_only": p.read_only,
                            "mode": format!("{:?}", p.breakdown.mode),
                            "arm": format!("{:?}", p.breakdown.arm),
                            "observation": format!("{:?}", p.breakdown.observation),
                            "capability": p.breakdown.capability.is_some(),
                        })
                    }
                    synvelis_workspace::controls_ui::ProjectionValue::Transport(p) => {
                        json!({
                            "status": format!("{:?}", p.status),
                            "position": format!("{}", p.position),
                            "loop_range": p
                                .loop_range
                                .map(|(a, b)| format!("{a}..{b}")),
                            "record_enabled": p.record_enabled,
                            "recording_active": p.recording_active,
                        })
                    }
                };
                // `provenance` is optional on the upstream
                // `StateProjection` — absent, never null.
                let mut observed = serde_json::Map::new();
                observed.insert("revision".to_owned(), Value::from(o.revision));
                observed.insert("value".to_owned(), value);
                if let Some(p) = &o.provenance {
                    observed.insert("provenance".to_owned(), Value::from(p.clone()));
                }
                Value::Object(observed)
            });
            self.push_notify(
                "syn.control.projection",
                json!({
                    "view_id": view,
                    "target": target,
                    "row": {
                        "kind": kind,
                        "phase": snap.phase,
                        "generation": snap.generation,
                        "attached": snap.attached,
                        "subscribed": snap.subscribed,
                        "timed_out": snap.timed_out,
                        "preview": snap.preview,
                        "last_receipt": snap.last_receipt,
                        "observed": observed,
                        "display_value": display_value,
                        "observed_seq": snap.observed_seq,
                    },
                }),
            );
        }
    }

    /// Latest-wins meter/spectrum frame pushes for bound surfaces.
    fn push_surface_frames(&mut self) {
        let keys: Vec<(String, String)> = self.ui.surfaces.iter().cloned().collect();
        for (view, surface) in keys {
            let (seq, frame) = if let Some(r) = self.ui.viz.meter_row(&view, &surface) {
                (r.frame.as_ref().map(|f| f.sequence), "meter")
            } else if let Some(r) = self.ui.viz.spectrum_row(&view, &surface) {
                (r.frame.as_ref().map(|f| f.sequence), "spectrum")
            } else {
                continue;
            };
            let Some(seq) = seq else { continue };
            if self
                .ui
                .frame_seq
                .get(&(view.clone(), surface.clone()))
                .is_some_and(|&s| s >= seq)
            {
                continue;
            }
            self.ui
                .frame_seq
                .insert((view.clone(), surface.clone()), seq);
            let payload = if frame == "meter" {
                self.ui
                    .viz
                    .meter_row(&view, &surface)
                    .and_then(|r| r.frame.map(|f| json!({ "frame": f })))
            } else {
                self.ui
                    .viz
                    .spectrum_row(&view, &surface)
                    .and_then(|r| r.frame.map(|f| json!({ "frame": f })))
            };
            if let Some(p) = payload {
                self.push_notify(
                    "syn.viz.frame",
                    json!({
                        "view_id": view,
                        "surface_id": surface,
                        "kind": frame,
                        "frame": p["frame"],
                    }),
                );
            }
        }
    }

    // -- syn.view.* --------------------------------------------------------

    /// UI half of a dock move — both gateways re-attach (implicit
    /// detach(dock-transfer) + generation bump) and the app registry
    /// records the new owner window. Never touches workspace state.
    fn ui_bind_view(&mut self, view: &str, window: &str) {
        let session = self.workspace.session_mut();
        self.ui.controls.attach_view(session, view, window);
        self.ui.viz.attach_view(view, window);
        self.ui.views.insert(
            view.to_string(),
            ViewEntry {
                window: Some(window.to_string()),
            },
        );
    }

    /// Rebinds the UI side of `view` to whatever the dock currently
    /// says owns it — `Some(window)` re-attaches, `None` (detached
    /// pool) detaches. Restores UI after a failed dock move.
    fn ui_sync_view_owner(&mut self, view: &str, inst: ViewInstanceId) {
        let owner = self.workspace.dock().owner_of(inst).map(|w| w.to_string());
        let current = self.ui.views.get(view).and_then(|e| e.window.clone());
        if current == owner {
            return;
        }
        if let Some(w) = owner {
            self.ui_bind_view(view, &w);
        } else {
            let session = self.workspace.session_mut();
            self.ui
                .controls
                .detach_view(session, view, &GestureCancelReason::Unmount);
            self.ui.viz.detach_view(view);
            if let Some(e) = self.ui.views.get_mut(view) {
                e.window = None;
            }
        }
    }

    /// `syn.view.attach` — attaches `view` on `window` to both UI
    /// gateways; returns the view's live generation.
    ///
    /// Dock-domain views (`view-{n}` with registered metadata) first
    /// update the workspace dock — adopt on first attach, one-shot
    /// `move_view` afterwards — so a dock rejection never leaves a
    /// half-bound view. While a `DockMove::AwaitingWindow` saga is
    /// pending, only the destination attach is allowed and it is
    /// UI-only (the dock placement commits later).
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_view_attach(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let window = str_param(params, "window_id")?.to_owned();
        if let Some(info) = dock_domain_view(&view)? {
            self.dock_view_attach(&view, &info, &window)?;
        }
        // Both gateways handle re-attach internally (dock-transfer
        // detach + generation bump) — no pre-detach here.
        self.ui_bind_view(&view, &window);
        self.ui_pump();
        let (gen_c, gen_v) = (
            self.ui.controls.view_state(&view).map_or(0, |(_, g, _)| g),
            self.ui.viz.generation(&view).unwrap_or(0),
        );
        self.push_notify(
            "syn.view.state",
            json!({ "view_id": view, "attached": true }),
        );
        Ok(json!({
            "view_id": view,
            "generation": gen_c,
            "viz_generation": gen_v,
            "attached": true,
        }))
    }

    /// Workspace half of `syn.view.attach` for dock-domain views —
    /// runs before the UI attach so a dock rejection never leaves a
    /// half-bound view.
    fn dock_view_attach(
        &mut self,
        view: &str,
        info: &DockViewInfo,
        window: &str,
    ) -> Result<(), ErrorBody> {
        let inst = info.instance;
        let win = parse_window_id(window)
            .ok_or_else(|| invalid(format!("params.window_id must be \"w{{n}}\", got {window}")))?;
        match self.ui.dock_moves.get(view) {
            Some(DockMove::AwaitingWindow {
                window: pending, ..
            }) => {
                // The saga's UI-first attach: allowed only toward the
                // destination the shell named on the window's
                // lifecycle — every other window (and any attach
                // before the destination exists) is busy-rejected.
                if *pending != Some(win) {
                    return Err(invalid(format!(
                        "view {view} has a pending dock move to a new window"
                    )));
                }
                return Ok(());
            }
            Some(DockMove::Prepared { .. }) => {
                return Err(invalid(format!("view {view} has a pending dock move")));
            }
            None => {}
        }
        if self.workspace.dock().is_transferring(inst) {
            return Err(invalid(format!("view {view} is transferring")));
        }
        if !self.workspace.registry().contains(inst) {
            self.workspace
                .adopt_view(inst, info.kind, info.title, Some(DockTarget::Window(win)))
                .map_err(|e| dock_err(&e))?;
        } else if self.workspace.dock().owner_of(inst) != Some(win) {
            self.workspace
                .move_view(inst, DockTarget::Window(win))
                .map_err(|e| dock_err(&e))?;
        }
        Ok(())
    }

    /// `syn.view.detach` — contractual cancel of in-flight gestures +
    /// generation bump; project/audio/render are untouched. Dock-domain
    /// views additionally return to the workspace's detached pool.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_view_detach(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let reason = params
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("unmount");
        let reason = match reason {
            "dock-transfer" => GestureCancelReason::DockTransfer,
            "escape" => GestureCancelReason::Escape,
            "lost-focus" => GestureCancelReason::LostFocus,
            "lost-pointer-capture" => GestureCancelReason::LostPointerCapture,
            _ => GestureCancelReason::Unmount,
        };
        let info = dock_domain_view(&view)?;
        if let Some(info) = &info {
            self.dock_view_detach(&view, info)?;
        }
        let session = self.workspace.session_mut();
        self.ui.controls.detach_view(session, &view, &reason);
        self.ui.viz.detach_view(&view);
        if info.is_some()
            && let Some(e) = self.ui.views.get_mut(&view)
        {
            e.window = None;
        }
        self.ui_pump();
        self.push_notify(
            "syn.view.state",
            json!({ "view_id": view, "attached": false }),
        );
        Ok(json!({ "view_id": view, "attached": false }))
    }

    /// Workspace half of `syn.view.detach` for dock-domain views —
    /// pools the view when it is docked; already-pooled and
    /// never-adopted views pass through.
    fn dock_view_detach(&mut self, view: &str, info: &DockViewInfo) -> Result<(), ErrorBody> {
        let inst = info.instance;
        if self.ui.dock_moves.contains_key(view) || self.workspace.dock().is_transferring(inst) {
            return Err(invalid(format!("view {view} has a pending dock move")));
        }
        if self.workspace.registry().contains(inst) && !self.workspace.dock().is_detached(inst) {
            self.workspace
                .detach_to_pool(inst)
                .map_err(|e| dock_err(&e))?;
        }
        Ok(())
    }

    /// `syn.view.close` — drops the view's UI state on both gateways.
    /// Dock-domain views additionally leave the workspace entirely;
    /// an in-flight dock move is aborted first ("close aborts then
    /// removes").
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_view_close(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        if let Some(info) = dock_domain_view(&view)? {
            self.dock_view_close(&view, &info)?;
        }
        let session = self.workspace.session_mut();
        self.ui.controls.close_view(session, &view);
        self.ui.viz.close_view(&view);
        self.ui.views.remove(&view);
        self.ui.subscriptions.retain(|(v, _)| v != &view);
        self.ui.frame_seq.retain(|(v, _), _| v != &view);
        self.ui.surfaces.retain(|(v, _)| v != &view);
        self.ui_pump();
        self.push_notify(
            "syn.view.state",
            json!({ "view_id": view, "attached": false, "closed": true }),
        );
        Ok(json!({ "view_id": view, "closed": true }))
    }

    /// Workspace half of `syn.view.close` for dock-domain views —
    /// aborts any pending dock move (releasing queued window
    /// requests), then removes the view from dock/registry/state.
    fn dock_view_close(&mut self, view: &str, info: &DockViewInfo) -> Result<(), ErrorBody> {
        let inst = info.instance;
        match self.ui.dock_moves.remove(view) {
            Some(DockMove::Prepared { token, .. }) => {
                if let Err(e) = self.workspace.abort_transfer(token)
                    && self.workspace.dock().is_transferring(inst)
                {
                    return Err(dock_err(&e));
                }
            }
            Some(DockMove::AwaitingWindow { .. }) => {
                self.ui
                    .window_requests
                    .borrow_mut()
                    .retain(|r| r.view_id != view);
            }
            None => {}
        }
        if self.workspace.registry().contains(inst) {
            self.workspace.remove_view(inst).map_err(|e| dock_err(&e))?;
        }
        Ok(())
    }

    /// `syn.view.list` — the view registry; `window_id`/`attached`
    /// distinguish live bindings from detached-pool entries.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    #[must_use]
    pub fn op_view_list(&self) -> Value {
        let views: Vec<Value> = self
            .ui
            .views
            .iter()
            .map(|(v, e)| {
                json!({
                    "view_id": v,
                    "window_id": e.window,
                    "attached": e.window.is_some(),
                    "generation": self.ui.controls.view_state(v).map_or(0, |(_, g, _)| g),
                })
            })
            .collect();
        json!({ "views": views })
    }

    // -- syn.window.* -------------------------------------------------------

    /// Rejects `view` when a dock move is pending for it — the
    /// `dock_moves` (app pending) and `is_transferring` (dock
    /// in-flight) locks are separate layers; both reject.
    fn check_dock_view_free(&self, view: &str, inst: ViewInstanceId) -> Result<(), ErrorBody> {
        if self.ui.dock_moves.contains_key(view) {
            return Err(invalid(format!("view {view} has a pending dock move")));
        }
        if self.workspace.dock().is_transferring(inst) {
            return Err(invalid(format!("view {view} is transferring")));
        }
        Ok(())
    }

    /// `syn.window.adopt` — idempotently registers a driver-created
    /// window with the workspace. Re-adoption with consistent
    /// params is a no-op ack; contradictory title/geometry is an
    /// error — never a silent overwrite.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or conflicting records.
    pub fn op_window_adopt(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let window = str_param(params, "window_id")?;
        let id = parse_window_id(window)
            .ok_or_else(|| invalid(format!("params.window_id must be \"w{{n}}\", got {window}")))?;
        let title = params.get("title").and_then(Value::as_str);
        let width = params
            .get("width")
            .and_then(Value::as_u64)
            .map(|v| u32::try_from(v).map_err(|_| invalid("params.width exceeds u32")))
            .transpose()?;
        let height = params
            .get("height")
            .and_then(Value::as_u64)
            .map(|v| u32::try_from(v).map_err(|_| invalid("params.height exceeds u32")))
            .transpose()?;
        let x = params
            .get("x")
            .and_then(Value::as_i64)
            .map(|v| i32::try_from(v).map_err(|_| invalid("params.x exceeds i32")))
            .transpose()?;
        let y = params
            .get("y")
            .and_then(Value::as_i64)
            .map(|v| i32::try_from(v).map_err(|_| invalid("params.y exceeds i32")))
            .transpose()?;
        if let Some(rec) = self.workspace.windows().iter().find(|w| w.id == id) {
            let g = &rec.geometry;
            let conflict = title.is_some_and(|t| t != rec.title)
                || width.is_some_and(|w| w != g.width)
                || height.is_some_and(|h| h != g.height)
                || x.is_some_and(|x| x != g.x)
                || y.is_some_and(|y| y != g.y);
            if conflict {
                return Err(invalid(format!(
                    "window {id} already adopted with different parameters"
                )));
            }
            return Ok(json!({ "window_id": id.to_string(), "adopted": true }));
        }
        self.workspace
            .adopt_window(
                id,
                title.unwrap_or("Synvelis"),
                WindowGeometry {
                    x: x.unwrap_or(0),
                    y: y.unwrap_or(0),
                    width: width.unwrap_or(1280),
                    height: height.unwrap_or(800),
                    monitor: None,
                },
            )
            .map_err(|e| dock_err(&e))?;
        Ok(json!({ "window_id": id.to_string(), "adopted": true }))
    }

    /// `syn.window.list` — the workspace's window/dock observation
    /// surface: adopted windows, docked vs detached views, and
    /// in-flight dock moves.
    #[must_use]
    pub fn op_window_list(&self) -> Value {
        let windows: Vec<Value> = self
            .workspace
            .windows()
            .iter()
            .map(|w| {
                json!({
                    "window_id": w.id.to_string(),
                    "title": w.title,
                    "geometry": {
                        "x": w.geometry.x,
                        "y": w.geometry.y,
                        "width": w.geometry.width,
                        "height": w.geometry.height,
                    },
                    "veto_close": w.veto_close,
                    "disposition": match w.disposition {
                        WindowCloseDisposition::DetachViews => "detach_views",
                        WindowCloseDisposition::ReleaseViews => "release_views",
                        WindowCloseDisposition::RelocateTo { .. } => "relocate_to",
                        _ => "unknown",
                    },
                })
            })
            .collect();
        let dock = self.workspace.dock();
        let mut docked = Vec::new();
        let mut detached = Vec::new();
        for v in dock.all_views() {
            let wire = format!("view-{}", v.get());
            match dock.owner_of(v) {
                Some(w) => docked.push(json!({ "view_id": wire, "window_id": w.to_string() })),
                None => detached.push(wire),
            }
        }
        let pending: Vec<Value> = self
            .ui
            .dock_moves
            .iter()
            .map(|(view, m)| match m {
                DockMove::Prepared { target, .. } => json!({
                    "view_id": view,
                    "state": "prepared",
                    "target": target.to_string(),
                }),
                DockMove::AwaitingWindow { title, window } => json!({
                    "view_id": view,
                    "state": "awaiting_window",
                    "title": title,
                    "window_id": window.map(|w| w.to_string()),
                }),
            })
            .collect();
        json!({
            "windows": windows,
            "docked_views": docked,
            "detached_views": detached,
            "pending_moves": pending,
        })
    }

    /// `syn.window.dock_prepare` — begins a two-phase move of `view`
    /// to an existing adopted `window` (`begin_transfer`); the token
    /// stays app-side until `syn.window.dock_commit`/`dock_abort`.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters, unknown ids, or a busy view.
    pub fn op_window_dock_prepare(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let window = str_param(params, "window_id")?.to_owned();
        let info = dock_domain_view(&view)?
            .ok_or_else(|| invalid(format!("view {view} is not a dock-domain view")))?;
        let inst = info.instance;
        let win = parse_window_id(&window)
            .ok_or_else(|| invalid(format!("params.window_id must be \"w{{n}}\", got {window}")))?;
        self.check_dock_view_free(&view, inst)?;
        if !self.workspace.registry().contains(inst) {
            return Err(internal(format!(
                "view {view} is not adopted — attach it first"
            )));
        }
        if !self.workspace.dock().is_registered(win) {
            return Err(invalid(format!("window {window} is not adopted")));
        }
        let token = self
            .workspace
            .begin_transfer(inst, DockTarget::Window(win))
            .map_err(|e| dock_err(&e))?;
        self.ui
            .dock_moves
            .insert(view.clone(), DockMove::Prepared { token, target: win });
        self.ui_pump();
        Ok(json!({ "view_id": view, "window_id": window, "prepared": true }))
    }

    /// `syn.window.dock_prepare_new_window` — phase 1 of the
    /// new-window saga: marks the view pending and pushes a
    /// [`WindowRequest`] the shell drains into `create_window`.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters, unknown ids, or a busy view.
    pub fn op_window_dock_prepare_new_window(
        &mut self,
        params: &Value,
    ) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let title = params
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let info = dock_domain_view(&view)?
            .ok_or_else(|| invalid(format!("view {view} is not a dock-domain view")))?;
        let inst = info.instance;
        self.check_dock_view_free(&view, inst)?;
        if !self.workspace.registry().contains(inst) {
            return Err(internal(format!(
                "view {view} is not adopted — attach it first"
            )));
        }
        if self.workspace.dock().owner_of(inst).is_none() {
            return Err(invalid(format!(
                "view {view} is not docked — reattach it first"
            )));
        }
        let title = title.unwrap_or_else(|| "Synvelis".to_string());
        self.ui.window_request_seq += 1;
        let request_id = self.ui.window_request_seq;
        self.ui.dock_moves.insert(
            view.clone(),
            DockMove::AwaitingWindow {
                title: title.clone(),
                window: None,
            },
        );
        self.ui
            .window_requests
            .borrow_mut()
            .push_back(WindowRequest {
                view_id: view.clone(),
                request_id,
                spec: WindowSpec {
                    title,
                    ..WindowSpec::default()
                },
            });
        self.ui_pump();
        Ok(json!({ "view_id": view, "pending": true, "request_id": request_id }))
    }

    /// `syn.window.dock_commit` — commits the pending move and
    /// rebinds the UI side to the destination. `Prepared` uses
    /// `commit_transfer`; `AwaitingWindow` uses the one-shot
    /// `move_view` once the saga's attach named the destination.
    ///
    /// # Errors
    /// [`ErrorBody`] when no move is pending, the destination is not
    /// ready, or the commit fails (the dock rolls back to source).
    pub fn op_window_dock_commit(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let info = dock_domain_view(&view)?
            .ok_or_else(|| invalid(format!("view {view} is not a dock-domain view")))?;
        let inst = info.instance;
        match self.ui.dock_moves.get(&view) {
            Some(DockMove::AwaitingWindow { window: None, .. }) => {
                return Err(invalid(format!(
                    "view {view} has no destination window yet"
                )));
            }
            Some(_) => {}
            None => {
                return Err(internal(format!("view {view} has no pending dock move")));
            }
        }
        let Some(entry) = self.ui.dock_moves.remove(&view) else {
            return Err(internal(format!("view {view} has no pending dock move")));
        };
        let (report, awaiting) = match entry {
            DockMove::Prepared { token, .. } => match self.workspace.commit_transfer(token) {
                Ok(report) => (report, false),
                Err(e) => {
                    self.push_notify(
                        "syn.window.state",
                        json!({ "view_id": view, "dock_move": "failed", "detail": format!("{e}") }),
                    );
                    return Err(dock_err(&e));
                }
            },
            DockMove::AwaitingWindow {
                window: Some(win), ..
            } => match self.workspace.move_view(inst, DockTarget::Window(win)) {
                Ok(report) => (report, true),
                Err(e) => {
                    self.ui_sync_view_owner(&view, inst);
                    self.push_notify(
                        "syn.window.state",
                        json!({ "view_id": view, "dock_move": "failed", "detail": format!("{e}") }),
                    );
                    return Err(dock_err(&e));
                }
            },
            DockMove::AwaitingWindow { window: None, .. } => unreachable!("checked above"),
        };
        let to = report.to.to_string();
        // The AwaitingWindow path already bound the UI to the
        // destination via the exception attach — bind only when the
        // recorded owner differs (Prepared path, or an attach that
        // never landed).
        let bound = self.ui.views.get(&view).and_then(|e| e.window.as_deref());
        if !awaiting || bound != Some(to.as_str()) {
            self.ui_bind_view(&view, &to);
        }
        self.ui_pump();
        self.push_notify(
            "syn.window.state",
            json!({
                "view_id": view,
                "dock_move": "committed",
                "from": report.from.map(|w| w.to_string()),
                "to": to,
            }),
        );
        let (gen_c, gen_v) = (
            self.ui.controls.view_state(&view).map_or(0, |(_, g, _)| g),
            self.ui.viz.generation(&view).unwrap_or(0),
        );
        Ok(json!({
            "view_id": view,
            "committed": true,
            "from": report.from.map(|w| w.to_string()),
            "to": to,
            "generation": gen_c,
            "viz_generation": gen_v,
        }))
    }

    /// `syn.window.dock_abort` — aborts the pending move; the view
    /// keeps (or returns to) its source ownership and UI binding.
    /// `AwaitingWindow` aborts also release queued window requests
    /// and restore the UI binding the exception attach may have moved.
    ///
    /// # Errors
    /// [`ErrorBody`] when no move is pending or the abort fails.
    pub fn op_window_dock_abort(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let failed = params
            .get("failed")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let detail = params.get("detail").and_then(Value::as_str);
        let info = dock_domain_view(&view)?
            .ok_or_else(|| invalid(format!("view {view} is not a dock-domain view")))?;
        let inst = info.instance;
        let Some(entry) = self.ui.dock_moves.remove(&view) else {
            return Err(internal(format!("view {view} has no pending dock move")));
        };
        match entry {
            DockMove::Prepared { token, .. } => {
                if let Err(e) = self.workspace.abort_transfer(token)
                    && self.workspace.dock().is_transferring(inst)
                {
                    return Err(dock_err(&e));
                }
            }
            DockMove::AwaitingWindow { .. } => {
                self.ui
                    .window_requests
                    .borrow_mut()
                    .retain(|r| r.view_id != view);
                self.ui_sync_view_owner(&view, inst);
            }
        }
        self.ui_pump();
        self.push_notify(
            "syn.window.state",
            json!({
                "view_id": view,
                "dock_move": if failed { "failed" } else { "aborted" },
                "detail": detail,
            }),
        );
        let owner = self.workspace.dock().owner_of(inst).map(|w| w.to_string());
        let (gen_c, gen_v) = (
            self.ui.controls.view_state(&view).map_or(0, |(_, g, _)| g),
            self.ui.viz.generation(&view).unwrap_or(0),
        );
        Ok(json!({
            "view_id": view,
            "aborted": true,
            "owner": owner,
            "generation": gen_c,
            "viz_generation": gen_v,
        }))
    }

    /// `syn.window.dock_detach` — one-shot move of `view` into the
    /// workspace's detached pool (`detach_to_pool` + UI detach).
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters, unknown ids, or a busy view.
    pub fn op_window_dock_detach(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let info = dock_domain_view(&view)?
            .ok_or_else(|| invalid(format!("view {view} is not a dock-domain view")))?;
        let inst = info.instance;
        self.check_dock_view_free(&view, inst)?;
        if !self.workspace.registry().contains(inst) {
            return Err(internal(format!(
                "view {view} is not adopted — attach it first"
            )));
        }
        if !self.workspace.dock().is_detached(inst) {
            self.workspace
                .detach_to_pool(inst)
                .map_err(|e| dock_err(&e))?;
        }
        let session = self.workspace.session_mut();
        self.ui
            .controls
            .detach_view(session, &view, &GestureCancelReason::DockTransfer);
        self.ui.viz.detach_view(&view);
        if let Some(e) = self.ui.views.get_mut(&view) {
            e.window = None;
        }
        self.ui_pump();
        self.push_notify(
            "syn.window.state",
            json!({ "view_id": view, "dock_move": "detached" }),
        );
        Ok(json!({ "view_id": view, "detached": true }))
    }

    /// `syn.window.dock_reattach` — one-shot move of `view` (from the
    /// pool or another window) into `window` (`move_view` + UI
    /// attach).
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters, unknown ids, or a busy view.
    pub fn op_window_dock_reattach(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let window = str_param(params, "window_id")?.to_owned();
        let info = dock_domain_view(&view)?
            .ok_or_else(|| invalid(format!("view {view} is not a dock-domain view")))?;
        let inst = info.instance;
        let win = parse_window_id(&window)
            .ok_or_else(|| invalid(format!("params.window_id must be \"w{{n}}\", got {window}")))?;
        self.check_dock_view_free(&view, inst)?;
        if !self.workspace.registry().contains(inst) {
            return Err(internal(format!(
                "view {view} is not adopted — attach it first"
            )));
        }
        if !self.workspace.dock().is_registered(win) {
            return Err(invalid(format!("window {window} is not adopted")));
        }
        let from = self.workspace.dock().owner_of(inst);
        if from != Some(win) {
            self.workspace
                .move_view(inst, DockTarget::Window(win))
                .map_err(|e| dock_err(&e))?;
        }
        self.ui_bind_view(&view, &win.to_string());
        self.ui_pump();
        self.push_notify(
            "syn.window.state",
            json!({
                "view_id": view,
                "dock_move": "reattached",
                "from": from.map(|w| w.to_string()),
                "to": win.to_string(),
            }),
        );
        let (gen_c, gen_v) = (
            self.ui.controls.view_state(&view).map_or(0, |(_, g, _)| g),
            self.ui.viz.generation(&view).unwrap_or(0),
        );
        Ok(json!({
            "view_id": view,
            "reattached": true,
            "from": from.map(|w| w.to_string()),
            "to": win.to_string(),
            "generation": gen_c,
            "viz_generation": gen_v,
        }))
    }

    /// `syn.window.lifecycle` — internal forwarding from
    /// `NativeShell` (the app is sealed inside `UiRuntime`; this op
    /// is the shell's only path into workspace window state). Params:
    /// `window_id`, `event` (`created`/`ready`/`resized`/`moved`/
    /// `focused`/`scale_factor`/`destroyed`) plus event payload
    /// fields. `destroyed` returns the applied `disposition` and the
    /// `views` the window held so the shell can replay the right
    /// teardown per view.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or workspace failures.
    pub fn op_window_lifecycle(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let window = str_param(params, "window_id")?;
        let id = parse_window_id(window)
            .ok_or_else(|| invalid(format!("params.window_id must be \"w{{n}}\", got {window}")))?;
        let event = str_param(params, "event")?;
        let result = match event {
            "created" | "ready" => {
                if !self.workspace.windows().iter().any(|w| w.id == id) {
                    let i32p = |key: &str| {
                        params
                            .get(key)
                            .and_then(Value::as_i64)
                            .and_then(|v| i32::try_from(v).ok())
                    };
                    let u32p = |key: &str| {
                        params
                            .get(key)
                            .and_then(Value::as_u64)
                            .and_then(|v| u32::try_from(v).ok())
                    };
                    self.workspace
                        .adopt_window(
                            id,
                            params
                                .get("title")
                                .and_then(Value::as_str)
                                .unwrap_or("Synvelis"),
                            WindowGeometry {
                                x: i32p("x").unwrap_or(0),
                                y: i32p("y").unwrap_or(0),
                                width: u32p("width").unwrap_or(1280),
                                height: u32p("height").unwrap_or(800),
                                monitor: None,
                            },
                        )
                        .map_err(|e| dock_err(&e))?;
                }
                // A new-window saga destination names its pending
                // move here — the shell only sends `dock_view` for
                // windows it created for a `WindowRequest`, so the
                // `AwaitingWindow` exception attach can then match
                // exactly this window (and no other).
                if let Some(view) = params.get("dock_view").and_then(Value::as_str)
                    && let Some(DockMove::AwaitingWindow { window: slot, .. }) =
                        self.ui.dock_moves.get_mut(view)
                {
                    *slot = Some(id);
                }
                json!({ "window_id": window, "event": event, "adopted": true })
            }
            "resized" => {
                self.workspace
                    .on_window_event(
                        id,
                        &HostEvent::Resized {
                            width: u32_param(params, "width")?,
                            height: u32_param(params, "height")?,
                        },
                    )
                    .map_err(|e| dock_err(&e))?;
                json!({ "window_id": window, "event": event })
            }
            "moved" => {
                let i32p = |key: &str| -> Result<i32, ErrorBody> {
                    params
                        .get(key)
                        .and_then(Value::as_i64)
                        .ok_or_else(|| invalid(format!("params.{key} (i32) required")))
                        .and_then(|v| {
                            i32::try_from(v)
                                .map_err(|_| invalid(format!("params.{key} exceeds i32")))
                        })
                };
                self.workspace
                    .on_window_event(
                        id,
                        &HostEvent::Moved {
                            x: i32p("x")?,
                            y: i32p("y")?,
                        },
                    )
                    .map_err(|e| dock_err(&e))?;
                json!({ "window_id": window, "event": event })
            }
            "focused" => {
                let focused = params
                    .get("focused")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| invalid("params.focused (bool) required"))?;
                self.workspace
                    .on_window_event(id, &HostEvent::Focused(focused))
                    .map_err(|e| dock_err(&e))?;
                json!({ "window_id": window, "event": event })
            }
            "scale_factor" => {
                // DPI/IME semantics belong to the #388/#389/#114
                // slice — acknowledged, never mapped into workspace
                // layout state.
                json!({ "window_id": window, "event": event, "recorded": false })
            }
            "destroyed" => self.op_window_lifecycle_destroyed(window, id)?,
            other => return Err(invalid(format!("unknown lifecycle event {other}"))),
        };
        self.ui_pump();
        Ok(result)
    }

    /// `syn.window.lifecycle` の `destroyed` — workspace の
    /// `WindowCloseDisposition` を適用し、orphan になった pending
    /// dock move を解放した上で、保持していた view ごとに
    /// disposition 相当の UI teardown（`DetachViews`→detach、
    /// `ReleaseViews`→close）を実行する。結果には適用した
    /// disposition と対象 view 一覧を載せ、shell が view 単位の
    /// 後始末（legacy view の close replay 等）を判断できるようにする。
    fn op_window_lifecycle_destroyed(
        &mut self,
        window: &str,
        id: WindowId,
    ) -> Result<Value, ErrorBody> {
        let disposition = self
            .workspace
            .windows()
            .iter()
            .find(|w| w.id == id)
            .map(|w| w.disposition);
        self.workspace
            .on_window_event(id, &HostEvent::Destroyed)
            .map_err(|e| dock_err(&e))?;
        let mut views = Vec::new();
        for ev in self.workspace.drain_dock_events() {
            if let DockEvent::WindowRemoved {
                window: w,
                views: vs,
            } = ev
                && w == id
            {
                views = vs.iter().map(|v| format!("view-{}", v.get())).collect();
            }
        }
        // Pending moves orphaned by the destroy: a Prepared
        // whose destination died or whose token the dock
        // already ended, and an AwaitingWindow whose recorded
        // destination died *or whose source view the destroy
        // just pooled* — release them so the view lock never
        // leaks.
        let stale: Vec<String> = self
            .ui
            .dock_moves
            .iter()
            .filter_map(|(view, m)| {
                let inst = parse_view_instance(view)?;
                let stale = match m {
                    DockMove::Prepared { target, .. } => {
                        *target == id || !self.workspace.dock().is_transferring(inst)
                    }
                    DockMove::AwaitingWindow { window, .. } => {
                        window.is_some_and(|w| w == id) || views.iter().any(|v| v == view)
                    }
                };
                stale.then(|| view.clone())
            })
            .collect();
        for view in stale {
            match self.ui.dock_moves.remove(&view) {
                Some(DockMove::Prepared { token, .. }) => {
                    let _ = self.workspace.abort_transfer(token);
                }
                Some(DockMove::AwaitingWindow { .. }) => {
                    self.ui
                        .window_requests
                        .borrow_mut()
                        .retain(|r| r.view_id != view);
                    if let Some(inst) = parse_view_instance(&view) {
                        self.ui_sync_view_owner(&view, inst);
                    }
                }
                None => {}
            }
            self.push_notify(
                "syn.window.state",
                json!({
                    "view_id": view,
                    "dock_move": "failed",
                    "detail": "window destroyed",
                }),
            );
        }
        // Per-view UI teardown under the applied disposition —
        // the same ops an explicit `syn.view.detach`/`close`
        // would run, so gateway state, the app registry, and
        // subscriptions stay consistent with one authority.
        // Views with no UI-side entry have nothing to tear down.
        let release = matches!(disposition, Some(WindowCloseDisposition::ReleaseViews));
        for view in &views {
            if !self.ui.views.contains_key(view) {
                continue;
            }
            let params = json!({ "view_id": view });
            let result = if release {
                self.op_view_close(&params)
            } else {
                self.op_view_detach(&params)
            };
            if let Err(e) = result {
                tracing::warn!(view_id = %view, error = %e.message, "window-destroy view teardown failed");
            }
        }
        let disposition = if release {
            "release_views"
        } else {
            "detach_views"
        };
        Ok(json!({
            "window_id": window,
            "event": "destroyed",
            "disposition": disposition,
            "views": views,
        }))
    }

    // -- syn.project.* -----------------------------------------------------

    /// `syn.project.undo` — `ControlsUi::undo` (`ProjectSession::undo`
    /// plus a re-projection of every bound truth) on the single
    /// project session. An empty history is `undone: false`, never a
    /// fabricated success (synvelis#112 slice 2).
    ///
    /// # Errors
    /// [`ErrorBody`] when the session undo fails.
    pub fn op_project_undo(&mut self) -> Result<Value, ErrorBody> {
        let session = self.workspace.session_mut();
        let undone = self
            .ui
            .controls
            .undo(session)
            .map_err(|e| internal(format!("undo: {e}")))?;
        self.ui_pump();
        Ok(json!({ "undone": undone }))
    }

    /// `syn.project.redo` — `ControlsUi::redo`, the redo counterpart
    /// of [`Self::op_project_undo`].
    ///
    /// # Errors
    /// [`ErrorBody`] when the session redo fails.
    pub fn op_project_redo(&mut self) -> Result<Value, ErrorBody> {
        let session = self.workspace.session_mut();
        let redone = self
            .ui
            .controls
            .redo(session)
            .map_err(|e| internal(format!("redo: {e}")))?;
        self.ui_pump();
        Ok(json!({ "redone": redone }))
    }

    // -- syn.control.* -----------------------------------------------------

    /// `syn.control.dispatch` — routes `params.intent` +
    /// `params.ctx` into [`ControlsUi`]; result is queue-ack only,
    /// receipts arrive on `syn.control.receipt`.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_control_dispatch(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let intent: CommandIntent = json_param(params, "intent")?;
        let ctx: DispatchContext = json_param(params, "ctx")?;
        let session = self.workspace.session_mut();
        self.ui.controls.dispatch(session, intent, ctx);
        self.ui_pump();
        Ok(json!({ "accepted": true }))
    }

    /// `syn.control.subscribe` — binds `(view, target)` and pushes the
    /// initial projection row via `syn.control.projection`.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_control_subscribe(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let target = str_param(params, "target")?.to_owned();
        {
            let session = self.workspace.session();
            self.ui
                .controls
                .subscribe(session, &view, &target)
                .map_err(|e| internal(format!("subscribe: {e}")))?;
        }
        self.ui.subscriptions.insert((view, target));
        self.ui_pump();
        Ok(json!({ "subscribed": true }))
    }

    /// `syn.control.unsubscribe` — drops the subscription row pushes.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_control_unsubscribe(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let target = str_param(params, "target")?.to_owned();
        self.ui.controls.unsubscribe(&view, &target);
        self.ui.subscriptions.remove(&(view, target));
        Ok(json!({ "subscribed": false }))
    }

    /// `syn.control.targets` — bounded bindable-target listing.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    #[must_use]
    pub fn op_control_targets(&self) -> Value {
        let mut targets: Vec<Value> = vec![
            json!({ "target": TRANSPORT_TARGET, "kind": "transport" }),
            json!({ "target": TRANSPORT_POSITION_TARGET, "kind": "transport" }),
        ];
        // `control.targets` is id-ordered, so the well-known bootstrap
        // fader (synvelis#185) sorts last — cap only *issued* targets
        // or a large project would silently drop the one row every
        // consumer needs.
        let ctl = &self.workspace.session().project().control;
        let issued = ctl
            .targets
            .iter()
            .filter(|(id, _)| is_issuable_raw(id.raw()))
            .take(TARGET_CAP);
        let structural = ctl
            .targets
            .iter()
            .filter(|(id, _)| !is_issuable_raw(id.raw()));
        for (id, t) in issued.chain(structural) {
            targets.push(json!({
                "target": id.to_tagged(),
                "kind": "param",
                "read_only": t.is_readonly(),
            }));
        }
        json!({ "targets": targets })
    }

    // -- syn.viz.* ---------------------------------------------------------

    /// `syn.viz.bind` — binds `surface` on `view` to `source`
    /// (`params.kind`: "waveform" | "meter" | "spectrum").
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_viz_bind(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let surface = str_param(params, "surface_id")?.to_owned();
        let source: VizSourceId = str_param(params, "source")?.to_owned();
        let kind = str_param(params, "kind")?.to_owned();
        match kind.as_str() {
            "waveform" => {
                let px = params
                    .get("plot_px")
                    .and_then(Value::as_u64)
                    .map_or(Ok(DEFAULT_PLOT_PX), |v| {
                        u32::try_from(v).map_err(|_| invalid("params.plot_px exceeds u32"))
                    })?;
                self.ui
                    .viz
                    .bind_waveform(&view, &surface, source, px)
                    .map_err(|e| viz_err(&e))?;
            }
            "meter" => self
                .ui
                .viz
                .bind_meter(&view, &surface, source)
                .map_err(|e| viz_err(&e))?,
            "spectrum" => self
                .ui
                .viz
                .bind_spectrum(&view, &surface, source)
                .map_err(|e| viz_err(&e))?,
            other => return Err(invalid(format!("unknown viz kind {other}"))),
        }
        self.ui.surfaces.insert((view, surface));
        self.ui_pump();
        Ok(json!({ "bound": true }))
    }

    /// `syn.viz.meta` — bounded surface row (status/meta/span/cache).
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_viz_meta(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?;
        let surface = str_param(params, "surface_id")?;
        self.require_attached_view(view)?;
        if let Some(r) = self.ui.viz.waveform_row(view, surface) {
            return Ok(json!({
                "kind": "waveform", "source": r.source,
                "status": r.status, "meta": r.meta, "span": r.span,
                "plot_px": r.plot_px, "view_stale": r.view_stale,
                "cache_entries": r.cache_entries, "cache_bytes": r.cache_bytes,
            }));
        }
        if let Some(r) = self.ui.viz.meter_row(view, surface) {
            return Ok(json!({
                "kind": "meter", "source": r.source, "status": r.status,
                "meta": r.meta, "scale": r.scale, "stale": r.stale,
                "dropped": r.dropped, "has_frame": r.frame.is_some(),
            }));
        }
        if let Some(r) = self.ui.viz.spectrum_row(view, surface) {
            return Ok(json!({
                "kind": "spectrum", "source": r.source, "status": r.status,
                "meta": r.meta, "scale": r.scale, "stale": r.stale,
                "dropped": r.dropped, "has_frame": r.frame.is_some(),
            }));
        }
        Err(internal(format!("unknown surface {surface} on {view}")))
    }

    /// `syn.viz.query` — one bounded waveform window query:
    /// `start`/`end` (seconds), `buckets` (plot px), `max_bytes` cap on
    /// the returned view.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_viz_query(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let surface = str_param(params, "surface_id")?.to_owned();
        self.require_attached_view(&view)?;
        let start = f64_param(params, "start")?;
        let end = f64_param(params, "end")?;
        let buckets = u32_param(params, "buckets")?;
        let max_bytes = params.get("max_bytes").and_then(Value::as_u64).map_or(
            Ok(synvelis_workspace::viz_ui::DEFAULT_MAX_QUERY_BYTES),
            |v| usize::try_from(v).map_err(|_| invalid("params.max_bytes exceeds usize")),
        )?;
        self.ui
            .viz
            .set_span(&view, &surface, start, end)
            .map_err(|e| viz_err(&e))?;
        self.ui
            .viz
            .set_plot_px(&view, &surface, buckets)
            .map_err(|e| viz_err(&e))?;
        self.ui.viz.ensure_waveform(&view, &surface);
        let row = self
            .ui
            .viz
            .waveform_row(&view, &surface)
            .ok_or_else(|| internal(format!("unknown surface {surface} on {view}")))?;
        let bytes = row
            .view
            .as_ref()
            .map_or(0, synvelis_workspace::WaveformView::byte_len);
        if bytes > max_bytes {
            return Err(internal(format!(
                "over-budget: view {bytes}B > max {max_bytes}B"
            )));
        }
        self.ui_pump();
        Ok(json!({
            "view": row.view,
            "status": row.status,
            "stale": row.view_stale,
            "source": row.source,
            "bytes": bytes,
        }))
    }

    /// `syn.viz.viewport` — pan (`d`) or zoom (`factor`, `at`).
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_viz_viewport(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let surface = str_param(params, "surface_id")?.to_owned();
        if let Some(d) = params.get("d").and_then(Value::as_f64) {
            self.ui
                .viz
                .pan(&view, &surface, d)
                .map_err(|e| viz_err(&e))?;
        }
        if let Some(f) = params.get("factor").and_then(Value::as_f64) {
            let at = params.get("at").and_then(Value::as_f64).unwrap_or(0.0);
            self.ui
                .viz
                .zoom(&view, &surface, f, at)
                .map_err(|e| viz_err(&e))?;
        }
        if let (Some(s), Some(e)) = (
            params.get("start").and_then(Value::as_f64),
            params.get("end").and_then(Value::as_f64),
        ) {
            self.ui
                .viz
                .set_span(&view, &surface, s, e)
                .map_err(|e| viz_err(&e))?;
        }
        self.ui_pump();
        Ok(json!({ "applied": true }))
    }

    /// `syn.viz.select` — forwards a selection range.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_viz_select(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let surface = str_param(params, "surface_id")?.to_owned();
        let start = f64_param(params, "start")?;
        let end = f64_param(params, "end")?;
        self.ui
            .viz
            .select(&view, &surface, start, end)
            .map_err(|e| viz_err(&e))?;
        self.ui_pump();
        Ok(json!({ "selected": true }))
    }

    /// `syn.viz.snapshot` — latest-complete meter/spectrum frame row.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_viz_snapshot(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?;
        let surface = str_param(params, "surface_id")?;
        self.require_attached_view(view)?;
        if let Some(r) = self.ui.viz.meter_row(view, surface) {
            return Ok(json!({
                "kind": "meter", "frame": r.frame, "stale": r.stale,
                "dropped": r.dropped, "status": r.status,
            }));
        }
        if let Some(r) = self.ui.viz.spectrum_row(view, surface) {
            return Ok(json!({
                "kind": "spectrum", "frame": r.frame, "stale": r.stale,
                "dropped": r.dropped, "status": r.status,
            }));
        }
        Err(internal(format!("no snapshot surface {surface} on {view}")))
    }

    /// `syn.viz.register` — registers a manually-driven port
    /// (`kind`: "meter" | "spectrum" | "waveform") so headless tests and
    /// the boot UI have a real source path; engine-owned ports register
    /// on the same registry later.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_viz_register(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let source = str_param(params, "source")?.to_owned();
        match str_param(params, "kind")? {
            "meter" => {
                let channels = usize_param(params, "channels")?;
                let port = Arc::new(ManualMeter::new(source.clone(), channels));
                self.ui
                    .viz
                    .register_meter(&source, port.clone() as Arc<dyn MeterPort>);
                self.ui.manual_ports.insert(source, ManualPort::Meter(port));
            }
            "spectrum" => {
                let bins = usize_param(params, "bins")?;
                let first = f64_param(params, "first_hz")?;
                let step = f64_param(params, "step_hz")?;
                let port = Arc::new(ManualSpectrum::new(source.clone(), bins, first, step));
                self.ui
                    .viz
                    .register_spectrum(&source, port.clone() as Arc<dyn SpectrumPort>);
                self.ui
                    .manual_ports
                    .insert(source, ManualPort::Spectrum(port));
            }
            "waveform" => {
                let rate = u32_param(params, "rate")?;
                let channels: Vec<Vec<f64>> = json_param(params, "channels")?;
                let total: usize = channels.iter().map(Vec::len).sum();
                if total > MAX_WAVEFORM_SAMPLES {
                    return Err(invalid(format!(
                        "channels exceed {MAX_WAVEFORM_SAMPLES} total samples"
                    )));
                }
                let port = Arc::new(MemoryWaveform::new(
                    source.clone(),
                    rate,
                    synvelis_workspace::viz_ui::WaveformAxis::Seconds,
                    &channels,
                ));
                self.ui
                    .viz
                    .register_waveform(&source, port as Arc<dyn WaveformPort>);
            }
            other => return Err(invalid(format!("unknown port kind {other}"))),
        }
        Ok(json!({ "registered": true }))
    }

    /// `syn.viz.push_frame` — pushes one `MeterFrame`/`SpectrumFrame`
    /// into a registered manual port.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_viz_push_frame(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let source = str_param(params, "source")?;
        match self.ui.manual_ports.get(source) {
            Some(ManualPort::Meter(p)) => {
                let frame: synvelis_workspace::MeterFrame = json_param(params, "frame")?;
                p.push(frame);
            }
            Some(ManualPort::Spectrum(p)) => {
                let frame: synvelis_workspace::SpectrumFrame = json_param(params, "frame")?;
                p.push(frame);
            }
            None => return Err(invalid(format!("no manual port {source}"))),
        }
        self.ui_pump();
        Ok(json!({ "pushed": true }))
    }

    /// `syn.viz.source_changed` — marks a source revision-changed; the
    /// next query/frame resolves stale by contract.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_viz_source_changed(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let source = str_param(params, "source")?;
        self.ui.viz.notify_source_changed(source);
        self.ui_pump();
        Ok(json!({ "changed": true }))
    }
}
