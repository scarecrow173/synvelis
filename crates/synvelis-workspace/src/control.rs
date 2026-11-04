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

use std::collections::BTreeMap;
use std::fmt;

use ruviform_media_core::{
    ClockAnchor, ClockEpochId, EngineFrame, InputStamp, MediaTime, MediaTimestamp, TimeBase,
    TimestampProvenance, map_stamp_to_frame,
};
use ruviform_plugin_core::{ParamWriteMeans, TimedParamEvent};
use synvelis_project::{
    AutomationLaneId, AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange,
    ControlEvaluation, ControlOwner, Error, Interpolation, ModulationAssignmentId,
    ParameterTargetId, Project, ProjectOp, ProjectRevision, TargetCapability, evaluate_control,
};

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
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            position: BeatPoint::ZERO,
            loop_range: None,
            epoch: ruviform_media_core::ClockEpochId::new(1),
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

/// An in-progress manual gesture on one target.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlGesture {
    /// Position where the gesture began.
    pub started_at: BeatPoint,
    /// `(pos, normalized value)` pairs in arrival order.
    pub points: Vec<(BeatPoint, f64)>,
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
    /// Collected manual (pre-modulation) points per destination lane.
    pub segments: BTreeMap<AutomationLaneId, Vec<AutomationPoint>>,
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
            self.modes.insert(target, mode);
            // A fixed target's mode change mid-recording closes the
            // write gate permanently — the transaction's captured
            // context no longer holds.
            self.seal_fixed_target(target, SealReason::ModeChanged);
        }
        if mode != AutomationMode::Latch {
            self.latch_held.remove(&target);
        }
        if mode != AutomationMode::Write && !self.gestures.contains_key(&target) {
            self.manual_overrides.remove(&target);
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
            self.seal_fixed_target(target, SealReason::ArmChanged);
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
    fn seal_fixed_target(&mut self, target: ParameterTargetId, reason: SealReason) {
        if let Some(tx) = self.recording.as_mut()
            && tx.sealed.is_none()
            && tx.fixed_targets.contains(&target)
        {
            tx.sealed = Some(reason);
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

    /// Starts rolling. Session-only.
    pub fn transport_play(&mut self) {
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

    /// Stops rolling. Latch holds and write-held overrides end —
    /// nothing carries into the next playback. An open recording
    /// transaction is sealed: a stopped session never resumes into
    /// the same transaction on the next play.
    pub fn transport_stop(&mut self) {
        self.playback.status = PlaybackStatus::Stopped;
        self.latch_held.clear();
        self.manual_overrides
            .retain(|t, _| self.gestures.contains_key(t));
        self.seal_recording(SealReason::Stopped);
    }

    /// Seeks the playhead (chase = the next evaluation simply reads the
    /// new position). Session-only. An open recording transaction is
    /// sealed — the recorded interval is no longer contiguous.
    pub fn transport_seek(&mut self, pos: BeatPoint) {
        self.playback.position = pos;
        self.seal_recording(SealReason::Seek);
    }

    /// Seals the open transaction's write gate with `reason` — the
    /// recorded columns stay for inspect/commit/abort.
    fn seal_recording(&mut self, reason: SealReason) {
        if let Some(tx) = self.recording.as_mut()
            && tx.sealed.is_none()
        {
            tx.sealed = Some(reason);
        }
    }

    /// Sets or clears the loop range.
    ///
    /// # Errors
    /// [`Error::Validation`] when `end <= start`.
    pub fn set_loop(&mut self, range: Option<(BeatPoint, BeatPoint)>) -> Result<(), Error> {
        if let Some((start, end)) = range
            && end <= start
        {
            return Err(Error::Validation("loop end must exceed start".into()));
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
            if let Some(tx) = self.recording.as_mut()
                && tx.sealed.is_none()
            {
                tx.sealed = Some(SealReason::LoopWrap);
                effect.sealed = true;
            }
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

    // -- gestures ------------------------------------------------------

    /// Begins a manual gesture on `target`. The override starts at the
    /// currently evaluated reference value so touching a knob does not
    /// jump. `READONLY` targets reject the gesture — the capability
    /// contract is enforced at the session boundary too.
    ///
    /// # Errors
    /// [`Error::NotFound`]/[`Error::WriteRejected`]/[`Error::Validation`].
    pub fn gesture_begin(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
        at: BeatPoint,
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
        let start_value =
            evaluate_control(project, target, at, None).map_or(t.base, |e| e.reference);
        self.gestures.insert(
            target,
            ControlGesture {
                started_at: at,
                points: Vec::new(),
            },
        );
        self.manual_overrides.insert(target, start_value);
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
    /// override are dropped and the target returns to its
    /// recorded/evaluated series. Nothing is written — a cancel never
    /// rewrites committed state and never creates an undo entry.
    ///
    /// # Errors
    /// [`Error::Validation`] on no active gesture.
    pub fn gesture_cancel(&mut self, target: ParameterTargetId) -> Result<(), Error> {
        if self.gestures.remove(&target).is_none() {
            return Err(Error::Validation(format!("no gesture on {target}")));
        }
        self.manual_overrides.remove(&target);
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
        self.next_txn_id = self.next_txn_id.checked_add(1).ok_or(Error::Overflow)?;
        let id = RecordingTransactionId(self.next_txn_id);
        self.recording = Some(RecordingTransaction {
            id,
            started_at: self.playback.position,
            epoch: self.playback.epoch,
            fixed_targets: fixed,
            fixed_lanes,
            fixed_revision: revision,
            segments: BTreeMap::new(),
            sealed: None,
            state: RecordingTxnState::Pending,
        });
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
        let (owner, time_scope) = (bound.owner, bound.time_scope);
        let lane = *self
            .recording
            .as_ref()
            .ok_or_else(|| Error::Validation("no open recording".into()))?
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
        let mut resolved = Vec::with_capacity(points.len());
        for (pos, value) in points {
            match synvelis_project::resolve_scope_pos(project, owner, time_scope, *pos) {
                Ok(local) => resolved.push((local, *value)),
                Err(synvelis_project::Error::ScopeUnavailable { .. }) => {}
                Err(e) => return Err(e),
            }
        }
        let written = resolved.len();
        let rec = self
            .recording
            .as_mut()
            .ok_or_else(|| Error::Validation("no open recording".into()))?;
        let total = rec.segments.values().map(Vec::len).sum::<usize>();
        if total + resolved.len() > RECORD_POINT_BUDGET {
            return Err(Error::Validation(format!(
                "recording point budget of {RECORD_POINT_BUDGET} exceeded"
            )));
        }
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
        Ok(written)
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
                // Cover the last point: extend the half-open range by
                // the smallest representable increment (2^-32 beats —
                // below any meaningful user point, documented).
                let end = last_pos
                    .checked_add(
                        BeatPoint::new(1, u32::MAX)
                            .map_err(|e| RecordCommitError::Invalid(e.to_string()))?,
                    )
                    .map_err(|e| RecordCommitError::Invalid(e.to_string()))?;
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
    /// now stale — close the write gate. The recorded columns are
    /// retained; the next prepare reports `Conflict`.
    pub fn note_project_changed(&mut self) {
        self.seal_recording(SealReason::ProjectChange);
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
    /// (gesture/latch/write-held) as the pre-modulation reference.
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
        evaluate_control(project, target, pos, manual)
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
    /// `EpochMismatch` with no path to correct it.
    pub fn install_clock_anchor(&mut self, anchor: ClockAnchor) {
        self.playback.epoch = anchor.frame.epoch();
        self.clock_anchor = Some(anchor);
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
/// FNV-1a over the id, fixed revision, start position, epoch, fixed
/// targets/lanes and every recorded point (position + value bits).
/// Stable across runs and platforms; two transactions with the same
/// fingerprint are byte-identical payloads.
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
    for target in &tx.fixed_targets {
        mix(&target.raw().to_le_bytes());
    }
    for (target, lane) in &tx.fixed_lanes {
        mix(&target.raw().to_le_bytes());
        mix(&lane.raw().to_le_bytes());
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
