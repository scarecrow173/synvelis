//! Runtime binding generation and typed invocation assembly.
//!
//! [`BindingSet`] is generated **from the [`ScriptApiModel`]** — the same
//! model that feeds [`crate::luau::LuauStub`] — so the two artifacts can
//! never describe different surfaces. Each [`BindingEntry`] pins the exact
//! capability ref, declared schema refs, resolved schemas, target
//! contract, domain, and execution contract for one invocable entry.
//!
//! [`ScriptApiModel::assemble_call`] lowers a typed [`InvocationCall`]
//! into an [`InvocationDraft`] — the wire-shaped `OperationRequest`
//! precursor. Assembly performs *structural* validation only (name →
//! verdict → target shape/kind/id → consistency legality → argument
//! schema). It never calls `Gateway::bind`/`invoke`, never mints
//! `CallerIdentity`/`AdapterSeal`/grants/approvals, and the emitted draft
//! still passes through full Gateway authorization and schema
//! re-validation at invoke time. Static types are not grants.
//!
//! # i64 precision
//!
//! Luau `number` is f64; schema `Integer` ranges outside the f64-exact
//! interval project to `SynvelisI64` (decimal text). [`lower_value`]
//! accepts that documented string encoding *only* where the schema says
//! `Integer`, parses it strictly to `i64`, range-checks it, and lowers it
//! to a JSON number — explicit decoding, never implicit rounding. Any
//! other type mismatch is a typed [`GatewayError::SchemaViolation`].

use serde::{Deserialize, Serialize};
use serde_json::Value;
use synvelis_gateway::{
    CapabilityRef, Consistency, ExecutionContract, GatewayError, OperationDomain, RequestId,
    ResourceKind, ResourceRef, SchemaRef, SnapshotRef, TargetContract, WireRequest,
};
use synvelis_project::ProjectRevision;

use crate::canon::digest_of;
use crate::error::{InvocableBlocker, ScriptApiError};
use crate::model::ScriptApiModel;
use crate::schema::ScriptSchema;

/// Binding document format version.
pub const BINDING_SCHEMA_VERSION: u32 = 1;

/// One invocable capability's runtime binding recipe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingEntry {
    /// Script API name.
    pub name: String,
    /// Exact capability pin.
    pub capability: CapabilityRef,
    /// Operation domain (consistency legality).
    pub domain: OperationDomain,
    /// Declared input schema ref — becomes the wire `schema` field.
    pub input_schema: SchemaRef,
    /// Declared output schema ref (provenance/completeness).
    pub output_schema: SchemaRef,
    /// Required target shape.
    pub target: TargetContract,
    /// Sync/async execution contract (provenance).
    pub execution: ExecutionContract,
    /// Resolved input schema (argument validation + i64 decode).
    pub input: ScriptSchema,
    /// Resolved output schema (provenance).
    pub output: ScriptSchema,
    /// Optional description (safe to disclose).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// The deterministic runtime binding set for a model.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingSet {
    /// Binding document format version.
    pub schema_version: u32,
    /// Digest of the [`ScriptApiModel`] this set was generated from.
    pub model_digest: String,
    /// Digest of the source registry snapshot.
    pub snapshot_digest: String,
    /// Digest of the surface spec.
    pub surface_digest: String,
    /// Digest of this set's canonical payload.
    pub binding_digest: String,
    /// Entries sorted by name.
    pub entries: Vec<BindingEntry>,
}

/// The hashed payload of a binding set.
#[derive(Serialize)]
struct BindingPayload<'a> {
    schema_version: u32,
    model_digest: &'a str,
    snapshot_digest: &'a str,
    surface_digest: &'a str,
    entries: &'a [BindingEntry],
}

