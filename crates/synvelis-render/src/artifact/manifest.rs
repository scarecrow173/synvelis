//! Immutable Render Artifact manifest — identity, content fields and
//! canonical encoding (synvelis#291, RCP-02; contract:
//! `docs/specs/render-checkpoint.md` §Artifact Publish/Retention).
//!
//! A [`RenderArtifactId`] is `SHA-256(domain || canonical_manifest_bytes)`
//! — it binds the immutable output content hash, container/codec format
//! and manifest schema version recorded inside the manifest, plus the
//! recipe key and renderer provenance. Two publishes that differ in any
//! hashed field (different PCM, different recipe, different backend
//! provenance) produce different artifact IDs; identical publishes are
//! idempotent. The *blob* namespace is content-addressed separately —
//! blob dedup never implies recipe or artifact identity (RCP-STORE-03).
//!
//! The manifest is the only persisted description of an artifact. It
//! carries no wall-clock or store-local fields, so canonical bytes —
//! and therefore the artifact ID — are a pure function of content.
//! `verified_tick`/`published_tick` style evidence lives on the store
//! record, not here.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use synvelis_player::rcp::model::ReproducibilityClass;
use synvelis_project::{SemanticPortId, SnapshotIdentity, snapshot::ContentDigest};

use crate::recipe::{
    OutputPortContract, PcmFormat, RecipeKey, RecipeTarget, RenderBoundary, RendererProfile,
    TemporalDomain,
};

/// Wire/schema version of [`ArtifactManifest`]. A version change yields
/// different canonical bytes — old artifact IDs are never reinterpreted
/// (same rule as [`crate::recipe::RECIPE_SCHEMA_VERSION`]).
pub const ARTIFACT_MANIFEST_VERSION: u32 = 1;

/// Domain separator prepended to canonical manifest bytes before the
/// artifact-ID hash — cross-protocol digest collisions are impossible
/// by construction.
pub const ARTIFACT_ID_DOMAIN: &[u8] = b"synvelis/render-artifact/v1\0";

/// Hash algorithm identifier recorded for artifact identity.
pub const ARTIFACT_HASH_ALGORITHM: &str = "sha-256";

/// Staging/readback chunk width used by the verified-publish pipeline
/// — per-chunk digests are recorded as receipt evidence.
pub const STAGE_CHUNK_BYTES: usize = 64 * 1024;

/// Maximum ports a manifest may declare — remote-offer bound.
pub const MAX_ARTIFACT_PORTS: usize = 64;

/// Maximum frames a manifest may declare — remote-offer bound.
pub const MAX_ARTIFACT_FRAMES: u64 = 1 << 40;

/// Maximum manifest byte length accepted from untrusted sources.
pub const MAX_MANIFEST_BYTES: usize = 1 << 20;

/// Maximum blob byte length accepted from untrusted sources.
pub const MAX_BLOB_BYTES: u64 = 1 << 34;

// ---------------------------------------------------------------------------
// artifact identity
// ---------------------------------------------------------------------------

/// Immutable artifact identity — `SHA-256(ARTIFACT_ID_DOMAIN ||
/// canonical_manifest_bytes)`.
///
/// The ID is derived, never assigned: it cannot be chosen independently
/// of content, so a store key collision with different bytes is
/// detectable by recomputation. Equality is byte equality of the
/// canonical manifest — including recipe key, boundary, temporal domain,
/// codec class and renderer provenance.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RenderArtifactId(pub ContentDigest);

impl RenderArtifactId {
    /// Computes the artifact ID over canonical manifest bytes.
    #[must_use]
    pub fn of_manifest_bytes(bytes: &[u8]) -> Self {
        let mut input = Vec::with_capacity(ARTIFACT_ID_DOMAIN.len() + bytes.len());
        input.extend_from_slice(ARTIFACT_ID_DOMAIN);
        input.extend_from_slice(bytes);
        Self(ContentDigest::sha256(&input))
    }

