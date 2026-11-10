//! Streaming verified import (ALIB-02, Synvelis #86).
//!
//! Imports a [`PinnedAssetRef`] from any [`AssetProvider`] into the
//! project's SHA-256 blob namespace with bounded memory:
//!
//! ```text
//! Started → Staged → Verified → Published → Committed
//! ```
//!
//! - **Staging** streams provider bytes into
//!   `<staging_root>/import-<id>/source.bin` while measuring SHA-256
//!   and byte length — never a whole-file `Vec`.
//! - **Verify** re-reads the staged file and re-measures, catching
//!   staging drift before anything is published.
//! - **Published** lands the bytes at
//!   `<media_root>/blobs/sha256/<64hex>` through the [`BlobPublisher`]
//!   port (`ruviform_io_file`'s `StagedFile`/`SealedFile` pipeline in
//!   production). Existing blobs are re-verified and reused; a
//!   digest-mismatched occupant is an [`ImportError::IntegrityMismatch`],
//!   never an overwrite.
//! - **Committed** applies [`CommitVerifiedImport`] — one atomic
//!   `Operation` registering asset + source + applied-import receipt
//!   (+ optional clip) at the manifest-persisted
//!   `expected_project_revision`. A stale revision is a typed
//!   [`ImportError::StaleRevision`]; the only resolution is an
//!   explicit [`rebase_import`].
//!
//! Idempotency authority is `project.applied_imports`: same
//! `ImportId` + same content replays to the same receipt
//! (`reused = true`); same `ImportId` + different content is
//! [`ImportError::IntegrityConflict`]. Crash recovery is driven by
//! the durable [`ImportManifest`]; [`inspect_imports`],
//! [`rebase_import`], and [`gc_orphans`] are the explicit recovery
//! surface. `CommittedLive` (live revision) is never reported as
//! `SavedRevision`/`HEAD` — saving stays the caller's
//! `ProjectController::save` concern.
//!
//! The committed [`Source`] state is `Imported`: this pipeline
//! verifies *byte integrity* (SHA-256 + length, staged readback,
//! published-blob re-verify), not media decodability — advancing
//! `SourceState` is a consumer's `SetSourceState` responsibility.
//!
//! Everything here is blocking, non-RT worker code: no Tokio, no
//! callbacks into real-time threads.

use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use ruviform_io_file::{
    Durability, DurabilityEvidence, PublishEffect, PublishFailureKind, PublishMode, PublishOptions,
    PublishPhase, StagedFile,
};
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use synvelis_project::{
    AppliedImport, Asset, AssetId, Clip, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command,
    IdGenerator, ImportId, LaneId, Operation, Project, ProjectController, ProjectRevision, Source,
    SourceId, SourceState, TrackId,
};

use crate::context::ProviderContext;
use crate::digest::stream_digest;
use crate::error::{ProviderError, ProviderErrorKind};
use crate::project::ProjectBlobStore;
use crate::provider::AssetProvider;
use crate::refs::{ContentDigest, DigestAlgorithm, ObjectIdentity, PinnedAssetRef};

/// Staging copy chunk size — matches the digest pipeline's bound.
const COPY_CHUNK: usize = 1 << 16;

/// Manifest file name inside each `import-<id>/` staging directory.
const MANIFEST_NAME: &str = "import-manifest.json";

/// Current manifest format version.
const MANIFEST_FORMAT: u32 = 1;

/// Where an imported clip should land — recorded in the intent and
/// enforced atomically at commit time.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportTarget {
    /// Track the lane must belong to. A lane owned by a different
    /// track is rejected — cross-track misplacement cannot commit.
    pub track_id: TrackId,
    /// Lane receiving the clip.
    pub lane_id: LaneId,
    /// Timeline placement (anchor + extent).
    pub placement: ClipPlacement,
    /// Source read window and project→source mapping.
    pub playback: ClipPlayback,
}

/// One verified-import request.
///
/// `import_id` is the idempotency key: replaying the same intent
/// converges to the same receipt; reusing the id for different
/// content is an integrity conflict. `expected_project_revision` is
/// captured into the manifest at import start and is the *only*
/// revision the commit may use — it is never silently replaced by
/// the live revision.
#[derive(Clone, Debug)]
pub struct ImportIntent {
    /// Idempotency identity (project `ImportId`).
    pub import_id: ImportId,
    /// Project revision the commit expects.
    pub expected_project_revision: ProjectRevision,
    /// The pinned provider asset to import.
    pub source: PinnedAssetRef,
    /// Optional clip placement committed atomically with the
    /// asset/source registration.
    pub target: Option<ImportTarget>,
}

/// Pipeline stage persisted in the manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportStage {
    /// Manifest reserved; source bytes not yet fully staged.
    Started,
    /// `source.bin` complete; SHA-256 + byte length measured.
    Staged,
    /// Staged bytes re-verified by readback (digest + size).
    Verified,
    /// Immutable blob published (or confirmed reused) under the
    /// SHA-256 namespace.
    Published,
    /// Live project commit applied.
    Committed,
}

/// How the verified blob landed in the store.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlobMaterialization {
    /// The blob was written and published during this import.
    Created,
    /// An identical-digest blob already satisfied the store.
    Reused,
}

/// Directory-sync outcome recorded on a publish — honest capability
/// evidence, never inferred from the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DirectorySync {
    /// File+directory sync requested and completed.
    Supported,
    /// Directory sync unsupported on this platform; file sync still
    /// applied. Recorded, never reported as "not published".
    Unsupported,
    /// Only file sync was requested.
    NotRequested,
    /// Directory sync failed after the file was published.
    Failed,
    /// Publish outcome was indeterminate — directory-sync state is
    /// unknowable and was not proven.
    Unknown,
}

/// Durable per-import job state at
/// `<staging_root>/import-<id>/import-manifest.json`.
///
/// Written atomically at every stage transition; a crash at any
/// point resumes from the last recorded stage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportManifest {
    /// Manifest format version.
    pub format_version: u32,
    /// Idempotency identity.
    pub import_id: ImportId,
    /// Reserved asset identity (drawn before any commit attempt).
    pub asset_id: AssetId,
    /// Reserved source identity.
    pub source_id: SourceId,
    /// Reserved clip identity when [`ImportIntent::target`] is set.
    pub clip_id: Option<ClipId>,
    /// Revision expectation captured at import start.
    pub expected_project_revision: ProjectRevision,
    /// The pin being imported — needed to resume the fetch.
    pub source: PinnedAssetRef,
    /// Optional clip target.
    pub target: Option<ImportTarget>,
    /// Measured content identity — `None` until staging completes.
    pub content_digest: Option<ContentDigest>,
    /// Measured byte length.
    pub byte_len: u64,
    /// Pipeline stage reached.
    pub stage: ImportStage,
    /// An indeterminate publish was observed (reconciled by stat).
    pub publish_indeterminate: bool,
    /// How the blob materialized, once published.
    pub materialized: Option<BlobMaterialization>,
    /// Achieved durability evidence label (never the request).
    pub durability: Option<String>,
    /// Directory-sync evidence.
    pub directory_sync: Option<DirectorySync>,
}

/// Filesystem layout for verified imports.
///
/// `staging_root` holds resumable `import-<id>/` job directories;
/// `media_root` holds the `blobs/` object store the
/// [`crate::LocalBlobDir`] read port is bound to.
#[derive(Clone, Debug)]
pub struct ImportStore {
    staging_root: PathBuf,
    media_root: PathBuf,
}

impl ImportStore {
    /// A store over `staging_root` + `media_root`.
    #[must_use]
    pub fn new(staging_root: impl Into<PathBuf>, media_root: impl Into<PathBuf>) -> Self {
        Self {
            staging_root: staging_root.into(),
            media_root: media_root.into(),
        }
    }

