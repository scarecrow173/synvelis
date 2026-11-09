//! TIM-04 (issue #128) — plugin state snapshot consistency: capture
//! quality classification, content-addressed blob/manifest store,
//! save/restore receipts and HEAD readback reconciliation.
//!
//! Ownership boundary: the generic plugin host/lifecycle/state
//! contract stays in Ruviform (`HostCommand::SaveState`/
//! `RestoreState`, issue ruviform#196). This module owns the
//! Synvelis-side quality model, persistence and receipts — it never
//! duplicates host lifecycle machinery and never runs on the RT
//! path. `PluginStatePort` is the non-RT seam a host adapter (e.g.
//! `synvelis-player`) or a test mock implements.
//!
//! Three receipts stay structurally separate by construction:
//! project save completion ([`crate::SavedState`]), plugin-state
//! quality ([`PluginStateReceipt`]/[`PluginStateRestoreReceipt`])
//! and RT activation ([`crate::RoutingActivationReceipt`]). A
//! completed file save is never reported as plugin-state-exact.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use ruviform_io_file::{
    Durability, PublishEffect, PublishFailure, PublishFailureKind, PublishMode, PublishOptions,
    StagedFile,
};
use serde::{Deserialize, Serialize};

use crate::id::{ProcessorId, ProjectId};
use crate::project::Project;
use crate::snapshot::ContentDigest;

/// Manifest schema version for [`PluginStateManifest`].
pub const PLUGIN_STATE_SCHEMA_VERSION: u32 = 1;

// ── Quality model ────────────────────────────────────────────────

/// Observed quality of one plugin-state capture — the Issue #128
/// vocabulary. Ordered worst→best so `>= Quiescent` selects the
/// exact-eligible classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateCaptureQuality {
    /// The plugin does not implement state save/restore.
    Unsupported,
    /// Captured without a point-in-time consistency guarantee.
    BestEffort,
    /// Captured while the instance was stopped or at a safe idle
    /// boundary.
    Quiescent,
    /// Captured at a confirmed plugin/host agreement point.
    Coordinated,
}

impl StateCaptureQuality {
    /// `true` for `Quiescent`/`Coordinated` — the classes an exact
    /// (Strict) consumer may rely on.
    #[must_use]
    pub const fn exact_eligible(self) -> bool {
        matches!(self, Self::Quiescent | Self::Coordinated)
    }
}

/// The guarantee level a caller asks the port to achieve. The port
/// reports the *observed* [`StateCaptureQuality`] separately — a
/// `BestEffort` request that lands on a stable boundary still
/// records the stronger observed class (never under-reports).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMode {
    /// Demand a coordinated agreement point.
    Coordinated,
    /// A quiescent (stopped/safely idle) capture suffices.
    Quiescent,
    /// Any captured state is acceptable; quality is still recorded.
    BestEffort,
}

/// Save-time quality policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateQualityPolicy {
    /// Record every outcome honestly and publish — degraded or
    /// unsupported captures stay visible in the receipt. A
    /// conditional save, never a perfect-reproduction claim.
    #[default]
    Standard,
    /// Fail closed: any `BestEffort`/`Unsupported`/`Failed` record
    /// rejects the whole save before anything is published, so an
    /// old state or recovery blob is never overwritten by a
    /// degraded capture.
    Strict,
}

/// Typed failure kind for plugin-state capture/restore — the Issue
/// #128 failure vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginStateFailureKind {
    /// The plugin hook declined or returned an error.
    CaptureFailed,
    /// The operation timed out.
    Timeout,
    /// Permission denied (OS/sandbox level).
    PermissionDenied,
    /// The plugin instance crashed or entered the failed lifecycle.
    Crashed,
    /// Stored bytes failed integrity verification.
    Corrupt,
    /// No live instance: unresolved binding, stale handle or a
    /// processor absent from the port's live set.
    Unavailable,
    /// A referenced blob/manifest is absent.
    Missing,
}

/// Live plugin binding identity recorded at bind time — enough to
/// detect a plugin/version/capability change across reloads
/// (`NeedsRebind` stays a routing-layer concern; this record makes
/// the mismatch visible instead of silently substituting).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginStateBinding {
    /// Plugin descriptor identity (`PluginRef::descriptor`).
    pub descriptor: String,
    /// Authored plugin version.
    pub version: String,
    /// Capability revision observed at bind.
    pub capability_revision: u64,
    /// Opaque host runtime instance identity.
    pub host_runtime_instance: String,
    /// Routing graph generation the instance belongs to.
    pub graph_generation: u64,
}

/// Facts the host observed around the capture call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureObservation {
    /// The instance was in the processing lifecycle at capture time.
    pub processing: bool,
    /// The host observed a stable non-processing boundary.
    pub stable_boundary: bool,
}