    /// The wrapped content digest.
    #[must_use]
    pub const fn digest(&self) -> ContentDigest {
        self.0
    }
}

impl fmt::Display for RenderArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl FromStr for RenderArtifactId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

// ---------------------------------------------------------------------------
// manifest field types
// ---------------------------------------------------------------------------

/// One ordered output port of the artifact — the channel mapping into
/// the decoded stream plus the port's own content digest.
///
/// `digest` is `SHA-256` over the port's canonical `f32` planar PCM:
/// for each channel in `[channel_offset, channel_offset + channels)`
/// order, `frames` little-endian `f32` samples, channel-major. The
/// recipe is fixed so independent verifiers reproduce it exactly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactPort {
    /// The semantic output port this signal replaces.
    pub port: SemanticPortId,
    /// Channel count carried by this port.
    pub channels: u32,
    /// First channel index inside the decoded stream — ports must tile
    /// `[0, total_channels)` contiguously in declaration order.
    pub channel_offset: u32,
    /// PCM layout declared for this port.
    pub format: PcmFormat,
    /// Per-port content digest (see type docs for the recipe).
    pub digest: ContentDigest,
}

/// Codec/lossless quality class of the artifact blob.
///
/// `Lossless` is required for `EligibleExact`; `Lossy` and `Probe`
/// cap eligibility at `EligiblePreview` — a probe or lossy artifact is
/// never silently reused as exact renderer output.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodecClass {
    /// Bit-exact codec family (PCM/FLAC/ALAC-class).
    Lossless,
    /// Lossy codec — preview/tolerance use only.
    Lossy,
    /// Explicit comparison probe — never promotes to Exact.
    Probe,
}

/// Container/codec description recorded in the manifest. `container`
/// and `codec` are stable lowercase identifiers (`"wav"`, `"pcm"`,
/// `"flac"`, …) matching the independent-reader observation recorded
/// in [`DecodeReceipt`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodecDescriptor {
    /// Container family (`"wav"`, `"flac"`, …).
    pub container: String,
    /// Codec identifier within the container (`"pcm"`, `"mp3"`, …).
    pub codec: String,
    /// Lossless/lossy/probe class — the Exact gate reads this, never
    /// the codec string alone.
    pub class: CodecClass,
}

/// Reproducibility class persisted with the artifact — the render-side
/// serde mirror of
/// [`synvelis_player::rcp::model::ReproducibilityClass`]. The two enums
/// are kept distinct because the player vocabulary is deliberately not
/// serializable; [`ArtifactReproducibility::to_player`] and
/// [`ArtifactReproducibility::from_player`] are the only conversion
/// points and are exercised by tests.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArtifactReproducibility {
    /// Bit-identical reproduction proven.
    BitExactCertified,
    /// Verified within a defined tolerance.
    VerifiedWithinTolerance,
    /// Replay of a verified artifact (sharing path, not re-render).
    ReplayOfVerifiedArtifact,
    /// Preview use only — never promotes to Exact.
    PreviewOnly,
    /// Unclassified — never valid for Exact.
    Unknown,
}

impl ArtifactReproducibility {
    /// The RCP-04 player-side vocabulary value.
    #[must_use]
    pub const fn to_player(self) -> ReproducibilityClass {
        match self {
            Self::BitExactCertified => ReproducibilityClass::BitExactCertified,
            Self::VerifiedWithinTolerance => ReproducibilityClass::VerifiedWithinTolerance,
            Self::ReplayOfVerifiedArtifact => ReproducibilityClass::ReplayOfVerifiedArtifact,
            Self::PreviewOnly => ReproducibilityClass::PreviewOnly,
            Self::Unknown => ReproducibilityClass::Unknown,
        }
    }

