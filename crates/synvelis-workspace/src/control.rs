//! Session-scoped parameter-control machinery (issue #40): the UI
//! automation mode state machine, write arm, gestures, the recording
//! transaction, playback state and observations — plus the read-only
//! Lane / Signal Flow / Inspector projections over `Project::control`.
//!
//! Everything here is **session state**: selecting a mode, arming a
//! target, moving a knob, feeding an observation — none of it mutates
//! the project, advances the revision or touches the undo history. The
//! only path back into the project is an explicit commit
//! ([`ProjectSession::control_gesture_end`] for `Off`-mode base edits,
//! [`ProjectSession::control_record_commit`] for a recording session).
//! A recording commit is split into three phases (issue #98):
//! [`ControlSession::prepare_record_commit`] snapshots the point
//! columns **non-destructively** and checks the transaction's fixed
//! revision against the live one, `ProjectController::apply` is the
//! CAS authority, and [`ControlSession::finish_record_commit`] is the
//! only consumer — a failed prepare or apply leaves every recorded
//! point in place for Inspect / Retry / Explicit Rebase / Abort.
//!
//! Consumed upstream contracts (never reimplemented):
//!
//! - rf#188 timed input — [`map_stamp_to_frame`] is the single
//!   stamp→frame authority; native [`InputStamp`]s are mapped exactly
//!   once and `MapRejection`s surface as [`crate::WorkspaceError::StampRejected`].
//! - rf#185 probe observation — [`MediaTimestamp`] +
//!   `TimestampProvenance` carry the observation's measurement time
//!   and trust level.
//! - rf#186 param write — [`ParamWriteMeans`]/[`TimedParamEvent`]
//!   through [`synvelis_project::dispatch_check`] and
//!   [`synvelis_project::lower_event`].
//!
//! [`ProjectSession::control_gesture_end`]: crate::session::ProjectSession::control_gesture_end
//! [`ProjectSession::control_record_commit`]: crate::session::ProjectSession::control_record_commit

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use ruviform_media_core::{
    ClockAnchor, ClockEpochId, EngineFrame, InputStamp, MediaDuration, MediaTime, MediaTimestamp,
    Rounding, TimeBase, TimestampProvenance, map_stamp_to_frame,
};
use ruviform_plugin_core::{ParamWriteMeans, TimedParamEvent};
use ruviform_rt::{Consumer, Producer};
use synvelis_project::{
    AutomationCurveView, AutomationLaneId, AutomationPoint, AutomationRegion, AutomationTimeScope,
    BeatPoint, BeatRange, ControlEvaluation, ControlOwner, CurveQuery, Error, Interpolation,
    ModulationAssignmentId, ParameterTargetId, ProcessorId, Project, ProjectOp, ProjectRevision,
    ReferenceSource, TargetCapability, evaluate_control,
};

use crate::probe_trace::ProbeTraceSession;

/// The UI-facing automation mode (`#6` 決定④). Selected per target;
/// default is `Read`. **Selecting a mode never mutates the project** —
/// it only changes how subsequent gestures behave.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum AutomationMode {
    /// Automation stopped; manual gestures edit the persisted base
    /// (a normal undoable edit).
    Off,
    /// Manual gestures temporarily override; releasing returns to the
    /// recorded series. Nothing is ever written.
    #[default]
    Read,
    /// While armed+record+transport, the gesture interval is written;
    /// release returns to the recorded series.
    Touch,
    /// Like `Touch`, but after release the last value keeps writing
    /// until transport/record stops or the mode changes — never
    /// carried into the next playback.
    Latch,
    /// While armed+record+transport, writing continues after release
    /// (the held manual value keeps landing at control rate).
    Write,
}

/// Per-target write arm. Default `Disarmed`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum WriteArm {
    /// The target cannot be written to.
    #[default]
    Disarmed,
    /// The target may be written when record+transport hold.
    Armed,
}

/// Transport status for the control session.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PlaybackStatus {
    /// Not rolling.
    #[default]
    Stopped,
    /// Rolling (record gates may open).
    Playing,
    /// Held mid-position: not rolling, but the playhead and any
    /// latch-held writes stay in place so `resume` continues the same
    /// playback. Record gates stay closed — a write must never land
    /// while the transport is not rolling.
    Paused,
}

/// Which loop pass of the current transport run a recording
/// transaction writes into (issue #133). `0` means "no run yet":
/// a non-`Playing` → `Playing` edge starts pass `1`, and each loop
/// wrap increments it. A new pass never records implicitly — only an
/// explicit `record_start` (re-arm) opens a transaction for it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoopPassId(pub u64);

impl fmt::Display for LoopPassId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pass-{}", self.0)
    }
}

/// Session playback state — kept in beats (`BeatPosition`-exact) so
/// tempo edits never rewrite history; `epoch` carries the clock epoch
/// for stamp→frame mapping (rf#188). The engine frame is **derived**
/// from `position` at the use site (`stamp_gesture`), never stored —
/// a stored copy can only go silently stale.
#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackState {
    /// Rolling or stopped.
    pub status: PlaybackStatus,
    /// Current project position in beats.
    pub position: BeatPoint,
    /// Optional loop range `[start, end)`; crossing `end` wraps to
    /// `start` and seals any open recording transaction.
    pub loop_range: Option<(BeatPoint, BeatPoint)>,
    /// Clock epoch for frame comparisons.
    pub epoch: ruviform_media_core::ClockEpochId,
    /// The loop pass of the current transport run (`1` on the first
    /// play edge, `+1` per wrap).
    pub pass: LoopPassId,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            position: BeatPoint::ZERO,
            loop_range: None,
            epoch: ruviform_media_core::ClockEpochId::new(1),
            pass: LoopPassId(0),
        }
    }
}

/// Why a recording transaction was sealed — further writes are
/// rejected; the transaction must be committed or aborted. A seal
/// closes the **write gate only**: prepare/commit/abort stay legal
/// on a sealed transaction (the `LoopWrap` contract).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SealReason {
    /// The playhead wrapped the loop end: continuing would silently
    /// overwrite the interval just recorded — forbidden by the agreed
    /// model.
    LoopWrap,
    /// The transport stopped — a stopped session never resumes into
    /// the same transaction on the next play.
    Stopped,
    /// The playhead was repositioned mid-recording; the recorded
    /// interval is no longer contiguous.
    Seek,
    /// A fixed target's UI mode changed mid-recording.
    ModeChanged,
    /// A fixed target's write arm changed mid-recording.
    ArmChanged,
    /// A project apply/undo/redo succeeded while the transaction was
    /// open — its fixed revision is now stale.
    ProjectChange,
    /// The transport clock epoch changed mid-recording — the epoch
    /// fixed at `record_start` no longer holds (issue #133).
    EpochChange,
    /// The host reported the control plan / `LastGood` graph stale —
    /// continuing could record through a mismatched mapping
    /// (issue #133). Session-only notification.
    GraphStale,
    /// The loop range changed mid-recording — the pass boundary's
    /// coordinate basis is void (issue #133).
    LoopChanged,
    /// A non-`Playing` → `Playing` edge began a new transport run
    /// while the transaction was open — a new run never resumes an
    /// old pass (issue #133; the AUT-01 run-boundary contract applied
    /// to the recording gate).
    NewTransportPlay,
}

/// Monotonic, unique id of a recording transaction inside one
/// `ControlSession` (issued from 1; never reused within the session).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordingTransactionId(pub u64);

impl fmt::Display for RecordingTransactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rec-tx-{}", self.0)
    }
}

/// Lifecycle state of a recording transaction. Failures leave the
/// transaction in `Conflict`/`Rejected` **with its point columns
/// retained** — only `Committed` consumes it.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum RecordingTxnState {
    /// Accumulating points or ready to commit.
    #[default]
    Pending,
    /// `prepare` found `fixed_revision != current` — needs an explicit
    /// user rebase approval before it can commit.
    Conflict,
    /// `prepare`'s structural checks or the controller's apply
    /// rejected the commit — the columns are retained.
    Rejected,
    /// Consumed by a successful `finish_record_commit`.
    Committed,
}

/// Errors of the recording-commit protocol (issue #98). The typed
/// variants map directly onto the UI choices Inspect / Retry /
/// Explicit Rebase / Abort.
#[derive(Clone, Debug, PartialEq)]
pub enum RecordCommitError {
    /// No open transaction and nothing to commit against.
    NoTransaction,
    /// The presented id matches neither the open transaction nor any
    /// applied record — a stale or foreign receipt.
    UnknownTransaction {
        /// The transaction id that was presented.
        id: RecordingTransactionId,
    },
    /// `fixed_revision` differs from the live revision — the default
    /// outcome for *any* drift, including unrelated edits. No silent
    /// rebinding to the newest revision.
    Conflict {
        /// Revision the transaction fixed at `record_start`.
        fixed: ProjectRevision,
        /// Live revision at prepare time.
        current: ProjectRevision,
        /// The targets whose recorded data is at stake.
        affected_targets: Vec<ParameterTargetId>,
    },
    /// The prepared op built and CAS-checked clean but the
    /// controller's `apply` rejected it (stale CAS, validation,
    /// store). The transaction is `Rejected`, columns retained.
    Rejected {
        /// The controller error.
        reason: Error,
    },
    /// The same transaction id with an identical payload was already
    /// committed — the retry wrote nothing (in-memory exactly-once).
    AlreadyApplied {
        /// The committed transaction id.
        id: RecordingTransactionId,
        /// The revision the first commit produced.
        revision: ProjectRevision,
    },
    /// The same transaction id was presented with a different
    /// payload fingerprint — refused, never merged.
    IntegrityConflict {
        /// The transaction id in question.
        id: RecordingTransactionId,
    },
    /// `prepare`'s structural checks failed (a fixed target or
    /// segment lane no longer exists, a lane was retargeted, a range
    /// cannot be represented, or the point budget was exceeded).
    Invalid(String),
}

impl fmt::Display for RecordCommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoTransaction => write!(f, "no open recording transaction"),
            Self::UnknownTransaction { id } => {
                write!(f, "unknown recording transaction {id}")
            }
            Self::Conflict {
                fixed,
                current,
                affected_targets,
            } => write!(
                f,
                "recording revision conflict: fixed {fixed:?}, current {current:?}, \
                 {} affected target(s)",
                affected_targets.len()
            ),
            Self::Rejected { reason } => write!(f, "recording commit rejected: {reason}"),
            Self::AlreadyApplied { id, revision } => {
                write!(
                    f,
                    "recording transaction {id} already applied at {revision:?}"
                )
            }
            Self::IntegrityConflict { id } => {
                write!(f, "recording transaction {id} payload mismatch")
            }
            Self::Invalid(msg) => write!(f, "recording commit invalid: {msg}"),
        }
    }
}

impl std::error::Error for RecordCommitError {}

/// Everything captured at gesture begin (issue #131 `GestureFixation`):
/// the execution context the write may not float away from — the
/// target itself, the project revision, the arm state and the clock
/// context. Inspector follow/focus changes later cannot retarget the
/// write or reinterpret the captured context.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlGestureFixation {
    /// The target the gesture bound to at begin.
    pub target: ParameterTargetId,
    /// Live project revision at begin.
    pub revision: ProjectRevision,
    /// Write arm state at begin.
    pub arm: WriteArm,
    /// Transport clock epoch at begin.
    pub epoch: ClockEpochId,
    /// Engine frame clock base at begin.
    pub frame_base: TimeBase,
}

/// An in-progress manual gesture on one target.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlGesture {
    /// Position where the gesture began.
    pub started_at: BeatPoint,
    /// `(pos, normalized value)` pairs in arrival order.
    pub points: Vec<(BeatPoint, f64)>,
    /// Context fixed at begin — immutable for the gesture's lifetime.
    pub fixation: ControlGestureFixation,
}

/// One recording session = one undo transaction. Target set, lane
/// mapping, arm state, transport epoch context and project revision
/// are **fixed at start** — later Inspector follow/focus changes
/// cannot retarget the write. The recorded point columns are never
/// consumed before a successful [`ControlSession::finish_record_commit`]
/// (issue #98).
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingTransaction {
    /// Session-unique transaction id.
    pub id: RecordingTransactionId,
    /// Session start position (project beats).
    pub started_at: BeatPoint,
    /// Clock epoch captured at start (transport context).
    pub epoch: ClockEpochId,
    /// Targets armed+write-capable when `record_start` ran.
    pub fixed_targets: Vec<ParameterTargetId>,
    /// Target → write-lane mapping fixed at `record_start`.
    pub fixed_lanes: BTreeMap<ParameterTargetId, AutomationLaneId>,
    /// Live project revision at start.
    pub fixed_revision: ProjectRevision,
    /// The loop pass this transaction writes into — fixed at
    /// `record_start` (issue #133). A later wrap increments the
    /// transport's pass but never this one.
    pub pass: LoopPassId,
    /// Target → `(owner, time_scope)` fixed at `record_start` —
    /// recorded positions resolve against this captured context, so
    /// a later clip move or scope change cannot reinterpret them.
    pub fixed_scopes: BTreeMap<ParameterTargetId, (ControlOwner, AutomationTimeScope)>,
    /// Per-lane write-range coverage end **in project beats** — the
    /// exclusive end of the interval the pass actually covered. Set
    /// once, when the write range closes, only for lanes whose target
    /// had an active writer at the boundary; resolved into the lane's
    /// scope coordinate at `prepare_record_commit`.
    pub coverage_end: BTreeMap<AutomationLaneId, BeatPoint>,
    /// Collected manual (pre-modulation) points per destination lane.
    pub segments: BTreeMap<AutomationLaneId, Vec<AutomationPoint>>,
    /// Delivered write intervals per lane (scope coordinates — each
    /// `(first, last)` marks the extent one accepted batch covered,
    /// issue #134). Bounded by [`RECORD_DELIVERY_CAP`].
    pub deliveries: BTreeMap<AutomationLaneId, Vec<(BeatPoint, BeatPoint)>>,
    /// Delivery intervals dropped because [`RECORD_DELIVERY_CAP`] was
    /// reached — counted, never silent.
    pub deliveries_dropped: u64,
    /// Explicit data-loss runs per lane in **project beats** (issue
    /// #134): dropped record events fold here at the next drain —
    /// never smoothed into the recorded series. Bounded by
    /// [`RECORD_GAP_CAP`]; further drops merge into the last gap.
    pub record_gaps: BTreeMap<AutomationLaneId, Vec<RecordGap>>,
    /// Drops merged into an at-capacity gap ledger — counted, never
    /// silent.
    pub record_gaps_merged: u64,
    /// Total dropped events attributed to this transaction.
    pub record_dropped: u64,
    /// `Some` once sealed — further `record_write` is rejected.
    pub sealed: Option<SealReason>,
    /// Commit lifecycle state.
    pub state: RecordingTxnState,
}