    /// The staging root.
    #[must_use]
    pub fn staging_root(&self) -> &Path {
        &self.staging_root
    }

    /// The media root.
    #[must_use]
    pub fn media_root(&self) -> &Path {
        &self.media_root
    }

    /// `<staging_root>/import-<32hex>/` for `import_id`.
    #[must_use]
    pub fn staging_dir(&self, import_id: ImportId) -> PathBuf {
        self.staging_root
            .join(format!("import-{:032x}", import_id.raw()))
    }

    /// `<media_root>/blobs` — the [`crate::LocalBlobDir`] root.
    #[must_use]
    pub fn blob_dir(&self) -> PathBuf {
        self.media_root.join("blobs")
    }

    /// `<media_root>/blobs/<alg>/<hex>` — the algorithm-namespaced
    /// blob path. SHA-256 is the only algorithm today, so this is
    /// `blobs/sha256/<64hex>`.
    #[must_use]
    pub fn blob_path(&self, digest: &ContentDigest) -> PathBuf {
        self.blob_dir()
            .join(digest.algorithm().tag())
            .join(digest.hex())
    }

    /// The [`ObjectIdentity`] under which `digest` resolves through
    /// [`crate::LocalBlobDir`] — the `"sha256:<hex>"` tagged form.
    ///
    /// # Panics
    /// Never in practice — a digest's display form is never empty.
    #[must_use]
    pub fn blob_identity(&self, digest: &ContentDigest) -> ObjectIdentity {
        ObjectIdentity::new(digest.to_string()).expect("digest display is non-empty")
    }
}

/// Durability level requested of a blob publish.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublishDurability {
    /// File contents synced before the atomic rename.
    FileSync,
    /// File sync plus post-publish directory sync where supported.
    FileAndDirectorySync,
}

/// What a successful blob publish achieved — the *observed* evidence,
/// never the requested level.
#[derive(Clone, Debug)]
pub struct BlobPublishReceipt {
    /// The published destination.
    pub destination: PathBuf,
    /// Sync work actually observed to succeed.
    pub durability: DurabilityEvidence,
    /// Directory-sync capability/failure evidence.
    pub directory_sync: DirectorySync,
}

/// A failed publish preserving the honest
/// not-published / published / indeterminate trichotomy.
#[derive(Clone, Debug)]
pub struct BlobPublishFailure {
    /// Failure class.
    pub kind: PublishFailureKind,
    /// Pipeline stage that failed.
    pub phase: PublishPhase,
    /// What the publish did to the destination.
    pub effect: PublishEffect,
    /// Sync evidence achieved before the failure.
    pub durability: DurabilityEvidence,
    /// Directory-sync evidence.
    pub directory_sync: DirectorySync,
    /// Human-readable diagnostic.
    pub detail: String,
}

/// Publish-path error.
#[derive(Debug)]
pub enum BlobPublishError {
    /// Classified publish failure.
    Publish(BlobPublishFailure),
    /// Cooperative cancel/deadline/budget during the copy.
    Interrupted(ProviderError),
    /// Unclassified I/O (temp create, source open, parent mkdir).
    Io(std::io::Error),
    /// The staged source did not match the expected digest/length —
    /// verified bytes drifted between verify and publish.
    IntegrityMismatch(String),
}

/// Write-side port for the SHA-256 blob namespace.
///
/// Implementations stream the *verified* file at `staged_source`
/// through the platform's atomic-publish primitive, re-checking
/// `expected`/`expected_len` while copying. The production adapter is
/// [`StagedBlobPublisher`]; fault-injecting doubles return scripted
/// [`BlobPublishError`]s — they prove classification, never real
/// durability.
pub trait BlobPublisher: Send + Sync {
    /// Publishes the verified file at `staged_source` to
    /// `destination` atomically at the requested durability.
    ///
    /// # Errors
    /// [`BlobPublishError`] — classified per stage; an
    /// [`PublishEffect::Indeterminate`] effect means the destination
    /// state is unknowable and must be reconciled by the caller.
    fn publish(
        &self,
        staged_source: &Path,
        expected: &ContentDigest,
        expected_len: u64,
        destination: &Path,
        durability: PublishDurability,
        cx: &ProviderContext,
    ) -> Result<BlobPublishReceipt, BlobPublishError>;
}

fn io_err(op: &'static str, e: impl std::fmt::Display) -> BlobPublishError {
    BlobPublishError::Io(std::io::Error::other(format!("{op}: {e}")))
}

/// Production [`BlobPublisher`] over `ruviform_io_file`'s
/// `StagedFile`/`SealedFile` type-state pipeline.
///
/// Publish mode is always [`PublishMode::CreateNew`] — a
/// content-addressed blob is never overwritten; an existing
/// destination surfaces as `DestinationExists`/`NotPublished` and the
/// pipeline's reuse path re-verifies instead.
#[derive(Copy, Clone, Debug, Default)]
pub struct StagedBlobPublisher;

impl StagedBlobPublisher {
    /// One publish attempt at `level`.
    #[allow(clippy::too_many_arguments)]
    fn publish_once(
        staged_source: &Path,
        expected: &ContentDigest,
        expected_len: u64,
        destination: &Path,
        level: PublishDurability,
        cx: &ProviderContext,
    ) -> Result<BlobPublishReceipt, BlobPublishError> {
        let mut input = File::open(staged_source).map_err(|e| io_err("open staged source", e))?;
        let mut staged =
            StagedFile::create(destination).map_err(|e| io_err("create staged file", e))?;
        let mut hasher = sha2::Sha256::new();
        let mut buf = vec![0u8; COPY_CHUNK];
        let mut total = 0u64;
        loop {
            cx.check_interrupts()
                .map_err(BlobPublishError::Interrupted)?;
            let n = input
                .read(&mut buf)
                .map_err(|e| io_err("read staged source", e))?;
            if n == 0 {
                break;
            }
            total += n as u64;
            hasher.update(&buf[..n]);
            staged
                .write_all(&buf[..n])
                .map_err(|e| io_err("write staged file", e))?;
        }
        staged.flush().map_err(|e| io_err("flush staged file", e))?;
        // Integrity gate before sealing: the staged source must still
        // measure exactly the verified digest/length.
        let measured = ContentDigest::new(DigestAlgorithm::Sha256, &hasher.finalize());
        if total != expected_len || measured != *expected {
            let _ = staged.abort();
            return Err(BlobPublishError::IntegrityMismatch(format!(
                "staged source drifted: {measured}/{total}B != expected {expected}/{expected_len}B"
            )));
        }
        let mut sealed = staged.seal();
        // Readback verify: the publish temp must carry the same bytes.
        let readback_ok = {
            let mut readback = sealed
                .open_readback()
                .map_err(|e| io_err("open publish readback", e))?;
            let mut reader = readback.reader();
            let (digest, bytes) = stream_digest(
                &mut reader,
                &cx.cancel,
                cx.limits.deadline,
                cx.limits.max_bytes,
            )
            .map_err(BlobPublishError::Interrupted)?;
            digest == *expected && bytes == expected_len
        };
        if !readback_ok {
            let _ = sealed.abort();
            return Err(BlobPublishError::IntegrityMismatch(
                "publish readback mismatch".into(),
            ));
        }
        let ruviform_durability = match level {
            PublishDurability::FileSync => Durability::FileSync,
            PublishDurability::FileAndDirectorySync => Durability::FileAndDirectorySync,
        };
        match sealed.publish(PublishOptions::new(
            PublishMode::CreateNew,
            ruviform_durability,
        )) {
            Ok(receipt) => Ok(BlobPublishReceipt {
                destination: receipt.destination().to_path_buf(),
                durability: receipt.durability(),
                directory_sync: match level {
                    PublishDurability::FileSync => DirectorySync::NotRequested,
                    PublishDurability::FileAndDirectorySync => DirectorySync::Supported,
                },
            }),
            Err(mut failure) => {
                // Best-effort temp cleanup — the classification is
                // already decided; the temp is never the destination.
                if let Some(handle) = failure.take_recovery_handle() {
                    let _ = handle.cleanup();
                }
                let dir_sync = match (level, failure.phase(), failure.kind()) {
                    (PublishDurability::FileSync, _, _) => DirectorySync::NotRequested,
                    (_, PublishPhase::DirectorySync, _) => DirectorySync::Failed,
                    (_, _, PublishFailureKind::Unsupported) => DirectorySync::Unsupported,
                    _ => DirectorySync::NotRequested,
                };
                Err(BlobPublishError::Publish(BlobPublishFailure {
                    kind: failure.kind(),
                    phase: failure.phase(),
                    effect: failure.effect(),
                    durability: failure.durability(),
                    directory_sync: dir_sync,
                    detail: failure.to_string(),
                }))
            }
        }
    }
}