    /// Inverse of [`Self::to_player`].
    #[must_use]
    pub const fn from_player(class: ReproducibilityClass) -> Self {
        match class {
            ReproducibilityClass::BitExactCertified => Self::BitExactCertified,
            ReproducibilityClass::VerifiedWithinTolerance => Self::VerifiedWithinTolerance,
            ReproducibilityClass::ReplayOfVerifiedArtifact => Self::ReplayOfVerifiedArtifact,
            ReproducibilityClass::PreviewOnly => Self::PreviewOnly,
            ReproducibilityClass::Unknown => Self::Unknown,
        }
    }
}

/// Non-authority provenance of the producing render — recorded for
/// audit, deliberately *not* a per-job identifier: identical content
/// under identical conditions deduplicates to one artifact ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactProvenance {
    /// Producing pipeline tag (e.g. `"synvelis-render/offline-export"`).
    pub produced_by: String,
    /// Optional job/run identifier when the caller wants job-level
    /// audit — informational only, never part of identity semantics
    /// beyond canonical bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
}

/// Access class recorded for shared/imported artifacts — the ACL the
/// artifact was published under. `None` denies all read use.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccessClass {
    /// Read/play permitted for the named principal scope.
    Read,
    /// No read permission — lookup must report `Denied`.
    None,
}

/// Trust metadata for artifacts that arrived through a shared/remote
/// path. `None` on the manifest means a locally produced artifact —
/// `TrustScope::Shared` lookups then fail closed (`Denied`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactTrust {
    /// Principal the artifact was shared to/from.
    pub principal: String,
    /// ACL under which the share was published.
    pub acl: AccessClass,
    /// Expiry tick (producer-local logical clock) — `None` = no expiry.
    /// Compared against the *verifying* store's clock only as evidence;
    /// cross-store clocks are not synchronized, so remote offers carry
    /// their own `now` at verification time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,
    /// Revocation flag — a revoked artifact is `Denied`, never played.
    #[serde(default)]
    pub revoked: bool,
    /// Whether the share channel asserted verified transport. Without
    /// it the artifact is untrusted input and `TrustScope::Shared`
    /// lookups deny.
    #[serde(default)]
    pub share_verified: bool,
}

/// What the independent decode/readback actually established — the
/// verification receipt persisted with the manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodeReceipt {
    /// Container family the independent reader identified.
    pub container: String,
    /// Codec identifier the independent reader identified.
    pub codec: String,
    /// Canonical decoded sample rate.
    pub sample_rate_hz: u32,
    /// Canonical decoded channel count.
    pub channels: u32,
    /// Decoded frame count.
    pub frames: u64,
    /// Whether every declared per-port digest was recomputed from the
    /// decoded planes and matched.
    pub port_digests_verified: bool,
}

/// Verification receipt — evidence that stage → seal → readback →
/// independent decode ran and passed at publish time. Lookup treats a
/// manifest whose receipt is incomplete as `Unverified`, never as
/// media-verified.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationReceipt {
    /// Whole-file digest was recomputed from readback bytes and
    /// matched the staged content.
    pub whole_digest_verified: bool,
    /// Byte length of the verified blob.
    pub byte_len: u64,
    /// Per-`STAGE_CHUNK_BYTES` chunk digests of the readback bytes.
    pub chunk_digests: Vec<ContentDigest>,
    /// Independent decode/readback evidence.
    pub decode: DecodeReceipt,
    /// Whether caller PCM comparison ran and passed (`false` = format
    /// -level verification only — still `VerifiedMedia`, never an
    /// Exact *re-render* claim by itself).
    pub pcm_compared: bool,
}

// ---------------------------------------------------------------------------
// the manifest
// ---------------------------------------------------------------------------

