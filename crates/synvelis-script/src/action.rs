//! [`ScriptActionDefinition`] and the typed [`ActionRecipe`] — the
//! SV-SCRIPT-02 lowering from a script-level action contract to the
//! existing Gateway invocation / atomic-batch path.
//!
//! Contract (`docs/automation/design-script.md`):
//!
//! - A script action is *one implementation of an Action*
//!   (`CapabilityRef`) — there is no second capability registry, no
//!   independent executor, and no script-owned run surface. Every
//!   member resolves through the same [`BindingSet`] /
//!   [`ScriptApiModel`] surface SV-SCRIPT-01 produced, and every
//!   lowered draft still passes `Gateway::bind` → authorization →
//!   schema re-validation → approval → commit unchanged.
//! - [`ScriptActionDefinition`] pins `script_id` + `version` +
//!   `source_digest` + `language_id` + `backend_requirement` +
//!   input/output [`SchemaRef`]s. Source or version drift is a typed
//!   fail-closed error — never a silent fallback to an older version
//!   or another backend.
//! - [`ActionRecipe::build`] resolves each member call through
//!   [`ScriptApiModel::resolve_call`] — the same structural validation
//!   [`crate::binding::ScriptApiModel::assemble_call`] performs — then
//!   classifies the member set:
//!
//!   | Members | Domain rule | Lowering |
//!   |---|---|---|
//!   | 1 | any invocable domain | single [`InvocationDraft`] — `ProjectMutation` members are wrapped in `synvelis.operation.prepare`; other domains go direct |
//!   | 2–64 | every member `ProjectMutation`, same project, same consistency | batch recipe → `synvelis-automation`'s `MacroCompiler` path → one `synvelis.operation.batch` inside `synvelis.operation.prepare` |
//!   | 65+ | — | [`ScriptApiError::TooManyMembers`] |
//!   | mixed domains / nested batch / cross-project / inconsistent consistency | — | typed rejection (route to Workflow for non-atomic mixes) |
//!
//! The recipe carries no `CallerIdentity`, grant, approval, token, or
//! permission — those types are not representable here and remain
//! Host/Gateway concerns ([`InvocationDraft::submit`] takes the
//! caller from the trusted host).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use synvelis_gateway::{
    CapabilityId, CapabilityRef, CapabilityVersion, OperationDomain, RequestId, SchemaRef,
};

use crate::binding::{BindingSet, InvocationDraft, ScriptConsistency, ScriptTarget};
use crate::canon::digest_of;
use crate::error::ScriptApiError;
use crate::model::ScriptApiModel;
use crate::snapshot::RegistrySnapshot;

/// Action-definition document format version.
pub const ACTION_DEFINITION_SCHEMA_VERSION: u32 = 1;
/// Action-recipe document format version.
pub const ACTION_RECIPE_SCHEMA_VERSION: u32 = 1;
/// Maximum members one recipe may carry — mirrors the
/// `synvelis.operation.batch` `max_ops` bound and
/// `synvelis_automation::MAX_MACRO_MEMBERS` (64).
pub const MAX_RECIPE_MEMBERS: usize = 64;

/// `synvelis.operation.prepare` — the Proposal-domain capability every
/// `ProjectMutation` lowering flows through (GW-04 contract; the same
/// canonical id any wire client names).
const PREPARE_CAPABILITY_ID: &str = "synvelis.operation.prepare";
/// `synvelis.operation.batch` — the single atomic-batch capability
/// (AUTX-04 / GW-04 contract).
const BATCH_CAPABILITY_ID: &str = "synvelis.operation.batch";
const PREPARE_INPUT_SCHEMA: &str = "synvelis.operation.prepare.input";

/// Script language identifier — a closed set per the RF-SCRIPT-00
/// contract vocabulary. Unknown languages fail at deserialization;
/// there is no Lua 5.4 or arbitrary-language fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LanguageId {
    /// YAML-defined action (declarative recipe source).
    Yaml,
    /// Luau action script — the SV-SCRIPT-02 target language.
    Luau,
    /// Rhai action script — PoC-level contract conformance only;
    /// formal product support is out of scope.
    Rhai,
}

