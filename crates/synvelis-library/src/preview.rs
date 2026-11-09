//! Preview byte cache (ALIB-03, Synvelis #87).
//!
//! `LibraryCache::preview` serves bounded, explicitly *unverified*
//! preview bytes from a principal-isolated disk cache keyed by
//! (provider, pinned `ObjectIdentity`, principal, decode format,
//! requested region, credential epoch). The same object digest under
//! a different principal is a different entry — private previews
//! never cross principals.
//!
//! Full-object fetches are never implicit: when the object size is
//! unknown or exceeds `max_preview_object_bytes`, [`preview_plan`]
//! reports [`PreviewPlan::FullDownload`] and `preview` refuses with
//! [`CacheError::FullDownloadRequired`] unless the caller sets
//! `PreviewRequest::allow_full_object`. Fetches beyond the hard
//! budgets report [`PreviewPlan::OverBudget`] and are refused
//! outright even with consent.
//!
//! Cache-store failures degrade to a live provider fetch
//! ([`PreviewLease::Live`]) — losing the cache never fails the
//! operation. Cached payloads carry a measured SHA-256 verified
//! incrementally as the lease is read.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ruviform_io_file::{Durability, PublishMode, PublishOptions, RecoveryHandle, StagedFile};
use serde::{Deserialize, Serialize};
use sha2::Digest as _;

use crate::cache::{
    CacheCategory, CacheCore, CacheError, CacheLease, EntryAux, EntryMeta, LibraryCache,
    PREVIEW_PAYLOAD, SIDECAR_FORMAT, SIDECAR_NAME, SidecarMeta,
};
use crate::context::ProviderContext;
use crate::digest::short_fingerprint;
use crate::error::{ProviderError, ProviderErrorKind};
use crate::id::ProviderId;
use crate::notify::LibraryEventKind;
use crate::observation::ObservedAt;
use crate::provider::{AssetProvider, ByteRange, FetchCompletion, PinnedRead};
use crate::refs::{ContentDigest, DigestAlgorithm, ObjectIdentity, PinnedAssetRef, RemoteAssetRef};

/// Preview payload stream chunk size — bounds per-read memory.
const PREVIEW_CHUNK: usize = 64 * 1024;

/// Decode/format/version identity of a preview payload.
///
/// `decoder` names the preview decoder pipeline (e.g. `"waveform"`,
/// `"thumbnail"`); `version` is the cache-format version the decoder
/// produces — bumping it invalidates older cached previews for the
/// same object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewFormat {
    /// Decoder pipeline identifier.
    pub decoder: String,
    /// Decoder output format version.
    pub version: u32,
}

/// What kind of preview bytes were requested.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewRequest {
    /// Decode/format identity — part of the cache key.
    pub format: PreviewFormat,
    /// Optional byte range of the *object* to fetch. `None` means the
    /// whole object (subject to the full-download rules).
    pub region: Option<ByteRange>,
    /// Explicit consent to fetch the whole object when it exceeds
    /// the bounded preview budget. Never implied — callers opt in.
    pub allow_full_object: bool,
}

/// The complete identity of one preview cache entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewCacheKey {
    /// Owning provider.
    pub provider: ProviderId,
    /// Pinned object identity — content identity, not a path.
    pub object_identity: ObjectIdentity,
    /// Principal scope — private previews never cross principals.
    pub principal: crate::PrincipalId,
    /// Decode/format identity.
    pub format: PreviewFormat,
    /// Requested region (`None` = whole object).
    pub region: Option<ByteRange>,
    /// Credential epoch the key was computed under.
    pub credential_epoch: u64,
}

impl PreviewCacheKey {
    /// Short fingerprint over the canonical key encoding.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        short_fingerprint(&bytes)
    }
}

/// Facts recorded alongside a cached preview payload.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreviewMeta {
    /// Pinned object identity the payload was fetched for.
    pub object_identity: ObjectIdentity,
    /// Cached payload bytes.
    pub byte_len: u64,
    /// Measured SHA-256 of the payload (`"sha256:<hex>"`), verified
    /// incrementally as a [`PreviewLease`] is read.
    pub digest: Option<ContentDigest>,
    /// When the payload was stored.
    pub stored_at: ObservedAt,
    /// Decode/format identity.
    pub format: PreviewFormat,
    /// Requested region (`None` = whole object).
    pub region: Option<ByteRange>,
}

