//! Runtime script-backend port — the consumer-side mirror of the
//! RF-SCRIPT-00 language-neutral contract (ruviform#558), scoped to
//! what synvelis#273 needs: a [`ScriptBackend`]/[`ScriptSession`] seam
//! that evaluates one `.luau` source once into a typed [`ScriptResult`]
//! whose artifact payload carries the `ScriptWorkflowSpec` record.
//!
//! Boundary rules inherited from the contract:
//!
//! - No vendor VM types (`mlua`, `rhai`, …) appear in any signature —
//!   backends adapt at their own edge.
//! - [`ScriptBackend::open_session`] takes a [`SessionConfig`]; a
//!   backend must not require `Send`/`Sync` unless its descriptor
//!   advertises it. Sessions are per-config objects; a backend must
//!   not keep VM state alive across `close`.
//! - Backend resolution is fail-closed: unknown language/backend,
//!   contract-version mismatch, missing capability, and ambiguity are
//!   all typed rejects — never a silent fallback to another backend.
//! - The port records *what the script declared* (step references,
//!   typed values). It never executes project/job side effects,
//!   authorizes calls, or mints grants.
//!
//! Intentionally out of scope for this slice (YAGNI — add when a
//! consumer needs them): consumer-injected `HostApiSurface`/
//! `HostBinding` handlers, `resume` checkpoints, session `inputs`,
//! placement hints, and `ScriptSource::schema_version` (the
//! workflow-builder surface has no versioned document format yet).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use crate::analyzer::{
    DiagnosticSeverity, ScriptBackendVersion, ScriptLanguageId, SourcePosition, SourceRevision,
};

/// Contract version this port mirrors (RF-SCRIPT-00 C1 `contract_version`).
pub const SCRIPT_CONTRACT_VERSION: u32 = 1;

/// `ScriptArtifact::schema_version` for artifacts this port produces.
pub const SCRIPT_ARTIFACT_SCHEMA_VERSION: u32 = 1;

/// Script backend identifier — opaque, backend-chosen (e.g. `luau-mlua`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScriptBackendId(pub String);

impl fmt::Display for ScriptBackendId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Source content digest — opaque algorithm-tagged string (e.g. `sha256:<hex>`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceDigest(pub String);

impl fmt::Display for SourceDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Artifact digest — opaque, backend-chosen canonical digest string.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DefinitionDigest(pub String);

impl fmt::Display for DefinitionDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A runtime script source handed to a backend session.
///
/// `digest`/`revision` are consumer-computed pins: the backend must
/// verify `source.text` against `digest` before evaluating and fail
/// with [`SessionError::SourceDigestMismatch`] on drift — it must not
/// trust caller claims it cannot check.
#[derive(Clone, Debug)]
pub struct ScriptSource {
    /// UTF-8 source text.
    pub text: String,
    /// Language id (`luau` for this slice).
    pub language: ScriptLanguageId,
    /// Optional backend-id hint; resolution stays fail-closed either way.
    pub backend_hint: Option<ScriptBackendId>,
    /// Consumer-computed digest of `text` (algorithm-tagged).
    pub digest: SourceDigest,
    /// Consumer-managed source revision.
    pub revision: SourceRevision,
}

/// Execution budget a session must honor. Every field is optional —
/// `None` means the consumer set no bound and the backend's own
/// descriptor ceilings still apply.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecutionBudget {
    /// Maximum VM instructions/ops before `BudgetExceeded`.
    pub max_instructions: Option<u64>,
    /// Maximum wall-clock time before `BudgetExceeded`/`Cancelled`.
    pub max_wall_time: Option<Duration>,
    /// Maximum VM memory footprint in bytes.
    pub max_memory_bytes: Option<u64>,
    /// Maximum serialized output/artifact size in bytes.
    pub max_output_bytes: Option<u64>,
    /// Maximum host-surface calls (builder invocations count).
    pub max_host_calls: Option<u64>,
}

