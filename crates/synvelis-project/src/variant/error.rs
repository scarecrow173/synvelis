//! Typed rejections for Creative Branch operations (synvelis#278).
//!
//! Variant operations fail closed with a typed variant so callers can
//! distinguish stale CAS, scope violations, ambiguous lineage and
//! unsupported materialization without string matching. Project-level
//! failures from the single writer are wrapped as
//! [`VariantError::Project`].

use std::fmt;

use crate::{
    Error,
    apply::ApplyError,
    snapshot::{ContentDigest, SnapshotError},
};

/// Typed failure of a variant-set operation.
#[derive(Debug)]
pub enum VariantError {
    /// A project-level failure from the single writer or a domain op.
    Project(Error),
    /// Snapshot capture/resolution failed while pinning the origin.
    Snapshot(SnapshotError),
    /// The scope declaration is structurally invalid for this project.
    InvalidScope {
        /// What was wrong with the scope.
        detail: String,
    },
    /// The scope overlaps an existing variant set's owned objects.
    ScopeConflict {
        /// The conflicting set.
        variant_set: String,
        /// Why the scopes conflict.
        detail: String,
    },
    /// `expected_branch_revision` did not match the branch's content
    /// revision — the caller's view is stale.
    StaleBranchRevision {
        /// Revision the caller expected.
        expected: u64,
        /// Revision actually recorded.
        actual: u64,
    },
    /// `expected_scope_digest` did not match the live scope fingerprint —
    /// the target drifted since the intent was captured.
    StaleScope {
        /// Digest the caller expected.
        expected: ContentDigest,
        /// Digest actually live.
        actual: ContentDigest,
    },
    /// `expected_project_revision` did not match the live revision.
    StaleProjectRevision {
        /// Revision the caller expected.
        expected: u64,
        /// Revision actually live.
        actual: u64,
    },
    /// The branch's candidate content is unavailable, corrupt, or was
    /// written at an unsupported schema version.
    CandidateUnavailable {
        /// Why the candidate could not be materialized.
        detail: String,
    },
    /// The requested operation is not supported by this slice — e.g.
    /// scope entity removal, comp-domain adopt materialization, or an
    /// edit op outside the fake adapter's contract.
    Unsupported {
        /// What is unsupported and why.
        detail: String,
    },
    /// A scoped entity cannot prove its lineage (not live, not minted
    /// by this branch, not from the fork origin).
    AmbiguousLineage {
        /// The unprovable object.
        detail: String,
    },
    /// A branch edit touched objects outside the declared scope.
    ScopeViolation {
        /// The out-of-scope object or operation.
        detail: String,
    },
    /// An adopt was attempted without an approval ticket.
    Unapproved,
    /// A newly introduced asset digest could not be proven present.
    MissingAsset {
        /// The digest that failed the probe.
        digest: String,
    },
    /// An asset digest probe returned `Unknown` — fail closed.
    AssetUnknown {
        /// The digest that could not be proven.
        digest: String,
    },
    /// A processor binding became or was introduced unresolved, or its
    /// plugin is unavailable on this host.
    MissingPlugin {
        /// Which processor/plugin state failed.
        detail: String,
    },
    /// A connection/send endpoint does not resolve post-apply.
    InvalidPort {
        /// Which endpoint failed.
        detail: String,
    },
    /// The branch content store failed.
    Store {
        /// Store failure detail.
        detail: String,
    },
}

impl fmt::Display for VariantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Project(e) => write!(f, "project error: {e}"),
            Self::Snapshot(e) => write!(f, "snapshot error: {e}"),
            Self::InvalidScope { detail } => write!(f, "invalid scope: {detail}"),
            Self::ScopeConflict {
                variant_set,
                detail,
            } => write!(f, "scope conflicts with {variant_set}: {detail}"),
            Self::StaleBranchRevision { expected, actual } => {
                write!(
                    f,
                    "stale branch revision: expected {expected}, actual {actual}"
                )
            }
            Self::StaleScope { expected, actual } => {
                write!(
                    f,
                    "stale scope digest: expected {expected}, actual {actual}"
                )
            }
            Self::StaleProjectRevision { expected, actual } => {
                write!(
                    f,
                    "stale project revision: expected {expected}, actual {actual}"
                )
            }
            Self::CandidateUnavailable { detail } => {
                write!(f, "candidate unavailable: {detail}")
            }
            Self::Unsupported { detail } => write!(f, "unsupported: {detail}"),
            Self::AmbiguousLineage { detail } => write!(f, "ambiguous lineage: {detail}"),
            Self::ScopeViolation { detail } => write!(f, "scope violation: {detail}"),
            Self::Unapproved => write!(f, "adopt intent is not approved"),
            Self::MissingAsset { digest } => write!(f, "missing asset {digest}"),
            Self::AssetUnknown { digest } => write!(f, "asset availability unknown {digest}"),
            Self::MissingPlugin { detail } => write!(f, "missing plugin: {detail}"),
            Self::InvalidPort { detail } => write!(f, "invalid port: {detail}"),
            Self::Store { detail } => write!(f, "content store error: {detail}"),
        }
    }
}

impl std::error::Error for VariantError {}

impl From<Error> for VariantError {
    fn from(e: Error) -> Self {
        Self::Project(e)
    }
}

impl From<SnapshotError> for VariantError {
    fn from(e: SnapshotError) -> Self {
        Self::Snapshot(e)
    }
}

impl From<ApplyError> for VariantError {
    fn from(e: ApplyError) -> Self {
        match e {
            ApplyError::MissingAsset { digest } => Self::MissingAsset { digest },
            ApplyError::AssetUnknown { digest } => Self::AssetUnknown { digest },
            ApplyError::MissingPlugin { detail } => Self::MissingPlugin { detail },
            ApplyError::InvalidPort { detail } => Self::InvalidPort { detail },
            ApplyError::Unsupported { detail, .. } => Self::Unsupported { detail },
            other => Self::Unsupported {
                detail: other.to_string(),
            },
        }
    }
}
