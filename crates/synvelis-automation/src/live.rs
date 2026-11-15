//! Live authority adapters — the production [`StepLane`] / [`JobLane`]
//! implementations that connect the headless workflow coordinator to a
//! real [`Gateway`] (AUTX-06 / synvelis#265).
//!
//! Authority boundary (unchanged from the coordinator contract):
//!
//! - [`GatewayStepLane`] turns each [`StepCall`] into a wire request and
//!   runs it through `Gateway::handle` — the same parse → bind → invoke
//!   path every trusted adapter takes. It mints the caller through a
//!   [`LocalCliAdapter`] (delegated agent-equivalent identity only; a
//!   `Human` run principal cannot be attested without a verified-session
//!   credential the step call does not carry, so it fails closed with
//!   [`GatewayError::Authentication`] rather than silently downgrading).
//! - [`GatewayJobLane`] reads job truth exclusively through the
//!   `synvelis.job.get` / `synvelis.job.cancel` capabilities — the same
//!   caller-scoped, disclosure-checked wire path an agent uses. It never
//!   inspects [`JobEngine::record`] or provider state directly, so the
//!   existence-blind scope rule and the honest cancel verdicts are the
//!   authority's own answers.
//! - [`GatewayStepLane::operation_result`] resolves `DuplicateRequest`
//!   reconciles through [`Gateway::operation_outcome`] (the recorded
//!   keyed outcome), then [`CommitPort::reconcile`] for commit-bound
//!   operations, then [`JobEngine::record`] for admitted job submits.
//!   An operation with no authoritative record is
//!   [`OperationObservation::Unknown`] — never a fabricated `Settled`.
//!
//! Both lanes are pull-based: `invoke`/`observe`/`request_cancel`
//! return before the next coordinator step begins, and neither holds a
//! gateway lock across calls — the [`Mutex`] is taken per dispatch only,
//! so a lane can never re-enter the gateway while holding commit or
//! project locks (the coordinator's sequential drive contract).

// The whole module is the typed-envelope boundary: every helper
// returns the gateway's `ErrorEnvelope` verbatim, and boxing it at
// each internal signature would only obscure the contract (the lane
// trait itself already boxes — see `StepLane::invoke`).
#![allow(clippy::result_large_err)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use synvelis_gateway::Gateway;
use synvelis_gateway::capability::{
    CapabilityDescriptor, CapabilityId, CapabilityRef, CapabilityVersion, OperationDomain,
    TargetContract,
};
use synvelis_gateway::cli::LocalCliAdapter;
use synvelis_gateway::commit::RetryOutcome;
use synvelis_gateway::id::{GrantRef, JobRef, OperationId, RequestId, TaskRef};
use synvelis_gateway::job::{CancelLink, CancelResponse, JobPhase};
use synvelis_gateway::job_engine::{JOB_CANCEL_ID, JOB_GET_ID, JobEngine};
use synvelis_gateway::mcp_server::CommitPort;
use synvelis_gateway::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome};
use synvelis_gateway::request::{CallerIdentity, Principal, PrincipalKind, ResourceKind};

use crate::coordinator::{JobLane, JobObservation, OperationObservation, StepCall, StepLane};
use crate::macros::BATCH_CAPABILITY_ID;
use crate::refs::VersionedRef;

/// The `synvelis.operation.prepare` capability every compiled atomic
/// macro is lowered through — `synvelis.operation.batch` is a named
/// operation *inside* a prepare request, never a directly invocable
/// capability (it registers with no executor).
const PREPARE_CAPABILITY_ID: &str = "synvelis.operation.prepare";

/// Mints the trusted caller a workflow step runs under.
///
/// `Agent` principals attest through the local CLI adapter's delegated
/// path — the same agent-equivalent `LocalCli` caller an unattended
/// `CliSessionClaim::Unattended` produces. `Human` principals fail with
/// [`GatewayError::Authentication`]: a step call carries no session
/// credential, so this slice cannot attest interactive authority and
/// must not downgrade it to agent scope.
fn attest(cli: &LocalCliAdapter, principal: &Principal) -> Result<CallerIdentity, ErrorEnvelope> {
    match principal.kind() {
        PrincipalKind::Agent => cli
            .caller_delegated(principal.id())
            .map_err(ErrorEnvelope::from),
        PrincipalKind::Human => Err(GatewayError::Authentication.into()),
    }
}