impl BindingSet {
    /// Generates the binding set from the model's invocable public
    /// entries. Hidden and non-`Invocable` entries produce **no**
    /// binding — they cannot be assembled into calls.
    ///
    /// # Errors
    /// [`ScriptApiError::Internal`] — an entry marked invocable lacks
    /// the descriptor/schema data its verdict guarantees (only
    /// reachable from a hand-edited serialized model);
    /// [`ScriptApiError::Json`] — canonicalization failure.
    pub fn generate(model: &ScriptApiModel) -> Result<Self, ScriptApiError> {
        let mut entries = Vec::new();
        for entry in model.invocable_entries() {
            let internal = |what: &str| ScriptApiError::Internal {
                reason: format!("invocable entry {:?} lacks {what}", entry.name),
            };
            let descriptor = entry
                .descriptor
                .as_ref()
                .ok_or_else(|| internal("a descriptor"))?;
            let input_binding = entry
                .input
                .as_ref()
                .ok_or_else(|| internal("an input schema"))?;
            let output_binding = entry
                .output
                .as_ref()
                .ok_or_else(|| internal("an output schema"))?;
            entries.push(BindingEntry {
                name: entry.name.clone(),
                capability: entry.requested.clone(),
                domain: descriptor.domain,
                input_schema: input_binding.schema.clone(),
                output_schema: output_binding.schema.clone(),
                target: descriptor.target,
                execution: descriptor.execution,
                input: input_binding.resolved.clone(),
                output: output_binding.resolved.clone(),
                description: entry.description.clone(),
            });
        }

        let payload = BindingPayload {
            schema_version: BINDING_SCHEMA_VERSION,
            model_digest: &model.model_digest,
            snapshot_digest: &model.snapshot_digest,
            surface_digest: &model.surface_digest,
            entries: &entries,
        };
        let binding_digest = digest_of(&payload)?;

        Ok(Self {
            schema_version: BINDING_SCHEMA_VERSION,
            model_digest: model.model_digest.clone(),
            snapshot_digest: model.snapshot_digest.clone(),
            surface_digest: model.surface_digest.clone(),
            binding_digest,
            entries,
        })
    }

    /// Verifies this set was generated from `model` — the drift check.
    ///
    /// # Errors
    /// [`ScriptApiError::StaleArtifact`] — model digest mismatch.
    pub fn verify_against(&self, model: &ScriptApiModel) -> Result<(), ScriptApiError> {
        model.verify_artifact("binding", &self.model_digest)
    }

    /// Looks up a binding by script name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&BindingEntry> {
        self.entries.iter().find(|e| e.name == name)
    }
}

/// Wire-shaped explicit target — mirrors `WireTarget` but with typed
/// [`ResourceKind`] fields so kind mismatches are compile-time
/// impossible and deserialize-time kind names are validated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "kebab-case")]
pub enum ScriptTarget {
    /// One existing resource.
    Resource {
        /// Required resource kind.
        kind: ResourceKind,
        /// Tagged stable id.
        id: String,
    },
    /// A non-empty set of existing resources of one kind.
    ResourceSet {
        /// Required resource kind.
        kind: ResourceKind,
        /// Tagged stable ids.
        ids: Vec<String>,
    },
    /// Create under an explicit parent; `creates` is implied by the
    /// capability's target contract.
    Create {
        /// Required kind of the parent.
        parent_kind: ResourceKind,
        /// Tagged stable id of the parent.
        parent: String,
    },
}

impl ScriptTarget {
    /// The wire `shape` tag, for typed mismatch errors.
    fn shape(&self) -> &'static str {
        match self {
            Self::Resource { .. } => "resource",
            Self::ResourceSet { .. } => "resource-set",
            Self::Create { .. } => "create",
        }
    }
}

/// Wire-shaped consistency requirement — mirrors `WireConsistency`
/// exactly (same `kind` tags and field names).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ScriptConsistency {
    /// Read latest committed state (query domain only).
    Latest,
    /// Read pinned to an exact committed revision (query only).
    ReadAt {
        /// The committed revision.
        revision: u64,
    },
    /// Read from a fixed immutable snapshot (query only).
    FixedSnapshot {
        /// The snapshot reference.
        snapshot: String,
    },
    /// Mutate only if live revision equals expectation (mutation only).
    ExpectedRevision {
        /// The expected live revision.
        revision: u64,
    },
    /// No project-revision condition (runtime/async only).
    Unconditional,
}

