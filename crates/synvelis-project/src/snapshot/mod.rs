//! VCS-independent immutable snapshot resolver (Issue #92 / VCS-01).
//!
//! [`SnapshotResolver`] captures project state from five source families
//! — Live, Saved HEAD, Recovery checkpoints, VCS-staged bytes, and
//! VCS-committed bytes — and projects each capture into an
//! [`ImmutableSnapshot`]: exact-byte digest identity, a read-only
//! schema-current [`Project`] projection, and explicit asset/plugin
//! completeness state.
//!
//! The resolver is strictly read-only. It never calls
//! [`crate::ProjectController::load`] (which replaces live state and
//! clears undo/redo), never writes through [`crate::ProjectStore`], and
//! never mutates a [`VcsBytesPort`]. Race windows (HEAD moves, index
//! generation changes, same-name byte refreshes) are detected inside a
//! bounded capture window and reported as explicit
//! [`SnapshotError`]s rather than false equality.

mod digest;
mod types;
mod vcs;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::{
    controller::{ProjectController, ProjectRevision},
    error::Error,
    id::ProjectId,
    migration,
    project::Project,
    routing::ProcessorBinding,
    store::{ProjectStore, SCHEMA_VERSION, SnapshotEnvelope},
};

pub use digest::ContentDigest;
pub use types::{
    AssetAvailabilityManifest, AssetResolution, ContentStatus, FixedSnapshotLocator,
    ImmutableSnapshot, IndexGeneration, OpaqueStateRef, OpaqueStateStatus,
    ReadOnlyProjectProjection, SnapshotCompleteness, SnapshotIdentity, SnapshotLocator,
    SnapshotOrigin, SnapshotValidation,
};
pub use vcs::{BlobProbe, BlobRead, VcsBytesPort, VcsFileRead, VcsLocator, VcsPortError};

/// Cooperative cancellation token for snapshot resolution.
///
/// The resolver checks the token at phase boundaries (before/after each
/// store or port read window, before returning). Cancelling produces
/// [`SnapshotError::Cancelled`]; in-flight single reads are not
/// interruptible, which is acceptable because every read is bounded by
/// [`SnapshotLimits`].
#[derive(Clone, Default)]
pub struct SnapshotCancel {
    cancelled: Arc<AtomicBool>,
}

impl SnapshotCancel {
    /// A fresh, non-cancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks the token cancelled. Idempotent; visible to every clone.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// `true` once [`Self::cancel`] has run on any clone.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

/// Resource bounds applied to every resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotLimits {
    /// Maximum accepted snapshot-envelope byte length. Larger payloads
    /// fail with [`SnapshotError::BudgetExceeded`].
    pub max_snapshot_bytes: usize,
}

