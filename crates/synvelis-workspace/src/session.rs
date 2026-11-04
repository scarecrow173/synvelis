//! Project session — the single project source of truth plus the
//! transport stub and the engine stub proving RT/graph separation
//! (WS-A scope 5).
//!
//! [`ProjectSession`] owns the sole `ProjectController`. Views read
//! projections from it; every mutation flows through routed commands.
//! A successful mutation advances the live revision and the engine
//! stub rebuilds its (fake) graph — dock/layout/focus operations never
//! touch either.

use synvelis_project::{
    Command, Error, Operation, ParameterTargetId, Project, ProjectController, ProjectOp,
    ProjectRevision,
};

use crate::control::{
    ControlSession, GestureEnd, PreparedRecordingCommit, RecordCommitError, RecordingOutcome,
    RecordingTransactionId,
};

/// Transport playback state — a stub, but a real concept that view
/// commands must not confuse with project mutations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransportState {
    /// Whether transport is rolling.
    pub playing: bool,
    /// Current playhead in ticks (arbitrary timeline unit).
    pub position_ticks: i64,
}

/// Stands in for the audio engine's graph builder. `graph_builds`
/// counts graph (re)builds; it only ever advances when the project
/// revision advances — proving view/layout changes do not rebuild.
#[derive(Debug, Clone, Default)]
pub struct EngineStub {
    /// Number of graph builds performed.
    pub graph_builds: u64,
    /// Project revision the current graph was built from.
    pub last_built_revision: ProjectRevision,
}

impl EngineStub {
    /// Rebuilds the graph for `revision`.
    pub fn on_project_revision(&mut self, revision: ProjectRevision) {
        self.graph_builds += 1;
        self.last_built_revision = revision;
    }
}

/// The one canonical project session.
pub struct ProjectSession {
    controller: ProjectController,
    transport: TransportState,
    engine: EngineStub,
    control: ControlSession,
}

impl ProjectSession {
    /// Wraps `controller` — the only project source of truth.
    #[must_use]
    pub fn new(controller: ProjectController) -> Self {
        Self {
            controller,
            transport: TransportState::default(),
            engine: EngineStub::default(),
            control: ControlSession::default(),
        }
    }

    /// The project controller (read access for projections).
    #[must_use]
    pub fn controller(&self) -> &ProjectController {
        &self.controller
    }

    /// Mutable controller access for undo/redo/save flows.
    #[must_use]
    pub fn controller_mut(&mut self) -> &mut ProjectController {
        &mut self.controller
    }

    /// The transport stub.
    #[must_use]
    pub fn transport(&self) -> &TransportState {
        &self.transport
    }

    /// The engine stub.
    #[must_use]
    pub fn engine(&self) -> &EngineStub {
        &self.engine
    }

    /// The session-scoped parameter-control state (issue #40): UI
    /// modes, arms, gestures, recording transaction, observations.
    /// Never persisted, never a project mutation by itself.
    #[must_use]
    pub fn control(&self) -> &ControlSession {
        &self.control
    }

    /// Mutable control-session access (mode/arm/transport/gesture).
    #[must_use]
    pub fn control_mut(&mut self) -> &mut ControlSession {
        &mut self.control
    }

    /// Runs `f` with the control session and the live project — the
    /// borrow-safe bridge for session ops that must read the project
    /// (target existence, capabilities, scope resolution) while
    /// mutating session state.
    pub fn control_with<R>(&mut self, f: impl FnOnce(&mut ControlSession, &Project) -> R) -> R {
        f(&mut self.control, self.controller.live())
    }

    /// Ends a manual gesture on `target`. `Off` mode returns
    /// `BaseEdit` and applies `SetParameterBase` — one undoable edit;
    /// every other outcome touches session state only.
    ///
    /// # Errors
    /// [`Error::Validation`] on no active gesture; controller errors
    /// for the `Off`-mode base apply.
    pub fn control_gesture_end(&mut self, target: ParameterTargetId) -> Result<GestureEnd, Error> {
        let end = self.control_with(|c, p| c.gesture_end(p, target))?;
        if let GestureEnd::BaseEdit { value } = end {
            let rev = self.revision();
            self.apply(ProjectOp::SetParameterBase { target, value }, rev)?;
        }
        Ok(end)
    }

    /// Commits the open recording transaction as **one undo step**
    /// through the PC-C1 protocol (issue #98):
    /// [`ControlSession::prepare_record_commit`] snapshots the columns
    /// non-destructively and CAS-checks the transaction's fixed
    /// revision, the controller applies the `Composite` against that
    /// expectation, and only a successful apply reaches
    /// [`ControlSession::finish_record_commit`] — the sole consumer.
    /// A conflict or rejection leaves every recorded point in place.
    ///
    /// The internal apply deliberately bypasses
    /// [`ControlSession::note_project_changed`] — a transaction's own
    /// commit is not a foreign project change.
    ///
    /// # Errors
    /// [`RecordCommitError::NoTransaction`]/`Conflict`/`Invalid` from
    /// prepare, or `Rejected` carrying the controller error — the
    /// transaction always retains its columns.
    pub fn control_record_commit(&mut self) -> Result<RecordingOutcome, RecordCommitError> {
        let current = self.revision();
        let prepared = self.control_with(|c, p| c.prepare_record_commit(p, current))?;
        let applied = match prepared.op.clone() {
            Some(op) => match self.apply_internal(op, prepared.expected_revision) {
                Ok(rev) => Some(rev),
                Err(reason) => {
                    self.control.note_commit_rejected(prepared.id);
                    return Err(RecordCommitError::Rejected { reason });
                }
            },
            None => None,
        };
        self.control.finish_record_commit(&prepared, applied)
    }