impl LanguageId {
    /// The kebab-case id used in errors and digests.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Yaml => "yaml",
            Self::Luau => "luau",
            Self::Rhai => "rhai",
        }
    }
}

/// Backend capability a script requires at evaluation time. The host
/// declares which backends exist via [`AvailableBackends`]; a
/// requirement the host cannot satisfy fails closed — scripts never
/// degrade to another backend or language.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendRequirement {
    /// No VM backend required (e.g. YAML recipes evaluated by the
    /// host's own lowering).
    None,
    /// Requires a Luau backend.
    Luau,
    /// Requires a Rhai backend.
    Rhai,
}

impl BackendRequirement {
    /// The kebab-case id used in errors.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Luau => "luau",
            Self::Rhai => "rhai",
        }
    }
}

/// The script backends the host declares as installed. Evaluation
/// backends are *host* capabilities — the definition only names what
/// it needs; absence is a typed rejection, never an assumption.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AvailableBackends {
    /// A Luau evaluation backend is present.
    pub luau: bool,
    /// A Rhai evaluation backend is present.
    pub rhai: bool,
}

impl AvailableBackends {
    /// Only a Luau backend.
    #[must_use]
    pub const fn luau() -> Self {
        Self {
            luau: true,
            rhai: false,
        }
    }

    /// Both backends present.
    #[must_use]
    pub const fn luau_and_rhai() -> Self {
        Self {
            luau: true,
            rhai: true,
        }
    }
}

/// The versioned, digest-pinned contract of one script action —
/// `docs/automation/design-script.md` §`ScriptActionDefinition`.
///
/// Authority-free by construction: the type carries identity,
/// language/backend requirements, a source digest, and schema refs —
/// no grants, approvals, tokens, principals, or executors.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptActionDefinition {
    /// Must equal [`ACTION_DEFINITION_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Stable script id (`[a-z0-9._-]`, ≤ 64) — unique with
    /// [`Self::version`].
    pub script_id: String,
    /// Script version — must agree with the source the
    /// [`Self::source_digest`] pins; drift is rejected, never
    /// substituted.
    pub version: u32,
    /// Script language.
    pub language_id: LanguageId,
    /// Required evaluation backend.
    pub backend_requirement: BackendRequirement,
    /// SHA-256 (lowercase hex) of the canonical script source —
    /// checked against the presented source via
    /// [`Self::verify_source`]; mismatch is [`ScriptApiError::SourceDrift`].
    pub source_digest: String,
    /// The script-level input schema ref.
    pub input_schema: SchemaRef,
    /// The script-level output schema ref.
    pub output_schema: SchemaRef,
}

