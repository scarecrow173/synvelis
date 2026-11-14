//! Workflow Run / Step state machine, run journal, and resume cursor
//! (AUTX-06 / synvelis#265).
//!
//! This module owns *progress state only*: which step occurrence of a
//! lowered [`ExecutablePlan`](crate::ExecutablePlan) has been admitted,
//! observed, or settled, plus the append-only journal that makes that
//! state replayable. It deliberately does **not** own Project revision /
//! CAS / Undo, Job execution phase, Grant / Approval state, or
//! Proposal / Apply state — those remain with the Gateway, Job engine,
//! and project authorities (`docs/automation/design-workflow-run.md`
//! §1 boundary table). Journal entries carry references and observed
//! values only; they never become a second source of truth for those
//! domains.
//!
//! The journal is the single durable record of a run. [`WorkflowRun`]
//! is a *projection* rebuilt from the journal header plus entries, so a
//! coordinator restart (new process, dropped connection, CLI cancel)
//! loses nothing: the same `run_id` re-reads state, resumes from the
//! cursor, and re-issues cancel requests honestly.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use synvelis_gateway::id::{
    ApprovalRef, GrantRef, IdempotencyKey, JobRef, OperationId, ProposalRef, TaskRef,
};
use synvelis_gateway::outcome::RetryAdvice;
use synvelis_gateway::request::Principal;

use crate::canon::digest_of;
use crate::error::AutomationError;

/// Maximum accepted run-id length (same bound discipline as gateway ids).
pub const MAX_RUN_ID_LEN: usize = 128;

/// A caller-visible workflow run identifier.
///
/// Run identity is workflow-owned: no gateway or project type covers it.
/// The string form is bounded and non-empty so it is safe to journal and
/// to echo back through status/cancel surfaces.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(String);

impl RunId {
    /// Wraps a run id, rejecting empty or over-long values.
    ///
    /// # Errors
    /// [`RunError::MalformedRunId`] when `id` is empty or exceeds
    /// [`MAX_RUN_ID_LEN`][Self::MAX_RUN_ID_LEN] characters.
    pub fn new(id: impl Into<String>) -> Result<Self, RunError> {
        let id = id.into();
        if id.is_empty() || id.chars().count() > MAX_RUN_ID_LEN {
            return Err(RunError::MalformedRunId {
                reason: format!("run id must be 1..={MAX_RUN_ID_LEN} chars"),
            });
        }
        Ok(Self(id))
    }

    /// The raw run id string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Run-level finite state machine
/// (`docs/automation/design-workflow-run.md` §2).
///
/// `Created → Ready → Running ⇄ Waiting → Approval? → Completed`,
/// `Running → CancelRequested → Completed|Failed`, `Running → Failed`,
/// and `Running/Waiting → Indeterminate → (reconcile) → Running|Failed`.
/// `Completed` and `Failed` are terminal. `Indeterminate` is
/// non-terminal and can only be left through the reconcile path — it
/// can never become `Completed` or `Failed` on this FSM alone, so
/// unknown state can never be promoted to success without evidence.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RunState {
    /// Run record created; plan/context not yet validated for start.
    Created,
    /// Plan digest/definition/revision identity verified; ready to run.
    Ready,
    /// Actively driving step occurrences.
    Running,
    /// Blocked on an authoritative job observation.
    Waiting,
    /// Blocked on a trusted approval the workflow cannot self-issue.
    Approval,
    /// Cancel requested; settling in-flight authoritative work before a
    /// terminal verdict. Never implies the cancel was confirmed.
    CancelRequested,
    /// Every required step settled with evidence; terminal success.
    Completed,
    /// Typed failure or confirmed cancellation; terminal.
    Failed,
    /// An authoritative observation was lost or unknown; reconcile
    /// before any further progress. Non-terminal.
    Indeterminate,
}

impl RunState {
    /// Terminal states admit no outgoing transition.
    #[must_use]
    pub const fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }

    /// Whether `next` is a legal FSM edge from `self`.
    ///
    /// `Indeterminate → CancelRequested` exists so a run cancelled
    /// while a job observation was unknown can resume its cancel settle
    /// after reconcile; `Indeterminate` itself can only be entered from
    /// live states and only left toward `Running`, `CancelRequested`,
    /// or `Failed` — never directly to `Completed`.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        use RunState::{
            Approval, CancelRequested, Completed, Created, Failed, Indeterminate, Ready, Running,
            Waiting,
        };
        match self {
            Created => matches!(next, Ready | Failed),
            Ready => matches!(next, Running | Failed | CancelRequested),
            Running => matches!(
                next,
                Waiting | Approval | CancelRequested | Completed | Failed | Indeterminate
            ),
            Waiting => matches!(next, Running | CancelRequested | Failed | Indeterminate),
            Approval => matches!(next, Running | CancelRequested | Failed | Indeterminate),
            CancelRequested => matches!(next, Completed | Failed | Indeterminate),
            Indeterminate => matches!(next, Running | CancelRequested | Failed),
            Completed | Failed => false,
        }
    }

    /// Validates and returns the `next` state.
    ///
    /// # Errors
    /// [`RunError::InvalidTransition`] when the edge is not in the FSM.
    pub fn transition(self, next: Self) -> Result<Self, RunError> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(RunError::InvalidTransition {
                from: self,
                to: next,
            })
        }
    }
}

