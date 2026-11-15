// Test closures mirror the gateway's typed-envelope signatures.
#![allow(clippy::result_large_err)]

//! AUTX-06 evidence — the headless workflow coordinator driven over
//! real Gateway authority (synvelis#265).
//!
//! These tests compose the production [`GatewayStepLane`] /
//! [`GatewayJobLane`] adapters against the same rig gateways the GW-*
//! suites use: every step invoke, job observe, and cancel travels the
//! authoritative `Gateway::handle` wire path — parse → bind → invoke —
//! under a `LocalCliAdapter`-attested delegated caller. The coordinator
//! keeps only its own progress/journal state; proposals, jobs, grants,
//! and commits remain the gateway's truth.
//!
//! Covered here:
//! - a sequential workflow (atomic-macro prepare → async job submit →
//!   await → emit) completing over live authority, with the prepared
//!   proposal then applied and committed through the real `CommitGate`;
//! - a query-domain step over `synvelis.context.query`;
//! - grant revocation denying the next step invocation (F-6);
//! - honest cancellation: provider-confirmed cancel fails the run,
//!   provider-unreachable cancel leaves `CancelRequested` — never an
//!   upgraded verdict (F-4).

mod rig;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use rig::*;
use serde_json::{Map, json};
use synvelis_automation::coordinator::{RunContext, RunMeta, WorkflowCoordinator};
use synvelis_automation::run::{MemoryRunJournal, RunId, RunState};
use synvelis_automation::{
    GatewayJobLane, GatewayStepLane, MacroRegistry, PlanCompiler, capture_registry_snapshot,
    parse_macro_yaml, parse_workflow_yaml,
};
use synvelis_gateway::mcp_server::{CommitPort, NoCommitPort};
use synvelis_gateway::*;
use synvelis_project::ProjectRevision;

// ── Workflow definitions ─────────────────────────────────────

/// Sequential live flow: an atomic macro lowers to
/// `synvelis.operation.prepare`, the async step admits a provider job,
/// `await-job` observes it through `synvelis.job.get`, and the run
/// emits its declared output.
const LIVE_FLOW: &str = r"
schema_version: 1
workflow_id: test.live-flow
version: 1
steps:
  - kind: atomic-macro
    id: mute
    mac: test.mute-clip@1.0
    args:
      clip: clip-0000000000000000000000000000000b
  - kind: async-submit
    id: render
    capability: synvelis.job.submit@1.0
    args:
      provider: stub
      kind: generation
      output: media
      inputs:
        - source-00000000000000000000000000000009
  - kind: await-job
    id: wait
    job: $steps.render
  - kind: emit-result
    id: done
    output: ok
    value: true
outputs:
  - name: ok
    schema:
      type: bool
limits:
  max_steps: 100
  max_depth: 8
  max_iterations: 100
  max_runtime_ms: 60000
  max_output_bytes: 1048576
  max_memory_bytes: 134217728
";

/// The job-only flow used by the revocation/cancel scenarios.
const JOB_FLOW: &str = r"
schema_version: 1
workflow_id: test.live-jobs
version: 1
steps:
  - kind: async-submit
    id: render
    capability: synvelis.job.submit@1.0
    args:
      provider: stub
      kind: generation
      output: media
      inputs:
        - source-00000000000000000000000000000009
  - kind: await-job
    id: wait
    job: $steps.render
  - kind: emit-result
    id: done
    output: ok
    value: true
outputs:
  - name: ok
    schema:
      type: bool
limits:
  max_steps: 100
  max_depth: 8
  max_iterations: 100
  max_runtime_ms: 60000
  max_output_bytes: 1048576
  max_memory_bytes: 134217728
";

/// A query-domain-only flow for the context capability.
const QUERY_FLOW: &str = r"
schema_version: 1
workflow_id: test.live-query
version: 1
steps:
  - kind: query
    id: clips
    capability: synvelis.context.query@1.0
    args:
      selector:
        kind: clip
  - kind: emit-result
    id: done
    output: ok
    value: true
outputs:
  - name: ok
    schema:
      type: bool