/// The kebab-case resource-kind name a [`WireTarget`]-shaped JSON
/// object carries — mirrors `ResourceRef::kind_of_tagged`'s accepted
/// tags exactly.
fn kind_name(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Project => "project",
        ResourceKind::Track => "track",
        ResourceKind::Lane => "lane",
        ResourceKind::Clip => "clip",
        ResourceKind::Source => "source",
        ResourceKind::Asset => "asset",
        ResourceKind::Note => "note",
        ResourceKind::TakeGroup => "take-group",
        ResourceKind::Comp => "comp",
        ResourceKind::ParameterTarget => "param",
        ResourceKind::AutomationLane => "automation-lane",
        ResourceKind::Strip => "strip",
        ResourceKind::Send => "send",
        ResourceKind::Processor => "processor",
        ResourceKind::VariantSet => "variant-set",
        ResourceKind::Branch => "branch",
    }
}

/// Maps a step's evaluated `target` value onto the wire target shape
/// the descriptor's [`TargetContract`] declares.
///
/// The workflow AST expresses targets as bare tagged ids: a single id
/// string for `Resource`/`Create` (the parent for `Create`), an array
/// of id strings for `ResourceSet`. Anything else is a typed failure —
/// the lane never guesses a shape.
fn target_wire(
    contract: TargetContract,
    target: Option<&Value>,
) -> Result<Option<Value>, ErrorEnvelope> {
    match (contract, target) {
        (TargetContract::None, None) => Ok(None),
        (TargetContract::None, Some(_)) => Err(GatewayError::UnexpectedTarget.into()),
        (_, None) => Err(GatewayError::TargetRequired.into()),
        (TargetContract::Resource { kind }, Some(Value::String(id))) => Ok(Some(json!({
            "shape": "resource",
            "kind": kind_name(kind),
            "id": id,
        }))),
        (TargetContract::ResourceSet { kind }, Some(Value::Array(ids))) => {
            let mut out = Vec::with_capacity(ids.len());
            for id in ids {
                let Some(id) = id.as_str() else {
                    return Err(GatewayError::MalformedRequest {
                        reason: "resource-set target entries must be tagged id strings".to_owned(),
                    }
                    .into());
                };
                out.push(id);
            }
            Ok(Some(json!({
                "shape": "resource-set",
                "kind": kind_name(kind),
                "ids": out,
            })))
        }
        (TargetContract::Create { parent, creates }, Some(Value::String(parent_id))) => {
            Ok(Some(json!({
                "shape": "create",
                "parent_kind": kind_name(parent),
                "parent": parent_id,
                "creates": kind_name(creates),
            })))
        }
        _ => Err(GatewayError::MalformedRequest {
            reason: "step target does not match the capability's target contract".to_owned(),
        }
        .into()),
    }
}

/// The domain-appropriate consistency for a step call. `expected_revision`
/// is the CAS pin the coordinator journaled on `StepBegan`; mutation and
/// proposal domains carry it, read/async domains never do. A mutation
/// without a pin cannot form a legal wire request — that is a caller
/// contract violation, reported as a typed failure rather than a guessed
/// consistency.
fn consistency_wire(
    domain: OperationDomain,
    expected_revision: Option<u64>,
) -> Result<Value, ErrorEnvelope> {
    Ok(match domain {
        OperationDomain::Query => json!({ "kind": "latest" }),
        OperationDomain::Proposal => match expected_revision {
            Some(revision) => json!({ "kind": "expected-revision", "revision": revision }),
            None => json!({ "kind": "latest" }),
        },
        OperationDomain::ProjectMutation => match expected_revision {
            Some(revision) => json!({ "kind": "expected-revision", "revision": revision }),
            None => {
                return Err(GatewayError::MalformedRequest {
                    reason: "project-mutation capability requires an expected revision pin"
                        .to_owned(),
                }
                .into());
            }
        },
        OperationDomain::RuntimeControl | OperationDomain::AsyncOperation => {
            json!({ "kind": "unconditional" })
        }
    })
}

/// Resolves the exact capability descriptor from the live registry —
/// never a "latest" fallback; an unregistered ref is a typed
/// [`GatewayError`] before any wire bytes exist.
fn describe<'a>(
    gateway: &'a Gateway,
    capability: &CapabilityRef,
) -> Result<&'a CapabilityDescriptor, ErrorEnvelope> {
    gateway
        .registry()
        .describe(capability)
        .map_err(ErrorEnvelope::from)
}

