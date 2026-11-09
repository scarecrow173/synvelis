//! Public value types for the immutable snapshot resolver.
//!
//! Every type here is plain immutable data: locators name *what* to
//! resolve, identities pin *which exact bytes* were captured, and the
//! snapshot/result types carry the read-only projection plus explicit
//! completeness/validation state. Nothing in this module exposes mutable
//! access to a captured [`Project`].

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{
    controller::ProjectRevision,
    id::{AssetId, ProcessorId, ProjectId},
    project::Project,
    store::SCHEMA_VERSION,
};

use super::digest::ContentDigest;

/// The source family a snapshot was resolved from.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotOrigin {
    /// The controller's live project, cloned at capture time.
    Live,
    /// A `ProjectStore` snapshot reached through HEAD (or a pinned ref).
    Saved,
    /// A checkpoint snapshot in a recovery-rooted `ProjectStore`.
    Recovery,
    /// Project bytes staged in a VCS index (provider-neutral port).
    VcsStaged,
    /// Project bytes committed at a fixed VCS revision.
    VcsCommitted,
}

/// A caller-supplied pointer to the snapshot to resolve.
///
/// Mutable references (`HEAD`, live revision, index generation) are
/// *expectations* or *optional pins*: the resolver re-reads them inside a
/// capture window and reports [`super::SnapshotError::HeadConflict`] /
/// [`super::SnapshotError::IndexChanged`] /
/// [`super::SnapshotError::StaleLiveRevision`] when reality moved. Once a
/// snapshot is built, its [`FixedSnapshotLocator`] records exactly what
/// was captured.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SnapshotLocator {
    /// The controller's live project at `expected_live_revision`.
    Live {
        /// Project the caller believes is live.
        project_id: ProjectId,
        /// Revision the caller captured the intent at.
        expected_live_revision: ProjectRevision,
    },
    /// The saved store's HEAD, optionally pinned to `head_ref`.
    Saved {
        /// Project the caller believes is saved.
        project_id: ProjectId,
        /// Optional expected HEAD snapshot name; mismatch is a conflict.
        head_ref: Option<String>,
    },
    /// A named checkpoint in a recovery-rooted store.
    Recovery {
        /// Project the caller believes the checkpoint belongs to.
        project_id: ProjectId,
        /// Snapshot/checkpoint name inside the recovery store.
        checkpoint: String,
    },
    /// Project file staged in a VCS index.
    VcsStaged {
        /// Provider-side repository identity.
        repository_id: String,
        /// Repository-relative project file path.
        project_path: String,
        /// Optional expected index generation; mismatch is a conflict.
        index_generation: Option<IndexGeneration>,
    },
    /// Project file committed at a fixed revision.
    VcsCommitted {
        /// Provider-side repository identity.
        repository_id: String,
        /// Fixed commit/revision the caller pinned.
        fixed_revision: String,
        /// Repository-relative project file path.
        project_path: String,
    },
}

/// An opaque provider-side index/staging generation marker.
///
/// The resolver treats it as an opaque equality token: equal values mean
/// "the staged set did not observably change", different values mean it
/// did. It is *not* a content digest and never substitutes for one.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IndexGeneration(pub String);

/// A fully resolved locator — every mutable reference fixed to the value
/// observed inside the capture window.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum FixedSnapshotLocator {
    /// Live capture pinned to the observed revision.
    Live {
        /// Project identity.
        project_id: ProjectId,
        /// Live revision at capture time.
        live_revision: ProjectRevision,
    },
    /// Saved capture pinned to the observed HEAD snapshot name.
    Saved {
        /// Project identity.
        project_id: ProjectId,
        /// HEAD snapshot name observed on both window reads.
        head: String,
    },
    /// Recovery capture pinned to the checkpoint name.
    Recovery {
        /// Project identity.
        project_id: ProjectId,
        /// Checkpoint name read from the recovery store.
        checkpoint: String,
    },
    /// Staged capture pinned to the stable index generation.
    VcsStaged {
        /// Provider-side repository identity.
        repository_id: String,
        /// Repository-relative project file path.
        project_path: String,
        /// Index generation stable across the capture window.
        index_generation: IndexGeneration,
    },
    /// Committed capture pinned to the fixed revision.
    VcsCommitted {
        /// Provider-side repository identity.
        repository_id: String,
        /// Fixed commit/revision read.
        fixed_revision: String,
        /// Repository-relative project file path.
        project_path: String,
    },
}

