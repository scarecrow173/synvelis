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

use ruviform_ui::ipc::{ErrorBody, Event, codes};
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
use crate::screens::{InspectorRow, InspectorScreenState, TransportScreenState, main_view_id};

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
/// `ViewLink` attach/detach contract.
pub(crate) struct ViewEntry {
    window: String,
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

    /// `syn.view.attach` — attaches `view` on `window` to both UI
    /// gateways; returns the view's live generation.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_view_attach(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
        let window = str_param(params, "window_id")?.to_owned();
        let session = self.workspace.session_mut();
        // Both gateways handle re-attach internally (dock-transfer
        // detach + generation bump) — no pre-detach here.
        self.ui.controls.attach_view(session, &view, &window);
        self.ui.viz.attach_view(&view, &window);
        self.ui.views.insert(view.clone(), ViewEntry { window });
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

    /// `syn.view.detach` — contractual cancel of in-flight gestures +
    /// generation bump; project/audio/render are untouched.
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
        let session = self.workspace.session_mut();
        self.ui.controls.detach_view(session, &view, &reason);
        self.ui.viz.detach_view(&view);
        self.ui_pump();
        self.push_notify(
            "syn.view.state",
            json!({ "view_id": view, "attached": false }),
        );
        Ok(json!({ "view_id": view, "attached": false }))
    }

    /// `syn.view.close` — drops the view's UI state on both gateways.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or gateway refusal.
    pub fn op_view_close(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = str_param(params, "view_id")?.to_owned();
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

    /// `syn.view.list` — the attached-view registry.
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
                    "generation": self.ui.controls.view_state(v).map_or(0, |(_, g, _)| g),
                })
            })
            .collect();
        json!({ "views": views })
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
