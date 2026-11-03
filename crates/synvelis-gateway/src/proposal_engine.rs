//! GW-05 proposal engine — the `synvelis.proposal.preview`,
//! `synvelis.proposal.approve`, and `synvelis.proposal.apply`
//! capabilities over the shared [`ProposalStore`].
//!
//! - **Preview** (`Proposal` domain) emits the frozen changeset's
//!   semantic diff — the review document a trusted user approves —
//!   through the same emission boundary preparation uses.
//! - **Approve** (`Proposal` domain) is the receipt-issuance path —
//!   trusted users only. An agent caller is denied even when its
//!   grant covers the capability: approvals are never self-declared
//!   (GT-008).
//! - **Apply** (`ProjectMutation` domain, `expected-revision`
//!   consistency) is the apply-request gate GW-06 consumes: proposal
//!   liveness, digest assertion, revision binding, approval
//!   verification+consumption, and the apply record. A cleared apply
//!   is recorded `CommitRequested` and answered `Accepted` — never a
//!   false commit success; `Committed`/`Rejected`/`Conflict`/
//!   `Indeterminate` are GW-06's to report through
//!   [`ProposalEngine::settle_apply`] (ACG-004).
//!
//! The engine holds no project handle and mutates no project state —
//! its only project-state read is `latest_revision` for the SEC-11
//! conflict check.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;
use synvelis_project::ProjectRevision;

use crate::authorization::{AuthorizationDecision, AuthorizationStage};
use crate::capability::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityVersion, DataAccessClass,
    DataAccessSet, EffectKind, EffectSet, ExecutionContract, OperationDomain, PermissionKind,
    PermissionSet, TargetContract,
};
use crate::grant::ApprovalPolicy;
use crate::id::{ApprovalRef, GrantRef, ProposalRef, TaskRef};
use crate::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome};
use crate::prepare::{self, PrepareAuthorizer};
use crate::proposal::{
    ApplyRecord, ApplyStatus, ApprovalReceipt, ApprovalRecord, ApprovalVerdict, PreparedChangeSet,
    ProposalStatus, ProposalStore, ProposalView, preview_value,
};
use crate::registry::CapabilityOperation;
use crate::request::{OperationRequest, OriginKind, PrincipalKind};
use crate::schema::{FieldSchema, SchemaRef, SchemaVersion, ValueSchema};
use crate::snapshot::SnapshotSource;
use crate::store::PolicyStore;

/// The preview capability id.
pub const PREVIEW_ID: &str = "synvelis.proposal.preview";
/// The approve capability id.
pub const APPROVE_ID: &str = "synvelis.proposal.approve";
/// The apply capability id.
pub const APPLY_ID: &str = "synvelis.proposal.apply";

const PREVIEW_INPUT: &str = "synvelis.proposal.preview.input";
const APPROVE_INPUT: &str = "synvelis.proposal.approve.input";
const APPLY_INPUT: &str = "synvelis.proposal.apply.input";
const PREVIEW_OUTPUT: &str = "synvelis.proposal.preview.output";
const APPROVE_OUTPUT: &str = "synvelis.proposal.approve.output";
const APPLY_OUTPUT: &str = "synvelis.proposal.apply.output";

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

/// The capability descriptor for `id` — its schemas are the
/// `{id}.input`/`.output` pair `install` registers.
fn descriptor(
    id: &str,
    domain: OperationDomain,
    effects: &[EffectKind],
    permissions: &[PermissionKind],
    data_access: &[DataAccessClass],
    execution: ExecutionContract,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: CapabilityId::new(id).expect("capability id is valid"),
        version: CapabilityVersion::new(1, 0),
        domain,
        input_schema: SchemaRef::new(format!("{id}.input"), 1, 0),
        output_schema: SchemaRef::new(format!("{id}.output"), 1, 0),
        target: TargetContract::None,
        effect_bound: EffectSet::of(effects.iter().copied()),
        required_permissions: PermissionSet::of(permissions.iter().copied()),
        data_access: DataAccessSet::of(data_access.iter().copied()),
        execution,
        availability: Availability::Available,
    }
}

/// The GW-05 proposal engine — preview/approve/apply over the shared
/// store, plus the seam surface GW-06's commit boundary consumes.
pub struct ProposalEngine {
    proposals: Arc<ProposalStore>,
    snapshots: Arc<dyn SnapshotSource>,
    store: Arc<dyn PolicyStore>,
    authorizer: Arc<dyn PrepareAuthorizer>,
    preview: CapabilityDescriptor,
    approve: CapabilityDescriptor,
    apply: CapabilityDescriptor,
}

