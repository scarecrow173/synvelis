//! Authorization seam (GTC-003) — stages, decisions, and the
//! evaluator trait GW-02 will implement.
//!
//! GW-01 owns only the *contract*: which stage an operation domain maps
//! to, what a decision looks like, and when the evaluator runs (inside
//! [`crate::Gateway::invoke`], before the executor). It deliberately
//! ships a fail-closed [`DenyAllPolicy`] default — a capability existing
//! in the registry grants nothing (GT-003).

use serde::{Deserialize, Serialize};

use crate::capability::CapabilityDescriptor;
use crate::request::OperationRequest;

/// The logical authorization stage being evaluated (GTC-003).
///
/// A permit is stage-scoped: `PermitForStage` is neither a commit token
/// nor a standing grant.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorizationStage {
    /// Discover/list capability metadata.
    DiscoverCapability,
    /// Read context through a query operation.
    ReadContext,
    /// Prepare a proposal (GW-04).
    PrepareProposal,
    /// Request application of a project mutation.
    RequestApply,
    /// Final commit authorization (GW-06 boundary).
    Commit,
    /// Start a runtime-control operation (transport).
    StartRuntimeOperation,
    /// Start an asynchronous job.
    StartAsyncOperation,
    /// Read an operation's result/handle.
    ReadOperationResult,
}

impl AuthorizationStage {
    /// The kebab-case stage name used in external messages.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::DiscoverCapability => "discover-capability",
            Self::ReadContext => "read-context",
            Self::PrepareProposal => "prepare-proposal",
            Self::RequestApply => "request-apply",
            Self::Commit => "commit",
            Self::StartRuntimeOperation => "start-runtime-operation",
            Self::StartAsyncOperation => "start-async-operation",
            Self::ReadOperationResult => "read-operation-result",
        }
    }
}

/// The evaluator's decision for one request at one stage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthorizationDecision {
    /// Permitted for this stage only.
    PermitForStage,
    /// An approval must be obtained first (GW-05/06 path).
    ApprovalRequired,
    /// Denied.
    Deny {
        /// Safe-to-disclose reason.
        reason: String,
    },
}

/// The policy evaluation seam (GTC-003).
///
/// GW-01 supplies [`DenyAllPolicy`] (the default) and fixture evaluators
/// for contract tests; real principal/grant/scope evaluation is GW-02.
/// The evaluator sees the *validated* request plus the resolved
/// capability descriptor — it never sees raw wire data.
pub trait PolicyEvaluator: Send + Sync {
    /// Decides whether `req` may proceed at `stage`.
    fn decide(
        &self,
        req: &OperationRequest,
        stage: AuthorizationStage,
        capability: &CapabilityDescriptor,
    ) -> AuthorizationDecision;
}

/// The fail-closed default: denies every stage for every caller.
///
/// This is what makes *capability existence ≠ permission* executable
/// instead of aspirational — with no policy installed, nothing runs.
#[derive(Copy, Clone, Debug, Default)]
pub struct DenyAllPolicy;

impl PolicyEvaluator for DenyAllPolicy {
    fn decide(
        &self,
        _req: &OperationRequest,
        stage: AuthorizationStage,
        _capability: &CapabilityDescriptor,
    ) -> AuthorizationDecision {
        AuthorizationDecision::Deny {
            reason: format!(
                "no policy installed; stage {} denied by default",
                stage.name()
            ),
        }
    }
}
