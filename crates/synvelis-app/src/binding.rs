//! [`ViewBinding`] — the non-RT typed view-side adapter (SV-GPUI-01).
//!
//! A native view owns one `ViewBinding` per logical view. The adapter
//! mints `rvf.ipc.v1` [`Intent`]s for the `syn.*` op surface
//! (`syn.view.*` lifecycle, `syn.control.*` binding/commands,
//! `syn.project.*`, `syn.transport.*`, `syn.app.*`,
//! `syn.workspace.mount`) into an outbox the view drains via
//! [`ViewBinding::take_outbox`] and forwards through its driver's
//! typed `Action` channel (for GPUI: `IntentSink::send`). Inbound
//! [`Event`]s posted by the driver are demuxed by
//! [`ViewBinding::apply`] into [`BindingNotice`]s: result correlation,
//! `syn.control.receipt`/`syn.control.projection`/`syn.view.state`/
//! `syn.viz.*` channels, lifecycle, jobs, and errors.
//!
//! The adapter is deliberately a *translation layer only*: binding
//! lifecycle, the gesture state machine, receipt ordering, generation
//! guards, and subscription truth all stay owned by the backend
//! `ControlsUi`/`VizUi` gateways — the same authority the `WebView`
//! frontend's `ViewLink`/`CommandBinding` consumed. The one piece of
//! epoch state the adapter owns is the `generation` reported by
//! `syn.view.attach`, which it applies to every subsequent
//! [`DispatchContext`] it mints (the RF#383 `ViewLink.attach`
//! generation seam).
//!
//! **Dependency direction:** this module is GPUI/OS/DOM/WebView-free —
//! it speaks [`Intent`]/[`Event`] only. Driver types never appear in
//! its API; `window` is the opaque wire `window_id` string (the native
//! path uses the driver `WindowId` display form, `w<N>`).
//!
//! **RT contract:** non-RT only — driven on the UI thread.

use std::collections::{HashMap, VecDeque};

use serde::Deserialize;
use serde_json::{Value, json};

use ruviform_ui::ipc::{ErrorBody, Event, Intent, JobState, LifecycleState};
use synvelis_workspace::controls_ui::{
    CommandIntent, CommandReceipt, ControlValue, DispatchContext, GestureCancelReason,
};

/// `notify` payload `data.channel` value for command receipts.
const CHANNEL_RECEIPT: &str = "syn.control.receipt";
/// `notify` payload `data.channel` value for binding projections.
const CHANNEL_PROJECTION: &str = "syn.control.projection";
/// `notify` payload `data.channel` value for view lifecycle pushes.
const CHANNEL_VIEW_STATE: &str = "syn.view.state";
/// `notify` payload `data.channel` value for viz frames.
const CHANNEL_VIZ_FRAME: &str = "syn.viz.frame";
/// `notify` payload `data.channel` value for viz events.
const CHANNEL_VIZ_EVENT: &str = "syn.viz.event";

/// A typed mirror of the `syn.control.projection` row wire shape
/// (produced by `push_subscription_rows` in [`crate::ui_ops`]).
///
/// `phase` stays a `String`: it mirrors `CommandPhase`'s kebab-case
/// wire form and the binding must not fail a projection on a phase
/// value a newer backend adds. `observed` stays [`Value`]: its shape
/// varies by `kind` (`param` vs `transport`). Other fields are the
/// contract's stable vocabulary.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProjectionRow {
    /// `"param"` or `"transport"`.
    pub kind: String,
    /// The binding's displayed phase (`CommandPhase` wire form).
    pub phase: String,
    /// The view's current generation epoch.
    pub generation: u32,
    /// Whether the view is attached.
    pub attached: bool,
    /// Whether `(view, target)` is subscribed.
    pub subscribed: bool,
    /// Whether the newest receipt window timed out.
    pub timed_out: bool,
    /// The in-flight preview value, if a gesture is previewing.
    pub preview: Option<ControlValue>,
    /// The newest receipt seen for this binding.
    pub last_receipt: Option<CommandReceipt>,
    /// The last landed `StateProjection` (`revision` + `value` +
    /// `provenance`) — the displayed truth.
    pub observed: Option<Value>,
    /// Backend observation sequence (stale-projection guard).
    pub observed_seq: u64,
}

