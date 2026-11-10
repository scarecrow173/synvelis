//! Persistence: the `ProjectStore` port plus two implementations.
//!
//! The snapshot envelope is strict (`deny_unknown_fields`), versioned, and
//! serialized deterministically (declaration-ordered fields, `BTreeMap`
//! sorted keys). `DirectoryBundleStore` follows the architecture's save
//! protocol through `ruviform-io-file`'s staged publish:
//! `StagedFile` → `SealedFile` → classified `publish` — snapshots are
//! immutable (`PublishMode::CreateNew`), HEAD is atomically replaced.
//! An accepted same-name collision re-publishes the kept bytes
//! (`Replace`): the save only completes when the snapshot's own
//! directory-entry durability is (re-)established, so an existing
//! name is never trusted as proof of it.
//! A failure reports its classified effect: `NotPublished` leaves the
//! previous HEAD intact, while `Published`/`Indeterminate` may have
//! already repointed it — `head()` remains the honest readback.

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use ruviform_io_file::{
    Durability, PublishEffect, PublishFailure, PublishFailureKind, PublishMode, PublishOptions,
    StagedFile,
};
use serde::{Deserialize, Serialize};

use crate::{controller::ProjectRevision, error::Error, id::ProjectId, project::Project};

/// The schema version this build writes. Reading accepts every version
/// reachable through `crate::migration`'s ordered step registry.
/// v2 adds `Project::control` (the issue-#40 Parameter Control Model);
/// v3 replaces the clip anchor/offset pair with the normalized
/// placement+playback contract and adds the comping registry (MD-A).
/// v4 adds `Project::notes` — the clip-local note entities anchoring
/// `ControlOwner::Note` (issue #43). v5 adds `Project::midi_sources` —
/// project-owned MIDI note sequences sharing the `SourceId` space
/// (issue #52). v6 adds `Project::routing` — the Semantic Routing
/// Model (issue #104); v5 bodies are read through the ordered
/// migration registry which projects the legacy clip→master route.
/// v7 splits `ControlScope` into `ControlOwner` +
/// `AutomationTimeScope` + `ParameterRole` (issue #121); v6 bodies are
/// read through the same registry which projects the legacy scope
/// into owner + derived time scope. v8 seeds the bootstrap
/// master-strip fader `ParameterTarget` (synvelis#185); v7 bodies are
/// read through the same registry which inserts the well-known
/// target when absent. v9 adds `Project::smf_documents` — SMF
/// provenance documents for imported MIDI sources (issue #126); v8
/// bodies are read through the same registry which needs no
/// projection (the map defaults empty). v10 registers the strip
/// channel parameter set — `Fader`/`Pan`/`Mute`/`Solo`
/// `ParameterTarget`s on every strip (issue #122); v9 bodies are read
/// through the same registry which seeds the missing roles. v11 types
/// the asset-digest contract — `Asset.digest`/`Source.content_digest`/
/// `AppliedImport.content_hash` stay verbatim strings classified by
/// `project::AssetDigest`, and applied-import receipts must name live
/// asset/source entities with non-empty digests (synvelis#86); v10
/// and older bodies are read through the same registry which enforces
/// that integrity floor without rewriting any digest.
/// New maps are `#[serde(default)]`, so an older body still
/// deserializes; each version bump records the intentional format
/// advance. v1–v4 snapshots are not migrated.
pub const SCHEMA_VERSION: u32 = 11;

/// Versioned snapshot wrapper. Unknown fields are rejected on load so a
/// newer format fails loudly instead of being silently degraded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotEnvelope {
    /// Snapshot schema version (must equal [`SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// The project's stable identity.
    pub project_id: ProjectId,
    /// Live revision at save time.
    pub revision: ProjectRevision,
    /// The complete project aggregate.
    pub project: Project,
}

/// Persistence port: stores canonical snapshot bytes and a HEAD pointer.
pub trait ProjectStore {
    /// Persists `bytes` immutably and repoints HEAD, returning the
    /// snapshot name.
    ///
    /// # Errors
    /// Any I/O/store failure. A `NotPublished` effect leaves the
    /// previous HEAD intact; `Published`/`Indeterminate` effects mean
    /// HEAD may already have been repointed — re-read `head()` to
    /// reconcile instead of assuming the old pointer.
    fn save(
        &mut self,
        project_id: &str,
        revision: ProjectRevision,
        bytes: &[u8],
    ) -> Result<String, Error>;

