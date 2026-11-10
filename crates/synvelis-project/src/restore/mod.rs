//! VCS-05 Restore As New Copy — headless slice (synvelis#96).
//!
//! Restores a fully resolved [`ImmutableSnapshot`] into a NEW,
//! independent project bundle at a caller-chosen destination: new
//! [`ProjectId`], preserved stable IDs, copied content-addressed
//! asset blobs, optional verbatim plugin-state copy, crash-safe
//! staged publication, and a durable [`RestoreReceipt`].
//!
//! Hard guarantees:
//!
//! - The source project, its Saved HEAD, the Git index, and the Git
//!   working tree are never touched — restore only *reads* snapshot
//!   projections and blob/plugin bytes through ports.
//! - The destination must not exist (`Absent` expectation only); a
//!   case-conflicting sibling, symlink, traversal attempt, or
//!   non-absolute path fails before any write.
//! - Missing/`Unknown` media fails closed unless
//!   [`MissingAssetsPolicy::AllowDegraded`] is explicitly selected —
//!   and then the receipt marks the copy degraded.
//! - Publication is staged-then-rename: an incomplete output is never
//!   reported as success, and indeterminate effects are reconciled by
//!   inspection rather than assumed.
//!
//! Out of scope (synvelis#97+): real OS filesystem durability
//! certification, real Git/provider E2E, GUI, RT, plugin-host
//! restore-into-engine, and `AudioLens` cross-consumer gates.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use ruviform_io_file::{Durability, PublishEffect, PublishMode, PublishOptions, StagedFile};

use crate::{
    ProjectId, ProjectRevision,
    plugin_state::{
        DirectoryPluginStateStore, PluginStateManifest, PluginStateOutcome, PluginStateStore,
        digest_hex, is_manifest_name,
    },
    project::Project,
    snapshot::{
        BlobRead, ContentDigest, ContentStatus, FixedSnapshotLocator, ImmutableSnapshot,
        SnapshotCancel,
    },
    store::{SCHEMA_VERSION, SnapshotEnvelope, snapshot_name, snapshot_revision},
};

// ---------------------------------------------------------------------------
// request / policy / ports
// ---------------------------------------------------------------------------

/// Asset-availability policy for restore.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MissingAssetsPolicy {
    /// Any `Missing`/`Unknown` asset aborts the restore — the default.
    Fail,
    /// `Missing`/`Unknown` assets are skipped; the copy is marked
    /// degraded in the receipt and must never be reported complete.
    AllowDegraded,
}

/// A restore request. `snapshot` must already be resolved through
/// `SnapshotResolver` — the engine trusts its validation/completeness
/// classification and re-verifies digests during materialization.
#[derive(Clone, Debug)]
pub struct RestoreRequest {
    /// Idempotency key — also the staging-directory discriminator.
    /// Filename-safe characters only (`[A-Za-z0-9._-]`).
    pub restore_id: String,
    /// The resolved source snapshot to copy.
    pub snapshot: ImmutableSnapshot,
    /// Absolute destination directory path. Must NOT exist; the
    /// parent must exist and canonicalize cleanly.
    pub output_dir: PathBuf,
    /// The new project identity for the copy (caller-drawn).
    pub new_project_id: ProjectId,
    /// Missing/unknown asset policy.
    pub missing_assets: MissingAssetsPolicy,
}

/// Read port for content-addressed asset/plugin blobs.
///
/// Production adapters wrap `ProjectBlobStore` (local bundles) or a
/// `VcsBytesPort` (VCS-located snapshots); tests inject faults.
pub trait RestoreBlobSource {
    /// Reads `digest` (`"sha256:<hex>"` form). `Absent`/`Unknown`
    /// classify honestly — never coerced to present.
    ///
    /// # Errors
    /// Transport/read failure detail.
    fn read_blob(&self, digest: &str, cancel: &SnapshotCancel) -> Result<BlobRead, String>;

    /// Best-effort byte size for disk preflight; `None` = unknown.
    ///
    /// # Errors
    /// Transport/stat failure detail.
    fn stat_blob(&self, digest: &str) -> Result<Option<u64>, String>;
}

/// Read port for the source project's plugin-state store (manifest +
/// content-addressed blobs). `None` in [`RestorePorts`] means no
/// plugin state is copied — required (and thus an error) only when
/// the project actually has routing processors.
pub trait RestorePluginStateSource {
    /// `(manifest_name, manifest_bytes)` at the source HEAD, or
    /// `None` when no manifest was ever saved.
    ///
    /// # Errors
    /// Read/transport failure detail.
    fn head_manifest(&self) -> Result<Option<(String, Vec<u8>)>, String>;

    /// Reads a plugin-state blob by content digest.
    ///
    /// # Errors
    /// Read/transport failure detail.
    fn read_blob(&self, digest: &ContentDigest) -> Result<Vec<u8>, String>;
}

/// Optional free-space probe for the destination parent.
pub trait RestoreStorageProbe {
    /// Available bytes at `dir`; `None` = cannot determine (the
    /// restore proceeds — write failures still fail honestly).
    ///
    /// # Errors
    /// Probe failure detail.
    fn available_bytes(&self, dir: &Path) -> Result<Option<u64>, String>;
}

/// Ports the restore engine needs beyond the request.
pub struct RestorePorts<'a> {
    /// Asset blob read/stat port — required.
    pub blobs: &'a dyn RestoreBlobSource,
    /// Plugin-state read port — required iff the project has
    /// processors, otherwise optional.
    pub plugin_state: Option<&'a dyn RestorePluginStateSource>,
    /// Optional disk-space probe.
    pub storage: Option<&'a dyn RestoreStorageProbe>,
}

// ---------------------------------------------------------------------------
// errors / receipts
// ---------------------------------------------------------------------------

