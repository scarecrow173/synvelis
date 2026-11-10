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
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
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

use synvelis_player::{
    LatencyBasis, LatencyComponent, LatencyQuality, LatencyReport, LowLatencyReadiness,
};
use synvelis_project::vcs::{RevisionSelector, VcsCancel};
use synvelis_project::{
    BusId, ClockEpochId, ExplicitChannelMap, LaneId, MusicalTick, ParameterTargetId,
    ProbeCaptureSegment, ProbeMode, ProbeObservation, ProbePortError, ProbeStatusReceipt,
    ProbeSubscriptionKey, ProbeTracePort, ProcessorId, ProjectRevision, ProjectStore,
    RoutingConnectionId, SemanticPortId, SemanticProbePoint, SendId, StripId, SubscriptionKind,
    TickTimeMapping, TraceSlot, TrackId, is_issuable_raw,
};
use synvelis_recording::{
    ArmState, ChannelMap, HardwareDirectMonitor, HardwareDirectState, MonitorState,
};
use synvelis_workspace::VizSourceId;
use synvelis_workspace::controls_ui::{
    CommandIntent, ControlsUi, DispatchContext, GestureCancelReason, ParamProjection,
    TRANSPORT_POSITION_TARGET, TRANSPORT_TARGET,
};
use synvelis_workspace::viz_ui::{
    ManualMeter, ManualSpectrum, MemoryWaveform, MeterPort, SpectrumPort, VizUi, VizViewEvent,
    WaveformPort,
};
use synvelis_workspace::{
    CancelReason, EndpointRequest, FollowSource, InspectionMode, LinkGroupId, PinSide,
    ProbeDrainReport, ProbeTraceError, RoutingGestureFixation, RoutingGestureOutcome,
    RoutingGesturePreview, RoutingPortAssessment, SignalFlowEdgeRole, SignalFlowEntityRef,
    SignalFlowInspectionFocus, SignalFlowScope, SignalFlowView, SignalFlowViewOptions,
    SignalFlowViewState, VcsDiffView, VcsEndpointView, VcsHistoryView, VcsSession, VcsUiError,
    VcsViewMode, ViewType, signal_flow_graph, vcs_history_view, visual_fold, visual_move_node,
    visual_set_group, visual_set_link_focus, visual_set_pan, visual_set_scope, visual_set_zoom,
};

use crate::app::SynvelisApp;
use crate::monitoring::{MonitorSessionParams, MonitorTransportAction, PunchSide, TrackPatch};
use crate::screens::{
    DockViewInfo, InspectorRow, InspectorScreenState, TransportScreenState, dock_view_info,
    main_view_id, mixer_strip_targets, vcs_equality_str, vcs_freshness_reason, vcs_freshness_str,
    vcs_kind_str, vcs_mode_str, vcs_status_detail, vcs_status_str,
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
    /// Last `ControlSession::probe_drain` report — session-scoped
    /// evidence surfaced by `syn.probe.state` (SF-04 slice 4).
    pub(crate) last_probe_drain: ProbeDrainReport,
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
            last_probe_drain: ProbeDrainReport::default(),
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

/// `syn.probe.state` trace-row cap — projections stay bounded like
/// every other UI-facing listing.
const PROBE_TRACE_CAP: usize = 64;

/// `ProbeDrainReport` → wire shape.
fn probe_drain_json(report: ProbeDrainReport) -> Value {
    json!({
        "updates": report.updates,
        "uncorrelated": report.uncorrelated,
        "recorded": report.recorded,
        "unrouted": report.unrouted,
    })
}

/// `ProbeStatusReceipt` → wire shape — the runtime evidence verbatim
/// (`clock` has no contract serde, so it projects as debug text).
fn probe_status_json(status: &ProbeStatusReceipt) -> Value {
    json!({
        "kind": status.kind,
        "generation": status.generation,
        "sample_position": status.sample_position,
        "clock": status.clock.as_ref().map(|c| format!("{c:?}")),
    })
}

/// `ProbeCaptureSegment` → wire shape — closed segments only; the
/// ledger never exposes open segments.
fn probe_segment_json(segment: &ProbeCaptureSegment) -> Value {
    json!({
        "capture_session_id": segment.capture_session_id,
        "segment_index": segment.segment_index,
        "point": segment.point,
        "active_plan_generation": segment.active_plan_generation,
        "clock_context": segment.clock_context.as_ref().map(|c| format!("{c:?}")),
        "frame_range": segment.frame_range,
        "discontinuities": &segment.discontinuities,
        "completeness": segment.completeness,
        "ended_by": segment.ended_by,
    })
}

/// `TraceSlot<ProbeObservation>` → wire shape — the slot state
/// verbatim plus the observation payload as debug text (the
/// observation vocabulary has no contract serde; `Missing`/
/// `Disabled`/`Lost`/`Measuring` stay distinguishable).
fn probe_observed_json(slot: &TraceSlot<ProbeObservation>) -> Value {
    let state = match slot {
        TraceSlot::Present(_) => "present",
        TraceSlot::Missing => "missing",
        TraceSlot::Unsupported => "unsupported",
        TraceSlot::Dropped => "dropped",
        TraceSlot::Expired => "expired",
        _ => "unknown",
    };
    json!({
        "state": state,
        "value": slot.present().map(|o| format!("{o:?}")),
    })
}

/// `ProbeTraceError` → wire error — an unknown target is a param
/// fault; named port rejections keep their contract message;
/// provider-internal `Failed` maps to `SERVICE_ERROR`.
fn probe_trace_err(error: ProbeTraceError) -> ErrorBody {
    match error {
        ProbeTraceError::UnknownTarget(target) => {
            invalid(format!("unknown parameter target {target}"))
        }
        ProbeTraceError::Port(ProbePortError::Failed(reason)) => {
            internal(format!("probe: {reason}"))
        }
        ProbeTraceError::Port(other) => invalid(other.to_string()),
        _ => internal("probe: unknown session error"),
    }
}

fn dock_err(e: &synvelis_workspace::WorkspaceError) -> ErrorBody {
    internal(format!("dock: {e}"))
}

// -- syn.signal_flow.* helpers -------------------------------------------

/// `view_id` param — defaults to the composition root's main view.
fn sf_view_param(params: &Value) -> String {
    params
        .get("view_id")
        .and_then(Value::as_str)
        .map_or_else(main_view_id, str::to_string)
}

/// One `SignalFlowEntityRef` wire value — `{"kind": ..., "id":
/// "<tagged>"}` where kind ∈ `track|bus|strip|processor|port|
/// connection|send`.
fn sf_entity(value: &Value, path: &str) -> Result<SignalFlowEntityRef, ErrorBody> {
    let bad = |msg: &str| invalid(format!("params.{path}: {msg}"));
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("kind (string) required"))?;
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("id (string) required"))?;
    macro_rules! tagged {
        ($e:expr) => {
            $e.map_err(|_| invalid(format!("params.{path}: malformed tagged id")))?
        };
    }
    Ok(match kind {
        "track" => SignalFlowEntityRef::Track(tagged!(TrackId::from_tagged(id))),
        "bus" => SignalFlowEntityRef::Bus(tagged!(BusId::from_tagged(id))),
        "strip" => SignalFlowEntityRef::Strip(tagged!(StripId::from_tagged(id))),
        "processor" => SignalFlowEntityRef::Processor(tagged!(ProcessorId::from_tagged(id))),
        "port" => SignalFlowEntityRef::Port(tagged!(SemanticPortId::from_tagged(id))),
        "connection" => {
            SignalFlowEntityRef::Connection(tagged!(RoutingConnectionId::from_tagged(id)))
        }
        "send" => SignalFlowEntityRef::Send(tagged!(SendId::from_tagged(id))),
        other => return Err(bad(&format!("unknown entity kind {other}"))),
    })
}

fn sf_entity_param(params: &Value, key: &str) -> Result<SignalFlowEntityRef, ErrorBody> {
    sf_entity(
        params
            .get(key)
            .ok_or_else(|| invalid(format!("params.{key} required")))?,
        key,
    )
}

/// An array of entity wire values → set.
fn sf_entities(value: &Value) -> Result<BTreeSet<SignalFlowEntityRef>, ErrorBody> {
    let items = value
        .as_array()
        .ok_or_else(|| invalid("expected an array of entity objects"))?;
    items.iter().map(|v| sf_entity(v, "entity")).collect()
}

/// `scope` wire value — `"overview"|"expanded"|"local"|"history"`;
/// `local` requires `params.root` (an entity object).
fn sf_scope(value: &Value, params: &Value) -> Result<SignalFlowScope, ErrorBody> {
    match value.as_str() {
        Some("overview") => Ok(SignalFlowScope::Overview),
        Some("expanded") => Ok(SignalFlowScope::Expanded),
        Some("history") => Ok(SignalFlowScope::History),
        Some("local") => Ok(SignalFlowScope::Local {
            root: sf_entity_param(params, "root")?,
        }),
        _ => Err(invalid(
            "params.scope must be overview|expanded|local|history",
        )),
    }
}