/// Step-level state machine
/// (`docs/automation/design-workflow-run.md` §2).
///
/// `Pending → Ready → Running → {Completed | Failed | Waiting |
/// Approval | CancelRequested | Indeterminate}`. `Waiting` and
/// `Approval` re-enter `Running` when re-driven; `Indeterminate` is
/// reconcile-only and never reaches `Completed` directly.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StepState {
    /// Defined by the plan but not yet admitted.
    Pending,
    /// Evaluated and cleared to execute.
    Ready,
    /// Admitted to the authoritative lane (side effect may exist).
    Running,
    /// Settled with evidence.
    Completed,
    /// Settled with a typed failure.
    Failed,
    /// Waiting on an authoritative job observation.
    Waiting,
    /// Waiting on a trusted approval.
    Approval,
    /// Run cancellation reached this in-flight step; settling only.
    CancelRequested,
    /// Observation lost; reconcile before any verdict.
    Indeterminate,
}

impl StepState {
    /// Terminal states admit no outgoing transition.
    #[must_use]
    pub const fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }

    /// Whether `next` is a legal FSM edge from `self`.
    #[must_use]
    pub const fn can_transition_to(self, next: Self) -> bool {
        use StepState::{
            Approval, CancelRequested, Completed, Failed, Indeterminate, Pending, Ready, Running,
            Waiting,
        };
        match self {
            Pending => matches!(next, Ready),
            Ready => matches!(next, Running | CancelRequested),
            Running => matches!(
                next,
                Completed | Failed | Waiting | Approval | CancelRequested | Indeterminate
            ),
            Waiting => matches!(next, Running | CancelRequested | Failed | Indeterminate),
            Approval => matches!(next, Running | CancelRequested | Failed | Indeterminate),
            CancelRequested => matches!(next, Completed | Failed | Indeterminate),
            Indeterminate => matches!(next, Running | Waiting | CancelRequested | Failed),
            Completed | Failed => false,
        }
    }

    /// Validates and returns the `next` state.
    ///
    /// # Errors
    /// [`RunError::InvalidStepTransition`] when the edge is not legal.
    pub fn transition(self, next: Self) -> Result<Self, RunError> {
        if self.can_transition_to(next) {
            Ok(next)
        } else {
            Err(RunError::InvalidStepTransition {
                from: self,
                to: next,
            })
        }
    }
}

/// The settled verdict recorded on a [`StepCheckpoint`].
///
/// Mirrors the non-`Pending`/`Ready`/`Running` step outcomes; the
/// checkpoint's `state` field carries the FSM position, `verdict` the
/// last authoritative observation class.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StepVerdict {
    /// The step's effect/result was observed and journaled.
    Completed,
    /// The step failed with a typed reason.
    Failed,
    /// The step is parked on a job observation.
    Waiting,
    /// The step is parked on a trusted approval.
    Approval,
    /// Run cancellation reached the step; provider confirmation pending
    /// or the cancel won the race.
    CancelRequested,
    /// The observation is unknown; reconcile required.
    Indeterminate,
}

/// One frame of a step occurrence path.
///
/// `iteration` is `Some(i)` only on frames belonging to a
/// `BoundedLoop` body occurrence; control-step frames and top-level
/// steps carry `None`. The full [`StepPath`] uniquely identifies one
/// step occurrence so journal entries, checkpoints, and the resume
/// cursor can distinguish loop iterations and nested branches.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StepFrame {
    /// The workflow step id.
    pub step_id: String,
    /// Loop iteration index for body occurrences, else `None`.
    pub iteration: Option<u64>,
}

impl StepFrame {
    /// A non-iterated frame for `step_id`.
    #[must_use]
    pub fn new(step_id: impl Into<String>) -> Self {
        Self {
            step_id: step_id.into(),
            iteration: None,
        }
    }

    /// An iterated frame for `step_id` at `iteration`.
    #[must_use]
    pub fn iterated(step_id: impl Into<String>, iteration: u64) -> Self {
        Self {
            step_id: step_id.into(),
            iteration: Some(iteration),
        }
    }
}

/// The occurrence path of a step — root-to-leaf frames.
pub type StepPath = Vec<StepFrame>;

/// Progress record for one step occurrence.
///
/// References (`operation_id`, `proposal_ref`, `approval_ref`,
/// `job_ref`) point at authoritative objects owned elsewhere; the
/// checkpoint stores ids and observed digests only, never a copy of the
/// referenced state. `expected_revision` is the CAS expectation the
/// step asserted; `output_digest` commits to the journaled output value
/// without duplicating large payloads in the checkpoint itself (the
/// journal entry carries the bounded output).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepCheckpoint {
    /// Occurrence path (distinguishes loop iterations / nesting).
    pub path: StepPath,
    /// Workflow step id (last frame's `step_id`).
    pub step_id: String,
    /// Current FSM state.
    pub state: StepState,
    /// Last authoritative verdict, when the step has been observed.
    pub verdict: Option<StepVerdict>,
    /// Operation id bound by the lane for this occurrence, when known.
    pub operation_id: Option<OperationId>,
    /// Proposal prepared by the lane, when the outcome was
    /// `ProposalReady`.
    pub proposal_ref: Option<ProposalRef>,
    /// Approval request awaiting trusted resolution, when the outcome
    /// was `ApprovalRequired`.
    pub approval_ref: Option<ApprovalRef>,
    /// Provider job ref disclosed by the job observation, when known.
    pub job_ref: Option<JobRef>,
    /// Idempotency key this occurrence was admitted under — stable
    /// across crash-retry so the authoritative side dedupes.
    pub idempotency_key: Option<IdempotencyKey>,
    /// Project revision the step asserted (mutation steps only).
    pub expected_revision: Option<u64>,
    /// sha256 digest of the journaled output value.
    pub output_digest: Option<String>,
    /// Gateway retry advice attached to the last error, when any.
    pub retry_policy: Option<RetryAdvice>,
    /// Bounded safe-to-disclose detail (failure reason, observation).
    pub detail: Option<String>,
}

