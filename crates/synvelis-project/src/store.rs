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
    fmt, fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
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
/// that integrity floor without rewriting any digest. v12 adds
/// `Project::monitoring` — authored Arm/Monitor/channel-map/lane
/// declarations and punch marks (synvelis#117); v11 and older bodies
/// are read through the same registry which needs no projection (the
/// map defaults empty — v11 carried no monitor declarations). v13 adds
/// `Project::plugin_instances` — the persisted plugin-instance
/// identity store with its own monotonic u64 watermark (Issue #101,
/// PC-C4-05); v12 and older bodies are read through the same registry
/// which backfills one identity per `Resolved` processor in
/// `ProcessorId` order.
/// New maps are `#[serde(default)]`, so an older body still
/// deserializes; each version bump records the intentional format
/// advance. v1–v4 snapshots are not migrated.
pub const SCHEMA_VERSION: u32 = 13;

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

/// How long a `HEAD.lock` file is trusted as held by a live writer
/// before a contender may break it as stale (syn-io-02-head-cas).
///
/// The lock guards only the read-compare-publish critical section —
/// two staged publishes of small files — so tens of seconds is
/// already far beyond any legitimate hold. A crashed writer's lock
/// becomes breakable once its recorded creation time is older than
/// this.
pub const HEAD_LOCK_STALE_AFTER: Duration = Duration::from_secs(20);

/// How long [`ProjectStore::save_cas`] waits for a *fresh* `HEAD.lock`
/// before reporting [`HeadCasError::Store`]. Deliberately longer than
/// [`HEAD_LOCK_STALE_AFTER`] so a wedged lock is broken and the call
/// proceeds within a single attempt instead of erroring first.
pub const HEAD_LOCK_BUSY_TIMEOUT: Duration = Duration::from_secs(30);

/// Poll interval while waiting on a held `HEAD.lock`.
const HEAD_LOCK_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Monotonic suffix for unique lock tokens and stale-lock aside names.
static LOCK_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Failure of [`ProjectStore::save_cas`].
///
/// Unlike [`Error`], this type keeps the expected-head conflict
/// structured so a caller can distinguish "HEAD moved under me"
/// (retryable after reload) from ordinary store failures — a CAS
/// conflict is never reported as success, and never disguised as a
/// generic I/O error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadCasError {
    /// The store's HEAD no longer names `expected` — a competing
    /// writer (or external edit) moved it between the caller's read
    /// and this save. `actual` is the HEAD observed inside the
    /// critical section (`None` = absent). No HEAD repoint happened;
    /// the snapshot publish is skipped entirely on this path, so no
    /// orphan is left behind either.
    Conflict {
        /// The HEAD name the caller asserted (`None` = asserted absent).
        expected: Option<String>,
        /// The HEAD name actually observed (`None` = absent).
        actual: Option<String>,
    },
    /// The store does not implement expected-head CAS — the default
    /// [`ProjectStore::save_cas`] body. Callers that require the
    /// guarantee must use a store known to implement it
    /// ([`DirectoryBundleStore`], [`InMemoryStore`]).
    Unsupported,
    /// Any other failure: lock acquisition, snapshot publish, HEAD
    /// publish, or I/O. The wrapped [`Error`] keeps the existing
    /// store classification (publish effect/phase/durability).
    Store(Error),
}

