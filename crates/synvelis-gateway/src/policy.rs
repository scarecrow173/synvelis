//! [`GrantPolicy`] — the GW-02 policy evaluator (GTC-003, AHG-006..010).
//!
//! Wired into [`Gateway::with_evaluator`][crate::Gateway::with_evaluator],
//! this evaluator decides each [`AuthorizationStage`] from the trusted
//! caller, the authorized task, the resolved grant, and the policy
//! store's current state — never from wire self-declarations.
//!
//! # Authority model (AHG-007)
//!
//! - **Manual authority**: [`PrincipalKind::Human`]. A human principal
//!   can only be attested by a trusted adapter (the [`AdapterSeal`]
//!   boundary — wire self-declaration cannot produce one, GT-002).
//!   Manual DAW operation does not depend on agent grants, so human
//!   callers are permitted at every stage.
//! - **Agent authority**: [`PrincipalKind::Agent`] requires *both* an
//!   authorized task (activation) *and* a matching execution grant.
//!   Grant permission and AI-start permission are independent
//!   contracts; a delegated-apply grant is not proactive-activation
//!   authority.
//!
//! # Stage separation (GTC-003)
//!
//! - **Preflight** — every stage evaluates target coverage, capability,
//!   permission, and data-access against the grant. A `PermitForStage`
//!   at an early stage is not a commit token.
//! - **Effect-aware** — [`evaluate_effects`][GrantPolicy::evaluate_effects]
//!   takes the Synvelis-derived [`ScopedEffects`] (GW-04's output view)
//!   and re-checks every actually-affected resource/class.
//! - **Commit-time** — [`evaluate_commit`][GrantPolicy::evaluate_commit]
//!   re-runs the full evaluation against *current* store state for
//!   GW-06's commit fence. A revoke confirmed before that read is seen
//!   and denies; one confirmed after does not undo the commit (ACG-003).
//!   The delegated-apply budget binds here: one unit per commit
//!   authorization, spent atomically with the fence read — preflight
//!   evaluations never spend.
//!
//! # Deny reasons and existence
//!
//! Before a grant is proven bound to the calling principal+project,
//! every failure shares one reason — `no applicable authorization` —
//! so probing cannot learn whether a grant, task, or resource exists.
//! Once the binding is proven, reasons name the policy dimension
//! (expired, revoked, scope, data access) without naming resources.

use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;

use serde_json::Value;
use synvelis_project::ProjectId;

use crate::authorization::{AuthorizationDecision, AuthorizationStage, PolicyEvaluator};
use crate::capability::{CapabilityDescriptor, DataAccessClass, EffectKind};
use crate::clock::PolicyClock;
use crate::grant::{
    ApprovalPolicy, CreateWithin, Grant, GrantBinding, MoveArgs, OwnerConstraint,
    ResourceAttributes, ResourceFacts, ResourceOracle, ResourceScope, ScopedEffects,
};
use crate::request::{
    OperationRequest, OperationTarget, PrincipalKind, ResourceKind, ResourceRef, TimeDomain,
    TypedRange,
};
use crate::store::{Activation, GrantRecord, PolicyStore};

/// Uniform pre-binding deny reason — no task/grant/resource existence
/// is disclosed before ownership is proven.
const NO_AUTH: &str = "no applicable authorization";

/// Deny reason when a resource fails scope coverage — deliberately
/// shared between "does not exist", "exists but outside scope", and
/// "unresolvable" so the verdict leaks no existence information.
/// Crate-visible: the GW-03 context engine's explicit-selector verdict
/// emits this same reason (one string, no drift between copies).
pub(crate) const OUTSIDE_SCOPE: &str = "target outside grant scope";

/// Deny reason when a create exceeds `create_within`.
const OUTSIDE_CREATE: &str = "creation outside grant constraints";

/// Per-request count of distinct dynamically-covered members —
/// compared against [`DynamicScope::max_members`][crate::grant::DynamicScope].
/// Shared across the target and effects passes of one evaluation, and
/// deduplicated by resource: the cap spans the whole request, and a
/// member appearing on both axes is counted once.
#[derive(Default)]
struct ScopeCounters {
    dynamic_members: u32,
    counted: BTreeSet<ResourceRef>,
}

