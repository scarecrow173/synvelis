//! Authorized commit gate (GW-06, ACG-003/ACG-004) — the only path from a
//! GW-05 accepted apply to the [`ProjectController`] commit boundary.
//!
//! ## 直列化モデル（Revoke linearization）
//!
//! `CommitGate` は [`CommitFence`] を境界に、最終認可・照合・project
//! commit・journal 記録を *1つの fence section* として実行する。
//! `revoke_grant` 等の policy 更新も同じ fence を通るため、Revoke と
//! Commit の順序は fence section の進入順で完全に直列化される:
//!
//! - Revoke section が先 → commit section 内の `evaluate_commit` は
//!   revoked grant を観測し、commit は `Denied` で拒否される（REC-01）。
//! - Commit section が先 → mutation は既に確定。後続の Revoke は記録
//!   されるが commit は取り消されない（REC-02、revoke≠undo）。
//!
//! 「最終認可の後・適用の前に Revoke が確定する」TOCTOU 窓は、read
//! （grant 評価）と write（`controller.apply`）が同一 section にあるため
//! 構造上存在しない。AI 推論・外部 I/O・proposal 解決などの長い処理は
//! 全て fence の外で済んでいる（GW-04/05 の成果物が入力）。
//!
//! ## Idempotency / 再試行（REC-04〜06）
//!
//! apply request が運ぶ `IdempotencyKey` を `(principal, project, key)`
//! で照合し、payload fingerprint が一致すれば記録済み結果を返し、
//! 一致しなければ `IdempotencyConflict` で拒否する。成否の判定は常に
//! journal の記録であり、transport の成否ではない。`Indeterminate`
//! な結果は成功と誤認しない — 呼出し側は `reconcile` で確認する。
//!
//! ## Crash / Recovery（REC-07〜10）
//!
//! 各決定は「project commit 後・返却前」に journal へ fsync 付きで
//! 追記される（apply→journal の順で、未適用の commit record は存在
//! しない）。再起動は [`CommitGate::open`] が journal を replay し、
//! 最新の intact commit record が運ぶ post-state を project store へ
//! restore して controller を再構築する。journal に outcome の無い
//! apply は確定不能（`Indeterminate`）として報告し、HEAD/snapshot
//! だけから commit を推測しない。policy lifecycle（issue/revoke/
//! epoch）も同一 journal に記録され、commit との順序は seq で完全に
//! 復元される（`PoC A`）。delegated apply の消費量は commit record が
//! 権威なので replay で復元され、二重消費も消失もしない（REC-07）。

// Typed envelopes are the contract — GW-01 already chose them.
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest as _, Sha256};
use synvelis_project::{
    Command, IdGenerator, Project, ProjectController, ProjectId, ProjectOp, ProjectRevision,
    ProjectStore, SCHEMA_VERSION, SnapshotEnvelope,
};

use crate::authorization::{AuthorizationDecision, AuthorizationStage};
use crate::capability::CapabilityDescriptor;
use crate::grant::{ApprovalPolicy, Grant, PolicyEpoch};
use crate::id::{
    ApprovalRef, CommitId, GrantRef, IdempotencyKey, OperationId, ProactivePermitRef, ProposalRef,
    TaskRef,
};
use crate::journal::{self, FileJournal};
use crate::outcome::{ErrorEnvelope, GatewayError};
use crate::policy::GrantPolicy;
use crate::prepare::{PreparedRecord, prepared_effects};
use crate::proposal::{ApplyEvent, ApplyStatus, ProposalStore};
use crate::request::{CallerIdentity, OperationRequest, Principal, PrincipalKind};
use crate::store::{AuthorizedTask, PolicyStore, ProactivePermit};

// ---------------------------------------------------------------------
// fence — the serialization boundary
// ---------------------------------------------------------------------

/// The linearization boundary (ACG-003 §2: Grant Fence + Single Writer).
///
/// One `section` is one indivisible ordering step: policy reads/spends,
/// the controller apply, and the durable journal append all happen inside
/// it. Revocation is confirmed by *entering* a section — whichever side
/// runs its section first is the recorded order.
pub trait CommitFence: Send + Sync {
    /// Runs `f` as one serialized section and returns its result.
    fn section<R>(&self, f: impl FnOnce() -> R) -> R;
}

/// Production fence — one mutex; section entry order is the order.
#[derive(Default)]
pub struct MutexFence {
    inner: Mutex<()>,
}

impl MutexFence {
    /// A fresh production fence.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl CommitFence for MutexFence {
    fn section<R>(&self, f: impl FnOnce() -> R) -> R {
        let _guard = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        f()
    }
}

/// Controlled fault injection at fence stage boundaries (test/PoC
/// evidence only — production commits go through [`CommitGate::commit`]
/// which passes [`NoHooks`]).
///
/// A hook returning `Err(GateCrash)` models process death *at that exact
/// stage*: the section unwinds, no further protocol step runs, and the
/// caller observes an `Indeterminate` outcome — which is precisely what a
/// real crash leaves behind. The gate still best-effort appends an
/// `Indeterminate` journal record: that marker documents the undecided
/// attempt (a real crash leaves the same gap where a record would be)
/// and lets replay prove a durable `Committed` is never displaced by it.
pub trait CommitHooks {
    /// After final commit authorization, before the project mutation.
    ///
    /// # Errors
    /// [`GateCrash`] models process death at this stage.
    fn after_authorize(&mut self, _ctx: &FenceCtx<'_>) -> Result<(), GateCrash> {
        Ok(())
    }
    /// After the in-memory project commit, before the durable record.
    ///
    /// # Errors
    /// [`GateCrash`] models process death at this stage.
    fn after_apply(&mut self, _ctx: &FenceCtx<'_>) -> Result<(), GateCrash> {
        Ok(())
    }
    /// After the durable record synced, before the apply settles.
    ///
    /// # Errors
    /// [`GateCrash`] models process death at this stage.
    fn after_journal(&mut self, _ctx: &FenceCtx<'_>) -> Result<(), GateCrash> {
        Ok(())
    }
}

/// The no-op hook set used by production commits.
pub struct NoHooks;

impl CommitHooks for NoHooks {}

/// Injected process death at a hook point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateCrash(pub String);

/// What a hook may observe or perturb inside a fence section. Exposes
/// the policy admin surface so a test can model an unordered write (e.g.
/// a revocation racing inside the section) — ordered revocations go
/// through [`CommitGate::revoke_grant`], which takes its own section.
pub struct FenceCtx<'a> {
    /// The policy administration surface.
    pub admin: &'a dyn PolicyAdmin,
    /// The project mutation currently under the fence.
    pub live_revision: ProjectRevision,
}

// ---------------------------------------------------------------------
// policy administration seam
// ---------------------------------------------------------------------

/// The mutating side of the policy surface (evaluation stays on
/// [`PolicyStore`]). Implemented by [`crate::store::InMemoryPolicyStore`]
/// and by the durable comparison store (`PoC B`).
///
/// Every mutating call is what the gate journals as a [`PolicyEvent`];
/// routing them through the gate is what makes "journal order ==
/// policy order" hold.
pub trait PolicyAdmin: PolicyStore {
    /// Registers a task authorization.
    ///
    /// # Errors
    /// Durable-side append failures (`PoC B`) — a mutation must not be
    /// reported confirmed when its record could not be persisted.
    fn issue_task(&self, task: AuthorizedTask) -> io::Result<()>;
    /// Revokes a task authorization.
    ///
    /// # Errors
    /// See [`Self::issue_task`].
    fn revoke_task(&self, id: &TaskRef) -> io::Result<()>;
    /// Registers a proactive-activation permit.
    ///
    /// # Errors
    /// See [`Self::issue_task`].
    fn issue_permit(&self, permit: ProactivePermit) -> io::Result<()>;
    /// Revokes a proactive-activation permit.
    ///
    /// # Errors
    /// See [`Self::issue_task`].
    fn revoke_permit(&self, id: &ProactivePermitRef) -> io::Result<()>;
    /// Issues an execution grant.
    ///
    /// # Errors
    /// See [`Self::issue_task`].
    fn issue_grant(&self, grant: Grant) -> io::Result<()>;
    /// Revokes an execution grant — the ordering-critical event.
    ///
    /// # Errors
    /// See [`Self::issue_task`].
    fn revoke_grant(&self, id: &GrantRef) -> io::Result<()>;
    /// Advances the policy epoch (administrative epoch change).
    ///
    /// # Errors
    /// See [`Self::issue_task`].
    fn advance_epoch(&self) -> io::Result<PolicyEpoch>;
    /// Restores one consumed delegated-apply unit during journal
    /// replay — unconditional; commit records are the authority.
    ///
    /// # Errors
    /// See [`Self::issue_task`].
    fn note_consumption(&self, id: &GrantRef) -> io::Result<()>;
    /// Durable ordering position of the policy log, when the store has
    /// one (`PoC B`). In-memory stores report `None`.
    fn durable_lsn(&self) -> Option<u64> {
        None
    }
    /// Writes the durable fence marker binding a commit to the policy
    /// LSN it observed (`PoC B`). Default: no-op.
    ///
    /// # Errors
    /// Store-side durability failures.
    fn mark_fence(&self, _commit_seq: u64, _policy_lsn: u64) -> io::Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------
// durable journal records
// ---------------------------------------------------------------------

/// The idempotency identity carried into a durable outcome record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalIdem {
    /// The caller-presented idempotency key.
    pub key: IdempotencyKey,
    /// `"sha256:<hex>"` over the canonical payload identity.
    pub fingerprint: String,
}

