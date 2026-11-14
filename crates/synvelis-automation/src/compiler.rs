//! [`PlanCompiler`] — validates a [`WorkflowDefinition`] against typed
//! inputs and a [`RegistrySnapshot`], producing an [`ExecutablePlan`].
//!
//! The compiler is **source-agnostic**: YAML, Luau, and Rhai frontends
//! all lower to the same [`WorkflowDefinition`], so every check here —
//! capability version/availability, schema compatibility, target
//! explicitness, effect bounds, static budgets — applies identically
//! regardless of where the definition came from.
//!
//! The plan carries *references and bounds only*: capability refs,
//! step order, effect/data-access upper bounds, and static budgets.
//! It contains no principal, grant, approval, or credential — runtime
//! authorization, revision, and job state are re-validated by
//! Gateway/Project/Job at execution time (synvelis#265+).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

use crate::canon::definition_digest;
use crate::definition::{Step, WorkflowDefinition};
use crate::error::{AutomationError, CompileError};
use crate::expr::{Condition, Expr};
use crate::macros::MacroRegistry;
use crate::refs::VersionedRef;
use crate::registry::{
    Availability, CapabilityDescriptor, DataAccessSet, EffectKind, EffectSet, ExecutionContract,
    OperationDomain, RegistrySnapshot, TargetContract,
};
use crate::schema::{
    FieldType, SchemaNode, StaticType, type_compatible, validate_tagged_id, validate_value,
};

/// The only `schema_version` emitted plans carry.
pub const PLAN_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Static ceilings — declared limits may not exceed these; the compiler
// additionally verifies computed worst cases fit the declared bounds.
// ---------------------------------------------------------------------------

/// Hard ceiling for declared `max_steps`.
pub const CEIL_MAX_STEPS: u64 = 1_000_000;
/// Hard ceiling for declared `max_depth`.
pub const CEIL_MAX_DEPTH: u64 = 64;
/// Hard ceiling for declared `max_iterations` (total loop-body
/// executions).
pub const CEIL_MAX_ITERATIONS: u64 = 100_000_000;
/// Hard ceiling for declared `max_runtime_ms` (24h).
pub const CEIL_MAX_RUNTIME_MS: u64 = 86_400_000;
/// Hard ceiling for declared `max_output_bytes` (64 MiB).
pub const CEIL_MAX_OUTPUT_BYTES: u64 = 67_108_864;
/// Hard ceiling for declared `max_memory_bytes` (1 GiB).
pub const CEIL_MAX_MEMORY_BYTES: u64 = 1_073_741_824;

/// A compiled, statically verified executable plan.
///
/// Contains references and bounds — never authority. Two compiles of
/// the same definition + inputs + registry snapshot produce identical
/// `step_order`, bounds, and [`Self::digest`].
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutablePlan {
    /// Must equal [`PLAN_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// [`crate::canon::definition_digest`] of the source definition.
    pub definition_digest: String,
    /// Sorted `id@major.minor` of every capability the plan may invoke.
    pub capability_refs_versions: Vec<String>,
    /// Deterministic pre-order step order: control-flow ids appear
    /// once, `if` branches list `then` before `else`, loop bodies list
    /// once (they repeat at runtime up to the bounded count).
    pub step_order: Vec<String>,
    /// Always `true` for v1 plans — implicit targets
    /// (selection/active-window/last-edited) cannot be expressed. The
    /// field exists so consumers fail closed if a future compiler
    /// ever emits implicit bindings.
    pub explicit_target_binding: bool,
    /// Per-capability union of possible effect kinds (`id@ver` →
    /// sorted kebab-case names). Branches contribute the *union* of
    /// both sides — never an under-approximation.
    pub effect_upper_bounds: BTreeMap<String, Vec<String>>,
    /// Per-capability union of possible data-access classes.
    pub data_access_bounds: BTreeMap<String, Vec<String>>,
    /// Enforced step-execution budget (declared, ≤ ceiling, ≥ worst
    /// case).
    pub max_steps: u64,
    /// Enforced control-depth budget.
    pub max_depth: u64,
    /// Enforced total loop-iteration budget.
    pub max_iterations: u64,
    /// Enforced runtime budget in milliseconds.
    pub max_runtime: u64,
    /// Enforced output-size budget in bytes.
    pub max_output_bytes: u64,
}

impl ExecutablePlan {
    /// Deterministic digest of the canonical plan encoding.
    ///
    /// # Errors
    /// [`AutomationError::Json`] if the plan cannot be represented.
    pub fn digest(&self) -> Result<String, AutomationError> {
        let value = serde_json::to_value(self).map_err(|e| AutomationError::Json {
            reason: e.to_string(),
        })?;
        crate::canon::digest_of(&value)
    }
}