/// The GW-02 policy evaluator.
///
/// Composes the authority store, the resource oracle, and the policy
/// clock. All three are injected — the store is the mock
/// [`InMemoryPolicyStore`][crate::InMemoryPolicyStore] or a future
/// durable one (GW-06), the oracle is the host's resource view
/// (GW-03/04 project snapshots; a static oracle in tests), the clock
/// pins validity evaluation.
pub struct GrantPolicy {
    store: Arc<dyn PolicyStore>,
    clock: Arc<dyn PolicyClock>,
    oracle: Arc<dyn ResourceOracle>,
}

impl GrantPolicy {
    /// An evaluator over `store`, `clock`, and `oracle`.
    #[must_use]
    pub fn new(
        store: Arc<dyn PolicyStore>,
        clock: Arc<dyn PolicyClock>,
        oracle: Arc<dyn ResourceOracle>,
    ) -> Self {
        Self {
            store,
            clock,
            oracle,
        }
    }

    /// The policy store (for administrative issue/revoke calls).
    #[must_use]
    pub fn store(&self) -> &Arc<dyn PolicyStore> {
        &self.store
    }

    /// Commit-time revalidation for GW-06's authorized-commit fence
    /// (ACG-003 interface contract).
    ///
    /// This is the same evaluation as [`AuthorizationStage::Commit`],
    /// exposed as a named entry point so the fence calls the seam
    /// explicitly rather than re-deriving a stage. It performs a
    /// *fresh* store read — a preflight `PermitForStage` is never
    /// reused as commit authority. The caller must serialize this read
    /// with the project commit so a concurrent `revoke_grant` lands on
    /// exactly one side of the linearization point:
    ///
    /// - revoke confirmed **before** the fence read → `Deny`
    /// - commit authorized **before** the revoke is confirmed → the
    ///   commit stands (revocation is not auto-undo)
    ///
    /// Final cross-store atomicity (policy store ↔ project store) is
    /// owned and judged by GW-06 — this is the seam it calls inside
    /// the fence, not the fence itself.
    #[must_use]
    pub fn evaluate_commit(
        &self,
        req: &OperationRequest,
        capability: &CapabilityDescriptor,
    ) -> AuthorizationDecision {
        self.evaluate(req, AuthorizationStage::Commit, capability)
    }

    /// Effect-aware authorization (GTC-003 middle stage).
    ///
    /// Runs the full base chain, then checks the Synvelis-derived
    /// `effects` — every actually read/written/deleted resource against
    /// the grant's scope, every create against `create_within`, every
    /// disclosure class and provider transfer against `data_access`.
    /// Any effect the derivation could not classify
    /// (`effects.unsupported`) denies outright — unknown side effects
    /// are never auto-permitted, under any apply mode (SEC-14).
    ///
    /// `stage` selects the apply gate evaluated afterwards — callers
    /// should pass the stage the effects were derived for
    /// (`RequestApply`/`Commit`).
    #[must_use]
    pub fn evaluate_effects(
        &self,
        req: &OperationRequest,
        stage: AuthorizationStage,
        capability: &CapabilityDescriptor,
        effects: &ScopedEffects,
    ) -> AuthorizationDecision {
        self.evaluate_with(req, stage, capability, Some(effects))
    }

    /// The full evaluation, shared by every entry point.
    fn evaluate(
        &self,
        req: &OperationRequest,
        stage: AuthorizationStage,
        capability: &CapabilityDescriptor,
    ) -> AuthorizationDecision {
        self.evaluate_with(req, stage, capability, None)
    }