/// The non-destructive output of
/// [`ControlSession::prepare_record_commit`]: an immutable snapshot of
/// what a commit would apply, ready for the controller's CAS.
#[derive(Clone, Debug)]
pub struct PreparedRecordingCommit {
    /// The transaction this commit was prepared from.
    pub id: RecordingTransactionId,
    /// Deterministic fingerprint of the transaction payload.
    pub fingerprint: u64,
    /// Revision the CAS will require — the transaction's
    /// `fixed_revision`, never silently rebound to a newer one.
    pub expected_revision: ProjectRevision,
    /// `Composite` of per-lane `Replace` region writes; `None` when
    /// the transaction recorded nothing.
    pub op: Option<ProjectOp>,
    /// Whether the transaction had been sealed at prepare time.
    pub was_sealed: bool,
}

/// Per-lane report inside [`RebasePreview`]: what an explicit rebase
/// commit would write and what it would overwrite.
#[derive(Clone, Debug, PartialEq)]
pub struct RebaseLaneReport {
    /// The recorded-into lane.
    pub lane: AutomationLaneId,
    /// The fixed target that recorded into the lane.
    pub fixed_target: ParameterTargetId,
    /// Whether the fixed target still exists.
    pub target_exists: bool,
    /// Whether the lane still exists and still feeds `fixed_target`.
    pub lane_exists: bool,
    /// Points recorded into this lane.
    pub recorded_points: usize,
    /// `(first, last)` recorded position, when the lane is non-empty.
    pub recorded_range: Option<(BeatPoint, BeatPoint)>,
    /// Existing regions on any lane of `fixed_target` overlapping the
    /// recorded range — what a `Replace` write would overwrite.
    pub overlapping_regions: usize,
}

/// What the user inspects before approving an explicit rebase: target
/// and lane existence, recorded extents, and overlap with current
/// regions — never an automatic merge.
#[derive(Clone, Debug, PartialEq)]
pub struct RebasePreview {
    /// The transaction under inspection.
    pub id: RecordingTransactionId,
    /// Revision fixed at `record_start`.
    pub fixed_revision: ProjectRevision,
    /// Live revision now.
    pub current_revision: ProjectRevision,
    /// Whether/why the write gate was sealed.
    pub sealed: Option<SealReason>,
    /// Per-recorded-lane reports, in lane order.
    pub lanes: Vec<RebaseLaneReport>,
}

/// The most recent lifecycle outcome of a transaction — used to map
/// an id-less session retry onto `AlreadyApplied`/`NoTransaction`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RecordDisposition {
    /// The transaction was committed at this revision (`None` for an
    /// empty session that applied nothing).
    Committed {
        /// The committed transaction id.
        id: RecordingTransactionId,
        /// The revision the commit produced, when an apply happened.
        revision: Option<ProjectRevision>,
    },
    /// The transaction was aborted — nothing was applied.
    Aborted {
        /// The aborted transaction id.
        id: RecordingTransactionId,
    },
}

/// What a committed transaction left in the applied registry
/// (in-memory, session-scoped, capacity-bounded).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AppliedRecord {
    /// Fingerprint of the committed payload.
    pub fingerprint: u64,
    /// The revision the commit produced.
    pub revision: ProjectRevision,
}

/// In-memory, session-scoped exactly-once window: the last
/// `APPLIED_REGISTRY_CAP` committed transactions. No crash recovery
/// or persisted registry is claimed (issue #12's separate contract).
pub const APPLIED_REGISTRY_CAP: usize = 64;

/// Total automation points one transaction may hold across all lanes.
/// Writes that would exceed the budget are rejected; recorded points
/// are never dropped (non-RT memory/size bound, REC-C1-06).
pub const RECORD_POINT_BUDGET: usize = 262_144;

// ---------------------------------------------------------------------------
// bounded record-event queue + health (issue #134)
// ---------------------------------------------------------------------------

/// Capacity of the RT→non-RT record-event ring (`ruviform_rt` SPSC):
/// fixed, wait-free and allocation-free for the producer. A full
/// queue is a typed [`RecordPushError::QueueFull`], never a silent
/// drop.
pub const RECORD_EVENT_QUEUE_CAP: usize = 4_096;
/// Default events one [`ControlSession::drain_record_events`]
/// consumes — the non-RT drain stays bounded so a starved RT path
/// never waits on an unbounded batch.
pub const RECORD_DRAIN_BUDGET: usize = 4_096;
/// Capacity bound of the session's record-notification log.
pub const RECORD_NOTIFY_CAP: usize = 256;
/// Capacity bound of a transaction's per-lane record-gap ledger.
pub const RECORD_GAP_CAP: usize = 256;
/// Capacity bound of the session's per-target record-health table.
pub const RECORD_HEALTH_CAP: usize = 64;
/// Capacity bound of a transaction's per-lane delivery ledger.
pub const RECORD_DELIVERY_CAP: usize = 4_096;
/// Capacity bound of a drain report's retained rejection list.
pub const RECORD_REJECT_CAP: usize = 256;

/// Tunable bounds of the record path (issue #134 REC-C2). Every
/// budget is a real limit — hitting one produces a typed
/// [`RecordNotification`]/counter, never a silent truncation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RecordBudgets {
    /// Point columns one transaction may hold (REC-C1-06 bound).
    pub max_points: usize,
    /// Events one `drain_record_events` consumes at most.
    pub max_events_per_drain: usize,
}

impl Default for RecordBudgets {
    fn default() -> Self {
        Self {
            max_points: RECORD_POINT_BUDGET,
            max_events_per_drain: RECORD_DRAIN_BUDGET,
        }
    }
}

/// One automation record event pushed from the RT side — the intent
/// to record `(pos, value)`. Only the non-RT drain mutates the
/// transaction; the RT path is bounded, wait-free and allocation-free.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AutomationRecordEvent {
    /// The transaction the event was pushed under.
    pub txn: RecordingTransactionId,
    /// The armed write target.
    pub target: ParameterTargetId,
    /// Project position (project beats).
    pub pos: BeatPoint,
    /// The manual (pre-modulation) normalized value.
    pub value: f64,
}

/// Why [`ControlSession::record_event_rt`] rejected the event — a
/// typed rejection, never a silent drop.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RecordPushError {
    /// Write gates closed (target not armed / not recording / not
    /// rolling / transaction sealed).
    GatesClosed,
    /// The event itself is not recordable (non-finite or out-of-domain
    /// value, position at/past the loop end).
    Invalid,
    /// The bounded RT queue is full — the event could not be enqueued.
    /// Counted on the target's [`RecordTargetHealth::queue_fulls`].
    QueueFull,
}

impl fmt::Display for RecordPushError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GatesClosed => write!(f, "record gates closed"),
            Self::Invalid => write!(f, "record event invalid"),
            Self::QueueFull => write!(f, "record event queue full"),
        }
    }
}

impl std::error::Error for RecordPushError {}

/// A run of dropped record events — an **explicit data gap** in the
/// recording, never smoothed into a curve (issue #134). Positions are
/// project beats; `dropped` counts the events lost in the run.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RecordGap {
    /// First dropped position of the run.
    pub start: BeatPoint,
    /// Last dropped position of the run.
    pub end: BeatPoint,
    /// Events dropped in this run.
    pub dropped: u64,
}

/// Why the drain rejected an event — each kind is a distinct, named
/// state (issue #134): the failure is reported, counted and leaves a
/// gap rather than disappearing.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecordRejectKind {
    /// The event's transaction is stale — it was pushed under a
    /// transaction that already ended.
    WrongTxn,
    /// The write gates closed between push and drain.
    GatesClosed,
    /// The position lies at/past the pass end (issue #133).
    OutsidePass,
    /// Non-finite or out-of-domain value.
    BadValue,
    /// The scope could not resolve the position — the point is
    /// unattributable.
    ScopeLost,
    /// The fixed lane or target vanished from the project.
    LaneGone,
    /// The transaction point budget is full.
    OverBudget,
    /// Any other commit-path failure.
    Failed,
}

/// One rejected event retained by the drain report (bounded —
/// [`RECORD_REJECT_CAP`] entries; `rejected_total` keeps the count).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RecordRejection {
    /// The event that was rejected.
    pub event: AutomationRecordEvent,
    /// Why it was rejected.
    pub kind: RecordRejectKind,
}

/// A bounded report of one [`ControlSession::drain_record_events`]
/// pass. `partial` flags events still queued — the caller retries;
/// nothing reports "done" while the queue holds events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RecordDrainReport {
    /// Events popped this drain.
    pub popped: usize,
    /// Events accepted into the transaction.
    pub accepted: usize,
    /// Events pushed under a stale/no transaction.
    pub stale: usize,
    /// Events rejected because the point budget was full.
    pub overflowed: usize,
    /// Events rejected for any drain-side reason.
    pub rejected_total: usize,
    /// Retained rejections (bounded by [`RECORD_REJECT_CAP`]).
    pub rejected: Vec<RecordRejection>,
    /// `true` when the queue still holds events — the drain was
    /// partial and must be retried.
    pub partial: bool,
}

impl RecordDrainReport {
    fn reject(&mut self, event: AutomationRecordEvent, kind: RecordRejectKind) {
        self.rejected_total += 1;
        if self.rejected.len() < RECORD_REJECT_CAP {
            self.rejected.push(RecordRejection { event, kind });
        }
    }
}

/// A session notification about the record path (issue #134): the
/// bounded log keeps at most [`RECORD_NOTIFY_CAP`] entries; overflow
/// drops the oldest and counts on
/// [`ControlSession::record_notifications_dropped`].
#[derive(Clone, Debug, PartialEq)]
pub enum RecordNotification {
    /// RT queue-full drops for `target` since the last drain.
    QueueFull {
        /// The affected target.
        target: ParameterTargetId,
        /// The target's cumulative drop counter.
        dropped_total: u64,
    },
    /// A dropped-event run was folded into the transaction's gap
    /// ledger — an explicit data hole.
    DataGap {
        /// The lane that lost data.
        lane: AutomationLaneId,
        /// Gap start (project beats).
        start: BeatPoint,
        /// Gap end (project beats).
        end: BeatPoint,
        /// Events lost in the run.
        dropped: u64,
    },
    /// Drain-side rejects, aggregated per `(target, kind)`.
    Rejected {
        /// The affected target.
        target: ParameterTargetId,
        /// Why the events were rejected.
        kind: RecordRejectKind,
        /// How many events this drain rejected.
        count: u64,
    },
    /// The drain hit its event budget while the queue was non-empty —
    /// retry pending.
    Partial {
        /// Events accepted before the budget ran out.
        accepted: usize,
    },
}

/// Per-target record-path health — cumulative counters, gap-run
/// tracking and the notification watermark. Session state only.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordTargetHealth {
    /// The tracked target.
    pub target: ParameterTargetId,
    /// Events dropped for this target, any cause.
    pub dropped: u64,
    /// RT queue-full rejections (push side).
    pub queue_fulls: u64,
    /// Point-budget rejections (drain side).
    pub overflowed: u64,
    /// All other drain-side rejects.
    pub rejected: u64,
    /// First dropped position of the open run (project beats).
    pub gap_open: Option<BeatPoint>,
    /// Last dropped position of the open run.
    pub gap_last: Option<BeatPoint>,
    /// Drops accumulated in the open run.
    pub gap_dropped: u64,
    /// `queue_fulls` value last reported via [`RecordNotification`] —
    /// a delta watermark, so each full event notifies once.
    pub notified_fulls: u64,
}

impl RecordTargetHealth {
    fn new(target: ParameterTargetId) -> Self {
        Self {
            target,
            dropped: 0,
            queue_fulls: 0,
            overflowed: 0,
            rejected: 0,
            gap_open: None,
            gap_last: None,
            gap_dropped: 0,
            notified_fulls: 0,
        }
    }
}

/// The read-only snapshot of one target's record health.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct RecordHealthSummary {
    /// The tracked target.
    pub target: ParameterTargetId,
    /// Events dropped for this target, any cause.
    pub dropped: u64,
    /// RT queue-full rejections.
    pub queue_fulls: u64,
    /// Point-budget rejections.
    pub overflowed: u64,
    /// All other rejects.
    pub rejected: u64,
}

impl From<&RecordTargetHealth> for RecordHealthSummary {
    fn from(h: &RecordTargetHealth) -> Self {
        Self {
            target: h.target,
            dropped: h.dropped,
            queue_fulls: h.queue_fulls,
            overflowed: h.overflowed,
            rejected: h.rejected,
        }
    }
}

/// A record gap resolved into the target's scope coordinate for
/// display — `f64`, consistent with [`synvelis_project::CurveQuery`].
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ResolvedRecordGap {
    /// Gap start in scope coordinates.
    pub start: f64,
    /// Gap end in scope coordinates.
    pub end: f64,
    /// Events lost in the gap.
    pub dropped: u64,
}

/// The Automation-Lane projection (issue #134): the bounded LOD
/// curve **plus** the session-only overlays — uncommitted recorded
/// points, delivered write intervals, explicit record gaps and
/// record-path health. Read-only; session data only annotates, the
/// project stays the single source of truth.
#[derive(Clone, Debug, PartialEq)]
pub struct AutomationLaneView {
    /// The projected target.
    pub target: ParameterTargetId,
    /// The project revision the projection was taken at — a stale
    /// revision in the UI reports staleness, not a silent mismatch.
    pub revision: ProjectRevision,
    /// The model-side bounded LOD curve.
    pub curve: AutomationCurveView,
    /// Uncommitted recorded points per lane inside the window —
    /// the recording session's live overlay.
    pub pending: Vec<(AutomationLaneId, Vec<AutomationPoint>)>,
    /// `pending` was cut by `max_points` — the count stays honest
    /// via `pending_truncated`.
    pub pending_truncated: bool,
    /// Delivered write intervals inside the window (scope `f64`).
    pub deliveries: Vec<(f64, f64)>,
    /// Record gaps resolved to the scope coordinate.
    pub gaps: Vec<ResolvedRecordGap>,
    /// Gaps that could not be resolved to the scope coordinate —
    /// counted, never silent.
    pub gaps_unresolved: usize,
    /// The target's record health, when any record event was seen.
    pub health: Option<RecordHealthSummary>,
}

/// An observed value plus its measurement stamp (rf#185 vocabulary).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ObservationData {
    /// Normalized value reported.
    pub value: f64,
    /// When/where the value was measured and how trustworthy the
    /// stamp is (`Carried` > `Derived` > `Estimated`).
    pub stamp: MediaTimestamp,
}