/// Content-addressed identity of one captured snapshot.
///
/// Two snapshots are the *same content* only when `digest` matches; every
/// other field disambiguates provenance. Equality must never be decided
/// on `digest` alone (SHA-256 collision resistance is a cryptographic
/// assumption, not an identity proof) nor on a human revision label
/// alone (different projects/VCS revisions can share `r1.json`-style
/// names).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotIdentity {
    /// Project the captured bytes belong to.
    pub project_id: ProjectId,
    /// Source family the bytes came from.
    pub source: SnapshotOrigin,
    /// Fully resolved capture locator.
    pub locator: FixedSnapshotLocator,
    /// SHA-256 of the exact captured envelope bytes.
    pub digest: ContentDigest,
    /// Schema version recorded *in the source bytes* (pre-migration).
    pub schema_version: u32,
    /// Schema version of the read-only projection (post-migration).
    pub projection_version: u32,
    /// Resolver-local monotonically increasing capture counter.
    pub capture_generation: u64,
}

/// Read-only projection of a captured project at the current schema.
///
/// The [`Project`] is shared through `Arc` and only exposed by shared
/// reference — there is deliberately no `&mut` path, so a consumer cannot
/// mutate captured state through the projection.
#[derive(Clone, Debug)]
pub struct ReadOnlyProjectProjection {
    source_schema_version: u32,
    projection_version: u32,
    project: Arc<Project>,
}

impl ReadOnlyProjectProjection {
    /// Wraps an already-migrated project.
    #[must_use]
    pub fn new(source_schema_version: u32, project: Project) -> Self {
        Self {
            source_schema_version,
            projection_version: SCHEMA_VERSION,
            project: Arc::new(project),
        }
    }

    /// Schema version recorded in the captured source bytes.
    #[must_use]
    pub const fn source_schema_version(&self) -> u32 {
        self.source_schema_version
    }

    /// Schema version the projection conforms to.
    #[must_use]
    pub const fn projection_version(&self) -> u32 {
        self.projection_version
    }

    /// The projected project (shared, read-only).
    #[must_use]
    pub fn project(&self) -> &Project {
        &self.project
    }
}

/// Whether a probed content region is available to the resolver.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContentStatus {
    /// Content bytes were read and verified present.
    Present,
    /// The provider/store affirmatively reports the content absent.
    Missing,
    /// Presence could not be determined (no probe, probe failure, or
    /// provider-reported unknown).
    Unknown,
}

/// One row of the asset-availability manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetResolution {
    /// Asset identity inside the project.
    pub asset_id: AssetId,
    /// Content digest the asset references (opaque to the resolver).
    pub digest: String,
    /// Probe outcome for the digest.
    pub status: ContentStatus,
}

/// Availability manifest for every asset the project references.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetAvailabilityManifest {
    /// One entry per project asset, in `AssetId` order.
    pub entries: Vec<AssetResolution>,
}

/// Verification state of a plugin's opaque state.
///
/// VCS-01 performs no plugin-host probing, so `Resolved` bindings can
/// only ever report [`OpaqueStateStatus::Unknown`] here; `Unresolved`
/// bindings report [`OpaqueStateStatus::Missing`]. A future probe port
/// may promote entries to `Known`/`Missing` without changing this type.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpaqueStateStatus {
    /// Opaque state verified against a live plugin host.
    Known,
    /// Opaque state could not be verified (no probe available).
    Unknown,
    /// The plugin/binding itself is unresolvable.
    Missing,
}

/// Opaque-state reference for one routing processor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpaqueStateRef {
    /// Processor identity inside the routing model.
    pub processor_id: ProcessorId,
    /// Last known plugin descriptor, when the binding records one.
    pub descriptor: Option<String>,
    /// Opaque-state verification status.
    pub status: OpaqueStateStatus,
}

/// Whole-snapshot completeness classification.
///
/// `Partial` means at least one referenced region is `Missing` or
/// `Unknown`; such a snapshot is still returned (its known regions are
/// comparable) but must never be treated as wholly `Equal` to another
/// snapshot on the strength of the verified regions alone.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotCompleteness {
    /// Every probed region reported `Present` and every opaque state is
    /// `Known`.
    Complete,
    /// At least one region is `Missing` or `Unknown`.
    Partial,
}

/// Validation outcome of the captured snapshot.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotValidation {
    /// Structure, references, watermark, and all probed regions verified.
    Valid,
    /// Structure is valid but the snapshot carries `Missing`/`Unknown`
    /// regions recorded in the manifest and opaque-state references.
    Degraded,
}

/// The immutable result of one snapshot resolution.
#[derive(Clone, Debug)]
pub struct ImmutableSnapshot {
    /// Content/provenance identity of the capture.
    pub identity: SnapshotIdentity,
    /// Source family (mirrors `identity.source` for convenience).
    pub origin: SnapshotOrigin,
    /// Read-only projection at the current schema.
    pub projection: ReadOnlyProjectProjection,
    /// Structural validation outcome.
    pub validation: SnapshotValidation,
    /// Per-asset availability records.
    pub assets: AssetAvailabilityManifest,
    /// Per-processor opaque-state records.
    pub opaque_state: Vec<OpaqueStateRef>,
    /// Whole-snapshot completeness.
    pub completeness: SnapshotCompleteness,
}