impl ExecutionBudget {
    /// The budget kinds this request actually bounds.
    #[must_use]
    pub fn requested_kinds(&self) -> BTreeSet<BudgetKind> {
        let mut kinds = BTreeSet::new();
        if self.max_instructions.is_some() {
            kinds.insert(BudgetKind::Instruction);
        }
        if self.max_wall_time.is_some() {
            kinds.insert(BudgetKind::WallClock);
        }
        if self.max_memory_bytes.is_some() {
            kinds.insert(BudgetKind::Memory);
        }
        if self.max_output_bytes.is_some() {
            kinds.insert(BudgetKind::Output);
        }
        if self.max_host_calls.is_some() {
            kinds.insert(BudgetKind::HostCall);
        }
        kinds
    }
}

/// A boundable resource dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum BudgetKind {
    /// VM instruction/op count.
    Instruction,
    /// Wall-clock time.
    WallClock,
    /// VM memory.
    Memory,
    /// Serialized output size.
    Output,
    /// Host-surface call count.
    HostCall,
}

impl BudgetKind {
    /// Stable capability-id fragment (`budget:<kind>`).
    #[must_use]
    pub fn capability_id(self) -> String {
        let name = match self {
            Self::Instruction => "instruction",
            Self::WallClock => "wall-clock",
            Self::Memory => "memory",
            Self::Output => "output",
            Self::HostCall => "host-call",
        };
        format!("budget:{name}")
    }
}

/// Finest granularity at which a backend can observe cancellation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum CancelGranularity {
    /// Only between top-level evaluations.
    #[default]
    BetweenEvaluations,
    /// At instruction/host-call safepoints.
    Safepoint,
}

/// What a backend session can produce.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ArtifactKind {
    /// A typed action recipe document.
    ActionRecipe,
    /// A `ScriptWorkflowSpec` record (workflow-builder surface).
    WorkflowDefinition,
    /// A consumer-defined extension artifact.
    ConsumerExtension {
        /// Consumer-chosen domain tag.
        domain: String,
    },
}

impl ArtifactKind {
    /// Stable capability-id fragment (`artifact:<kind>`).
    #[must_use]
    pub fn capability_id(&self) -> String {
        match self {
            Self::ActionRecipe => "artifact:action-recipe".to_owned(),
            Self::WorkflowDefinition => "artifact:workflow-definition".to_owned(),
            Self::ConsumerExtension { domain } => format!("artifact:consumer:{domain}"),
        }
    }
}

/// Declared capabilities of a backend — the resolve-time contract a
/// [`ScriptRequirement`] is checked against.
#[derive(Clone, Debug, Default)]
// Independent contract flags — bundling them into enums would lie
// about orthogonality (send/sync/threading/integer are unrelated axes).
#[allow(clippy::struct_excessive_bools)]
pub struct BackendCapabilities {
    /// Budget kinds the backend enforces itself (not advisory).
    pub enforced_budgets: BTreeSet<BudgetKind>,
    /// Maximum accepted source size in bytes (`None` = backend default).
    pub max_source_bytes: Option<u64>,
    /// Artifact kinds the backend can produce.
    pub artifact_kinds: BTreeSet<ArtifactKind>,
    /// Whether sessions/values may cross threads (`Send`).
    pub send: bool,
    /// Whether `evaluate` tolerates concurrent callers (`Sync`).
    pub sync: bool,
    /// Whether the session is bound to its creating thread.
    pub owner_thread_only: bool,
    /// Finest observed cancel granularity.
    pub cancel_granularity: CancelGranularity,
    /// Whether the VM exposes a native integer type (affects i64 fidelity).
    pub native_integer: bool,
}

