//! SV-47 — headless bridge from the Ruviform controls/binding contract
//! into the Synvelis Mixer, Transport and parameter-editing surfaces.
//!
//! Upstream `ui/core` + `ui/components` own the generic gesture
//! contract (`begin → preview* → commit|cancel`) and `ui/binding` owns
//! the command lifecycle: an intent is *dispatched*, receipts report
//! `accepted`/`applied`/`rejected`/`unsupported`, and a value becomes
//! displayed truth only when a [`StateProjection`] lands. Ids —
//! `gestureId`, `controlId`, `target`, `originView` — are opaque
//! strings owned by the consumer.
//!
//! [`ControlsUi`] is the Synvelis consumer side. It resolves
//! [`LogicalTarget`] strings to typed domain targets, lowers intents to
//! the existing [`ProjectSession`]/[`ControlSession`] operations —
//! gesture state machines, value mapping and preview coalescing are
//! never reimplemented here — and mirrors the binding lifecycle
//! (tracked commands, receipt ordering, generation guards) so every
//! window/view bound to a target sees the same authoritative truth.
//!
//! - Mixer / inspector controls bind `param-<32hex>` logical targets
//!   ([`ParameterTargetId::to_tagged`]). `begin`/`preview`/`commit` map
//!   to `gesture_begin`/`gesture_update`/`control_gesture_end`: the
//!   session's automation mode decides `BaseEdit`/`Written`/`Latched`/
//!   `Returned`, so command/undo semantics are untouched. `cancel` maps
//!   to [`ControlSession::gesture_cancel`].
//! - The transport surface binds [`TRANSPORT_TARGET`] for actions
//!   (`play`/`pause`/`resume`/`stop`) and [`TRANSPORT_POSITION_TARGET`]
//!   for scrub gestures → `transport_seek`. Requested vs applied state
//!   stays distinct: the receipt reports the command result and the
//!   [`TransportProjection`] reports the actual state.
//! - `begin`/`commit`/`cancel`/`action` are non-droppable and always
//!   receive a [`CommandReceipt`] — a rejection must be visible
//!   (`begin`/`cancel` are not receipt-expected upstream, but the
//!   consumer may still report on them). `preview` is the only
//!   coalescible channel: it never produces a receipt and a
//!   misrouted/late preview is dropped, never delivered to another
//!   gesture.
//! - Views attach/detach/move like the upstream `ViewLink`: detach
//!   contractually cancels the in-flight gesture and bumps the
//!   generation so stale receipts/projections can never land on a
//!   remounted binding.
//!
//! The bridge is headless — no OS windows, webviews, GPU or IME. The
//! webview runtime side (Custom Elements + the upstream TS
//! `CommandBinding`) wires the same wire-shaped types at the IPC
//! boundary; real-device GUI verification remains a manual gate.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use ruviform_media_core::MediaTimestamp;
use synvelis_project::{BeatPoint, Error, ParameterTargetId, TargetCapability};

use crate::{
    control::{
        AutomationMode, GestureEnd, InspectorBreakdown, PlaybackStatus, RecordingOutcome, WriteArm,
    },
    session::ProjectSession,
};

// ── wire contract (upstream ui/core + ui/binding vocabulary) ──────────

/// Opaque gesture id — consumer-owned in the upstream contract.
pub type GestureId = String;
/// Opaque control id.
pub type ControlId = String;
/// The generic logical target. Synvelis resolves `param-<32hex>` to a
/// [`ParameterTargetId`] and reserves [`TRANSPORT_TARGET`] /
/// [`TRANSPORT_POSITION_TARGET`] for the transport surface.
pub type LogicalTarget = String;
/// Opaque command id, unique per producing binding.
pub type CommandId = String;
/// Logical view id — stable across window attach/detach/move.
pub type ViewId = String;
/// Opaque window id.
pub type WindowId = String;

/// Logical target carrying transport actions and the transport
/// projection. Only `action` intents are meaningful here; gestures
/// (scrub) belong on [`TRANSPORT_POSITION_TARGET`].
pub const TRANSPORT_TARGET: &str = "transport";
/// Logical target for the seekbar: scrub gestures preview a position
/// and `commit` applies `transport_seek`.
pub const TRANSPORT_POSITION_TARGET: &str = "transport/position";

/// The value a control emits: `plain` is the target's natural domain
/// (beats for transport position), `normalized` the `[0,1]` contract
/// domain used by parameter targets.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlValue {
    /// Natural-domain value.
    pub plain: f64,
    /// Normalized `[0,1]` contract value.
    pub normalized: f64,
}

/// Why a gesture was cancelled — the upstream open union.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GestureCancelReason {
    /// Escape key.
    Escape,
    /// The control lost keyboard focus.
    LostFocus,
    /// The control lost pointer capture.
    LostPointerCapture,
    /// The control/view unmounted.
    #[default]
    Unmount,
    /// The view moved to another window (dock transfer).
    DockTransfer,
    /// Explicit caller cancel.
    Explicit,
    /// A producer-defined reason outside the common set — carried, not
    /// truncated, so the wire never loses the producer's meaning.
    Other(String),
}

impl GestureCancelReason {
    fn as_str(&self) -> &str {
        match self {
            Self::Escape => "escape",
            Self::LostFocus => "lost-focus",
            Self::LostPointerCapture => "lost-pointer-capture",
            Self::Unmount => "unmount",
            Self::DockTransfer => "dock-transfer",
            Self::Explicit => "explicit",
            Self::Other(s) => s.as_str(),
        }
    }

    fn parse(text: &str) -> Self {
        match text {
            "escape" => Self::Escape,
            "lost-focus" => Self::LostFocus,
            "lost-pointer-capture" => Self::LostPointerCapture,
            "unmount" => Self::Unmount,
            "dock-transfer" => Self::DockTransfer,
            "explicit" => Self::Explicit,
            other => Self::Other(other.to_owned()),
        }
    }
}

impl Serialize for GestureCancelReason {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for GestureCancelReason {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::parse(&String::deserialize(d)?))
    }
}

/// The command kinds — wire discriminants of [`CommandIntent`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IntentKind {
    /// Gesture start.
    Begin,
    /// Tentative gesture value (coalescible).
    Preview,
    /// Gesture finish intent.
    Commit,
    /// Gesture abort.
    Cancel,
    /// Discrete action (e.g. transport button).
    Action,
}

impl IntentKind {
    /// Whether the upstream contract expects a receipt for this kind.
    /// `commit` and `action` are receipt-expected; Synvelis still
    /// reports on `begin`/`cancel` so rejections stay visible.
    #[must_use]
    pub const fn expects_receipt(self) -> bool {
        matches!(self, Self::Commit | Self::Action)
    }
}

/// A generic command intent — the upstream `ui/binding` `CommandIntent`
/// union, discriminated by `kind`. Produced by controls/bindings,
/// consumed by [`ControlsUi::dispatch`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum CommandIntent {
    /// Gesture start — fixes `target` + `origin_view` for the whole
    /// gesture; mid-gesture focus/selection changes must not retarget.
    Begin {
        /// Logical target.
        target: LogicalTarget,
        /// Owning control.
        control_id: ControlId,
        /// Per-control monotonic sequence.
        sequence: u64,
        /// Consumer-owned gesture id.
        gesture_id: GestureId,
        /// Originating view (opaque).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin_view: Option<String>,
        /// Undo-boundary hint from the producer — carried, never
        /// interpreted by the gateway.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        atomic: Option<bool>,
        /// The control's value at grab.
        value: ControlValue,
    },
    /// Tentative gesture value — coalescible upstream, never a receipt.
    Preview {
        /// Logical target.
        target: LogicalTarget,
        /// Owning control.
        control_id: ControlId,
        /// Per-control monotonic sequence.
        sequence: u64,
        /// Gesture id fixed at `begin`.
        gesture_id: GestureId,
        /// Originating view (opaque).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin_view: Option<String>,
        /// Undo-boundary hint — carried.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        atomic: Option<bool>,
        /// Tentative value.
        value: ControlValue,
        /// Always `true` on the wire — previews are the only
        /// coalescible command.
        #[serde(default = "default_true")]
        coalescible: bool,
    },
    /// Gesture finish intent — the final value, not proof of apply.
    Commit {
        /// Logical target.
        target: LogicalTarget,
        /// Owning control.
        control_id: ControlId,
        /// Per-control monotonic sequence.
        sequence: u64,
        /// Gesture id fixed at `begin`.
        gesture_id: GestureId,
        /// Originating view (opaque).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin_view: Option<String>,
        /// Undo-boundary hint — carried.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        atomic: Option<bool>,
        /// The final value — authoritative even if previews were
        /// coalesced away.
        value: ControlValue,
    },
    /// Gesture abort — uncommitted preview is discarded.
    Cancel {
        /// Logical target.
        target: LogicalTarget,
        /// Owning control.
        control_id: ControlId,
        /// Per-control monotonic sequence.
        sequence: u64,
        /// Gesture id fixed at `begin`.
        gesture_id: GestureId,
        /// Originating view (opaque).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin_view: Option<String>,
        /// Undo-boundary hint — carried.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        atomic: Option<bool>,
        /// Why the gesture was cancelled.
        reason: GestureCancelReason,
    },
    /// Discrete action — non-droppable, receipt-expected.
    Action {
        /// Logical target.
        target: LogicalTarget,
        /// Owning control.
        control_id: ControlId,
        /// Per-control monotonic sequence.
        sequence: u64,
        /// Originating view (opaque).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin_view: Option<String>,
        /// Undo-boundary hint — carried.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        atomic: Option<bool>,
        /// Action name (`play`/`pause`/`resume`/`stop` on transport).
        action: String,
        /// Optional producer provenance.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
}

