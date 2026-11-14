//! AUTX-06 fault-fixture suite: WorkflowRun/Step FSM, journal replay,
//! idempotent resume, cancel honesty, and indeterminate preservation —
//! all over `MemoryRunJournal` + scripted lanes (F-1..F-10 matrix,
//! `docs/automation/design-workflow-run.md` §5).

mod common;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};
use synvelis_automation::coordinator::{
    JobLane, JobObservation, OperationObservation, RunContext, RunMeta, StepCall, StepLane,
    WorkflowCoordinator,
};
use synvelis_automation::run::{
    JournalEntry, JournalKind, JournalRefs, MemoryRunJournal, RunError, RunHeader, RunId,
    RunJournal, RunState, StepFrame, StepPath, StepVerdict, entry_digest, replay_entries,
};
use synvelis_automation::{
    ExecutablePlan, PlanCompiler, RegistrySnapshot, StepOutputRef, WorkflowDefinition,
};
use synvelis_gateway::id::{IdempotencyKey, OperationId, ProposalRef};
use synvelis_gateway::job::{CancelLink, CancelResponse, JobPhase};
use synvelis_gateway::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome, RetryAdvice};
use synvelis_gateway::request::{Principal, PrincipalKind};

use crate::common::{
    ASYNC_WORKFLOW, BASIC_WORKFLOW, LOOP_WORKFLOW, async_inputs, basic_inputs, definition,
    loop_inputs, registry,
};

// ---------- fixtures -------------------------------------------------------

type LaneResult = Result<GatewayOutcome<Value>, Box<ErrorEnvelope>>;

/// Scripted capability lane: pops one outcome per `invoke`, records
/// every call (idempotency key included) for assertions.
struct MockLane {
    script: Mutex<VecDeque<LaneResult>>,
    calls: Mutex<Vec<StepCall>>,
    op_results: Mutex<HashMap<OperationId, OperationObservation>>,
}

impl MockLane {
    fn new(script: Vec<LaneResult>) -> Self {
        Self {
            script: Mutex::new(script.into()),
            calls: Mutex::new(Vec::new()),
            op_results: Mutex::new(HashMap::new()),
        }
    }

    fn completed(result: Value) -> GatewayOutcome<Value> {
        GatewayOutcome::Completed { result }
    }

    fn calls(&self) -> Vec<StepCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl StepLane for MockLane {
    fn invoke(&self, call: &StepCall) -> LaneResult {
        self.calls.lock().unwrap().push(call.clone());
        self.script
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Ok(GatewayOutcome::Completed { result: json!({}) }))
    }

    fn operation_result(&self, operation: &OperationId) -> OperationObservation {
        self.op_results
            .lock()
            .unwrap()
            .get(operation)
            .cloned()
            .unwrap_or(OperationObservation::Unknown)
    }
}

/// Scripted job lane: one mutable observation + recorded cancels.
struct MockJobs {
    observation: Mutex<Option<JobObservation>>,
    cancel_response: Mutex<CancelResponse>,
    cancels: Mutex<Vec<OperationId>>,
}

impl MockJobs {
    fn new(observation: JobObservation) -> Self {
        Self {
            observation: Mutex::new(Some(observation)),
            cancel_response: Mutex::new(CancelResponse::Unreachable),
            cancels: Mutex::new(Vec::new()),
        }
    }

    fn live(phase: JobPhase) -> JobObservation {
        JobObservation::Live {
            phase,
            progress: None,
            result: None,
            job: None,
            cancel: CancelLink::NotRequested,
        }
    }

    fn set(&self, observation: JobObservation) {
        *self.observation.lock().unwrap() = Some(observation);
    }

    fn cancel_count(&self) -> usize {
        self.cancels.lock().unwrap().len()
    }
}

impl JobLane for MockJobs {
    fn observe(&self, _operation: &OperationId) -> JobObservation {
        self.observation
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(JobObservation::Unknown)
    }

    fn request_cancel(&self, operation: &OperationId) -> CancelResponse {
        self.cancels.lock().unwrap().push(*operation);
        self.cancel_response.lock().unwrap().clone()
    }
}

struct Fixture {
    journal: Arc<MemoryRunJournal>,
    lane: Arc<MockLane>,
    jobs: Arc<MockJobs>,
    coord: WorkflowCoordinator,
}