/// What [`ViewBinding::apply`] extracted from one inbound [`Event`].
#[derive(Debug)]
pub enum BindingNotice {
    /// Synchronous reply to an intent this binding minted (`None` op
    /// when the id is not one of ours — callers may still inspect it).
    CallResult {
        /// The intent's correlation id.
        id: String,
        /// The `syn.*` op the id was minted for, if ours.
        op: Option<String>,
        /// Whether the call succeeded.
        ok: bool,
        /// Success payload.
        data: Option<Value>,
        /// Failure payload.
        error: Option<ErrorBody>,
    },
    /// `syn.view.attach` succeeded — `generation` is the adopted epoch.
    Attached {
        /// The authoritative generation from the attach reply.
        generation: u32,
    },
    /// `syn.view.detach` succeeded.
    Detached,
    /// `syn.view.close` succeeded — the binding is terminal.
    Closed,
    /// A `syn.control.receipt` for this view.
    Receipt {
        /// The logical target the receipt belongs to.
        target: String,
        /// The typed command outcome.
        receipt: CommandReceipt,
    },
    /// A `syn.control.projection` for this view.
    Projection {
        /// The logical target the row belongs to.
        target: String,
        /// The typed binding-state row.
        row: ProjectionRow,
    },
    /// A `syn.view.state` push for this view.
    ViewState {
        /// Current attached state.
        attached: bool,
        /// Whether the view was closed.
        closed: bool,
    },
    /// The latest-wins app-state push (`{app, project, capabilities}`).
    AppState {
        /// The push payload.
        data: Value,
    },
    /// A `syn.viz.frame` push for this view.
    VizFrame {
        /// The bound surface id.
        surface_id: String,
        /// `meter`/`spectrum`.
        kind: String,
        /// The frame payload (shape varies by `kind`).
        frame: Value,
    },
    /// A `syn.viz.event` push.
    VizEvent {
        /// The event payload.
        event: Value,
    },
    /// A recognized `syn.*` channel payload the typed mirror could not
    /// parse, or a channel with no typed notice (forward-compat
    /// escape hatch — never silently dropped).
    Channel {
        /// The `data.channel` value.
        channel: String,
        /// The `data.data` payload.
        data: Value,
    },
    /// A `lifecycle` host notification (`Closing` on close request).
    Lifecycle {
        /// The lifecycle state.
        state: LifecycleState,
    },
    /// A job lifecycle event.
    Job {
        /// The job id.
        job: u64,
        /// The job state.
        state: JobState,
        /// Progress counters (`state == Progress`).
        done: Option<u64>,
        /// Progress total (`state == Progress`).
        total: Option<u64>,
        /// Progress detail label (`state == Progress`).
        detail: Option<String>,
        /// Result payload (`state == Finished`).
        data: Option<Value>,
        /// Failure payload (`state == Failed`).
        error: Option<ErrorBody>,
    },
    /// An uncorrelated protocol error.
    Error {
        /// The error body.
        error: ErrorBody,
    },
    /// The event carried nothing for this binding (a notify addressed
    /// to another view, or an unknown channel).
    Ignored,
}

/// One logical view's typed adapter to the `syn.*` op surface.
///
/// Intent ids, command ids, gesture ids, and per-control sequences are
/// minted view-locally (`<view>-i<n>`, `<view>-c<n>`, `<view>-g<n>`) so
/// they stay unique inside the view's scope and readable in traces.
///
/// **RT contract:** non-RT only.
pub struct ViewBinding {
    view: String,
    window: String,
    seq: u64,
    gesture_seq: u64,
    control_seq: HashMap<String, u64>,
    /// `intent.id → op` for result correlation.
    pending: HashMap<String, String>,
    outbox: VecDeque<Intent>,
    /// The authoritative epoch — `0` until `syn.view.attach` replies.
    generation: u32,
    /// Producer clock (ms) stamped into `DispatchContext.issued_at`.
    issued_at: u64,
    attached: bool,
    closed: bool,
}

