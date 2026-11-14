//! Workflow Script definitions, call specs, and the lowering builder
//! (SV-SCRIPT-03 / synvelis#273).
//!
//! A *workflow script* is one typed implementation of a
//! [`synvelis_automation::WorkflowDefinition`] — the script-side
//! contract from `docs/automation/design-script.md`. This module owns
//! three plain-data artifacts:
//!
//! - [`WorkflowScriptDefinition`] — the versioned, digest-pinned
//!   identity of the script source (language / backend / source
//!   digest), parallel to [`crate::action::ScriptActionDefinition`]
//!   but without schema refs: a workflow declares its own typed
//!   inputs/outputs inside the produced definition.
//! - [`ScriptWorkflowSpec`] — the evaluation record a script backend
//!   produces: an ordered tree of `action()` / `workflow()` /
//!   `if` / `for_each` / `await_job` / `atomic_macro` / `emit` calls.
//!   Expression positions carry the same ergonomic `$`-reference
//!   surface the YAML frontend accepts; they are lowered by the
//!   automation importer, never evaluated here.
//! - [`WorkflowScriptBuilder`] — lowers a spec into the canonical
//!   *import document* JSON the automation importer consumes. `action()`
//!   names resolve through [`ScriptApiModel`]/[`BindingSet`] to exact
//!   `id@major.minor` capability pins and the matching workflow step
//!   kind; nothing is invoked, scheduled, or authorized — calls become
//!   step references only.
//!
//! Boundary: this module never executes script source, never holds
//! VM state, and never mints authority. Real Luau/Rhai evaluation is
//! the backend's job (ruviform#559 integration gate); the builder
//! consumes the typed spec that evaluation yields.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use synvelis_gateway::{ExecutionContract, OperationDomain};

use crate::action::{AvailableBackends, BackendRequirement, LanguageId, validate_script_id};
use crate::binding::BindingSet;
use crate::canon::digest_of;
use crate::error::{InvocableBlocker, ScriptApiError};
use crate::model::ScriptApiModel;

/// Workflow-script definition document format version.
pub const WORKFLOW_SCRIPT_DEFINITION_SCHEMA_VERSION: u32 = 1;
/// Workflow spec document format version.
pub const WORKFLOW_SPEC_SCHEMA_VERSION: u32 = 1;
/// Maximum call nodes one spec may carry — bounds builder work; the
/// automation importer additionally bounds the expanded step count.
pub const MAX_WORKFLOW_NODES: usize = 16_384;

/// The versioned, digest-pinned contract of one workflow script —
/// `docs/automation/design-script.md` §workflow script.
///
/// Authority-free by construction: identity, language/backend
/// requirements, and a source digest only — no grants, approvals,
/// principals, executors, or VM state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowScriptDefinition {
    /// Must equal [`WORKFLOW_SCRIPT_DEFINITION_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Stable script id (`[a-z0-9._-]`, ≤ 64).
    pub script_id: String,
    /// Script version — must agree with the pinned source.
    pub version: u32,
    /// Script language (`yaml` / `luau` / `rhai`).
    pub language_id: LanguageId,
    /// Required evaluation backend.
    pub backend_requirement: BackendRequirement,
    /// SHA-256 (lowercase hex) of the canonical script source.
    pub source_digest: String,
}

