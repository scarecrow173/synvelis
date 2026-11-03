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
//! GW-06 (Issue #58) closes the commit boundary: [`CommitGate`] is the
//! only path from an accepted apply to the host's `ProjectController`.
//! Inside one [`CommitFence`] section it revalidates the receipt
//! binding, grant/task/epoch/scope/effects against the *current*
//! [`PolicyStore`], and the expected revision — then applies through
//! the single writer and fsyncs the durable [`JournalPayload`] record
//! before reporting success. Revocation enters the same fence, so
//! revoke/commit ordering is the journal order (REC-01/02); keyed
//! retries replay recorded outcomes and never re-apply (REC-04..06);
//! `Indeterminate` is a first-class verdict, never a silent success.
//! [`CommitGate::open`] replays the journal across restarts — the
//! ordered Recovery Journal is the adopted PoC-A durability approach;
//! [`FilePolicyStore`] + durable fence markers is the evaluated PoC-B
//! comparison (ACG-003/004, REC-07..10).
//!
//! GW-08 (Issue #59) exposes that internal boundary as an MCP server
//! adapter: [`McpHost`] is the transport-agnostic session core —
//! `synvelis.connect` binds a session through a Synvelis-issued
//! connection permit ([`mcp_permit`]), the semantic tool surface
//! (`mcp_tools`) translates calls into the same typed wire contract
//! every adapter uses, and the operation book + [`CommitPort`] report
//! settled verdicts through `operation.get` reconciliation instead of
//! a second job authority. The crate owns the protocol translation
//! and the trusted-identity binding; transports (the `mcp-poc` stdio
//! helper + authenticated loopback control channel) live outside it.
//!
//! # Ownership boundary (GTC-001)
//!
//! This crate owns the contract, its validation pipeline, and the
//! authorization decision. It does **not** own: the project aggregate
//! or its revision (GW-06 commits through `ProjectController`), the
//! project-state resource view ([`ResourceOracle`] is the host's
//! snapshot seam), MCP transport processes (the `PoC` binary owns
//! framing), or UX (GW-09). It holds no project handle — a bound
//! request can never mutate project state through this crate.
//!
//! GW-10 (Issue #62) adds the verification harness — the
//! [`evidence`] record schema + JSONL sink feeding G0..G6 gate
//! verdicts, the [`oracle`] expectation model independent of the
//! production evaluator, the [`schedule`] deterministic fence +
//! fault hooks, and the [`trace`] causal event log. These modules
//! are test/PoC seams only: they never participate in production
//! authorization, and the oracle re-expresses contract rules rather
//! than reusing [`GrantPolicy`].
//!
//! # Trust boundary (GT-002)
//!
//! The wire type [`WireRequest`] has no caller/authority fields, and
//! [`CallerIdentity`] can only be built with an [`AdapterSeal`] minted
//! inside this crate — wire payloads and external crates cannot forge
//! a trusted caller. Seal issuance is crate-internal: GW-01's
//! [`fixture::FixtureAdapter`] (test origin) and GW-08's
//! [`McpHost`] (external-adapter origin, gated on a verified
//! connection permit) are the issuers.

#![warn(missing_docs)]