/// What a preview fetch would cost — the explicit pre-flight answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PreviewPlan {
    /// The fetch is bounded — `preview` may proceed without
    /// `allow_full_object`.
    Bounded {
        /// Bytes the fetch would deliver.
        bytes: u64,
    },
    /// The fetch delivers the whole object — requires
    /// `PreviewRequest::allow_full_object = true`, and is still
    /// refused beyond `max_full_object_bytes`.
    FullDownload {
        /// Expected object size when the pin reported one.
        bytes: Option<u64>,
    },
    /// The fetch exceeds a hard cache budget — `preview` refuses
    /// with `BudgetExceeded` even with full-download consent.
    OverBudget {
        /// Bytes the fetch would deliver (or the declared object
        /// size for a full-object fetch).
        bytes: u64,
    },
}

/// The flight result of a preview fetch — shared with waiters.
#[derive(Clone)]
enum PreviewFetchOutcome {
    /// Payload committed to the cache.
    Stored(Box<EntryMeta>),
    /// The cache store path failed — caller falls back to a live
    /// provider fetch.
    Uncached,
}

/// A held preview payload — either a leased cache entry or a live
/// provider stream fallback.
///
/// Implements `Read`: sequential consumption only. Cached payloads
/// verify their recorded digest at EOF; the live fallback carries
/// the provider stream's own verification contract (unverified
/// preview bytes). Dropping releases the cache lease and closes the
/// file/stream — no FD or lease leaks on cancel or early drop.
pub struct PreviewLease {
    inner: PreviewLeaseInner,
    cancel: crate::CancellationToken,
}

/// Cached lease state — boxed out of [`PreviewLeaseInner`] so the
/// enum stays small (the incremental digest hasher alone is ~200
/// bytes).
struct CachedLease {
    /// Lease pinning the entry against eviction for the read's
    /// lifetime.
    lease: CacheLease,
    /// Open payload file.
    file: File,
    /// Payload bytes left to deliver.
    remaining: u64,
    /// Incremental SHA-256 over delivered bytes.
    hasher: sha2::Sha256,
    /// Recorded digest to verify at EOF.
    expected_digest: Option<ContentDigest>,
    /// `true` once EOF digest verification ran.
    verified: bool,
    /// Total payload bytes.
    byte_len: u64,
    /// Recorded preview metadata.
    meta: PreviewMeta,
}

enum PreviewLeaseInner {
    /// A leased, digest-verified cache payload.
    Cached(Box<CachedLease>),
    /// A live provider stream — used when the cache store path
    /// failed. Bytes are the provider's unverified preview contract.
    Live {
        reader: Box<dyn Read + Send>,
        expected_size: Option<u64>,
    },
}

impl PreviewLease {
    /// Expected payload bytes — `Some` for cached leases and for live
    /// fallbacks when the provider declared a size.
    #[must_use]
    pub fn byte_len(&self) -> Option<u64> {
        match &self.inner {
            PreviewLeaseInner::Cached(c) => Some(c.byte_len),
            PreviewLeaseInner::Live { expected_size, .. } => *expected_size,
        }
    }

    /// `true` when the bytes come from the cache (leased +
    /// digest-verified at EOF), `false` for the live fallback.
    #[must_use]
    pub fn is_cached(&self) -> bool {
        matches!(self.inner, PreviewLeaseInner::Cached(_))
    }

    /// The recorded preview metadata — `Some` for cached leases only.
    #[must_use]
    pub fn meta(&self) -> Option<&PreviewMeta> {
        match &self.inner {
            PreviewLeaseInner::Cached(c) => Some(&c.meta),
            PreviewLeaseInner::Live { .. } => None,
        }
    }

    /// Cached payload path for diagnostics — `None` for live
    /// fallback.
    #[must_use]
    pub fn payload_path(&self) -> Option<PathBuf> {
        match &self.inner {
            PreviewLeaseInner::Cached(c) => c.lease.payload_path(),
            PreviewLeaseInner::Live { .. } => None,
        }
    }
}