impl ScriptActionDefinition {
    /// Structural + environment validation: schema version, id/digest
    /// shapes, language↔backend consistency, backend availability, and
    /// input/output schema resolution in `snapshot`.
    ///
    /// # Errors
    /// [`ScriptApiError::UnsupportedDefinitionVersion`] /
    /// [`ScriptApiError::Definition`] — bad fields;
    /// [`ScriptApiError::LanguageBackendMismatch`] —
    /// [`ScriptApiError::BackendUnavailable`] — environment;
    /// [`ScriptApiError::UnresolvedDefinitionSchema`] — dangling refs.
    pub fn validate(
        &self,
        backends: &AvailableBackends,
        snapshot: &RegistrySnapshot,
    ) -> Result<(), ScriptApiError> {
        if self.schema_version != ACTION_DEFINITION_SCHEMA_VERSION {
            return Err(ScriptApiError::UnsupportedDefinitionVersion {
                found: self.schema_version,
            });
        }
        validate_script_id(&self.script_id)?;
        if self.source_digest.len() != 64
            || !self
                .source_digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ScriptApiError::Definition {
                reason: format!(
                    "source_digest must be 64 lowercase hex chars, got {:?}",
                    self.source_digest
                ),
            });
        }
        // Language ↔ backend consistency: each language has exactly
        // one honest backend requirement.
        let expected = match self.language_id {
            LanguageId::Yaml => BackendRequirement::None,
            LanguageId::Luau => BackendRequirement::Luau,
            LanguageId::Rhai => BackendRequirement::Rhai,
        };
        if self.backend_requirement != expected {
            return Err(ScriptApiError::LanguageBackendMismatch {
                language: self.language_id.as_str().to_owned(),
                backend: self.backend_requirement.as_str().to_owned(),
            });
        }
        let available = match self.backend_requirement {
            BackendRequirement::None => true,
            BackendRequirement::Luau => backends.luau,
            BackendRequirement::Rhai => backends.rhai,
        };
        if !available {
            return Err(ScriptApiError::BackendUnavailable {
                required: self.backend_requirement.as_str().to_owned(),
            });
        }
        for schema_ref in [&self.input_schema, &self.output_schema] {
            if snapshot.resolve_schema(schema_ref).is_none() {
                return Err(ScriptApiError::UnresolvedDefinitionSchema {
                    schema: format!(
                        "{}@{}.{}",
                        schema_ref.name, schema_ref.version.major, schema_ref.version.minor
                    ),
                });
            }
        }
        Ok(())
    }

    /// Canonical digest of the definition — the drift anchor a built
    /// [`ActionRecipe`] carries and [`ActionRecipe::verify_definition`]
    /// re-checks.
    ///
    /// # Errors
    /// [`ScriptApiError::Json`] — canonicalization failure.
    pub fn digest(&self) -> Result<String, ScriptApiError> {
        digest_of(self)
    }

    /// Verifies `actual_source_digest` (the digest recomputed from the
    /// script source being registered/run) against the declared
    /// [`Self::source_digest`]. Any mismatch is
    /// [`ScriptApiError::SourceDrift`] — fail closed, no fallback.
    ///
    /// # Errors
    /// [`ScriptApiError::SourceDrift`] — digests differ.
    pub fn verify_source(&self, actual_source_digest: &str) -> Result<(), ScriptApiError> {
        if actual_source_digest == self.source_digest {
            Ok(())
        } else {
            Err(ScriptApiError::SourceDrift {
                declared: self.source_digest.clone(),
                actual: actual_source_digest.to_owned(),
            })
        }
    }
}

/// One member call inside a recipe spec — the per-call inputs a
/// script evaluation produced, minus request-level fields (request
/// id / task / grant / idempotency key live on [`RecipeSpec`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeMemberSpec {
    /// Script API name — resolved through the [`BindingSet`].
    pub name: String,
    /// Operation arguments (schema-validated; `SynvelisI64` decimal
    /// strings decode where the schema says `Integer`).
    pub args: Value,
    /// Explicit target — required iff the capability's target
    /// contract requires one.
    #[serde(default)]
    pub target: Option<ScriptTarget>,
    /// Tagged project id — every member of a batch recipe must agree.
    pub project: String,
    /// Domain-appropriate consistency requirement.
    pub consistency: ScriptConsistency,
}

/// Request-level fields shared by every member of a recipe — mirrors
/// the wire request envelope minus authority (which scripts cannot
/// express).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeSpec {
    /// The member calls, in order.
    pub members: Vec<RecipeMemberSpec>,
    /// Caller-assigned correlation id for the lowered request.
    pub request_id: String,
    /// Optional opaque task reference — not an authorization proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// Optional opaque grant reference — not an authorization proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant: Option<String>,
    /// Optional idempotency key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// One resolved recipe member — the exact capability pin, lowered
