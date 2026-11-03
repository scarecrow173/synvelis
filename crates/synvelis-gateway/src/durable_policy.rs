//! `PoC B` — an independent durable policy store (ACG-004 比較 B:
//! 「独立 Policy Store ＋ Durable Fence」)。
//!
//! [`FilePolicyStore`] は [`InMemoryPolicyStore`] を内部状態として抱き、
//! すべての mutation を自身の append-only policy log（commit journal と
//! 同一 framing、別ファイル）に fsync 付きで追記する。再起動時は
//! [`FilePolicyStore::open`] が log を replay して policy 内容を復元する。
//!
//! `PoC A`（順序付き Recovery Journal 単一永続面）との比較で重要なのは
//! *順序証明の所在*:
//!
//! - B では policy の永続化が独立しているため、project commit と
//!   policy event の順序は単一ログの seq では証明できない。
//!   commit record が authorization 時点の `policy_lsn` を観測値として
//!   記録し、commit journal 側の sync 後に policy log へ `Fence`
//!   marker を追記することで、crash を跨いだ「commit がどの policy
//!   状態を見て確定したか」を再構成する。
//! - この 2-store プロトコルは fsync 境界が 2 つに増え、crash 時の
//!   「commit record あり・fence marker なし」の窓を commit record
//!   自身の `policy_lsn` で救済する設計が必要になる — それが本実装が
//!   比較根拠として示す運用コストである。

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use crate::clock::PolicyInstant;
use crate::commit::PolicyAdmin;
use crate::grant::{Grant, PolicyEpoch};
use crate::id::{GrantRef, ProactivePermitRef, TaskRef};
use crate::journal::{FileJournal, ReplayOutcome};
use crate::store::{
    AuthorizedTask, GrantRecord, InMemoryPolicyStore, PolicyStore, ProactivePermit,
};

/// policy log の 1 record。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyLogEvent {
    /// Task authorization issued.
    IssueTask(Box<AuthorizedTask>),
    /// Task authorization revoked.
    RevokeTask(TaskRef),
    /// Proactive permit issued.
    IssuePermit(Box<ProactivePermit>),
    /// Proactive permit revoked.
    RevokePermit(ProactivePermitRef),
    /// Execution grant issued.
    IssueGrant(Box<Grant>),
    /// Execution grant revoked.
    RevokeGrant(GrantRef),
    /// Policy epoch advanced.
    EpochAdvanced,
    /// Delegated-apply budget unit consumed.
    Consume(GrantRef),
    /// Durable fence marker — binds a commit journal seq to the policy
    /// LSN the commit observed at authorization time.
    Fence {
        /// The commit record's journal sequence.
        commit_seq: u64,
        /// The policy log position the commit observed.
        policy_lsn: u64,
    },
}

/// `open` の報告。
#[derive(Clone, Debug)]
pub struct PolicyOpenReport {
    /// Scanned policy log path.
    pub path: PathBuf,
    /// Replay outcome (intact records + stop reason).
    pub replay: ReplayOutcome,
    /// Bytes truncated as an incomplete tail.
    pub truncated_bytes: u64,
    /// Fence markers seen — `(commit_seq, policy_lsn)` pairs.
    pub fences: Vec<(u64, u64)>,
    /// CRC-valid frames whose payload did not decode — replay stops at
    /// the first hole because skipping it could silently resurrect a
    /// revoked grant or refund a consumed budget.
    pub unreadable_records: u64,
}

/// An independently durable policy store — every mutation is a synced
/// append to its own log before it lands in memory.
pub struct FilePolicyStore {
    inner: InMemoryPolicyStore,
    journal: Mutex<FileJournal>,
}

impl FilePolicyStore {
    /// Opens (or creates) the policy log at `path` and replays it into a
    /// fresh in-memory view.
    ///
    /// # Errors
    /// IO failures scanning/repairing the log.
    pub fn open(path: impl AsRef<Path>) -> io::Result<(Self, PolicyOpenReport)> {
        let path = path.as_ref().to_path_buf();
        let (journal, report) = FileJournal::open(&path)?;
        let inner = InMemoryPolicyStore::new();
        let mut fences = Vec::new();
        let replay = report.replay.unwrap_or(ReplayOutcome {
            records: Vec::new(),
            prefix_len: 0,
            stop: crate::journal::StopReason::CleanEnd,
        });
        let mut unreadable_records = 0u64;
        for rec in &replay.records {
            let Ok(event) = serde_json::from_slice::<PolicyLogEvent>(&rec.payload) else {
                // A decode hole mid-log is fail-closed: skipping it could
                // resurrect a revoke or refund a spend behind it.
                unreadable_records = unreadable_records.saturating_add(1);
                break;
            };
            apply_event(&inner, &event);
            if let PolicyLogEvent::Fence {
                commit_seq,
                policy_lsn,
            } = event
            {
                fences.push((commit_seq, policy_lsn));
            }
        }
        let open_report = PolicyOpenReport {
            path,
            replay,
            truncated_bytes: report.truncated_bytes,
            fences,
            unreadable_records,
        };
        Ok((
            Self {
                inner,
                journal: Mutex::new(journal),
            },
            open_report,
        ))
    }

