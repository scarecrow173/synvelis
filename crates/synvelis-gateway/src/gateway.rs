//! The typed invocation boundary.
//!
//! [`Gateway`] composes the registry, schema store, policy evaluator,
//! operation-id generator, and the in-memory idempotency ledger into
//! the three-step invocation path:
//!
//! ```text
//! json ──▶ parse_wire ──▶ WireRequest          (shape + forbidden keys)
//!      ──▶ bind(caller) ──▶ OperationRequest   (resolve + validate + mint)
//!      ──▶ invoke       ──▶ GatewayOutcome     (authorize + execute)
//! ```
//!
//! `bind` and `invoke` are separate on purpose: validation must not
//! execute, and execution requires a successfully bound request. The
//! gateway itself never mutates a project — it owns no project handle
//! (GT-010); executors that need the commit boundary (GW-06) receive it
//! through the operation handler seam, not through this type.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;
use synvelis_project::{CounterIdGenerator, IdGenerator, ProjectId, ProjectRevision};

use crate::authorization::{
    AuthorizationDecision, AuthorizationStage, DenyAllPolicy, PolicyEvaluator,
};
use crate::capability::{Availability, CapabilityDescriptor, OperationDomain, TargetContract};
use crate::id::{ApprovalRef, GrantRef, IdempotencyKey, OperationId, SnapshotRef, TaskRef};
use crate::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome};
use crate::registry::{CapabilityOperation, CapabilityRegistry};
use crate::request::{CallerIdentity, Consistency, OperationRequest, OperationTarget, ResourceRef};
use crate::schema::SchemaRegistry;
use crate::wire::{WireConsistency, WireRequest, WireTarget};

/// Per-domain authorization-stage mapping.
#[must_use]
const fn stage_for(domain: OperationDomain) -> AuthorizationStage {
    match domain {
        OperationDomain::Query => AuthorizationStage::ReadContext,
        OperationDomain::ProjectMutation => AuthorizationStage::RequestApply,
        OperationDomain::RuntimeControl => AuthorizationStage::StartRuntimeOperation,
        OperationDomain::AsyncOperation => AuthorizationStage::StartAsyncOperation,
    }
}

/// Ledger key: idempotency keys are scoped to (principal kind + id +
/// adapter) + project — a `Human` and an `Agent` named alike, or two
/// adapters attesting the same id string, never share a dedup scope.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct LedgerKey {
    principal_kind: crate::request::PrincipalKind,
    principal_id: String,
    adapter: crate::id::AdapterId,
    project: ProjectId,
    key: IdempotencyKey,
}

struct LedgerEntry {
    fingerprint: String,
    operation: OperationId,
}

/// Maximum retained entries — oldest-first eviction keeps the ledger
/// bounded (durable, `TTLed` records are GW-06's; this is a contract
/// guard, not a store).
const LEDGER_CAP: usize = 4096;

/// In-memory idempotency ledger (GW-01 contract scope).
///
/// Same key + same payload fingerprint → duplicate. Same key +
/// different fingerprint → conflict. Entries are recorded only after a
/// successful execution — a denied, failed, or never-run request never
/// burns its key, so `original` always names an operation that actually
/// ran. Durable records, replay of stored outcomes, and crash recovery
/// belong to GW-06 (ACG-004).
#[derive(Default)]
struct IdempotencyLedger {
    entries: BTreeMap<LedgerKey, LedgerEntry>,
    order: std::collections::VecDeque<LedgerKey>,
}

impl IdempotencyLedger {
    /// Checks `key` without recording: duplicate payloads name the
    /// operation that actually ran.
    fn check(&self, key: &LedgerKey, fingerprint: &str) -> Result<(), GatewayError> {
        match self.entries.get(key) {
            Some(entry) if entry.fingerprint == fingerprint => {
                Err(GatewayError::DuplicateRequest {
                    original: entry.operation,
                })
            }
            Some(_) => Err(GatewayError::IdempotencyConflict),
            None => Ok(()),
        }
    }

