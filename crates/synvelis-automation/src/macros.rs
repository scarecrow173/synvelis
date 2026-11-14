//! Atomic Macro v1 — versioned Recipe definitions and the
//! Recipe → `synvelis.operation.batch` compiler (AUTX-04 /
//! synvelis#263, canonical contract `docs/automation/design-atomic-macro.md`).
//!
//! An [`AtomicMacroDefinition`] is a *static, authority-free* recipe:
//! schema-versioned identity, typed parameters, an ordered list of
//! [`MacroStep`]s (each an exact-versioned `ProjectMutation`
//! capability with an explicit target and schema-checked args), named
//! [`OutputBinding`]s for create-then-reference, and optional effect /
//! data-access policy ceilings. Definitions cannot express a
//! principal, grant, approval state, secret, or implicit target —
//! those types simply do not exist here.
//!
//! [`MacroCompiler::compile`] lowers Definition + runtime inputs +
//! pinned project revision into the flat `ops[]` member list of one
//! `synvelis.operation.batch` operation (depth 1, ≤ 64 members). The
//! resulting batch flows through the *existing* GW-04 prepare →
//! GW-05 approval → GW-06 commit/undo path as one
//! `ProjectOp::Composite` — this crate never prepares, authorizes, or
//! commits anything itself.
//!
//! Create-then-reference: a step that creates a resource later steps
//! must reference binds it through a named [`OutputBinding`]. At
//! compile time the reference lowers to a *placeholder* tagged id in
//! the reserved [`OUTPUT_BINDING_ID_BASE`] range — never a guessed or
//! hard-coded real id. GW-04's prepare resolves placeholders against
//! the deterministic scratch apply on the pinned base revision, so
//! preview and commit derive the same ids from the same watermark;
//! any revision drift fails the commit CAS and requires re-prepare.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::canon::digest_of;
use crate::compiler::PlanWarning;
use crate::definition::{InputDecl, MAX_DECLARED_INPUTS, MAX_NAME_LEN, def_err, validate_name};
use crate::error::{AutomationError, CompileError};
use crate::expr::Expr;
use crate::refs::{Version, VersionedRef};
use crate::registry::{
    Availability, CapabilityDescriptor, DataAccessClass, DataAccessSet, EffectKind, EffectSet,
    ExecutionContract, OperationDomain, RegistrySnapshot, TargetContract,
};
use crate::schema::{
    FieldType, ResourceKind, SchemaNode, StaticType, type_compatible, validate_tagged_id,
    validate_value,
};

/// The only `schema_version` macro definitions accept.
pub const MACRO_SCHEMA_VERSION: u32 = 1;
/// The only `schema_version` compiled macro artifacts carry.
pub const COMPILED_MACRO_SCHEMA_VERSION: u32 = 1;
/// Maximum members one macro may expand to — mirrors the batch
/// operation's member bound (`PrepareBudget::max_ops`).
pub const MAX_MACRO_MEMBERS: usize = 64;
/// Maximum declared parameters (mirrors `MAX_DECLARED_INPUTS`).
pub const MAX_MACRO_PARAMETERS: usize = MAX_DECLARED_INPUTS;
/// Maximum declared output bindings.
pub const MAX_MACRO_BINDINGS: usize = MAX_MACRO_MEMBERS;
/// The `synvelis.operation.batch` capability id — a macro member can
/// never name it (nested batches are rejected at compile *and* again
/// by GW-04's existing `nested batch is not allowed` rule).
pub const BATCH_CAPABILITY_ID: &str = "synvelis.operation.batch";

/// First raw id value of the output-binding placeholder range.
///
/// Tagged ids are `<kind>-<32 lowercase hex>` over a `u128` raw.
/// Issuable ids are always `< WELL_KNOWN_BASE` (`1 << 96`); the
/// well-known block occupies `WELL_KNOWN_BASE + 1..`. Placeholder ids
/// live at `OUTPUT_BINDING_ID_BASE + slot` for `slot in 0..64`, far
/// above every well-known id and unreachable by the project id
/// allocator — a placeholder can therefore never collide with, or be
/// mistaken for, a real resource id. The gateway currently rejects
/// non-empty `bindings` outright; this constant becomes the wire
/// contract the GW-04 scratch-ID seam will mirror when it lands.
pub const OUTPUT_BINDING_ID_BASE: u128 = (1u128 << 96) + (1u128 << 32);

/// A versioned atomic macro definition — the shared compiler input.
///
/// Wire `version` is the macro's major version: references resolve as
/// `macro_id@<version>.0` through [`VersionedRef`]'s exact
/// `(id, major, minor)` matching.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AtomicMacroDefinition {
    /// Must equal [`MACRO_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Macro identifier (`[a-z0-9._-]`, ≤ 64).
    pub macro_id: String,
    /// The macro's major version (referenced as `id@version.0`).
    pub version: u32,
    /// Declared parameters — bound at compile time from runtime
    /// inputs, referenced inside steps as `$param.<name>`.
    #[serde(default)]
    pub parameters: Vec<InputDecl>,
    /// Ordered member steps — each lowers to one batch `ops[]` entry.
    #[serde(default)]
    pub steps: Vec<MacroStep>,
    /// Named create-then-reference bindings (`$output.<name>`).
    #[serde(default)]
    pub output_bindings: Vec<OutputBinding>,
    /// Optional effect / data-access ceilings over the member union.
    #[serde(default)]
    pub policy: MacroPolicy,
}