/// A successful capture: opaque bytes plus the achieved quality and
/// the observations behind it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginStateCapture {
    /// Plugin-defined state bytes (opaque to Synvelis).
    pub blob: Vec<u8>,
    /// The quality the port actually achieved — recorded verbatim,
    /// never upgraded by the session.
    pub achieved: StateCaptureQuality,
    /// Observation facts behind `achieved`.
    pub observation: CaptureObservation,
    /// Optional diagnostic detail.
    pub detail: Option<String>,
}

/// Port-level capture/restore failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginStateCaptureError {
    /// The instance does not implement state save/restore.
    Unsupported,
    /// Typed failure.
    Failed(PluginStateFailureKind),
}

/// Per-processor outcome persisted in a [`PluginStateManifest`] and
/// reported in receipts. Failed/Unsupported records carry **no**
/// blob — an empty or absent blob is never mistaken for state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginStateOutcome {
    /// State captured at `quality`; `blob_digest`/`blob_len` locate
    /// the content-addressed blob.
    Captured {
        /// Achieved capture quality.
        quality: StateCaptureQuality,
        /// SHA-256 content digest of the blob bytes.
        blob_digest: ContentDigest,
        /// Blob length in bytes.
        blob_len: u64,
    },
    /// The plugin does not support state capture.
    Unsupported,
    /// Capture failed — typed, blob-less.
    Failed {
        /// Failure class.
        kind: PluginStateFailureKind,
    },
}

/// One processor's state record inside a manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginStateRecord {
    /// Semantic processor this state belongs to.
    pub processor_id: ProcessorId,
    /// Binding identity at capture time (`None` when the binding
    /// was unresolved or the port had no live instance).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding: Option<PluginStateBinding>,
    /// Project revision the capture corresponds to.
    pub project_revision: u64,
    /// Caller-supplied timestamp token shared by one save.
    pub stored_at: String,
    /// Capture outcome.
    pub outcome: PluginStateOutcome,
    /// Optional diagnostic detail.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Versioned manifest: project identity + per-processor records.
/// Blobs are content-addressed and referenced by digest only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginStateManifest {
    /// [`PLUGIN_STATE_SCHEMA_VERSION`] at write time.
    pub schema_version: u32,
    /// Owning project.
    pub project_id: ProjectId,
    /// Live project revision at capture.
    pub project_revision: u64,
    /// Project HEAD snapshot name at capture (`None` = unsaved
    /// project — the manifest is then only restorable by explicit
    /// name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_head: Option<String>,
    /// Per-processor records, sorted by [`ProcessorId`].
    pub entries: Vec<PluginStateRecord>,
}

/// Aggregate quality evidence — also the offline recipe's evidence
/// payload. Counts only; per-record detail stays in the manifest.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginStateQualitySummary {
    /// Total processor records.
    pub processors: u64,
    /// Records captured at `Quiescent`/`Coordinated`.
    pub exact: u64,
    /// Records captured at `BestEffort`.
    pub degraded: u64,
    /// Records whose plugin does not support state.
    pub unsupported: u64,
    /// Records that failed.
    pub failed: u64,
}

impl PluginStateQualitySummary {
    /// `true` when every record is exact-eligible — vacuously true
    /// for a project with no processors (nothing to restore).
    #[must_use]
    pub const fn exact_eligible(&self) -> bool {
        self.degraded == 0 && self.unsupported == 0 && self.failed == 0
    }

    fn of(records: &[PluginStateRecord]) -> Self {
        let mut summary = Self {
            processors: records.len() as u64,
            ..Self::default()
        };
        for record in records {
            match &record.outcome {
                PluginStateOutcome::Captured { quality, .. } => {
                    if quality.exact_eligible() {
                        summary.exact += 1;
                    } else {
                        summary.degraded += 1;
                    }
                }
                PluginStateOutcome::Unsupported => summary.unsupported += 1,
                PluginStateOutcome::Failed { .. } => summary.failed += 1,
            }
        }
        summary
    }
}

/// Receipt for one plugin-state save — a separate artifact from
/// [`crate::SavedState`] (project HEAD) by construction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginStateReceipt {
    /// Manifest name HEAD points at after the save (`None` when
    /// nothing was published — e.g. a `QualityRejected` never
    /// reaches this type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_name: Option<String>,
    /// Content digest of the published manifest bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_digest: Option<ContentDigest>,
    /// Project revision captured.
    pub project_revision: u64,
    /// Project HEAD name at capture time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_head: Option<String>,
    /// Policy the save ran under.
    pub policy: StateQualityPolicy,
    /// Per-record outcomes (same order as the manifest).
    pub records: Vec<PluginStateRecord>,
    /// Aggregate quality evidence.
    pub summary: PluginStateQualitySummary,
    /// `true` when an indeterminate manifest publish was proven by
    /// HEAD/blob readback — never asserted without readback.
    pub reconciled: bool,
}

