//! `synvelis-gateway`: the typed internal gateway contract (GW-01).
//!
//! Implements Issue #53 — a versioned [`CapabilityRegistry`], typed
//! invocation ([`Gateway::parse_wire`] → [`Gateway::bind`] →
//! [`Gateway::invoke`]), and the versioned [`GatewayOutcome`] /
//! [`ErrorEnvelope`] contract, per the #45 GTC-001/002/007/008 design.
//!
//! # Ownership boundary (GTC-001)
//!
//! This crate owns the contract and its validation pipeline. It does
//! **not** own: the project aggregate or its revision (GW-06 commits
//! through `ProjectController`), grant/policy evaluation (GW-02
//! implements [`PolicyEvaluator`]), MCP transport (GW-08), or UX
//! (GW-09). It holds no project handle — a bound request can never
//! mutate project state through this crate.
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
pub mod fixture;
pub mod gateway;
pub mod id;
pub mod outcome;
pub mod registry;
pub mod request;
pub mod schema;
pub mod wire;

pub use authorization::{
    AuthorizationDecision, AuthorizationStage, DenyAllPolicy, PolicyEvaluator,
};
pub use capability::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityRef, CapabilityVersion, EffectKind,
    EffectSet, ExecutionContract, OperationDomain, PermissionKind, PermissionSet, TargetContract,
};
pub use gateway::Gateway;
pub use id::{
    AdapterId, ApprovalRef, GrantRef, IdempotencyKey, OperationId, ProposalRef, RequestId,
    SnapshotRef, TaskRef,
};
pub use outcome::{
    ErrorCategory, ErrorEnvelope, ExternalError, GatewayError, GatewayOutcome, RetryAdvice,
};
pub use registry::{CapabilityOperation, CapabilityRegistry, Registration};
pub use request::{
    AdapterSeal, CallerIdentity, Consistency, ConsistencyKind, OperationRequest, OperationTarget,
    OriginKind, Principal, PrincipalKind, ResourceKind, ResourceRef, TimeDomain, TypedRange,
};
pub use schema::{FieldSchema, SchemaRef, SchemaRegistry, SchemaVersion, ValueSchema};
pub use wire::{
    FORBIDDEN_AUTHORITY_FIELDS, FORBIDDEN_IMPLICIT_TARGET_FIELDS, WireConsistency, WireRequest,
    WireTarget,
};
