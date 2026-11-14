//! The versioned [`WorkflowDefinition`] — the source-agnostic typed AST
//! every frontend (YAML today; Luau/Rhai builders in synvelis#273)
//! lowers into before [`crate::PlanCompiler`] validation.
//!
//! A definition carries *no* authority: no principal, grant, approval,
//! credential, shell, eval, or script payload can be expressed — the
//! typed step/expr/condition enums simply have no such variants, and
//! unknown fields are rejected at decode.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::AutomationError;
use crate::expr::{Condition, Expr};
use crate::refs::VersionedRef;
use crate::registry::{DataAccessClass, EffectKind};
use crate::schema::SchemaNode;

/// The only `schema_version` this crate accepts.
pub const DEFINITION_SCHEMA_VERSION: u32 = 1;

/// Maximum declared inputs accepted.
pub const MAX_DECLARED_INPUTS: usize = 256;
/// Maximum declared outputs accepted.
pub const MAX_DECLARED_OUTPUTS: usize = 256;
/// Maximum total step nodes (including nested) accepted.
pub const MAX_STATIC_STEPS: usize = 4_096;

/// Maximum identifier length (workflow id, step id, io name).
pub const MAX_NAME_LEN: usize = 64;

/// A declared workflow input: name plus its structural schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputDecl {
    /// The input name — referenced as `$input.<name>`.
    pub name: String,
    /// The structural schema the provided value must satisfy.
    pub schema: SchemaNode,
}

/// A declared workflow output: name plus its structural schema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputDecl {
    /// The output name — bound by exactly one top-level
    /// `emit-result` step.
    pub name: String,
    /// The structural schema the emitted value must satisfy.
    pub schema: SchemaNode,
}

/// Static budgets declared by the definition.
///
/// Every value is a fail-closed ceiling: the compiler rejects declared
/// values above its hard limits ([`crate::compiler::STATIC_CEILINGS`])
/// and rejects definitions whose *computed* worst case (steps, depth,
/// iterations) exceeds the declared bound. Runtime/output/memory have
/// no static worst case — their declared values are carried into the
/// plan as enforcement budgets for the runtime owner (synvelis#265).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowLimits {
    /// Worst-case step executions allowed.
    pub max_steps: u64,
    /// Maximum control-flow nesting depth (`if`/`for-each`).
    pub max_depth: u64,
    /// Worst-case total loop-body executions allowed.
    pub max_iterations: u64,
    /// Runtime budget in milliseconds.
    pub max_runtime_ms: u64,
    /// Maximum serialized output size in bytes.
    pub max_output_bytes: u64,
    /// Maximum memory budget in bytes (definition-level constraint;
    /// enforced by the runtime owner, carried in the definition digest).
    pub max_memory_bytes: u64,
    /// Optional ceiling on the union of step effect kinds. When
    /// present, any step effect outside this set rejects the compile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_effects: Option<Vec<EffectKind>>,
    /// Optional ceiling on the union of step data-access classes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_data_access: Option<Vec<DataAccessClass>>,
}

