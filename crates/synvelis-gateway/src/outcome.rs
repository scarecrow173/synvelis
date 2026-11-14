//! Versioned result / error contract (GTC-007/008).
//!
//! [`GatewayOutcome`] is the typed classification of an invocation's
//! disposition; [`ErrorEnvelope`] is the common failure form. Every
//! failure carries a typed [`GatewayError`] whose [`ErrorCategory`] and
//! [`RetryAdvice`] are derived — not hand-picked — and a public message
//! split from an optional internal diagnostic that
//! [`ErrorEnvelope::to_external`] strips before disclosure.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::authorization::AuthorizationStage;
use crate::capability::{Availability, CapabilityVersion, OperationDomain};
use crate::id::{ApprovalRef, OperationId, ProposalRef};
use crate::request::{ConsistencyKind, ResourceKind, TimeDomain};
use crate::schema::SchemaVersion;

/// The typed disposition of an accepted invocation (GTC-008).
///
/// `Completed` means *the capability finished its work* — it does not,
/// in general, mean the project was committed; `ProjectMutation`
/// completion semantics belong to the GW-06 commit boundary.
/// Variants carry named fields rather than newtype payloads — an
/// internally-tagged enum cannot serialize scalar content (string
/// refs, `OperationId`, non-map results) into its tag envelope.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum GatewayOutcome<T> {
    /// The capability finished and returned a typed result.
    Completed {
        /// The operation's result payload (validated against the
        /// declared output schema).
        result: T,
    },
    /// A proposal was prepared (GW-04/05 domain) — previewable, not
    /// committed, and not a promise of applicability.
    ProposalReady {
        /// Reference to the prepared proposal.
        proposal: ProposalRef,
    },
    /// The operation needs an approval before it may apply (GW-05/06).
    ApprovalRequired {
        /// Reference to the approval request.
        approval: ApprovalRef,
    },
    /// The operation was accepted for later completion (async/job).
    Accepted {
        /// The accepted operation instance.
        operation: OperationId,
    },
}