/// A non-fatal compile observation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "warning", rename_all = "kebab-case")]
pub enum PlanWarning {
    /// A declared input is never referenced by any expression.
    UnusedInput {
        /// The input name.
        name: String,
    },
    /// An `await-job` step declared no `output_schema`, so its result
    /// is opaque JSON — downstream checks rely on runtime
    /// re-validation.
    OpaqueJobResult {
        /// The step id.
        step: String,
    },
}

/// The compiler output: the plan plus deterministic warnings.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanOutput {
    /// The verified plan.
    pub plan: ExecutablePlan,
    /// Deterministic warnings (sorted by construction order).
    pub warnings: Vec<PlanWarning>,
}

/// The stateless plan compiler — all state lives in [`Check`].
#[derive(Clone, Copy, Debug, Default)]
pub struct PlanCompiler;

impl PlanCompiler {
    /// Compiles `definition` against provided `inputs` and `registry`.
    ///
    /// Equivalent to [`Self::compile_with_macros`] with an empty macro
    /// registry — every `atomic-macro` step reference fails closed
    /// with [`CompileError::UndefinedMacro`].
    ///
    /// # Errors
    /// [`AutomationError`] — definition/registry validation failures,
    /// or any [`CompileError`] rejection (fail-closed).
    pub fn compile(
        definition: &WorkflowDefinition,
        inputs: &serde_json::Map<String, Value>,
        registry: &RegistrySnapshot,
    ) -> Result<PlanOutput, AutomationError> {
        Self::compile_with_macros(definition, inputs, registry, &MacroRegistry::empty())
    }

    /// Compiles `definition` against provided `inputs`, `registry`,
    /// and a [`MacroRegistry`] of statically verified atomic macro
    /// definitions (AUTX-04).
    ///
    /// An `atomic-macro` step resolves its `mac` reference exactly —
    /// unknown ids fail [`CompileError::UndefinedMacro`], unregistered
    /// versions fail [`CompileError::UnsupportedMacroVersion`]. A
    /// resolved macro contributes its member count to the step-cost
    /// budget and its members' capability refs and effect/data-access
    /// bounds to the plan's conservative upper bounds; the plan never
    /// carries the expanded `ops[]` (recipe compilation is
    /// [`crate::MacroCompiler`]'s job at execution-prep time).
    ///
    /// # Errors
    /// [`AutomationError`] — definition/registry validation failures,
    /// or any [`CompileError`] rejection (fail-closed).
    pub fn compile_with_macros(
        definition: &WorkflowDefinition,
        inputs: &serde_json::Map<String, Value>,
        registry: &RegistrySnapshot,
        macros: &MacroRegistry,
    ) -> Result<PlanOutput, AutomationError> {
        definition.validate()?;
        registry.validate()?;
        let mut check = Check::new(definition, registry, macros);
        check.check_inputs(inputs)?;
        check.check_limits_ceiling()?;
        let cost = check.process_steps(&definition.steps, true)?;
        check.check_budgets(cost)?;
        check.check_output_emits()?;
        check.check_effect_ceilings()?;
        check.into_output()
    }
}

// ---------------------------------------------------------------------------
// Internal checker
// ---------------------------------------------------------------------------

/// Worst-case cost of one pass over a step list, in u128 to make
/// overflow impossible before comparison.
#[derive(Clone, Copy, Debug, Default)]
struct Cost {
    /// Worst-case step executions.
    steps: u128,
    /// Worst-case total loop-body executions.
    iters: u128,
    /// Maximum control-flow nesting depth.
    depth: u64,
}

impl Cost {
    fn leaf() -> Self {
        Self {
            steps: 1,
            iters: 0,
            depth: 0,
        }
    }

    /// Sequence combination: steps/iters sum, depth maxes.
    fn seq(self, other: Self) -> Self {
        Self {
            steps: self.steps.saturating_add(other.steps),
            iters: self.iters.saturating_add(other.iters),
            depth: self.depth.max(other.depth),
        }
    }
}

struct Check<'a> {
    def: &'a WorkflowDefinition,
    registry: &'a RegistrySnapshot,
    macros: &'a MacroRegistry,
    input_types: BTreeMap<String, StaticType>,
    output_schemas: BTreeMap<String, SchemaNode>,
    all_ids: BTreeSet<String>,
    /// Scope stack: innermost last. `None` = step defined but produces
    /// no referenceable output (`if`/`for-each-bounded`).
    scopes: Vec<BTreeMap<String, Option<StaticType>>>,
    /// `$item` types, innermost last.
    item_types: Vec<StaticType>,
    referenced_inputs: BTreeSet<String>,
    warnings: Vec<PlanWarning>,
    cap_refs: BTreeSet<String>,
    effect_bounds: BTreeMap<String, EffectSet>,
    access_bounds: BTreeMap<String, DataAccessSet>,
    step_order: Vec<String>,
    /// output name → emitting step id (top level only).
    emits: BTreeMap<String, String>,
}