limits:
  max_steps: 100
  max_depth: 8
  max_iterations: 100
  max_runtime_ms: 60000
  max_output_bytes: 1048576
  max_memory_bytes: 134217728
";

/// The mute-clip atomic macro — one `synvelis.clip.mute` member whose
/// target is the `clip` parameter (the proven `$param` target form).
const MUTE_CLIP_MACRO: &str = r"
schema_version: 1
macro_id: test.mute-clip
version: 1
parameters:
  - name: clip
    schema:
      type: id
      kind: clip
steps:
  - id: mute
    capability: synvelis.clip.mute@1.0
    args:
      muted: true
    target: $param.clip
";

// ── Rig composition helpers ──────────────────────────────────

/// The job rig plus its gateway moved behind the shared cell the live
/// lanes lock per dispatch. Must run *after* `boot` — `boot` reads
/// `rig.gw.registry()` for the apply descriptor, and the cell leaves a
/// placeholder gateway in the rig that no test path touches again.
struct LiveJobRig {
    jr: JobRig,
    gateway: Arc<Mutex<Gateway>>,
    gate: Arc<CommitGate>,
}

fn live_job_rig(stub: ProviderStub, tag: &str) -> LiveJobRig {
    let mut jr = job_rig(vec![("stub".to_owned(), stub)]);
    let dir = tmpdir(tag);
    let (gate, _report, _store) = boot(&jr.rig, &dir);
    let gate = Arc::new(gate);
    let gateway = Arc::new(Mutex::new(std::mem::replace(
        &mut jr.rig.gw,
        Gateway::new(),
    )));
    LiveJobRig { jr, gateway, gate }
}

/// Issues the run's task+grant on the rig's policy store. The grant
/// covers the workflow's capabilities: job submit/get/cancel, the
/// prepare/batch axis with the `clip.mute` member, and apply for the
/// trusted commit step.
fn issue_flow_grant(jr: &JobRig, task: &str, grant_tag: &str) {
    let mut grant = job_grant(
        grant_tag,
        task,
        &["synvelis.clip.mute", "synvelis.operation.batch"],
    );
    grant.permissions = PermissionSet::of([
        PermissionKind::MutateProject,
        PermissionKind::RunJob,
        PermissionKind::ReadContext,
        PermissionKind::DiscloseContext,
    ]);
    issue_agent(&jr.rig.store, task, grant);
}

fn cli_adapter(clock: &Arc<ManualClock>) -> Arc<LocalCliAdapter> {
    Arc::new(LocalCliAdapter::new(
        AdapterId::new(CLI_ADAPTER_RAW),
        Box::new(MemoryCliSessionStore::new()),
        clock.clone(),
    ))
}

fn run_meta(run: &str, key: &str) -> RunMeta {
    RunMeta {
        run_id: RunId::new(run).unwrap(),
        idempotency_key: IdempotencyKey::new(key).unwrap(),
        principal: agent_principal(),
        project_id: project().to_tagged(),
        task: Some(task_ref("t1")),
        grant: Some(grant_ref("g1")),
        policy_epoch: None,
        expected_revision: Some(1),
    }
}

// ── AUTX-06 AC1: sequential workflow over real authority ─────