impl ScriptConsistency {
    /// Converts to the authoritative Gateway [`Consistency`] for the
    /// `allowed_in` legality check.
    pub(crate) fn to_gateway(&self) -> Result<Consistency, GatewayError> {
        Ok(match self {
            Self::Latest => Consistency::Latest,
            Self::ReadAt { revision } => Consistency::ReadAt {
                revision: ProjectRevision(*revision),
            },
            Self::FixedSnapshot { snapshot } => Consistency::FixedSnapshot {
                snapshot: SnapshotRef::new(snapshot.clone())?,
            },
            Self::ExpectedRevision { revision } => Consistency::ExpectedRevision {
                revision: ProjectRevision(*revision),
            },
            Self::Unconditional => Consistency::Unconditional,
        })
    }
}

/// One typed script call — everything needed to lower into a wire
/// request except caller authority, which scripts cannot express.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationCall {
    /// Operation arguments (schema-validated; `SynvelisI64` decimal
    /// strings are decoded where the schema says `Integer`).
    pub args: Value,
    /// Explicit target — required iff the capability's
    /// [`TargetContract`] requires one.
    #[serde(default)]
    pub target: Option<ScriptTarget>,
    /// Domain-appropriate consistency requirement.
    pub consistency: ScriptConsistency,
    /// Tagged project id (`"project-<32hex>"`).
    pub project: String,
    /// Caller-assigned correlation id.
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

/// The wire-shaped invocation document produced by assembly — the
/// `OperationRequest` precursor that still goes through
/// `Gateway::bind` → authorization → schema re-validation → approval.
///
/// Carries no `CallerIdentity`, `AdapterSeal`, grant, approval, or
/// permission field — such keys cannot be represented here, and
/// [`InvocationDraft::verify_wire`] runs the same
/// [`WireRequest::parse`] authority/implicit-target rejection the real
/// wire boundary applies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvocationDraft {
    /// Caller-assigned correlation id.
    pub request_id: String,
    /// Exact capability ref.
    pub capability: CapabilityRef,
    /// Declared input schema ref (the wire `schema` field).
    pub schema: SchemaRef,
    /// Tagged project id.
    pub project: String,
    /// Explicit target — present iff the contract requires/allows one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Value>,
    /// Lowered, schema-validated arguments.
    pub args: Value,
    /// Serialized wire consistency object.
    pub consistency: Value,
    /// Optional opaque task reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// Optional opaque grant reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant: Option<String>,
    /// Optional idempotency key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

impl InvocationDraft {
    /// The draft as the exact JSON object the wire boundary consumes.
    #[must_use]
    pub fn to_wire_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// Runs the authoritative [`WireRequest::parse`] over the draft —
    /// proves wire compatibility and re-applies the authority /
    /// implicit-target key rejection.
    ///
    /// # Errors
    /// [`GatewayError`] — anything the wire boundary rejects.
    pub fn verify_wire(&self) -> Result<WireRequest, GatewayError> {
        WireRequest::parse(&self.to_wire_value())
    }

    /// Submits the draft through the authoritative Gateway invocation
    /// path — `Gateway::handle` runs `WireRequest::parse` →
    /// [`synvelis_gateway::Gateway::bind`] → authorization →
    /// [`synvelis_gateway::Gateway::invoke`] unchanged.
    ///
    /// `caller` is supplied by the trusted host (adapter/CLI); scripts
    /// cannot express or mint one — the draft type has no field that
    /// could carry it. Authorization, schema re-validation, approval,
    /// and receipt semantics are entirely the Gateway's.
    ///
    /// # Errors
    /// [`synvelis_gateway::ErrorEnvelope`] — any pipeline stage's
    /// typed failure (wire rejection, unavailable capability, denied,
    /// schema violation, executor error).
    #[allow(clippy::result_large_err)] // typed envelope is the contract
    pub fn submit(
        &self,
        gateway: &mut synvelis_gateway::Gateway,
        caller: synvelis_gateway::CallerIdentity,
    ) -> Result<synvelis_gateway::GatewayOutcome<Value>, synvelis_gateway::ErrorEnvelope> {
        gateway.handle(&self.to_wire_value(), caller)
    }
}

