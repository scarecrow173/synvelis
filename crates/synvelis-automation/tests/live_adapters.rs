//! Live adapter contract tests (AUTX-06 / synvelis#265).
//!
//! These tests drive [`GatewayStepLane`] / [`GatewayJobLane`] against a
//! real [`Gateway`] — real `parse_wire → bind → invoke`, the real
//! idempotency ledger, and real registry/schema resolution — with a
//! permit-all evaluator and recording stub executors standing in for
//! production capabilities. The real `JobEngine`/`CommitGate`/`prepare`
//! wiring is covered by the headless e2e in `synvelis-gateway`.
// Test closures mirror the gateway's typed-envelope signatures.
#![allow(clippy::result_large_err)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use synvelis_automation::coordinator::{
    JobLane, JobObservation, OperationObservation, StepCall, StepLane,
};
use synvelis_automation::live::{GatewayJobLane, GatewayStepLane};
use synvelis_automation::refs::VersionedRef;
use synvelis_automation::{RegistrySnapshot, capture_registry_snapshot};
use synvelis_gateway::authorization::{AuthorizationDecision, AuthorizationStage, PolicyEvaluator};
use synvelis_gateway::capability::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityVersion, DataAccessSet, EffectKind,
    EffectSet, ExecutionContract, OperationDomain, PermissionKind, PermissionSet, TargetContract,
};
use synvelis_gateway::cli::{LocalCliAdapter, MemoryCliSessionStore};
use synvelis_gateway::clock::SystemClock;
use synvelis_gateway::commit::{CommitReceipt, RetryOutcome};
use synvelis_gateway::id::{AdapterId, GrantRef, IdempotencyKey, JobRef, OperationId, TaskRef};
use synvelis_gateway::job::{CancelLink, CancelResponse, JobPhase};
use synvelis_gateway::mcp_server::{CommitPort, NoCommitPort};
use synvelis_gateway::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome};
use synvelis_gateway::registry::CapabilityOperation;
use synvelis_gateway::request::{Consistency, OperationRequest, Principal, PrincipalKind};
use synvelis_gateway::schema::{FieldSchema, SchemaRef, SchemaVersion, ValueSchema};
use synvelis_gateway::{Gateway, ProposalRef};

// ── Test doubles ──────────────────────────────────────────────

/// Permits every stage — capability-existence tests, not policy tests.
struct PermitAll;

impl PolicyEvaluator for PermitAll {
    fn decide(
        &self,
        _req: &OperationRequest,
        _stage: AuthorizationStage,
        _capability: &CapabilityDescriptor,
    ) -> AuthorizationDecision {
        AuthorizationDecision::PermitForStage
    }
}

/// What a recording handler observed on the bound request.
#[derive(Clone, Debug)]
struct Seen {
    capability: String,
    project: String,
    args: Value,
    target: String,
    consistency: Consistency,
    task: Option<String>,
    grant: Option<String>,
    idempotency: Option<String>,
}

impl Seen {
    fn of(req: &OperationRequest) -> Self {
        Self {
            capability: req.capability().to_string(),
            project: req.project().to_tagged(),
            args: req.args().clone(),
            target: format!("{:?}", req.target()),
            consistency: req.consistency().clone(),
            task: req.task().map(|t| t.as_str().to_owned()),
            grant: req.grant().map(|g| g.as_str().to_owned()),
            idempotency: req.idempotency().map(|k| k.as_str().to_owned()),
        }
    }
}

/// A commit port with a fixed reconcile verdict (unit seam only).
struct FixedCommit(RetryOutcome);

impl CommitPort for FixedCommit {
    fn commit(&self, _proposal: &ProposalRef) -> Result<CommitReceipt, ErrorEnvelope> {
        Err(GatewayError::Internal {
            reason: "test commit port has no boundary".to_owned(),
        }
        .into())
    }

    fn reconcile(&self, _operation: OperationId) -> RetryOutcome {
        self.0.clone()
    }
}

// ── Builders ──────────────────────────────────────────────────

fn cli() -> Arc<LocalCliAdapter> {
    Arc::new(LocalCliAdapter::new(
        AdapterId::new(7),
        Box::new(MemoryCliSessionStore::new()),
        Arc::new(SystemClock),
    ))
}