#[test]
fn live_macro_submit_await_completes_and_commits_once() {
    let rig = live_job_rig(
        ProviderStub::generation(StubContent::shared()),
        "autx06-e2e",
    );
    issue_flow_grant(&rig.jr, "t1", "g1");

    // Registry snapshot is captured live — exact versions, no "latest".
    let snapshot =
        capture_registry_snapshot(&rig.gateway.lock().unwrap()).expect("live registry snapshot");
    let gateway = rig.gateway.clone();

    let def = parse_workflow_yaml(LIVE_FLOW).unwrap();
    let macro_def = parse_macro_yaml(MUTE_CLIP_MACRO).unwrap();
    let macros = MacroRegistry::from_definitions([&macro_def], &snapshot).unwrap();
    let inputs = Map::new();
    let out = PlanCompiler::compile_with_macros(&def, &inputs, &snapshot, &macros).unwrap();

    let cli = cli_adapter(&rig.jr.rig.clock);
    let committer: Arc<dyn CommitPort> = rig.gate.clone();
    let coord = WorkflowCoordinator::new(
        Arc::new(MemoryRunJournal::new()),
        Arc::new(GatewayStepLane::new(
            gateway.clone(),
            cli.clone(),
            committer,
            Some(rig.jr.jobs.clone()),
        )),
        Arc::new(GatewayJobLane::new(
            gateway.clone(),
            cli,
            agent_principal(),
            project().to_tagged(),
            Some(task_ref("t1")),
            Some(grant_ref("g1")),
        )),
    );
    let ctx = RunContext {
        definition: &def,
        plan: &out.plan,
        inputs: &inputs,
        registry: &snapshot,
        macros: std::slice::from_ref(&macro_def),
    };
    let meta = run_meta("run-live-1", "key-live-1");
    let run_id = coord.start_run(&ctx, &meta).unwrap();

    // Drive 1: macro prepares (Completed), job submits (Accepted),
    // await observes Pending → the run parks at Waiting.
    let run = coord.drive(&run_id, &ctx).unwrap();
    assert_eq!(run.state, RunState::Waiting);
    assert_eq!(rig.jr.providers["stub"].submissions().len(), 1);

    // Re-drive while parked: the settled submit is not re-issued and
    // the await re-observes authority — still exactly one submission.
    let run = coord.drive(&run_id, &ctx).unwrap();
    assert_eq!(run.state, RunState::Waiting);
    assert_eq!(rig.jr.providers["stub"].submissions().len(), 1);

    // The provider finishes; the next drive observes Completed and
    // the run emits its output.
    let job = rig.jr.providers["stub"].job_at(1).unwrap();
    rig.jr.providers["stub"].finish(&job);
    let run = coord.drive(&run_id, &ctx).unwrap();
    assert_eq!(run.state, RunState::Completed);
    let outputs = coord.result(&run_id, &ctx).unwrap();
    assert_eq!(outputs["ok"], json!(true));

    // The macro step's journaled output is the real prepare result —
    // the proposal/digest pair the trusted apply path consumes.
    let mute = run.step_outputs.get("mute").expect("mute step output");
    let proposal = mute["proposal"].as_str().unwrap().to_owned();
    let digest = mute["digest"].as_str().unwrap().to_owned();
    assert!(!clip11_muted(rig.gate.as_ref()), "prepare never commits");

    // Trusted apply + commit through the real gate: the workflow
    // produced authority, it did not perform the mutation itself.
    let pref = ProposalRef::new(&proposal).unwrap();
    let outcome = gateway
        .lock()
        .unwrap()
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));
    rig.gate.commit(&pref).unwrap();
    assert!(clip11_muted(rig.gate.as_ref()));
}

// ── AUTX-06 AC2: query domain over live authority ────────────