/// The resolved, schema-validated pieces of one script call — shared
/// by [`ScriptApiModel::assemble_call`] (single draft) and
/// [`crate::action::ActionRecipe::build`] (typed recipe members).
pub(crate) struct ResolvedCall {
    /// Exact capability pin.
    pub capability: CapabilityRef,
    /// Operation domain (batch eligibility / consistency legality).
    pub domain: OperationDomain,
    /// Declared input schema ref — the wire `schema` field.
    pub input_schema: SchemaRef,
    /// Wire-shaped explicit target (contract-fixed `creates` injected).
    pub target: Option<Value>,
    /// Lowered, schema-validated arguments.
    pub args: Value,
    /// Serialized wire consistency object.
    pub consistency_json: Value,
}

impl ScriptApiModel {
    /// Lowers a typed [`InvocationCall`] for `name` into an
    /// [`InvocationDraft`].
    ///
    /// Fail-closed order: name must exist → entry must be invocable
    /// (public + `Invocable` verdict) → target must satisfy the
    /// capability's [`TargetContract`] → consistency must be legal for
    /// the domain → arguments must decode + validate against the input
    /// schema. Every failure is a typed [`ScriptApiError`]; Gateway
    /// wire-layer failures are preserved verbatim.
    ///
    /// # Errors
    /// [`ScriptApiError::UnknownApi`] — name not in the model;
    /// [`ScriptApiError::NotInvocable`] — hidden/disabled/planned/
    /// retired/unsupported/unresolved/invalid-name;
    /// [`ScriptApiError::Gateway`] — target/consistency/argument
    /// rejection;
    /// [`ScriptApiError::Internal`] — model invariant violation.
    pub fn assemble_call(
        &self,
        name: &str,
        call: InvocationCall,
    ) -> Result<InvocationDraft, ScriptApiError> {
        // request_id: caller-assigned correlation id (length-bound).
        RequestId::new(call.request_id.clone())?;

        let resolved = self.resolve_call(
            name,
            &call.args,
            call.target.as_ref(),
            &call.project,
            &call.consistency,
        )?;

        Ok(InvocationDraft {
            request_id: call.request_id,
            capability: resolved.capability,
            schema: resolved.input_schema,
            project: call.project,
            target: resolved.target,
            args: resolved.args,
            consistency: resolved.consistency_json,
            task: call.task,
            grant: call.grant,
            idempotency_key: call.idempotency_key,
        })
    }