    fn lock_journal(&self) -> MutexGuard<'_, FileJournal> {
        self.journal.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Appends + syncs one event, then applies it to the live view.
    /// Journal-first ordering means a crash between the two can only
    /// *redo* the mutation on replay — never lose a confirmed one.
    fn mutate(
        &self,
        event: &PolicyLogEvent,
        apply: impl FnOnce(&InMemoryPolicyStore),
    ) -> io::Result<u64> {
        let bytes = serde_json::to_vec(event).map_err(io::Error::other)?;
        let seq = self.lock_journal().append(&bytes)?;
        apply(&self.inner);
        Ok(seq)
    }

    /// The current durable log position — the value a commit record
    /// carries as `policy_lsn`.
    #[must_use]
    pub fn durable_lsn(&self) -> u64 {
        self.lock_journal().next_seq()
    }
}

fn apply_event(store: &InMemoryPolicyStore, event: &PolicyLogEvent) {
    match event {
        PolicyLogEvent::IssueTask(t) => store.issue_task((**t).clone()),
        PolicyLogEvent::RevokeTask(id) => store.revoke_task(id),
        PolicyLogEvent::IssuePermit(p) => store.issue_proactive_permit((**p).clone()),
        PolicyLogEvent::RevokePermit(id) => store.revoke_proactive_permit(id),
        PolicyLogEvent::IssueGrant(g) => store.issue_grant((**g).clone()),
        PolicyLogEvent::RevokeGrant(id) => store.revoke_grant(id),
        PolicyLogEvent::EpochAdvanced => {
            store.update_policy();
        }
        PolicyLogEvent::Consume(id) => store.note_consumption(id),
        PolicyLogEvent::Fence { .. } => {}
    }
}

impl PolicyStore for FilePolicyStore {
    fn grant(&self, id: &GrantRef) -> Option<GrantRecord> {
        self.inner.grant(id)
    }
    fn task(&self, id: &TaskRef) -> Option<AuthorizedTask> {
        self.inner.task(id)
    }
    fn proactive_permit(&self, id: &ProactivePermitRef) -> Option<ProactivePermit> {
        self.inner.proactive_permit(id)
    }
    fn policy_epoch(&self) -> PolicyEpoch {
        self.inner.policy_epoch()
    }
    fn try_consume_apply(&self, id: &GrantRef, now: PolicyInstant) -> bool {
        // Atomic check+spend in memory first; the durable consume record
        // follows it — a journal-first order would log phantom spends
        // for attempts the budget check rejected. A swallowed append
        // failure would refund the unit on restart, so a failed record
        // poisons the store instead: every later mutation must fail
        // rather than diverge from the log.
        let ok = self.inner.try_consume_apply(id, now);
        if ok
            && self
                .mutate(&PolicyLogEvent::Consume(id.clone()), |_| {})
                .is_err()
        {
            self.inner.poison();
        }
        ok
    }
}

impl PolicyAdmin for FilePolicyStore {
    fn issue_task(&self, task: AuthorizedTask) -> io::Result<()> {
        self.mutate(&PolicyLogEvent::IssueTask(Box::new(task.clone())), |s| {
            s.issue_task(task);
        })
        .map(|_| ())
    }
    fn revoke_task(&self, id: &TaskRef) -> io::Result<()> {
        self.mutate(&PolicyLogEvent::RevokeTask(id.clone()), |s| {
            s.revoke_task(id);
        })
        .map(|_| ())
    }
    fn issue_permit(&self, permit: ProactivePermit) -> io::Result<()> {
        self.mutate(
            &PolicyLogEvent::IssuePermit(Box::new(permit.clone())),
            |s| {
                s.issue_proactive_permit(permit);
            },
        )
        .map(|_| ())
    }
    fn revoke_permit(&self, id: &ProactivePermitRef) -> io::Result<()> {
        self.mutate(&PolicyLogEvent::RevokePermit(id.clone()), |s| {
            s.revoke_proactive_permit(id);
        })
        .map(|_| ())
    }
    fn issue_grant(&self, grant: Grant) -> io::Result<()> {
        self.mutate(&PolicyLogEvent::IssueGrant(Box::new(grant.clone())), |s| {
            s.issue_grant(grant);
        })
        .map(|_| ())
    }
    fn revoke_grant(&self, id: &GrantRef) -> io::Result<()> {
        self.mutate(&PolicyLogEvent::RevokeGrant(id.clone()), |s| {
            s.revoke_grant(id);
        })
        .map(|_| ())
    }
    fn advance_epoch(&self) -> io::Result<PolicyEpoch> {
        self.mutate(&PolicyLogEvent::EpochAdvanced, |s| {
            s.update_policy();
        })
        .map(|_| self.inner.policy_epoch())
    }
    fn note_consumption(&self, id: &GrantRef) -> io::Result<()> {
        self.mutate(&PolicyLogEvent::Consume(id.clone()), |s| {
            s.note_consumption(id);
        })
        .map(|_| ())
    }
    fn durable_lsn(&self) -> Option<u64> {
        Some(FilePolicyStore::durable_lsn(self))
    }
    fn mark_fence(&self, commit_seq: u64, policy_lsn: u64) -> io::Result<()> {
        self.mutate(
            &PolicyLogEvent::Fence {
                commit_seq,
                policy_lsn,
            },
            |_| {},
        )
        .map(|_| ())
    }
}