const fn default_true() -> bool {
    true
}

impl CommandIntent {
    /// The intent's logical target.
    #[must_use]
    pub fn target(&self) -> &LogicalTarget {
        match self {
            Self::Begin { target, .. }
            | Self::Preview { target, .. }
            | Self::Commit { target, .. }
            | Self::Cancel { target, .. }
            | Self::Action { target, .. } => target,
        }
    }

    /// The intent's kind.
    #[must_use]
    pub const fn kind(&self) -> IntentKind {
        match self {
            Self::Begin { .. } => IntentKind::Begin,
            Self::Preview { .. } => IntentKind::Preview,
            Self::Commit { .. } => IntentKind::Commit,
            Self::Cancel { .. } => IntentKind::Cancel,
            Self::Action { .. } => IntentKind::Action,
        }
    }

    /// The gesture id for gesture intents, `None` for `action`.
    #[must_use]
    pub fn gesture_id(&self) -> Option<&GestureId> {
        match self {
            Self::Begin { gesture_id, .. }
            | Self::Preview { gesture_id, .. }
            | Self::Commit { gesture_id, .. }
            | Self::Cancel { gesture_id, .. } => Some(gesture_id),
            Self::Action { .. } => None,
        }
    }
}

/// Dispatch metadata minted by the producing binding.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DispatchContext {
    /// Correlates the command to its receipts.
    pub command_id: CommandId,
    /// The producing view, when link-managed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_id: Option<ViewId>,
    /// The view's current window, when link-managed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_id: Option<WindowId>,
    /// The producer's generation at mint time.
    #[serde(default)]
    pub generation: u32,
    /// Producer issue time (ms, producer clock).
    #[serde(default)]
    pub issued_at: u64,
}

/// What a [`CommandReceipt`] reports — the upstream lifecycle status.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReceiptStatus {
    /// The consumer took responsibility; domain state unchanged.
    Accepted,
    /// The command is queued for later application.
    Queued,
    /// The command's domain effect landed.
    Applied,
    /// The consumer refused this command.
    Rejected,
    /// The consumer does not support this kind on this target.
    Unsupported,
}

/// A command outcome — correlated by `command_id`, never by order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommandReceipt {
    /// The dispatched command this answers.
    pub command_id: CommandId,
    /// Lifecycle status.
    pub status: ReceiptStatus,
    /// The consumer's revision after the effect (project revision for
    /// mutations), when meaningful.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<u64>,
    /// Human-readable detail (rejection reason, write outcome).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// What a binding shows — the upstream `CommandPhase`. Distinct
/// `pending`/`accepted`/`applied`/`rejected`/`observed` states so a
/// provisional value is never displayed as confirmed truth.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandPhase {
    /// Nothing dispatched or observed yet.
    Idle,
    /// A gesture is active; the shown value is a local tentative
    /// value — never a claim about the domain.
    Preview,
    /// A receipt-expected command awaits its receipt.
    Pending,
    /// The newest receipt is `accepted`/`queued`.
    Accepted,
    /// The newest receipt is `applied`.
    Applied,
    /// The last landed projection is the displayed truth.
    Observed,
    /// The newest receipt is `rejected` — the displayed value stays the
    /// last observed truth, never the failed intent.
    Rejected,
    /// The newest receipt is `unsupported`.
    Unsupported,
    /// The receipt window elapsed (`ack_timeout`).
    Timeout,
    /// The binding is detached — no live state.
    Stale,
}

// ── projections ───────────────────────────────────────────────────────

/// Authoritative projection of a bound parameter target — the agreed
/// breakdown shape (evaluation + observation + mode + arm +
/// capability) plus the resolved read-only flag.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamProjection {
    /// The host-resolved breakdown at the current playhead.
    pub breakdown: InspectorBreakdown,
    /// Whether the target's capability marks it read-only.
    pub read_only: bool,
}

/// Authoritative projection of the transport surface.
#[derive(Clone, Debug, PartialEq)]
pub struct TransportProjection {
    /// Actual playback status — the applied truth, never the request.
    pub status: PlaybackStatus,
    /// Actual playhead position.
    pub position: BeatPoint,
    /// Active loop range, if any.
    pub loop_range: Option<(BeatPoint, BeatPoint)>,
    /// Automation record enable.
    pub record_enabled: bool,
    /// Whether a recording transaction is open.
    pub recording_active: bool,
}

/// The consumer-owned payload inside a [`StateProjection`].
#[derive(Clone, Debug, PartialEq)]
pub enum ProjectionValue {
    /// A `param-…` target projection.
    Parameter(Box<ParamProjection>),
    /// A transport surface projection.
    Transport(TransportProjection),
}

/// The unit a binding `observe`s — the only path by which displayed
/// truth changes.
#[derive(Clone, Debug, PartialEq)]
pub struct StateProjection {
    /// Monotone revision per logical target. Params advance on every
    /// state change that can alter the projection — including session
    /// changes that do not touch the project revision — so the guard
    /// never lets a stale read land. Carried separately from the
    /// receipt's project `revision`.
    pub revision: u64,
    /// Consumer payload.
    pub value: ProjectionValue,
    /// Optional provenance (`"commit"`, `"undo"`, `"tick"`, `"attach"`).
    pub provenance: Option<String>,
}

// ── binding lifecycle mirror ──────────────────────────────────────────

/// Receipt status kept on a tracked command.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum TrackedStatus {
    InFlight,
    Accepted,
    Queued,
    Applied,
    Rejected,
    Unsupported,
}

impl TrackedStatus {
    const fn from_receipt(s: ReceiptStatus) -> Self {
        match s {
            ReceiptStatus::Accepted => Self::Accepted,
            ReceiptStatus::Queued => Self::Queued,
            ReceiptStatus::Applied => Self::Applied,
            ReceiptStatus::Rejected => Self::Rejected,
            ReceiptStatus::Unsupported => Self::Unsupported,
        }
    }

    /// Terminal statuses can only be superseded by a fresh command.
    const fn terminal(self) -> bool {
        matches!(self, Self::Applied | Self::Rejected | Self::Unsupported)
    }

    /// Ordering rank — receipts may only advance it (upstream `ack`).
    const fn rank(self) -> u8 {
        match self {
            Self::InFlight => 0,
            Self::Accepted | Self::Queued => 1,
            Self::Applied => 2,
            Self::Rejected | Self::Unsupported => 3,
        }
    }
}

/// A dispatched command the gateway mirrors for the binding's phase
/// machine. Receipts correlate by `command_id`, never by order.
#[derive(Clone, Debug)]
struct Tracked {
    status: TrackedStatus,
    expect_receipt: bool,
    timed_out: bool,
    issued_at: u64,
    /// Binding generation at mint — a receipt for a dead generation is
    /// stale and must not move the phase.
    generation: u32,
    /// The target's projection revision at mint — when the observed
    /// revision already covers this command's effect, a terminal
    /// receipt retires the head instead of pinning the phase.
    rev_at_mint: u64,
}

/// Per-(view, target) lifecycle mirror of the upstream
/// `CommandBinding`: tracked commands, the head receipt-expected
/// command, the active gesture, the last observed projection and the
/// phase rules.
#[derive(Debug)]
struct SurfaceBinding {
    generation: u32,
    attached: bool,
    /// Logical subscription intent — survives detach so attach
    /// re-subscribes, like the upstream link.
    subscribed: bool,
    active_gesture: Option<GestureId>,
    /// Latest previewed value (normalized for params, beats for
    /// `transport/position`) — tentative display only.
    preview: Option<f64>,
    tracked: BTreeMap<CommandId, Tracked>,
    /// Insertion order for the bounded tracked map (upstream limit 64).
    tracked_order: VecDeque<CommandId>,
    head: Option<CommandId>,
    last_receipt: Option<CommandReceipt>,
    last_observed: Option<StateProjection>,
    observed_seq: u64,
}

/// Upstream `TRACKED_LIMIT`.
const TRACKED_LIMIT: usize = 64;

impl SurfaceBinding {
    fn new(generation: u32, attached: bool) -> Self {
        Self {
            generation,
            attached,
            subscribed: false,
            active_gesture: None,
            preview: None,
            tracked: BTreeMap::new(),
            tracked_order: VecDeque::new(),
            head: None,
            last_receipt: None,
            last_observed: None,
            observed_seq: 0,
        }
    }

    fn track(&mut self, command_id: CommandId, kind: IntentKind, issued_at: u64, rev_at_mint: u64) {
        let expect_receipt = kind.expects_receipt();
        self.tracked.insert(
            command_id.clone(),
            Tracked {
                status: TrackedStatus::InFlight,
                expect_receipt,
                timed_out: false,
                issued_at,
                generation: self.generation,
                rev_at_mint,
            },
        );
        self.tracked_order.push_back(command_id.clone());
        while self.tracked.len() > TRACKED_LIMIT {
            let Some(oldest) = self.tracked_order.pop_front() else {
                break;
            };
            if self.head.as_ref() == Some(&oldest) {
                self.head = None;
            }
            self.tracked.remove(&oldest);
        }
        if expect_receipt {
            self.head = Some(command_id);
        }
    }

