//! The GW-02 authorizer adapter for GW-04 preparation.
//!
//! [`GrantPolicy`] owns its resource oracle at construction, so the
//! pinned-base facts a preparation must be judged against cannot ride
//! a long-lived evaluator — a live view would leak post-pin state
//! into the coverage verdict (the same rule GW-03's `admitted_scope`
//! encodes). [`PreparePolicy`] holds only the shared authority inputs
//! (store, clock) and instantiates a `GrantPolicy` per call with the
//! *pinned* oracle the engine supplies, then layers the two proposal-
//! specific checks on top of the standard effect evaluation:
//!
//! - **Inner-capability floor (SEC-13)** — every semantic-operation
//!   capability the proposal executes must be inside the grant's
//!   explicit set; `prepare` covering itself never covers its members.
//! - **After-state coverage (SEC-06)** — a write that moves a resource
//!   (range or owner changed) must land inside the same grant bound;
//!   the destination is re-checked with `GrantPolicy::coverage`
//!   verbatim, not an args-derived guess.
//!
//! `authorize_emission` delegates to the GW-03 disclosure boundary
//! verbatim — same binding chain, same external-disclosure class.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::PolicyClock;
use crate::authorization::AuthorizationDecision;
use crate::authorization::AuthorizationStage;
use crate::capability::{CapabilityDescriptor, DataAccessClass};
use crate::context::ContextAuthorizer;
use crate::grant::{ResourceFacts, ResourceOracle};
use crate::policy::{GrantPolicy, NO_AUTH, OUTSIDE_SCOPE, ScopeCounters};
use crate::prepare::{PrepareAuthorizer, PreparedEffects};
use crate::request::{OperationRequest, PrincipalKind};
use crate::store::PolicyStore;

/// An oracle that resolves nothing — the emission boundary never
/// consults resource facts, so this stands in where `GrantPolicy`
/// requires one.
struct NeverFacts;

impl ResourceOracle for NeverFacts {
    fn facts(
        &self,
        _project: synvelis_project::ProjectId,
        _resource: &crate::request::ResourceRef,
    ) -> ResourceFacts {
        ResourceFacts::Unresolved
    }
}

/// The production [`PrepareAuthorizer`]: a `GrantPolicy` factory
/// keyed to the pinned base, plus the proposal-specific checks
/// GW-02's evaluator does not own.
pub struct PreparePolicy {
    store: Arc<dyn PolicyStore>,
    clock: Arc<dyn PolicyClock>,
}

impl PreparePolicy {
    /// An authorizer over `store` (authority state) and `clock`
    /// (validity evaluation) — the same inputs a `GrantPolicy` takes.
    #[must_use]
    pub fn new(store: Arc<dyn PolicyStore>, clock: Arc<dyn PolicyClock>) -> Self {
        Self { store, clock }
    }

    /// The evaluator for one pinned base.
    fn evaluator(&self, facts: Arc<dyn ResourceOracle>) -> GrantPolicy {
        GrantPolicy::new(self.store.clone(), self.clock.clone(), facts)
    }
}

impl PrepareAuthorizer for PreparePolicy {
    fn authorize_effects(
        &self,
        req: &OperationRequest,
        capability: &CapabilityDescriptor,
        prepared: &PreparedEffects,
        facts: Arc<dyn ResourceOracle>,
    ) -> AuthorizationDecision {
        let policy = self.evaluator(facts);
        match policy.evaluate_effects(
            req,
            AuthorizationStage::PrepareProposal,
            capability,
            &prepared.scoped,
        ) {
            AuthorizationDecision::PermitForStage => {}
            other => return other,
        }
        // Humans carry trusted manual authority (AHG-007) — the grant
        // dimensions below exist only for delegated principals.
        if req.caller().principal().kind() == PrincipalKind::Human {
            return AuthorizationDecision::PermitForStage;
        }
        let Some(record) = policy.authorize_agent(req) else {
            return AuthorizationDecision::Deny {
                reason: NO_AUTH.to_owned(),
            };
        };
        for cap in &prepared.capabilities {
            if !record.grant.capabilities.contains(cap) {
                return AuthorizationDecision::Deny {
                    reason: "capability outside grant".to_owned(),
                };
            }
        }
        let mut counters = ScopeCounters::default();
        for (resource, after) in &prepared.after_states {
            if GrantPolicy::coverage(&record.grant, resource, after, &mut counters).is_none() {
                return AuthorizationDecision::Deny {
                    reason: OUTSIDE_SCOPE.to_owned(),
                };
            }
        }
        AuthorizationDecision::PermitForStage
    }

    fn authorize_emission(
        &self,
        req: &OperationRequest,
        capability: &CapabilityDescriptor,
        classes: &BTreeSet<DataAccessClass>,
        external: bool,
    ) -> AuthorizationDecision {
        self.evaluator(Arc::new(NeverFacts))
            .authorize_disclosure(req, capability, classes, external)
    }
}