    /// Resolves and validates one typed call's semantic content —
    /// name → verdict → project id → target contract → consistency
    /// legality → argument schema — without assembling request-level
    /// fields (request id / task / grant / idempotency key). Shared by
    /// [`Self::assemble_call`] and [`crate::action::ActionRecipe`].
    ///
    /// # Errors
    /// Same fail-closed vocabulary as [`Self::assemble_call`].
    pub(crate) fn resolve_call(
        &self,
        name: &str,
        args: &Value,
        target: Option<&ScriptTarget>,
        project: &str,
        consistency: &ScriptConsistency,
    ) -> Result<ResolvedCall, ScriptApiError> {
        let entry = self
            .describe(name)
            .ok_or_else(|| ScriptApiError::UnknownApi {
                name: name.to_owned(),
            })?;

        if let Some(blocker) = InvocableBlocker::of(entry.visibility, &entry.verdict) {
            return Err(ScriptApiError::NotInvocable {
                name: name.to_owned(),
                blocker,
            });
        }

        let descriptor = entry
            .descriptor
            .as_ref()
            .ok_or_else(|| ScriptApiError::Internal {
                reason: format!("invocable entry {name:?} lacks a descriptor"),
            })?;
        let input_binding = entry
            .input
            .as_ref()
            .ok_or_else(|| ScriptApiError::Internal {
                reason: format!("invocable entry {name:?} lacks an input schema"),
            })?;

        // project: tagged stable id of kind `project`.
        ResourceRef::expect(ResourceKind::Project, project)?;

        // target: contract shape → wire JSON.
        let target_wire = lower_target(descriptor.target, target)?;

        // consistency: domain legality via the authoritative rule.
        let consistency_gateway = consistency.to_gateway()?;
        if !consistency_gateway.allowed_in(descriptor.domain) {
            return Err(GatewayError::ConsistencyNotAllowed {
                domain: descriptor.domain,
                consistency: consistency_gateway.kind(),
            }
            .into());
        }
        let consistency_json =
            serde_json::to_value(&consistency_gateway).map_err(|e| ScriptApiError::Json {
                reason: e.to_string(),
            })?;

        // args: i64-string decode → authoritative ValueSchema validation.
        let lowered = lower_value(&input_binding.resolved, args, "$")?;
        input_binding
            .resolved
            .to_value_schema()
            .validate("$", &lowered)?;

        Ok(ResolvedCall {
            capability: entry.requested.clone(),
            domain: descriptor.domain,
            input_schema: input_binding.schema.clone(),
            target: target_wire,
            args: lowered,
            consistency_json,
        })
    }
}

/// Validates `target` against the capability's [`TargetContract`] and
/// returns the wire-shaped target JSON (or `None` for `None` contract).
fn lower_target(
    contract: TargetContract,
    target: Option<&ScriptTarget>,
) -> Result<Option<Value>, ScriptApiError> {
    let shape_err =
        |expected: &'static str, actual: &ScriptTarget| GatewayError::TargetShapeMismatch {
            expected,
            actual: actual.shape(),
        };

    match (contract, target) {
        (TargetContract::None, None) => Ok(None),
        (TargetContract::None, Some(_)) => Err(GatewayError::UnexpectedTarget.into()),
        (
            TargetContract::Resource { .. }
            | TargetContract::ResourceSet { .. }
            | TargetContract::Create { .. },
            None,
        ) => Err(GatewayError::TargetRequired.into()),
        (TargetContract::Resource { kind }, Some(t)) => match t {
            ScriptTarget::Resource { kind: actual, id } => {
                if *actual != kind {
                    return Err(GatewayError::WrongResourceKind {
                        expected: kind,
                        actual: *actual,
                    }
                    .into());
                }
                ResourceRef::expect(kind, id)?;
                Ok(Some(serde_json::to_value(t).map_err(|e| {
                    ScriptApiError::Json {
                        reason: e.to_string(),
                    }
                })?))
            }
            other => Err(shape_err("resource", other).into()),
        },
        (TargetContract::ResourceSet { kind }, Some(t)) => match t {
            ScriptTarget::ResourceSet { kind: actual, ids } => {
                if *actual != kind {
                    return Err(GatewayError::WrongResourceKind {
                        expected: kind,
                        actual: *actual,
                    }
                    .into());
                }
                if ids.is_empty() {
                    return Err(GatewayError::SchemaViolation {
                        path: "$.target.ids".to_owned(),
                        reason: "resource-set target requires at least one id".to_owned(),
                    }
                    .into());
                }
                for id in ids {
                    ResourceRef::expect(kind, id)?;
                }
                Ok(Some(serde_json::to_value(t).map_err(|e| {
                    ScriptApiError::Json {
                        reason: e.to_string(),
                    }
                })?))
            }
            other => Err(shape_err("resource-set", other).into()),
        },
        (TargetContract::Create { parent, creates }, Some(t)) => match t {
            ScriptTarget::Create {
                parent_kind,
                parent: parent_id,
            } => {
                if *parent_kind != parent {
                    return Err(GatewayError::WrongResourceKind {
                        expected: parent,
                        actual: *parent_kind,
                    }
                    .into());
                }
                ResourceRef::expect(parent, parent_id)?;
                // The wire `create` target also carries `creates`; the
                // contract fixes it, so inject the declared kind.
                let mut value = serde_json::to_value(t).map_err(|e| ScriptApiError::Json {
                    reason: e.to_string(),
                })?;
                if let Some(obj) = value.as_object_mut() {
                    obj.insert(
                        "creates".to_owned(),
                        serde_json::to_value(creates).map_err(|e| ScriptApiError::Json {
                            reason: e.to_string(),
                        })?,
                    );
                }
                Ok(Some(value))
            }
            other => Err(shape_err("create", other).into()),
        },
    }
}