/// One named member of a macro — the `NamedAction` of the contract.
///
/// Carries an exact `capability@major.minor` reference, an explicit
/// target binding (required iff the capability's target contract
/// demands one; forbidden otherwise — implicit targets do not exist),
/// and args checked against the capability's input schema. Args may
/// contain literals plus `$param.`/`$output.` references only.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacroStep {
    /// Step id — unique across the definition; output bindings name
    /// their producing step by this id.
    pub id: String,
    /// `capability@major.minor` — must resolve to an available,
    /// synchronous `ProjectMutation` capability.
    pub capability: VersionedRef,
    /// Arguments — an [`Expr`] object evaluated against the capability
    /// input schema.
    #[serde(default)]
    pub args: Expr,
    /// Explicit target binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Expr>,
}

/// A named output binding: `name` resolves to the single resource of
/// `kind` created by the step `step`.
///
/// Later steps reference it as `$output.<name>`; at compile time the
/// reference becomes a placeholder tagged id that GW-04 resolves
/// during prepare's scratch apply. A binding whose step creates zero
/// or several resources of `kind` is a typed prepare rejection — ids
/// are never guessed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputBinding {
    /// The binding name — referenced as `$output.<name>`.
    pub name: String,
    /// The producing step's id.
    pub step: String,
    /// The resource kind the step must create exactly one of.
    pub kind: ResourceKind,
}

/// Optional macro-level ceilings over the union of member
/// effect/data-access bounds — the macro-local analogue of
/// [`crate::WorkflowLimits`]' `allowed_*` fields.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacroPolicy {
    /// When present, every member effect kind must be in this set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_effects: Option<Vec<EffectKind>>,
    /// When present, every member data-access class must be in this
    /// set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_data_access: Option<Vec<DataAccessClass>>,
}

impl AtomicMacroDefinition {
    /// The exact [`VersionedRef`] this definition registers under:
    /// `macro_id@<version>.0`.
    #[must_use]
    pub fn macro_ref(&self) -> VersionedRef {
        VersionedRef::new(self.macro_id.clone(), self.version, 0)
    }

    /// Source-agnostic structural validation: schema version,
    /// identifier shapes, member/binding counts, unique names, and
    /// binding→step referential integrity. Capability/schema/expression
    /// checks live in [`MacroCompiler`].
    ///
    /// # Errors
    /// [`AutomationError::UnsupportedDefinitionVersion`] /
    /// [`AutomationError::Definition`] on the first violation.
    pub fn validate(&self) -> Result<(), AutomationError> {
        if self.schema_version != MACRO_SCHEMA_VERSION {
            return Err(AutomationError::UnsupportedDefinitionVersion {
                found: self.schema_version,
            });
        }
        validate_macro_id(&self.macro_id)?;
        if self.parameters.len() > MAX_MACRO_PARAMETERS {
            return Err(def_err(format!(
                "too many parameters: {} > {MAX_MACRO_PARAMETERS}",
                self.parameters.len()
            )));
        }
        if self.steps.is_empty() {
            return Err(def_err("macro requires at least one step".to_owned()));
        }
        if self.steps.len() > MAX_MACRO_MEMBERS {
            return Err(def_err(format!(
                "too many members: {} > {MAX_MACRO_MEMBERS}",
                self.steps.len()
            )));
        }
        if self.output_bindings.len() > MAX_MACRO_BINDINGS {
            return Err(def_err(format!(
                "too many output bindings: {} > {MAX_MACRO_BINDINGS}",
                self.output_bindings.len()
            )));
        }
        let mut param_names = BTreeSet::new();
        for param in &self.parameters {
            validate_name(&param.name, "parameter")?;
            if !param_names.insert(param.name.as_str()) {
                return Err(def_err(format!("duplicate parameter name: {}", param.name)));
            }
        }
        let mut step_ids = BTreeSet::new();
        for step in &self.steps {
            validate_name(&step.id, "step")?;
            if !step_ids.insert(step.id.as_str()) {
                return Err(def_err(format!("duplicate step id: {}", step.id)));
            }
        }
        let mut binding_names = BTreeSet::new();
        for binding in &self.output_bindings {
            validate_name(&binding.name, "output binding")?;
            if !binding_names.insert(binding.name.as_str()) {
                return Err(def_err(format!(
                    "duplicate output binding name: {}",
                    binding.name
                )));
            }
            if !step_ids.contains(binding.step.as_str()) {
                return Err(def_err(format!(
                    "output binding '{}' references unknown step '{}'",
                    binding.name, binding.step
                )));
            }
            if binding.kind == ResourceKind::Project {
                return Err(def_err(format!(
                    "output binding '{}' cannot target the project aggregate",
                    binding.name
                )));
            }
        }
        Ok(())
    }
}