/// A durable `Committed` record — carries everything recovery needs to
/// restore the exact post-commit project without trusting HEAD.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitRecord {
    /// The apply operation this commit settles — `None` for manual/
    /// undo/redo commits, which settle no wire operation and are not
    /// reconcileable by operation id.
    pub operation: Option<OperationId>,
    /// The proposal it applied (`None` for manual commits).
    pub proposal: Option<ProposalRef>,
    /// Whose commit this was.
    pub principal: Principal,
    /// The project mutated.
    pub project: ProjectId,
    /// The revision the apply asserted (`expected_revision`).
    pub base_revision: ProjectRevision,
    /// The revision the commit produced.
    pub revision: ProjectRevision,
    /// Idempotency identity, when the request carried a key.
    pub idempotency: Option<JournalIdem>,
    /// The grant under which the commit authorized (agent path).
    pub grant: Option<GrantRef>,
    /// Whether the authorization spent a delegated-apply budget unit —
    /// commit records are the durable authority for consumption.
    pub budget_spend: bool,
    /// The approval receipt consumed by this apply, when required.
    pub approval: Option<ApprovalRef>,
    /// The changeset digest the commit asserted.
    pub digest: Option<String>,
    /// Policy-store durable position observed at authorization (`PoC B`).
    pub policy_lsn: Option<u64>,
    /// Serialized [`SnapshotEnvelope`] of the post-commit live project —
    /// recovery restores this rather than re-deriving the mutation.
    pub state: Vec<u8>,
}

/// A durable non-commit outcome (`Rejected` / `Conflicted` /
/// `Indeterminate`) — a retry must see the same verdict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeRecord {
    /// The apply operation this outcome settles.
    pub operation: OperationId,
    /// The proposal it applied.
    pub proposal: Option<ProposalRef>,
    /// Whose apply this was.
    pub principal: Principal,
    /// The project addressed.
    pub project: ProjectId,
    /// Safe-to-disclose reason / conflict details.
    pub reason: String,
    /// For `Conflicted`: the asserted expected revision.
    pub expected: Option<ProjectRevision>,
    /// For `Conflicted`: the live revision observed.
    pub actual: Option<ProjectRevision>,
    /// The grant under which a delegated-apply unit was spent before
    /// the outcome was reached — the spend precedes the decision it
    /// funded, so post-authorization outcomes carry it (REC-07).
    pub grant: Option<GrantRef>,
    /// Whether the authorization already spent a budget unit.
    pub budget_spend: bool,
    /// Idempotency identity, when the request carried a key.
    pub idempotency: Option<JournalIdem>,
}

/// A durable policy lifecycle event — ordering vs commits is seq order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub enum PolicyEvent {
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
}

/// A durable save event — `Saved` is a distinct receipt/state from
/// `Committed` (ACG-004): a commit mutates live state; a save publishes
/// HEAD.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveRecord {
    /// The project saved.
    pub project: ProjectId,
    /// The revision the snapshot names.
    pub revision: ProjectRevision,
    /// The snapshot name HEAD now points at.
    pub head: String,
}

/// One durable journal payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JournalPayload {
    /// An apply committed at the project boundary.
    Committed(Box<CommitRecord>),
    /// An apply rejected (authorization/validation decided `no`).
    Rejected(Box<OutcomeRecord>),
    /// An apply conflicted on expected revision.
    Conflicted(Box<OutcomeRecord>),
    /// An apply whose outcome cannot be determined.
    Indeterminate(Box<OutcomeRecord>),
    /// A policy lifecycle event.
    Policy(PolicyEvent),
    /// A HEAD publication.
    Saved(SaveRecord),
}

// ---------------------------------------------------------------------
// typed results
// ---------------------------------------------------------------------

/// The durable commit identity + settled outcome (ACG-004 — distinct
/// from `Saved` / engine-application / VCS states).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitReceipt {
    /// Durable commit identity (= journal sequence).
    pub commit: CommitId,
    /// The apply operation this receipt settles (`None` for manual/
    /// undo/redo commits).
    pub operation: Option<OperationId>,
    /// The proposal applied (`None` for manual commits).
    pub proposal: Option<ProposalRef>,
    /// Whose commit this was.
    pub principal: Principal,
    /// The project mutated.
    pub project: ProjectId,
    /// The base revision asserted.
    pub base_revision: ProjectRevision,
    /// The revision produced.
    pub revision: ProjectRevision,
    /// The journal sequence of this record.
    pub journal_seq: u64,
}

/// The settled outcome of a commit attempt, as recoverable state —
/// mirrors [`ApplyStatus`] but is a durable/reportable verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitOutcome {
    /// Committed — carries the durable receipt.
    Committed(CommitReceipt),
    /// Rejected — authorization or validation refused.
    Rejected {
        /// Safe-to-disclose reason.
        reason: String,
    },
    /// Revision conflict at the commit boundary.
    Conflicted {
        /// The asserted expected revision.
        expected: ProjectRevision,
        /// The live revision observed.
        actual: ProjectRevision,
    },
    /// The outcome cannot be determined — never report as success.
    Indeterminate {
        /// Safe-to-disclose reason.
        reason: String,
    },
}

/// What a retry/reconcile resolves for an operation or idempotency key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RetryOutcome {
    /// A determined outcome exists — return it, never re-apply.
    Settled(CommitOutcome),
    /// A commit attempt is in flight right now — wait or reconcile.
    InFlight,
    /// Nothing is recorded for this identity.
    Unknown,
}

// ---------------------------------------------------------------------
// the gate
// ---------------------------------------------------------------------

/// Dependencies the gate coordinates (all pre-existing GW-02/05 seams).
pub struct GateDeps {
    /// The evaluation-time policy surface.
    pub policy: Arc<dyn PolicyStore>,
    /// The policy administration surface (same store for A; a durable
    /// store for B).
    pub admin: Arc<dyn PolicyAdmin>,
    /// The GW-02 evaluator used for commit-stage revalidation.
    pub evaluator: GrantPolicy,
    /// The GW-05 proposal/approval/apply store.
    pub proposals: Arc<ProposalStore>,
    /// The capability descriptor `synvelis.proposal.apply` resolves to —
    /// supplied by the host so the gate never redefines a capability.
    pub apply_capability: CapabilityDescriptor,
}

/// In-flight / settled bookkeeping the gate keeps keyed by idempotency
/// identity and operation id.
#[derive(Default)]
struct Ledger {
    /// `(principal-tag, project, key)` → fingerprint + recorded outcome.
    by_key: BTreeMap<String, (String, CommitOutcome)>,
    /// operation raw id → recorded outcome.
    by_operation: BTreeMap<u128, CommitOutcome>,
    /// idempotency keys with a commit attempt currently in flight.
    pending: std::collections::BTreeSet<String>,
}

/// The authorized commit gate — fences, revalidates, applies, journals,
/// and settles one apply attempt at a time.
///
/// `F` is the fence implementation. Production wiring is
/// `CommitGate::new` (mutex fence). Tests install a scheduled fence to
/// enumerate revoke/commit orderings deterministically.
pub struct CommitGate<F: CommitFence = MutexFence> {
    controller: Mutex<ProjectController>,
    /// Post-apply divergence: a commit mutated the live project but its
    /// durable outcome could not be recorded. The gate refuses every
    /// further project mutation until `open` re-baselines — a `save`
    /// here would publish a phantom commit no journal can attribute.
    diverged: std::sync::atomic::AtomicBool,
    journal: Mutex<FileJournal>,
    deps: GateDeps,
    fence: F,
    ledger: Mutex<Ledger>,
    project: ProjectId,
}

