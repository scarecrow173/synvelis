//! `synvelis-automation` — versioned workflow DSL and plan compiler
//! (AUTX-05 / synvelis#264, AUTX-04 / synvelis#263).
//!
//! This crate owns the *definition → plan* boundary only:
//!
//! - [`parse_workflow_yaml`] — restricted YAML safe-subset frontend.
//! - [`WorkflowDefinition`] — the source-agnostic typed AST that YAML
//!   (here) and Luau/Rhai builders (synvelis#273) both lower into.
//! - [`PlanCompiler`] — validates definitions against typed inputs and
//!   a serialized [`RegistrySnapshot`], producing an [`ExecutablePlan`]
//!   with deterministic digests, step order, and static budgets.
//! - [`AtomicMacroDefinition`] / [`MacroCompiler`] — versioned atomic
//!   macro recipes compiled to the flat `synvelis.operation.batch`
//!   member list (AUTX-04); execution still flows through the existing
//!   GW-04/05/06 path.
//!
//! Out of scope (owned elsewhere): runtime execution/coordinator FSM
//! (#265), script builders/importers (#273), live Gateway descriptor
//! reads (integration slice), arbitrary script/shell/eval — none of
//! which can be expressed here.

pub mod canon;
pub mod compiler;
pub mod definition;
pub mod error;
pub mod expr;
pub mod frontend;
pub mod macros;
pub mod refs;
pub mod registry;
pub mod schema;
pub mod yaml;

pub use compiler::{ExecutablePlan, PlanCompiler, PlanOutput, PlanWarning};
pub use definition::{DEFINITION_SCHEMA_VERSION, Step, WorkflowDefinition, WorkflowLimits};
pub use error::{AutomationError, CompileError};
pub use expr::{Condition, Expr, StepOutputRef};
pub use frontend::{
    decode_macro_definition, decode_registry_snapshot, decode_workflow_definition,
    parse_macro_yaml, parse_registry_snapshot_yaml, parse_workflow_yaml,
};
pub use macros::{
    AtomicMacroDefinition, COMPILED_MACRO_SCHEMA_VERSION, CompiledMacro, MACRO_SCHEMA_VERSION,
    MacroBindingWire, MacroCompiler, MacroMemberSig, MacroPolicy, MacroRegistry, MacroSignature,
    MacroStep, OUTPUT_BINDING_ID_BASE, OutputBinding, macro_digest,
};
pub use refs::{Version, VersionedRef};
pub use registry::RegistrySnapshot;
pub use schema::{SchemaNode, StaticType};
