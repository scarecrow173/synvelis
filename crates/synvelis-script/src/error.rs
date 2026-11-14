//! Typed fail-closed errors for the script API surface.
//!
//! Every rejection names the contract that was violated so golden fixtures
//! can assert *which* rule fired, not just that something failed. Gateway
//! wire-layer failures (bad target shape, schema violation, consistency
//! rules, id/kind/time-domain mismatches) are preserved verbatim inside
//! [`ScriptApiError::Gateway`] — this crate never re-interprets them.

use std::fmt;

use synvelis_gateway::{CapabilityVersion, GatewayError, SchemaVersion};

use crate::analyzer::AnalyzerError;
use crate::model::ScriptVerdict;
use crate::surface::NameRejection;

/// Errors produced while capturing, projecting, generating, or assembling
/// the script API surface.
#[derive(Debug)]
pub enum ScriptApiError {
    /// Canonical JSON / serde failure.
    Json {
        /// Human-readable reason.
        reason: String,
    },
    /// A serialized snapshot or surface document is malformed or carries
    /// an unsupported `schema_version`.
    Snapshot {
        /// Human-readable reason.
        reason: String,
    },
    /// The snapshot lists the same `(id, version)` capability twice.
    DuplicateCapability {
        /// The duplicated capability id.
        id: String,
        /// The duplicated version.
        version: CapabilityVersion,
    },
    /// The snapshot lists the same `(name, version)` schema twice.
    DuplicateSchema {
        /// The duplicated schema name.
        name: String,
        /// The duplicated version.
        version: SchemaVersion,
    },
    /// Two surface entries claim the same script name — the public API
    /// name space must be unambiguous.
    DuplicateApiName {
        /// The duplicated script name.
        name: String,
    },
    /// A generated artifact (binding set / Luau stub) was built from a
    /// different model digest than the one it is checked against —
    /// schema/model drift.
    StaleArtifact {
        /// Which artifact drifted (`binding` / `luau-stub`).
        artifact: &'static str,
        /// The digest the artifact was generated from.
        generated_from: String,
        /// The digest of the model it is checked against.
        expected: String,
    },
    /// The runtime binding surface and the Luau stub surface disagree on
    /// the invocable entry set or its capability/schema pins.
    BindingStubMismatch {
        /// Human-readable detail.
        detail: String,
    },
    /// The script name is not present in the model at all.
    UnknownApi {
        /// The unknown script name.
        name: String,
    },
    /// The script name exists but cannot be invoked (hidden, disabled,
    /// planned, retired, unsupported version, unresolved schema, or an
    /// unusable name). Static discovery metadata never becomes authority.
    NotInvocable {
        /// The script name that was attempted.
        name: String,
        /// The typed blocker.
        blocker: InvocableBlocker,
    },
    /// A wire-layer failure surfaced while lowering a call — target
    /// shape/kind/id, consistency legality, or argument schema
    /// validation. The inner [`GatewayError`] is authoritative.
    Gateway(GatewayError),
    /// An internal model invariant failed — e.g. an entry marked
    /// invocable is missing data the verdict guaranteed. Never a
    /// user-facing rejection.
    Internal {
        /// Safe-to-disclose reason.
        reason: String,
    },
    /// The analyzer port failed (unavailable / unsupported capability /
    /// stale result / invalid request / version mismatch / internal).
    Analyzer(AnalyzerError),
    /// A [`crate::action::ScriptActionDefinition`] failed structural
    /// validation — bad `schema_version`-adjacent field shape
    /// (`script_id`, `source_digest` encoding, …). Unsupported
    /// `schema_version` has its own variant.
    Definition {
        /// Safe-to-disclose reason.
        reason: String,
    },
    /// The definition's `schema_version` is not supported — no
    /// fallback to another version exists.
    UnsupportedDefinitionVersion {
        /// The version found in the document.
        found: u32,
    },
    /// `source_digest` does not match the digest recomputed from the
    /// presented source/definition — source drift fails closed
    /// (RF-SCRIPT-00 M-13 family); there is no stale-version fallback.
    SourceDrift {
        /// The digest the definition declares.
        declared: String,
        /// The digest recomputed from the actual input.
        actual: String,
    },
    /// `backend_requirement` names a backend the host did not declare
    /// available — fail closed, never degrade to another backend.
    BackendUnavailable {
        /// The required backend (`luau` / `rhai`).
        required: String,
    },
    /// `language_id` and `backend_requirement` disagree (e.g. a Luau
    /// script requiring no backend, or YAML requiring Rhai).
    LanguageBackendMismatch {
        /// The declared language id.
        language: String,
        /// The declared backend requirement.
        backend: String,
    },
    /// A definition `input_schema`/`output_schema` ref did not resolve
    /// in the registry snapshot the model was built from.
    UnresolvedDefinitionSchema {
        /// The `name@major.minor` reference that failed.
        schema: String,
    },
    /// An [`crate::action::ActionRecipe`] was asked to lower through
    /// the wrong shape — e.g. `to_draft` on a batch recipe or
    /// `to_batch_draft` on a single recipe.
    RecipeKindMismatch {
        /// What was requested (`single` / `batch`).
        requested: &'static str,
        /// The recipe's actual kind.
        actual: &'static str,
    },
    /// The recipe spec contained zero member calls.
    EmptyRecipe,
    /// The recipe exceeds the batch member bound (mirrors the
    /// `synvelis.operation.batch` `max_ops` / `MAX_MACRO_MEMBERS` = 64
    /// contract).
    TooManyMembers {
        /// Members supplied.
        members: usize,
        /// The enforced maximum.
        max: usize,
    },
    /// A multi-member recipe contains a member whose domain cannot
    /// participate in one atomic batch (Query / Proposal /
    /// `RuntimeControl` / `AsyncOperation`). Such mixes route to the
    /// Workflow Coordinator instead — atomicity cannot be promised.
    UnsupportedMemberDomain {
        /// The script API name of the offending member.
        member: String,
        /// The member's operation domain.
        domain: synvelis_gateway::OperationDomain,
    },
    /// A multi-member recipe contains `synvelis.operation.batch`
    /// itself — nested batches are rejected.
    NestedBatch {
        /// The script API name of the offending member.
        member: String,
    },
    /// Multi-member recipe members disagree on the project — a batch
    /// binds exactly one project; cross-project work decomposes into
    /// Workflow steps.
    CrossProject {
        /// The member whose project differed from the first member's.
        member: String,
        /// The first member's project (the batch's single project).
        project: String,
    },
    /// Multi-member recipe members disagree on the consistency
    /// requirement — one batch carries exactly one
    /// `expected-revision`.
    ConsistencyMismatch {
        /// The member whose consistency differed.
        member: String,
    },
    /// A built recipe was verified against a definition that is not
    /// the one it was compiled from — `script_id`, `version`, or
    /// `definition_digest` drifted.
    DefinitionMismatch {
        /// Which field disagreed.
        field: &'static str,
    },
    /// The recipe's `script_version` disagrees with the presented
    /// definition's `version` — version drift fails closed.
    VersionDrift {
        /// The version the recipe was built from.
        expected: u32,
        /// The version the presented definition declares.
        actual: u32,
    },
    /// A workflow spec reuses a step/call id — ids occupy one
    /// namespace across actions, workflow calls, and control nodes.
    DuplicateStepId {
        /// The duplicated id.
        step: String,
    },
    /// An `action()` call inside a workflow spec names a capability
    /// whose domain/execution contract has no workflow step kind
    /// (Proposal / `RuntimeControl` / synchronous `AsyncOperation` /
    /// async-accept Query or `ProjectMutation`). Such calls belong to
    /// other surfaces — workflows compose query/invoke/async-submit.
    UnsupportedWorkflowStep {
        /// The script API name of the offending call.
        member: String,
        /// The capability's operation domain.
        domain: synvelis_gateway::OperationDomain,
        /// The capability's execution contract.
        execution: synvelis_gateway::ExecutionContract,
    },
}