    /// The upstream phase rules: stale when detached, preview while a
    /// gesture is live, head status otherwise, observed when a
    /// projection landed, idle when nothing has ever happened.
    fn phase(&self) -> CommandPhase {
        if !self.attached {
            return CommandPhase::Stale;
        }
        if self.active_gesture.is_some() {
            return CommandPhase::Preview;
        }
        if let Some(head) = self.head.as_ref().and_then(|id| self.tracked.get(id)) {
            if head.timed_out && !head.status.terminal() {
                return CommandPhase::Timeout;
            }
            return match head.status {
                TrackedStatus::InFlight => CommandPhase::Pending,
                TrackedStatus::Accepted | TrackedStatus::Queued => CommandPhase::Accepted,
                TrackedStatus::Applied => CommandPhase::Applied,
                TrackedStatus::Rejected => CommandPhase::Rejected,
                TrackedStatus::Unsupported => CommandPhase::Unsupported,
            };
        }
        if self.last_observed.is_some() {
            CommandPhase::Observed
        } else {
            CommandPhase::Idle
        }
    }

    fn head_timed_out(&self) -> bool {
        self.head
            .as_ref()
            .and_then(|id| self.tracked.get(id))
            .is_some_and(|t| t.timed_out)
    }
}

/// What a gesture owns — the target, the originating view and the
/// binding generation it was minted under. Preview and terminal
/// events must match all three or they are dropped; a dead-generation
/// owner can never route a late commit.
#[derive(Clone, Debug)]
struct GestureOwner {
    view: ViewId,
    target: LogicalTarget,
    /// Binding generation at `begin` — commits/cancels from an older
    /// epoch are stale and cannot land.
    generation: u32,
}

/// The routed command's addressing triple — every route helper
/// takes it as one unit.
struct Route<'a> {
    view: &'a str,
    target: &'a str,
    command_id: &'a CommandId,
}

/// Per-view link state — `viewId` is stable across windows; the map,
/// not the window, owns the link.
#[derive(Clone, Debug)]
struct ViewState {
    window: Option<WindowId>,
    generation: u32,
    attached: bool,
    attaches: u32,
}

/// A receipt queued on the consumer-facing wire — delivered to the
/// binding by [`ControlsUi::pump`] or [`ControlsUi::deliver`].
#[derive(Clone, Debug, PartialEq)]
pub struct WireReceipt {
    /// Destination view.
    pub view: ViewId,
    /// The command's target.
    pub target: LogicalTarget,
    /// The receipt itself.
    pub receipt: CommandReceipt,
}

/// Resolved logical target.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum TargetRef {
    Parameter(ParameterTargetId),
    /// Both transport surfaces; gestures additionally require
    /// `transport/position`.
    Transport,
}

fn parse_target(target: &str) -> Option<TargetRef> {
    if target == TRANSPORT_TARGET || target == TRANSPORT_POSITION_TARGET {
        return Some(TargetRef::Transport);
    }
    ParameterTargetId::from_tagged(target)
        .ok()
        .map(TargetRef::Parameter)
}

/// Denominator for `f64` beats → [`BeatPoint`] conversion (micro-beat —
/// far below seekbar resolution, reduced on store).
const SEEK_DEN: u32 = 1_000_000;
/// Sensible upper bound for a scrub position (beats).
const SEEK_MAX_BEATS: f64 = 1e12;

fn beats_to_point(beats: f64) -> Result<BeatPoint, Error> {
    if !beats.is_finite() || !(0.0..SEEK_MAX_BEATS).contains(&beats) {
        return Err(Error::Validation(format!(
            "seek position {beats} out of range"
        )));
    }
    #[allow(clippy::cast_possible_truncation)] // bounded by SEEK_MAX_BEATS
    BeatPoint::new((beats * f64::from(SEEK_DEN)).round() as i64, SEEK_DEN)
}

/// Every session/domain error is a `rejected` receipt — the domain
/// supports the command but refuses it now. `unsupported` is minted
/// only by the router, for kinds a target class does not implement.
const fn error_status(_e: &Error) -> ReceiptStatus {
    ReceiptStatus::Rejected
}

fn error_detail(e: &Error) -> String {
    match e {
        Error::WriteRejected { rejection, .. } => rejection.clone(),
        other => other.to_string(),
    }
}

// ── rows (the headless "display" surface) ─────────────────────────────

/// Raw lifecycle state of one (view, target) binding — the test-facing
/// mirror of the upstream `CommandBinding` getters.
#[derive(Clone, Debug, PartialEq)]
pub struct BindingSnapshot {
    /// Owning view.
    pub view: ViewId,
    /// Bound logical target.
    pub target: LogicalTarget,
    /// Live generation (adopted from the view on attach, bumped on
    /// detach).
    pub generation: u32,
    /// Whether the binding is live (not detached).
    pub attached: bool,
    /// Whether the binding holds a subscription.
    pub subscribed: bool,
    /// Current phase.
    pub phase: CommandPhase,
    /// The in-flight gesture, if any.
    pub active_gesture: Option<GestureId>,
    /// Tentative preview value, if any.
    pub preview: Option<f64>,
    /// Newest receipt-expected command still governing the phase.
    pub head: Option<CommandId>,
    /// Newest effective receipt.
    pub last_receipt: Option<CommandReceipt>,
    /// Last landed projection — the displayed truth.
    pub last_observed: Option<StateProjection>,
    /// Number of projections landed (upstream `observedSeq`).
    pub observed_seq: u64,
    /// Head command's ack window elapsed.
    pub timed_out: bool,
}

/// A channel-strip / inspector surface row — what a bound
/// fader/knob/readout may display.
#[derive(Clone, Debug, PartialEq)]
pub struct StripRow {
    /// Owning view.
    pub view: ViewId,
    /// Bound logical target.
    pub target: LogicalTarget,
    /// Current phase.
    pub phase: CommandPhase,
    /// Binding generation.
    pub generation: u32,
    /// Whether the binding is live.
    pub attached: bool,
    /// Last landed truth — the only value the row may show as
    /// confirmed.
    pub observed: Option<ParamProjection>,
    /// Tentative value while a gesture previews — never truth.
    pub preview: Option<f64>,
    /// Newest effective receipt.
    pub last_receipt: Option<CommandReceipt>,
    /// The head command's ack window elapsed.
    pub timed_out: bool,
}

/// A transport-bar row — what play/stop/seek buttons + position
/// readout may display.
#[derive(Clone, Debug, PartialEq)]
pub struct TransportRow {
    /// Owning view.
    pub view: ViewId,
    /// Current phase of the action binding (`transport`).
    pub phase: CommandPhase,
    /// Binding generation (`transport`).
    pub generation: u32,
    /// Whether the action binding is live.
    pub attached: bool,
    /// Last landed transport truth.
    pub observed: Option<TransportProjection>,
    /// Scrub position preview while a seek gesture is active.
    pub preview: Option<f64>,
    /// Newest effective receipt on the action binding.
    pub last_receipt: Option<CommandReceipt>,
    /// The head command's ack window elapsed.
    pub timed_out: bool,
}

/// Gateway counters — the upstream `stats` mirror plus router drops.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiDiagnostics {
    /// Intents received by [`ControlsUi::dispatch`].
    pub dispatched: u64,
    /// Receipts minted onto the wire.
    pub receipts_emitted: u64,
    /// Intents answered `rejected`.
    pub rejected_intents: u64,
    /// Intents answered `unsupported`.
    pub unsupported_intents: u64,
    /// Previews dropped (no owner, wrong view, out-of-domain, session
    /// error) — the droppable channel, by contract.
    pub dropped_previews: u64,
    /// Intents ignored because their `command_id` was already tracked.
    pub duplicate_commands: u64,
    /// Intents dropped because their generation was behind the
    /// binding's live generation.
    pub stale_intents: u64,
    /// Intents dropped because the binding was detached.
    pub detached_intents: u64,
    /// Receipts that could not move the phase (unknown or dead-epoch
    /// command).
    pub stale_receipts: u64,
    /// Receipts absorbed by the ordering guard (already superseded).
    pub out_of_order_receipts: u64,
    /// Projections dropped by the revision guard or while detached.
    pub stale_projections: u64,
    /// Projections landed on bindings.
    pub delivered_projections: u64,
    /// Gestures cancelled by a view detach (contractual cancel).
    pub cancelled_on_detach: u64,
}

// ── the gateway ───────────────────────────────────────────────────────

/// The Synvelis-side consumer of the Ruviform controls/binding
/// contract: dispatches [`CommandIntent`]s onto [`ProjectSession`],
/// emits [`CommandReceipt`]s, pushes [`StateProjection`]s to bound
/// views, and owns the view/generation lifecycle.
///
/// Owns **no** project state — every domain read/write flows through
/// the passed `ProjectSession`, so the session stays inside
/// `WorkspaceManager` (or a fixture) as the single source of truth.
#[derive(Debug, Default)]
pub struct ControlsUi {
    views: BTreeMap<ViewId, ViewState>,
    bindings: BTreeMap<(ViewId, LogicalTarget), SurfaceBinding>,
    gestures: BTreeMap<GestureId, GestureOwner>,
    wire: VecDeque<WireReceipt>,
    /// Monotone projection revision per canonical target.
    revisions: BTreeMap<String, u64>,
    clock_ms: u64,
    ack_timeout_ms: u64,
    diagnostics: UiDiagnostics,
}

impl ControlsUi {
    /// A gateway with the upstream default ack window (5000 ms).
    #[must_use]
    pub fn new() -> Self {
        Self {
            ack_timeout_ms: 5_000,
            ..Self::default()
        }
    }