/// Per-target observation channels: what the plugin *reported* and
/// what a probe *captured*. Distinct from the host-resolved
/// evaluation — observation is measurement, not computation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ControlObservation {
    /// Latest plugin-reported value + stamp.
    pub plugin_reported: Option<ObservationData>,
    /// Latest probe-captured value + stamp.
    pub probe: Option<ObservationData>,
}

/// The observation's freshness/availability state.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ObservationState {
    /// An observation exists and transport is stopped — the value
    /// corresponds to the playhead.
    AtPlayhead,
    /// An observation exists and transport is rolling.
    Live,
    /// The value was captured via the probe channel.
    Captured,
    /// The observation's stamp provenance is `Estimated` — it cannot
    /// be trusted as a measured fact.
    Stale,
    /// No observation has ever arrived.
    Unavailable,
}

/// What [`ControlSession::transport_advance`] reports.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransportEffect {
    /// The playhead wrapped to the loop start.
    pub wrapped: bool,
    /// The open recording transaction was sealed by the wrap.
    pub sealed: bool,
}

/// What [`ControlSession::gesture_end`] decided. `BaseEdit` is the
/// only variant that needs a project apply (performed by
/// [`ProjectSession::control_gesture_end`]).
///
/// [`ProjectSession::control_gesture_end`]: crate::session::ProjectSession::control_gesture_end
#[derive(Clone, Debug, PartialEq)]
pub enum GestureEnd {
    /// `Off` mode: apply `SetParameterBase` (one undoable edit).
    BaseEdit {
        /// The gesture's last normalized value.
        value: f64,
    },
    /// Nothing was written; the manual override was discarded and the
    /// target returns to its recorded series.
    Returned,
    /// The gesture's points were appended to the open recording
    /// transaction.
    Written {
        /// Number of points recorded.
        points: usize,
    },
    /// `Latch`: gesture points written and the last value is held for
    /// continued control-rate writing.
    Latched {
        /// The held normalized value.
        value: f64,
    },
}

/// Result of lowering an evaluated value into an rf#186 event.
#[derive(Clone, Debug, PartialEq)]
pub enum Lowered {
    /// The requested means was accepted verbatim.
    Exact(TimedParamEvent),
    /// The requested means was rejected; delivery degraded to an
    /// explicit [`ParamWriteMeans::BlockBoundary`] event instead —
    /// reported, never silent.
    Degraded {
        /// The means the caller originally requested.
        requested: ParamWriteMeans,
        /// The event actually produced.
        event: TimedParamEvent,
    },
}

/// What a committed recording session produced.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingOutcome {
    /// The consumed transaction id.
    pub id: RecordingTransactionId,
    /// Whether a project apply happened (empty sessions commit
    /// nothing — no undo entry is created).
    pub applied: bool,
    /// The new project revision when `applied`.
    pub revision: Option<ProjectRevision>,
    /// Whether the transaction had been sealed (loop wrap).
    pub was_sealed: bool,
}

/// Default Touch return duration: the provisional 150 ms agreed for
/// AUT-01 (issue #131) — a later pass approves the final default.
pub const DEFAULT_TOUCH_RETURN_MS: i64 = 150;

/// Capacity bound of the [`ControlSession`] writer-closure log.
pub const WRITER_CLOSE_CAP: usize = 256;

/// An in-flight Touch return: after a `Touch` gesture releases, the
/// reference glides back to the automation series over the session's
/// return duration instead of jumping (issue #131). Runtime state
/// only — the return never writes points, never mutates the project.
#[derive(Clone, Debug, PartialEq)]
pub struct TouchReturn {
    /// Normalized reference value at gesture release.
    pub release_value: f64,
    /// Session clock instant at release (`frame_base` ticks).
    pub released_at: MediaTime,
}

/// The per-target runtime writer state — the inspectable half of the
/// mode boundary contract (issue #131). Session state only; no variant
/// implies a project mutation by itself.
#[derive(Clone, Debug, PartialEq)]
pub enum WriterState {
    /// No gesture, held value or return is active.
    Idle,
    /// A manual gesture is in flight (the override is live).
    Gesture,
    /// `Latch`: the released value keeps writing while the gates hold.
    LatchHeld {
        /// The held normalized value.
        value: f64,
    },
    /// `Write`: the reference fixed at `record_start` (or the last
    /// gesture value) keeps writing while the gates hold.
    WriteHeld {
        /// The held normalized value.
        value: f64,
    },
    /// `Touch`: the released value is returning to the automation
    /// series over the configured duration.
    Returning {
        /// The normalized reference at release.
        release_value: f64,
        /// `x = clamp(elapsed / duration, 0, 1)` on the session clock.
        progress: f64,
        /// `true` when the configured duration is 0 — an explicit
        /// instant return whose click/zipper risk must be visible.
        instant: bool,
    },
}

/// Why runtime gesture/writer state was explicitly closed (issue #131
/// boundary events). Every variant is an *observable* transition —
/// recorded in the session's writer-closure log.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WriterCloseReason {
    /// The transport stopped.
    TransportStop,
    /// The playhead was repositioned.
    TransportSeek,
    /// The playhead wrapped the loop end.
    LoopWrap,
    /// A new transport run began (non-`Playing` → `Playing` edge;
    /// `Paused` → `Playing` via `transport_resume` is the same
    /// playback and does **not** close).
    NewTransportPlay,
    /// Window/IME/plugin-GUI input focus moved away mid-gesture.
    FocusLost,
    /// The owning processor reported a plugin crash.
    PluginCrash,
    /// The target was deleted from the project.
    TargetDeleted,
    /// The selected mode changed (held `Latch`/`Write` state only —
    /// an in-flight return is mode-independent and completes).
    ModeChanged,
    /// The gesture was cancelled explicitly.
    Cancelled,
}

/// One explicit closure of runtime gesture/writer state — what closed
/// and which state it had.
#[derive(Clone, Debug, PartialEq)]
pub struct WriterClosure {
    /// The target whose state was closed.
    pub target: ParameterTargetId,
    /// The boundary event that closed it.
    pub reason: WriterCloseReason,
    /// The state at close time.
    pub state: WriterState,
}

/// A lane/Signal-Flow projection row — borrowed from `Project::control`;
/// the projection holds no copy of the series.
#[derive(Clone, Debug, PartialEq)]
pub struct AssignmentView<'a> {
    /// The assignment ID.
    pub id: ModulationAssignmentId,
    /// The source entity feeding it.
    pub source: &'a synvelis_project::ModulationSource,
    /// Normalized depth.
    pub depth: f64,
    /// Optional depth driver.
    pub depth_source: Option<ParameterTargetId>,
    /// Whether the assignment is live.
    pub enabled: bool,
}

/// The Inspector's combined breakdown for one target at one position:
/// evaluation + observation + UI mode + arm + capability — the `#6`
/// agreed value-projection shape.
#[derive(Clone, Debug, PartialEq)]
pub struct InspectorBreakdown {
    /// The host-resolved evaluation breakdown.
    pub evaluation: ControlEvaluation,
    /// Observation freshness state.
    pub observation: ObservationState,
    /// Plugin-reported value, if any.
    pub plugin_reported: Option<ObservationData>,
    /// Probe-captured value, if any.
    pub probe: Option<ObservationData>,
    /// Selected UI automation mode.
    pub mode: AutomationMode,
    /// Write arm state.
    pub arm: WriteArm,
    /// Runtime writer state (issue #131 boundary introspection).
    pub writer_state: WriterState,
    /// Capability contract (fixture `ParamInfo`), if declared.
    pub capability: Option<TargetCapability>,
}

/// The session-scoped control state: UI modes, write arms, gestures,
/// the recording transaction, playback and observations. Owns **no**
/// persisted data — the project's `control` model is the only series.
#[derive(Debug)]
pub struct ControlSession {
    modes: BTreeMap<ParameterTargetId, AutomationMode>,
    arms: BTreeMap<ParameterTargetId, WriteArm>,
    record_enabled: bool,
    playback: PlaybackState,
    gestures: BTreeMap<ParameterTargetId, ControlGesture>,
    manual_overrides: BTreeMap<ParameterTargetId, f64>,
    latch_held: BTreeMap<ParameterTargetId, f64>,
    recording: Option<RecordingTransaction>,
    observations: BTreeMap<ParameterTargetId, ControlObservation>,
    write_lanes: BTreeMap<ParameterTargetId, AutomationLaneId>,
    clock_anchor: Option<ClockAnchor>,
    frame_base: TimeBase,
    next_txn_id: u64,
    applied: BTreeMap<RecordingTransactionId, AppliedRecord>,
    last_disposition: Option<RecordDisposition>,
    touch_returns: BTreeMap<ParameterTargetId, TouchReturn>,
    clock_now: MediaTime,
    return_ticks: i64,
    writer_closes: VecDeque<WriterClosure>,
    record_producer: Producer<AutomationRecordEvent>,
    record_consumer: Consumer<AutomationRecordEvent>,
    record_budgets: RecordBudgets,
    record_health: Vec<RecordTargetHealth>,
    record_notifications: VecDeque<RecordNotification>,
    record_health_overflows: u64,
    record_notifications_dropped: u64,
    pub(crate) probe_session: ProbeTraceSession,
}

impl Default for ControlSession {
    fn default() -> Self {
        Self::new(TimeBase::COMMON_90K)
    }
}

impl ControlSession {
    /// A session with `frame_base` as the engine frame clock.
    #[must_use]
    pub fn new(frame_base: TimeBase) -> Self {
        // The provisional 150 ms default expressed in `frame_base`
        // ticks (150·den/(num·1000), truncating toward zero — exact
        // for every sample-rate-derived base).
        let return_ticks = TimeBase::new(1, 1_000).map_or(0, |ms| {
            MediaDuration::new(DEFAULT_TOUCH_RETURN_MS, ms)
                .convert(frame_base, Rounding::TowardZero)
                .map_or(0, MediaDuration::value)
        });
        let (record_producer, record_consumer) = ruviform_rt::channel(RECORD_EVENT_QUEUE_CAP);
        Self {
            modes: BTreeMap::new(),
            arms: BTreeMap::new(),
            record_enabled: false,
            playback: PlaybackState::default(),
            gestures: BTreeMap::new(),
            manual_overrides: BTreeMap::new(),
            latch_held: BTreeMap::new(),
            recording: None,
            observations: BTreeMap::new(),
            write_lanes: BTreeMap::new(),
            clock_anchor: None,
            frame_base,
            next_txn_id: 0,
            applied: BTreeMap::new(),
            last_disposition: None,
            touch_returns: BTreeMap::new(),
            clock_now: MediaTime::new(0, frame_base),
            return_ticks,
            writer_closes: VecDeque::new(),
            record_producer,
            record_consumer,
            record_budgets: RecordBudgets::default(),
            record_health: Vec::with_capacity(RECORD_HEALTH_CAP),
            record_notifications: VecDeque::with_capacity(RECORD_NOTIFY_CAP),
            record_health_overflows: 0,
            record_notifications_dropped: 0,
            probe_session: ProbeTraceSession::new(),
        }
    }

    fn target(
        project: &Project,
        id: ParameterTargetId,
    ) -> Result<&synvelis_project::ParameterTarget, Error> {
        project
            .control
            .targets
            .get(&id)
            .ok_or_else(|| Error::NotFound(format!("param {id}")))
    }

    // -- mode / arm (session-only, never a project mutation) ----------

    /// The target's selected UI mode (default `Read`).
    #[must_use]
    pub fn mode(&self, target: ParameterTargetId) -> AutomationMode {
        self.modes.get(&target).copied().unwrap_or_default()
    }

    /// The target's write arm (default `Disarmed`).
    #[must_use]
    pub fn arm(&self, target: ParameterTargetId) -> WriteArm {
        self.arms.get(&target).copied().unwrap_or_default()
    }

    /// Whether automation record is enabled.
    #[must_use]
    pub fn record_enabled(&self) -> bool {
        self.record_enabled
    }

    /// Selects the UI automation mode. Session-only: no project apply,
    /// no revision bump, no undo entry. Leaving `Latch`/`Write` ends
    /// any held write for the target.
    ///
    /// # Errors
    /// [`Error::NotFound`] for an unknown target.
    pub fn set_mode(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
        mode: AutomationMode,
    ) -> Result<(), Error> {
        Self::target(project, target)?;
        if self.mode(target) != mode {
            // A fixed target's mode change mid-recording closes the
            // write range permanently — the transaction's captured
            // context no longer holds. The close runs before the new
            // mode lands so a held `Write` writer is still counted
            // for coverage at the boundary (issue #133).
            self.seal_fixed_target(project, target, SealReason::ModeChanged);
            self.modes.insert(target, mode);
        }
        // Leaving `Latch`/`Write` drops the held write value — an
        // explicit closure, recorded. An in-flight `Touch` return is
        // mode-independent (it only finishes the release) and runs on.
        if mode != AutomationMode::Latch
            && let Some(value) = self.latch_held.remove(&target)
        {
            self.log_closure(
                target,
                WriterCloseReason::ModeChanged,
                WriterState::LatchHeld { value },
            );
        }
        if mode != AutomationMode::Write
            && !self.gestures.contains_key(&target)
            && let Some(value) = self.manual_overrides.remove(&target)
        {
            self.log_closure(
                target,
                WriterCloseReason::ModeChanged,
                WriterState::WriteHeld { value },
            );
        }
        Ok(())
    }

    /// Arms/disarms a target for writing (session-only).
    ///
    /// # Errors
    /// [`Error::NotFound`] for an unknown target.
    pub fn set_arm(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
        arm: WriteArm,
    ) -> Result<(), Error> {
        Self::target(project, target)?;
        if self.arm(target) != arm {
            self.arms.insert(target, arm);
            self.seal_fixed_target(project, target, SealReason::ArmChanged);
        }
        Ok(())
    }

    /// Enables/disables automation recording (session-only).
    pub fn set_record_enabled(&mut self, enabled: bool) {
        self.record_enabled = enabled;
    }

    /// Fixes which automation lane a write-mode target records into.
    ///
    /// # Errors
    /// [`Error::Validation`] when the lane does not feed `target`, or
    /// when `target` is fixed by an open recording transaction — the
    /// transaction's lane mapping was captured at `record_start` and
    /// cannot be rebound underneath it.
    pub fn bind_write_lane(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
        lane: AutomationLaneId,
    ) -> Result<(), Error> {
        if self
            .recording
            .as_ref()
            .is_some_and(|tx| tx.fixed_targets.contains(&target))
        {
            return Err(Error::Validation(format!(
                "cannot rebind {target} while its recording transaction is open"
            )));
        }
        let lane_ref = project
            .control
            .lanes
            .get(&lane)
            .ok_or_else(|| Error::NotFound(format!("automation lane {lane}")))?;
        if lane_ref.target != target {
            return Err(Error::Validation(format!(
                "lane {lane} does not feed param {target}"
            )));
        }
        self.write_lanes.insert(target, lane);
        Ok(())
    }