/// Why a surface entry cannot be invoked — the fail-closed verdict
/// vocabulary for the script-facing API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvocableBlocker {
    /// The entry is marked hidden — it is never projected to the public
    /// surface (completion, stubs, bindings).
    Hidden,
    /// The capability is administratively disabled.
    Disabled {
        /// The registered, safe-to-disclose reason.
        reason: String,
    },
    /// The capability is declared for a later milestone.
    Planned,
    /// The capability was retired.
    Retired,
    /// The capability id is registered but not at the requested version.
    UnsupportedVersion {
        /// Versions actually registered for the id.
        supported: Vec<CapabilityVersion>,
    },
    /// The capability id is not registered at all.
    UnknownCapability,
    /// A declared input/output schema ref did not resolve in the
    /// snapshot.
    UnresolvedSchema {
        /// The unresolvable `name@version` reference.
        schema: String,
    },
    /// The script name is not a usable Luau identifier.
    InvalidName {
        /// Why the name was rejected.
        reason: NameRejection,
    },
}

impl InvocableBlocker {
    /// Derives the blocker for an entry that is not invocable, given its
    /// visibility and verdict. `None` when the entry *is* invocable.
    #[must_use]
    pub fn of(
        visibility: crate::surface::ScriptVisibility,
        verdict: &ScriptVerdict,
    ) -> Option<Self> {
        use crate::surface::ScriptVisibility;
        if visibility == ScriptVisibility::Hidden {
            return Some(Self::Hidden);
        }
        match verdict {
            ScriptVerdict::Invocable => None,
            ScriptVerdict::Disabled { reason } => Some(Self::Disabled {
                reason: reason.clone(),
            }),
            ScriptVerdict::Planned => Some(Self::Planned),
            ScriptVerdict::Retired => Some(Self::Retired),
            ScriptVerdict::UnsupportedVersion { supported } => Some(Self::UnsupportedVersion {
                supported: supported.clone(),
            }),
            ScriptVerdict::UnknownCapability => Some(Self::UnknownCapability),
            ScriptVerdict::UnresolvedSchema { schema } => Some(Self::UnresolvedSchema {
                schema: schema.clone(),
            }),
            ScriptVerdict::InvalidName { reason } => Some(Self::InvalidName { reason: *reason }),
        }
    }
}