    /// Overrides the receipt ack window (ms).
    #[must_use]
    pub fn with_ack_timeout(ack_timeout_ms: u64) -> Self {
        Self {
            ack_timeout_ms,
            ..Self::new()
        }
    }

    /// Advances the gateway clock — receipt-expected commands past
    /// `ack_timeout_ms` flip to `timed_out` (their phase reads
    /// [`CommandPhase::Timeout`]; a late `applied` still lands, like
    /// upstream).
    pub fn set_clock(&mut self, now_ms: u64) {
        self.clock_ms = now_ms;
        for binding in self.bindings.values_mut() {
            for tracked in binding.tracked.values_mut() {
                if tracked.expect_receipt
                    && !tracked.status.terminal()
                    && now_ms.saturating_sub(tracked.issued_at) > self.ack_timeout_ms
                {
                    tracked.timed_out = true;
                }
            }
        }
    }

    /// The diagnostics counters.
    #[must_use]
    pub fn diagnostics(&self) -> &UiDiagnostics {
        &self.diagnostics
    }

    /// The receipts queued for delivery (oldest first).
    #[must_use]
    pub fn wire(&self) -> &VecDeque<WireReceipt> {
        &self.wire
    }

    // -- views (upstream ViewLink mirror) ---------------------------------

    /// Attaches `view` to `window` — first mount or the far side of a
    /// dock transfer. Idempotent for the same window; a different
    /// window while attached performs an implicit
    /// `detach(dock-transfer)` first so in-flight work terminates under
    /// the old epoch. The view generation bumps; bindings adopt it and
    /// re-read the current truth for every held subscription.
    pub fn attach_view(&mut self, session: &mut ProjectSession, view: &str, window: &str) {
        {
            let v = self.views.entry(view.to_owned()).or_insert(ViewState {
                window: None,
                generation: 0,
                attached: false,
                attaches: 0,
            });
            if v.attached && v.window.as_deref() == Some(window) {
                return;
            }
        }
        if self.views.get(view).is_some_and(|v| v.attached) {
            self.detach_view(session, view, &GestureCancelReason::DockTransfer);
        }
        let generation = {
            let v = self.views.entry(view.to_owned()).or_insert(ViewState {
                window: None,
                generation: 0,
                attached: false,
                attaches: 0,
            });
            v.attached = true;
            v.window = Some(window.to_owned());
            v.generation += 1;
            v.attaches += 1;
            v.generation
        };
        // Owner records for this view are dead by now (detach already
        // ran the contractual cancel) — drop any stragglers so a
        // remount cannot resurrect a dead-epoch gesture.
        self.gestures.retain(|_, owner| owner.view != view);
        let targets: Vec<LogicalTarget> = self
            .bindings
            .range((view.to_owned(), String::new())..)
            .take_while(|((v, _), _)| v == view)
            .map(|((_, t), _)| t.clone())
            .collect();
        for target in targets {
            if let Some(binding) = self.bindings.get_mut(&(view.to_owned(), target.clone())) {
                binding.attached = true;
                binding.generation = generation;
                binding.head = None;
                binding.active_gesture = None;
                binding.preview = None;
            }
            self.refresh_binding(session, view, &target);
        }
    }

    /// Detaches `view` — unmount, dock transfer or host close. Cancels
    /// in-flight gestures (the contractual cancel: the session gesture
    /// is discarded, never committed), bumps binding generations so
    /// stale receipts/projections cannot land, and releases the
    /// display. Project truth is never signalled — this layer owns UI
    /// lifecycle only.
    pub fn detach_view(
        &mut self,
        session: &mut ProjectSession,
        view: &str,
        reason: &GestureCancelReason,
    ) {
        let Some(v) = self.views.get_mut(view) else {
            return;
        };
        if !v.attached {
            return;
        }
        v.attached = false;
        v.window = None;
        // Every gesture the view owns is contractually cancelled under
        // the dying epoch — not only the binding-marked one. Each gets
        // a tracked cancel command + accepted receipt so a late
        // receipt lands stale and the cancel is visible evidence.
        let owned: Vec<(GestureId, LogicalTarget)> = self
            .gestures
            .iter()
            .filter(|(_, owner)| owner.view == view)
            .map(|(gesture_id, owner)| (gesture_id.clone(), owner.target.clone()))
            .collect();
        for (gesture_id, target) in owned {
            let command_id = format!("detach-cancel:{gesture_id}");
            let clock = self.clock_ms;
            let mint_rev = self
                .revisions
                .get(Self::rev_key(&target))
                .copied()
                .unwrap_or(0);
            if let Some(binding) = self.bindings.get_mut(&(view.to_owned(), target.clone())) {
                binding.track(command_id.clone(), IntentKind::Cancel, clock, mint_rev);
            }
            self.cancel_gesture(session, &gesture_id, reason);
            self.emit_receipt(
                view,
                &target,
                command_id,
                ReceiptStatus::Accepted,
                None,
                Some(format!("cancelled on detach: {}", reason.as_str())),
            );
            self.diagnostics.cancelled_on_detach += 1;
        }
        // Detach every binding: the generation bump kills late
        // commands/receipts/projections, and in-flight command state
        // clears per the upstream link contract — a remount re-minting
        // a command id must not read as a duplicate.
        let targets: Vec<LogicalTarget> = self
            .bindings
            .range((view.to_owned(), String::new())..)
            .take_while(|((v, _), _)| v == view)
            .map(|((_, t), _)| t.clone())
            .collect();
        for target in targets {
            if let Some(binding) = self.bindings.get_mut(&(view.to_owned(), target)) {
                binding.attached = false;
                binding.preview = None;
                binding.active_gesture = None;
                binding.generation += 1;
                binding.tracked.clear();
                binding.tracked_order.clear();
                binding.head = None;
            }
        }
    }

    /// Detaches and drops every binding of `view` — a window closed
    /// for good. The view entry stays as a tombstone (`attached:
    /// false`): intents on the dead view are dropped as detached and
    /// can never resurrect a binding or mint domain state. A later
    /// `attach_view` on the same id resurrects cleanly at a bumped
    /// generation.
    pub fn close_view(&mut self, session: &mut ProjectSession, view: &str) {
        self.detach_view(session, view, &GestureCancelReason::Explicit);
        let dead: Vec<(ViewId, LogicalTarget)> = self
            .bindings
            .range((view.to_owned(), String::new())..)
            .take_while(|((v, _), _)| v == view)
            .map(|(k, _)| k.clone())
            .collect();
        for key in dead {
            self.bindings.remove(&key);
        }
        // Defense-in-depth: detach already cancelled every owned
        // gesture; nothing should remain.
        self.gestures.retain(|_, owner| owner.view != view);
    }

    // -- subscriptions ------------------------------------------------------

    /// Binds `view` to `target` and pushes the current truth — the
    /// upstream `port.subscribe` contract: a subscription always
    /// re-reads, it never replays cached state.
    ///
    /// # Errors
    /// [`Error::NotFound`] for an unresolvable logical target.
    pub fn subscribe(
        &mut self,
        session: &ProjectSession,
        view: &str,
        target: &str,
    ) -> Result<(), Error> {
        match parse_target(target) {
            Some(TargetRef::Parameter(id)) => {
                if !session.project().control.targets.contains_key(&id) {
                    return Err(Error::NotFound(format!("param {target}")));
                }
            }
            Some(TargetRef::Transport) => {}
            None => return Err(Error::NotFound(format!("target {target}"))),
        }
        self.ensure_binding(view, target).subscribed = true;
        self.refresh_binding(session, view, target);
        Ok(())
    }

    /// Releases the binding's subscription intent (the binding itself
    /// and its command history stay live).
    pub fn unsubscribe(&mut self, view: &str, target: &str) {
        if let Some(b) = self.bindings.get_mut(&(view.to_owned(), target.to_owned())) {
            b.subscribed = false;
        }
    }

    // -- dispatch -----------------------------------------------------------

