//! Persistence: the `ProjectStore` port plus two implementations.
//!
//! The snapshot envelope is strict (`deny_unknown_fields`), versioned, and
//! serialized deterministically (declaration-ordered fields, `BTreeMap`
//! sorted keys). `DirectoryBundleStore` follows the architecture's save
//! protocol: temp file → flush/sync → immutable publish → HEAD rename —
//! so a failure at any step leaves the previous HEAD intact.

use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{controller::ProjectRevision, error::Error, id::ProjectId, project::Project};

/// The only schema version this build reads or writes.
/// v2 adds `Project::control` (the issue-#40 Parameter Control Model);
/// v3 replaces the clip anchor/offset pair with the normalized
/// placement+playback contract and adds the comping registry (MD-A).
/// v1/v2 snapshots are not migrated (PoC-stage format change).
pub const SCHEMA_VERSION: u32 = 3;

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
    /// Any I/O/store failure; the previous HEAD must remain intact.
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

    fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), Error> {
        let tmp = path.with_extension("tmp");
        {
            let mut file = fs::File::create(&tmp).map_err(|e| Error::Store(e.to_string()))?;
            file.write_all(bytes)
                .map_err(|e| Error::Store(e.to_string()))?;
            file.sync_all().map_err(|e| Error::Store(e.to_string()))?;
        }
        fs::rename(&tmp, path).map_err(|e| Error::Store(e.to_string()))
    }

    fn snapshots_dir(&self) -> PathBuf {
        self.root.join("snapshots")
    }
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
        Self::write_atomic(&target, bytes)?;
        Self::write_atomic(&self.root.join("HEAD"), name.as_bytes())?;
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
        self.snapshots.insert(name.clone(), bytes.to_vec());
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
