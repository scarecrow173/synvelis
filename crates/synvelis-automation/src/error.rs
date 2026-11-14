//! Error types for the workflow DSL frontend and plan compiler.
//!
//! Every failure is fail-closed and carries a stable kind so golden
//! fixtures can assert *which* contract was violated, not just that
//! something failed.

use std::fmt;

/// Errors produced while parsing, decoding, or compiling workflows.
#[derive(Debug)]
pub enum AutomationError {
    /// The YAML safe-subset parser rejected the source.
    Yaml {
        /// 1-based line number where parsing failed (0 when unknown).
        line: usize,
        /// Human-readable reason.
        reason: String,
    },
    /// A canonical JSON / serde decode failure.
    Json {
        /// Human-readable reason.
        reason: String,
    },
    /// Semantic validation of a decoded [`crate::WorkflowDefinition`]
    /// failed (identifier shapes, duplicate names, forbidden fields).
    Definition {
        /// Human-readable reason.
        reason: String,
    },
    /// The document's `schema_version` is not supported.
    UnsupportedDefinitionVersion {
        /// The version found in the document.
        found: u32,
    },
    /// Semantic validation of a [`crate::RegistrySnapshot`] failed.
    Registry {
        /// Human-readable reason.
        reason: String,
    },
    /// A typed compile rejection from [`crate::PlanCompiler`].
    Compile(CompileError),
}