    fn evaluate_with(
        &self,
        req: &OperationRequest,
        stage: AuthorizationStage,
        capability: &CapabilityDescriptor,
        effects: Option<&ScopedEffects>,
    ) -> AuthorizationDecision {
        // The descriptor must describe the capability the request
        // names — a caller passing a weakened descriptor for the same
        // ref must not soften the declared bounds.
        if capability.id != req.capability().id || capability.version != req.capability().version {
            return AuthorizationDecision::Deny {
                reason: "capability descriptor mismatch".to_owned(),
            };
        }
        if req.caller().principal().kind() == PrincipalKind::Human {
            // Trusted manual authority — manual DAW operation never
            // depends on agent grants (AHG-007). The human is the
            // verifier for effect-level checks too: unsupported/
            // unverifiable effects constrain *delegated* authority,
            // not the manual operator.
            return AuthorizationDecision::PermitForStage;
        }
        let Some(record) = self.authorize_agent(req) else {
            return AuthorizationDecision::Deny {
                reason: NO_AUTH.to_owned(),
            };
        };
        if let Some(reason) = self.check_grant_dimensions(req, &record.grant, capability) {
            return AuthorizationDecision::Deny { reason };
        }
        // One counter set spans the whole request — target coverage
        // and derived effects draw from the same `max_members` budget.
        let mut counters = ScopeCounters::default();
        if let Some(reason) = self.check_target_scope(req, &record.grant, capability, &mut counters)
        {
            return AuthorizationDecision::Deny { reason };
        }
        if let Some(effects) = effects
            && let Some(reason) = self.check_effects(req, &record.grant, effects, &mut counters)
        {
            return AuthorizationDecision::Deny { reason };
        }
        self.stage_gate(stage, &record)
    }

    /// Resolves the authorized task and the bound grant for an agent
    /// caller. Every failure returns `None` — nothing about existence
    /// is disclosed before the grant is proven bound to this
    /// principal+project.
    ///
    /// Crate-internal: the GW-03 context authorizer reuses this same
    /// binding proof rather than re-deriving task/grant resolution
    /// (one authoritative chain).
    pub(crate) fn authorize_agent(&self, req: &OperationRequest) -> Option<GrantRecord> {
        let caller = req.caller();
        let now = self.clock.now();

        // Activation: the task ref must resolve to a live task bound to
        // this principal and project (AHG-007).
        let task = self.store.task(req.task()?)?;
        if task.principal != *caller.principal()
            || task.project != req.project()
            || !task.validity.live(now)
        {
            return None;
        }
        match &task.activation {
            Activation::UserIssued => {}
            Activation::Proactive { permit } => {
                let permit = self.store.proactive_permit(permit)?;
                if permit.principal != *caller.principal()
                    || permit.project != req.project()
                    || !permit.validity.live(now)
                {
                    // A delegated-apply grant is not proactive
                    // authority — only a live permit activates
                    // (AHG-007).
                    return None;
                }
            }
        }

        // Execution grant: resolve and prove binding.
        let record = self.store.grant(req.grant()?)?;
        let grant = &record.grant;
        if grant.principal != *caller.principal() || grant.project != req.project() {
            return None;
        }
        let bound = match &grant.binding {
            GrantBinding::Task { task } => req.task().is_some_and(|t| t == task),
            GrantBinding::Session { adapter } => *adapter == caller.adapter(),
            GrantBinding::PersistentProject => true,
        };
        bound.then_some(record)
    }

    /// Grant dimensions evaluated after binding is proven — reasons
    /// may name the policy dimension but never a resource.
    fn check_grant_dimensions(
        &self,
        req: &OperationRequest,
        grant: &Grant,
        capability: &CapabilityDescriptor,
    ) -> Option<String> {
        let now = self.clock.now();
        if grant.validity.revoked {
            return Some("grant revoked".to_owned());
        }
        if grant.validity.expires_at.is_some_and(|at| now > at) {
            return Some("grant expired".to_owned());
        }
        if grant
            .epoch_ceiling
            .is_some_and(|ceiling| self.store.policy_epoch() > ceiling)
        {
            return Some("policy version superseded".to_owned());
        }
        if !grant.capabilities.contains(req.capability()) {
            return Some("capability outside grant".to_owned());
        }
        if !capability
            .required_permissions
            .is_subset_of(&grant.permissions)
        {
            return Some("required permissions outside grant".to_owned());
        }
        if !capability.data_access.is_subset_of(&grant.data_access) {
            return Some("data access outside grant".to_owned());
        }
        None
    }

