//! The `AssetProvider` contract and fetch plumbing.
//!
//! Providers are `Send + Sync` blocking workers: calls may perform
//! filesystem or network I/O, but they run on non-RT worker threads
//! and never touch Tokio/HTTP SDK types in the public API.
//! `open_pinned` returns a chunked `Read + Send` stream whose terminal
//! outcome (a typed [`FetchReceipt`] or [`ProviderError`]) is written
//! into the shared [`FetchCompletion`] slot at EOF.

use std::fmt;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::browse::{BrowsePage, BrowseRequest, SearchQuery};
use crate::capability::ProviderCapabilities;
use crate::context::ProviderContext;
use crate::error::ProviderError;
use crate::id::ProviderId;
use crate::observation::AssetStat;
use crate::refs::{ContentDigest, ObjectIdentity, PinnedAssetRef, RemoteAssetRef};
use crate::registry::ProviderKind;

/// A half-open byte range `[offset, offset + len)`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ByteRange {
    /// Start offset in bytes.
    pub offset: u64,
    /// Length in bytes.
    pub len: u64,
}

/// What `open_pinned` promises to verify while streaming.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerificationPolicy {
    /// The stream verifies a strong digest at EOF; a mismatch fails
    /// with `IntegrityMismatch` before the caller sees a clean end.
    StreamVerified {
        /// The expected strong digest.
        digest: ContentDigest,
    },
    /// Bytes are bound by the store's identity convention (a
    /// content-addressed name). No strong digest was pinned; byte
    /// integrity is deferred to downstream verification.
    IdentityBound {
        /// The identity scheme (`"cas-blob"`).
        scheme: String,
    },
    /// No verification — preview/range reads are explicitly
    /// unverified observations.
    Unverified,
}

/// What a fetch claims to deliver — declared before streaming, so the
/// caller knows the verification policy upfront.
#[derive(Clone, Debug, PartialEq)]
pub struct FetchClaims {
    /// The object identity being fetched.
    pub object_identity: ObjectIdentity,
    /// Expected byte length when known — a hint, not a guarantee.
    pub expected_size: Option<u64>,
    /// The verification the stream applies.
    pub verification: VerificationPolicy,
}

/// The terminal outcome of a fetch stream.
#[derive(Clone, Debug, PartialEq)]
pub struct FetchReceipt {
    /// The object identity actually fetched.
    pub object_identity: ObjectIdentity,
    /// Bytes delivered.
    pub bytes_read: u64,
    /// The measured strong digest when the stream computed one.
    pub digest: Option<ContentDigest>,
    /// `true` only when the pinned strong digest was verified at EOF.
    pub verified: bool,
}

/// The shared terminal-outcome slot a fetch stream writes into.
///
/// Public so [`AssetProvider`] implementors can construct a
/// [`FetchCompletion`]; consumers only observe it through that handle.
/// `finished` is latched: consuming the outcome does not reset the
/// terminal state.
#[derive(Debug)]
pub struct FetchSlot {
    outcome: Mutex<Option<Result<FetchReceipt, ProviderError>>>,
    finished: AtomicBool,
}

impl FetchSlot {
    /// An empty slot.
    #[must_use]
    pub fn new() -> Self {
        Self {
            outcome: Mutex::new(None),
            finished: AtomicBool::new(false),
        }
    }

    /// Records the terminal outcome (the reader-side exactly-once
    /// guard lives in `VerifyingReader::finished`).
    pub(crate) fn record(&self, outcome: Result<FetchReceipt, ProviderError>) {
        if let Ok(mut slot) = self.outcome.lock() {
            *slot = Some(outcome);
        }
        self.finished.store(true, Ordering::Release);
    }

    /// Consumes the recorded outcome, or `None` while unfinished.
    fn take_outcome(&self) -> Option<Result<FetchReceipt, ProviderError>> {
        self.outcome.lock().ok()?.take()
    }

    /// `true` once a terminal outcome was recorded — stays `true`
    /// after the outcome is taken.
    fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire) || self.outcome.lock().is_ok_and(|s| s.is_some())
    }
}