pub mod authorization;
pub mod capability;
pub mod clock;
pub mod commit;
pub mod context;
pub mod context_engine;
pub mod durable_policy;
pub mod evidence;
pub mod fixture;
pub mod gateway;
pub mod grant;
pub mod id;
pub mod job;
pub mod job_engine;
pub mod journal;
pub mod mcp;
pub mod mcp_permit;
pub mod mcp_server;
mod mcp_tools;
pub mod media;
pub mod oracle;
pub mod outcome;
pub mod policy;
pub mod prepare;
pub mod prepare_engine;
pub mod prepare_policy;
pub mod proposal;
pub mod proposal_engine;
pub mod proposal_policy;
pub mod provenance;
pub mod registry;
pub mod request;
pub mod schedule;
pub mod schema;
pub mod snapshot;
pub mod store;
pub mod stub;
pub mod trace;
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
pub use commit::{
    CommitFence, CommitGate, CommitHooks, CommitOutcome, CommitReceipt, CommitRecord, FenceCtx,
    GateCrash, GateDeps, JournalIdem, JournalPayload, MutexFence, NoHooks, OutcomeRecord,
    PolicyAdmin, PolicyEvent, RecoveryReport, RetryOutcome, SaveRecord,
};
pub use context::{
    Completeness, ContextAuthorizer, ContextBudget, ContextProjection, ContextQuery, FieldGroup,
    FieldSelection, NoSelectionSource, PageInfo, PageRequest, ProjectedItem, ResourceSelector,
    ScopeAdmission, SelectionFacts, SelectionProjection, SelectionSource,
};
pub use context_engine::ContextProjectionEngine;
pub use durable_policy::{FilePolicyStore, PolicyLogEvent, PolicyOpenReport};
pub use evidence::{
    Evidence, EvidenceCase, EvidenceRecord, Gate, GateReport, Layer, MatrixRow, RunMode, Verdict,
    fault,
};
pub use gateway::Gateway;
pub use grant::{
    ApprovalPolicy, CreateEffect, CreateWithin, DynamicScope, Grant, GrantBinding, GrantLimits,
    MoveArgs, OwnerConstraint, PlacementConstraint, PolicyEpoch, ResourceAttributes, ResourceFacts,
    ResourceOracle, ResourceScope, ScopedEffects, Validity,
};
pub use id::{
    AdapterId, ApprovalRef, CommitId, CursorRef, GrantRef, IdempotencyKey, JobRef, MediaRef,
    OperationId, ProactivePermitRef, ProposalRef, RequestId, SelectionViewRef, SnapshotRef,
    TaskRef,
};
pub use job::{
    CancelLink, CancelResponse, DeclaredOutput, InputPin, InputSnapshot, JobBook, JobKind,
    JobPhase, JobPort, JobRecord, JobResult, JobSpec, JobStatus, OutputKind, OutputReport,
    SharedJobBook,
};
pub use job_engine::{JOB_CANCEL_ID, JOB_GET_ID, JOB_SUBMIT_ID, JobEngine, MEDIA_REGISTER_ID};
pub use journal::{FileJournal, OpenReport, ReplayOutcome, StopReason};
pub use mcp::{AdapterFault, CallToolResult, McpBounds, McpChannel};
pub use mcp_permit::{
    FilePermitStore, MemoryPermitStore, PermitIssue, PermitRecord, PermitSpec, PermitStore,
};
pub use mcp_server::{
    AdminPort, BoundIdentity, CommitPort, McpConfig, McpHost, McpSession, NoAdminPort,
    NoCommitPort, OpStatus,
};
pub use media::{
    MediaBook, MediaGate, MediaLifecycle, MediaPort, MediaRecord, MediaVerdict, VerifiedMedia,
};
pub use oracle::{
    Expected, FirstConfirmed, OracleApproval, OracleGrant, OracleStage, OracleTask, OrderedVerdict,
    ReqView, SpecOracle,
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
pub use provenance::{EvidenceLevel, GenerationProvenance, ProvenanceField, ProviderClaim};
pub use registry::{CapabilityOperation, CapabilityRegistry, Registration};
pub use request::{
    AdapterSeal, CallerIdentity, Consistency, ConsistencyKind, OperationRequest, OperationTarget,
    OriginKind, Principal, PrincipalKind, ResourceKind, ResourceRef, TimeDomain, TypedRange,
};
pub use schedule::{CrashHook, ScheduledFence};
pub use schema::{FieldSchema, SchemaRef, SchemaRegistry, SchemaVersion, ValueSchema};
pub use snapshot::{MemorySnapshotSource, PinnedSnapshot, ProjectOracle, SnapshotSource};
pub use store::{
    Activation, AuthorizedTask, GrantRecord, InMemoryPolicyStore, PolicyStore, ProactivePermit,
};
pub use stub::{CancelMode, DriveMode, MediaStub, ProviderStub, StubContent, StubOutputSpec};
pub use trace::{CausalTrace, TraceEvent};
pub use wire::{
    FORBIDDEN_AUTHORITY_FIELDS, FORBIDDEN_IMPLICIT_TARGET_FIELDS, WireConsistency, WireRequest,
    WireTarget,
};