impl ViewBinding {
    /// Creates the binding for `view` hosted on `window` (the wire
    /// `window_id` string — native views pass the driver `WindowId`
    /// display form so `syn.view.list` lines up with driver logs).
    #[must_use]
    pub fn new(view: impl Into<String>, window: impl Into<String>) -> Self {
        Self {
            view: view.into(),
            window: window.into(),
            seq: 0,
            gesture_seq: 0,
            control_seq: HashMap::new(),
            pending: HashMap::new(),
            outbox: VecDeque::new(),
            generation: 0,
            issued_at: 0,
            attached: false,
            closed: false,
        }
    }

    /// The view id this binding mints `view_id`/`origin_view` for.
    #[must_use]
    pub fn view_id(&self) -> &str {
        &self.view
    }

    /// The wire `window_id` this binding reports.
    #[must_use]
    pub fn window_id(&self) -> &str {
        &self.window
    }

    /// The authoritative generation epoch adopted from the attach
    /// reply (`0` before attach).
    #[must_use]
    pub const fn generation(&self) -> u32 {
        self.generation
    }

    /// Whether `syn.view.attach` succeeded and no detach/close landed.
    #[must_use]
    pub const fn attached(&self) -> bool {
        self.attached
    }

    /// Whether `syn.view.close` succeeded — the binding is terminal.
    #[must_use]
    pub const fn closed(&self) -> bool {
        self.closed
    }

    /// Sets the producer clock (ms) stamped into
    /// `DispatchContext.issued_at` — views drive it from the driver
    /// `Tick`, never wall-clock.
    pub fn set_clock(&mut self, now_ms: u64) {
        self.issued_at = now_ms;
    }

    // -- view lifecycle ---------------------------------------------------

    /// `syn.view.attach` — attaches this view on its window.
    /// Returns the intent's correlation id.
    pub fn attach(&mut self) -> String {
        self.push(
            "syn.view.attach",
            json!({ "view_id": self.view, "window_id": self.window }),
        )
    }

    /// `syn.view.detach` — contractual gesture cancel + generation
    /// bump (`reason` defaults to `"unmount"`; see
    /// `GestureCancelReason` wire vocabulary).
    pub fn detach(&mut self, reason: &str) -> String {
        self.push(
            "syn.view.detach",
            json!({ "view_id": self.view, "reason": reason }),
        )
    }

    /// `syn.view.close` — drops the view's UI state entirely.
    pub fn close(&mut self) -> String {
        self.push("syn.view.close", json!({ "view_id": self.view }))
    }

    /// `syn.view.list` — the attached-view registry.
    pub fn view_list(&mut self) -> String {
        self.push("syn.view.list", json!({}))
    }

    // -- control binding + commands ----------------------------------------

    /// `syn.control.subscribe` — binds `(view, target)`; the backend
    /// pushes the initial projection row.
    pub fn subscribe(&mut self, target: &str) -> String {
        self.push(
            "syn.control.subscribe",
            json!({ "view_id": self.view, "target": target }),
        )
    }

    /// `syn.control.unsubscribe` — drops the subscription.
    pub fn unsubscribe(&mut self, target: &str) -> String {
        self.push(
            "syn.control.unsubscribe",
            json!({ "view_id": self.view, "target": target }),
        )
    }

    /// `syn.control.targets` — the bounded bindable-target listing.
    pub fn control_targets(&mut self) -> String {
        self.push("syn.control.targets", json!({}))
    }

    /// Gesture `begin` — returns the consumer-owned `gesture_id`
    /// (`<view>-g<n>`) preview/commit/cancel reuse.
    pub fn gesture_begin(
        &mut self,
        target: impl Into<String>,
        control_id: impl Into<String>,
        value: ControlValue,
    ) -> String {
        let target = target.into();
        let control_id = control_id.into();
        self.gesture_seq += 1;
        let gesture_id = format!("{}-g{}", self.view, self.gesture_seq);
        let sequence = self.control_sequence(&control_id);
        self.dispatch(&CommandIntent::Begin {
            target,
            control_id,
            sequence,
            gesture_id: gesture_id.clone(),
            origin_view: Some(self.view.clone()),
            atomic: None,
            value,
        });
        gesture_id
    }