impl<'a> Check<'a> {
    fn new(
        def: &'a WorkflowDefinition,
        registry: &'a RegistrySnapshot,
        macros: &'a MacroRegistry,
    ) -> Self {
        let input_types = def
            .inputs
            .iter()
            .map(|i| (i.name.clone(), StaticType::from_schema(&i.schema)))
            .collect();
        let output_schemas = def
            .outputs
            .iter()
            .map(|o| (o.name.clone(), o.schema.clone()))
            .collect();
        let mut all_ids = BTreeSet::new();
        collect_ids(&def.steps, &mut all_ids);
        Self {
            def,
            registry,
            macros,
            input_types,
            output_schemas,
            all_ids,
            scopes: vec![BTreeMap::new()],
            item_types: Vec::new(),
            referenced_inputs: BTreeSet::new(),
            warnings: Vec::new(),
            cap_refs: BTreeSet::new(),
            effect_bounds: BTreeMap::new(),
            access_bounds: BTreeMap::new(),
            step_order: Vec::new(),
            emits: BTreeMap::new(),
        }
    }

    // -- inputs ---------------------------------------------------------

    fn check_inputs(
        &mut self,
        inputs: &serde_json::Map<String, Value>,
    ) -> Result<(), AutomationError> {
        for decl in &self.def.inputs {
            let Some(value) = inputs.get(&decl.name) else {
                return Err(CompileError::MissingInput {
                    name: decl.name.clone(),
                }
                .into());
            };
            if let Err(err) = validate_value(&decl.schema, &decl.name, value) {
                return Err(match err {
                    CompileError::SchemaViolation { path, reason } => {
                        CompileError::InputTypeMismatch {
                            name: decl.name.clone(),
                            path,
                            reason,
                        }
                        .into()
                    }
                    other => other.into(),
                });
            }
        }
        for name in inputs.keys() {
            if !self.def.inputs.iter().any(|i| &i.name == name) {
                return Err(CompileError::UnexpectedInput { name: name.clone() }.into());
            }
        }
        Ok(())
    }

    // -- budgets ---------------------------------------------------------

    fn check_limits_ceiling(&self) -> Result<(), AutomationError> {
        let l = &self.def.limits;
        ceiling("steps", l.max_steps, CEIL_MAX_STEPS)?;
        ceiling("depth", l.max_depth, CEIL_MAX_DEPTH)?;
        ceiling("iterations", l.max_iterations, CEIL_MAX_ITERATIONS)?;
        ceiling("runtime", l.max_runtime_ms, CEIL_MAX_RUNTIME_MS)?;
        ceiling("output-bytes", l.max_output_bytes, CEIL_MAX_OUTPUT_BYTES)?;
        ceiling("memory-bytes", l.max_memory_bytes, CEIL_MAX_MEMORY_BYTES)?;
        Ok(())
    }

    fn check_budgets(&self, cost: Cost) -> Result<(), AutomationError> {
        let l = &self.def.limits;
        underestimate("steps", cost.steps, l.max_steps)?;
        underestimate_u64("depth", cost.depth, l.max_depth)?;
        underestimate("iterations", cost.iters, l.max_iterations)?;
        Ok(())
    }