/// Per-record restore outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginStateRestoreOutcome {
    /// Blob verified against its digest and handed to the plugin.
    Restored,
    /// The manifest record carried no blob (save-time
    /// `Unsupported`/`Failed`) — nothing to restore, reported
    /// explicitly.
    NotCaptured,
    /// The plugin refused the restore as unsupported.
    Unsupported,
    /// Typed failure — `Corrupt` on digest mismatch, `Missing` on
    /// absent blob, port-reported kinds otherwise.
    Failed {
        /// Failure class.
        kind: PluginStateFailureKind,
    },
}

/// One processor's restore result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginStateRestoreRecord {
    /// Semantic processor.
    pub processor_id: ProcessorId,
    /// Restore outcome.
    pub outcome: PluginStateRestoreOutcome,
}

/// Receipt for one plugin-state restore.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginStateRestoreReceipt {
    /// Manifest name the restore read.
    pub manifest_name: String,
    /// Content digest of the manifest bytes read.
    pub manifest_digest: ContentDigest,
    /// Per-record outcomes, in manifest order.
    pub records: Vec<PluginStateRestoreRecord>,
    /// Live project processors absent from the manifest — reported,
    /// never silently skipped.
    pub missing_states: Vec<ProcessorId>,
}

/// Session-level error: quality gate, store or manifest failures.
/// Per-processor problems are *records*, not errors — the session
/// only errors when the save/restore as a whole cannot proceed or
/// an integrity/publish guarantee could not be proven.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginStateError {
    /// `Strict` policy rejected the save — nothing was published.
    QualityRejected {
        /// The summary that failed the gate.
        summary: PluginStateQualitySummary,
    },
    /// The store failed or could not prove a publish effect.
    Store(PluginStateStoreError),
    /// The manifest bytes could not be read, parsed or verified.
    Manifest(String),
    /// No manifest exists to restore (no explicit name and no HEAD).
    NotFound,
}

impl fmt::Display for PluginStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::QualityRejected { .. } => write!(f, "plugin state quality rejected"),
            Self::Store(e) => write!(f, "plugin state store: {e}"),
            Self::Manifest(d) => write!(f, "plugin state manifest: {d}"),
            Self::NotFound => write!(f, "no plugin state manifest"),
        }
    }
}

impl std::error::Error for PluginStateError {}

// ── Port ─────────────────────────────────────────────────────────

/// Non-RT plugin-state seam. Implementations vend live plugin
/// instances (e.g. `synvelis-player`'s `PluginHost` adapter) or
/// scripted mocks. **Never called from an RT callback** — capture
/// and restore may allocate and call plugin hooks.
pub trait PluginStatePort {
    /// The live binding identity for `processor`, or `None` when no
    /// live instance is bound (unresolved binding, stale generation,
    /// unknown processor).
    fn describe(&self, processor: ProcessorId) -> Option<PluginStateBinding>;

    /// Captures the instance's opaque state. `mode` is the caller's
    /// requested guarantee; the returned [`PluginStateCapture`]
    /// reports the *achieved* quality and observations.
    ///
    /// # Errors
    /// [`PluginStateCaptureError::Unsupported`] when the plugin has
    /// no state support; [`PluginStateCaptureError::Failed`] with a
    /// typed kind otherwise. An error carries no blob.
    fn capture(
        &mut self,
        processor: ProcessorId,
        mode: CaptureMode,
    ) -> Result<PluginStateCapture, PluginStateCaptureError>;

    /// Applies bytes produced by an earlier `capture`.
    ///
    /// # Errors
    /// Same contract as [`PluginStatePort::capture`].
    fn restore(
        &mut self,
        processor: ProcessorId,
        bytes: &[u8],
    ) -> Result<(), PluginStateCaptureError>;
}

// ── Store ────────────────────────────────────────────────────────

/// Store-level failure. `Indeterminate` means the publish effect
/// could not be proven either way — the session must reconcile via
/// HEAD/blob readback before claiming success or failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginStateStoreError {
    /// The publish provably did not take effect; previous HEAD is
    /// intact.
    NotPublished,
    /// The publish effect is undetermined — readback required.
    Indeterminate,
    /// The object does not exist.
    NotFound,
    /// Any other failure, with detail.
    Failed(String),
}

impl fmt::Display for PluginStateStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotPublished => write!(f, "not published"),
            Self::Indeterminate => write!(f, "publish effect indeterminate"),
            Self::NotFound => write!(f, "not found"),
            Self::Failed(d) => write!(f, "{d}"),
        }
    }
}

