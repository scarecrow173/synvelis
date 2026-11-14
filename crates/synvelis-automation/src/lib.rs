//! `synvelis-automation` — versioned workflow DSL and plan compiler
//! (AUTX-05 / synvelis#264).
//!
//! This crate owns the *definition → plan* boundary only:
//!
//! - [`parse_workflow_yaml`] — restricted YAML safe-subset frontend.
//! - [`WorkflowDefinition`] — the source-agnostic typed AST that YAML
//!   (here) and Luau/Rhai builders (synvelis#273) both lower into.
//! - [`PlanCompiler`] — validates definitions against typed inputs and
//!   a serialized [`RegistrySnapshot`], producing an [`ExecutablePlan`]
//!   with deterministic digests, step order, and static budgets.
//!
//! Out of scope (owned elsewhere): runtime execution/coordinator FSM
//! (#265), script builders/importers (#273), live Gateway descriptor
//! reads (integration slice), macro definitions (#263), arbitrary
//! script/shell/eval — none of which can be expressed here.

pub mod canon;
pub mod compiler;
pub mod definition;
pub mod error;
pub mod expr;
pub mod frontend;
pub mod refs;
pub mod registry;
pub mod schema;
pub mod yaml;

pub use compiler::{ExecutablePlan, PlanCompiler, PlanOutput, PlanWarning};
pub use definition::{DEFINITION_SCHEMA_VERSION, Step, WorkflowDefinition, WorkflowLimits};
pub use error::{AutomationError, CompileError};
pub use expr::{Condition, Expr, StepOutputRef};
pub use frontend::{
    decode_registry_snapshot, decode_workflow_definition, parse_registry_snapshot_yaml,
    parse_workflow_yaml,
};
pub use refs::{Version, VersionedRef};
pub use registry::RegistrySnapshot;
pub use schema::{SchemaNode, StaticType};
