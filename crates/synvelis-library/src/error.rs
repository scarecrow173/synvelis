//! Typed provider errors and retry advice.
//!
//! `ProviderError` pairs a machine-readable [`ProviderErrorKind`] with
//! an independent [`RetryAdvice`]: the kind says *what* failed, the
//! advice says *whether retrying can help*. Callers must not infer
//! project-mutation outcomes from these errors — the provider layer
//! never mutates the project.

use std::fmt;
use std::time::Duration;

/// Machine-readable failure classification for provider operations.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ProviderErrorKind {
    /// The locator or object does not exist on the provider.
    NotFound,
    /// The pinned revision existed once but is gone now.
    RevisionGone,
    /// The current principal lacks permission for the operation.
    PermissionDenied,
    /// Credentials expired or were revoked.
    AuthExpired,
    /// The provider backend is unreachable (remote providers).
    Offline,
    /// The backend asked the caller to slow down.
    RateLimited,
    /// A transient I/O or backend failure worth retrying.
    TransientIo,
    /// The capability is not supported (typed reason travels in
    /// [`crate::ProviderCapabilities`] / the error detail).
    UnsupportedCapability,
    /// A fetch precondition (strong validator, size, identity) failed.
    PreconditionFailed,
    /// The source changed since it was observed or pinned.
    SourceChanged,
    /// Bytes read do not match the pin's verification evidence.
    IntegrityMismatch,
    /// The object is not a valid media/asset payload.
    InvalidMedia,
    /// A caller- or context-imposed budget (bytes, entries, deadline)
    /// was exceeded.
    BudgetExceeded,
    /// The operation was cancelled through the provider context.
    Cancelled,
    /// A page cursor failed validation (foreign provider, changed
    /// query/scope/principal, expired epoch, or undecodable payload).
    CursorInvalid,
    /// The request itself is malformed (bad locator, wrong proof type,
    /// illegal argument combination).
    InvalidRequest,
    /// Anything else — details in `detail`.
    Internal,
}

/// Independent retry guidance for a [`ProviderError`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum RetryAdvice {
    /// Do not retry; the failure is deterministic.
    Never,
    /// Retrying may succeed after a delay.
    After(Option<Duration>),
    /// Retry only after re-authenticating.
    AfterAuth,
    /// Retry only after the user re-pins the asset (the pin is stale).
    AfterUserRePin,
    /// The failure may reconcile itself (e.g. a published-but-
    /// unacknowledged write); reconcile before retrying.
    MayReconcile,
}

/// A typed provider error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderError {
    kind: ProviderErrorKind,
    retry: RetryAdvice,
    detail: String,
}

impl ProviderError {
    /// Builds an error with the default retry advice for `kind`.
    #[must_use]
    pub fn new(kind: ProviderErrorKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            retry: default_retry(kind),
            detail: detail.into(),
        }
    }

    /// Overrides the default retry advice.
    #[must_use]
    pub fn with_retry(mut self, retry: RetryAdvice) -> Self {
        self.retry = retry;
        self
    }

    /// The failure kind.
    #[must_use]
    pub const fn kind(&self) -> ProviderErrorKind {
        self.kind
    }

    /// The retry advice.
    #[must_use]
    pub const fn retry(&self) -> RetryAdvice {
        self.retry
    }

    /// Human-readable diagnostic detail.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// `true` when `kind` is [`ProviderErrorKind::UnsupportedCapability`].
    #[must_use]
    pub const fn is_unsupported(&self) -> bool {
        matches!(self.kind, ProviderErrorKind::UnsupportedCapability)
    }

    /// Convenience constructor for an unsupported capability.
    #[must_use]
    pub fn unsupported(capability: &'static str, provider: &'static str) -> Self {
        Self::new(
            ProviderErrorKind::UnsupportedCapability,
            format!("{provider} does not support {capability}"),
        )
    }
}

/// Default retry advice per error kind.
#[must_use]
const fn default_retry(kind: ProviderErrorKind) -> RetryAdvice {
    match kind {
        ProviderErrorKind::NotFound
        | ProviderErrorKind::PermissionDenied
        | ProviderErrorKind::UnsupportedCapability
        | ProviderErrorKind::IntegrityMismatch
        | ProviderErrorKind::InvalidMedia
        | ProviderErrorKind::InvalidRequest
        // A stale cursor is invalidated by re-browsing, never by
        // re-pinning — the asset pin is unrelated to page position.
        | ProviderErrorKind::CursorInvalid
        | ProviderErrorKind::Cancelled => RetryAdvice::Never,
        ProviderErrorKind::TransientIo | ProviderErrorKind::Offline => RetryAdvice::After(None),
        ProviderErrorKind::RateLimited => RetryAdvice::After(Some(Duration::from_secs(1))),
        ProviderErrorKind::AuthExpired => RetryAdvice::AfterAuth,
        ProviderErrorKind::RevisionGone
        | ProviderErrorKind::PreconditionFailed
        | ProviderErrorKind::SourceChanged => RetryAdvice::AfterUserRePin,
        ProviderErrorKind::BudgetExceeded | ProviderErrorKind::Internal => {
            RetryAdvice::MayReconcile
        }
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for ProviderError {}

impl From<ProviderError> for std::io::Error {
    fn from(err: ProviderError) -> Self {
        let kind = match err.kind() {
            ProviderErrorKind::NotFound => std::io::ErrorKind::NotFound,
            ProviderErrorKind::PermissionDenied | ProviderErrorKind::AuthExpired => {
                std::io::ErrorKind::PermissionDenied
            }
            ProviderErrorKind::Cancelled => std::io::ErrorKind::Interrupted,
            ProviderErrorKind::IntegrityMismatch
            | ProviderErrorKind::InvalidMedia
            | ProviderErrorKind::SourceChanged
            | ProviderErrorKind::PreconditionFailed
            | ProviderErrorKind::RevisionGone => std::io::ErrorKind::InvalidData,
            _ => std::io::ErrorKind::Other,
        };
        // Keep the typed error as the source so readers can recover it
        // with `std::io::Error::get_ref` / `downcast_ref`.
        std::io::Error::new(kind, err)
    }
}