    /// Records a completed operation under `key` — called only after a
    /// handler returned success.
    fn record(&mut self, key: LedgerKey, fingerprint: String, operation: OperationId) {
        if self.entries.contains_key(&key) {
            return;
        }
        if self.entries.len() >= LEDGER_CAP
            && let Some(oldest) = self.order.pop_front()
        {
            self.entries.remove(&oldest);
        }
        self.order.push_back(key.clone());
        self.entries.insert(
            key,
            LedgerEntry {
                fingerprint,
                operation,
            },
        );
    }
}

/// The typed invocation boundary.
pub struct Gateway {
    registry: CapabilityRegistry,
    schemas: SchemaRegistry,
    evaluator: Box<dyn PolicyEvaluator>,
    ids: CounterIdGenerator,
    ledger: IdempotencyLedger,
}

// Error envelopes are the API contract — typed, sized by the error
// enum, and returned on the cold path; boxing would only obscure it.
#[allow(clippy::result_large_err)]
impl Gateway {
    /// A gateway with an empty registry, empty schema store, and the
    /// fail-closed [`DenyAllPolicy`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_evaluator(Box::new(DenyAllPolicy))
    }

    /// A gateway with an explicit policy evaluator.
    #[must_use]
    pub fn with_evaluator(evaluator: Box<dyn PolicyEvaluator>) -> Self {
        Self {
            registry: CapabilityRegistry::new(),
            schemas: SchemaRegistry::new(),
            evaluator,
            ids: CounterIdGenerator::new(1),
            ledger: IdempotencyLedger::default(),
        }
    }

    /// Mutable access to the capability registry (registration is an
    /// administrative act, not an invocation).
    pub fn registry_mut(&mut self) -> &mut CapabilityRegistry {
        &mut self.registry
    }

    /// Mutable access to the schema registry.
    pub fn schemas_mut(&mut self) -> &mut SchemaRegistry {
        &mut self.schemas
    }

    /// Read access to the capability registry (discovery metadata).
    #[must_use]
    pub fn registry(&self) -> &CapabilityRegistry {
        &self.registry
    }

    /// Replaces the policy evaluator.
    pub fn set_evaluator(&mut self, evaluator: Box<dyn PolicyEvaluator>) {
        self.evaluator = evaluator;
    }

    /// Registers a capability descriptor and optional executor.
    ///
    /// Registration is a coherence check, not just an insert: the
    /// declared input/output schemas must resolve in the schema store
    /// (no `Available` capability may be uninvocable by construction),
    /// and a `Query` descriptor may not declare mutating effects — the
    /// domain drives the authorization stage, so an incoherent domain
    /// would misroute the permission check.
    ///
    /// # Errors
    /// - [`GatewayError::CapabilityConflict`] — duplicate id+version.
    /// - [`GatewayError::UnknownSchema`] /
    ///   [`GatewayError::UnsupportedSchemaVersion`] — declared schemas
    ///   not yet registered.
    /// - [`GatewayError::IncoherentDescriptor`] — domain/effect
    ///   contradiction.
    pub fn register_capability(
        &mut self,
        descriptor: CapabilityDescriptor,
        handler: Option<Arc<dyn CapabilityOperation>>,
    ) -> Result<(), ErrorEnvelope> {
        self.schemas.resolve(&descriptor.input_schema)?;
        self.schemas.resolve(&descriptor.output_schema)?;
        if matches!(descriptor.domain, OperationDomain::Query)
            && descriptor.effect_bound.iter().any(|e| {
                matches!(
                    e,
                    crate::capability::EffectKind::Write
                        | crate::capability::EffectKind::Create
                        | crate::capability::EffectKind::Delete
                )
            })
        {
            return Err(GatewayError::IncoherentDescriptor {
                id: descriptor.id.to_string(),
                reason: "query capabilities may not declare mutating effects".to_owned(),
            }
            .into());
        }
        self.registry
            .register(descriptor, handler)
            .map_err(Into::into)
    }

    /// Step 1 — parse raw JSON into a [`WireRequest`].
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — `SelfDeclaredAuthority`,
    /// `ImplicitTargetRejected`, or `MalformedRequest`.
    pub fn parse_wire(value: &Value) -> Result<WireRequest, ErrorEnvelope> {
        WireRequest::parse(value).map_err(Into::into)
    }

    /// Step 2 — validate a parsed request and bind the trusted caller
    /// into an [`OperationRequest`].
    ///
    /// Runs every contract check in order; each failure is a typed
    /// error, never a guess (Issue #53 AC). `caller` must come from a
    /// trusted adapter — it is impossible to smuggle one through the
    /// wire.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — see the module docs for the full pipeline.
    pub fn bind(
        &mut self,
        wire: WireRequest,
        caller: CallerIdentity,
    ) -> Result<OperationRequest, ErrorEnvelope> {
        let registration = self.registry.resolve(&wire.capability)?;
        let descriptor = &registration.descriptor;
        if !matches!(descriptor.availability, Availability::Available) {
            return Err(GatewayError::Unavailable {
                id: descriptor.id.to_string(),
                availability: descriptor.availability.clone(),
            }
            .into());
        }

        let schema = self.schemas.resolve(&wire.schema)?;
        if !wire.schema.is_compatible_with(&descriptor.input_schema) {
            return Err(GatewayError::IncompatibleSchema {
                name: wire.schema.name.clone(),
                requested: wire.schema.version,
                declared: descriptor.input_schema.version,
            }
            .into());
        }

        let project =
            ProjectId::from_tagged(&wire.project).map_err(|_| GatewayError::InvalidId {
                text: wire.project.clone(),
            })?;

        let target = Self::bind_target(descriptor, wire.target.as_ref())?;
        let consistency = Self::bind_consistency(descriptor, &wire.consistency)?;
        schema.validate("args", &wire.args)?;

        let operation_id = OperationId::new(self.ids.next());

        Ok(OperationRequest {
            request_id: wire.request_id,
            operation_id,
            caller,
            capability: wire.capability,
            domain: descriptor.domain,
            project,
            target,
            args: wire.args,
            consistency,
            input_schema: wire.schema,
            task: wire.task.map(TaskRef::new).transpose()?,
            grant: wire.grant.map(GrantRef::new).transpose()?,
            idempotency: wire.idempotency_key.map(IdempotencyKey::new).transpose()?,
        })
    }

    /// Step 3 — authorize and execute a bound request.
    ///
    /// The evaluator runs at the domain-mapped stage before any
    /// executor sees the request; the stage is derived from the
    /// **registered descriptor's** domain and availability is re-checked
    /// here — `invoke` never trusts bind-time state. `PermitForStage` is
    /// the only decision that reaches a handler, and the returned
    /// outcome is checked against the declared `ExecutionContract` and
    /// output schema before it leaves the boundary.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — `Denied`, idempotency duplicates/conflicts,
    /// or whatever the executor returns.
    pub fn invoke(
        &mut self,
        req: &OperationRequest,
    ) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let registration = self.registry.resolve(&req.capability)?;
        let descriptor = &registration.descriptor;
        if !matches!(descriptor.availability, Availability::Available) {
            return Err(GatewayError::Unavailable {
                id: descriptor.id.to_string(),
                availability: descriptor.availability.clone(),
            }
            .into());
        }
        let stage = stage_for(descriptor.domain);
        match self.evaluator.decide(req, stage, descriptor) {
            AuthorizationDecision::PermitForStage => {}
            AuthorizationDecision::ApprovalRequired => {
                return Ok(GatewayOutcome::ApprovalRequired {
                    approval: approval_ref_for(req.operation_id),
                });
            }
            AuthorizationDecision::Deny { reason } => {
                return Err(GatewayError::Denied {
                    stage,
                    reason: Some(reason),
                }
                .into());
            }
        }
        let Some(handler) = registration.handler.as_ref() else {
            return Err(GatewayError::NoExecutor {
                id: descriptor.id.to_string(),
            }
            .into());
        };

        // Idempotency dedupes before side effects but records only after
        // success — denied/failed attempts never burn the key.
        let ledger = req.idempotency.as_ref().map(|key| {
            (
                LedgerKey {
                    principal_kind: req.caller.principal.kind,
                    principal_id: req.caller.principal.id.clone(),
                    adapter: req.caller.adapter,
                    project: req.project,
                    key: key.clone(),
                },
                Self::fingerprint(req),
            )
        });
        if let Some((key, fingerprint)) = &ledger {
            self.ledger.check(key, fingerprint)?;
        }

        let outcome = handler.execute(req)?;
        Self::check_outcome(descriptor, &outcome, &self.schemas)?;

        if let Some((key, fingerprint)) = ledger {
            self.ledger.record(key, fingerprint, req.operation_id);
        }
        Ok(outcome)
    }

    /// `parse_wire` + `bind` + `invoke` — the full typed invocation an
    /// adapter performs per wire request.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — any pipeline stage's typed failure.
    pub fn handle(
        &mut self,
        value: &Value,
        caller: CallerIdentity,
    ) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let wire = Self::parse_wire(value)?;
        let req = self.bind(wire, caller)?;
        self.invoke(&req)
    }

    fn bind_target(
        descriptor: &CapabilityDescriptor,
        target: Option<&WireTarget>,
    ) -> Result<OperationTarget, GatewayError> {
        match (descriptor.target, target) {
            (TargetContract::None, None) => Ok(OperationTarget::None),
            (TargetContract::None, Some(_)) => Err(GatewayError::UnexpectedTarget),
            (_, None) => Err(GatewayError::TargetRequired),
            (TargetContract::Resource { kind }, Some(WireTarget::Resource { kind: k, id })) => {
                let expected = crate::request::ResourceKind::parse(k)?;
                if expected != kind {
                    return Err(GatewayError::WrongResourceKind {
                        expected: kind,
                        actual: expected,
                    });
                }
                Ok(OperationTarget::Resource {
                    resource: ResourceRef::expect(kind, id)?,
                })
            }
            (
                TargetContract::ResourceSet { kind },
                Some(WireTarget::ResourceSet { kind: k, ids }),
            ) => {
                let expected = crate::request::ResourceKind::parse(k)?;
                if expected != kind {
                    return Err(GatewayError::WrongResourceKind {
                        expected: kind,
                        actual: expected,
                    });
                }
                if ids.is_empty() {
                    return Err(GatewayError::SchemaViolation {
                        path: "target.ids".to_owned(),
                        reason: "resource set must not be empty".to_owned(),
                    });
                }
                if ids.len() > 1024 {
                    return Err(GatewayError::SchemaViolation {
                        path: "target.ids".to_owned(),
                        reason: "resource set exceeds 1024 ids".to_owned(),
                    });
                }
                let refs = ids
                    .iter()
                    .map(|id| ResourceRef::expect(kind, id))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(OperationTarget::ResourceSet { resources: refs })
            }
            (
                TargetContract::Create { parent, creates },
                Some(WireTarget::Create {
                    parent_kind,
                    parent: parent_id,
                    creates: creates_kind,
                }),
            ) => {
                let parent_declared = crate::request::ResourceKind::parse(parent_kind)?;
                if parent_declared != parent {
                    return Err(GatewayError::WrongResourceKind {
                        expected: parent,
                        actual: parent_declared,
                    });
                }
                let creates_actual = crate::request::ResourceKind::parse(creates_kind)?;
                if creates_actual != creates {
                    return Err(GatewayError::WrongResourceKind {
                        expected: creates,
                        actual: creates_actual,
                    });
                }
                let parent_ref = ResourceRef::expect(parent, parent_id)?;
                Ok(OperationTarget::Create {
                    parent: parent_ref,
                    creates,
                })
            }
            (contract, Some(wire_target)) => Err(GatewayError::TargetShapeMismatch {
                expected: Self::contract_shape(contract),
                actual: Self::wire_shape(wire_target),
            }),
        }
    }

    fn contract_shape(contract: TargetContract) -> &'static str {
        match contract {
            TargetContract::None => "none",
            TargetContract::Resource { .. } => "resource",
            TargetContract::ResourceSet { .. } => "resource-set",
            TargetContract::Create { .. } => "create",
        }
    }

    fn wire_shape(target: &WireTarget) -> &'static str {
        match target {
            WireTarget::Resource { .. } => "resource",
            WireTarget::ResourceSet { .. } => "resource-set",
            WireTarget::Create { .. } => "create",
        }
    }

    fn bind_consistency(
        descriptor: &CapabilityDescriptor,
        consistency: &WireConsistency,
    ) -> Result<Consistency, GatewayError> {
        let mapped = match consistency {
            WireConsistency::Latest => Consistency::Latest,
            WireConsistency::ReadAt { revision } => Consistency::ReadAt {
                revision: *revision,
            },
            WireConsistency::FixedSnapshot { snapshot } => Consistency::FixedSnapshot {
                snapshot: SnapshotRef::new(snapshot.clone())?,
            },
            WireConsistency::ExpectedRevision { revision } => Consistency::ExpectedRevision {
                revision: ProjectRevision(revision.0),
            },
            WireConsistency::Unconditional => Consistency::Unconditional,
        };
        if !mapped.allowed_in(descriptor.domain) {
            return Err(GatewayError::ConsistencyNotAllowed {
                domain: descriptor.domain,
                consistency: mapped.kind(),
            });
        }
        Ok(mapped)
    }

    /// Canonical payload fingerprint for idempotency — everything that
    /// makes the request semantically distinct except caller identity
    /// (already in the ledger key) and the request/operation ids.
    /// `task`/`grant` are covered: a retry carrying different
    /// authorization references is a conflict, not a silent duplicate.
    fn fingerprint(req: &OperationRequest) -> String {
        serde_json::json!({
            "capability": req.capability,
            "schema": req.input_schema,
            "project": req.project.to_tagged(),
            "target": req.target,
            "args": req.args,
            "consistency": req.consistency,
            "task": req.task,
            "grant": req.grant,
        })
        .to_string()
    }

    /// The outcome must honor the declared contract: a `Sync`
    /// capability may not return `Accepted` (nothing will complete the
    /// handle), and a `Completed` payload must satisfy the declared
    /// output schema. Violations are executor bugs, surfaced as
    /// `Internal` — never passed to the caller as a valid result.
    fn check_outcome(
        descriptor: &CapabilityDescriptor,
        outcome: &GatewayOutcome<Value>,
        schemas: &SchemaRegistry,
    ) -> Result<(), GatewayError> {
        if matches!(
            (descriptor.execution, outcome),
            (
                crate::capability::ExecutionContract::Sync,
                GatewayOutcome::Accepted { .. }
            )
        ) {
            return Err(GatewayError::Internal {
                reason: format!("capability {} is Sync but returned Accepted", descriptor.id),
            });
        }
        if let GatewayOutcome::Completed { result } = outcome
            && let Ok(schema) = schemas.resolve(&descriptor.output_schema)
            && let Err(violation) = schema.validate("result", result)
        {
            return Err(GatewayError::Internal {
                reason: format!(
                    "capability {} produced out-of-contract output: {violation}",
                    descriptor.id
                ),
            });
        }
        Ok(())
    }
}

impl Default for Gateway {
    fn default() -> Self {
        Self::new()
    }
}

/// Deterministic approval-request ref for GW-01's outcome boundary —
/// the real receipt lifecycle is GW-05's.
fn approval_ref_for(operation: OperationId) -> ApprovalRef {
    // Fixed-width output ("approval-for-operation-<32 hex>") is far
    // below the ref length bound, so construction cannot fail.
    ApprovalRef::new(format!("approval-for-{operation}"))
        .expect("fixed-width approval ref is always valid")
}