    /// The apply-mode gate — derived from the grant's approval policy,
    /// never from anything the agent declared (AHG-009). Stages are
    /// enumerated explicitly: a stage added later does not inherit a
    /// permissive default by accident.
    fn stage_gate(&self, stage: AuthorizationStage, record: &GrantRecord) -> AuthorizationDecision {
        let deny = |reason: &str| AuthorizationDecision::Deny {
            reason: reason.to_owned(),
        };
        match stage {
            AuthorizationStage::RequestApply => match record.grant.approval_policy {
                ApprovalPolicy::Suggest => {
                    deny("apply is not permitted under a suggest-mode grant")
                }
                ApprovalPolicy::ApproveToApply => AuthorizationDecision::ApprovalRequired,
                ApprovalPolicy::DelegatedApply => {
                    // Preflight only reports — consumption is monotone
                    // (single-issuance store), so an exhausted snapshot
                    // denies accurately. The unit itself spends at the
                    // Commit authorization.
                    if record
                        .grant
                        .limits
                        .apply_budget
                        .is_some_and(|b| record.consumed_applies >= b)
                    {
                        deny("apply budget exhausted")
                    } else {
                        AuthorizationDecision::PermitForStage
                    }
                }
            },
            AuthorizationStage::Commit => match record.grant.approval_policy {
                ApprovalPolicy::Suggest => {
                    deny("apply is not permitted under a suggest-mode grant")
                }
                // Receipt verification belongs to GW-05/06 — the gate
                // reports the requirement, not a permit.
                ApprovalPolicy::ApproveToApply => AuthorizationDecision::ApprovalRequired,
                ApprovalPolicy::DelegatedApply => {
                    // The budget unit binds here: the commit
                    // authorization is where an apply actually becomes
                    // authorized, and `try_consume_apply` revalidates
                    // liveness/epoch/budget on the current record
                    // atomically (ACG-003). Exactly one unit per commit
                    // authorization — preflight evaluations never spend.
                    if self
                        .store
                        .try_consume_apply(&record.grant.id, self.clock.now())
                    {
                        AuthorizationDecision::PermitForStage
                    } else {
                        deny(&self.spend_failure_reason(&record.grant.id))
                    }
                }
            },
            AuthorizationStage::DiscoverCapability
            | AuthorizationStage::ReadContext
            | AuthorizationStage::PrepareProposal
            | AuthorizationStage::StartRuntimeOperation
            | AuthorizationStage::StartAsyncOperation
            | AuthorizationStage::ReadOperationResult => AuthorizationDecision::PermitForStage,
        }
    }

    /// Why a commit-time apply spend failed — re-reads the record to
    /// name the actual dimension (a revoke or expiry landing between
    /// the decision read and the spend is reported precisely).
    fn spend_failure_reason(&self, id: &crate::id::GrantRef) -> String {
        let now = self.clock.now();
        match self.store.grant(id) {
            Some(fresh) if fresh.grant.validity.revoked => "grant revoked".to_owned(),
            Some(fresh) if fresh.grant.validity.expires_at.is_some_and(|at| now > at) => {
                "grant expired".to_owned()
            }
            Some(fresh)
                if fresh
                    .grant
                    .epoch_ceiling
                    .is_some_and(|c| self.store.policy_epoch() > c) =>
            {
                "policy version superseded".to_owned()
            }
            _ => "apply budget exhausted".to_owned(),
        }
    }

    /// Target-level scope evaluation (preflight). Returns a deny
    /// reason on failure.
    fn check_target_scope(
        &self,
        req: &OperationRequest,
        grant: &Grant,
        capability: &CapabilityDescriptor,
        counters: &mut ScopeCounters,
    ) -> Option<String> {
        match req.target() {
            OperationTarget::None => None,
            OperationTarget::Create { parent, creates } => {
                // The create axis is off by default — a grant that
                // names no `create_within` permits no creation.
                let Some(within) = grant.create_within.as_ref() else {
                    return Some(OUTSIDE_CREATE.to_owned());
                };
                let placed = within.placement.as_ref().and_then(|p| {
                    req.args()
                        .get(&p.arg)
                        .and_then(|v| parse_typed_range(v).ok())
                });
                self.check_create(req.project(), within, *parent, *creates, placed)
            }
            OperationTarget::Resource { resource } => {
                self.check_resources(req, grant, capability, vec![*resource], counters)
            }
            OperationTarget::ResourceSet { resources } => {
                self.check_resources(req, grant, capability, resources.clone(), counters)
            }
        }
    }

