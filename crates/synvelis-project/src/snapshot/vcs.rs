//! Provider-neutral read port for VCS-staged and VCS-committed bytes.
//!
//! VCS-01 defines only the *boundary*: a [`VcsBytesPort`] hands the
//! resolver opaque byte reads plus an index-generation token so staged
//! captures can detect mid-capture index mutation. The concrete Git
//! provider implementation is Issue #94 (VCS-03) and is deliberately
//! not part of this crate slice — tests use in-memory fakes.

use super::types::IndexGeneration;

/// A resolved VCS locator — the concrete repository-relative position a
/// [`VcsBytesPort`] read is pinned to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VcsLocator {
    /// A staged (index) read pinned to an index generation.
    Staged {
        /// Provider-side repository identity.
        repository_id: String,
        /// Repository-relative project file path.
        project_path: String,
        /// Index generation the read is pinned to.
        index_generation: IndexGeneration,
    },
    /// A committed read pinned to a fixed revision.
    Committed {
        /// Provider-side repository identity.
        repository_id: String,
        /// Fixed commit/revision.
        fixed_revision: String,
        /// Repository-relative project file path.
        project_path: String,
    },
}

/// Result of a single file read through a [`VcsBytesPort`].
///
/// `Unknown` is distinct from `Absent`: the provider could not
/// determine presence (e.g. partial index state it refuses to guess
/// at), which the resolver surfaces as degraded/unknown rather than
/// missing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VcsFileRead {
    /// Exact bytes were read.
    Present(Vec<u8>),
    /// The provider affirmatively reports the path absent.
    Absent,
    /// Presence could not be determined.
    Unknown,
}

/// Result of a content-addressed blob read through a [`VcsBytesPort`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlobRead {
    /// Exact bytes were read.
    Present(Vec<u8>),
    /// The provider affirmatively reports the blob absent (e.g. a
    /// partially staged asset set).
    Absent,
    /// Presence could not be determined.
    Unknown,
}

/// Errors a [`VcsBytesPort`] may report. Provider-specific detail stays
/// inside `detail`; the resolver never inspects it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VcsPortError {
    /// The repository/locator does not exist.
    NotFound {
        /// What was not found.
        detail: String,
    },
    /// The provider could not complete the read (transport, lock,
    /// unsupported capability, ...).
    Unavailable {
        /// Provider-reported detail.
        detail: String,
    },
}

/// Read-only boundary between the snapshot resolver and a VCS provider.
///
/// Implementations must be side-effect free with respect to the
/// repository: `index_generation` reads must not mutate the index, and
/// file/blob reads must not touch the worktree. The trait has no write
/// methods by construction.
pub trait VcsBytesPort {
    /// Current staged-set generation for `repository_id`.
    ///
    /// Two calls returning equal [`IndexGeneration`] values assert the
    /// staged set did not observably change between them.
    ///
    /// # Errors
    /// [`VcsPortError::NotFound`] for an unknown repository,
    /// [`VcsPortError::Unavailable`] when the generation cannot be read.
    fn index_generation(&self, repository_id: &str) -> Result<IndexGeneration, VcsPortError>;

    /// Reads `path` as pinned by `locator`.
    ///
    /// # Errors
    /// [`VcsPortError::NotFound`] when the repository/revision is
    /// unknown, [`VcsPortError::Unavailable`] on read failure.
    fn read_file(&self, locator: &VcsLocator, path: &str) -> Result<VcsFileRead, VcsPortError>;

    /// Reads the content-addressed blob `digest` as pinned by `locator`.
    /// `digest` is the project's opaque asset digest string — the port
    /// decides how it maps onto provider object IDs; it is *not*
    /// interpreted as a Git SHA here.
    ///
    /// # Errors
    /// [`VcsPortError::NotFound`] when the repository/revision is
    /// unknown, [`VcsPortError::Unavailable`] on read failure.
    fn read_blob(&self, locator: &VcsLocator, digest: &str) -> Result<BlobRead, VcsPortError>;
}

/// Optional content probe used by non-VCS resolutions (Live/Saved/
/// Recovery) to classify asset availability.
///
/// `None` (no probe supplied) classifies every asset
/// [`super::types::ContentStatus::Unknown`] — never silently `Present`.
pub trait BlobProbe {
    /// Probes `digest` for availability.
    fn probe(&self, digest: &str) -> BlobRead;
}
