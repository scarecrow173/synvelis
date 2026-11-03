//! GW-07 job engine — the `synvelis.job.submit`, `synvelis.job.get`,
//! and `synvelis.job.cancel` capabilities over the existing
//! grant/policy/snapshot contracts (issues #45/#61, GIA-001..003).
//!
//! Staged flow (GIA-002):
//!
//! ```text
//! submit → StartAsyncOperation eval + input-scope/ProviderTransfer
//!          effects eval + input snapshot + execution budget →
//!          JobPort::submit → job record → Accepted{operation}
//! get    → blind owner-scope lookup → ReadOperationResult eval →
//!          disclosure authorization → status → (once) materialize:
//!          media verify + provenance → bounded result
//! cancel → blind owner-scope lookup → JobPort::request_cancel →
//!          honest verdict (confirmed/requested/unconfirmed/
//!          not-cancellable + actual phase)
//! ```
//!
//! Boundary rules the engine owns:
//!
//! - `OperationId` (the caller-visible handle), `RequestId`, `JobRef`
//!   (provider-owned), `ProposalRef`, and `SourceId` stay distinct —
//!   a job is *not* a proposal and a media ref is *not* a project
//!   resource.
//! - Job records live in the adapter's book — MCP disconnect and
//!   window close delete nothing (JOB-01/02).
//! - Grant revocation protection is *not* cancellation: revoking a
//!   grant makes every later protected call fail in evaluation, and
//!   separately the host asks the engine to *request* cancellation of
//!   the grant's live jobs — a request, never a silent `Cancelled`
//!   ([`JobEngine::note_grant_revoked`], JOB-03).
//! - Completed ≠ verified: media verification failure leaves the
//!   output `Rejected` and unregisterable (JOB-05); verified media
//!   moves the project only through GW-04..06 (JOB-06/JM-04).

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};

use crate::authorization::{AuthorizationDecision, AuthorizationStage, PolicyEvaluator};
use crate::capability::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityVersion, DataAccessClass,
    DataAccessSet, EffectKind, EffectSet, ExecutionContract, OperationDomain, PermissionKind,
    PermissionSet, TargetContract,
};
use crate::context::ContextAuthorizer;
use crate::grant::ScopedEffects;
use crate::id::{GrantRef, JobRef, MediaRef, OperationId};
use crate::job::{
    self, CancelLink, CancelResponse, InputPin, InputSnapshot, JobPhase, JobPort, JobRecord,
    JobResult, JobSpec, OutputReport, SharedJobBook,
};
use crate::media::{MediaBook, MediaPort};
use crate::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome};
use crate::policy::GrantPolicy;
use crate::provenance::GenerationProvenance;
use crate::registry::CapabilityOperation;
use crate::request::{OperationRequest, ResourceRef};
use crate::schema::{FieldSchema, SchemaRef, SchemaVersion, ValueSchema};
use crate::snapshot::SnapshotSource;
use crate::store::PolicyStore;

/// The job-submit capability id.
pub const JOB_SUBMIT_ID: &str = "synvelis.job.submit";
/// The job-result capability id.
pub const JOB_GET_ID: &str = "synvelis.job.get";
/// The job-cancel capability id.
pub const JOB_CANCEL_ID: &str = "synvelis.job.cancel";
/// The verified-media placement capability id — a GW-04 semantic op
/// gated by [`crate::media::MediaGate`], registered by `PrepareEngine`.
pub const MEDIA_REGISTER_ID: &str = "synvelis.media.register";

const SUBMIT_INPUT: &str = "synvelis.job.submit.input";
const GET_INPUT: &str = "synvelis.job.get.input";
const CANCEL_INPUT: &str = "synvelis.job.cancel.input";
const JOB_OUTPUT: &str = "synvelis.job.output";

/// The job book's record bound.
const JOB_BOOK_CAP: usize = 512;

/// Provider-declared outputs per completion are bounded: a job may
/// stage at most this many media refs. Provider data enters the
/// adapter only through declared fields; bounding their count keeps
/// materialization (and the disclosed result) finite by contract.
const MAX_JOB_OUTPUTS: usize = 64;