/// The durable run header — recorded once by [`RunJournal::create_run`].
///
/// This is the plan-idempotency fingerprint: a second `start_run` with
/// the same `run_id` replays only when every identity field matches;
/// any mismatch is an [`RunError::IdempotencyConflict`]. Principal,
/// task, and grant are *references* — the journal stores who the run
/// runs as, never their authority material.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunHeader {
    /// Run identity.
    pub run_id: RunId,
    /// `ExecutablePlan::digest()` at start — plan tamper evidence (F-10).
    pub plan_digest: String,
    /// `WorkflowDefinition::version` the plan was compiled from.
    pub definition_version: u32,
    /// Tagged project id the run mutates/reads.
    pub project_id: String,
    /// Canonical digest of the provided inputs.
    pub input_digest: String,
    /// The run principal (identity reference only).
    pub principal: Principal,
    /// Policy epoch observed at start, when the caller pins one.
    pub policy_epoch: Option<u64>,
    /// Base project revision pinned at start, when the caller pins one.
    pub expected_revision: Option<u64>,
    /// Caller idempotency key for this run start.
    pub idempotency_key: IdempotencyKey,
    /// Optional task/grant scope references (identity only).
    pub task: Option<TaskRef>,
    /// Grant reference, when the run presents one.
    pub grant: Option<GrantRef>,
}

/// Structured reference set carried by a journal entry — ids and
/// observed values only, never authoritative state copies.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct JournalRefs {
    /// Bound operation id (capability call / job handle).
    pub operation: Option<OperationId>,
    /// Prepared proposal reference.
    pub proposal: Option<ProposalRef>,
    /// Approval reference awaiting trusted resolution.
    pub approval: Option<ApprovalRef>,
    /// Disclosed provider job reference.
    pub job: Option<JobRef>,
}

/// Journal entry kind — the append-only event vocabulary.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JournalKind {
    /// Run-level state transition (`from_state`/`to_state` set).
    RunTransition,
    /// A step occurrence was admitted — intent record written *before*
    /// any lane call so a crash between admit and settle is detectable.
    StepBegan,
    /// A step occurrence produced a non-terminal observation
    /// (`verdict` = `Waiting`/`Approval`).
    StepObserved,
    /// A step occurrence reached a settled verdict
    /// (`verdict` = `Completed`/`Failed`/`Indeterminate`).
    StepSettled,
    /// A cancel request was issued to an authoritative job owner; the
    /// response class is recorded in `detail` — never upgraded to
    /// "cancelled" without provider confirmation.
    CancelIssued,
    /// Terminal run record (`to_state` = `Completed`/`Failed`).
    RunSettled,
}

/// One append-only journal record.
///
/// `seq` is the per-run append order assigned by the journal. `digest`
/// commits to the canonical form of `output` (or the entry's semantic
/// payload when there is no output) so replay can detect torn writes.
/// `plan_revision` carries the plan digest — the contract's
/// `plan_revision` field — on every entry so a plan swap mid-run is
/// detectable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Owning run.
    pub run_id: RunId,
    /// Per-run append sequence (0-based, dense).
    pub seq: u64,
    /// Step occurrence this entry concerns; `None` for run-level kinds.
    pub step: Option<StepPath>,
    /// Plan digest — the contract's `plan_revision`.
    pub plan_revision: String,
    /// Effective capability ref invoked, when the entry concerns a call.
    pub capability_ref: Option<String>,
    /// Event kind.
    pub kind: JournalKind,
    /// Step verdict for `StepObserved`/`StepSettled`.
    pub verdict: Option<StepVerdict>,
    /// Source state for `RunTransition`.
    pub from_state: Option<RunState>,
    /// Target state for `RunTransition`/`RunSettled`.
    pub to_state: Option<RunState>,
    /// Authoritative references observed.
    pub refs: JournalRefs,
    /// Idempotency key the occurrence was admitted under.
    pub idempotency_key: Option<IdempotencyKey>,
    /// Project revision asserted by the call, when applicable.
    pub expected_revision: Option<u64>,
    /// sha256 of the canonical payload this entry commits to.
    pub digest: String,
    /// Bounded step output value (already size-capped by the caller).
    pub output: Option<Value>,
    /// Bounded safe-to-disclose note (reasons, cancel response class).
    pub detail: Option<String>,
    /// Gateway retry advice recorded with the verdict — journaled so a
    /// replayed checkpoint carries the same correction guidance the
    /// live session saw.
    pub retry_advice: Option<RetryAdvice>,
}