impl BlobPublisher for StagedBlobPublisher {
    fn publish(
        &self,
        staged_source: &Path,
        expected: &ContentDigest,
        expected_len: u64,
        destination: &Path,
        durability: PublishDurability,
        cx: &ProviderContext,
    ) -> Result<BlobPublishReceipt, BlobPublishError> {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| io_err("create blob dir", e))?;
        }
        // FileAndDirectorySync falls back to FileSync only when the
        // platform reports the stronger request unsupported *before
        // publishing* — the receipt then records Unsupported honestly.
        let mut receipt = Self::publish_once(
            staged_source,
            expected,
            expected_len,
            destination,
            durability,
            cx,
        );
        if let (PublishDurability::FileAndDirectorySync, Err(BlobPublishError::Publish(f))) =
            (durability, &receipt)
            && f.kind == PublishFailureKind::Unsupported
            && f.effect == PublishEffect::NotPublished
        {
            receipt = Self::publish_once(
                staged_source,
                expected,
                expected_len,
                destination,
                PublishDurability::FileSync,
                cx,
            )
            .map(|mut r| {
                r.directory_sync = DirectorySync::Unsupported;
                r
            });
        }
        receipt
    }
}

/// Receipt of the blob publication — separate from the project
/// commit receipt per the Issue #24 contract.
#[derive(Clone, Debug)]
pub struct BlobImportReceipt {
    /// The object identity (`"sha256:<hex>"`) naming the blob.
    pub identity: ObjectIdentity,
    /// Published destination path.
    pub destination: PathBuf,
    /// Blob byte length.
    pub byte_len: u64,
    /// Created or reused.
    pub materialized: BlobMaterialization,
    /// Achieved durability evidence label (never the request).
    pub durability: String,
    /// Directory-sync evidence.
    pub directory_sync: DirectorySync,
}

/// Receipt of the expected-revision project mutation.
#[derive(Clone, Debug)]
pub struct CommitReceipt {
    /// Revision returned by `ProjectController::apply`.
    pub revision: ProjectRevision,
    /// Registered asset.
    pub asset_id: AssetId,
    /// Registered source.
    pub source_id: SourceId,
    /// Placed clip when the intent carried a target.
    pub clip_id: Option<ClipId>,
}

/// Result of [`import_verified`].
#[derive(Clone, Debug)]
pub struct ImportOutcome {
    /// Idempotency identity.
    pub import_id: ImportId,
    /// Measured SHA-256 content identity.
    pub content_digest: ContentDigest,
    /// Byte length of the verified content.
    pub byte_len: u64,
    /// Blob publication receipt.
    pub blob: BlobImportReceipt,
    /// Project mutation receipt.
    pub commit: CommitReceipt,
    /// `true` when the project registry already held this import —
    /// nothing was registered twice.
    pub reused: bool,
    /// An indeterminate publish was observed and reconciled during
    /// this run — recorded, never hidden.
    pub publish_indeterminate: bool,
    /// The committed source state — always `Imported`: byte
    /// integrity is verified here; media decode verification is a
    /// separate consumer stage.
    pub source_state: SourceState,
}

/// Typed import failure — per stage, never a silent partial state.
#[derive(Debug)]
pub enum ImportError {
    /// Provider-side failure (open/stat/pin/fetch).
    Provider(ProviderError),
    /// Filesystem failure in staging/manifest/publish-adjacent IO.
    Io {
        /// Operation tag.
        op: &'static str,
        /// Diagnostic detail.
        detail: String,
    },
    /// Persisted state failed verification (manifest parse, staged
    /// bytes drifted, existing blob mismatch is `IntegrityMismatch`).
    Corrupt {
        /// Diagnostic detail.
        detail: String,
    },
    /// Content identity mismatch — digest or byte length differed
    /// from the pin expectation or an existing blob.
    IntegrityMismatch {
        /// Diagnostic detail.
        detail: String,
    },
    /// Same `ImportId` already applied with different content.
    IntegrityConflict {
        /// The conflicting import identity.
        import_id: ImportId,
    },
    /// The manifest's `expected_project_revision` is stale — an
    /// explicit [`rebase_import`] or a new intent is required.
    StaleRevision {
        /// Revision the manifest expected.
        expected: u64,
        /// Revision actually live.
        actual: u64,
    },
    /// The project commit was rejected; the published blob is
    /// retained as a protected orphan for retry/review.
    CommitRejected {
        /// The import identity.
        import_id: ImportId,
        /// The project error.
        source: synvelis_project::Error,
    },
    /// The publish pipeline failed; `effect` says whether the
    /// destination may exist — never assume "not published".
    PublishFailed {
        /// Failure class.
        kind: PublishFailureKind,
        /// Pipeline stage that failed.
        phase: PublishPhase,
        /// Destination effect.
        effect: PublishEffect,
        /// Directory-sync evidence.
        directory_sync: DirectorySync,
        /// Diagnostic detail.
        detail: String,
    },
    /// Publish stayed indeterminate after reconciliation — inspect
    /// `destination` before retrying.
    PublishIndeterminate {
        /// The destination whose state is unknowable.
        destination: PathBuf,
    },
    /// Job-state problem (missing manifest, illegal rebase, foreign
    /// manifest).
    JobState {
        /// Diagnostic detail.
        detail: String,
    },
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(e) => write!(f, "provider: {e}"),
            Self::Io { op, detail } => write!(f, "io {op}: {detail}"),
            Self::Corrupt { detail } => write!(f, "corrupt: {detail}"),
            Self::IntegrityMismatch { detail } => write!(f, "integrity mismatch: {detail}"),
            Self::IntegrityConflict { import_id } => {
                write!(f, "import {import_id} integrity conflict")
            }
            Self::StaleRevision { expected, actual } => {
                write!(f, "stale revision: expected {expected}, live {actual}")
            }
            Self::CommitRejected { import_id, source } => {
                write!(f, "commit rejected for import {import_id}: {source}")
            }
            Self::PublishFailed {
                kind,
                phase,
                effect,
                detail,
                ..
            } => write!(f, "publish failed ({kind} at {phase}, {effect}): {detail}"),
            Self::PublishIndeterminate { destination } => {
                write!(f, "publish indeterminate: {}", destination.display())
            }
            Self::JobState { detail } => write!(f, "job state: {detail}"),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<ProviderError> for ImportError {
    fn from(e: ProviderError) -> Self {
        Self::Provider(e)
    }
}

fn io_import(op: &'static str, e: impl fmt::Display) -> ImportError {
    ImportError::Io {
        op,
        detail: e.to_string(),
    }
}

fn manifest_path(stage_dir: &Path) -> PathBuf {
    stage_dir.join(MANIFEST_NAME)
}

fn read_manifest(path: &Path) -> Result<ImportManifest, ImportError> {
    let bytes = fs::read(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ImportError::JobState {
                detail: format!("manifest {} not found", path.display()),
            }
        } else {
            io_import("read manifest", e)
        }
    })?;
    let manifest: ImportManifest =
        serde_json::from_slice(&bytes).map_err(|e| ImportError::Corrupt {
            detail: format!("manifest {}: {e}", path.display()),
        })?;
    if manifest.format_version != MANIFEST_FORMAT {
        return Err(ImportError::Corrupt {
            detail: format!(
                "manifest {} format {} unsupported",
                path.display(),
                manifest.format_version
            ),
        });
    }
    Ok(manifest)
}