    /// Whether every `targets` member is covered by the grant's scope.
    /// Related resources (linked/shared sources) are followed
    /// transitively and must be covered independently — clip
    /// permission never implies source permission (SEC-05, AHG-008).
    /// Related coverage is required only when the capability's
    /// declared effect bound includes a mutating effect; the precise
    /// per-resource write check is the effect-aware stage's.
    fn check_resources(
        &self,
        req: &OperationRequest,
        grant: &Grant,
        capability: &CapabilityDescriptor,
        targets: Vec<ResourceRef>,
        counters: &mut ScopeCounters,
    ) -> Option<String> {
        let mutating = capability.effect_bound.iter().any(|e| {
            matches!(
                e,
                EffectKind::Write | EffectKind::Create | EffectKind::Delete
            )
        });
        let mut visited = BTreeSet::new();
        let mut queue: VecDeque<(ResourceRef, bool)> =
            targets.into_iter().map(|r| (r, true)).collect();
        while let Some((resource, direct)) = queue.pop_front() {
            if !visited.insert(resource) {
                continue;
            }
            let ResourceFacts::Known(attrs) = self.oracle.facts(req.project(), &resource) else {
                return Some(OUTSIDE_SCOPE.to_owned());
            };
            if Self::coverage(grant, &resource, &attrs, counters).is_none() {
                return Some(OUTSIDE_SCOPE.to_owned());
            }
            // A move/reparent re-derives the member's after-state from
            // the declared args fields — the destination must satisfy
            // the same constraints (SEC-06, AHG-008). The member was
            // already counted once; the deduplicated counter does not
            // re-count it for the after-state check.
            if direct
                && let ResourceScope::Dynamic(dynamic) = &grant.scope
                && let Some(move_args) = &dynamic.move_args
            {
                match after_attributes(&attrs, req.args(), move_args) {
                    Some(after) if Self::coverage(grant, &resource, &after, counters).is_some() => {
                    }
                    _ => return Some(OUTSIDE_SCOPE.to_owned()),
                }
            }
            if mutating {
                queue.extend(attrs.related.iter().map(|r| (*r, false)));
            }
        }
        None
    }

    /// Classifies `resource`'s membership under `grant.scope` —
    /// `None` means not covered. Dynamic coverage is counted in
    /// `counters` and capped by `max_members`.
    fn coverage(
        grant: &Grant,
        resource: &ResourceRef,
        attrs: &ResourceAttributes,
        counters: &mut ScopeCounters,
    ) -> Option<()> {
        match &grant.scope {
            ResourceScope::Exact { resources } | ResourceScope::Snapshot { resources, .. } => {
                resources.contains(resource).then_some(())
            }
            ResourceScope::Dynamic(dynamic) => {
                if !dynamic.kinds.contains(&attrs.kind) {
                    return None;
                }
                match &dynamic.owners {
                    OwnerConstraint::Any => {}
                    OwnerConstraint::OneOf { refs } => {
                        if !attrs.owner.is_some_and(|o| refs.contains(&o)) {
                            return None;
                        }
                    }
                }
                if let Some(bound) = &dynamic.range {
                    let member = attrs.range?;
                    if !range_within(bound, &member) {
                        return None;
                    }
                }
                // Preexisting members joined at issuance; later-created
                // ones need explicit future inclusion — a new object
                // never slips in silently (AHG-008).
                let preexisting = match (attrs.created_revision, grant.issued_at_revision) {
                    (Some(created), Some(issued)) => created <= issued,
                    _ => false,
                };
                if !preexisting && !dynamic.include_future {
                    return None;
                }
                // Each distinct member spends one dynamic-membership
                // unit per request — re-encounters (related traversal,
                // after-state re-check, effects overlap) are covered
                // without re-spending.
                if counters.counted.insert(*resource) {
                    counters.dynamic_members = counters.dynamic_members.saturating_add(1);
                    if dynamic
                        .max_members
                        .is_some_and(|max| counters.dynamic_members > max)
                    {
                        return None;
                    }
                }
                Some(())
            }
        }
    }