    /// Gesture `preview` — the only coalescible command; never a
    /// receipt.
    pub fn gesture_preview(
        &mut self,
        gesture_id: &str,
        target: impl Into<String>,
        control_id: impl Into<String>,
        value: ControlValue,
    ) {
        let control_id = control_id.into();
        let sequence = self.control_sequence(&control_id);
        self.dispatch(&CommandIntent::Preview {
            target: target.into(),
            control_id,
            sequence,
            gesture_id: gesture_id.to_string(),
            origin_view: Some(self.view.clone()),
            atomic: None,
            value,
            coalescible: true,
        });
    }

    /// Gesture `commit` — the final value; receipt-expected. Returns
    /// the minted `command_id` for receipt correlation.
    pub fn gesture_commit(
        &mut self,
        gesture_id: &str,
        target: impl Into<String>,
        control_id: impl Into<String>,
        value: ControlValue,
    ) -> String {
        let control_id = control_id.into();
        let sequence = self.control_sequence(&control_id);
        self.dispatch(&CommandIntent::Commit {
            target: target.into(),
            control_id,
            sequence,
            gesture_id: gesture_id.to_string(),
            origin_view: Some(self.view.clone()),
            atomic: None,
            value,
        })
    }

    /// Gesture `cancel` — discards the uncommitted preview.
    pub fn gesture_cancel(
        &mut self,
        gesture_id: &str,
        target: impl Into<String>,
        control_id: impl Into<String>,
        reason: GestureCancelReason,
    ) -> String {
        let control_id = control_id.into();
        let sequence = self.control_sequence(&control_id);
        self.dispatch(&CommandIntent::Cancel {
            target: target.into(),
            control_id,
            sequence,
            gesture_id: gesture_id.to_string(),
            origin_view: Some(self.view.clone()),
            atomic: None,
            reason,
        })
    }

    /// Discrete `action` — non-droppable, receipt-expected (for
    /// example `play` on the `transport` target). Returns the minted
    /// `command_id`.
    pub fn control_action(
        &mut self,
        target: impl Into<String>,
        control_id: impl Into<String>,
        action: &str,
    ) -> String {
        let control_id = control_id.into();
        let sequence = self.control_sequence(&control_id);
        self.dispatch(&CommandIntent::Action {
            target: target.into(),
            control_id,
            sequence,
            origin_view: Some(self.view.clone()),
            atomic: None,
            action: action.to_string(),
            source: None,
        })
    }

    // -- project / transport / app ------------------------------------------

    /// `syn.project.new` — create a bundle at `path`.
    pub fn project_new(&mut self, path: &str, name: Option<&str>) -> String {
        self.push("syn.project.new", json!({ "path": path, "name": name }))
    }

    /// `syn.project.open` — open the bundle at `path`.
    pub fn project_open(&mut self, path: &str) -> String {
        self.push("syn.project.open", json!({ "path": path }))
    }

    /// `syn.project.save` — save the live project to the open bundle.
    pub fn project_save(&mut self) -> String {
        self.push("syn.project.save", json!({}))
    }

    /// `syn.project.info` — project summary.
    pub fn project_info(&mut self) -> String {
        self.push("syn.project.info", json!({}))
    }

    /// `syn.transport.set` — set transport rolling.
    pub fn transport_set(&mut self, playing: bool) -> String {
        self.push("syn.transport.set", json!({ "playing": playing }))
    }

    /// `syn.app.info` — application/host/IPC revisions + counters.
    pub fn app_info(&mut self) -> String {
        self.push("syn.app.info", json!({}))
    }

    /// `syn.app.capabilities` — backend + feature availability.
    pub fn app_capabilities(&mut self) -> String {
        self.push("syn.app.capabilities", json!({}))
    }

    /// `syn.app.quit` — request application shutdown.
    pub fn app_quit(&mut self) -> String {
        self.push("syn.app.quit", json!({}))
    }

    /// `syn.workspace.mount` — workspace mount-point summary.
    pub fn workspace_mount(&mut self) -> String {
        self.push("syn.workspace.mount", json!({}))
    }

    // -- wire plumbing -------------------------------------------------------

