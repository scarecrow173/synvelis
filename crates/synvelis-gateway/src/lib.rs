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
pub mod fixture;
pub mod gateway;
pub mod grant;
pub mod id;
pub mod outcome;
pub mod policy;
pub mod registry;
pub mod request;
pub mod schema;
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
pub use gateway::Gateway;
pub use grant::{
    ApprovalPolicy, CreateEffect, CreateWithin, DynamicScope, Grant, GrantBinding, GrantLimits,
    MoveArgs, OwnerConstraint, PlacementConstraint, PolicyEpoch, ResourceAttributes, ResourceFacts,
    ResourceOracle, ResourceScope, ScopedEffects, Validity,
};
pub use id::{
    AdapterId, ApprovalRef, GrantRef, IdempotencyKey, OperationId, ProactivePermitRef, ProposalRef,
    RequestId, SnapshotRef, TaskRef,
};
pub use outcome::{
    ErrorCategory, ErrorEnvelope, ExternalError, GatewayError, GatewayOutcome, RetryAdvice,
};
pub use policy::GrantPolicy;
pub use registry::{CapabilityOperation, CapabilityRegistry, Registration};
pub use request::{
    AdapterSeal, CallerIdentity, Consistency, ConsistencyKind, OperationRequest, OperationTarget,
    OriginKind, Principal, PrincipalKind, ResourceKind, ResourceRef, TimeDomain, TypedRange,
};
pub use schema::{FieldSchema, SchemaRef, SchemaRegistry, SchemaVersion, ValueSchema};
pub use store::{
    Activation, AuthorizedTask, GrantRecord, InMemoryPolicyStore, PolicyStore, ProactivePermit,
};
pub use wire::{
    FORBIDDEN_AUTHORITY_FIELDS, FORBIDDEN_IMPLICIT_TARGET_FIELDS, WireConsistency, WireRequest,
    WireTarget,
};