    fn check_effect_ceilings(&self) -> Result<(), AutomationError> {
        let l = &self.def.limits;
        if let Some(allowed) = &l.allowed_effects {
            let allowed: BTreeSet<EffectKind> = allowed.iter().copied().collect();
            let mut union = EffectSet::new();
            for set in self.effect_bounds.values() {
                union.union_with(set);
            }
            for kind in union.iter() {
                if !allowed.contains(&kind) {
                    return Err(CompileError::BoundCeilingExceeded {
                        kind: "effect",
                        item: kind.name().to_owned(),
                    }
                    .into());
                }
            }
        }
        if let Some(allowed) = &l.allowed_data_access {
            let allowed: BTreeSet<_> = allowed.iter().copied().collect();
            let mut union = DataAccessSet::new();
            for set in self.access_bounds.values() {
                union.union_with(set);
            }
            for class in union.iter() {
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

    fn check_output_emits(&self) -> Result<(), AutomationError> {
        for output in &self.def.outputs {
            if !self.emits.contains_key(&output.name) {
                return Err(CompileError::MissingOutputEmit {
                    output: output.name.clone(),
                }
                .into());
            }
        }
        Ok(())
    }

    // -- step processing -------------------------------------------------

    /// Processes a step list, returning its worst-case [`Cost`].
    /// `top_level` is true only for `definition.steps`.
    fn process_steps(&mut self, steps: &[Step], top_level: bool) -> Result<Cost, AutomationError> {
        let mut cost = Cost::default();
        for step in steps {
            cost = cost.seq(self.process_step(step, top_level)?);
        }
        Ok(cost)
    }

    /// Dispatches one step. Flat per-kind table — splitting the match
    /// into per-variant methods adds indirection without clarity.
    #[allow(clippy::too_many_lines)]
    fn process_step(&mut self, step: &Step, top_level: bool) -> Result<Cost, AutomationError> {
        let id = step.id().to_owned();
        match step {
            Step::Query {
                capability,
                args,
                target,
                ..
            } => {
                let out_ty = self.check_capability_step(
                    &id,
                    capability,
                    args,
                    target.as_ref(),
                    OperationDomain::Query,
                    ExecutionContract::Sync,
                    "query",
                )?;
                self.register_output(id.clone(), Some(out_ty));
                self.step_order.push(id);
                Ok(Cost::leaf())
            }
            Step::Invoke {
                capability,
                args,
                target,
                ..
            } => {
                let out_ty = self.check_capability_step(
                    &id,
                    capability,
                    args,
                    target.as_ref(),
                    OperationDomain::ProjectMutation,
                    ExecutionContract::Sync,
                    "invoke",
                )?;
                self.register_output(id.clone(), Some(out_ty));
                self.step_order.push(id);
                Ok(Cost::leaf())
            }
            Step::AsyncSubmit {
                capability,
                args,
                target,
                ..
            } => {
                self.check_capability_step(
                    &id,
                    capability,
                    args,
                    target.as_ref(),
                    OperationDomain::AsyncOperation,
                    ExecutionContract::AsyncAccept,
                    "async-submit",
                )?;
                self.register_output(id.clone(), Some(StaticType::Job));
                self.step_order.push(id);
                Ok(Cost::leaf())
            }
            Step::AwaitJob {
                job, output_schema, ..
            } => {
                let ty = self.type_of(job, "await-job.job")?;
                if ty != StaticType::Job {
                    return Err(CompileError::JobRefTypeMismatch {
                        step: id.clone(),
                        actual: ty.describe(),
                    }
                    .into());
                }
                let out_ty = if let Some(schema_ref) = output_schema {
                    let node = self.registry.resolve_schema(schema_ref)?;
                    StaticType::from_schema(node)
                } else {
                    self.warnings
                        .push(PlanWarning::OpaqueJobResult { step: id.clone() });
                    StaticType::Json
                };
                self.register_output(id.clone(), Some(out_ty.clone()));
                self.step_order.push(id);
                Ok(Cost::leaf())
            }
            Step::AtomicMacro {
                mac, args, target, ..
            } => {
                // AUTX-04: resolve the macro exactly, check call-site
                // arguments against its parameter signature, and
                // account the flat member expansion in cost/bounds —
                // the batch itself takes no target. No registry at
                // all keeps the AUTX-05 fail-closed answer.
                if self.macros.is_empty() {
                    return Err(CompileError::UndefinedMacro {
                        mac: mac.to_string(),
                    }
                    .into());
                }
                let sig = self.macros.resolve(mac)?;
                if target.is_some() {
                    return Err(CompileError::UnexpectedTarget { step: id.clone() }.into());
                }
                self.check_macro_arguments(args, &sig.parameters)?;
                self.step_order.push(id.clone());
                self.register_output(id.clone(), None);
                for member in &sig.members {
                    let key = member.capability.to_string();
                    self.cap_refs.insert(key.clone());
                    self.effect_bounds
                        .entry(key.clone())
                        .or_default()
                        .union_with(&member.effects);
                    self.access_bounds
                        .entry(key)
                        .or_default()
                        .union_with(&member.data_access);
                }
                Ok(Cost {
                    steps: sig.members.len() as u128,
                    iters: 0,
                    depth: 0,
                })
            }
            Step::If {
                condition,
                then,
                else_steps,
                ..
            } => {
                self.check_condition(condition, "if.condition")?;
                self.register_output(id.clone(), None);
                self.step_order.push(id.clone());
                self.scopes.push(BTreeMap::new());
                let then_cost = self.process_steps(then, false)?;
                self.scopes.pop();
                self.scopes.push(BTreeMap::new());
                let else_cost = self.process_steps(else_steps, false)?;
                self.scopes.pop();
                Ok(Cost {
                    steps: 1u128.saturating_add(then_cost.steps.max(else_cost.steps)),
                    iters: then_cost.iters.max(else_cost.iters),
                    depth: 1 + then_cost.depth.max(else_cost.depth),
                })
            }
            Step::ForEachBounded {
                items,
                max_iterations,
                body,
                ..
            } => {
                if *max_iterations == 0 {
                    return Err(CompileError::UnboundedIteration { step: id.clone() }.into());
                }
                let items_ty = self.type_of(items, "for-each.items")?;
                let StaticType::Array(item_ty) = items_ty else {
                    return Err(CompileError::ItemsNotArray {
                        step: id.clone(),
                        actual: items_ty.describe(),
                    }
                    .into());
                };
                self.register_output(id.clone(), None);
                self.step_order.push(id.clone());
                self.scopes.push(BTreeMap::new());
                self.item_types.push(*item_ty);
                let body_cost = self.process_steps(body, false)?;
                self.item_types.pop();
                self.scopes.pop();
                let m = u128::from(*max_iterations);
                Ok(Cost {
                    steps: 1u128.saturating_add(m.saturating_mul(body_cost.steps)),
                    iters: m.saturating_mul(1u128.saturating_add(body_cost.iters)),
                    depth: 1 + body_cost.depth,
                })
            }
            Step::EmitResult { output, value, .. } => {
                if !top_level {
                    return Err(CompileError::EmitResultNotTopLevel { step: id.clone() }.into());
                }
                let schema = self.output_schemas.get(output).cloned().ok_or_else(|| {
                    CompileError::UndeclaredOutput {
                        step: id.clone(),
                        output: output.clone(),
                    }
                })?;
                self.check_expr_schema(value, &schema, "emit-result.value")
                    .map_err(|e| match e {
                        // Type/schema mismatches get emit-result context;
                        // reference-resolution errors keep their precise
                        // variant (unknown step/input, out-of-scope item,
                        // forward reference, …).
                        AutomationError::Compile(
                            inner @ (CompileError::TypeMismatch { .. }
                            | CompileError::SchemaViolation { .. }),
                        ) => CompileError::OutputTypeMismatch {
                            step: id.clone(),
                            output: output.clone(),
                            reason: inner.to_string(),
                        }
                        .into(),
                        other => other,
                    })?;
                if self.emits.insert(output.clone(), id.clone()).is_some() {
                    return Err(CompileError::DuplicateOutputEmit {
                        output: output.clone(),
                    }
                    .into());
                }
                let out_ty = StaticType::from_schema(&schema);
                self.register_output(id.clone(), Some(out_ty));
                self.step_order.push(id);
                Ok(Cost::leaf())
            }
        }
    }

    /// Registers a completed step's output type in the current scope.
    fn register_output(&mut self, id: String, ty: Option<StaticType>) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(id, ty);
        }
    }

    // -- capability steps ------------------------------------------------

    /// Validates one capability-invoking step and returns its output
    /// static type.
    #[allow(clippy::too_many_arguments)]
    fn check_capability_step(
        &mut self,
        step_id: &str,
        cap: &VersionedRef,
        args: &Expr,
        target: Option<&Expr>,
        domain_req: OperationDomain,
        exec_req: ExecutionContract,
        step_kind: &str,
    ) -> Result<StaticType, AutomationError> {
        let registry = self.registry;
        let desc: &CapabilityDescriptor = registry.resolve_capability(cap)?;
        if desc.availability != Availability::Available {
            return Err(CompileError::CapabilityUnavailable {
                capability: cap.to_string(),
                state: desc.availability.name().to_owned(),
            }
            .into());
        }
        if desc.domain != domain_req {
            return Err(CompileError::DomainMismatch {
                step: step_id.to_owned(),
                capability: cap.to_string(),
                domain: desc.domain.name().to_owned(),
                step_kind: step_kind.to_owned(),
            }
            .into());
        }
        if desc.execution != exec_req {
            return Err(CompileError::ExecutionMismatch {
                step: step_id.to_owned(),
                capability: cap.to_string(),
                execution: desc.execution.name().to_owned(),
                step_kind: step_kind.to_owned(),
            }
            .into());
        }
        let in_schema = registry.resolve_schema(&desc.input_schema)?;
        let out_schema = registry.resolve_schema(&desc.output_schema)?;
        self.check_expr_schema(args, in_schema, "args")?;
        self.check_target(step_id, target, &desc.target)?;
        // Accumulate bounds — union over every reachable step is the
        // conservative branch upper bound.
        self.cap_refs.insert(cap.to_string());
        self.effect_bounds
            .entry(cap.to_string())
            .or_default()
            .union_with(&desc.effect_bound);
        self.access_bounds
            .entry(cap.to_string())
            .or_default()
            .union_with(&desc.data_access);
        Ok(StaticType::from_schema(out_schema))
    }

    /// Checks a step's optional target expr against the capability's
    /// declared [`TargetContract`]. Implicit targets do not exist in
    /// the AST — absence is the only "implicit" and is rejected when
    /// the contract requires an explicit binding.
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

    /// `expect_set` distinguishes `Resource` (single id) from
    /// `ResourceSet` (array of ids).
    fn check_target_expr(
        &mut self,
        step_id: &str,
        expr: &Expr,
        kind: crate::schema::ResourceKind,
        expect_set: bool,
    ) -> Result<(), AutomationError> {
        // Concrete literals get exact tagged-id validation.
        if let Expr::Literal { value } = expr {
            return check_literal_target(step_id, value, kind, expect_set);
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

    // -- expressions ------------------------------------------------------

    /// Static type of an expression; also records referenced inputs.
    fn type_of(&mut self, expr: &Expr, at: &str) -> Result<StaticType, AutomationError> {
        Ok(match expr {
            Expr::Literal { value } => literal_type(value, at)?,
            Expr::Input { path } => {
                let Some(name) = path.first() else {
                    return Err(CompileError::TypeMismatch {
                        path: at.to_owned(),
                        expected: "input path".to_owned(),
                        actual: "empty $input path".to_owned(),
                    }
                    .into());
                };
                self.referenced_inputs.insert(name.clone());
                let ty = self
                    .input_types
                    .get(name)
                    .ok_or_else(|| CompileError::UnknownInput { name: name.clone() })?;
                project_path(ty, &path[1..]).ok_or_else(|| CompileError::TypeMismatch {
                    path: at.to_owned(),
                    expected: "resolvable input field path".to_owned(),
                    actual: format!("$input.{}", path.join(".")),
                })?
            }
            Expr::StepOutput { step_output } => {
                let found = self
                    .scopes
                    .iter()
                    .rev()
                    .find_map(|s| s.get(&step_output.step).map(|v| (s, v)));
                let Some((_, entry)) = found else {
                    return Err(if self.all_ids.contains(&step_output.step) {
                        CompileError::ForwardStepReference {
                            at: at.to_owned(),
                            step: step_output.step.clone(),
                        }
                    } else {
                        CompileError::UnknownStepId {
                            step: step_output.step.clone(),
                        }
                    }
                    .into());
                };
                let Some(ty) = entry else {
                    return Err(CompileError::NoStepOutput {
                        step: step_output.step.clone(),
                    }
                    .into());
                };
                project_path(ty, &step_output.field).ok_or_else(|| {
                    CompileError::UnknownOutputField {
                        step: step_output.step.clone(),
                        field: step_output.field.join("."),
                    }
                })?
            }
            Expr::Item { path } => {
                let Some(ty) = self.item_types.last() else {
                    return Err(CompileError::ItemOutOfScope { at: at.to_owned() }.into());
                };
                project_path(ty, path).ok_or_else(|| CompileError::TypeMismatch {
                    path: at.to_owned(),
                    expected: "resolvable item field path".to_owned(),
                    actual: format!("$item.{}", path.join(".")),
                })?
            }
            Expr::Param { .. } => {
                return Err(CompileError::MacroOnlyExpr {
                    at: at.to_owned(),
                    expr: "param",
                }
                .into());
            }
            Expr::Output { .. } => {
                return Err(CompileError::MacroOnlyExpr {
                    at: at.to_owned(),
                    expr: "output",
                }
                .into());
            }
            Expr::Array { items } => {
                let mut elem: Option<StaticType> = None;
                for item in items {
                    let ty = self.type_of(item, at)?;
                    elem = Some(match elem {
                        None => ty,
                        Some(prev) => join_types(&prev, &ty, at)?,
                    });
                }
                StaticType::Array(Box::new(elem.unwrap_or(StaticType::Json)))
            }
            Expr::Object { fields } => self.type_of_object(fields, at)?,
            Expr::Field { base, path } => {
                let ty = self.type_of(base, at)?;
                project_path(&ty, path).ok_or_else(|| CompileError::TypeMismatch {
                    path: at.to_owned(),
                    expected: "resolvable field path".to_owned(),
                    actual: format!("field projection .{}", path.join(".")),
                })?
            }
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

    /// Checks `expr` against a structural [`SchemaNode`]. Literals are
    /// validated concretely; references are checked by static type.
    fn check_expr_schema(
        &mut self,
        expr: &Expr,
        schema: &SchemaNode,
        path: &str,
    ) -> Result<(), AutomationError> {
        // Structural exprs unwrap Optional schemas to reach their shape.
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
                        // Schema ignores the field, but the expr's
                        // references still must resolve.
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

    /// Checks an `atomic-macro` step's `args` — an object expression
    /// binding each declared macro parameter — against the macro
    /// signature's parameter types. Exact-name matching mirrors the
    /// workflow input contract: undeclared names, missing parameters,
    /// and static type mismatches all reject.
    fn check_macro_arguments(
        &mut self,
        args: &Expr,
        params: &BTreeMap<String, StaticType>,
    ) -> Result<(), AutomationError> {
        let Expr::Object { fields } = args else {
            return Err(CompileError::TypeMismatch {
                path: "args".to_owned(),
                expected: "object of macro parameters".to_owned(),
                actual: "non-object args".to_owned(),
            }
            .into());
        };
        for (name, expr) in fields {
            let Some(expected) = params.get(name) else {
                return Err(CompileError::UnexpectedInput { name: name.clone() }.into());
            };
            let actual = self.type_of(expr, "args")?;
            if !type_compatible(&actual, expected) {
                return Err(CompileError::TypeMismatch {
                    path: format!("args.{name}"),
                    expected: expected.describe(),
                    actual: actual.describe(),
                }
                .into());
            }
        }
        for name in params.keys() {
            if !fields.contains_key(name) {
                return Err(CompileError::MissingInput { name: name.clone() }.into());
            }
        }
        Ok(())
    }

    // -- conditions --------------------------------------------------------

    fn check_condition(&mut self, cond: &Condition, at: &str) -> Result<(), AutomationError> {
        match cond {
            Condition::Eq { left, right } | Condition::Ne { left, right } => {
                let l = self.type_of(left, at)?;
                let r = self.type_of(right, at)?;
                if !(type_compatible(&l, &r) || type_compatible(&r, &l)) {
                    return Err(CompileError::ConditionTypeMismatch {
                        at: at.to_owned(),
                        reason: format!(
                            "incomparable operands: {} vs {}",
                            l.describe(),
                            r.describe()
                        ),
                    }
                    .into());
                }
            }
            Condition::Lt { left, right }
            | Condition::Le { left, right }
            | Condition::Gt { left, right }
            | Condition::Ge { left, right } => {
                for (name, e) in [("left", left), ("right", right)] {
                    let ty = self.type_of(e, at)?;
                    if !matches!(ty, StaticType::Int | StaticType::Number | StaticType::Json) {
                        return Err(CompileError::ConditionTypeMismatch {
                            at: format!("{at}.{name}"),
                            reason: format!(
                                "ordering requires numeric operand, got {}",
                                ty.describe()
                            ),
                        }
                        .into());
                    }
                }
            }
            Condition::Exists { expr } => {
                self.type_of(expr, at)?;
            }
            Condition::And { all } | Condition::Or { any: all } => {
                for c in all {
                    self.check_condition(c, at)?;
                }
            }
            Condition::Not { cond } => self.check_condition(cond, at)?,
        }
        Ok(())
    }

    // -- output ------------------------------------------------------------

    fn into_output(mut self) -> Result<PlanOutput, AutomationError> {
        for decl in &self.def.inputs {
            if !self.referenced_inputs.contains(&decl.name) {
                self.warnings.push(PlanWarning::UnusedInput {
                    name: decl.name.clone(),
                });
            }
        }
        let effect_upper_bounds = self
            .effect_bounds
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    v.iter().map(|e| e.name().to_owned()).collect::<Vec<_>>(),
                )
            })
            .collect();
        let data_access_bounds = self
            .access_bounds
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    v.iter().map(|c| c.name().to_owned()).collect::<Vec<_>>(),
                )
            })
            .collect();
        let digest = definition_digest(self.def)?;
        Ok(PlanOutput {
            plan: ExecutablePlan {
                schema_version: PLAN_SCHEMA_VERSION,
                definition_digest: digest,
                capability_refs_versions: self.cap_refs.iter().cloned().collect(),
                step_order: std::mem::take(&mut self.step_order),
                explicit_target_binding: true,
                effect_upper_bounds,
                data_access_bounds,
                max_steps: self.def.limits.max_steps,
                max_depth: self.def.limits.max_depth,
                max_iterations: self.def.limits.max_iterations,
                max_runtime: self.def.limits.max_runtime_ms,
                max_output_bytes: self.def.limits.max_output_bytes,
            },
            warnings: self.warnings,
        })
    }
}

/// Validates a literal target value against the required resource
/// kind — exact tagged-id checks for concrete strings.
pub(crate) fn check_literal_target(
    step_id: &str,
    value: &Value,
    kind: crate::schema::ResourceKind,
    expect_set: bool,
) -> Result<(), AutomationError> {
    let check_one = |v: &Value| -> Result<(), AutomationError> {
        let s = v.as_str().ok_or_else(|| CompileError::TargetKindMismatch {
            step: step_id.to_owned(),
            expected: format!("id:{kind}"),
            actual: "non-string literal".to_owned(),
        })?;
        validate_tagged_id(kind, s).map_err(|_| CompileError::TargetKindMismatch {
            step: step_id.to_owned(),
            expected: kind.tag().to_owned(),
            actual: s.to_owned(),
        })?;
        Ok(())
    };
    if expect_set {
        let arr = value
            .as_array()
            .ok_or_else(|| CompileError::TargetShapeMismatch {
                step: step_id.to_owned(),
                expected: "array of ids".to_owned(),
                actual: "non-array literal".to_owned(),
            })?;
        if arr.is_empty() {
            return Err(CompileError::TargetShapeMismatch {
                step: step_id.to_owned(),
                expected: "non-empty id array".to_owned(),
                actual: "empty array".to_owned(),
            }
            .into());
        }
        for v in arr {
            check_one(v)?;
        }
        Ok(())
    } else {
        check_one(value)
    }
}

fn collect_ids(steps: &[Step], out: &mut BTreeSet<String>) {
    for step in steps {
        out.insert(step.id().to_owned());
        for children in step.children() {
            collect_ids(children, out);
        }
    }
}

fn ceiling(kind: &'static str, declared: u64, max: u64) -> Result<(), AutomationError> {
    if declared > max {
        return Err(CompileError::BudgetExceeded {
            kind,
            declared,
            ceiling: max,
        }
        .into());
    }
    Ok(())
}

fn underestimate(kind: &'static str, required: u128, declared: u64) -> Result<(), AutomationError> {
    if required > u128::from(declared) {
        return Err(CompileError::BudgetUnderestimated {
            kind,
            required: u64::try_from(required.min(u128::from(u64::MAX))).unwrap_or(u64::MAX),
            declared,
        }
        .into());
    }
    Ok(())
}

fn underestimate_u64(
    kind: &'static str,
    required: u64,
    declared: u64,
) -> Result<(), AutomationError> {
    if required > declared {
        return Err(CompileError::BudgetUnderestimated {
            kind,
            required,
            declared,
        }
        .into());
    }
    Ok(())
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

/// Static type of a literal JSON value. Strings that parse as
/// `<kind>-<32 hex>` typed ids get [`StaticType::Id`]; everything else
/// stays `String`.
pub(crate) fn literal_type(value: &Value, at: &str) -> Result<StaticType, AutomationError> {
    Ok(match value {
        Value::Null => StaticType::Null,
        Value::Bool(_) => StaticType::Bool,
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                StaticType::Int
            } else {
                StaticType::Number
            }
        }
        Value::String(s) => match tagged_id_kind(s) {
            Some(kind) => StaticType::Id(kind),
            None => StaticType::String,
        },
        Value::Array(items) => {
            let mut elem: Option<StaticType> = None;
            for item in items {
                let ty = literal_type(item, at)?;
                elem = Some(match elem {
                    None => ty,
                    Some(prev) => join_types(&prev, &ty, at)?,
                });
            }
            StaticType::Array(Box::new(elem.unwrap_or(StaticType::Json)))
        }
        Value::Object(map) => StaticType::Object(
            map.iter()
                .map(|(k, v)| {
                    Ok((
                        k.clone(),
                        FieldType {
                            ty: literal_type(v, at)?,
                            required: true,
                        },
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, AutomationError>>()?,
        ),
    })
}

/// Parses `"<kind>-<32 hex>"` into its [`crate::schema::ResourceKind`].
fn tagged_id_kind(s: &str) -> Option<crate::schema::ResourceKind> {
    let (tag, hex) = s.rsplit_once('-')?;
    if hex.len() != 32 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    crate::schema::ResourceKind::parse_tag(tag)
}

/// Unifies two element types; fails on genuinely heterogeneous shapes.
pub(crate) fn join_types(
    a: &StaticType,
    b: &StaticType,
    at: &str,
) -> Result<StaticType, AutomationError> {
    use StaticType as T;
    if a == b {
        return Ok(a.clone());
    }
    match (a, b) {
        (T::Int, T::Number) | (T::Number, T::Int) => Ok(T::Number),
        (T::Json, _) | (_, T::Json) => Ok(T::Json),
        _ => Err(CompileError::TypeMismatch {
            path: at.to_owned(),
            expected: a.describe(),
            actual: b.describe(),
        }
        .into()),
    }
}

/// Projects a field path into a static type. `Optional` unwraps and
/// re-wraps; `Json` passes through opaquely; anything unresolvable
/// returns `None`.
pub(crate) fn project_path(ty: &StaticType, path: &[String]) -> Option<StaticType> {
    if path.is_empty() {
        return Some(ty.clone());
    }
    match ty {
        StaticType::Optional(inner) => {
            let projected = project_path(inner, path)?;
            Some(StaticType::Optional(Box::new(projected)))
        }
        StaticType::Object(fields) => {
            let field = fields.get(&path[0])?;
            let projected = project_path(&field.ty, &path[1..])?;
            // Re-wrap optionality of intermediate fields.
            Some(if field.required {
                projected
            } else {
                StaticType::Optional(Box::new(projected))
            })
        }
        StaticType::Json => Some(StaticType::Json),
        _ => None,
    }
}
