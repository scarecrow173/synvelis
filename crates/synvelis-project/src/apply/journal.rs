//! Apply receipt journal — idempotency and crash/ACK-gap recovery for
//! VCS-05 apply (synvelis#96).
//!
//! The journal records one [`ApplyJournalRecord`] per deterministic
//! apply key (`apply-<sha256 of the canonical intent>`):
//! `Pending` while an attempt is in flight, `Applied` with the full
//! [`ApplyReceipt`] on commit, `Failed` with a detail string on any
//! terminal error. A retry that finds `Applied` replays the recorded
//! receipt without re-applying; a retry that finds `Pending`
//! reconciles against live state (already-at-after ⇒ `Replayed`,
//! otherwise the mutation provably never committed and the attempt
//! proceeds under the same revision CAS).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use ruviform_io_file::{Durability, PublishMode, PublishOptions, StagedFile};

use super::ApplyReceipt;
use crate::snapshot::ContentDigest;

/// Journal failure — always surfaced as [`super::ApplyError::Journal`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplyJournalError {
    /// Filesystem/serialization failure.
    Io(String),
    /// A publish effect could not be reconciled by readback.
    Indeterminate(String),
}

impl std::fmt::Display for ApplyJournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(detail) => write!(f, "apply journal io: {detail}"),
            Self::Indeterminate(detail) => write!(f, "apply journal indeterminate: {detail}"),
        }
    }
}

impl std::error::Error for ApplyJournalError {}

/// Recorded state of one apply attempt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ApplyJournalState {
    /// An attempt started but has not reached a terminal record.
    Pending,
    /// The mutation committed; carries the authoritative receipt.
    Applied(ApplyReceipt),
    /// The attempt failed; `detail` is the terminal error text.
    Failed {
        /// Error description.
        detail: String,
    },
}

/// One journal entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyJournalRecord {
    /// Deterministic apply key (`apply-<hex>`).
    pub key: String,
    /// Digest of the canonical intent.
    pub intent_digest: ContentDigest,
    /// Current attempt state.
    pub state: ApplyJournalState,
}

/// Persistence port for apply receipts. Implementations must make
/// [`Self::store`] atomic per key (temp → publish → replace) so a
/// crash never leaves a torn record.
pub trait ApplyJournal {
    /// Loads the record for `key`; `None` = no attempt recorded.
    ///
    /// # Errors
    /// [`ApplyJournalError`] on read/parse failure.
    fn load(&mut self, key: &str) -> Result<Option<ApplyJournalRecord>, ApplyJournalError>;

    /// Atomically stores `record` under its key.
    ///
    /// # Errors
    /// [`ApplyJournalError`] on write/publish failure.
    fn store(&mut self, record: ApplyJournalRecord) -> Result<(), ApplyJournalError>;
}

/// Volatile journal for tests and single-session hosts.
#[derive(Default)]
pub struct InMemoryApplyJournal {
    records: BTreeMap<String, ApplyJournalRecord>,
}

impl InMemoryApplyJournal {
    /// An empty journal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ApplyJournal for InMemoryApplyJournal {
    fn load(&mut self, key: &str) -> Result<Option<ApplyJournalRecord>, ApplyJournalError> {
        Ok(self.records.get(key).cloned())
    }

    fn store(&mut self, record: ApplyJournalRecord) -> Result<(), ApplyJournalError> {
        self.records.insert(record.key.clone(), record);
        Ok(())
    }
}

/// File-backed journal: one `<key>.json` per attempt under `dir`,
/// each written via staged publish (`Replace` + file+dir sync) so a
/// crash mid-write never produces a torn record.
pub struct DirectoryApplyJournal {
    dir: PathBuf,
}

impl DirectoryApplyJournal {
    /// Journal rooted at `dir` (created on first store).
    #[must_use]
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            dir: dir.as_ref().to_path_buf(),
        }
    }

    fn path_for(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.json"))
    }

    fn publish(path: &Path, bytes: &[u8]) -> Result<(), ApplyJournalError> {
        let mut staged =
            StagedFile::create(path).map_err(|e| ApplyJournalError::Io(format!("stage: {e}")))?;
        staged
            .write_all(bytes)
            .map_err(|e| ApplyJournalError::Io(format!("write: {e}")))?;
        let mut receipt = staged
            .seal()
            .publish(PublishOptions::new(
                PublishMode::Replace,
                Durability::FileAndDirectorySync,
            ))
            .map_err(|mut failure| {
                if failure.take_recovery_handle().is_some() {
                    // best-effort cleanup of a non-indeterminate leftover
                }
                ApplyJournalError::Io(format!("publish: {failure}"))
            })?;
        if let Some(handle) = receipt.take_recovery_handle() {
            let _ = handle.cleanup();
        }
        Ok(())
    }
}

impl ApplyJournal for DirectoryApplyJournal {
    fn load(&mut self, key: &str) -> Result<Option<ApplyJournalRecord>, ApplyJournalError> {
        let path = self.path_for(key);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(ApplyJournalError::Io(format!(
                    "read {}: {e}",
                    path.display()
                )));
            }
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| ApplyJournalError::Io(format!("parse {}: {e}", path.display())))
    }

    fn store(&mut self, record: ApplyJournalRecord) -> Result<(), ApplyJournalError> {
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| ApplyJournalError::Io(format!("create journal dir: {e}")))?;
        let bytes = serde_json::to_vec(&record)
            .map_err(|e| ApplyJournalError::Io(format!("serialize record: {e}")))?;
        Self::publish(&self.path_for(&record.key), &bytes)
    }
}