impl std::error::Error for PluginStateStoreError {}

/// Content-addressed plugin-state persistence: immutable blobs +
/// immutable manifests + a mutable HEAD pointer. Manifest names are
/// derived from content (`ps-<digest16>.json`), so re-saving
/// identical state is idempotent and distinct states never collide
/// on a name.
pub trait PluginStateStore {
    /// Publishes `bytes` as an immutable manifest and repoints HEAD,
    /// returning the manifest name. `Indeterminate` means HEAD may or
    /// may not have been repointed — reconcile via [`Self::head`].
    ///
    /// # Errors
    /// [`PluginStateStoreError`] — see variants.
    fn save_manifest(&mut self, bytes: &[u8]) -> Result<String, PluginStateStoreError>;

    /// The manifest name HEAD currently points at.
    ///
    /// # Errors
    /// [`PluginStateStoreError::Failed`] on read errors.
    fn head(&self) -> Result<Option<String>, PluginStateStoreError>;

    /// Reads manifest `name` (validated — no path traversal).
    ///
    /// # Errors
    /// [`PluginStateStoreError::NotFound`]/[`PluginStateStoreError::Failed`].
    fn read_manifest(&self, name: &str) -> Result<Vec<u8>, PluginStateStoreError>;

    /// Stores `bytes` under `digest`'s content address. Idempotent:
    /// an existing identical blob is accepted. `Indeterminate`
    /// requires readback reconciliation via [`Self::read_blob`].
    ///
    /// # Errors
    /// [`PluginStateStoreError`] — see variants.
    fn put_blob(
        &mut self,
        digest: &ContentDigest,
        bytes: &[u8],
    ) -> Result<(), PluginStateStoreError>;

    /// Reads the blob stored under `digest`.
    ///
    /// # Errors
    /// [`PluginStateStoreError::NotFound`]/[`PluginStateStoreError::Failed`].
    fn read_blob(&self, digest: &ContentDigest) -> Result<Vec<u8>, PluginStateStoreError>;
}

/// The manifest file name for `bytes` — content-derived, immutable.
#[must_use]
pub fn plugin_state_manifest_name(bytes: &[u8]) -> String {
    let hex = digest_hex(&ContentDigest::sha256(bytes));
    format!("ps-{}.json", &hex[..16])
}

/// Filesystem-safe hex form of a [`ContentDigest`] (`sha256:`
/// prefix stripped — `:` is not portable in file names).
#[must_use]
pub fn digest_hex(digest: &ContentDigest) -> String {
    digest
        .to_string()
        .strip_prefix("sha256:")
        .unwrap_or_default()
        .to_owned()
}

/// `true` for well-formed manifest names (`ps-<16 hex>.json`) —
/// the only names [`PluginStateStore::read_manifest`] accepts.
#[must_use]
pub fn is_manifest_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("ps-") else {
        return false;
    };
    let Some(hex) = rest.strip_suffix(".json") else {
        return false;
    };
    hex.len() == 16 && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

/// In-memory store — deterministic, atomic, always reports a
/// definite publish effect. Tests wrap it (or implement the trait)
/// to inject `Indeterminate`/`NotPublished`/failures.
#[derive(Clone, Debug, Default)]
pub struct InMemoryPluginStateStore {
    manifests: BTreeMap<String, Vec<u8>>,
    blobs: BTreeMap<String, Vec<u8>>,
    head_name: Option<String>,
}

impl InMemoryPluginStateStore {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl PluginStateStore for InMemoryPluginStateStore {
    fn save_manifest(&mut self, bytes: &[u8]) -> Result<String, PluginStateStoreError> {
        let name = plugin_state_manifest_name(bytes);
        self.manifests.insert(name.clone(), bytes.to_vec());
        self.head_name = Some(name.clone());
        Ok(name)
    }

    fn head(&self) -> Result<Option<String>, PluginStateStoreError> {
        Ok(self.head_name.clone())
    }

    fn read_manifest(&self, name: &str) -> Result<Vec<u8>, PluginStateStoreError> {
        if !is_manifest_name(name) {
            return Err(PluginStateStoreError::Failed(format!(
                "invalid manifest name {name:?}"
            )));
        }
        self.manifests
            .get(name)
            .cloned()
            .ok_or(PluginStateStoreError::NotFound)
    }

    fn put_blob(
        &mut self,
        digest: &ContentDigest,
        bytes: &[u8],
    ) -> Result<(), PluginStateStoreError> {
        self.blobs.insert(digest_hex(digest), bytes.to_vec());
        Ok(())
    }