fn str_(max_len: usize) -> ValueSchema {
    ValueSchema::String {
        max_len: Some(max_len),
    }
}

fn obj(fields: &[(&str, ValueSchema)], required: &[&str]) -> ValueSchema {
    let req: std::collections::BTreeSet<&str> = required.iter().copied().collect();
    ValueSchema::Object {
        fields: fields
            .iter()
            .map(|(name, schema)| {
                (
                    (*name).to_owned(),
                    if req.contains(name) {
                        FieldSchema::required(schema.clone())
                    } else {
                        FieldSchema::optional(schema.clone())
                    },
                )
            })
            .collect(),
        deny_unknown: true,
    }
}

fn loose() -> ValueSchema {
    ValueSchema::Object {
        fields: BTreeMap::new(),
        deny_unknown: false,
    }
}

fn descriptor(
    id: &str,
    effects: &[EffectKind],
    permissions: &[PermissionKind],
    data_access: &[DataAccessClass],
    execution: ExecutionContract,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: CapabilityId::new(id).expect("capability id is valid"),
        version: CapabilityVersion::new(1, 0),
        domain: OperationDomain::AsyncOperation,
        input_schema: SchemaRef::new(format!("{id}.input"), 1, 0),
        output_schema: SchemaRef::new(JOB_OUTPUT, 1, 0),
        target: TargetContract::None,
        effect_bound: EffectSet::of(effects.iter().copied()),
        required_permissions: PermissionSet::of(permissions.iter().copied()),
        data_access: DataAccessSet::of(data_access.iter().copied()),
        execution,
        availability: Availability::Available,
    }
}

/// The GW-07 job engine — admission, result disclosure, and cancel
/// steering over the shared policy/snapshot contracts.
pub struct JobEngine {
    providers: BTreeMap<String, Arc<dyn JobPort>>,
    media_port: Arc<dyn MediaPort>,
    media_book: Arc<MediaBook>,
    jobs: SharedJobBook,
    snapshots: Arc<dyn SnapshotSource>,
    store: Arc<dyn PolicyStore>,
    policy: Arc<GrantPolicy>,
    submit: CapabilityDescriptor,
    get: CapabilityDescriptor,
    cancel: CapabilityDescriptor,
}

#[allow(clippy::result_large_err)] // typed envelope is the contract
impl JobEngine {
    /// The engine over the provider registry (`key → JobPort`), the
    /// media-verification seam, the shared media book (also the
    /// `synvelis.media.register` gate), the snapshot seam, the policy
    /// store (budget reads), and the base policy evaluator.
    ///
    /// `GrantPolicy` is deliberately concrete: the adapter needs its
    /// effect-aware `evaluate_effects` — a capability outside the
    /// `PolicyEvaluator` trait.
    #[must_use]
    pub fn new(
        providers: BTreeMap<String, Arc<dyn JobPort>>,
        media_port: Arc<dyn MediaPort>,
        media_book: Arc<MediaBook>,
        snapshots: Arc<dyn SnapshotSource>,
        store: Arc<dyn PolicyStore>,
        policy: Arc<GrantPolicy>,
    ) -> Self {
        Self {
            providers,
            media_port,
            media_book,
            jobs: SharedJobBook::new(JOB_BOOK_CAP),
            snapshots,
            store,
            policy,
            submit: descriptor(
                JOB_SUBMIT_ID,
                &[EffectKind::Read, EffectKind::ExternalAction],
                &[PermissionKind::RunJob],
                &[DataAccessClass::ProviderTransfer],
                ExecutionContract::AsyncAccept,
            ),
            get: descriptor(
                JOB_GET_ID,
                &[EffectKind::Read, EffectKind::Disclose],
                &[PermissionKind::RunJob],
                // The declared floor — the emitted classes are checked
                // at the disclosure boundary (ExternalDisclosure is
                // added for external callers there).
                &[DataAccessClass::MetadataRead],
                ExecutionContract::Sync,
            ),
            cancel: descriptor(
                JOB_CANCEL_ID,
                &[EffectKind::ExternalAction],
                &[PermissionKind::RunJob],
                &[DataAccessClass::MetadataRead],
                ExecutionContract::Sync,
            ),
        }
    }

