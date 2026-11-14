//! The headless workflow coordinator (AUTX-06 / synvelis#265).
//!
//! [`WorkflowCoordinator`] drives a validated [`WorkflowDefinition`] +
//! [`ExecutablePlan`] pair one step boundary at a time over two narrow
//! ports:
//!
//! - [`StepLane`] — the capability invocation seam. Production binds it
//!   to the Gateway's authorize/effect/revalidate/commit path
//!   (GW-04/05/06); fixtures bind it to a deterministic fake. Every
//!   call carries an [`IdempotencyKey`] derived from `(run_id, step
//!   path)` so crash-retry can never double-commit or double-submit.
//! - [`JobLane`] — the job status/cancel read seam keyed by the
//!   [`OperationId`] handle an `async-submit` produced. It mirrors the
//!   `JobPort` truth (`JobPhase`/`CancelLink`/`CancelResponse`
//!   honesty) without owning job state.
//!
//! Boundaries this coordinator keeps (per
//! `docs/automation/design-workflow-run.md` §1):
//!
//! - It never holds Gateway, `CommitFence`, or project locks across a
//!   call — every port call is sequential and returns before the next
//!   begins; there is no re-entrant invocation path.
//! - It owns *progress state only*: the run journal, resume cursor,
//!   and per-step verdicts. Job phase, project revision, proposal and
//!   approval truth stay with their authorities; the journal records
//!   references and observations, never copies.
//! - `Indeterminate` is never promoted: unknown observations park the
//!   run until [`WorkflowCoordinator::reconcile`] re-queries the
//!   authoritative side with the same idempotency identity.
//! - Cancellation is honest: `CancelRequested` records the request and
//!   per-job [`CancelResponse`] classes; `Cancelled` appears only when
//!   the provider confirms, and `CancelRequested → Completed` is taken
//!   only when authoritative evidence shows the work actually settled.
//!
//! The coordinator is synchronous and non-RT: no allocation policy,
//! threading, or audio-callback concerns apply. Driving is pull-based —
//! `drive` advances until the run blocks (job wait, approval wait),
//! fails, completes, or goes indeterminate.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;
use synvelis_gateway::id::{GrantRef, IdempotencyKey, JobRef, OperationId, TaskRef};
use synvelis_gateway::job::{CancelLink, CancelResponse, JobPhase};
use synvelis_gateway::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome, RetryAdvice};
use synvelis_gateway::request::Principal;

use crate::canon::{definition_digest, digest_of};
use crate::compiler::{ExecutablePlan, PLAN_SCHEMA_VERSION};
use crate::definition::{Step, WorkflowDefinition};
use crate::eval::{self, EvalEnv};
use crate::macros::{AtomicMacroDefinition, BATCH_CAPABILITY_ID, MacroCompiler};
use crate::refs::VersionedRef;
use crate::registry::{OperationDomain, RegistrySnapshot};
use crate::run::{
    self, JournalEntry, JournalKind, JournalRefs, RunError, RunHeader, RunId, RunJournal, RunState,
    StepFrame, StepPath, StepState, StepVerdict, WorkflowRun, entry_digest, replay_entries,
};
use crate::schema::validate_value;

/// The batch capability every compiled atomic macro lowers to
/// (`synvelis.operation.batch@1.0`).
fn batch_capability() -> VersionedRef {
    VersionedRef::new(BATCH_CAPABILITY_ID, 1, 0)
}

/// One authoritative capability invocation prepared by the coordinator.
///
/// The lane maps this onto the gateway request shape; every field is
/// data the workflow legitimately owns — never authority material.
#[derive(Clone, Debug)]
pub struct StepCall {
    /// Exact capability reference (`id@major.minor`).
    pub capability: VersionedRef,
    /// Evaluated argument object.
    pub args: Value,
    /// Evaluated explicit target binding, when the step declares one.
    pub target: Option<Value>,
    /// CAS expectation for mutation-domain calls — the revision the
    /// step asserted (journaled on `StepBegan`), `None` when unpinned.
    pub expected_revision: Option<u64>,
    /// Step-occurrence idempotency key — stable across crash-retry.
    pub idempotency_key: IdempotencyKey,
    /// Policy epoch the run observed at start.
    pub policy_epoch: Option<u64>,
    /// Run principal (identity reference).
    pub principal: Principal,
    /// Tagged project id.
    pub project_id: String,
    /// Task scope reference, when the run carries one.
    pub task: Option<TaskRef>,
    /// Grant scope reference, when the run carries one.
    pub grant: Option<GrantRef>,
}

/// What the authoritative side knows about a prior operation — the
/// reconcile read behind [`StepLane::operation_result`].
#[derive(Clone, Debug)]
pub enum OperationObservation {
    /// The operation settled with this recorded outcome.
    Settled(GatewayOutcome<Value>),
    /// The operation was admitted but has not settled yet.
    Pending,
    /// The authoritative side has no record — the operation's fate is
    /// unknown (provider restart / foreign id).
    Unknown,
}

/// The capability invocation seam.
///
/// Contract for implementors (the gateway adapter in production, the
/// fixture in tests):
///
/// - `invoke` is *idempotent on `idempotency_key`*: an identical
///   repeated call must surface the recorded outcome — as a replayed
///   [`GatewayOutcome`], or as [`GatewayError::DuplicateRequest`]
///   which the coordinator resolves through
///   [`Self::operation_result`]. It must never silently apply the same
///   key's side effect twice.
/// - Non-settled dispositions ([`GatewayOutcome::ApprovalRequired`],
///   [`GatewayOutcome::ProposalReady`]) are requests for *external*
///   trusted action; re-presenting them on re-drive is correct — the
///   workflow cannot and must not self-approve.
/// - `operation_result` reads the settled outcome for an
///   [`OperationId`] previously returned by `invoke` (or surfaced via
///   `DuplicateRequest.original`).
pub trait StepLane: Send + Sync {
    /// Invokes one capability call.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — typed gateway failure; the coordinator maps
    /// it onto step/run verdicts without guessing.
    fn invoke(&self, call: &StepCall) -> Result<GatewayOutcome<Value>, Box<ErrorEnvelope>>;

    /// Reads the recorded disposition of a prior operation.
    fn operation_result(&self, operation: &OperationId) -> OperationObservation;
}

/// What the job owner reports for one awaited handle.
#[derive(Clone, Debug)]
pub enum JobObservation {
    /// Live provider truth for the job behind `operation`.
    Live {
        /// Provider-confirmed phase (`Cancelled` only when confirmed).
        phase: JobPhase,
        /// Provider-reported progress, when any.
        progress: Option<u64>,
        /// The disclosed result payload on `Completed`.
        result: Option<Value>,
        /// Provider job ref, when disclosed.
        job: Option<JobRef>,
        /// The cancel-request linkage state.
        cancel: CancelLink,
    },
    /// The job owner has no record — lost, foreign, or never admitted.
    /// The coordinator must treat this as indeterminate, never as
    /// failure or success (F-5).
    Unknown,
}

/// The job status/cancel read seam — keyed by the [`OperationId`]
/// handle an `async-submit` step produced (the same handle `job.get` /
/// `job.cancel` consume on the wire).
pub trait JobLane: Send + Sync {
    /// Reads the authoritative job observation.
    fn observe(&self, operation: &OperationId) -> JobObservation;

    /// Issues a cancel request. The response is honest about what the
    /// provider could confirm — `Unreachable`/`Pending` are recorded,
    /// never upgraded (F-4).
    fn request_cancel(&self, operation: &OperationId) -> CancelResponse;
}

/// Per-drive context: the compiled artifacts the journal references by
/// digest. The journal stores digests and refs — the AST itself is
/// supplied by the caller (the plan store is an integration concern).
pub struct RunContext<'a> {
    /// The validated workflow definition the plan was compiled from.
    pub definition: &'a WorkflowDefinition,
    /// The compiled executable plan.
    pub plan: &'a ExecutablePlan,
    /// Provided workflow inputs by declared name.
    pub inputs: &'a serde_json::Map<String, Value>,
    /// Capability/schema snapshot the plan was compiled against.
    pub registry: &'a RegistrySnapshot,
    /// Atomic macro definitions referenced by `atomic-macro` steps.
    pub macros: &'a [AtomicMacroDefinition],
}