    /// Drains the minted intents — the view feeds them to the driver's
    /// `Action` channel (GPUI: `IntentSink::send`; headless: inject as
    /// `DriverEvent::Windowed { event: Action(..) }`).
    pub fn take_outbox(&mut self) -> Vec<Intent> {
        self.outbox.drain(..).collect()
    }

    /// Demuxes one inbound [`Event`] into [`BindingNotice`]s. A
    /// `result` may yield multiple notices (the call reply plus the
    /// lifecycle transition it completed). Events not addressed to
    /// this view yield [`BindingNotice::Ignored`].
    pub fn apply(&mut self, event: &Event) -> Vec<BindingNotice> {
        match event {
            Event::Result {
                id,
                ok,
                data,
                error,
            } => self.on_result(id, *ok, data.as_ref(), error.as_ref()),
            Event::Notify { data, .. } => self.on_notify(data),
            Event::Lifecycle { state } => vec![BindingNotice::Lifecycle { state: *state }],
            Event::Job {
                job,
                state,
                done,
                total,
                detail,
                data,
                error,
            } => vec![BindingNotice::Job {
                job: *job,
                state: *state,
                done: *done,
                total: *total,
                detail: detail.clone(),
                data: data.clone(),
                error: error.clone(),
            }],
            Event::Error { error } => vec![BindingNotice::Error {
                error: error.clone(),
            }],
            // `Event` is non-exhaustive — an unknown variant has no
            // semantics this adapter understands.
            _ => vec![BindingNotice::Ignored],
        }
    }

    fn mint(&mut self, tag: &str) -> String {
        self.seq += 1;
        format!("{}-{}{}", self.view, tag, self.seq)
    }

    fn control_sequence(&mut self, control_id: &str) -> u64 {
        let seq = self.control_seq.entry(control_id.to_string()).or_insert(0);
        *seq += 1;
        *seq
    }

    fn push(&mut self, op: &str, params: Value) -> String {
        let id = self.mint("i");
        self.pending.insert(id.clone(), op.to_string());
        self.outbox.push_back(Intent::new(id.clone(), op, params));
        id
    }

    fn dispatch(&mut self, intent: &CommandIntent) -> String {
        let command_id = self.mint("c");
        let ctx = DispatchContext {
            command_id: command_id.clone(),
            view_id: Some(self.view.clone()),
            window_id: Some(self.window.clone()),
            generation: self.generation,
            issued_at: self.issued_at,
        };
        self.push(
            "syn.control.dispatch",
            json!({ "intent": intent, "ctx": ctx }),
        );
        command_id
    }

    fn on_result(
        &mut self,
        id: &str,
        ok: bool,
        data: Option<&Value>,
        error: Option<&ErrorBody>,
    ) -> Vec<BindingNotice> {
        let op = self.pending.remove(id);
        let mut notices = vec![BindingNotice::CallResult {
            id: id.to_string(),
            op: op.clone(),
            ok,
            data: data.cloned(),
            error: error.cloned(),
        }];
        if !ok {
            return notices;
        }
        match op.as_deref() {
            Some("syn.view.attach") => {
                let generation = u32::try_from(
                    data.and_then(|d| d["generation"].as_u64())
                        .unwrap_or(0)
                        .min(u64::from(u32::MAX)),
                )
                .unwrap_or(u32::MAX);
                self.generation = generation;
                self.attached = true;
                notices.push(BindingNotice::Attached { generation });
            }
            Some("syn.view.detach") => {
                self.attached = false;
                notices.push(BindingNotice::Detached);
            }
            Some("syn.view.close") => {
                self.attached = false;
                self.closed = true;
                notices.push(BindingNotice::Closed);
            }
            _ => {}
        }
        notices
    }