    /// The shared media book — `PrepareEngine::with_media` consumes it
    /// as the `synvelis.media.register` gate.
    #[must_use]
    pub fn media_book(&self) -> Arc<MediaBook> {
        self.media_book.clone()
    }

    /// Registers the three input schemas, the shared output schema,
    /// and the three job capabilities, all served by `self`.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — schema or capability registration conflict.
    pub fn install(self: &Arc<Self>, gateway: &mut crate::Gateway) -> Result<(), ErrorEnvelope> {
        for (name, schema) in [
            (
                SUBMIT_INPUT,
                obj(
                    &[
                        ("provider", str_(128)),
                        (
                            "kind",
                            ValueSchema::Enum {
                                values: vec!["generation".to_owned(), "analysis".to_owned()],
                            },
                        ),
                        (
                            "output",
                            ValueSchema::Enum {
                                values: vec!["media".to_owned(), "midi".to_owned()],
                            },
                        ),
                        (
                            "inputs",
                            ValueSchema::Array {
                                item: Box::new(str_(128)),
                                max_len: Some(32),
                            },
                        ),
                        ("prompt", str_(4096)),
                    ],
                    &["provider", "kind", "output"],
                ),
            ),
            (GET_INPUT, obj(&[("operation", str_(64))], &["operation"])),
            (
                CANCEL_INPUT,
                obj(&[("operation", str_(64))], &["operation"]),
            ),
        ] {
            gateway
                .schemas_mut()
                .register((*name).to_owned(), SchemaVersion::new(1, 0), schema)
                .map_err(ErrorEnvelope::from)?;
        }
        gateway
            .schemas_mut()
            .register((*JOB_OUTPUT).to_owned(), SchemaVersion::new(1, 0), loose())
            .map_err(ErrorEnvelope::from)?;
        for descriptor in [self.submit.clone(), self.get.clone(), self.cancel.clone()] {
            gateway.register_capability(descriptor, Some(self.clone()))?;
        }
        Ok(())
    }

    /// A job's raw record by operation id — admin/introspection only,
    /// never a caller-scoped lookup.
    #[must_use]
    pub fn record(&self, operation: OperationId) -> Option<JobRecord> {
        self.jobs.with(|book| {
            // JobRecord is not Clone-by-design heavy; expose a shallow
            // field copy for inspection.
            book.get(operation).map(|r| JobRecord {
                operation: r.operation,
                request: r.request.clone(),
                job: r.job.clone(),
                principal: r.principal.clone(),
                project: r.project,
                task: r.task.clone(),
                grant: r.grant.clone(),
                spec: r.spec.clone(),
                snapshot: r.snapshot.clone(),
                cancel: r.cancel,
                last_phase: r.last_phase.clone(),
                result: r.result.clone(),
            })
        })
    }

    /// Grant-revocation protection (JOB-03): the host calls this after
    /// the policy store confirms `grant`'s revocation. Every *live* job
    /// admitted under `grant` gets a cancel *request* through its
    /// provider — the response is recorded honestly (`Confirmed` /
    /// `Requested` / `Unconfirmed`); jobs already settled are left as
    /// they are. Returns the affected job refs.
    ///
    /// This is deliberately *not* an implicit cancel: revocation's
    /// authorization effect is denial, and this is a separate,
    /// observable request to the job coordinator — `CancelRequested`,
    /// never a fabricated `Cancelled`.
    pub fn note_grant_revoked(&self, grant: &GrantRef) -> Vec<crate::id::JobRef> {
        let ops = self.jobs.with(|book| book.operations_for_grant(grant));
        let mut affected = Vec::new();
        for operation in ops {
            let Some((port, job)) = self.jobs.with(|book| {
                book.get_mut(operation)
                    .map(|r| (r.spec.provider.clone(), r.job.clone()))
            }) else {
                continue;
            };
            let Some(port) = self.providers.get(&port) else {
                continue;
            };
            let status = port.status(&job);
            if status.phase.terminal() {
                // The settle race resolved to the job already — there
                // is nothing to cancel; the record keeps its truth.
                self.jobs.with(|book| {
                    if let Some(r) = book.get_mut(operation) {
                        r.last_phase = status.phase;
                    }
                });
                continue;
            }
            let response = port.request_cancel(&job);
            self.jobs.with(|book| {
                if let Some(r) = book.get_mut(operation) {
                    r.note_cancel(&response);
                }
            });
            affected.push(job);
        }
        affected
    }