    /// The create axis — parent resolvability, permitted parents,
    /// creatable kinds, and the placement bound. Used by the `Create`
    /// target (arg-derived placement) and by [`CreateEffect`]s (typed
    /// placement).
    fn check_create(
        &self,
        project: ProjectId,
        within: &CreateWithin,
        parent: ResourceRef,
        creates: ResourceKind,
        placed: Option<TypedRange>,
    ) -> Option<String> {
        // The parent must be resolvable to be verified — an
        // unresolvable parent denies like any other unknown resource.
        if !matches!(self.oracle.facts(project, &parent), ResourceFacts::Known(_)) {
            return Some(OUTSIDE_CREATE.to_owned());
        }
        match &within.parents {
            OwnerConstraint::Any => {}
            OwnerConstraint::OneOf { refs } => {
                if !refs.contains(&parent) {
                    return Some(OUTSIDE_CREATE.to_owned());
                }
            }
        }
        if !within.kinds.contains(&creates) {
            return Some(OUTSIDE_CREATE.to_owned());
        }
        if let Some(placement) = &within.placement {
            match placed {
                Some(range) if range_within(&placement.bound, &range) => {}
                _ => return Some(OUTSIDE_CREATE.to_owned()),
            }
        }
        None
    }

    /// Effect-aware evaluation over the Synvelis-derived effect set.
    /// Shares the request's `counters` — dynamic membership is capped
    /// across target and effects together.
    fn check_effects(
        &self,
        req: &OperationRequest,
        grant: &Grant,
        effects: &ScopedEffects,
        counters: &mut ScopeCounters,
    ) -> Option<String> {
        if !effects.unsupported.is_empty() {
            // Opaque/unverifiable effects never auto-permit (SEC-14).
            return Some("unverifiable effects".to_owned());
        }
        for resource in effects
            .reads
            .iter()
            .chain(&effects.writes)
            .chain(&effects.deletes)
        {
            let ResourceFacts::Known(attrs) = self.oracle.facts(req.project(), resource) else {
                return Some(OUTSIDE_SCOPE.to_owned());
            };
            if Self::coverage(grant, resource, &attrs, counters).is_none() {
                return Some(OUTSIDE_SCOPE.to_owned());
            }
        }
        for create in &effects.creates {
            if let Some(within) = grant.create_within.as_ref() {
                if let reason @ Some(_) = self.check_create(
                    req.project(),
                    within,
                    create.parent,
                    create.creates,
                    create.range,
                ) {
                    return reason;
                }
            } else {
                return Some(OUTSIDE_CREATE.to_owned());
            }
        }
        for class in &effects.disclosures {
            if !grant.data_access.contains(*class) {
                return Some("data access outside grant".to_owned());
            }
        }
        if effects.external_transfers > 0
            && !grant
                .data_access
                .contains(DataAccessClass::ProviderTransfer)
        {
            return Some("data access outside grant".to_owned());
        }
        None
    }
}

impl PolicyEvaluator for GrantPolicy {
    fn decide(
        &self,
        req: &OperationRequest,
        stage: AuthorizationStage,
        capability: &CapabilityDescriptor,
    ) -> AuthorizationDecision {
        self.evaluate(req, stage, capability)
    }
}

impl crate::context::ContextAuthorizer for GrantPolicy {
    /// Scope admission for a context query's candidate set (GTC-004).
    ///
    /// Runs the same base chain as a read evaluation — binding proof,
    /// grant dimensions — then classifies each candidate by scope
    /// coverage. Coverage is a *filter* here: uncovered or
    /// unresolvable members are dropped, never named (the caller
    /// decides whether the filtered shape denies — an explicit id
    /// list denies on any drop, an enumeration emits the subset).
    /// The dynamic-membership budget is shared across the candidate
    /// set, so a capped scope admits at most `max_members`.
    ///
    /// `facts` is the pinned-snapshot fact source — scope coverage is
    /// judged on the attributes the projection will actually disclose,
    /// never a newer live view (DATA-03).
    fn admitted_scope(
        &self,
        req: &OperationRequest,
        capability: &CapabilityDescriptor,
        candidates: &[ResourceRef],
        facts: &dyn ResourceOracle,
    ) -> Result<crate::context::ScopeAdmission, String> {
        if capability.id != req.capability().id || capability.version != req.capability().version {
            return Err("capability descriptor mismatch".to_owned());
        }
        if req.caller().principal().kind() == PrincipalKind::Human {
            return Ok(crate::context::ScopeAdmission::All);
        }
        let Some(record) = self.authorize_agent(req) else {
            return Err(NO_AUTH.to_owned());
        };
        if let Some(reason) = self.check_grant_dimensions(req, &record.grant, capability) {
            return Err(reason);
        }
        let mut counters = ScopeCounters::default();
        let mut covered = Vec::with_capacity(candidates.len());
        for resource in candidates {
            let ResourceFacts::Known(attrs) = facts.facts(req.project(), resource) else {
                continue;
            };
            if Self::coverage(&record.grant, resource, &attrs, &mut counters).is_some() {
                covered.push(*resource);
            }
        }
        Ok(crate::context::ScopeAdmission::Subset(covered))
    }

