//! Typed failure classes for safe import and verified resolution.

use std::fmt;

use synvelis_project::{AssetId, ImportId, SourceId};

/// Failures of the safe local import pipeline. Every variant names a
/// distinct stage so a caller can report exactly where an import died;
/// none of them leaves a partial project mutation behind.
#[derive(Debug)]
#[non_exhaustive]
pub enum ImportError {
    /// Source path could not be opened/read, or staging I/O failed.
    StagingIo {
        /// Underlying diagnostic.
        detail: String,
    },
    /// The staged bytes failed container probing or decoding.
    Corrupt {
        /// Underlying diagnostic.
        detail: String,
    },
    /// Container parsed but holds no decodable PCM audio stream.
    UnsupportedCodec {
        /// Underlying diagnostic.
        detail: String,
    },
    /// The verified blob was published but the project commit failed —
    /// the blob is an explicit orphan (path + digest recorded for
    /// cleanup or retry).
    Orphaned {
        /// Content digest of the orphaned blob.
        digest: String,
        /// The commit-stage failure.
        detail: String,
    },
    /// A resume token names an import whose manifest is absent.
    UnknownImport {
        /// The unresolved import identity.
        import_id: ImportId,
    },
    /// The reserved import identity is already applied with different
    /// content — an integrity conflict, never a second apply.
    Conflict {
        /// The conflicting import identity.
        import_id: ImportId,
    },
    /// The caller cancelled between stages.
    Cancelled,
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StagingIo { detail } => write!(f, "staging I/O failed: {detail}"),
            Self::Corrupt { detail } => write!(f, "corrupt input: {detail}"),
            Self::UnsupportedCodec { detail } => {
                write!(f, "unsupported codec/container: {detail}")
            }
            Self::Orphaned { digest, detail } => {
                write!(f, "commit failed; orphan blob {digest}: {detail}")
            }
            Self::UnknownImport { import_id } => {
                write!(f, "no manifest for import {import_id}")
            }
            Self::Conflict { import_id } => {
                write!(f, "import {import_id} applied with different content")
            }
            Self::Cancelled => write!(f, "import cancelled"),
        }
    }
}

impl std::error::Error for ImportError {}

/// Typed failures of the verified resolver: stable Source → blob →
/// reader/prefetch. Each variant is a distinct contract violation so
/// playback never confuses "no audio" with "silence".
#[derive(Debug)]
#[non_exhaustive]
pub enum ResolveError {
    /// The `SourceId` is not registered in the project.
    UnresolvedSource {
        /// The missing source.
        source_id: SourceId,
    },
    /// The source exists but is below `Verified` — a pre-import path
    /// must never become playable.
    UnverifiedSource {
        /// The source.
        source_id: SourceId,
        /// Its current state label.
        state: String,
    },
    /// The backing asset registration is absent.
    MissingAsset {
        /// The missing asset.
        asset_id: AssetId,
    },
    /// The verified blob is absent from the media store.
    MissingMedia {
        /// Expected content digest.
        digest: String,
    },
    /// Blob bytes on disk differ from the recorded digest.
    HashMismatch {
        /// Recorded digest.
        expected: String,
        /// Measured digest.
        actual: String,
    },
    /// Blob bytes fail container probing/decoding.
    Corrupt {
        /// Underlying diagnostic.
        detail: String,
    },
    /// Codec/container/format unsupported by the decode backend.
    UnsupportedCodec {
        /// Underlying diagnostic.
        detail: String,
    },
    /// Underlying I/O failure.
    Io {
        /// Underlying diagnostic.
        detail: String,
    },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedSource { source_id } => {
                write!(f, "unresolved source {source_id}")
            }
            Self::UnverifiedSource { source_id, state } => {
                write!(f, "source {source_id} is {state}, not verified")
            }
            Self::MissingAsset { asset_id } => write!(f, "missing asset {asset_id}"),
            Self::MissingMedia { digest } => write!(f, "missing blob {digest}"),
            Self::HashMismatch { expected, actual } => {
                write!(f, "blob hash mismatch: expected {expected}, got {actual}")
            }
            Self::Corrupt { detail } => write!(f, "corrupt media: {detail}"),
            Self::UnsupportedCodec { detail } => {
                write!(f, "unsupported codec/format: {detail}")
            }
            Self::Io { detail } => write!(f, "I/O failure: {detail}"),
        }
    }
}

impl std::error::Error for ResolveError {}
