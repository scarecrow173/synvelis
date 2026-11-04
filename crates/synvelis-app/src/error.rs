//! Startup and asset-validation error types.
//!
//! `AssetError` describes exactly why a frontend bundle was rejected
//! (each [`AssetErrorKind`] maps to a distinct user-facing explanation);
//! `AppError` is the crate-level startup failure surface used by the
//! `synvelis` binary to decide between a clean explanation and an
//! emergency plain-shell fallback.

use std::error::Error;
use std::fmt::{Display, Formatter};

/// Why an asset bundle check failed.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetErrorKind {
    /// `synvelis-assets.json` is absent from the bundle.
    MissingManifest,
    /// The manifest is not valid JSON or does not match the expected
    /// shape.
    MalformedManifest,
    /// The manifest's own `schema` field differs from
    /// [`crate::revision::APP_ASSET_SCHEMA`].
    SchemaMismatch,
    /// The manifest's `ipc` schema version differs from the host's
    /// [`ruviform_ui::ipc::SCHEMA_VERSION`].
    IpcMismatch,
    /// The manifest's `host` ruviform revision differs from the
    /// revision the binary was built against.
    RevisionMismatch,
    /// A file listed in the manifest is missing from the bundle.
    MissingEntry,
    /// A file's byte length or SHA-256 differs from the manifest.
    HashMismatch,
    /// A manifest entry name escapes the `rvf://app` namespace.
    UnsafeEntry,
}

impl AssetErrorKind {
    /// Stable machine-readable tag (kept in sync with the frontend's
    /// error presentation table).
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::MissingManifest => "missing-manifest",
            Self::MalformedManifest => "malformed-manifest",
            Self::SchemaMismatch => "schema-mismatch",
            Self::IpcMismatch => "ipc-mismatch",
            Self::RevisionMismatch => "revision-mismatch",
            Self::MissingEntry => "missing-entry",
            Self::HashMismatch => "hash-mismatch",
            Self::UnsafeEntry => "unsafe-entry",
        }
    }
}

/// A rejected frontend bundle — the app stops with this explanation
/// instead of serving arbitrary or mismatched files.
#[derive(Debug)]
pub struct AssetError {
    kind: AssetErrorKind,
    detail: String,
}

impl AssetError {
    /// Creates an error of `kind` with human-readable `detail`.
    #[must_use]
    pub fn new(kind: AssetErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    /// The failure category.
    #[must_use]
    pub const fn kind(&self) -> AssetErrorKind {
        self.kind
    }

    /// Human-readable detail.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl Display for AssetError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind.tag(), self.detail)
    }
}

impl Error for AssetError {}

/// Application startup failure surface.
#[non_exhaustive]
#[derive(Debug)]
pub enum AppError {
    /// The platform `WebView` backend is missing or unusable.
    BackendUnavailable(String),
    /// The frontend asset bundle failed validation.
    Asset(AssetError),
    /// A host/window operation failed.
    Host(String),
    /// A project store operation failed.
    Store(String),
}

impl AppError {
    /// Convenience constructor for [`AppError::BackendUnavailable`].
    #[must_use]
    pub fn backend_unavailable(detail: impl Into<String>) -> Self {
        Self::BackendUnavailable(detail.into())
    }

    /// Convenience constructor for [`AppError::Host`].
    #[must_use]
    pub fn host(detail: impl Into<String>) -> Self {
        Self::Host(detail.into())
    }

    /// Convenience constructor for [`AppError::Store`].
    #[must_use]
    pub fn store(detail: impl Into<String>) -> Self {
        Self::Store(detail.into())
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendUnavailable(d) => write!(f, "webview backend unavailable: {d}"),
            Self::Asset(e) => write!(f, "frontend asset bundle rejected: {e}"),
            Self::Host(d) => write!(f, "host error: {d}"),
            Self::Store(d) => write!(f, "project store error: {d}"),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Asset(e) => Some(e),
            Self::BackendUnavailable(_) | Self::Host(_) | Self::Store(_) => None,
        }
    }
}

impl From<AssetError> for AppError {
    fn from(e: AssetError) -> Self {
        Self::Asset(e)
    }
}

impl AppError {
    /// Converts into an [`ruviform_ui::ipc::ErrorBody`] for the wire —
    /// `service_error` with the explanation preserved in `message`.
    #[must_use]
    pub fn into_body(self) -> ruviform_ui::ipc::ErrorBody {
        ruviform_ui::ipc::ErrorBody::new(ruviform_ui::ipc::codes::SERVICE_ERROR, self.to_string())
    }
}