/// One typed workflow step.
///
/// The `kind` tag selects the variant; every variant carries `id`.
/// `if`/`for-each-bounded` nest [`Step`]s; all other variants invoke a
/// versioned capability, a macro reference, or bind an output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Step {
    /// Synchronous read-only capability invocation.
    Query {
        /// Step id — unique across the whole definition.
        id: String,
        /// `capability@major.minor`.
        capability: VersionedRef,
        /// Arguments validated against the capability input schema.
        #[serde(default)]
        args: Expr,
        /// Explicit target binding (required iff the capability's
        /// target contract demands one; forbidden otherwise).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<Expr>,
    },
    /// Synchronous project-mutation capability invocation.
    Invoke {
        /// Step id.
        id: String,
        /// `capability@major.minor`.
        capability: VersionedRef,
        /// Arguments.
        #[serde(default)]
        args: Expr,
        /// Explicit target binding.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<Expr>,
    },
    /// Asynchronous job submission (`async-accept` capabilities only).
    /// Produces a job-typed output referenceable as `$steps.<id>`.
    AsyncSubmit {
        /// Step id.
        id: String,
        /// `capability@major.minor`.
        capability: VersionedRef,
        /// Arguments.
        #[serde(default)]
        args: Expr,
        /// Explicit target binding.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<Expr>,
    },
    /// Waits on a job-typed value (typically an `async-submit` output).
    /// `output_schema`, when declared, types the awaited result;
    /// otherwise the result is opaque JSON (runtime re-validates).
    AwaitJob {
        /// Step id.
        id: String,
        /// The job handle expression — must be job-typed.
        job: Expr,
        /// Optional `schema@major.minor` typing the job result.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        output_schema: Option<VersionedRef>,
    },
    /// References an atomic macro by `id@major.minor`.
    ///
    /// Macro definitions, registries, and expansion are owned by
    /// synvelis#263; until that lands every macro reference fails
    /// closed with [`crate::error::CompileError::UndefinedMacro`].
    AtomicMacro {
        /// Step id.
        id: String,
        /// `macro@major.minor`.
        mac: VersionedRef,
        /// Macro arguments.
        #[serde(default)]
        args: Expr,
        /// Explicit target binding.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<Expr>,
    },
    /// Bounded conditional: evaluates `condition`, then runs exactly
    /// one of `then`/`else`. Effects are the conservative union of
    /// both branches.
    If {
        /// Step id.
        id: String,
        /// The branch condition.
        condition: Condition,
        /// Steps run when the condition holds (non-empty).
        then: Vec<Step>,
        /// Steps run otherwise (may be empty/absent).
        #[serde(default, rename = "else", skip_serializing_if = "Vec::is_empty")]
        else_steps: Vec<Step>,
    },
    /// Bounded iteration: runs `body` at most `max_iterations` times
    /// over `items`. `$item` inside the body is the element type.
    ForEachBounded {
        /// Step id.
        id: String,
        /// Array-typed expression iterated over.
        items: Expr,
        /// Hard iteration bound — must be ≥ 1; the compiler also
        /// verifies the total worst case fits `limits.max_iterations`.
        max_iterations: u64,
        /// Body steps (may be empty).
        #[serde(default)]
        body: Vec<Step>,
    },
    /// Binds a value to a declared workflow output. Must appear at
    /// top level exactly once per declared output.
    EmitResult {
        /// Step id.
        id: String,
        /// The declared output name.
        output: String,
        /// The emitted value — checked against the output schema.
        value: Expr,
    },
}

impl Step {
    /// The step id.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Query { id, .. }
            | Self::Invoke { id, .. }
            | Self::AsyncSubmit { id, .. }
            | Self::AwaitJob { id, .. }
            | Self::AtomicMacro { id, .. }
            | Self::If { id, .. }
            | Self::ForEachBounded { id, .. }
            | Self::EmitResult { id, .. } => id,
        }
    }

    /// The nested step lists of control-flow variants (empty for leaf
    /// steps). `If` yields `then` then `else`; `ForEachBounded` yields
    /// `body`.
    #[must_use]
    pub fn children(&self) -> Vec<&[Step]> {
        match self {
            Self::If {
                then, else_steps, ..
            } => vec![then.as_slice(), else_steps.as_slice()],
            Self::ForEachBounded { body, .. } => vec![body.as_slice()],
            _ => Vec::new(),
        }
    }
}

/// A versioned workflow definition — the shared compiler input.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDefinition {
    /// Must equal [`DEFINITION_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Workflow identifier (`[a-z0-9._-]`, ≤ [`MAX_NAME_LEN`]).
    pub workflow_id: String,
    /// The workflow's own semantic version.
    pub version: u32,
    /// Declared inputs.
    #[serde(default)]
    pub inputs: Vec<InputDecl>,
    /// Top-level steps in document order.
    #[serde(default)]
    pub steps: Vec<Step>,
    /// Declared outputs.
    #[serde(default)]
    pub outputs: Vec<OutputDecl>,
    /// Static budgets.
    pub limits: WorkflowLimits,
}