    /// Seals the open transaction when `target` is one of its fixed
    /// targets — a mode/arm change on it permanently invalidates the
    /// captured recording context.
    fn seal_fixed_target(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
        reason: SealReason,
    ) {
        if self
            .recording
            .as_ref()
            .is_some_and(|tx| tx.sealed.is_none() && tx.fixed_targets.contains(&target))
        {
            self.close_write_range(project, reason, self.playback.position);
        }
    }

    /// All three write gates: armed + record enabled + rolling, with an
    /// open, unsealed transaction that fixed this target.
    fn write_gates(&self, target: ParameterTargetId) -> bool {
        self.arm(target) == WriteArm::Armed
            && self.record_enabled
            && self.playback.status == PlaybackStatus::Playing
            && self
                .recording
                .as_ref()
                .is_some_and(|r| r.sealed.is_none() && r.fixed_targets.contains(&target))
    }

    // -- playback ------------------------------------------------------

    /// The playback state.
    #[must_use]
    pub fn playback(&self) -> &PlaybackState {
        &self.playback
    }

    /// Starts rolling. Session-only. A non-`Playing` → `Playing` edge
    /// is a **new** transport run: every gesture/writer state is
    /// explicitly closed — nothing carries over from the previous run.
    /// (`transport_resume` from `Paused` continues the same playback
    /// and does not close.) The edge also starts loop pass `1` and
    /// seals an open recording transaction — a new run never resumes
    /// an old pass (issue #133).
    pub fn transport_play(&mut self, project: &Project) {
        if self.playback.status != PlaybackStatus::Playing {
            self.close_write_range(
                project,
                SealReason::NewTransportPlay,
                self.playback.position,
            );
            self.close_runtime_state(WriterCloseReason::NewTransportPlay);
            self.playback.pass = LoopPassId(1);
        }
        self.playback.status = PlaybackStatus::Playing;
    }

    /// Pauses a rolling transport: the playhead holds its position and
    /// latch-held writes stay armed, but record gates close — nothing
    /// lands while paused. A no-op when not `Playing`.
    pub fn transport_pause(&mut self) {
        if self.playback.status == PlaybackStatus::Playing {
            self.playback.status = PlaybackStatus::Paused;
        }
    }

    /// Resumes a paused transport. A no-op unless `Paused` — resume
    /// never starts a stopped transport.
    pub fn transport_resume(&mut self) {
        if self.playback.status == PlaybackStatus::Paused {
            self.playback.status = PlaybackStatus::Playing;
        }
    }

    /// Stops rolling. Every gesture/writer state is explicitly closed —
    /// latch holds, write-held overrides, in-flight gestures and
    /// returns never carry into the next playback. An open recording
    /// transaction has its write range closed at the current position
    /// and is sealed: a stopped session never resumes into the same
    /// transaction on the next play.
    pub fn transport_stop(&mut self, project: &Project) {
        self.playback.status = PlaybackStatus::Stopped;
        self.close_write_range(project, SealReason::Stopped, self.playback.position);
        self.close_runtime_state(WriterCloseReason::TransportStop);
    }

    /// Seeks the playhead (chase = the next evaluation simply reads the
    /// new position). Session-only. Gesture/writer state is explicitly
    /// closed and an open recording transaction's write range closes
    /// at the pre-seek position — the recorded interval is no longer
    /// contiguous.
    pub fn transport_seek(&mut self, project: &Project, pos: BeatPoint) {
        let bound = self.playback.position;
        self.playback.position = pos;
        self.close_write_range(project, SealReason::Seek, bound);
        self.close_runtime_state(WriterCloseReason::TransportSeek);
    }

    /// Closes the open transaction's write range and seals its write
    /// gate with `reason` (issue #133). First-seal-wins: an already
    /// sealed transaction is left untouched and `false` is returned.
    ///
    /// `bound` is the exclusive end of the recorded interval in
    /// project beats (clamped to the loop end when a loop is set):
    /// the loop `end` for a wrap, the current position for every
    /// other boundary.
    ///
    /// Two things happen while the transaction is still unsealed:
    ///
    /// 1. **Flush** — an in-flight gesture on a fixed target has its
    ///    delivered points appended (`pos <= bound`, and `pos < end`
    ///    under a loop): they are part of the pass's recording, not a
    ///    discardable preview. The gesture itself still closes as a
    ///    runtime boundary. Flush is best-effort — a lane that
    ///    vanished mid-recording fails the commit's structural check
    ///    anyway, so the close never aborts on a failed append.
    /// 2. **Coverage** — a fixed target with an active writer
    ///    (in-flight gesture, held `Latch`, held `Write`) marks its
    ///    lane's `coverage_end` at `bound`: a held write covered the
    ///    interval continuously, which `last point + ε` cannot
    ///    express. The recorded columns stay for inspect/commit/abort.
    fn close_write_range(
        &mut self,
        project: &Project,
        reason: SealReason,
        bound: BeatPoint,
    ) -> bool {
        let loop_end = self.playback.loop_range.map(|(_, end)| end);
        let bound = loop_end.map_or(bound, |end| bound.min(end));
        let Some(tx) = self.recording.as_ref() else {
            return false;
        };
        if tx.sealed.is_some() {
            return false;
        }
        let fixed: Vec<(ParameterTargetId, AutomationLaneId)> = tx
            .fixed_lanes
            .iter()
            .map(|(target, lane)| (*target, *lane))
            .collect();
        for (target, _) in &fixed {
            let Some(g) = self.gestures.get(target) else {
                continue;
            };
            let clipped: Vec<(BeatPoint, f64)> = g
                .points
                .iter()
                .copied()
                .filter(|(pos, _)| *pos <= bound && loop_end.is_none_or(|end| *pos < end))
                .collect();
            if !clipped.is_empty() {
                let _ = self.record_points(project, *target, &clipped);
            }
        }
        let covered: Vec<AutomationLaneId> = fixed
            .iter()
            .filter(|(target, _)| {
                self.gestures.contains_key(target)
                    || self.latch_held.contains_key(target)
                    || (self.manual_overrides.contains_key(target)
                        && self.mode(*target) == AutomationMode::Write)
            })
            .map(|(_, lane)| *lane)
            .collect();
        let Some(tx) = self.recording.as_mut() else {
            return false;
        };
        for lane in covered {
            tx.coverage_end.insert(lane, bound);
        }
        tx.sealed = Some(reason);
        true
    }

    /// Sets or clears the loop range. Changing it while a recording
    /// transaction is open seals the write range — the pass
    /// boundary's coordinate basis no longer holds (issue #133).
    ///
    /// # Errors
    /// [`Error::Validation`] when `end <= start`.
    pub fn set_loop(
        &mut self,
        project: &Project,
        range: Option<(BeatPoint, BeatPoint)>,
    ) -> Result<(), Error> {
        if let Some((start, end)) = range
            && end <= start
        {
            return Err(Error::Validation("loop end must exceed start".into()));
        }
        if self.playback.loop_range != range {
            self.close_write_range(project, SealReason::LoopChanged, self.playback.position);
        }
        self.playback.loop_range = range;
        Ok(())
    }

    /// Advances the playhead to `to`. Crossing the loop end wraps to
    /// the loop start and seals the open recording transaction (the
    /// loop interval is never silently overwritten). While rolling,
    /// armed `Latch`/`Write` held values land at the new position
    /// (control-rate write).
    ///
    /// # Errors
    /// Propagates [`ControlSession::record_write`] failures.
    pub fn transport_advance(
        &mut self,
        project: &Project,
        to: BeatPoint,
    ) -> Result<TransportEffect, Error> {
        let mut effect = TransportEffect::default();
        let mut pos = to;
        if let Some((start, end)) = self.playback.loop_range
            && to >= end
        {
            pos = start;
            effect.wrapped = true;
            // The pass ends at the loop end — `end` itself belongs to
            // the next pass and stays outside the half-open write
            // range (issue #133). The wrap boundary explicitly closes
            // every gesture/writer state — a return or held write
            // anchored to the old position must not straddle the
            // wrap.
            effect.sealed = self.close_write_range(project, SealReason::LoopWrap, end);
            self.close_runtime_state(WriterCloseReason::LoopWrap);
            self.playback.pass = LoopPassId(self.playback.pass.0.saturating_add(1));
        }
        self.playback.position = pos;
        // The engine frame is derived from `position` inside
        // `stamp_gesture` — nothing to keep fresh here.
        // Control-rate continuous write for held values.
        let held: Vec<(ParameterTargetId, f64)> = self
            .latch_held
            .iter()
            .chain(
                self.manual_overrides
                    .iter()
                    .filter(|(t, _)| self.mode(**t) == AutomationMode::Write),
            )
            .map(|(t, v)| (*t, *v))
            .collect();
        for (target, value) in held {
            // Held writes land only while the gates hold — a pause, a
            // sealed or closed transaction, or a disarmed target mutes
            // the write, never the clock. The held value survives for
            // a later transaction.
            if self.write_gates(target) {
                self.record_write(project, target, pos, value)?;
            }
        }
        Ok(effect)
    }

    // -- session clock / touch return (issue #131) ---------------------

    /// Advances the deterministic session clock to `now` — the sole
    /// progress source for Touch returns. The host drives it at
    /// control rate, independent of the beat playhead; a return
    /// completes even while the transport is stopped or paused. `now`
    /// must be exactly representable in `frame_base` and
    /// monotonically non-decreasing; returns past `x = 1` are swept.
    ///
    /// # Errors
    /// [`Error::Validation`] on a non-representable base or a
    /// non-monotonic move.
    pub fn clock_advance(&mut self, now: MediaTime) -> Result<(), Error> {
        let now = now
            .convert_exact(self.frame_base)
            .ok_or_else(|| Error::Validation("session clock base not representable".into()))?;
        if now < self.clock_now {
            return Err(Error::Validation("session clock is monotonic".into()));
        }
        self.clock_now = now;
        // Sweep completed returns — evaluation at `x = 1` equals the
        // automation value, so removal is behavior-preserving.
        let (now_v, dur) = (self.clock_now.value(), self.return_ticks);
        self.touch_returns
            .retain(|_, ret| now_v - ret.released_at.value() < dur);
        Ok(())
    }

    /// The session clock's current instant.
    #[must_use]
    pub fn clock_now(&self) -> MediaTime {
        self.clock_now
    }

    /// Sets the Touch return duration — the runtime interpolation
    /// window between release and the automation series. `0` is legal
    /// for an explicit user value but reports `instant` on
    /// [`WriterState::Returning`] — a zero-duration return is an
    /// audio click/zipper boundary, not a smooth fade.
    ///
    /// # Errors
    /// [`Error::Validation`] on a negative duration or a base not
    /// exactly representable in `frame_base`.
    pub fn set_return_duration(&mut self, duration: MediaDuration) -> Result<(), Error> {
        if duration.value() < 0 {
            return Err(Error::Validation("return duration must be >= 0".into()));
        }
        self.return_ticks = duration
            .convert_exact(self.frame_base)
            .ok_or_else(|| Error::Validation("return duration base not representable".into()))?
            .value();
        Ok(())
    }

    /// The configured Touch return duration.
    #[must_use]
    pub fn return_duration(&self) -> MediaDuration {
        MediaDuration::new(self.return_ticks, self.frame_base)
    }

    /// The target's runtime writer state — the inspectable contract
    /// behind the mode boundary matrix (issue #131): `Idle`,
    /// `Gesture`, `LatchHeld`, `WriteHeld` or `Returning`.
    #[must_use]
    pub fn writer_state(&self, target: ParameterTargetId) -> WriterState {
        if self.gestures.contains_key(&target) {
            return WriterState::Gesture;
        }
        if let Some(&value) = self.latch_held.get(&target) {
            return WriterState::LatchHeld { value };
        }
        if let Some(&value) = self.manual_overrides.get(&target) {
            return WriterState::WriteHeld { value };
        }
        if let Some(ret) = self.touch_returns.get(&target) {
            return WriterState::Returning {
                release_value: ret.release_value,
                progress: self.return_progress(ret),
                instant: self.return_ticks == 0,
            };
        }
        WriterState::Idle
    }

    /// The writer-closure log — every explicit boundary closure in
    /// order, bounded to [`WRITER_CLOSE_CAP`].
    #[must_use]
    pub fn writer_closes(&self) -> &VecDeque<WriterClosure> {
        &self.writer_closes
    }

    /// The context fixed at `gesture_begin` for `target`, if a gesture
    /// is in flight.
    #[must_use]
    pub fn gesture_fixation(&self, target: ParameterTargetId) -> Option<&ControlGestureFixation> {
        self.gestures.get(&target).map(|g| &g.fixation)
    }

    /// Window/IME/plugin-GUI input focus moved away: every in-flight
    /// gesture and held state is explicitly closed (issue #131 — a
    /// gesture cannot silently continue against a focus it no longer
    /// owns). Session-only; the recording transaction is untouched.
    pub fn note_focus_lost(&mut self) {
        self.close_runtime_state(WriterCloseReason::FocusLost);
    }

    /// The owning processor reported a plugin crash: runtime state of
    /// every target bound to that processor is explicitly closed —
    /// writes to a crashed plugin's lane must not continue.
    /// Session-only.
    pub fn note_plugin_crash(&mut self, project: &Project, processor: ProcessorId) {
        for target in self.runtime_targets() {
            if project
                .control
                .targets
                .get(&target)
                .is_some_and(|t| t.owner == ControlOwner::Processor(processor))
            {
                self.close_target_runtime(target, WriterCloseReason::PluginCrash);
            }
        }
    }

    /// `x = clamp(elapsed / return_duration, 0, 1)` on the session
    /// clock — the Touch return phase (documented lossy `f64`
    /// evaluation, same rule as curve interpolation).
    fn return_progress(&self, ret: &TouchReturn) -> f64 {
        if self.return_ticks <= 0 {
            return 1.0;
        }
        let elapsed = self.clock_now.value() - ret.released_at.value();
        #[allow(clippy::cast_precision_loss)] // documented lossy evaluation path
        (elapsed as f64 / self.return_ticks as f64).clamp(0.0, 1.0)
    }

