//! Shared/remote verification — untrusted manifests and blobs are
//! validated fail-closed before registration, and commit-associated
//! attach uses an external render-manifest registry that never
//! rewrites VCS history
//! (synvelis#291, RCP-02; contract: `docs/specs/render-checkpoint.md`
//! §Sharing).
//!
//! A remote artifact is *not* trusted because it arrived over a share
//! channel: manifest canonicality, identity binding, resource bounds,
//! blob digest/length, container/codec/frame verification, ACL,
//! principal provenance, expiry and revocation are all re-validated
//! against the same invariants as locally published artifacts. Blob
//! bytes are verified by re-hashing plus the caller-supplied decode
//! check; the manifest is verified by [`verify_manifest`]. Anything
//! that cannot be confirmed yields `Denied`/`Missing` — never a
//! silently degraded success.

use synvelis_project::snapshot::ContentDigest;

use crate::artifact::manifest::{
    AccessClass, ArtifactManifest, RenderArtifactId, STAGE_CHUNK_BYTES, codec_class_consistent,
    port_digest, ports_tile_channels, verify_manifest,
};
use crate::artifact::publish::{DecodedBlob, decode_blob};
use crate::artifact::store::{
    ArtifactStore, ExternalManifestEntry, PublishCommit, StoreError, valid_locator,
};

/// A fetched remote artifact — the raw bytes plus the transport
/// claims. Everything is untrusted until verified; `blob` must be the
/// *complete* download — partial downloads are rejected by the caller
/// before constructing this value.
#[derive(Clone, Debug)]
pub struct RemoteArtifact {
    /// Artifact identity claimed by the remote.
    pub artifact: RenderArtifactId,
    /// Manifest bytes as received.
    pub manifest_bytes: Vec<u8>,
    /// Blob bytes as received.
    pub blob: Option<Vec<u8>>,
    /// Transport-claimed share principal — recorded as provenance
    /// only, never trusted.
    pub principal: String,
    /// Transport-claimed expiry (store logical ticks) — informational;
    /// the manifest's own trust expiry is authoritative.
    pub expires_at: Option<u64>,
}

/// Why a remote artifact was rejected — the Issue-vocabulary reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShareRejection {
    /// Manifest bytes failed canonical verification.
    ManifestInvalid,
    /// Blob missing where the manifest requires one.
    BlobMissing,
    /// Blob digest/length contradicts the manifest.
    BlobMismatch,
    /// Blob bytes fail container/codec/frame verification.
    BlobInvalid,
    /// Manifest trust/ACL/principal/expiry/revocation insufficient.
    AccessDenied,
    /// Transport claims contradict the verified manifest.
    ClaimMismatch,
}

/// Result of registering a verified remote artifact.
#[derive(Clone, Debug)]
pub enum ShareRegistration {
    /// Artifact verified and committed into the store.
    Registered {
        /// The artifact identity.
        artifact: RenderArtifactId,
    },
    /// Already present with identical content — idempotent no-op.
    AlreadyPresent {
        /// The artifact identity.
        artifact: RenderArtifactId,
    },
    /// Rejected — fail closed, nothing registered.
    Rejected {
        /// Rejection reason.
        reason: ShareRejection,
    },
}

/// Verifies and registers a remote artifact into `store`.
///
/// Order is fixed: manifest canonical check → trust/ACL/expiry →
/// blob presence/digest/format verification → idempotence → atomic
/// commit through the same staging/CAS path as local publish. Any
/// failure leaves the store untouched; the staging slot is aborted.
///
/// # Errors
/// [`StoreError`] — store-layer failure during staging/commit; every
/// *verification* failure is reported as [`ShareRegistration::Rejected`]
/// instead, so callers can distinguish "untrusted input" from "store
/// broken".
pub fn register_remote(
    store: &mut dyn ArtifactStore,
    remote: &RemoteArtifact,
) -> Result<ShareRegistration, StoreError> {
    let Ok(manifest) = verify_manifest(&remote.artifact, &remote.manifest_bytes) else {
        return Ok(ShareRegistration::Rejected {
            reason: ShareRejection::ManifestInvalid,
        });
    };
    // Trust gates — the manifest's own trust block is authoritative;
    // transport claims are provenance only.
    let Some(trust) = &manifest.trust else {
        return Ok(ShareRegistration::Rejected {
            reason: ShareRejection::AccessDenied,
        });
    };
    if trust.revoked || trust.acl == AccessClass::None || !trust.share_verified {
        return Ok(ShareRegistration::Rejected {
            reason: ShareRejection::AccessDenied,
        });
    }
    if trust.expires_at.is_some_and(|at| at <= store.now()) {
        return Ok(ShareRegistration::Rejected {
            reason: ShareRejection::AccessDenied,
        });
    }
    // Transport expiry claim must not contradict the manifest — a
    // remote claiming *longer* validity than the manifest allows is a
    // claim mismatch; shorter is harmless (manifest still governs).
    if let (Some(claimed), Some(manifest_expiry)) = (remote.expires_at, trust.expires_at)
        && claimed > manifest_expiry
    {
        return Ok(ShareRegistration::Rejected {
            reason: ShareRejection::ClaimMismatch,
        });
    }
    // Blob verification — mandatory: a manifest without verified
    // bytes is Missing, never usable.
    let Some(blob) = &remote.blob else {
        return Ok(ShareRegistration::Rejected {
            reason: ShareRejection::BlobMissing,
        });
    };
    if blob.len() as u64 != manifest.byte_len
        || ContentDigest::sha256(blob) != manifest.content_digest
    {
        return Ok(ShareRegistration::Rejected {
            reason: ShareRejection::BlobMismatch,
        });
    }
    // Independent decode + full media consistency — the same reader
    // contract as local publish, re-measured against every manifest
    // fact the receipt claims, not just the decode scalars. A remote
    // manifest whose codec class, frame count, port layout or per-port
    // PCM digests were never actually verified is rejected here.
    match decode_blob(blob) {
        Ok(decoded) => {
            if !remote_media_consistent(&manifest, &decoded, blob) {
                return Ok(ShareRegistration::Rejected {
                    reason: ShareRejection::BlobInvalid,
                });
            }
        }
        Err(_) => {
            return Ok(ShareRegistration::Rejected {
                reason: ShareRejection::BlobInvalid,
            });
        }
    }
    // Idempotent: identical already-present content is a no-op.
    // (Manifest ID is the hash of its bytes, so presence under this ID
    // implies byte-identical content — conflicts cannot exist.)
    if store.read_manifest(&remote.artifact)?.is_some() {
        return Ok(ShareRegistration::AlreadyPresent {
            artifact: remote.artifact,
        });
    }
    // Commit through the same atomic path as local publish: stage the
    // verified blob, then one CAS commit of blob + manifest + index.
    let staging = store.stage_begin()?;
    let commit_result = (|| -> Result<(), StoreError> {
        store.stage_write(&staging, blob)?;
        store.stage_seal(&staging, blob.len() as u64)?;
        let expected_index = store.index_entries(&manifest.recipe_key)?;
        store.commit_publish(&PublishCommit {
            staging,
            blob_digest: manifest.content_digest,
            manifest_id: remote.artifact,
            manifest_bytes: remote.manifest_bytes.clone(),
            recipe_key: manifest.recipe_key,
            expected_index,
            reference: None,
        })?;
        Ok(())
    })();
    if commit_result.is_err() {
        store.stage_abort(&staging);
    }
    commit_result?;
    Ok(ShareRegistration::Registered {
        artifact: remote.artifact,
    })
}