impl WorkflowDefinition {
    /// Source-agnostic structural validation: identifier shapes,
    /// duplicate names/ids, arity of boolean combinators, and size
    /// caps. Reference/capability/schema checks live in
    /// [`crate::PlanCompiler`].
    ///
    /// # Errors
    /// [`AutomationError::Definition`] on the first violation.
    pub fn validate(&self) -> Result<(), AutomationError> {
        if self.schema_version != DEFINITION_SCHEMA_VERSION {
            return Err(AutomationError::UnsupportedDefinitionVersion {
                found: self.schema_version,
            });
        }
        validate_workflow_id(&self.workflow_id)?;
        if self.inputs.len() > MAX_DECLARED_INPUTS {
            return Err(def_err(format!(
                "too many inputs: {} > {MAX_DECLARED_INPUTS}",
                self.inputs.len()
            )));
        }
        if self.outputs.len() > MAX_DECLARED_OUTPUTS {
            return Err(def_err(format!(
                "too many outputs: {} > {MAX_DECLARED_OUTPUTS}",
                self.outputs.len()
            )));
        }
        let mut input_names = BTreeSet::new();
        for input in &self.inputs {
            validate_name(&input.name, "input")?;
            if !input_names.insert(&input.name) {
                return Err(def_err(format!("duplicate input name: {}", input.name)));
            }
        }
        let mut output_names = BTreeSet::new();
        for output in &self.outputs {
            validate_name(&output.name, "output")?;
            if !output_names.insert(&output.name) {
                return Err(def_err(format!("duplicate output name: {}", output.name)));
            }
        }
        let mut step_ids = BTreeSet::new();
        let mut step_count = 0usize;
        validate_steps(&self.steps, &mut step_ids, &mut step_count)?;
        if step_count > MAX_STATIC_STEPS {
            return Err(def_err(format!(
                "too many steps: {step_count} > {MAX_STATIC_STEPS}"
            )));
        }
        Ok(())
    }
}

fn validate_steps(
    steps: &[Step],
    ids: &mut BTreeSet<String>,
    count: &mut usize,
) -> Result<(), AutomationError> {
    for step in steps {
        *count += 1;
        validate_name(step.id(), "step")?;
        if !ids.insert(step.id().to_owned()) {
            return Err(def_err(format!("duplicate step id: {}", step.id())));
        }
        match step {
            Step::If {
                condition,
                then,
                else_steps,
                ..
            } => {
                if then.is_empty() {
                    return Err(def_err(format!(
                        "if step {} has an empty then branch",
                        step.id()
                    )));
                }
                validate_condition(condition)?;
                validate_steps(then, ids, count)?;
                validate_steps(else_steps, ids, count)?;
            }
            Step::ForEachBounded { body, .. } => {
                validate_steps(body, ids, count)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_condition(cond: &Condition) -> Result<(), AutomationError> {
    match cond {
        Condition::And { all } | Condition::Or { any: all } => {
            if all.is_empty() {
                return Err(def_err("empty and/or condition".to_owned()));
            }
            for c in all {
                validate_condition(c)?;
            }
        }
        Condition::Not { cond } => validate_condition(cond)?,
        _ => {}
    }
    Ok(())
}

/// `[a-z0-9._-]`, 1..=[`MAX_NAME_LEN`], first char alphabetic —
/// mirrors the capability-id shape used across the gateway contract.
pub(crate) fn validate_workflow_id(id: &str) -> Result<(), AutomationError> {
    let valid = !id.is_empty()
        && id.len() <= MAX_NAME_LEN
        && id.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if !valid {
        return Err(def_err(format!(
            "invalid workflow_id '{id}' (want [a-z0-9._-], ≤{MAX_NAME_LEN})"
        )));
    }
    Ok(())
}

/// Step/io names: `[A-Za-z0-9_-]`, must start alphabetic. Dots are
/// excluded because `$input.<name>` / `$steps.<id>.<field>` paths split
/// on `.`.
pub(crate) fn validate_name(name: &str, what: &str) -> Result<(), AutomationError> {
    let valid = !name.is_empty()
        && name.len() <= MAX_NAME_LEN
        && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
    if !valid {
        return Err(def_err(format!(
            "invalid {what} name '{name}' (want [A-Za-z0-9_-], ≤{MAX_NAME_LEN})"
        )));
    }
    Ok(())
}

pub(crate) fn def_err(reason: String) -> AutomationError {
    AutomationError::Definition { reason }
}