impl fmt::Display for HeadCasError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict { expected, actual } => write!(
                f,
                "head CAS conflict: expected {expected:?}, actual {actual:?}"
            ),
            Self::Unsupported => write!(f, "store does not support head CAS"),
            Self::Store(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for HeadCasError {}

impl From<Error> for HeadCasError {
    fn from(err: Error) -> Self {
        Self::Store(err)
    }
}

impl From<HeadCasError> for Error {
    fn from(err: HeadCasError) -> Self {
        match err {
            HeadCasError::Conflict { expected, actual } => Self::Store(format!(
                "head CAS conflict: expected {expected:?}, actual {actual:?}"
            )),
            HeadCasError::Unsupported => Self::Store("store does not support head CAS".to_owned()),
            HeadCasError::Store(e) => e,
        }
    }
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

    /// Persists `bytes` immutably and repoints HEAD **only if** HEAD
    /// still names `expected_head` — the expected-head compare-and-swap
    /// (syn-io-02-head-cas).
    ///
    /// `expected_head` is exactly what [`Self::head`] reports: the
    /// snapshot name HEAD currently points at, or `None` to assert
    /// HEAD is absent. The comparison happens inside the store's
    /// mutual-exclusion boundary, so two cooperating `save_cas`
    /// callers on the same root cannot both observe the same expected
    /// HEAD and both commit — the loser receives
    /// [`HeadCasError::Conflict`] instead of silently repointing HEAD
    /// (the lost-update `save` cannot prevent).
    ///
    /// The default implementation returns
    /// [`HeadCasError::Unsupported`]; stores that cannot honor the
    /// precondition must not silently degrade to unconditional
    /// `save`. [`DirectoryBundleStore`] implements the contract with a
    /// `HEAD.lock` create-new mutex plus stale-lock breaking;
    /// [`InMemoryStore`] mirrors the same compare semantics.
    ///
    /// # Errors
    /// [`HeadCasError::Conflict`] when HEAD moved, `Unsupported` when
    /// the store lacks CAS, [`HeadCasError::Store`] for any other
    /// failure — same publish-effect honesty rules as [`Self::save`]
    /// (`NotPublished` leaves HEAD intact; `Published`/
    /// `Indeterminate` may already have repointed it — re-read
    /// `head()`).
    fn save_cas(
        &mut self,
        project_id: &str,
        expected_head: Option<&str>,
        revision: ProjectRevision,
        bytes: &[u8],
    ) -> Result<String, HeadCasError> {
        let _ = (project_id, expected_head, revision, bytes);
        Err(HeadCasError::Unsupported)
    }

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
    /// `mode`/`durability` — atomic visibility required
    /// (`VisibilityRequirement::RequireAtomic`).
    ///
    /// `Ok` is returned only when every requested guarantee was
    /// achieved; failures keep the [`PublishFailure`] classification
    /// so the caller can decide recovery before mapping to
    /// [`Error::Store`].
    fn publish_staged(
        path: &Path,
        bytes: &[u8],
        mode: PublishMode,
        durability: Durability,
    ) -> Result<(), PublishStepError> {
        let mut staged = StagedFile::create(path).map_err(PublishStepError::Stage)?;
        if let Err(source) = staged.write_all(bytes) {
            let temp = staged.temp_path().map(Path::to_path_buf);
            return Err(PublishStepError::Write { source, temp });
        }
        let mut receipt = staged
            .seal()
            .publish(PublishOptions::new(mode, durability))
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

    /// Publishes the immutable snapshot `r<revision>.json` and
    /// returns its name. Shared by the unconditional `save` path and
    /// the CAS path — the collision/durability rules are identical;
    /// only the HEAD precondition differs.
    fn publish_snapshot(&self, revision: ProjectRevision, bytes: &[u8]) -> Result<String, Error> {
        let name = snapshot_name(revision);
        let target = self.snapshots_dir().join(&name);
        match Self::publish_staged(
            &target,
            bytes,
            PublishMode::CreateNew,
            Durability::FileAndDirectorySync,
        ) {
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
                        Self::publish_staged(
                            &target,
                            winning,
                            PublishMode::Replace,
                            Durability::FileAndDirectorySync,
                        )
                        .map_err(|e| Self::publish_error(&target, e))?;
                    }
                }
            }
            Err(e) => return Err(Self::publish_error(&target, e)),
        }
        Ok(name)
    }

    /// Repoints `HEAD` at `name` — the unconditional `Replace`
    /// publish shared by `save` and the CAS critical section.
    fn publish_head(&self, name: &str) -> Result<(), Error> {
        let path = self.root.join("HEAD");
        Self::publish_staged(
            &path,
            name.as_bytes(),
            PublishMode::Replace,
            Durability::FileAndDirectorySync,
        )
        .map_err(|e| Self::publish_error(&path, e))
    }

    // ----- HEAD lock (save_cas mutual exclusion) -----
    //
    // `HEAD.lock` is acquired with an atomic `CreateNew` publish —
    // the only no-replace primitive the File API exposes — and held
    // across the read-compare-publish critical section. The lock file
    // carries `created_ns` + a unique `token`; a contender that finds
    // it may break it once it is older than [`HEAD_LOCK_STALE_AFTER`]
    // by renaming it aside (atomic claim — only one contender's
    // rename wins) and deleting the aside. Release is
    // verify-then-delete so a broken-and-recreated lock belonging to
    // another writer is never removed.

    /// Path of the HEAD mutual-exclusion lock file.
    fn head_lock_path(&self) -> PathBuf {
        self.root.join("HEAD.lock")
    }

    /// Freshness classification of an existing `HEAD.lock`.
    fn head_lock_state(path: &Path) -> HeadLockState {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return HeadLockState::Gone,
            // Unreadable (permissions, not-a-file, …) cannot prove a
            // live holder — treat as breakable; the break itself
            // surfaces real rename errors.
            Err(_) => return HeadLockState::Stale,
        };
        let created_ns = content
            .lines()
            .find_map(|line| line.strip_prefix("created_ns=")?.parse::<u128>().ok());
        let Some(created_ns) = created_ns else {
            // Malformed content cannot prove freshness — breakable.
            return HeadLockState::Stale;
        };
        let age = now_unix_nanos().saturating_sub(created_ns);
        if age > HEAD_LOCK_STALE_AFTER.as_nanos() {
            HeadLockState::Stale
        } else {
            HeadLockState::Held
        }
    }

    /// Claims a stale/absent `HEAD.lock` by renaming it to a unique
    /// aside name and deleting the aside. The rename is the atomic
    /// arbitration: only one contender moves the source, the rest see
    /// `NotFound` and simply retry `CreateNew`.
    fn break_head_lock(path: &Path) -> Result<(), Error> {
        let aside = path.with_file_name(format!(
            "HEAD.lock.stale-{}-{}-{}",
            std::process::id(),
            LOCK_COUNTER.fetch_add(1, Ordering::Relaxed),
            now_unix_nanos()
        ));
        match fs::rename(path, &aside) {
            Ok(()) => {
                // The aside is normally the lock file; remove_dir_all
                // covers the degenerate case of `HEAD.lock` being a
                // directory so no `*.stale-*` garbage is left behind.
                let _ = fs::remove_file(&aside).or_else(|_| fs::remove_dir_all(&aside));
                Ok(())
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Error::Store(format!(
                "break stale HEAD lock {}: {e}",
                path.display()
            ))),
        }
    }

    /// Acquires `HEAD.lock`, waiting out fresh holders and breaking
    /// stale ones, until [`HEAD_LOCK_BUSY_TIMEOUT`] elapses.
    fn acquire_head_lock(&self) -> Result<HeadLock, HeadCasError> {
        let path = self.head_lock_path();
        let deadline = Instant::now() + HEAD_LOCK_BUSY_TIMEOUT;
        loop {
            let token = format!(
                "{}-{}-{}",
                std::process::id(),
                LOCK_COUNTER.fetch_add(1, Ordering::Relaxed),
                now_unix_nanos()
            );
            let content = format!(
                "synvelis-head-lock/1\ncreated_ns={}\ntoken={token}\n",
                now_unix_nanos()
            );
            match Self::publish_staged(
                &path,
                content.as_bytes(),
                PublishMode::CreateNew,
                // The lock is advisory mutual exclusion, not durable
                // state — a crash may lose the name entirely, which
                // is exactly the stale-lock case the breaker handles.
                Durability::BestEffort,
            ) {
                Ok(()) => return Ok(HeadLock { path, token }),
                Err(PublishStepError::Publish(mut failure))
                    if failure.kind() == PublishFailureKind::DestinationExists =>
                {
                    if failure.effect() != PublishEffect::Indeterminate
                        && let Some(handle) = failure.take_recovery_handle()
                    {
                        let _ = handle.cleanup();
                    }
                    match Self::head_lock_state(&path) {
                        HeadLockState::Held => {
                            if Instant::now() >= deadline {
                                return Err(HeadCasError::Store(Error::Store(format!(
                                    "HEAD lock busy: {}",
                                    path.display()
                                ))));
                            }
                            std::thread::sleep(HEAD_LOCK_POLL_INTERVAL);
                        }
                        HeadLockState::Stale => {
                            Self::break_head_lock(&path).map_err(HeadCasError::Store)?;
                        }
                        HeadLockState::Gone => {}
                    }
                }
                Err(e) => {
                    return Err(HeadCasError::Store(Self::publish_error(&path, e)));
                }
            }
        }
    }
}