/// Typed gateway error classification.
///
/// Every variant is produced at a specific contract boundary; the code
/// never guesses or coerces invalid input into a nearby valid form.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "kebab-case")]
pub enum GatewayError {
    /// The wire named a capability id that is not registered.
    UnknownCapability {
        /// The unrecognized capability id.
        id: String,
    },
    /// The capability id is known but the requested version is not
    /// registered.
    UnsupportedCapabilityVersion {
        /// The capability id.
        id: String,
        /// The requested version.
        requested: CapabilityVersion,
        /// Versions actually registered.
        supported: Vec<CapabilityVersion>,
    },
    /// The wire named a schema name the registry does not know.
    UnknownSchema {
        /// The unrecognized schema name.
        name: String,
    },
    /// The schema name is known but the requested version is not
    /// registered.
    UnsupportedSchemaVersion {
        /// The schema name.
        name: String,
        /// The requested version.
        requested: SchemaVersion,
        /// Versions actually registered.
        supported: Vec<SchemaVersion>,
    },
    /// The requested schema version is registered but incompatible with
    /// the capability's declared input schema (different name or major,
    /// or a newer minor than declared).
    IncompatibleSchema {
        /// The schema name.
        name: String,
        /// The requested version.
        requested: SchemaVersion,
        /// The capability-declared version.
        declared: SchemaVersion,
    },
    /// The wire payload is not a well-formed request.
    MalformedRequest {
        /// What was malformed (safe to disclose).
        reason: String,
    },
    /// The wire carried a field that asserts caller identity or granted
    /// authority — self-declared trust is always rejected (GT-002).
    SelfDeclaredAuthority {
        /// The offending field name.
        field: String,
    },
    /// Caller authentication failed — a presented credential or
    /// session did not verify (AUTX-02). Deliberately fieldless: every
    /// verification failure shares this one blind verdict so a probe
    /// learns nothing about which sessions or credentials exist.
    Authentication,
    /// The wire carried an implicit-targeting field (UI selection /
    /// focus / active window) — targets must be explicit (GT-001).
    ImplicitTargetRejected {
        /// The offending field name.
        field: String,
    },
    /// A tagged stable id string is malformed.
    InvalidId {
        /// The offending value.
        text: String,
    },
    /// A wire-declared resource kind name is not a known kind.
    UnknownResourceKind {
        /// The unrecognized kind name.
        name: String,
    },
    /// The target's kind disagrees with the declared or contract kind.
    WrongResourceKind {
        /// The required kind.
        expected: ResourceKind,
        /// The kind that actually arrived.
        actual: ResourceKind,
    },
    /// A time range's declared domain disagrees with the expected one.
    WrongTimeDomain {
        /// The required domain.
        expected: TimeDomain,
        /// The domain that actually arrived.
        actual: TimeDomain,
    },
    /// The capability requires a target and the wire carried none.
    TargetRequired,
    /// The capability declares no target but the wire carried one.
    UnexpectedTarget,
    /// The wire target's shape disagrees with the contract's shape —
    /// e.g. a `create` target where a `resource` was required.
    TargetShapeMismatch {
        /// The required target shape (`resource` / `resource-set` /
        /// `create`).
        expected: &'static str,
        /// The wire target shape that arrived.
        actual: &'static str,
    },
    /// An argument failed schema validation.
    SchemaViolation {
        /// JSON path to the failing value.
        path: String,
        /// What violated the schema (safe to disclose).
        reason: String,
    },
    /// The consistency variant is not legal for the capability's
    /// operation domain — e.g. a mutation without `ExpectedRevision`,
    /// or a query demanding mutation semantics.
    ConsistencyNotAllowed {
        /// The capability's domain.
        domain: OperationDomain,
        /// The consistency variant that arrived.
        consistency: ConsistencyKind,
    },
    /// The capability is not invocable in its current availability.
    Unavailable {
        /// The capability id.
        id: String,
        /// The registered availability.
        availability: Availability,
    },
    /// A capability id+version was registered twice.
    CapabilityConflict {
        /// The capability id.
        id: String,
        /// The conflicting version.
        version: CapabilityVersion,
    },
    /// A descriptor whose fields contradict each other was rejected at
    /// registration — e.g. a `Query` domain declaring mutating effects
    /// would misroute the authorization stage.
    IncoherentDescriptor {
        /// The capability id.
        id: String,
        /// What was incoherent (safe to disclose).
        reason: String,
    },
    /// A schema name+version was registered twice.
    SchemaConflict {
        /// The schema name.
        name: String,
        /// The conflicting version.
        version: SchemaVersion,
    },
    /// The policy evaluator denied the request at the given stage.
    Denied {
        /// The stage at which denial occurred.
        stage: AuthorizationStage,
        /// The evaluator's safe-to-disclose reason, when supplied.
        reason: Option<String>,
    },
    /// The expected project revision does not match the live one.
    /// Produced by commit-boundary executors; surfaced here so the
    /// envelope taxonomy is stable for GW-06.
    RevisionConflict {
        /// The revision the request expected.
        expected: u64,
        /// The live revision.
        actual: u64,
    },
    /// The pinned snapshot is no longer available — evicted from the
    /// pinned store, or the requested reference/revision was never
    /// pinnable. The two cases share one verdict (existence-blind); the
    /// engine never silently re-bases onto `Latest` (GW-03, DATA-03).
    SnapshotExpired,
    /// The semantic operation was rejected by the project aggregate's
    /// own validator against the pinned base (GW-04, GTC-005). The
    /// request was well-formed — the base state does not satisfy the
    /// operation's preconditions (absent ids, illegal references,
    /// unrepresentable ranges).
    PreparationRejected {
        /// Safe-to-disclose reason.
        reason: String,
    },
    /// The asserted content digest does not match the proposal's
    /// stored changeset — the request names one proposal but asserts
    /// different semantic content (GW-05, SEC-09). Re-preparation is
    /// the correction path, not guessing a new digest.
    DigestMismatch {
        /// The proposal the mismatch was detected against.
        proposal: String,
    },
    /// The named proposal cannot be previewed, approved, or applied —
    /// it is unknown to this caller, expired, or invalidated. Unknown
    /// and inaccessible proposals share one verdict (existence
    /// blinding); a dead proposal names its terminal status.
    ProposalUnavailable {
        /// Safe-to-disclose reason.
        reason: String,
    },
    /// An approval receipt could not authorize this apply — expired,
    /// revoked, invalidated, declined, already consumed, or bound to
    /// a different principal/project/grant/task/digest/revision than
    /// the request carries (GW-05, SEC-10/11).
    ApprovalInvalid {
        /// Safe-to-disclose reason.
        reason: String,
    },
    /// The page cursor passed its expiry — the pagination chain is
    /// dead and a new query must be issued to start a fresh one
    /// (GW-03, expiring cursor).
    CursorExpired,
    /// The page cursor does not identify a live pagination chain for
    /// this request — unknown id, wrong caller/task/grant, mismatched
    /// snapshot reference, or query parameters carried alongside a
    /// continuation. Unknown and foreign cursors share one verdict —
    /// the error never confirms whether the token exists (GW-03).
    CursorInvalid {
        /// What was invalid (safe to disclose).
        reason: String,
    },
    /// A bounded budget was exceeded — fetch amount, page count,
    /// selector size, or range window (GW-03, DATA-08).
    LimitExceeded {
        /// Which budget was exceeded (safe to disclose).
        limit: String,
        /// The configured bound.
        max: u64,
    },
    /// The idempotency key was already seen with an identical payload.
    DuplicateRequest {
        /// The operation the first identical request produced.
        original: OperationId,
    },
    /// The idempotency key was already seen with a *different* payload.
    IdempotencyConflict,
    /// The registered capability has no executor installed.
    NoExecutor {
        /// The capability id.
        id: String,
    },
    /// The outcome cannot be determined (persistence boundary crossed,
    /// crash, …) — reconcile state before retrying.
    Indeterminate {
        /// Safe-to-disclose reason.
        reason: String,
    },
    /// An internal invariant failed.
    Internal {
        /// Safe-to-disclose reason.
        reason: String,
    },
}