/// The immutable artifact manifest — every field required by the
/// RCP-02 contract: recipe key, boundary semantic reference (inside
/// `target`), ordered output ports/channel mapping, half-open time
/// range plus pre-roll/cut/tail/latency/PDC (inside `temporal` +
/// `cut_frames`/`latency_frames`), sample format/rate (inside
/// `temporal`), codec/lossless quality, frame count, per-port digests,
/// whole-file digest, renderer capability/provenance, verification
/// receipt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactManifest {
    /// [`ARTIFACT_MANIFEST_VERSION`].
    pub schema_version: u32,
    /// Recipe this artifact was rendered under — `RecipeKey` is a
    /// processing-condition identity, not PCM proof.
    pub recipe_key: RecipeKey,
    /// Boundary semantic reference + project identity + branch pin.
    pub target: RecipeTarget,
    /// Immutable project snapshot the render resolved from, when the
    /// caller pinned one (authority provenance, non-authoritative).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<SnapshotIdentity>,
    /// Temporal domain: `[start,end)` interval, pre-roll, warmup, tail,
    /// lookahead/PDC, clock, loop, seek — sample rate/channels/format
    /// live here too.
    pub temporal: TemporalDomain,
    /// Frames cut from the head of the source interval (cut-in).
    pub cut_frames: u64,
    /// Effective output latency in frames (PDC-realigned).
    pub latency_frames: u64,
    /// Ordered output ports with channel mapping and per-port digests.
    pub ports: Vec<ArtifactPort>,
    /// Container/codec/lossless class.
    pub codec: CodecDescriptor,
    /// Emitted frame count (must equal `temporal.interval` length minus
    /// tail handling — recorded as measured, verified at publish).
    pub frame_count: u64,
    /// Whole-blob byte length.
    pub byte_len: u64,
    /// Whole-file `SHA-256` content digest — the blob's CAS key.
    pub content_digest: ContentDigest,
    /// Renderer capability/provenance profile.
    pub renderer: RendererProfile,
    /// Reproducibility class — no auto-promotion across classes.
    pub reproducibility: ArtifactReproducibility,
    /// Verification receipt from the publish pipeline.
    pub verification: VerificationReceipt,
    /// Producing-pipeline provenance.
    pub provenance: ArtifactProvenance,
    /// Shared/remote trust metadata — `None` for local artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<ArtifactTrust>,
}

/// Why manifest bytes fail to decode or verify.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    /// JSON/schema decode failure.
    Decode {
        /// Parser detail.
        detail: String,
    },
    /// `schema_version` is not [`ARTIFACT_MANIFEST_VERSION`] — old and
    /// new manifests are never reinterpreted under foreign rules.
    UnsupportedVersion {
        /// Version found.
        found: u32,
    },
    /// Recomputed artifact ID differs from the store key — content
    /// addressing violation; fail closed.
    IdMismatch,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode { detail } => write!(f, "manifest decode failed: {detail}"),
            Self::UnsupportedVersion { found } => {
                write!(f, "unsupported manifest schema version {found}")
            }
            Self::IdMismatch => write!(f, "artifact id does not match manifest bytes"),
        }
    }
}

impl std::error::Error for ManifestError {}

impl ArtifactManifest {
    /// Deterministic canonical bytes — `serde_json` of the manifest;
    /// every collection is canonically ordered at construction.
    ///
    /// # Errors
    /// `Err` on serialization failure (unreachable for this type —
    /// surfaced so callers never unwrap a digest input).
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// The artifact ID this manifest hashes to.
    ///
    /// # Errors
    /// `Err` when canonical serialization fails.
    pub fn artifact_id(&self) -> Result<RenderArtifactId, serde_json::Error> {
        Ok(RenderArtifactId::of_manifest_bytes(
            &self.canonical_bytes()?,
        ))
    }
}

/// Decodes canonical manifest bytes — schema version must be current.
///
/// # Errors
/// [`ManifestError::Decode`] on malformed bytes,
/// [`ManifestError::UnsupportedVersion`] on schema skew.
pub fn decode_manifest(bytes: &[u8]) -> Result<ArtifactManifest, ManifestError> {
    let manifest: ArtifactManifest =
        serde_json::from_slice(bytes).map_err(|e| ManifestError::Decode {
            detail: e.to_string(),
        })?;
    if manifest.schema_version != ARTIFACT_MANIFEST_VERSION {
        return Err(ManifestError::UnsupportedVersion {
            found: manifest.schema_version,
        });
    }
    Ok(manifest)
}