/// Atomically rewrites the manifest via the staged-publish
/// primitive (`Replace` + file sync — the manifest is recoverable
/// job state, not the authoritative snapshot).
fn write_manifest(stage_dir: &Path, manifest: &ImportManifest) -> Result<(), ImportError> {
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|e| ImportError::Corrupt {
        detail: format!("manifest serialize: {e}"),
    })?;
    let path = manifest_path(stage_dir);
    let mut staged = StagedFile::create(&path).map_err(|e| io_import("stage manifest", e))?;
    staged
        .write_all(&bytes)
        .and_then(|()| staged.flush())
        .map_err(|e| io_import("write manifest", e))?;
    staged
        .seal()
        .publish(PublishOptions::new(
            PublishMode::Replace,
            Durability::FileSync,
        ))
        .map_err(|e| io_import("publish manifest", e))?;
    Ok(())
}

/// Scans every manifest under `staging_root` and returns the maximum
/// reserved raw ID — the floor new reservations must exceed so a
/// generator restored from an older save cannot re-issue IDs that
/// durable manifests still reference.
fn reservation_floor(staging_root: &Path) -> u128 {
    let mut floor = 0u128;
    let Ok(entries) = fs::read_dir(staging_root) else {
        return floor;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // The directory name alone reserves the import_id raw even
        // when the manifest is unreadable.
        if let Some(raw) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.strip_prefix("import-"))
            .and_then(|h| u128::from_str_radix(h, 16).ok())
        {
            floor = floor.max(raw);
        }
        let Ok(manifest) = read_manifest(&manifest_path(&path)) else {
            continue;
        };
        let mut max = manifest.import_id.raw();
        max = max.max(manifest.asset_id.raw());
        max = max.max(manifest.source_id.raw());
        if let Some(clip) = manifest.clip_id {
            max = max.max(clip.raw());
        }
        floor = floor.max(max);
    }
    floor
}

/// Draws IDs from `controller` until `count` values strictly greater
/// than `floor` are produced, returning them in draw order.
fn draw_reserved(controller: &mut ProjectController, floor: u128, count: usize) -> Vec<u128> {
    let mut out = Vec::with_capacity(count);
    while out.len() < count {
        let raw = controller.draw_raw_id();
        if raw > floor {
            out.push(raw);
        }
    }
    out
}

fn validate_intent(provider: &dyn AssetProvider, intent: &ImportIntent) -> Result<(), ImportError> {
    if intent.source.asset.provider != provider.id() {
        return Err(ImportError::JobState {
            detail: format!(
                "pin belongs to provider {}, not {}",
                intent.source.asset.provider,
                provider.id()
            ),
        });
    }
    Ok(())
}

/// Digests the pin expects the fetched bytes to have — the optional
/// `digest_hint` plus a `ContentHash` proof token when present.
fn expected_digests(pin: &PinnedAssetRef) -> Vec<ContentDigest> {
    let mut out = Vec::new();
    if let Some(hint) = &pin.digest_hint {
        out.push(hint.clone());
    }
    if let crate::PinProof::ConditionalStrong {
        token: crate::StrongRevisionToken::ContentHash { digest },
    } = &pin.proof
        && !out.contains(digest)
    {
        out.push(digest.clone());
    }
    out
}

/// Verifies the measured staging result against every digest/size
/// expectation the pin carries.
fn check_pin_expectations(
    pin: &PinnedAssetRef,
    digest: &ContentDigest,
    byte_len: u64,
) -> Result<(), ImportError> {
    for expected in expected_digests(pin) {
        if expected != *digest {
            return Err(ImportError::IntegrityMismatch {
                detail: format!("measured {digest} != pinned {expected}"),
            });
        }
    }
    if let Some(expected_size) = pin.expected_size
        && byte_len != expected_size
    {
        return Err(ImportError::IntegrityMismatch {
            detail: format!("byte length {byte_len} != pinned expected_size {expected_size}"),
        });
    }
    Ok(())
}

/// Stage: stream the pinned provider asset into `source.bin` while
/// measuring SHA-256 + byte length. Bounded memory; cancel/deadline/
/// byte-budget checked per chunk.
fn stage_source(
    provider: &dyn AssetProvider,
    pin: &PinnedAssetRef,
    stage_dir: &Path,
    manifest: &mut ImportManifest,
    cx: &ProviderContext,
) -> Result<(), ImportError> {
    let mut pinned = provider.open_pinned(pin, cx)?;
    let staged_path = stage_dir.join("source.bin");
    let mut out = File::create(&staged_path).map_err(|e| io_import("create staged file", e))?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; COPY_CHUNK];
    let mut total = 0u64;
    loop {
        cx.check_interrupts()?;
        match pinned.reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                total += n as u64;
                if total > cx.limits.max_bytes {
                    return Err(ProviderError::new(
                        ProviderErrorKind::BudgetExceeded,
                        format!("content exceeds byte budget {}", cx.limits.max_bytes),
                    )
                    .into());
                }
                hasher.update(&buf[..n]);
                out.write_all(&buf[..n])
                    .map_err(|e| io_import("write staged file", e))?;
            }
            Err(e) => {
                // A typed provider error may be embedded in the io
                // error or recorded in the completion slot.
                if let Some(typed) = e
                    .get_ref()
                    .and_then(|inner| inner.downcast_ref::<ProviderError>())
                {
                    return Err(ImportError::Provider(typed.clone()));
                }
                if let Some(Err(typed)) = pinned.completion.take() {
                    return Err(ImportError::Provider(typed));
                }
                return Err(io_import("stage read", e));
            }
        }
    }
    out.sync_all()
        .map_err(|e| io_import("sync staged file", e))?;
    drop(out);
    if let Some(outcome) = pinned.completion.take() {
        match outcome {
            Err(typed) => return Err(ImportError::Provider(typed)),
            Ok(receipt) => {
                // The provider's measured digest must agree with ours
                // — both are SHA-256 over the same bytes.
                if let Some(provider_digest) = receipt.digest
                    && provider_digest
                        != ContentDigest::new(DigestAlgorithm::Sha256, &hasher.clone().finalize())
                {
                    return Err(ImportError::Corrupt {
                        detail: format!(
                            "provider receipt digest {provider_digest} disagrees with staged measurement"
                        ),
                    });
                }
            }
        }
    }
    let digest = ContentDigest::new(DigestAlgorithm::Sha256, &hasher.finalize());
    check_pin_expectations(pin, &digest, total)?;
    manifest.content_digest = Some(digest);
    manifest.byte_len = total;
    manifest.stage = ImportStage::Staged;
    write_manifest(stage_dir, manifest)
}