/// `roles` wire value — array of `"main"|"sidechain"|"send"|"chain"`.
fn sf_roles(value: &Value) -> Result<BTreeSet<SignalFlowEdgeRole>, ErrorBody> {
    let items = value
        .as_array()
        .ok_or_else(|| invalid("params.roles must be an array"))?;
    items
        .iter()
        .map(|v| match v.as_str() {
            Some("main") => Ok(SignalFlowEdgeRole::Main),
            Some("sidechain") => Ok(SignalFlowEdgeRole::Sidechain),
            Some("send") => Ok(SignalFlowEdgeRole::Send),
            Some("chain") => Ok(SignalFlowEdgeRole::Chain),
            _ => Err(invalid(
                "params.roles entries must be main|sidechain|send|chain",
            )),
        })
        .collect()
}

/// Read-side per-view [`SignalFlowViewState`] — `None` for
/// unparseable `view_id`s or views holding no Signal Flow state
/// (the projection then falls back to defaults, matching the
/// pre-persistence behaviour). Never adopts or mutates.
fn sf_view_state<'a>(app: &'a SynvelisApp, view: &str) -> Option<&'a SignalFlowViewState> {
    let inst = parse_view_instance(view)?;
    app.workspace().state(inst).ok()?.signal_flow.as_ref()
}

/// One tagged `SemanticPortId` wire value.
fn sf_port(value: &Value, path: &str) -> Result<SemanticPortId, ErrorBody> {
    let bad = |msg: &str| invalid(format!("params.{path}: {msg}"));
    let s = value
        .as_str()
        .ok_or_else(|| bad("tagged port id (string) required"))?;
    SemanticPortId::from_tagged(s).map_err(|_| bad("malformed tagged port id"))
}

fn sf_port_param(params: &Value, key: &str) -> Result<SemanticPortId, ErrorBody> {
    sf_port(
        params
            .get(key)
            .ok_or_else(|| invalid(format!("params.{key} required")))?,
        key,
    )
}

/// Inverse of [`sf_entity`] — one `SignalFlowEntityRef` → wire object.
fn sf_entity_json(entity: SignalFlowEntityRef) -> Value {
    let (kind, id) = match entity {
        SignalFlowEntityRef::Track(id) => ("track", id.to_tagged()),
        SignalFlowEntityRef::Bus(id) => ("bus", id.to_tagged()),
        SignalFlowEntityRef::Strip(id) => ("strip", id.to_tagged()),
        SignalFlowEntityRef::Processor(id) => ("processor", id.to_tagged()),
        SignalFlowEntityRef::Port(id) => ("port", id.to_tagged()),
        SignalFlowEntityRef::Connection(id) => ("connection", id.to_tagged()),
        SignalFlowEntityRef::Send(id) => ("send", id.to_tagged()),
    };
    json!({ "kind": kind, "id": id })
}

/// [`SignalFlowInspectionFocus`] → wire shape.
fn signal_flow_inspection_json(focus: &SignalFlowInspectionFocus) -> Value {
    let mode = match focus.mode {
        InspectionMode::Follow => "follow",
        InspectionMode::Explore => "explore",
        InspectionMode::Pinned => "pinned",
    };
    let follow_source = focus.follow_source.map(|src| match src {
        FollowSource::View(v) => json!({ "view": format!("view-{}", v.get()) }),
        FollowSource::Group(g) => json!({ "group": g.0 }),
    });
    json!({
        "target": focus.target.map(sf_entity_json),
        "mode": mode,
        "follow_source": follow_source,
        "explore_back": focus.explore_back.len(),
        "explore_fwd": focus.explore_fwd.len(),
    })
}

/// [`RoutingGestureFixation`] → wire shape.
fn routing_fixation_json(fixation: &RoutingGestureFixation) -> Value {
    json!({
        "view": format!("view-{}", fixation.view.get()),
        "source": fixation.source.to_tagged(),
        "revision": fixation.revision.0,
        "capability_generation": fixation.capability_generation,
        "window": fixation.window.map(|w| w.to_string()),
    })
}

/// [`RoutingGesturePreview`] → wire shape.
fn routing_preview_json(preview: &RoutingGesturePreview) -> Value {
    json!({
        "candidate": preview.candidate.map(SemanticPortId::to_tagged),
        "assessment": preview.assessment.as_ref().map(routing_assessment_json),
        "dropped_target": preview.dropped_target.map(SemanticPortId::to_tagged),
    })
}

/// [`RoutingPortAssessment`] → wire shape.
fn routing_assessment_json(assessment: &RoutingPortAssessment) -> Value {
    match assessment {
        RoutingPortAssessment::Compatible => json!({ "verdict": "compatible" }),
        RoutingPortAssessment::NeedsMapping {
            source_channels,
            destination_channels,
        } => json!({
            "verdict": "needs_mapping",
            "source_channels": source_channels,
            "destination_channels": destination_channels,
        }),
        RoutingPortAssessment::Prohibited { reason } => {
            json!({ "verdict": "prohibited", "reason": reason })
        }
    }
}

/// [`RoutingGestureOutcome`] → wire shape.
fn routing_outcome_json(outcome: &RoutingGestureOutcome) -> Value {
    match outcome {
        RoutingGestureOutcome::Applied { revision } => {
            json!({ "outcome": "applied", "revision": revision.0 })
        }
        RoutingGestureOutcome::Cancelled { reason } => json!({
            "outcome": "cancelled",
            "reason": serde_json::to_value(reason).unwrap_or(Value::Null),
        }),
        RoutingGestureOutcome::Stale => json!({ "outcome": "stale" }),
    }
}

/// Wire shape for [`SignalFlowView`] — every field verbatim; ids and
/// enums render as `Debug` strings (the wire has no semantic-id
/// codec — callers match on the debug vocabulary).
fn signal_flow_view_json(view: &SignalFlowView) -> Value {
    let opt_revision = |r: Option<ProjectRevision>| r.map(|r| r.0);
    json!({
        "project_id": view.project_id.to_tagged(),
        "project_revision": view.project_revision.0,
        "view_generation": view.view_generation,
        "coverage": format!("{:?}", view.coverage),
        "scope": format!("{:?}", view.scope),
        "nodes": view.nodes.iter().map(|node| json!({
            "id": format!("{:?}", node.id),
            "kind": format!("{:?}", node.kind),
            "label": node.label,
            "state": format!("{:?}", node.state),
            "runtime": format!("{:?}", node.runtime),
            "probe": node.probe.map(|p| format!("{p:?}")),
            "muted": node.muted,
            "soloed": node.soloed,
            "processor_count": node.processor_count,
        })).collect::<Vec<_>>(),
        "edges": view.edges.iter().map(|edge| json!({
            "id": format!("{:?}", edge.id),
            "role": format!("{:?}", edge.role),
            "source": {
                "node": format!("{:?}", edge.source.node),
                "port": edge.source.port.map(SemanticPortId::to_tagged),
                "port_role": format!("{:?}", edge.source.port_role),
            },
            "target": {
                "node": format!("{:?}", edge.target.node),
                "port": edge.target.port.map(SemanticPortId::to_tagged),
                "port_role": format!("{:?}", edge.target.port_role),
            },
            "enabled": edge.enabled,
            "tap": edge.tap.map(|t| format!("{t:?}")),
        })).collect::<Vec<_>>(),
        "status": {
            "project_revision": view.status.project_revision.0,
            "engine_attached": view.status.engine_attached,
            "activation": view.status.activation.as_ref().map(|a| format!("{a:?}")),
            "planned_generation": view.status.planned_generation,
            "published_generation": view.status.published_generation,
            "observed_active_generation": view.status.observed_active_generation,
            "last_good_generation": view.status.last_good_generation,
            "fault": view.status.fault.as_ref().map(|f| format!("{f:?}")),
            "source_map_revision": opt_revision(view.status.source_map_revision),
            "source_map_generation": view.status.source_map_generation,
            "stale": view.status.stale,
            "probes": view.status.probes.iter().map(|p| json!({
                "point": format!("{:?}", p.point),
                "state": format!("{:?}", p.state),
                "requested_mode": format!("{:?}", p.requested_mode),
                "confirmed_mode": p.confirmed_mode.map(|m| format!("{m:?}")),
                "generation": p.generation,
                "project_revision": p.project_revision.0,
            })).collect::<Vec<_>>(),
        },
    })
}

// -- syn.vcs.* helpers ---------------------------------------------------