fn fixture(script: Vec<LaneResult>, job_observation: JobObservation) -> Fixture {
    let journal = Arc::new(MemoryRunJournal::new());
    let lane = Arc::new(MockLane::new(script));
    let jobs = Arc::new(MockJobs::new(job_observation));
    let coord = WorkflowCoordinator::new(journal.clone(), lane.clone(), jobs.clone());
    Fixture {
        journal,
        lane,
        jobs,
        coord,
    }
}

fn compiled(yaml: &str, inputs: Value) -> (WorkflowDefinition, ExecutablePlan) {
    let def = definition(yaml);
    let inputs_map: Map<String, Value> = match inputs {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    let out = PlanCompiler::compile(&def, &inputs_map, &registry()).expect("fixture must compile");
    (def, out.plan)
}

fn ctx<'a>(
    def: &'a WorkflowDefinition,
    plan: &'a ExecutablePlan,
    inputs: &'a Map<String, Value>,
    registry: &'a RegistrySnapshot,
) -> RunContext<'a> {
    RunContext {
        definition: def,
        plan,
        inputs,
        registry,
        macros: &[],
    }
}

fn meta(run_id: &str) -> RunMeta {
    RunMeta {
        run_id: RunId::new(run_id).unwrap(),
        idempotency_key: IdempotencyKey::new(format!("start-{run_id}")).unwrap(),
        principal: Principal::new(PrincipalKind::Agent, "wf-test").unwrap(),
        project_id: "project-test".to_owned(),
        task: None,
        grant: None,
        policy_epoch: Some(1),
        expected_revision: None,
    }
}