    fn read_blob(&self, digest: &ContentDigest) -> Result<Vec<u8>, PluginStateStoreError> {
        self.blobs
            .get(&digest_hex(digest))
            .cloned()
            .ok_or(PluginStateStoreError::NotFound)
    }
}

/// Filesystem failure inside [`DirectoryPluginStateStore`]'s staged
/// publish — mirrors `store.rs`'s `PublishStepError`, flattened to
/// [`PluginStateStoreError`] by the caller.
enum PublishStepError {
    /// `StagedFile::create` failed before any bytes were written.
    Stage(ruviform_io_file::FileError),
    /// Writing the staged temp failed.
    Write {
        /// The underlying write failure.
        source: std::io::Error,
        /// Staged temp path at failure time.
        temp: Option<PathBuf>,
    },
    /// `SealedFile::publish` failed — effect preserved for mapping.
    Publish(PublishFailure),
}

/// Directory store: `<root>/manifests/ps-*.json`,
/// `<root>/blobs/<hex>`, `<root>/HEAD`. Publishes go through
/// `ruviform-io-file` staged publish (`FileAndDirectorySync`,
/// atomic visibility) — the same contract `DirectoryBundleStore`
/// uses for project snapshots.
pub struct DirectoryPluginStateStore {
    root: PathBuf,
}

impl DirectoryPluginStateStore {
    /// Creates `<root>/manifests` and `<root>/blobs`.
    ///
    /// # Errors
    /// [`PluginStateStoreError::Failed`] on filesystem errors.
    pub fn create(root: impl AsRef<Path>) -> Result<Self, PluginStateStoreError> {
        let root = root.as_ref().to_path_buf();
        for dir in ["manifests", "blobs"] {
            fs::create_dir_all(root.join(dir))
                .map_err(|e| PluginStateStoreError::Failed(format!("create {dir}: {e}")))?;
        }
        Ok(Self { root })
    }

    fn manifests_dir(&self) -> PathBuf {
        self.root.join("manifests")
    }

    fn blobs_dir(&self) -> PathBuf {
        self.root.join("blobs")
    }

    fn head_path(&self) -> PathBuf {
        self.root.join("HEAD")
    }

    /// Stages `bytes` and publishes at `path` — file sync + atomic
    /// publish + directory sync.
    fn publish_staged(
        path: &Path,
        bytes: &[u8],
        mode: PublishMode,
    ) -> Result<(), PublishStepError> {
        let mut staged = StagedFile::create(path).map_err(PublishStepError::Stage)?;
        if let Err(source) = staged.write_all(bytes) {
            let temp = staged.temp_path().map(Path::to_path_buf);
            return Err(PublishStepError::Write { source, temp });
        }
        let mut receipt = staged
            .seal()
            .publish(PublishOptions::new(mode, Durability::FileAndDirectorySync))
            .map_err(PublishStepError::Publish)?;
        if let Some(handle) = receipt.take_recovery_handle() {
            let _ = handle.cleanup();
        }
        Ok(())
    }

    /// Flattens a staged-publish failure, preserving `Indeterminate`
    /// for the session's readback reconciliation. Non-indeterminate
    /// leftovers are cleaned up best-effort.
    fn publish_error(error: PublishStepError) -> PluginStateStoreError {
        match error {
            PublishStepError::Stage(e) => {
                PluginStateStoreError::Failed(format!("stage create: {e}"))
            }
            PublishStepError::Write { source, temp } => PluginStateStoreError::Failed(format!(
                "stage write: {source}; temp {}",
                temp.map_or_else(|| "unknown".to_owned(), |p| p.display().to_string())
            )),
            PublishStepError::Publish(mut failure) => {
                if failure.effect() != PublishEffect::Indeterminate
                    && let Some(handle) = failure.take_recovery_handle()
                {
                    let _ = handle.cleanup();
                }
                match failure.effect() {
                    PublishEffect::Indeterminate => PluginStateStoreError::Indeterminate,
                    _ => PluginStateStoreError::Failed(format!("publish: {failure}")),
                }
            }
        }
    }

    /// Publishes immutable content at `path`: `CreateNew`, and an
    /// existing destination is accepted only when byte-identical
    /// (content-addressed idempotence); differing bytes are a
    /// `Failed` collision — never silently overwritten.
    fn publish_immutable(path: &Path, bytes: &[u8]) -> Result<(), PluginStateStoreError> {
        match Self::publish_staged(path, bytes, PublishMode::CreateNew) {
            Ok(()) => Ok(()),
            Err(PublishStepError::Publish(failure))
                if failure.kind() == PublishFailureKind::DestinationExists =>
            {
                match fs::read(path) {
                    Ok(existing) if existing == bytes => Ok(()),
                    Ok(_) => Err(PluginStateStoreError::Failed(format!(
                        "content collision at {}",
                        path.display()
                    ))),
                    Err(e) => Err(PluginStateStoreError::Failed(format!(
                        "collision at {} unreadable: {e}",
                        path.display()
                    ))),
                }
            }
            Err(e) => Err(Self::publish_error(e)),
        }
    }
}

impl PluginStateStore for DirectoryPluginStateStore {
    fn save_manifest(&mut self, bytes: &[u8]) -> Result<String, PluginStateStoreError> {
        let name = plugin_state_manifest_name(bytes);
        let manifest_path = self.manifests_dir().join(&name);
        Self::publish_immutable(&manifest_path, bytes)?;
        Self::publish_staged(&self.head_path(), name.as_bytes(), PublishMode::Replace)
            .map_err(Self::publish_error)?;
        Ok(name)
    }