    /// The snapshot name HEAD currently points at.
    ///
    /// # Errors
    /// Store read failures.
    fn head(&self) -> Result<Option<String>, Error>;

    /// Reads a snapshot by name.
    ///
    /// # Errors
    /// Missing snapshot or read failure.
    fn read_snapshot(&self, name: &str) -> Result<Vec<u8>, Error>;
}

/// Deterministic snapshot file name for a revision.
#[must_use]
pub fn snapshot_name(revision: ProjectRevision) -> String {
    format!("r{}.json", revision.0)
}

/// Parses a canonical snapshot name back into its revision — the
/// inverse of [`snapshot_name`]. Non-canonical names (`r01.json`,
/// foreign prefixes, missing suffix) return `None` rather than being
/// silently accepted.
#[must_use]
pub fn snapshot_revision(name: &str) -> Option<ProjectRevision> {
    let rev = name
        .strip_prefix('r')?
        .strip_suffix(".json")?
        .parse::<u64>()
        .ok()?;
    (snapshot_name(ProjectRevision(rev)) == name).then_some(ProjectRevision(rev))
}

/// The revision the store's HEAD snapshot records.
///
/// `Ok(None)` means the store has no HEAD (never saved). A HEAD that
/// exists but does not name a canonical `r<rev>.json` snapshot is a
/// store integrity problem and returns [`Error::Store`] — callers must
/// not treat it as "unsaved".
///
/// # Errors
/// Store read failures or a non-canonical HEAD snapshot name.
pub fn head_revision(store: &dyn ProjectStore) -> Result<Option<ProjectRevision>, Error> {
    match store.head()? {
        None => Ok(None),
        Some(name) => snapshot_revision(&name)
            .map(Some)
            .ok_or_else(|| Error::Store(format!("HEAD names non-canonical snapshot: {name}"))),
    }
}

/// How an existing snapshot at the target name relates to incoming
/// bytes.
enum SnapshotCollision {
    /// Byte-identical, or the same logical snapshot whose stored
    /// `id_watermark` is at least the incoming one — the stored
    /// bytes are the winner. The durable-publish guarantee is still
    /// owed: existence proves nothing about directory-entry
    /// durability, so the stored bytes are re-published.
    Satisfied,
    /// The same logical snapshot — [`SnapshotEnvelope`] identical
    /// modulo `Project::id_watermark` — carrying a newer watermark.
    /// The incoming bytes are the winner: they refresh the file so
    /// the monotone watermark record is not lost.
    Refresh,
    /// Different content under the immutable name — a real conflict.
    Conflict,
}

/// Classifies an existing snapshot's bytes against an incoming save.
///
/// `ProjectController::save` stamps `id_watermark = generator.peek()`
/// into the envelope, so `draw_raw_id` reservations made since the
/// last apply change the canonical bytes of the *same* revision —
/// and a journaled committed state can likewise differ from what a
/// crashed save published. Two envelopes that differ only in that
/// bookkeeping field are the same logical snapshot: the watermark
/// record is monotone, so the file may advance to the newer bytes
/// ([`SnapshotCollision::Refresh`]) or keep the newer record it
/// already holds ([`SnapshotCollision::Satisfied`]). Genuinely
/// different content under the same immutable name — a stale writer,
/// a foreign project, or corrupt bytes — is a real conflict.
///
/// The classification only decides *which bytes* the immutable name
/// must end up holding; both accepted outcomes still owe the
/// `FileAndDirectorySync` publish before HEAD may advance — the
/// file's prior existence is no proof the earlier publish ever
/// completed its directory sync.
fn snapshot_collision(existing: &[u8], incoming: &[u8]) -> SnapshotCollision {
    if existing == incoming {
        return SnapshotCollision::Satisfied;
    }
    let (Ok(mut stored), Ok(mut next)) = (
        serde_json::from_slice::<SnapshotEnvelope>(existing),
        serde_json::from_slice::<SnapshotEnvelope>(incoming),
    ) else {
        return SnapshotCollision::Conflict;
    };
    let stored_watermark = stored.project.id_watermark;
    let next_watermark = next.project.id_watermark;
    stored.project.id_watermark = 0;
    next.project.id_watermark = 0;
    if stored != next {
        return SnapshotCollision::Conflict;
    }
    if next_watermark > stored_watermark {
        SnapshotCollision::Refresh
    } else {
        SnapshotCollision::Satisfied
    }
}

/// Directory-bundle store: `<root>/snapshots/<name>` + `<root>/HEAD`.
pub struct DirectoryBundleStore {
    root: PathBuf,
}