fn capability_ref(versioned: &VersionedRef) -> Result<CapabilityRef, ErrorEnvelope> {
    Ok(CapabilityRef {
        id: CapabilityId::new(versioned.id.clone()).map_err(ErrorEnvelope::from)?,
        version: CapabilityVersion::new(versioned.version.major, versioned.version.minor),
    })
}

fn job_capability_ref(id: &str) -> Result<CapabilityRef, ErrorEnvelope> {
    Ok(CapabilityRef {
        id: CapabilityId::new(id).map_err(ErrorEnvelope::from)?,
        version: CapabilityVersion::new(1, 0),
    })
}

/// Builds the wire JSON for one [`StepCall`].
///
/// `synvelis.operation.batch` calls are wrapped as a
/// `synvelis.operation.prepare` request whose `operation` names the
/// batch — the only legal wire form, since the batch capability itself
/// has no executor. Every other capability is sent verbatim with its
/// registry-declared input schema, domain-derived consistency, and the
/// step's explicit target.
fn step_wire(
    gateway: &Gateway,
    call: &StepCall,
    request_id: &RequestId,
) -> Result<Value, ErrorEnvelope> {
    let (capability, descriptor, args, target) = if call.capability.id == BATCH_CAPABILITY_ID {
        if call.target.is_some() {
            return Err(GatewayError::MalformedRequest {
                reason: "atomic-macro batch takes no target".to_owned(),
            }
            .into());
        }
        let capability = job_capability_ref(PREPARE_CAPABILITY_ID)?;
        let descriptor = describe(gateway, &capability)?;
        let batch = CapabilityRef {
            id: CapabilityId::new(BATCH_CAPABILITY_ID).map_err(ErrorEnvelope::from)?,
            version: CapabilityVersion::new(
                call.capability.version.major,
                call.capability.version.minor,
            ),
        };
        let operation = json!({
            "capability": batch,
            "args": call.args,
        });
        (
            capability,
            descriptor,
            json!({ "operation": operation }),
            None,
        )
    } else {
        let capability = capability_ref(&call.capability)?;
        let descriptor = describe(gateway, &capability)?;
        let target = target_wire(descriptor.target, call.target.as_ref())?;
        (capability, descriptor, call.args.clone(), target)
    };

    let schema = serde_json::to_value(&descriptor.input_schema).map_err(|e| {
        ErrorEnvelope::from(GatewayError::Internal {
            reason: format!("input schema is not wire-representable: {e}"),
        })
    })?;
    let consistency = consistency_wire(descriptor.domain, call.expected_revision)?;
    let mut wire = json!({
        "request_id": request_id,
        "capability": capability,
        "schema": schema,
        "project": call.project_id,
        "args": args,
        "consistency": consistency,
        "idempotency_key": call.idempotency_key.as_str(),
    });
    if let Some(target) = target {
        wire["target"] = target;
    }
    if let Some(task) = &call.task {
        wire["task"] = json!(task.as_str());
    }
    if let Some(grant) = &call.grant {
        wire["grant"] = json!(grant.as_str());
    }
    Ok(wire)
}

/// The live [`StepLane`] — `StepCall` → wire JSON → `Gateway::handle`,
/// plus the authoritative `DuplicateRequest` reconcile.
pub struct GatewayStepLane {
    gateway: Arc<Mutex<Gateway>>,
    cli: Arc<LocalCliAdapter>,
    committer: Arc<dyn CommitPort>,
    jobs: Option<Arc<JobEngine>>,
    next_request: AtomicU64,
}

impl GatewayStepLane {
    /// Composes the lane over the shared gateway, the CLI adapter that
    /// attests run principals, the commit boundary used for
    /// commit-bound operation reconciles, and (optionally) the job
    /// engine whose book proves admitted submits.
    #[must_use]
    pub fn new(
        gateway: Arc<Mutex<Gateway>>,
        cli: Arc<LocalCliAdapter>,
        committer: Arc<dyn CommitPort>,
        jobs: Option<Arc<JobEngine>>,
    ) -> Self {
        Self {
            gateway,
            cli,
            committer,
            jobs,
            next_request: AtomicU64::new(0),
        }
    }

    fn request_id(&self) -> Result<RequestId, ErrorEnvelope> {
        let n = self.next_request.fetch_add(1, Ordering::Relaxed);
        RequestId::new(format!("wf-req-{n:016x}")).map_err(ErrorEnvelope::from)
    }
}

