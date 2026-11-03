//! Execution-grant and resource-scope types (GW-02; AHG-007/008/009/010).
//!
//! A [`Grant`] is the Synvelis-owned authority record an agent-side
//! invocation is evaluated against. It is issued and revoked only
//! through the policy store — a `grant` ref carried on the wire is a
//! lookup key, never a proof (GTC-002). Grants bind to a principal and
//! a project: copying or sharing a project file never moves a grant to
//! another principal (AHG-010).
//!
//! Scope forms (AHG-008):
//!
//! - [`ResourceScope::Exact`] — an explicit set of stable ids.
//! - [`ResourceScope::Snapshot`] — a fixed id set captured from an
//!   issuance-time snapshot (same membership semantics; the snapshot
//!   provenance is recorded).
//! - [`ResourceScope::Dynamic`] — constrained membership: owner, kind,
//!   time range, quantity, and whether objects created after issuance
//!   may join.
//!
//! Resource creation is a separate axis — [`CreateWithin`] names the
//! permitted parents, creatable kinds, and an optional placement bound.
//! Membership never flows down a parent/child relation implicitly, and
//! a shared source is evaluated as its own resource (AHG-008).

use std::collections::BTreeSet;

use serde::Serialize;
use synvelis_project::{ProjectId, ProjectRevision};

use crate::capability::{CapabilityRef, DataAccessSet, PermissionSet};
use crate::clock::PolicyInstant;
use crate::id::{AdapterId, GrantRef, TaskRef};
use crate::request::{Principal, ResourceKind, ResourceRef, TypedRange};

/// Policy epoch — the policy store's version counter.
///
/// A store bumps the epoch on an explicit policy-version update; a
/// grant may bound itself to a ceiling epoch, so a policy update
/// invalidates grants written against the older policy version.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct PolicyEpoch(pub u64);

/// Whether the grant applies to a task, a session, or a project
/// persistently (AHG-010 — task-scoped is the initial default).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "binding", rename_all = "kebab-case")]
pub enum GrantBinding {
    /// Bound to one authorized task — the initial default form. The
    /// request's task ref must equal the bound task.
    Task {
        /// The task the grant is bound to.
        task: TaskRef,
    },
    /// Bound to the caller's adapter-issued session identity.
    ///
    /// `AdapterId` *is* the session key at this layer — transports
    /// that multiplex several sessions over one endpoint must mint a
    /// distinct `AdapterId` per session (GW-08); two callers attested
    /// by the same adapter id are the same session and cannot be told
    /// apart here.
    Session {
        /// The adapter session identity the grant is bound to.
        adapter: AdapterId,
    },
    /// Persistent project-scoped — bound only to principal and project.
    /// Still never migrates to another principal on project copy/share.
    PersistentProject,
}

/// How a grant permits project mutations to be applied (AHG-009).
///
/// The mode is *derived from the grant's policy* — an agent cannot
/// escalate itself by declaring a mode on the wire.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "mode", rename_all = "kebab-case")]
pub enum ApprovalPolicy {
    /// The agent may prepare proposals only; apply requests are denied.
    /// The user may still apply a proposal through the manual UI as an
    /// independent operation.
    Suggest,
    /// Each apply requires a Synvelis-issued approval bound to the
    /// specific changeset/principal/project/revision (GW-05/06).
    ApproveToApply,
    /// Applies inside the grant's bounds require no per-change
    /// approval, subject to [`GrantLimits`]. This is **not**
    /// proactive-activation authority (AHG-007).
    DelegatedApply,
}

/// Quantitative bounds on delegated activity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GrantLimits {
    /// Maximum number of delegated apply authorizations the grant may
    /// issue. Consumed atomically at the Commit authorization (one
    /// unit per commit decision, inside GW-06's fence); `None` is
    /// unbounded within validity — never a substitute for expiry or
    /// revocation.
    pub apply_budget: Option<u64>,
}

/// Time and revocation state shared by grants, tasks, and permits.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Validity {
    /// Latest instant the record is live (`now <= expires_at`);
    /// `None` is unbounded in time — still bounded by revocation and
    /// the other dimensions.
    pub expires_at: Option<PolicyInstant>,
    /// Revocation has been confirmed. A revoked record stays recorded
    /// — revocation is a state, not a deletion.
    pub revoked: bool,
}