impl DirectoryBundleStore {
    /// Opens (creating if needed) a bundle directory.
    ///
    /// # Errors
    /// I/O failures creating the directory tree.
    pub fn create(root: impl AsRef<Path>) -> Result<Self, Error> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join("snapshots"))
            .map_err(|e| Error::Store(format!("create bundle: {e}")))?;
        Ok(Self { root })
    }

    /// Stages `bytes` and publishes at `path` with the requested
    /// `mode` — file sync + atomic publish + directory sync
    /// (`Durability::FileAndDirectorySync`), atomic visibility
    /// required.
    ///
    /// `Ok` is returned only when every requested guarantee was
    /// achieved; failures keep the [`PublishFailure`] classification
    /// so the caller can decide recovery before mapping to
    /// [`Error::Store`].
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
        // A retained temp after a successful publish is an orphan;
        // best-effort cleanup never changes the publish result.
        if let Some(handle) = receipt.take_recovery_handle() {
            let _ = handle.cleanup();
        }
        Ok(())
    }

    /// Maps a staged-publish failure into [`Error::Store`],
    /// transcribing the full `PublishFailure` classification
    /// (phase/kind/effect/achieved durability/operation id) plus the
    /// staged temp path.
    ///
    /// A `NotPublished`/`Published` leftover is cleaned up
    /// best-effort; an `Indeterminate` leftover is preserved for
    /// recovery and never auto-deleted (the `ruviform-io-file`
    /// contract).
    fn publish_error(path: &Path, error: PublishStepError) -> Error {
        match error {
            PublishStepError::Stage(e) => {
                Error::Store(format!("stage create {}: {e}", path.display()))
            }
            PublishStepError::Write { source, temp } => {
                let temp = temp.map_or_else(|| "unknown".to_owned(), |p| p.display().to_string());
                Error::Store(format!(
                    "stage write {}: {source}; temp {temp}",
                    path.display()
                ))
            }
            PublishStepError::Publish(mut failure) => {
                if failure.effect() != PublishEffect::Indeterminate
                    && let Some(handle) = failure.take_recovery_handle()
                {
                    let _ = handle.cleanup();
                }
                let temp = failure
                    .info()
                    .temp_path()
                    .map_or_else(|| "unknown".to_owned(), |p| p.display().to_string());
                let cause = std::error::Error::source(&failure)
                    .map_or_else(String::new, |s| format!("; cause {s}"));
                Error::Store(format!(
                    "publish {}: {failure}; temp {temp}{cause}",
                    path.display()
                ))
            }
        }
    }

    fn snapshots_dir(&self) -> PathBuf {
        self.root.join("snapshots")
    }
}

/// Failure inside [`DirectoryBundleStore::publish_staged`].
///
/// `Publish` keeps the `ruviform-io-file` [`PublishFailure`] intact so
/// the store can classify before flattening to [`Error::Store`].
enum PublishStepError {
    /// `StagedFile::create` failed before any bytes were written; the
    /// destination was never touched.
    Stage(ruviform_io_file::FileError),
    /// Writing the staged temp failed; the dropped temp is cleaned
    /// best-effort by the staged file's `Drop`.
    Write {
        /// The underlying write failure.
        source: std::io::Error,
        /// The staged temp path at failure time (cleaned on drop).
        temp: Option<PathBuf>,
    },
    /// `SealedFile::publish` failed — effect/phase/durability and the
    /// recovery handle are preserved.
    Publish(PublishFailure),
}