impl Default for FetchSlot {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared terminal slot — the reader writes exactly one outcome.
///
/// `take()` returns `None` while the stream is unfinished (EOF not
/// reached or caller abandoned mid-read).
#[derive(Clone, Debug)]
pub struct FetchCompletion {
    slot: Arc<FetchSlot>,
}

impl FetchCompletion {
    /// Wraps the shared slot.
    #[must_use]
    pub fn new(slot: Arc<FetchSlot>) -> Self {
        Self { slot }
    }

    /// The typed outcome once the stream finished, or `None` while
    /// unfinished. Consumes the stored result.
    #[must_use]
    pub fn take(&self) -> Option<Result<FetchReceipt, ProviderError>> {
        self.slot.take_outcome()
    }

    /// `true` once a terminal outcome was recorded — stays `true`
    /// after `take()` consumes it.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.slot.is_finished()
    }
}

/// An open fetch stream plus its claims and completion slot.
pub struct PinnedRead {
    /// Chunked byte stream. `Send`, not `Sync` — consume on one worker.
    pub reader: Box<dyn Read + Send>,
    /// Up-front claims about the fetch.
    pub claims: FetchClaims,
    /// Terminal typed outcome slot.
    pub completion: FetchCompletion,
}

impl fmt::Debug for PinnedRead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PinnedRead")
            .field("claims", &self.claims)
            .field("finished", &self.completion.is_finished())
            .finish_non_exhaustive()
    }
}

/// The typed asset-provider contract.
///
/// Blocking, non-RT worker contract. Implementations must be
/// `Send + Sync` and must not require a Tokio runtime, an HTTP client,
/// or a GUI framework in their public surface.
pub trait AssetProvider: Send + Sync {
    /// The provider connection identity.
    fn id(&self) -> ProviderId;

    /// The provider family — cross-checked against the registry
    /// descriptor so a misdescribed registration fails fast.
    fn kind(&self) -> ProviderKind;

    /// Declared capabilities, optionally narrowed to one asset.
    fn capabilities(&self, asset: Option<&RemoteAssetRef>) -> ProviderCapabilities;

    /// Lists one page of the scope's catalog.
    ///
    /// # Errors
    /// [`ProviderError`] — typed per failure class; an empty-but-valid
    /// scope is a `Complete` page, not an error.
    fn browse(
        &self,
        req: &BrowseRequest,
        cx: &ProviderContext,
    ) -> Result<BrowsePage, ProviderError>;

    /// Observes one asset's facts.
    ///
    /// # Errors
    /// [`ProviderError`] — `NotFound`, `PermissionDenied`, traversal
    /// blocks, and other typed failures.
    fn stat(
        &self,
        asset: &RemoteAssetRef,
        cx: &ProviderContext,
    ) -> Result<AssetStat, ProviderError>;

    /// Fixes the asset to a fetchable object revision.
    ///
    /// # Errors
    /// [`ProviderError`] — `UnsupportedCapability` when the provider
    /// cannot pin, `RevisionGone`/`SourceChanged` when the target
    /// moved, `BudgetExceeded` when the evidence is too large to
    /// measure.
    fn pin(
        &self,
        asset: &RemoteAssetRef,
        cx: &ProviderContext,
    ) -> Result<PinnedAssetRef, ProviderError>;

    /// Opens a pinned asset for chunked reading.
    ///
    /// # Errors
    /// [`ProviderError`] — `NotFound`, `SourceChanged` (precondition
    /// failure at open), `InvalidRequest` (foreign proof), and other
    /// typed failures. Mid-stream failures surface as `io::Error`
    /// on the reader plus a typed record in
    /// [`PinnedRead::completion`].
    fn open_pinned(
        &self,
        pin: &PinnedAssetRef,
        cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError>;

    /// Optional provider-side search.
    ///
    /// # Errors
    /// The default returns `UnsupportedCapability`. Providers without
    /// a query index must not fake an empty result.
    fn search(
        &self,
        _query: &SearchQuery,
        _cx: &ProviderContext,
    ) -> Result<BrowsePage, ProviderError> {
        Err(ProviderError::unsupported("search", "provider"))
    }

    /// Optional bounded preview read — explicitly unverified bytes.
    ///
    /// # Errors
    /// The default returns `UnsupportedCapability`.
    fn open_preview(
        &self,
        _asset: &RemoteAssetRef,
        _range: Option<ByteRange>,
        _cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        Err(ProviderError::unsupported("open_preview", "provider"))
    }
}