#[allow(clippy::result_large_err)] // typed envelope is the contract
impl ProposalEngine {
    /// The engine over the shared proposal store, the snapshot seam
    /// (revision fence only), the policy store (grant-mode lookup),
    /// and the GW-04 authorizer (emission boundary reuse).
    #[must_use]
    pub fn new(
        proposals: Arc<ProposalStore>,
        snapshots: Arc<dyn SnapshotSource>,
        store: Arc<dyn PolicyStore>,
        authorizer: Arc<dyn PrepareAuthorizer>,
    ) -> Self {
        Self {
            proposals,
            snapshots,
            store,
            authorizer,
            preview: descriptor(
                PREVIEW_ID,
                OperationDomain::Proposal,
                &[EffectKind::Read],
                &[PermissionKind::ReadContext],
                // Metadata is the floor — the real bound is the
                // changeset's emitted classes, checked at the
                // emission boundary the prepare result passed.
                &[DataAccessClass::MetadataRead],
                ExecutionContract::Sync,
            ),
            approve: descriptor(
                APPROVE_ID,
                OperationDomain::Proposal,
                &[EffectKind::Read],
                &[PermissionKind::MutateProject],
                &[DataAccessClass::MetadataRead],
                ExecutionContract::Sync,
            ),
            apply: descriptor(
                APPLY_ID,
                OperationDomain::ProjectMutation,
                &[
                    EffectKind::Read,
                    EffectKind::Write,
                    EffectKind::Create,
                    EffectKind::Delete,
                ],
                &[PermissionKind::MutateProject],
                &[DataAccessClass::MetadataRead],
                // Accepted-for-commit semantics: GW-06 completes the
                // operation — the apply gate never claims it.
                ExecutionContract::AsyncAccept,
            ),
        }
    }

    /// The shared proposal store — gate wiring and GW-06's read side.
    #[must_use]
    pub fn proposal_store(&self) -> Arc<ProposalStore> {
        self.proposals.clone()
    }

    /// Registers the three input/output schemas and capabilities,
    /// all served by `self`.
    ///
    /// `ApproveToApply` grants also need the receipt question at the
    /// apply stage — the host composes the evaluator as
    /// `ProposalGate::new(proposals, inner)` around its base policy
    /// (the test rig shows the wiring). Without it an approve-to-apply
    /// apply can never clear `ApprovalRequired`.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — schema or capability registration conflict.
    pub fn install(self: &Arc<Self>, gateway: &mut crate::Gateway) -> Result<(), ErrorEnvelope> {
        for (name, schema) in [
            (
                PREVIEW_INPUT,
                obj(
                    &[("proposal", str_(256)), ("digest", str_(128))],
                    &["proposal"],
                ),
            ),
            (
                APPROVE_INPUT,
                obj(
                    &[("proposal", str_(256)), ("digest", str_(128))],
                    &["proposal", "digest"],
                ),
            ),
            (
                APPLY_INPUT,
                obj(
                    &[
                        ("proposal", str_(256)),
                        ("digest", str_(128)),
                        ("approval", str_(256)),
                    ],
                    &["proposal", "digest"],
                ),
            ),
        ] {
            gateway
                .schemas_mut()
                .register((*name).to_owned(), SchemaVersion::new(1, 0), schema)
                .map_err(ErrorEnvelope::from)?;
        }
        for name in [PREVIEW_OUTPUT, APPROVE_OUTPUT, APPLY_OUTPUT] {
            gateway
                .schemas_mut()
                .register((*name).to_owned(), SchemaVersion::new(1, 0), loose())
                .map_err(ErrorEnvelope::from)?;
        }
        for descriptor in [
            self.preview.clone(),
            self.approve.clone(),
            self.apply.clone(),
        ] {
            gateway.register_capability(descriptor, Some(self.clone()))?;
        }
        Ok(())
    }

    // ── GW-06 / Synvelis-side seam surface ─────────────────────

    /// The frozen changeset — digest, dependency identity, commands,
    /// preconditions, effects: everything GW-06 revalidates against.
    #[must_use]
    pub fn changeset(&self, proposal: &ProposalRef) -> Option<PreparedChangeSet> {
        self.proposals.changeset(proposal)
    }