/// Fail-closed restore error. No variant ever reports a partial or
/// unverified output as success.
#[derive(Clone, Debug, PartialEq)]
pub enum RestoreError {
    /// Cooperative cancellation.
    Cancelled,
    /// The request itself is malformed (bad `restore_id`, non-absolute
    /// or unsafe destination, non-sha256 asset digest, ...).
    InvalidRequest {
        /// What was wrong.
        detail: String,
    },
    /// The destination already exists (or a case-conflicting sibling
    /// does) — `Absent` is the only supported expectation.
    DestinationExists {
        /// The conflicting path/name.
        detail: String,
    },
    /// The destination parent is missing or not a directory.
    ParentMissing {
        /// The parent path.
        detail: String,
    },
    /// A staging directory for this `restore_id` exists but its
    /// manifest is missing/corrupt/foreign — never deleted blindly.
    StagingConflict {
        /// The staging path.
        detail: String,
    },
    /// An asset required by the `Fail` policy (or unverifiable under
    /// it) is absent/unknown.
    MissingAsset {
        /// Asset digest or ID.
        detail: String,
    },
    /// The project has processors but no plugin-state source (or no
    /// HEAD manifest) was supplied — opaque state cannot be copied.
    PluginStateRequired {
        /// Detail.
        detail: String,
    },
    /// Estimated output size exceeds the probed free space.
    LowDisk {
        /// Estimated required bytes.
        required: u64,
        /// Probed available bytes.
        available: u64,
    },
    /// Filesystem/publish failure before a verified publish.
    Io {
        /// What failed.
        detail: String,
    },
    /// A publish effect could not be classified — the caller must
    /// reconcile by re-running (the engine never claims rollback or
    /// success blindly).
    Indeterminate {
        /// What is indeterminate.
        detail: String,
    },
    /// Readback validation of the staged bundle failed.
    Validation {
        /// What failed verification.
        detail: String,
    },
}

impl std::fmt::Display for RestoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => write!(f, "restore cancelled"),
            Self::InvalidRequest { detail } => write!(f, "invalid restore request: {detail}"),
            Self::DestinationExists { detail } => write!(f, "destination exists: {detail}"),
            Self::ParentMissing { detail } => write!(f, "destination parent missing: {detail}"),
            Self::StagingConflict { detail } => write!(f, "staging conflict: {detail}"),
            Self::MissingAsset { detail } => write!(f, "missing asset: {detail}"),
            Self::PluginStateRequired { detail } => {
                write!(f, "plugin state required: {detail}")
            }
            Self::LowDisk {
                required,
                available,
            } => {
                write!(f, "low disk: need {required}, have {available}")
            }
            Self::Io { detail } => write!(f, "restore io: {detail}"),
            Self::Indeterminate { detail } => write!(f, "restore indeterminate: {detail}"),
            Self::Validation { detail } => write!(f, "restore validation: {detail}"),
        }
    }
}

impl std::error::Error for RestoreError {}

/// Directory-fsync evidence — recorded honestly per platform
/// capability (Windows cannot fsync directories through `std`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DirSyncEvidence {
    /// Parent directory fsync succeeded.
    Synced,
    /// The platform cannot fsync directories — recorded, not hidden.
    Unsupported,
    /// The fsync attempt failed.
    Failed,
    /// Not reached (publish did not complete).
    Unknown,
}

/// Per-kind stable-ID census for the lineage report.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StableIdReport {
    /// `preserve` — every entity keeps its source stable ID; nothing
    /// is remapped in this slice.
    pub policy: String,
    /// Entity count per object kind.
    pub counts: BTreeMap<String, u64>,
}

/// Asset copy report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreAssetReport {
    /// Assets referenced by the project.
    pub total: u64,
    /// Blobs actually materialized into the copy.
    pub copied: u64,
    /// Digests skipped under `AllowDegraded`.
    pub degraded: Vec<String>,
}

/// Plugin-state copy report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestorePluginReport {
    /// Source manifest name copied verbatim.
    pub manifest_name: String,
    /// Processor records in the manifest.
    pub records: u64,
    /// Content-addressed blobs copied.
    pub blobs: u64,
}

/// Per-domain effect report — restore never mutates its sources.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreEffects {
    /// Source-side invariants.
    pub source: RestoreSourceEffects,
    /// Git-side invariants (no checkout / index / worktree writes).
    pub git: RestoreGitEffects,
    /// Copy-shape flags.
    pub copy: RestoreCopyEffects,
}

/// Invariants about the restore's inputs — all must hold.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreSourceEffects {
    /// Source project bytes were only read.
    pub project_unchanged: bool,
    /// Source Saved HEAD was not repointed.
    pub saved_head_unchanged: bool,
}

/// Git invariants — restore never checks out or writes Git state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreGitEffects {
    /// Git index was not touched (no checkout/stage).
    pub index_untouched: bool,
    /// Git working tree was not touched.
    pub worktree_untouched: bool,
}

/// What kind of copy was produced.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreCopyEffects {
    /// `true` when `AllowDegraded` skipped media — the copy is
    /// explicitly incomplete and must be labeled as such downstream.
    pub degraded_copy: bool,
    /// This is a NEW project copy — never an in-place restore.
    pub in_place_restore: bool,
}

fn untouched_source_effects() -> RestoreSourceEffects {
    RestoreSourceEffects {
        project_unchanged: true,
        saved_head_unchanged: true,
    }
}

fn untouched_git_effects() -> RestoreGitEffects {
    RestoreGitEffects {
        index_untouched: true,
        worktree_untouched: true,
    }
}

/// Receipt outcome.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RestoreOutcome {
    /// This invocation published the copy.
    Published,
    /// A prior attempt already published; its recorded receipt is
    /// returned after destination verification.
    Replayed,
}

/// The durable result of a restore — returned to the caller AND
/// written into the published bundle as `restore-receipt.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreReceipt {
    /// Request idempotency key.
    pub restore_id: String,
    /// Outcome.
    pub outcome: RestoreOutcome,
    /// Source project identity.
    pub source_project_id: ProjectId,
    /// The fork's new project identity.
    pub new_project_id: ProjectId,
    /// Source snapshot content digest.
    pub source_snapshot_digest: ContentDigest,
    /// Fully resolved source locator (provenance).
    pub source_locator: FixedSnapshotLocator,
    /// Project revision recorded in the copy's envelope.
    pub project_revision: ProjectRevision,
    /// Schema version written (always current).
    pub schema_version: u32,
    /// Snapshot file name inside the bundle.
    pub snapshot_name: String,
    /// Asset copy report.
    pub assets: RestoreAssetReport,
    /// Plugin-state copy report (`None` = no processors / not copied).
    pub plugin_state: Option<RestorePluginReport>,
    /// Stable-ID lineage report.
    pub stable_ids: StableIdReport,
    /// Directory-fsync evidence for the publish step.
    pub directory_sync: DirSyncEvidence,
    /// Published destination directory.
    pub destination: PathBuf,
    /// Set when the in-bundle receipt file could not be written —
    /// the returned receipt remains authoritative.
    pub receipt_file_error: Option<String>,
    /// Per-domain effect report.
    pub effects: RestoreEffects,
}