/// Per-run start parameters — identity and scope references only.
pub struct RunMeta {
    /// Caller-chosen run id.
    pub run_id: RunId,
    /// Caller idempotency key for this run start.
    pub idempotency_key: IdempotencyKey,
    /// The run principal.
    pub principal: Principal,
    /// Tagged project id.
    pub project_id: String,
    /// Task scope reference.
    pub task: Option<TaskRef>,
    /// Grant scope reference.
    pub grant: Option<GrantRef>,
    /// Policy epoch the caller observed — forwarded on every step call
    /// so the authoritative side revalidates against the live epoch
    /// (F-6).
    pub policy_epoch: Option<u64>,
    /// Base project revision pin, when the caller asserts one.
    pub expected_revision: Option<u64>,
}

/// The headless workflow coordinator.
///
/// Cheap to construct — all durable truth lives in the [`RunJournal`]
/// and the lanes, so a "restarted" coordinator is just a new instance
/// over the same `Arc`s (the connection-lifetime independence the
/// contract requires).
pub struct WorkflowCoordinator {
    journal: Arc<dyn RunJournal>,
    lane: Arc<dyn StepLane>,
    jobs: Arc<dyn JobLane>,
}

impl WorkflowCoordinator {
    /// Creates a coordinator over shared journal + lanes.
    #[must_use]
    pub fn new(
        journal: Arc<dyn RunJournal>,
        lane: Arc<dyn StepLane>,
        jobs: Arc<dyn JobLane>,
    ) -> Self {
        Self {
            journal,
            lane,
            jobs,
        }
    }

    /// Validates plan/definition identity, records the run header, and
    /// transitions `Created → Ready`.
    ///
    /// Plan idempotency: a second start with the same `run_id` and an
    /// identical fingerprint returns [`RunError::DuplicateRun`]; any
    /// differing field is [`RunError::IdempotencyConflict`]. Plan
    /// tampering — definition digest, schema version, or implicit
    /// target binding violations — is rejected before any journal
    /// write (F-10).
    ///
    /// # Errors
    /// [`RunError::PlanMismatch`], [`RunError::DuplicateRun`],
    /// [`RunError::IdempotencyConflict`], [`RunError::Journal`],
    /// [`RunError::Eval`] on input validation failure.
    pub fn start_run(&self, ctx: &RunContext<'_>, meta: &RunMeta) -> Result<RunId, RunError> {
        verify_plan_identity(ctx)?;
        let inputs_value = Value::Object(ctx.inputs.clone());
        let input_digest = digest_of(&inputs_value).map_err(|e| RunError::Journal {
            reason: format!("input digest failed: {e}"),
        })?;
        validate_inputs(ctx.definition, ctx.inputs)?;

        let header = RunHeader {
            run_id: meta.run_id.clone(),
            plan_digest: plan_digest(ctx.plan)?,
            definition_version: ctx.definition.version,
            project_id: meta.project_id.clone(),
            input_digest,
            principal: meta.principal.clone(),
            policy_epoch: meta.policy_epoch,
            expected_revision: meta.expected_revision,
            idempotency_key: meta.idempotency_key.clone(),
            task: meta.task.clone(),
            grant: meta.grant.clone(),
        };
        self.journal.create_run(&header)?;

        // Created -> Ready: plan identity verified, inputs validated.
        let mut session = Session::new(self, ctx, &meta.run_id)?;
        session.transition_run(RunState::Ready, Some("plan verified".to_owned()))?;
        Ok(meta.run_id.clone())
    }

    /// Advances the run until it blocks, fails, completes, or goes
    /// indeterminate. Idempotent: calling on a terminal run returns the
    /// recorded state; calling on a parked run re-observes authority.
    ///
    /// # Errors
    /// [`RunError::UnknownRun`], [`RunError::PlanMismatch`],
    /// [`RunError::Journal`], [`RunError::ReplayDivergence`].
    pub fn drive(&self, run_id: &RunId, ctx: &RunContext<'_>) -> Result<WorkflowRun, RunError> {
        let mut session = Session::new(self, ctx, run_id)?;
        session.run_loop()?;
        Ok(session.run)
    }

    /// Requests cancellation and settles in-flight authoritative work.
    ///
    /// For every pending job handle the coordinator issues one
    /// [`JobLane::request_cancel`] and journals the honest
    /// [`CancelResponse`] class — never resending on later drives
    /// (F-4). The run then observes job truth: provider-confirmed
    /// `Cancelled` fails the awaiting step; `Completed` evidence is
    /// journaled and the run completes only if no work remains
    /// (cancel lost the race); anything else keeps `CancelRequested`
    /// until evidence arrives.
    ///
    /// # Errors
    /// Same as [`Self::drive`].
    pub fn cancel(&self, run_id: &RunId, ctx: &RunContext<'_>) -> Result<WorkflowRun, RunError> {
        let mut session = Session::new(self, ctx, run_id)?;
        session.request_cancel()?;
        session.run_loop()?;
        Ok(session.run)
    }

    /// Reconciles an `Indeterminate` run against authoritative state.
    ///
    /// Re-enters traversal so the parked step re-issues its lane call
    /// under its *original* idempotency key (the authoritative side
    /// dedupes — the F-1/F-2/F-3 recovery path) or re-observes the
    /// awaited job. Only evidence resolves indeterminacy: a
    /// still-unknown observation leaves the run `Indeterminate`.
    /// Unknown state is never promoted to `Completed`/`Failed` without
    /// it.
    ///
    /// # Errors
    /// [`RunError::Lane`] when the run is not `Indeterminate`, plus the
    /// [`Self::drive`] set.
    pub fn reconcile(&self, run_id: &RunId, ctx: &RunContext<'_>) -> Result<WorkflowRun, RunError> {
        let mut session = Session::new(self, ctx, run_id)?;
        if session.run.state != RunState::Indeterminate {
            return Err(RunError::Lane {
                reason: format!(
                    "reconcile requires an indeterminate run, got {:?}",
                    session.run.state
                ),
            });
        }
        // Indeterminate -> Running (or back to CancelRequested when a
        // cancel was in flight) is the only evidence-free edge the FSM
        // allows; the traversal then re-observes authority before any
        // verdict.
        let target = if session.run.cancel_requested {
            RunState::CancelRequested
        } else {
            RunState::Running
        };
        session.transition_run(target, Some("reconcile re-observation".to_owned()))?;
        session.run_loop()?;
        Ok(session.run)
    }

    /// Read-only status re-query — rebuilds the projection without
    /// driving. Connection-independent: any caller holding `run_id` +
    /// context gets the same authoritative progress view.
    ///
    /// # Errors
    /// Same as [`Self::drive`].
    pub fn status(&self, run_id: &RunId, ctx: &RunContext<'_>) -> Result<WorkflowRun, RunError> {
        Session::new(self, ctx, run_id).map(|s| s.run)
    }

    /// The declared run outputs — only once the run `Completed`.
    ///
    /// # Errors
    /// [`RunError::NotCompleted`] while the run is non-terminal or
    /// failed; the [`Self::status`] set otherwise.
    pub fn result(
        &self,
        run_id: &RunId,
        ctx: &RunContext<'_>,
    ) -> Result<BTreeMap<String, Value>, RunError> {
        let run = self.status(run_id, ctx)?;
        if run.state != RunState::Completed {
            return Err(RunError::NotCompleted {
                run_id: run.run_id.clone(),
                state: run.state,
            });
        }
        Ok(run.outputs)
    }
}

// ---------------------------------------------------------------------------
// Plan identity + input validation (start/resume gates — F-10)
// ---------------------------------------------------------------------------

fn plan_digest(plan: &ExecutablePlan) -> Result<String, RunError> {
    plan.digest().map_err(|e| RunError::PlanMismatch {
        reason: format!("plan digest failed: {e}"),
    })
}