fn agent() -> Principal {
    Principal::new(PrincipalKind::Agent, "wf-agent").expect("agent principal")
}

fn project_tag() -> String {
    format!("project-{:032x}", 1)
}

fn step_call(capability: &str, args: Value) -> StepCall {
    StepCall {
        capability: VersionedRef::new(capability, 1, 0),
        args,
        target: None,
        expected_revision: None,
        idempotency_key: IdempotencyKey::new("wf-key-1").expect("key"),
        policy_epoch: None,
        principal: agent(),
        project_id: project_tag(),
        task: None,
        grant: None,
    }
}

fn any_object() -> ValueSchema {
    ValueSchema::Object {
        fields: BTreeMap::new(),
        deny_unknown: false,
    }
}

fn closed_object(fields: &[(&str, ValueSchema)]) -> ValueSchema {
    ValueSchema::Object {
        fields: fields
            .iter()
            .map(|(name, schema)| ((*name).to_owned(), FieldSchema::required(schema.clone())))
            .collect(),
        deny_unknown: true,
    }
}

fn descriptor(
    id: &str,
    domain: OperationDomain,
    target: TargetContract,
    effects: &[EffectKind],
    permissions: &[PermissionKind],
    input: &str,
    output: &str,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: CapabilityId::new(id).expect("cap id"),
        version: CapabilityVersion::new(1, 0),
        domain,
        input_schema: SchemaRef::new(input, 1, 0),
        output_schema: SchemaRef::new(output, 1, 0),
        target,
        effect_bound: EffectSet::of(effects.iter().copied()),
        required_permissions: PermissionSet::of(permissions.iter().copied()),
        data_access: DataAccessSet::new(),
        execution: ExecutionContract::Sync,
        availability: Availability::Available,
    }
}

/// Registers `name@1.0` with the given schema.
fn register_schema(gateway: &mut Gateway, name: &str, schema: ValueSchema) {
    gateway
        .schemas_mut()
        .register(name, SchemaVersion::new(1, 0), schema)
        .expect("schema registration");
}

/// A recording handler: appends [`Seen`] and returns `outcome`.
#[allow(clippy::needless_pass_by_value)] // callers hand an owned outcome for clarity
fn recording(
    seen: &Arc<Mutex<Vec<Seen>>>,
    outcome: GatewayOutcome<Value>,
) -> Arc<dyn CapabilityOperation> {
    let seen = seen.clone();
    let outcome = outcome.clone();
    Arc::new(move |req: &OperationRequest| {
        seen.lock().expect("seen poisoned").push(Seen::of(req));
        Ok(outcome.clone())
    })
}

/// A gateway with the standard test schemas and the given capability
/// registrations applied through [`Gateway::register_capability`].
fn gateway_with(register: impl FnOnce(&mut Gateway)) -> Arc<Mutex<Gateway>> {
    let mut gateway = Gateway::with_evaluator(Box::new(PermitAll));
    register_schema(&mut gateway, "test.any.input", any_object());
    register_schema(&mut gateway, "test.any.output", any_object());
    register(&mut gateway);
    Arc::new(Mutex::new(gateway))
}

fn step_lane(gateway: &Arc<Mutex<Gateway>>, committer: Arc<dyn CommitPort>) -> GatewayStepLane {
    GatewayStepLane::new(gateway.clone(), cli(), committer, None)
}

// ── StepLane ──────────────────────────────────────────────────