impl Default for SnapshotLimits {
    fn default() -> Self {
        Self {
            max_snapshot_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Errors a snapshot resolution can report.
///
/// Every variant is an explicit failure or an explicit degraded record
/// upstream of this error; nothing here is a silent fallback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotError {
    /// The [`SnapshotCancel`] token was cancelled before completion.
    Cancelled,
    /// Captured bytes exceeded [`SnapshotLimits::max_snapshot_bytes`].
    BudgetExceeded {
        /// Configured limit.
        limit: usize,
        /// Observed byte length.
        actual: usize,
    },
    /// Live revision moved between the caller's intent and capture.
    StaleLiveRevision {
        /// Revision the caller expected.
        expected: ProjectRevision,
        /// Revision actually live.
        actual: ProjectRevision,
    },
    /// HEAD (or a pinned `head_ref`) disagreed inside the capture
    /// window — the saved pointer moved mid-read or never matched.
    HeadConflict {
        /// Expected HEAD name (`head_ref` or first window read).
        expected: Option<String>,
        /// HEAD name actually observed.
        actual: Option<String>,
    },
    /// Bytes under a stable name changed inside the capture window
    /// (store collision refresh or provider mutation).
    SourceChanged {
        /// What changed.
        detail: String,
    },
    /// The VCS index generation changed inside the capture window, or
    /// disagreed with the locator's pinned expectation.
    IndexChanged {
        /// Generation observed first / expected.
        before: IndexGeneration,
        /// Generation observed last / actual.
        after: IndexGeneration,
    },
    /// A staged VCS capture references content absent from the index —
    /// a partially staged project/asset set, never reported as equal.
    PartialStage {
        /// Digests/paths found absent from the staged set.
        missing: Vec<String>,
    },
    /// The referenced snapshot/checkpoint/revision does not exist.
    NotFound {
        /// What was not found.
        detail: String,
    },
    /// The locator variant does not match the resolve entry point.
    InvalidLocator(String),
    /// The envelope's schema version is outside the migration ladder
    /// (unknown, too old, or newer than this build).
    UnsupportedSchema {
        /// Version found in the source bytes.
        found: u32,
    },
    /// Bytes parsed but failed project validation (references,
    /// watermark, project-id mismatch, migration projection errors).
    InvalidSnapshot(String),
    /// A `ProjectStore` read failed.
    Store(Error),
    /// A `VcsBytesPort` call failed.
    Vcs(VcsPortError),
    /// Serializing the live capture failed.
    Serialize(String),
    /// A VCS source read could not determine presence
    /// (`VcsFileRead::Unknown` / `BlobRead::Unknown` at a hard
    /// requirement boundary).
    SourceUnavailable {
        /// What could not be determined.
        detail: String,
    },
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "snapshot resolution cancelled"),
            Self::BudgetExceeded { limit, actual } => {
                write!(f, "snapshot bytes {actual} exceed limit {limit}")
            }
            Self::StaleLiveRevision { expected, actual } => {
                write!(
                    f,
                    "stale live revision: expected {expected:?}, live {actual:?}"
                )
            }
            Self::HeadConflict { expected, actual } => {
                write!(f, "HEAD conflict: expected {expected:?}, found {actual:?}")
            }
            Self::SourceChanged { detail } => write!(f, "source changed during capture: {detail}"),
            Self::IndexChanged { before, after } => {
                write!(f, "index generation changed: {before:?} -> {after:?}")
            }
            Self::PartialStage { missing } => {
                write!(f, "partially staged snapshot, missing: {missing:?}")
            }
            Self::NotFound { detail } => write!(f, "snapshot source not found: {detail}"),
            Self::InvalidLocator(detail) => write!(f, "invalid locator: {detail}"),
            Self::UnsupportedSchema { found } => {
                write!(f, "unsupported snapshot schema version: {found}")
            }
            Self::InvalidSnapshot(detail) => write!(f, "invalid snapshot: {detail}"),
            Self::Store(err) => write!(f, "store error: {err}"),
            Self::Vcs(err) => write!(f, "vcs port error: {err:?}"),
            Self::Serialize(detail) => write!(f, "snapshot serialize failed: {detail}"),
            Self::SourceUnavailable { detail } => {
                write!(f, "snapshot source availability unknown: {detail}")
            }
        }
    }
}

impl std::error::Error for SnapshotError {}

impl From<VcsPortError> for SnapshotError {
    fn from(err: VcsPortError) -> Self {
        Self::Vcs(err)
    }
}

/// The immutable snapshot resolver.
///
/// Holds only configuration (limits) and a capture-generation counter —
/// no project, store, or provider state. All resolve methods take their
/// sources by shared reference and are safe to call concurrently.
pub struct SnapshotResolver {
    limits: SnapshotLimits,
    next_generation: AtomicU64,
}

