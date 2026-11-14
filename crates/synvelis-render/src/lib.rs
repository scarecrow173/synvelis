//! `synvelis-render` — the L2 render slice: frozen render-input
//! snapshot, fail-closed preflight, verified decode-to-memory clip
//! rendering, deterministic master-bus DSP, sequential offline render
//! through Ruviform's `ruviform-audio::render` driver, real PCM WAV
//! output via `ruviform-io-wav`, independent reopen verification and
//! staged safe publish.
//!
//! Ownership boundary (Issue #25): Synvelis owns `RenderSnapshot`,
//! `ExportRequest`/`TailPolicy`/`Insert`/`OutputHooks`, preflight,
//! publish policy and the job model. Generic graph execution,
//! transport scheduling, codec handling and the staged output
//! lifecycle stay in Ruviform — no second sequential engine, no
//! hand-built WAV writer, no processor instance shared with the live
//! `HeadlessDriver`.

pub mod artifact;
pub mod dependency;
pub mod error;
pub mod export;
pub mod invalidation;
pub mod job;
pub mod policy;
pub mod preflight;
pub mod prepare;
pub mod processor;
pub mod recipe;
pub mod render_job;
pub mod snapshot;
pub mod spec;
pub mod verify;

pub use dependency::{
    DependencyClosure, DependencyEdge, DependencyEdgeKind, DependencyNode, ExternalAssetProbe,
    ExternalPin, NoExternalAssets, NoPluginStates, PluginStateEvidence, PluginStateProbe,
    RecipeEnvironment, RecipeRequest, StaticExternalAssets, StaticPluginStates, resolve_recipe,
};
pub use error::{ExportError, PreflightError};
pub use export::{export, export_with_host};
pub use invalidation::{
    ChangedObject, InvalidationPlan, InvalidationReason, classify_invalidation,
};
pub use job::{CapabilityEvidence, ExportJob, ExportOutcome, ExportReceipt, ExportStage, stage_of};
pub use prepare::{DecodedWindow, OpenedSource};
pub use processor::{ClipRenderer, DelayProc, GainProc, OnePoleProc};
pub use recipe::{
    AUDIO_SEMANTICS_VERSION, CANONICALIZATION_VERSION, ClockDependency, DenormalMode,
    FrameInterval, OutputContract, OutputPortContract, PcmFormat, RECIPE_DOMAIN_TAG,
    RECIPE_HASH_ALGORITHM, RECIPE_HASH_DOMAIN, RECIPE_SCHEMA_VERSION, RecipeBranchPin, RecipeKey,
    RecipeRecord, RecipeRecordError, RecipeResolution, RenderBoundary, RenderPrecision,
    RenderRecipeV1, RendererProfile, ResolutionProvenance, TemporalDomain, UnknownCause,
    UnknownDependency, UnsupportedReason, Validity, evaluate_validity, lookup_record,
    recipe_record,
};
pub use ruviform_audio::typed::OfflineRenderSupport;
pub use snapshot::{RenderSnapshot, SnapshotIds};
pub use spec::{
    ExportRequest, Insert, OutputHooks, PcmExpect, PcmExpectRef, PluginStateRecipe, TailPolicy,
};
pub use verify::{VerifiedOutput, reopen_check};