#[test]
fn step_invoke_forwards_scope_through_real_wire_path() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let gateway = gateway_with(|g| {
        g.register_capability(
            descriptor(
                "synvelis.test.echo",
                OperationDomain::Query,
                TargetContract::None,
                &[EffectKind::Read],
                &[PermissionKind::ReadContext],
                "test.any.input",
                "test.any.output",
            ),
            Some(recording(
                &seen,
                GatewayOutcome::Completed {
                    result: json!({"ok": true}),
                },
            )),
        )
        .expect("register echo");
    });
    let lane = step_lane(&gateway, Arc::new(NoCommitPort));

    let mut call = step_call("synvelis.test.echo", json!({"note": "hi"}));
    call.task = Some(TaskRef::new("task-1").expect("task"));
    call.grant = Some(GrantRef::new("grant-1").expect("grant"));

    let outcome = lane.invoke(&call).expect("invoke");
    assert!(matches!(outcome, GatewayOutcome::Completed { .. }));

    let seen = seen.lock().expect("seen").clone();
    assert_eq!(seen.len(), 1);
    let req = &seen[0];
    assert_eq!(req.capability, "synvelis.test.echo@1.0");
    assert_eq!(req.project, project_tag());
    assert_eq!(req.args, json!({"note": "hi"}));
    assert_eq!(req.task.as_deref(), Some("task-1"));
    assert_eq!(req.grant.as_deref(), Some("grant-1"));
    assert_eq!(req.idempotency.as_deref(), Some("wf-key-1"));
    assert!(matches!(req.consistency, Consistency::Latest));
}

#[test]
fn step_invoke_mutation_requires_expected_revision_pin() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let gateway = gateway_with(|g| {
        g.register_capability(
            descriptor(
                "synvelis.test.mutate",
                OperationDomain::ProjectMutation,
                TargetContract::None,
                &[EffectKind::Write],
                &[PermissionKind::MutateProject],
                "test.any.input",
                "test.any.output",
            ),
            Some(recording(
                &seen,
                GatewayOutcome::Completed { result: json!({}) },
            )),
        )
        .expect("register mutate");
    });
    let lane = step_lane(&gateway, Arc::new(NoCommitPort));

    // No pin → typed failure before any wire bytes exist.
    let err = lane
        .invoke(&step_call("synvelis.test.mutate", json!({})))
        .expect_err("unpinned mutation must fail");
    assert!(matches!(err.code, GatewayError::MalformedRequest { .. }));
    assert!(seen.lock().expect("seen").is_empty());

    // Pinned → ExpectedRevision reaches the bound request.
    let mut call = step_call("synvelis.test.mutate", json!({}));
    call.expected_revision = Some(3);
    lane.invoke(&call).expect("pinned mutation");
    let seen = seen.lock().expect("seen").clone();
    assert_eq!(seen.len(), 1);
    match seen[0].consistency {
        Consistency::ExpectedRevision { revision } => assert_eq!(revision.0, 3),
        ref other => panic!("expected ExpectedRevision, got {other:?}"),
    }
}

#[test]
fn step_invoke_batch_wraps_as_prepare_operation() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let gateway = gateway_with(|g| {
        g.register_capability(
            descriptor(
                "synvelis.operation.prepare",
                OperationDomain::Proposal,
                TargetContract::None,
                &[EffectKind::Read],
                &[PermissionKind::ReadContext],
                "test.any.input",
                "test.any.output",
            ),
            Some(recording(
                &seen,
                GatewayOutcome::Completed { result: json!({}) },
            )),
        )
        .expect("register prepare");
    });
    let lane = step_lane(&gateway, Arc::new(NoCommitPort));

    let mut call = step_call(
        synvelis_automation::macros::BATCH_CAPABILITY_ID,
        json!({"ops": [{"capability": "synvelis.test.echo@1.0", "args": {}}]}),
    );
    call.expected_revision = Some(1);
    lane.invoke(&call).expect("batch invoke");

    let seen = seen.lock().expect("seen").clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].capability, "synvelis.operation.prepare@1.0");
    let operation = &seen[0].args["operation"];
    assert_eq!(operation["capability"]["id"], "synvelis.operation.batch");
    assert_eq!(
        operation["args"]["ops"][0]["capability"],
        "synvelis.test.echo@1.0"
    );
    assert!(matches!(
        seen[0].consistency,
        Consistency::ExpectedRevision { .. }
    ));
}