impl BackendCapabilities {
    /// Capability ids this backend advertises — enforced budgets,
    /// artifact kinds, threading/cancel/integer traits.
    #[must_use]
    pub fn capability_ids(&self) -> BTreeSet<String> {
        let mut ids: BTreeSet<String> = self
            .enforced_budgets
            .iter()
            .map(|k| k.capability_id())
            .collect();
        ids.extend(self.artifact_kinds.iter().map(ArtifactKind::capability_id));
        if self.send {
            ids.insert("send".to_owned());
        }
        if self.sync {
            ids.insert("sync".to_owned());
        }
        if self.owner_thread_only {
            ids.insert("owner-thread".to_owned());
        }
        ids.insert(
            match self.cancel_granularity {
                CancelGranularity::BetweenEvaluations => "cancel:between-evaluations",
                CancelGranularity::Safepoint => "cancel:safepoint",
            }
            .to_owned(),
        );
        if self.native_integer {
            ids.insert("native-integer".to_owned());
        }
        ids
    }
}

/// Static description of a script backend implementation.
#[derive(Clone, Debug)]
pub struct BackendDescriptor {
    /// Backend implementation id (unique within a registry).
    pub id: ScriptBackendId,
    /// Language this backend evaluates.
    pub language: ScriptLanguageId,
    /// Implementation + contract versions.
    pub version: ScriptBackendVersion,
    /// Declared capabilities.
    pub capabilities: BackendCapabilities,
}

/// A language-neutral typed value — the only value shape that may
/// cross the backend boundary. VM-internal values, pointers, and
/// authority objects are unrepresentable by construction.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ScriptValue {
    /// Property absent (distinct from explicit null).
    Absent,
    /// Explicit null.
    Null,
    /// Boolean.
    Bool(bool),
    /// Exact i64 integer.
    Integer(i64),
    /// f64 number (binary64 — Luau's only number type).
    Number(f64),
    /// UTF-8 string.
    String(String),
    /// Binary bytes.
    Bytes(Vec<u8>),
    /// Ordered array.
    Array(Vec<ScriptValue>),
    /// String-keyed record.
    Record(BTreeMap<String, ScriptValue>),
    /// Time-domain-tagged integer ticks.
    Time(i64),
    /// Opaque consumer resource reference (never authority).
    ResourceId(String),
}

impl ScriptValue {
    /// Lifts a `serde_json::Value` into the neutral model.
    ///
    /// JSON integers become [`ScriptValue::Integer`] (i64-exact);
    /// u64 above `i64::MAX` and non-finite floats are typed rejects —
    /// they have no honest neutral representation.
    ///
    /// # Errors
    /// [`ScriptErrorKind::ValueOutOfRange`] — integer/float outside the
    /// representable range; [`ScriptErrorKind::ContractViolation`] —
    /// unsupported JSON shape.
    pub fn from_json(value: &serde_json::Value) -> Result<Self, ScriptError> {
        use serde_json::Value;
        Ok(match value {
            Value::Null => Self::Null,
            Value::Bool(b) => Self::Bool(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Self::Integer(i)
                } else if n.as_u64().is_some() {
                    return Err(ScriptError::new(
                        ScriptErrorKind::ValueOutOfRange,
                        "u64 integer exceeds i64 range",
                    ));
                } else if let Some(f) = n.as_f64() {
                    if f.is_finite() {
                        Self::Number(f)
                    } else {
                        return Err(ScriptError::new(
                            ScriptErrorKind::ValueOutOfRange,
                            "non-finite JSON number",
                        ));
                    }
                } else {
                    return Err(ScriptError::new(
                        ScriptErrorKind::ContractViolation {
                            detail: "unrecognized JSON number".to_owned(),
                        },
                        "unrecognized JSON number",
                    ));
                }
            }
            Value::String(s) => Self::String(s.clone()),
            Value::Array(items) => Self::Array(
                items
                    .iter()
                    .map(Self::from_json)
                    .collect::<Result<_, _>>()?,
            ),
            Value::Object(map) => Self::Record(
                map.iter()
                    .map(|(k, v)| Ok((k.clone(), Self::from_json(v)?)))
                    .collect::<Result<_, ScriptError>>()?,
            ),
        })
    }

    /// Lowers the neutral model back to `serde_json::Value`.
    ///
    /// [`ScriptValue::Absent`] record entries are omitted (absent =
    /// property not present); a bare `Absent`/`Bytes`/`Time`/
    /// `ResourceId` has no JSON form and is a typed reject — callers
    /// that need those shapes must not route them through JSON.
    ///
    /// # Errors
    /// [`ScriptErrorKind::ContractViolation`] — value has no JSON form;
    /// [`ScriptErrorKind::ValueOutOfRange`] — non-finite float.
    pub fn to_json(&self) -> Result<serde_json::Value, ScriptError> {
        use serde_json::Value;
        Ok(match self {
            Self::Absent => {
                return Err(ScriptError::new(
                    ScriptErrorKind::ContractViolation {
                        detail: "absent value has no JSON form".to_owned(),
                    },
                    "absent value has no JSON form",
                ));
            }
            Self::Null => Value::Null,
            Self::Bool(b) => Value::Bool(*b),
            Self::Integer(i) => Value::from(*i),
            Self::Number(f) => {
                let f = *f;
                if !f.is_finite() {
                    return Err(ScriptError::new(
                        ScriptErrorKind::ValueOutOfRange,
                        "non-finite float cannot be JSON",
                    ));
                }
                serde_json::Number::from_f64(f)
                    .map(Value::Number)
                    .ok_or_else(|| {
                        ScriptError::new(
                            ScriptErrorKind::ValueOutOfRange,
                            "float cannot be represented as JSON number",
                        )
                    })?
            }
            Self::String(s) => Value::String(s.clone()),
            Self::Bytes(_) | Self::Time(_) | Self::ResourceId(_) => {
                return Err(ScriptError::new(
                    ScriptErrorKind::ContractViolation {
                        detail: "value kind has no JSON form".to_owned(),
                    },
                    "value kind has no JSON form",
                ));
            }
            Self::Array(items) => {
                Value::Array(items.iter().map(Self::to_json).collect::<Result<_, _>>()?)
            }
            Self::Record(map) => Value::Object(
                map.iter()
                    .filter(|(_, v)| !matches!(v, Self::Absent))
                    .map(|(k, v)| Ok((k.clone(), v.to_json()?)))
                    .collect::<Result<_, ScriptError>>()?,
            ),
        })
    }
}