impl WorkflowScriptDefinition {
    /// Structural + environment validation: schema version, id/digest
    /// shapes, language↔backend consistency, backend availability.
    ///
    /// # Errors
    /// [`ScriptApiError::UnsupportedDefinitionVersion`] /
    /// [`ScriptApiError::Definition`] — bad fields;
    /// [`ScriptApiError::LanguageBackendMismatch`] /
    /// [`ScriptApiError::BackendUnavailable`] — environment.
    pub fn validate(&self, backends: &AvailableBackends) -> Result<(), ScriptApiError> {
        if self.schema_version != WORKFLOW_SCRIPT_DEFINITION_SCHEMA_VERSION {
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
        Ok(())
    }

    /// Canonical digest of the definition — the drift anchor carried
    /// into evidence and reference pins.
    ///
    /// # Errors
    /// [`ScriptApiError::Json`] — canonicalization failure.
    pub fn digest(&self) -> Result<String, ScriptApiError> {
        digest_of(self)
    }

    /// Verifies `actual_source_digest` against the declared
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

/// A versioned, digest-pinned reference to another workflow — the
/// heterogeneous `workflow()` call target (Luau→Rhai→YAML and friends).
///
/// The pin names *what* to resolve: language, semantic version, and
/// the SHA-256 of the referenced source bytes. `path` is the canonical
/// relative location under the host's import root (empty for
/// in-memory/embedded sources). Resolution happens entirely at
/// compile time — no runtime import, no VM-to-VM call.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptWorkflowRef {
    /// Referenced workflow's language.
    pub language_id: LanguageId,
    /// Referenced workflow's semantic version.
    pub version: u32,
    /// SHA-256 (lowercase hex) of the referenced source bytes.
    pub source_digest: String,
    /// Canonical path relative to the import root (`""` = embedded).
    pub path: String,
}

fn empty_object() -> Value {
    json!({})
}

/// One script-side call inside a [`ScriptWorkflowSpec`] — the
/// evaluation record of `action()` / `workflow()` / control calls.
///
/// This is a *source* representation, not a plan IR: the automation
/// importer lowers it into the single [`synvelis_automation::Step`]
/// AST. Expression positions (`args`, `target`, `condition`,
/// `items`, `job`, `value`) carry the ergonomic `$`-reference surface
/// verbatim — the importer rewrites them into tagged exprs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "call", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ScriptStepCall {
    /// `action(name, args?, target?)` — a capability call recorded as
    /// a step reference. `name` resolves through the script API model;
    /// the capability's domain/execution contract selects the step
    /// kind (`query` / `invoke` / `async-submit`).
    Action {
        /// Step id — unique across the whole spec.
        id: String,
        /// Script API name.
        name: String,
        /// Arguments in the ergonomic expression surface.
        #[serde(default = "empty_object")]
        args: Value,
        /// Explicit target expression, when supplied.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<Value>,
    },
    /// `workflow(ref, args)` — a heterogeneous workflow reference.
    /// `args` maps the referenced workflow's declared input names to
    /// argument expressions; the call's declared outputs become
    /// referenceable as `$steps.<id>.<output>`.
    Workflow {
        /// Call-site id — occupies the step-id namespace and names the
        /// virtual output object.
        id: String,
        /// The pinned reference.
        reference: ScriptWorkflowRef,
        /// Typed inputs by declared name.
        #[serde(default)]
        args: Map<String, Value>,
    },
    /// `if condition then … else … end` — lowered to the AST `if`
    /// step; the condition uses the restricted condition surface.
    If {
        /// Step id.
        id: String,
        /// Condition in the ergonomic surface (`{op: …}` or `$`-refs).
        condition: Value,
        /// Then-branch calls.
        then: Vec<ScriptStepCall>,
        /// Else-branch calls.
        #[serde(default, rename = "else", skip_serializing_if = "Vec::is_empty")]
        else_steps: Vec<ScriptStepCall>,
    },
    /// `for_each items do … end` — lowered to `for-each-bounded`.
    ForEach {
        /// Step id.
        id: String,
        /// Items expression (must be array-typed).
        items: Value,
        /// Hard iteration bound (≥ 1).
        max_iterations: u64,
        /// Body calls.
        #[serde(default)]
        body: Vec<ScriptStepCall>,
    },
    /// `await_job(job, output_schema?)` — waits on a job-typed value.
    AwaitJob {
        /// Step id.
        id: String,
        /// Job-handle expression.
        job: Value,
        /// Optional `schema@major.minor` typing the job result.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output_schema: Option<String>,
    },
    /// `atomic_macro(mac, args?, target?)` — references an atomic
    /// macro by `id@major.minor` (AUTX-04 registry resolves it).
    AtomicMacro {
        /// Step id.
        id: String,
        /// `macro@major.minor`.
        mac: String,
        /// Macro arguments.
        #[serde(default = "empty_object")]
        args: Value,
        /// Explicit target expression.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<Value>,
    },
    /// `emit(output, value)` — binds a declared workflow output.
    Emit {
        /// Step id.
        id: String,
        /// Declared output name.
        output: String,
        /// Emitted value expression.
        value: Value,
    },
}

/// A declared workflow input/output inside a spec — `name` plus the
/// canonical [`synvelis_automation::SchemaNode`] JSON (opaque to the
/// script side; the definition decoder validates it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecIoDecl {
    /// Input/output name.
    pub name: String,
    /// Canonical schema JSON.
    pub schema: Value,
}

/// The evaluation record a workflow-script backend produces — the
/// typed port output [`WorkflowScriptBuilder`] lowers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptWorkflowSpec {
    /// Must equal [`WORKFLOW_SPEC_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Workflow identifier (`[a-z0-9._-]`, ≤ 64).
    pub workflow_id: String,
    /// The workflow's semantic version — the pin heterogeneous
    /// references check.
    pub version: u32,
    /// Declared inputs.
    #[serde(default)]
    pub inputs: Vec<SpecIoDecl>,
    /// Ordered call tree.
    #[serde(default)]
    pub steps: Vec<ScriptStepCall>,
    /// Declared outputs.
    #[serde(default)]
    pub outputs: Vec<SpecIoDecl>,
    /// Canonical [`synvelis_automation::WorkflowLimits`] JSON.
    pub limits: Value,
}

