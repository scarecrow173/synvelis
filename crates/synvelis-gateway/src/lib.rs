//! `synvelis-gateway`: the typed internal gateway contract (GW-01) and
//! the authorization/policy layer built on it (GW-02).
//!
//! GW-01 (Issue #53) implements the versioned [`CapabilityRegistry`],
//! typed invocation ([`Gateway::parse_wire`] → [`Gateway::bind`] →
//! [`Gateway::invoke`]), and the versioned [`GatewayOutcome`] /
//! [`ErrorEnvelope`] contract, per the #45 GTC-001/002/007/008 design.
//!
//! GW-02 (Issue #54) implements principal/task/grant authorization:
//! [`GrantPolicy`] evaluates each [`AuthorizationStage`] against the
//! [`PolicyStore`] authority — authorized tasks, execution grants with
//! explicit capability sets, fixed/dynamic [`ResourceScope`],
//! [`DataAccessSet`] classes, approval policies, validity, budget, and
//! the policy epoch — per #44 AHG-006..010 and #45 GTC-003.
//!
//! GW-03 (Issue #55) implements scoped context projection: the
//! `synvelis.context.query` capability served by
//! [`ContextProjectionEngine`], which projects a [`PinnedSnapshot`] of
//! the host's project state (never a second source of truth) through
//! the typed [`ContextQuery`] contract — snapshot-revision-fixed
//! pagination with expiring cursors, per-field-class disclosure
//! authorization, and honest `complete`/`unknown`/`unavailable`
//! completeness — per #45 GTC-004.
//!
//! GW-04 (Issue #56) implements semantic operation preparation: the
//! `synvelis.operation.prepare` capability served by
//! [`PrepareEngine`], which lowers a typed semantic operation to a
//! `synvelis-project` [`ProjectOp`], validates and applies it on a
//! scratch clone of a pinned base — never the live project — derives
//! the real structural [`DerivedEffect`]s from the before/after
//! attribute diff (linked MIDI sources named explicitly, independent
//! copies as `create` + reference update), and evaluates them through
//! the GW-02 authorizer ([`PrepareAuthorizer`]/[`PreparePolicy`])
//! before recording a [`PreparedRecord`] GW-06 can revalidate and
//! commit — per #45 GTC-005.
//!
//! GW-05 (Issue #57) freezes that product into the immutable
//! [`PreparedChangeSet`] — canonicalized and digested, bound to its
//! dependency identity — and layers the three independent lifecycle
//! machines on it: proposal (`Prepared`/`Invalidated`/`Expired`),
//! approval (`Requested` → `Granted`/`Declined`/`Expired`/`Revoked`/
//! `Invalidated`/`Consumed`), and apply (`AwaitingApproval` →
//! `CommitRequested` → `Committed`/`Rejected`/`Conflict`/
//! `Indeterminate`). [`ProposalEngine`] serves preview/approve/apply;
//! [`ProposalGate`] adds the receipt verdict to the apply stage; only
//! digest-bound trusted receipts unlock an `ApproveToApply` apply, and
//! consumption precedes any success claim — commit-result
//! reconciliation stays GW-06's (GT-007..010, SEC-09..12).
//!
//! # Ownership boundary (GTC-001)
//!
//! This crate owns the contract, its validation pipeline, and the
//! authorization decision. It does **not** own: the project aggregate
//! or its revision (GW-06 commits through `ProjectController`), the
//! project-state resource view ([`ResourceOracle`] is the host's
//! snapshot seam), MCP transport (GW-08), or UX (GW-09). It holds no
//! project handle — a bound request can never mutate project state
//! through this crate.
//!
//! # Trust boundary (GT-002)
//!
//! The wire type [`WireRequest`] has no caller/authority fields, and
//! [`CallerIdentity`] can only be built with an [`AdapterSeal`] minted
//! inside this crate — wire payloads and external crates cannot forge
//! a trusted caller. GW-01's [`fixture::FixtureAdapter`] is the only
//! seal issuer; real transports get their issuance seam in GW-08.