impl StepLane for GatewayStepLane {
    fn invoke(&self, call: &StepCall) -> Result<GatewayOutcome<Value>, Box<ErrorEnvelope>> {
        let caller = attest(&self.cli, &call.principal).map_err(Box::new)?;
        let request_id = self.request_id().map_err(Box::new)?;
        let mut gateway = self.gateway.lock().expect("gateway poisoned");
        let wire = step_wire(&gateway, call, &request_id).map_err(Box::new)?;
        gateway.handle(&wire, caller).map_err(Box::new)
    }

    fn operation_result(&self, operation: &OperationId) -> OperationObservation {
        // 1. The recorded keyed outcome — covers every successfully
        //    invoked idempotent operation (queries, prepares, submits,
        //    applies). A miss is honest: evicted or never recorded.
        if let Some(outcome) = self
            .gateway
            .lock()
            .expect("gateway poisoned")
            .operation_outcome(*operation)
        {
            return OperationObservation::Settled(outcome);
        }
        // 2. Commit-bound operations reconcile through the commit
        //    gate's durable ledger — `InFlight` is pending, a settled
        //    commit proves the apply was admitted (its operation-level
        //    outcome is `Accepted`; the commit verdict itself is the
        //    commit port's truth, not the step's).
        match self.committer.reconcile(*operation) {
            RetryOutcome::InFlight => return OperationObservation::Pending,
            RetryOutcome::Settled(_) => {
                return OperationObservation::Settled(GatewayOutcome::Accepted {
                    operation: *operation,
                });
            }
            RetryOutcome::Unknown => {}
        }
        // 3. An admitted job submit leaves a book record even after the
        //    gateway ledger evicts — the record's existence proves the
        //    `Accepted` admission outcome; job liveness is JobLane's
        //    read, not this one.
        if self
            .jobs
            .as_ref()
            .is_some_and(|engine| engine.record(*operation).is_some())
        {
            return OperationObservation::Settled(GatewayOutcome::Accepted {
                operation: *operation,
            });
        }
        OperationObservation::Unknown
    }
}

/// Parses the `job.get` result payload into a [`JobObservation`].
///
/// Every field is the authority's own disclosure — an unparseable
/// phase/cancel verdict yields [`JobObservation::Unknown`] rather than
/// a guessed live state. `result` carries the disclosed payload only on
/// `Completed`; a non-terminal or failed job discloses no result.
fn job_observation(result: &Value) -> JobObservation {
    let failure = result.get("failure").and_then(Value::as_str);
    let phase = match result.get("phase").and_then(Value::as_str) {
        Some("pending") => JobPhase::Pending,
        Some("running") => JobPhase::Running,
        Some("completed") => JobPhase::Completed,
        Some("failed") => JobPhase::Failed {
            reason: failure.unwrap_or("provider failure").to_owned(),
        },
        Some("cancelled") => JobPhase::Cancelled,
        _ => return JobObservation::Unknown,
    };
    let cancel = match result.get("cancel").and_then(Value::as_str) {
        Some("none") => CancelLink::NotRequested,
        Some("requested") => CancelLink::Requested,
        Some("confirmed") => CancelLink::Confirmed,
        Some("unconfirmed") => CancelLink::Unconfirmed,
        _ => return JobObservation::Unknown,
    };
    let job = result
        .get("job")
        .and_then(Value::as_str)
        .and_then(|s| JobRef::new(s).ok());
    let progress = result.get("progress").and_then(Value::as_u64);
    let payload = (phase == JobPhase::Completed).then(|| result.clone());
    JobObservation::Live {
        phase,
        progress,
        result: payload,
        job,
        cancel,
    }
}

/// Parses a `job.cancel` result payload into a [`CancelResponse`].
///
/// `not-cancellable` is the settle race — the carried phase is the
/// job's actual truth. `unconfirmed` and any unrecognized verdict
/// degrade to [`CancelResponse::Unreachable`]: the honest
/// non-committal, never an upgraded verdict.
fn cancel_response(result: &Value) -> CancelResponse {
    match result.get("cancel").and_then(Value::as_str) {
        Some("confirmed") => CancelResponse::Confirmed,
        Some("requested") => CancelResponse::Pending,
        Some("not-cancellable") => match result.get("phase").and_then(Value::as_str) {
            Some("pending") => CancelResponse::Settled {
                phase: JobPhase::Pending,
            },
            Some("running") => CancelResponse::Settled {
                phase: JobPhase::Running,
            },
            Some("completed") => CancelResponse::Settled {
                phase: JobPhase::Completed,
            },
            Some("failed") => CancelResponse::Settled {
                phase: JobPhase::Failed {
                    reason: "provider failure".to_owned(),
                },
            },
            Some("cancelled") => CancelResponse::Settled {
                phase: JobPhase::Cancelled,
            },
            _ => CancelResponse::Unreachable,
        },
        _ => CancelResponse::Unreachable,
    }
}

