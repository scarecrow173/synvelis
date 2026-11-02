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
    Command, Error, ParameterTargetId, Project, ProjectController, ProjectOp, ProjectRevision,
};

use crate::control::{ControlSession, GestureEnd, RecordingOutcome};

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
        let end = self.control.gesture_end(target)?;
        if let GestureEnd::BaseEdit { value } = end {
            let rev = self.revision();
            self.apply(ProjectOp::SetParameterBase { target, value }, rev)?;
        }
        Ok(end)
    }

    /// Commits the open recording transaction as **one undo step**:
    /// every lane segment becomes an explicit `Replace` region write
    /// inside a single `Composite` apply.
    ///
    /// # Errors
    /// [`Error::Validation`] on no open transaction; controller errors
    /// on a rejected commit (the transaction is already consumed).
    pub fn control_record_commit(&mut self) -> Result<RecordingOutcome, Error> {
        let (was_sealed, ops) = self.control.build_commit_ops()?;
        if ops.is_empty() {
            return Ok(RecordingOutcome {
                applied: false,
                revision: None,
                was_sealed,
            });
        }
        let rev = self.revision();
        let revision = self.apply(ProjectOp::Composite { ops }, rev)?;
        Ok(RecordingOutcome {
            applied: true,
            revision: Some(revision),
            was_sealed,
        })
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
    /// check + atomic apply), then rebuilds the engine stub's graph.
    ///
    /// # Errors
    /// [`Error::StaleRevision`]/validation errors from the controller —
    /// project unchanged.
    pub fn apply(
        &mut self,
        op: ProjectOp,
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
    /// graph — the project changed, so the graph must too.
    ///
    /// # Errors
    /// Propagates controller errors (none in practice).
    pub fn undo(&mut self) -> Result<bool, Error> {
        if self.controller.undo()? {
            self.engine
                .on_project_revision(self.controller.live_revision());
            return Ok(true);
        }
        Ok(false)
    }

    /// Redoes the last undone command and rebuilds the engine stub's
    /// graph.
    ///
    /// # Errors
    /// Propagates controller errors (none in practice).
    pub fn redo(&mut self) -> Result<bool, Error> {
        if self.controller.redo()? {
            self.engine
                .on_project_revision(self.controller.live_revision());
            return Ok(true);
        }
        Ok(false)
    }
}