#[test]
fn live_query_step_reads_through_context_authority() {
    let mut rig = fixture_rig();
    let snapshot = capture_registry_snapshot(&rig.gw).expect("registry snapshot");
    // Move the fixture gateway into the shared cell — same seam the
    // job rig uses; the placeholder is never invoked.
    let gateway = Arc::new(Mutex::new(std::mem::replace(&mut rig.gw, Gateway::new())));

    // A minimal grant covering only the query capability.
    let grant = Grant {
        id: grant_ref("g1"),
        issued_by: human_principal(),
        principal: agent_principal(),
        project: project(),
        binding: GrantBinding::Task {
            task: task_ref("t1"),
        },
        capabilities: BTreeSet::from([cap_ref("synvelis.context.query")]),
        permissions: PermissionSet::of([
            PermissionKind::ReadContext,
            PermissionKind::DiscloseContext,
        ]),
        data_access: DataAccessSet::of([
            DataAccessClass::MetadataRead,
            DataAccessClass::ExternalDisclosure,
        ]),
        scope: ResourceScope::Exact {
            resources: BTreeSet::from([clip11(), clip12(), clip13()]),
        },
        create_within: None,
        approval_policy: ApprovalPolicy::DelegatedApply,
        limits: GrantLimits::default(),
        validity: Validity::default(),
        epoch_ceiling: None,
        issued_at_revision: Some(ProjectRevision(1)),
    };
    issue_agent(&rig.store, "t1", grant);

    let def = parse_workflow_yaml(QUERY_FLOW).unwrap();
    let inputs = Map::new();
    let out = PlanCompiler::compile(&def, &inputs, &snapshot).unwrap();

    let cli = cli_adapter(&rig.clock);
    let coord = WorkflowCoordinator::new(
        Arc::new(MemoryRunJournal::new()),
        Arc::new(GatewayStepLane::new(
            gateway.clone(),
            cli.clone(),
            Arc::new(NoCommitPort),
            None,
        )),
        Arc::new(GatewayJobLane::new(
            gateway,
            cli,
            agent_principal(),
            project().to_tagged(),
            Some(task_ref("t1")),
            Some(grant_ref("g1")),
        )),
    );
    let ctx = RunContext {
        definition: &def,
        plan: &out.plan,
        inputs: &inputs,
        registry: &snapshot,
        macros: &[],
    };
    let run_id = coord
        .start_run(&ctx, &run_meta("run-query-1", "key-query-1"))
        .unwrap();
    let run = coord.drive(&run_id, &ctx).unwrap();
    assert_eq!(run.state, RunState::Completed);
    let clips = run.step_outputs.get("clips").expect("query step output");
    // The query result is the authority's disclosure — in-scope clips
    // only, under the run's grant.
    assert!(clips.to_string().contains("clip-"));
}

// ── AUTX-06 AC3: revocation revalidation (F-6) ───────────────

#[test]
fn live_grant_revocation_denies_next_step() {
    let rig = live_job_rig(
        ProviderStub::generation(StubContent::shared()),
        "autx06-revoke",
    );
    issue_flow_grant(&rig.jr, "t1", "g1");
    let snapshot =
        capture_registry_snapshot(&rig.gateway.lock().unwrap()).expect("registry snapshot");
    let gateway = rig.gateway.clone();

    let def = parse_workflow_yaml(JOB_FLOW).unwrap();
    let inputs = Map::new();
    let out = PlanCompiler::compile(&def, &inputs, &snapshot).unwrap();

    let cli = cli_adapter(&rig.jr.rig.clock);
    let coord = WorkflowCoordinator::new(
        Arc::new(MemoryRunJournal::new()),
        Arc::new(GatewayStepLane::new(
            gateway.clone(),
            cli.clone(),
            Arc::new(NoCommitPort),
            Some(rig.jr.jobs.clone()),
        )),
        Arc::new(GatewayJobLane::new(
            gateway,
            cli,
            agent_principal(),
            project().to_tagged(),
            Some(task_ref("t1")),
            Some(grant_ref("g1")),
        )),
    );
    let ctx = RunContext {
        definition: &def,
        plan: &out.plan,
        inputs: &inputs,
        registry: &snapshot,
        macros: &[],
    };
    let run_id = coord
        .start_run(&ctx, &run_meta("run-revoke-1", "key-revoke-1"))
        .unwrap();

    // The grant dies between start and drive — the next invoke is
    // revalidated against live policy and denied at the authority.
    rig.jr.rig.store.revoke_grant(&grant_ref("g1"));
    let run = coord.drive(&run_id, &ctx).unwrap();
    assert_eq!(run.state, RunState::Failed);
    assert!(
        rig.jr.providers["stub"].submissions().is_empty(),
        "a denied submit must never reach the provider"
    );
}

// ── AUTX-06 AC4: honest cancellation (F-4) ───────────────────