impl std::fmt::Debug for PreviewLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreviewLease")
            .field("cached", &self.is_cached())
            .field("byte_len", &self.byte_len())
            .finish_non_exhaustive()
    }
}

impl Read for PreviewLease {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "preview lease cancelled",
            ));
        }
        match &mut self.inner {
            PreviewLeaseInner::Cached(c) => {
                if c.remaining == 0 {
                    if !c.verified {
                        c.verified = true;
                        if let Some(expected) = &c.expected_digest {
                            let measured = ContentDigest::new(
                                DigestAlgorithm::Sha256,
                                &c.hasher.clone().finalize(),
                            );
                            if &measured != expected {
                                return Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    format!(
                                        "cached preview digest {measured} != recorded {expected}"
                                    ),
                                ));
                            }
                        }
                    }
                    return Ok(0);
                }
                let want = usize::try_from(c.remaining)
                    .unwrap_or(usize::MAX)
                    .min(buf.len());
                if want == 0 {
                    return Ok(0);
                }
                let n = c.file.read(&mut buf[..want])?;
                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "cached preview payload truncated",
                    ));
                }
                c.hasher.update(&buf[..n]);
                c.remaining -= n as u64;
                Ok(n)
            }
            PreviewLeaseInner::Live { reader, .. } => reader.read(buf),
        }
    }
}

/// Maps provider denials onto cache retirement; everything else
/// passes through unchanged.
fn classify_preview_error(
    err: CacheError,
    cache: &LibraryCache,
    cx: &ProviderContext,
) -> CacheError {
    if let Some(kind) = err.provider_error().map(ProviderError::kind)
        && matches!(
            kind,
            ProviderErrorKind::PermissionDenied | ProviderErrorKind::AuthExpired
        )
    {
        cache.retire_for_provider_denial(cx);
    }
    err
}

impl LibraryCache {
    /// Pre-flight plan for `preview` — pins the asset and reports
    /// whether the fetch is bounded or needs full-download consent.
    ///
    /// # Errors
    /// Provider failures (denials retire the principal's entries);
    /// [`CacheError`] otherwise.
    pub fn preview_plan(
        &self,
        provider: &dyn AssetProvider,
        asset: &RemoteAssetRef,
        request: &PreviewRequest,
        cx: &ProviderContext,
    ) -> Result<PreviewPlan, CacheError> {
        let pin = self.pin_for_preview(provider, asset, cx)?;
        Ok(plan_for(request, pin.expected_size, &self.core.policy))
    }

    /// Serves preview bytes for `asset`, from cache when a fresh
    /// entry exists and from a bounded provider fetch otherwise.
    ///
    /// # Errors
    /// [`CacheError::FullDownloadRequired`] when a whole-object fetch
    /// lacks consent; [`CacheError::Provider`] for provider failures;
    /// [`CacheError::LeaseExhausted`] at the global lease cap.
    pub fn preview(
        &self,
        provider: &dyn AssetProvider,
        asset: &RemoteAssetRef,
        request: &PreviewRequest,
        cx: &ProviderContext,
    ) -> Result<PreviewLease, CacheError> {
        let pin = self.pin_for_preview(provider, asset, cx)?;
        let key = PreviewCacheKey {
            provider: provider.id(),
            object_identity: pin.object_identity.clone(),
            principal: cx.principal.clone(),
            format: request.format.clone(),
            region: request.region,
            credential_epoch: self.credential_epoch(&cx.principal),
        };
        let key_fp = key.fingerprint();
        let ptag = CacheCore::principal_tag(&cx.principal);
        let now = SystemTime::now();
        let meta = self
            .core
            .lookup(CacheCategory::Preview, &ptag, &key_fp, now);

        // Fresh hit — open a leased payload. A vanished or
        // unreadable entry falls through to the fetch path.
        if let Some(meta) = meta.as_ref().filter(|m| !m.expired(now))
            && let Some(lease) = self.open_cached_lease(meta, cx)?
        {
            self.core.note_hit();
            return Ok(lease);
        }

        // Miss or expired — single-flight fetch+store.
        self.core.note_miss();
        let rel_dir = CacheCore::rel_dir(CacheCategory::Preview, &ptag, &key_fp);
        let outcome = self.core.run_flight(&rel_dir, cx, || {
            self.fetch_and_store_preview(provider, asset, &pin, request, &key, cx)
        })?;
        match outcome {
            PreviewFetchOutcome::Stored(meta) => {
                // A concurrent commit may still evict the entry
                // between store and lease — the cache is
                // best-effort, so degrade to a live read rather
                // than failing the operation.
                match self.open_cached_lease(&meta, cx)? {
                    Some(lease) => Ok(lease),
                    None => self.live_preview(provider, asset, request, cx),
                }
            }
            PreviewFetchOutcome::Uncached => self.live_preview(provider, asset, request, cx),
        }
    }