/// Lowers [`ScriptWorkflowSpec`]s into canonical import documents.
///
/// Stateless — all inputs are explicit. The output is a
/// `serde_json::Value` in the `WorkflowDefinition` wire shape whose
/// `steps` may additionally contain `kind: "workflow"` nodes; the
/// automation importer expands those and produces the single
/// [`synvelis_automation::WorkflowDefinition`].
pub struct WorkflowScriptBuilder;

impl WorkflowScriptBuilder {
    /// Builds the import document for `spec` under `definition`.
    ///
    /// `model`/`bindings` resolve `action()` names to exact capability
    /// pins — the same drift-checked contract SV-SCRIPT-01/02 uses.
    ///
    /// # Errors
    /// [`ScriptApiError::UnsupportedDefinitionVersion`] — bad spec or
    /// definition version;
    /// [`ScriptApiError::BackendUnavailable`] /
    /// [`ScriptApiError::LanguageBackendMismatch`] — environment;
    /// [`ScriptApiError::StaleArtifact`] — binding/model drift;
    /// [`ScriptApiError::UnknownApi`] / [`ScriptApiError::NotInvocable`]
    /// — unresolvable action names;
    /// [`ScriptApiError::UnsupportedWorkflowStep`] — a capability whose
    /// domain/execution has no workflow step kind;
    /// [`ScriptApiError::DuplicateStepId`] — id reuse.
    pub fn build(
        definition: &WorkflowScriptDefinition,
        spec: &ScriptWorkflowSpec,
        backends: &AvailableBackends,
        model: &ScriptApiModel,
        bindings: &BindingSet,
    ) -> Result<Value, ScriptApiError> {
        definition.validate(backends)?;
        bindings.verify_against(model)?;
        if spec.schema_version != WORKFLOW_SPEC_SCHEMA_VERSION {
            return Err(ScriptApiError::UnsupportedDefinitionVersion {
                found: spec.schema_version,
            });
        }
        let mut ids = BTreeSet::new();
        let mut nodes = 0usize;
        check_calls(&spec.steps, &mut ids, &mut nodes)?;
        let steps = spec
            .steps
            .iter()
            .map(|call| Self::call_to_step(call, model, bindings))
            .collect::<Result<Vec<_>, _>>()?;
        let io = |decls: &[SpecIoDecl]| -> Vec<Value> {
            decls
                .iter()
                .map(|d| json!({"name": d.name, "schema": d.schema}))
                .collect()
        };
        Ok(json!({
            "schema_version": 1,
            "workflow_id": spec.workflow_id,
            "version": spec.version,
            "inputs": io(&spec.inputs),
            "outputs": io(&spec.outputs),
            "limits": spec.limits,
            "steps": steps,
        }))
    }

    /// Converts one call node into its import-document step object.
    fn call_to_step(
        call: &ScriptStepCall,
        model: &ScriptApiModel,
        bindings: &BindingSet,
    ) -> Result<Value, ScriptApiError> {
        Ok(match call {
            ScriptStepCall::Action {
                id,
                name,
                args,
                target,
            } => {
                let (kind, capability) = Self::resolve_action(name, model, bindings)?;
                let mut step = json!({
                    "kind": kind,
                    "id": id,
                    "capability": capability,
                    "args": args,
                });
                if let Some(target) = target {
                    step["target"] = target.clone();
                }
                step
            }
            ScriptStepCall::Workflow {
                id,
                reference,
                args,
            } => json!({
                "kind": "workflow",
                "id": id,
                "reference": reference,
                "args": Value::Object(args.clone()),
            }),
            ScriptStepCall::If {
                id,
                condition,
                then,
                else_steps,
            } => Self::if_step(id, condition, then, else_steps, model, bindings)?,
            ScriptStepCall::ForEach {
                id,
                items,
                max_iterations,
                body,
            } => Self::for_each_step(id, items, *max_iterations, body, model, bindings)?,
            ScriptStepCall::AwaitJob {
                id,
                job,
                output_schema,
            } => {
                let mut step = json!({
                    "kind": "await-job",
                    "id": id,
                    "job": job,
                });
                if let Some(schema) = output_schema {
                    step["output_schema"] = json!(schema);
                }
                step
            }
            ScriptStepCall::AtomicMacro {
                id,
                mac,
                args,
                target,
            } => {
                let mut step = json!({
                    "kind": "atomic-macro",
                    "id": id,
                    "mac": mac,
                    "args": args,
                });
                if let Some(target) = target {
                    step["target"] = target.clone();
                }
                step
            }
            ScriptStepCall::Emit { id, output, value } => json!({
                "kind": "emit-result",
                "id": id,
                "output": output,
                "value": value,
            }),
        })
    }