// ---------------------------------------------------------------------------
// staging manifest (crash-recovery journal inside the staging dir)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RestoreStage {
    Started,
    Materialized,
    Validated,
    Published,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreManifest {
    format_version: u32,
    restore_id: String,
    stage: RestoreStage,
    source_project_id: ProjectId,
    new_project_id: ProjectId,
    source_snapshot_digest: ContentDigest,
    snapshot_name: String,
    project_revision: ProjectRevision,
    /// The authoritative receipt once `Published`; `None` before.
    receipt: Option<RestoreReceipt>,
}

const MANIFEST_FORMAT: u32 = 1;
const MANIFEST_NAME: &str = "restore.json";
const RECEIPT_NAME: &str = "restore-receipt.json";

/// Windows device names rejected as destination leaf stems.
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

// ---------------------------------------------------------------------------
// destination validation
// ---------------------------------------------------------------------------

fn valid_leaf(name: &str) -> bool {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':', '\0'])
        || name.ends_with(['.', ' '])
    {
        return false;
    }
    let stem = name.split('.').next().unwrap_or(name);
    !RESERVED_NAMES.contains(&stem.to_ascii_uppercase().as_str())
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Syntactic destination resolution + parent canonicalization — no
/// destination-existence checks (those depend on resume state).
fn resolve_paths(req: &RestoreRequest) -> Result<RestorePaths, RestoreError> {
    if req.restore_id.trim().is_empty()
        || !req
            .restore_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(RestoreError::InvalidRequest {
            detail: format!("restore_id {:?} is not filename-safe", req.restore_id),
        });
    }
    let output = &req.output_dir;
    if !output.is_absolute() {
        return Err(RestoreError::InvalidRequest {
            detail: format!("output dir must be absolute: {}", output.display()),
        });
    }
    let leaf = output
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| RestoreError::InvalidRequest {
            detail: format!("output dir has no usable leaf: {}", output.display()),
        })?
        .to_owned();
    if !valid_leaf(&leaf) {
        return Err(RestoreError::InvalidRequest {
            detail: format!("unsafe destination leaf {leaf:?}"),
        });
    }
    let parent = output
        .parent()
        .ok_or_else(|| RestoreError::InvalidRequest {
            detail: "output dir has no parent".into(),
        })?;
    let meta = std::fs::symlink_metadata(parent).map_err(|e| RestoreError::ParentMissing {
        detail: format!("{}: {e}", parent.display()),
    })?;
    if !meta.is_dir() {
        return Err(RestoreError::ParentMissing {
            detail: format!("not a directory: {}", parent.display()),
        });
    }
    let canonical_parent = std::fs::canonicalize(parent).map_err(|e| RestoreError::Io {
        detail: format!("canonicalize {}: {e}", parent.display()),
    })?;
    let dest = canonical_parent.join(&leaf);
    let staging =
        canonical_parent.join(format!(".{leaf}.restore-{}", sanitize_id(&req.restore_id)));
    Ok(RestorePaths {
        parent: canonical_parent,
        leaf,
        dest,
        staging,
    })
}

