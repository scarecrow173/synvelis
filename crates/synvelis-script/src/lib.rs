//! `synvelis-script` — Gateway capability → [`ScriptApiModel`] projection,
//! runtime binding generation, and Luau type stub generation (SV-SCRIPT-01,
//! synvelis#271).
//!
//! This crate is a **read-only consumer** of the Gateway contract surface:
//! [`synvelis_gateway::CapabilityRegistry`] and
//! [`synvelis_gateway::SchemaRegistry`] remain the single source of truth for
//! capability/schema identity, versions, targets, effects, permissions, and
//! availability. Nothing here registers capabilities, evaluates
//! authorization, mints [`synvelis_gateway::CallerIdentity`] /
//! [`synvelis_gateway::AdapterSeal`] / grants / approvals, or invokes the
//! Gateway.
//!
//! # Pipeline
//!
//! 1. [`snapshot::RegistrySnapshot::capture`] freezes the discovery surface
//!    (`CapabilityRegistry::list`/`describe`/`versions` +
//!    `SchemaRegistry::resolve`) into a versioned, digest-pinned plain-data
//!    snapshot. Serialized snapshots are valid test/fixture input.
//! 2. [`surface::ScriptSurfaceSpec`] declares the script-facing names mapped
//!    to exact [`synvelis_gateway::CapabilityRef`]s.
//! 3. [`model::ScriptApiModel::build`] projects snapshot + surface into one
//!    deterministic model carrying per-entry typed [`model::ScriptVerdict`]s
//!    (unknown capability / unsupported version / disabled / planned /
//!    retired / unresolved schema / unusable name) and visibility.
//! 4. [`luau::LuauStub`] and [`binding::BindingSet`] are both generated from
//!    that same model and carry its digest, so binding/stub drift or
//!    snapshot drift is a typed error, never silent staleness.
//! 5. [`binding::ScriptApiModel::assemble_call`] lowers a typed script call
//!    into an [`binding::InvocationDraft`] — the wire-shaped
//!    `OperationRequest` precursor. The draft still passes through
//!    `Gateway::bind` → schema re-validation → authorization → approval at
//!    invoke time; static types never grant execution.
//!
//! # Analyzer boundary
//!
//! [`analyzer`] defines a thin consumer-side port mirroring the
//! RF-SCRIPT-00 `ScriptAnalyzer` contract shape (definition injection,
//! diagnostics, completion, `source_revision` stale rejection,
//! `Unsupported`/`StaleResult`/`Unavailable` fail-closed vocabulary). No
//! Ruviform or Luau.Analysis dependency lives here; real analyzer
//! integration is tracked separately.

pub mod action;
pub mod analyzer;
pub mod binding;
pub mod canon;
pub mod error;
pub mod luau;
pub mod model;
pub mod schema;
pub mod snapshot;
pub mod surface;

pub use action::{
    ACTION_DEFINITION_SCHEMA_VERSION, ACTION_RECIPE_SCHEMA_VERSION, ActionRecipe, ActionRecipeKind,
    AvailableBackends, BackendRequirement, LanguageId, MAX_RECIPE_MEMBERS, RecipeMember,
    RecipeMemberSpec, RecipeSpec, ScriptActionDefinition,
};
pub use analyzer::{
    AnalyzeRequest, AnalyzeResult, AnalyzerCapabilities, AnalyzerDescriptor, AnalyzerError,
    Completion, CompletionKind, Cursor, DefinitionInjection, Diagnostic, DiagnosticKind,
    DiagnosticLevel, DiagnosticSeverity, ScriptAnalyzer, ScriptBackendVersion, ScriptLanguageId,
    ScriptSource, SourcePosition, SourceRange, SourceRevision, TypeDefinition, WantedCapabilities,
    verify_result_revision,
};
pub use binding::{
    BINDING_SCHEMA_VERSION, BindingEntry, BindingSet, InvocationCall, InvocationDraft,
    ScriptConsistency, ScriptTarget,
};
pub use canon::{canonical_json, digest_of, sha256_hex};
pub use error::{InvocableBlocker, ScriptApiError};
pub use luau::{GeneratedArtifacts, LuauStub, STUB_SCHEMA_VERSION};
pub use model::{
    DescriptorProvenance, MODEL_SCHEMA_VERSION, SchemaBinding, ScriptApiEntry, ScriptApiModel,
    ScriptVerdict,
};
pub use schema::{ScriptField, ScriptSchema};
pub use snapshot::{RegistrySnapshot, SNAPSHOT_SCHEMA_VERSION, SchemaEntry};
pub use surface::{
    NameRejection, SURFACE_SCHEMA_VERSION, ScriptSurfaceEntry, ScriptSurfaceSpec, ScriptVisibility,
};