    fn head(&self) -> Result<Option<String>, PluginStateStoreError> {
        match fs::read_to_string(self.head_path()) {
            Ok(name) => {
                let name = name.trim().to_owned();
                if is_manifest_name(&name) {
                    Ok(Some(name))
                } else {
                    Err(PluginStateStoreError::Failed(format!(
                        "HEAD holds invalid manifest name {name:?}"
                    )))
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(PluginStateStoreError::Failed(format!("read HEAD: {e}"))),
        }
    }

    fn read_manifest(&self, name: &str) -> Result<Vec<u8>, PluginStateStoreError> {
        if !is_manifest_name(name) {
            return Err(PluginStateStoreError::Failed(format!(
                "invalid manifest name {name:?}"
            )));
        }
        fs::read(self.manifests_dir().join(name)).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => PluginStateStoreError::NotFound,
            _ => PluginStateStoreError::Failed(format!("read manifest {name}: {e}")),
        })
    }

    fn put_blob(
        &mut self,
        digest: &ContentDigest,
        bytes: &[u8],
    ) -> Result<(), PluginStateStoreError> {
        Self::publish_immutable(&self.blobs_dir().join(digest_hex(digest)), bytes)
    }

    fn read_blob(&self, digest: &ContentDigest) -> Result<Vec<u8>, PluginStateStoreError> {
        fs::read(self.blobs_dir().join(digest_hex(digest))).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => PluginStateStoreError::NotFound,
            _ => PluginStateStoreError::Failed(format!("read blob {digest}: {e}")),
        })
    }
}

// ── Session ──────────────────────────────────────────────────────

/// Inputs for one plugin-state save.
pub struct PluginStateSaveRequest<'a> {
    /// Live project — its `routing.processors` are captured in
    /// `ProcessorId` order.
    pub project: &'a Project,
    /// Live project revision the capture corresponds to.
    pub project_revision: u64,
    /// Project HEAD snapshot name at save time (`None` = unsaved).
    pub project_head: Option<String>,
    /// Requested capture guarantee.
    pub mode: CaptureMode,
    /// Save quality policy.
    pub policy: StateQualityPolicy,
    /// Timestamp token recorded on every record of this save.
    pub stored_at: String,
}

/// Inputs for one plugin-state restore.
pub struct PluginStateRestoreRequest<'a> {
    /// Explicit manifest name; `None` restores from HEAD.
    pub manifest: Option<String>,
    /// Live project — processors absent from the manifest are
    /// reported in `missing_states`.
    pub project: &'a Project,
}

/// Save/restore orchestration over a [`PluginStatePort`] +
/// [`PluginStateStore`]. All methods are non-RT.
pub struct PluginStateSession<P, S> {
    port: P,
    store: S,
}

impl<P: PluginStatePort, S: PluginStateStore> PluginStateSession<P, S> {
    /// A session over `port` + `store`.
    pub fn new(port: P, store: S) -> Self {
        Self { port, store }
    }

    /// The underlying port.
    #[must_use]
    pub const fn port(&self) -> &P {
        &self.port
    }

    /// Mutable port access (e.g. rebinding after a generation
    /// change).
    pub const fn port_mut(&mut self) -> &mut P {
        &mut self.port
    }

    /// The underlying store.
    #[must_use]
    pub const fn store(&self) -> &S {
        &self.store
    }

    /// Splits the session back into port + store.
    #[must_use]
    pub fn into_parts(self) -> (P, S) {
        (self.port, self.store)
    }