/// `params.endpoint` wire value → [`EndpointRequest`]. Kinds:
/// `live`, `saved` (`head` optional), `staged`, `committed`
/// (`expr` optional — absent means `HEAD`), `node` (`index` into the
/// view's loaded history page).
fn vcs_endpoint_request(params: &Value) -> Result<EndpointRequest, ErrorBody> {
    let endpoint = params
        .get("endpoint")
        .ok_or_else(|| invalid("params.endpoint required"))?;
    let kind = endpoint
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("params.endpoint.kind (string) required"))?;
    Ok(match kind {
        "live" => EndpointRequest::Live,
        "saved" => EndpointRequest::Saved {
            head: endpoint
                .get("head")
                .and_then(Value::as_str)
                .map(str::to_string),
        },
        "staged" => EndpointRequest::Staged,
        "committed" => EndpointRequest::Committed {
            selector: match endpoint.get("expr").and_then(Value::as_str) {
                Some(expr) => RevisionSelector::Expr(expr.to_string()),
                None => RevisionSelector::Head,
            },
        },
        "node" => EndpointRequest::Node {
            index: usize_param(endpoint, "index")?,
        },
        other => return Err(invalid(format!("unknown endpoint kind {other}"))),
    })
}

/// [`VcsUiError`] → op error: request-shape failures are caller
/// errors; provider/snapshot/diff failures stay service errors with
/// the verbatim detail preserved.
fn vcs_ui_err(e: VcsUiError) -> ErrorBody {
    match e {
        VcsUiError::InvalidRequest { .. } | VcsUiError::LimitExceeded { .. } => {
            invalid(format!("vcs: {e}"))
        }
        other => internal(format!("vcs: {other}")),
    }
}

/// Parsed `syn.vcs.visual` state action (view lifecycle `open`/`close`
/// are handled before this enum — they manage the view itself).
enum VcsOp {
    Refresh,
    LoadMore,
    SelectNode(usize),
    Pin(PinSide, EndpointRequest),
    Compare,
    ViewMode(VcsViewMode),
    Clear,
}

fn vcs_endpoint_json(e: &VcsEndpointView) -> Value {
    json!({
        "kind": vcs_kind_str(e.kind),
        "label": e.label,
        "freshness": vcs_freshness_str(&e.freshness),
        "freshness_reason": vcs_freshness_reason(&e.freshness),
        "digest": e.digest,
    })
}

fn vcs_diff_json(d: &VcsDiffView) -> Value {
    json!({
        "key_digest": d.key_digest,
        "overall": vcs_equality_str(d.overall),
        "completeness": format!("{:?}", d.completeness),
        "summary": {
            "added": d.summary.added,
            "removed": d.summary.removed,
            "modified": d.summary.modified,
            "moved": d.summary.moved,
            "reordered": d.summary.reordered,
            "reconnected": d.summary.reconnected,
            "replaced": d.summary.replaced,
            "total": d.summary.total,
            "unresolved": d.summary.unresolved,
        },
        "changes": d.changes.iter().map(|c| json!({
            "id": c.id,
            "domain": format!("{:?}", c.domain),
            "object_kind": format!("{:?}", c.object_kind),
            "stable_id": c.stable_id,
            "kind": format!("{:?}", c.kind),
            "field_count": c.field_count,
            "has_range": c.has_range,
        })).collect::<Vec<_>>(),
        "issues": d.issues.iter().map(|i| json!({
            "kind": i.kind,
            "detail": i.detail,
        })).collect::<Vec<_>>(),
        "truncated": d.truncated,
    })
}

/// Wire shape for [`VcsHistoryView`] — enums render as their
/// kebab-case vocabulary, provider/snapshot/diff detail verbatim.
fn vcs_view_json(view: &VcsHistoryView) -> Value {
    json!({
        "project_revision": view.project_revision.0,
        "status": vcs_status_str(&view.status),
        "status_detail": vcs_status_detail(&view.status),
        "backend_attached": view.backend_attached,
        "rows": view.rows.iter().map(|r| json!({
            "commit_oid": r.commit_oid,
            "short_oid": r.short_oid,
            "parents": r.parents,
            "is_merge": r.is_merge,
            "committed_at_unix": r.committed_at_unix,
            "author": r.author,
            "message_headline": r.message_headline,
            "completeness": format!("{:?}", r.completeness),
            "selected": r.selected,
        })).collect::<Vec<_>>(),
        "next_cursor": view.next_cursor,
        "selected": view.selected,
        "selection": view.selection.as_ref().map(|s| json!({
            "before": vcs_endpoint_json(&s.before),
            "after": vcs_endpoint_json(&s.after),
            "chosen_parent": s.chosen_parent,
            "view_mode": vcs_mode_str(s.view_mode),
            "diff": s.diff.as_ref().map(vcs_diff_json),
        })),
        "new_revision_available": view.new_revision_available,
        "last_error": view.last_error,
        "repository_id": view.repository_id,
    })
}

/// `"w{n}"` → [`WindowId`]; `0`/malformed → `None`.
fn parse_window_id(s: &str) -> Option<WindowId> {
    WindowId::new(s.strip_prefix('w')?.parse().ok()?)
}