/// The destination must not exist — case-conflicting siblings,
/// symlinks, and any existing entry all fail before any write.
fn ensure_dest_absent(
    canonical_parent: &Path,
    leaf: &str,
    dest: &Path,
) -> Result<(), RestoreError> {
    // Case-conflict scan: any sibling whose name differs only by case
    // is a conflict on case-insensitive filesystems.
    for entry in std::fs::read_dir(canonical_parent).map_err(|e| RestoreError::Io {
        detail: format!("read parent {}: {e}", canonical_parent.display()),
    })? {
        let entry = entry.map_err(|e| RestoreError::Io {
            detail: format!("parent entry: {e}"),
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.eq_ignore_ascii_case(leaf) {
            return Err(RestoreError::DestinationExists {
                detail: format!(
                    "case-conflicting sibling {name:?} in {}",
                    canonical_parent.display()
                ),
            });
        }
    }
    match std::fs::symlink_metadata(dest) {
        Ok(_) => Err(RestoreError::DestinationExists {
            detail: format!("destination already exists: {}", dest.display()),
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(RestoreError::Io {
            detail: format!("stat destination {}: {e}", dest.display()),
        }),
    }
}

fn check_manifest_binding(
    m: &RestoreManifest,
    req: &RestoreRequest,
    snapshot: &ImmutableSnapshot,
) -> Result<(), RestoreError> {
    if m.format_version != MANIFEST_FORMAT || m.restore_id != req.restore_id {
        return Err(RestoreError::StagingConflict {
            detail: "manifest belongs to a different restore".into(),
        });
    }
    if m.source_snapshot_digest != snapshot.identity.digest
        || m.new_project_id != req.new_project_id
        || m.source_project_id != snapshot.identity.project_id
    {
        return Err(RestoreError::StagingConflict {
            detail: "manifest does not match this request".into(),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// staged file publish helpers
// ---------------------------------------------------------------------------

/// Publishes `bytes` at `path` inside the staging tree —
/// `CreateNew`-style idempotent on resume (existing identical bytes
/// accepted, differing bytes rejected).
fn publish_file(path: &Path, bytes: &[u8], cancel: &SnapshotCancel) -> Result<(), RestoreError> {
    if cancel.is_cancelled() {
        return Err(RestoreError::Cancelled);
    }
    if let Ok(existing) = std::fs::read(path) {
        if existing == bytes {
            return Ok(());
        }
        return Err(RestoreError::Validation {
            detail: format!("staged file differs on resume: {}", path.display()),
        });
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| RestoreError::Io {
            detail: format!("create {}: {e}", parent.display()),
        })?;
    }
    let mut staged = StagedFile::create(path).map_err(|e| RestoreError::Io {
        detail: format!("stage {}: {e}", path.display()),
    })?;
    staged.write_all(bytes).map_err(|e| RestoreError::Io {
        detail: format!("write {}: {e}", path.display()),
    })?;
    let mut receipt = staged
        .seal()
        .publish(PublishOptions::new(
            PublishMode::CreateNew,
            Durability::FileAndDirectorySync,
        ))
        .map_err(|mut failure| {
            let indeterminate = failure.effect() == PublishEffect::Indeterminate;
            if let Some(handle) = failure.take_recovery_handle() {
                let _ = handle.cleanup();
            }
            if indeterminate {
                RestoreError::Indeterminate {
                    detail: format!("publish {}: {failure}", path.display()),
                }
            } else {
                RestoreError::Io {
                    detail: format!("publish {}: {failure}", path.display()),
                }
            }
        })?;
    if let Some(handle) = receipt.take_recovery_handle() {
        let _ = handle.cleanup();
    }
    Ok(())
}

fn publish_replace(path: &Path, bytes: &[u8]) -> Result<(), RestoreError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| RestoreError::Io {
            detail: format!("create {}: {e}", parent.display()),
        })?;
    }
    let mut staged = StagedFile::create(path).map_err(|e| RestoreError::Io {
        detail: format!("stage {}: {e}", path.display()),
    })?;
    staged.write_all(bytes).map_err(|e| RestoreError::Io {
        detail: format!("write {}: {e}", path.display()),
    })?;
    let mut receipt = staged
        .seal()
        .publish(PublishOptions::new(
            PublishMode::Replace,
            Durability::FileAndDirectorySync,
        ))
        .map_err(|failure| RestoreError::Io {
            detail: format!("publish {}: {failure}", path.display()),
        })?;
    if let Some(handle) = receipt.take_recovery_handle() {
        let _ = handle.cleanup();
    }
    Ok(())
}

fn read_manifest(path: &Path) -> Result<Option<RestoreManifest>, RestoreError> {
    match std::fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| RestoreError::StagingConflict {
                    detail: format!("corrupt manifest {}: {e}", path.display()),
                })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(RestoreError::Io {
            detail: format!("read {}: {e}", path.display()),
        }),
    }
}

fn write_manifest(path: &Path, manifest: &RestoreManifest) -> Result<(), RestoreError> {
    let bytes = serde_json::to_vec(manifest).map_err(|e| RestoreError::Io {
        detail: format!("serialize manifest: {e}"),
    })?;
    publish_replace(path, &bytes)
}

// ---------------------------------------------------------------------------
// fork construction
// ---------------------------------------------------------------------------

fn locator_revision(locator: &FixedSnapshotLocator) -> ProjectRevision {
    match locator {
        FixedSnapshotLocator::Live { live_revision, .. } => *live_revision,
        FixedSnapshotLocator::Saved { head, .. }
        | FixedSnapshotLocator::Recovery {
            checkpoint: head, ..
        } => snapshot_revision(head).unwrap_or(ProjectRevision(0)),
        FixedSnapshotLocator::VcsStaged { .. } | FixedSnapshotLocator::VcsCommitted { .. } => {
            ProjectRevision(0)
        }
    }
}

fn fork_project(snapshot: &ImmutableSnapshot, new_id: ProjectId) -> Project {
    let mut fork = snapshot.projection.project().clone();
    fork.project_id = new_id;
    fork
}

fn stable_id_counts(project: &Project) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    let mut bump = |key: &str, n: usize| {
        *counts.entry(key.to_owned()).or_insert(0) += n as u64;
    };
    bump("track", project.tracks.len());
    bump("lane", project.lanes.len());
    bump("clip", project.clips.len());
    bump("note", project.notes.len());
    bump("asset", project.assets.len());
    bump("source", project.sources.len());
    bump("midi-source", project.midi_sources.len());
    bump(
        "midi-note",
        project.midi_sources.values().map(|s| s.notes.len()).sum(),
    );
    bump("smf-document", project.smf_documents.len());
    bump("clock-epoch", project.clock_epochs.len());
    bump("recording-session", project.recording.sessions.len());
    bump("recording-pass", project.recording.passes.len());
    bump("capture-stream", project.recording.streams.len());
    bump("capture-segment", project.recording.segments.len());
    bump("take", project.recording.takes.len());
    bump("applied-commit", project.recording.applied_commits.len());
    bump("applied-import", project.applied_imports.len());
    bump("take-group", project.comp.take_groups.len());
    bump("comp", project.comp.comps.len());
    bump("comp-region", project.comp.regions.len());
    bump("comp-transition", project.comp.transitions.len());
    bump("param-target", project.control.targets.len());
    bump("automation-lane", project.control.lanes.len());
    bump("mod-source", project.control.sources.len());
    bump("mod-assignment", project.control.assignments.len());
    bump("bus", project.routing.buses.len());
    bump("strip", project.routing.strips.len());
    bump("processor", project.routing.processors.len());
    bump("chain", project.routing.chains.len());
    bump("semantic-port", project.routing.ports.len());
    bump("connection", project.routing.connections.len());
    bump("send", project.routing.sends.len());
    counts
}

// ---------------------------------------------------------------------------
// engine
// ---------------------------------------------------------------------------

/// Restores `req.snapshot` as a new independent project copy at
/// `req.output_dir`. See module docs for the guarantee set.
///
/// # Errors
/// Any [`RestoreError`]; on cancellation/crash the destination is
/// never created and the staging tree (if any) remains for resume
/// under the same `restore_id`.
pub fn restore_as_copy(
    req: &RestoreRequest,
    ports: &RestorePorts<'_>,
    cancel: &SnapshotCancel,
) -> Result<RestoreReceipt, RestoreError> {
    if cancel.is_cancelled() {
        return Err(RestoreError::Cancelled);
    }
    let snapshot = &req.snapshot;
    if snapshot.identity.project_id != snapshot.projection.project().project_id {
        return Err(RestoreError::InvalidRequest {
            detail: "snapshot identity/project mismatch".into(),
        });
    }
    if req.new_project_id == snapshot.identity.project_id {
        return Err(RestoreError::InvalidRequest {
            detail: "new_project_id must differ from the source project".into(),
        });
    }

    let paths = resolve_paths(req)?;
    let manifest_path = paths.staging.join(MANIFEST_NAME);

    if let Some(manifest) = read_manifest(&manifest_path)? {
        return resume_staging(req, ports, cancel, &paths, manifest);
    }
    if paths.dest.exists() {
        return resume_destination(req, &paths, snapshot);
    }
    fresh_restore(req, ports, cancel, &paths, &manifest_path)
}

/// Resolved destination paths: canonical parent, leaf name, final
/// destination, and the sibling staging directory.
struct RestorePaths {
    parent: PathBuf,
    leaf: String,
    dest: PathBuf,
    staging: PathBuf,
}

/// Resume a restore interrupted mid-staging: the manifest lives in the
/// staging tree at stage Started/Materialized/Validated.
fn resume_staging(
    req: &RestoreRequest,
    ports: &RestorePorts<'_>,
    cancel: &SnapshotCancel,
    paths: &RestorePaths,
    mut manifest: RestoreManifest,
) -> Result<RestoreReceipt, RestoreError> {
    let manifest_path = paths.staging.join(MANIFEST_NAME);
    let snapshot = &req.snapshot;
    check_manifest_binding(&manifest, req, snapshot)?;
    ensure_dest_absent(&paths.parent, &paths.leaf, &paths.dest)?;
    preflight(req, ports, &paths.dest)?;
    if manifest.stage == RestoreStage::Started {
        materialize(&paths.staging, req, ports, cancel)?;
        manifest.stage = RestoreStage::Materialized;
        write_manifest(&manifest_path, &manifest)?;
    }
    if manifest.stage == RestoreStage::Materialized {
        validate_staged(&paths.staging, &manifest, req)?;
        manifest.stage = RestoreStage::Validated;
        write_manifest(&manifest_path, &manifest)?;
    }
    if manifest.stage != RestoreStage::Validated {
        return Err(RestoreError::Indeterminate {
            detail: format!("staging manifest at unexpected stage {:?}", manifest.stage),
        });
    }
    // stage == Validated → publish (the only remaining stage here).
    let dir_sync = publish_staged_dir(&paths.staging, &paths.dest, cancel)?;
    finish_publish(&paths.dest, &mut manifest, req, dir_sync)
}

/// Resume after the staging→destination rename: the destination
/// carries our manifest. Covers the Published replay AND the crash
/// window between `rename` and the manifest's Published write
/// (manifest still at stage Validated inside the published tree).
fn resume_destination(
    req: &RestoreRequest,
    paths: &RestorePaths,
    snapshot: &ImmutableSnapshot,
) -> Result<RestoreReceipt, RestoreError> {
    let dest_manifest = read_manifest(&paths.dest.join(MANIFEST_NAME))?;
    match dest_manifest {
        Some(mut manifest) => {
            check_manifest_binding(&manifest, req, snapshot)?;
            validate_destination_bundle(&paths.dest)?;
            match (manifest.stage, manifest.receipt.take()) {
                (RestoreStage::Published, Some(receipt)) => {
                    let mut replayed = receipt;
                    replayed.outcome = RestoreOutcome::Replayed;
                    Ok(replayed)
                }
                (RestoreStage::Validated, _) => {
                    // Publish completed but bookkeeping did not —
                    // finish it now that the bundle verifies.
                    let dir_sync = probe_dir_sync(&paths.parent);
                    finish_publish(&paths.dest, &mut manifest, req, dir_sync)
                }
                (stage, _) => Err(RestoreError::Indeterminate {
                    detail: format!("destination manifest at unexpected stage {stage:?}"),
                }),
            }
        }
        None => Err(RestoreError::DestinationExists {
            detail: format!("destination already exists: {}", paths.dest.display()),
        }),
    }
}

/// Fresh attempt: staging absent, destination absent.
fn fresh_restore(
    req: &RestoreRequest,
    ports: &RestorePorts<'_>,
    cancel: &SnapshotCancel,
    paths: &RestorePaths,
    manifest_path: &Path,
) -> Result<RestoreReceipt, RestoreError> {
    if paths.staging.exists() {
        return Err(RestoreError::StagingConflict {
            detail: format!(
                "staging {} exists without a manifest",
                paths.staging.display()
            ),
        });
    }
    ensure_dest_absent(&paths.parent, &paths.leaf, &paths.dest)?;
    std::fs::create_dir_all(&paths.staging).map_err(|e| RestoreError::Io {
        detail: format!("create staging {}: {e}", paths.staging.display()),
    })?;
    let snapshot = &req.snapshot;
    let revision = locator_revision(&snapshot.identity.locator);
    let mut manifest = RestoreManifest {
        format_version: MANIFEST_FORMAT,
        restore_id: req.restore_id.clone(),
        stage: RestoreStage::Started,
        source_project_id: snapshot.identity.project_id,
        new_project_id: req.new_project_id,
        source_snapshot_digest: snapshot.identity.digest,
        snapshot_name: snapshot_name(revision),
        project_revision: revision,
        receipt: None,
    };
    write_manifest(manifest_path, &manifest)?;

    preflight(req, ports, &paths.dest)?;
    materialize(&paths.staging, req, ports, cancel)?;
    manifest.stage = RestoreStage::Materialized;
    write_manifest(manifest_path, &manifest)?;
    validate_staged(&paths.staging, &manifest, req)?;
    manifest.stage = RestoreStage::Validated;
    write_manifest(manifest_path, &manifest)?;

    let dir_sync = publish_staged_dir(&paths.staging, &paths.dest, cancel)?;
    finish_publish(&paths.dest, &mut manifest, req, dir_sync)
}

/// Shared publish tail: build the `Published` receipt, write the
/// in-bundle receipt file (failure is recorded, not fatal), then
/// finalize the manifest inside the published destination.
fn finish_publish(
    dest: &Path,
    manifest: &mut RestoreManifest,
    req: &RestoreRequest,
    dir_sync: DirSyncEvidence,
) -> Result<RestoreReceipt, RestoreError> {
    let plugin = read_plugin_report(dest, req);
    let mut receipt = build_receipt(req, manifest, dir_sync, dest.to_path_buf(), plugin, None);
    receipt.outcome = RestoreOutcome::Published;
    let receipt_bytes = serde_json::to_vec(&receipt).map_err(|e| RestoreError::Io {
        detail: format!("serialize receipt: {e}"),
    })?;
    if let Err(e) = publish_replace(&dest.join(RECEIPT_NAME), &receipt_bytes) {
        receipt.receipt_file_error = Some(e.to_string());
    }
    manifest.receipt = Some(receipt.clone());
    manifest.stage = RestoreStage::Published;
    write_manifest(&dest.join(MANIFEST_NAME), manifest)?;
    Ok(receipt)
}

// ---------------------------------------------------------------------------
// preflight
// ---------------------------------------------------------------------------

fn preflight(
    req: &RestoreRequest,
    ports: &RestorePorts<'_>,
    dest: &Path,
) -> Result<(), RestoreError> {
    let snapshot = &req.snapshot;
    let project = snapshot.projection.project();

    // -- plugin state requirement -------------------------------------------
    if !project.routing.processors.is_empty() {
        let source = ports
            .plugin_state
            .ok_or_else(|| RestoreError::PluginStateRequired {
                detail: "project has routing processors but no plugin-state source".into(),
            })?;
        let (name, bytes) = source
            .head_manifest()
            .map_err(|e| RestoreError::Io {
                detail: format!("plugin head manifest: {e}"),
            })?
            .ok_or_else(|| RestoreError::PluginStateRequired {
                detail: "plugin-state store has no HEAD manifest".into(),
            })?;
        if !is_manifest_name(&name) {
            return Err(RestoreError::Validation {
                detail: format!("invalid manifest name {name:?}"),
            });
        }
        let manifest: PluginStateManifest =
            serde_json::from_slice(&bytes).map_err(|e| RestoreError::Validation {
                detail: format!("plugin manifest parse: {e}"),
            })?;
        if manifest.project_id != snapshot.identity.project_id {
            return Err(RestoreError::Validation {
                detail: "plugin manifest belongs to a different project".into(),
            });
        }
    }

    // -- asset classification -------------------------------------------------
    let mut required_bytes: u64 = 0;
    for entry in &snapshot.assets.entries {
        match entry.status {
            ContentStatus::Present => {
                if ContentDigest::from_str(&entry.digest).is_err() {
                    return Err(RestoreError::InvalidRequest {
                        detail: format!("asset digest is not sha256-form: {}", entry.digest),
                    });
                }
                if let Ok(Some(size)) = ports.blobs.stat_blob(&entry.digest) {
                    required_bytes = required_bytes.saturating_add(size);
                }
            }
            ContentStatus::Missing | ContentStatus::Unknown => {
                if req.missing_assets == MissingAssetsPolicy::Fail {
                    return Err(RestoreError::MissingAsset {
                        detail: format!("asset {} status {:?}", entry.asset_id, entry.status),
                    });
                }
            }
        }
    }
    // envelope size estimate + slack
    let fork = fork_project(snapshot, req.new_project_id);
    let envelope_len = serde_json::to_vec(&SnapshotEnvelope {
        schema_version: SCHEMA_VERSION,
        project_id: req.new_project_id,
        revision: locator_revision(&snapshot.identity.locator),
        project: fork,
    })
    .map_or(0, |b| b.len() as u64);
    required_bytes = required_bytes
        .saturating_add(envelope_len)
        .saturating_add(1 << 20);

    // -- disk probe -------------------------------------------------------------
    if let Some(probe) = ports.storage
        && let Ok(Some(available)) = probe.available_bytes(dest.parent().unwrap_or(dest))
        && required_bytes > available
    {
        return Err(RestoreError::LowDisk {
            required: required_bytes,
            available,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// materialize / validate / publish
// ---------------------------------------------------------------------------

fn materialize(
    staging: &Path,
    req: &RestoreRequest,
    ports: &RestorePorts<'_>,
    cancel: &SnapshotCancel,
) -> Result<(), RestoreError> {
    let snapshot = &req.snapshot;
    let project = fork_project(snapshot, req.new_project_id);
    let revision = locator_revision(&snapshot.identity.locator);
    let snap_name = snapshot_name(revision);

    // 1. project envelope (DirectoryBundleStore-compatible layout)
    let envelope = SnapshotEnvelope {
        schema_version: SCHEMA_VERSION,
        project_id: req.new_project_id,
        revision,
        project,
    };
    let envelope_bytes = serde_json::to_vec(&envelope).map_err(|e| RestoreError::Io {
        detail: format!("serialize envelope: {e}"),
    })?;
    publish_file(
        &staging.join("snapshots").join(&snap_name),
        &envelope_bytes,
        cancel,
    )?;
    publish_replace(&staging.join("HEAD"), snap_name.as_bytes())?;

    // 2. asset blobs (content-addressed, digest-verified)
    materialize_blobs(staging, req, ports, cancel)?;

    // 3. plugin state (verbatim manifest + verified blobs)
    materialize_plugin_state(staging, snapshot, ports, cancel)?;
    Ok(())
}

/// Copies every `Present` asset blob into the staged `blobs/sha256/`
/// tree, verifying content digests on both fresh writes and resume
/// readback.
fn materialize_blobs(
    staging: &Path,
    req: &RestoreRequest,
    ports: &RestorePorts<'_>,
    cancel: &SnapshotCancel,
) -> Result<(), RestoreError> {
    for entry in &req.snapshot.assets.entries {
        if cancel.is_cancelled() {
            return Err(RestoreError::Cancelled);
        }
        if entry.status != ContentStatus::Present {
            continue; // degraded skip — recorded in the receipt
        }
        let digest =
            ContentDigest::from_str(&entry.digest).map_err(|_| RestoreError::InvalidRequest {
                detail: format!("asset digest is not sha256-form: {}", entry.digest),
            })?;
        let hex = digest_hex(&digest);
        let dest = staging.join("blobs").join("sha256").join(&hex);
        if dest.exists() {
            // resume: verify existing bytes match the digest
            let existing = std::fs::read(&dest).map_err(|e| RestoreError::Io {
                detail: format!("readback {}: {e}", dest.display()),
            })?;
            if ContentDigest::sha256(&existing) != digest {
                return Err(RestoreError::Validation {
                    detail: format!("staged blob digest mismatch: {}", dest.display()),
                });
            }
            continue;
        }
        match ports
            .blobs
            .read_blob(&entry.digest, cancel)
            .map_err(|e| RestoreError::Io {
                detail: format!("blob read {}: {e}", entry.digest),
            })? {
            BlobRead::Present(bytes) => {
                if ContentDigest::sha256(&bytes) != digest {
                    return Err(RestoreError::Validation {
                        detail: format!("blob content mismatch for {}", entry.digest),
                    });
                }
                publish_file(&dest, &bytes, cancel)?;
            }
            BlobRead::Absent => {
                return Err(RestoreError::MissingAsset {
                    detail: format!("blob absent for {}", entry.digest),
                });
            }
            BlobRead::Unknown => {
                return Err(RestoreError::MissingAsset {
                    detail: format!("blob availability unknown for {}", entry.digest),
                });
            }
        }
    }
    Ok(())
}

/// Copies the plugin-state manifest and every captured blob into the
/// staged `plugin-state/` tree via [`DirectoryPluginStateStore`].
fn materialize_plugin_state(
    staging: &Path,
    snapshot: &ImmutableSnapshot,
    ports: &RestorePorts<'_>,
    cancel: &SnapshotCancel,
) -> Result<(), RestoreError> {
    if snapshot.projection.project().routing.processors.is_empty() {
        return Ok(());
    }
    let source = ports
        .plugin_state
        .ok_or_else(|| RestoreError::PluginStateRequired {
            detail: "plugin-state source missing at materialize".into(),
        })?;
    let (_name, manifest_bytes) = source
        .head_manifest()
        .map_err(|e| RestoreError::Io {
            detail: format!("plugin head manifest: {e}"),
        })?
        .ok_or_else(|| RestoreError::PluginStateRequired {
            detail: "no plugin-state HEAD manifest".into(),
        })?;
    let manifest: PluginStateManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|e| RestoreError::Validation {
            detail: format!("plugin manifest parse: {e}"),
        })?;
    let mut store =
        DirectoryPluginStateStore::create(staging.join("plugin-state")).map_err(|e| {
            RestoreError::Io {
                detail: format!("create plugin-state dir: {e}"),
            }
        })?;
    for record in &manifest.entries {
        if let PluginStateOutcome::Captured { blob_digest, .. } = &record.outcome {
            if cancel.is_cancelled() {
                return Err(RestoreError::Cancelled);
            }
            let bytes = source
                .read_blob(blob_digest)
                .map_err(|e| RestoreError::Io {
                    detail: format!("plugin blob read {blob_digest}: {e}"),
                })?;
            if ContentDigest::sha256(&bytes) != *blob_digest {
                return Err(RestoreError::Validation {
                    detail: format!("plugin blob digest mismatch {blob_digest}"),
                });
            }
            store
                .put_blob(blob_digest, &bytes)
                .map_err(|e| RestoreError::Io {
                    detail: format!("plugin blob put {blob_digest}: {e}"),
                })?;
        }
    }
    store
        .save_manifest(&manifest_bytes)
        .map_err(|e| RestoreError::Io {
            detail: format!("plugin manifest publish: {e}"),
        })?;
    Ok(())
}

/// Re-opens the staged bundle and verifies every byte that will be
/// published: envelope parse + project validation + watermark, HEAD
/// consistency, every blob digest, and the plugin-state manifest
/// chain.
fn validate_staged(
    staging: &Path,
    manifest: &RestoreManifest,
    req: &RestoreRequest,
) -> Result<(), RestoreError> {
    // envelope
    let snap_path = staging.join("snapshots").join(&manifest.snapshot_name);
    let bytes = std::fs::read(&snap_path).map_err(|e| RestoreError::Io {
        detail: format!("read staged snapshot: {e}"),
    })?;
    let envelope: SnapshotEnvelope =
        serde_json::from_slice(&bytes).map_err(|e| RestoreError::Validation {
            detail: format!("staged envelope parse: {e}"),
        })?;
    if envelope.schema_version != SCHEMA_VERSION
        || envelope.project_id != req.new_project_id
        || envelope.project.project_id != req.new_project_id
    {
        return Err(RestoreError::Validation {
            detail: "staged envelope identity mismatch".into(),
        });
    }
    envelope
        .project
        .validate_references()
        .map_err(|e| RestoreError::Validation {
            detail: format!("staged project references: {e}"),
        })?;
    envelope
        .project
        .check_id_watermark()
        .map_err(|e| RestoreError::Validation {
            detail: format!("staged id watermark: {e}"),
        })?;
    // HEAD
    let head = std::fs::read_to_string(staging.join("HEAD")).map_err(|e| RestoreError::Io {
        detail: format!("read staged HEAD: {e}"),
    })?;
    if head.trim() != manifest.snapshot_name {
        return Err(RestoreError::Validation {
            detail: "staged HEAD does not name the snapshot".into(),
        });
    }
    // blobs
    for entry in &req.snapshot.assets.entries {
        if entry.status != ContentStatus::Present {
            continue;
        }
        let digest =
            ContentDigest::from_str(&entry.digest).map_err(|_| RestoreError::InvalidRequest {
                detail: format!("asset digest is not sha256-form: {}", entry.digest),
            })?;
        let path = staging
            .join("blobs")
            .join("sha256")
            .join(digest_hex(&digest));
        let bytes = std::fs::read(&path).map_err(|e| RestoreError::Validation {
            detail: format!("staged blob missing {}: {e}", path.display()),
        })?;
        if ContentDigest::sha256(&bytes) != digest {
            return Err(RestoreError::Validation {
                detail: format!("staged blob digest mismatch: {}", path.display()),
            });
        }
    }
    // plugin state
    if !req
        .snapshot
        .projection
        .project()
        .routing
        .processors
        .is_empty()
    {
        let store =
            DirectoryPluginStateStore::create(staging.join("plugin-state")).map_err(|e| {
                RestoreError::Io {
                    detail: format!("open staged plugin-state: {e}"),
                }
            })?;
        let head = store.head().map_err(|e| RestoreError::Io {
            detail: format!("plugin HEAD: {e}"),
        })?;
        let Some(head) = head else {
            return Err(RestoreError::Validation {
                detail: "staged plugin-state has no HEAD".into(),
            });
        };
        let manifest_bytes = store.read_manifest(&head).map_err(|e| RestoreError::Io {
            detail: format!("plugin manifest read: {e}"),
        })?;
        let plugin_manifest: PluginStateManifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|e| RestoreError::Validation {
                detail: format!("staged plugin manifest parse: {e}"),
            })?;
        for record in &plugin_manifest.entries {
            if let PluginStateOutcome::Captured { blob_digest, .. } = &record.outcome {
                let bytes = store.read_blob(blob_digest).map_err(|e| RestoreError::Io {
                    detail: format!("plugin blob readback {blob_digest}: {e}"),
                })?;
                if ContentDigest::sha256(&bytes) != *blob_digest {
                    return Err(RestoreError::Validation {
                        detail: format!("plugin blob readback digest mismatch {blob_digest}"),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Best-effort directory fsync — the result is *evidence*, never a
/// gate: platforms that cannot fsync directories report
/// [`DirSyncEvidence::Unsupported`] honestly.
fn probe_dir_sync(path: &Path) -> DirSyncEvidence {
    match std::fs::File::open(path).and_then(|f| f.sync_all()) {
        Ok(()) => DirSyncEvidence::Synced,
        Err(e) => {
            // Opening a directory as a file fails on Windows — that
            // is the platform-capability case, not an IO failure.
            if e.kind() == std::io::ErrorKind::PermissionDenied
                || e.kind() == std::io::ErrorKind::Unsupported
            {
                DirSyncEvidence::Unsupported
            } else {
                DirSyncEvidence::Failed
            }
        }
    }
}

/// Publishes the verified staging tree at `dest` via same-directory
/// rename, then records parent-dir fsync evidence.
fn publish_staged_dir(
    staging: &Path,
    dest: &Path,
    cancel: &SnapshotCancel,
) -> Result<DirSyncEvidence, RestoreError> {
    if cancel.is_cancelled() {
        return Err(RestoreError::Cancelled);
    }
    let _staging_sync = probe_dir_sync(staging);
    match std::fs::rename(staging, dest) {
        Ok(()) => {
            let parent = dest.parent().unwrap_or(dest);
            Ok(probe_dir_sync(parent))
        }
        Err(e) => {
            // Reconcile: did the rename actually happen?
            if dest.exists() {
                // Destination appeared — verify it is a complete
                // bundle before claiming anything.
                match quick_bundle_check(dest) {
                    Ok(()) => return Ok(probe_dir_sync(dest.parent().unwrap_or(dest))),
                    Err(_) => {
                        return Err(RestoreError::Indeterminate {
                            detail: format!(
                                "rename reported error ({e}) but destination exists unverified"
                            ),
                        });
                    }
                }
            }
            Err(RestoreError::Io {
                detail: format!("rename {} → {}: {e}", staging.display(), dest.display()),
            })
        }
    }
}

fn quick_bundle_check(dest: &Path) -> Result<(), RestoreError> {
    let head = std::fs::read_to_string(dest.join("HEAD")).map_err(|e| RestoreError::Io {
        detail: format!("dest HEAD: {e}"),
    })?;
    let name = head.trim();
    let bytes = std::fs::read(dest.join("snapshots").join(name)).map_err(|e| RestoreError::Io {
        detail: format!("dest snapshot: {e}"),
    })?;
    let envelope: SnapshotEnvelope =
        serde_json::from_slice(&bytes).map_err(|e| RestoreError::Validation {
            detail: format!("dest envelope: {e}"),
        })?;
    envelope
        .project
        .validate_references()
        .map_err(|e| RestoreError::Validation {
            detail: format!("dest references: {e}"),
        })
}

fn validate_destination_bundle(dest: &Path) -> Result<(), RestoreError> {
    if !dest.exists() {
        return Err(RestoreError::Indeterminate {
            detail: format!("published destination missing: {}", dest.display()),
        });
    }
    quick_bundle_check(dest)
}

// ---------------------------------------------------------------------------
// receipt construction
// ---------------------------------------------------------------------------

/// Reads the plugin-state report back from the *staged* store (the
/// authoritative post-materialize state).
fn read_plugin_report(staging: &Path, req: &RestoreRequest) -> Option<RestorePluginReport> {
    if req
        .snapshot
        .projection
        .project()
        .routing
        .processors
        .is_empty()
    {
        return None;
    }
    let store = DirectoryPluginStateStore::create(staging.join("plugin-state")).ok()?;
    let head = store.head().ok()??;
    let bytes = store.read_manifest(&head).ok()?;
    let manifest: PluginStateManifest = serde_json::from_slice(&bytes).ok()?;
    let blobs = manifest
        .entries
        .iter()
        .filter(|r| matches!(r.outcome, PluginStateOutcome::Captured { .. }))
        .count() as u64;
    Some(RestorePluginReport {
        manifest_name: head,
        records: manifest.entries.len() as u64,
        blobs,
    })
}

fn build_receipt(
    req: &RestoreRequest,
    manifest: &RestoreManifest,
    dir_sync: DirSyncEvidence,
    dest: PathBuf,
    plugin: Option<RestorePluginReport>,
    receipt_file_error: Option<String>,
) -> RestoreReceipt {
    let snapshot = &req.snapshot;
    let degraded: Vec<String> = snapshot
        .assets
        .entries
        .iter()
        .filter(|e| e.status != ContentStatus::Present)
        .map(|e| e.digest.clone())
        .collect();
    RestoreReceipt {
        restore_id: req.restore_id.clone(),
        outcome: RestoreOutcome::Published,
        source_project_id: snapshot.identity.project_id,
        new_project_id: req.new_project_id,
        source_snapshot_digest: snapshot.identity.digest,
        source_locator: snapshot.identity.locator.clone(),
        project_revision: manifest.project_revision,
        schema_version: SCHEMA_VERSION,
        snapshot_name: manifest.snapshot_name.clone(),
        assets: RestoreAssetReport {
            total: snapshot.assets.entries.len() as u64,
            copied: (snapshot.assets.entries.len() - degraded.len()) as u64,
            degraded,
        },
        plugin_state: plugin,
        stable_ids: StableIdReport {
            policy: "preserve".into(),
            counts: stable_id_counts(&fork_project(snapshot, req.new_project_id)),
        },
        directory_sync: dir_sync,
        destination: dest,
        receipt_file_error,
        effects: RestoreEffects {
            source: untouched_source_effects(),
            git: untouched_git_effects(),
            copy: RestoreCopyEffects {
                in_place_restore: false,
                degraded_copy: !degraded_empty(req, snapshot),
            },
        },
    }
}

fn degraded_empty(req: &RestoreRequest, snapshot: &ImmutableSnapshot) -> bool {
    req.missing_assets == MissingAssetsPolicy::Fail
        || !snapshot
            .assets
            .entries
            .iter()
            .any(|e| e.status != ContentStatus::Present)
}