/// F-10 gate: the supplied definition+plan pair must be internally
/// consistent and carry the only supported plan shape. A digest or
/// version mismatch rejects start *and* resume before any state is
/// trusted.
fn verify_plan_identity(ctx: &RunContext<'_>) -> Result<(), RunError> {
    if ctx.plan.schema_version != PLAN_SCHEMA_VERSION {
        return Err(RunError::PlanMismatch {
            reason: format!(
                "unsupported plan schema_version {}",
                ctx.plan.schema_version
            ),
        });
    }
    if !ctx.plan.explicit_target_binding {
        return Err(RunError::PlanMismatch {
            reason: "plan does not require explicit target binding".to_owned(),
        });
    }
    let actual = definition_digest(ctx.definition).map_err(|e| RunError::PlanMismatch {
        reason: format!("definition digest failed: {e}"),
    })?;
    if actual != ctx.plan.definition_digest {
        return Err(RunError::PlanMismatch {
            reason: format!(
                "definition digest {actual} != plan.definition_digest {}",
                ctx.plan.definition_digest
            ),
        });
    }
    Ok(())
}

/// Validates provided inputs against declared input schemas — exact
/// name matching, missing and unexpected inputs both rejected.
fn validate_inputs(
    definition: &WorkflowDefinition,
    inputs: &serde_json::Map<String, Value>,
) -> Result<(), RunError> {
    for decl in &definition.inputs {
        let value = inputs.get(&decl.name).ok_or_else(|| {
            RunError::Eval(eval::EvalError::MissingInput {
                name: decl.name.clone(),
            })
        })?;
        validate_value(&decl.schema, &decl.name, value).map_err(|_| {
            RunError::Eval(eval::EvalError::WrongType {
                path: decl.name.clone(),
                expected: "declared input schema",
            })
        })?;
    }
    for name in inputs.keys() {
        if !definition.inputs.iter().any(|d| &d.name == name) {
            return Err(RunError::Eval(eval::EvalError::MissingInput {
                name: format!("unexpected input: {name}"),
            }));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Drive session
// ---------------------------------------------------------------------------

/// How a step-list pass ended.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Flow {
    /// Every occurrence in the list settled or was skipped.
    Continue,
    /// The run left the traversable state — parked, failed,
    /// indeterminate, or cancelled.
    Blocked,
}

/// One drive/cancel/reconcile invocation's working state: the replayed
/// run projection plus journal append sequencing. All mutations go
/// through [`Session::journal`], which appends then applies the same
/// entry — journal and projection can never diverge.
struct Session<'a> {
    coord: &'a WorkflowCoordinator,
    ctx: &'a RunContext<'a>,
    run: WorkflowRun,
    next_seq: u64,
    emit_outputs: BTreeMap<String, String>,
    started: Instant,
}

impl<'a> Session<'a> {
    fn new(
        coord: &'a WorkflowCoordinator,
        ctx: &'a RunContext<'a>,
        run_id: &RunId,
    ) -> Result<Self, RunError> {
        let digest = plan_digest(ctx.plan)?;
        let header = coord
            .journal
            .run_header(run_id)?
            .ok_or_else(|| RunError::UnknownRun {
                run_id: run_id.clone(),
            })?;
        if header.plan_digest != digest {
            return Err(RunError::PlanMismatch {
                reason: format!(
                    "journal plan {} != supplied plan {digest}",
                    header.plan_digest
                ),
            });
        }
        let entries = coord.journal.entries(run_id)?;
        let next_seq = entries.len() as u64;
        let run = replay_entries(&header, &entries, ctx.definition)?;
        let emit_outputs = ctx
            .definition
            .steps
            .iter()
            .filter_map(|s| {
                if let Step::EmitResult { output, .. } = s {
                    Some((s.id().to_owned(), output.clone()))
                } else {
                    None
                }
            })
            .collect();
        Ok(Self {
            coord,
            ctx,
            run,
            next_seq,
            emit_outputs,
            started: Instant::now(),
        })
    }

    /// Appends one journal entry and applies it to the projection.
    fn journal(&mut self, mut entry: JournalEntry) -> Result<(), RunError> {
        entry.seq = self.next_seq;
        entry.digest = entry_digest(&entry).map_err(|e| RunError::Journal {
            reason: format!("entry digest failed: {e}"),
        })?;
        self.coord.journal.append(entry.clone())?;
        run::apply_entry(
            &mut self.run,
            &entry,
            &self.emit_outputs,
            self.ctx.definition,
        )?;
        self.next_seq += 1;
        Ok(())
    }

    /// Journals a run-level transition.
    fn transition_run(&mut self, to: RunState, detail: Option<String>) -> Result<(), RunError> {
        let from = self.run.state;
        self.journal(JournalEntry {
            run_id: self.run.run_id.clone(),
            seq: 0,
            step: None,
            plan_revision: self.run.plan_digest.clone(),
            capability_ref: None,
            kind: if to.terminal() {
                JournalKind::RunSettled
            } else {
                JournalKind::RunTransition
            },
            verdict: None,
            from_state: Some(from),
            to_state: Some(to),
            refs: JournalRefs::default(),
            idempotency_key: None,
            expected_revision: None,
            digest: String::new(),
            output: None,
            detail,
            retry_advice: None,
        })
    }

    /// The main drive loop — advances until the run leaves the
    /// traversable states or reaches a terminal/parked state.
    fn run_loop(&mut self) -> Result<(), RunError> {
        loop {
            match self.run.state {
                RunState::Created => self.transition_run(RunState::Ready, None)?,
                RunState::Ready => self.transition_run(RunState::Running, None)?,
                RunState::Running | RunState::Waiting | RunState::Approval => {
                    // Parked runs re-enter traversal: settled steps skip
                    // and the parked occurrence re-observes authority in
                    // context. A resolved park promotes back to Running
                    // inside the step pass.
                    let steps: &'a [Step] = &self.ctx.definition.steps;
                    let mut path = StepPath::new();
                    let flow = self.exec_steps(steps, &mut path, None, None)?;
                    if flow == Flow::Continue {
                        self.transition_run(
                            RunState::Completed,
                            Some("all steps settled".to_owned()),
                        )?;
                    }
                    return Ok(());
                }
                RunState::CancelRequested => {
                    self.settle_cancel()?;
                    return Ok(());
                }
                RunState::Indeterminate | RunState::Completed | RunState::Failed => {
                    return Ok(());
                }
            }
        }
    }

    /// Runtime budget gate. On exceedance the run is *failed in the
    /// journal* (terminal evidence) and the pass blocks — the budget
    /// breach is run truth, not a caller error.
    fn budget_exceeded(&mut self) -> Result<bool, RunError> {
        let reason = if self.run.executed_steps >= self.ctx.plan.max_steps {
            Some("max-steps: step execution budget exhausted")
        } else if self.run.loop_iterations >= self.ctx.plan.max_iterations {
            Some("max-iterations: loop iteration budget exhausted")
        } else if u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
            > self.ctx.plan.max_runtime
        {
            Some("max-runtime: runtime budget exhausted")
        } else {
            None
        };
        if let Some(reason) = reason {
            self.transition_run(RunState::Failed, Some(reason.to_owned()))?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Executes a step list in order, one boundary at a time.
    fn exec_steps(
        &mut self,
        steps: &[Step],
        path: &mut StepPath,
        iteration: Option<u64>,
        item: Option<&Value>,
    ) -> Result<Flow, RunError> {
        for step in steps {
            // Waiting/Approval pass the gate: traversal must reach the
            // parked occurrence to re-observe it. The run promotes
            // back to Running only when that occurrence resolves —
            // never before.
            if self.run.state.terminal()
                || matches!(
                    self.run.state,
                    RunState::Indeterminate | RunState::CancelRequested
                )
            {
                return Ok(Flow::Blocked);
            }
            if self.budget_exceeded()? {
                return Ok(Flow::Blocked);
            }
            path.push(StepFrame {
                step_id: step.id().to_owned(),
                iteration,
            });
            let flow = match step {
                Step::If {
                    condition,
                    then,
                    else_steps,
                    ..
                } => self.exec_if(condition, then, else_steps, path, iteration, item),
                Step::ForEachBounded {
                    items: items_expr,
                    max_iterations,
                    body,
                    ..
                } => self.exec_loop(items_expr, *max_iterations, body, path, item),
                _ => self.exec_leaf(step, path, item),
            };
            path.pop();
            match flow? {
                Flow::Continue => {}
                Flow::Blocked => return Ok(Flow::Blocked),
            }
        }
        Ok(Flow::Continue)
    }

    /// `if` control step: journal begin once, evaluate the condition
    /// deterministically, execute the taken branch, then settle.
    fn exec_if(
        &mut self,
        condition: &crate::expr::Condition,
        then: &[Step],
        else_steps: &[Step],
        path: &mut StepPath,
        iteration: Option<u64>,
        item: Option<&Value>,
    ) -> Result<Flow, RunError> {
        let full = path.clone();
        match self.checkpoint_state(&full) {
            Some(StepState::Completed) => return Ok(Flow::Continue),
            Some(StepState::Failed) => return Ok(Flow::Blocked),
            _ => {}
        }
        if self.run.checkpoint(&full).is_none() {
            self.journal_step_begin(&full, None, None, None)?;
        }
        let cond = {
            let env = self.eval_env(item);
            eval::eval_condition(condition, &env)
        };
        let cond = match cond {
            Ok(c) => c,
            Err(e) => {
                self.settle_step_failed(&full, &e.to_string(), None)?;
                return Ok(Flow::Blocked);
            }
        };
        let branch: &[Step] = if cond { then } else { else_steps };
        let flow = self.exec_steps(branch, path, iteration, item)?;
        if flow == Flow::Blocked {
            return Ok(Flow::Blocked);
        }
        self.journal(JournalEntry {
            run_id: self.run.run_id.clone(),
            seq: 0,
            step: Some(full.clone()),
            plan_revision: self.run.plan_digest.clone(),
            capability_ref: None,
            kind: JournalKind::StepSettled,
            verdict: Some(StepVerdict::Completed),
            from_state: None,
            to_state: None,
            refs: JournalRefs::default(),
            idempotency_key: None,
            expected_revision: None,
            digest: String::new(),
            output: None,
            detail: Some("branch settled".to_owned()),
            retry_advice: None,
        })?;
        Ok(Flow::Continue)
    }

    /// `for-each-bounded`: evaluate items, enforce the declared
    /// iteration bound fail-closed, execute the body per item.
    fn exec_loop(
        &mut self,
        items_expr: &crate::expr::Expr,
        max_iterations: u64,
        body: &[Step],
        path: &mut StepPath,
        item: Option<&Value>,
    ) -> Result<Flow, RunError> {
        let full = path.clone();
        match self.checkpoint_state(&full) {
            Some(StepState::Completed) => return Ok(Flow::Continue),
            Some(StepState::Failed) => return Ok(Flow::Blocked),
            _ => {}
        }
        if self.run.checkpoint(&full).is_none() {
            self.journal_step_begin(&full, None, None, None)?;
        }
        let items = {
            let env = self.eval_env(item);
            eval::eval_expr(items_expr, &env)
        };
        let items = match items {
            Ok(Value::Array(arr)) => arr,
            Ok(_) => {
                self.settle_step_failed(&full, "for-each items must evaluate to an array", None)?;
                return Ok(Flow::Blocked);
            }
            Err(e) => {
                self.settle_step_failed(&full, &e.to_string(), None)?;
                return Ok(Flow::Blocked);
            }
        };
        if items.len() as u64 > max_iterations {
            self.settle_step_failed(
                &full,
                &format!(
                    "items length {} exceeds for-each bound {max_iterations}",
                    items.len()
                ),
                Some(RetryAdvice::AfterCorrection),
            )?;
            return Ok(Flow::Blocked);
        }
        for (i, element) in items.iter().enumerate() {
            if self.budget_exceeded()? {
                return Ok(Flow::Blocked);
            }
            if self.run.state.terminal()
                || matches!(
                    self.run.state,
                    RunState::Indeterminate | RunState::CancelRequested
                )
            {
                return Ok(Flow::Blocked);
            }
            let flow = self.exec_steps(body, path, Some(i as u64), Some(element))?;
            if flow == Flow::Blocked {
                return Ok(Flow::Blocked);
            }
        }
        self.journal(JournalEntry {
            run_id: self.run.run_id.clone(),
            seq: 0,
            step: Some(full.clone()),
            plan_revision: self.run.plan_digest.clone(),
            capability_ref: None,
            kind: JournalKind::StepSettled,
            verdict: Some(StepVerdict::Completed),
            from_state: None,
            to_state: None,
            refs: JournalRefs::default(),
            idempotency_key: None,
            expected_revision: None,
            digest: String::new(),
            output: None,
            detail: Some("loop settled".to_owned()),
            retry_advice: None,
        })?;
        Ok(Flow::Continue)
    }

    /// Executes (or resumes) one leaf step occurrence.
    fn exec_leaf(
        &mut self,
        step: &Step,
        path: &mut StepPath,
        item: Option<&Value>,
    ) -> Result<Flow, RunError> {
        let full = path.clone();
        match self.checkpoint_state(&full) {
            Some(StepState::Completed) => return Ok(Flow::Continue),
            Some(StepState::Failed) => return Ok(Flow::Blocked),
            _ => {}
        }
        match step {
            Step::AwaitJob {
                job, output_schema, ..
            } => self.exec_await(&full, job, output_schema.as_ref(), item),
            Step::EmitResult { output, value, .. } => self.exec_emit(&full, output, value, item),
            Step::Query { .. }
            | Step::Invoke { .. }
            | Step::AsyncSubmit { .. }
            | Step::AtomicMacro { .. } => self.exec_call(step, &full, item),
            _ => unreachable!("control steps handled by exec_steps"),
        }
    }
}

impl<'a> Session<'a> {
    /// The capability + asserted revision a lane step would invoke —
    /// resolved *without* evaluating args so `StepBegan` can be
    /// journaled before any evaluation can fail.
    fn lane_identity(&self, step: &Step) -> Option<(VersionedRef, Option<u64>)> {
        match step {
            Step::Query { capability, .. }
            | Step::Invoke { capability, .. }
            | Step::AsyncSubmit { capability, .. } => {
                Some((capability.clone(), self.expected_revision_for(capability)))
            }
            Step::AtomicMacro { .. } => Some((
                batch_capability(),
                // The batch capability is mutation-domain; the macro
                // asserts the run's latest known revision.
                self.run.expected_revision,
            )),
            _ => None,
        }
    }

    /// Executes (or resumes) one capability leaf step.
    ///
    /// Ordering invariant: `StepBegan` is journaled *before* args are
    /// evaluated, so an evaluation failure still leaves an honest
    /// began+failed pair rather than a settle without a begin.
    fn exec_call(
        &mut self,
        step: &Step,
        path: &StepPath,
        item: Option<&Value>,
    ) -> Result<Flow, RunError> {
        let Some((capability, fresh_expected)) = self.lane_identity(step) else {
            return Ok(Flow::Blocked);
        };
        // A resumed occurrence re-asserts the revision journaled on its
        // StepBegan — never a silently newer one.
        let expected_revision = self
            .run
            .checkpoint(path)
            .and_then(|c| c.expected_revision)
            .or(fresh_expected);

        // Admission: fresh occurrences get a deterministic key journaled
        // on StepBegan; resumed ones reuse the journaled key — the
        // authoritative side dedupes (F-1/F-2).
        let key = if let Some(k) = self
            .run
            .checkpoint(path)
            .and_then(|c| c.idempotency_key.clone())
        {
            k
        } else {
            let key = self.step_key(path)?;
            self.journal_step_begin(
                path,
                Some(capability.to_string()),
                expected_revision,
                Some(key.clone()),
            )?;
            key
        };

        // Argument/target evaluation + macro lowering.
        let prepared = self.prepare_call(step, item, expected_revision);
        let (args, target) = match prepared {
            Ok(p) => p,
            Err(detail) => {
                self.settle_step_failed(path, &detail, None)?;
                return Ok(Flow::Blocked);
            }
        };

        let call = StepCall {
            capability: capability.clone(),
            args,
            target,
            expected_revision,
            idempotency_key: key,
            policy_epoch: self.run.policy_epoch,
            principal: self.run.principal.clone(),
            project_id: self.run.project_id.clone(),
            task: self.run.task.clone(),
            grant: self.run.grant.clone(),
        };

        match self.coord.lane.invoke(&call) {
            Ok(outcome) => self.settle_outcome(path, &call.capability, outcome, step),
            Err(env) => self.settle_lane_error(path, &env, step, &call.capability),
        }
    }

    /// Evaluates step args/target and lowers atomic macros to the batch
    /// capability. Returns `(args, target)`; the capability itself was
    /// already resolved by [`Self::lane_identity`].
    fn prepare_call(
        &self,
        step: &Step,
        item: Option<&Value>,
        expected_revision: Option<u64>,
    ) -> Result<(Value, Option<Value>), String> {
        let env = self.eval_env(item);
        match step {
            Step::Query { args, target, .. }
            | Step::Invoke { args, target, .. }
            | Step::AsyncSubmit { args, target, .. } => {
                let args = eval::eval_expr(args, &env).map_err(|e| e.to_string())?;
                let target = match target {
                    Some(t) => Some(eval::eval_expr(t, &env).map_err(|e| e.to_string())?),
                    None => None,
                };
                Ok((args, target))
            }
            Step::AtomicMacro {
                mac, args, target, ..
            } => {
                let def = self
                    .ctx
                    .macros
                    .iter()
                    .find(|d| d.macro_ref() == *mac)
                    .ok_or_else(|| format!("macro definition unavailable: {mac}"))?;
                let args_value = eval::eval_expr(args, &env).map_err(|e| e.to_string())?;
                let Value::Object(param_inputs) = args_value else {
                    return Err("atomic-macro args must evaluate to an object".to_owned());
                };
                let compiled = MacroCompiler::compile(
                    def,
                    &param_inputs,
                    self.ctx.registry,
                    expected_revision,
                )
                .map_err(|e| format!("macro compile failed: {e}"))?;
                let batch_args = compiled
                    .batch_args()
                    .map_err(|e| format!("macro batch args failed: {e}"))?;
                let target = match target {
                    Some(t) => Some(eval::eval_expr(t, &env).map_err(|e| e.to_string())?),
                    None => None,
                };
                Ok((batch_args, target))
            }
            _ => Err("not a capability step".to_owned()),
        }
    }

    /// The CAS expectation a *fresh* capability call asserts:
    /// mutation/proposal domains carry the run's latest known revision;
    /// read/async domains carry none. Atomic macros always assert.
    fn expected_revision_for(&self, capability: &VersionedRef) -> Option<u64> {
        let domain = self
            .ctx
            .registry
            .resolve_capability(capability)
            .ok()
            .map(|d| d.domain);
        match domain {
            Some(OperationDomain::ProjectMutation | OperationDomain::Proposal) => {
                self.run.expected_revision
            }
            _ => None,
        }
    }

    /// Awaits a job handle: evaluates the handle expression, observes
    /// the job owner, and settles or parks honestly.
    fn exec_await(
        &mut self,
        path: &StepPath,
        job_expr: &crate::expr::Expr,
        output_schema: Option<&VersionedRef>,
        item: Option<&Value>,
    ) -> Result<Flow, RunError> {
        let full = path.clone();
        if self.run.checkpoint(&full).is_none() {
            self.journal_step_begin(&full, None, None, None)?;
        }
        let handle = {
            let env = self.eval_env(item);
            eval::eval_expr(job_expr, &env)
        };
        let handle = match handle {
            Ok(Value::String(s)) => s,
            Ok(_) => {
                self.settle_step_failed(&full, "await-job handle must be a string", None)?;
                return Ok(Flow::Blocked);
            }
            Err(e) => {
                self.settle_step_failed(&full, &e.to_string(), None)?;
                return Ok(Flow::Blocked);
            }
        };
        let operation = match run::parse_operation_id(&handle) {
            Ok(op) => op,
            Err(e) => {
                self.settle_step_failed(&full, &e.to_string(), None)?;
                return Ok(Flow::Blocked);
            }
        };
        self.observe_job(&full, &operation, output_schema, None)
    }

    /// Emits a declared output: evaluates, schema-validates, journals.
    fn exec_emit(
        &mut self,
        path: &StepPath,
        output_name: &str,
        value_expr: &crate::expr::Expr,
        item: Option<&Value>,
    ) -> Result<Flow, RunError> {
        let full = path.clone();
        if self.run.checkpoint(&full).is_none() {
            self.journal_step_begin(&full, None, None, None)?;
        }
        let decl = self
            .ctx
            .definition
            .outputs
            .iter()
            .find(|o| o.name == output_name);
        let Some(decl) = decl else {
            self.settle_step_failed(&full, &format!("undeclared output: {output_name}"), None)?;
            return Ok(Flow::Blocked);
        };
        let value = {
            let env = self.eval_env(item);
            eval::eval_expr(value_expr, &env)
        };
        let value = match value {
            Ok(v) => v,
            Err(e) => {
                self.settle_step_failed(&full, &e.to_string(), None)?;
                return Ok(Flow::Blocked);
            }
        };
        if let Err(e) = validate_value(&decl.schema, output_name, &value) {
            self.settle_step_failed(&full, &format!("output schema violation: {e}"), None)?;
            return Ok(Flow::Blocked);
        }
        if self.output_too_large(&value) {
            self.settle_step_failed(
                &full,
                "emit output exceeds max_output_bytes",
                Some(RetryAdvice::AfterCorrection),
            )?;
            return Ok(Flow::Blocked);
        }
        self.journal(JournalEntry {
            run_id: self.run.run_id.clone(),
            seq: 0,
            step: Some(full.clone()),
            plan_revision: self.run.plan_digest.clone(),
            capability_ref: None,
            kind: JournalKind::StepSettled,
            verdict: Some(StepVerdict::Completed),
            from_state: None,
            to_state: None,
            refs: JournalRefs::default(),
            idempotency_key: None,
            expected_revision: None,
            digest: String::new(),
            output: Some(value),
            detail: None,
            retry_advice: None,
        })?;
        Ok(Flow::Continue)
    }

    /// Observes the job behind `operation` and maps the phase onto the
    /// awaiting step's verdict — the F-2/F-3/F-5 honest path.
    fn observe_job(
        &mut self,
        path: &StepPath,
        operation: &OperationId,
        output_schema: Option<&VersionedRef>,
        detail_prefix: Option<&str>,
    ) -> Result<Flow, RunError> {
        match self.coord.jobs.observe(operation) {
            JobObservation::Unknown => {
                // F-5: the job owner has no record — indeterminate,
                // never failed, never succeeded.
                self.settle_step(
                    path,
                    StepVerdict::Indeterminate,
                    JournalRefs {
                        operation: Some(*operation),
                        ..JournalRefs::default()
                    },
                    None,
                    Some("job observation unknown — reconcile required".to_owned()),
                    None,
                )?;
                self.run_to(RunState::Indeterminate)?;
                Ok(Flow::Blocked)
            }
            JobObservation::Live {
                phase,
                progress,
                result,
                job,
                cancel,
            } => match phase {
                JobPhase::Pending | JobPhase::Running => {
                    let detail = format!(
                        "job {} phase {} progress {:?} cancel {}{}",
                        operation.to_tagged(),
                        phase.name(),
                        progress,
                        cancel.name(),
                        detail_prefix.map(|p| format!(" ({p})")).unwrap_or_default(),
                    );
                    // settle_step deduplicates repeat observations of
                    // the same class — re-drives do not spam the journal.
                    self.settle_step(
                        path,
                        StepVerdict::Waiting,
                        JournalRefs {
                            operation: Some(*operation),
                            job: job.clone(),
                            ..JournalRefs::default()
                        },
                        None,
                        Some(detail),
                        None,
                    )?;
                    self.run_to(RunState::Waiting)?;
                    Ok(Flow::Blocked)
                }
                JobPhase::Completed => self.settle_completed_job(
                    path,
                    operation,
                    result,
                    job,
                    output_schema,
                    detail_prefix,
                ),
                JobPhase::Failed { reason } => {
                    self.settle_step_failed(
                        path,
                        &format!("job failed: {reason}"),
                        Some(RetryAdvice::Never),
                    )?;
                    Ok(Flow::Blocked)
                }
                JobPhase::Cancelled => {
                    self.settle_step_failed(path, "job cancelled", Some(RetryAdvice::Never))?;
                    Ok(Flow::Blocked)
                }
            },
        }
    }

    /// Settles an awaiting step whose job reported `Completed` —
    /// output-size and declared-schema gates run before the verdict is
    /// journaled, exactly like every other settled output.
    fn settle_completed_job(
        &mut self,
        path: &StepPath,
        operation: &OperationId,
        result: Option<Value>,
        job: Option<JobRef>,
        output_schema: Option<&VersionedRef>,
        detail_prefix: Option<&str>,
    ) -> Result<Flow, RunError> {
        let value = result.unwrap_or_else(|| {
            serde_json::json!({
                "operation": operation.to_tagged(),
                "phase": "completed",
            })
        });
        if self.output_too_large(&value) {
            self.settle_step_failed(
                path,
                "job result exceeds max_output_bytes",
                Some(RetryAdvice::AfterCorrection),
            )?;
            return Ok(Flow::Blocked);
        }
        if let Some(schema_ref) = output_schema {
            match self.ctx.registry.resolve_schema(schema_ref) {
                Ok(schema) => {
                    if let Err(e) = validate_value(schema, "await-job result", &value) {
                        self.settle_step_failed(
                            path,
                            &format!("job result schema violation: {e}"),
                            None,
                        )?;
                        return Ok(Flow::Blocked);
                    }
                }
                Err(e) => {
                    self.settle_step_failed(
                        path,
                        &format!("output schema unresolvable: {e}"),
                        None,
                    )?;
                    return Ok(Flow::Blocked);
                }
            }
        }
        self.settle_step(
            path,
            StepVerdict::Completed,
            JournalRefs {
                operation: Some(*operation),
                job,
                ..JournalRefs::default()
            },
            Some(value),
            detail_prefix.map(str::to_owned),
            None,
        )?;
        // The parked occurrence resolved — promote the run back to
        // Running before any further work.
        self.run_to(RunState::Running)?;
        Ok(Flow::Continue)
    }

    /// Maps a lane outcome onto the step verdict — the single place
    /// `GatewayOutcome` becomes workflow truth.
    fn settle_outcome(
        &mut self,
        path: &StepPath,
        capability: &VersionedRef,
        outcome: GatewayOutcome<Value>,
        step: &Step,
    ) -> Result<Flow, RunError> {
        match outcome {
            GatewayOutcome::Completed { result } => {
                if matches!(step, Step::AsyncSubmit { .. }) {
                    self.settle_step_failed(
                        path,
                        "async-submit returned Completed — expected Accepted",
                        Some(RetryAdvice::Never),
                    )?;
                    return Ok(Flow::Blocked);
                }
                if self.output_too_large(&result) {
                    self.settle_step_failed(
                        path,
                        "step output exceeds max_output_bytes",
                        Some(RetryAdvice::AfterCorrection),
                    )?;
                    return Ok(Flow::Blocked);
                }
                self.settle_step(
                    path,
                    StepVerdict::Completed,
                    JournalRefs::default(),
                    Some(result),
                    Some(format!("completed via {capability}")),
                    None,
                )?;
                self.run_to(RunState::Running)?;
                Ok(Flow::Continue)
            }
            GatewayOutcome::Accepted { operation } => {
                if !matches!(step, Step::AsyncSubmit { .. }) {
                    self.settle_step_failed(
                        path,
                        "sync capability returned Accepted",
                        Some(RetryAdvice::Never),
                    )?;
                    return Ok(Flow::Blocked);
                }
                let handle = Value::String(operation.to_tagged());
                self.settle_step(
                    path,
                    StepVerdict::Completed,
                    JournalRefs {
                        operation: Some(operation),
                        ..JournalRefs::default()
                    },
                    Some(handle),
                    Some(format!("accepted via {capability}")),
                    None,
                )?;
                self.run_to(RunState::Running)?;
                Ok(Flow::Continue)
            }
            GatewayOutcome::ProposalReady { proposal } => {
                self.observe_step(
                    path,
                    StepVerdict::Approval,
                    JournalRefs {
                        proposal: Some(proposal),
                        ..JournalRefs::default()
                    },
                    Some("proposal prepared — awaiting trusted approval/apply".to_owned()),
                )?;
                self.run_to(RunState::Approval)?;
                Ok(Flow::Blocked)
            }
            GatewayOutcome::ApprovalRequired { approval } => {
                self.observe_step(
                    path,
                    StepVerdict::Approval,
                    JournalRefs {
                        approval: Some(approval),
                        ..JournalRefs::default()
                    },
                    Some("approval required — awaiting trusted approval".to_owned()),
                )?;
                self.run_to(RunState::Approval)?;
                Ok(Flow::Blocked)
            }
        }
    }

    /// Maps a lane error envelope onto the step verdict.
    fn settle_lane_error(
        &mut self,
        path: &StepPath,
        env: &ErrorEnvelope,
        step: &Step,
        capability: &VersionedRef,
    ) -> Result<Flow, RunError> {
        match &env.code {
            GatewayError::DuplicateRequest { original } => {
                // Idempotent replay: the key was already admitted —
                // reconcile through the recorded operation, never
                // re-execute (F-1/F-2).
                match self.coord.lane.operation_result(original) {
                    OperationObservation::Settled(outcome) => {
                        self.settle_outcome(path, capability, outcome, step)
                    }
                    OperationObservation::Pending | OperationObservation::Unknown => {
                        self.settle_step(
                            path,
                            StepVerdict::Indeterminate,
                            JournalRefs {
                                operation: Some(*original),
                                ..JournalRefs::default()
                            },
                            None,
                            Some(format!(
                                "duplicate request; original {} unresolved",
                                original.to_tagged()
                            )),
                            Some(env.retry),
                        )?;
                        self.run_to(RunState::Indeterminate)?;
                        Ok(Flow::Blocked)
                    }
                }
            }
            GatewayError::Indeterminate { .. } => {
                self.settle_indeterminate(
                    path,
                    None,
                    &format!("lane indeterminate: {}", env.message),
                    env.retry,
                )?;
                Ok(Flow::Blocked)
            }
            GatewayError::RevisionConflict { .. } => {
                // F-8: the pinned base is stale — the plan must be
                // recompiled/reprepared; the run fails with correction
                // advice rather than guessing a new revision.
                self.settle_step_failed_retry(
                    path,
                    &format!("revision conflict: {}", env.message),
                    Some(RetryAdvice::AfterCorrection),
                )?;
                Ok(Flow::Blocked)
            }
            _ if env.retry == RetryAdvice::ReconcileState => {
                // Consistency-state failures (expired snapshot/cursor,
                // dead proposal, …) may resolve on reconcile — the
                // observation is unknown, not failed.
                self.settle_indeterminate(
                    path,
                    None,
                    &format!("reconcile-state error: {}", env.message),
                    env.retry,
                )?;
                Ok(Flow::Blocked)
            }
            _ => {
                self.settle_step_failed_retry(path, &env.message, Some(env.retry))?;
                Ok(Flow::Blocked)
            }
        }
    }

    /// Journals an `Indeterminate` step verdict and parks the run.
    fn settle_indeterminate(
        &mut self,
        path: &StepPath,
        refs: Option<JournalRefs>,
        detail: &str,
        retry: RetryAdvice,
    ) -> Result<(), RunError> {
        self.settle_step(
            path,
            StepVerdict::Indeterminate,
            refs.unwrap_or_default(),
            None,
            Some(bound_detail(detail)),
            Some(retry),
        )?;
        self.run_to(RunState::Indeterminate)
    }

    // -- cancellation ----------------------------------------------------

    /// Cancel-request propagation: transition to `CancelRequested` and
    /// issue one honest cancel request per pending job handle — never
    /// resending on later drives (F-4).
    fn request_cancel(&mut self) -> Result<(), RunError> {
        if self.run.state.terminal() {
            return Ok(());
        }
        if self.run.state != RunState::CancelRequested {
            self.transition_run(
                RunState::CancelRequested,
                Some("cancel requested".to_owned()),
            )?;
        }
        let pending = self.run.pending_jobs.clone();
        for op in pending {
            if self.run.cancel_issued.contains(&op) {
                continue;
            }
            let response = self.coord.jobs.request_cancel(&op);
            let class = match &response {
                CancelResponse::Confirmed => "confirmed",
                CancelResponse::Pending => "pending",
                CancelResponse::Unreachable => "unreachable",
                CancelResponse::Settled { phase } => match phase {
                    // The settle race went to the job — record the
                    // actual phase as evidence, never "cancelled".
                    JobPhase::Completed => "settled:completed",
                    JobPhase::Failed { .. } => "settled:failed",
                    JobPhase::Cancelled => "settled:cancelled",
                    JobPhase::Pending => "settled:pending",
                    JobPhase::Running => "settled:running",
                },
            };
            self.journal(JournalEntry {
                run_id: self.run.run_id.clone(),
                seq: 0,
                step: None,
                plan_revision: self.run.plan_digest.clone(),
                capability_ref: None,
                kind: JournalKind::CancelIssued,
                verdict: None,
                from_state: None,
                to_state: None,
                refs: JournalRefs {
                    operation: Some(op),
                    ..JournalRefs::default()
                },
                idempotency_key: None,
                expected_revision: None,
                digest: String::new(),
                output: None,
                detail: Some(format!("cancel response: {class}")),
                retry_advice: None,
            })?;
        }
        // Mark every job-parked step occurrence `CancelRequested` —
        // the cancel reached the step; only provider truth decides the
        // final verdict. Dedup makes repeat cancels silent.
        let parked: Vec<(StepPath, JournalRefs)> = self
            .run
            .steps
            .iter()
            .filter(|c| c.state == StepState::Waiting)
            .map(|c| {
                (
                    c.path.clone(),
                    JournalRefs {
                        operation: c.operation_id,
                        job: c.job_ref.clone(),
                        ..JournalRefs::default()
                    },
                )
            })
            .collect();
        for (path, refs) in parked {
            self.settle_step(
                &path,
                StepVerdict::CancelRequested,
                refs,
                None,
                Some("run cancel reached this in-flight step".to_owned()),
                None,
            )?;
        }
        Ok(())
    }

    /// Settles a `CancelRequested` run: observe each pending job's
    /// truth, record evidence on its awaiting step, then resolve to
    /// `Completed` (work actually finished — cancel lost the race) or
    /// `Failed` (cancelled).
    fn settle_cancel(&mut self) -> Result<(), RunError> {
        let pending = self.run.pending_jobs.clone();
        let mut all_terminal = true;
        for op in &pending {
            match self.coord.jobs.observe(op) {
                JobObservation::Unknown => {
                    // Job truth lost mid-cancel — indeterminate, never
                    // guess (F-5 during cancel).
                    self.run_to(RunState::Indeterminate)?;
                    return Ok(());
                }
                JobObservation::Live {
                    phase, result, job, ..
                } => match phase {
                    JobPhase::Pending | JobPhase::Running => {
                        all_terminal = false;
                    }
                    JobPhase::Completed => {
                        self.settle_await_for_op(op, StepVerdict::Completed, result, job)?;
                    }
                    JobPhase::Failed { .. } | JobPhase::Cancelled => {
                        self.settle_await_for_op(op, StepVerdict::Failed, None, job)?;
                    }
                },
            }
        }
        if !all_terminal {
            // Still in flight — stay CancelRequested; later drives keep
            // observing (no cancel resend — F-4).
            return Ok(());
        }
        // All authoritative work settled. `Completed` only when nothing
        // remains — the honest cancel-lost-the-race case.
        if self.has_pending_work()? {
            self.transition_run(RunState::Failed, Some("cancelled".to_owned()))?;
        } else {
            self.transition_run(
                RunState::Completed,
                Some("cancel lost the race — work settled".to_owned()),
            )?;
        }
        Ok(())
    }

    /// Settles the await-job checkpoint bound to `op` with `verdict`,
    /// validating a completed result against the step's declared
    /// output schema exactly like the live observe path.
    fn settle_await_for_op(
        &mut self,
        op: &OperationId,
        verdict: StepVerdict,
        result: Option<Value>,
        job: Option<JobRef>,
    ) -> Result<(), RunError> {
        let found = self
            .run
            .steps
            .iter()
            .find(|c| {
                c.operation_id.as_ref() == Some(op)
                    && matches!(
                        c.state,
                        StepState::Waiting | StepState::CancelRequested | StepState::Running
                    )
            })
            .map(|c| (c.path.clone(), c.step_id.clone()));
        let Some((path, step_id)) = found else {
            return Ok(());
        };
        let mut output = None;
        if verdict == StepVerdict::Completed {
            let value = result.unwrap_or_else(|| {
                serde_json::json!({
                    "operation": op.to_tagged(),
                    "phase": "completed",
                })
            });
            // Same honesty gates as the live path: size bound, then the
            // declared output schema when resolvable.
            if self.output_too_large(&value) {
                self.settle_step_failed(
                    &path,
                    "job result exceeds max_output_bytes",
                    Some(RetryAdvice::AfterCorrection),
                )?;
                return Ok(());
            }
            if let Some(Step::AwaitJob {
                output_schema: Some(schema_ref),
                ..
            }) = run::find_step(&self.ctx.definition.steps, &step_id)
            {
                match self.ctx.registry.resolve_schema(schema_ref) {
                    Ok(schema) => {
                        if let Err(e) = validate_value(schema, "await-job result", &value) {
                            self.settle_step_failed(
                                &path,
                                &format!("job result schema violation: {e}"),
                                None,
                            )?;
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        self.settle_step_failed(
                            &path,
                            &format!("output schema unresolvable: {e}"),
                            None,
                        )?;
                        return Ok(());
                    }
                }
            }
            output = Some(value);
        }
        self.settle_step(
            &path,
            verdict,
            JournalRefs {
                operation: Some(*op),
                job,
                ..JournalRefs::default()
            },
            output,
            Some("settled during cancel".to_owned()),
            None,
        )
    }

    /// Read-only walk: would any leaf step still need to execute?
    /// Used by the cancel settle to distinguish "cancelled mid-plan"
    /// from "cancel arrived after the last side effect".
    fn has_pending_work(&self) -> Result<bool, RunError> {
        let mut path = StepPath::new();
        let steps: &'a [Step] = &self.ctx.definition.steps;
        self.pending_in(steps, &mut path, None, None)
    }

    fn pending_in(
        &self,
        steps: &[Step],
        path: &mut StepPath,
        iteration: Option<u64>,
        item: Option<&Value>,
    ) -> Result<bool, RunError> {
        for step in steps {
            path.push(StepFrame {
                step_id: step.id().to_owned(),
                iteration,
            });
            let full = path.clone();
            let settled = self
                .run
                .checkpoint(&full)
                .is_some_and(|c| c.state == StepState::Completed);
            if !settled {
                match step {
                    Step::If {
                        condition,
                        then,
                        else_steps,
                        ..
                    } => {
                        let env = EvalEnv {
                            inputs: self.ctx.inputs,
                            steps: &self.run.step_outputs,
                            item,
                        };
                        let cond = eval::eval_condition(condition, &env).unwrap_or(true);
                        let branch: &[Step] = if cond { then } else { else_steps };
                        let pending = self.pending_in(branch, path, iteration, item)?;
                        path.pop();
                        if pending {
                            return Ok(true);
                        }
                        continue;
                    }
                    Step::ForEachBounded {
                        items: items_expr,
                        max_iterations,
                        body,
                        ..
                    } => {
                        let env = EvalEnv {
                            inputs: self.ctx.inputs,
                            steps: &self.run.step_outputs,
                            item,
                        };
                        let items = eval::eval_expr(items_expr, &env)
                            .ok()
                            .and_then(|v| v.as_array().cloned());
                        let Some(items) = items else {
                            // Cannot prove complete — conservative: work
                            // may remain.
                            path.pop();
                            return Ok(true);
                        };
                        let count = items
                            .len()
                            .min(usize::try_from(*max_iterations).unwrap_or(usize::MAX));
                        for i in 0..count {
                            let pending =
                                self.pending_in(body, path, Some(i as u64), items.get(i))?;
                            if pending {
                                path.pop();
                                return Ok(true);
                            }
                        }
                        path.pop();
                        continue;
                    }
                    _ => {
                        path.pop();
                        return Ok(true);
                    }
                }
            }
            path.pop();
        }
        Ok(false)
    }

    // -- journal helpers -------------------------------------------------

    fn eval_env<'e>(&'e self, item: Option<&'e Value>) -> EvalEnv<'e> {
        EvalEnv {
            inputs: self.ctx.inputs,
            steps: &self.run.step_outputs,
            item,
        }
    }

    fn checkpoint_state(&self, path: &StepPath) -> Option<StepState> {
        self.run.checkpoint(path).map(|c| c.state)
    }

    /// Deterministic per-occurrence idempotency key.
    fn step_key(&self, path: &StepPath) -> Result<IdempotencyKey, RunError> {
        let rendered = path
            .iter()
            .map(|f| match f.iteration {
                Some(i) => format!("{}#{i}", f.step_id),
                None => f.step_id.clone(),
            })
            .collect::<Vec<_>>()
            .join(".");
        let candidate = format!("{}:{rendered}", self.run.run_id);
        if candidate.chars().count() <= 250 {
            return IdempotencyKey::new(candidate).map_err(|e| RunError::Lane {
                reason: format!("idempotency key invalid: {e}"),
            });
        }
        let digest = digest_of(&Value::String(rendered)).map_err(|e| RunError::Journal {
            reason: format!("key digest failed: {e}"),
        })?;
        IdempotencyKey::new(format!("{}:{}", self.run.run_id, &digest[..32])).map_err(|e| {
            RunError::Lane {
                reason: format!("idempotency key invalid: {e}"),
            }
        })
    }

    /// Journals `StepBegan` for `path`. Lane steps carry the capability
    /// ref, the asserted revision, and the occurrence idempotency key;
    /// control/local steps carry none.
    fn journal_step_begin(
        &mut self,
        path: &StepPath,
        capability_ref: Option<String>,
        expected_revision: Option<u64>,
        key: Option<IdempotencyKey>,
    ) -> Result<(), RunError> {
        self.journal(JournalEntry {
            run_id: self.run.run_id.clone(),
            seq: 0,
            step: Some(path.clone()),
            plan_revision: self.run.plan_digest.clone(),
            capability_ref,
            kind: JournalKind::StepBegan,
            verdict: None,
            from_state: None,
            to_state: None,
            refs: JournalRefs::default(),
            idempotency_key: key,
            expected_revision,
            digest: String::new(),
            output: None,
            detail: None,
            retry_advice: None,
        })
    }

    /// Journals a step verdict. `Waiting`/`Approval` observations are
    /// `StepObserved`; everything else is `StepSettled`. Identical
    /// repeat observations are deduplicated — re-drives do not spam the
    /// journal.
    ///
    /// Re-activation: the step FSM only admits new verdicts from
    /// `Running` (or the legal park exits). When the checkpoint sits in
    /// `Waiting`/`Approval`/`Indeterminate` and the incoming verdict is
    /// not directly reachable, a `StepBegan` re-activation is journaled
    /// first — the occurrence returns to `Running` under its *same*
    /// idempotency identity before the new observation lands.
    fn settle_step(
        &mut self,
        path: &StepPath,
        verdict: StepVerdict,
        refs: JournalRefs,
        output: Option<Value>,
        detail: Option<String>,
        retry_policy: Option<RetryAdvice>,
    ) -> Result<(), RunError> {
        let target = match verdict {
            StepVerdict::Waiting => StepState::Waiting,
            StepVerdict::Approval => StepState::Approval,
            StepVerdict::CancelRequested => StepState::CancelRequested,
            StepVerdict::Indeterminate => StepState::Indeterminate,
            StepVerdict::Completed => StepState::Completed,
            StepVerdict::Failed => StepState::Failed,
        };
        if output.is_none()
            && let Some(cp) = self.run.checkpoint(path)
            && cp.verdict == Some(verdict)
            && cp.operation_id == refs.operation
            && cp.proposal_ref == refs.proposal
            && cp.approval_ref == refs.approval
            && cp.job_ref == refs.job
        {
            return Ok(());
        }
        // Re-activation gate: journal StepBegan when the FSM has no
        // direct edge to the target verdict's state.
        let needs_readmit = self.run.checkpoint(path).is_some_and(|cp| {
            cp.state != StepState::Running && !cp.state.can_transition_to(target)
        });
        if needs_readmit {
            // Fields stay empty: apply_entry's refresh preserves the
            // checkpoint's existing idempotency key and asserted
            // revision — re-activation never mints a new identity.
            self.journal_step_begin(path, None, None, None)?;
        }
        let kind = if matches!(verdict, StepVerdict::Waiting | StepVerdict::Approval) {
            JournalKind::StepObserved
        } else {
            JournalKind::StepSettled
        };
        self.journal(JournalEntry {
            run_id: self.run.run_id.clone(),
            seq: 0,
            step: Some(path.clone()),
            plan_revision: self.run.plan_digest.clone(),
            capability_ref: None,
            kind,
            verdict: Some(verdict),
            from_state: None,
            to_state: None,
            refs,
            idempotency_key: None,
            expected_revision: None,
            digest: String::new(),
            output,
            detail,
            retry_advice: retry_policy,
        })?;
        Ok(())
    }

    /// Journals a non-terminal observation only when it changed.
    fn observe_step(
        &mut self,
        path: &StepPath,
        verdict: StepVerdict,
        refs: JournalRefs,
        detail: Option<String>,
    ) -> Result<(), RunError> {
        self.settle_step(path, verdict, refs, None, detail, None)
    }

    /// Journals a `Failed` verdict then fails the run — the typed step
    /// failure is evidence; the run settlement records it.
    fn settle_step_failed(
        &mut self,
        path: &StepPath,
        reason: &str,
        retry_policy: Option<RetryAdvice>,
    ) -> Result<(), RunError> {
        self.settle_step_failed_retry(path, reason, retry_policy)
    }

    fn settle_step_failed_retry(
        &mut self,
        path: &StepPath,
        reason: &str,
        retry_policy: Option<RetryAdvice>,
    ) -> Result<(), RunError> {
        self.settle_step(
            path,
            StepVerdict::Failed,
            JournalRefs::default(),
            None,
            Some(bound_detail(reason)),
            retry_policy,
        )?;
        self.run_to(RunState::Failed)
    }

    /// Transitions the run state with a journaled record.
    fn run_to(&mut self, to: RunState) -> Result<(), RunError> {
        if self.run.state == to || self.run.state.terminal() {
            return Ok(());
        }
        self.transition_run(to, None)
    }

    /// Serialized-size guard for step/run outputs.
    fn output_too_large(&self, value: &Value) -> bool {
        serde_json::to_string(value)
            .map_or(true, |s| s.len() as u64 > self.ctx.plan.max_output_bytes)
    }
}

/// Bounds a detail string before it enters the journal.
fn bound_detail(s: &str) -> String {
    s.chars().take(512).collect()
}