/// `true` when every media fact the manifest declares — receipt
/// flags, decode receipt, top-level codec/frame/temporal fields,
/// codec class consistency, port layout and per-port PCM digests —
/// agrees with the independently decoded blob and its chunk
/// digests. Anything less would register a "verified" artifact
/// whose evidence was never measured.
fn remote_media_consistent(
    manifest: &ArtifactManifest,
    decoded: &DecodedBlob,
    blob: &[u8],
) -> bool {
    let receipt = &manifest.verification;
    let d = &receipt.decode;
    // The receipt must claim completed verification — a manifest
    // admitting incomplete verification is not a verified share.
    receipt.whole_digest_verified
        && receipt.byte_len == manifest.byte_len
        && d.port_digests_verified
        // Decode receipt vs independently measured blob.
        && d.container == decoded.container
        && d.codec == decoded.codec
        && d.sample_rate_hz == decoded.sample_rate_hz
        && d.channels == decoded.channels
        && d.frames == decoded.frames
        // Top-level declarations vs measured blob.
        && manifest.frame_count == decoded.frames
        && manifest.codec.container == decoded.container
        && manifest.codec.codec == decoded.codec
        && manifest.temporal.sample_rate_hz == decoded.sample_rate_hz
        && manifest.temporal.channels == decoded.channels
        && codec_class_consistent(manifest.codec.class, &decoded.codec)
        // Port layout tiles the decoded channels and every declared
        // per-port PCM digest re-measures identically.
        && ports_tile_channels(&manifest.ports, decoded.channels)
        && manifest.ports.iter().all(|port| {
            port_digest(&decoded.planes, port.channel_offset, port.channels) == port.digest
        })
        // Chunk digest evidence re-hashes identically.
        && manifest.verification.chunk_digests
            == blob
                .chunks(STAGE_CHUNK_BYTES)
                .map(ContentDigest::sha256)
                .collect::<Vec<_>>()
}

/// An external render-manifest attachment request — binds an artifact
/// to a VCS commit *without* rewriting commit history: the locator +
/// snapshot digest pair is the root of reachability for GC.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalManifestRef {
    /// Opaque commit/branch locator (provider-independent — e.g. a
    /// Git SHA or snapshot digest; validated by [`valid_locator`]).
    pub commit_locator: String,
    /// Digest of the pinned snapshot the manifest was rendered from.
    pub snapshot_digest: ContentDigest,
    /// Artifact identity the manifest attaches.
    pub artifact: RenderArtifactId,
}

/// Attaches an external manifest reference — the commit SHA stays
/// immutable; only the registry entry is added, stamped at the
/// store's current logical tick.
///
/// # Errors
/// [`StoreError::Conflict`] for an unsafe locator or a quarantined
/// artifact; [`StoreError::NotFound`] for an unknown artifact.
pub fn attach_external_manifest(
    store: &mut dyn ArtifactStore,
    reference: &ExternalManifestRef,
) -> Result<(), StoreError> {
    if !valid_locator(&reference.commit_locator) {
        return Err(StoreError::Conflict {
            detail: format!(
                "unsafe external manifest locator {:?}",
                reference.commit_locator
            ),
        });
    }
    let attached_tick = store.now();
    store.attach_external_manifest(ExternalManifestEntry {
        locator: reference.commit_locator.clone(),
        snapshot_digest: reference.snapshot_digest,
        artifact: reference.artifact,
        attached_tick,
    })
}