    fn on_notify(&self, data: &Value) -> Vec<BindingNotice> {
        let Some(channel) = data.get("channel").and_then(Value::as_str) else {
            // Channel-less pushes are the app-state broadcast.
            return vec![BindingNotice::AppState { data: data.clone() }];
        };
        let payload = data.get("data").cloned().unwrap_or(Value::Null);
        let for_me = payload.get("view_id").and_then(Value::as_str) == Some(self.view.as_str());
        match channel {
            CHANNEL_RECEIPT if for_me => {
                match serde_json::from_value::<CommandReceipt>(payload["receipt"].clone()) {
                    Ok(receipt) => vec![BindingNotice::Receipt {
                        target: payload["target"].as_str().unwrap_or_default().to_string(),
                        receipt,
                    }],
                    Err(_) => vec![Self::channel(channel, payload)],
                }
            }
            CHANNEL_PROJECTION if for_me => {
                match serde_json::from_value::<ProjectionRow>(payload["row"].clone()) {
                    Ok(row) => vec![BindingNotice::Projection {
                        target: payload["target"].as_str().unwrap_or_default().to_string(),
                        row,
                    }],
                    Err(_) => vec![Self::channel(channel, payload)],
                }
            }
            CHANNEL_VIEW_STATE if for_me => vec![BindingNotice::ViewState {
                attached: payload["attached"].as_bool().unwrap_or(false),
                closed: payload["closed"].as_bool().unwrap_or(false),
            }],
            CHANNEL_VIZ_FRAME if for_me => vec![BindingNotice::VizFrame {
                surface_id: payload["surface_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                kind: payload["kind"].as_str().unwrap_or_default().to_string(),
                frame: payload["frame"].clone(),
            }],
            CHANNEL_VIZ_EVENT => vec![BindingNotice::VizEvent {
                event: payload["event"].clone(),
            }],
            _ => vec![BindingNotice::Ignored],
        }
    }

    fn channel(channel: &str, data: Value) -> BindingNotice {
        BindingNotice::Channel {
            channel: channel.to_string(),
            data,
        }
    }
}

// ── RF387 view wire translation (`rvf.ui.command.v1`) ─────────────────
//
// The RF387 `ViewHost`/renderer lane mints window `Action` intents as
// `Intent::new(command_id, COMMAND_INTENT_OP, {command_id,
// expect_receipt, intent: <snake_case CommandIntent wire>})` and
// consumes `post_event` `Result`/`Notify` payloads correlated by
// `command_id`/`control_id`. The Synvelis wire speaks camelCase
// [`CommandIntent`] + [`DispatchContext`] over `syn.control.*` and
// `syn.control.receipt`/`syn.control.projection` notifies — so this
// adapter translates type-to-type: every inbound field is decoded
// strictly (`deny_unknown_fields`), converted per variant, and
// re-encoded by the target type's own serde. No generic key renames,
// no lossy defaults: values the peer schema cannot express are
// rejected, never silently substituted.

/// The `Intent.op` a mounted view binding uses for backend-bound
/// command intents (RF387 `COMMAND_INTENT_OP`).
pub const COMMAND_INTENT_OP: &str = "rvf.ui.command.v1";

/// A `rvf.ui.command.v1` payload that could not be decoded or is not
/// representable in the Synvelis wire vocabulary.
#[derive(Debug, Clone, PartialEq)]
pub struct WireTranslateError(pub String);

impl std::fmt::Display for WireTranslateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "view wire translation failed: {}", self.0)
    }
}

impl std::error::Error for WireTranslateError {}