    /// Commits an already-prepared recording transaction — the
    /// idempotent path for a delayed/duplicate ACK or an explicit
    /// retry after a rejection. Same id + fingerprint is
    /// `AlreadyApplied`; same id + different fingerprint is
    /// `IntegrityConflict`; an unknown id is `UnknownTransaction`.
    ///
    /// # Errors
    /// [`RecordCommitError::AlreadyApplied`]/`IntegrityConflict`/
    /// `UnknownTransaction`/`NoTransaction`, or `Rejected` carrying
    /// the controller error.
    pub fn control_record_finish(
        &mut self,
        prepared: &PreparedRecordingCommit,
    ) -> Result<RecordingOutcome, RecordCommitError> {
        // Identity first: a duplicate receipt, a payload mismatch or
        // an unknown id must never reach the apply — the project must
        // not move for a transaction that is not the open one.
        self.control.preflight_record_finish(prepared)?;
        let applied = match prepared.op.clone() {
            Some(op) => match self.apply_internal(op, prepared.expected_revision) {
                Ok(rev) => Some(rev),
                Err(reason) => {
                    self.control.note_commit_rejected(prepared.id);
                    return Err(RecordCommitError::Rejected { reason });
                }
            },
            None => None,
        };
        self.control.finish_record_commit(prepared, applied)
    }

    /// The user-requested rebase report for the open transaction —
    /// never an automatic merge.
    ///
    /// # Errors
    /// [`RecordCommitError::NoTransaction`].
    pub fn control_record_rebase_preview(
        &self,
    ) -> Result<crate::control::RebasePreview, RecordCommitError> {
        self.control
            .rebase_preview(self.controller.live(), self.revision())
    }

    /// Explicit approval of a rebase inspected through
    /// [`ProjectSession::control_record_rebase_preview`]: re-pins the
    /// fixed revision to the live one and returns the transaction to
    /// pending. The write gate stays exactly as sealed.
    ///
    /// # Errors
    /// [`RecordCommitError::NoTransaction`]/`UnknownTransaction`.
    pub fn control_record_approve_rebase(
        &mut self,
        id: RecordingTransactionId,
    ) -> Result<(), RecordCommitError> {
        self.control.approve_rebase(id, self.revision())
    }

    /// Aborts the open recording transaction — nothing is applied.
    pub fn control_record_abort(&mut self) {
        self.control.record_abort();
    }

    /// The live project.
    #[must_use]
    pub fn project(&self) -> &Project {
        self.controller.live()
    }

    /// The live revision.
    #[must_use]
    pub fn revision(&self) -> ProjectRevision {
        self.controller.live_revision()
    }

    /// Applies `op` through the controller (validation + revision
    /// check + atomic apply), rebuilds the engine stub's graph, then
    /// notifies the control session: a successful apply is a foreign
    /// project change that seals an open recording transaction's
    /// write gate.
    ///
    /// Generic over the operation type: the workspace command layer
    /// produces `ProjectOp`, and any other domain op that satisfies
    /// the project's `Operation` contract — `CompOp`, `SynvelisOp` —
    /// reaches the controller through the same surface.
    ///
    /// # Errors
    /// [`Error::StaleRevision`]/validation errors from the controller —
    /// project unchanged.
    pub fn apply<O: Operation>(
        &mut self,
        op: O,
        expected_revision: ProjectRevision,
    ) -> Result<ProjectRevision, Error> {
        let revision = self.apply_internal(op, expected_revision)?;
        self.control.note_project_changed();
        Ok(revision)
    }

    /// The controller apply + engine rebuild without the
    /// foreign-change notification — used internally by the
    /// recording-commit path, whose transaction consumes its own
    /// revision advance and must never seal itself.
    fn apply_internal<O: Operation>(
        &mut self,
        op: O,
        expected_revision: ProjectRevision,
    ) -> Result<ProjectRevision, Error> {
        let revision = self.controller.apply(Command {
            expected_revision,
            op,
        })?;
        self.engine.on_project_revision(revision);
        Ok(revision)
    }

    /// Sets transport rolling/stopped. Never touches the project.
    pub fn set_playing(&mut self, playing: bool) {
        self.transport.playing = playing;
    }

    /// Undoes the last project command and rebuilds the engine stub's
    /// graph — the project changed, so the graph must too. A
    /// successful undo is a foreign project change: it seals an open
    /// recording transaction's write gate.
    ///
    /// # Errors
    /// Propagates controller errors (none in practice).
    pub fn undo(&mut self) -> Result<bool, Error> {
        if self.controller.undo()? {
            self.engine
                .on_project_revision(self.controller.live_revision());
            self.control.note_project_changed();
            return Ok(true);
        }
        Ok(false)
    }

    /// Redoes the last undone command and rebuilds the engine stub's
    /// graph — a foreign project change that seals an open recording
    /// transaction's write gate.
    ///
    /// # Errors
    /// Propagates controller errors (none in practice).
    pub fn redo(&mut self) -> Result<bool, Error> {
        if self.controller.redo()? {
            self.engine
                .on_project_revision(self.controller.live_revision());
            self.control.note_project_changed();
            return Ok(true);
        }
        Ok(false)
    }
}