/// Freshness of an existing `HEAD.lock` file.
enum HeadLockState {
    /// Created recently enough to belong to a live writer.
    Held,
    /// Older than [`HEAD_LOCK_STALE_AFTER`], malformed, or unreadable
    /// — safe to break via rename-aside.
    Stale,
    /// Vanished between the failed `CreateNew` and the read.
    Gone,
}

/// Unix epoch nanoseconds — the lock file's creation timestamp.
fn now_unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

/// A held `HEAD.lock`. Dropping verifies the file still carries this
/// writer's token before deleting — a lock that was broken as stale
/// and recreated by another writer is never removed by the loser.
struct HeadLock {
    /// Lock file path (`<root>/HEAD.lock`).
    path: PathBuf,
    /// Unique token this holder published.
    token: String,
}

impl HeadLock {
    /// Whether `HEAD.lock` still names this holder's token — the
    /// fencing check run before repointing HEAD.
    fn still_ours(&self) -> bool {
        fs::read_to_string(&self.path).is_ok_and(|content| {
            content
                .lines()
                .any(|line| line.strip_prefix("token=") == Some(self.token.as_str()))
        })
    }
}

impl Drop for HeadLock {
    fn drop(&mut self) {
        if self.still_ours() {
            let _ = fs::remove_file(&self.path);
        }
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
        // Unconditional publish — the single-writer/journal-restore
        // path. Callers that need lost-update protection use
        // `save_cas`; this method deliberately takes no lock and
        // keeps its historical semantics.
        let name = self.publish_snapshot(revision, bytes)?;
        self.publish_head(&name)?;
        Ok(name)
    }