/// Errors raised by journal operations and run-state validation.
#[derive(Clone, Debug, PartialEq)]
pub enum RunError {
    /// Journal read/write failure.
    Journal {
        /// Human-safe reason.
        reason: String,
    },
    /// `run_id` string violated the bound.
    MalformedRunId {
        /// Human-safe reason.
        reason: String,
    },
    /// Run id already exists with an identical fingerprint — the
    /// duplicate-start rejection (plan idempotency layer).
    DuplicateRun {
        /// The conflicting run id.
        run_id: RunId,
    },
    /// Run id already exists with a different fingerprint.
    IdempotencyConflict {
        /// The conflicting run id.
        run_id: RunId,
    },
    /// No journal header for this run id.
    UnknownRun {
        /// The unknown run id.
        run_id: RunId,
    },
    /// Plan/definition identity mismatch at start or resume (F-10).
    PlanMismatch {
        /// Human-safe reason.
        reason: String,
    },
    /// Illegal run FSM transition.
    InvalidTransition {
        /// Source state.
        from: RunState,
        /// Target state.
        to: RunState,
    },
    /// Illegal step FSM transition.
    InvalidStepTransition {
        /// Source state.
        from: StepState,
        /// Target state.
        to: StepState,
    },
    /// Expression/condition evaluation failure inside a step.
    Eval(crate::eval::EvalError),
    /// A declared bound was exceeded at runtime.
    BudgetExceeded {
        /// Which budget (`max-steps`/`max-iterations`/`max-runtime`).
        kind: &'static str,
        /// Human-safe detail.
        detail: String,
    },
    /// The lane violated its contract (e.g. unparsable job handle).
    Lane {
        /// Human-safe reason.
        reason: String,
    },
    /// `result()` was asked for a run that has not completed.
    NotCompleted {
        /// The run id.
        run_id: RunId,
        /// The run's current state.
        state: RunState,
    },
    /// Journal replay detected divergent records.
    ReplayDivergence {
        /// Human-safe reason.
        reason: String,
    },
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Journal { reason } => write!(f, "run journal error: {reason}"),
            Self::MalformedRunId { reason } => write!(f, "malformed run id: {reason}"),
            Self::DuplicateRun { run_id } => write!(f, "duplicate run start: {run_id}"),
            Self::IdempotencyConflict { run_id } => {
                write!(f, "run idempotency conflict: {run_id}")
            }
            Self::UnknownRun { run_id } => write!(f, "unknown run: {run_id}"),
            Self::PlanMismatch { reason } => write!(f, "plan identity mismatch: {reason}"),
            Self::InvalidTransition { from, to } => {
                write!(f, "invalid run transition {from:?} -> {to:?}")
            }
            Self::InvalidStepTransition { from, to } => {
                write!(f, "invalid step transition {from:?} -> {to:?}")
            }
            Self::Eval(e) => write!(f, "step evaluation failed: {e}"),
            Self::BudgetExceeded { kind, detail } => {
                write!(f, "run budget exceeded ({kind}): {detail}")
            }
            Self::Lane { reason } => write!(f, "lane contract violation: {reason}"),
            Self::NotCompleted { run_id, state } => {
                write!(f, "run {run_id} has no result (state {state:?})")
            }
            Self::ReplayDivergence { reason } => {
                write!(f, "journal replay divergence: {reason}")
            }
        }
    }
}

impl std::error::Error for RunError {}

impl From<crate::eval::EvalError> for RunError {
    fn from(e: crate::eval::EvalError) -> Self {
        Self::Eval(e)
    }
}

/// The append-only run journal seam.
///
/// Implementations must be durable-enough for the slice's contract:
/// `create_run` is atomic with respect to duplicate detection, `append`
/// preserves per-run `seq` order, and reads return entries in `seq`
/// order. The in-crate [`MemoryRunJournal`] satisfies this for tests
/// and headless fixtures; a filesystem/DB journal is an integration
/// concern and plugs in behind the same trait.
pub trait RunJournal: Send + Sync {
    /// Records the run header, rejecting duplicate/conflicting ids.
    ///
    /// # Errors
    /// [`RunError::DuplicateRun`] when `run_id` exists with an
    /// identical fingerprint; [`RunError::IdempotencyConflict`] when it
    /// exists with a different one; [`RunError::Journal`] on storage
    /// failure.
    fn create_run(&self, header: &RunHeader) -> Result<(), RunError>;

    /// Returns the stored header for `run_id`, if any.
    ///
    /// # Errors
    /// [`RunError::Journal`] on storage failure.
    fn run_header(&self, run_id: &RunId) -> Result<Option<RunHeader>, RunError>;

    /// Appends one entry. The journal assigns/validates `seq`.
    ///
    /// # Errors
    /// [`RunError::Journal`] on storage failure or unknown `run_id`.
    fn append(&self, entry: JournalEntry) -> Result<(), RunError>;

    /// Returns all entries for `run_id` in `seq` order.
    ///
    /// # Errors
    /// [`RunError::Journal`] on storage failure.
    fn entries(&self, run_id: &RunId) -> Result<Vec<JournalEntry>, RunError>;
}

/// In-memory journal for fixtures and headless runs.
///
/// Entries live behind a `Mutex` so the journal can be shared across
/// coordinator instances (the disconnect/restart seam). Durability
/// across process death is intentionally out of scope for this slice —
/// the *contract* (append order, dedup, replay) is what the fault
/// fixtures exercise.
#[derive(Default)]
pub struct MemoryRunJournal {
    inner: std::sync::Mutex<BTreeMap<RunId, MemoryRunData>>,
}