/// The live [`JobLane`] — `observe`/`request_cancel` through the
/// `synvelis.job.get`/`synvelis.job.cancel` wire capabilities under the
/// run's own principal/project/task/grant scope.
///
/// Scope is fixed at construction: the lane serves exactly the run
/// tuple it was built for, so a job submitted under a different
/// (principal, project, task, grant) is invisible to it — the same
/// existence-blind rule the job book enforces for every caller.
pub struct GatewayJobLane {
    gateway: Arc<Mutex<Gateway>>,
    cli: Arc<LocalCliAdapter>,
    principal: Principal,
    project: String,
    task: Option<TaskRef>,
    grant: Option<GrantRef>,
    next_request: AtomicU64,
}

impl GatewayJobLane {
    /// Composes the lane for one run's authority scope.
    #[must_use]
    pub fn new(
        gateway: Arc<Mutex<Gateway>>,
        cli: Arc<LocalCliAdapter>,
        principal: Principal,
        project_id: String,
        task: Option<TaskRef>,
        grant: Option<GrantRef>,
    ) -> Self {
        Self {
            gateway,
            cli,
            principal,
            project: project_id,
            task,
            grant,
            next_request: AtomicU64::new(0),
        }
    }

    fn request_id(&self) -> Result<RequestId, ErrorEnvelope> {
        let n = self.next_request.fetch_add(1, Ordering::Relaxed);
        RequestId::new(format!("wf-job-{n:016x}")).map_err(ErrorEnvelope::from)
    }

    /// Builds the `job.get`/`job.cancel` wire request — the capability's
    /// registry-declared input schema and `Unconditional` consistency
    /// (the job domain is `AsyncOperation`).
    fn job_wire(
        &self,
        gateway: &Gateway,
        capability_id: &str,
        operation: &OperationId,
    ) -> Result<Value, ErrorEnvelope> {
        let capability = job_capability_ref(capability_id)?;
        let descriptor = describe(gateway, &capability)?;
        let request_id = self.request_id()?;
        let schema = serde_json::to_value(&descriptor.input_schema).map_err(|e| {
            ErrorEnvelope::from(GatewayError::Internal {
                reason: format!("input schema is not wire-representable: {e}"),
            })
        })?;
        let mut wire = json!({
            "request_id": request_id,
            "capability": capability,
            "schema": schema,
            "project": self.project,
            "args": { "operation": operation.to_tagged() },
            "consistency": { "kind": "unconditional" },
        });
        if let Some(task) = &self.task {
            wire["task"] = json!(task.as_str());
        }
        if let Some(grant) = &self.grant {
            wire["grant"] = json!(grant.as_str());
        }
        Ok(wire)
    }

    /// Dispatches one job wire request under the run's caller.
    ///
    /// Every failure — attestation, transport, typed denial — collapses
    /// to the caller-visible "no honest answer" form: `observe` yields
    /// [`JobObservation::Unknown`], `request_cancel` yields
    /// [`CancelResponse::Unreachable`]. The job book's existence-blind
    /// scope already answers foreign/unknown operations with `Denied`,
    /// which is exactly `Unknown` here.
    fn dispatch(&self, capability_id: &str, operation: &OperationId) -> Option<Value> {
        let caller = attest(&self.cli, &self.principal).ok()?;
        let mut gateway = self.gateway.lock().expect("gateway poisoned");
        let wire = self.job_wire(&gateway, capability_id, operation).ok()?;
        match gateway.handle(&wire, caller) {
            Ok(GatewayOutcome::Completed { result }) => Some(result),
            _ => None,
        }
    }
}

impl JobLane for GatewayJobLane {
    fn observe(&self, operation: &OperationId) -> JobObservation {
        self.dispatch(JOB_GET_ID, operation)
            .as_ref()
            .map_or(JobObservation::Unknown, job_observation)
    }

    fn request_cancel(&self, operation: &OperationId) -> CancelResponse {
        self.dispatch(JOB_CANCEL_ID, operation)
            .as_ref()
            .map_or(CancelResponse::Unreachable, cancel_response)
    }
}