/// `"view-{n}"` → [`ViewInstanceId`]; `0`/malformed → `None`.
pub(crate) fn parse_view_instance(s: &str) -> Option<ViewInstanceId> {
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
        self.reconcile_mixer_subscriptions();
        self.push_subscription_rows();
        self.push_surface_frames();
        self.capture_screen_state();
        self.drain_probe();
    }

    /// SF-04 slice 4 drain hook — the *only* session path that pulls
    /// runtime probe events into `ControlTrace.observed` slots. Runs
    /// at the `ui_pump` tail whenever a routing engine is attached;
    /// `ControlSession::probe_drain` stays the single drain owner and
    /// its report lands in [`UiOpsState::last_probe_drain`] for
    /// `syn.probe.state`. No engine → no port → nothing drains and no
    /// observation is fabricated.
    fn drain_probe(&mut self) {
        let Some(engine) = self.routing_engine.clone() else {
            return;
        };
        self.ui.last_probe_drain = engine
            .with_probe_bridge(|port| self.workspace.session_mut().control_mut().probe_drain(port));
    }

    /// Keeps the main view's strip-toggle subscriptions in step with
    /// the project (synvelis#112 slice 3): the master strip's seeded
    /// `Mute`/`Solo` targets are discovered from the control model —
    /// never hard-coded — and subscribed on first sight. The targets
    /// are well-known and cannot be removed, so the desired set is
    /// stable for a session; view close already drops per-view subs
    /// and there is deliberately no unsubscribe path here. Runs
    /// before [`Self::push_subscription_rows`] so a freshly
    /// subscribed target's first projection row lands in the same
    /// pump.
    fn reconcile_mixer_subscriptions(&mut self) {
        let view = main_view_id();
        if self
            .ui
            .views
            .get(&view)
            .is_none_or(|entry| entry.window.is_none())
        {
            return;
        }
        let desired = mixer_strip_targets(&self.workspace.session().project().control);
        for target in [desired.mute_target, desired.solo_target]
            .into_iter()
            .flatten()
        {
            if self
                .ui
                .subscriptions
                .contains(&(view.clone(), target.clone()))
            {
                continue;
            }
            let session = self.workspace.session();
            if self.ui.controls.subscribe(session, &view, &target).is_ok() {
                self.ui.subscriptions.insert((view.clone(), target));
            }
        }
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
        // Strip channel toggles (slice 3) are mixer controls, not
        // inspector candidates — the breakdown stays on the gain
        // fader regardless of tagged-id sort order.
        let strip_toggles: HashSet<String> = {
            let mixer = mixer_strip_targets(&self.workspace.session().project().control);
            [mixer.mute_target, mixer.solo_target]
                .into_iter()
                .flatten()
                .collect()
        };
        let mut param_targets: Vec<String> = self
            .ui
            .subscriptions
            .iter()
            .filter(|(v, t)| {
                v == &view
                    && t != TRANSPORT_TARGET
                    && t != TRANSPORT_POSITION_TARGET
                    && !strip_toggles.contains(t)
            })
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

    // -- syn.signal_flow.* -------------------------------------------------

    /// `syn.signal_flow.view` — the headless Signal Flow projection
    /// (SF-05 slice 1, synvelis#108). Params `{view_id?, scope?,
    /// root?, roles?, collapsed?, max_nodes?, max_edges?,
    /// expected_revision?}`; anything unspecified falls back to the
    /// per-view persisted [`SignalFlowViewState::visual`] (default
    /// when the view never ran a visual op or is not dock-registered).
    /// The response is the full [`SignalFlowView`] wire shape —
    /// coverage, revisions, generations, nodes, edges, and status
    /// verbatim.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters.
    pub fn op_signal_flow_view(&self, params: &Value) -> Result<Value, ErrorBody> {
        let view = sf_view_param(params);
        let mut options =
            sf_view_state(self, &view).map_or_else(SignalFlowViewOptions::default, |sf| {
                sf.visual.view_options(
                    crate::app::SIGNAL_FLOW_MAX_NODES,
                    crate::app::SIGNAL_FLOW_MAX_EDGES,
                )
            });
        if let Some(scope) = params.get("scope") {
            options.scope = sf_scope(scope, params)?;
        }
        if let Some(roles) = params.get("roles") {
            options.roles = sf_roles(roles)?;
        }
        if let Some(collapsed) = params.get("collapsed") {
            options.collapsed = sf_entities(collapsed)?;
        }
        if params.get("max_nodes").is_some() {
            options.max_nodes = usize_param(params, "max_nodes")?;
        }
        if params.get("max_edges").is_some() {
            options.max_edges = usize_param(params, "max_edges")?;
        }
        if params.get("expected_revision").is_some() {
            options.expected_revision =
                Some(ProjectRevision(u64_param(params, "expected_revision")?));
        }
        let evidence = self.signal_flow_runtime_evidence();
        let view_result = signal_flow_graph(self.workspace.session(), &options, evidence.as_ref());
        Ok(signal_flow_view_json(&view_result))
    }

    /// `syn.signal_flow.visual` — visual-only mutation of the per-view
    /// persisted [`SignalFlowViewState::visual`] (SF-UX-02/07). The
    /// state lives in the workspace [`ViewState`] keyed by the wire
    /// `view_id`'s [`ViewInstanceId`], so `WorkspaceManager::capture`/
    /// `restore` round-trips it per view. `params.action` ∈
    /// `move_node|set_group|fold|unfold|zoom|pan|link_focus|
    /// set_scope`; the ack echoes `{ok, view, view_generation}`.
    /// Session revision, undo depth, engine graph builds, and the
    /// activation receipt are never touched — the visual state is a
    /// pure value type by construction.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters, an unknown action, or a
    /// `view_id` that is not `"view-{n}"` shaped.
    pub fn op_signal_flow_visual(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = sf_view_param(params);
        let action = str_param(params, "action")?;
        let visual = &mut self.sf_view_state_mut(&view)?.visual;
        match action {
            "move_node" => {
                let entity = sf_entity_param(params, "entity")?;
                let x = f64_param(params, "x")?;
                let y = f64_param(params, "y")?;
                visual_move_node(visual, entity, [x, y]);
            }
            "set_group" => {
                let group = synvelis_workspace::SignalFlowVisualGroup {
                    id: u64_param(params, "group_id")?,
                    label: str_param(params, "label")?.to_string(),
                    members: sf_entities(
                        params
                            .get("members")
                            .ok_or_else(|| invalid("params.members required"))?,
                    )?,
                    collapsed: params
                        .get("collapsed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                };
                visual_set_group(visual, group);
            }
            "fold" | "unfold" => {
                let entity = sf_entity_param(params, "entity")?;
                let folded = action == "fold"
                    && params
                        .get("folded")
                        .and_then(Value::as_bool)
                        .unwrap_or(true);
                visual_fold(visual, entity, folded);
            }
            "zoom" => visual_set_zoom(visual, f64_param(params, "zoom")?),
            "pan" => {
                let x = f64_param(params, "x")?;
                let y = f64_param(params, "y")?;
                visual_set_pan(visual, [x, y]);
            }
            "link_focus" => {
                let focus = match params.get("entity") {
                    None | Some(Value::Null) => None,
                    Some(value) => Some(sf_entity(value, "entity")?),
                };
                visual_set_link_focus(visual, focus);
            }
            "set_scope" => {
                let scope = sf_scope(
                    params
                        .get("scope")
                        .ok_or_else(|| invalid("params.scope required"))?,
                    params,
                )?;
                visual_set_scope(visual, scope);
            }
            other => {
                return Err(invalid(format!(
                    "unknown signal_flow visual action {other}"
                )));
            }
        }
        Ok(json!({
            "ok": true,
            "view": view,
            "view_generation": visual.visual_revision,
        }))
    }

    /// `view_id` → dock-registered [`ViewInstanceId`] for the
    /// `syn.signal_flow.*` mutation/inspection/routing ops — the
    /// view is adopted into the workspace's detached pool on first
    /// use (kind/title from [`dock_view_info`] when the id is a
    /// composition-root view, [`ViewType::SignalFlow`] otherwise) so
    /// every per-view Signal Flow state lands in the persisted
    /// [`ViewState`] (SF-UX-07).
    fn sf_view_instance(&mut self, view: &str) -> Result<ViewInstanceId, ErrorBody> {
        let inst = parse_view_instance(view)
            .ok_or_else(|| invalid(format!("params.view_id must be \"view-{{n}}\", got {view}")))?;
        if !self.workspace.registry().contains(inst) {
            let (kind, title) =
                dock_view_info(view).map_or((ViewType::SignalFlow, view), |i| (i.kind, i.title));
            self.workspace
                .adopt_view(inst, kind, title, None)
                .map_err(|e| dock_err(&e))?;
        }
        Ok(inst)
    }

    /// Mutable per-view [`SignalFlowViewState`] — adopts the view
    /// when needed, then returns (creating on first use) its
    /// workspace-owned state.
    fn sf_view_state_mut(&mut self, view: &str) -> Result<&mut SignalFlowViewState, ErrorBody> {
        let inst = self.sf_view_instance(view)?;
        Ok(self
            .workspace
            .state_mut(inst)
            .map_err(|e| dock_err(&e))?
            .signal_flow
            .get_or_insert_with(SignalFlowViewState::default))
    }

    /// `syn.signal_flow.inspection` — the view-local Signal Flow
    /// inspection focus (SF-UX-04). `params.action` ∈
    /// `inspect|set_mode|set_follow_source|back|forward`:
    ///
    /// - `inspect` takes `entity` (the `sf_entity` wire object) and
    ///   enters Explore mode, pushing the previous target onto the
    ///   session back-stack.
    /// - `set_mode` takes `mode` ∈ `follow|explore|pinned`.
    /// - `set_follow_source` takes `follow_source` — `null` clears,
    ///   `{"view": "view-{n}"}` or `{"group": <u64>}` narrows Follow.
    /// - `back`/`forward` walk the Explore history.
    ///
    /// The response echoes the resulting inspection focus. Link
    /// delivery retargets only Follow-mode focus whose source filter
    /// matches — Explore/Pinned are untouched, and the editing
    /// selection is never modified.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or an unknown action.
    pub fn op_signal_flow_inspection(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = sf_view_param(params);
        let inst = self.sf_view_instance(&view)?;
        let action = str_param(params, "action")?;
        match action {
            "inspect" => {
                let entity = sf_entity_param(params, "entity")?;
                self.workspace
                    .signal_flow_inspect(inst, entity)
                    .map_err(|e| dock_err(&e))?;
            }
            "set_mode" => {
                let mode = match str_param(params, "mode")? {
                    "follow" => InspectionMode::Follow,
                    "explore" => InspectionMode::Explore,
                    "pinned" => InspectionMode::Pinned,
                    other => {
                        return Err(invalid(format!(
                            "params.mode must be follow|explore|pinned, got {other}"
                        )));
                    }
                };
                self.workspace
                    .signal_flow_set_inspection_mode(inst, mode)
                    .map_err(|e| dock_err(&e))?;
            }
            "set_follow_source" => {
                let source = match params.get("follow_source") {
                    None | Some(Value::Null) => None,
                    Some(value) => {
                        if let Some(v) = value.get("view").and_then(Value::as_str) {
                            let view_inst = parse_view_instance(v).ok_or_else(|| {
                                invalid(format!(
                                    "params.follow_source.view must be \"view-{{n}}\", got {v}"
                                ))
                            })?;
                            Some(FollowSource::View(view_inst))
                        } else if let Some(g) = value.get("group").and_then(Value::as_u64) {
                            Some(FollowSource::Group(LinkGroupId(g)))
                        } else {
                            return Err(invalid(
                                "params.follow_source must be null|{\"view\": ...}|{\"group\": n}",
                            ));
                        }
                    }
                };
                self.workspace
                    .signal_flow_set_follow_source(inst, source)
                    .map_err(|e| dock_err(&e))?;
            }
            "back" => {
                self.workspace
                    .signal_flow_explore_back(inst)
                    .map_err(|e| dock_err(&e))?;
            }
            "forward" => {
                self.workspace
                    .signal_flow_explore_forward(inst)
                    .map_err(|e| dock_err(&e))?;
            }
            other => {
                return Err(invalid(format!(
                    "unknown signal_flow inspection action {other}"
                )));
            }
        }
        let state = self
            .workspace
            .state(inst)
            .map_err(|e| dock_err(&e))?
            .signal_flow
            .clone()
            .unwrap_or_default();
        Ok(json!({
            "ok": true,
            "view": view,
            "inspection": signal_flow_inspection_json(&state.inspection),
        }))
    }

    /// `syn.signal_flow.routing` — the port-drag routing gesture
    /// (SF-UX-03). `params.action` ∈ `begin|hover|drop|confirm|cancel|
    /// state`:
    ///
    /// - `begin` takes `port` (tagged `SemanticPortId`) — fixes view,
    ///   source port, project revision, port-capability generation
    ///   and window epoch.
    /// - `hover` takes `port` (tagged id, or `null` for empty canvas)
    ///   and returns the dry-run assessment — `compatible`,
    ///   `needs_mapping` (channel layouts differ; an explicit
    ///   `channel_map` is required to commit), or `prohibited`.
    /// - `drop` fixes the hovered candidate as the immutable target;
    ///   dropping on empty canvas cancels. Returns the fixed port or
    ///   `null`.
    /// - `confirm` takes optional `channel_map` (`{"entries": [...]}`)
    ///   — the explicit channel-conversion consent. One atomic
    ///   `ProjectOp::Routing` commit = one undo boundary; stale
    ///   revision/capability/window outcomes return `stale` and never
    ///   retarget.
    /// - `cancel` takes optional `reason` ∈ `escape|pointer-cancel|
    ///   window-closed|target-deleted|input-scope-lost` (default
    ///   `escape`) — discards the preview, project untouched.
    /// - `state` returns the running gesture's fixation + preview.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters, an unknown action, or a
    /// workspace refusal (no gesture, gesture already active,
    /// mid-transfer, non-stale project rejection).
    pub fn op_signal_flow_routing(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view = sf_view_param(params);
        let inst = self.sf_view_instance(&view)?;
        let action = str_param(params, "action")?;
        match action {
            "begin" => {
                let port = sf_port_param(params, "port")?;
                let fixation = self
                    .workspace
                    .routing_gesture_begin(inst, port)
                    .map_err(|e| dock_err(&e))?;
                Ok(json!({
                    "ok": true,
                    "view": view,
                    "fixation": routing_fixation_json(&fixation),
                }))
            }
            "hover" => {
                let port = match params.get("port") {
                    None | Some(Value::Null) => None,
                    Some(value) => Some(sf_port(value, "port")?),
                };
                let assessment = self
                    .workspace
                    .routing_gesture_hover(inst, port)
                    .map_err(|e| dock_err(&e))?;
                Ok(json!({
                    "ok": true,
                    "view": view,
                    "assessment": assessment.as_ref().map(routing_assessment_json),
                }))
            }
            "drop" => {
                let dropped = self
                    .workspace
                    .routing_gesture_drop(inst)
                    .map_err(|e| dock_err(&e))?;
                Ok(json!({
                    "ok": true,
                    "view": view,
                    "dropped": dropped.map(SemanticPortId::to_tagged),
                }))
            }
            "confirm" => {
                let channel_map = match params.get("channel_map") {
                    None | Some(Value::Null) => None,
                    Some(value) => Some(
                        serde_json::from_value::<ExplicitChannelMap>(value.clone())
                            .map_err(|e| invalid(format!("params.channel_map: {e}")))?,
                    ),
                };
                let outcome = self
                    .workspace
                    .routing_gesture_confirm(inst, channel_map.as_ref())
                    .map_err(|e| dock_err(&e))?;
                Ok(json!({
                    "ok": true,
                    "view": view,
                    "outcome": routing_outcome_json(&outcome),
                }))
            }
            "cancel" => {
                let reason = match params.get("reason").and_then(Value::as_str) {
                    None | Some("escape") => CancelReason::Escape,
                    Some("pointer-cancel") => CancelReason::PointerCancel,
                    Some("window-closed") => CancelReason::WindowClosed,
                    Some("target-deleted") => CancelReason::TargetDeleted,
                    Some("input-scope-lost") => CancelReason::InputScopeLost,
                    Some(other) => {
                        return Err(invalid(format!(
                            "params.reason must be escape|pointer-cancel|window-closed|target-deleted|input-scope-lost, got {other}"
                        )));
                    }
                };
                let outcome = self
                    .workspace
                    .routing_gesture_cancel(inst, reason)
                    .map_err(|e| dock_err(&e))?;
                Ok(json!({
                    "ok": true,
                    "view": view,
                    "outcome": routing_outcome_json(&outcome),
                }))
            }
            "state" => {
                let fixation = self.workspace.routing_gesture(inst);
                let preview = self.workspace.routing_gesture_preview(inst);
                Ok(json!({
                    "ok": true,
                    "view": view,
                    "gesture": fixation.map(|f| json!({
                        "fixation": routing_fixation_json(f),
                        "preview": preview.map(routing_preview_json),
                    })),
                }))
            }
            other => Err(invalid(format!(
                "unknown signal_flow routing action {other}"
            ))),
        }
    }

    // -- syn.vcs.* ---------------------------------------------------------

    /// `view_id` → registered [`ViewInstanceId`] for `syn.vcs.*` ops.
    /// VCS state is ensured for every spawned view; the op still
    /// requires a real dock-registered id so typos fail loudly.
    fn vcs_view_instance(&self, params: &Value) -> Result<ViewInstanceId, ErrorBody> {
        let view = str_param(params, "view_id")?;
        let inst = parse_view_instance(view)
            .ok_or_else(|| invalid(format!("params.view_id must be \"view-{{n}}\", got {view}")))?;
        if !self.workspace.registry().contains(inst) {
            return Err(invalid(format!("view {view} is not registered")));
        }
        Ok(inst)
    }

    /// The project store as a `ProjectStore` read port for Saved
    /// endpoints (`None` until `syn.project.new`/`open`).
    fn vcs_store_ref(&self) -> Option<&dyn ProjectStore> {
        self.store.as_ref().map(|s| -> &dyn ProjectStore { s })
    }

    /// `syn.vcs.view` — the headless History/A-B Visual Diff
    /// projection (VCS-04, synvelis#95). Params `{view_id}`; the
    /// response is the full [`VcsHistoryView`] wire shape — status,
    /// history rows, A/B selection, endpoint freshness, and the
    /// semantic diff verbatim. Pure read: freshness probes are the
    /// only backend touches.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters.
    pub fn op_vcs_view(&self, params: &Value) -> Result<Value, ErrorBody> {
        let view = self.vcs_view_instance(params)?;
        let ctx = VcsSession {
            backend: self.vcs_backend.as_deref(),
            controller: self.workspace.session().controller(),
            store: self.vcs_store_ref(),
            blobs: None,
        };
        Ok(vcs_view_json(&vcs_history_view(
            self.workspace.vcs(),
            view,
            &ctx,
        )))
    }

    /// `syn.vcs.visual{action:"open"}` — spawns a detached
    /// [`ViewType::VcsHistory`] view and returns its wire `view_id`.
    /// The id comes from the workspace's monotonic high-water mark
    /// (floored above the composition root's reserved
    /// `view-1`/`view-2`), so it can never collide with a late
    /// `adopt_view` and is never recycled after `close`.
    ///
    /// # Errors
    /// [`ErrorBody`] when the dock refuses the adoption.
    fn open_vcs_view(&mut self) -> Result<String, ErrorBody> {
        let next = self
            .workspace
            .view_id_watermark()
            .max(2)
            .checked_add(1)
            .ok_or_else(|| internal("vcs view id space exhausted"))?;
        let id =
            ViewInstanceId::new(next).ok_or_else(|| internal("vcs view id space exhausted"))?;
        self.workspace
            .adopt_view(id, ViewType::VcsHistory, "VCS History", None)
            .map_err(|e| dock_err(&e))?;
        Ok(format!("view-{next}"))
    }

    /// `syn.vcs.visual{action:"close"}` — removes a
    /// [`ViewType::VcsHistory`] view (dock entry, registry, per-view
    /// state, VCS state). Composition-root views are refused by kind
    /// so this op can never close `view-1`/`view-2`.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters, a non-VCS view, or a
    /// workspace refusal.
    fn close_vcs_view(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let view_name = str_param(params, "view_id")?.to_string();
        let inst = self.vcs_view_instance(params)?;
        let is_vcs = self
            .workspace
            .registry()
            .descriptor(inst)
            .is_some_and(|d| d.kind == ViewType::VcsHistory);
        if !is_vcs {
            return Err(invalid(format!(
                "view {view_name} is not a vcs-history view"
            )));
        }
        self.workspace.remove_view(inst).map_err(|e| dock_err(&e))?;
        // Mirror `syn.view.close`'s UI-side cleanup: a client may have
        // attached controls/subscriptions/frame state to the wire
        // `view_id` even though the view itself is headless.
        let session = self.workspace.session_mut();
        self.ui.controls.close_view(session, &view_name);
        self.ui.viz.close_view(&view_name);
        self.ui.views.remove(&view_name);
        self.ui.subscriptions.retain(|(v, _)| v != &view_name);
        self.ui.frame_seq.retain(|(v, _), _| v != &view_name);
        self.ui.surfaces.retain(|(v, _)| v != &view_name);
        self.ui_pump();
        self.push_notify(
            "syn.view.state",
            json!({ "view_id": view_name, "attached": false, "closed": true }),
        );
        Ok(json!({ "ok": true, "view_id": view_name, "closed": true }))
    }

    /// `syn.vcs.visual` — one op on a view's VCS history state
    /// (VCS-04). Lifecycle actions `open` (spawn a detached
    /// `VcsHistory` view, no `view_id`) and `close` (`view_id`,
    /// VCS-kind views only) manage the view itself; the remaining
    /// actions — `refresh|load_more|select_node|pin|compare|
    /// view_mode|clear` — mutate only the view's headless VCS state.
    /// `select_node` takes `index`, `pin` takes `side`
    /// (`before|after`) + `endpoint`, `view_mode` takes `mode`
    /// (`side_by_side|change_focus`). State acks are minimal — query
    /// [`Self::op_vcs_view`] for the projection. Every action is
    /// read-only toward project, undo, transport, engine, index, and
    /// worktree by construction (`VcsSession` holds shared refs).
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or a [`VcsUiError`]
    /// failure (recorded verbatim in the view's `last_error`).
    pub fn op_vcs_visual(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let action = str_param(params, "action")?;
        // Lifecycle actions manage the view itself — they run outside
        // `with_vcs` because spawn/remove need `&mut workspace`.
        match action {
            "open" => {
                let view_id = self.open_vcs_view()?;
                return Ok(json!({ "ok": true, "view_id": view_id, "opened": true }));
            }
            "close" => return self.close_vcs_view(params),
            _ => {}
        }
        let view_name = str_param(params, "view_id")?.to_string();
        let view = self.vcs_view_instance(params)?;
        // Parse every action argument up front — param errors must
        // not depend on view/backend state.
        let op = match action {
            "refresh" => VcsOp::Refresh,
            "load_more" => VcsOp::LoadMore,
            "select_node" => VcsOp::SelectNode(usize_param(params, "index")?),
            "pin" => {
                let side = match str_param(params, "side")? {
                    "before" => PinSide::Before,
                    "after" => PinSide::After,
                    other => {
                        return Err(invalid(format!(
                            "params.side must be before|after, got {other}"
                        )));
                    }
                };
                VcsOp::Pin(side, vcs_endpoint_request(params)?)
            }
            "compare" => VcsOp::Compare,
            "view_mode" => {
                let mode = match str_param(params, "mode")? {
                    "side_by_side" => VcsViewMode::SideBySide,
                    "change_focus" => VcsViewMode::ChangeFocus,
                    other => {
                        return Err(invalid(format!(
                            "params.mode must be side_by_side|change_focus, got {other}"
                        )));
                    }
                };
                VcsOp::ViewMode(mode)
            }
            "clear" => VcsOp::Clear,
            other => return Err(invalid(format!("unknown vcs action {other}"))),
        };
        // Field-level borrows, not `self.vcs_store_ref()`: the method
        // would borrow all of `self` and collide with `with_vcs`'s
        // mutable `workspace` borrow.
        let backend = self.vcs_backend.as_deref();
        let store = self.store.as_ref().map(|s| -> &dyn ProjectStore { s });
        let cancel = VcsCancel::new();
        self.workspace
            .with_vcs(|views, session| {
                let ctx = VcsSession {
                    backend,
                    controller: session.controller(),
                    store,
                    blobs: None,
                };
                match op {
                    VcsOp::Refresh => views.refresh_history(view, &ctx, None, &cancel),
                    VcsOp::LoadMore => views.load_more_history(view, &ctx, &cancel),
                    VcsOp::SelectNode(index) => views.select_node(view, index),
                    VcsOp::Pin(side, request) => {
                        views.pin_endpoint(view, &ctx, side, &request, &cancel)
                    }
                    VcsOp::Compare => views.compare_selection(view, &ctx, &cancel),
                    VcsOp::ViewMode(mode) => {
                        views.set_view_mode(view, mode);
                        Ok(())
                    }
                    VcsOp::Clear => {
                        views.clear_selection(view);
                        Ok(())
                    }
                }
            })
            .map_err(vcs_ui_err)?;
        Ok(json!({ "ok": true, "view_id": view_name }))
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

    // -- syn.monitoring.* -----------------------------------------------------

    /// `syn.monitoring.set` — session-scope の track monitor 宣言を
    /// patch 更新または clear する（synvelis#117 RCD-03 slice）。
    ///
    /// `params.track`（tagged `TrackId`）は必須。`params.clear ==
    /// true` で宣言を削除する。それ以外は patch:
    ///
    /// - `lane`: tagged `LaneId` で lane 指定、`null` で自動解決へ
    ///   戻す（key 自体が無ければ変更なし）。
    /// - `arm`: `"off"`|`"on"`。
    /// - `monitor`: `"off"`|`"auto"`|`"on"`。
    /// - `binding`: `{"channels": [u32, …]}` — channel→input 写像。
    /// - `input_output_ready`: bool — 入力 Binding + Output
    ///   Capability の検証済みフラグ（未検証を真としない）。
    /// - `hardware_direct`: `{"state": "unknown"|"user_reported"|
    ///   "device_controlled", "observed_active": bool|null}` — 外部
    ///   観測の verbatim 注入。`Unknown` は制御可能と偽らない。
    ///
    /// 宣言は session-only で project 永続化・revision・Undo に
    /// 影響しない。存在しない track、またはその track に属さない
    /// lane は `INVALID_PARAMS`。
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or unknown track/lane.
    pub fn op_monitoring_set(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let track = TrackId::from_tagged(str_param(params, "track")?)
            .map_err(|e| invalid(format!("params.track: {e}")))?;
        let clear = match params.get("clear") {
            None | Some(Value::Null) => false,
            Some(v) => v
                .as_bool()
                .ok_or_else(|| invalid("params.clear (bool) required"))?,
        };
        if clear {
            let cleared = self.monitoring.clear_track(track);
            return Ok(json!({ "track": track.to_tagged(), "cleared": cleared }));
        }
        let mut patch = TrackPatch::default();
        if let Some(lane) = params.get("lane") {
            patch.lane = Some(match lane {
                Value::Null => None,
                v => Some(
                    LaneId::from_tagged(
                        v.as_str()
                            .ok_or_else(|| invalid("params.lane (tagged string|null)"))?,
                    )
                    .map_err(|e| invalid(format!("params.lane: {e}")))?,
                ),
            });
        }
        if let Some(arm) = params.get("arm") {
            patch.arm = Some(arm_state_param(
                arm.as_str().ok_or_else(|| invalid("params.arm (string)"))?,
            )?);
        }
        if let Some(monitor) = params.get("monitor") {
            patch.monitor = Some(monitor_state_param(
                monitor
                    .as_str()
                    .ok_or_else(|| invalid("params.monitor (string)"))?,
            )?);
        }
        if let Some(binding) = params.get("binding") {
            let channels = binding
                .get("channels")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("params.binding.channels (array) required"))?;
            let channels: Vec<u32> = channels
                .iter()
                .map(|c| {
                    c.as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .ok_or_else(|| invalid("params.binding.channels[] (u32) required"))
                })
                .collect::<Result<_, _>>()?;
            patch.channel_map = Some(
                ChannelMap::new(channels)
                    .map_err(|e| invalid(format!("params.binding.channels: {e}")))?,
            );
        }
        if let Some(ready) = params.get("input_output_ready") {
            patch.input_output_ready = Some(
                ready
                    .as_bool()
                    .ok_or_else(|| invalid("params.input_output_ready (bool) required"))?,
            );
        }
        if let Some(hw) = params.get("hardware_direct") {
            let state = hw_state_param(
                hw.get("state")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("params.hardware_direct.state (string) required"))?,
            )?;
            let observed_active = match hw.get("observed_active") {
                None | Some(Value::Null) => None,
                Some(v) => Some(v.as_bool().ok_or_else(|| {
                    invalid("params.hardware_direct.observed_active (bool|null)")
                })?),
            };
            patch.hardware_direct = Some(HardwareDirectMonitor {
                state,
                observed_active,
            });
        }
        let project = self.workspace.session().project();
        self.monitoring
            .set_track(project, track, patch)
            .map_err(invalid)?;
        Ok(json!({ "track": track.to_tagged(), "set": true }))
    }

    /// `syn.monitoring.punch` — punch in/out マークの設定・解除
    /// （project tick、半開 `[in, out)`）。session active 中は
    /// 変更不可（gate は開始時に確定済み）。
    ///
    /// `params.action`:
    /// - `"set"` — `punch_in`/`punch_out`（tick、省略は `null` 扱い）
    ///   で両端をまとめて置き換える。
    /// - `"in"` / `"out"` — `params.tick` で片側だけ設定。
    /// - `"clear"` — 両端を解除（gate は Normal に戻る）。
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or an active session.
    pub fn op_monitoring_punch(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let action = str_param(params, "action")?;
        match action {
            "set" => {
                let punch_in = opt_tick_param(params, "punch_in")?;
                let punch_out = opt_tick_param(params, "punch_out")?;
                self.monitoring
                    .punch_set(punch_in, punch_out)
                    .map_err(invalid)?;
            }
            "in" | "out" => {
                let tick = tick_param(params, "tick")?;
                let side = if action == "in" {
                    PunchSide::In
                } else {
                    PunchSide::Out
                };
                self.monitoring.punch_mark(side, tick).map_err(invalid)?;
            }
            "clear" => self.monitoring.punch_clear().map_err(invalid)?,
            _ => {
                return Err(invalid(format!("params.action: unknown action {action:?}")));
            }
        }
        Ok(json!({ "action": action }))
    }

    /// `syn.monitoring.transport` — monitor session の transport を
    /// 動かす。`params.action` は `"play"`|`"pause"`|`"stop"`|
    /// `"seek"`（seek は `params.position` の project tick 必須）。
    ///
    /// session が無い場合は `INVALID_PARAMS`。project transport
    /// （`syn.transport.set`）とは独立 — monitor session の
    /// 再生位相のみを動かし、project 側の状態は変更しない。
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters or no active session.
    pub fn op_monitoring_transport(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let action = str_param(params, "action")?;
        let mapped = match action {
            "play" => MonitorTransportAction::Play,
            "pause" => MonitorTransportAction::Pause,
            "stop" => MonitorTransportAction::Stop,
            "seek" => MonitorTransportAction::Seek,
            _ => {
                return Err(invalid(format!("params.action: unknown action {action:?}")));
            }
        };
        let position = if mapped == MonitorTransportAction::Seek {
            Some(tick_param(params, "position")?)
        } else {
            None
        };
        self.monitoring
            .transport(mapped, position)
            .map_err(invalid)?;
        Ok(json!({ "action": action }))
    }

    /// `syn.monitoring.session` — monitor session の start/stop。
    ///
    /// `"start"` は宣言 track から `RecordingRequest` を組み立て
    /// `synvelis-recording::preflight` で plan を確定する
    /// （app 側で gate/tick 意味論を再実装しない）。params:
    /// `transport_start`/`stop`（tick、必須）、`sample_rate`
    /// （u32、必須）、`clock_epoch`（tagged、省略時は project の
    /// 最小 epoch）、`max_block_frames`（u32、省略時は既定）。
    ///
    /// `"stop"` は session を破棄する（宣言・punch・latency 注入は
    /// 保持）。session の start/stop は project revision・capture・
    /// writer・file bytes を一切変更しない。
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters, preflight failure,
    /// missing clock epoch/track/lane, or an already-active session.
    pub fn op_monitoring_session(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let action = str_param(params, "action")?;
        match action {
            "start" => {
                let transport_start = tick_param(params, "transport_start")?;
                let stop = tick_param(params, "stop")?;
                let sample_rate = u32_param(params, "sample_rate")?;
                let clock_epoch = match params.get("clock_epoch") {
                    None | Some(Value::Null) => None,
                    Some(v) => Some(
                        ClockEpochId::from_tagged(
                            v.as_str()
                                .ok_or_else(|| invalid("params.clock_epoch (tagged string)"))?,
                        )
                        .map_err(|e| invalid(format!("params.clock_epoch: {e}")))?,
                    ),
                };
                let max_block_frames = params
                    .get("max_block_frames")
                    .map(|v| {
                        v.as_u64()
                            .and_then(|n| u32::try_from(n).ok())
                            .ok_or_else(|| invalid("params.max_block_frames (u32) required"))
                    })
                    .transpose()?;
                let request = MonitorSessionParams {
                    transport_start,
                    stop,
                    clock_epoch,
                    sample_rate,
                    max_block_frames,
                };
                let project = self.workspace.session().project();
                self.monitoring
                    .start_session(project, request)
                    .map_err(invalid)?;
                Ok(json!({ "action": "start", "active": true }))
            }
            "stop" => {
                self.monitoring.stop_session();
                Ok(json!({ "action": "stop", "active": false }))
            }
            _ => Err(invalid(format!("params.action: unknown action {action:?}"))),
        }
    }

    /// `syn.monitoring.latency` — latency 報告・low-latency
    /// readiness・tone-difference の注入点（実 device 層が値を
    /// 提供するまでの headless 注入）。全 field 省略可で、
    /// 指定した field のみ更新する。
    ///
    /// - `report`: `LatencyReport` 全体を置き換える
    ///   （`round_trip`/`input`/`output` は component object または
    ///   `null`、`reported` は component 配列、`reported_verified`
    ///   は bool）。component は `{"frames": u64, "quality":
    ///   "unknown"|"estimated"|"reported"|"measured", "basis":
    ///   "callback_reported"|"device_timestamp"|
    ///   "round_trip_measured", "clock_epoch": u64|null}`。
    /// - `readiness`: `{"prepared","input_ready","output_ready"}`
    ///   （全て bool 必須）。
    /// - `tone_difference`: bool。
    ///
    /// 解決投影は `syn.monitoring.state` の `latency` が
    /// `LatencyReport::resolve`/`prepare_low_latency` の結果を
    /// verbatim に返す。
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters.
    pub fn op_monitoring_latency(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let report = match params.get("report") {
            None | Some(Value::Null) => None,
            Some(v) => Some(latency_report_param(v)?),
        };
        let readiness = match params.get("readiness") {
            None | Some(Value::Null) => None,
            Some(v) => Some(latency_readiness_param(v)?),
        };
        let tone_difference = match params.get("tone_difference") {
            None | Some(Value::Null) => None,
            Some(v) => Some(
                v.as_bool()
                    .ok_or_else(|| invalid("params.tone_difference (bool) required"))?,
            ),
        };
        self.monitoring
            .set_latency(report, readiness, tone_difference);
        Ok(json!({ "updated": true }))
    }

    /// `syn.monitoring.state` — Arm/Monitor/Punch/session/latency の
    /// 完全な読み取り投影。route は `synvelis-recording::decide`、
    /// segments は `MonitorProbe`、latency は
    /// `LatencyReport::resolve`/`prepare_low_latency` の結果のみを
    /// 返し、session 無しでは route/segment を捏造しない
    /// （`session.active == false`・空 segments・明示的な
    /// unavailable 値）。
    #[must_use]
    pub fn op_monitoring_state(&self) -> Value {
        self.monitoring.state(self.workspace.session().project())
    }

    // -- syn.probe.* ---------------------------------------------------------

    /// `syn.probe.control` — semantic probe lifecycle (SF-04 slice 4).
    /// `action` selects the operation:
    ///
    /// - `"request"` — `params.target` (tagged `ParameterTargetId`),
    ///   `params.point` (serde `SemanticProbePoint`), `params.kind`
    ///   (serde `SubscriptionKind`), `params.mode` (serde `ProbeMode`)
    ///   → `ControlSession::probe_request`; returns the minted
    ///   `key` (pass it back verbatim to the other actions) and the
    ///   session-correlated `targets`.
    /// - `"set_mode"` — `params.key` + `params.mode`.
    /// - `"capture"` — `params.key` + `params.max_frames` (u64).
    /// - `"stop_capture"` / `"remove"` — `params.key` only.
    ///
    /// Every action goes through `ControlSession`'s probe surface
    /// with the attached engine's `ProbeTraceBridge` as the per-call
    /// port — the session stays the single correlation/drain owner.
    /// Fails closed with `SERVICE_ERROR` when no routing engine is
    /// attached; it never fabricates a binding or observation.
    ///
    /// # Errors
    /// [`ErrorBody`] on invalid parameters, an unknown action, a
    /// missing routing engine, or a session/port rejection.
    pub fn op_probe_control(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let action = str_param(params, "action")?;
        // Validate the action before the engine check — a malformed
        // request is a param fault regardless of attachment state.
        if !matches!(
            action,
            "request" | "set_mode" | "capture" | "stop_capture" | "remove"
        ) {
            return Err(invalid(format!("params.action: unknown action {action:?}")));
        }
        let engine = self
            .routing_engine
            .clone()
            .ok_or_else(|| internal("no routing engine attached"))?;
        let result = engine.with_probe_bridge(|port| -> Result<Value, ErrorBody> {
            let session = self.workspace.session_mut();
            match action {
                "request" => {
                    let target = ParameterTargetId::from_tagged(str_param(params, "target")?)
                        .map_err(|e| invalid(format!("params.target: {e}")))?;
                    let point: SemanticProbePoint = json_param(params, "point")?;
                    let kind: SubscriptionKind = json_param(params, "kind")?;
                    let mode: ProbeMode = json_param(params, "mode")?;
                    let key = session
                        .control_with(|control, project| {
                            control.probe_request(project, port, target, point, kind, mode)
                        })
                        .map_err(probe_trace_err)?;
                    let targets: Vec<String> = session
                        .control()
                        .probe_targets(key)
                        .iter()
                        .map(|t| t.to_tagged())
                        .collect();
                    Ok(json!({ "key": key, "targets": targets }))
                }
                "set_mode" => {
                    let key: ProbeSubscriptionKey = json_param(params, "key")?;
                    let mode: ProbeMode = json_param(params, "mode")?;
                    session
                        .control_mut()
                        .probe_set_mode(port, key, mode)
                        .map_err(probe_trace_err)?;
                    Ok(json!({ "key": key, "mode": mode }))
                }
                "capture" => {
                    let key: ProbeSubscriptionKey = json_param(params, "key")?;
                    let max_frames = u64_param(params, "max_frames")?;
                    session
                        .control_mut()
                        .probe_capture(port, key, max_frames)
                        .map_err(probe_trace_err)?;
                    Ok(json!({ "key": key, "max_frames": max_frames }))
                }
                "stop_capture" => {
                    let key: ProbeSubscriptionKey = json_param(params, "key")?;
                    session
                        .control_mut()
                        .probe_stop_capture(port, key)
                        .map_err(probe_trace_err)?;
                    Ok(json!({ "key": key }))
                }
                "remove" => {
                    let key: ProbeSubscriptionKey = json_param(params, "key")?;
                    session
                        .control_mut()
                        .probe_remove(port, key)
                        .map_err(probe_trace_err)?;
                    Ok(json!({ "key": key, "removed": true }))
                }
                _ => Err(invalid(format!("params.action: unknown action {action:?}"))),
            }
        })?;
        self.ui_pump();
        Ok(result)
    }

    /// `syn.probe.state` — probe projection (SF-04 slice 4):
    /// binding ledger (requested vs RT-confirmed mode and resolve
    /// state kept distinct — pending is never shown as observed),
    /// closed capture segments, the last drain report, and routed
    /// `observed` trace slots. Read-only: it never drains. With no
    /// routing engine attached it reports `engine_attached:false`
    /// and empty collections rather than erroring, so consumers can
    /// render the honest unavailable state.
    #[must_use]
    pub fn op_probe_state(&self) -> Value {
        let Some(engine) = self.routing_engine.as_ref() else {
            return json!({
                "engine_attached": false,
                "bindings": [],
                "segments": [],
                "drain": probe_drain_json(ProbeDrainReport::default()),
                "traces": [],
            });
        };
        let drain = self.ui.last_probe_drain;
        engine.with_probe_bridge(|port| {
            let control = self.workspace.session().control();
            let bindings: Vec<Value> = port
                .probe_bindings()
                .iter()
                .map(|(key, binding)| {
                    json!({
                        "key": key,
                        "targets": control
                            .probe_targets(*key)
                            .iter()
                            .map(|t| t.to_tagged())
                            .collect::<Vec<_>>(),
                        "point": binding.point,
                        "project_revision": binding.project_revision,
                        "active_plan_generation": binding.active_plan_generation,
                        "runtime_probe_id": binding.runtime_probe_id,
                        "requested_mode": binding.requested_mode,
                        "confirmed_mode": binding.confirmed_mode,
                        "state": binding.state,
                        "last_status": binding.last_status.as_ref().map(probe_status_json),
                    })
                })
                .collect();
            let segments: Vec<Value> = port
                .probe_segments()
                .iter()
                .map(probe_segment_json)
                .collect();
            let traces: Vec<Value> = port
                .traces()
                .into_iter()
                .take(PROBE_TRACE_CAP)
                .map(|(target, trace)| {
                    json!({
                        "target": target.to_tagged(),
                        "observed": probe_observed_json(&trace.observed),
                    })
                })
                .collect();
            json!({
                "engine_attached": true,
                "bindings": bindings,
                "segments": segments,
                "drain": probe_drain_json(drain),
                "traces": traces,
            })
        })
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

// -- syn.monitoring.* param helpers ------------------------------------------

/// `params.<key>` を project tick（`MusicalTick`、負値可）として読む。
fn tick_param(params: &Value, key: &str) -> Result<MusicalTick, ErrorBody> {
    params
        .get(key)
        .and_then(Value::as_i64)
        .map(MusicalTick::new)
        .ok_or_else(|| invalid(format!("params.{key} (tick) required")))
}

/// `params.<key>` を省略可の project tick として読む
/// （key 無し・`null` は `None`）。
fn opt_tick_param(params: &Value, key: &str) -> Result<Option<MusicalTick>, ErrorBody> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_i64()
            .map(|t| Some(MusicalTick::new(t)))
            .ok_or_else(|| invalid(format!("params.{key} (tick) required"))),
    }
}

fn arm_state_param(s: &str) -> Result<ArmState, ErrorBody> {
    match s {
        "off" => Ok(ArmState::Off),
        "on" => Ok(ArmState::On),
        _ => Err(invalid(format!("arm: unknown state {s:?}"))),
    }
}

fn monitor_state_param(s: &str) -> Result<MonitorState, ErrorBody> {
    match s {
        "off" => Ok(MonitorState::Off),
        "auto" => Ok(MonitorState::Auto),
        "on" => Ok(MonitorState::On),
        _ => Err(invalid(format!("monitor: unknown mode {s:?}"))),
    }
}

fn hw_state_param(s: &str) -> Result<HardwareDirectState, ErrorBody> {
    match s {
        "unknown" => Ok(HardwareDirectState::Unknown),
        "user_reported" => Ok(HardwareDirectState::UserReported),
        "device_controlled" => Ok(HardwareDirectState::DeviceControlled),
        _ => Err(invalid(format!("hardware_direct.state: unknown {s:?}"))),
    }
}

fn latency_quality_param(s: &str) -> Result<LatencyQuality, ErrorBody> {
    match s {
        "unknown" => Ok(LatencyQuality::Unknown),
        "estimated" => Ok(LatencyQuality::Estimated),
        "reported" => Ok(LatencyQuality::Reported),
        "measured" => Ok(LatencyQuality::Measured),
        _ => Err(invalid(format!("latency quality: unknown {s:?}"))),
    }
}

fn latency_basis_param(s: &str) -> Result<LatencyBasis, ErrorBody> {
    match s {
        "callback_reported" => Ok(LatencyBasis::CallbackReported),
        "device_timestamp" => Ok(LatencyBasis::DeviceTimestamp),
        "round_trip_measured" => Ok(LatencyBasis::RoundTripMeasured),
        _ => Err(invalid(format!("latency basis: unknown {s:?}"))),
    }
}

/// `LatencyComponent` 1 件分の wire object を読む。
/// `clock_epoch` は ruviform の device epoch 生値（u64）。
fn latency_component_param(v: &Value) -> Result<LatencyComponent, ErrorBody> {
    let frames = v
        .get("frames")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("latency component.frames (u64) required"))?;
    let quality = latency_quality_param(
        v.get("quality")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("latency component.quality (string) required"))?,
    )?;
    let basis = latency_basis_param(
        v.get("basis")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("latency component.basis (string) required"))?,
    )?;
    let clock_epoch = match v.get("clock_epoch") {
        None | Some(Value::Null) => None,
        Some(e) => Some(ruviform_media_core::ClockEpochId::new(
            e.as_u64()
                .ok_or_else(|| invalid("latency component.clock_epoch (u64)"))?,
        )),
    };
    Ok(LatencyComponent {
        frames,
        quality,
        basis,
        clock_epoch,
    })
}