    /// Routes one intent to the session. Non-droppable kinds always
    /// mint a [`CommandReceipt`] onto [`Self::wire`] — deliver with
    /// [`Self::pump`] / [`Self::deliver`]. Previews apply to the
    /// owning gesture or are dropped, by contract.
    ///
    /// Dispatch **never** borrows the session beyond the call: intents
    /// map to `gesture_*` / `control_gesture_end` / `gesture_cancel` /
    /// transport ops, and every mutation is followed by a projection
    /// broadcast so all bound windows converge on the same truth.
    pub fn dispatch(
        &mut self,
        session: &mut ProjectSession,
        intent: CommandIntent,
        ctx: DispatchContext,
    ) {
        self.diagnostics.dispatched += 1;
        let view = ctx.view_id.clone().unwrap_or_default();
        let target = intent.target().clone();
        let key = (view.clone(), target.clone());
        // Commands need a live link: intents on unknown or detached
        // views are dropped — a dead epoch can never reach the domain.
        if !self.views.get(&view).is_some_and(|v| v.attached) {
            self.diagnostics.detached_intents += 1;
            return;
        }
        // Dead-generation and detached intents never reach the domain.
        if let Some(binding) = self.bindings.get(&key) {
            if !binding.attached {
                self.diagnostics.detached_intents += 1;
                return;
            }
            if ctx.generation < binding.generation {
                self.diagnostics.stale_intents += 1;
                return;
            }
            if binding.tracked.contains_key(&ctx.command_id) {
                self.diagnostics.duplicate_commands += 1;
                return;
            }
        } else if ctx.generation < self.views.get(&view).map_or(0, |v| v.generation) {
            // No binding yet — the view's own epoch still guards the
            // intent (a first dispatch must stamp the live generation).
            self.diagnostics.stale_intents += 1;
            return;
        }
        let kind = intent.kind();
        // Unresolvable targets are rejected at the router — they mint
        // no bindings and no tracked state.
        if parse_target(&target).is_none() {
            if kind == IntentKind::Preview {
                self.diagnostics.dropped_previews += 1;
            } else {
                self.emit_receipt(
                    &view,
                    &target,
                    ctx.command_id,
                    ReceiptStatus::Rejected,
                    None,
                    Some(format!("unknown target {target}")),
                );
            }
            return;
        }
        let issued_at = ctx.issued_at.max(self.clock_ms);
        let mint_rev = self
            .revisions
            .get(Self::rev_key(&target))
            .copied()
            .unwrap_or(0);
        // Previews are the droppable/coalescible channel — they mint
        // no tracked state and cannot evict real commands.
        if kind != IntentKind::Preview {
            self.ensure_binding(&view, &target).track(
                ctx.command_id.clone(),
                kind,
                issued_at,
                mint_rev,
            );
        }
        let command_id = ctx.command_id;
        match intent {
            CommandIntent::Begin { gesture_id, .. } => {
                self.route_begin(session, &view, &target, &command_id, gesture_id);
            }
            CommandIntent::Preview {
                gesture_id, value, ..
            } => {
                self.route_preview(session, &view, &target, &gesture_id, value);
            }
            CommandIntent::Commit {
                gesture_id, value, ..
            } => {
                self.route_commit(session, &view, &target, &command_id, &gesture_id, value);
            }
            CommandIntent::Cancel {
                gesture_id, reason, ..
            } => {
                self.route_cancel(session, &view, &target, &command_id, &gesture_id, &reason);
            }
            CommandIntent::Action { action, .. } => {
                self.route_action(session, &view, &target, &command_id, &action);
            }
        }
    }

    /// Resolves a logical target once for a gesture route.
    fn gesture_target(
        &mut self,
        view: &str,
        target: &str,
        command_id: &CommandId,
    ) -> Option<TargetRef> {
        if let Some(t) = parse_target(target) {
            return Some(t);
        }
        self.emit_receipt(
            view,
            target,
            command_id.clone(),
            ReceiptStatus::Rejected,
            None,
            Some(format!("unknown target {target}")),
        );
        None
    }

    fn route_begin(
        &mut self,
        session: &mut ProjectSession,
        view: &str,
        target: &str,
        command_id: &CommandId,
        gesture_id: GestureId,
    ) {
        let Some(t) = self.gesture_target(view, target, command_id) else {
            return;
        };
        // A gesture id names exactly one live gesture — reusing it must
        // not overwrite the owner record and orphan a session gesture.
        if self.gestures.contains_key(&gesture_id) {
            self.emit_receipt(
                view,
                target,
                command_id.clone(),
                ReceiptStatus::Rejected,
                None,
                Some(format!("gesture {gesture_id} is already live")),
            );
            return;
        }
        // One live gesture per binding — a second grab is rejected,
        // never merged into or detached from the running gesture.
        if self
            .bindings
            .get(&(view.to_owned(), target.to_owned()))
            .is_some_and(|b| b.active_gesture.is_some())
        {
            self.emit_receipt(
                view,
                target,
                command_id.clone(),
                ReceiptStatus::Rejected,
                None,
                Some("a gesture is already active on this binding".into()),
            );
            return;
        }
        match t {
            TargetRef::Transport if target == TRANSPORT_TARGET => self.emit_receipt(
                view,
                target,
                command_id.clone(),
                ReceiptStatus::Unsupported,
                None,
                Some("gestures are not supported on the transport action surface".into()),
            ),
            TargetRef::Transport => {
                // The playhead is one truth: a scrub in any window
                // holds it exclusively until it ends.
                if self.gestures.values().any(|o| o.target == target) {
                    self.emit_receipt(
                        view,
                        target,
                        command_id.clone(),
                        ReceiptStatus::Rejected,
                        None,
                        Some("a scrub gesture is already active".into()),
                    );
                    return;
                }
                // Scrub gesture — preview is display-only until commit.
                let generation = self
                    .bindings
                    .get(&(view.to_owned(), target.to_owned()))
                    .map_or(0, |b| b.generation);
                self.gestures.insert(
                    gesture_id.clone(),
                    GestureOwner {
                        view: view.to_owned(),
                        target: target.to_owned(),
                        generation,
                    },
                );
                if let Some(b) = self.bindings.get_mut(&(view.to_owned(), target.to_owned())) {
                    b.active_gesture = Some(gesture_id);
                }
                self.emit_receipt(
                    view,
                    target,
                    command_id.clone(),
                    ReceiptStatus::Accepted,
                    None,
                    None,
                );
            }
            TargetRef::Parameter(param) => {
                self.begin_param(
                    session,
                    &Route {
                        view,
                        target,
                        command_id,
                    },
                    gesture_id,
                    param,
                );
            }
        }
    }

    /// Param `begin`: the session's own exclusivity rules apply (one
    /// gesture per target across all views).
    fn begin_param(
        &mut self,
        session: &mut ProjectSession,
        route: &Route<'_>,
        gesture_id: GestureId,
        param: ParameterTargetId,
    ) {
        let begun = session.control_with(|c, p| {
            let pos = c.playback().position;
            c.gesture_begin(p, param, pos)
        });
        match begun {
            Ok(()) => {
                let generation = self
                    .bindings
                    .get(&(route.view.to_owned(), route.target.to_owned()))
                    .map_or(0, |b| b.generation);
                self.gestures.insert(
                    gesture_id.clone(),
                    GestureOwner {
                        view: route.view.to_owned(),
                        target: route.target.to_owned(),
                        generation,
                    },
                );
                if let Some(b) = self
                    .bindings
                    .get_mut(&(route.view.to_owned(), route.target.to_owned()))
                {
                    b.active_gesture = Some(gesture_id);
                }
                self.emit_receipt(
                    route.view,
                    route.target,
                    route.command_id.clone(),
                    ReceiptStatus::Accepted,
                    None,
                    None,
                );
                // The injected override changes every dependent
                // evaluation, not only the grabbed target.
                self.broadcast_all_params(session, "begin");
            }
            Err(e) => self.emit_receipt(
                route.view,
                route.target,
                route.command_id.clone(),
                error_status(&e),
                None,
                Some(error_detail(&e)),
            ),
        }
    }

    fn route_preview(
        &mut self,
        session: &mut ProjectSession,
        view: &str,
        target: &str,
        gesture_id: &GestureId,
        value: ControlValue,
    ) {
        // Previews are the droppable channel: a misrouted or orphaned
        // preview is dropped, never delivered to another gesture.
        let Some(owner) = self.gestures.get(gesture_id) else {
            self.diagnostics.dropped_previews += 1;
            return;
        };
        if owner.view != view || owner.target != target {
            self.diagnostics.dropped_previews += 1;
            return;
        }
        match parse_target(target) {
            Some(TargetRef::Transport) => {
                if target == TRANSPORT_POSITION_TARGET && value.plain.is_finite() {
                    if let Some(b) = self.bindings.get_mut(&(view.to_owned(), target.to_owned())) {
                        b.preview = Some(value.plain);
                    }
                } else {
                    self.diagnostics.dropped_previews += 1;
                }
            }
            Some(TargetRef::Parameter(param)) => {
                let applied = session.control_with(|c, _| {
                    let pos = c.playback().position;
                    c.gesture_update(param, pos, value.normalized)
                });
                match applied {
                    Ok(()) => {
                        if let Some(b) =
                            self.bindings.get_mut(&(view.to_owned(), target.to_owned()))
                        {
                            b.preview = Some(value.normalized);
                        }
                        self.touch_and_broadcast_param(session, param);
                    }
                    Err(_) => self.diagnostics.dropped_previews += 1,
                }
            }
            None => self.diagnostics.dropped_previews += 1,
        }
    }

    fn route_commit(
        &mut self,
        session: &mut ProjectSession,
        view: &str,
        target: &str,
        command_id: &CommandId,
        gesture_id: &GestureId,
        value: ControlValue,
    ) {
        let Some(t) = self.gesture_target(view, target, command_id) else {
            return;
        };
        let live_gen = self
            .bindings
            .get(&(view.to_owned(), target.to_owned()))
            .map_or(0, |b| b.generation);
        match self.gestures.get(gesture_id) {
            Some(owner)
                if owner.view == view && owner.target == target && owner.generation == live_gen => {
            }
            _ => {
                self.emit_receipt(
                    view,
                    target,
                    command_id.clone(),
                    ReceiptStatus::Rejected,
                    None,
                    Some("no such gesture on this view/target".into()),
                );
                return;
            }
        }
        match t {
            TargetRef::Transport => {
                // A seekbar commit applies the final scrub position.
                match beats_to_point(value.plain) {
                    Ok(pos) => {
                        session.control_mut().transport_seek(pos);
                        self.end_gesture(view, target, gesture_id);
                        self.emit_receipt(
                            view,
                            target,
                            command_id.clone(),
                            ReceiptStatus::Applied,
                            None,
                            Some(format!("seek {pos}")),
                        );
                        // The playhead moved — every position-dependent
                        // evaluation re-projects, not only transport.
                        self.touch(TRANSPORT_TARGET);
                        self.broadcast_all(session, "commit");
                    }
                    Err(e) => self.emit_receipt(
                        view,
                        target,
                        command_id.clone(),
                        error_status(&e),
                        None,
                        Some(error_detail(&e)),
                    ),
                }
            }
            TargetRef::Parameter(param) => {
                self.commit_param(
                    session,
                    &Route {
                        view,
                        target,
                        command_id,
                    },
                    gesture_id,
                    param,
                    value,
                );
            }
        }
    }