/// args, and wire-shaped target after [`ScriptApiModel::resolve_call`]
/// validation. Plain data; carries no authority.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeMember {
    /// Script API name (traceability back to the surface spec).
    pub name: String,
    /// Exact capability pin (`id@major.minor`).
    pub capability: CapabilityRef,
    /// Operation domain — batch eligibility was checked at build.
    pub domain: OperationDomain,
    /// Declared input schema ref — the wire `schema` field for a
    /// direct (non-prepare) lowering.
    pub input_schema: SchemaRef,
    /// The typed script target as supplied (drives the automation
    /// `Literal` target lowering).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<ScriptTarget>,
    /// Wire-shaped target JSON (contract-fixed `creates` injected) —
    /// used verbatim by the single-member draft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_wire: Option<Value>,
    /// Lowered, schema-validated arguments (always an object for
    /// `ProjectMutation` members — the batch wire requires it).
    pub args: Value,
}

/// How a built [`ActionRecipe`] lowers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionRecipeKind {
    /// One member — lowers to a single [`InvocationDraft`]
    /// (prepare-wrapped for `ProjectMutation`, direct otherwise).
    Single,
    /// 2–64 `ProjectMutation` members on one project with one
    /// consistency — lowers through the atomic batch path.
    Batch,
}

impl ActionRecipeKind {
    /// The kebab-case kind name for typed errors.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Batch => "batch",
        }
    }
}

/// The hashed payload of a recipe — everything except `recipe_digest`
/// itself.
#[derive(Serialize)]
struct RecipePayload<'a> {
    schema_version: u32,
    script_id: &'a str,
    script_version: u32,
    definition_digest: &'a str,
    binding_digest: &'a str,
    model_digest: &'a str,
    members: &'a [RecipeMember],
    project: &'a str,
    consistency: &'a ScriptConsistency,
    request_id: &'a str,
    task: &'a Option<String>,
    grant: &'a Option<String>,
    idempotency_key: &'a Option<String>,
}

/// A typed Action Recipe: the resolved, digest-pinned lowering of one
/// script action's member calls. Authority-free — request-level
/// identity fields (`task`/`grant`/`idempotency_key`) are opaque
/// references, and `CallerIdentity` is supplied by the host at submit
/// time, never carried here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionRecipe {
    /// Must equal [`ACTION_RECIPE_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// The [`ScriptActionDefinition::script_id`] this recipe implements.
    pub script_id: String,
    /// The [`ScriptActionDefinition::version`] this recipe implements.
    pub script_version: u32,
    /// Digest of the [`ScriptActionDefinition`] the recipe was built
    /// from — drift-checked by [`Self::verify_definition`].
    pub definition_digest: String,
    /// Digest of the [`BindingSet`] the members resolved through.
    pub binding_digest: String,
    /// Digest of the [`ScriptApiModel`] the bindings were generated
    /// from.
    pub model_digest: String,
    /// Resolved members in order (1 for `Single`, 2–64 for `Batch`).
    pub members: Vec<RecipeMember>,
    /// The single project every member binds (batch invariant).
    pub project: String,
    /// The single consistency every member shares.
    pub consistency: ScriptConsistency,
    /// Caller-assigned correlation id.
    pub request_id: String,
    /// Optional opaque task reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// Optional opaque grant reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant: Option<String>,
    /// Optional idempotency key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
    /// Digest of this recipe's canonical payload.
    pub recipe_digest: String,
}