    /// The retained preparation product (op + candidate), present
    /// while the proposal is `Prepared` — GW-06's commit input.
    #[must_use]
    pub fn prepared_record(
        &self,
        proposal: &ProposalRef,
    ) -> Option<Arc<crate::prepare::PreparedRecord>> {
        self.proposals.record(proposal)
    }

    /// The proposal's approval lifecycle record.
    #[must_use]
    pub fn approval_record(&self, proposal: &ProposalRef) -> Option<ApprovalRecord> {
        self.proposals.view(proposal).and_then(|v| v.approval)
    }

    /// The proposal's apply lifecycle record.
    #[must_use]
    pub fn apply_record(&self, proposal: &ProposalRef) -> Option<ApplyRecord> {
        self.proposals.view(proposal).and_then(|v| v.apply)
    }

    /// Atomically verifies and spends a receipt — the seam GW-06's
    /// fence calls for approval-consumption/commit-result atomicity.
    /// The wire apply consumes at accept, so for the apply that
    /// already spent the receipt this is an idempotent *verify* —
    /// it re-confirms that this exact attempt consumed it; any other
    /// caller can never succeed.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — `ApprovalInvalid`/`ProposalUnavailable`.
    pub fn try_consume_approval(
        &self,
        proposal: &ProposalRef,
        approval: &ApprovalRef,
        req: &OperationRequest,
    ) -> Result<ApprovalReceipt, ErrorEnvelope> {
        self.proposals
            .consume_approval(proposal, approval, req)
            .map_err(ErrorEnvelope::from)
    }

    /// Reports the commit-side outcome of an accepted apply — GW-06's
    /// settlement seam (`Commit`/`Reject`/`Conflict`/`Indeterminate`
    /// on a `CommitRequested` record; the table governs legality,
    /// and a re-report of the current status is idempotent).
    /// `Accept` is the apply gate's transition, never a commit
    /// outcome the fence may report.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — no apply exists, the transition is
    /// illegal, or the event is not a commit-side outcome.
    pub fn settle_apply(
        &self,
        proposal: &ProposalRef,
        event: crate::proposal::ApplyEvent,
    ) -> Result<ApplyRecord, ErrorEnvelope> {
        if matches!(event, crate::proposal::ApplyEvent::Accept) {
            return Err(GatewayError::Internal {
                reason: "accept is the apply gate's transition, not a commit outcome".to_owned(),
            }
            .into());
        }
        self.proposals
            .transit_apply(proposal, event)
            .map_err(ErrorEnvelope::from)
    }

    /// The trusted-user decline transition (Synvelis-side; never a
    /// wire capability — a decline is a user decision recorded, not
    /// an agent-reachable verdict).
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — illegal transition or no approval.
    pub fn decline_approval(
        &self,
        proposal: &ProposalRef,
    ) -> Result<ApprovalRecord, ErrorEnvelope> {
        self.proposals
            .transit_approval(proposal, crate::proposal::ApprovalEvent::Decline)
            .map_err(ErrorEnvelope::from)
    }

    /// Synvelis-side revocation of an outstanding request/receipt.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — illegal transition or no approval.
    pub fn revoke_approval(&self, proposal: &ProposalRef) -> Result<ApprovalRecord, ErrorEnvelope> {
        self.proposals
            .transit_approval(proposal, crate::proposal::ApprovalEvent::Revoke)
            .map_err(ErrorEnvelope::from)
    }

    // ── Executors ──────────────────────────────────────────────

    fn is_external(req: &OperationRequest) -> bool {
        matches!(req.caller().principal().kind(), PrincipalKind::Agent)
            || matches!(req.caller().origin(), OriginKind::ExternalAdapter)
    }

    fn proposal_arg(req: &OperationRequest) -> Result<ProposalRef, ErrorEnvelope> {
        let text = req
            .args()
            .get("proposal")
            .and_then(Value::as_str)
            .ok_or_else(|| prepare::violation("args.proposal", "expected a string"))?;
        ProposalRef::new(text).map_err(ErrorEnvelope::from)
    }

    fn resolve_live(
        &self,
        proposal: &ProposalRef,
        req: &OperationRequest,
    ) -> Result<ProposalView, ErrorEnvelope> {
        let Some(view) = self
            .proposals
            .resolve(proposal, req.caller().principal(), req.project())
        else {
            // Unknown, foreign, and inaccessible proposals share one
            // verdict — an agent never learns a proposal exists.
            return Err(GatewayError::ProposalUnavailable {
                reason: "unknown proposal".to_owned(),
            }
            .into());
        };
        if view.status != ProposalStatus::Prepared {
            return Err(GatewayError::ProposalUnavailable {
                reason: format!("proposal {}", view.status.name()),
            }
            .into());
        }
        Ok(view)
    }