    /// `w = 3x² - 2x³` — the Smoothstep weight driving the Touch
    /// return blend.
    fn return_blend(&self, ret: &TouchReturn) -> f64 {
        let x = self.return_progress(ret);
        3.0 * x * x - 2.0 * x * x * x
    }

    /// Every target with live runtime state (gesture / override /
    /// latch / return).
    fn runtime_targets(&self) -> BTreeSet<ParameterTargetId> {
        self.gestures
            .keys()
            .chain(self.manual_overrides.keys())
            .chain(self.latch_held.keys())
            .chain(self.touch_returns.keys())
            .copied()
            .collect()
    }

    /// Explicitly closes every in-flight gesture, manual override,
    /// latch/write-held value and Touch return (issue #131 boundary
    /// closure): each non-idle target transitions to `Idle` and is
    /// recorded as a [`WriterClosure`].
    fn close_runtime_state(&mut self, reason: WriterCloseReason) {
        for target in self.runtime_targets() {
            self.close_target_runtime(target, reason);
        }
    }

    /// Closes one target's runtime state with `reason` — discards the
    /// uncommitted gesture preview, the manual override, held values
    /// and any in-flight return, and logs a [`WriterClosure`].
    fn close_target_runtime(&mut self, target: ParameterTargetId, reason: WriterCloseReason) {
        let state = self.writer_state(target);
        if matches!(state, WriterState::Idle) {
            return;
        }
        self.gestures.remove(&target);
        self.manual_overrides.remove(&target);
        self.latch_held.remove(&target);
        self.touch_returns.remove(&target);
        self.log_closure(target, reason, state);
    }

    /// Appends a closure record to the bounded writer-closure log.
    fn log_closure(
        &mut self,
        target: ParameterTargetId,
        reason: WriterCloseReason,
        state: WriterState,
    ) {
        self.writer_closes.push_back(WriterClosure {
            target,
            reason,
            state,
        });
        while self.writer_closes.len() > WRITER_CLOSE_CAP {
            self.writer_closes.pop_front();
        }
    }

    // -- gestures ------------------------------------------------------

    /// Begins a manual gesture on `target`. The override starts at the
    /// currently evaluated reference value so touching a knob does not
    /// jump — during an in-flight Touch return that is the blended
    /// value. `READONLY` targets reject the gesture — the capability
    /// contract is enforced at the session boundary too.
    ///
    /// The gesture's execution context — target, `revision`, arm and
    /// clock context — is fixed now (issue #131 `GestureFixation`):
    /// later Inspector follow/focus changes cannot retarget the write.
    ///
    /// # Errors
    /// [`Error::NotFound`]/[`Error::WriteRejected`]/[`Error::Validation`].
    pub fn gesture_begin(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
        at: BeatPoint,
        revision: ProjectRevision,
    ) -> Result<(), Error> {
        let t = Self::target(project, target)?;
        if t.is_readonly() {
            return Err(Error::WriteRejected {
                means: "gesture".into(),
                rejection: "param_readonly".into(),
            });
        }
        if self.gestures.contains_key(&target) {
            return Err(Error::Validation(format!("gesture active on {target}")));
        }
        let start_value = self
            .evaluate(project, target, at)
            .map_or(t.base, |e| e.reference);
        self.gestures.insert(
            target,
            ControlGesture {
                started_at: at,
                points: Vec::new(),
                fixation: ControlGestureFixation {
                    target,
                    revision,
                    arm: self.arm(target),
                    epoch: self.playback.epoch,
                    frame_base: self.frame_base,
                },
            },
        );
        self.manual_overrides.insert(target, start_value);
        // A re-touch supersedes an in-flight return — the new gesture
        // takes over from the blended reference it just seeded.
        self.touch_returns.remove(&target);
        Ok(())
    }