/// Provenance of an artifact's source.
#[derive(Clone, Debug)]
pub struct ArtifactSource {
    /// Language evaluated.
    pub language: ScriptLanguageId,
    /// Backend that produced the artifact.
    pub backend_id: ScriptBackendId,
    /// Digest of the evaluated source.
    pub source_digest: SourceDigest,
    /// Source revision evaluated.
    pub source_revision: SourceRevision,
}

/// Which backend implementation produced an artifact.
#[derive(Clone, Debug)]
pub struct ProducedBy {
    /// Backend id.
    pub backend_id: ScriptBackendId,
    /// Backend `impl_version` string.
    pub impl_version: String,
}

/// A typed artifact a session produced — for this slice, the
/// `ScriptWorkflowSpec` record under [`ArtifactKind::WorkflowDefinition`].
#[derive(Clone, Debug)]
pub struct ScriptArtifact {
    /// Must equal [`SCRIPT_ARTIFACT_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Artifact kind.
    pub kind: ArtifactKind,
    /// Canonical digest of `payload`.
    pub digest: DefinitionDigest,
    /// Typed payload — the spec record in [`ScriptValue`] form.
    pub payload: ScriptValue,
    /// Source provenance.
    pub source: ArtifactSource,
    /// Producer identity.
    pub produced_by: ProducedBy,
}

/// A non-fatal diagnostic emitted during evaluation.
#[derive(Clone, Debug)]
pub struct ScriptDiagnostic {
    /// Severity.
    pub severity: DiagnosticSeverity,
    /// Stable diagnostic code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Optional source position.
    pub position: Option<SourcePosition>,
}

/// The result of one `evaluate` — at most one artifact plus the
/// script's own return value and diagnostics.
#[derive(Clone, Debug, Default)]
pub struct ScriptResult {
    /// Produced artifact, when the script declared one.
    pub artifact: Option<ScriptArtifact>,
    /// The chunk's return value, when present.
    pub return_value: Option<ScriptValue>,
    /// Diagnostics collected during evaluation.
    pub diagnostics: Vec<ScriptDiagnostic>,
}