/// Schema-directed lowering: decodes `SynvelisI64` decimal strings to
/// JSON integers where the schema says `Integer`, then leaves every
/// other node untouched for [`ValueSchema::validate`] (via
/// [`ScriptSchema::to_value_schema`]) to judge. Never coerces types the
/// schema does not declare.
fn lower_value(schema: &ScriptSchema, value: &Value, path: &str) -> Result<Value, ScriptApiError> {
    match schema {
        ScriptSchema::Integer { min, max } if value.is_string() => {
            decode_i64_string(value.as_str().unwrap_or_default(), path, *min, *max)
        }
        ScriptSchema::Optional(inner) => {
            if value.is_null() {
                Ok(Value::Null)
            } else {
                lower_value(inner, value, path)
            }
        }
        ScriptSchema::Array { item, .. } => {
            if let Value::Array(items) = value {
                let mut out = Vec::with_capacity(items.len());
                for (i, item_value) in items.iter().enumerate() {
                    out.push(lower_value(item, item_value, &format!("{path}[{i}]"))?);
                }
                Ok(Value::Array(out))
            } else {
                Ok(value.clone())
            }
        }
        ScriptSchema::Object { fields, .. } => {
            if let Value::Object(map) = value {
                let mut out = serde_json::Map::new();
                for (key, field_value) in map {
                    let lowered = match fields.get(key) {
                        Some(field) => {
                            lower_value(&field.schema, field_value, &format!("{path}.{key}"))?
                        }
                        None => field_value.clone(),
                    };
                    out.insert(key.clone(), lowered);
                }
                Ok(Value::Object(out))
            } else {
                Ok(value.clone())
            }
        }
        ScriptSchema::TimeRange { .. } => {
            // `{domain, start, len}` — start/len are i64 on the wire;
            // decode SynvelisI64 strings, leave everything else for
            // validate_time_range's typed checks (WrongTimeDomain etc.).
            if let Value::Object(map) = value {
                let mut out = map.clone();
                for key in ["start", "len"] {
                    if let Some(s) = out.get(key).and_then(Value::as_str) {
                        let decoded = decode_i64_string(s, &format!("{path}.{key}"), None, None)?;
                        out.insert(key.to_owned(), decoded);
                    }
                }
                Ok(Value::Object(out))
            } else {
                Ok(value.clone())
            }
        }
        _ => Ok(value.clone()),
    }
}

/// Strict decimal-i64 decode for the `SynvelisI64` encoding. Rejects
/// non-integer text, overflow, and out-of-bounds values with typed
/// [`GatewayError::SchemaViolation`]s — never rounds.
fn decode_i64_string(
    text: &str,
    path: &str,
    min: Option<i64>,
    max: Option<i64>,
) -> Result<Value, ScriptApiError> {
    let parsed: i64 = text.parse().map_err(|_| GatewayError::SchemaViolation {
        path: path.to_owned(),
        reason: format!("expected an integer, got {text:?}"),
    })?;
    if let Some(min) = min
        && parsed < min
    {
        return Err(GatewayError::SchemaViolation {
            path: path.to_owned(),
            reason: format!("below minimum {min}"),
        }
        .into());
    }
    if let Some(max) = max
        && parsed > max
    {
        return Err(GatewayError::SchemaViolation {
            path: path.to_owned(),
            reason: format!("above maximum {max}"),
        }
        .into());
    }
    Ok(Value::from(parsed))
}
