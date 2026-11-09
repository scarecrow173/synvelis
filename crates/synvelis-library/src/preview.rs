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
//! `PreviewRequest::allow_full_object`. A hard ceiling
//! (`max_full_object_bytes`) refuses outright even with consent.
//!
//! Cache-store failures degrade to a live provider fetch
//! ([`PreviewLease::Live`]) — losing the cache never fails the
//! operation. Cached payloads carry a measured SHA-256 verified
//! incrementally as the lease is read.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::time::SystemTime;

use ruviform_io_file::{Durability, PublishMode, PublishOptions};
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
use crate::provider::{AssetProvider, ByteRange, PinnedRead};
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
}

/// The flight result of a preview fetch — shared with waiters.
#[derive(Clone)]
enum PreviewFetchOutcome {
    /// Payload committed to the cache.
    Stored(EntryMeta),
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

enum PreviewLeaseInner {
    /// A leased, digest-verified cache payload.
    Cached {
        _lease: CacheLease,
        file: File,
        remaining: u64,
        hasher: sha2::Sha256,
        expected_digest: Option<ContentDigest>,
        verified: bool,
        byte_len: u64,
        meta: PreviewMeta,
    },
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
            PreviewLeaseInner::Cached { byte_len, .. } => Some(*byte_len),
            PreviewLeaseInner::Live { expected_size, .. } => *expected_size,
        }
    }

    /// `true` when the bytes come from the cache (leased +
    /// digest-verified at EOF), `false` for the live fallback.
    #[must_use]
    pub fn is_cached(&self) -> bool {
        matches!(self.inner, PreviewLeaseInner::Cached { .. })
    }

    /// The recorded preview metadata — `Some` for cached leases only.
    #[must_use]
    pub fn meta(&self) -> Option<&PreviewMeta> {
        match &self.inner {
            PreviewLeaseInner::Cached { meta, .. } => Some(meta),
            PreviewLeaseInner::Live { .. } => None,
        }
    }

    /// Cached payload path for diagnostics — `None` for live
    /// fallback.
    #[must_use]
    pub fn payload_path(&self) -> Option<PathBuf> {
        match &self.inner {
            PreviewLeaseInner::Cached { _lease, .. } => _lease.payload_path(),
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
            PreviewLeaseInner::Cached {
                file,
                remaining,
                hasher,
                expected_digest,
                verified,
                ..
            } => {
                if *remaining == 0 {
                    if !*verified {
                        *verified = true;
                        if let Some(expected) = expected_digest {
                            let measured = ContentDigest::new(
                                DigestAlgorithm::Sha256,
                                &hasher.clone().finalize(),
                            );
                            if measured != *expected {
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
                let want = usize::try_from(*remaining)
                    .unwrap_or(usize::MAX)
                    .min(buf.len());
                if want == 0 {
                    return Ok(0);
                }
                let n = file.read(&mut buf[..want])?;
                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "cached preview payload truncated",
                    ));
                }
                hasher.update(&buf[..n]);
                *remaining -= n as u64;
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
    if let Some(kind) = err.provider_error().map(|e| e.kind())
        && matches!(
            kind,
            ProviderErrorKind::PermissionDenied | ProviderErrorKind::AuthExpired
        )
    {
        let _ = cache.retire_for_provider_denial(cx);
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

        // Fresh hit — open a leased payload.
        if let Some(meta) = meta.as_ref().filter(|m| !m.expired(now)) {
            match self.open_cached_lease(meta, cx)? {
                Some(lease) => {
                    self.core.note_hit();
                    return Ok(lease);
                }
                None => {
                    // Vanished or unreadable — fall through to fetch.
                }
            }
        }

        // Miss or expired — single-flight fetch+store.
        self.core.note_miss();
        let rel_dir = CacheCore::rel_dir(CacheCategory::Preview, &ptag, &key_fp);
        let outcome = self.core.run_flight(&rel_dir, cx, || {
            self.fetch_and_store_preview(provider, asset, &pin, request, &key, &ptag, &key_fp, cx)
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
        let lease = match self.core.acquire_lease(&meta.rel_dir)? {
            Some(lease) => lease,
            None => return Ok(None),
        };
        let Some(path) = lease.payload_path() else {
            return Ok(None);
        };
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(_) => {
                self.core.remove_entry(&meta.rel_dir);
                return Ok(None);
            }
        };
        // Cheap integrity gate: the payload must be at least as long
        // as recorded; the digest itself verifies at EOF.
        let actual = file.metadata().map(|m| m.len()).unwrap_or(0);
        if actual < preview_meta.byte_len {
            self.core.remove_entry(&meta.rel_dir);
            return Ok(None);
        }
        Ok(Some(PreviewLease {
            inner: PreviewLeaseInner::Cached {
                _lease: lease,
                file,
                remaining: preview_meta.byte_len,
                hasher: sha2::Sha256::new(),
                expected_digest: preview_meta.digest.clone(),
                verified: false,
                byte_len: preview_meta.byte_len,
                meta: preview_meta.clone(),
            },
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
        principal_tag: &str,
        key_fp: &str,
        cx: &ProviderContext,
    ) -> Result<PreviewFetchOutcome, CacheError> {
        // Policy gate — mirrors `preview_plan` but enforced at fetch.
        let hard_cap = match request.region {
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
                range.len
            }
            None => match pin.expected_size {
                Some(size) if size > self.core.policy.max_full_object_bytes => {
                    return Err(CacheError::Provider(ProviderError::new(
                        ProviderErrorKind::BudgetExceeded,
                        format!(
                            "preview object {size}B exceeds hard ceiling {}B",
                            self.core.policy.max_full_object_bytes
                        ),
                    )));
                }
                Some(size)
                    if size > self.core.policy.max_preview_object_bytes
                        && !request.allow_full_object =>
                {
                    return Err(CacheError::FullDownloadRequired {
                        expected_size: Some(size),
                    });
                }
                None if !request.allow_full_object => {
                    return Err(CacheError::FullDownloadRequired {
                        expected_size: None,
                    });
                }
                // Known size → the stream bound is the pin's own
                // length; unknown + consented → the hard ceiling.
                Some(size) => size,
                None => self.core.policy.max_full_object_bytes,
            },
        };

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

        let rel_dir = CacheCore::rel_dir(CacheCategory::Preview, principal_tag, key_fp);
        let dir = self.core.entry_dir(&rel_dir);
        let payload_path = dir.join(PREVIEW_PAYLOAD);
        let mut staged = match self.core.begin_payload(&payload_path) {
            Ok(staged) => staged,
            Err(_) => return Ok(PreviewFetchOutcome::Uncached),
        };

        let mut hasher = sha2::Sha256::new();
        let mut total = 0u64;
        let mut buf = vec![0u8; PREVIEW_CHUNK];
        let stream_result = loop {
            if let Err(err) = cx.check_interrupts() {
                break Err(CacheError::Provider(err));
            }
            match reader.read(&mut buf) {
                Ok(0) => break Ok(()),
                Ok(n) => {
                    total += n as u64;
                    if total > hard_cap {
                        break Err(CacheError::Provider(ProviderError::new(
                            ProviderErrorKind::BudgetExceeded,
                            format!("preview fetch exceeds cache budget {hard_cap}B"),
                        )));
                    }
                    hasher.update(&buf[..n]);
                    if let Err(e) = staged.write_all(&buf[..n]) {
                        break Err(CacheError::Io {
                            op: "stage preview",
                            detail: e.to_string(),
                        });
                    }
                }
                Err(e) => {
                    // Provider streams surface typed errors through
                    // the io::Error source chain.
                    break Err(CacheError::Provider(crate::digest::io_provider_error(
                        "preview read",
                        &e,
                    )));
                }
            }
        };
        if let Err(err) = stream_result {
            let _ = staged.abort();
            // Store-path I/O failures degrade to the live fallback;
            // provider/fetch failures propagate.
            return match err {
                CacheError::Io { .. } => Ok(PreviewFetchOutcome::Uncached),
                other => Err(classify_preview_error(other, self, cx)),
            };
        }
        if let Err(_e) = staged.flush() {
            let _ = staged.abort();
            return Ok(PreviewFetchOutcome::Uncached);
        }

        // Terminal receipt — the provider's own accounting is a
        // cross-check only: the stored facts are always the cache's
        // measurement of the bytes it actually wrote. A provider
        // that records no outcome still yields a valid (unverified)
        // preview payload.
        match completion.take() {
            Some(Err(err)) => {
                let _ = staged.abort();
                return Err(classify_preview_error(CacheError::Provider(err), self, cx));
            }
            Some(Ok(receipt)) => {
                let measured =
                    ContentDigest::new(DigestAlgorithm::Sha256, &hasher.clone().finalize());
                let digest_ok = receipt.digest.as_ref().is_none_or(|d| *d == measured);
                if receipt.bytes_read != total || !digest_ok {
                    let _ = staged.abort();
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
            let _ = staged.abort();
            return Err(CacheError::Provider(ProviderError::new(
                ProviderErrorKind::SourceChanged,
                format!("preview delivered {total}B != expected {expected}B"),
            )));
        }
        let measured = ContentDigest::new(DigestAlgorithm::Sha256, &hasher.finalize());

        // Publish payload then sidecar, then commit.
        let sealed = staged.seal();
        if let Err(mut failure) = sealed.publish(PublishOptions::new(
            PublishMode::Replace,
            Durability::FileSync,
        )) {
            let _ = failure.take_recovery_handle().map(|h| h.cleanup());
            return Ok(PreviewFetchOutcome::Uncached);
        }
        let digest = Some(measured);
        let meta_record = PreviewMeta {
            object_identity: pin.object_identity.clone(),
            byte_len: total,
            digest: digest.clone(),
            stored_at: cx.now(),
            format: request.format.clone(),
            region: request.region,
        };
        // TTL anchors on the cache's wall clock — provider logical
        // time (`stored_at`) is informational only.
        let committed_at = SystemTime::now();
        let sidecar = SidecarMeta::Preview {
            format_version: SIDECAR_FORMAT,
            sweep_tag: preview_sweep_tag(key, principal_tag),
            credential_epoch: key.credential_epoch,
            meta: meta_record.clone(),
            committed_at,
        };
        let sidecar_bytes = match serde_json::to_vec(&sidecar) {
            Ok(bytes) => bytes,
            Err(e) => {
                return Err(CacheError::internal(format!("sidecar encode: {e}")));
            }
        };
        if let Err(_) = self.core.publish_file(
            &dir.join(SIDECAR_NAME),
            &sidecar_bytes,
            PublishMode::Replace,
        ) {
            return Ok(PreviewFetchOutcome::Uncached);
        }
        let expires_at = committed_at
            .checked_add(self.core.policy.preview_ttl)
            .unwrap_or(committed_at);
        let entry = EntryMeta {
            rel_dir: rel_dir.clone(),
            category: CacheCategory::Preview,
            payload_bytes: total + sidecar_bytes.len() as u64,
            last_used_at: SystemTime::now(),
            expires_at,
            leases: 0,
            sweep_tag: preview_sweep_tag(key, principal_tag),
            generation: 0,
            credential_epoch: key.credential_epoch,
            principal: Some(key.principal.clone()),
            aux: EntryAux::Preview(meta_record.clone()),
        };
        match self.core.commit_entry(entry, SystemTime::now()) {
            Ok(Some(meta)) => {
                let generation = self.core.events.generation();
                self.core.events.emit(
                    generation,
                    LibraryEventKind::PreviewStored {
                        key_fp: key_fp.to_owned(),
                        byte_len: total,
                    },
                    meta_record.stored_at,
                );
                Ok(PreviewFetchOutcome::Stored(meta))
            }
            // Epoch moved mid-fetch — remove the orphaned payload
            // and serve live rather than resurrect retired data.
            Ok(None) => {
                self.core.remove_entry(&rel_dir);
                Ok(PreviewFetchOutcome::Uncached)
            }
            Err(_) => Ok(PreviewFetchOutcome::Uncached),
        }
    }
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

/// The `preview_plan` decision for `expected_size` under `policy`.
fn plan_for(
    request: &PreviewRequest,
    expected_size: Option<u64>,
    policy: &crate::CachePolicy,
) -> PreviewPlan {
    match request.region {
        Some(range) => PreviewPlan::Bounded { bytes: range.len },
        None => match expected_size {
            Some(size) if size <= policy.max_preview_object_bytes => {
                PreviewPlan::Bounded { bytes: size }
            }
            other => PreviewPlan::FullDownload { bytes: other },
        },
    }
}