/// `[a-z0-9._-]`, 1..=64, first char lowercase — the same identifier
/// shape as workflow ids, reported under the macro field name.
fn validate_macro_id(id: &str) -> Result<(), AutomationError> {
    let valid = !id.is_empty()
        && id.len() <= MAX_NAME_LEN
        && id.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if !valid {
        return Err(def_err(format!(
            "invalid macro_id '{id}' (want [a-z0-9._-], ≤{MAX_NAME_LEN})"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Static check — shared by MacroRegistry (plan-time signatures) and
// MacroCompiler (recipe compile).
// ---------------------------------------------------------------------------

/// Where a declared [`OutputBinding`] resolves: the resource kind the
/// producing step must create exactly one of.
#[derive(Clone, Copy, Debug)]
struct OutputSlot {
    /// The bound resource kind.
    kind: ResourceKind,
}

/// Expression/type environment for one macro definition.
struct MacroCheck<'a> {
    /// Declared parameter name → static type.
    param_types: &'a BTreeMap<String, StaticType>,
    /// Declared binding name → producing slot.
    outputs: &'a BTreeMap<String, OutputSlot>,
    /// Binding names whose producer step precedes the current one.
    available: BTreeSet<String>,
    /// Parameters actually referenced (unused-param warnings).
    referenced_params: BTreeSet<String>,
}

impl<'a> MacroCheck<'a> {
    fn new(
        param_types: &'a BTreeMap<String, StaticType>,
        outputs: &'a BTreeMap<String, OutputSlot>,
    ) -> Self {
        Self {
            param_types,
            outputs,
            available: BTreeSet::new(),
            referenced_params: BTreeSet::new(),
        }
    }

    /// Static type of a macro expression — mirrors
    /// `PlanCompiler`'s `type_of` with the macro-local environment:
    /// `$param.` resolves against declared parameters, `$output.`
    /// against bindings produced by earlier steps, and workflow-only
    /// references (`$input.`/`$steps.`/`$item.`) are rejected.
    fn type_of(&mut self, expr: &Expr, at: &str) -> Result<StaticType, AutomationError> {
        Ok(match expr {
            Expr::Literal { value } => crate::compiler::literal_type(value, at)?,
            Expr::Param { path } => {
                let [name] = path.as_slice() else {
                    return Err(CompileError::TypeMismatch {
                        path: at.to_owned(),
                        expected: "bare $param.<name> reference".to_owned(),
                        actual: format!("$param.{}", path.join(".")),
                    }
                    .into());
                };
                self.referenced_params.insert(name.clone());
                self.param_types
                    .get(name)
                    .cloned()
                    .ok_or_else(|| CompileError::UnknownInput { name: name.clone() })?
            }
            Expr::Output { name } => {
                if !self.available.contains(name) {
                    if self.outputs.contains_key(name) {
                        return Err(CompileError::ForwardMacroOutput {
                            at: at.to_owned(),
                            name: name.clone(),
                        }
                        .into());
                    }
                    return Err(CompileError::UnknownMacroOutput {
                        at: at.to_owned(),
                        name: name.clone(),
                    }
                    .into());
                }
                StaticType::Id(self.outputs[name].kind)
            }
            Expr::Input { .. } | Expr::StepOutput { .. } | Expr::Item { .. } => {
                return Err(CompileError::WorkflowOnlyExpr {
                    at: at.to_owned(),
                    expr: match expr {
                        Expr::Input { .. } => "input",
                        Expr::StepOutput { .. } => "steps",
                        _ => "item",
                    },
                }
                .into());
            }
            Expr::Array { items } => {
                let mut elem: Option<StaticType> = None;
                for item in items {
                    let ty = self.type_of(item, at)?;
                    elem = Some(match elem {
                        None => ty,
                        Some(prev) => crate::compiler::join_types(&prev, &ty, at)?,
                    });
                }
                StaticType::Array(Box::new(elem.unwrap_or(StaticType::Json)))
            }
            Expr::Object { fields } => self.type_of_object(fields, at)?,
        })
    }

    /// Static type of an `Expr::Object` — each field's type is
    /// computed recursively and all fields are required.
    fn type_of_object(
        &mut self,
        fields: &BTreeMap<String, Expr>,
        at: &str,
    ) -> Result<StaticType, AutomationError> {
        Ok(StaticType::Object(
            fields
                .iter()
                .map(|(name, e)| {
                    Ok((
                        name.clone(),
                        FieldType {
                            ty: self.type_of(e, at)?,
                            required: true,
                        },
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, AutomationError>>()?,
        ))
    }

    /// Checks `expr` against a structural [`SchemaNode`] — the same
    /// rules as the workflow compiler's `check_expr_schema`.
    fn check_expr_schema(
        &mut self,
        expr: &Expr,
        schema: &SchemaNode,
        path: &str,
    ) -> Result<(), AutomationError> {
        if matches!(expr, Expr::Array { .. } | Expr::Object { .. })
            && let SchemaNode::Optional { inner } = schema
        {
            return self.check_expr_schema(expr, inner, path);
        }
        match expr {
            Expr::Literal { value } => {
                validate_value(schema, path, value)?;
                Ok(())
            }
            Expr::Array { items } => {
                let SchemaNode::Array { item, .. } = schema else {
                    return Err(type_mismatch(path, schema, "array"));
                };
                for (i, e) in items.iter().enumerate() {
                    self.check_expr_schema(e, item, &format!("{path}[{i}]"))?;
                }
                Ok(())
            }
            Expr::Object { fields } => {
                let SchemaNode::Object {
                    fields: sfields,
                    deny_unknown,
                } = schema
                else {
                    return Err(type_mismatch(path, schema, "object"));
                };
                for (name, fexpr) in fields {
                    let child = format!("{path}.{name}");
                    match sfields.get(name) {
                        Some(f) => self.check_expr_schema(fexpr, &f.schema, &child)?,
                        None if *deny_unknown => {
                            return Err(CompileError::SchemaViolation {
                                path: child,
                                reason: "unknown field".to_owned(),
                            }
                            .into());
                        }
                        None => {
                            self.type_of(fexpr, &child)?;
                        }
                    }
                }
                for (name, f) in sfields {
                    if f.required && !fields.contains_key(name) {
                        return Err(CompileError::SchemaViolation {
                            path: format!("{path}.{name}"),
                            reason: "missing required field".to_owned(),
                        }
                        .into());
                    }
                }
                Ok(())
            }
            _ => {
                let ty = self.type_of(expr, path)?;
                let expected = StaticType::from_schema(schema);
                if type_compatible(&ty, &expected) {
                    Ok(())
                } else {
                    Err(CompileError::TypeMismatch {
                        path: path.to_owned(),
                        expected: expected.describe(),
                        actual: ty.describe(),
                    }
                    .into())
                }
            }
        }
    }

    /// Checks a step's optional target expr against the capability's
    /// declared [`TargetContract`] — identical rules to the workflow
    /// compiler's `check_target`.
    fn check_target(
        &mut self,
        step_id: &str,
        target: Option<&Expr>,
        contract: &TargetContract,
    ) -> Result<(), AutomationError> {
        match contract {
            TargetContract::None => {
                if target.is_some() {
                    return Err(CompileError::UnexpectedTarget {
                        step: step_id.to_owned(),
                    }
                    .into());
                }
            }
            TargetContract::Resource { kind } => {
                let expr = target.ok_or_else(|| CompileError::MissingTarget {
                    step: step_id.to_owned(),
                })?;
                self.check_target_expr(step_id, expr, *kind, false)?;
            }
            TargetContract::ResourceSet { kind } => {
                let expr = target.ok_or_else(|| CompileError::MissingTarget {
                    step: step_id.to_owned(),
                })?;
                self.check_target_expr(step_id, expr, *kind, true)?;
            }
            TargetContract::Create { parent, .. } => {
                let expr = target.ok_or_else(|| CompileError::MissingTarget {
                    step: step_id.to_owned(),
                })?;
                self.check_target_expr(step_id, expr, *parent, false)?;
            }
        }
        Ok(())
    }

    fn check_target_expr(
        &mut self,
        step_id: &str,
        expr: &Expr,
        kind: ResourceKind,
        expect_set: bool,
    ) -> Result<(), AutomationError> {
        if let Expr::Literal { value } = expr {
            return crate::compiler::check_literal_target(step_id, value, kind, expect_set);
        }
        let ty = self.type_of(expr, "target")?;
        let expected = if expect_set {
            StaticType::Array(Box::new(StaticType::Id(kind)))
        } else {
            StaticType::Id(kind)
        };
        if type_compatible(&ty, &expected) {
            return Ok(());
        }
        Err(CompileError::TargetKindMismatch {
            step: step_id.to_owned(),
            expected: if expect_set {
                format!("array[id:{kind}]")
            } else {
                format!("id:{kind}")
            },
            actual: ty.describe(),
        }
        .into())
    }
}

fn type_mismatch(path: &str, schema: &SchemaNode, actual: &str) -> AutomationError {
    CompileError::TypeMismatch {
        path: path.to_owned(),
        expected: format!("{} schema", schema_kind_name(schema)),
        actual: actual.to_owned(),
    }
    .into()
}

fn schema_kind_name(schema: &SchemaNode) -> &'static str {
    match schema {
        SchemaNode::Bool => "bool",
        SchemaNode::Integer { .. } => "integer",
        SchemaNode::Number => "number",
        SchemaNode::String { .. } => "string",
        SchemaNode::Id { .. } => "id",
        SchemaNode::Enum { .. } => "enum",
        SchemaNode::Array { .. } => "array",
        SchemaNode::Object { .. } => "object",
        SchemaNode::Optional { .. } => "optional",
        SchemaNode::TimeRange { .. } => "time-range",
        SchemaNode::Job => "job",
    }
}

/// One statically verified macro member.
#[derive(Clone, Debug)]
struct CheckedMember {
    /// Index into `AtomicMacroDefinition::steps`.
    index: usize,
    /// Exact capability reference.
    capability: VersionedRef,
    /// Resolved input schema.
    input_schema: SchemaNode,
    /// Declared target contract.
    target: TargetContract,
    /// Capability effect upper bound.
    effects: EffectSet,
    /// Capability data-access upper bound.
    data_access: DataAccessSet,
}

/// The static-check product shared by registry signatures and recipe
/// compilation.
struct CheckedStatic {
    /// Verified members in step order.
    members: Vec<CheckedMember>,
    /// Parameter name → static type.
    param_types: BTreeMap<String, StaticType>,
    /// Parameters referenced by at least one step (unused → warning).
    referenced_params: BTreeSet<String>,
}

/// Runs the full static pass over `definition` against `registry`:
/// definition validation is the caller's job; this resolves every
/// capability exactly, enforces `ProjectMutation` + `Sync` +
/// `Available`, schema-checks args/targets, enforces backward-only
/// output references, and applies `policy` ceilings.
fn check_static(
    definition: &AtomicMacroDefinition,
    registry: &RegistrySnapshot,
) -> Result<CheckedStatic, AutomationError> {
    let param_types: BTreeMap<String, StaticType> = definition
        .parameters
        .iter()
        .map(|p| (p.name.clone(), StaticType::from_schema(&p.schema)))
        .collect();
    let outputs: BTreeMap<String, OutputSlot> = definition
        .output_bindings
        .iter()
        .map(|b| (b.name.clone(), OutputSlot { kind: b.kind }))
        .collect();
    let mut env = MacroCheck::new(&param_types, &outputs);
    let mut members = Vec::with_capacity(definition.steps.len());
    let mut union_effects = EffectSet::new();
    let mut union_access = DataAccessSet::new();
    for (index, step) in definition.steps.iter().enumerate() {
        let member = check_member(index, step, registry, &mut env)?;
        union_effects.union_with(&member.effects);
        union_access.union_with(&member.data_access);
        members.push(member);
        // Bindings produced by this step become referenceable by all
        // later steps — inserted *after* the step's own checks so a
        // step can never reference its own (or a later) output.
        for binding in &definition.output_bindings {
            if binding.step == step.id {
                env.available.insert(binding.name.clone());
            }
        }
    }
    check_policy_ceilings(definition, &union_effects, &union_access)?;
    let referenced_params = std::mem::take(&mut env.referenced_params);
    Ok(CheckedStatic {
        members,
        param_types,
        referenced_params,
    })
}

/// Validates one macro step against the registry snapshot.
fn check_member(
    index: usize,
    step: &MacroStep,
    registry: &RegistrySnapshot,
    env: &mut MacroCheck<'_>,
) -> Result<CheckedMember, AutomationError> {
    if step.capability.id == BATCH_CAPABILITY_ID {
        return Err(CompileError::NestedBatch {
            step: step.id.clone(),
        }
        .into());
    }
    let desc: &CapabilityDescriptor = registry.resolve_capability(&step.capability)?;
    if desc.availability != Availability::Available {
        return Err(CompileError::CapabilityUnavailable {
            capability: step.capability.to_string(),
            state: desc.availability.name().to_owned(),
        }
        .into());
    }
    if desc.domain != OperationDomain::ProjectMutation {
        return Err(CompileError::DomainMismatch {
            step: step.id.clone(),
            capability: step.capability.to_string(),
            domain: desc.domain.name().to_owned(),
            step_kind: "macro-member".to_owned(),
        }
        .into());
    }
    if desc.execution != ExecutionContract::Sync {
        return Err(CompileError::ExecutionMismatch {
            step: step.id.clone(),
            capability: step.capability.to_string(),
            execution: desc.execution.name().to_owned(),
            step_kind: "macro-member".to_owned(),
        }
        .into());
    }
    let in_schema = registry.resolve_schema(&desc.input_schema)?;
    env.check_expr_schema(&step.args, in_schema, "args")?;
    env.check_target(&step.id, step.target.as_ref(), &desc.target)?;
    Ok(CheckedMember {
        index,
        capability: step.capability.clone(),
        input_schema: in_schema.clone(),
        target: desc.target.clone(),
        effects: desc.effect_bound.clone(),
        data_access: desc.data_access.clone(),
    })
}

/// Applies `policy.allowed_*` ceilings over the member union.
fn check_policy_ceilings(
    definition: &AtomicMacroDefinition,
    union_effects: &EffectSet,
    union_access: &DataAccessSet,
) -> Result<(), AutomationError> {
    if let Some(allowed) = &definition.policy.allowed_effects {
        let allowed: BTreeSet<EffectKind> = allowed.iter().copied().collect();
        for kind in union_effects.iter() {
            if !allowed.contains(&kind) {
                return Err(CompileError::BoundCeilingExceeded {
                    kind: "effect",
                    item: kind.name().to_owned(),
                }
                .into());
            }
        }
    }
    if let Some(allowed) = &definition.policy.allowed_data_access {
        let allowed: BTreeSet<_> = allowed.iter().copied().collect();
        for class in union_access.iter() {
            if !allowed.contains(&class) {
                return Err(CompileError::BoundCeilingExceeded {
                    kind: "data-access",
                    item: class.name().to_owned(),
                }
                .into());
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Compiled artifact — the batch ops[] + binding declarations + digest.
// ---------------------------------------------------------------------------

/// One output-binding declaration in compiled wire form. `member` is
/// the 0-based index into `ops[]` of the producing member; `kind` is
/// the resource kind GW-04 requires that member to create exactly
/// one of. `name` is carried for traceability — GW-04 keys resolution
/// on `(member, kind)` positionally.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MacroBindingWire {
    /// The binding name from the definition.
    pub name: String,
    /// Producing member index inside `ops[]`.
    pub member: usize,
    /// Required created-resource kind.
    pub kind: ResourceKind,
}

/// The deterministic product of [`MacroCompiler::compile`]: the flat
/// `synvelis.operation.batch` member list, the binding declarations
/// GW-04 resolves, and content digests. Contains no authority — the
/// same definition + inputs + registry snapshot always produces the
/// same bytes.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompiledMacro {
    /// Must equal [`COMPILED_MACRO_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// `macro_id@<version>.0` of the source definition.
    pub macro_ref: String,
    /// Canonical digest of the source [`AtomicMacroDefinition`].
    pub definition_digest: String,
    /// The pinned project revision the caller asserted — echoed so the
    /// batch request's `expected-revision` consistency binds the same
    /// base the recipe was compiled against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<u64>,
    /// Flat batch member triples `{capability, target?, args}` —
    /// depth 1, 1..=[`MAX_MACRO_MEMBERS`] entries, `ProjectMutation`
    /// capabilities only.
    pub ops: Vec<Value>,
    /// Output-binding declarations aligned with `ops[]` member indices.
    pub bindings: Vec<MacroBindingWire>,
    /// Deterministic compile warnings (e.g. unused parameters).
    pub warnings: Vec<PlanWarning>,
}

impl CompiledMacro {
    /// Deterministic digest of the canonical compiled artifact.
    ///
    /// # Errors
    /// [`AutomationError::Json`] if the artifact cannot be represented.
    pub fn digest(&self) -> Result<String, AutomationError> {
        let value = serde_json::to_value(self).map_err(|e| AutomationError::Json {
            reason: e.to_string(),
        })?;
        digest_of(&value)
    }

    /// The `args` object for a `synvelis.operation.batch` request
    /// carrying this compilation: `ops` plus the `bindings`
    /// declarations GW-04 consumes for output-binding resolution.
    ///
    /// `bindings` is omitted when empty — an absent or empty
    /// declaration carries no create-then-reference, which is the
    /// only form the gateway currently accepts (non-empty bindings
    /// answer with the typed unsupported verdict until the GW-04
    /// scratch-ID seam lands; see
    /// `docs/automation/design-atomic-macro.md` §3).
    ///
    /// # Errors
    /// [`AutomationError::Json`] if the artifact cannot be represented.
    pub fn batch_args(&self) -> Result<Value, AutomationError> {
        let mut args = serde_json::Map::new();
        args.insert("ops".to_owned(), Value::Array(self.ops.clone()));
        if !self.bindings.is_empty() {
            args.insert(
                "bindings".to_owned(),
                serde_json::to_value(&self.bindings).map_err(|e| AutomationError::Json {
                    reason: e.to_string(),
                })?,
            );
        }
        Ok(Value::Object(args))
    }
}

/// Canonical digest of a macro definition — same policy as
/// [`crate::canon::definition_digest`]: canonical JSON → SHA-256 hex.
///
/// # Errors
/// [`AutomationError::Json`] on serialization failure.
pub fn macro_digest(definition: &AtomicMacroDefinition) -> Result<String, AutomationError> {
    let value = serde_json::to_value(definition).map_err(|e| AutomationError::Json {
        reason: e.to_string(),
    })?;
    digest_of(&value)
}

// ---------------------------------------------------------------------------
// Expression evaluation — concrete values with placeholder outputs.
// ---------------------------------------------------------------------------

/// Concrete evaluation environment: bound parameter values plus the
/// precomputed binding name → placeholder id map.
struct EvalEnv<'a> {
    /// Parameter name → provided (schema-validated) JSON value.
    param_values: &'a BTreeMap<String, Value>,
    /// Binding name → reserved-range placeholder tagged id (declaration
    /// order = compiled `bindings[]` slot order).
    placeholders: &'a BTreeMap<String, String>,
}

/// Evaluates `expr` to a concrete JSON value. `$param.` splices the
/// bound value; `$output.` emits the reserved-range placeholder id
/// GW-04 resolves at prepare time. Workflow-only references cannot
/// appear (the static pass rejects them first) but are rejected here
/// too — fail-closed at both layers.
fn eval_expr(expr: &Expr, env: &EvalEnv<'_>, at: &str) -> Result<Value, AutomationError> {
    Ok(match expr {
        Expr::Literal { value } => {
            check_no_reserved_id(value, at)?;
            value.clone()
        }
        Expr::Param { path } => {
            let [name] = path.as_slice() else {
                return Err(CompileError::TypeMismatch {
                    path: at.to_owned(),
                    expected: "bare $param.<name> reference".to_owned(),
                    actual: format!("$param.{}", path.join(".")),
                }
                .into());
            };
            env.param_values
                .get(name)
                .cloned()
                .ok_or_else(|| CompileError::UnknownInput { name: name.clone() })?
        }
        Expr::Output { name } => {
            let Some(placeholder) = env.placeholders.get(name) else {
                return Err(CompileError::UnknownMacroOutput {
                    at: at.to_owned(),
                    name: name.clone(),
                }
                .into());
            };
            Value::String(placeholder.clone())
        }
        Expr::Input { .. } | Expr::StepOutput { .. } | Expr::Item { .. } => {
            return Err(CompileError::WorkflowOnlyExpr {
                at: at.to_owned(),
                expr: match expr {
                    Expr::Input { .. } => "input",
                    Expr::StepOutput { .. } => "steps",
                    _ => "item",
                },
            }
            .into());
        }
        Expr::Array { items } => Value::Array(
            items
                .iter()
                .enumerate()
                .map(|(i, e)| eval_expr(e, env, &format!("{at}[{i}]")))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Expr::Object { fields } => Value::Object(
            fields
                .iter()
                .map(|(name, e)| Ok((name.clone(), eval_expr(e, env, &format!("{at}.{name}"))?)))
                .collect::<Result<serde_json::Map<_, _>, AutomationError>>()?,
        ),
    })
}

/// Formats the placeholder tagged id for binding `slot` of `kind`.
fn placeholder_id(kind: ResourceKind, slot: usize) -> String {
    debug_assert!(slot < MAX_MACRO_BINDINGS);
    format!(
        "{}-{:032x}",
        kind.tag(),
        OUTPUT_BINDING_ID_BASE + slot as u128
    )
}

/// True when `s` is a tagged id whose raw falls inside the
/// output-binding placeholder range — reserved for compiler/GW-04
/// use, never valid as user data.
fn is_reserved_placeholder(s: &str) -> bool {
    let Some((_, hex)) = s.rsplit_once('-') else {
        return false;
    };
    if hex.len() != 32 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return false;
    }
    let Ok(raw) = u128::from_str_radix(hex, 16) else {
        return false;
    };
    (OUTPUT_BINDING_ID_BASE..OUTPUT_BINDING_ID_BASE + MAX_MACRO_BINDINGS as u128).contains(&raw)
}

/// Rejects literal values carrying a reserved-range id string —
/// user data can never legitimately occupy the placeholder range.
fn check_no_reserved_id(value: &Value, at: &str) -> Result<(), AutomationError> {
    match value {
        Value::String(s) if is_reserved_placeholder(s) => Err(CompileError::ReservedIdLiteral {
            at: at.to_owned(),
            id: s.clone(),
        }
        .into()),
        Value::Array(items) => {
            for (i, v) in items.iter().enumerate() {
                check_no_reserved_id(v, &format!("{at}[{i}]"))?;
            }
            Ok(())
        }
        Value::Object(map) => {
            for (k, v) in map {
                check_no_reserved_id(v, &format!("{at}.{k}"))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Member lowering — Expr → batch `ops[]` triple.
// ---------------------------------------------------------------------------

/// Lowers one checked step to its `{capability, target?, args}` wire
/// member.
fn eval_member(
    step: &MacroStep,
    member: &CheckedMember,
    param_values: &BTreeMap<String, Value>,
    placeholders: &BTreeMap<String, String>,
) -> Result<Value, AutomationError> {
    let at = format!("steps[{}]", step.id);
    let env = EvalEnv {
        param_values,
        placeholders,
    };
    let args_value = eval_expr(&step.args, &env, &format!("{at}.args"))?;
    if !args_value.is_object() {
        return Err(CompileError::TypeMismatch {
            path: format!("{at}.args"),
            expected: "object args".to_owned(),
            actual: "non-object evaluated args".to_owned(),
        }
        .into());
    }
    // Concrete validation against the resolved input schema — catches
    // param-value vs capability-schema mismatches the static pass
    // could not see (e.g. a param int outside the capability's range).
    validate_value(&member.input_schema, "args", &args_value)?;
    let mut out = serde_json::Map::new();
    out.insert(
        "capability".to_owned(),
        serde_json::json!({
            "id": member.capability.id,
            "version": {
                "major": member.capability.version.major,
                "minor": member.capability.version.minor,
            },
        }),
    );
    if let Some(target) = &step.target {
        let target_value = eval_target(target, &member.target, &env, &at)?;
        out.insert("target".to_owned(), target_value);
    }
    out.insert("args".to_owned(), args_value);
    Ok(Value::Object(out))
}

/// Evaluates a target `Expr` into the `WireTarget` JSON shape
/// (`{"shape": ...}`), validating concrete tagged ids against the
/// contract kind.
fn eval_target(
    expr: &Expr,
    contract: &TargetContract,
    env: &EvalEnv<'_>,
    at: &str,
) -> Result<Value, AutomationError> {
    let value = eval_expr(expr, env, at)?;
    match contract {
        TargetContract::None => Err(CompileError::UnexpectedTarget {
            step: at.to_owned(),
        }
        .into()),
        TargetContract::Resource { kind } => {
            let id = single_id(&value, *kind, at)?;
            Ok(serde_json::json!({
                "shape": "resource",
                "kind": kind.tag(),
                "id": id,
            }))
        }
        TargetContract::ResourceSet { kind } => {
            let ids = id_set(&value, *kind, at)?;
            Ok(serde_json::json!({
                "shape": "resource-set",
                "kind": kind.tag(),
                "ids": ids,
            }))
        }
        TargetContract::Create { parent, creates } => {
            let id = single_id(&value, *parent, at)?;
            Ok(serde_json::json!({
                "shape": "create",
                "parent_kind": parent.tag(),
                "parent": id,
                "creates": creates.tag(),
            }))
        }
    }
}

fn single_id(value: &Value, kind: ResourceKind, at: &str) -> Result<String, AutomationError> {
    let s = value
        .as_str()
        .ok_or_else(|| CompileError::TargetKindMismatch {
            step: at.to_owned(),
            expected: format!("id:{kind}"),
            actual: "non-string target".to_owned(),
        })?;
    validate_tagged_id(kind, s).map_err(|_| CompileError::TargetKindMismatch {
        step: at.to_owned(),
        expected: kind.tag().to_owned(),
        actual: s.to_owned(),
    })?;
    Ok(s.to_owned())
}

fn id_set(value: &Value, kind: ResourceKind, at: &str) -> Result<Vec<String>, AutomationError> {
    let arr = value
        .as_array()
        .ok_or_else(|| CompileError::TargetShapeMismatch {
            step: at.to_owned(),
            expected: "array of ids".to_owned(),
            actual: "non-array target".to_owned(),
        })?;
    if arr.is_empty() {
        return Err(CompileError::TargetShapeMismatch {
            step: at.to_owned(),
            expected: "non-empty id array".to_owned(),
            actual: "empty array".to_owned(),
        }
        .into());
    }
    arr.iter().map(|v| single_id(v, kind, at)).collect()
}

// ---------------------------------------------------------------------------
// MacroCompiler — Definition + inputs + registry → CompiledMacro.
// ---------------------------------------------------------------------------

/// The recipe compiler: statically verifies the definition (shared
/// with [`MacroRegistry`]) then lowers it with concrete runtime inputs
/// into batch `ops[]` + binding declarations.
#[derive(Clone, Copy, Debug, Default)]
pub struct MacroCompiler;

impl MacroCompiler {
    /// Compiles `definition` against runtime `inputs`, the capability
    /// `registry` snapshot, and the pinned `expected_revision` echoed
    /// into the artifact for the caller's `expected-revision`
    /// consistency binding.
    ///
    /// # Errors
    /// [`AutomationError`] — definition/registry validation failures,
    /// or any [`CompileError`] rejection (fail-closed, all-or-nothing:
    /// no partial `ops[]` is ever produced).
    pub fn compile(
        definition: &AtomicMacroDefinition,
        inputs: &serde_json::Map<String, Value>,
        registry: &RegistrySnapshot,
        expected_revision: Option<u64>,
    ) -> Result<CompiledMacro, AutomationError> {
        definition.validate()?;
        registry.validate()?;
        let checked = check_static(definition, registry)?;
        let param_values = bind_parameters(definition, inputs)?;
        // Binding name → placeholder id, in declaration order — the
        // same order the compiled `bindings[]` array carries, so slot
        // `i` here is `bindings[i].member`'s producer for GW-04.
        let placeholders: BTreeMap<String, String> = definition
            .output_bindings
            .iter()
            .enumerate()
            .map(|(slot, b)| (b.name.clone(), placeholder_id(b.kind, slot)))
            .collect();
        let mut ops = Vec::with_capacity(checked.members.len());
        for member in &checked.members {
            let step = &definition.steps[member.index];
            ops.push(eval_member(step, member, &param_values, &placeholders)?);
        }
        let step_index: BTreeMap<&str, usize> = definition
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id.as_str(), i))
            .collect();
        let bindings = definition
            .output_bindings
            .iter()
            .map(|b| MacroBindingWire {
                name: b.name.clone(),
                member: step_index[b.step.as_str()],
                kind: b.kind,
            })
            .collect();
        let mut warnings = Vec::new();
        for name in checked.param_types.keys() {
            if !checked.referenced_params.contains(name) {
                warnings.push(PlanWarning::UnusedInput { name: name.clone() });
            }
        }
        Ok(CompiledMacro {
            schema_version: COMPILED_MACRO_SCHEMA_VERSION,
            macro_ref: definition.macro_ref().to_string(),
            definition_digest: macro_digest(definition)?,
            expected_revision,
            ops,
            bindings,
            warnings,
        })
    }
}

/// Binds runtime inputs to declared parameters — exact-name matching
/// plus per-parameter schema validation, mirroring the workflow
/// compiler's input contract. Reserved-range id strings inside
/// provided values are rejected (they could collide with compiler
/// placeholders).
fn bind_parameters(
    definition: &AtomicMacroDefinition,
    inputs: &serde_json::Map<String, Value>,
) -> Result<BTreeMap<String, Value>, AutomationError> {
    let mut values = BTreeMap::new();
    for decl in &definition.parameters {
        let Some(value) = inputs.get(&decl.name) else {
            return Err(CompileError::MissingInput {
                name: decl.name.clone(),
            }
            .into());
        };
        if let Err(err) = validate_value(&decl.schema, &decl.name, value) {
            return Err(match err {
                CompileError::SchemaViolation { path, reason } => CompileError::InputTypeMismatch {
                    name: decl.name.clone(),
                    path,
                    reason,
                }
                .into(),
                other => other.into(),
            });
        }
        check_no_reserved_id(value, &decl.name)?;
        values.insert(decl.name.clone(), value.clone());
    }
    for name in inputs.keys() {
        if !definition.parameters.iter().any(|p| &p.name == name) {
            return Err(CompileError::UnexpectedInput { name: name.clone() }.into());
        }
    }
    Ok(values)
}

// ---------------------------------------------------------------------------
// MacroRegistry — exact-version macro signatures for PlanCompiler.
// ---------------------------------------------------------------------------

/// The plan-visible signature of one registered macro: parameter
/// types for call-site argument checking plus the per-member
/// capability/effect/data-access bounds the expanded plan accounts
/// for. Produced only by [`MacroRegistry::from_definitions`], which
/// runs the full static check — a registered macro is already
/// verified against the same registry snapshot the workflow compiles
/// against.
#[derive(Clone, Debug)]
pub struct MacroSignature {
    /// `macro_id@<version>.0`.
    pub mac: VersionedRef,
    /// Canonical digest of the source definition.
    pub definition_digest: String,
    /// Parameter name → static type (all required at call sites).
    pub parameters: BTreeMap<String, StaticType>,
    /// Verified members in step order.
    pub members: Vec<MacroMemberSig>,
}

/// One macro member's plan contribution.
#[derive(Clone, Debug)]
pub struct MacroMemberSig {
    /// Exact capability reference (`id@major.minor`).
    pub capability: VersionedRef,
    /// Capability effect upper bound.
    pub effects: EffectSet,
    /// Capability data-access upper bound.
    pub data_access: DataAccessSet,
}

/// The registry of statically verified macro definitions — the
/// [`crate::PlanCompiler`]'s view of `Step::AtomicMacro` references.
///
/// Resolution is exact `(id, version)` only — no "latest". An
/// unregistered id fails with [`CompileError::UndefinedMacro`]; a
/// known id at an unregistered version fails with
/// [`CompileError::UnsupportedMacroVersion`]. When no registry is
/// supplied at all, every macro reference remains
/// [`CompileError::UndefinedMacro`] (fail-closed, AUTX-05 behavior).
#[derive(Clone, Debug, Default)]
pub struct MacroRegistry {
    entries: BTreeMap<(String, Version), MacroSignature>,
}

impl MacroRegistry {
    /// The empty registry — every macro reference is undefined.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Number of registered macros (test/introspection).
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry holds no macros.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Statically verifies each definition against `registry` and
    /// indexes the resulting signatures by exact `(id, version)`.
    ///
    /// # Errors
    /// [`AutomationError`] — definition/registry validation or any
    /// static-check rejection; duplicate `(id, version)` registrations
    /// are a [`AutomationError::Registry`] failure.
    pub fn from_definitions<'a, I>(
        definitions: I,
        registry: &RegistrySnapshot,
    ) -> Result<Self, AutomationError>
    where
        I: IntoIterator<Item = &'a AtomicMacroDefinition>,
    {
        registry.validate()?;
        let mut entries = BTreeMap::new();
        for def in definitions {
            def.validate()?;
            let checked = check_static(def, registry)?;
            let mac = def.macro_ref();
            let key = (mac.id.clone(), mac.version);
            if entries.contains_key(&key) {
                return Err(AutomationError::Registry {
                    reason: format!("duplicate macro registration: {mac}"),
                });
            }
            entries.insert(
                key,
                MacroSignature {
                    mac,
                    definition_digest: macro_digest(def)?,
                    parameters: checked.param_types,
                    members: checked
                        .members
                        .iter()
                        .map(|m| MacroMemberSig {
                            capability: m.capability.clone(),
                            effects: m.effects.clone(),
                            data_access: m.data_access.clone(),
                        })
                        .collect(),
                },
            );
        }
        Ok(Self { entries })
    }

    /// Exact macro resolution — mirrors
    /// [`RegistrySnapshot::resolve_capability`]: unknown id and
    /// unsupported version are distinct failures.
    ///
    /// # Errors
    /// [`CompileError::UnknownMacro`] /
    /// [`CompileError::UnsupportedMacroVersion`]. Callers compiling
    /// *without* a registry keep the fail-closed
    /// [`CompileError::UndefinedMacro`] answer instead.
    pub fn resolve(&self, mac: &VersionedRef) -> Result<&MacroSignature, CompileError> {
        if !self.entries.keys().any(|(id, _)| id == &mac.id) {
            return Err(CompileError::UnknownMacro {
                mac: mac.to_string(),
            });
        }
        self.entries
            .get(&(mac.id.clone(), mac.version))
            .ok_or_else(|| CompileError::UnsupportedMacroVersion {
                id: mac.id.clone(),
                requested: mac.version.to_string(),
                supported: self
                    .entries
                    .keys()
                    .filter(|(id, _)| id == &mac.id)
                    .map(|(_, v)| v.to_string())
                    .collect(),
            })
    }
}