#![warn(missing_docs)]

pub mod authorization;
pub mod capability;
pub mod clock;
pub mod context;
pub mod context_engine;
pub mod fixture;
pub mod gateway;
pub mod grant;
pub mod id;
pub mod outcome;
pub mod policy;
pub mod prepare;
pub mod prepare_engine;
pub mod prepare_policy;
pub mod proposal;
pub mod proposal_engine;
pub mod proposal_policy;
pub mod registry;
pub mod request;
pub mod schema;
pub mod snapshot;
pub mod store;
pub mod wire;

pub use authorization::{
    AuthorizationDecision, AuthorizationStage, DenyAllPolicy, PolicyEvaluator,
};
pub use capability::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityRef, CapabilityVersion,
    DataAccessClass, DataAccessSet, EffectKind, EffectSet, ExecutionContract, OperationDomain,
    PermissionKind, PermissionSet, TargetContract,
};
pub use clock::{ManualClock, PolicyClock, PolicyInstant, SystemClock};
pub use context::{
    Completeness, ContextAuthorizer, ContextBudget, ContextProjection, ContextQuery, FieldGroup,
    FieldSelection, NoSelectionSource, PageInfo, PageRequest, ProjectedItem, ResourceSelector,
    ScopeAdmission, SelectionFacts, SelectionProjection, SelectionSource,
};
pub use context_engine::ContextProjectionEngine;
pub use gateway::Gateway;
pub use grant::{
    ApprovalPolicy, CreateEffect, CreateWithin, DynamicScope, Grant, GrantBinding, GrantLimits,
    MoveArgs, OwnerConstraint, PlacementConstraint, PolicyEpoch, ResourceAttributes, ResourceFacts,
    ResourceOracle, ResourceScope, ScopedEffects, Validity,
};
pub use id::{
    AdapterId, ApprovalRef, CursorRef, GrantRef, IdempotencyKey, OperationId, ProactivePermitRef,
    ProposalRef, RequestId, SelectionViewRef, SnapshotRef, TaskRef,
};
pub use outcome::{
    ErrorCategory, ErrorEnvelope, ExternalError, GatewayError, GatewayOutcome, RetryAdvice,
};
pub use policy::GrantPolicy;
pub use prepare::{
    DerivedEffect, EffectVisibility, NamedOperation, Precondition, PreparationBasis,
    PrepareAuthorizer, PreparedEffects, PreparedOperation, PreparedRecord, SemanticCommand,
};
pub use prepare_engine::{PrepareBudget, PrepareEngine};
pub use prepare_policy::PreparePolicy;
pub use proposal::{
    ApplyEvent, ApplyRecord, ApplyStatus, ApprovalEvent, ApprovalReceipt, ApprovalRecord,
    ApprovalStatus, CHANGESET_SCHEMA, ContentDigest, PreparedChangeSet, ProposalEvent,
    ProposalStatus, ProposalStore, ProposalView,
};
pub use proposal_engine::{APPLY_ID, APPROVE_ID, PREVIEW_ID, ProposalEngine};
pub use proposal_policy::ProposalGate;
pub use registry::{CapabilityOperation, CapabilityRegistry, Registration};
pub use request::{
    AdapterSeal, CallerIdentity, Consistency, ConsistencyKind, OperationRequest, OperationTarget,
    OriginKind, Principal, PrincipalKind, ResourceKind, ResourceRef, TimeDomain, TypedRange,
};
pub use schema::{FieldSchema, SchemaRef, SchemaRegistry, SchemaVersion, ValueSchema};
pub use snapshot::{MemorySnapshotSource, PinnedSnapshot, ProjectOracle, SnapshotSource};
pub use store::{
    Activation, AuthorizedTask, GrantRecord, InMemoryPolicyStore, PolicyStore, ProactivePermit,
};
pub use wire::{
    FORBIDDEN_AUTHORITY_FIELDS, FORBIDDEN_IMPLICIT_TARGET_FIELDS, WireConsistency, WireRequest,
    WireTarget,
};