/// Verify: re-read `source.bin` and re-measure — catches staging
/// drift before anything is published.
fn verify_staged(
    stage_dir: &Path,
    manifest: &mut ImportManifest,
    cx: &ProviderContext,
) -> Result<(), ImportError> {
    let mut file =
        File::open(stage_dir.join("source.bin")).map_err(|e| io_import("open staged file", e))?;
    let (digest, bytes) = stream_digest(
        &mut file,
        &cx.cancel,
        cx.limits.deadline,
        cx.limits.max_bytes,
    )?;
    let expected_digest = manifest
        .content_digest
        .clone()
        .ok_or_else(|| ImportError::JobState {
            detail: "verify without measured digest".into(),
        })?;
    if digest != expected_digest || bytes != manifest.byte_len {
        return Err(ImportError::Corrupt {
            detail: format!(
                "staged bytes drifted: {digest}/{bytes}B != {expected_digest}/{}B",
                manifest.byte_len
            ),
        });
    }
    manifest.stage = ImportStage::Verified;
    write_manifest(stage_dir, manifest)
}

/// Opens `identity` through the blob read port and re-measures its
/// content — the reuse/reconcile check for an existing destination.
fn verify_existing_blob(
    blobs: &dyn ProjectBlobStore,
    identity: &ObjectIdentity,
    expected: &ContentDigest,
    expected_len: u64,
    cx: &ProviderContext,
) -> Result<(), ImportError> {
    let object = blobs.open(identity)?;
    let mut reader = object.reader;
    let (digest, bytes) = stream_digest(&mut reader, &cx.cancel, cx.limits.deadline, u64::MAX)?;
    if digest != *expected || bytes != expected_len {
        return Err(ImportError::IntegrityMismatch {
            detail: format!(
                "existing blob {} measures {digest}/{bytes}B != {expected}/{expected_len}B",
                identity.as_str()
            ),
        });
    }
    Ok(())
}

/// Publish: land the verified staged bytes under the SHA-256 blob
/// namespace, or confirm an identical existing blob.
#[allow(clippy::too_many_lines)] // fault trichotomy must stay explicit
fn publish_blob(
    store: &ImportStore,
    blobs: &dyn ProjectBlobStore,
    publisher: &dyn BlobPublisher,
    stage_dir: &Path,
    manifest: &mut ImportManifest,
    cx: &ProviderContext,
) -> Result<(), ImportError> {
    let digest = manifest
        .content_digest
        .clone()
        .ok_or_else(|| ImportError::JobState {
            detail: "publish without measured digest".into(),
        })?;
    let identity = store.blob_identity(&digest);
    let destination = store.blob_path(&digest);
    let staged_source = stage_dir.join("source.bin");

    // Existing destination? Re-verify content, then reuse — never
    // overwrite and never trust the name alone.
    match blobs.stat(&identity) {
        Ok(_) => {
            verify_existing_blob(blobs, &identity, &digest, manifest.byte_len, cx)?;
            manifest.materialized = Some(BlobMaterialization::Reused);
            manifest.durability = Some("preexisting-verified".into());
            manifest.directory_sync = Some(DirectorySync::NotRequested);
            manifest.stage = ImportStage::Published;
            return write_manifest(stage_dir, manifest);
        }
        Err(e) if e.kind() == ProviderErrorKind::NotFound => {}
        Err(e) => return Err(ImportError::Provider(e)),
    }

    match publisher.publish(
        &staged_source,
        &digest,
        manifest.byte_len,
        &destination,
        PublishDurability::FileAndDirectorySync,
        cx,
    ) {
        Ok(receipt) => {
            manifest.materialized = Some(BlobMaterialization::Created);
            manifest.durability = Some(receipt.durability.to_string());
            manifest.directory_sync = Some(receipt.directory_sync);
        }
        Err(BlobPublishError::Publish(failure)) => match failure.effect {
            // The file landed; only a post-publish guarantee failed.
            // Verify the destination, then record honestly.
            PublishEffect::Published => {
                verify_existing_blob(blobs, &identity, &digest, manifest.byte_len, cx)?;
                manifest.materialized = Some(BlobMaterialization::Created);
                manifest.durability = Some(failure.durability.to_string());
                // Publish landed but a post-publish guarantee failed —
                // record exactly what the failure reported.
                manifest.directory_sync = Some(failure.directory_sync);
            }
            PublishEffect::Indeterminate => {
                manifest.publish_indeterminate = true;
                write_manifest(stage_dir, manifest)?;
                // Reconcile by observation — never assume either way.
                match blobs.stat(&identity) {
                    Ok(_) => {
                        verify_existing_blob(blobs, &identity, &digest, manifest.byte_len, cx)?;
                        manifest.materialized = Some(BlobMaterialization::Created);
                        manifest.durability = Some(failure.durability.to_string());
                        manifest.directory_sync = Some(DirectorySync::Unknown);
                    }
                    Err(e) if e.kind() == ProviderErrorKind::NotFound => {
                        // Definitively absent now — one fresh attempt.
                        match publisher.publish(
                            &staged_source,
                            &digest,
                            manifest.byte_len,
                            &destination,
                            PublishDurability::FileAndDirectorySync,
                            cx,
                        ) {
                            Ok(receipt) => {
                                manifest.materialized = Some(BlobMaterialization::Created);
                                manifest.durability = Some(receipt.durability.to_string());
                                manifest.directory_sync = Some(receipt.directory_sync);
                            }
                            Err(BlobPublishError::Publish(f2))
                                if f2.effect == PublishEffect::Indeterminate =>
                            {
                                return Err(ImportError::PublishIndeterminate { destination });
                            }
                            Err(BlobPublishError::Publish(f2)) => {
                                return Err(publish_failed(f2));
                            }
                            Err(e) => return Err(map_publish_error(e)),
                        }
                    }
                    Err(e) => return Err(ImportError::Provider(e)),
                }
            }
            _ => return Err(publish_failed(failure)),
        },
        Err(e) => return Err(map_publish_error(e)),
    }
    manifest.stage = ImportStage::Published;
    write_manifest(stage_dir, manifest)
}

fn publish_failed(f: BlobPublishFailure) -> ImportError {
    ImportError::PublishFailed {
        kind: f.kind,
        phase: f.phase,
        effect: f.effect,
        directory_sync: f.directory_sync,
        detail: f.detail,
    }
}

fn map_publish_error(e: BlobPublishError) -> ImportError {
    match e {
        BlobPublishError::Publish(f) => publish_failed(f),
        BlobPublishError::Interrupted(p) => ImportError::Provider(p),
        BlobPublishError::Io(io) => io_import("publish io", io),
        BlobPublishError::IntegrityMismatch(d) => ImportError::Corrupt { detail: d },
    }
}

/// The atomic project mutation of a verified import.
///
/// Registers the asset, the `Imported` source, the applied-import
/// receipt, and — when `target` is set — the clip, in one
/// `Operation`. `apply` reseeds the ID generator past the reserved
/// IDs when a reload left it behind, and `validate` rejects
/// reserved-ID collisions, foreign-track lanes, and illegal clip
/// contracts before anything mutates.
#[derive(Clone, Debug)]
pub struct CommitVerifiedImport {
    /// Idempotency identity reserved by the manifest.
    pub import_id: ImportId,
    /// Reserved asset identity.
    pub asset_id: AssetId,
    /// Reserved source identity.
    pub source_id: SourceId,
    /// Reserved clip identity when a target is present.
    pub clip_id: Option<ClipId>,
    /// Verified content digest (`"sha256:<hex>"`).
    pub content_digest: String,
    /// Optional clip target.
    pub target: Option<ImportTarget>,
}