/// Coarse failure category for routing and metrics (GTC-008).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorCategory {
    /// Caller authentication / trust attestation failures.
    Authentication,
    /// Permission evaluation failures.
    Authorization,
    /// Approval lifecycle failures.
    Approval,
    /// Consistency requirement failures.
    Consistency,
    /// Shape/argument validation failures.
    Validation,
    /// Capability/version/schema resolution failures.
    Capability,
    /// Effect bound or effect validation failures.
    Effects,
    /// Idempotency / duplicate submission failures.
    DuplicateRequest,
    /// Recovery / indeterminate outcome.
    Recovery,
    /// Internal invariant failures.
    Internal,
}

/// Whether and how a failed request may be retried.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RetryAdvice {
    /// Retrying the identical request will never succeed.
    Never,
    /// Retry only after correcting the request or the caller's grants.
    AfterCorrection,
    /// Transient condition — retry may succeed unchanged.
    Transient,
    /// The outcome is unknown — reconcile state before any retry.
    ReconcileState,
}

impl GatewayError {
    /// The category this error belongs to (derived, not declared).
    #[must_use]
    pub const fn category(&self) -> ErrorCategory {
        match self {
            Self::SelfDeclaredAuthority { .. } | Self::Authentication => {
                ErrorCategory::Authentication
            }
            Self::Denied { .. } => ErrorCategory::Authorization,
            Self::ApprovalInvalid { .. } => ErrorCategory::Approval,
            Self::ConsistencyNotAllowed { .. }
            | Self::RevisionConflict { .. }
            | Self::SnapshotExpired
            | Self::CursorExpired
            | Self::DigestMismatch { .. }
            | Self::ProposalUnavailable { .. } => ErrorCategory::Consistency,
            Self::MalformedRequest { .. }
            | Self::ImplicitTargetRejected { .. }
            | Self::InvalidId { .. }
            | Self::UnknownResourceKind { .. }
            | Self::WrongResourceKind { .. }
            | Self::WrongTimeDomain { .. }
            | Self::TargetRequired
            | Self::UnexpectedTarget
            | Self::TargetShapeMismatch { .. }
            | Self::SchemaViolation { .. }
            | Self::CursorInvalid { .. }
            | Self::PreparationRejected { .. }
            | Self::LimitExceeded { .. } => ErrorCategory::Validation,
            Self::UnknownCapability { .. }
            | Self::UnsupportedCapabilityVersion { .. }
            | Self::UnknownSchema { .. }
            | Self::UnsupportedSchemaVersion { .. }
            | Self::IncompatibleSchema { .. }
            | Self::Unavailable { .. }
            | Self::CapabilityConflict { .. }
            | Self::SchemaConflict { .. }
            | Self::IncoherentDescriptor { .. }
            | Self::NoExecutor { .. } => ErrorCategory::Capability,
            Self::DuplicateRequest { .. } | Self::IdempotencyConflict => {
                ErrorCategory::DuplicateRequest
            }
            Self::Indeterminate { .. } => ErrorCategory::Recovery,
            Self::Internal { .. } => ErrorCategory::Internal,
        }
    }