/// Typed compile-time rejections.
///
/// Variants are deliberately granular: each maps to one fail-closed rule
/// in the Issue #264 acceptance contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileError {
    /// A `StepOutputRef` names a step id that does not exist.
    UnknownStepId {
        /// The referenced step id.
        step: String,
    },
    /// A `StepOutputRef` (or loop item) refers to a step defined later
    /// in document order — forward references and cycles are rejected
    /// by only allowing backward references.
    ForwardStepReference {
        /// The referencing location (step id or `<inputs>`/`<outputs>`).
        at: String,
        /// The referenced step id.
        step: String,
    },
    /// A step id is defined more than once in the definition.
    DuplicateStepId {
        /// The duplicated step id.
        step: String,
    },
    /// A step references an input name that is not declared.
    UnknownInput {
        /// The unknown input name.
        name: String,
    },
    /// A provided compile input is not declared by the definition.
    UnexpectedInput {
        /// The undeclared input name.
        name: String,
    },
    /// A required input was not provided.
    MissingInput {
        /// The missing input name.
        name: String,
    },
    /// A provided input value does not satisfy its declared schema.
    InputTypeMismatch {
        /// The input name.
        name: String,
        /// Path inside the value.
        path: String,
        /// Reason.
        reason: String,
    },
    /// A step `args`/`target`/condition references a schema or
    /// capability that the registry snapshot does not contain.
    UnknownCapability {
        /// The capability id.
        id: String,
    },
    /// The requested capability version is not registered.
    UnsupportedCapabilityVersion {
        /// The capability id.
        id: String,
        /// The requested version (`major.minor`).
        requested: String,
    },
    /// The capability exists but is not `available`
    /// (planned / retired / disabled are all fail-closed).
    CapabilityUnavailable {
        /// The capability ref (`id@major.minor`).
        capability: String,
        /// The availability state name.
        state: String,
    },
    /// The capability's operation domain does not match the step kind
    /// (e.g. `async-submit` on a synchronous capability).
    DomainMismatch {
        /// The step id.
        step: String,
        /// The capability ref.
        capability: String,
        /// The declared domain.
        domain: String,
        /// The step kind that required a different domain.
        step_kind: String,
    },
    /// The capability's execution contract does not match the step
    /// kind (e.g. `query` on an `async-accept` capability).
    ExecutionMismatch {
        /// The step id.
        step: String,
        /// The capability ref.
        capability: String,
        /// The declared execution contract.
        execution: String,
        /// The step kind.
        step_kind: String,
    },
    /// A schema ref used by the definition or snapshot cannot be
    /// resolved.
    UnknownSchema {
        /// The schema name.
        name: String,
    },
    /// A schema version is not registered for its name.
    UnsupportedSchemaVersion {
        /// The schema name.
        name: String,
        /// The requested version (`major.minor`).
        requested: String,
    },
    /// An argument literal does not satisfy the declared input schema.
    SchemaViolation {
        /// JSON-path-like location of the violation.
        path: String,
        /// Reason.
        reason: String,
    },
    /// A referenced value's static type is not assignable to the
    /// expected schema position.
    TypeMismatch {
        /// Location being checked.
        path: String,
        /// Expected type description.
        expected: String,
        /// Actual static type description.
        actual: String,
    },
    /// A capability declares `TargetContract::None` but the step
    /// supplies a target binding.
    UnexpectedTarget {
        /// The step id.
        step: String,
    },
    /// A capability requires an explicit target but the step does not
    /// supply one — implicit targets (selection / active window /
    /// last-edited) do not exist in this DSL.
    MissingTarget {
        /// The step id.
        step: String,
    },
    /// The supplied target binding shape does not match the declared
    /// target contract.
    TargetShapeMismatch {
        /// The step id.
        step: String,
        /// The declared contract shape.
        expected: String,
        /// The supplied binding shape.
        actual: String,
    },
    /// A target argument cannot be proven to have the required
    /// resource kind statically.
    TargetKindMismatch {
        /// The step id.
        step: String,
        /// The required resource kind.
        expected: String,
        /// What was found instead.
        actual: String,
    },
    /// An `atomic-macro` step references a macro that is not defined.
    /// Macro definitions/registry/expansion are owned by synvelis#263;
    /// until then every macro reference fails closed.
    UndefinedMacro {
        /// The macro ref (`id@major.minor`).
        mac: String,
    },
    /// A `for-each-bounded` step is missing its explicit iteration
    /// bound or the bound is zero.
    UnboundedIteration {
        /// The step id.
        step: String,
    },
    /// The `items` expression of a `for-each-bounded` step is not
    /// statically known to be an array.
    ItemsNotArray {
        /// The step id.
        step: String,
        /// The static type found.
        actual: String,
    },
    /// An `await-job` step's `job` argument is not a job-typed value.
    JobRefTypeMismatch {
        /// The step id.
        step: String,
        /// The static type found.
        actual: String,
    },
    /// An `emit-result` step names an output the definition does not
    /// declare.
    UndeclaredOutput {
        /// The step id.
        step: String,
        /// The undeclared output name.
        output: String,
    },
    /// An `emit-result` value is not assignable to the declared output
    /// schema.
    OutputTypeMismatch {
        /// The step id.
        step: String,
        /// The output name.
        output: String,
        /// Reason.
        reason: String,
    },
    /// A condition operand pair cannot be compared statically.
    ConditionTypeMismatch {
        /// Location of the condition.
        at: String,
        /// Reason.
        reason: String,
    },
    /// A definition `limits` value exceeds the compiler's hard static
    /// budget — budgets are fail-closed ceilings, not hints.
    BudgetExceeded {
        /// Which budget (`steps`, `depth`, `iterations`, `runtime`,
        /// `output-bytes`, `memory-bytes`).
        kind: &'static str,
        /// The declared value.
        declared: u64,
        /// The hard ceiling.
        ceiling: u64,
    },
    /// The statically computed worst case exceeds a declared limit.
    BudgetUnderestimated {
        /// Which budget.
        kind: &'static str,
        /// The worst-case requirement.
        required: u64,
        /// The declared limit.
        declared: u64,
    },
    /// `$item` was referenced outside any `for-each-bounded` body.
    ItemOutOfScope {
        /// Location of the reference.
        at: String,
    },
    /// An `emit-result` step appears inside `if`/`for-each` control
    /// flow — outputs must be produced unconditionally at top level.
    EmitResultNotTopLevel {
        /// The step id.
        step: String,
    },
    /// A declared output has no `emit-result` step.
    MissingOutputEmit {
        /// The output name.
        output: String,
    },
    /// A declared output is emitted by more than one step.
    DuplicateOutputEmit {
        /// The output name.
        output: String,
    },
    /// The union of step effects/data-access exceeds a declared
    /// `limits.allowed_*` ceiling.
    BoundCeilingExceeded {
        /// `"effect"` or `"data-access"`.
        kind: &'static str,
        /// The offending bound name.
        item: String,
    },
    /// A step produces no referenceable output but is referenced via
    /// `StepOutputRef`.
    NoStepOutput {
        /// The referenced step id.
        step: String,
    },
    /// A `StepOutputRef.field` path does not resolve inside the
    /// producing step's output schema.
    UnknownOutputField {
        /// The step id.
        step: String,
        /// The field path that failed.
        field: String,
    },
}

impl fmt::Display for AutomationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yaml { line, reason } => write!(f, "yaml parse error at line {line}: {reason}"),
            Self::Json { reason } => write!(f, "json error: {reason}"),
            Self::Definition { reason } => write!(f, "definition error: {reason}"),
            Self::UnsupportedDefinitionVersion { found } => {
                write!(f, "unsupported definition schema_version: {found}")
            }
            Self::Registry { reason } => write!(f, "registry snapshot error: {reason}"),
            Self::Compile(err) => write!(f, "compile rejected: {err}"),
        }
    }
}