impl Operation for CommitVerifiedImport {
    fn validate(&self, project: &Project) -> Result<(), synvelis_project::Error> {
        use synvelis_project::Error;
        // This op's contract is verified SHA-256 import — a legacy or
        // opaque digest must never enter the project through it
        // unverified (synvelis#86 typed-digest rule).
        if !synvelis_project::project::AssetDigest::parse(&self.content_digest)
            .is_content_addressed()
        {
            return Err(Error::Validation(
                "commit digest is not a sha256 content address".into(),
            ));
        }
        if let Some(applied) = project.applied_imports.get(&self.import_id) {
            return Err(Error::Validation(
                if applied.content_hash == self.content_digest {
                    format!("import {} already applied", self.import_id)
                } else {
                    format!("import {} integrity conflict", self.import_id)
                },
            ));
        }
        if project.assets.contains_key(&self.asset_id)
            || project.contains_source(&self.source_id)
            || self
                .clip_id
                .is_some_and(|clip| project.clips.contains_key(&clip))
        {
            return Err(Error::Validation("import reserved-id collision".into()));
        }
        if let Some(target) = &self.target {
            let lane = project
                .lanes
                .get(&target.lane_id)
                .ok_or_else(|| Error::NotFound(format!("lane {}", target.lane_id)))?;
            if lane.track_id != target.track_id {
                return Err(Error::WrongOwner(format!(
                    "lane {} belongs to track {}, not {}",
                    target.lane_id, lane.track_id, target.track_id
                )));
            }
            if !project.tracks.contains_key(&target.track_id) {
                return Err(Error::NotFound(format!("track {}", target.track_id)));
            }
            // Mirrors placement::validate_clip_contract through the
            // public surface; validate_references re-checks it as the
            // final authority inside apply.
            if target.placement.extent.is_empty() {
                return Err(Error::Validation("clip extent is empty".into()));
            }
            match &target.placement.extent {
                ClipExtent::Absolute(dur) => {
                    let frames = u64::try_from(dur.frames_at(dur.denominator()))
                        .map_err(|_| Error::Negative)?;
                    target.playback.validate_window(frames)?;
                }
                ClipExtent::Musical(_) => target.playback.validate_window(0)?,
            }
        }
        Ok(())
    }

    fn apply(
        &self,
        project: &mut Project,
        ids: &mut dyn IdGenerator,
    ) -> Result<(), synvelis_project::Error> {
        // Reseed past the reserved IDs only when the generator was
        // restored behind them (older save/undo) — never move it
        // backwards.
        let mut max_reserved = self
            .import_id
            .raw()
            .max(self.asset_id.raw())
            .max(self.source_id.raw());
        if let Some(clip) = self.clip_id {
            max_reserved = max_reserved.max(clip.raw());
        }
        let needed = max_reserved.saturating_add(1);
        if needed > ids.peek() {
            ids.reseed(needed);
        }
        project.assets.insert(
            self.asset_id,
            Asset {
                id: self.asset_id,
                digest: self.content_digest.clone(),
            },
        );
        project.sources.insert(
            self.source_id,
            Source {
                id: self.source_id,
                asset_id: self.asset_id,
                content_digest: self.content_digest.clone(),
                state: SourceState::Imported,
            },
        );
        project.applied_imports.insert(
            self.import_id,
            AppliedImport {
                content_hash: self.content_digest.clone(),
                asset_id: self.asset_id,
                source_id: self.source_id,
            },
        );
        if let (Some(target), Some(clip_id)) = (&self.target, self.clip_id) {
            project.clips.insert(
                clip_id,
                Clip {
                    id: clip_id,
                    lane_id: target.lane_id,
                    source_id: self.source_id,
                    placement: target.placement,
                    playback: target.playback,
                    muted: false,
                },
            );
            if let Some(lane) = project.lanes.get_mut(&target.lane_id) {
                lane.clip_ids.push(clip_id);
            }
        }
        Ok(())
    }
}

/// Builds the commit operation from a manifest at the Published
/// stage or later.
fn commit_op(manifest: &ImportManifest) -> Result<CommitVerifiedImport, ImportError> {
    let content_digest = manifest
        .content_digest
        .clone()
        .ok_or_else(|| ImportError::JobState {
            detail: "commit without measured digest".into(),
        })?
        .to_string();
    Ok(CommitVerifiedImport {
        import_id: manifest.import_id,
        asset_id: manifest.asset_id,
        source_id: manifest.source_id,
        clip_id: manifest.clip_id,
        content_digest,
        target: manifest.target,
    })
}

/// The registry-replay check shared by the entry fast path and the
/// post-staging path. Returns `Some(outcome)` when the import is
/// already applied — reconciling the manifest to `Committed`.
fn applied_replay(
    project: &Project,
    live_revision: ProjectRevision,
    manifest: &mut ImportManifest,
    stage_dir: &Path,
    store: &ImportStore,
) -> Result<Option<ImportOutcome>, ImportError> {
    let Some(content_digest) = manifest.content_digest.clone() else {
        return Ok(None);
    };
    let Some(applied) = project.applied_imports.get(&manifest.import_id) else {
        return Ok(None);
    };
    if applied.content_hash != content_digest.to_string() {
        return Err(ImportError::IntegrityConflict {
            import_id: manifest.import_id,
        });
    }
    // The registry is authoritative for the committed identities —
    // sync the manifest (a manifest-less replay reserves fresh IDs
    // that must not leak into the receipt). The clip is only reported
    // when this import carried a target; prefer the manifest's
    // reserved clip id, falling back to a source scan.
    manifest.asset_id = applied.asset_id;
    manifest.source_id = applied.source_id;
    manifest.clip_id = if manifest.target.is_some() {
        manifest
            .clip_id
            .filter(|clip| {
                project
                    .clips
                    .get(clip)
                    .is_some_and(|c| c.source_id == applied.source_id)
            })
            .or_else(|| {
                project
                    .clips
                    .values()
                    .find(|clip| clip.source_id == applied.source_id)
                    .map(|clip| clip.id)
            })
    } else {
        None
    };
    if manifest.stage != ImportStage::Committed {
        manifest.stage = ImportStage::Committed;
    }
    write_manifest(stage_dir, manifest)?;
    Ok(Some(outcome_from(
        manifest,
        store,
        applied.asset_id,
        applied.source_id,
        manifest.clip_id,
        live_revision,
        true,
    )))
}

/// Assembles the public outcome from manifest + receipt fields.
#[allow(clippy::too_many_arguments)]
fn outcome_from(
    manifest: &ImportManifest,
    store: &ImportStore,
    asset_id: AssetId,
    source_id: SourceId,
    clip_id: Option<ClipId>,
    revision: ProjectRevision,
    reused: bool,
) -> ImportOutcome {
    let digest = manifest
        .content_digest
        .clone()
        .expect("outcome requires measured digest");
    ImportOutcome {
        import_id: manifest.import_id,
        content_digest: digest.clone(),
        byte_len: manifest.byte_len,
        blob: BlobImportReceipt {
            identity: store.blob_identity(&digest),
            destination: store.blob_path(&digest),
            byte_len: manifest.byte_len,
            materialized: manifest.materialized.unwrap_or(BlobMaterialization::Reused),
            durability: manifest.durability.clone().unwrap_or_default(),
            directory_sync: manifest
                .directory_sync
                .unwrap_or(DirectorySync::NotRequested),
        },
        commit: CommitReceipt {
            revision,
            asset_id,
            source_id,
            clip_id,
        },
        reused,
        publish_indeterminate: manifest.publish_indeterminate,
        source_state: SourceState::Imported,
    }
}