#[test]
fn step_invoke_duplicate_reconciles_through_operation_result() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let gateway = gateway_with(|g| {
        g.register_capability(
            descriptor(
                "synvelis.test.echo",
                OperationDomain::Query,
                TargetContract::None,
                &[EffectKind::Read],
                &[PermissionKind::ReadContext],
                "test.any.input",
                "test.any.output",
            ),
            Some(recording(
                &seen,
                GatewayOutcome::Completed {
                    result: json!({"ok": true}),
                },
            )),
        )
        .expect("register echo");
    });
    let lane = step_lane(&gateway, Arc::new(NoCommitPort));

    let call = step_call("synvelis.test.echo", json!({}));
    lane.invoke(&call).expect("first invoke");
    let err = lane.invoke(&call).expect_err("duplicate key");
    let GatewayError::DuplicateRequest { original } = err.code else {
        panic!("expected DuplicateRequest, got {:?}", err.code);
    };

    // The recorded keyed outcome is the reconcile answer — the
    // executor ran exactly once.
    match lane.operation_result(&original) {
        OperationObservation::Settled(GatewayOutcome::Completed { result }) => {
            assert_eq!(result, json!({"ok": true}));
        }
        other => panic!("expected Settled(Completed), got {other:?}"),
    }
    assert_eq!(seen.lock().expect("seen").len(), 1);
}

#[test]
fn step_operation_result_unknown_for_unrecorded_operation() {
    let gateway = gateway_with(|_| {});
    let lane = step_lane(&gateway, Arc::new(NoCommitPort));
    assert!(matches!(
        lane.operation_result(&OperationId::new(0xdead)),
        OperationObservation::Unknown
    ));
}

#[test]
fn step_operation_result_commit_reconcile_pending() {
    let gateway = gateway_with(|_| {});
    let lane = step_lane(&gateway, Arc::new(FixedCommit(RetryOutcome::InFlight)));
    assert!(matches!(
        lane.operation_result(&OperationId::new(9)),
        OperationObservation::Pending
    ));
}

#[test]
fn step_operation_result_commit_settled_is_operation_accepted() {
    // A recorded commit verdict proves the apply was admitted through
    // the commit gate — the *operation-level* outcome is `Accepted`
    // regardless of the commit verdict itself (Committed/Rejected/
    // Conflicted/Indeterminate are the commit port's truth, not the
    // step's). `Rejected` is used here because it is the determined
    // verdict most likely to be mis-mapped.
    let gateway = gateway_with(|_| {});
    let lane = step_lane(
        &gateway,
        Arc::new(FixedCommit(RetryOutcome::Settled(
            synvelis_gateway::commit::CommitOutcome::Rejected {
                reason: "policy refused".to_owned(),
            },
        ))),
    );
    assert!(matches!(
        lane.operation_result(&OperationId::new(9)),
        OperationObservation::Settled(GatewayOutcome::Accepted { operation })
            if operation == OperationId::new(9)
    ));
}

#[test]
fn step_invoke_human_principal_fails_closed() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let gateway = gateway_with(|g| {
        g.register_capability(
            descriptor(
                "synvelis.test.echo",
                OperationDomain::Query,
                TargetContract::None,
                &[EffectKind::Read],
                &[PermissionKind::ReadContext],
                "test.any.input",
                "test.any.output",
            ),
            Some(recording(
                &seen,
                GatewayOutcome::Completed { result: json!({}) },
            )),
        )
        .expect("register echo");
    });
    let lane = step_lane(&gateway, Arc::new(NoCommitPort));

    let mut call = step_call("synvelis.test.echo", json!({}));
    call.principal = Principal::new(PrincipalKind::Human, "alice").expect("human");
    let err = lane.invoke(&call).expect_err("human must fail closed");
    assert!(matches!(err.code, GatewayError::Authentication));
    assert!(seen.lock().expect("seen").is_empty());
}