    /// Param `commit`: the commit's value is authoritative even when
    /// earlier previews were coalesced away — land it before ending.
    /// The session's automation mode decides the outcome (base edit /
    /// written / latched / returned).
    fn commit_param(
        &mut self,
        session: &mut ProjectSession,
        route: &Route<'_>,
        gesture_id: &GestureId,
        param: ParameterTargetId,
        value: ControlValue,
    ) {
        let updated = session.control_with(|c, _| {
            let pos = c.playback().position;
            c.gesture_update(param, pos, value.normalized)
        });
        if let Err(e) = updated {
            self.emit_receipt(
                route.view,
                route.target,
                route.command_id.clone(),
                error_status(&e),
                None,
                Some(error_detail(&e)),
            );
            return;
        }
        match session.control_gesture_end(param) {
            Ok(end) => {
                self.end_gesture(route.view, route.target, gesture_id);
                let (status, detail) = match end {
                    GestureEnd::BaseEdit { .. } => {
                        (ReceiptStatus::Applied, Some("base edit".to_owned()))
                    }
                    GestureEnd::Written { points } => (
                        ReceiptStatus::Applied,
                        Some(format!("written {points} points")),
                    ),
                    GestureEnd::Latched { .. } => {
                        (ReceiptStatus::Applied, Some("latched".to_owned()))
                    }
                    GestureEnd::Returned => (
                        ReceiptStatus::Applied,
                        Some("returned — nothing written".to_owned()),
                    ),
                };
                self.emit_receipt(
                    route.view,
                    route.target,
                    route.command_id.clone(),
                    status,
                    Some(session.revision().0),
                    detail,
                );
            }
            Err(e) => {
                // `gesture_end` already removed the session gesture
                // before the failed apply — drop the UI records to
                // match or the binding is stuck previewing a dead
                // gesture.
                self.end_gesture(route.view, route.target, gesture_id);
                self.emit_receipt(
                    route.view,
                    route.target,
                    route.command_id.clone(),
                    error_status(&e),
                    None,
                    Some(error_detail(&e)),
                );
            }
        }
        // The landed value and dropped override change every
        // dependent evaluation, not only the grabbed target.
        self.broadcast_all_params(session, "commit");
    }

    fn route_cancel(
        &mut self,
        session: &mut ProjectSession,
        view: &str,
        target: &str,
        command_id: &CommandId,
        gesture_id: &GestureId,
        reason: &GestureCancelReason,
    ) {
        if self.gesture_target(view, target, command_id).is_none() {
            return;
        }
        let live_gen = self
            .bindings
            .get(&(view.to_owned(), target.to_owned()))
            .map_or(0, |b| b.generation);
        match self.gestures.get(gesture_id) {
            Some(owner)
                if owner.view == view && owner.target == target && owner.generation == live_gen => {
            }
            _ => {
                self.emit_receipt(
                    view,
                    target,
                    command_id.clone(),
                    ReceiptStatus::Rejected,
                    None,
                    Some("no such gesture on this view/target".into()),
                );
                return;
            }
        }
        self.cancel_gesture(session, gesture_id, reason);
        self.emit_receipt(
            view,
            target,
            command_id.clone(),
            ReceiptStatus::Accepted,
            None,
            None,
        );
    }

    /// The contractual cancel shared by `cancel` intents and view
    /// detach: session preview state is discarded and the owner/binding
    /// records cleared. Never writes — a cancel cannot rewrite
    /// committed state. Every bound view re-projects: the dropped
    /// override must revert everywhere, not only in the owning window.
    fn cancel_gesture(
        &mut self,
        session: &mut ProjectSession,
        gesture_id: &GestureId,
        _reason: &GestureCancelReason,
    ) {
        let Some(owner) = self.gestures.remove(gesture_id) else {
            return;
        };
        if let Some(binding) = self
            .bindings
            .get_mut(&(owner.view.clone(), owner.target.clone()))
        {
            binding.active_gesture = None;
            binding.preview = None;
        }
        match parse_target(&owner.target) {
            Some(TargetRef::Parameter(param)) => {
                let _ = session.control_with(|c, _| c.gesture_cancel(param));
                // The dropped override changes every dependent
                // evaluation, not only the grabbed target.
                self.broadcast_all_params(session, "cancel");
            }
            Some(TargetRef::Transport) => {
                self.touch_and_broadcast_transport(session, "cancel");
            }
            None => {}
        }
    }

    /// Clears the gesture owner + binding preview after a commit.
    fn end_gesture(&mut self, view: &str, target: &str, gesture_id: &GestureId) {
        self.gestures.remove(gesture_id);
        if let Some(binding) = self.bindings.get_mut(&(view.to_owned(), target.to_owned())) {
            binding.active_gesture = None;
            binding.preview = None;
        }
    }

    fn route_action(
        &mut self,
        session: &mut ProjectSession,
        view: &str,
        target: &str,
        command_id: &CommandId,
        action: &str,
    ) {
        match parse_target(target) {
            Some(TargetRef::Parameter(_)) => self.emit_receipt(
                view,
                target,
                command_id.clone(),
                ReceiptStatus::Unsupported,
                None,
                Some("actions are not supported on parameter targets".into()),
            ),
            Some(TargetRef::Transport) if target == TRANSPORT_TARGET => {
                let before = session.control().playback().status;
                match action {
                    "play" => {
                        session.control_mut().transport_play();
                        session.set_playing(true);
                    }
                    "pause" => {
                        session.control_mut().transport_pause();
                        session.set_playing(false);
                    }
                    "resume" => {
                        session.control_mut().transport_resume();
                        if session.control().playback().status == PlaybackStatus::Playing {
                            session.set_playing(true);
                        }
                    }
                    "stop" => {
                        session.control_mut().transport_stop();
                        session.set_playing(false);
                    }
                    _ => {
                        self.emit_receipt(
                            view,
                            target,
                            command_id.clone(),
                            ReceiptStatus::Unsupported,
                            None,
                            Some(format!("unknown transport action {action}")),
                        );
                        return;
                    }
                }
                // A completed action is `applied`; `detail` reports
                // whether the request actually moved the truth.
                let after = session.control().playback().status;
                let detail = (after == before).then(|| "no change".to_owned());
                self.emit_receipt(
                    view,
                    target,
                    command_id.clone(),
                    ReceiptStatus::Applied,
                    None,
                    detail,
                );
                // Stop clears held writes and manual overrides; any
                // transition can flip observation state — every bound
                // truth re-projects, not only transport.
                self.touch(TRANSPORT_TARGET);
                self.broadcast_all(session, "action");
            }
            Some(TargetRef::Transport) => self.emit_receipt(
                view,
                target,
                command_id.clone(),
                ReceiptStatus::Unsupported,
                None,
                Some(
                    "actions belong on the transport surface — gestures on transport/position"
                        .into(),
                ),
            ),
            None => self.emit_receipt(
                view,
                target,
                command_id.clone(),
                ReceiptStatus::Rejected,
                None,
                Some(format!("unknown target {target}")),
            ),
        }
    }

    // -- receipt channel ------------------------------------------------------

    /// Delivers every queued receipt to its binding (upstream `ack`
    /// semantics: correlated by `command_id` *within its own
    /// (view, target) binding*, ordered by status rank, dead
    /// generations dropped).
    pub fn pump(&mut self) {
        while let Some(d) = self.wire.pop_front() {
            self.deliver(&d.view, &d.target, d.receipt);
        }
    }

    /// Drains the wire without delivering — the raw receipt stream for
    /// fixtures that model a lossy/reordering transport.
    pub fn drain_wire(&mut self) -> Vec<WireReceipt> {
        self.wire.drain(..).collect()
    }

    /// Feeds one receipt back to the owning `(view, target)` binding —
    /// command ids are unique per producing binding, so routing must
    /// hit the binding the receipt was minted for, never a sibling.
    pub fn deliver(&mut self, view: &str, target: &str, receipt: CommandReceipt) {
        let Some(binding) = self.bindings.get_mut(&(view.to_owned(), target.to_owned())) else {
            self.diagnostics.stale_receipts += 1;
            return;
        };
        let Some(tracked) = binding.tracked.get_mut(&receipt.command_id) else {
            self.diagnostics.stale_receipts += 1;
            return;
        };
        if tracked.generation != binding.generation {
            self.diagnostics.stale_receipts += 1;
            return;
        }
        let next = TrackedStatus::from_receipt(receipt.status);
        if next.rank() <= tracked.status.rank() {
            self.diagnostics.out_of_order_receipts += 1;
            return;
        }
        tracked.status = next;
        binding.last_receipt = Some(receipt);
        // The command's own projection may already have landed —
        // truth ahead of the receipt retires the head and the
        // phase settles to `observed` (upstream `observe` rule,
        // applied in the synchronous-push direction).
        if next.terminal()
            && binding
                .last_observed
                .as_ref()
                .is_some_and(|o| o.revision > tracked.rev_at_mint)
        {
            binding.head = None;
        }
    }