    /// Feeds one gesture position/value pair (the manual,
    /// pre-modulation value — the only value a write mode may record).
    ///
    /// # Errors
    /// [`Error::Validation`] on no active gesture or non-finite /
    /// out-of-domain value.
    pub fn gesture_update(
        &mut self,
        target: ParameterTargetId,
        at: BeatPoint,
        value: f64,
    ) -> Result<(), Error> {
        if !self.gestures.contains_key(&target) {
            return Err(Error::Validation(format!("no gesture on {target}")));
        }
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(Error::Validation("gesture value outside [0,1]".into()));
        }
        if let Some(g) = self.gestures.get_mut(&target) {
            g.points.push((at, value));
        }
        self.manual_overrides.insert(target, value);
        Ok(())
    }

    /// Ends the gesture per the target's selected mode. `Off` returns
    /// [`GestureEnd::BaseEdit`] for a gesture with points (the caller
    /// applies `SetParameterBase`); an empty gesture returns
    /// [`GestureEnd::Returned`] so the seeded evaluation reference is
    /// never persisted as a base edit. `Touch`/`Latch`/`Write` write
    /// the gesture interval only when the write gates hold (armed,
    /// record-enabled, rolling, open transaction fixing this
    /// target). Gesture point positions are normalized into the
    /// target's scope coordinate before they enter the transaction.
    ///
    /// # Errors
    /// [`Error::Validation`] on no active gesture.
    pub fn gesture_end(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
    ) -> Result<GestureEnd, Error> {
        let Some(g) = self.gestures.remove(&target) else {
            return Err(Error::Validation(format!("no gesture on {target}")));
        };
        let last = self.manual_overrides.get(&target).copied().unwrap_or(0.0);
        let mode = self.mode(target);
        match mode {
            AutomationMode::Off => {
                self.manual_overrides.remove(&target);
                // An empty gesture wrote nothing — the seeded override
                // was only the evaluation reference. Committing it as
                // a base edit would silently persist the *evaluated*
                // (possibly modulated/automated) value as the base.
                if g.points.is_empty() {
                    Ok(GestureEnd::Returned)
                } else {
                    Ok(GestureEnd::BaseEdit { value: last })
                }
            }
            AutomationMode::Read => {
                self.manual_overrides.remove(&target);
                Ok(GestureEnd::Returned)
            }
            AutomationMode::Touch => {
                self.manual_overrides.remove(&target);
                // Release starts the runtime Touch return — the
                // reference glides back to the automation series over
                // the configured duration (issue #131). The return is
                // independent of the write gates: it only reshapes the
                // reference layer, never the recorded series.
                self.touch_returns.insert(
                    target,
                    TouchReturn {
                        release_value: last,
                        released_at: self.clock_now,
                    },
                );
                if self.write_gates(target) {
                    let n = self.record_points(project, target, &g.points)?;
                    Ok(GestureEnd::Written { points: n })
                } else {
                    Ok(GestureEnd::Returned)
                }
            }
            AutomationMode::Latch => {
                self.manual_overrides.remove(&target);
                if self.write_gates(target) {
                    let _ = self.record_points(project, target, &g.points)?;
                    self.latch_held.insert(target, last);
                    Ok(GestureEnd::Latched { value: last })
                } else {
                    Ok(GestureEnd::Returned)
                }
            }
            AutomationMode::Write => {
                if self.write_gates(target) {
                    let n = self.record_points(project, target, &g.points)?;
                    Ok(GestureEnd::Written { points: n })
                } else {
                    self.manual_overrides.remove(&target);
                    Ok(GestureEnd::Returned)
                }
            }
        }
    }

    /// Cancels the in-flight gesture on `target`, discarding the
    /// uncommitted preview: the collected points and the manual
    /// override are dropped and the target snaps back to its
    /// recorded/evaluated series (no return interpolation — cancel is
    /// not a release). Nothing is written — a cancel never rewrites
    /// committed state and never creates an undo entry.
    ///
    /// # Errors
    /// [`Error::Validation`] on no active gesture.
    pub fn gesture_cancel(&mut self, target: ParameterTargetId) -> Result<(), Error> {
        if !self.gestures.contains_key(&target) {
            return Err(Error::Validation(format!("no gesture on {target}")));
        }
        self.close_target_runtime(target, WriterCloseReason::Cancelled);
        Ok(())
    }

    // -- recording ------------------------------------------------------

    /// The open recording transaction, if any.
    #[must_use]
    pub fn recording(&self) -> Option<&RecordingTransaction> {
        self.recording.as_ref()
    }

    /// Opens a recording transaction. Requires record-enabled +
    /// rolling + at least one armed write-mode target. The target set,
    /// the target→lane mapping, the arm state, the clock-epoch
    /// context and the project revision are **fixed now** — later
    /// Inspector follow changes cannot retarget the write. Returns
    /// the session-unique transaction id for id-tagged lifecycle
    /// calls (`finish_record_commit`, `approve_rebase`).
    ///
    /// # Errors
    /// [`Error::Validation`] when the gates do not hold or a
    /// transaction is already open.
    pub fn record_start(
        &mut self,
        project: &Project,
        revision: ProjectRevision,
    ) -> Result<RecordingTransactionId, Error> {
        if self.recording.is_some() {
            return Err(Error::Validation("recording already open".into()));
        }
        if !self.record_enabled || self.playback.status != PlaybackStatus::Playing {
            return Err(Error::Validation(
                "record requires record-enabled + rolling".into(),
            ));
        }
        let fixed: Vec<ParameterTargetId> = self
            .arms
            .iter()
            .filter(|(t, a)| {
                **a == WriteArm::Armed
                    && matches!(
                        self.mode(**t),
                        AutomationMode::Touch | AutomationMode::Latch | AutomationMode::Write
                    )
            })
            .map(|(t, _)| *t)
            .collect();
        if fixed.is_empty() {
            return Err(Error::Validation("no armed write-mode target".into()));
        }
        // Every fixed target must have its write lane bound now — a
        // lane missing here only surfaced at `gesture_end`/
        // `record_write`, stranding the write the transaction fixed.
        // The mapping is captured into `fixed_lanes` so a later rebind
        // cannot retarget recorded points.
        let mut fixed_lanes = BTreeMap::new();
        for target in &fixed {
            let lane = self
                .write_lanes
                .get(target)
                .ok_or_else(|| Error::Validation(format!("no write lane bound for {target}")))?;
            fixed_lanes.insert(*target, *lane);
        }
        // The `(owner, time_scope)` pair each recorded position
        // resolves against — fixed now (issue #133): a clip move or
        // scope change mid-recording cannot reinterpret the pass's
        // positions. A target that no longer resolves is skipped
        // here; `record_points` surfaces it as `NotFound`.
        let mut fixed_scopes = BTreeMap::new();
        for target in &fixed {
            if let Ok(t) = Self::target(project, *target) {
                fixed_scopes.insert(*target, (t.owner, t.time_scope));
            }
        }
        // `Write`-mode fixed targets hold the reference evaluated at
        // the start position — the agreed "記録開始時に固定した
        // Reference 基準値から継続書込み": the held value writes at
        // control rate until a gesture moves it. A target with an
        // in-flight gesture keeps its gesture value instead. A start
        // position outside the target's scope domain has no Reference
        // to hold — the target simply stays unheld rather than
        // failing the transaction open.
        let mut write_seeds = Vec::new();
        for target in &fixed {
            if self.mode(*target) == AutomationMode::Write
                && !self.gestures.contains_key(target)
                && let Ok(eval) = evaluate_control(project, *target, self.playback.position, None)
            {
                write_seeds.push((*target, eval.reference));
            }
        }
        self.next_txn_id = self.next_txn_id.checked_add(1).ok_or(Error::Overflow)?;
        let id = RecordingTransactionId(self.next_txn_id);
        self.recording = Some(RecordingTransaction {
            id,
            started_at: self.playback.position,
            epoch: self.playback.epoch,
            fixed_targets: fixed,
            fixed_lanes,
            fixed_revision: revision,
            pass: self.playback.pass,
            fixed_scopes,
            coverage_end: BTreeMap::new(),
            segments: BTreeMap::new(),
            deliveries: BTreeMap::new(),
            deliveries_dropped: 0,
            record_gaps: BTreeMap::new(),
            record_gaps_merged: 0,
            record_dropped: 0,
            sealed: None,
            state: RecordingTxnState::Pending,
        });
        for (target, value) in write_seeds {
            self.manual_overrides.insert(target, value);
        }
        Ok(id)
    }

    /// Appends a manual (pre-modulation) point to the open
    /// transaction — never a host-resolved value. `at` is a project
    /// position; it is normalized into the target's scope coordinate
    /// before storage. A position outside the scope's domain (e.g.
    /// beyond a note's range) records nothing.
    ///
    /// # Errors
    /// [`Error::Validation`] when no open unsealed transaction fixed
    /// `target`, no write lane is bound, or the value is out of domain.
    pub fn record_write(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
        at: BeatPoint,
        value: f64,
    ) -> Result<(), Error> {
        if !self.write_gates(target) {
            return Err(Error::Validation(format!(
                "record gates closed for {target}"
            )));
        }
        // A position at or beyond the loop end belongs to the next
        // pass — it can never be attributed to this transaction
        // (issue #133).
        if let Some((_, end)) = self.playback.loop_range
            && at >= end
        {
            return Err(Error::Validation(format!(
                "position {at} is outside the loop pass [.., {end})"
            )));
        }
        self.record_points(project, target, &[(at, value)])?;
        Ok(())
    }

    /// Stores `points` (project positions) into the open transaction
    /// in the target's scope coordinate. Positions outside the scope
    /// domain are skipped — a write sweep crossing a domain edge must
    /// not silently inject project-coordinate points into a local
    /// series; other resolution failures propagate.
    ///
    /// Points land on the lane **fixed** for the target at
    /// `record_start`; a batch that would exceed
    /// [`RECORD_POINT_BUDGET`] is rejected whole — already-recorded
    /// points are never dropped.
    fn record_points(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
        points: &[(BeatPoint, f64)],
    ) -> Result<usize, Error> {
        let bound = Self::target(project, target)?;
        let tx_ref = self
            .recording
            .as_ref()
            .ok_or_else(|| Error::Validation("no open recording".into()))?;
        // Positions resolve against the `(owner, time_scope)` fixed at
        // `record_start` — never a scope that moved mid-recording.
        let (owner, time_scope) = tx_ref
            .fixed_scopes
            .get(&target)
            .copied()
            .unwrap_or((bound.owner, bound.time_scope));
        let lane = *tx_ref
            .fixed_lanes
            .get(&target)
            .ok_or_else(|| Error::Validation(format!("no fixed write lane for {target}")))?;
        // The fixed lane lives in the project — a lane removed
        // mid-recording must surface now, not strand a segment the
        // commit could never write back.
        if !project.control.lanes.contains_key(&lane) {
            return Err(Error::Validation(format!(
                "write lane {lane} no longer exists"
            )));
        }
        let pass_end = self.playback.loop_range.map(|(_, end)| end);
        let mut resolved = Vec::with_capacity(points.len());
        for (pos, value) in points {
            // A batch position at or beyond the loop end belongs to the
            // next pass — clip it, never attribute it here (issue #133).
            if pass_end.is_some_and(|end| *pos >= end) {
                continue;
            }
            match synvelis_project::resolve_scope_pos(project, owner, time_scope, *pos) {
                Ok(local) => resolved.push((local, *value)),
                Err(synvelis_project::Error::ScopeUnavailable { .. }) => {}
                Err(e) => return Err(e),
            }
        }
        let written = resolved.len();
        let max_points = self.record_budgets.max_points;
        let rec = self
            .recording
            .as_mut()
            .ok_or_else(|| Error::Validation("no open recording".into()))?;
        let total = rec.segments.values().map(Vec::len).sum::<usize>();
        if total + resolved.len() > max_points {
            return Err(Error::Validation(format!(
                "recording point budget of {max_points} exceeded"
            )));
        }
        // The delivered write extent this batch covered — recorded
        // before dedup collapses duplicate positions (issue #134):
        // the delivery *happened* even when a dedup rewrites a point.
        let delivered = resolved
            .iter()
            .map(|(pos, _)| *pos)
            .min()
            .zip(resolved.iter().map(|(pos, _)| *pos).max());
        let seg = rec.segments.entry(lane).or_default();
        for (pos, value) in resolved {
            seg.push(AutomationPoint { pos, value });
        }
        seg.sort_by_key(|p| p.pos);
        seg.dedup_by(|next, prev| {
            if next.pos == prev.pos {
                *prev = *next;
                true
            } else {
                false
            }
        });
        if let Some((first, last)) = delivered {
            let dels = rec.deliveries.entry(lane).or_default();
            if dels.len() < RECORD_DELIVERY_CAP {
                dels.push((first, last));
            } else {
                rec.deliveries_dropped = rec.deliveries_dropped.saturating_add(1);
            }
        }
        Ok(written)
    }

    // -- record event queue (issue #134) --------------------------------

    /// The RT-side ingress for automation record events — the only
    /// write path callable from a real-time callback: bounded,
    /// wait-free and allocation-free (`ruviform_rt` SPSC
    /// [`Producer::try_push`]). A full queue rejects with
    /// [`RecordPushError::QueueFull`]; the loss is counted on the
    /// target's health and folded into an explicit gap at the next
    /// [`Self::drain_record_events`] — never a smooth curve.
    ///
    /// `pos` is a project position (project beats); `value` is the
    /// manual normalized value. The event binds the **currently open**
    /// transaction — an event drained after the transaction ended is
    /// rejected as `stale`/`WrongTxn`, never attributed to a new one.
    ///
    /// # Errors
    /// [`RecordPushError::GatesClosed`] when the write gates are shut,
    /// [`RecordPushError::Invalid`] for a non-recordable event, and
    /// [`RecordPushError::QueueFull`] when the bounded ring is full.
    pub fn record_event_rt(
        &mut self,
        target: ParameterTargetId,
        pos: BeatPoint,
        value: f64,
    ) -> Result<(), RecordPushError> {
        if !self.write_gates(target) {
            return Err(RecordPushError::GatesClosed);
        }
        if let Some((_, end)) = self.playback.loop_range
            && pos >= end
        {
            return Err(RecordPushError::Invalid);
        }
        if !(0.0..=1.0).contains(&value) {
            return Err(RecordPushError::Invalid);
        }
        let txn = self
            .recording
            .as_ref()
            .map_or(RecordingTransactionId(0), |tx| tx.id);
        if self
            .record_producer
            .try_push(AutomationRecordEvent {
                txn,
                target,
                pos,
                value,
            })
            .is_err()
        {
            self.note_record_drop(target, Some(pos), |h| {
                h.queue_fulls = h.queue_fulls.saturating_add(1);
            });
            if let Some(tx) = self.recording.as_mut() {
                tx.record_dropped = tx.record_dropped.saturating_add(1);
            }
            return Err(RecordPushError::QueueFull);
        }
        Ok(())
    }

    /// Non-RT consumer of the record queue — drains up to
    /// [`RecordBudgets::max_events_per_drain`] events into the open
    /// transaction, folds dropped-event runs into explicit
    /// [`RecordGap`]s, and emits bounded [`RecordNotification`]s.
    /// Each failure kind is reported distinctly — `partial` means
    /// events remain queued and the caller retries; nothing reports
    /// "done" while the queue holds events.
    pub fn drain_record_events(&mut self, project: &Project) -> RecordDrainReport {
        let mut rep = RecordDrainReport::default();
        let budget = self.record_budgets.max_events_per_drain;
        let mut rejects: BTreeMap<(ParameterTargetId, RecordRejectKind), u64> = BTreeMap::new();
        for _ in 0..budget {
            let Ok(ev) = self.record_consumer.try_pop() else {
                break;
            };
            rep.popped += 1;
            self.drain_record_event(project, ev, &mut rep, &mut rejects);
        }
        rep.partial = !self.record_consumer.is_empty();
        self.fold_record_gaps();
        self.emit_drain_notifications(&rep, &rejects);
        rep
    }

    /// One drained event through the reject pipeline — stale txn,
    /// closed gates, out-of-pass position, bad value, full point
    /// budget, scope/structural failure; each kind is counted and
    /// retained distinctly, and every drop leaves a health mark.
    fn drain_record_event(
        &mut self,
        project: &Project,
        ev: AutomationRecordEvent,
        rep: &mut RecordDrainReport,
        rejects: &mut BTreeMap<(ParameterTargetId, RecordRejectKind), u64>,
    ) {
        // Only the open transaction may take the event — a stale
        // event is rejected, never attributed to a new session.
        if self.recording.as_ref().map(|tx| tx.id) != Some(ev.txn) {
            rep.stale += 1;
            self.drain_reject(ev, rep, rejects, RecordRejectKind::WrongTxn, None);
            return;
        }
        // Gates may have closed between push and drain.
        let kind = if !self.write_gates(ev.target) {
            Some(RecordRejectKind::GatesClosed)
        } else if self
            .playback
            .loop_range
            .is_some_and(|(_, end)| ev.pos >= end)
        {
            Some(RecordRejectKind::OutsidePass)
        } else if !(0.0..=1.0).contains(&ev.value) {
            Some(RecordRejectKind::BadValue)
        } else {
            None
        };
        if let Some(kind) = kind {
            self.drain_reject(ev, rep, rejects, kind, Some(ev.pos));
            return;
        }
        // Drain-side point budget — a full budget is `OverBudget`,
        // not a silent loss.
        let full = self.recording.as_ref().is_some_and(|tx| {
            tx.segments.values().map(Vec::len).sum::<usize>() >= self.record_budgets.max_points
        });
        if full {
            rep.overflowed += 1;
            self.drain_reject(ev, rep, rejects, RecordRejectKind::OverBudget, Some(ev.pos));
            return;
        }
        match self.record_points(project, ev.target, &[(ev.pos, ev.value)]) {
            Ok(0) => {
                // Resolved to nothing — the scope could not attribute
                // the point: an explicit gap, not data.
                self.drain_reject(ev, rep, rejects, RecordRejectKind::ScopeLost, Some(ev.pos));
            }
            Ok(_) => rep.accepted += 1,
            Err(e) => {
                let kind = if matches!(e, Error::NotFound(_)) {
                    RecordRejectKind::LaneGone
                } else {
                    RecordRejectKind::Failed
                };
                self.drain_reject(ev, rep, rejects, kind, Some(ev.pos));
            }
        }
    }

    /// One rejected event: aggregate the (target, kind) count, mark
    /// the health counters, open/extend the gap run when `pos` is
    /// known, attribute the drop to the open transaction, and retain
    /// the reject in the report (bounded).
    fn drain_reject(
        &mut self,
        ev: AutomationRecordEvent,
        rep: &mut RecordDrainReport,
        rejects: &mut BTreeMap<(ParameterTargetId, RecordRejectKind), u64>,
        kind: RecordRejectKind,
        pos: Option<BeatPoint>,
    ) {
        *rejects.entry((ev.target, kind)).or_default() += 1;
        self.note_record_drop(ev.target, pos, |h| match kind {
            RecordRejectKind::OverBudget => {
                h.overflowed = h.overflowed.saturating_add(1);
            }
            _ => h.rejected = h.rejected.saturating_add(1),
        });
        if pos.is_some()
            && let Some(tx) = self.recording.as_mut()
        {
            tx.record_dropped = tx.record_dropped.saturating_add(1);
        }
        rep.reject(ev, kind);
    }

    /// Folds each target's open gap run into the transaction's
    /// explicit [`RecordGap`] ledger — a dropped run becomes a data
    /// hole on the lane the target records into, never smoothed data.
    fn fold_record_gaps(&mut self) {
        let mut notes = Vec::new();
        for h in &mut self.record_health {
            if let (Some(start), Some(end)) = (h.gap_open, h.gap_last)
                && h.gap_dropped > 0
                && let Some(tx) = self.recording.as_mut()
                && let Some(lane) = tx.fixed_lanes.get(&h.target).copied()
            {
                let gaps = tx.record_gaps.entry(lane).or_default();
                if gaps.len() >= RECORD_GAP_CAP {
                    if let Some(last) = gaps.last_mut() {
                        last.end = last.end.max(end);
                        last.dropped = last.dropped.saturating_add(h.gap_dropped);
                    }
                    tx.record_gaps_merged = tx.record_gaps_merged.saturating_add(1);
                } else {
                    gaps.push(RecordGap {
                        start,
                        end,
                        dropped: h.gap_dropped,
                    });
                    notes.push(RecordNotification::DataGap {
                        lane,
                        start,
                        end,
                        dropped: h.gap_dropped,
                    });
                }
            }
            h.gap_open = None;
            h.gap_last = None;
            h.gap_dropped = 0;
        }
        for n in notes {
            self.notify(n);
        }
    }

    /// Emits the bounded notifications for one drain — queue-full
    /// deltas (watermark, once per target), per-(target, kind)
    /// rejects, and the partial-drain state.
    fn emit_drain_notifications(
        &mut self,
        rep: &RecordDrainReport,
        rejects: &BTreeMap<(ParameterTargetId, RecordRejectKind), u64>,
    ) {
        let full_notes: Vec<(ParameterTargetId, u64)> = self
            .record_health
            .iter_mut()
            .filter(|h| h.queue_fulls > h.notified_fulls)
            .map(|h| {
                h.notified_fulls = h.queue_fulls;
                (h.target, h.dropped)
            })
            .collect();
        for (target, dropped_total) in full_notes {
            self.notify(RecordNotification::QueueFull {
                target,
                dropped_total,
            });
        }
        for ((target, kind), count) in rejects {
            self.notify(RecordNotification::Rejected {
                target: *target,
                kind: *kind,
                count: *count,
            });
        }
        if rep.partial {
            self.notify(RecordNotification::Partial {
                accepted: rep.accepted,
            });
        }
    }

    /// Counts a dropped record event on the target's health —
    /// opening/extending a gap run when `pos` is known. When the
    /// health table is full, the drop falls onto
    /// [`Self::record_health_overflows`] — counted, never silent.
    fn note_record_drop(
        &mut self,
        target: ParameterTargetId,
        pos: Option<BeatPoint>,
        bump: impl Fn(&mut RecordTargetHealth),
    ) {
        let idx = self.record_health.iter().position(|h| h.target == target);
        let health = match idx {
            Some(i) => &mut self.record_health[i],
            None if self.record_health.len() < RECORD_HEALTH_CAP => {
                self.record_health.push(RecordTargetHealth::new(target));
                self.record_health.last_mut().expect("just pushed")
            }
            None => {
                self.record_health_overflows = self.record_health_overflows.saturating_add(1);
                return;
            }
        };
        health.dropped = health.dropped.saturating_add(1);
        bump(health);
        if let Some(p) = pos {
            health.gap_open.get_or_insert(p);
            health.gap_last = Some(p);
            health.gap_dropped = health.gap_dropped.saturating_add(1);
        }
    }

    /// Appends a bounded record notification; at capacity the oldest
    /// entry is dropped and counted.
    fn notify(&mut self, n: RecordNotification) {
        if self.record_notifications.len() >= RECORD_NOTIFY_CAP {
            self.record_notifications.pop_front();
            self.record_notifications_dropped = self.record_notifications_dropped.saturating_add(1);
        }
        self.record_notifications.push_back(n);
    }

    /// The record-path budgets.
    #[must_use]
    pub fn record_budgets(&self) -> RecordBudgets {
        self.record_budgets
    }

    /// Updates the record-path budgets (point columns, drain size).
    /// Session-only — never a project mutation.
    pub fn set_record_budgets(&mut self, budgets: RecordBudgets) {
        self.record_budgets = budgets;
    }

    /// The target's cumulative record health, when any record event
    /// was seen for it.
    #[must_use]
    pub fn record_health(&self, target: ParameterTargetId) -> Option<RecordHealthSummary> {
        self.record_health
            .iter()
            .find(|h| h.target == target)
            .map(RecordHealthSummary::from)
    }

    /// Drops that could not enter the bounded health table.
    #[must_use]
    pub fn record_health_overflows(&self) -> u64 {
        self.record_health_overflows
    }

    /// The bounded record-notification log (newest last).
    #[must_use]
    pub fn record_notifications(&self) -> &VecDeque<RecordNotification> {
        &self.record_notifications
    }

    /// Notifications dropped because the bounded log was full.
    #[must_use]
    pub fn record_notifications_dropped(&self) -> u64 {
        self.record_notifications_dropped
    }

    /// The Automation-Lane projection (issue #134): the model-side
    /// bounded LOD curve plus the session-only overlays — uncommitted
    /// recorded points inside the window, delivered write intervals,
    /// explicit record gaps (resolved into the scope coordinate) and
    /// record-path health. Read-only: `revision` is captured so a UI
    /// can detect a stale view; nothing mutates the project.
    ///
    /// # Errors
    /// Propagates [`synvelis_project::automation_view`]'s errors —
    /// [`Error::NotFound`] for an unknown target,
    /// [`Error::Validation`] for an invalid window.
    pub fn automation_view(
        &self,
        project: &Project,
        target: ParameterTargetId,
        query: &CurveQuery,
        revision: ProjectRevision,
    ) -> Result<AutomationLaneView, Error> {
        let curve = synvelis_project::automation_view(project, target, query)?;
        let mut pending = Vec::new();
        let mut pending_truncated = false;
        let mut deliveries = Vec::new();
        let mut gaps = Vec::new();
        let mut gaps_unresolved = 0usize;
        if let Some(tx) = &self.recording {
            for (t, lane) in &tx.fixed_lanes {
                if *t != target {
                    continue;
                }
                if let Some(seg) = tx.segments.get(lane) {
                    let in_win: Vec<AutomationPoint> = seg
                        .iter()
                        .copied()
                        .filter(|p| {
                            let x = p.pos.to_f64();
                            query.start <= x && x <= query.end
                        })
                        .collect();
                    if in_win.len() > query.max_points {
                        pending_truncated = true;
                        pending.push((*lane, in_win.into_iter().take(query.max_points).collect()));
                    } else {
                        pending.push((*lane, in_win));
                    }
                }
                if let Some(ds) = tx.deliveries.get(lane) {
                    deliveries.extend(
                        ds.iter()
                            .filter(|(a, b)| a.to_f64() < query.end && b.to_f64() > query.start)
                            .map(|(a, b)| (a.to_f64(), b.to_f64())),
                    );
                }
                if let Some(gs) = tx.record_gaps.get(lane)
                    && let Some((owner, scope)) = tx.fixed_scopes.get(t).copied()
                {
                    for g in gs {
                        match (
                            synvelis_project::resolve_scope_pos(project, owner, scope, g.start),
                            synvelis_project::resolve_scope_pos(project, owner, scope, g.end),
                        ) {
                            (Ok(a), Ok(b)) => gaps.push(ResolvedRecordGap {
                                start: a.to_f64(),
                                end: b.to_f64(),
                                dropped: g.dropped,
                            }),
                            _ => gaps_unresolved += 1,
                        }
                    }
                }
            }
        }
        let health = self
            .record_health
            .iter()
            .find(|h| h.target == target)
            .map(RecordHealthSummary::from);
        Ok(AutomationLaneView {
            target,
            revision,
            curve,
            pending,
            pending_truncated,
            deliveries,
            gaps,
            gaps_unresolved,
            health,
        })
    }

    /// Snapshots the open transaction into an immutable
    /// [`PreparedRecordingCommit`] — **non-destructively**: the
    /// recorded columns, fixed targets and fixed lanes stay in place
    /// until [`ControlSession::finish_record_commit`] consumes them
    /// after a successful apply.
    ///
    /// The prepare performs, in order:
    /// 1. the fixed-revision check — `fixed_revision` vs
    ///    `current_revision`, where *any* drift (including unrelated
    ///    edits) is the default [`RecordCommitError::Conflict`];
    /// 2. structural integrity — every fixed target still exists and
    ///    every segment lane still exists and still feeds the target
    ///    that recorded into it;
    /// 3. the region-write construction — one `Replace` op per lane
    ///    inside one `Composite`, so the session lands as **one undo
    ///    step**.
    ///
    /// A failed prepare leaves the transaction open — `Conflict` for
    /// a revision mismatch (Inspect / Explicit Rebase / Abort),
    /// `Rejected` for a structural problem. No silent rebinding to
    /// the newest revision and no merge ever happens here.
    ///
    /// # Errors
    /// [`RecordCommitError::NoTransaction`], [`RecordCommitError::Conflict`],
    /// or [`RecordCommitError::Invalid`].
    pub fn prepare_record_commit(
        &mut self,
        project: &Project,
        current_revision: ProjectRevision,
    ) -> Result<PreparedRecordingCommit, RecordCommitError> {
        let Some(tx) = self.recording.as_ref() else {
            return Err(RecordCommitError::NoTransaction);
        };
        if tx.fixed_revision != current_revision {
            let conflict = RecordCommitError::Conflict {
                fixed: tx.fixed_revision,
                current: current_revision,
                affected_targets: tx.fixed_targets.clone(),
            };
            if let Some(tx) = self.recording.as_mut() {
                tx.state = RecordingTxnState::Conflict;
            }
            return Err(conflict);
        }
        // Everything below is read-only against `tx`; failures mark
        // the transaction `Rejected` afterwards — `tx` borrows
        // `self.recording`, so no mutable call can happen inside.
        let built = (|| {
            // Structural integrity: the write-back must land on the
            // same target/lane mapping the transaction fixed. A
            // deleted target or a removed/retargeted lane rejects the
            // commit — the columns stay for inspection.
            for target in &tx.fixed_targets {
                if !project.control.targets.contains_key(target) {
                    return Err(RecordCommitError::Invalid(format!(
                        "fixed target {target} no longer exists"
                    )));
                }
            }
            for (lane, points) in &tx.segments {
                if points.is_empty() {
                    continue;
                }
                let Some(lane_ref) = project.control.lanes.get(lane) else {
                    return Err(RecordCommitError::Invalid(format!(
                        "write lane {lane} no longer exists"
                    )));
                };
                let fixed_target = tx
                    .fixed_lanes
                    .iter()
                    .find_map(|(t, l)| (*l == *lane).then_some(*t));
                if fixed_target.is_some_and(|t| lane_ref.target != t) {
                    return Err(RecordCommitError::Invalid(format!(
                        "write lane {lane} no longer feeds its fixed target"
                    )));
                }
            }
            let mut ops = Vec::new();
            for (lane, points) in &tx.segments {
                let mut points = points.clone();
                points.sort_by_key(|p| p.pos);
                let (Some(first), Some(last)) = (points.first(), points.last()) else {
                    continue;
                };
                let (start, last_pos) = (first.pos, last.pos);
                // The write range's exclusive end (issue #133): the
                // lane's `coverage_end` — recorded in project beats at
                // the boundary and resolved here against the fixed
                // scope — covers a held write's continuous tail. It
                // only applies when it actually extends the recorded
                // points (`> last_pos`); a scope the bound cannot be
                // expressed in falls back to the last-point increment.
                let coverage = tx
                    .coverage_end
                    .get(lane)
                    .and_then(|bound| {
                        let fixed_target = tx
                            .fixed_lanes
                            .iter()
                            .find_map(|(t, l)| (*l == *lane).then_some(*t))?;
                        let (owner, scope) = tx.fixed_scopes.get(&fixed_target).copied()?;
                        synvelis_project::resolve_scope_pos(project, owner, scope, *bound).ok()
                    })
                    .filter(|end| *end > last_pos);
                // Cover the last point: extend the half-open range by
                // the smallest representable increment (2^-32 beats —
                // below any meaningful user point, documented).
                let fallback_end = || {
                    last_pos
                        .checked_add(
                            BeatPoint::new(1, u32::MAX)
                                .map_err(|e| RecordCommitError::Invalid(e.to_string()))?,
                        )
                        .map_err(|e| RecordCommitError::Invalid(e.to_string()))
                };
                let end = match coverage {
                    Some(end) => end,
                    None => fallback_end()?,
                };
                let range = BeatRange::try_new(start, end)
                    .map_err(|e| RecordCommitError::Invalid(e.to_string()))?;
                ops.push(ProjectOp::WriteAutomationRegion {
                    lane: *lane,
                    region: AutomationRegion {
                        range,
                        interpolation: Interpolation::Linear,
                        points,
                    },
                    policy: synvelis_project::ConflictPolicy::Replace,
                });
            }
            Ok(PreparedRecordingCommit {
                id: tx.id,
                fingerprint: txn_fingerprint(tx),
                expected_revision: tx.fixed_revision,
                op: (!ops.is_empty()).then_some(ProjectOp::Composite { ops }),
                was_sealed: tx.sealed.is_some(),
            })
        })();
        match built {
            Ok(prepared) => Ok(prepared),
            Err(e) => {
                self.reject_open();
                Err(e)
            }
        }
    }

    /// Pre-apply gate for [`ProjectSession::control_record_finish`]:
    /// resolves the prepared payload's identity **before** any apply
    /// so a duplicate receipt, a payload mismatch or an unknown id
    /// can never write. Checks, in order:
    /// 1. the applied registry — same id + fingerprint is
    ///    `AlreadyApplied`, same id + different fingerprint is
    ///    `IntegrityConflict` (in-memory, session-scoped only);
    /// 2. the open transaction — same id requires a fingerprint
    ///    match (`IntegrityConflict` otherwise); any other id is
    ///    `UnknownTransaction`.
    ///
    /// # Errors
    /// [`RecordCommitError::AlreadyApplied`]/`IntegrityConflict`/
    /// `UnknownTransaction`.
    pub fn preflight_record_finish(
        &self,
        prepared: &PreparedRecordingCommit,
    ) -> Result<(), RecordCommitError> {
        self.check_record_receipt(prepared)?;
        match self.recording.as_ref() {
            Some(tx) if tx.id == prepared.id => {
                if txn_fingerprint(tx) != prepared.fingerprint {
                    return Err(RecordCommitError::IntegrityConflict { id: prepared.id });
                }
                Ok(())
            }
            _ => Err(RecordCommitError::UnknownTransaction { id: prepared.id }),
        }
    }

    /// The applied-registry half of the finish checks — shared by
    /// `preflight` (pre-apply) and `finish` (the consumer).
    fn check_record_receipt(
        &self,
        prepared: &PreparedRecordingCommit,
    ) -> Result<(), RecordCommitError> {
        if let Some(receipt) = self.applied.get(&prepared.id) {
            return if receipt.fingerprint == prepared.fingerprint {
                Err(RecordCommitError::AlreadyApplied {
                    id: prepared.id,
                    revision: receipt.revision,
                })
            } else {
                Err(RecordCommitError::IntegrityConflict { id: prepared.id })
            };
        }
        Ok(())
    }

    /// Consumes the open transaction **after** its prepared op was
    /// successfully applied — the only consumer of the recorded
    /// columns. `applied_revision` is `Some` when an apply happened
    /// (a `Some` op), `None` for an empty session that committed
    /// nothing.
    ///
    /// The same id + fingerprint presented again resolves against the
    /// applied registry to [`RecordCommitError::AlreadyApplied`] — an
    /// in-memory, session-scoped exactly-once receipt; the same id
    /// with a different fingerprint is
    /// [`RecordCommitError::IntegrityConflict`].
    ///
    /// # Errors
    /// [`RecordCommitError::NoTransaction`],
    /// [`RecordCommitError::UnknownTransaction`],
    /// [`RecordCommitError::IntegrityConflict`], or
    /// [`RecordCommitError::AlreadyApplied`].
    pub fn finish_record_commit(
        &mut self,
        prepared: &PreparedRecordingCommit,
        applied_revision: Option<ProjectRevision>,
    ) -> Result<RecordingOutcome, RecordCommitError> {
        // Idempotent retry: the id was already consumed once.
        self.check_record_receipt(prepared)?;
        let Some(tx) = self.recording.take() else {
            return Err(RecordCommitError::UnknownTransaction { id: prepared.id });
        };
        if tx.id != prepared.id {
            self.recording = Some(tx);
            return Err(RecordCommitError::UnknownTransaction { id: prepared.id });
        }
        if txn_fingerprint(&tx) != prepared.fingerprint {
            self.recording = Some(tx);
            return Err(RecordCommitError::IntegrityConflict { id: prepared.id });
        }
        let outcome = RecordingOutcome {
            id: tx.id,
            applied: applied_revision.is_some(),
            revision: applied_revision,
            was_sealed: tx.sealed.is_some(),
        };
        self.last_disposition = Some(RecordDisposition::Committed {
            id: tx.id,
            revision: applied_revision,
        });
        if let Some(revision) = applied_revision {
            self.applied.insert(
                tx.id,
                AppliedRecord {
                    fingerprint: prepared.fingerprint,
                    revision,
                },
            );
            // Bounded window: evict the oldest receipt beyond the cap.
            while self.applied.len() > APPLIED_REGISTRY_CAP {
                self.applied.pop_first();
            }
        }
        Ok(outcome)
    }

    /// Marks the open transaction `Rejected` after the controller
    /// refused the prepared apply — the columns, fixed targets and
    /// fixed lanes stay for Inspect / Retry / Abort.
    pub fn note_commit_rejected(&mut self, id: RecordingTransactionId) {
        if let Some(tx) = self.recording.as_mut()
            && tx.id == id
        {
            tx.state = RecordingTxnState::Rejected;
        }
    }

    /// A foreign project change succeeded (an apply/undo/redo that is
    /// **not** this transaction's own commit): the fixed revision is
    /// now stale — close the write gate. Runtime state whose target no
    /// longer exists is explicitly closed (`TargetDeleted`) — a
    /// gesture/return/writer can never silently continue on a deleted
    /// target. The recorded columns are retained; the next prepare
    /// reports `Conflict`.
    pub fn note_project_changed(&mut self, project: &Project) {
        self.close_write_range(project, SealReason::ProjectChange, self.playback.position);
        for target in self.runtime_targets() {
            if !project.control.targets.contains_key(&target) {
                self.close_target_runtime(target, WriterCloseReason::TargetDeleted);
            }
        }
    }

    /// Marks the open transaction `Rejected` — used by
    /// `prepare_record_commit`'s structural checks.
    fn reject_open(&mut self) {
        if let Some(tx) = self.recording.as_mut() {
            tx.state = RecordingTxnState::Rejected;
        }
    }

    /// The user-facing report behind an explicit rebase approval:
    /// for every recorded lane, whether its fixed target and lane
    /// still exist, how much was recorded, and which existing regions
    /// a `Replace` write would overwrite. Never merges — it only
    /// reports what approval would write.
    ///
    /// # Errors
    /// [`RecordCommitError::NoTransaction`].
    pub fn rebase_preview(
        &self,
        project: &Project,
        current_revision: ProjectRevision,
    ) -> Result<RebasePreview, RecordCommitError> {
        let Some(tx) = self.recording.as_ref() else {
            return Err(RecordCommitError::NoTransaction);
        };
        // Iterate the fixed target→lane mapping joined with the
        // recorded segments: a segment lane is always written through
        // `fixed_lanes`, so every recorded lane reports its target.
        let mut lanes = Vec::new();
        for (target, lane) in &tx.fixed_lanes {
            let Some(points) = tx.segments.get(lane) else {
                continue;
            };
            let recorded_range = points
                .first()
                .and_then(|first| points.last().map(|last| (first.pos, last.pos)));
            let overlapping_regions = recorded_range.map_or(0, |(start, last)| {
                // The written extent covers the last point by the same
                // smallest increment `prepare` uses.
                let end = last
                    .checked_add(BeatPoint::new(1, u32::MAX).unwrap_or(last))
                    .unwrap_or(last);
                BeatRange::try_new(start, end).map_or(0, |rec_range| {
                    project
                        .control
                        .lanes
                        .values()
                        .filter(|l| l.target == *target)
                        .flat_map(|l| l.regions.iter())
                        .filter(|r| r.range.overlaps(&rec_range))
                        .count()
                })
            });
            lanes.push(RebaseLaneReport {
                lane: *lane,
                fixed_target: *target,
                target_exists: project.control.targets.contains_key(target),
                lane_exists: project
                    .control
                    .lanes
                    .get(lane)
                    .is_some_and(|l| l.target == *target),
                recorded_points: points.len(),
                recorded_range,
                overlapping_regions,
            });
        }
        Ok(RebasePreview {
            id: tx.id,
            fixed_revision: tx.fixed_revision,
            current_revision,
            sealed: tx.sealed,
            lanes,
        })
    }

    /// Explicit user approval to rebase the transaction onto the
    /// current revision: re-pins `fixed_revision` and returns the
    /// transaction to `Pending` for a fresh prepare. Never reopens
    /// the write gate — the seal (if any) is kept — and never runs
    /// automatically.
    ///
    /// # Errors
    /// [`RecordCommitError::NoTransaction`] or
    /// [`RecordCommitError::UnknownTransaction`].
    pub fn approve_rebase(
        &mut self,
        id: RecordingTransactionId,
        current_revision: ProjectRevision,
    ) -> Result<(), RecordCommitError> {
        let Some(tx) = self.recording.as_mut() else {
            return Err(RecordCommitError::NoTransaction);
        };
        if tx.id != id {
            return Err(RecordCommitError::UnknownTransaction { id });
        }
        tx.fixed_revision = current_revision;
        tx.state = RecordingTxnState::Pending;
        Ok(())
    }

    /// The applied-registry receipt for `id`, if committed.
    #[must_use]
    pub fn applied_record(&self, id: RecordingTransactionId) -> Option<AppliedRecord> {
        self.applied.get(&id).copied()
    }

    /// The most recent lifecycle disposition (commit or abort).
    #[must_use]
    pub fn last_disposition(&self) -> Option<RecordDisposition> {
        self.last_disposition
    }

    /// Discards the open transaction — nothing is applied; the
    /// recorded columns are dropped and the abort is recorded as the
    /// last disposition.
    pub fn record_abort(&mut self) {
        if let Some(tx) = self.recording.take() {
            self.last_disposition = Some(RecordDisposition::Aborted { id: tx.id });
        }
    }

    // -- evaluation / dispatch ------------------------------------------

    /// Evaluates `target` at `pos`, injecting any held manual override
    /// (gesture/latch/write-held) — or an in-flight Touch return's
    /// blended reference — as the pre-modulation reference.
    ///
    /// The return runs `v = release + (v_auto - release) · w` with
    /// `w = 3x² - 2x³`, `x = elapsed / return_duration` on the session
    /// clock (issue #131). `v_auto` is the evaluation of the
    /// automation layer at `pos` itself — a series changed mid-return
    /// is followed continuously. The result reports
    /// [`ReferenceSource::Return`]; nothing is written to the project.
    ///
    /// # Errors
    /// Propagates [`synvelis_project::evaluate_control`].
    pub fn evaluate(
        &self,
        project: &Project,
        target: ParameterTargetId,
        pos: BeatPoint,
    ) -> Result<ControlEvaluation, Error> {
        let manual = self
            .manual_overrides
            .get(&target)
            .or_else(|| self.latch_held.get(&target))
            .copied();
        let mut eval = evaluate_control(project, target, pos, manual)?;
        if manual.is_none()
            && let Some(ret) = self.touch_returns.get(&target)
        {
            // `v_auto` is the reference evaluated without an override —
            // the automation series where it reaches, else base.
            let v_auto = eval.reference;
            let w = self.return_blend(ret);
            let blended = ret.release_value + (v_auto - ret.release_value) * w;
            let mut returned = evaluate_control(project, target, pos, Some(blended))?;
            returned.reference_source = ReferenceSource::Return;
            eval = returned;
        }
        Ok(eval)
    }

    /// Lowers the evaluated value into an rf#186 `TimedParamEvent` at
    /// `pos`. `TimedEvent` rejection degrades explicitly to
    /// `BlockBoundary` — reported as [`Lowered::Degraded`].
    ///
    /// # Errors
    /// Evaluation errors, tempo-map compile errors, or write
    /// rejections that cannot be degraded.
    pub fn lower(
        &self,
        project: &Project,
        target: ParameterTargetId,
        pos: BeatPoint,
        means: ParamWriteMeans,
    ) -> Result<Lowered, Error> {
        let eval = self.evaluate(project, target, pos)?;
        let t = Self::target(project, target)?;
        let map = project
            .tempo_map
            .compile()
            .map_err(|e| Error::Validation(format!("tempo map: {e}")))?;
        let media = map
            .beat_to_media(pos.to_beat(), self.frame_base)
            .map_err(|_| Error::Overflow)?;
        match synvelis_project::lower_event(t, eval.host_normalized, media, means) {
            Ok(event) => Ok(Lowered::Exact(event)),
            Err(Error::WriteRejected { .. }) if means == ParamWriteMeans::TimedEvent => {
                synvelis_project::lower_event(
                    t,
                    eval.host_normalized,
                    media,
                    ParamWriteMeans::BlockBoundary,
                )
                .map(|event| Lowered::Degraded {
                    requested: means,
                    event,
                })
            }
            Err(e) => Err(e),
        }
    }

    // -- observations -----------------------------------------------------

    /// Feeds a plugin-reported value + measurement stamp (rf#185).
    ///
    /// # Errors
    /// [`Error::Validation`] on out-of-domain value.
    pub fn feed_plugin_report(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
        value: f64,
        stamp: MediaTimestamp,
    ) -> Result<(), Error> {
        Self::target(project, target)?;
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(Error::Validation("reported value outside [0,1]".into()));
        }
        self.observations.entry(target).or_default().plugin_reported =
            Some(ObservationData { value, stamp });
        Ok(())
    }

    /// Feeds a probe-captured value + measurement stamp (rf#185).
    ///
    /// # Errors
    /// [`Error::Validation`] on out-of-domain value.
    pub fn feed_probe(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
        value: f64,
        stamp: MediaTimestamp,
    ) -> Result<(), Error> {
        Self::target(project, target)?;
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(Error::Validation("probe value outside [0,1]".into()));
        }
        self.observations.entry(target).or_default().probe = Some(ObservationData { value, stamp });
        Ok(())
    }

    /// The target's observation channels.
    #[must_use]
    pub fn observation(&self, target: ParameterTargetId) -> ControlObservation {
        self.observations.get(&target).cloned().unwrap_or_default()
    }

    /// The freshness state: no data → `Unavailable`; an `Estimated`
    /// stamp → `Stale`; a probe capture → `Captured`; a plugin report
    /// is `Live` while rolling, `AtPlayhead` while stopped.
    #[must_use]
    pub fn observation_state(&self, target: ParameterTargetId) -> ObservationState {
        let Some(obs) = self.observations.get(&target) else {
            return ObservationState::Unavailable;
        };
        if let Some(p) = obs.probe {
            return if p.stamp.provenance == TimestampProvenance::Estimated {
                ObservationState::Stale
            } else {
                ObservationState::Captured
            };
        }
        if let Some(r) = obs.plugin_reported {
            return if r.stamp.provenance == TimestampProvenance::Estimated {
                ObservationState::Stale
            } else if self.playback.status == PlaybackStatus::Playing {
                ObservationState::Live
            } else {
                ObservationState::AtPlayhead
            };
        }
        ObservationState::Unavailable
    }

    // -- stamp→gesture (rf#188) -------------------------------------------

    /// Installs the clock anchor used by [`map_stamp_to_frame`]. The
    /// session's playback epoch follows the anchor's frame epoch —
    /// it is the engine clock's own epoch, and a mismatched stored
    /// epoch would make every `stamp_gesture` reject as
    /// `EpochMismatch` with no path to correct it. A device/clock
    /// epoch change seals an open recording transaction — the epoch
    /// fixed at `record_start` no longer holds (issue #133).
    pub fn install_clock_anchor(&mut self, project: &Project, anchor: ClockAnchor) {
        let epoch_changed = self
            .recording
            .as_ref()
            .is_some_and(|tx| tx.sealed.is_none() && tx.epoch != anchor.frame.epoch());
        if epoch_changed {
            self.close_write_range(project, SealReason::EpochChange, self.playback.position);
        }
        self.playback.epoch = anchor.frame.epoch();
        self.clock_anchor = Some(anchor);
    }

    /// The host detected a stale control plan / `LastGood` graph —
    /// writes through it could land on a mismatched parameter
    /// (issue #133). The open transaction's write range closes at the
    /// current position; runtime gesture state is untouched (the
    /// gesture preview remains a session concern, not a project one).
    /// No-op with no open transaction.
    pub fn note_graph_stale(&mut self, project: &Project) {
        self.close_write_range(project, SealReason::GraphStale, self.playback.position);
    }

    /// Maps a native [`InputStamp`] to an engine frame exactly once via
    /// [`map_stamp_to_frame`], resolves it to a beat position through
    /// the tempo map, and feeds it to the target's active gesture.
    ///
    /// # Errors
    /// [`Error::Validation`] on missing anchor/gesture or conversion
    /// overflow; [`crate::WorkspaceError::StampRejected`] is reported
    /// through the workspace error enum at the call boundary — this
    /// method returns the project error surface.
    pub fn stamp_gesture(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
        stamp: &InputStamp,
        value: f64,
    ) -> Result<(), Error> {
        let anchor = self
            .clock_anchor
            .ok_or_else(|| Error::Validation("no clock anchor installed".into()))?;
        // `now` derives from the transport position — the session's
        // single authoritative clock — so a failed conversion surfaces
        // here instead of leaving a stored frame silently stale.
        let media_now = project
            .tempo_map
            .compile()
            .map_err(|e| Error::Validation(format!("tempo map: {e}")))?
            .beat_to_media(self.playback.position.to_beat(), self.frame_base)
            .map_err(|_| Error::Overflow)?;
        let now = EngineFrame::new(
            self.playback.epoch,
            u64::try_from(media_now.value()).map_err(|_| Error::Overflow)?,
        );
        let mapped = map_stamp_to_frame(stamp, &anchor, self.frame_base, now)
            .map_err(|rej| Error::Validation(format!("stamp rejected: {rej:?}")))?;
        let media = MediaTime::new(
            i64::try_from(mapped.frame.frame()).map_err(|_| Error::Overflow)?,
            self.frame_base,
        );
        let beat = project
            .tempo_map
            .compile()
            .map_err(|e| Error::Validation(format!("tempo map: {e}")))?
            .media_to_beat(media)
            .map_err(|_| Error::Overflow)?;
        self.gesture_update(target, BeatPoint::from_beat(beat), value)
    }

    // -- projections (read-only views of Project::control) -----------------

    /// The Lane projection: every `(lane, region)` feeding `target`,
    /// borrowed from the single source — no duplicated series.
    #[must_use]
    pub fn lane_view(
        project: &Project,
        target: ParameterTargetId,
    ) -> Vec<(&AutomationLaneId, &AutomationRegion)> {
        project
            .control
            .lanes
            .values()
            .filter(|l| l.target == target)
            .flat_map(|l| l.regions.iter().map(move |r| (&l.id, r)))
            .collect()
    }

    /// The Signal-Flow projection: assignment views into `target`,
    /// sourced from the same `Project::control` entities.
    #[must_use]
    pub fn signal_flow_view(
        project: &Project,
        target: ParameterTargetId,
    ) -> Vec<AssignmentView<'_>> {
        project
            .control
            .assignments
            .values()
            .filter(|a| a.target == target)
            .filter_map(|a| {
                project
                    .control
                    .sources
                    .get(&a.source)
                    .map(|s| AssignmentView {
                        id: a.id,
                        source: s,
                        depth: a.depth,
                        depth_source: a.depth_source,
                        enabled: a.enabled,
                    })
            })
            .collect()
    }

    /// The Inspector projection: evaluation + observation + mode +
    /// arm + capability — the agreed breakdown shape.
    ///
    /// # Errors
    /// Propagates [`ControlSession::evaluate`] errors.
    pub fn inspector_breakdown(
        &self,
        project: &Project,
        target: ParameterTargetId,
        pos: BeatPoint,
    ) -> Result<InspectorBreakdown, Error> {
        let evaluation = self.evaluate(project, target, pos)?;
        let obs = self.observation(target);
        Ok(InspectorBreakdown {
            evaluation,
            observation: self.observation_state(target),
            plugin_reported: obs.plugin_reported,
            probe: obs.probe,
            mode: self.mode(target),
            arm: self.arm(target),
            writer_state: self.writer_state(target),
            capability: Self::target(project, target)?.capability,
        })
    }

    /// Resolves a target's binding — owning entity and declared time
    /// coordinate (exposed for tests and UI coordinate mapping).
    ///
    /// # Errors
    /// [`Error::NotFound`] for an unknown target.
    pub fn target_binding(
        project: &Project,
        target: ParameterTargetId,
    ) -> Result<(ControlOwner, AutomationTimeScope), Error> {
        let t = Self::target(project, target)?;
        Ok((t.owner, t.time_scope))
    }
}