    fn assert_digest(
        proposal: &ProposalRef,
        view: &ProposalView,
        req: &OperationRequest,
    ) -> Result<(), ErrorEnvelope> {
        let asserted = req
            .args()
            .get("digest")
            .and_then(Value::as_str)
            .ok_or_else(|| prepare::violation("args.digest", "expected a string"))?;
        if asserted != view.changeset.digest.as_str() {
            // SEC-09: the request names one proposal but asserts other
            // content — tampered or stale, never silently re-bound.
            return Err(GatewayError::DigestMismatch {
                proposal: proposal.as_str().to_owned(),
            }
            .into());
        }
        Ok(())
    }

    fn preview(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let proposal = Self::proposal_arg(req)?;
        let view = self.resolve_live(&proposal, req)?;
        if req.args().get("digest").is_some() {
            Self::assert_digest(&proposal, &view, req)?;
        }
        // The preview emits real content classes — same emission
        // boundary the prepare result passed (GT-004).
        let classes = prepare::emitted_classes(&view.changeset.effects);
        let decision = self.authorizer.authorize_emission(
            req,
            &self.preview,
            &classes,
            Self::is_external(req),
        );
        match decision {
            AuthorizationDecision::PermitForStage => {}
            AuthorizationDecision::Deny { reason } => {
                return Err(GatewayError::Denied {
                    stage: AuthorizationStage::PrepareProposal,
                    reason: Some(reason),
                }
                .into());
            }
            AuthorizationDecision::ApprovalRequired => {
                return Err(GatewayError::Internal {
                    reason: "preview emission must never require approval".to_owned(),
                }
                .into());
            }
        }
        Ok(GatewayOutcome::Completed {
            result: preview_value(&view),
        })
    }

    fn approve(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        // Issuance is a trusted-user act — an agent can never mint a
        // receipt, even with the capability inside its grant (GT-008).
        if req.caller().principal().kind() != PrincipalKind::Human {
            return Err(GatewayError::Denied {
                stage: AuthorizationStage::PrepareProposal,
                reason: Some("approval issuance is a trusted-user act".to_owned()),
            }
            .into());
        }
        let proposal = Self::proposal_arg(req)?;
        let view = self.resolve_live(&proposal, req)?;
        Self::assert_digest(&proposal, &view, req)?;
        let record = self
            .proposals
            .grant_approval(&proposal, req.caller().principal())
            .map_err(ErrorEnvelope::from)?;
        let receipt = record.receipt.expect("granted carries a receipt");
        Ok(GatewayOutcome::Completed {
            result: serde_json::json!({
                "approval": record.approval.as_str(),
                "status": record.status.name(),
                "approved_by": receipt.approved_by.id(),
                "for_principal": receipt.for_principal.id(),
                "project": receipt.project.to_tagged(),
                "grant": receipt.grant.as_ref().map(GrantRef::as_str),
                "task": receipt.task.as_ref().map(TaskRef::as_str),
                "proposal": receipt.proposal.as_str(),
                "digest": receipt.digest.as_str(),
                "expected_revision": receipt.expected_revision.0,
                "issued_at": receipt.issued_at.0,
                "expires_at": receipt.expires_at.0,
            }),
        })
    }

    /// Whether this apply must present a consumable receipt —
    /// derived from the *grant's* policy, never from a wire field:
    /// `ApproveToApply` requires one; `DelegatedApply` proceeds on
    /// grant+commit checks alone; humans apply on trusted authority.
    fn receipt_required(&self, req: &OperationRequest) -> bool {
        if req.caller().principal().kind() == PrincipalKind::Human {
            return false;
        }
        let Some(grant_ref) = req.grant() else {
            return true;
        };
        match self.store.grant(grant_ref) {
            Some(record) => record.grant.approval_policy == ApprovalPolicy::ApproveToApply,
            None => true,
        }
    }