    /// Pins `asset` for preview keying, retiring the principal's
    /// entries on provider-reported denial.
    fn pin_for_preview(
        &self,
        provider: &dyn AssetProvider,
        asset: &RemoteAssetRef,
        cx: &ProviderContext,
    ) -> Result<PinnedAssetRef, CacheError> {
        match provider.pin(asset, cx) {
            Ok(pin) => Ok(pin),
            Err(err) => Err(classify_preview_error(CacheError::Provider(err), self, cx)),
        }
    }

    /// Opens a [`PreviewLease::Cached`] for `meta` — acquires the
    /// lease, opens the payload, wraps digest verification.
    /// `Ok(None)` when the entry vanished or its payload is
    /// unreadable (the entry is retired; caller refetches).
    fn open_cached_lease(
        &self,
        meta: &EntryMeta,
        cx: &ProviderContext,
    ) -> Result<Option<PreviewLease>, CacheError> {
        let EntryAux::Preview(preview_meta) = &meta.aux else {
            return Ok(None);
        };
        let Some(lease) = self.core.acquire_lease(&meta.rel_dir)? else {
            return Ok(None);
        };
        let Some(path) = lease.payload_path() else {
            return Ok(None);
        };
        let Ok(file) = File::open(&path) else {
            self.core.remove_entry(&meta.rel_dir);
            return Ok(None);
        };
        // Cheap integrity gate: the payload must be at least as long
        // as recorded; the digest itself verifies at EOF.
        let actual = file.metadata().map_or(0, |m| m.len());
        if actual < preview_meta.byte_len {
            self.core.remove_entry(&meta.rel_dir);
            return Ok(None);
        }
        Ok(Some(PreviewLease {
            inner: PreviewLeaseInner::Cached(Box::new(CachedLease {
                lease,
                file,
                remaining: preview_meta.byte_len,
                hasher: sha2::Sha256::new(),
                expected_digest: preview_meta.digest.clone(),
                verified: false,
                byte_len: preview_meta.byte_len,
                meta: preview_meta.clone(),
            })),
            cancel: cx.cancel.clone(),
        }))
    }

    /// Live fallback — a fresh `open_preview` stream wrapped as a
    /// [`PreviewLease::Live`]. Used when the cache store path
    /// failed; the provider contract (unverified preview bytes)
    /// applies unchanged.
    fn live_preview(
        &self,
        provider: &dyn AssetProvider,
        asset: &RemoteAssetRef,
        request: &PreviewRequest,
        cx: &ProviderContext,
    ) -> Result<PreviewLease, CacheError> {
        let pinned = match provider.open_preview(asset, request.region, cx) {
            Ok(pinned) => pinned,
            Err(err) => {
                return Err(classify_preview_error(CacheError::Provider(err), self, cx));
            }
        };
        Ok(PreviewLease {
            inner: PreviewLeaseInner::Live {
                reader: pinned.reader,
                expected_size: pinned.claims.expected_size,
            },
            cancel: cx.cancel.clone(),
        })
    }