fn werr(msg: impl Into<String>) -> WireTranslateError {
    WireTranslateError(msg.into())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ViewCommandParams {
    command_id: String,
    expect_receipt: bool,
    intent: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RvfControlValue {
    plain: f64,
    normalized: f64,
}

/// `begin`/`preview`/`commit` share the same wire body.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RvfGestureIntent {
    target: String,
    control_id: String,
    sequence: u64,
    gesture_id: String,
    #[serde(default)]
    origin_view: Option<String>,
    /// The peer schema's `atomic` is a definite `bool`; its wire omits
    /// the key only when the value is `false`. Our `Option<bool>`
    /// reserves `None` for callers that never said — the peer always
    /// did, so absence maps to `Some(false)`, not `None`.
    #[serde(default)]
    atomic: bool,
    value: RvfControlValue,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RvfCancelIntent {
    target: String,
    control_id: String,
    sequence: u64,
    gesture_id: String,
    #[serde(default)]
    origin_view: Option<String>,
    #[serde(default)]
    atomic: bool,
    reason: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RvfActionIntent {
    target: String,
    control_id: String,
    sequence: u64,
    #[serde(default)]
    origin_view: Option<String>,
    action: String,
    #[serde(default)]
    source: Option<String>,
}

/// Builds the Synvelis-typed gesture variant for `kind` — caller
/// guarantees `kind` is `begin`/`preview`/`commit`.
fn gesture_to_intent(kind: &str, w: RvfGestureIntent) -> CommandIntent {
    let value = ControlValue {
        plain: w.value.plain,
        normalized: w.value.normalized,
    };
    match kind {
        "begin" => CommandIntent::Begin {
            target: w.target,
            control_id: w.control_id,
            sequence: w.sequence,
            gesture_id: w.gesture_id,
            origin_view: w.origin_view,
            atomic: Some(w.atomic),
            value,
        },
        "preview" => CommandIntent::Preview {
            target: w.target,
            control_id: w.control_id,
            sequence: w.sequence,
            gesture_id: w.gesture_id,
            origin_view: w.origin_view,
            atomic: Some(w.atomic),
            value,
            coalescible: true,
        },
        _ => CommandIntent::Commit {
            target: w.target,
            control_id: w.control_id,
            sequence: w.sequence,
            gesture_id: w.gesture_id,
            origin_view: w.origin_view,
            atomic: Some(w.atomic),
            value,
        },
    }
}

/// A `rvf.ui.command.v1` intent decoded into the Synvelis-typed
/// command vocabulary, plus the binding-minted `command_id` and the
/// view's own receipt expectation.
#[derive(Debug, Clone)]
pub struct ViewCommand {
    /// Binding-minted correlation id — reused as the dispatch intent
    /// id so receipts and results correlate end-to-end.
    pub command_id: String,
    /// Whether the view's binding expects a receipt for this command.
    pub expect_receipt: bool,
    /// The translated command (Synvelis [`CommandIntent`]).
    pub intent: CommandIntent,
    /// The view id the intent carries (`origin_view`), when set —
    /// the gesture's declared origin, preferred over shell bookkeeping.
    pub origin_view: Option<String>,
}

/// Decodes the `params` of a [`COMMAND_INTENT_OP`] intent. Strict:
/// unknown envelope keys, unknown `kind`, missing fields, and wrong
/// types all fail rather than defaulting.
///
/// # Errors
/// [`WireTranslateError`] naming the offending field.
pub fn decode_view_command(params: &Value) -> Result<ViewCommand, WireTranslateError> {
    let env: ViewCommandParams = serde_json::from_value(params.clone())
        .map_err(|e| werr(format!("rvf.ui.command.v1 params: {e}")))?;
    let Value::Object(mut map) = env.intent else {
        return Err(werr("intent is not an object"));
    };
    let Some(kind) = map
        .remove("kind")
        .and_then(|v| v.as_str().map(str::to_string))
    else {
        return Err(werr("intent.kind missing or not a string"));
    };
    let intent_wire = Value::Object(map);
    let decode = |e: serde_json::Error| werr(format!("{kind} intent: {e}"));
    let (intent, origin_view) = match kind.as_str() {
        "begin" | "preview" | "commit" => {
            let w: RvfGestureIntent = serde_json::from_value(intent_wire).map_err(decode)?;
            let origin_view = w.origin_view.clone();
            (gesture_to_intent(&kind, w), origin_view)
        }
        "cancel" => {
            let w: RvfCancelIntent = serde_json::from_value(intent_wire.clone()).map_err(decode)?;
            let reason: GestureCancelReason =
                serde_json::from_value(Value::String(w.reason.clone()))
                    .map_err(|e| werr(format!("cancel.reason: {e}")))?;
            (
                CommandIntent::Cancel {
                    target: w.target,
                    control_id: w.control_id,
                    sequence: w.sequence,
                    gesture_id: w.gesture_id,
                    origin_view: w.origin_view.clone(),
                    atomic: Some(w.atomic),
                    reason,
                },
                w.origin_view,
            )
        }
        "action" => {
            let w: RvfActionIntent = serde_json::from_value(intent_wire).map_err(decode)?;
            (
                CommandIntent::Action {
                    target: w.target,
                    control_id: w.control_id,
                    sequence: w.sequence,
                    origin_view: w.origin_view.clone(),
                    // The peer's `Action` carries no `atomic` — `None`
                    // (never said), not `Some(false)`.
                    atomic: None,
                    action: w.action,
                    source: w.source,
                },
                w.origin_view,
            )
        }
        other => return Err(werr(format!("unsupported intent kind {other:?}"))),
    };
    Ok(ViewCommand {
        command_id: env.command_id,
        expect_receipt: env.expect_receipt,
        intent,
        origin_view,
    })
}

/// Mints the `syn.control.dispatch` [`Intent`] a decoded view command
/// becomes. `command_id` is reused as the intent id so result/receipt
/// correlation end-to-end stays on the binding's namespace; `view_id`,
/// `window_id`, `generation`, and `issued_at` are filled by the shell
/// (the owner of those contracts) into a typed [`DispatchContext`].
#[must_use]
pub fn view_command_dispatch(
    cmd: &ViewCommand,
    view_id: &str,
    window_id: &str,
    generation: u32,
    issued_at: u64,
) -> Intent {
    let ctx = DispatchContext {
        command_id: cmd.command_id.clone(),
        view_id: Some(view_id.to_string()),
        window_id: Some(window_id.to_string()),
        generation,
        issued_at,
    };
    Intent::new(
        cmd.command_id.clone(),
        "syn.control.dispatch",
        json!({ "intent": cmd.intent, "ctx": ctx }),
    )
}

/// Translates one app-outbox event into the event a mounted view
/// consumes, when the channel is one it understands. Returns
/// `(view_id, event)` — the caller posts to the view's window (the
/// original event is still broadcast for `ViewBinding` consumers).
///
/// - `syn.control.receipt` → `Result{ id: commandId, ok, data:
///   {status, revision?, detail?}, error? }`. `status` is the exact
///   Synvelis status string so consumers that distinguish
///   accepted/queued/applied keep the vocabulary; `ok` is false only
///   for the terminal failures `rejected`/`unsupported`.
/// - `syn.control.projection` → `Notify{ seq, data: {control_id:
///   target, target, revision, value, provenance?} }`. `control_id`
///   is the subscription's logical target — the mounted slice binds
///   controls under their target name; a view whose `control_id`
///   differs from its `target` must register its own map (follow-up).
///   Returns `None` when the row carries no `observed` state yet.
#[must_use]
pub fn notify_to_view_event(event: &Event) -> Option<(String, Event)> {
    let Event::Notify { seq, data } = event else {
        return None;
    };
    let channel = data.get("channel")?.as_str()?;
    let payload = data.get("data")?;
    let view_id = payload.get("view_id")?.as_str()?.to_string();
    match channel {
        CHANNEL_RECEIPT => {
            let receipt: CommandReceipt =
                serde_json::from_value(payload.get("receipt")?.clone()).ok()?;
            let status = serde_json::to_value(receipt.status)
                .ok()?
                .as_str()
                .unwrap_or_default()
                .to_string();
            let ok = !matches!(status.as_str(), "rejected" | "unsupported");
            let mut body = json!({ "status": status });
            if let Some(rev) = receipt.revision {
                body["revision"] = Value::from(rev);
            }
            if let Some(detail) = &receipt.detail {
                body["detail"] = Value::from(detail.clone());
            }
            let error = (!ok).then(|| {
                ErrorBody::new(
                    status.as_str(),
                    receipt.detail.clone().unwrap_or_else(|| status.clone()),
                )
            });
            Some((
                view_id,
                Event::Result {
                    id: receipt.command_id,
                    ok,
                    data: Some(body),
                    error,
                },
            ))
        }
        CHANNEL_PROJECTION => {
            let target = payload.get("target")?.as_str()?;
            let observed = payload.get("row")?.get("observed")?;
            if observed.is_null() {
                return None;
            }
            let revision = observed.get("revision")?.clone();
            let mut body = json!({
                "control_id": target,
                "target": target,
                "revision": revision,
                "value": observed.get("value").cloned().unwrap_or(Value::Null),
            });
            if let Some(p) = observed.get("provenance") {
                body["provenance"] = p.clone();
            }
            Some((
                view_id,
                Event::Notify {
                    seq: *seq,
                    data: body,
                },
            ))
        }
        _ => None,
    }
}