impl Default for SnapshotResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotResolver {
    /// A resolver with default [`SnapshotLimits`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            limits: SnapshotLimits::default(),
            next_generation: AtomicU64::new(1),
        }
    }

    /// A resolver with explicit limits.
    #[must_use]
    pub fn with_limits(limits: SnapshotLimits) -> Self {
        Self {
            limits,
            next_generation: AtomicU64::new(1),
        }
    }

    /// The configured limits.
    #[must_use]
    pub const fn limits(&self) -> SnapshotLimits {
        self.limits
    }

    /// Captures the controller's live project without mutating it.
    ///
    /// The live project is cloned at capture time; the clone's
    /// `id_watermark` is stamped with the generator watermark exactly as
    /// [`ProjectController::save`] would stamp it, but the controller —
    /// including live state, revision, undo/redo, and saved metadata —
    /// is untouched. Subsequent edits, undo, transport, or engine
    /// activity cannot change the returned snapshot.
    ///
    /// # Errors
    /// [`SnapshotError::StaleLiveRevision`] when `expected_revision`
    /// disagrees with the live revision, [`SnapshotError::Cancelled`],
    /// [`SnapshotError::BudgetExceeded`], or serialization/validation
    /// failures.
    pub fn capture_live(
        &self,
        controller: &ProjectController,
        expected_revision: ProjectRevision,
        cancel: &SnapshotCancel,
        blobs: Option<&dyn BlobProbe>,
    ) -> Result<ImmutableSnapshot, SnapshotError> {
        Self::check_cancel(cancel)?;
        let actual = controller.live_revision();
        if actual != expected_revision {
            return Err(SnapshotError::StaleLiveRevision {
                expected: expected_revision,
                actual,
            });
        }
        let mut project = controller.live().clone();
        project.id_watermark = controller.snapshot_id_watermark();
        let envelope = SnapshotEnvelope {
            schema_version: SCHEMA_VERSION,
            project_id: project.project_id,
            revision: expected_revision,
            project,
        };
        let bytes = serde_json::to_vec(&envelope)
            .map_err(|err| SnapshotError::Serialize(err.to_string()))?;
        Self::check_budget(bytes.len(), self.limits)?;
        Self::check_cancel(cancel)?;
        let (project, schema_version) = Self::parse_project(&bytes, &envelope.project_id, cancel)?;
        let locator = FixedSnapshotLocator::Live {
            project_id: envelope.project_id,
            live_revision: expected_revision,
        };
        self.assemble(
            SnapshotOrigin::Live,
            locator,
            &bytes,
            project,
            schema_version,
            blobs,
            cancel,
        )
    }

    /// Resolves a saved snapshot through a `ProjectStore` HEAD window.
    ///
    /// Reads HEAD → bytes → HEAD → bytes; any HEAD movement is
    /// [`SnapshotError::HeadConflict`], any same-name byte change is
    /// [`SnapshotError::SourceChanged`], and a `head_ref` pin that does
    /// not match the observed HEAD is also `HeadConflict`. The store is
    /// only ever read through `&dyn ProjectStore`.
    ///
    /// # Errors
    /// Locator/variant mismatches, store failures, race conflicts,
    /// cancellation, budget, or invalid snapshot content.
    pub fn resolve_saved(
        &self,
        store: &dyn ProjectStore,
        locator: &SnapshotLocator,
        cancel: &SnapshotCancel,
        blobs: Option<&dyn BlobProbe>,
    ) -> Result<ImmutableSnapshot, SnapshotError> {
        let SnapshotLocator::Saved {
            project_id,
            head_ref,
        } = locator
        else {
            return Err(SnapshotError::InvalidLocator(format!(
                "resolve_saved requires a saved locator, got {locator:?}"
            )));
        };
        Self::check_cancel(cancel)?;
        let head1 = Self::store_head(store)?;
        let Some(head1) = head1 else {
            return Err(SnapshotError::NotFound {
                detail: "store has no HEAD".into(),
            });
        };
        if let Some(expected) = head_ref
            && head1 != *expected
        {
            return Err(SnapshotError::HeadConflict {
                expected: Some(expected.clone()),
                actual: Some(head1),
            });
        }
        let bytes1 = Self::store_read(store, &head1)?;
        Self::check_cancel(cancel)?;
        let head2 = Self::store_head(store)?;
        if head2.as_deref() != Some(head1.as_str()) {
            return Err(SnapshotError::HeadConflict {
                expected: Some(head1.clone()),
                actual: head2,
            });
        }
        let bytes2 = Self::store_read(store, &head1)?;
        if bytes1 != bytes2 {
            return Err(SnapshotError::SourceChanged {
                detail: format!("snapshot {head1} bytes changed during capture"),
            });
        }
        Self::check_budget(bytes1.len(), self.limits)?;
        Self::check_cancel(cancel)?;
        let (project, schema_version) = Self::parse_project(&bytes1, project_id, cancel)?;
        let locator = FixedSnapshotLocator::Saved {
            project_id: *project_id,
            head: head1,
        };
        self.assemble(
            SnapshotOrigin::Saved,
            locator,
            &bytes1,
            project,
            schema_version,
            blobs,
            cancel,
        )
    }

    /// Resolves the store's current HEAD for `project_id` — shorthand for
    /// [`Self::resolve_saved`] with `head_ref: None`.
    ///
    /// # Errors
    /// Same as [`Self::resolve_saved`].
    pub fn resolve_saved_head(
        &self,
        store: &dyn ProjectStore,
        project_id: ProjectId,
        cancel: &SnapshotCancel,
        blobs: Option<&dyn BlobProbe>,
    ) -> Result<ImmutableSnapshot, SnapshotError> {
        self.resolve_saved(
            store,
            &SnapshotLocator::Saved {
                project_id,
                head_ref: None,
            },
            cancel,
            blobs,
        )
    }

    /// Resolves a named checkpoint from a recovery-rooted store.
    ///
    /// Recovery snapshots live under their own store root/HEAD and are
    /// read by checkpoint name only — this method never consults or
    /// modifies a saved HEAD. The checkpoint bytes are read twice and
    /// compared so a mid-capture refresh is [`SnapshotError::SourceChanged`].
    ///
    /// # Errors
    /// Locator/variant mismatches, store failures, byte changes,
    /// cancellation, budget, or invalid snapshot content.
    pub fn resolve_recovery(
        &self,
        store: &dyn ProjectStore,
        locator: &SnapshotLocator,
        cancel: &SnapshotCancel,
        blobs: Option<&dyn BlobProbe>,
    ) -> Result<ImmutableSnapshot, SnapshotError> {
        let SnapshotLocator::Recovery {
            project_id,
            checkpoint,
        } = locator
        else {
            return Err(SnapshotError::InvalidLocator(format!(
                "resolve_recovery requires a recovery locator, got {locator:?}"
            )));
        };
        Self::check_cancel(cancel)?;
        let bytes1 = Self::store_read(store, checkpoint)?;
        Self::check_cancel(cancel)?;
        let bytes2 = Self::store_read(store, checkpoint)?;
        if bytes1 != bytes2 {
            return Err(SnapshotError::SourceChanged {
                detail: format!("recovery checkpoint {checkpoint} bytes changed during capture"),
            });
        }
        Self::check_budget(bytes1.len(), self.limits)?;
        Self::check_cancel(cancel)?;
        let (project, schema_version) = Self::parse_project(&bytes1, project_id, cancel)?;
        let locator = FixedSnapshotLocator::Recovery {
            project_id: *project_id,
            checkpoint: checkpoint.clone(),
        };
        self.assemble(
            SnapshotOrigin::Recovery,
            locator,
            &bytes1,
            project,
            schema_version,
            blobs,
            cancel,
        )
    }

    /// Resolves VCS-staged or VCS-committed project bytes through a
    /// [`VcsBytesPort`].
    ///
    /// Staged captures bracket the read window with
    /// [`VcsBytesPort::index_generation`] calls; any drift is
    /// [`SnapshotError::IndexChanged`], and content the index does not
    /// carry is [`SnapshotError::PartialStage`] — never silent equality.
    /// Committed captures pin `fixed_revision` and treat absent blobs as
    /// degraded `Missing` records instead of failures.
    ///
    /// # Errors
    /// Locator/variant mismatches, port failures, race conflicts,
    /// cancellation, budget, partial staging, or invalid content.
    pub fn resolve_vcs(
        &self,
        port: &dyn VcsBytesPort,
        locator: &SnapshotLocator,
        cancel: &SnapshotCancel,
    ) -> Result<ImmutableSnapshot, SnapshotError> {
        Self::check_cancel(cancel)?;
        match locator {
            SnapshotLocator::VcsStaged {
                repository_id,
                project_path,
                index_generation,
            } => {
                let gen_before = port.index_generation(repository_id)?;
                if let Some(expected) = index_generation
                    && *expected != gen_before
                {
                    return Err(SnapshotError::IndexChanged {
                        before: expected.clone(),
                        after: gen_before,
                    });
                }
                let vcs_locator = VcsLocator::Staged {
                    repository_id: repository_id.clone(),
                    project_path: project_path.clone(),
                    index_generation: gen_before.clone(),
                };
                let bytes = Self::read_vcs_file(port, &vcs_locator, project_path, true)?;
                Self::check_budget(bytes.len(), self.limits)?;
                Self::check_cancel(cancel)?;
                // project_id is only known after parsing; VCS locators do
                // not carry it, so validate structure first, then compare
                // against the locator-implied identity below.
                let (project, schema_version, project_id) =
                    Self::parse_project_untyped(&bytes, cancel)?;
                let (manifest, opaque_state, missing) =
                    Self::probe_vcs_assets(port, &vcs_locator, &project, cancel)?;
                let gen_after = port.index_generation(repository_id)?;
                if gen_after != gen_before {
                    return Err(SnapshotError::IndexChanged {
                        before: gen_before,
                        after: gen_after,
                    });
                }
                if !missing.is_empty() {
                    return Err(SnapshotError::PartialStage { missing });
                }
                Self::check_cancel(cancel)?;
                let locator = FixedSnapshotLocator::VcsStaged {
                    repository_id: repository_id.clone(),
                    project_path: project_path.clone(),
                    index_generation: gen_before,
                };
                Ok(self.assemble_with(
                    SnapshotOrigin::VcsStaged,
                    locator,
                    &bytes,
                    project,
                    project_id,
                    schema_version,
                    manifest,
                    opaque_state,
                ))
            }
            SnapshotLocator::VcsCommitted {
                repository_id,
                fixed_revision,
                project_path,
            } => {
                let vcs_locator = VcsLocator::Committed {
                    repository_id: repository_id.clone(),
                    fixed_revision: fixed_revision.clone(),
                    project_path: project_path.clone(),
                };
                let bytes = Self::read_vcs_file(port, &vcs_locator, project_path, false)?;
                Self::check_budget(bytes.len(), self.limits)?;
                Self::check_cancel(cancel)?;
                let (project, schema_version, project_id) =
                    Self::parse_project_untyped(&bytes, cancel)?;
                let (manifest, opaque_state, _missing) =
                    Self::probe_vcs_assets(port, &vcs_locator, &project, cancel)?;
                Self::check_cancel(cancel)?;
                let locator = FixedSnapshotLocator::VcsCommitted {
                    repository_id: repository_id.clone(),
                    fixed_revision: fixed_revision.clone(),
                    project_path: project_path.clone(),
                };
                Ok(self.assemble_with(
                    SnapshotOrigin::VcsCommitted,
                    locator,
                    &bytes,
                    project,
                    project_id,
                    schema_version,
                    manifest,
                    opaque_state,
                ))
            }
            other => Err(SnapshotError::InvalidLocator(format!(
                "resolve_vcs requires a vcs locator, got {other:?}"
            ))),
        }
    }

    fn check_cancel(cancel: &SnapshotCancel) -> Result<(), SnapshotError> {
        if cancel.is_cancelled() {
            Err(SnapshotError::Cancelled)
        } else {
            Ok(())
        }
    }

    fn check_budget(len: usize, limits: SnapshotLimits) -> Result<(), SnapshotError> {
        if len > limits.max_snapshot_bytes {
            Err(SnapshotError::BudgetExceeded {
                limit: limits.max_snapshot_bytes,
                actual: len,
            })
        } else {
            Ok(())
        }
    }

    fn store_head(store: &dyn ProjectStore) -> Result<Option<String>, SnapshotError> {
        store.head().map_err(SnapshotError::Store)
    }

    fn store_read(store: &dyn ProjectStore, name: &str) -> Result<Vec<u8>, SnapshotError> {
        store.read_snapshot(name).map_err(|err| match err {
            Error::NotFound(detail) => SnapshotError::NotFound { detail },
            other => SnapshotError::Store(other),
        })
    }

    /// Parses envelope bytes, migrates to the current schema, and runs
    /// the same validation oracle `ProjectController::load` uses —
    /// without touching any controller.
    fn parse_project(
        bytes: &[u8],
        expected_project_id: &ProjectId,
        cancel: &SnapshotCancel,
    ) -> Result<(Project, u32), SnapshotError> {
        let (project, schema_version, project_id) = Self::parse_project_untyped(bytes, cancel)?;
        if project_id != *expected_project_id {
            return Err(SnapshotError::InvalidSnapshot(format!(
                "envelope project {} does not match locator project {}",
                project_id.to_tagged(),
                expected_project_id.to_tagged()
            )));
        }
        Ok((project, schema_version))
    }

    /// Parses without a caller-supplied project expectation (VCS
    /// locators carry no project id); returns the envelope's identity.
    fn parse_project_untyped(
        bytes: &[u8],
        cancel: &SnapshotCancel,
    ) -> Result<(Project, u32, ProjectId), SnapshotError> {
        Self::check_cancel(cancel)?;
        let envelope: SnapshotEnvelope = serde_json::from_slice(bytes)
            .map_err(|err| SnapshotError::InvalidSnapshot(err.to_string()))?;
        Self::check_cancel(cancel)?;
        let project = if envelope.schema_version == SCHEMA_VERSION {
            envelope.project
        } else {
            migration::migrate_to_current(envelope.schema_version, envelope.project).map_err(
                |err| match err {
                    Error::UnsupportedSchema { found } => {
                        SnapshotError::UnsupportedSchema { found }
                    }
                    other => SnapshotError::InvalidSnapshot(other.to_string()),
                },
            )?
        };
        project
            .validate_references()
            .map_err(|err| SnapshotError::InvalidSnapshot(err.to_string()))?;
        project
            .check_id_watermark()
            .map_err(|err| SnapshotError::InvalidSnapshot(err.to_string()))?;
        Ok((project, envelope.schema_version, envelope.project_id))
    }

    fn read_vcs_file(
        port: &dyn VcsBytesPort,
        locator: &VcsLocator,
        path: &str,
        staged: bool,
    ) -> Result<Vec<u8>, SnapshotError> {
        match port.read_file(locator, path)? {
            VcsFileRead::Present(bytes) => Ok(bytes),
            VcsFileRead::Absent => {
                if staged {
                    Err(SnapshotError::PartialStage {
                        missing: vec![path.to_string()],
                    })
                } else {
                    Err(SnapshotError::NotFound {
                        detail: format!("vcs path {path} absent at fixed revision"),
                    })
                }
            }
            VcsFileRead::Unknown => Err(SnapshotError::SourceUnavailable {
                detail: format!("vcs path {path} presence unknown"),
            }),
        }
    }

    /// Probes every project asset digest through the VCS port, returning
    /// the manifest, opaque-state refs, and the list of digests the
    /// staged index does not carry.
    fn probe_vcs_assets(
        port: &dyn VcsBytesPort,
        locator: &VcsLocator,
        project: &Project,
        cancel: &SnapshotCancel,
    ) -> Result<(AssetAvailabilityManifest, Vec<OpaqueStateRef>, Vec<String>), SnapshotError> {
        let mut entries = Vec::with_capacity(project.assets.len());
        let mut missing = Vec::new();
        for asset in project.assets.values() {
            Self::check_cancel(cancel)?;
            let read = port.read_blob(locator, &asset.digest)?;
            let (status, absent) = match read {
                BlobRead::Present(_) => (ContentStatus::Present, false),
                BlobRead::Absent => (ContentStatus::Missing, true),
                BlobRead::Unknown => (ContentStatus::Unknown, false),
            };
            if absent {
                missing.push(asset.digest.clone());
            }
            entries.push(AssetResolution {
                asset_id: asset.id,
                digest: asset.digest.clone(),
                status,
            });
        }
        let opaque_state = opaque_state_refs(project);
        Ok((AssetAvailabilityManifest { entries }, opaque_state, missing))
    }

    /// Builds the final snapshot from already-parsed content.
    #[allow(clippy::too_many_arguments)]
    fn assemble(
        &self,
        origin: SnapshotOrigin,
        locator: FixedSnapshotLocator,
        bytes: &[u8],
        project: Project,
        schema_version: u32,
        blobs: Option<&dyn BlobProbe>,
        cancel: &SnapshotCancel,
    ) -> Result<ImmutableSnapshot, SnapshotError> {
        let mut entries = Vec::with_capacity(project.assets.len());
        for asset in project.assets.values() {
            Self::check_cancel(cancel)?;
            let read = blobs.map_or(BlobRead::Unknown, |probe| probe.probe(&asset.digest));
            entries.push(AssetResolution {
                asset_id: asset.id,
                digest: asset.digest.clone(),
                status: match read {
                    BlobRead::Present(_) => ContentStatus::Present,
                    BlobRead::Absent => ContentStatus::Missing,
                    BlobRead::Unknown => ContentStatus::Unknown,
                },
            });
        }
        let manifest = AssetAvailabilityManifest { entries };
        Self::check_cancel(cancel)?;
        let opaque_state = opaque_state_refs(&project);
        let project_id = project.project_id;
        Ok(self.assemble_with(
            origin,
            locator,
            bytes,
            project,
            project_id,
            schema_version,
            manifest,
            opaque_state,
        ))
    }

    /// Final assembly shared by every resolve path: digest the exact
    /// captured bytes, stamp identity, and classify completeness.
    #[allow(clippy::too_many_arguments)]
    fn assemble_with(
        &self,
        origin: SnapshotOrigin,
        locator: FixedSnapshotLocator,
        bytes: &[u8],
        project: Project,
        project_id: ProjectId,
        schema_version: u32,
        manifest: AssetAvailabilityManifest,
        opaque_state: Vec<OpaqueStateRef>,
    ) -> ImmutableSnapshot {
        let degraded = manifest
            .entries
            .iter()
            .any(|entry| entry.status != ContentStatus::Present)
            || opaque_state
                .iter()
                .any(|opaque| opaque.status != OpaqueStateStatus::Known);
        let capture_generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        ImmutableSnapshot {
            identity: SnapshotIdentity {
                project_id,
                source: origin,
                locator,
                digest: ContentDigest::sha256(bytes),
                schema_version,
                projection_version: SCHEMA_VERSION,
                capture_generation,
            },
            origin,
            projection: ReadOnlyProjectProjection::new(schema_version, project),
            validation: if degraded {
                SnapshotValidation::Degraded
            } else {
                SnapshotValidation::Valid
            },
            completeness: if degraded {
                SnapshotCompleteness::Partial
            } else {
                SnapshotCompleteness::Complete
            },
            assets: manifest,
            opaque_state,
        }
    }
}

/// Records one [`OpaqueStateRef`] per routing processor. Without a
/// plugin-host probe, `Resolved` bindings are `Unknown` and
/// `Unresolved` bindings are `Missing` — never silently `Known`.
fn opaque_state_refs(project: &Project) -> Vec<OpaqueStateRef> {
    project
        .routing
        .processors
        .values()
        .map(|processor| {
            let (descriptor, status) = match &processor.binding {
                ProcessorBinding::Resolved(plugin) => {
                    (Some(plugin.descriptor.clone()), OpaqueStateStatus::Unknown)
                }
                ProcessorBinding::Unresolved { descriptor } => {
                    (Some(descriptor.clone()), OpaqueStateStatus::Missing)
                }
            };
            OpaqueStateRef {
                processor_id: processor.id,
                descriptor,
                status,
            }
        })
        .collect()
}