    /// The retry advice for this error (derived, not declared).
    #[must_use]
    pub const fn retry(&self) -> RetryAdvice {
        match self {
            Self::UnknownCapability { .. }
            | Self::UnsupportedCapabilityVersion { .. }
            | Self::UnknownSchema { .. }
            | Self::UnsupportedSchemaVersion { .. }
            | Self::SelfDeclaredAuthority { .. }
            | Self::ImplicitTargetRejected { .. }
            | Self::DuplicateRequest { .. }
            | Self::CursorInvalid { .. }
            | Self::NoExecutor { .. }
            | Self::Authentication => RetryAdvice::Never,
            Self::IncompatibleSchema { .. }
            | Self::MalformedRequest { .. }
            | Self::InvalidId { .. }
            | Self::UnknownResourceKind { .. }
            | Self::WrongResourceKind { .. }
            | Self::WrongTimeDomain { .. }
            | Self::TargetRequired
            | Self::UnexpectedTarget
            | Self::TargetShapeMismatch { .. }
            | Self::SchemaViolation { .. }
            | Self::PreparationRejected { .. }
            | Self::ConsistencyNotAllowed { .. }
            | Self::CapabilityConflict { .. }
            | Self::SchemaConflict { .. }
            | Self::IncoherentDescriptor { .. }
            | Self::Denied { .. }
            | Self::DigestMismatch { .. }
            | Self::ApprovalInvalid { .. }
            | Self::LimitExceeded { .. }
            | Self::IdempotencyConflict => RetryAdvice::AfterCorrection,
            Self::Unavailable { availability, .. } => match availability {
                Availability::Disabled { .. } => RetryAdvice::AfterCorrection,
                Availability::Available | Availability::Planned | Availability::Retired => {
                    RetryAdvice::Never
                }
            },
            Self::RevisionConflict { .. }
            | Self::SnapshotExpired
            | Self::CursorExpired
            | Self::ProposalUnavailable { .. }
            | Self::Indeterminate { .. } => RetryAdvice::ReconcileState,
            Self::Internal { .. } => RetryAdvice::Transient,
        }
    }

    /// A safe, external-facing message built only from discloseable
    /// fields — never containing internals like serde paths.
    #[allow(clippy::too_many_lines)] // one message arm per variant
    #[must_use]
    pub fn safe_message(&self) -> String {
        match self {
            Self::UnknownCapability { id } => format!("unknown capability: {id}"),
            Self::UnsupportedCapabilityVersion {
                id,
                requested,
                supported,
            } => format!(
                "unsupported capability version {requested} for {id}; supported: {}",
                supported
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::UnknownSchema { name } => format!("unknown schema: {name}"),
            Self::UnsupportedSchemaVersion {
                name, requested, ..
            } => format!("unsupported schema version {requested} for {name}"),
            Self::IncompatibleSchema {
                name,
                requested,
                declared,
            } => {
                format!("schema {name}@{requested} is not compatible with declared {declared}")
            }
            Self::MalformedRequest { reason } => format!("malformed request: {reason}"),
            Self::SelfDeclaredAuthority { field } => {
                format!("self-declared authority is not accepted: {field}")
            }
            Self::Authentication => "caller authentication failed".to_owned(),
            Self::ImplicitTargetRejected { field } => {
                format!("implicit targeting is not accepted: {field}")
            }
            Self::InvalidId { .. } => "malformed stable id".to_owned(),
            Self::UnknownResourceKind { name } => format!("unknown resource kind: {name}"),
            Self::WrongResourceKind { expected, actual } => {
                format!("wrong resource kind: expected {expected:?}, got {actual:?}")
            }
            Self::WrongTimeDomain { expected, actual } => {
                format!("wrong time domain: expected {expected:?}, got {actual:?}")
            }
            Self::TargetRequired => "operation target is required".to_owned(),
            Self::UnexpectedTarget => "operation takes no target".to_owned(),
            Self::TargetShapeMismatch { expected, actual } => {
                format!("target shape mismatch: expected {expected}, got {actual}")
            }
            Self::SchemaViolation { path, reason } => {
                format!("schema violation at {path}: {reason}")
            }
            Self::ConsistencyNotAllowed {
                domain,
                consistency,
            } => {
                format!(
                    "consistency {} is not allowed for domain {}",
                    consistency.name(),
                    domain.name()
                )
            }
            Self::Unavailable { id, availability } => {
                format!("capability {id} is unavailable ({})", availability.name())
            }
            Self::CapabilityConflict { id, version } => {
                format!("capability {id}@{version} is already registered")
            }
            Self::SchemaConflict { name, version } => {
                format!("schema {name}@{version} is already registered")
            }
            Self::IncoherentDescriptor { id, reason } => {
                format!("incoherent capability descriptor {id}: {reason}")
            }
            Self::Denied { stage, reason } => match reason {
                Some(reason) => {
                    format!("operation denied at stage {}: {reason}", stage.name())
                }
                None => format!("operation denied at stage {}", stage.name()),
            },
            Self::RevisionConflict { expected, actual } => {
                format!("revision conflict: expected {expected}, actual {actual}")
            }
            Self::SnapshotExpired => {
                "pinned snapshot is no longer available; issue a new query to pin a fresh revision"
                    .to_owned()
            }
            Self::PreparationRejected { reason } => {
                format!("operation rejected at preparation: {reason}")
            }
            Self::DigestMismatch { proposal } => {
                format!("changeset digest mismatch on proposal {proposal}")
            }
            Self::ProposalUnavailable { reason } => {
                format!("proposal is not applicable: {reason}")
            }
            Self::ApprovalInvalid { reason } => {
                format!("approval cannot authorize this apply: {reason}")
            }
            Self::CursorExpired => {
                "page cursor expired; issue a new query to start a new pagination chain".to_owned()
            }
            Self::CursorInvalid { reason } => format!("page cursor is not valid: {reason}"),
            Self::LimitExceeded { limit, max } => {
                format!("request exceeds {limit} (max {max})")
            }
            Self::DuplicateRequest { .. } => "duplicate request".to_owned(),
            Self::IdempotencyConflict => {
                "idempotency key reused with a different payload".to_owned()
            }
            Self::NoExecutor { id } => format!("capability {id} has no executor"),
            Self::Indeterminate { reason } => format!("indeterminate outcome: {reason}"),
            Self::Internal { reason } => format!("internal error: {reason}"),
        }
    }
}

impl fmt::Display for GatewayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.safe_message())
    }
}