fn inputs_map(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

fn path_of(step_id: &str) -> StepPath {
    vec![StepFrame {
        step_id: step_id.to_owned(),
        iteration: None,
    }]
}

fn entry(
    run_id: &RunId,
    seq: u64,
    kind: JournalKind,
    step: Option<StepPath>,
    verdict: Option<StepVerdict>,
    from: Option<RunState>,
    to: Option<RunState>,
) -> JournalEntry {
    let mut e = JournalEntry {
        run_id: run_id.clone(),
        seq,
        step,
        plan_revision: "plan".to_owned(),
        capability_ref: None,
        kind,
        verdict,
        from_state: from,
        to_state: to,
        refs: JournalRefs::default(),
        idempotency_key: None,
        expected_revision: None,
        digest: String::new(),
        output: None,
        detail: None,
        retry_advice: None,
    };
    e.digest = entry_digest(&e).unwrap();
    e
}

fn header_for(run_id: &RunId) -> RunHeader {
    RunHeader {
        run_id: run_id.clone(),
        plan_digest: "plan".to_owned(),
        definition_version: 1,
        project_id: "project-test".to_owned(),
        input_digest: "inputs".to_owned(),
        principal: Principal::new(PrincipalKind::Agent, "wf-test").unwrap(),
        policy_epoch: None,
        expected_revision: None,
        idempotency_key: IdempotencyKey::new("start").unwrap(),
        task: None,
        grant: None,
    }
}

// ---------- happy path + plan identity -------------------------------------

#[test]
fn basic_workflow_completes_with_journaled_boundaries() {
    let (def, plan) = compiled(BASIC_WORKFLOW, basic_inputs());
    let reg = registry();
    let inputs = inputs_map(basic_inputs());
    let f = fixture(
        vec![
            Ok(MockLane::completed(json!([{"id": "track-1", "name": "a"}]))),
            Ok(MockLane::completed(json!(
                "note-0123456789abcdef0123456789abcdef"
            ))),
        ],
        MockJobs::live(JobPhase::Pending),
    );
    let run_id = f
        .coord
        .start_run(&ctx(&def, &plan, &inputs, &reg), &meta("run-basic"))
        .unwrap();
    let run = f
        .coord
        .drive(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(run.state, RunState::Completed);
    let out = f
        .coord
        .result(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(out["note"], json!("note-0123456789abcdef0123456789abcdef"));

    // Step boundaries: every lane step has StepBegan before its settle.
    let entries = f.journal.entries(&run_id).unwrap();
    let add_begin = entries
        .iter()
        .position(|e| e.kind == JournalKind::StepBegan && e.step.as_ref() == Some(&path_of("add")))
        .expect("add StepBegan");
    let add_settle = entries
        .iter()
        .position(|e| {
            e.kind == JournalKind::StepSettled
                && e.step.as_ref() == Some(&path_of("add"))
                && e.verdict == Some(StepVerdict::Completed)
        })
        .expect("add StepSettled");
    assert!(add_begin < add_settle, "begin must precede settle");
    // Exactly two lane invocations (query + invoke); emit is local.
    assert_eq!(f.lane.calls().len(), 2);
}

#[test]
fn duplicate_start_is_idempotent_rejected() {
    let (def, plan) = compiled(BASIC_WORKFLOW, basic_inputs());
    let reg = registry();
    let inputs = inputs_map(basic_inputs());
    let f = fixture(
        vec![
            Ok(MockLane::completed(json!([]))),
            Ok(MockLane::completed(json!(
                "note-0123456789abcdef0123456789abcdef"
            ))),
        ],
        MockJobs::live(JobPhase::Pending),
    );
    let m = meta("run-dup");
    let c = ctx(&def, &plan, &inputs, &reg);
    f.coord.start_run(&c, &m).unwrap();
    // Identical fingerprint -> DuplicateRun; no second header, no re-drive.
    let err = f.coord.start_run(&c, &m).unwrap_err();
    assert!(matches!(err, RunError::DuplicateRun { .. }), "{err:?}");
    // Different idempotency fingerprint -> IdempotencyConflict.
    let mut other = meta("run-dup");
    other.idempotency_key = IdempotencyKey::new("start-other").unwrap();
    let err = f.coord.start_run(&c, &other).unwrap_err();
    assert!(
        matches!(err, RunError::IdempotencyConflict { .. }),
        "{err:?}"
    );
}

#[test]
fn plan_identity_mismatch_rejected_on_resume() {
    let (def, plan) = compiled(BASIC_WORKFLOW, basic_inputs());
    let reg = registry();
    let inputs = inputs_map(basic_inputs());
    let f = fixture(
        vec![
            Ok(MockLane::completed(json!([]))),
            Ok(MockLane::completed(json!(
                "note-0123456789abcdef0123456789abcdef"
            ))),
        ],
        MockJobs::live(JobPhase::Pending),
    );
    let run_id = f
        .coord
        .start_run(&ctx(&def, &plan, &inputs, &reg), &meta("run-pm"))
        .unwrap();
    // A tampered plan (reordered steps) must not be trusted on resume.
    let mut tampered = plan.clone();
    tampered.step_order.reverse();
    let err = f
        .coord
        .status(&run_id, &ctx(&def, &tampered, &inputs, &reg))
        .unwrap_err();
    assert!(matches!(err, RunError::PlanMismatch { .. }), "{err:?}");
}

// ---------- async submit/await + resume (F-1/F-2/F-5) ------------------------

fn async_fixture(
    job: JobObservation,
) -> (
    Fixture,
    ExecutablePlan,
    WorkflowDefinition,
    Map<String, Value>,
    RegistrySnapshot,
) {
    let (def, plan) = compiled(ASYNC_WORKFLOW, async_inputs());
    let inputs = inputs_map(async_inputs());
    let reg = registry();
    let op = OperationId::new(7);
    let f = fixture(
        vec![
            Ok(GatewayOutcome::Accepted { operation: op }),
            Ok(MockLane::completed(json!({"state": "succeeded"}))),
        ],
        job,
    );
    (f, plan, def, inputs, reg)
}

fn live_completed(result: Value) -> JobObservation {
    JobObservation::Live {
        phase: JobPhase::Completed,
        progress: Some(100),
        result: Some(result),
        job: None,
        cancel: CancelLink::NotRequested,
    }
}

#[test]
fn async_submit_await_completes_and_retires_pending_job() {
    let (f, plan, def, inputs, reg) = async_fixture(MockJobs::live(JobPhase::Running));
    let run_id = f
        .coord
        .start_run(&ctx(&def, &plan, &inputs, &reg), &meta("run-async"))
        .unwrap();
    f.coord
        .drive(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    let waiting = f
        .coord
        .status(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(waiting.state, RunState::Waiting);
    assert_eq!(waiting.pending_jobs.len(), 1);

    // Job finishes; a fresh coordinator over the same journal resumes.
    f.jobs.set(live_completed(json!({"state": "succeeded"})));
    let f2 = WorkflowCoordinator::new(f.journal.clone(), f.lane.clone(), f.jobs.clone());
    let run = f2.drive(&run_id, &ctx(&def, &plan, &inputs, &reg)).unwrap();
    assert_eq!(run.state, RunState::Completed);
    assert_eq!(run.pending_jobs, Vec::<OperationId>::new());
    let out = f2
        .result(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(out["status"], json!("succeeded"));

    // No duplicate submit: render invoked exactly once across both
    // coordinators (F-2 — idempotent resume).
    let render_calls = f
        .lane
        .calls()
        .iter()
        .filter(|c| c.capability.to_string().contains("job.render"))
        .count();
    assert_eq!(render_calls, 1);
}

#[test]
fn unknown_job_observation_stays_indeterminate_until_evidence() {
    let (f, plan, def, inputs, reg) = async_fixture(JobObservation::Unknown);
    let run_id = f
        .coord
        .start_run(&ctx(&def, &plan, &inputs, &reg), &meta("run-f5"))
        .unwrap();
    f.coord
        .drive(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    let run = f
        .coord
        .status(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(run.state, RunState::Indeterminate);
    // Unknown is never promoted: result() refuses.
    assert!(matches!(
        f.coord.result(&run_id, &ctx(&def, &plan, &inputs, &reg)),
        Err(RunError::NotCompleted { .. })
    ));
    // Evidence arrives -> reconcile resolves to Completed.
    f.jobs.set(live_completed(json!({"state": "succeeded"})));
    let run = f
        .coord
        .reconcile(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(run.state, RunState::Completed);
}

#[test]
fn lane_indeterminate_error_parks_then_reconciles_same_key() {
    let (def, plan) = compiled(BASIC_WORKFLOW, basic_inputs());
    let reg = registry();
    let inputs = inputs_map(basic_inputs());
    let env = || {
        Box::new(ErrorEnvelope::new(GatewayError::Indeterminate {
            reason: "provider restart".to_owned(),
        }))
    };
    let f = fixture(
        vec![
            Err(env()),
            Ok(MockLane::completed(json!([{"id": "t", "name": "a"}]))),
            Ok(MockLane::completed(json!(
                "note-0123456789abcdef0123456789abcdef"
            ))),
        ],
        MockJobs::live(JobPhase::Pending),
    );
    let run_id = f
        .coord
        .start_run(&ctx(&def, &plan, &inputs, &reg), &meta("run-ind"))
        .unwrap();
    f.coord
        .drive(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    let run = f
        .coord
        .status(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(run.state, RunState::Indeterminate);

    let run = f
        .coord
        .reconcile(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(run.state, RunState::Completed);
    // The retried call reused the journaled idempotency key (F-1).
    let calls = f.lane.calls();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].idempotency_key, calls[1].idempotency_key);
}

// ---------- cancel honesty (F-4) --------------------------------------------

#[test]
fn cancel_requests_once_and_provider_truth_settles() {
    let (f, plan, def, inputs, reg) = async_fixture(MockJobs::live(JobPhase::Running));
    *f.jobs.cancel_response.lock().unwrap() = CancelResponse::Pending;
    let run_id = f
        .coord
        .start_run(&ctx(&def, &plan, &inputs, &reg), &meta("run-cancel"))
        .unwrap();
    f.coord
        .drive(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    let run = f
        .coord
        .cancel(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(run.state, RunState::CancelRequested);
    assert_eq!(f.jobs.cancel_count(), 1);

    // Provider confirms the stop; a second cancel must not resend.
    f.jobs.set(JobObservation::Live {
        phase: JobPhase::Cancelled,
        progress: None,
        result: None,
        job: None,
        cancel: CancelLink::Confirmed,
    });
    let run = f
        .coord
        .cancel(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(run.state, RunState::Failed);
    assert_eq!(f.jobs.cancel_count(), 1, "cancel must not be resent");
}

#[test]
fn cancel_settled_race_records_actual_phase() {
    let (f, plan, def, inputs, reg) = async_fixture(MockJobs::live(JobPhase::Running));
    *f.jobs.cancel_response.lock().unwrap() = CancelResponse::Settled {
        phase: JobPhase::Completed,
    };
    let run_id = f
        .coord
        .start_run(&ctx(&def, &plan, &inputs, &reg), &meta("run-race"))
        .unwrap();
    f.coord
        .drive(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    // The job completed before cancel landed: the await step settles
    // Completed with evidence; remaining plan work was interrupted, so
    // the run ends Failed — never "cancelled" by assertion.
    f.jobs.set(live_completed(json!({"state": "succeeded"})));
    let run = f
        .coord
        .cancel(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    let wait = run.checkpoint(&path_of("wait")).expect("wait checkpoint");
    assert_eq!(wait.verdict, Some(StepVerdict::Completed));
    assert_eq!(run.state, RunState::Failed);
}

// ---------- gateway error mapping -------------------------------------------

#[test]
fn revision_conflict_fails_with_correction_advice() {
    let (def, plan) = compiled(BASIC_WORKFLOW, basic_inputs());
    let reg = registry();
    let inputs = inputs_map(basic_inputs());
    let f = fixture(
        vec![Err(Box::new(ErrorEnvelope::new(
            GatewayError::RevisionConflict {
                expected: 3,
                actual: 4,
            },
        )))],
        MockJobs::live(JobPhase::Pending),
    );
    let run_id = f
        .coord
        .start_run(&ctx(&def, &plan, &inputs, &reg), &meta("run-rev"))
        .unwrap();
    f.coord
        .drive(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    let run = f
        .coord
        .status(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(run.state, RunState::Failed);
    let tracks = run.checkpoint(&path_of("tracks")).unwrap();
    assert_eq!(tracks.verdict, Some(StepVerdict::Failed));
    assert_eq!(tracks.retry_policy, Some(RetryAdvice::AfterCorrection));
}

#[test]
fn approval_required_parks_run_without_self_approving() {
    let (def, plan) = compiled(BASIC_WORKFLOW, basic_inputs());
    let reg = registry();
    let inputs = inputs_map(basic_inputs());
    let approval = synvelis_gateway::id::ApprovalRef::new("appr-1").unwrap();
    // The authoritative side keeps reporting the pending approval on
    // every re-presentation — the workflow never self-approves.
    let f = fixture(
        vec![
            Ok(GatewayOutcome::ApprovalRequired {
                approval: approval.clone(),
            }),
            Ok(GatewayOutcome::ApprovalRequired {
                approval: approval.clone(),
            }),
        ],
        MockJobs::live(JobPhase::Pending),
    );
    let run_id = f
        .coord
        .start_run(&ctx(&def, &plan, &inputs, &reg), &meta("run-appr"))
        .unwrap();
    f.coord
        .drive(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    let run = f
        .coord
        .status(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(run.state, RunState::Approval);
    // Re-presenting on re-drive is the lane's job — the workflow never
    // self-approves; the run stays parked.
    let run = f
        .coord
        .drive(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(run.state, RunState::Approval);
    assert!(matches!(
        f.coord.result(&run_id, &ctx(&def, &plan, &inputs, &reg)),
        Err(RunError::NotCompleted { .. })
    ));
}

#[test]
fn proposal_ready_parks_run() {
    let (def, plan) = compiled(BASIC_WORKFLOW, basic_inputs());
    let reg = registry();
    let inputs = inputs_map(basic_inputs());
    let proposal = ProposalRef::new("prop-1").unwrap();
    let f = fixture(
        vec![Ok(GatewayOutcome::ProposalReady {
            proposal: proposal.clone(),
        })],
        MockJobs::live(JobPhase::Pending),
    );
    let run_id = f
        .coord
        .start_run(&ctx(&def, &plan, &inputs, &reg), &meta("run-prop"))
        .unwrap();
    f.coord
        .drive(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    let run = f
        .coord
        .status(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(run.state, RunState::Approval);
    let tracks = run.checkpoint(&path_of("tracks")).unwrap();
    assert_eq!(tracks.proposal_ref, Some(proposal));
}

// ---------- replay integrity -------------------------------------------------

#[test]
fn journal_digest_mismatch_is_replay_divergence() {
    let (def, plan) = compiled(BASIC_WORKFLOW, basic_inputs());
    let reg = registry();
    let inputs = inputs_map(basic_inputs());
    let f = fixture(
        vec![
            Ok(MockLane::completed(json!([]))),
            Ok(MockLane::completed(json!(
                "note-0123456789abcdef0123456789abcdef"
            ))),
        ],
        MockJobs::live(JobPhase::Pending),
    );
    let run_id = f
        .coord
        .start_run(&ctx(&def, &plan, &inputs, &reg), &meta("run-tamper"))
        .unwrap();
    f.coord
        .drive(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    let header = f.journal.run_header(&run_id).unwrap().unwrap();
    let mut entries = f.journal.entries(&run_id).unwrap();
    // Forge: mutate a payload field without recomputing the digest.
    if let Some(e) = entries
        .iter_mut()
        .find(|e| e.kind == JournalKind::StepBegan)
    {
        e.detail = Some("forged".to_owned());
    }
    let err = replay_entries(&header, &entries, &def).unwrap_err();
    assert!(matches!(err, RunError::ReplayDivergence { .. }), "{err:?}");
}

#[test]
fn settle_without_begin_is_replay_divergence() {
    let (def, _plan) = compiled(BASIC_WORKFLOW, basic_inputs());
    let run_id = RunId::new("run-crafted").unwrap();
    let entries = vec![entry(
        &run_id,
        0,
        JournalKind::StepSettled,
        Some(path_of("ghost")),
        Some(StepVerdict::Completed),
        None,
        None,
    )];
    let err = replay_entries(&header_for(&run_id), &entries, &def).unwrap_err();
    assert!(matches!(err, RunError::ReplayDivergence { .. }), "{err:?}");
}

#[test]
fn illegal_run_transition_is_replay_divergence() {
    let (def, _plan) = compiled(BASIC_WORKFLOW, basic_inputs());
    let run_id = RunId::new("run-illegal").unwrap();
    let entries = vec![entry(
        &run_id,
        0,
        JournalKind::RunTransition,
        None,
        None,
        Some(RunState::Created),
        Some(RunState::Completed),
    )];
    let err = replay_entries(&header_for(&run_id), &entries, &def).unwrap_err();
    assert!(matches!(err, RunError::ReplayDivergence { .. }), "{err:?}");
}

// ---------- bounded loop + budgets -------------------------------------------

#[test]
fn bounded_loop_executes_each_item() {
    let (def, plan) = compiled(LOOP_WORKFLOW, loop_inputs());
    let reg = registry();
    let inputs = inputs_map(loop_inputs());
    let f = fixture(
        vec![
            Ok(MockLane::completed(json!(
                "note-11111111111111111111111111111111"
            ))),
            Ok(MockLane::completed(json!(
                "note-22222222222222222222222222222222"
            ))),
        ],
        MockJobs::live(JobPhase::Pending),
    );
    let run_id = f
        .coord
        .start_run(&ctx(&def, &plan, &inputs, &reg), &meta("run-loop"))
        .unwrap();
    let run = f
        .coord
        .drive(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(run.state, RunState::Completed);
    // Two items -> two invoke calls, each under its own occurrence key.
    let calls = f.lane.calls();
    assert_eq!(calls.len(), 2);
    assert_ne!(calls[0].idempotency_key, calls[1].idempotency_key);
}

#[test]
fn runtime_budget_exceedance_fails_closed() {
    let (def, mut plan) = compiled(LOOP_WORKFLOW, loop_inputs());
    plan.max_steps = 1; // below the plan's own worst case — runtime gate
    let reg = registry();
    let inputs = inputs_map(loop_inputs());
    let f = fixture(
        vec![Ok(MockLane::completed(json!("note-1")))],
        MockJobs::live(JobPhase::Pending),
    );
    let run_id = f
        .coord
        .start_run(&ctx(&def, &plan, &inputs, &reg), &meta("run-budget"))
        .unwrap();
    f.coord
        .drive(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    let run = f
        .coord
        .status(&run_id, &ctx(&def, &plan, &inputs, &reg))
        .unwrap();
    assert_eq!(run.state, RunState::Failed);
}

// ---------- eval unit coverage ------------------------------------------------

#[test]
fn eval_resolves_nested_paths_and_rejects_out_of_scope_refs() {
    use synvelis_automation::eval::{EvalEnv, eval_expr};
    use synvelis_automation::expr::Expr;

    let inputs: Map<String, Value> = inputs_map(json!({"cfg": {"gain": 3}}));
    let steps = BTreeMap::from([("s1".to_owned(), json!({"nested": {"value": 42}}))]);
    let env = EvalEnv {
        inputs: &inputs,
        steps: &steps,
        item: None,
    };
    let v = eval_expr(
        &Expr::Input {
            path: vec!["cfg".to_owned(), "gain".to_owned()],
        },
        &env,
    )
    .unwrap();
    assert_eq!(v, json!(3));
    let v = eval_expr(
        &Expr::StepOutput {
            step_output: StepOutputRef {
                step: "s1".to_owned(),
                field: vec!["nested".to_owned(), "value".to_owned()],
            },
        },
        &env,
    )
    .unwrap();
    assert_eq!(v, json!(42));
    // Missing input -> error, never a default.
    assert!(
        eval_expr(
            &Expr::Input {
                path: vec!["nope".to_owned()]
            },
            &env
        )
        .is_err()
    );
    // $item outside a loop -> error.
    assert!(eval_expr(&Expr::Item { path: vec![] }, &env).is_err());
    // Macro-only references are rejected at workflow runtime.
    assert!(eval_expr(&Expr::Param { path: vec![] }, &env).is_err());
    assert!(
        eval_expr(
            &Expr::Output {
                name: "x".to_owned()
            },
            &env
        )
        .is_err()
    );
}