    /// The single-flight leader body: bounds-check, stream the
    /// provider preview into a staged payload, publish, commit.
    /// Store-path failures return [`PreviewFetchOutcome::Uncached`];
    /// provider/fetch failures propagate as errors.
    fn fetch_and_store_preview(
        &self,
        provider: &dyn AssetProvider,
        asset: &RemoteAssetRef,
        pin: &PinnedAssetRef,
        request: &PreviewRequest,
        key: &PreviewCacheKey,
        cx: &ProviderContext,
    ) -> Result<PreviewFetchOutcome, CacheError> {
        let hard_cap = self.preview_hard_cap(pin, request)?;
        let principal_tag = CacheCore::principal_tag(&cx.principal);
        let key_fp = key.fingerprint();
        let rel_dir = CacheCore::rel_dir(CacheCategory::Preview, &principal_tag, &key_fp);
        let dir = self.core.entry_dir(&rel_dir);

        let pinned = match provider.open_preview(asset, request.region, cx) {
            Ok(pinned) => pinned,
            Err(err) => {
                return Err(classify_preview_error(CacheError::Provider(err), self, cx));
            }
        };
        let PinnedRead {
            mut reader,
            completion,
            ..
        } = pinned;

        let payload_path = dir.join(PREVIEW_PAYLOAD);
        let Ok(mut staged) = CacheCore::begin_payload(&payload_path) else {
            return Ok(PreviewFetchOutcome::Uncached);
        };

        let (total, hasher) = match Self::stream_payload(&mut *reader, &mut staged, hard_cap, cx) {
            Ok(done) => done,
            Err(CacheError::Io { .. }) => {
                let _ = staged.abort();
                return Ok(PreviewFetchOutcome::Uncached);
            }
            Err(other) => {
                let _ = staged.abort();
                return Err(classify_preview_error(other, self, cx));
            }
        };
        if staged.flush().is_err() {
            let _ = staged.abort();
            return Ok(PreviewFetchOutcome::Uncached);
        }

        let measured = match Self::verify_preview_fetch(&completion, &hasher, total, pin, request) {
            Ok(measured) => measured,
            Err(err) => {
                let _ = staged.abort();
                return Err(classify_preview_error(err, self, cx));
            }
        };

        // Publish payload, then sidecar, then commit.
        let sealed = staged.seal();
        if let Err(mut failure) = sealed.publish(PublishOptions::new(
            PublishMode::Replace,
            Durability::FileSync,
        )) {
            let _ = failure.take_recovery_handle().map(RecoveryHandle::cleanup);
            return Ok(PreviewFetchOutcome::Uncached);
        }

        self.commit_preview_payload(PreviewCommit {
            dir: &dir,
            rel_dir: &rel_dir,
            key,
            principal_tag: &principal_tag,
            key_fp: &key_fp,
            pin,
            request,
            total,
            measured,
            cx,
        })
    }

    /// The stream bound for this fetch — mirrors the
    /// `preview_plan` decision but enforced at fetch time.
    fn preview_hard_cap(
        &self,
        pin: &PinnedAssetRef,
        request: &PreviewRequest,
    ) -> Result<u64, CacheError> {
        match request.region {
            Some(range) => {
                if range.len > self.core.policy.max_preview_object_bytes {
                    return Err(CacheError::Provider(ProviderError::new(
                        ProviderErrorKind::BudgetExceeded,
                        format!(
                            "preview range {}B exceeds cache budget {}B",
                            range.len, self.core.policy.max_preview_object_bytes
                        ),
                    )));
                }
                Ok(range.len)
            }
            None => match pin.expected_size {
                Some(size) if size > self.core.policy.max_full_object_bytes => {
                    Err(CacheError::Provider(ProviderError::new(
                        ProviderErrorKind::BudgetExceeded,
                        format!(
                            "preview object {size}B exceeds hard ceiling {}B",
                            self.core.policy.max_full_object_bytes
                        ),
                    )))
                }
                Some(size)
                    if size > self.core.policy.max_preview_object_bytes
                        && !request.allow_full_object =>
                {
                    Err(CacheError::FullDownloadRequired {
                        expected_size: Some(size),
                    })
                }
                None if !request.allow_full_object => Err(CacheError::FullDownloadRequired {
                    expected_size: None,
                }),
                // Known size → the stream bound is the pin's own
                // length; unknown + consented → the hard ceiling.
                Some(size) => Ok(size),
                None => Ok(self.core.policy.max_full_object_bytes),
            },
        }
    }

