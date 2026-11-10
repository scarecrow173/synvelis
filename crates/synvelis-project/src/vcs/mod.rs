//! Read-only VCS provider contract (VCS-03, Issue `#94`).
//!
//! [`VcsReadProvider`] is the provider-neutral surface that produces
//! [`FixedVcsRevision`] values, merge-aware [`HistoryNode`] listings,
//! working-tree status, and fixed index snapshots for a detected
//! workspace. It is strictly read-only: no checkout, stage, commit,
//! branch movement, push/pull, index update, or worktree/HEAD
//! mutation. Git-specific concepts (index generations, merge parents,
//! object formats) appear only behind optional
//! [`VcsCapabilities`] — a provider that lacks them reports
//! [`VcsFeature::Unsupported`] and fails the corresponding call with
//! [`VcsError::UnsupportedCapability`] rather than faking behavior.
//!
//! The same provider also implements
//! [`crate::snapshot::VcsBytesPort`] so the VCS-01 snapshot resolver
//! can read staged/committed project bytes through it without knowing
//! it is Git.

mod types;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub use types::{
    FixedVcsRevision, HistoryCompleteness, HistoryNode, HistoryPage, HistoryQuery, IndexEntry,
    IndexEntryMode, ProjectPath, ProjectRelativePath, ProjectScope, RevisionRelations,
    RevisionSelector, StagedSnapshot, VcsCapabilities, VcsFeature, VcsObjectFormat,
    WorkingChangeKind, WorkingStatus, WorkingStatusEntry, WorkspaceInfo,
};

/// Cooperative cancellation token for VCS provider calls.
///
/// Providers check the token at operation boundaries (before/after
/// each backend invocation, between history page steps). Cancelling
/// produces [`VcsError::Cancelled`]. In-flight single backend reads
/// are bounded by the provider's own resource limits.
#[derive(Clone, Default)]
pub struct VcsCancel {
    cancelled: Arc<AtomicBool>,
}

impl VcsCancel {
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

    /// The shared flag, for adapters bridging to another crate's
    /// cancellation token type.
    #[doc(hidden)]
    #[must_use]
    pub fn shared_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }
}

/// Typed failures for [`VcsReadProvider`] operations.
///
/// Variants mirror the VCS-03 contract: missing/degraded states are
/// explicit and are never silently coerced into equality by consumers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VcsError {
    /// The path is not inside a detectable repository.
    NotRepository {
        /// Diagnostic detail (sanitized — never contains secrets).
        detail: String,
    },
    /// A previously fixed revision no longer resolves (e.g. objects
    /// were garbage-collected).
    RevisionGone {
        /// Diagnostic detail.
        detail: String,
    },
    /// The project file does not exist at the requested revision.
    MissingProjectFile {
        /// Diagnostic detail.
        detail: String,
    },
    /// The staged set changed inside an observation window.
    IndexChanged {
        /// Generation observed before the window.
        before: crate::snapshot::IndexGeneration,
        /// Generation observed after the window.
        after: crate::snapshot::IndexGeneration,
    },
    /// Staged content required by the project is absent (partial
    /// stage, untracked asset, missing index entry).
    PartialStage {
        /// Repository-relative paths that could not be staged-read.
        missing: Vec<String>,
    },
    /// A referenced object (blob/tree/commit) is absent from the
    /// object store.
    MissingObject {
        /// Diagnostic detail.
        detail: String,
    },
    /// Content is an LFS pointer whose object bytes are not present
    /// in the local LFS store. Remote fetch is out of scope here.
    MissingLfs {
        /// Diagnostic detail.
        detail: String,
    },
    /// The backend denied the operation.
    PermissionDenied {
        /// Diagnostic detail.
        detail: String,
    },
    /// Authentication material expired or was rejected.
    AuthExpired {
        /// Diagnostic detail.
        detail: String,
    },
    /// The backend is unreachable.
    Offline {
        /// Diagnostic detail.
        detail: String,
    },
    /// The backend asked the caller to slow down.
    RateLimited {
        /// Diagnostic detail.
        detail: String,
    },
    /// The provider does not implement this capability.
    UnsupportedCapability {
        /// Which capability and why.
        detail: String,
    },
    /// A repository-relative path failed validation.
    InvalidProjectPath {
        /// Diagnostic detail.
        detail: String,
    },
    /// A caller- or provider-imposed resource bound was exceeded.
    BudgetExceeded {
        /// Diagnostic detail.
        detail: String,
    },
    /// The operation was cancelled through [`VcsCancel`].
    Cancelled,
    /// The request itself is malformed (bad selector, foreign
    /// revision binding, undecodable cursor, illegal argument
    /// combination).
    InvalidRequest {
        /// Diagnostic detail.
        detail: String,
    },
    /// The repository/workspace binding does not match (e.g. a
    /// [`FixedVcsRevision`] minted by a different repository or
    /// project root presented to this provider).
    ForeignBinding {
        /// Diagnostic detail.
        detail: String,
    },
    /// Any other backend/transport failure.
    Unavailable {
        /// Diagnostic detail (sanitized).
        detail: String,
    },
}

