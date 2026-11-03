//! The GW-05 apply gate — a [`PolicyEvaluator`] decorator that adds
//! the approval-receipt question to `RequestApply` on
//! `synvelis.proposal.apply`.
//!
//! Every other request delegates to the inner evaluator unchanged.
//! For the apply capability the decision is layered on the standard
//! GW-02 verdict:
//!
//! - `Deny` — forwarded as-is (binding, scope, liveness, Suggest mode:
//!   a revoked grant denies before any receipt is consulted — SEC-12).
//! - `PermitForStage` — `DelegatedApply` or a trusted user: the apply
//!   proceeds on grant+commit checks alone, no receipt needed.
//! - `ApprovalRequired` — `ApproveToApply`: the gate looks into the
//!   proposal store. A fresh receipt bound to this exact apply
//!   (principal/project/grant/task/digest/revision — SEC-09/10) flips
//!   the verdict to `PermitForStage`; the executor then consumes the
//!   receipt atomically. Anything else keeps `ApprovalRequired` —
//!   and records the `Requested` approval plus the `AwaitingApproval`
//!   apply attempt so the lifecycle machines stay observable — while
//!   a presented-but-invalid receipt denies outright.
//!
//! When the gate cannot judge (unresolvable proposal, dead proposal,
//! mismatched digest assertion) it permits — the apply executor owns
//! the typed consistency errors (`ProposalUnavailable`,
//! `DigestMismatch`, `RevisionConflict`).

use std::sync::Arc;

use crate::authorization::{AuthorizationDecision, AuthorizationStage, PolicyEvaluator};
use crate::capability::CapabilityDescriptor;
use crate::proposal::ProposalStore;
use crate::proposal_engine::{APPLY_ID, GateProbe, gate_probe};
use crate::request::OperationRequest;

/// The apply-side approval gate — wraps the real evaluator for the
/// one capability whose permit needs a receipt verdict.
pub struct ProposalGate {
    inner: Box<dyn PolicyEvaluator>,
    proposals: Arc<ProposalStore>,
}

impl ProposalGate {
    /// A gate over the shared proposal store, delegating every
    /// non-apply decision to `inner`.
    #[must_use]
    pub fn new(proposals: Arc<ProposalStore>, inner: Box<dyn PolicyEvaluator>) -> Self {
        Self { inner, proposals }
    }
}

impl PolicyEvaluator for ProposalGate {
    fn decide(
        &self,
        req: &OperationRequest,
        stage: AuthorizationStage,
        capability: &CapabilityDescriptor,
    ) -> AuthorizationDecision {
        if stage != AuthorizationStage::RequestApply || req.capability().id.as_str() != APPLY_ID {
            return self.inner.decide(req, stage, capability);
        }
        match self.inner.decide(req, stage, capability) {
            AuthorizationDecision::ApprovalRequired => {}
            other => return other,
        }
        match gate_probe(&self.proposals, req) {
            // The executor owns these errors — the gate's job is only
            // the receipt question.
            GateProbe::NotAProposal | GateProbe::OpenApply | GateProbe::Granted => {
                AuthorizationDecision::PermitForStage
            }
            GateProbe::Pending(proposal) => {
                // Bookkeeping for the lifecycle machines: the request
                // exists (or is opened) and this attempt waits on it.
                // Idempotent — a retried request keeps its records,
                // re-pointing the wire-visible approval ref at this
                // attempt.
                self.proposals.request_approval(&proposal, req);
                self.proposals.mark_apply_awaiting(&proposal, req);
                AuthorizationDecision::ApprovalRequired
            }
            GateProbe::Invalid(reason) => AuthorizationDecision::Deny {
                reason: reason.to_owned(),
            },
        }
    }
}