    /// Streams `reader` into `staged`, hashing as it goes. Returns
    /// the measured byte count and digest state; exceeding
    /// `hard_cap` aborts early with `BudgetExceeded`.
    fn stream_payload<R: Read + ?Sized>(
        reader: &mut R,
        staged: &mut StagedFile,
        hard_cap: u64,
        cx: &ProviderContext,
    ) -> Result<(u64, sha2::Sha256), CacheError> {
        let mut hasher = sha2::Sha256::new();
        let mut total = 0u64;
        let mut buf = vec![0u8; PREVIEW_CHUNK];
        loop {
            if let Err(err) = cx.check_interrupts() {
                return Err(CacheError::Provider(err));
            }
            match reader.read(&mut buf) {
                Ok(0) => return Ok((total, hasher)),
                Ok(n) => {
                    total += n as u64;
                    if total > hard_cap {
                        return Err(CacheError::Provider(ProviderError::new(
                            ProviderErrorKind::BudgetExceeded,
                            format!("preview fetch exceeds cache budget {hard_cap}B"),
                        )));
                    }
                    hasher.update(&buf[..n]);
                    staged.write_all(&buf[..n]).map_err(|e| CacheError::Io {
                        op: "stage preview",
                        detail: e.to_string(),
                    })?;
                }
                Err(e) => {
                    // Provider streams surface typed errors through
                    // the io::Error source chain.
                    return Err(CacheError::Provider(crate::digest::io_provider_error(
                        "preview read",
                        &e,
                    )));
                }
            }
        }
    }

    /// Terminal verification of a streamed payload. The provider
    /// receipt is a cross-check only — stored facts are always the
    /// cache's own measurement; a provider that records no outcome
    /// still yields a valid (unverified) preview payload. The
    /// length gate then requires exact delivery. Returns the
    /// measured digest.
    fn verify_preview_fetch(
        completion: &FetchCompletion,
        hasher: &sha2::Sha256,
        total: u64,
        pin: &PinnedAssetRef,
        request: &PreviewRequest,
    ) -> Result<ContentDigest, CacheError> {
        match completion.take() {
            Some(Err(err)) => return Err(CacheError::Provider(err)),
            Some(Ok(receipt)) => {
                let measured =
                    ContentDigest::new(DigestAlgorithm::Sha256, &hasher.clone().finalize());
                let digest_ok = receipt.digest.as_ref().is_none_or(|d| *d == measured);
                if receipt.bytes_read != total || !digest_ok {
                    return Err(CacheError::Provider(ProviderError::new(
                        ProviderErrorKind::IntegrityMismatch,
                        format!(
                            "preview fetch mismatch: receipt {}/{}B vs measured {}/{}B",
                            receipt.bytes_read,
                            pin.expected_size.unwrap_or(0),
                            total,
                            measured,
                        ),
                    )));
                }
            }
            None => {}
        }
        // Length gate: a ranged fetch must deliver exactly the
        // range; a full-object fetch must match the pin's declared
        // size when one exists.
        let expected_total = match request.region {
            Some(range) => Some(range.len),
            None => pin.expected_size,
        };
        if let Some(expected) = expected_total
            && total != expected
        {
            return Err(CacheError::Provider(ProviderError::new(
                ProviderErrorKind::SourceChanged,
                format!("preview delivered {total}B != expected {expected}B"),
            )));
        }
        Ok(ContentDigest::new(
            DigestAlgorithm::Sha256,
            &hasher.clone().finalize(),
        ))
    }