#[test]
fn step_target_maps_resource_contract() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let gateway = gateway_with(|g| {
        g.schemas_mut()
            .register(
                "test.clip.input",
                SchemaVersion::new(1, 0),
                closed_object(&[("note", ValueSchema::String { max_len: None })]),
            )
            .expect("clip input schema");
        g.register_capability(
            descriptor(
                "synvelis.test.clip",
                OperationDomain::Query,
                TargetContract::Resource {
                    kind: synvelis_gateway::request::ResourceKind::Clip,
                },
                &[EffectKind::Read],
                &[PermissionKind::ReadContext],
                "test.clip.input",
                "test.any.output",
            ),
            Some(recording(
                &seen,
                GatewayOutcome::Completed { result: json!({}) },
            )),
        )
        .expect("register clip");
    });
    let lane = step_lane(&gateway, Arc::new(NoCommitPort));

    // Missing target → TargetRequired.
    let err = lane
        .invoke(&step_call("synvelis.test.clip", json!({"note": "x"})))
        .expect_err("target required");
    assert!(matches!(err.code, GatewayError::TargetRequired));

    // Wrong kind tag → typed resource-kind failure at bind.
    let mut call = step_call("synvelis.test.clip", json!({"note": "x"}));
    call.target = Some(json!(format!("track-{:032x}", 9)));
    let err = lane.invoke(&call).expect_err("wrong kind");
    assert!(matches!(err.code, GatewayError::WrongResourceKind { .. }));

    // Right shape → bound Resource target reaches the handler.
    let mut call = step_call("synvelis.test.clip", json!({"note": "x"}));
    call.target = Some(json!(format!("clip-{:032x}", 9)));
    lane.invoke(&call).expect("resource target");
    let seen = seen.lock().expect("seen").clone();
    assert_eq!(seen.len(), 1);
    assert!(seen[0].target.contains("Clip"), "{}", seen[0].target);
}

// ── JobLane ───────────────────────────────────────────────────

/// A job.get/job.cancel fake pair whose canned payloads live in cells.
fn job_gateway(get: &Arc<Mutex<Value>>, cancel: &Arc<Mutex<Value>>) -> Arc<Mutex<Gateway>> {
    let get_cell = get.clone();
    let cancel_cell = cancel.clone();
    gateway_with(|g| {
        for (id, cell) in [
            ("synvelis.job.get", get_cell),
            ("synvelis.job.cancel", cancel_cell),
        ] {
            g.schemas_mut()
                .register(
                    format!("test.{id}.input"),
                    SchemaVersion::new(1, 0),
                    closed_object(&[("operation", ValueSchema::String { max_len: None })]),
                )
                .expect("job input schema");
            g.schemas_mut()
                .register(
                    format!("test.{id}.output"),
                    SchemaVersion::new(1, 0),
                    any_object(),
                )
                .expect("job output schema");
            let cell = cell.clone();
            g.register_capability(
                descriptor(
                    id,
                    OperationDomain::AsyncOperation,
                    TargetContract::None,
                    &[EffectKind::Read],
                    &[PermissionKind::RunJob],
                    &format!("test.{id}.input"),
                    &format!("test.{id}.output"),
                ),
                Some(Arc::new(move |_req: &OperationRequest| {
                    Ok(GatewayOutcome::Completed {
                        result: cell.lock().expect("cell").clone(),
                    })
                })),
            )
            .expect("register job capability");
        }
    })
}

fn job_lane(gateway: &Arc<Mutex<Gateway>>) -> GatewayJobLane {
    GatewayJobLane::new(
        gateway.clone(),
        cli(),
        agent(),
        project_tag(),
        Some(TaskRef::new("task-1").expect("task")),
        Some(GrantRef::new("grant-1").expect("grant")),
    )
}

#[test]
fn job_observe_maps_live_payload() {
    let get = Arc::new(Mutex::new(json!({
        "phase": "running",
        "progress": 42,
        "job": "job-abc",
        "cancel": "requested",
    })));
    let cancel = Arc::new(Mutex::new(json!({})));
    let lane = job_lane(&job_gateway(&get, &cancel));

    match lane.observe(&OperationId::new(42)) {
        JobObservation::Live {
            phase,
            progress,
            result,
            job,
            cancel,
        } => {
            assert_eq!(phase, JobPhase::Running);
            assert_eq!(progress, Some(42));
            assert_eq!(job, Some(JobRef::new("job-abc").expect("job ref")));
            assert_eq!(cancel, CancelLink::Requested);
            assert!(result.is_none(), "non-terminal jobs disclose no result");
        }
        other @ JobObservation::Unknown => panic!("expected Live, got {other:?}"),
    }
}

#[test]
fn job_observe_completed_carries_disclosed_result() {
    let payload = json!({"phase": "completed", "cancel": "none", "out": {"a": 1}});
    let get = Arc::new(Mutex::new(payload.clone()));
    let cancel = Arc::new(Mutex::new(json!({})));
    let lane = job_lane(&job_gateway(&get, &cancel));

    match lane.observe(&OperationId::new(7)) {
        JobObservation::Live { phase, result, .. } => {
            assert_eq!(phase, JobPhase::Completed);
            assert_eq!(result, Some(payload));
        }
        other @ JobObservation::Unknown => panic!("expected Live, got {other:?}"),
    }
}