/// `params.report` の wire object を `LatencyReport` へ写す
/// （report 全体の置き換え — 部分 patch ではない）。
fn latency_report_param(v: &Value) -> Result<LatencyReport, ErrorBody> {
    let component = |key: &str| -> Result<Option<LatencyComponent>, ErrorBody> {
        match v.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(c) => latency_component_param(c).map(Some),
        }
    };
    let reported = match v.get("reported") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(latency_component_param)
            .collect::<Result<_, _>>()?,
        Some(_) => return Err(invalid("params.report.reported (array)")),
    };
    Ok(LatencyReport {
        round_trip: component("round_trip")?,
        input: component("input")?,
        output: component("output")?,
        reported,
        reported_verified: match v.get("reported_verified") {
            None | Some(Value::Null) => false,
            Some(b) => b
                .as_bool()
                .ok_or_else(|| invalid("params.report.reported_verified (bool)"))?,
        },
    })
}

/// `params.readiness` の wire object を `LowLatencyReadiness` へ写す。
fn latency_readiness_param(v: &Value) -> Result<LowLatencyReadiness, ErrorBody> {
    let flag = |key: &str| -> Result<bool, ErrorBody> {
        v.get(key)
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid(format!("params.readiness.{key} (bool) required")))
    };
    Ok(LowLatencyReadiness {
        prepared: flag("prepared")?,
        input_ready: flag("input_ready")?,
        output_ready: flag("output_ready")?,
    })
}