impl ActionRecipe {
    /// Builds a typed recipe from a validated definition, the
    /// SV-SCRIPT-01 model/binding surface, and the member call spec.
    ///
    /// Fail-closed order: definition validation (incl. backend
    /// availability + schema resolution) → binding↔model drift →
    /// member count bounds → per-member resolution (the same
    /// name/verdict/target/consistency/args checks
    /// [`ScriptApiModel::assemble_call`] performs) → batch
    /// classification rules (single-project, single-consistency,
    /// `ProjectMutation`-only, no nested batch).
    ///
    /// # Errors
    /// [`ScriptApiError`] — any definition, drift, resolution, or
    /// batch-classification rejection; all typed, all fail-closed.
    pub fn build(
        definition: &ScriptActionDefinition,
        backends: &AvailableBackends,
        snapshot: &RegistrySnapshot,
        model: &ScriptApiModel,
        bindings: &BindingSet,
        spec: RecipeSpec,
    ) -> Result<Self, ScriptApiError> {
        definition.validate(backends, snapshot)?;
        bindings.verify_against(model)?;
        RequestId::new(spec.request_id.clone())?;

        if spec.members.is_empty() {
            return Err(ScriptApiError::EmptyRecipe);
        }
        if spec.members.len() > MAX_RECIPE_MEMBERS {
            return Err(ScriptApiError::TooManyMembers {
                members: spec.members.len(),
                max: MAX_RECIPE_MEMBERS,
            });
        }

        let mut members = Vec::with_capacity(spec.members.len());
        for member_spec in &spec.members {
            let resolved = model.resolve_call(
                &member_spec.name,
                &member_spec.args,
                member_spec.target.as_ref(),
                &member_spec.project,
                &member_spec.consistency,
            )?;
            // Batch members lower through `ops[]` triples whose `args`
            // must be a JSON object (the prepare wire contract) —
            // reject early with a typed error rather than at prepare.
            if resolved.domain == OperationDomain::ProjectMutation && !resolved.args.is_object() {
                return Err(ScriptApiError::Gateway(
                    synvelis_gateway::GatewayError::SchemaViolation {
                        path: format!("$.members[{}].args", members.len()),
                        reason: "mutation member args must be a JSON object".to_owned(),
                    },
                ));
            }
            members.push(RecipeMember {
                name: member_spec.name.clone(),
                capability: resolved.capability,
                domain: resolved.domain,
                input_schema: resolved.input_schema,
                target: member_spec.target.clone(),
                target_wire: resolved.target,
                args: resolved.args,
            });
        }

        let (project, consistency) = if members.len() == 1 {
            let spec_member = &spec.members[0];
            (spec_member.project.clone(), spec_member.consistency.clone())
        } else {
            // Batch classification: every member must be a
            // synchronous-eligible ProjectMutation on ONE project
            // with ONE consistency — anything else either cannot
            // be atomic (route to Workflow) or is a typed reject.
            let first_project = &spec.members[0].project;
            let first_consistency = &spec.members[0].consistency;
            for (member, resolved) in spec.members.iter().zip(&members) {
                if resolved.domain != OperationDomain::ProjectMutation {
                    return Err(ScriptApiError::UnsupportedMemberDomain {
                        member: member.name.clone(),
                        domain: resolved.domain,
                    });
                }
                if resolved.capability.id.as_str() == BATCH_CAPABILITY_ID {
                    return Err(ScriptApiError::NestedBatch {
                        member: member.name.clone(),
                    });
                }
                if &member.project != first_project {
                    return Err(ScriptApiError::CrossProject {
                        member: member.name.clone(),
                        project: first_project.clone(),
                    });
                }
                if &member.consistency != first_consistency {
                    return Err(ScriptApiError::ConsistencyMismatch {
                        member: member.name.clone(),
                    });
                }
            }
            (first_project.clone(), first_consistency.clone())
        };

        let definition_digest = definition.digest()?;
        let mut recipe = Self {
            schema_version: ACTION_RECIPE_SCHEMA_VERSION,
            script_id: definition.script_id.clone(),
            script_version: definition.version,
            definition_digest,
            binding_digest: bindings.binding_digest.clone(),
            model_digest: model.model_digest.clone(),
            members,
            project,
            consistency,
            request_id: spec.request_id,
            task: spec.task,
            grant: spec.grant,
            idempotency_key: spec.idempotency_key,
            recipe_digest: String::new(),
        };
        recipe.recipe_digest = recipe.compute_digest()?;
        Ok(recipe)
    }

    /// Whether the recipe lowers as a single draft or an atomic batch.
    #[must_use]
    pub fn kind(&self) -> ActionRecipeKind {
        if self.members.len() == 1 {
            ActionRecipeKind::Single
        } else {
            ActionRecipeKind::Batch
        }
    }