    /// The disclosure boundary (GTC-004 / GT-004).
    ///
    /// A fresh policy-chain read (task/grant binding, dimensions) plus
    /// the emitted `classes` against the grant's `data_access`. An
    /// `external` caller additionally needs
    /// [`DataAccessClass::ExternalDisclosure`] — an id, cursor, or
    /// operation handle never discloses by itself. This is distinct
    /// from the preflight `decide(ReadContext)`, which only checks
    /// the capability's declared `data_access` *bound*; the classes
    /// checked here are the ones the response actually carries.
    fn authorize_disclosure(
        &self,
        req: &OperationRequest,
        capability: &CapabilityDescriptor,
        classes: &BTreeSet<DataAccessClass>,
        external: bool,
    ) -> AuthorizationDecision {
        if capability.id != req.capability().id || capability.version != req.capability().version {
            return AuthorizationDecision::Deny {
                reason: "capability descriptor mismatch".to_owned(),
            };
        }
        if req.caller().principal().kind() == PrincipalKind::Human {
            return AuthorizationDecision::PermitForStage;
        }
        let Some(record) = self.authorize_agent(req) else {
            return AuthorizationDecision::Deny {
                reason: NO_AUTH.to_owned(),
            };
        };
        if let Some(reason) = self.check_grant_dimensions(req, &record.grant, capability) {
            return AuthorizationDecision::Deny { reason };
        }
        let mut emitted = classes.clone();
        if external {
            emitted.insert(DataAccessClass::ExternalDisclosure);
        }
        if !emitted
            .iter()
            .all(|class| record.grant.data_access.contains(*class))
        {
            return AuthorizationDecision::Deny {
                reason: "data access outside grant".to_owned(),
            };
        }
        self.stage_gate(AuthorizationStage::ReadContext, &record)
    }
}

/// Whether `member`'s extent is fully contained in `bound`, in the
/// same time domain — checked arithmetic, no implicit conversion
/// (AHG-008).
fn range_within(bound: &TypedRange, member: &TypedRange) -> bool {
    let (Some(bound_end), Some(member_end)) = (
        bound.start.checked_add(bound.len),
        member.start.checked_add(member.len),
    ) else {
        return false;
    };
    member.domain == bound.domain && member.start >= bound.start && member_end <= bound_end
}

/// Re-derives a member's after-state for move/reparent evaluation —
/// the declared args fields override the owner and/or range. Returns
/// `None` when a declared field is present but malformed (the
/// after-state cannot be verified → deny).
fn after_attributes(
    attrs: &ResourceAttributes,
    args: &Value,
    move_args: &MoveArgs,
) -> Option<ResourceAttributes> {
    let mut after = attrs.clone();
    if let Some(field) = &move_args.parent_arg
        && let Some(value) = args.get(field)
    {
        after.owner = Some(ResourceRef::parse_tagged(value.as_str()?).ok()?);
    }
    if let Some(field) = &move_args.range_arg
        && let Some(value) = args.get(field)
    {
        after.range = Some(parse_typed_range(value).ok()?);
    }
    Some(after)
}

/// Parses a `{domain,start,len}` object into a [`TypedRange`].
fn parse_typed_range(value: &Value) -> Result<TypedRange, ()> {
    let obj = value.as_object().ok_or(())?;
    let domain = match obj.get("domain").and_then(Value::as_str).ok_or(())? {
        "project-musical" => TimeDomain::ProjectMusical,
        "media" => TimeDomain::Media,
        "source-native" => TimeDomain::SourceNative,
        _ => return Err(()),
    };
    let start = obj.get("start").and_then(Value::as_i64).ok_or(())?;
    let len = obj.get("len").and_then(Value::as_i64).ok_or(())?;
    if len <= 0 {
        return Err(());
    }
    Ok(TypedRange { domain, start, len })
}