    /// Captures every processor's plugin state and publishes a
    /// manifest + blobs.
    ///
    /// Order: capture all → build manifest → `Strict` quality gate
    /// (rejects before any store write) → blob puts (indeterminate
    /// puts reconcile by readback; unreconciled aborts before the
    /// manifest publish, leaving HEAD untouched) → manifest publish
    /// → indeterminate publish reconciles by HEAD readback.
    ///
    /// # Errors
    /// [`PluginStateError::QualityRejected`] under `Strict` when any
    /// record is not exact-eligible; [`PluginStateError::Store`]
    /// when a publish effect cannot be proven; [`PluginStateError::Manifest`]
    /// on serialization failure.
    pub fn save(
        &mut self,
        request: &PluginStateSaveRequest<'_>,
    ) -> Result<PluginStateReceipt, PluginStateError> {
        let mut records = Vec::with_capacity(request.project.routing.processors.len());
        let mut blobs: Vec<(ContentDigest, Vec<u8>)> = Vec::new();
        for processor in request.project.routing.processors.values() {
            records.push(self.capture_record(processor, request, &mut blobs));
        }
        let manifest = PluginStateManifest {
            schema_version: PLUGIN_STATE_SCHEMA_VERSION,
            project_id: request.project.project_id,
            project_revision: request.project_revision,
            project_head: request.project_head.clone(),
            entries: records.clone(),
        };
        let summary = PluginStateQualitySummary::of(&records);
        if request.policy == StateQualityPolicy::Strict && !summary.exact_eligible() {
            return Err(PluginStateError::QualityRejected { summary });
        }
        let manifest_bytes = serde_json::to_vec(&manifest)
            .map_err(|e| PluginStateError::Manifest(format!("serialize: {e}")))?;
        let manifest_digest = ContentDigest::sha256(&manifest_bytes);

        for (digest, bytes) in &blobs {
            self.put_blob_reconciled(digest, bytes)?;
        }

        let (manifest_name, reconciled) = match self.store.save_manifest(&manifest_bytes) {
            Ok(name) => (Some(name), false),
            Err(PluginStateStoreError::Indeterminate) => {
                let name = self.reconcile_manifest(&manifest_bytes)?;
                (Some(name), true)
            }
            Err(e) => return Err(PluginStateError::Store(e)),
        };

        Ok(PluginStateReceipt {
            manifest_name,
            manifest_digest: Some(manifest_digest),
            project_revision: request.project_revision,
            project_head: request.project_head.clone(),
            policy: request.policy,
            records,
            summary,
            reconciled,
        })
    }

    /// Restores every captured record of a manifest through the
    /// port, verifying each blob against its recorded digest first.
    /// Live processors absent from the manifest land in
    /// `missing_states` — never silently skipped.
    ///
    /// # Errors
    /// [`PluginStateError::NotFound`] when no manifest exists;
    /// [`PluginStateError::Manifest`] on read/parse/schema failure;
    /// [`PluginStateError::Store`] on store read failures.
    pub fn restore(
        &mut self,
        request: &PluginStateRestoreRequest<'_>,
    ) -> Result<PluginStateRestoreReceipt, PluginStateError> {
        let name = match &request.manifest {
            Some(name) => name.clone(),
            None => self
                .store
                .head()
                .map_err(PluginStateError::Store)?
                .ok_or(PluginStateError::NotFound)?,
        };
        let bytes = self
            .store
            .read_manifest(&name)
            .map_err(PluginStateError::Store)?;
        let manifest_digest = ContentDigest::sha256(&bytes);
        let manifest: PluginStateManifest = serde_json::from_slice(&bytes)
            .map_err(|e| PluginStateError::Manifest(format!("parse {name}: {e}")))?;
        if manifest.schema_version != PLUGIN_STATE_SCHEMA_VERSION {
            return Err(PluginStateError::Manifest(format!(
                "{name}: unsupported schema_version {}",
                manifest.schema_version
            )));
        }

        let mut records = Vec::with_capacity(manifest.entries.len());
        for entry in &manifest.entries {
            records.push(PluginStateRestoreRecord {
                processor_id: entry.processor_id,
                outcome: self.restore_record(entry),
            });
        }
        let manifest_processors: std::collections::BTreeSet<ProcessorId> = manifest
            .entries
            .iter()
            .map(|entry| entry.processor_id)
            .collect();
        let missing_states = request
            .project
            .routing
            .processors
            .keys()
            .filter(|id| !manifest_processors.contains(id))
            .copied()
            .collect();

        Ok(PluginStateRestoreReceipt {
            manifest_name: name,
            manifest_digest,
            records,
            missing_states,
        })
    }

