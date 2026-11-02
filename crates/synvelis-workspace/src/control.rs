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
//! [`ProjectSession::control_record_commit`] for a recording session),
//! which is one atomic apply = one undo step.
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

use ruviform_media_core::{
    ClockAnchor, EngineFrame, InputStamp, MediaTime, MediaTimestamp, TimeBase, TimestampProvenance,
    map_stamp_to_frame,
};
use ruviform_plugin_core::{ParamWriteMeans, TimedParamEvent};
use synvelis_project::{
    AutomationLaneId, AutomationPoint, AutomationRegion, BeatPoint, BeatRange, ControlEvaluation,
    ControlScope, Error, Interpolation, ModulationAssignmentId, ParameterTargetId, Project,
    ProjectOp, ProjectRevision, TargetCapability, evaluate_control,
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
}

/// Session playback state — kept in beats (`BeatPosition`-exact) so
/// tempo edits never rewrite history; `frame`/`epoch` track the engine
/// clock for stamp→frame mapping (rf#188).
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
    /// Current engine frame.
    pub frame: u64,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            status: PlaybackStatus::Stopped,
            position: BeatPoint::ZERO,
            loop_range: None,
            epoch: ruviform_media_core::ClockEpochId::new(1),
            frame: 0,
        }
    }
}

/// Why a recording transaction was sealed — further writes are
/// rejected; the transaction must be committed or aborted.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SealReason {
    /// The playhead wrapped the loop end: continuing would silently
    /// overwrite the interval just recorded — forbidden by the agreed
    /// model.
    LoopWrap,
}

/// An in-progress manual gesture on one target.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlGesture {
    /// Position where the gesture began.
    pub started_at: BeatPoint,
    /// `(pos, normalized value)` pairs in arrival order.
    pub points: Vec<(BeatPoint, f64)>,
}