    fn emit_receipt(
        &mut self,
        view: &str,
        target: &str,
        command_id: CommandId,
        status: ReceiptStatus,
        revision: Option<u64>,
        detail: Option<String>,
    ) {
        match status {
            ReceiptStatus::Rejected => self.diagnostics.rejected_intents += 1,
            ReceiptStatus::Unsupported => self.diagnostics.unsupported_intents += 1,
            _ => {}
        }
        self.diagnostics.receipts_emitted += 1;
        self.wire.push_back(WireReceipt {
            view: view.to_owned(),
            target: target.to_owned(),
            receipt: CommandReceipt {
                command_id,
                status,
                revision,
                detail,
            },
        });
    }

    // -- projections ----------------------------------------------------------

    /// The canonical revision key: transport surfaces share the
    /// transport epoch; params key on their own tagged id.
    fn rev_key(target: &str) -> &str {
        if target == TRANSPORT_POSITION_TARGET {
            TRANSPORT_TARGET
        } else {
            target
        }
    }

    /// Bumps the target's projection revision.
    fn touch(&mut self, target: &str) -> u64 {
        let rev = self
            .revisions
            .entry(Self::rev_key(target).to_owned())
            .or_insert(0);
        *rev += 1;
        *rev
    }

    /// Re-reads the current truth into a subscribed binding — the
    /// subscribe/attach contract. The revision is bumped first so the
    /// fresh read always lands: truth that changed while the binding
    /// was detached (or outside this gateway) must converge on
    /// re-attach, never be dropped by its own stale guard.
    fn refresh_binding(&mut self, session: &ProjectSession, view: &str, target: &str) {
        let mut p = match parse_target(target) {
            Some(TargetRef::Parameter(param)) => {
                self.touch(target);
                let Some(p) = self.param_projection(session, param) else {
                    self.clear_observed(target);
                    return;
                };
                p
            }
            Some(TargetRef::Transport) => {
                self.touch(target);
                self.transport_projection(session)
            }
            None => return,
        };
        p.provenance = Some("subscribe".to_owned());
        let (mut stale, mut landed) = (0, 0);
        self.push(view, target, &p, &mut stale, &mut landed);
        self.diagnostics.stale_projections += stale;
        self.diagnostics.delivered_projections += landed;
    }

    /// Drops the landed truth for every binding of `target` — used
    /// when the target can no longer be resolved (e.g. a deleted
    /// param) so views never keep showing phantom values.
    fn clear_observed(&mut self, target: &str) {
        for ((_, t), binding) in &mut self.bindings {
            if t == target {
                binding.last_observed = None;
            }
        }
    }

    fn param_projection(
        &self,
        session: &ProjectSession,
        param: ParameterTargetId,
    ) -> Option<StateProjection> {
        let control = session.control();
        let pos = control.playback().position;
        let project = session.project();
        let breakdown = control.inspector_breakdown(project, param, pos).ok()?;
        let read_only = project
            .control
            .targets
            .get(&param)
            .is_some_and(synvelis_project::ParameterTarget::is_readonly);
        let revision = *self.revisions.get(&param.to_tagged()).unwrap_or(&0);
        Some(StateProjection {
            revision,
            value: ProjectionValue::Parameter(Box::new(ParamProjection {
                breakdown,
                read_only,
            })),
            provenance: None,
        })
    }

    fn transport_projection(&self, session: &ProjectSession) -> StateProjection {
        let control = session.control();
        let revision = *self.revisions.get(TRANSPORT_TARGET).unwrap_or(&0);
        StateProjection {
            revision,
            value: ProjectionValue::Transport(TransportProjection {
                status: control.playback().status,
                position: control.playback().position,
                loop_range: control.playback().loop_range,
                record_enabled: control.record_enabled(),
                recording_active: control.recording().is_some(),
            }),
            provenance: None,
        }
    }

    /// Lands a projection on one binding under the revision + attached
    /// guards (the upstream `observe` rules).
    fn observe(binding: &mut SurfaceBinding, p: StateProjection) -> bool {
        if !binding.attached {
            return false;
        }
        if binding
            .last_observed
            .as_ref()
            .is_some_and(|o| p.revision <= o.revision)
        {
            return false;
        }
        binding.last_observed = Some(p);
        binding.observed_seq += 1;
        // A landed truth retires a terminal head — the observed value
        // is what the binding now shows (upstream `observe`).
        if binding
            .head
            .as_ref()
            .and_then(|id| binding.tracked.get(id))
            .is_some_and(|t| t.status.terminal())
        {
            binding.head = None;
        }
        true
    }

    fn push(
        &mut self,
        view: &str,
        target: &str,
        p: &StateProjection,
        stale: &mut u64,
        landed: &mut u64,
    ) {
        let key = (view.to_owned(), target.to_owned());
        let Some(binding) = self.bindings.get_mut(&key) else {
            return;
        };
        if !binding.subscribed {
            return;
        }
        if Self::observe(binding, p.clone()) {
            *landed += 1;
        } else {
            *stale += 1;
        }
    }

    /// Bumps the param revision and pushes its projection to every
    /// subscribed view.
    fn touch_and_broadcast_param(&mut self, session: &ProjectSession, param: ParameterTargetId) {
        self.touch(&param.to_tagged());
        self.broadcast_param(session, param, "commit");
    }

    fn broadcast_param(
        &mut self,
        session: &ProjectSession,
        param: ParameterTargetId,
        provenance: &str,
    ) {
        let Some(mut p) = self.param_projection(session, param) else {
            // The target vanished — bound views must not keep showing
            // phantom truth.
            self.clear_observed(&param.to_tagged());
            return;
        };
        p.provenance = Some(provenance.to_owned());
        let target = param.to_tagged();
        let (mut stale, mut landed) = (0, 0);
        let views: Vec<ViewId> = self
            .bindings
            .keys()
            .filter(|(_, t)| t == &target)
            .map(|(v, _)| v.clone())
            .collect();
        for view in views {
            self.push(&view, &target, &p, &mut stale, &mut landed);
        }
        self.diagnostics.stale_projections += stale;
        self.diagnostics.delivered_projections += landed;
    }

    /// Bumps the transport revision and pushes its projection to every
    /// subscribed transport binding (`transport` and
    /// `transport/position` share the epoch).
    fn touch_and_broadcast_transport(&mut self, session: &ProjectSession, provenance: &str) {
        self.touch(TRANSPORT_TARGET);
        self.broadcast_transport(session, provenance);
    }

    fn broadcast_transport(&mut self, session: &ProjectSession, provenance: &str) {
        let mut p = self.transport_projection(session);
        p.provenance = Some(provenance.to_owned());
        let (mut stale, mut landed) = (0, 0);
        let keys: Vec<(ViewId, LogicalTarget)> = self
            .bindings
            .keys()
            .filter(|(_, t)| Self::rev_key(t) == TRANSPORT_TARGET)
            .cloned()
            .collect();
        for (view, target) in keys {
            self.push(&view, &target, &p, &mut stale, &mut landed);
        }
        self.diagnostics.stale_projections += stale;
        self.diagnostics.delivered_projections += landed;
    }

    /// Re-pushes every bound param target — a mutation that can
    /// change evaluations (commit, cancel, feed, mode change,
    /// gesture begin) must converge dependent targets too: a
    /// `depth_source` driver resolves through the driver's own
    /// evaluation, so a single-target push leaves dependents stale.
    fn broadcast_all_params(&mut self, session: &ProjectSession, provenance: &str) {
        let params: Vec<ParameterTargetId> = self
            .bindings
            .keys()
            .filter_map(|(_, t)| match parse_target(t) {
                Some(TargetRef::Parameter(id)) => Some(id),
                Some(TargetRef::Transport) | None => None,
            })
            .collect();
        for param in params {
            self.touch(&param.to_tagged());
            self.broadcast_param(session, param, provenance);
        }
    }

    /// Re-pushes every subscribed param target — project truth may
    /// have changed under a wide apply (undo/redo, record commit,
    /// playhead move). Transport re-reads at its current revision:
    /// callers `touch(TRANSPORT_TARGET)` first when the op itself
    /// moves transport truth, so unchanged transport is dropped by
    /// the guard rather than inflating the epoch.
    fn broadcast_all(&mut self, session: &ProjectSession, provenance: &str) {
        self.broadcast_all_params(session, provenance);
        self.broadcast_transport(session, provenance);
    }

    // -- session ops (the non-contract host surface) --------------------------

    /// Selects the automation mode for a target; broadcasts the new
    /// truth. Session-only — never a project mutation. The mode
    /// changes the target's own evaluation, so dependents re-project.
    ///
    /// # Errors
    /// Propagates [`ControlSession::set_mode`].
    pub fn set_mode(
        &mut self,
        session: &mut ProjectSession,
        target: ParameterTargetId,
        mode: AutomationMode,
    ) -> Result<(), Error> {
        session.control_with(|c, p| c.set_mode(p, target, mode))?;
        self.broadcast_all_params(session, "mode");
        Ok(())
    }

    /// Arms/disarms a target for writing; broadcasts the new truth.
    ///
    /// # Errors
    /// Propagates [`ControlSession::set_arm`].
    pub fn set_arm(
        &mut self,
        session: &mut ProjectSession,
        target: ParameterTargetId,
        arm: WriteArm,
    ) -> Result<(), Error> {
        session.control_with(|c, p| c.set_arm(p, target, arm))?;
        self.touch_and_broadcast_param(session, target);
        Ok(())
    }

    /// Enables/disables automation record; broadcasts the transport
    /// truth.
    pub fn set_record_enabled(&mut self, session: &mut ProjectSession, enabled: bool) {
        session.control_mut().set_record_enabled(enabled);
        self.touch_and_broadcast_transport(session, "record");
    }