    /// Lowers an `if` call node — branches recurse through
    /// [`Self::call_to_step`].
    fn if_step(
        id: &str,
        condition: &Value,
        then: &[ScriptStepCall],
        else_steps: &[ScriptStepCall],
        model: &ScriptApiModel,
        bindings: &BindingSet,
    ) -> Result<Value, ScriptApiError> {
        let lowered = |calls: &[ScriptStepCall]| {
            calls
                .iter()
                .map(|c| Self::call_to_step(c, model, bindings))
                .collect::<Result<Vec<_>, _>>()
        };
        let mut step = json!({
            "kind": "if",
            "id": id,
            "condition": condition,
            "then": lowered(then)?,
        });
        if !else_steps.is_empty() {
            step["else"] = json!(lowered(else_steps)?);
        }
        Ok(step)
    }

    /// Lowers a bounded `for_each` call node.
    fn for_each_step(
        id: &str,
        items: &Value,
        max_iterations: u64,
        body: &[ScriptStepCall],
        model: &ScriptApiModel,
        bindings: &BindingSet,
    ) -> Result<Value, ScriptApiError> {
        let body_steps = body
            .iter()
            .map(|c| Self::call_to_step(c, model, bindings))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(json!({
            "kind": "for-each-bounded",
            "id": id,
            "items": items,
            "max_iterations": max_iterations,
            "body": body_steps,
        }))
    }

    /// Resolves an `action()` name to `(step kind, capability ref)`.
    ///
    /// The step kind derives from the capability's operation domain and
    /// execution contract — the single authoritative mapping; scripts
    /// cannot name a kind that disagrees with the contract.
    fn resolve_action(
        name: &str,
        model: &ScriptApiModel,
        bindings: &BindingSet,
    ) -> Result<(&'static str, String), ScriptApiError> {
        let entry = model
            .describe(name)
            .ok_or_else(|| ScriptApiError::UnknownApi {
                name: name.to_owned(),
            })?;
        if !entry.is_invocable() {
            let blocker = InvocableBlocker::of(entry.visibility, &entry.verdict)
                .unwrap_or(InvocableBlocker::Hidden);
            return Err(ScriptApiError::NotInvocable {
                name: name.to_owned(),
                blocker,
            });
        }
        let binding = bindings.get(name).ok_or_else(|| ScriptApiError::Internal {
            reason: format!("invocable entry {name:?} lacks a binding"),
        })?;
        let kind = match (binding.domain, binding.execution) {
            (OperationDomain::Query, ExecutionContract::Sync) => "query",
            (OperationDomain::ProjectMutation, ExecutionContract::Sync) => "invoke",
            (OperationDomain::AsyncOperation, ExecutionContract::AsyncAccept) => "async-submit",
            (domain, execution) => {
                return Err(ScriptApiError::UnsupportedWorkflowStep {
                    member: name.to_owned(),
                    domain,
                    execution,
                });
            }
        };
        let capability = format!(
            "{}@{}.{}",
            binding.capability.id.as_str(),
            binding.capability.version.major,
            binding.capability.version.minor
        );
        Ok((kind, capability))
    }
}

/// Recursive spec walk: duplicate step ids and the node budget are
/// rejected before any lowering — workflow-call ids occupy the same
/// namespace as real step ids.
fn check_calls(
    calls: &[ScriptStepCall],
    ids: &mut BTreeSet<String>,
    nodes: &mut usize,
) -> Result<(), ScriptApiError> {
    for call in calls {
        *nodes += 1;
        if *nodes > MAX_WORKFLOW_NODES {
            return Err(ScriptApiError::Definition {
                reason: format!("too many call nodes: > {MAX_WORKFLOW_NODES}"),
            });
        }
        let id = match call {
            ScriptStepCall::Action { id, .. }
            | ScriptStepCall::Workflow { id, .. }
            | ScriptStepCall::If { id, .. }
            | ScriptStepCall::ForEach { id, .. }
            | ScriptStepCall::AwaitJob { id, .. }
            | ScriptStepCall::AtomicMacro { id, .. }
            | ScriptStepCall::Emit { id, .. } => id,
        };
        if !ids.insert(id.clone()) {
            return Err(ScriptApiError::DuplicateStepId { step: id.clone() });
        }
        match call {
            ScriptStepCall::If {
                then, else_steps, ..
            } => {
                check_calls(then, ids, nodes)?;
                check_calls(else_steps, ids, nodes)?;
            }
            ScriptStepCall::ForEach { body, .. } => check_calls(body, ids, nodes)?,
            _ => {}
        }
    }
    Ok(())
}