    fn compute_digest(&self) -> Result<String, ScriptApiError> {
        let payload = RecipePayload {
            schema_version: self.schema_version,
            script_id: &self.script_id,
            script_version: self.script_version,
            definition_digest: &self.definition_digest,
            binding_digest: &self.binding_digest,
            model_digest: &self.model_digest,
            members: &self.members,
            project: &self.project,
            consistency: &self.consistency,
            request_id: &self.request_id,
            task: &self.task,
            grant: &self.grant,
            idempotency_key: &self.idempotency_key,
        };
        digest_of(&payload)
    }

    /// Digest of the canonical recipe payload.
    ///
    /// # Errors
    /// [`ScriptApiError::Json`] — canonicalization failure.
    pub fn digest(&self) -> Result<String, ScriptApiError> {
        self.compute_digest()
    }

    /// Verifies a presented definition is the one this recipe was
    /// built from — `script_id`, `version`, and `definition_digest`
    /// must all agree. Any drift is typed and fail-closed.
    ///
    /// # Errors
    /// [`ScriptApiError::DefinitionMismatch`] — id/digest drift;
    /// [`ScriptApiError::VersionDrift`] — version drift;
    /// [`ScriptApiError::Json`] — digest canonicalization failure.
    pub fn verify_definition(
        &self,
        definition: &ScriptActionDefinition,
    ) -> Result<(), ScriptApiError> {
        if definition.script_id != self.script_id {
            return Err(ScriptApiError::DefinitionMismatch { field: "script_id" });
        }
        if definition.version != self.script_version {
            return Err(ScriptApiError::VersionDrift {
                expected: self.script_version,
                actual: definition.version,
            });
        }
        if definition.digest()? != self.definition_digest {
            return Err(ScriptApiError::DefinitionMismatch {
                field: "definition_digest",
            });
        }
        Ok(())
    }

    /// Whether the recipe's shared consistency pins an expected
    /// revision — informational for callers binding the same base the
    /// recipe was built against (the automation compiler echoes it
    /// into its compiled artifact).
    #[must_use]
    pub fn expected_revision(&self) -> Option<u64> {
        match self.consistency {
            ScriptConsistency::ExpectedRevision { revision } => Some(revision),
            _ => None,
        }
    }

    /// Lowers a `Single` recipe to its [`InvocationDraft`].
    ///
    /// - `ProjectMutation` member → a `synvelis.operation.prepare`
    ///   draft wrapping the member triple (the only path mutation ops
    ///   have — they carry no direct executor).
    /// - Any other domain → a direct draft at the member's own
    ///   capability/schema/target.
    ///
    /// # Errors
    /// [`ScriptApiError::RecipeKindMismatch`] — not a single recipe;
    /// [`ScriptApiError::Gateway`] — consistency serialization failure.
    pub fn to_draft(&self) -> Result<InvocationDraft, ScriptApiError> {
        if self.kind() != ActionRecipeKind::Single {
            return Err(ScriptApiError::RecipeKindMismatch {
                requested: ActionRecipeKind::Single.as_str(),
                actual: self.kind().as_str(),
            });
        }
        let member = &self.members[0];
        if member.domain == OperationDomain::ProjectMutation {
            let mut operation = serde_json::Map::new();
            operation.insert(
                "capability".to_owned(),
                serde_json::to_value(&member.capability).map_err(|e| ScriptApiError::Json {
                    reason: e.to_string(),
                })?,
            );
            if let Some(target) = &member.target_wire {
                operation.insert("target".to_owned(), target.clone());
            }
            operation.insert("args".to_owned(), member.args.clone());
            self.prepare_draft(json!({ "operation": Value::Object(operation) }))
        } else {
            Ok(InvocationDraft {
                request_id: self.request_id.clone(),
                capability: member.capability.clone(),
                schema: member.input_schema.clone(),
                project: self.project.clone(),
                target: member.target_wire.clone(),
                args: member.args.clone(),
                consistency: self.wire_consistency()?,
                task: self.task.clone(),
                grant: self.grant.clone(),
                idempotency_key: self.idempotency_key.clone(),
            })
        }
    }