    /// Binds the write lane for a target; broadcasts the new truth.
    ///
    /// # Errors
    /// Propagates [`ControlSession::bind_write_lane`].
    pub fn bind_write_lane(
        &mut self,
        session: &mut ProjectSession,
        target: ParameterTargetId,
        lane: synvelis_project::AutomationLaneId,
    ) -> Result<(), Error> {
        session.control_with(|c, p| c.bind_write_lane(p, target, lane))?;
        self.touch_and_broadcast_param(session, target);
        Ok(())
    }

    /// Opens the recording transaction; broadcasts the transport truth.
    ///
    /// # Errors
    /// Propagates [`ControlSession::record_start`].
    pub fn record_start(&mut self, session: &mut ProjectSession) -> Result<(), Error> {
        let rev = session.revision();
        session.control_mut().record_start(rev)?;
        self.touch_and_broadcast_transport(session, "record");
        Ok(())
    }

    /// Commits the open recording transaction as one undo step;
    /// broadcasts all bound truths.
    ///
    /// # Errors
    /// Propagates [`ProjectSession::control_record_commit`].
    pub fn record_commit(
        &mut self,
        session: &mut ProjectSession,
    ) -> Result<RecordingOutcome, Error> {
        let outcome = session.control_record_commit()?;
        // The consumed transaction flips `recording_active` —
        // transport truth changed, so its epoch must advance or the
        // new state can never land past the revision guard.
        self.touch(TRANSPORT_TARGET);
        self.broadcast_all(session, "commit");
        Ok(outcome)
    }

    /// Aborts the open recording transaction; broadcasts the transport
    /// truth.
    pub fn record_abort(&mut self, session: &mut ProjectSession) {
        session.control_record_abort();
        self.touch_and_broadcast_transport(session, "record");
    }

    /// Feeds a plugin-reported value; broadcasts the target's truth.
    ///
    /// # Errors
    /// Propagates [`ControlSession::feed_plugin_report`].
    pub fn feed_plugin_report(
        &mut self,
        session: &mut ProjectSession,
        target: ParameterTargetId,
        value: f64,
        stamp: MediaTimestamp,
    ) -> Result<(), Error> {
        session.control_with(|c, p| c.feed_plugin_report(p, target, value, stamp))?;
        self.broadcast_all_params(session, "observe");
        Ok(())
    }

    /// Feeds a probe-captured value; broadcasts the target's truth.
    ///
    /// # Errors
    /// Propagates [`ControlSession::feed_probe`].
    pub fn feed_probe(
        &mut self,
        session: &mut ProjectSession,
        target: ParameterTargetId,
        value: f64,
        stamp: MediaTimestamp,
    ) -> Result<(), Error> {
        session.control_with(|c, p| c.feed_probe(p, target, value, stamp))?;
        self.broadcast_all_params(session, "observe");
        Ok(())
    }

    /// Sets/clears the transport loop; broadcasts the transport truth.
    ///
    /// # Errors
    /// Propagates [`ControlSession::set_loop`].
    pub fn set_loop(
        &mut self,
        session: &mut ProjectSession,
        range: Option<(BeatPoint, BeatPoint)>,
    ) -> Result<(), Error> {
        session.control_mut().set_loop(range)?;
        self.touch_and_broadcast_transport(session, "loop");
        Ok(())
    }

    /// Advances the playhead — the engine-clock surface. Held
    /// latch/write values land at control rate per the session rules;
    /// the transport truth and every bound param truth re-projects
    /// (evaluations are position-dependent).
    ///
    /// # Errors
    /// Propagates [`ControlSession::transport_advance`].
    pub fn transport_advance(
        &mut self,
        session: &mut ProjectSession,
        to: BeatPoint,
    ) -> Result<crate::control::TransportEffect, Error> {
        let effect = session.control_with(|c, p| c.transport_advance(p, to))?;
        self.touch(TRANSPORT_TARGET);
        self.broadcast_all(session, "tick");
        Ok(effect)
    }

    /// Re-reads every bound truth — the healing surface for domain
    /// mutations that bypassed this gateway (e.g. a direct
    /// `ProjectSession::apply` from another host surface). Every
    /// revision is bumped so the stale guard cannot block the fresh
    /// read.
    pub fn resync(&mut self, session: &ProjectSession) {
        self.touch(TRANSPORT_TARGET);
        self.broadcast_all(session, "resync");
    }

    /// Undoes the last project command; every bound truth re-projects.
    ///
    /// # Errors
    /// Propagates [`ProjectSession::undo`].
    pub fn undo(&mut self, session: &mut ProjectSession) -> Result<bool, Error> {
        let undone = session.undo()?;
        self.broadcast_all(session, "undo");
        Ok(undone)
    }

    /// Redoes the last undone command; every bound truth re-projects.
    ///
    /// # Errors
    /// Propagates [`ProjectSession::redo`].
    pub fn redo(&mut self, session: &mut ProjectSession) -> Result<bool, Error> {
        let redone = session.redo()?;
        self.broadcast_all(session, "redo");
        Ok(redone)
    }

    // -- read surface ---------------------------------------------------------

    /// The lifecycle snapshot of one (view, target) binding.
    #[must_use]
    pub fn binding(&self, view: &str, target: &str) -> Option<BindingSnapshot> {
        self.bindings
            .get(&(view.to_owned(), target.to_owned()))
            .map(|b| BindingSnapshot {
                view: view.to_owned(),
                target: target.to_owned(),
                generation: b.generation,
                attached: b.attached,
                subscribed: b.subscribed,
                phase: b.phase(),
                active_gesture: b.active_gesture.clone(),
                preview: b.preview,
                head: b.head.clone(),
                last_receipt: b.last_receipt.clone(),
                last_observed: b.last_observed.clone(),
                observed_seq: b.observed_seq,
                timed_out: b.head_timed_out(),
            })
    }

    /// The channel-strip row for a bound `param-…` target — observed
    /// truth, tentative preview, phase, receipts.
    #[must_use]
    pub fn strip_row(&self, view: &str, target: &str) -> Option<StripRow> {
        let b = self.bindings.get(&(view.to_owned(), target.to_owned()))?;
        let observed = b.last_observed.as_ref().and_then(|p| match &p.value {
            ProjectionValue::Parameter(v) => Some((**v).clone()),
            ProjectionValue::Transport(_) => None,
        });
        Some(StripRow {
            view: view.to_owned(),
            target: target.to_owned(),
            phase: b.phase(),
            generation: b.generation,
            attached: b.attached,
            observed,
            preview: b.preview,
            last_receipt: b.last_receipt.clone(),
            timed_out: b.head_timed_out(),
        })
    }

    /// The transport-bar row — action binding phase + position binding
    /// scrub preview + the last landed transport truth. Both bindings
    /// share the transport epoch; the row merges them so an
    /// unsubscribed action binding can never mask the position
    /// binding's landed truth (or an in-flight scrub commit).
    #[must_use]
    pub fn transport_row(&self, view: &str) -> Option<TransportRow> {
        let action = self
            .bindings
            .get(&(view.to_owned(), TRANSPORT_TARGET.to_owned()));
        let scrub = self
            .bindings
            .get(&(view.to_owned(), TRANSPORT_POSITION_TARGET.to_owned()));
        action.or(scrub)?;
        let observed = [scrub, action]
            .into_iter()
            .flatten()
            .find_map(|b| b.last_observed.as_ref())
            .and_then(|p| match &p.value {
                ProjectionValue::Transport(v) => Some(v.clone()),
                ProjectionValue::Parameter(_) => None,
            });
        let phase = [action, scrub]
            .into_iter()
            .flatten()
            .map(SurfaceBinding::phase)
            .find(|p| *p != CommandPhase::Idle)
            .unwrap_or(CommandPhase::Idle);
        Some(TransportRow {
            view: view.to_owned(),
            phase,
            generation: action.or(scrub).map_or(0, |b| b.generation),
            attached: action.or(scrub).is_some_and(|b| b.attached),
            observed,
            preview: scrub.and_then(|b| b.preview),
            last_receipt: action
                .and_then(|b| b.last_receipt.clone())
                .or_else(|| scrub.and_then(|b| b.last_receipt.clone())),
            timed_out: [action, scrub]
                .into_iter()
                .flatten()
                .any(SurfaceBinding::head_timed_out),
        })
    }

    /// A view's live link state `(window, generation, attached)`.
    #[must_use]
    pub fn view_state(&self, view: &str) -> Option<(Option<&str>, u32, bool)> {
        self.views
            .get(view)
            .map(|v| (v.window.as_deref(), v.generation, v.attached))
    }

    // -- internals -------------------------------------------------------------

    fn ensure_binding(&mut self, view: &str, target: &str) -> &mut SurfaceBinding {
        // An unregistered view yields a *detached* binding: the logical
        // subscription survives so `attach_view` can re-subscribe, but
        // nothing lands or dispatches while the link is dead.
        let (generation, attached) = self
            .views
            .get(view)
            .map_or((0, false), |v| (v.generation, v.attached));
        self.bindings
            .entry((view.to_owned(), target.to_owned()))
            .or_insert_with(|| SurfaceBinding::new(generation, attached))
    }

    /// The capability contract a bound control may mirror (e.g. a
    /// stepped knob's steps, a choice control's options).
    #[must_use]
    pub fn capability(
        session: &ProjectSession,
        target: ParameterTargetId,
    ) -> Option<TargetCapability> {
        session
            .project()
            .control
            .targets
            .get(&target)
            .and_then(|t| t.capability)
    }
}
