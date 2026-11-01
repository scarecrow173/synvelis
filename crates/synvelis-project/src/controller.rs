//! `ProjectController`: the single writer for project authoring state.
//!
//! Commands carry an `expected_revision`; validation is total before any
//! mutation (applied onto a clone, swapped in on success) so a rejected
//! command leaves the live project untouched. Undo/redo restore content
//! only — the live revision always advances, never rewinds, and saved
//! state is unaffected.

use serde::{Deserialize, Serialize};

use crate::{
    error::Error,
    id::IdGenerator,
    project::Project,
    store::{ProjectStore, SCHEMA_VERSION, SnapshotEnvelope},
};

/// Monotonic live revision (never rewound by undo/redo).
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct ProjectRevision(pub u64);

/// Record of the last successful save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedState {
    /// The live revision at save time.
    pub revision: ProjectRevision,
    /// Snapshot name the store's HEAD points at.
    pub head: String,
}

/// A domain mutation request bound to a revision expectation.
#[derive(Clone, Debug)]
pub struct Command<O> {
    /// Must equal the controller's live revision or the command is stale.
    pub expected_revision: ProjectRevision,
    /// The operation to validate then apply.
    pub op: O,
}

/// A domain operation against the project aggregate.
///
/// `validate` must be side-effect free and total: after it passes,
/// `apply` performs the mutation. Fresh IDs are drawn from the injected
/// generator inside `apply` so validation stays pure.
pub trait Operation {
    /// Checks all targets, references and state legality.
    ///
    /// # Errors
    /// Any [`Error`] variant when the operation is not legal now.
    fn validate(&self, project: &Project) -> Result<(), Error>;

    /// Mutates the project. Called only after `validate` passed, on a
    /// scratch clone — a returned error discards every partial change.
    ///
    /// # Errors
    /// Any [`Error`] variant; callers treat it as "project unchanged".
    fn apply(&self, project: &mut Project, ids: &mut dyn IdGenerator) -> Result<(), Error>;
}

/// The authoritative in-memory project editor. Commands of any
/// [`Operation`] type may be applied; the operation type is a parameter
/// of [`ProjectController::apply`], not of the controller.
pub struct ProjectController {
    live: Project,
    live_revision: u64,
    generator: Box<dyn IdGenerator>,
    undo: Vec<Project>,
    redo: Vec<Project>,
    saved: Option<SavedState>,
}

impl ProjectController {
    /// Creates a controller over `project` at revision 0.
    #[must_use]
    pub fn new(project: Project, generator: Box<dyn IdGenerator>) -> Self {
        Self {
            live: project,
            live_revision: 0,
            generator,
            undo: Vec::new(),
            redo: Vec::new(),
            saved: None,
        }
    }

    /// The live project (read-only).
    #[must_use]
    pub fn live(&self) -> &Project {
        &self.live
    }

    /// The current live revision.
    #[must_use]
    pub fn live_revision(&self) -> ProjectRevision {
        ProjectRevision(self.live_revision)
    }

    /// The last saved state, if any.
    #[must_use]
    pub fn saved(&self) -> Option<&SavedState> {
        self.saved.as_ref()
    }

    /// Applies a command atomically.
    ///
    /// Validation failure or a stale expected revision leaves both the
    /// project and the revision untouched.
    ///
    /// # Errors
    /// [`Error::StaleRevision`] or any validation/apply error.
    #[allow(clippy::needless_pass_by_value)] // commands are consumed by design
    pub fn apply<O: Operation>(&mut self, command: Command<O>) -> Result<ProjectRevision, Error> {
        if command.expected_revision.0 != self.live_revision {
            return Err(Error::StaleRevision {
                expected: command.expected_revision.0,
                actual: self.live_revision,
            });
        }
        command.op.validate(&self.live)?;
        let mut next = self.live.clone();
        command.op.apply(&mut next, self.generator.as_mut())?;
        next.id_watermark = self.generator.peek();
        let previous = std::mem::replace(&mut self.live, next);
        self.undo.push(previous);
        self.redo.clear();
        self.live_revision += 1;
        Ok(ProjectRevision(self.live_revision))
    }

    /// Reverts the most recent command's content. Issues a *new* live
    /// revision and reseeds the ID generator from the restored snapshot's
    /// `id_watermark` (documented re-issue behavior; no two entities with
    /// the same ID coexist on one revision).
    ///
    /// # Errors
    /// Never fails; `Ok(false)` reports an empty history.
    pub fn undo(&mut self) -> Result<bool, Error> {
        let Some(restored) = self.undo.pop() else {
            return Ok(false);
        };
        let current = std::mem::replace(&mut self.live, restored);
        self.redo.push(current);
        self.live_revision += 1;
        self.generator.reseed(self.live.id_watermark);
        Ok(true)
    }

    /// Re-applies the most recently undone content (new live revision).
    ///
    /// # Errors
    /// Never fails; `Ok(false)` reports an empty redo stack.
    pub fn redo(&mut self) -> Result<bool, Error> {
        let Some(restored) = self.redo.pop() else {
            return Ok(false);
        };
        let current = std::mem::replace(&mut self.live, restored);
        self.undo.push(current);
        self.live_revision += 1;
        self.generator.reseed(self.live.id_watermark);
        Ok(true)
    }

    /// Saves the live project through the store (temp → publish → HEAD).
    ///
    /// # Errors
    /// Any store/serialization error; live and prior saved state persist.
    pub fn save(&mut self, store: &mut dyn ProjectStore) -> Result<SavedState, Error> {
        self.live.id_watermark = self.generator.peek();
        let envelope = SnapshotEnvelope {
            schema_version: SCHEMA_VERSION,
            project_id: self.live.project_id,
            revision: ProjectRevision(self.live_revision),
            project: self.live.clone(),
        };
        let bytes = serde_json::to_vec(&envelope)?;
        let head = store.save(
            &envelope.project_id.to_tagged(),
            ProjectRevision(self.live_revision),
            &bytes,
        )?;
        let state = SavedState {
            revision: ProjectRevision(self.live_revision),
            head,
        };
        self.saved = Some(state.clone());
        Ok(state)
    }

    /// Loads the snapshot at the store's HEAD. On success the controller
    /// takes the saved state as its new baseline: `live_revision` becomes
    /// the saved revision and the undo/redo stacks are cleared.
    ///
    /// # Errors
    /// Missing HEAD, parse failure, [`Error::UnsupportedSchema`], or a
    /// failed reference check — in all cases prior state is preserved.
    pub fn load(&mut self, store: &dyn ProjectStore) -> Result<(), Error> {
        let head = store
            .head()?
            .ok_or_else(|| Error::Store("store has no HEAD".into()))?;
        let bytes = store.read_snapshot(&head)?;
        let envelope: SnapshotEnvelope = serde_json::from_slice(&bytes)?;
        if envelope.schema_version != SCHEMA_VERSION {
            return Err(Error::UnsupportedSchema {
                found: envelope.schema_version,
            });
        }
        envelope.project.validate_references()?;
        self.live = envelope.project;
        self.live_revision = envelope.revision.0;
        self.saved = Some(SavedState {
            revision: envelope.revision,
            head,
        });
        self.undo.clear();
        self.redo.clear();
        self.generator.reseed(self.live.id_watermark);
        Ok(())
    }
}