/// Runs the verified-import pipeline against `controller`.
///
/// Order of effects: staged copy → verify → manifest → blob publish →
/// single atomic `CommitVerifiedImport`. The caller owns `save`
/// afterwards — `CommittedLive` and `SavedRevision`/`HEAD` stay
/// separate receipts.
///
/// # Errors
/// [`ImportError`] — typed per stage; no variant leaves a partial
/// project mutation. A published-but-uncommitted blob is a
/// protected orphan resumable by retrying this call.
#[allow(clippy::too_many_lines)] // linear pipeline; stages must stay ordered
pub fn import_verified(
    provider: &dyn AssetProvider,
    intent: &ImportIntent,
    store: &ImportStore,
    blobs: &dyn ProjectBlobStore,
    publisher: &dyn BlobPublisher,
    controller: &mut ProjectController,
    cx: &ProviderContext,
) -> Result<ImportOutcome, ImportError> {
    cx.check_interrupts()?;
    validate_intent(provider, intent)?;

    let stage_dir = store.staging_dir(intent.import_id);
    let manifest_file = manifest_path(&stage_dir);
    let mut manifest = match fs::metadata(&manifest_file) {
        Ok(_) => {
            let manifest = read_manifest(&manifest_file)?;
            if manifest.import_id != intent.import_id {
                return Err(ImportError::Corrupt {
                    detail: format!(
                        "manifest import_id {} != intent {}",
                        manifest.import_id, intent.import_id
                    ),
                });
            }
            // A replay must address the same object — a different pin
            // under the same ImportId is an integrity conflict.
            if manifest.source.object_identity != intent.source.object_identity
                || manifest.source.asset != intent.source.asset
            {
                return Err(ImportError::IntegrityConflict {
                    import_id: intent.import_id,
                });
            }
            manifest
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // New import: reserve IDs strictly past every durable
            // manifest reservation so a generator restored from an
            // older save cannot re-issue them. The floor is computed
            // BEFORE this job's directory exists so it cannot count
            // itself.
            let floor = reservation_floor(store.staging_root());
            // import_id itself must also exceed the floor — the caller
            // chooses it, so a colliding choice is a job-state error.
            if intent.import_id.raw() <= floor {
                return Err(ImportError::JobState {
                    detail: format!(
                        "import_id {} collides with a durable reservation (floor {floor})",
                        intent.import_id
                    ),
                });
            }
            fs::create_dir_all(&stage_dir).map_err(|e| io_import("create staging dir", e))?;
            let count = if intent.target.is_some() { 4 } else { 3 };
            let raws = draw_reserved(controller, floor, count);
            let manifest = ImportManifest {
                format_version: MANIFEST_FORMAT,
                import_id: intent.import_id,
                asset_id: AssetId::new(raws[0]),
                source_id: SourceId::new(raws[1]),
                clip_id: raws.get(3).copied().map(ClipId::new),
                expected_project_revision: intent.expected_project_revision,
                source: intent.source.clone(),
                target: intent.target,
                content_digest: None,
                byte_len: 0,
                stage: ImportStage::Started,
                publish_indeterminate: false,
                materialized: None,
                durability: None,
                directory_sync: None,
            };
            write_manifest(&stage_dir, &manifest)?;
            manifest
        }
        Err(e) => return Err(io_import("stat manifest", e)),
    };

    // Registry fast-path: digest already measured → replay cheaply.
    if let Some(outcome) = applied_replay(
        controller.live(),
        controller.live_revision(),
        &mut manifest,
        &stage_dir,
        store,
    )? {
        return Ok(outcome);
    }

    // ---- Staging -----------------------------------------------------
    if manifest.stage == ImportStage::Started {
        let pin = manifest.source.clone();
        stage_source(provider, &pin, &stage_dir, &mut manifest, cx)?;
    }
    // Post-staging replay check: catches a manifest-less replay whose
    // digest only became known after streaming.
    if let Some(outcome) = applied_replay(
        controller.live(),
        controller.live_revision(),
        &mut manifest,
        &stage_dir,
        store,
    )? {
        return Ok(outcome);
    }

    // ---- Verify ------------------------------------------------------
    if manifest.stage == ImportStage::Staged {
        verify_staged(&stage_dir, &mut manifest, cx)?;
    }

    // ---- Published ----------------------------------------------------
    if manifest.stage < ImportStage::Published {
        publish_blob(store, blobs, publisher, &stage_dir, &mut manifest, cx)?;
    }
    // The blob may have been lost between publish and commit (or the
    // manifest resumed past a GC'd blob): re-stat before committing
    // so the project never references a missing verified blob.
    {
        let digest = manifest
            .content_digest
            .clone()
            .ok_or_else(|| ImportError::JobState {
                detail: "commit without measured digest".into(),
            })?;
        let identity = store.blob_identity(&digest);
        match blobs.stat(&identity) {
            Ok(_) => {}
            Err(e) if e.kind() == ProviderErrorKind::NotFound => {
                publish_blob(store, blobs, publisher, &stage_dir, &mut manifest, cx)?;
            }
            Err(e) => return Err(ImportError::Provider(e)),
        }
    }

    // ---- Live commit --------------------------------------------------
    // Last cooperative checkpoint before the point of no return: a
    // cancel that landed during the publish tail or the blob re-stat
    // parks the job at `Published` — the verified blob stays
    // manifest-protected and a retry converges without re-publishing.
    // Once `apply` returns the commit is a fact, so no checkpoint may
    // follow it (a `Cancelled` there would misreport a committed
    // import); a crash between apply and the manifest write is
    // reconciled by `applied_replay` on the next call.
    cx.check_interrupts()?;
    let op = commit_op(&manifest)?;
    let revision = controller
        .apply(Command {
            expected_revision: ProjectRevision(manifest.expected_project_revision.0),
            op,
        })
        .map_err(|e| match e {
            synvelis_project::Error::StaleRevision { expected, actual } => {
                ImportError::StaleRevision { expected, actual }
            }
            other => ImportError::CommitRejected {
                import_id: manifest.import_id,
                source: other,
            },
        })?;
    manifest.stage = ImportStage::Committed;
    write_manifest(&stage_dir, &manifest)?;

    Ok(outcome_from(
        &manifest,
        store,
        manifest.asset_id,
        manifest.source_id,
        manifest.clip_id,
        revision,
        false,
    ))
}

/// Explicitly moves an import's project-revision expectation — the
/// only resolution for [`ImportError::StaleRevision`] on the same
/// `ImportId`. Rejected when the import is already applied.
///
/// # Errors
/// [`ImportError::JobState`] when no manifest exists or the import is
/// already applied; [`ImportError::Corrupt`] on manifest mismatch;
/// [`ImportError::Io`] on manifest write failure.
pub fn rebase_import(
    store: &ImportStore,
    import_id: ImportId,
    expected_project_revision: ProjectRevision,
    project: &Project,
) -> Result<(), ImportError> {
    if project.applied_imports.contains_key(&import_id) {
        return Err(ImportError::JobState {
            detail: format!("import {import_id} already applied"),
        });
    }
    let stage_dir = store.staging_dir(import_id);
    let manifest_file = manifest_path(&stage_dir);
    let mut manifest = read_manifest(&manifest_file).map_err(|_| ImportError::JobState {
        detail: format!("no manifest for import {import_id}"),
    })?;
    if manifest.import_id != import_id {
        return Err(ImportError::Corrupt {
            detail: "manifest import_id mismatch".into(),
        });
    }
    manifest.expected_project_revision = expected_project_revision;
    write_manifest(&stage_dir, &manifest)
}

/// One inspected import job.
#[derive(Clone, Debug)]
pub struct ImportJobInfo {
    /// Idempotency identity.
    pub import_id: ImportId,
    /// Last recorded stage.
    pub stage: ImportStage,
    /// Measured content identity once staged.
    pub content_digest: Option<ContentDigest>,
    /// Measured byte length.
    pub byte_len: u64,
    /// Persisted revision expectation.
    pub expected_project_revision: ProjectRevision,
    /// Whether the project registry already applied this import.
    pub applied: bool,
    /// An indeterminate publish was observed.
    pub publish_indeterminate: bool,
}