#[derive(Default)]
struct MemoryRunData {
    header: Option<RunHeader>,
    entries: Vec<JournalEntry>,
}

impl MemoryRunJournal {
    /// An empty journal.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<RunId, MemoryRunData>> {
        self.inner.lock().expect("run journal poisoned")
    }
}

impl RunJournal for MemoryRunJournal {
    fn create_run(&self, header: &RunHeader) -> Result<(), RunError> {
        let mut map = self.lock();
        if let Some(existing) = map.get(&header.run_id).and_then(|d| d.header.as_ref()) {
            return if same_run_fingerprint(existing, header) {
                Err(RunError::DuplicateRun {
                    run_id: header.run_id.clone(),
                })
            } else {
                Err(RunError::IdempotencyConflict {
                    run_id: header.run_id.clone(),
                })
            };
        }
        map.entry(header.run_id.clone()).or_default().header = Some(header.clone());
        Ok(())
    }

    fn run_header(&self, run_id: &RunId) -> Result<Option<RunHeader>, RunError> {
        Ok(self.lock().get(run_id).and_then(|d| d.header.clone()))
    }

    fn append(&self, entry: JournalEntry) -> Result<(), RunError> {
        let mut map = self.lock();
        let data = map
            .get_mut(&entry.run_id)
            .ok_or_else(|| RunError::Journal {
                reason: format!("append for unknown run {}", entry.run_id),
            })?;
        let seq = data.entries.len() as u64;
        if entry.seq != seq {
            return Err(RunError::Journal {
                reason: format!(
                    "journal seq mismatch for {}: expected {seq}, got {}",
                    entry.run_id, entry.seq
                ),
            });
        }
        data.entries.push(entry);
        Ok(())
    }

    fn entries(&self, run_id: &RunId) -> Result<Vec<JournalEntry>, RunError> {
        Ok(self
            .lock()
            .get(run_id)
            .map(|d| d.entries.clone())
            .unwrap_or_default())
    }
}

/// Fingerprint equality for plan idempotency — the fields that define
/// "the same run request". Two starts with the same `run_id` but any
/// differing field are a conflict, not a replay.
fn same_run_fingerprint(a: &RunHeader, b: &RunHeader) -> bool {
    a.plan_digest == b.plan_digest
        && a.definition_version == b.definition_version
        && a.project_id == b.project_id
        && a.input_digest == b.input_digest
        && a.idempotency_key == b.idempotency_key
        && a.principal == b.principal
        && a.policy_epoch == b.policy_epoch
        && a.expected_revision == b.expected_revision
        && a.task == b.task
        && a.grant == b.grant
}

/// The materialized run state — a projection of header + journal.
///
/// `steps` holds one checkpoint per *observed* occurrence (steps that
/// never began have no checkpoint; untaken branches/iterations simply
/// never appear). `step_outputs` feeds `$steps.*` references during
/// execution and is rebuilt on replay. `pending_jobs` lists job handles
/// submitted but not yet authoritatively settled — the cancel
/// propagation set. `cancel_issued` records which jobs already received
/// a cancel request so re-drives poll status instead of resending
/// (F-4 honesty).
#[derive(Clone, Debug)]
pub struct WorkflowRun {
    /// Run identity.
    pub run_id: RunId,
    /// Current FSM state.
    pub state: RunState,
    /// Plan digest from the header.
    pub plan_digest: String,
    /// Definition version from the header.
    pub definition_version: u32,
    /// Tagged project id.
    pub project_id: String,
    /// Inputs digest from the header.
    pub input_digest: String,
    /// Run principal reference.
    pub principal: Principal,
    /// Policy epoch pinned at start.
    pub policy_epoch: Option<u64>,
    /// Latest known committed project revision (evolves as mutation
    /// outcomes report `revision`; the asserted CAS for the next
    /// mutation step).
    pub expected_revision: Option<u64>,
    /// Task/grant scope references.
    pub task: Option<TaskRef>,
    /// Grant reference.
    pub grant: Option<GrantRef>,
    /// Start idempotency key.
    pub idempotency_key: IdempotencyKey,
    /// Resume cursor — path of the last settled/observed step
    /// occurrence. Informational; the checkpoint set is authoritative
    /// for skip decisions.
    pub cursor: Option<StepPath>,
    /// Per-occurrence checkpoints in begin order.
    pub steps: Vec<StepCheckpoint>,
    /// Run outputs collected from `emit-result` steps.
    pub outputs: BTreeMap<String, Value>,
    /// Latest output per step id (feeds `$steps.*`).
    pub step_outputs: BTreeMap<String, Value>,
    /// Job handles submitted but not yet settled.
    pub pending_jobs: Vec<OperationId>,
    /// Whether run-level cancel has been requested.
    pub cancel_requested: bool,
    /// Jobs that already received a cancel request (no resend).
    pub cancel_issued: BTreeSet<OperationId>,
    /// Number of step begins journaled (runtime budget counter).
    pub executed_steps: u64,
    /// Number of loop-body step begins journaled — the runtime
    /// `max_iterations` enforcement counter.
    pub loop_iterations: u64,
}

impl WorkflowRun {
    /// Looks up the checkpoint for an occurrence path.
    #[must_use]
    pub fn checkpoint(&self, path: &StepPath) -> Option<&StepCheckpoint> {
        self.steps.iter().find(|c| &c.path == path)
    }