    /// Lowers a `Batch` recipe to the `synvelis.operation.prepare`
    /// draft carrying `batch_args` — the `{ops[, bindings]}` object a
    /// compiled atomic macro produces (AUTX-04 owns the ops[]/binding
    /// semantics; this method only wraps them in the prepare
    /// envelope).
    ///
    /// `batch_args` must be an object containing an `ops` array; full
    /// member validation is the Gateway's job at prepare time.
    ///
    /// # Errors
    /// [`ScriptApiError::RecipeKindMismatch`] — not a batch recipe;
    /// [`ScriptApiError::Gateway`] — malformed `batch_args` or
    /// consistency serialization failure.
    pub fn to_batch_draft(&self, batch_args: &Value) -> Result<InvocationDraft, ScriptApiError> {
        if self.kind() != ActionRecipeKind::Batch {
            return Err(ScriptApiError::RecipeKindMismatch {
                requested: ActionRecipeKind::Batch.as_str(),
                actual: self.kind().as_str(),
            });
        }
        let ops_ok = batch_args
            .get("ops")
            .and_then(Value::as_array)
            .is_some_and(|ops| !ops.is_empty());
        if !batch_args.is_object() || !ops_ok {
            return Err(ScriptApiError::Gateway(
                synvelis_gateway::GatewayError::SchemaViolation {
                    path: "$.args.operation.args".to_owned(),
                    reason: "batch args must be an object with a non-empty ops array".to_owned(),
                },
            ));
        }
        let operation = json!({
            "capability": {
                "id": BATCH_CAPABILITY_ID,
                "version": {"major": 1, "minor": 0},
            },
            "args": batch_args,
        });
        self.prepare_draft(json!({ "operation": operation }))
    }

    /// The shared prepare-envelope draft — `synvelis.operation.prepare`
    /// at v1.0 with `args = {"operation": <inner>}` and the recipe's
    /// consistency (every mutation lowering pins
    /// `expected-revision`; CAS/undo semantics stay with GW-04/06).
    fn prepare_draft(&self, args: Value) -> Result<InvocationDraft, ScriptApiError> {
        Ok(InvocationDraft {
            request_id: self.request_id.clone(),
            capability: prepare_capability(),
            schema: prepare_input_schema(),
            project: self.project.clone(),
            target: None,
            args,
            consistency: self.wire_consistency()?,
            task: self.task.clone(),
            grant: self.grant.clone(),
            idempotency_key: self.idempotency_key.clone(),
        })
    }

    /// The wire consistency object for a direct (non-prepare) draft —
    /// the same serialization [`crate::binding`] assembly produces.
    fn wire_consistency(&self) -> Result<Value, ScriptApiError> {
        let consistency = self.consistency.to_gateway()?;
        serde_json::to_value(&consistency).map_err(|e| ScriptApiError::Json {
            reason: e.to_string(),
        })
    }
}

/// The `synvelis.operation.prepare@1.0` capability ref — mirrors the
/// GW-04 registered descriptor exactly (any drift fails `bind`'s
/// exact-version resolution, so this constant cannot silently rot).
fn prepare_capability() -> CapabilityRef {
    CapabilityRef {
        id: CapabilityId::new(PREPARE_CAPABILITY_ID).expect("prepare capability id is valid"),
        version: CapabilityVersion::new(1, 0),
    }
}

/// The `synvelis.operation.prepare.input@1.0` schema ref.
fn prepare_input_schema() -> SchemaRef {
    SchemaRef::new(PREPARE_INPUT_SCHEMA, 1, 0)
}

/// `[a-z0-9._-]`, 1..=64 — the script-id shape shared with macro ids
/// and capability ids.
pub(crate) fn validate_script_id(script_id: &str) -> Result<(), ScriptApiError> {
    let valid = !script_id.is_empty()
        && script_id.len() <= 64
        && script_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(ScriptApiError::Definition {
            reason: format!("script_id must be 1..=64 chars of [a-z0-9._-], got {script_id:?}"),
        })
    }
}