    // ── Executors ──────────────────────────────────────────────

    fn denied(stage: AuthorizationStage, reason: impl Into<String>) -> ErrorEnvelope {
        GatewayError::Denied {
            stage,
            reason: Some(reason.into()),
        }
        .into()
    }

    /// The scope/transfer evaluation for `submit`'s inputs — the same
    /// effect-aware path GW-02 runs for derived effects: every input
    /// must be covered by the grant's scope, and the external transfer
    /// requires `ProviderTransfer`.
    fn evaluate_submit_effects(
        &self,
        req: &OperationRequest,
        inputs: &[ResourceRef],
    ) -> Result<(), ErrorEnvelope> {
        let effects = ScopedEffects {
            reads: inputs.to_vec(),
            external_transfers: 1,
            ..ScopedEffects::default()
        };
        match self.policy.evaluate_effects(
            req,
            AuthorizationStage::StartAsyncOperation,
            &self.submit,
            &effects,
        ) {
            AuthorizationDecision::PermitForStage => Ok(()),
            AuthorizationDecision::Deny { reason } => Err(Self::denied(
                AuthorizationStage::StartAsyncOperation,
                reason,
            )),
            AuthorizationDecision::ApprovalRequired => Err(GatewayError::Internal {
                reason: "job admission must never require approval".to_owned(),
            }
            .into()),
        }
    }

    /// The admission-time input snapshot — pins the committed base the
    /// inputs resolve against and records each input's digest/state.
    fn input_snapshot(
        &self,
        req: &OperationRequest,
        inputs: &[ResourceRef],
    ) -> Result<InputSnapshot, ErrorEnvelope> {
        if inputs.is_empty() {
            return Ok(InputSnapshot {
                revision: None,
                members: Vec::new(),
            });
        }
        let Some(revision) = self.snapshots.latest_revision(req.project()) else {
            return Err(GatewayError::Internal {
                reason: "no committed base to snapshot inputs against".to_owned(),
            }
            .into());
        };
        let Some(state) = self.snapshots.pin(req.project(), revision) else {
            return Err(GatewayError::SnapshotExpired.into());
        };
        let members = inputs
            .iter()
            .map(|resource| {
                let (content_digest, source_state) = match resource {
                    ResourceRef::Source(id) => state
                        .sources
                        .get(id)
                        .map(|s| {
                            (
                                Some(s.content_digest.clone()),
                                Some(crate::snapshot::source_state_name(s.state).to_owned()),
                            )
                        })
                        .unwrap_or_default(),
                    _ => (None, None),
                };
                InputPin {
                    resource: *resource,
                    content_digest,
                    source_state,
                }
            })
            .collect();
        Ok(InputSnapshot {
            revision: Some(revision),
            members,
        })
    }

    /// The execution-budget spend — `limits.job_budget` is spent
    /// atomically at admission, one unit per submit dispatched to a
    /// provider; refunded only when the provider never admitted it.
    fn spend_budget(&self, req: &OperationRequest) -> Result<(), ErrorEnvelope> {
        let Some(grant_ref) = req.grant() else {
            return Ok(());
        };
        let Some(record) = self.store.grant(grant_ref) else {
            return Ok(());
        };
        let Some(limit) = record.grant.limits.job_budget else {
            return Ok(());
        };
        // One atomic check-and-spend under the book lock — a
        // concurrent submit can never slip between the check and the
        // spend, and the spend survives eviction (it bounds work
        // dispatched, not records retained).
        if self.jobs.with(|book| book.try_spend(grant_ref, limit)) {
            Ok(())
        } else {
            Err(GatewayError::LimitExceeded {
                limit: "job budget".to_owned(),
                max: limit,
            }
            .into())
        }
    }