    /// Mutable checkpoint lookup.
    pub fn checkpoint_mut(&mut self, path: &StepPath) -> Option<&mut StepCheckpoint> {
        self.steps.iter_mut().find(|c| &c.path == path)
    }

    /// The step id a path refers to (last frame).
    #[must_use]
    pub fn path_step_id(path: &StepPath) -> Option<&str> {
        path.last().map(|f| f.step_id.as_str())
    }
}

/// Computes the canonical digest committed into a journal entry.
///
/// The digest covers the entry's semantic payload — kind, step path,
/// verdict, refs, output — so a torn or reordered append is detectable
/// on replay without trusting the writer.
///
/// # Errors
/// [`AutomationError::Json`] if the payload cannot be represented.
pub fn entry_digest(entry: &JournalEntry) -> Result<String, AutomationError> {
    let payload = serde_json::json!({
        "run": entry.run_id.as_str(),
        "seq": entry.seq,
        "step": entry.step,
        "plan": entry.plan_revision,
        "capability": entry.capability_ref,
        "kind": entry.kind,
        "verdict": entry.verdict,
        "from": entry.from_state,
        "to": entry.to_state,
        "refs": {
            "operation": entry.refs.operation.as_ref().map(|o| o.to_tagged()),
            "proposal": entry.refs.proposal.as_ref().map(ProposalRef::as_str),
            "approval": entry.refs.approval.as_ref().map(ApprovalRef::as_str),
            "job": entry.refs.job.as_ref().map(JobRef::as_str),
        },
        "key": entry.idempotency_key.as_ref().map(IdempotencyKey::as_str),
        "expected_revision": entry.expected_revision,
        "output": entry.output,
        "detail": entry.detail,
        "retry_advice": entry.retry_advice,
    });
    digest_of(&payload)
}

/// Rebuilds a [`WorkflowRun`] projection from the journal.
///
/// `definition` supplies the compiled AST so `emit-result` outputs can
/// be routed into run outputs and job bookkeeping can classify step
/// kinds; the plan digest is verified against the header before any
/// state is trusted (F-10). Replay is fail-closed: divergent
/// `from_state`/`to_state` chains produce [`RunError::ReplayDivergence`].
///
/// # Errors
/// [`RunError::UnknownRun`], [`RunError::PlanMismatch`],
/// [`RunError::ReplayDivergence`], [`RunError::Journal`].
pub fn replay_run(
    journal: &dyn RunJournal,
    run_id: &RunId,
    definition: &crate::definition::WorkflowDefinition,
    plan_digest: &str,
) -> Result<WorkflowRun, RunError> {
    let header = journal
        .run_header(run_id)?
        .ok_or_else(|| RunError::UnknownRun {
            run_id: run_id.clone(),
        })?;
    if header.plan_digest != plan_digest {
        return Err(RunError::PlanMismatch {
            reason: format!(
                "journal plan {} != supplied plan {plan_digest}",
                header.plan_digest
            ),
        });
    }
    let entries = journal.entries(run_id)?;
    replay_entries(&header, &entries, definition)
}

/// Rebuilds the projection from an already-fetched header + entries —
/// the coordinator's per-drive path (avoids a second journal read).
///
/// # Errors
/// Same as [`replay_run`] minus the header fetch.
pub fn replay_entries(
    header: &RunHeader,
    entries: &[JournalEntry],
    definition: &crate::definition::WorkflowDefinition,
) -> Result<WorkflowRun, RunError> {
    // emit-result step id -> declared output name (top-level only —
    // the compiler enforces emit-result placement).
    let emit_outputs: BTreeMap<String, String> = definition
        .steps
        .iter()
        .filter_map(|s| {
            if let crate::definition::Step::EmitResult { output, .. } = s {
                Some((s.id().to_owned(), output.clone()))
            } else {
                None
            }
        })
        .collect();

    let mut run = WorkflowRun {
        run_id: header.run_id.clone(),
        state: RunState::Created,
        plan_digest: header.plan_digest.clone(),
        definition_version: header.definition_version,
        project_id: header.project_id.clone(),
        input_digest: header.input_digest.clone(),
        principal: header.principal.clone(),
        policy_epoch: header.policy_epoch,
        expected_revision: header.expected_revision,
        task: header.task.clone(),
        grant: header.grant.clone(),
        idempotency_key: header.idempotency_key.clone(),
        cursor: None,
        steps: Vec::new(),
        outputs: BTreeMap::new(),
        step_outputs: BTreeMap::new(),
        pending_jobs: Vec::new(),
        cancel_requested: false,
        cancel_issued: BTreeSet::new(),
        executed_steps: 0,
        loop_iterations: 0,
    };

    for (index, entry) in entries.iter().enumerate() {
        // Dense append order is part of the contract — a gap or
        // reorder means the journal was torn or rewritten.
        if entry.seq != index as u64 {
            return Err(RunError::ReplayDivergence {
                reason: format!(
                    "seq {}: expected dense seq {index}, journal is not append-ordered",
                    entry.seq
                ),
            });
        }
        // Torn-write / forgery gate: the stored digest must match the
        // canonical payload before the entry is trusted.
        let expected = entry_digest(entry).map_err(|e| RunError::Journal {
            reason: format!("entry digest recompute failed: {e}"),
        })?;
        if expected != entry.digest {
            return Err(RunError::ReplayDivergence {
                reason: format!(
                    "seq {}: entry digest mismatch (stored {}, computed {expected})",
                    entry.seq, entry.digest
                ),
            });
        }
        apply_entry(&mut run, entry, &emit_outputs, definition)?;
    }

    Ok(run)
}