impl ProjectStore for DirectoryBundleStore {
    fn save(
        &mut self,
        _project_id: &str,
        revision: ProjectRevision,
        bytes: &[u8],
    ) -> Result<String, Error> {
        let name = snapshot_name(revision);
        let target = self.snapshots_dir().join(&name);
        match Self::publish_staged(&target, bytes, PublishMode::CreateNew) {
            Ok(()) => {}
            // Immutable snapshots are idempotent under retry: an
            // existing snapshot holding the same logical content is
            // accepted (a crash between the original snapshot
            // publish and the HEAD repoint leaves an orphan that
            // recovery legitimately restores to, and `id_watermark`
            // bookkeeping may advance the same revision's bytes —
            // see `snapshot_collision`). Acceptance is not a skip:
            // the kept bytes are re-published so this save still
            // owes — and re-establishes — the full durability
            // guarantee on the snapshot's own directory entry.
            Err(PublishStepError::Publish(mut failure))
                if failure.kind() == PublishFailureKind::DestinationExists =>
            {
                if failure.effect() != PublishEffect::Indeterminate
                    && let Some(handle) = failure.take_recovery_handle()
                {
                    let _ = handle.cleanup();
                }
                match fs::read(&target) {
                    Err(e) => {
                        // The refused publish left the destination
                        // untouched but we cannot prove equivalence —
                        // surface the read error rather than masking
                        // it behind the collision.
                        return Err(Error::Store(format!(
                            "publish {}: {failure}; snapshot exists but could not be read to resolve: {e}",
                            target.display()
                        )));
                    }
                    Ok(existing) => {
                        // Content-verified winner bytes: the stored
                        // bytes for `Satisfied` (byte-identical, or
                        // the stored newer watermark is kept), the
                        // incoming bytes for `Refresh`. Accepting
                        // the collision re-publishes the winner
                        // through the full staged pipeline — the only
                        // public operation that re-establishes
                        // `FileAndDirectorySync` for this directory
                        // entry. A bare existing name is never taken
                        // as proof of achieved durability, and a
                        // failed re-publish propagates instead of
                        // letting HEAD advance onto an undurable name.
                        let winning: &[u8] = match snapshot_collision(&existing, bytes) {
                            SnapshotCollision::Satisfied => existing.as_slice(),
                            SnapshotCollision::Refresh => bytes,
                            SnapshotCollision::Conflict => {
                                return Err(Self::publish_error(
                                    &target,
                                    PublishStepError::Publish(failure),
                                ));
                            }
                        };
                        Self::publish_staged(&target, winning, PublishMode::Replace)
                            .map_err(|e| Self::publish_error(&target, e))?;
                    }
                }
            }
            Err(e) => return Err(Self::publish_error(&target, e)),
        }
        Self::publish_staged(
            &self.root.join("HEAD"),
            name.as_bytes(),
            PublishMode::Replace,
        )
        .map_err(|e| Self::publish_error(&self.root.join("HEAD"), e))?;
        Ok(name)
    }

    fn head(&self) -> Result<Option<String>, Error> {
        match fs::read_to_string(self.root.join("HEAD")) {
            Ok(name) => Ok(Some(name.trim().to_owned())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::Store(e.to_string())),
        }
    }

    fn read_snapshot(&self, name: &str) -> Result<Vec<u8>, Error> {
        if name.contains(['/', '\\']) || name.contains("..") {
            return Err(Error::Store(format!("invalid snapshot name: {name}")));
        }
        fs::read(self.snapshots_dir().join(name))
            .map_err(|e| Error::Store(format!("read snapshot {name}: {e}")))
    }
}

/// In-memory store holding canonical snapshot bytes (round-trips through
/// the same serialization path as the directory store).
#[derive(Default)]
pub struct InMemoryStore {
    snapshots: BTreeMap<String, Vec<u8>>,
    head: Option<String>,
}

impl InMemoryStore {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ProjectStore for InMemoryStore {
    fn save(
        &mut self,
        _project_id: &str,
        revision: ProjectRevision,
        bytes: &[u8],
    ) -> Result<String, Error> {
        let name = snapshot_name(revision);
        // The immutable-name contract of `DirectoryBundleStore`:
        // a satisfied collision keeps the stored bytes, a refresh
        // stores the newer-watermark bytes, a conflict is rejected.
        // (The directory store additionally re-publishes the kept
        // bytes to re-establish FS directory-entry durability —
        // in memory there is no such dimension.)
        match self
            .snapshots
            .get(&name)
            .map(|existing| snapshot_collision(existing, bytes))
        {
            None | Some(SnapshotCollision::Satisfied) => {}
            Some(SnapshotCollision::Refresh) => {
                self.snapshots.insert(name.clone(), bytes.to_vec());
            }
            Some(SnapshotCollision::Conflict) => {
                return Err(Error::Store(format!("snapshot {name} already exists")));
            }
        }
        self.snapshots
            .entry(name.clone())
            .or_insert_with(|| bytes.to_vec());
        self.head = Some(name.clone());
        Ok(name)
    }

    fn head(&self) -> Result<Option<String>, Error> {
        Ok(self.head.clone())
    }

    fn read_snapshot(&self, name: &str) -> Result<Vec<u8>, Error> {
        self.snapshots
            .get(name)
            .cloned()
            .ok_or_else(|| Error::Store(format!("missing snapshot {name}")))
    }
}