/// Language-neutral script error — the only failure shape that may
/// cross the session boundary.
#[derive(Clone, Debug)]
pub struct ScriptError {
    /// Error kind.
    pub kind: ScriptErrorKind,
    /// Human-readable message.
    pub message: String,
    /// Optional source position.
    pub position: Option<SourcePosition>,
    /// Backend that raised it, when known.
    pub backend_id: Option<ScriptBackendId>,
    /// Session that raised it, when known.
    pub session_id: Option<String>,
}

impl ScriptError {
    /// A bare error with no position/backend/session context.
    #[must_use]
    pub fn new(kind: ScriptErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            position: None,
            backend_id: None,
            session_id: None,
        }
    }
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for ScriptError {}

/// Language-neutral error kind — mirrors the RF-SCRIPT-00 C3 set.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ScriptErrorKind {
    /// Source would not parse.
    Parse,
    /// Source parsed but failed compilation/analysis.
    Compile,
    /// Type error inside the script.
    Type,
    /// Uncaught runtime error.
    Runtime,
    /// A declared budget was exceeded.
    BudgetExceeded {
        /// Which budget.
        kind: BudgetKind,
    },
    /// Evaluation was cancelled.
    Cancelled,
    /// The script used a feature the backend does not support.
    Unsupported {
        /// What is unsupported.
        what: String,
    },
    /// A host-surface call failed.
    Host {
        /// Host-side detail.
        detail: String,
    },
    /// Backend-internal failure.
    Backend {
        /// Backend-side detail.
        detail: String,
    },
    /// Backend bug / invariant violation.
    Internal {
        /// Detail.
        detail: String,
    },
    /// A value cannot be represented in the neutral model.
    ValueOutOfRange,
    /// The script violated the artifact/port contract.
    ContractViolation {
        /// Detail.
        detail: String,
    },
}

/// Session-open failure — the backend refused the [`SessionConfig`]
/// before any evaluation ran.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SessionError {
    /// A requested budget kind is not enforced by this backend.
    UnsupportedBudget {
        /// The budget kind.
        kind: BudgetKind,
    },
    /// The requested artifact kinds cannot be produced.
    UnsupportedArtifact {
        /// The artifact kind.
        kind: ArtifactKind,
    },
    /// Source text does not match the declared digest.
    SourceDigestMismatch {
        /// Declared digest.
        expected: SourceDigest,
        /// Computed digest.
        actual: SourceDigest,
    },
    /// Source exceeds the backend's size ceiling.
    SourceTooLarge {
        /// Observed size.
        size: u64,
        /// Enforced limit.
        limit: u64,
    },
    /// Source language is not this backend's language.
    LanguageMismatch {
        /// Expected language.
        expected: ScriptLanguageId,
        /// Source language.
        actual: ScriptLanguageId,
    },
    /// The backend cannot honor the requested isolation/threading.
    IsolationViolation {
        /// Reason.
        reason: String,
    },
    /// A session/resource limit was hit at open time.
    ResourceExhausted {
        /// Which resource.
        resource: String,
    },
    /// Backend-internal open failure.
    Backend {
        /// Detail.
        reason: String,
    },
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SessionError {}

/// Everything a session needs: budget + source + wanted artifacts.
#[derive(Clone, Debug)]
pub struct SessionConfig {
    /// Execution budget (all fields optional).
    pub budget: ExecutionBudget,
    /// Source to evaluate.
    pub source: ScriptSource,
    /// Artifact kinds the consumer expects back.
    pub artifact_kinds: BTreeSet<ArtifactKind>,
}