/// Applies one journal entry to the run projection — the single code
/// path shared by cold replay and live drive, so the in-session run
/// and a restarted replay can never diverge.
///
/// # Errors
/// [`RunError::ReplayDivergence`] on inconsistent records,
/// [`RunError::Journal`] on digest failure.
pub(crate) fn apply_entry(
    run: &mut WorkflowRun,
    entry: &JournalEntry,
    emit_outputs: &BTreeMap<String, String>,
    definition: &crate::definition::WorkflowDefinition,
) -> Result<(), RunError> {
    match entry.kind {
        JournalKind::RunTransition | JournalKind::RunSettled => apply_run_record(run, entry),
        JournalKind::StepBegan => apply_step_began(run, entry),
        JournalKind::StepObserved | JournalKind::StepSettled => {
            apply_step_verdict(run, entry, emit_outputs, definition)
        }
        JournalKind::CancelIssued => {
            run.cancel_requested = true;
            if let Some(op) = entry.refs.operation {
                run.cancel_issued.insert(op);
            }
            Ok(())
        }
    }
}

/// Applies a run-level transition/settlement record.
fn apply_run_record(run: &mut WorkflowRun, entry: &JournalEntry) -> Result<(), RunError> {
    if let Some(from) = entry.from_state
        && from != run.state
    {
        return Err(RunError::ReplayDivergence {
            reason: format!(
                "seq {}: expected from_state {from:?}, run is {:?}",
                entry.seq, run.state
            ),
        });
    }
    if let Some(to) = entry.to_state {
        run.state = run
            .state
            .transition(to)
            .map_err(|_| RunError::ReplayDivergence {
                reason: format!(
                    "seq {}: illegal journaled transition {:?} -> {to:?}",
                    entry.seq, run.state
                ),
            })?;
        if to == RunState::CancelRequested {
            run.cancel_requested = true;
        }
    }
    Ok(())
}

/// Applies a `StepBegan` record — first admission creates the
/// checkpoint and counts budgets; re-activation only re-enters
/// `Running` through the FSM.
fn apply_step_began(run: &mut WorkflowRun, entry: &JournalEntry) -> Result<(), RunError> {
    let path = entry
        .step
        .clone()
        .ok_or_else(|| RunError::ReplayDivergence {
            reason: format!("seq {}: StepBegan without step path", entry.seq),
        })?;
    let step_id = WorkflowRun::path_step_id(&path)
        .unwrap_or_default()
        .to_owned();
    let iterated = path.last().is_some_and(|f| f.iteration.is_some());
    if let Some(existing) = run.checkpoint_mut(&path) {
        // Re-activation / idempotent refresh: only FSM-legal returns
        // to Running are admitted — a settled or cancel-settling
        // occurrence can never re-run, and the budget counters count
        // first admissions only.
        if existing.state != StepState::Running {
            let cur = existing.state;
            existing.state =
                cur.transition(StepState::Running)
                    .map_err(|_| RunError::ReplayDivergence {
                        reason: format!(
                            "seq {}: illegal re-begin for {path:?} from {cur:?}",
                            entry.seq
                        ),
                    })?;
        }
        if entry.idempotency_key.is_some() {
            existing.idempotency_key.clone_from(&entry.idempotency_key);
        }
        if entry.expected_revision.is_some() {
            existing.expected_revision = entry.expected_revision;
        }
    } else {
        run.executed_steps += 1;
        if iterated {
            run.loop_iterations += 1;
        }
        run.steps.push(StepCheckpoint {
            state: StepState::Running,
            path: path.clone(),
            step_id,
            verdict: None,
            operation_id: None,
            proposal_ref: None,
            approval_ref: None,
            job_ref: None,
            idempotency_key: entry.idempotency_key.clone(),
            expected_revision: entry.expected_revision,
            output_digest: None,
            retry_policy: None,
            detail: None,
        });
    }
    run.cursor = Some(path);
    Ok(())
}