#[test]
fn live_cancel_confirmed_by_provider_fails_the_run() {
    let rig = live_job_rig(
        ProviderStub::generation(StubContent::shared()),
        "autx06-cancel",
    );
    issue_flow_grant(&rig.jr, "t1", "g1");
    let snapshot =
        capture_registry_snapshot(&rig.gateway.lock().unwrap()).expect("registry snapshot");
    let gateway = rig.gateway.clone();

    let def = parse_workflow_yaml(JOB_FLOW).unwrap();
    let inputs = Map::new();
    let out = PlanCompiler::compile(&def, &inputs, &snapshot).unwrap();

    let cli = cli_adapter(&rig.jr.rig.clock);
    let coord = WorkflowCoordinator::new(
        Arc::new(MemoryRunJournal::new()),
        Arc::new(GatewayStepLane::new(
            gateway.clone(),
            cli.clone(),
            Arc::new(NoCommitPort),
            Some(rig.jr.jobs.clone()),
        )),
        Arc::new(GatewayJobLane::new(
            gateway,
            cli,
            agent_principal(),
            project().to_tagged(),
            Some(task_ref("t1")),
            Some(grant_ref("g1")),
        )),
    );
    let ctx = RunContext {
        definition: &def,
        plan: &out.plan,
        inputs: &inputs,
        registry: &snapshot,
        macros: &[],
    };
    let run_id = coord
        .start_run(&ctx, &run_meta("run-cancel-1", "key-cancel-1"))
        .unwrap();
    let run = coord.drive(&run_id, &ctx).unwrap();
    assert_eq!(run.state, RunState::Waiting);

    // Provider confirms the stop: the awaiting step observes the
    // authoritative `cancelled` phase and the run fails — the cancel
    // verdict is evidence, not a coordinator guess.
    let run = coord.cancel(&run_id, &ctx).unwrap();
    assert_eq!(run.state, RunState::Failed);
    assert_eq!(run.cancel_issued.len(), 1);
    assert_eq!(rig.jr.providers["stub"].submissions().len(), 1);
}

#[test]
fn live_cancel_unreachable_stays_cancel_requested() {
    let rig = live_job_rig(
        ProviderStub::generation(StubContent::shared()).cancel_mode(CancelMode::Unreachable),
        "autx06-cancel-unreach",
    );
    issue_flow_grant(&rig.jr, "t1", "g1");
    let snapshot =
        capture_registry_snapshot(&rig.gateway.lock().unwrap()).expect("registry snapshot");
    let gateway = rig.gateway.clone();

    let def = parse_workflow_yaml(JOB_FLOW).unwrap();
    let inputs = Map::new();
    let out = PlanCompiler::compile(&def, &inputs, &snapshot).unwrap();

    let cli = cli_adapter(&rig.jr.rig.clock);
    let coord = WorkflowCoordinator::new(
        Arc::new(MemoryRunJournal::new()),
        Arc::new(GatewayStepLane::new(
            gateway.clone(),
            cli.clone(),
            Arc::new(NoCommitPort),
            Some(rig.jr.jobs.clone()),
        )),
        Arc::new(GatewayJobLane::new(
            gateway,
            cli,
            agent_principal(),
            project().to_tagged(),
            Some(task_ref("t1")),
            Some(grant_ref("g1")),
        )),
    );
    let ctx = RunContext {
        definition: &def,
        plan: &out.plan,
        inputs: &inputs,
        registry: &snapshot,
        macros: &[],
    };
    let run_id = coord
        .start_run(&ctx, &run_meta("run-cancel-2", "key-cancel-2"))
        .unwrap();
    let run = coord.drive(&run_id, &ctx).unwrap();
    assert_eq!(run.state, RunState::Waiting);

    // The provider never answers the cancel: the run stays
    // `CancelRequested` — an unconfirmed cancel is never promoted to
    // `Failed` on the coordinator's say-so.
    let run = coord.cancel(&run_id, &ctx).unwrap();
    assert_eq!(run.state, RunState::CancelRequested);
    assert_eq!(run.cancel_issued.len(), 1);

    // A later drive re-observes: the job is still pending and the
    // cancel was already issued — no resend, still not terminal.
    let run = coord.drive(&run_id, &ctx).unwrap();
    assert_eq!(run.state, RunState::CancelRequested);
    assert_eq!(run.cancel_issued.len(), 1);
}
