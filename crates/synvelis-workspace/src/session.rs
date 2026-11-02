//! Project session — the single project source of truth plus the
//! transport stub and the engine stub proving RT/graph separation
//! (WS-A scope 5).
//!
//! [`ProjectSession`] owns the sole `ProjectController`. Views read
//! projections from it; every mutation flows through routed commands.
//! A successful mutation advances the live revision and the engine
//! stub rebuilds its (fake) graph — dock/layout/focus operations never
//! touch either.

use synvelis_project::{Command, Error, Project, ProjectController, ProjectOp, ProjectRevision};

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
}

impl ProjectSession {
    /// Wraps `controller` — the only project source of truth.
    #[must_use]
    pub fn new(controller: ProjectController) -> Self {
        Self {
            controller,
            transport: TransportState::default(),
            engine: EngineStub::default(),
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
}