/// Decodes and verifies that `bytes` really are the content addressed
/// by `id` — the store-key/content consistency check every read path
/// runs before trusting a manifest.
///
/// # Errors
/// [`ManifestError::Decode`], [`ManifestError::UnsupportedVersion`],
/// [`ManifestError::IdMismatch`].
pub fn verify_manifest(
    id: &RenderArtifactId,
    bytes: &[u8],
) -> Result<ArtifactManifest, ManifestError> {
    let manifest = decode_manifest(bytes)?;
    if RenderArtifactId::of_manifest_bytes(bytes) != *id {
        return Err(ManifestError::IdMismatch);
    }
    Ok(manifest)
}

/// Codec identifiers considered lossless for [`CodecClass::Lossless`]
/// consistency checks.
const LOSSLESS_CODECS: &[&str] = &["pcm", "flac", "alac"];

/// `true` when the codec class declaration is consistent with the
/// codec identity — `Lossless` requires a lossless codec family,
/// `Lossy` a non-lossless one, `Probe` is a usage marker with no
/// codec constraint (the codec string must still match).
///
/// Enforced at publish, re-verified on the shared/remote path and
/// re-checked at lookup: a manifest whose class contradicts its
/// codec identity is an integrity contradiction, never
/// `EligibleExact`.
#[must_use]
pub fn codec_class_consistent(class: CodecClass, codec: &str) -> bool {
    let lossless = LOSSLESS_CODECS.contains(&codec);
    match class {
        CodecClass::Lossless => lossless,
        CodecClass::Lossy => !lossless,
        CodecClass::Probe => true,
    }
}

/// Computes the per-port digest recipe documented on [`ArtifactPort`]:
/// `SHA-256` over each channel's `frames` little-endian `f32` samples,
/// channel-major, for channels `[offset, offset + channels)`.
#[must_use]
pub fn port_digest(planes: &[Vec<f32>], offset: u32, channels: u32) -> ContentDigest {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    let start = offset as usize;
    let end = start + channels as usize;
    for plane in planes.iter().take(end).skip(start) {
        for sample in plane {
            hasher.update(sample.to_le_bytes());
        }
    }
    ContentDigest::sha256(&hasher.finalize())
}

/// `true` when `port` declarations form a valid tiling of
/// `[0, total_channels)` in declaration order — the invariant publish
/// and share verification both enforce.
#[must_use]
pub fn ports_tile_channels(ports: &[ArtifactPort], total_channels: u32) -> bool {
    let mut expected = 0u32;
    for port in ports {
        if port.channel_offset != expected || port.channels == 0 {
            return false;
        }
        expected = match expected.checked_add(port.channels) {
            Some(next) => next,
            None => return false,
        };
    }
    expected == total_channels
}

/// Draft-side port declaration — identical to [`ArtifactPort`] minus
/// the digest, which publish measures from decoded PCM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactPortSpec {
    /// Semantic output port.
    pub port: SemanticPortId,
    /// Channel count.
    pub channels: u32,
    /// First channel index in the decoded stream.
    pub channel_offset: u32,
    /// PCM layout.
    pub format: PcmFormat,
}

impl ArtifactPortSpec {
    /// Converts to the contract form used by [`crate::recipe::OutputContract`]
    /// comparisons.
    #[must_use]
    pub fn contract(&self) -> OutputPortContract {
        OutputPortContract {
            port: self.port,
            channels: self.channels,
            format: self.format,
        }
    }
}

/// The boundary reference carried inside the manifest — re-exported
/// for callers building drafts without importing `recipe` directly.
pub type BoundaryRef = RenderBoundary;
