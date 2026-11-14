//! `synvelis-automation` — versioned workflow DSL, plan compiler, and
//! headless run coordinator (AUTX-05 / synvelis#264, AUTX-04 / synvelis#263,
//! AUTX-06 / synvelis#265).
//!
//! This crate owns two boundaries:
//!
//! - *Definition → plan*: [`parse_workflow_yaml`] restricted YAML
//!   frontend, the source-agnostic [`WorkflowDefinition`] AST that
//!   YAML (here) and Luau/Rhai builders (synvelis#273) both lower into,
//!   and [`PlanCompiler`] producing an [`ExecutablePlan`] with
//!   deterministic digests, step order, and static budgets.
//!   [`AtomicMacroDefinition`] / [`MacroCompiler`] compile versioned
//!   atomic macro recipes to the flat `synvelis.operation.batch`
//!   member list (AUTX-04).
//! - *Run → journal*: [`run`] owns the WorkflowRun/Step FSM, the
//!   append-only run journal, and the resume cursor; [`coordinator`]
//!   drives a plan occurrence-by-occurrence over the [`StepLane`] /
//!   [`JobLane`] authority seams — progress state only, never a second
//!   source of truth for Job/Project/Grant/Approval domains.
//!
//! Out of scope (owned elsewhere): live Gateway descriptor reads and
//! real job/provider binding (integration slice), script
//! builders/importers (#273), arbitrary script/shell/eval — none of
//! which can be expressed here.

pub mod action_recipe;
pub mod canon;
pub mod compiler;
pub mod coordinator;
pub mod definition;
pub mod error;
pub mod eval;
pub mod expr;
pub mod frontend;
pub mod importer;
pub mod macros;
pub mod refs;
pub mod registry;
pub mod run;
pub mod schema;
pub mod yaml;

pub use action_recipe::compile_action_recipe;
pub use compiler::{ExecutablePlan, PlanCompiler, PlanOutput, PlanWarning};
pub use definition::{DEFINITION_SCHEMA_VERSION, Step, WorkflowDefinition, WorkflowLimits};
pub use error::{AutomationError, CompileError, ImportError};
pub use expr::{Condition, Expr, StepOutputRef};
pub use frontend::{
    decode_macro_definition, decode_registry_snapshot, decode_workflow_definition,
    parse_macro_yaml, parse_registry_snapshot_yaml, parse_workflow_yaml,
};
pub use importer::{
    FileWorkflowResolver, ImportedWorkflow, ImportedWorkflowPayload, MAX_IMPORT_DEPTH,
    MapWorkflowResolver, WorkflowResolver, WorkflowSourceKind, compile_script_workflow,
    compile_workflow_yaml, import_workflow_document, workflow_source_kind,
};
pub use macros::{
    AtomicMacroDefinition, COMPILED_MACRO_SCHEMA_VERSION, CompiledMacro, MACRO_SCHEMA_VERSION,
    MacroBindingWire, MacroCompiler, MacroMemberSig, MacroPolicy, MacroRegistry, MacroSignature,
    MacroStep, OUTPUT_BINDING_ID_BASE, OutputBinding, macro_digest,
};
pub use refs::{Version, VersionedRef};
pub use registry::RegistrySnapshot;
pub use schema::{SchemaNode, StaticType};