impl Validity {
    /// Whether the record is live at `now`.
    #[must_use]
    pub fn live(&self, now: PolicyInstant) -> bool {
        !self.revoked && self.expires_at.is_none_or(|at| now <= at)
    }
}

/// The owner dimension of a dynamic scope or a create constraint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "owners", rename_all = "kebab-case")]
pub enum OwnerConstraint {
    /// Membership is not constrained by owner.
    Any,
    /// The resource's direct owner must be one of these explicit refs.
    OneOf {
        /// Permitted owner refs (explicit stable ids only — no
        /// implicit parent/child inheritance, AHG-008).
        refs: BTreeSet<ResourceRef>,
    },
}

/// Args fields carrying a mutation's destination state (AHG-008).
///
/// A move/reparent changes the *after-state* attributes of the target;
/// the policy author names the args fields that carry the destination
/// parent and/or range, and the evaluator re-checks the after-state
/// against the same scope constraints — a move that escapes the scope
/// is denied even when the before-state matches (SEC-06).
///
/// This is opt-in per grant: with `move_args: None` no after-state
/// re-evaluation runs and the dynamic constraints apply as a
/// snapshot of current state. Policy authors constraining relocation
/// must declare the destination fields.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MoveArgs {
    /// Args field holding the destination parent's tagged id.
    pub parent_arg: Option<String>,
    /// Args field holding the destination `{domain,start,len}` range.
    pub range_arg: Option<String>,
}

/// Constrained dynamic membership (AHG-008).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicScope {
    /// Permitted owners.
    pub owners: OwnerConstraint,
    /// Permitted resource kinds.
    pub kinds: BTreeSet<ResourceKind>,
    /// Optional permitted range bound — a member's range must be fully
    /// contained, in the same time domain (no implicit conversion).
    pub range: Option<TypedRange>,
    /// Optional cap on dynamic-covered membership per request.
    pub max_members: Option<u32>,
    /// Whether resources created after the grant's issuance revision
    /// may join by satisfying the constraints. `false` restricts
    /// membership to resources proven to predate issuance — a newly
    /// added object is never silently covered.
    pub include_future: bool,
    /// Args fields carrying destination state for move/reparent
    /// re-evaluation.
    pub move_args: Option<MoveArgs>,
}

/// A constraint on the placement of created resources.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlacementConstraint {
    /// Args field holding the `{domain,start,len}` placement range.
    pub arg: String,
    /// The bound the placement range must be fully contained in.
    pub bound: TypedRange,
}

/// The create axis of a grant (AHG-008): creating resources is
/// evaluated separately from editing existing ones.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CreateWithin {
    /// Permitted parent refs for the created resource.
    pub parents: OwnerConstraint,
    /// Creatable resource kinds.
    pub kinds: BTreeSet<ResourceKind>,
    /// Optional placement bound on the created range.
    pub placement: Option<PlacementConstraint>,
}

/// The resource scope a grant covers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "scope", rename_all = "kebab-case")]
pub enum ResourceScope {
    /// An explicit set of stable ids.
    Exact {
        /// The covered resources.
        resources: BTreeSet<ResourceRef>,
    },
    /// A fixed id set captured from an issuance-time snapshot —
    /// membership is identical to `Exact`; the capture provenance is
    /// recorded for audit.
    Snapshot {
        /// The covered resources.
        resources: BTreeSet<ResourceRef>,
        /// The revision the set was captured at.
        captured_revision: ProjectRevision,
    },
    /// Constrained dynamic membership.
    Dynamic(DynamicScope),
}