/// Lists every import job under the staging root with its registry
/// state — the recovery/review surface.
///
/// # Errors
/// [`ImportError::Io`] on staging-root read failures. Manifests that
/// fail to parse are reported as `Started`/empty-digest jobs rather
/// than silently dropped — they still reserve their directory.
pub fn inspect_imports(
    store: &ImportStore,
    project: &Project,
) -> Result<Vec<ImportJobInfo>, ImportError> {
    let mut jobs = Vec::new();
    let entries = match fs::read_dir(store.staging_root()) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(jobs),
        Err(e) => return Err(io_import("read staging root", e)),
    };
    for entry in entries {
        let entry = entry.map_err(|e| io_import("read staging entry", e))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(hex) = name.to_str().and_then(|n| n.strip_prefix("import-")) else {
            continue;
        };
        let Ok(raw) = u128::from_str_radix(hex, 16) else {
            continue;
        };
        let import_id = ImportId::new(raw);
        let manifest = read_manifest(&manifest_path(&path)).ok();
        jobs.push(ImportJobInfo {
            import_id,
            stage: manifest.as_ref().map_or(ImportStage::Started, |m| m.stage),
            content_digest: manifest.as_ref().and_then(|m| m.content_digest.clone()),
            byte_len: manifest.as_ref().map_or(0, |m| m.byte_len),
            expected_project_revision: manifest
                .as_ref()
                .map_or(ProjectRevision(0), |m| m.expected_project_revision),
            applied: project.applied_imports.contains_key(&import_id),
            publish_indeterminate: manifest.as_ref().is_some_and(|m| m.publish_indeterminate),
        });
    }
    jobs.sort_by_key(|job| job.import_id.raw());
    Ok(jobs)
}

/// Why a blob was retained by GC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobRetainReason {
    /// Referenced by a project asset digest.
    ReferencedByProject,
    /// Named by an active (non-deleted) import manifest.
    ActiveImport,
}

/// Why a staging directory was retained by GC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StagingRetainReason {
    /// Stage before `Committed` — resumable job state.
    Incomplete,
    /// `Committed` but the project registry no longer applies it
    /// (e.g. undone) — needed for replay/re-commit.
    CommittedNotApplied,
    /// Manifest unreadable — never auto-delete what cannot be
    /// understood.
    UnreadableManifest,
}

/// Result of [`gc_orphans`].
#[derive(Clone, Debug, Default)]
pub struct OrphanGcReport {
    /// Deleted blob identities in sha256-namespaced form.
    pub deleted_blobs: Vec<String>,
    /// Retained blobs with reasons.
    pub retained_blobs: Vec<(String, BlobRetainReason)>,
    /// Entries under `blobs/sha256/` that are not `<64hex>` files —
    /// never touched (foreign names, temps, dirs).
    pub skipped_foreign_blobs: usize,
    /// Deleted staging directories' import IDs.
    pub deleted_staging: Vec<ImportId>,
    /// Retained staging directories with reasons.
    pub retained_staging: Vec<(ImportId, StagingRetainReason)>,
}

/// One scanned staging job: directory name → import id, plus the
/// parsed manifest when readable.
struct JobDir {
    import_id: ImportId,
    path: PathBuf,
    manifest: Option<ImportManifest>,
}

/// Explicit orphan GC — the only deletion path.
///
/// Reachability: a `blobs/sha256/<hex>` file is protected when its
/// `"sha256:<hex>"` identity appears in a project asset digest or in
/// any surviving manifest. A staging directory is deleted only when
/// its manifest reached `Committed` *and* the project registry
/// applies the import. Nothing else is touched — legacy flat blobs,
/// foreign names, and unreadable manifests are always retained.
///
/// # Errors
/// [`ImportError::Io`] on filesystem failures; GC is re-runnable.
pub fn gc_orphans(store: &ImportStore, project: &Project) -> Result<OrphanGcReport, ImportError> {
    let mut report = OrphanGcReport::default();
    let applied: BTreeSet<ImportId> = project.applied_imports.keys().copied().collect();

    // Scan staging jobs once — manifests protect both their blobs and
    // their directories.
    let mut jobs: Vec<JobDir> = Vec::new();
    let project_refs: BTreeSet<String> = project
        .assets
        .values()
        .map(|asset| asset.digest.clone())
        .collect();
    let mut manifest_digests: BTreeSet<String> = BTreeSet::new();
    match fs::read_dir(store.staging_root()) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(|e| io_import("read staging entry", e))?;
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let Some(raw) = entry
                    .file_name()
                    .to_str()
                    .and_then(|n| n.strip_prefix("import-"))
                    .and_then(|h| u128::from_str_radix(h, 16).ok())
                else {
                    continue;
                };
                let import_id = ImportId::new(raw);
                let manifest = read_manifest(&manifest_path(&path))
                    .ok()
                    .filter(|m| m.import_id == import_id);
                if let Some(m) = &manifest
                    && let Some(digest) = &m.content_digest
                {
                    manifest_digests.insert(digest.to_string());
                }
                jobs.push(JobDir {
                    import_id,
                    path,
                    manifest,
                });
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_import("read staging root", e)),
    }
    // Blob sweep — only inside the sha256 namespace directory; legacy
    // flat blobs and foreign names are never touched.
    let sha_dir = store.blob_dir().join(DigestAlgorithm::Sha256.tag());
    if let Ok(entries) = fs::read_dir(&sha_dir) {
        for entry in entries {
            let entry = entry.map_err(|e| io_import("read blob dir", e))?;
            let name = entry.file_name();
            let is_hex = name
                .to_str()
                .is_some_and(|t| t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit()));
            if !is_hex || !entry.file_type().is_ok_and(|t| t.is_file()) {
                report.skipped_foreign_blobs += 1;
                continue;
            }
            let text = name.to_string_lossy().to_ascii_lowercase();
            let identity = format!("{}:{text}", DigestAlgorithm::Sha256.tag());
            let reason = if project_refs.contains(&identity) {
                Some(BlobRetainReason::ReferencedByProject)
            } else if manifest_digests.contains(&identity) {
                Some(BlobRetainReason::ActiveImport)
            } else {
                None
            };
            if let Some(r) = reason {
                report.retained_blobs.push((identity, r));
            } else {
                fs::remove_file(entry.path()).map_err(|e| io_import("delete orphan blob", e))?;
                report.deleted_blobs.push(identity);
            }
        }
    }

    // Staging sweep — delete only confirmed committed+applied jobs.
    for job in &jobs {
        match &job.manifest {
            Some(m) if m.stage == ImportStage::Committed && applied.contains(&job.import_id) => {
                fs::remove_dir_all(&job.path).map_err(|e| io_import("delete staging dir", e))?;
                report.deleted_staging.push(job.import_id);
            }
            Some(m) => report.retained_staging.push((
                job.import_id,
                if m.stage == ImportStage::Committed {
                    StagingRetainReason::CommittedNotApplied
                } else {
                    StagingRetainReason::Incomplete
                },
            )),
            None => report
                .retained_staging
                .push((job.import_id, StagingRetainReason::UnreadableManifest)),
        }
    }

    report.deleted_blobs.sort();
    report.retained_blobs.sort_by(|a, b| a.0.cmp(&b.0));
    report.deleted_staging.sort_by_key(|id| id.raw());
    report.retained_staging.sort_by_key(|(id, _)| id.raw());
    Ok(report)
}