    fn apply(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let proposal = Self::proposal_arg(req)?;
        let Some(view) = self
            .proposals
            .resolve(&proposal, req.caller().principal(), req.project())
        else {
            // Unknown, foreign, and inaccessible proposals share one
            // verdict — an agent never learns a proposal exists.
            return Err(GatewayError::ProposalUnavailable {
                reason: "unknown proposal".to_owned(),
            }
            .into());
        };
        Self::assert_digest(&proposal, &view, req)?;

        // Existing-result reference: an accepted-or-later apply is
        // referenced, never re-run — no second consume, no second
        // commit request (the GW-06 retry contract). The record is
        // the durable truth, so this answers even after the proposal
        // itself left `Prepared`.
        if let Some(apply) = &view.apply
            && apply.status.accepted_or_later()
        {
            return Ok(GatewayOutcome::Accepted {
                operation: apply.operation,
            });
        }
        if view.status != ProposalStatus::Prepared {
            return Err(GatewayError::ProposalUnavailable {
                reason: format!("proposal {}", view.status.name()),
            }
            .into());
        }

        // The request must assert exactly the prepared base — an
        // expected revision that moved on is rebase-by-assertion,
        // never allowed (ACG-002).
        let expected = req
            .expected_revision()
            .expect("apply is bound expected-revision");
        if expected != view.changeset.base.revision {
            return Err(GatewayError::RevisionConflict {
                expected: expected.0,
                actual: view.changeset.base.revision.0,
            }
            .into());
        }

        // SEC-11 revision fence: the prepared base must still be the
        // live tip. GW-06 re-checks inside its fence; this gate keeps
        // a Conflict from ever being presented as a success.
        let live = self.snapshots.latest_revision(req.project());
        if live != Some(view.changeset.base.revision) {
            self.proposals
                .fail_apply(&proposal, req, ApplyStatus::Conflict);
            return Err(GatewayError::RevisionConflict {
                expected: view.changeset.base.revision.0,
                actual: live.map_or(0, |ProjectRevision(r)| r),
            }
            .into());
        }

        // Accept is atomic with consumption — liveness, the retry
        // reference, and the receipt spend all commit under one
        // store lock, so consumption always means a real attempt
        // and precedes any success claim.
        let operation = self
            .proposals
            .accept_apply(
                &proposal,
                req,
                self.receipt_required(req),
                req.args().get("approval").and_then(Value::as_str),
            )
            .map_err(ErrorEnvelope::from)?;
        Ok(GatewayOutcome::Accepted { operation })
    }
}

impl CapabilityOperation for ProposalEngine {
    fn execute(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        match req.capability().id.as_str() {
            PREVIEW_ID => self.preview(req),
            APPROVE_ID => self.approve(req),
            APPLY_ID => self.apply(req),
            other => Err(GatewayError::Internal {
                reason: format!("proposal engine cannot serve {other}"),
            }
            .into()),
        }
    }
}

/// The gate-side helper the [`crate::proposal_policy::ProposalGate`]
/// consults: resolves the apply's proposal/digest pair into the
/// receipt verdict the evaluator needs.
#[allow(clippy::result_large_err)]
pub(crate) fn gate_probe(proposals: &ProposalStore, req: &OperationRequest) -> GateProbe {
    let Some(text) = req.args().get("proposal").and_then(Value::as_str) else {
        return GateProbe::NotAProposal;
    };
    let Ok(proposal) = ProposalRef::new(text) else {
        return GateProbe::NotAProposal;
    };
    let Some(view) = proposals.resolve(&proposal, req.caller().principal(), req.project()) else {
        return GateProbe::NotAProposal;
    };
    if view.status != ProposalStatus::Prepared {
        return GateProbe::NotAProposal;
    }
    let asserted = req
        .args()
        .get("digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if asserted != view.changeset.digest.as_str() {
        return GateProbe::NotAProposal;
    }
    if let Some(apply) = &view.apply
        && apply.status.accepted_or_later()
    {
        return GateProbe::OpenApply;
    }
    let presented = req.args().get("approval").and_then(Value::as_str);
    match proposals.approval_verdict(&proposal, req, presented) {
        ApprovalVerdict::Granted => GateProbe::Granted,
        ApprovalVerdict::Pending => GateProbe::Pending(proposal),
        ApprovalVerdict::Invalid(reason) => GateProbe::Invalid(reason),
    }
}

/// What the apply gate learned about the presented apply.
#[derive(Debug)]
pub(crate) enum GateProbe {
    /// The gate cannot judge — the executor will produce the typed
    /// error (`ProposalUnavailable`/`DigestMismatch`/malformed args).
    NotAProposal,
    /// An accepted-or-later apply exists — the retry references it.
    OpenApply,
    /// A fresh receipt is bound to this apply — consumable.
    Granted,
    /// The apply waits on the user — open/refresh the request.
    Pending(ProposalRef),
    /// The presented approval can never authorize this apply.
    Invalid(&'static str),
}