    /// One save record: an `Unresolved` binding or absent live
    /// instance → `Failed(Unavailable)`; port `Unsupported`/`Failed`
    /// map to the matching blob-less outcome; `Ok` digests the blob
    /// for later publish. The port is never asked to capture an
    /// `Unresolved` binding — there is no live instance to ask.
    fn capture_record(
        &mut self,
        processor: &crate::routing::Processor,
        request: &PluginStateSaveRequest<'_>,
        blobs: &mut Vec<(ContentDigest, Vec<u8>)>,
    ) -> PluginStateRecord {
        let binding = self.port.describe(processor.id);
        let unresolved = matches!(
            processor.binding,
            crate::routing::ProcessorBinding::Unresolved { .. }
        );
        let outcome = if unresolved || binding.is_none() {
            PluginStateOutcome::Failed {
                kind: PluginStateFailureKind::Unavailable,
            }
        } else {
            match self.port.capture(processor.id, request.mode) {
                Err(PluginStateCaptureError::Unsupported) => PluginStateOutcome::Unsupported,
                Err(PluginStateCaptureError::Failed(kind)) => PluginStateOutcome::Failed { kind },
                Ok(capture) => {
                    let digest = ContentDigest::sha256(&capture.blob);
                    blobs.push((digest, capture.blob.clone()));
                    PluginStateOutcome::Captured {
                        quality: capture.achieved,
                        blob_digest: digest,
                        blob_len: capture.blob.len() as u64,
                    }
                }
            }
        };
        PluginStateRecord {
            processor_id: processor.id,
            binding,
            project_revision: request.project_revision,
            stored_at: request.stored_at.clone(),
            outcome,
            detail: None,
        }
    }

    /// `put_blob` with indeterminate reconciliation: an
    /// `Indeterminate` put is proven by reading the blob back and
    /// re-hashing; an unproven put aborts the save *before* the
    /// manifest publish, so HEAD still points at the previous
    /// manifest (old state/recovery blob protected).
    fn put_blob_reconciled(
        &mut self,
        digest: &ContentDigest,
        bytes: &[u8],
    ) -> Result<(), PluginStateError> {
        match self.store.put_blob(digest, bytes) {
            Ok(()) => Ok(()),
            Err(PluginStateStoreError::Indeterminate) => match self.store.read_blob(digest) {
                Ok(existing) if ContentDigest::sha256(&existing) == *digest => Ok(()),
                _ => Err(PluginStateError::Store(
                    PluginStateStoreError::Indeterminate,
                )),
            },
            Err(e) => Err(PluginStateError::Store(e)),
        }
    }

    /// Reconciles an indeterminate manifest publish: HEAD must name
    /// a manifest whose bytes equal what we tried to publish.
    /// Anything else stays an error — indeterminate is never
    /// asserted as success or failure without readback.
    fn reconcile_manifest(&mut self, bytes: &[u8]) -> Result<String, PluginStateError> {
        let expected = plugin_state_manifest_name(bytes);
        let head =
            self.store
                .head()
                .map_err(PluginStateError::Store)?
                .ok_or(PluginStateError::Store(
                    PluginStateStoreError::Indeterminate,
                ))?;
        if head != expected {
            return Err(PluginStateError::Store(
                PluginStateStoreError::Indeterminate,
            ));
        }
        let stored = self
            .store
            .read_manifest(&head)
            .map_err(PluginStateError::Store)?;
        if stored != bytes {
            return Err(PluginStateError::Store(
                PluginStateStoreError::Indeterminate,
            ));
        }
        Ok(head)
    }

    /// One restore record: blob-less save outcomes restore as
    /// `NotCaptured`/`Unsupported`; captured records verify the
    /// blob digest (`Corrupt` on mismatch, `Missing` on absent
    /// blob) before the port sees any bytes.
    fn restore_record(&mut self, entry: &PluginStateRecord) -> PluginStateRestoreOutcome {
        let PluginStateOutcome::Captured { blob_digest, .. } = &entry.outcome else {
            return match &entry.outcome {
                PluginStateOutcome::Unsupported => PluginStateRestoreOutcome::Unsupported,
                PluginStateOutcome::Failed { .. } => PluginStateRestoreOutcome::NotCaptured,
                PluginStateOutcome::Captured { .. } => unreachable!(),
            };
        };
        let blob = match self.store.read_blob(blob_digest) {
            Ok(blob) => blob,
            Err(PluginStateStoreError::NotFound) => {
                return PluginStateRestoreOutcome::Failed {
                    kind: PluginStateFailureKind::Missing,
                };
            }
            Err(_) => {
                return PluginStateRestoreOutcome::Failed {
                    kind: PluginStateFailureKind::CaptureFailed,
                };
            }
        };
        if ContentDigest::sha256(&blob) != *blob_digest {
            return PluginStateRestoreOutcome::Failed {
                kind: PluginStateFailureKind::Corrupt,
            };
        }
        match self.port.restore(entry.processor_id, &blob) {
            Ok(()) => PluginStateRestoreOutcome::Restored,
            Err(PluginStateCaptureError::Unsupported) => PluginStateRestoreOutcome::Unsupported,
            Err(PluginStateCaptureError::Failed(kind)) => {
                PluginStateRestoreOutcome::Failed { kind }
            }
        }
    }
}
