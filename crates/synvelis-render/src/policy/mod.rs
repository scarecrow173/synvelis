//! Render Policy — non-exclusive trigger declarations, scope,
//! quality-profile reference, completion requirement, resource
//! budget, retention, and provenance-tracked three-layer resolution
//! (RCP-03, synvelis#292; canonical contract
//! `docs/specs/render-checkpoint.md` §6).
//!
//! Policy is a *request and caps* document: it never grants
//! CPU/GPU/network/remote permission ([`LocalGrants`] does, at job
//! preflight), and it is never a recipe input — trigger/scope/
//! completion/budget/local-override changes leave every
//! [`crate::recipe::RecipeKey`] untouched (RCP-POL-02).

pub mod model;
pub mod trigger;

pub use model::{
    CompletionRequirement, EffectivePolicy, FieldProvenance, LocalGrants, POLICY_RESOLUTION_DOMAIN,
    POLICY_SCHEMA_VERSION, PolicyField, PolicyLayer, PolicyPatch, RenderPolicy, RenderScope,
    RenderSettingsRef, RenderTrigger, ResourceBudget, RetentionPolicy, resolve_effective_policy,
};
pub use trigger::{
    CommitCapability, CommitEvent, CommitReceipt, DemandAuthorization, DemandAuthorizer,
    DemandProposal, ExternalReceipt, InertReason, JobIntent, PolicyResolver, RoutedTrigger,
    SaveEvent, SaveReceipt, ScopeDemandResolver, ShareRequest, TriggerEvent, TriggerOutcome,
    TriggerProvenance, TriggerReport,
};