impl<F: CommitFence> CommitGate<F> {
    /// Builds a gate over an already-live controller and journal.
    /// (`open` is the restart path: it recovers then constructs.)
    pub fn new(
        controller: ProjectController,
        journal: FileJournal,
        project: ProjectId,
        deps: GateDeps,
        fence: F,
    ) -> Self {
        Self {
            controller: Mutex::new(controller),
            diverged: std::sync::atomic::AtomicBool::new(false),
            journal: Mutex::new(journal),
            deps,
            fence,
            ledger: Mutex::new(Ledger::default()),
            project,
        }
    }

    /// The project this gate serves (`single-project PoC`).
    #[must_use]
    pub fn project(&self) -> ProjectId {
        self.project
    }

    /// The live revision — read outside the fence for reporting only.
    #[must_use]
    pub fn live_revision(&self) -> ProjectRevision {
        self.controller
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .live_revision()
    }

    fn lock_controller(&self) -> MutexGuard<'_, ProjectController> {
        self.controller
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_journal(&self) -> MutexGuard<'_, FileJournal> {
        self.journal.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn lock_ledger(&self) -> MutexGuard<'_, Ledger> {
        self.ledger.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Appends one record to the durable journal (caller holds the
    /// fence — this is the `fsync` point of the commit protocol).
    fn record(&self, payload: &JournalPayload) -> io::Result<u64> {
        let bytes = serde_json::to_vec(payload).map_err(io::Error::other)?;
        self.lock_journal().append(&bytes)
    }

    /// The `(principal, project, key)` ledger identity for `req`.
    fn ledger_key(req: &OperationRequest) -> Option<(String, String)> {
        let key = req.idempotency()?;
        let p = req.caller().principal();
        let tag = match p.kind() {
            PrincipalKind::Human => "human",
            PrincipalKind::Agent => "agent",
        };
        Some((
            format!("{tag}|{}|{}|{}", p.id(), req.project(), key.as_str()),
            key.as_str().to_owned(),
        ))
    }

    /// Canonical `"sha256:<hex>"` fingerprint of the payload identity a
    /// retry must match (capability, project, target, args, consistency,
    /// task, grant — the operation/request ids are *not* payload).
    fn payload_fingerprint(req: &OperationRequest) -> String {
        let mut map = Map::new();
        map.insert(
            "capability".to_owned(),
            serde_json::to_value(req.capability()).unwrap_or(Value::Null),
        );
        map.insert(
            "project".to_owned(),
            serde_json::to_value(req.project()).unwrap_or(Value::Null),
        );
        map.insert(
            "target".to_owned(),
            serde_json::to_value(req.target()).unwrap_or(Value::Null),
        );
        map.insert(
            "consistency".to_owned(),
            serde_json::to_value(req.consistency()).unwrap_or(Value::Null),
        );
        map.insert("args".to_owned(), req.args().clone());
        map.insert(
            "schema".to_owned(),
            serde_json::to_value(req.input_schema()).unwrap_or(Value::Null),
        );
        map.insert(
            "task".to_owned(),
            req.task()
                .map_or(Value::Null, |t| Value::String(t.as_str().to_owned())),
        );
        map.insert(
            "grant".to_owned(),
            req.grant()
                .map_or(Value::Null, |g| Value::String(g.as_str().to_owned())),
        );
        let bytes = serde_json::to_vec(&Value::Object(map)).unwrap_or_default();
        format!("sha256:{:x}", Sha256::digest(&bytes))
    }

    fn idem_record(req: &OperationRequest) -> Option<JournalIdem> {
        req.idempotency().map(|key| JournalIdem {
            key: key.clone(),
            fingerprint: Self::payload_fingerprint(req),
        })
    }

    // ----- recovery / restart -----

    /// Restart path: replays the journal, restores the newest committed
    /// project state through the project store, rebuilds the durable
    /// idempotency/outcome ledger and the policy ordering, and reports
    /// exactly what could and could not be recovered.
    ///
    /// Restore semantics (`PoC A`): the newest intact `Committed` record's
    /// embedded snapshot supersedes the saved HEAD — it *was* committed;
    /// the save that re-materializes it is itself recovery evidence, not
    /// a user save. Policy lifecycle events replay into `deps.admin`
    /// (expected empty at restart — re-issued bodies come from the
    /// journaled `Issue*` records, so grant content survives restart);
    /// delegated consumption is restored from commit records.
    ///
    /// # Errors
    /// IO failures while scanning/repairing the journal or restoring
    /// snapshots; a corrupt-but-recoverable tail is reported, not fatal.
    #[allow(clippy::too_many_lines)]
    pub fn open(
        project: Project,
        generator: Box<dyn IdGenerator>,
        store: &mut dyn ProjectStore,
        journal_path: impl AsRef<Path>,
        deps: GateDeps,
        fence: F,
    ) -> Result<(Self, RecoveryReport), io::Error> {
        let path = journal_path.as_ref().to_path_buf();
        let (journal, open_report) = FileJournal::open(&path)?;
        let mut report = RecoveryReport {
            journal_path: path.clone(),
            truncated_bytes: open_report.truncated_bytes,
            stop: open_report
                .replay
                .as_ref()
                .map_or(journal::StopReason::CleanEnd, |r| r.stop),
            ..RecoveryReport::default()
        };

        let project_id = project.project_id;
        let mut controller = ProjectController::new(project, generator);
        let mut ledger = Ledger::default();
        let mut last_commit: Option<(ProjectRevision, Vec<u8>)> = None;
        let mut head_revision: Option<ProjectRevision> = None;

        // PoC B: an independently durable admin already replayed its own
        // log at open — re-applying policy lifecycle / consumption from
        // the commit journal would double-count spends and duplicate
        // log entries. In-memory admins treat this journal as the
        // policy authority and must replay into it (PoC A).
        let admin_is_durable = deps.admin.durable_lsn().is_some();

        if let Some(replay) = &open_report.replay {
            for rec in &replay.records {
                let Ok(payload) = serde_json::from_slice::<JournalPayload>(&rec.payload) else {
                    // A payload that fails to decode is corruption the
                    // frame CRC did not catch (e.g. schema drift) — the
                    // honest verdict is to stop trusting this record
                    // and everything after it.
                    report.unreadable_records = report.unreadable_records.saturating_add(1);
                    break;
                };
                match payload {
                    JournalPayload::Committed(c) => {
                        report.commits_replayed = report.commits_replayed.saturating_add(1);
                        // A record bound to another project must never
                        // seed our ledger or restore path.
                        if c.project != project_id {
                            report.unreadable_records = report.unreadable_records.saturating_add(1);
                            continue;
                        }
                        let receipt = CommitReceipt {
                            commit: CommitId::new(u128::from(rec.seq)),
                            operation: c.operation,
                            proposal: c.proposal.clone(),
                            principal: c.principal.clone(),
                            project: c.project,
                            base_revision: c.base_revision,
                            revision: c.revision,
                            journal_seq: rec.seq,
                        };
                        let outcome = CommitOutcome::Committed(receipt);
                        if let Some(op) = c.operation {
                            ledger.by_operation.insert(op.raw(), outcome.clone());
                        }
                        if let Some(idem) = &c.idempotency {
                            let p = &c.principal;
                            let tag = match p.kind() {
                                PrincipalKind::Human => "human",
                                PrincipalKind::Agent => "agent",
                            };
                            ledger.by_key.insert(
                                format!("{tag}|{}|{}|{}", p.id(), c.project, idem.key.as_str()),
                                (idem.fingerprint.clone(), outcome),
                            );
                        }
                        if !admin_is_durable
                            && let Some(grant) = &c.grant
                            && c.budget_spend
                        {
                            let _ = deps.admin.note_consumption(grant);
                        }
                        last_commit = Some((c.revision, c.state.clone()));
                    }
                    JournalPayload::Rejected(o) => {
                        if o.project != project_id {
                            report.unreadable_records = report.unreadable_records.saturating_add(1);
                            continue;
                        }
                        Self::replay_noncommit(
                            &mut ledger,
                            &o,
                            CommitOutcome::Rejected {
                                reason: o.reason.clone(),
                            },
                            &*deps.admin,
                            admin_is_durable,
                        );
                        report.outcomes_replayed = report.outcomes_replayed.saturating_add(1);
                    }
                    JournalPayload::Indeterminate(o) => {
                        if o.project != project_id {
                            report.unreadable_records = report.unreadable_records.saturating_add(1);
                            continue;
                        }
                        Self::replay_noncommit(
                            &mut ledger,
                            &o,
                            CommitOutcome::Indeterminate {
                                reason: o.reason.clone(),
                            },
                            &*deps.admin,
                            admin_is_durable,
                        );
                        report.outcomes_replayed = report.outcomes_replayed.saturating_add(1);
                    }
                    JournalPayload::Conflicted(o) => {
                        if o.project != project_id {
                            report.unreadable_records = report.unreadable_records.saturating_add(1);
                            continue;
                        }
                        Self::replay_noncommit(
                            &mut ledger,
                            &o,
                            CommitOutcome::Conflicted {
                                expected: o.expected.unwrap_or_default(),
                                actual: o.actual.unwrap_or_default(),
                            },
                            &*deps.admin,
                            admin_is_durable,
                        );
                        report.outcomes_replayed = report.outcomes_replayed.saturating_add(1);
                    }
                    JournalPayload::Policy(event) => {
                        report.policy_events_replayed =
                            report.policy_events_replayed.saturating_add(1);
                        if admin_is_durable {
                            continue;
                        }
                        match event {
                            PolicyEvent::IssueTask(t) => {
                                let _ = deps.admin.issue_task(*t);
                            }
                            PolicyEvent::RevokeTask(id) => {
                                let _ = deps.admin.revoke_task(&id);
                            }
                            PolicyEvent::IssuePermit(p) => {
                                let _ = deps.admin.issue_permit(*p);
                            }
                            PolicyEvent::RevokePermit(id) => {
                                let _ = deps.admin.revoke_permit(&id);
                            }
                            PolicyEvent::IssueGrant(g) => {
                                let _ = deps.admin.issue_grant(*g);
                            }
                            PolicyEvent::RevokeGrant(id) => {
                                let _ = deps.admin.revoke_grant(&id);
                            }
                            PolicyEvent::EpochAdvanced => {
                                let _ = deps.admin.advance_epoch();
                            }
                        }
                    }
                    JournalPayload::Saved(s) => {
                        report.saves_replayed = report.saves_replayed.saturating_add(1);
                        head_revision = Some(s.revision);
                    }
                }
            }
        }

        // Load persisted HEAD first. Absent HEAD is the fresh-project
        // case; a HEAD that exists but fails to load is *not* — surface
        // it rather than re-baseline onto an empty project.
        if store.head().map_err(io::Error::other)?.is_some() {
            controller.load(store).map_err(io::Error::other)?;
        }
        report.saved_head = controller.saved().map(|s| s.revision);

        // The newest committed record supersedes HEAD when ahead — the
        // committed state *was* decided; restore materializes it and the
        // recovery receipt records provenance.
        if let Some((revision, state)) = last_commit
            && revision > controller.live_revision()
        {
            store
                .save(&project_id.to_tagged(), revision, &state)
                .map_err(io::Error::other)?;
            controller.load(store).map_err(io::Error::other)?;
            report.restored_revision = Some(revision);
            report.restored_from_journal = true;
        }
        report.head_revision = head_revision;
        report.recovered_revision = controller.live_revision();

        Ok((
            Self {
                controller: Mutex::new(controller),
                diverged: std::sync::atomic::AtomicBool::new(false),
                journal: Mutex::new(journal),
                deps,
                fence,
                ledger: Mutex::new(ledger),
                project: project_id,
            },
            report,
        ))
    }

    /// Replays one non-commit outcome record into the ledger. A
    /// `Committed` outcome is terminal — a later `Indeterminate` record
    /// (e.g. a crash between the fsync'd commit record and the caller's
    /// receipt) can never displace the durable truth that the commit
    /// landed.
    fn replay_noncommit(
        ledger: &mut Ledger,
        o: &OutcomeRecord,
        outcome: CommitOutcome,
        admin: &dyn PolicyAdmin,
        admin_is_durable: bool,
    ) {
        if !admin_is_durable
            && o.budget_spend
            && let Some(grant) = &o.grant
        {
            // The spend was authorized before the outcome — a restart
            // must not refund it (REC-07).
            let _ = admin.note_consumption(grant);
        }
        if matches!(
            ledger.by_operation.get(&o.operation.raw()),
            Some(CommitOutcome::Committed(_))
        ) {
            return;
        }
        ledger
            .by_operation
            .insert(o.operation.raw(), outcome.clone());
        if let Some(idem) = &o.idempotency {
            let p = &o.principal;
            let tag = match p.kind() {
                PrincipalKind::Human => "human",
                PrincipalKind::Agent => "agent",
            };
            let key = format!("{tag}|{}|{}|{}", p.id(), o.project, idem.key.as_str());
            if matches!(
                ledger.by_key.get(&key),
                Some((_, CommitOutcome::Committed(_)))
            ) {
                return;
            }
            ledger
                .by_key
                .insert(key, (idem.fingerprint.clone(), outcome));
        }
    }

    /// What the journal/ledger can prove about an operation attempt.
    #[must_use]
    pub fn reconcile(&self, operation: OperationId) -> RetryOutcome {
        let ledger = self.lock_ledger();
        ledger
            .by_operation
            .get(&operation.raw())
            .map_or(RetryOutcome::Unknown, |o| RetryOutcome::Settled(o.clone()))
    }

    /// What the durable ledger proves for an idempotency-key retry.
    /// `fingerprint` must be recomputed from the retried request.
    #[must_use]
    pub fn reconcile_key(&self, ledger_key: &str, fingerprint: &str) -> RetryOutcome {
        let ledger = self.lock_ledger();
        match ledger.by_key.get(ledger_key) {
            Some((fp, outcome)) if fp == fingerprint => RetryOutcome::Settled(outcome.clone()),
            Some(_) => RetryOutcome::Settled(CommitOutcome::Rejected {
                reason: "idempotency key replayed with different payload".to_owned(),
            }),
            None if ledger.pending.contains(ledger_key) => RetryOutcome::InFlight,
            None => RetryOutcome::Unknown,
        }
    }

    /// Ledger key for a retrying request (`None` when unkeyed).
    #[must_use]
    pub fn ledger_key_of(req: &OperationRequest) -> Option<String> {
        Self::ledger_key(req).map(|(k, _)| k)
    }

    /// Fingerprint for a retrying request.
    #[must_use]
    pub fn fingerprint_of(req: &OperationRequest) -> String {
        Self::payload_fingerprint(req)
    }

    // ----- the apply → commit path -----

    /// Drives one accepted apply to its settled outcome (production
    /// entry point — no fault hooks).
    ///
    /// # Errors
    /// [`ErrorEnvelope`] for undecidable (`Indeterminate`) or
    /// request-level failures; determined non-commit outcomes are also
    /// reported as errors *and* recorded, so a retry sees the verdict.
    pub fn commit(&self, proposal: &ProposalRef) -> Result<CommitReceipt, ErrorEnvelope> {
        self.commit_hooked(proposal, &mut NoHooks)
    }

    /// Fault-injection variant of [`Self::commit`] — identical path with
    /// stage hooks for the `PoC` evidence matrix.
    ///
    /// # Errors
    /// See [`Self::commit`].
    pub fn commit_hooked(
        &self,
        proposal: &ProposalRef,
        hooks: &mut dyn CommitHooks,
    ) -> Result<CommitReceipt, ErrorEnvelope> {
        self.fence.section(|| self.commit_in_fence(proposal, hooks))
    }

    /// The fence section — every ordering-sensitive step lives here.
    fn commit_in_fence(
        &self,
        proposal: &ProposalRef,
        hooks: &mut dyn CommitHooks,
    ) -> Result<CommitReceipt, ErrorEnvelope> {
        // A post-apply record failure poisons the gate: the live
        // project moved without a durable outcome, so every further
        // mutation must refuse until `open` re-baselines.
        if self.diverged() {
            return Err(GatewayError::Indeterminate {
                reason: "commit gate diverged from the durable journal; reopen to reconcile"
                    .to_owned(),
            }
            .into());
        }
        // Resolve the apply + prepared record inside the section —
        // anything decided outside the fence is re-verified here.
        let view = self.deps.proposals.view(proposal);
        let Some(view) = view else {
            return Err(GatewayError::ProposalUnavailable {
                reason: "unknown proposal".to_owned(),
            }
            .into());
        };
        let Some(apply) = view.apply.clone() else {
            return Err(GatewayError::ProposalUnavailable {
                reason: "no apply attempt for proposal".to_owned(),
            }
            .into());
        };
        // An apply that already settled replays its recorded outcome —
        // the ledger is the authority, so a retry never re-applies. One
        // still waiting on a receipt is a plain refusal, not an
        // undecidable outcome.
        if apply.status != ApplyStatus::CommitRequested {
            if apply.status == ApplyStatus::AwaitingApproval {
                return Err(GatewayError::Denied {
                    stage: AuthorizationStage::Commit,
                    reason: Some("apply awaits an approval decision".to_owned()),
                }
                .into());
            }
            if let Some(outcome) = self.lock_ledger().by_operation.get(&apply.operation.raw()) {
                return Self::outcome_to_result(outcome.clone());
            }
            return Err(GatewayError::Indeterminate {
                reason: format!(
                    "apply settled ({}) without a durable outcome record",
                    apply.status.name()
                ),
            }
            .into());
        }
        let Some(record) = view.record.clone() else {
            // Accepted apply whose proposal died before commit began —
            // determined failure: nothing was or can be applied.
            return self.finish_noncommit(
                &apply.request,
                proposal,
                &CommitOutcome::Rejected {
                    reason: "proposal is no longer prepared".to_owned(),
                },
                None,
                GatewayError::ProposalUnavailable {
                    reason: "proposal is no longer prepared".to_owned(),
                },
            );
        };
        let req = &apply.request;
        // The ledger can hold a settled outcome for an apply the
        // store has not transitioned yet (a racing settle landed after
        // the journal write but before the apply transition) — replay
        // it instead of re-entering the commit path.
        if let Some(outcome) = self.lock_ledger().by_operation.get(&apply.operation.raw()) {
            return Self::outcome_to_result(outcome.clone());
        }

        // Idempotency: same key+payload replays the recorded outcome;
        // different payload conflicts. A pending entry means an earlier
        // attempt is in flight or died undecided — never silently retry.
        if let Some((ledger_key, _)) = Self::ledger_key(req) {
            let fingerprint = Self::payload_fingerprint(req);
            let mut ledger = self.lock_ledger();
            if let Some((fp, outcome)) = ledger.by_key.get(&ledger_key) {
                if fp == &fingerprint {
                    return Self::outcome_to_result(outcome.clone());
                }
                drop(ledger);
                return Err(GatewayError::IdempotencyConflict.into());
            }
            if ledger.pending.contains(&ledger_key) {
                return Err(GatewayError::Indeterminate {
                    reason: "prior commit attempt died undecided; reconcile before retry"
                        .to_owned(),
                }
                .into());
            }
            ledger.pending.insert(ledger_key.clone());
            drop(ledger);
            let result =
                self.commit_authorized(req, proposal, &record, hooks, Some(ledger_key.as_str()));
            self.lock_ledger().pending.remove(&ledger_key);
            return result;
        }
        self.commit_authorized(req, proposal, &record, hooks, None)
    }

    /// Steps between authorization and durable record — the part an
    /// idempotent retry must never re-enter with a recorded outcome.
    #[allow(clippy::too_many_lines)]
    fn commit_authorized(
        &self,
        req: &OperationRequest,
        proposal: &ProposalRef,
        record: &PreparedRecord,
        hooks: &mut dyn CommitHooks,
        pending_key: Option<&str>,
    ) -> Result<CommitReceipt, ErrorEnvelope> {
        let mut controller = self.lock_controller();
        let principal = req.caller().principal().clone();
        let idem = Self::idem_record(req);

        // 1. Receipt re-verification — an apply that consumed a receipt
        //    re-verifies the same consumption inside the fence. The
        //    verification feeds the commit-stage decision below:
        //    `ApproveToApply` reports `ApprovalRequired`, which a
        //    verified consumption satisfies.
        let mut receipt_verified = false;
        if let Some(approval) = self
            .deps
            .proposals
            .view(proposal)
            .and_then(|v| v.apply)
            .and_then(|a| a.approval)
        {
            if let Err(err) = self
                .deps
                .proposals
                .consume_approval(proposal, &approval, req)
            {
                return self.finish_noncommit(
                    req,
                    proposal,
                    &CommitOutcome::Rejected {
                        reason: format!("approval receipt not consumable: {err}"),
                    },
                    None,
                    GatewayError::ApprovalInvalid {
                        reason: format!("approval receipt not consumable at commit: {err}"),
                    },
                );
            }
            receipt_verified = true;
        }

        // 2. Final commit authorization — grant/task/epoch/scope/effects
        //    revalidated against the *current* store inside the fence,
        //    plus the delegated-apply budget spend (atomic in the store).
        let effects = prepared_effects(
            &record.prepared.effects,
            vec![req.capability().clone()],
            record.prepared.unsupported.clone(),
        );
        let decision = self.deps.evaluator.evaluate_effects(
            req,
            AuthorizationStage::Commit,
            &self.deps.apply_capability,
            &effects.scoped,
        );
        let grant = req.grant().cloned();
        let budget_spend = req.caller().principal().kind() == PrincipalKind::Agent
            && grant
                .as_ref()
                .and_then(|g| self.deps.policy.grant(g))
                .is_some_and(|g| g.grant.approval_policy == ApprovalPolicy::DelegatedApply);
        match decision {
            AuthorizationDecision::PermitForStage => {}
            // `ApproveToApply` reports the requirement; the consumed
            // receipt verified in step 1 is what satisfies it.
            AuthorizationDecision::ApprovalRequired if receipt_verified => {}
            AuthorizationDecision::ApprovalRequired => {
                return self.finish_noncommit(
                    req,
                    proposal,
                    &CommitOutcome::Rejected {
                        reason: "commit requires a consumed approval receipt".to_owned(),
                    },
                    None,
                    GatewayError::ApprovalInvalid {
                        reason: "commit-stage authorization still requires an approval".to_owned(),
                    },
                );
            }
            AuthorizationDecision::Deny { reason } => {
                return self.finish_noncommit(
                    req,
                    proposal,
                    &CommitOutcome::Rejected {
                        reason: format!("commit-stage denial: {reason}"),
                    },
                    None,
                    GatewayError::Denied {
                        stage: AuthorizationStage::Commit,
                        reason: Some(reason),
                    },
                );
            }
        }
        // The layered checks prepare-time `authorize_effects` runs:
        // scoped effects alone are not the full grant — the inner
        // operation capabilities the changeset executes must also be
        // covered by the *apply* grant (SEC-13), and every after-state
        // it writes must stay inside grant scope (SEC-06). Revalidating
        // only `scoped` would let an apply commit under a second grant
        // that covers `apply` but never covered the inner op.
        if req.caller().principal().kind() != PrincipalKind::Human {
            let Some(auth) = self.deps.evaluator.authorize_agent(req) else {
                return self.finish_noncommit(
                    req,
                    proposal,
                    &CommitOutcome::Rejected {
                        reason: "commit-stage denial: no live authorization".to_owned(),
                    },
                    None,
                    GatewayError::Denied {
                        stage: AuthorizationStage::Commit,
                        reason: Some("no live authorization".to_owned()),
                    },
                );
            };
            for cap in &effects.capabilities {
                if !auth.grant.capabilities.contains(cap) {
                    return self.finish_noncommit(
                        req,
                        proposal,
                        &CommitOutcome::Rejected {
                            reason: "commit-stage denial: capability outside grant".to_owned(),
                        },
                        None,
                        GatewayError::Denied {
                            stage: AuthorizationStage::Commit,
                            reason: Some("capability outside grant".to_owned()),
                        },
                    );
                }
            }
            let mut counters = crate::policy::ScopeCounters::default();
            for (resource, after) in &effects.after_states {
                if GrantPolicy::coverage(&auth.grant, resource, after, &mut counters).is_none() {
                    return self.finish_noncommit(
                        req,
                        proposal,
                        &CommitOutcome::Rejected {
                            reason: "commit-stage denial: outside scope".to_owned(),
                        },
                        None,
                        GatewayError::Denied {
                            stage: AuthorizationStage::Commit,
                            reason: Some("outside scope".to_owned()),
                        },
                    );
                }
            }
        }
        let ctx = FenceCtx {
            admin: &*self.deps.admin,
            live_revision: controller.live_revision(),
        };
        if let Err(GateCrash(reason)) = hooks.after_authorize(&ctx) {
            return self.finish_indeterminate(
                req,
                proposal,
                &format!("crash after authorization: {reason}"),
                pending_key,
                if budget_spend { grant.as_ref() } else { None },
                false,
            );
        }

        // 3. The single-writer mutation — revision revalidated by the
        //    controller itself inside the same section (no TOCTOU).
        let expected = req
            .expected_revision()
            .unwrap_or(record.prepared.base.revision);
        let revision = match controller.apply(Command {
            expected_revision: expected,
            op: record.op.clone(),
        }) {
            Ok(rev) => rev,
            Err(synvelis_project::Error::StaleRevision {
                expected: e,
                actual: a,
            }) => {
                return self.finish_noncommit(
                    req,
                    proposal,
                    &CommitOutcome::Conflicted {
                        expected: ProjectRevision(e),
                        actual: ProjectRevision(a),
                    },
                    if budget_spend { grant.as_ref() } else { None },
                    GatewayError::RevisionConflict {
                        expected: e,
                        actual: a,
                    },
                );
            }
            Err(err) => {
                return self.finish_noncommit(
                    req,
                    proposal,
                    &CommitOutcome::Rejected {
                        reason: format!("project validation failed at commit: {err}"),
                    },
                    if budget_spend { grant.as_ref() } else { None },
                    GatewayError::Internal {
                        reason: format!("commit validation failed: {err}"),
                    },
                );
            }
        };
        let ctx = FenceCtx {
            admin: &*self.deps.admin,
            live_revision: revision,
        };
        if let Err(GateCrash(reason)) = hooks.after_apply(&ctx) {
            return self.finish_indeterminate(
                req,
                proposal,
                &format!("crash after project commit: {reason}"),
                pending_key,
                if budget_spend { grant.as_ref() } else { None },
                true,
            );
        }

        // 4. The durable commit record — fsync'd before any caller sees
        //    success. `policy_lsn` binds the commit to the policy
        //    position it observed (PoC B ordering proof).
        let state = serde_json::to_vec(&SnapshotEnvelope {
            schema_version: SCHEMA_VERSION,
            project_id: self.project,
            revision,
            project: controller.live().clone(),
        })
        .map_err(|e| {
            ErrorEnvelope::from(GatewayError::Internal {
                reason: format!("snapshot encode failed: {e}"),
            })
        })?;
        let policy_lsn = self.deps.admin.durable_lsn();
        let record_payload = JournalPayload::Committed(Box::new(CommitRecord {
            operation: Some(req.operation_id()),
            proposal: Some(proposal.clone()),
            principal: principal.clone(),
            project: self.project,
            base_revision: expected,
            revision,
            idempotency: idem,
            grant: grant.clone(),
            budget_spend,
            approval: self
                .deps
                .proposals
                .view(proposal)
                .and_then(|v| v.apply)
                .and_then(|a| a.approval),
            digest: Some(
                self.deps
                    .proposals
                    .view(proposal)
                    .map(|v| v.changeset.digest.to_string())
                    .unwrap_or_default(),
            ),
            policy_lsn,
            state,
        }));
        let seq = match self.record(&record_payload) {
            Ok(seq) => seq,
            Err(err) => {
                return self.finish_indeterminate(
                    req,
                    proposal,
                    &format!("durable commit record failed: {err}"),
                    pending_key,
                    if budget_spend { grant.as_ref() } else { None },
                    true,
                );
            }
        };
        // A fence-marker failure cannot un-commit — the durable record
        // already carries `policy_lsn`, which is the marker's own
        // rescue. Keep the committed verdict; skip the marker.
        if let Some(lsn) = policy_lsn {
            let _ = self.deps.admin.mark_fence(seq, lsn);
        }
        if let Err(GateCrash(reason)) = hooks.after_journal(&ctx) {
            return self.finish_indeterminate(
                req,
                proposal,
                &format!("crash after durable record: {reason}"),
                pending_key,
                None,
                false,
            );
        }

        // 5. Settle + ledger — the committed outcome becomes replayable.
        let receipt = CommitReceipt {
            commit: CommitId::new(u128::from(seq)),
            operation: Some(req.operation_id()),
            proposal: Some(proposal.clone()),
            principal,
            project: self.project,
            base_revision: expected,
            revision,
            journal_seq: seq,
        };
        let outcome = CommitOutcome::Committed(receipt.clone());
        self.settle_outcome(req, proposal, &outcome, pending_key);
        let _ = self
            .deps
            .proposals
            .transit_apply(proposal, ApplyEvent::Commit);
        Ok(receipt)
    }

    /// Whether the live project diverged from the journal — post-apply
    /// record failure. A diverged gate refuses project mutations.
    fn diverged(&self) -> bool {
        self.diverged.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Records + settles a determined non-commit outcome.
    fn finish_noncommit(
        &self,
        req: &OperationRequest,
        proposal: &ProposalRef,
        outcome: &CommitOutcome,
        spent: Option<&GrantRef>,
        err: GatewayError,
    ) -> Result<CommitReceipt, ErrorEnvelope> {
        self.settle_outcome(req, proposal, outcome, None);
        let (payload, event) = match outcome {
            CommitOutcome::Rejected { reason } => (
                JournalPayload::Rejected(Box::new(OutcomeRecord {
                    operation: req.operation_id(),
                    proposal: Some(proposal.clone()),
                    principal: req.caller().principal().clone(),
                    project: self.project,
                    reason: reason.clone(),
                    expected: None,
                    actual: None,
                    grant: spent.cloned(),
                    budget_spend: spent.is_some(),
                    idempotency: Self::idem_record(req),
                })),
                ApplyEvent::Reject,
            ),
            CommitOutcome::Conflicted { expected, actual } => (
                JournalPayload::Conflicted(Box::new(OutcomeRecord {
                    operation: req.operation_id(),
                    proposal: Some(proposal.clone()),
                    principal: req.caller().principal().clone(),
                    project: self.project,
                    reason: "revision conflict".to_owned(),
                    expected: Some(*expected),
                    actual: Some(*actual),
                    grant: spent.cloned(),
                    budget_spend: spent.is_some(),
                    idempotency: Self::idem_record(req),
                })),
                ApplyEvent::Conflict,
            ),
            _ => (
                JournalPayload::Indeterminate(Box::new(OutcomeRecord {
                    operation: req.operation_id(),
                    proposal: Some(proposal.clone()),
                    principal: req.caller().principal().clone(),
                    project: self.project,
                    reason: "indeterminate".to_owned(),
                    expected: None,
                    actual: None,
                    grant: spent.cloned(),
                    budget_spend: spent.is_some(),
                    idempotency: Self::idem_record(req),
                })),
                ApplyEvent::Indeterminate,
            ),
        };
        let _ = self.record(&payload);
        let _ = self.deps.proposals.transit_apply(proposal, event);
        Err(err.into())
    }

    /// The undecidable path — best-effort `Indeterminate` record; the
    /// pending marker keeps a same-process keyed retry honest.
    fn finish_indeterminate(
        &self,
        req: &OperationRequest,
        proposal: &ProposalRef,
        reason: &str,
        pending_key: Option<&str>,
        spent: Option<&GrantRef>,
        diverge: bool,
    ) -> Result<CommitReceipt, ErrorEnvelope> {
        if diverge {
            // The live project moved without a durable outcome — refuse
            // further mutations until `open` re-baselines.
            self.diverged
                .store(true, std::sync::atomic::Ordering::Release);
        }
        let outcome = CommitOutcome::Indeterminate {
            reason: reason.to_owned(),
        };
        self.settle_outcome(req, proposal, &outcome, pending_key);
        let payload = JournalPayload::Indeterminate(Box::new(OutcomeRecord {
            operation: req.operation_id(),
            proposal: Some(proposal.clone()),
            principal: req.caller().principal().clone(),
            project: self.project,
            reason: reason.to_owned(),
            expected: None,
            actual: None,
            grant: spent.cloned(),
            budget_spend: spent.is_some(),
            idempotency: Self::idem_record(req),
        }));
        let _ = self.record(&payload);
        let _ = self
            .deps
            .proposals
            .transit_apply(proposal, ApplyEvent::Indeterminate);
        Err(GatewayError::Indeterminate {
            reason: reason.to_owned(),
        }
        .into())
    }

    /// Writes an outcome into the operation + key ledgers.
    fn settle_outcome(
        &self,
        req: &OperationRequest,
        _proposal: &ProposalRef,
        outcome: &CommitOutcome,
        pending_key: Option<&str>,
    ) {
        let mut ledger = self.lock_ledger();
        ledger
            .by_operation
            .insert(req.operation_id().raw(), outcome.clone());
        if let Some((key, _)) = Self::ledger_key(req) {
            ledger
                .by_key
                .insert(key, (Self::payload_fingerprint(req), outcome.clone()));
        }
        if let Some(k) = pending_key {
            ledger.pending.remove(k);
        }
    }

    /// Maps a recorded outcome back onto the commit result shape.
    fn outcome_to_result(outcome: CommitOutcome) -> Result<CommitReceipt, ErrorEnvelope> {
        match outcome {
            CommitOutcome::Committed(receipt) => Ok(receipt),
            CommitOutcome::Rejected { reason } => Err(GatewayError::Denied {
                stage: AuthorizationStage::Commit,
                reason: Some(reason),
            }
            .into()),
            CommitOutcome::Conflicted { expected, actual } => Err(GatewayError::RevisionConflict {
                expected: expected.0,
                actual: actual.0,
            }
            .into()),
            CommitOutcome::Indeterminate { reason } => {
                Err(GatewayError::Indeterminate { reason }.into())
            }
        }
    }

    // ----- manual path (trusted human authority, GT-012) -----

    /// A manual (human-authority) mutation — no grant/approval path, but
    /// still fenced and journaled so project-revision ordering covers UI
    /// edits too (REC-03, AHG-manual-path).
    ///
    /// # Errors
    /// `Denied` when the caller is not a trusted human; project errors
    /// otherwise.
    pub fn commit_manual(
        &self,
        caller: &CallerIdentity,
        op: ProjectOp,
        expected: ProjectRevision,
    ) -> Result<CommitReceipt, ErrorEnvelope> {
        if caller.principal().kind() != PrincipalKind::Human {
            return Err(GatewayError::Denied {
                stage: AuthorizationStage::Commit,
                reason: Some("manual commit path requires human authority".to_owned()),
            }
            .into());
        }
        self.fence.section(|| {
            if self.diverged() {
                return Err(GatewayError::Indeterminate {
                    reason: "commit gate diverged from the durable journal; reopen to reconcile"
                        .to_owned(),
                }
                .into());
            }
            let mut controller = self.lock_controller();
            let revision = match controller.apply(Command {
                expected_revision: expected,
                op,
            }) {
                Ok(rev) => rev,
                Err(synvelis_project::Error::StaleRevision {
                    expected: e,
                    actual: a,
                }) => {
                    return Err(GatewayError::RevisionConflict {
                        expected: e,
                        actual: a,
                    }
                    .into());
                }
                Err(err) => {
                    return Err(GatewayError::Internal {
                        reason: format!("manual commit failed: {err}"),
                    }
                    .into());
                }
            };
            let state = serde_json::to_vec(&SnapshotEnvelope {
                schema_version: SCHEMA_VERSION,
                project_id: self.project,
                revision,
                project: controller.live().clone(),
            })
            .map_err(|e| {
                ErrorEnvelope::from(GatewayError::Internal {
                    reason: format!("snapshot encode failed: {e}"),
                })
            })?;
            let payload = JournalPayload::Committed(Box::new(CommitRecord {
                operation: None,
                proposal: None,
                principal: caller.principal().clone(),
                project: self.project,
                base_revision: expected,
                revision,
                idempotency: None,
                grant: None,
                budget_spend: false,
                approval: None,
                digest: None,
                policy_lsn: None,
                state,
            }));
            let seq = match self.record(&payload) {
                Ok(seq) => seq,
                Err(e) => {
                    self.diverged
                        .store(true, std::sync::atomic::Ordering::Release);
                    return Err(ErrorEnvelope::from(GatewayError::Indeterminate {
                        reason: format!("durable commit record failed: {e}"),
                    }));
                }
            };
            Ok(CommitReceipt {
                commit: CommitId::new(u128::from(seq)),
                operation: None,
                proposal: None,
                principal: caller.principal().clone(),
                project: self.project,
                base_revision: expected,
                revision,
                journal_seq: seq,
            })
        })
    }

    /// Undo / redo through the single writer — still fenced and
    /// journaled, because an undo is itself a commit (it produces a new
    /// live revision) and must serialize with revocations exactly like
    /// an apply does (AC-01: the commit gate rides the existing Undo
    /// path rather than maintaining a second history).
    ///
    /// # Errors
    /// `Denied` when the caller is not a trusted human; store/journal
    /// failures otherwise. `Ok(false)` reports an empty stack.
    pub fn undo(&self, caller: &CallerIdentity) -> Result<bool, ErrorEnvelope> {
        self.history_step(caller, ProjectController::undo)
    }

    /// Re-applies the most recently undone content.
    ///
    /// # Errors
    /// `Denied` when the caller is not a trusted human; store/journal
    /// failures otherwise. `Ok(false)` reports an empty stack.
    pub fn redo(&self, caller: &CallerIdentity) -> Result<bool, ErrorEnvelope> {
        self.history_step(caller, ProjectController::redo)
    }

    /// Read-only access to the single writer (tests/reporting — never
    /// a mutation path).
    pub fn inspect<R>(&self, f: impl FnOnce(&ProjectController) -> R) -> R {
        f(&self.lock_controller())
    }

    /// One fenced+journaled history navigation step — a history
    /// mutation is a human-authority commit, same class as
    /// `commit_manual`: any unauthenticated caller must never rewind
    /// committed work, and the durable record names the real caller.
    fn history_step(
        &self,
        caller: &CallerIdentity,
        step: impl FnOnce(&mut ProjectController) -> Result<bool, synvelis_project::Error>,
    ) -> Result<bool, ErrorEnvelope> {
        if caller.principal().kind() != PrincipalKind::Human {
            return Err(GatewayError::Denied {
                stage: AuthorizationStage::Commit,
                reason: Some("history navigation requires human authority".to_owned()),
            }
            .into());
        }
        self.fence.section(|| {
            if self.diverged() {
                return Err(GatewayError::Indeterminate {
                    reason: "commit gate diverged from the durable journal; reopen to reconcile"
                        .to_owned(),
                }
                .into());
            }
            let mut controller = self.lock_controller();
            let base_revision = controller.live_revision();
            let changed = step(&mut controller).map_err(|e| {
                ErrorEnvelope::from(GatewayError::Internal {
                    reason: format!("history navigation failed: {e}"),
                })
            })?;
            if !changed {
                return Ok(false);
            }
            let revision = controller.live_revision();
            let state = serde_json::to_vec(&SnapshotEnvelope {
                schema_version: SCHEMA_VERSION,
                project_id: self.project,
                revision,
                project: controller.live().clone(),
            })
            .map_err(|e| {
                ErrorEnvelope::from(GatewayError::Internal {
                    reason: format!("snapshot encode failed: {e}"),
                })
            })?;
            // An undo/redo is a commit decision like any other — the
            // journaled post-state is what recovery restores.
            let payload = JournalPayload::Committed(Box::new(CommitRecord {
                operation: None,
                proposal: None,
                principal: caller.principal().clone(),
                project: self.project,
                base_revision,
                revision,
                idempotency: None,
                grant: None,
                budget_spend: false,
                approval: None,
                digest: None,
                policy_lsn: None,
                state,
            }));
            match self.record(&payload) {
                Ok(_) => {}
                Err(e) => {
                    self.diverged
                        .store(true, std::sync::atomic::Ordering::Release);
                    return Err(ErrorEnvelope::from(GatewayError::Indeterminate {
                        reason: format!("durable history record failed: {e}"),
                    }));
                }
            }
            Ok(true)
        })
    }

    // ----- fenced policy administration (ordering = journal seq) -----

    /// Revokes a grant through the fence — the durable record is written
    /// *before* the store mutation so a crash can never lose a revoke
    /// the journal confirmed (REC-01/02/08).
    ///
    /// # Errors
    /// IO failure writing the durable revoke record.
    pub fn revoke_grant(&self, id: &GrantRef) -> Result<(), ErrorEnvelope> {
        self.policy_event(PolicyEvent::RevokeGrant(id.clone()), |a| a.revoke_grant(id))
    }

    /// Issues a grant through the fence (durable record first).
    ///
    /// # Errors
    /// IO failure writing the durable policy record.
    pub fn issue_grant(&self, grant: &Grant) -> Result<(), ErrorEnvelope> {
        self.policy_event(PolicyEvent::IssueGrant(Box::new(grant.clone())), |a| {
            a.issue_grant(grant.clone())
        })
    }

    /// Issues a task authorization through the fence.
    ///
    /// # Errors
    /// IO failure writing the durable policy record.
    pub fn issue_task(&self, task: &AuthorizedTask) -> Result<(), ErrorEnvelope> {
        self.policy_event(PolicyEvent::IssueTask(Box::new(task.clone())), |a| {
            a.issue_task(task.clone())
        })
    }

    /// Revokes a task authorization through the fence.
    ///
    /// # Errors
    /// IO failure writing the durable policy record.
    pub fn revoke_task(&self, id: &TaskRef) -> Result<(), ErrorEnvelope> {
        self.policy_event(PolicyEvent::RevokeTask(id.clone()), |a| a.revoke_task(id))
    }

    /// Issues a proactive permit through the fence.
    ///
    /// # Errors
    /// IO failure writing the durable policy record.
    pub fn issue_permit(&self, permit: &ProactivePermit) -> Result<(), ErrorEnvelope> {
        self.policy_event(PolicyEvent::IssuePermit(Box::new(permit.clone())), |a| {
            a.issue_permit(permit.clone())
        })
    }

    /// Revokes a proactive permit through the fence.
    ///
    /// # Errors
    /// IO failure writing the durable policy record.
    pub fn revoke_permit(&self, id: &ProactivePermitRef) -> Result<(), ErrorEnvelope> {
        self.policy_event(PolicyEvent::RevokePermit(id.clone()), |a| {
            a.revoke_permit(id)
        })
    }

    /// Advances the policy epoch through the fence.
    ///
    /// # Errors
    /// IO failure writing the durable policy record.
    pub fn advance_epoch(&self) -> Result<PolicyEpoch, ErrorEnvelope> {
        let mut epoch = None;
        self.policy_event(PolicyEvent::EpochAdvanced, |a| {
            epoch = Some(a.advance_epoch()?);
            Ok(())
        })?;
        Ok(epoch.unwrap_or(PolicyEpoch(0)))
    }

    fn policy_event(
        &self,
        event: PolicyEvent,
        apply: impl FnOnce(&dyn PolicyAdmin) -> io::Result<()>,
    ) -> Result<(), ErrorEnvelope> {
        self.fence.section(|| {
            self.record(&JournalPayload::Policy(event)).map_err(|e| {
                ErrorEnvelope::from(GatewayError::Indeterminate {
                    reason: format!("durable policy record failed: {e}"),
                })
            })?;
            apply(&*self.deps.admin).map_err(|e| {
                ErrorEnvelope::from(GatewayError::Indeterminate {
                    reason: format!("durable policy mutation failed: {e}"),
                })
            })
        })
    }

    /// Publishes a snapshot HEAD through the fence — serialized with
    /// commits so HEAD can never rewind to an older state after a newer
    /// one published (REC-10).
    ///
    /// # Errors
    /// `Denied` when the caller is not a trusted human; store errors
    /// otherwise. Durable-record failures surface `Indeterminate`.
    pub fn save(
        &self,
        caller: &CallerIdentity,
        store: &mut dyn ProjectStore,
    ) -> Result<synvelis_project::SavedState, ErrorEnvelope> {
        if caller.principal().kind() != PrincipalKind::Human {
            return Err(GatewayError::Denied {
                stage: AuthorizationStage::Commit,
                reason: Some("publishing HEAD requires human authority".to_owned()),
            }
            .into());
        }
        self.fence.section(|| {
            if self.diverged() {
                return Err(GatewayError::Indeterminate {
                    reason: "commit gate diverged from the durable journal; reopen to reconcile"
                        .to_owned(),
                }
                .into());
            }
            let mut controller = self.lock_controller();
            let saved = controller.save(store).map_err(|e| {
                ErrorEnvelope::from(GatewayError::Internal {
                    reason: format!("save failed: {e}"),
                })
            })?;
            let payload = JournalPayload::Saved(SaveRecord {
                project: self.project,
                revision: saved.revision,
                head: saved.head.clone(),
            });
            self.record(&payload).map_err(|e| {
                ErrorEnvelope::from(GatewayError::Indeterminate {
                    reason: format!("durable save record failed: {e}"),
                })
            })?;
            Ok(saved)
        })
    }
}

/// What the restart path proved about durable state (REC-09 evidence —
/// a caller must see exactly which guarantees held).
#[derive(Clone, Debug, Default)]
pub struct RecoveryReport {
    /// The journal scanned.
    pub journal_path: PathBuf,
    /// Replay stop reason (torn tail / corruption evidence).
    pub stop: journal::StopReason,
    /// Bytes truncated as an incomplete tail.
    pub truncated_bytes: u64,
    /// Payloads that decoded to no known variant (post-CRC schema
    /// drift) — the replay stopped trusting the log there.
    pub unreadable_records: u64,
    /// Commit records replayed.
    pub commits_replayed: u64,
    /// Non-commit outcome records replayed.
    pub outcomes_replayed: u64,
    /// Policy lifecycle events replayed.
    pub policy_events_replayed: u64,
    /// Save records replayed.
    pub saves_replayed: u64,
    /// HEAD's revision as persisted at restart.
    pub saved_head: Option<ProjectRevision>,
    /// HEAD after replay (last `Saved` record).
    pub head_revision: Option<ProjectRevision>,
    /// The revision recovery restored from the journal, when ahead of
    /// the saved head.
    pub restored_revision: Option<ProjectRevision>,
    /// Whether the recovered state came from a journaled commit rather
    /// than a persisted snapshot alone.
    pub restored_from_journal: bool,
    /// The controller's revision after recovery.
    pub recovered_revision: ProjectRevision,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(operation: u128, request: &str, args: serde_json::Value) -> OperationRequest {
        use crate::capability::{CapabilityRef, OperationDomain};
        use crate::id::{AdapterId, RequestId};
        use crate::request::{AdapterSeal, Consistency, OperationTarget, OriginKind};
        use crate::schema::SchemaRef;

        OperationRequest {
            request_id: RequestId::new(request).unwrap(),
            operation_id: OperationId::new(operation),
            caller: CallerIdentity::new(
                Principal::new(PrincipalKind::Agent, "a1").unwrap(),
                OriginKind::TestFixture,
                &AdapterSeal::issue(AdapterId::new(1)),
            ),
            capability: CapabilityRef {
                id: crate::capability::CapabilityId::new("synvelis.apply").unwrap(),
                version: crate::capability::CapabilityVersion::new(1, 0),
            },
            domain: OperationDomain::ProjectMutation,
            project: ProjectId::new(7),
            target: OperationTarget::None,
            args,
            consistency: Consistency::ExpectedRevision {
                revision: ProjectRevision(1),
            },
            input_schema: SchemaRef::new("schema", 1, 0),
            task: Some(TaskRef::new("t1").unwrap()),
            grant: Some(GrantRef::new("g1").unwrap()),
            idempotency: Some(IdempotencyKey::new("k1").unwrap()),
        }
    }

    #[test]
    fn fingerprint_is_stable_and_identity_blind() {
        // The fingerprint covers payload identity, not operation ids —
        // a retry re-sends the same content under a fresh request id.
        let first = req(1, "req-a", serde_json::json!({"muted": true}));
        let retry = req(2, "req-b", serde_json::json!({"muted": true}));
        assert_eq!(
            CommitGate::<MutexFence>::fingerprint_of(&first),
            CommitGate::<MutexFence>::fingerprint_of(&retry),
            "same payload must collide across fresh operation/request ids"
        );
        assert_eq!(
            CommitGate::<MutexFence>::ledger_key_of(&first),
            CommitGate::<MutexFence>::ledger_key_of(&retry),
        );
        let drift = req(3, "req-c", serde_json::json!({"muted": false}));
        assert_ne!(
            CommitGate::<MutexFence>::fingerprint_of(&first),
            CommitGate::<MutexFence>::fingerprint_of(&drift),
            "a payload drift under the same key must conflict"
        );
    }
}