impl core::fmt::Display for VcsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotRepository { detail }
            | Self::RevisionGone { detail }
            | Self::MissingProjectFile { detail }
            | Self::MissingObject { detail }
            | Self::MissingLfs { detail }
            | Self::PermissionDenied { detail }
            | Self::AuthExpired { detail }
            | Self::Offline { detail }
            | Self::RateLimited { detail }
            | Self::UnsupportedCapability { detail }
            | Self::InvalidProjectPath { detail }
            | Self::BudgetExceeded { detail }
            | Self::InvalidRequest { detail }
            | Self::ForeignBinding { detail }
            | Self::Unavailable { detail } => write!(f, "{detail}"),
            Self::IndexChanged { before, after } => {
                write!(f, "index changed: {before:?} -> {after:?}")
            }
            Self::PartialStage { missing } => {
                write!(f, "partial stage: {} missing", missing.len())
            }
            Self::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for VcsError {}

/// Read-only VCS provider bound to one detected workspace.
///
/// Implementations must be `Send + Sync` and non-real-time: every
/// method may block on backend I/O but must honor [`VcsCancel`] and
/// the provider's declared [`VcsCapabilities::resource_bounds`].
///
/// Workspace detection precedes binding: the concrete provider type
/// exposes an associated `detect_workspace`-style constructor; once
/// bound, every method operates on that workspace only.
pub trait VcsReadProvider: Send + Sync {
    /// Detects the workspace containing `root` without binding a
    /// provider instance. The returned [`WorkspaceInfo`] carries the
    /// repository identity and the suggested `project_root` (the
    /// detected directory's path relative to the worktree root).
    ///
    /// # Errors
    /// [`VcsError::NotRepository`], [`VcsError::InvalidProjectPath`],
    /// [`VcsError::Unavailable`].
    fn detect_workspace(root: &ProjectPath) -> Result<WorkspaceInfo, VcsError>;

    /// Capabilities for the bound workspace.
    fn capabilities(&self) -> VcsCapabilities;

    /// Working-tree/index status for `scope` (`ProjectScope::root()`
    /// = whole repository).
    ///
    /// # Errors
    /// [`VcsError::UnsupportedCapability`], [`VcsError::Cancelled`],
    /// [`VcsError::Unavailable`].
    fn working_status(
        &self,
        scope: &ProjectScope,
        cancel: &VcsCancel,
    ) -> Result<WorkingStatus, VcsError>;

    /// One bounded page of merge-aware history starting at `query.start`
    /// (default `HEAD`), restricted to `query.path_scope` when set.
    ///
    /// # Errors
    /// [`VcsError::UnsupportedCapability`], [`VcsError::RevisionGone`],
    /// [`VcsError::InvalidRequest`] (bad cursor), [`VcsError::Cancelled`],
    /// [`VcsError::BudgetExceeded`], [`VcsError::Unavailable`].
    fn list_history(
        &self,
        query: &HistoryQuery,
        cancel: &VcsCancel,
    ) -> Result<HistoryPage, VcsError>;

    /// Resolves `selector` exactly once to a [`FixedVcsRevision`].
    /// Mutable refs (branches, tags, `HEAD`) are pinned to the commit
    /// observed at call time; later ref movement cannot affect the
    /// returned value.
    ///
    /// # Errors
    /// [`VcsError::RevisionGone`], [`VcsError::InvalidRequest`],
    /// [`VcsError::Cancelled`], [`VcsError::Unavailable`].
    fn resolve_fixed_revision(
        &self,
        selector: &RevisionSelector,
        cancel: &VcsCancel,
    ) -> Result<FixedVcsRevision, VcsError>;

    /// Parent relations of `fixed`. All merge parents are returned.
    ///
    /// # Errors
    /// [`VcsError::ForeignBinding`], [`VcsError::RevisionGone`],
    /// [`VcsError::Cancelled`], [`VcsError::Unavailable`].
    fn revision_relations(
        &self,
        fixed: &FixedVcsRevision,
        cancel: &VcsCancel,
    ) -> Result<RevisionRelations, VcsError>;

    /// Reads one repository-relative file's bytes at `fixed`.
    /// Bounded by the provider's resource limits.
    ///
    /// # Errors
    /// [`VcsError::ForeignBinding`], [`VcsError::RevisionGone`],
    /// [`VcsError::MissingProjectFile`], [`VcsError::BudgetExceeded`],
    /// [`VcsError::Cancelled`], [`VcsError::Unavailable`].
    fn read_revision_file(
        &self,
        fixed: &FixedVcsRevision,
        path: &ProjectRelativePath,
        cancel: &VcsCancel,
    ) -> Result<Vec<u8>, VcsError>;

    /// A fixed snapshot of the index (staging area) under the bound
    /// project scope, with its [`IndexGeneration`] fingerprint.
    ///
    /// # Errors
    /// [`VcsError::UnsupportedCapability`], [`VcsError::Cancelled`],
    /// [`VcsError::Unavailable`].
    fn resolve_staged_snapshot(&self, cancel: &VcsCancel) -> Result<StagedSnapshot, VcsError>;
}