/// Deterministic fingerprint of a transaction's canonical payload —
/// FNV-1a over the id, fixed revision, start position, epoch, loop
/// pass, fixed targets/lanes/scopes, write-range coverage and every
/// recorded point (position + value bits). Stable across runs and
/// platforms; two transactions with the same fingerprint are
/// byte-identical payloads.
fn txn_fingerprint(tx: &RecordingTransaction) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(PRIME);
        }
    };
    mix(&tx.id.0.to_le_bytes());
    mix(&tx.fixed_revision.0.to_le_bytes());
    mix(&tx.started_at.num().to_le_bytes());
    mix(&tx.started_at.den().to_le_bytes());
    mix(&tx.epoch.get().to_le_bytes());
    mix(&tx.pass.0.to_le_bytes());
    for target in &tx.fixed_targets {
        mix(&target.raw().to_le_bytes());
    }
    for (target, lane) in &tx.fixed_lanes {
        mix(&target.raw().to_le_bytes());
        mix(&lane.raw().to_le_bytes());
    }
    for (target, (owner, scope)) in &tx.fixed_scopes {
        mix(&target.raw().to_le_bytes());
        mix(format!("{owner:?}").as_bytes());
        mix(format!("{scope:?}").as_bytes());
    }
    for (lane, bound) in &tx.coverage_end {
        mix(&lane.raw().to_le_bytes());
        mix(&bound.num().to_le_bytes());
        mix(&bound.den().to_le_bytes());
    }
    for (lane, points) in &tx.segments {
        mix(&lane.raw().to_le_bytes());
        for p in points {
            mix(&p.pos.num().to_le_bytes());
            mix(&p.pos.den().to_le_bytes());
            mix(&p.value.to_bits().to_le_bytes());
        }
    }
    h
}