    fn save_cas(
        &mut self,
        _project_id: &str,
        expected_head: Option<&str>,
        revision: ProjectRevision,
        bytes: &[u8],
    ) -> Result<String, HeadCasError> {
        // The whole compare-and-swap runs inside the HEAD.lock
        // critical section: acquire → read HEAD → compare → publish
        // snapshot → fence-check → publish HEAD → release. Two
        // cooperating save_cas callers on the same root serialize
        // here, so the second always observes the first's committed
        // HEAD and reports Conflict instead of repointing blindly.
        let lock = self.acquire_head_lock()?;
        let actual = self.head().map_err(HeadCasError::Store)?;
        if actual.as_deref() != expected_head {
            return Err(HeadCasError::Conflict {
                expected: expected_head.map(str::to_owned),
                actual,
            });
        }
        let name = self
            .publish_snapshot(revision, bytes)
            .map_err(HeadCasError::Store)?;
        // Fencing: if the lock was broken as stale while the
        // snapshot published, another writer may already have
        // committed — abort before repointing HEAD. The published
        // snapshot stays an orphan (same crash-window semantics as
        // `save`), HEAD is untouched.
        if !lock.still_ours() {
            return Err(HeadCasError::Store(Error::Store(format!(
                "HEAD lock lost during CAS save: {}",
                self.head_lock_path().display()
            ))));
        }
        self.publish_head(&name).map_err(HeadCasError::Store)?;
        // Post-publish fencing: the pre-publish check cannot cover a
        // break landing between it and the HEAD rename — a critical
        // section held past HEAD_LOCK_STALE_AFTER is breakable, so a
        // contender may have committed concurrently. Our publish did
        // land, but reporting Ok could hide that the other writer's
        // commit was overwritten (or ours was) — surface an
        // indeterminate store error and let the caller reconcile via
        // `head()` instead of claiming success.
        if !lock.still_ours() {
            return Err(HeadCasError::Store(Error::Store(format!(
                "HEAD CAS indeterminate: lock lost during HEAD publish: {}",
                self.head_lock_path().display()
            ))));
        }
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

    fn save_cas(
        &mut self,
        project_id: &str,
        expected_head: Option<&str>,
        revision: ProjectRevision,
        bytes: &[u8],
    ) -> Result<String, HeadCasError> {
        // Port parity with `DirectoryBundleStore`: the expected-head
        // compare runs against the same value `head()` reports, and a
        // mismatch is the typed conflict — never a silent repoint.
        // (`&mut self` already serializes callers of one instance;
        // there is no cross-instance or on-disk dimension to lock.)
        if self.head.as_deref() != expected_head {
            return Err(HeadCasError::Conflict {
                expected: expected_head.map(str::to_owned),
                actual: self.head.clone(),
            });
        }
        self.save(project_id, revision, bytes)
            .map_err(HeadCasError::Store)
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