/// A live evaluation session. `evaluate` runs the source once;
/// `cancel` is cooperative (observed at safepoints); `close` releases
/// VM state and is idempotent.
///
/// No `Send`/`Sync` bound is required — a backend that needs them
/// declares so in [`BackendCapabilities`].
pub trait ScriptSession {
    /// Session-unique id for diagnostics.
    fn session_id(&self) -> &str;
    /// The backend that owns this session.
    fn backend_id(&self) -> &ScriptBackendId;
    /// Evaluates the configured source once.
    ///
    /// # Errors
    /// [`ScriptError`] — any typed evaluation failure.
    fn evaluate(&mut self) -> Result<ScriptResult, ScriptError>;
    /// Requests cooperative cancellation (idempotent).
    fn cancel(&mut self);
    /// Releases VM state (idempotent; evaluate after close fails).
    fn close(&mut self);
}

/// A script backend — owns no session state between `open_session`
/// calls beyond immutable descriptor/configuration.
pub trait ScriptBackend: fmt::Debug {
    /// Static descriptor.
    fn descriptor(&self) -> &BackendDescriptor;
    /// Opens a session for `config`.
    ///
    /// # Errors
    /// [`SessionError`] — unsupported budget/artifact, digest or
    /// language mismatch, size limit, isolation conflict.
    fn open_session(&self, config: &SessionConfig) -> Result<Box<dyn ScriptSession>, SessionError>;
}

/// What a consumer asks the registry to resolve.
#[derive(Clone, Debug, Default)]
pub struct ScriptRequirement {
    /// Required language (`None` = any).
    pub language: Option<ScriptLanguageId>,
    /// Required backend id (`None` = any backend for the language).
    pub backend: Option<ScriptBackendId>,
    /// Capability ids the backend must advertise (see
    /// [`BackendCapabilities::capability_ids`]).
    pub required_capabilities: BTreeSet<String>,
    /// Minimum acceptable `contract_version`.
    pub min_contract_version: Option<u32>,
    /// Artifact kinds the consumer needs (checked as
    /// `artifact:<kind>` capabilities).
    pub artifact_kinds: BTreeSet<ArtifactKind>,
}

/// Why registry resolution failed.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ResolveErrorReason {
    /// The requested language has no registered backend.
    UnknownLanguage,
    /// The requested backend id is not registered.
    UnknownBackend,
    /// Candidates exist but none meet the contract-version floor.
    UnsupportedContractVersion,
    /// Candidates exist but lack required capabilities/artifacts.
    MissingCapability,
    /// Multiple backends satisfy the requirement.
    Ambiguous,
    /// No backend matched at all.
    NoCandidates,
}

/// Typed resolution failure.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolveError {
    /// Failure reason.
    pub reason: ResolveErrorReason,
    /// Language requested, when any.
    pub language: Option<ScriptLanguageId>,
    /// Backend requested, when any.
    pub backend: Option<ScriptBackendId>,
    /// Capability ids that were missing (for `MissingCapability`).
    pub missing: Vec<String>,
}

/// Registry mutation failure.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum RegistryError {
    /// Two backends share one `ScriptBackendId`.
    DuplicateBackend {
        /// The duplicated id.
        backend: ScriptBackendId,
    },
    /// A backend descriptor is malformed (e.g. empty id).
    InvalidBackend {
        /// Reason.
        reason: String,
    },
}

/// A process-local registry of script backends. Resolution is
/// fail-closed: every non-unique or under-specified outcome is a
/// typed [`ResolveError`], never a guess.
#[derive(Default)]
pub struct ScriptBackendRegistry {
    backends: Vec<Arc<dyn ScriptBackend>>,
}