impl std::error::Error for GatewayError {}

/// The common failure envelope (GTC-008).
///
/// `message` is safe for external disclosure; `diagnostic` holds
/// internal detail (serde paths, store diagnostics) and is stripped by
/// [`to_external`][ErrorEnvelope::to_external].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ErrorEnvelope {
    /// The typed error — flattened so the wire sees a single level:
    /// `{"code":"…", …variant fields…, "category":…, "retry":…, "message":…}`.
    #[serde(flatten)]
    pub code: GatewayError,
    /// Derived category.
    pub category: ErrorCategory,
    /// Derived retry advice.
    pub retry: RetryAdvice,
    /// Safe-to-disclose message.
    pub message: String,
    /// Internal-only diagnostic detail — never serialized externally.
    #[serde(skip_serializing)]
    pub diagnostic: Option<String>,
}

/// The wire-safe projection of an [`ErrorEnvelope`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ExternalError {
    /// The typed error — flattened (see [`ErrorEnvelope`]).
    #[serde(flatten)]
    pub code: GatewayError,
    /// Derived category.
    pub category: ErrorCategory,
    /// Derived retry advice.
    pub retry: RetryAdvice,
    /// Safe-to-disclose message.
    pub message: String,
}

impl ErrorEnvelope {
    /// Builds an envelope, deriving category, retry advice, and the
    /// safe message from `code`.
    #[must_use]
    pub fn new(code: GatewayError) -> Self {
        Self {
            message: code.safe_message(),
            category: code.category(),
            retry: code.retry(),
            code,
            diagnostic: None,
        }
    }

    /// Attaches internal-only diagnostic detail.
    #[must_use]
    pub fn with_diagnostic(mut self, diagnostic: impl Into<String>) -> Self {
        self.diagnostic = Some(diagnostic.into());
        self
    }

    /// The wire-safe projection — drops `diagnostic`.
    #[must_use]
    pub fn to_external(&self) -> ExternalError {
        ExternalError {
            code: self.code.clone(),
            category: self.category,
            retry: self.retry,
            message: self.message.clone(),
        }
    }
}

impl fmt::Display for ErrorEnvelope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ErrorEnvelope {}

impl From<GatewayError> for ErrorEnvelope {
    fn from(code: GatewayError) -> Self {
        Self::new(code)
    }
}