    fn parse_inputs(req: &OperationRequest) -> Result<Vec<ResourceRef>, ErrorEnvelope> {
        let list = match req.args().get("inputs") {
            None | Some(Value::Null) => Vec::new(),
            Some(v) => v
                .as_array()
                .ok_or_else(|| crate::prepare::violation("args.inputs", "expected an array"))?
                .iter()
                .map(|item| {
                    let text = item.as_str().ok_or_else(|| {
                        crate::prepare::violation("args.inputs", "expected tagged id strings")
                    })?;
                    ResourceRef::parse_tagged(text)
                        .map_err(|e| crate::prepare::violation("args.inputs", e.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        Ok(list)
    }

    fn submit(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let provider = req
            .args()
            .get("provider")
            .and_then(Value::as_str)
            .ok_or_else(|| crate::prepare::violation("args.provider", "expected a string"))?;
        let kind = match req
            .args()
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "generation" => crate::job::JobKind::Generation,
            "analysis" => crate::job::JobKind::Analysis,
            other => {
                return Err(crate::prepare::violation(
                    "args.kind",
                    format!("unknown job kind {other}"),
                ));
            }
        };
        let output = match req
            .args()
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "media" => crate::job::OutputKind::Media,
            "midi" => crate::job::OutputKind::Midi,
            other => {
                return Err(crate::prepare::violation(
                    "args.output",
                    format!("unknown output kind {other}"),
                ));
            }
        };
        let inputs = Self::parse_inputs(req)?;
        let spec = JobSpec {
            kind,
            output,
            provider: provider.to_owned(),
            inputs: inputs.clone(),
            prompt: req
                .args()
                .get("prompt")
                .and_then(Value::as_str)
                .map(str::to_owned),
        };
        // External Transfer + input-scope evaluation (Read/External
        // Transfer authorization before admission).
        self.evaluate_submit_effects(req, &inputs)?;
        // Admission through the job coordinator — resolve the
        // provider before spending budget so an unknown provider
        // never costs a unit.
        let Some(port) = self.providers.get(provider) else {
            return Err(GatewayError::Denied {
                stage: AuthorizationStage::StartAsyncOperation,
                reason: Some("unknown provider".to_owned()),
            }
            .into());
        };
        // Input Snapshot binding.
        let snapshot = self.input_snapshot(req, &inputs)?;
        // Execution Budget.
        self.spend_budget(req)?;
        let job = match port.submit(&spec) {
            Ok(job) => job,
            Err(e) => {
                // The provider never admitted the work — refund the
                // budget unit the reservation spent.
                if let Some(grant) = req.grant() {
                    self.jobs.with(|book| book.unspend(grant));
                }
                return Err(e);
            }
        };
        let admitted = self.jobs.with(|book| {
            book.insert(JobRecord {
                operation: req.operation_id(),
                request: req.request_id().clone(),
                job: job.clone(),
                principal: req.caller().principal().clone(),
                project: req.project(),
                task: req.task().cloned(),
                grant: req.grant().cloned(),
                spec,
                snapshot,
                cancel: CancelLink::NotRequested,
                last_phase: JobPhase::Pending,
                result: None,
            })
        });
        if !admitted {
            // Never hand back a handle the book cannot resolve — and
            // never abandon the provider-side job: request its
            // cancellation and answer honestly.
            let _ = port.request_cancel(&job);
            return Err(GatewayError::LimitExceeded {
                limit: "job book capacity".to_owned(),
                max: JOB_BOOK_CAP as u64,
            }
            .into());
        }
        Ok(GatewayOutcome::Accepted {
            operation: req.operation_id(),
        })
    }

    /// Builds the materialized result — verification + provenance —
    /// for a `Completed` job whose record has no result yet. Runs
    /// *outside* the job-book lock: the media/provider seams may
    /// block, and the lock serializes only record truth.
    fn build_result(
        &self,
        operation: OperationId,
        job: &JobRef,
        status: &crate::job::JobStatus,
        record: &JobRecord,
    ) -> JobResult {
        let outputs = status
            .outputs
            .iter()
            .take(MAX_JOB_OUTPUTS)
            .map(|out| {
                let verdict = self.media_port.verify(out);
                let media_record = self.media_book.record(
                    job,
                    record.principal.clone(),
                    record.project,
                    record.spec.output,
                    out,
                    &verdict,
                );
                if media_record.job != *job {
                    // A provider-minted media ref can collide with a
                    // ref another job already recorded — the book
                    // keeps the first writer, so report this output
                    // honestly as unregistrable rather than parroting
                    // the foreign record's verdict.
                    return OutputReport {
                        media: out.media.clone(),
                        lifecycle: crate::media::MediaLifecycle::Unavailable,
                        verified: None,
                        reason: Some("media ref collision".to_owned()),
                    };
                }
                let reason = match &verdict {
                    crate::media::MediaVerdict::Rejected { reason }
                    | crate::media::MediaVerdict::Unavailable { reason } => {
                        Some(job::bounded_reason(reason))
                    }
                    crate::media::MediaVerdict::Verified(_) => None,
                };
                OutputReport {
                    media: out.media.clone(),
                    lifecycle: media_record.lifecycle,
                    verified: media_record.verified.clone(),
                    reason,
                }
            })
            .collect::<Vec<_>>();
        let media: Vec<MediaRef> = status.outputs.iter().map(|o| o.media.clone()).collect();
        let claim = self
            .providers
            .get(&record.spec.provider)
            .map_or_else(Default::default, |p| p.claim(job));
        let provenance = GenerationProvenance::build(
            job,
            operation,
            &claim,
            record.snapshot.members.iter().map(|m| m.resource).collect(),
            record.snapshot.revision,
            media,
            record.spec.kind,
        );
        JobResult {
            outputs,
            provenance,
        }
    }

    /// The disclosure authorization for `job.get` — the emitted
    /// classes (result metadata) plus `ExternalDisclosure` for
    /// external callers, evaluated at `ReadOperationResult`.
    fn authorize_result(&self, req: &OperationRequest) -> Result<(), ErrorEnvelope> {
        // ReadOperationResult — the result-disclosure stage.
        match self
            .policy
            .decide(req, AuthorizationStage::ReadOperationResult, &self.get)
        {
            AuthorizationDecision::PermitForStage => {}
            AuthorizationDecision::Deny { reason } => {
                return Err(Self::denied(
                    AuthorizationStage::ReadOperationResult,
                    reason,
                ));
            }
            AuthorizationDecision::ApprovalRequired => {
                return Err(GatewayError::Internal {
                    reason: "result disclosure must never require approval".to_owned(),
                }
                .into());
            }
        }
        let external = matches!(
            req.caller().principal().kind(),
            crate::request::PrincipalKind::Agent
        ) || matches!(
            req.caller().origin(),
            crate::request::OriginKind::ExternalAdapter
        );
        let mut classes = std::collections::BTreeSet::new();
        classes.insert(DataAccessClass::MetadataRead);
        match ContextAuthorizer::authorize_disclosure(
            &*self.policy,
            req,
            &self.get,
            &classes,
            external,
        ) {
            AuthorizationDecision::PermitForStage => Ok(()),
            AuthorizationDecision::Deny { reason } => Err(Self::denied(
                AuthorizationStage::ReadOperationResult,
                reason,
            )),
            AuthorizationDecision::ApprovalRequired => Err(GatewayError::Internal {
                reason: "result disclosure must never require approval".to_owned(),
            }
            .into()),
        }
    }

    fn get(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let operation = job::operation_arg(req)?;
        // Existence-blind owner scope — foreign and unknown share one
        // verdict (JOB-08).
        let (port, job) = self
            .jobs
            .with(|book| {
                book.scoped(operation, req)
                    .map(|r| (r.spec.provider.clone(), r.job.clone()))
            })
            .ok_or_else(|| {
                Self::denied(AuthorizationStage::ReadOperationResult, "unknown operation")
            })?;
        // Result-disclosure authorization — after the blind scope
        // check so the deny reason never differs by existence.
        self.authorize_result(req)?;
        let Some(port) = self.providers.get(&port) else {
            return Err(GatewayError::Internal {
                reason: "job's provider is no longer registered".to_owned(),
            }
            .into());
        };
        let status = port.status(&job);
        // Under the lock: scope the record, upgrade the cancel link
        // on observed truth, advance the phase without ever
        // regressing a terminal phase to a live one.
        let materialize = self.jobs.with(|book| {
            let record = book.scoped_mut(operation, req)?;
            if matches!(
                record.cancel,
                CancelLink::Requested | CancelLink::Unconfirmed
            ) && status.phase == JobPhase::Cancelled
            {
                record.cancel = CancelLink::Confirmed;
            }
            if !record.last_phase.terminal() || status.phase.terminal() {
                record.last_phase = status.phase.clone();
            }
            Some(record.result.is_none() && status.phase == JobPhase::Completed)
        });
        let Some(materialize) = materialize else {
            return Err(Self::denied(
                AuthorizationStage::ReadOperationResult,
                "unknown operation",
            ));
        };
        if materialize {
            // Verification + provenance outside the lock; the commit
            // writes only while the record is still unmaterialized —
            // the media book is idempotent per media ref, so a
            // racing second poll converges on the same record.
            let built = self.jobs.with(|book| {
                book.scoped(operation, req)
                    .map(|r| self.build_result(operation, &job, &status, r))
            });
            if let Some(result) = built {
                self.jobs.with(|book| {
                    if let Some(record) = book.scoped_mut(operation, req)
                        && record.result.is_none()
                    {
                        record.result = Some(result);
                    }
                });
            }
        }
        let Some(mut result) = self
            .jobs
            .with(|book| book.scoped(operation, req).map(job::result_json))
        else {
            return Err(Self::denied(
                AuthorizationStage::ReadOperationResult,
                "unknown operation",
            ));
        };
        result["progress"] = json!(status.progress);
        Ok(GatewayOutcome::Completed { result })
    }

    fn cancel(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let operation = job::operation_arg(req)?;
        let (port, job) = self
            .jobs
            .with(|book| {
                book.scoped(operation, req)
                    .map(|r| (r.spec.provider.clone(), r.job.clone()))
            })
            .ok_or_else(|| {
                Self::denied(AuthorizationStage::StartAsyncOperation, "unknown operation")
            })?;
        let Some(port) = self.providers.get(&port) else {
            return Err(GatewayError::Internal {
                reason: "job's provider is no longer registered".to_owned(),
            }
            .into());
        };
        let response = port.request_cancel(&job);
        let verdict = match &response {
            CancelResponse::Confirmed => "confirmed",
            CancelResponse::Pending => "requested",
            CancelResponse::Unreachable => "unconfirmed",
            CancelResponse::Settled { .. } => "not-cancellable",
        };
        // The record can be evicted by a concurrent same-scope submit
        // between the read above and this write — the provider-side
        // cancel was still delivered, so report "unknown" rather than
        // a null phase.
        let phase = self.jobs.with(|book| {
            let r = book.scoped_mut(operation, req)?;
            r.note_cancel(&response);
            Some(r.last_phase.name())
        });
        let phase = phase.unwrap_or("unknown");
        Ok(GatewayOutcome::Completed {
            result: json!({
                "operation": operation.to_tagged(),
                "job": job.as_str(),
                "cancel": verdict,
                "phase": phase,
            }),
        })
    }
}

impl CapabilityOperation for JobEngine {
    fn execute(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        match req.capability().id.as_str() {
            JOB_SUBMIT_ID => self.submit(req),
            JOB_GET_ID => self.get(req),
            JOB_CANCEL_ID => self.cancel(req),
            _ => Err(GatewayError::Internal {
                reason: "job engine bound to an unknown capability".to_owned(),
            }
            .into()),
        }
    }
}