/// Applies an observation/settlement record: validates the step FSM
/// edge, updates checkpoint refs, then (for settlements) routes
/// outputs, advances the revision expectation, and maintains the
/// pending-job set.
fn apply_step_verdict(
    run: &mut WorkflowRun,
    entry: &JournalEntry,
    emit_outputs: &BTreeMap<String, String>,
    definition: &crate::definition::WorkflowDefinition,
) -> Result<(), RunError> {
    let path = entry
        .step
        .clone()
        .ok_or_else(|| RunError::ReplayDivergence {
            reason: format!("seq {}: {:?} without step path", entry.seq, entry.kind),
        })?;
    let verdict = entry.verdict.ok_or_else(|| RunError::ReplayDivergence {
        reason: format!("seq {}: {:?} without verdict", entry.seq, entry.kind),
    })?;
    let target = match verdict {
        StepVerdict::Waiting => StepState::Waiting,
        StepVerdict::Approval => StepState::Approval,
        StepVerdict::CancelRequested => StepState::CancelRequested,
        StepVerdict::Indeterminate => StepState::Indeterminate,
        StepVerdict::Completed => StepState::Completed,
        StepVerdict::Failed => StepState::Failed,
    };
    let step_id = {
        let cp = run
            .checkpoint_mut(&path)
            .ok_or_else(|| RunError::ReplayDivergence {
                reason: format!("seq {}: settle without begin for {path:?}", entry.seq),
            })?;
        // The step FSM is enforced on replay exactly as on live
        // drive: parked/indeterminate occurrences must pass through
        // Running (a journaled re-begin) before a new verdict —
        // unknown state is never promoted directly.
        let cur = cp.state;
        cp.state = cur
            .transition(target)
            .map_err(|_| RunError::ReplayDivergence {
                reason: format!(
                    "seq {}: illegal step transition {cur:?} -> {target:?} for {path:?}",
                    entry.seq
                ),
            })?;
        cp.verdict = Some(verdict);
        cp.operation_id = entry.refs.operation.or(cp.operation_id);
        if entry.refs.proposal.is_some() {
            cp.proposal_ref.clone_from(&entry.refs.proposal);
        }
        if entry.refs.approval.is_some() {
            cp.approval_ref.clone_from(&entry.refs.approval);
        }
        if entry.refs.job.is_some() {
            cp.job_ref.clone_from(&entry.refs.job);
        }
        if entry.detail.is_some() {
            cp.detail.clone_from(&entry.detail);
        }
        if entry.retry_advice.is_some() {
            cp.retry_policy = entry.retry_advice;
        }
        cp.step_id.clone()
    };
    run.cursor = Some(path.clone());

    if entry.kind == JournalKind::StepSettled {
        apply_settled(
            run,
            entry,
            &path,
            &step_id,
            verdict,
            emit_outputs,
            definition,
        )?;
    }
    Ok(())
}

/// Settlement bookkeeping: declared outputs, step-output map,
/// output digests, revision chaining, and pending-job tracking.
fn apply_settled(
    run: &mut WorkflowRun,
    entry: &JournalEntry,
    path: &StepPath,
    step_id: &str,
    verdict: StepVerdict,
    emit_outputs: &BTreeMap<String, String>,
    definition: &crate::definition::WorkflowDefinition,
) -> Result<(), RunError> {
    if let Some(output) = &entry.output
        && verdict == StepVerdict::Completed
    {
        if let Some(name) = emit_outputs.get(step_id) {
            run.outputs.insert(name.clone(), output.clone());
        }
        run.step_outputs.insert(step_id.to_owned(), output.clone());
        let digest = digest_of(output).map_err(|e| RunError::Journal {
            reason: format!("output digest failed: {e}"),
        })?;
        if let Some(cp) = run.checkpoint_mut(path) {
            cp.output_digest = Some(digest);
        }
        // Revision chaining: mutation outcomes that disclose the
        // committed revision advance the run's CAS expectation.
        if let Some(rev) = output.get("revision").and_then(Value::as_u64) {
            run.expected_revision = Some(rev);
        }
    }
    // Pending-job bookkeeping: an async-submit completion adds a
    // handle; an await-job reaching a terminal verdict retires it.
    // Indeterminate keeps the handle — the job's fate is unknown,
    // never assumed settled.
    if verdict == StepVerdict::Completed
        && let Some(job_handle) = async_submit_output(entry, definition)
        && let Ok(op) = parse_operation_id(&job_handle)
        && !run.pending_jobs.contains(&op)
    {
        run.pending_jobs.push(op);
    }
    if matches!(verdict, StepVerdict::Completed | StepVerdict::Failed)
        && let (Some(op), true) = (&entry.refs.operation, is_await_step(step_id, definition))
    {
        run.pending_jobs.retain(|j| j != op);
    }
    Ok(())
}

/// Whether `step_id` names an `await-job` step anywhere in the
/// definition (nested occurrences included — step ids are unique).
fn is_await_step(step_id: &str, definition: &crate::definition::WorkflowDefinition) -> bool {
    find_step(&definition.steps, step_id)
        .is_some_and(|s| matches!(s, crate::definition::Step::AwaitJob { .. }))
}

/// Depth-first step lookup by id across nested control flow.
pub(crate) fn find_step<'a>(
    steps: &'a [crate::definition::Step],
    id: &str,
) -> Option<&'a crate::definition::Step> {
    for step in steps {
        if step.id() == id {
            return Some(step);
        }
        for child in step.children() {
            if let Some(found) = find_step(child, id) {
                return Some(found);
            }
        }
    }
    None
}

/// Extracts the job-handle output of a settled `async-submit` step.
fn async_submit_output(
    entry: &JournalEntry,
    definition: &crate::definition::WorkflowDefinition,
) -> Option<String> {
    let path = entry.step.as_ref()?;
    let step_id = WorkflowRun::path_step_id(path)?;
    let is_async = find_step(&definition.steps, step_id)
        .is_some_and(|s| matches!(s, crate::definition::Step::AsyncSubmit { .. }));
    if !is_async {
        return None;
    }
    entry
        .output
        .as_ref()
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Parses a journaled job-handle string back into an [`OperationId`].
///
/// # Errors
/// [`RunError::Lane`] when the handle is not a tagged operation id.
pub fn parse_operation_id(handle: &str) -> Result<OperationId, RunError> {
    serde_json::from_value::<OperationId>(Value::String(handle.to_owned())).map_err(|e| {
        RunError::Lane {
            reason: format!("job handle {handle:?} is not an operation id: {e}"),
        }
    })
}