impl fmt::Display for CompileError {
    #[allow(clippy::too_many_lines)] // one message arm per variant
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownStepId { step } => write!(f, "unknown step id: {step}"),
            Self::ForwardStepReference { at, step } => {
                write!(f, "forward step reference at {at}: {step}")
            }
            Self::DuplicateStepId { step } => write!(f, "duplicate step id: {step}"),
            Self::UnknownInput { name } => write!(f, "unknown input: {name}"),
            Self::UnexpectedInput { name } => write!(f, "undeclared input provided: {name}"),
            Self::MissingInput { name } => write!(f, "required input missing: {name}"),
            Self::InputTypeMismatch { name, path, reason } => {
                write!(f, "input {name} mismatch at {path}: {reason}")
            }
            Self::UnknownCapability { id } => write!(f, "unknown capability: {id}"),
            Self::UnsupportedCapabilityVersion { id, requested } => {
                write!(f, "unsupported capability version: {id}@{requested}")
            }
            Self::CapabilityUnavailable { capability, state } => {
                write!(f, "capability unavailable ({state}): {capability}")
            }
            Self::DomainMismatch {
                step,
                capability,
                domain,
                step_kind,
            } => write!(
                f,
                "domain mismatch at {step}: {capability} is {domain}, not usable as {step_kind}"
            ),
            Self::ExecutionMismatch {
                step,
                capability,
                execution,
                step_kind,
            } => write!(
                f,
                "execution mismatch at {step}: {capability} is {execution}, not usable as {step_kind}"
            ),
            Self::UnknownSchema { name } => write!(f, "unknown schema: {name}"),
            Self::UnsupportedSchemaVersion { name, requested } => {
                write!(f, "unsupported schema version: {name}@{requested}")
            }
            Self::SchemaViolation { path, reason } => {
                write!(f, "schema violation at {path}: {reason}")
            }
            Self::TypeMismatch {
                path,
                expected,
                actual,
            } => write!(
                f,
                "type mismatch at {path}: expected {expected}, got {actual}"
            ),
            Self::UnexpectedTarget { step } => {
                write!(f, "unexpected target binding at {step}")
            }
            Self::MissingTarget { step } => {
                write!(f, "missing explicit target at {step}")
            }
            Self::TargetShapeMismatch {
                step,
                expected,
                actual,
            } => write!(
                f,
                "target shape mismatch at {step}: expected {expected}, got {actual}"
            ),
            Self::TargetKindMismatch {
                step,
                expected,
                actual,
            } => write!(
                f,
                "target kind mismatch at {step}: expected {expected}, got {actual}"
            ),
            Self::UndefinedMacro { mac } => write!(f, "undefined macro reference: {mac}"),
            Self::UnboundedIteration { step } => {
                write!(f, "unbounded iteration at {step}")
            }
            Self::ItemsNotArray { step, actual } => {
                write!(f, "for-each items is not an array at {step}: {actual}")
            }
            Self::JobRefTypeMismatch { step, actual } => {
                write!(f, "await-job argument is not a job ref at {step}: {actual}")
            }
            Self::UndeclaredOutput { step, output } => {
                write!(f, "emit-result at {step} names undeclared output: {output}")
            }
            Self::OutputTypeMismatch {
                step,
                output,
                reason,
            } => write!(f, "emit-result at {step} for {output}: {reason}"),
            Self::ConditionTypeMismatch { at, reason } => {
                write!(f, "condition type mismatch at {at}: {reason}")
            }
            Self::BudgetExceeded {
                kind,
                declared,
                ceiling,
            } => write!(
                f,
                "budget {kind} exceeds static ceiling: declared {declared} > {ceiling}"
            ),
            Self::BudgetUnderestimated {
                kind,
                required,
                declared,
            } => write!(
                f,
                "budget {kind} underestimated: worst case {required} > declared {declared}"
            ),
            Self::ItemOutOfScope { at } => {
                write!(f, "$item referenced outside for-each body at {at}")
            }
            Self::EmitResultNotTopLevel { step } => {
                write!(f, "emit-result at {step} is not at top level")
            }
            Self::MissingOutputEmit { output } => {
                write!(f, "declared output has no emit-result: {output}")
            }
            Self::DuplicateOutputEmit { output } => {
                write!(f, "output emitted more than once: {output}")
            }
            Self::BoundCeilingExceeded { kind, item } => {
                write!(f, "{kind} bound '{item}' exceeds declared ceiling")
            }
            Self::NoStepOutput { step } => {
                write!(f, "step {step} produces no referenceable output")
            }
            Self::UnknownOutputField { step, field } => {
                write!(f, "unknown output field {field} on step {step}")
            }
        }
    }
}

impl std::error::Error for AutomationError {}
impl std::error::Error for CompileError {}

impl From<CompileError> for AutomationError {
    fn from(err: CompileError) -> Self {
        Self::Compile(err)
    }
}

/// Convenience: wraps a [`CompileError`] into [`AutomationError`].
#[must_use]
pub fn compile_err(err: CompileError) -> AutomationError {
    AutomationError::Compile(err)
}