/// The record a request's `grant` ref resolves to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    /// The grant's stable reference.
    pub id: GrantRef,
    /// Who issued the grant (Synvelis-side record).
    pub issued_by: Principal,
    /// The principal the grant is bound to — only this principal's
    /// requests may use it. Never migrates on project copy/share.
    pub principal: Principal,
    /// The project the grant is bound to.
    pub project: ProjectId,
    /// Task / session / persistent binding.
    pub binding: GrantBinding,
    /// The explicit capability set — exact `id@version` refs only. A
    /// capability or version outside this set is denied (SEC-13): new
    /// capabilities are never implicitly granted.
    pub capabilities: BTreeSet<CapabilityRef>,
    /// Permission kinds the grant covers (the descriptor's
    /// `required_permissions` must be a subset).
    pub permissions: PermissionSet,
    /// Data-access classes the grant covers (the descriptor's
    /// `data_access` must be a subset).
    pub data_access: DataAccessSet,
    /// The covered resource scope.
    pub scope: ResourceScope,
    /// The create axis, when creation is permitted.
    pub create_within: Option<CreateWithin>,
    /// Which apply mode the grant derives (AHG-009).
    pub approval_policy: ApprovalPolicy,
    /// Quantitative bounds.
    pub limits: GrantLimits,
    /// Expiry / revocation.
    pub validity: Validity,
    /// Optional ceiling on the store's policy epoch — a policy-version
    /// update past this epoch invalidates the grant.
    pub epoch_ceiling: Option<PolicyEpoch>,
    /// The project revision the grant was issued under — the baseline
    /// for [`DynamicScope::include_future`] checks. `None` means every
    /// dynamically-covered resource is treated as future.
    pub issued_at_revision: Option<ProjectRevision>,
}

/// Facts the resource oracle reports about one resource.
///
/// GW-02 does not own project state: scope constraints are evaluated
/// against these attributes, supplied by the host (a project snapshot
/// in GW-03/04/06 integration; a static oracle in tests).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceAttributes {
    /// The resource's kind.
    pub kind: ResourceKind,
    /// Its direct owner, when it has one.
    pub owner: Option<ResourceRef>,
    /// Its current placement range, when it occupies one.
    pub range: Option<TypedRange>,
    /// The revision the resource was created under — compared with the
    /// grant's issuance revision for [`DynamicScope::include_future`].
    pub created_revision: Option<ProjectRevision>,
    /// Resources this resource structurally depends on — e.g. a clip's
    /// linked/shared sources. Each is evaluated as its own resource:
    /// clip permission never implies source permission (SEC-05).
    pub related: Vec<ResourceRef>,
}

/// What the oracle can say about a resource.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResourceFacts {
    /// Attributes are known.
    Known(ResourceAttributes),
    /// The resource cannot be resolved — absent or not visible to
    /// policy evaluation. The two cases are deliberately one value:
    /// evaluation never leaks which one applied.
    Unresolved,
}

/// The policy's view of project resources.
///
/// Implementations answer with facts as of the evaluation's snapshot;
/// the evaluator treats [`ResourceFacts::Unresolved`] as
/// non-membership without distinguishing "does not exist" from
/// "exists but is not visible".
pub trait ResourceOracle: Send + Sync {
    /// Facts about `resource` in `project`.
    fn facts(&self, project: ProjectId, resource: &ResourceRef) -> ResourceFacts;
}

/// A created resource reported in an effect set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateEffect {
    /// The explicit parent the resource is created under.
    pub parent: ResourceRef,
    /// The kind being created.
    pub creates: ResourceKind,
    /// The created placement range, when the operation places one.
    pub range: Option<TypedRange>,
}

/// The Synvelis-derived effect view for effect-aware authorization
/// (GTC-003 `PreparedEffects`).
///
/// This is an **input view**: GW-04 derives the actual effects of a
/// prepared operation and supplies them here; the policy evaluator
/// checks every member against the grant's scope and data-access —
/// never an agent's self-declared effect list. Members are the
/// resources actually read, written, deleted, or the parents/kinds
/// actually created under.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScopedEffects {
    /// Resources structurally read.
    pub reads: Vec<ResourceRef>,
    /// Resources written.
    pub writes: Vec<ResourceRef>,
    /// Resources created.
    pub creates: Vec<CreateEffect>,
    /// Resources deleted.
    pub deletes: Vec<ResourceRef>,
    /// Data-access classes actually disclosed.
    pub disclosures: Vec<crate::capability::DataAccessClass>,
    /// External provider transfers performed (count).
    pub external_transfers: u32,
    /// Tags of effects that could not be classified or verified —
    /// opaque plugin state, unverifiable media side effects. Any
    /// non-empty value denies: unknown effects are never
    /// auto-permitted (SEC-14).
    pub unsupported: Vec<String>,
}