impl fmt::Debug for ScriptBackendRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScriptBackendRegistry")
            .field(
                "backends",
                &self
                    .backends
                    .iter()
                    .map(|b| b.descriptor().id.clone())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl ScriptBackendRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `backend`.
    ///
    /// # Errors
    /// [`RegistryError::DuplicateBackend`] — id already registered;
    /// [`RegistryError::InvalidBackend`] — empty backend id.
    pub fn register(&mut self, backend: Arc<dyn ScriptBackend>) -> Result<(), RegistryError> {
        let id = &backend.descriptor().id;
        if id.0.is_empty() {
            return Err(RegistryError::InvalidBackend {
                reason: "backend id must not be empty".to_owned(),
            });
        }
        if self.backends.iter().any(|b| &b.descriptor().id == id) {
            return Err(RegistryError::DuplicateBackend {
                backend: id.clone(),
            });
        }
        self.backends.push(backend);
        Ok(())
    }

    /// Registered backend count (introspection/tests).
    #[must_use]
    pub fn len(&self) -> usize {
        self.backends.len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }

    /// All registered descriptors, in registration order.
    #[must_use]
    pub fn descriptors(&self) -> Vec<&BackendDescriptor> {
        self.backends.iter().map(|b| b.descriptor()).collect()
    }

    /// Resolves `req` to exactly one backend.
    ///
    /// Order of checks (each a distinct typed failure): explicit
    /// language/backend ids must be registered → contract-version
    /// floor → required capabilities + artifact kinds → uniqueness.
    ///
    /// # Errors
    /// [`ResolveError`] — see [`ResolveErrorReason`].
    pub fn resolve(&self, req: &ScriptRequirement) -> Result<Arc<dyn ScriptBackend>, ResolveError> {
        if let Some(lang) = &req.language
            && !self
                .backends
                .iter()
                .any(|b| &b.descriptor().language == lang)
        {
            return Err(ResolveError {
                reason: ResolveErrorReason::UnknownLanguage,
                language: req.language.clone(),
                backend: req.backend.clone(),
                missing: Vec::new(),
            });
        }
        if let Some(id) = &req.backend
            && !self.backends.iter().any(|b| &b.descriptor().id == id)
        {
            return Err(ResolveError {
                reason: ResolveErrorReason::UnknownBackend,
                language: req.language.clone(),
                backend: req.backend.clone(),
                missing: Vec::new(),
            });
        }

        let mut candidates: Vec<&Arc<dyn ScriptBackend>> = self
            .backends
            .iter()
            .filter(|b| {
                req.language
                    .as_ref()
                    .is_none_or(|l| &b.descriptor().language == l)
                    && req.backend.as_ref().is_none_or(|i| &b.descriptor().id == i)
            })
            .collect();

        if let Some(min) = req.min_contract_version {
            let before = candidates.len();
            candidates.retain(|b| b.descriptor().version.contract_version >= min);
            if candidates.is_empty() && before > 0 {
                return Err(ResolveError {
                    reason: ResolveErrorReason::UnsupportedContractVersion,
                    language: req.language.clone(),
                    backend: req.backend.clone(),
                    missing: Vec::new(),
                });
            }
        }

        let mut needed: BTreeSet<String> = req.required_capabilities.clone();
        needed.extend(req.artifact_kinds.iter().map(ArtifactKind::capability_id));
        if !needed.is_empty() {
            let mut best: Option<(&Arc<dyn ScriptBackend>, Vec<String>)> = None;
            for b in &candidates {
                let have = b.descriptor().capabilities.capability_ids();
                let missing: Vec<String> = needed.difference(&have).cloned().collect();
                match &best {
                    None => best = Some((b, missing)),
                    Some((_, prev)) if missing.len() < prev.len() => best = Some((b, missing)),
                    _ => {}
                }
            }
            if let Some((_, missing)) = &best
                && !missing.is_empty()
            {
                return Err(ResolveError {
                    reason: ResolveErrorReason::MissingCapability,
                    language: req.language.clone(),
                    backend: req.backend.clone(),
                    missing: missing.clone(),
                });
            }
            // Keep only candidates satisfying every needed capability —
            // otherwise two partial backends would look ambiguous.
            candidates.retain(|b| {
                let have = b.descriptor().capabilities.capability_ids();
                needed.is_subset(&have)
            });
        }

        match candidates.len() {
            0 => Err(ResolveError {
                reason: ResolveErrorReason::NoCandidates,
                language: req.language.clone(),
                backend: req.backend.clone(),
                missing: Vec::new(),
            }),
            1 => Ok(Arc::clone(candidates[0])),
            _ => Err(ResolveError {
                reason: ResolveErrorReason::Ambiguous,
                language: req.language.clone(),
                backend: req.backend.clone(),
                missing: Vec::new(),
            }),
        }
    }
}