    /// Persists a verified, published preview payload: sidecar,
    /// entry metadata, budget enforcement, store event.
    /// [`PreviewFetchOutcome::Uncached`] when the store path fails
    /// or the credential epoch moved mid-fetch.
    fn commit_preview_payload(
        &self,
        c: PreviewCommit<'_>,
    ) -> Result<PreviewFetchOutcome, CacheError> {
        let meta_record = PreviewMeta {
            object_identity: c.pin.object_identity.clone(),
            byte_len: c.total,
            digest: Some(c.measured),
            stored_at: c.cx.now(),
            format: c.request.format.clone(),
            region: c.request.region,
        };
        // TTL anchors on the cache's wall clock — provider logical
        // time (`stored_at`) is informational only.
        let committed_at = SystemTime::now();
        let sweep_tag = preview_sweep_tag(c.key, c.principal_tag);
        let sidecar = SidecarMeta::Preview {
            format_version: SIDECAR_FORMAT,
            sweep_tag: sweep_tag.clone(),
            credential_epoch: c.key.credential_epoch,
            meta: meta_record.clone(),
            committed_at,
        };
        let sidecar_bytes = match serde_json::to_vec(&sidecar) {
            Ok(bytes) => bytes,
            Err(e) => {
                return Err(CacheError::internal(format!("sidecar encode: {e}")));
            }
        };
        if CacheCore::publish_file(
            &c.dir.join(SIDECAR_NAME),
            &sidecar_bytes,
            PublishMode::Replace,
        )
        .is_err()
        {
            return Ok(PreviewFetchOutcome::Uncached);
        }
        let expires_at = committed_at
            .checked_add(self.core.policy.preview_ttl)
            .unwrap_or(committed_at);
        let entry = EntryMeta {
            rel_dir: c.rel_dir.to_owned(),
            category: CacheCategory::Preview,
            payload_bytes: c.total + sidecar_bytes.len() as u64,
            last_used_at: SystemTime::now(),
            expires_at,
            leases: 0,
            sweep_tag,
            generation: 0,
            credential_epoch: c.key.credential_epoch,
            principal: Some(c.key.principal.clone()),
            aux: EntryAux::Preview(meta_record.clone()),
            instance: 0,
        };
        if let Some(meta) = self.core.commit_entry(entry, SystemTime::now()) {
            let generation = self.core.events.generation();
            self.core.events.emit(
                generation,
                LibraryEventKind::PreviewStored {
                    key_fp: c.key_fp.to_owned(),
                    byte_len: c.total,
                },
                meta_record.stored_at,
            );
            Ok(PreviewFetchOutcome::Stored(Box::new(meta)))
        } else {
            // Epoch moved mid-fetch or budget enforcement evicted
            // the entry itself — remove the orphaned payload and
            // serve live rather than resurrect retired data.
            self.core.remove_entry(c.rel_dir);
            Ok(PreviewFetchOutcome::Uncached)
        }
    }
}

/// Facts collected while streaming a preview payload — everything
/// the commit step needs to persist it.
struct PreviewCommit<'a> {
    /// Entry directory (payload already published inside).
    dir: &'a Path,
    /// Cache-relative entry directory.
    rel_dir: &'a str,
    /// Preview cache key.
    key: &'a PreviewCacheKey,
    /// Principal namespace tag.
    principal_tag: &'a str,
    /// Key fingerprint (event payload).
    key_fp: &'a str,
    /// Pinned object identity.
    pin: &'a PinnedAssetRef,
    /// Original request.
    request: &'a PreviewRequest,
    /// Measured payload bytes.
    total: u64,
    /// Measured payload digest.
    measured: ContentDigest,
    /// Provider context.
    cx: &'a ProviderContext,
}

/// Sweep tag for preview entries — groups by
/// (provider, principal, credential epoch); preview entries retire
/// via the principal path rather than generation sweeps.
fn preview_sweep_tag(key: &PreviewCacheKey, principal_tag: &str) -> String {
    short_fingerprint(
        format!(
            "preview|{}|{}|{}|{}",
            key.provider.to_tagged(),
            principal_tag,
            key.credential_epoch,
            key.object_identity.as_str(),
        )
        .as_bytes(),
    )
}

/// The `preview_plan` decision for `expected_size` under `policy` —
/// mirrors the `preview_hard_cap` checks so the plan never promises
/// a fetch `preview` would refuse.
fn plan_for(
    request: &PreviewRequest,
    expected_size: Option<u64>,
    policy: &crate::CachePolicy,
) -> PreviewPlan {
    match request.region {
        Some(range) if range.len > policy.max_preview_object_bytes => {
            PreviewPlan::OverBudget { bytes: range.len }
        }
        Some(range) => PreviewPlan::Bounded { bytes: range.len },
        None => match expected_size {
            Some(size) if size > policy.max_full_object_bytes => {
                PreviewPlan::OverBudget { bytes: size }
            }
            Some(size) if size > policy.max_preview_object_bytes => {
                PreviewPlan::FullDownload { bytes: Some(size) }
            }
            Some(size) => PreviewPlan::Bounded { bytes: size },
            None => PreviewPlan::FullDownload { bytes: None },
        },
    }
}