/// One recording session = one undo transaction. Target, arm state
/// and project revision are **fixed at start** — later Inspector
/// follow/focus changes cannot retarget the write.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingTransaction {
    /// Session start position (project beats).
    pub started_at: BeatPoint,
    /// Targets armed+write-capable when `record_start` ran.
    pub fixed_targets: Vec<ParameterTargetId>,
    /// Live project revision at start.
    pub fixed_revision: ProjectRevision,
    /// Collected manual (pre-modulation) points per destination lane.
    pub segments: BTreeMap<AutomationLaneId, Vec<AutomationPoint>>,
    /// `Some` once sealed — further `record_write` is rejected.
    pub sealed: Option<SealReason>,
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
        self.modes.insert(target, mode);
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
        self.arms.insert(target, arm);
        Ok(())
    }

    /// Enables/disables automation recording (session-only).
    pub fn set_record_enabled(&mut self, enabled: bool) {
        self.record_enabled = enabled;
    }

    /// Fixes which automation lane a write-mode target records into.
    ///
    /// # Errors
    /// [`Error::Validation`] when the lane does not feed `target`.
    pub fn bind_write_lane(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
        lane: AutomationLaneId,
    ) -> Result<(), Error> {
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

    /// Stops rolling. Latch holds and write-held overrides end —
    /// nothing carries into the next playback.
    pub fn transport_stop(&mut self) {
        self.playback.status = PlaybackStatus::Stopped;
        self.latch_held.clear();
        self.manual_overrides
            .retain(|t, _| self.gestures.contains_key(t));
    }

    /// Seeks the playhead (chase = the next evaluation simply reads the
    /// new position). Session-only.
    ///
    /// # Errors
    /// [`Error::Validation`] when `pos` is not finite-representable.
    pub fn transport_seek(&mut self, pos: BeatPoint) {
        self.playback.position = pos;
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
        // Track the engine frame from the position (beat → media →
        // frame ticks) so stamp→frame `now` stays fresh.
        if let Ok(map) = project.tempo_map.compile()
            && let Ok(media) = map.beat_to_media(pos.to_beat(), self.frame_base)
        {
            self.playback.frame = u64::try_from(media.value()).unwrap_or(0);
        }
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
            self.record_write(project, target, pos, value)?;
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
    /// [`GestureEnd::BaseEdit`] — the caller applies `SetParameterBase`.
    /// `Touch`/`Latch`/`Write` write the gesture interval only when the
    /// write gates (armed + record + rolling + open transaction fixing
    /// this target) hold. Gesture point positions are normalized into
    /// the target's scope coordinate before they enter the transaction.
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
                Ok(GestureEnd::BaseEdit { value: last })
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

    // -- recording ------------------------------------------------------

    /// The open recording transaction, if any.
    #[must_use]
    pub fn recording(&self) -> Option<&RecordingTransaction> {
        self.recording.as_ref()
    }

    /// Opens a recording transaction. Requires record-enabled +
    /// rolling + at least one armed write-mode target. The target set,
    /// arm state and project revision are **fixed now** — later
    /// Inspector follow changes cannot retarget the write.
    ///
    /// # Errors
    /// [`Error::Validation`] when the gates do not hold or a
    /// transaction is already open.
    pub fn record_start(&mut self, revision: ProjectRevision) -> Result<(), Error> {
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
        self.recording = Some(RecordingTransaction {
            started_at: self.playback.position,
            fixed_targets: fixed,
            fixed_revision: revision,
            segments: BTreeMap::new(),
            sealed: None,
        });
        Ok(())
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
    fn record_points(
        &mut self,
        project: &Project,
        target: ParameterTargetId,
        points: &[(BeatPoint, f64)],
    ) -> Result<usize, Error> {
        let scope = Self::target(project, target)?.owner;
        let lane = *self
            .write_lanes
            .get(&target)
            .ok_or_else(|| Error::Validation(format!("no write lane bound for {target}")))?;
        let mut resolved = Vec::with_capacity(points.len());
        for (pos, value) in points {
            match synvelis_project::resolve_scope_pos(project, scope, *pos) {
                Ok(local) => resolved.push((local, *value)),
                Err(synvelis_project::Error::ScopeUnavailable { .. }) => {}
                Err(e) => return Err(e),
            }
        }
        let written = resolved.len();
        let seg = self
            .recording
            .as_mut()
            .ok_or_else(|| Error::Validation("no open recording".into()))?
            .segments
            .entry(lane)
            .or_default();
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

    /// Builds the commit op list for the open transaction and closes
    /// it: one `Replace` region-write per lane — an explicit overwrite
    /// of the recorded interval — inside one `Composite` so the whole
    /// session lands as **one undo step**.
    ///
    /// # Errors
    /// [`Error::Validation`] on no open transaction or an interval
    /// edge that cannot be represented.
    pub fn build_commit_ops(&mut self) -> Result<(bool, Vec<ProjectOp>), Error> {
        let Some(tx) = self.recording.take() else {
            return Err(Error::Validation("no open recording".into()));
        };
        let mut ops = Vec::new();
        for (lane, mut points) in tx.segments {
            points.sort_by_key(|p| p.pos);
            let (Some(first), Some(last)) = (points.first(), points.last()) else {
                continue;
            };
            let (start, last_pos) = (first.pos, last.pos);
            // Cover the last point: extend the half-open range by the
            // smallest representable increment (2^-32 beats — below
            // any meaningful user point, documented).
            let end = last_pos.checked_add(BeatPoint::new(1, u32::MAX)?)?;
            let range = BeatRange::try_new(start, end)?;
            ops.push(ProjectOp::WriteAutomationRegion {
                lane,
                region: AutomationRegion {
                    range,
                    interpolation: Interpolation::Linear,
                    points,
                },
                policy: synvelis_project::ConflictPolicy::Replace,
            });
        }
        Ok((tx.sealed.is_some(), ops))
    }

    /// Discards the open transaction — nothing is applied.
    pub fn record_abort(&mut self) {
        self.recording = None;
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

    /// Installs the clock anchor used by [`map_stamp_to_frame`].
    pub fn install_clock_anchor(&mut self, anchor: ClockAnchor) {
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
        let now = EngineFrame::new(self.playback.epoch, self.playback.frame);
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

    /// Resolves which scope a target belongs to (exposed for tests and
    /// UI coordinate mapping).
    ///
    /// # Errors
    /// [`Error::NotFound`] for an unknown target.
    pub fn target_scope(
        project: &Project,
        target: ParameterTargetId,
    ) -> Result<ControlScope, Error> {
        Ok(Self::target(project, target)?.owner)
    }
}