impl fmt::Display for InvocableBlocker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hidden => write!(f, "hidden"),
            Self::Disabled { reason } => write!(f, "disabled: {reason}"),
            Self::Planned => write!(f, "planned"),
            Self::Retired => write!(f, "retired"),
            Self::UnsupportedVersion { supported } => {
                write!(f, "unsupported version (supported: {supported:?})")
            }
            Self::UnknownCapability => write!(f, "unknown capability"),
            Self::UnresolvedSchema { schema } => write!(f, "unresolved schema {schema}"),
            Self::InvalidName { reason } => write!(f, "invalid api name: {reason:?}"),
        }
    }
}

impl fmt::Display for ScriptApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json { reason } => write!(f, "canonical json failure: {reason}"),
            Self::Snapshot { reason } => write!(f, "invalid registry snapshot: {reason}"),
            Self::DuplicateCapability { id, version } => {
                write!(f, "duplicate capability registration {id}@{version}")
            }
            Self::DuplicateSchema { name, version } => {
                write!(f, "duplicate schema registration {name}@{version}")
            }
            Self::DuplicateApiName { name } => {
                write!(f, "duplicate script api name {name:?}")
            }
            Self::StaleArtifact {
                artifact,
                generated_from,
                expected,
            } => write!(
                f,
                "stale {artifact}: generated from model {generated_from}, expected {expected}"
            ),
            Self::BindingStubMismatch { detail } => {
                write!(f, "binding/stub surface mismatch: {detail}")
            }
            Self::UnknownApi { name } => write!(f, "unknown script api {name:?}"),
            Self::NotInvocable { name, blocker } => {
                write!(f, "script api {name:?} is not invocable: {blocker}")
            }
            Self::Gateway(err) => write!(f, "gateway rejection: {err:?}"),
            Self::Internal { reason } => write!(f, "internal invariant failure: {reason}"),
            Self::Analyzer(err) => write!(f, "analyzer failure: {err}"),
            Self::Definition { reason } => write!(f, "invalid action definition: {reason}"),
            Self::UnsupportedDefinitionVersion { found } => {
                write!(f, "unsupported action definition schema_version {found}")
            }
            Self::SourceDrift { declared, actual } => write!(
                f,
                "source digest drift: declared {declared}, recomputed {actual}"
            ),
            Self::BackendUnavailable { required } => {
                write!(f, "required script backend unavailable: {required}")
            }
            Self::LanguageBackendMismatch { language, backend } => write!(
                f,
                "language/backend mismatch: language {language}, backend {backend}"
            ),
            Self::UnresolvedDefinitionSchema { schema } => {
                write!(f, "unresolved definition schema {schema}")
            }
            Self::RecipeKindMismatch { requested, actual } => {
                write!(
                    f,
                    "recipe kind mismatch: requested {requested}, actual {actual}"
                )
            }
            Self::EmptyRecipe => write!(f, "action recipe has no members"),
            Self::TooManyMembers { members, max } => {
                write!(f, "action recipe has {members} members (max {max})")
            }
            Self::UnsupportedMemberDomain { member, domain } => {
                write!(
                    f,
                    "member {member:?} domain {domain:?} cannot join an atomic batch"
                )
            }
            Self::NestedBatch { member } => {
                write!(
                    f,
                    "member {member:?} is synvelis.operation.batch (nested batch)"
                )
            }
            Self::CrossProject { member, project } => {
                write!(
                    f,
                    "member {member:?} targets a different project than {project}"
                )
            }
            Self::ConsistencyMismatch { member } => {
                write!(f, "member {member:?} consistency differs from the batch's")
            }
            Self::DefinitionMismatch { field } => {
                write!(f, "recipe/definition mismatch on {field}")
            }
            Self::VersionDrift { expected, actual } => {
                write!(
                    f,
                    "script version drift: recipe built from {expected}, definition declares {actual}"
                )
            }
            Self::DuplicateStepId { step } => {
                write!(f, "duplicate workflow step/call id: {step}")
            }
            Self::UnsupportedWorkflowStep {
                member,
                domain,
                execution,
            } => write!(
                f,
                "member {member:?} ({domain:?}/{execution:?}) has no workflow step kind"
            ),
        }
    }
}

impl std::error::Error for ScriptApiError {}

impl From<GatewayError> for ScriptApiError {
    fn from(err: GatewayError) -> Self {
        Self::Gateway(err)
    }
}

impl From<crate::analyzer::AnalyzerError> for ScriptApiError {
    fn from(err: crate::analyzer::AnalyzerError) -> Self {
        Self::Analyzer(err)
    }
}