#[test]
fn job_observe_unparseable_phase_is_unknown_never_guessed() {
    let get = Arc::new(Mutex::new(json!({"phase": "surprising", "cancel": "none"})));
    let cancel = Arc::new(Mutex::new(json!({})));
    let lane = job_lane(&job_gateway(&get, &cancel));
    assert!(matches!(
        lane.observe(&OperationId::new(1)),
        JobObservation::Unknown
    ));
}

#[test]
fn job_cancel_maps_authoritative_verdicts() {
    let get = Arc::new(Mutex::new(json!({})));
    let cancel = Arc::new(Mutex::new(json!({"cancel": "confirmed"})));
    let lane = job_lane(&job_gateway(&get, &cancel));
    let op = OperationId::new(3);

    assert_eq!(lane.request_cancel(&op), CancelResponse::Confirmed);

    *cancel.lock().expect("cell") = json!({"cancel": "requested"});
    assert_eq!(lane.request_cancel(&op), CancelResponse::Pending);

    *cancel.lock().expect("cell") = json!({"cancel": "unconfirmed"});
    assert_eq!(lane.request_cancel(&op), CancelResponse::Unreachable);

    // Settle race — the carried phase is the job's actual truth.
    *cancel.lock().expect("cell") = json!({"cancel": "not-cancellable", "phase": "completed"});
    assert_eq!(
        lane.request_cancel(&op),
        CancelResponse::Settled {
            phase: JobPhase::Completed
        }
    );
}

#[test]
fn job_lane_without_job_capabilities_is_honestly_unknown() {
    // A gateway that never installed the job domain: observe must not
    // fabricate liveness, cancel must not fabricate a verdict.
    let gateway = gateway_with(|_| {});
    let lane = job_lane(&gateway);
    let op = OperationId::new(5);
    assert!(matches!(lane.observe(&op), JobObservation::Unknown));
    assert_eq!(lane.request_cancel(&op), CancelResponse::Unreachable);
}

// ── Registry capture ──────────────────────────────────────────

#[test]
fn registry_capture_projects_live_metadata() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let gateway = gateway_with(|g| {
        g.register_capability(
            descriptor(
                "synvelis.test.echo",
                OperationDomain::Query,
                TargetContract::None,
                &[EffectKind::Read],
                &[PermissionKind::ReadContext],
                "test.any.input",
                "test.any.output",
            ),
            Some(recording(
                &seen,
                GatewayOutcome::Completed { result: json!({}) },
            )),
        )
        .expect("register echo");
        g.register_capability(
            descriptor(
                "synvelis.test.mutate",
                OperationDomain::ProjectMutation,
                TargetContract::Create {
                    parent: synvelis_gateway::request::ResourceKind::Project,
                    creates: synvelis_gateway::request::ResourceKind::Track,
                },
                &[EffectKind::Create],
                &[PermissionKind::MutateProject],
                "test.any.input",
                "test.any.output",
            ),
            None,
        )
        .expect("register mutate");
    });

    let snapshot: RegistrySnapshot =
        capture_registry_snapshot(&gateway.lock().expect("gw")).expect("capture");

    let echo = snapshot
        .capabilities
        .iter()
        .find(|c| c.id == "synvelis.test.echo")
        .expect("echo descriptor");
    assert_eq!(echo.version.major, 1);
    assert_eq!(echo.version.minor, 0);
    assert_eq!(
        echo.domain,
        synvelis_automation::registry::OperationDomain::Query
    );
    assert_eq!(echo.input_schema.id, "test.any.input");

    let mutate = snapshot
        .capabilities
        .iter()
        .find(|c| c.id == "synvelis.test.mutate")
        .expect("mutate descriptor");
    assert!(matches!(
        mutate.target,
        synvelis_automation::registry::TargetContract::Create { .. }
    ));

    assert!(
        snapshot
            .schemas
            .iter()
            .any(|s| s.name == "test.any.input" && s.version.major == 1)
    );
}
