//! RCP-06 (synvelis#295) — render availability / policy presentation
//! projection, headless-first slice.
//!
//! This module is the *single display vocabulary* every surface
//! (GPUI `ViewSpec`, headless CLI output, Workflow/Script receipts)
//! consumes for render state:
//!
//! - [`RenderReason`] — stable machine reason codes
//!   (`Source/*`, `State/*`, `Capability/*`, `Boundary/*`,
//!   `Policy/*`, `Job/*`, `Render/*`) shared verbatim between UI
//!   labels and CLI/diagnostic output. Codes are contract; labels
//!   are prose — surfaces must never parse labels.
//! - The five presentation axes — [`Editability`],
//!   [`Executability`], [`RenderValidity`], [`PlaybackQuality`],
//!   [`RuntimeState`] — projected independently so "edit possible /
//!   final render stale / preview" is representable without
//!   collapsing into one status color.
//! - View models — [`RenderNodePresentation`], [`RenderSelectionView`],
//!   [`BranchDiffView`], [`RenderPolicyView`], [`RenderRequestPreview`],
//!   [`RenderJobView`] — pure projections over the RCP-01..RCP-04
//!   authority types (`synvelis-render`, `synvelis-player`). Nothing
//!   here mutates project, render, policy, grant, or job truth.
//! - [`render_request_report`] — the *only* lowering from a surface
//!   action to the authority [`TriggerReport`] contract every entry
//!   surface (GUI/CLI/Workflow/Script) shares (RCP-POL-07). It
//!   fail-closes on missing grants for publish/remote/demand-class
//!   actions.
//!
//! Non-goals: no second catalog/grant/job/project authority, no GPUI
//! or OS types, no media-secret disclosure on read paths, no
//! Workflow/Script/Luau/Rhai surface implementation (other leases —
//! they consume the same typed contracts this module projects).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use synvelis_player::rcp::{
    ArtifactEligibility, ExecutionIntent, ExecutionSelection, PluginVerdict, RcpNodeId,
    ReproducibilityClass, SelectionVerdict, Substitution, UnresolvedNode, UnresolvedReason,
};
use synvelis_project::{ContentDigest, ProjectRevision, SnapshotIdentity};
use synvelis_render::artifact::lookup::LookupOutcome;
use synvelis_render::policy::{
    CommitCapability, CompletionRequirement, EffectivePolicy, FieldProvenance, PolicyField,
    PolicyLayer, RenderPolicy, RenderScope, RenderTrigger, TriggerEvent, TriggerReport,
};
use synvelis_render::recipe::{
    RecipeKey, RecipeResolution, RenderBoundary, UnknownCause, UnsupportedReason,
};
use synvelis_render::render_job::{
    BoundaryResolution, CompletenessGapKind, CompletenessReport, CompletenessVerdict, EntrySurface,
    GatewayActionRef, GrantRef, JobFailure, JobFailureReason, JobPurpose, JobReceipt, JobRecord,
    ReceiptKind, RenderDemand, RenderJobKey, RenderJobState, RenderSelection,
};

// ---------------------------------------------------------------------------
// reason-code vocabulary — the UI/CLI shared diagnostic contract
// ---------------------------------------------------------------------------

/// Diagnostic category — the first segment of [`RenderReason::code`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReasonCategory {
    /// Source/asset availability.
    Source,
    /// Plugin/process state evidence.
    State,
    /// Capability/identity/permission facts.
    Capability,
    /// Render boundary / substitution decisions.
    Boundary,
    /// Render policy decisions.
    Policy,
    /// Render job lifecycle failures.
    Job,
    /// Recipe/render-resolution vocabulary.
    Render,
}

/// Stable machine reason code — the single vocabulary GUI labels,
/// headless CLI output, and Workflow/Script receipts share (RCP-UX-02).
/// `code()` is the contract string; `label()` is display prose.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RenderReason {
    // capability — plugin identity/availability/permission
    /// No environment entry for the plugin — binary not installed.
    PluginMissing,
    /// Environment denies plugin access (ACL/quarantine/sandbox).
    PluginDenied,
    /// Host does not support the plugin format.
    PluginUnsupportedFormat,
    /// Environment resolved a different version/digest than pinned.
    PluginIdentityDrift,
    /// Advertised port layout differs from the authored declaration.
    PluginPortLayoutMismatch,
    /// Automation/parameter identities do not resolve — semantic
    /// editing is also blocked for this node.
    PluginAutomationIncomplete,
    // state — plugin state evidence
    /// Opaque plugin state cannot be restored (evidence absent).
    PluginStateUnknown,
    /// State exists but the host cannot restore it.
    PluginStateUnsupported,
    // source — asset/content availability
    /// Referenced source content is unavailable.
    SourceUnavailable,
    /// Source content is not verified.
    SourceUnverified,
    /// External asset is not pinned to verified content.
    AssetUnpinned,
    /// Asset digest is not a strong content address.
    AssetWeakDigest,
    // boundary — substitution/selection decisions
    /// No verified artifact covers the node and skipping is not
    /// permitted.
    NoEligibleSubstitution,
    /// Every covering candidate failed cutset validation.
    CutsetRejected,
    /// Selection failed closed — required outputs unsupplied.
    SelectionBlocked,
    /// Selection succeeded with explicit non-equivalence marks.
    NonEquivalentPreview,
    // render — recipe/artifact validity vocabulary
    /// No artifact exists or is reachable.
    ArtifactMissing,
    /// Artifact exists but verification is incomplete.
    ArtifactUnverified,
    /// Verification failed (hash/manifest/decode contradiction).
    ArtifactCorrupt,
    /// Recipe/context/snapshot changed — revalidation needed.
    ArtifactStale,
    /// Capability/format technically unsupported.
    ArtifactUnsupported,
    /// Access/ACL/trust denied.
    ArtifactDenied,
    /// Undecidable — never treated as valid.
    ArtifactUnknown,
    /// Recipe resolution failed closed on typed unknowns.
    RecipeUnknown,
    /// The contract cannot express the request.
    RecipeUnsupported,
    /// Live (unpinnable) external clock/device participates.
    LiveClock,
    /// Routing graph contains a rejected cycle.
    CycleDetected,
    /// Routing model failed validation.
    InvalidRouting,
    /// Automation scope the contract cannot evaluate.
    UnsupportedAutomationScope,
    /// Render-input adapter rejected the input.
    InputRejected,
    /// Canonicalization failed.
    CanonicalizationFailed,
    /// Snapshot/input schema outside the supported ladder.
    UnsupportedSchema,
    // policy — trigger/grant decisions
    /// The effective policy does not enable this trigger.
    TriggerDisabled,
    /// Commit capability unsupported by the VCS provider.
    CommitUnsupported,
    /// Action requires an authorization grant that is absent.
    GrantRequired,
    // job — render job lifecycle
    /// Remote render/backend use without consent.
    RemoteConsentDenied,
    /// Declared resource budget exceeded.
    BudgetExceeded,
    /// Rate/concurrency quota — retryable when capacity frees.
    QuotaExceeded,
    /// Storage quota/headroom exhausted — retryable.
    InsufficientStorage,
    /// Cooperative cancellation.
    Cancelled,
    /// Worker/render backend failure — retryable.
    WorkerFailure,
    /// OS/permission denial.
    AuthDenied,
    /// Required artifacts incomplete at a `RequireComplete` gate.
    IncompleteArtifacts,
    /// Typed unknown dependency blocked exact resolution.
    UnknownDependency,
    /// The contract cannot express the job request.
    UnsupportedContract,
}

impl RenderReason {
    /// The machine-stable code — identical on every surface.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::PluginMissing => "capability.plugin-missing",
            Self::PluginDenied => "capability.plugin-denied",
            Self::PluginUnsupportedFormat => "capability.plugin-unsupported-format",
            Self::PluginIdentityDrift => "capability.plugin-identity-drift",
            Self::PluginPortLayoutMismatch => "capability.plugin-port-layout-mismatch",
            Self::PluginAutomationIncomplete => "capability.plugin-automation-incomplete",
            Self::PluginStateUnknown => "state.plugin-state-unknown",
            Self::PluginStateUnsupported => "state.plugin-state-unsupported",
            Self::SourceUnavailable => "source.unavailable",
            Self::SourceUnverified => "source.unverified",
            Self::AssetUnpinned => "source.asset-unpinned",
            Self::AssetWeakDigest => "source.asset-weak-digest",
            Self::NoEligibleSubstitution => "boundary.no-eligible-substitution",
            Self::CutsetRejected => "boundary.cutset-rejected",
            Self::SelectionBlocked => "render.selection-blocked",
            Self::NonEquivalentPreview => "render.non-equivalent-preview",
            Self::ArtifactMissing => "render.artifact-missing",
            Self::ArtifactUnverified => "render.artifact-unverified",
            Self::ArtifactCorrupt => "render.artifact-corrupt",
            Self::ArtifactStale => "render.artifact-stale",
            Self::ArtifactUnsupported => "render.artifact-unsupported",
            Self::ArtifactDenied => "render.artifact-denied",
            Self::ArtifactUnknown => "render.artifact-unknown",
            Self::RecipeUnknown => "render.recipe-unknown",
            Self::RecipeUnsupported => "render.recipe-unsupported",
            Self::LiveClock => "render.live-clock",
            Self::CycleDetected => "render.cycle-detected",
            Self::InvalidRouting => "render.invalid-routing",
            Self::UnsupportedAutomationScope => "render.unsupported-automation-scope",
            Self::InputRejected => "render.input-rejected",
            Self::CanonicalizationFailed => "render.canonicalization-failed",
            Self::UnsupportedSchema => "render.unsupported-schema",
            Self::TriggerDisabled => "policy.trigger-disabled",
            Self::CommitUnsupported => "policy.commit-unsupported",
            Self::GrantRequired => "policy.grant-required",
            Self::RemoteConsentDenied => "job.remote-consent-denied",
            Self::BudgetExceeded => "job.budget-exceeded",
            Self::QuotaExceeded => "job.quota-exceeded",
            Self::InsufficientStorage => "job.insufficient-storage",
            Self::Cancelled => "job.cancelled",
            Self::WorkerFailure => "job.worker-failure",
            Self::AuthDenied => "job.auth-denied",
            Self::IncompleteArtifacts => "job.incomplete-artifacts",
            Self::UnknownDependency => "render.unknown-dependency",
            Self::UnsupportedContract => "render.unsupported-contract",
        }
    }

    /// Human display prose — never a parse target.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::PluginMissing => "plugin missing",
            Self::PluginDenied => "plugin access denied",
            Self::PluginUnsupportedFormat => "plugin format unsupported",
            Self::PluginIdentityDrift => "plugin identity drift",
            Self::PluginPortLayoutMismatch => "plugin port layout mismatch",
            Self::PluginAutomationIncomplete => "plugin automation identity incomplete",
            Self::PluginStateUnknown => "plugin state unknown",
            Self::PluginStateUnsupported => "plugin state unrestorable",
            Self::SourceUnavailable => "source unavailable",
            Self::SourceUnverified => "source unverified",
            Self::AssetUnpinned => "asset unpinned",
            Self::AssetWeakDigest => "asset digest not content-addressed",
            Self::NoEligibleSubstitution => "no eligible substitution",
            Self::CutsetRejected => "substitution cutset rejected",
            Self::SelectionBlocked => "selection blocked",
            Self::NonEquivalentPreview => "non-equivalent preview",
            Self::ArtifactMissing => "render artifact missing",
            Self::ArtifactUnverified => "render artifact unverified",
            Self::ArtifactCorrupt => "render artifact corrupt",
            Self::ArtifactStale => "render artifact stale",
            Self::ArtifactUnsupported => "render unsupported",
            Self::ArtifactDenied => "render access denied",
            Self::ArtifactUnknown => "render state unknown",
            Self::RecipeUnknown => "recipe unresolved (unknown dependencies)",
            Self::RecipeUnsupported => "recipe unsupported",
            Self::LiveClock => "live clock cannot be pinned",
            Self::CycleDetected => "routing cycle detected",
            Self::InvalidRouting => "invalid routing model",
            Self::UnsupportedAutomationScope => "automation scope unsupported",
            Self::InputRejected => "render input rejected",
            Self::CanonicalizationFailed => "canonicalization failed",
            Self::UnsupportedSchema => "unsupported schema version",
            Self::TriggerDisabled => "trigger disabled by policy",
            Self::CommitUnsupported => "commit unsupported by provider",
            Self::GrantRequired => "authorization grant required",
            Self::RemoteConsentDenied => "remote render consent denied",
            Self::BudgetExceeded => "resource budget exceeded",
            Self::QuotaExceeded => "concurrency quota exceeded",
            Self::InsufficientStorage => "insufficient storage",
            Self::Cancelled => "cancelled",
            Self::WorkerFailure => "render worker failure",
            Self::AuthDenied => "permission denied",
            Self::IncompleteArtifacts => "required artifacts incomplete",
            Self::UnknownDependency => "unknown dependency",
            Self::UnsupportedContract => "request unsupported by contract",
        }
    }

    /// The diagnostic category — `code()`'s first segment.
    #[must_use]
    pub fn category(self) -> ReasonCategory {
        match self.code().split('.').next() {
            Some("source") => ReasonCategory::Source,
            Some("state") => ReasonCategory::State,
            Some("capability") => ReasonCategory::Capability,
            Some("boundary") => ReasonCategory::Boundary,
            Some("policy") => ReasonCategory::Policy,
            Some("job") => ReasonCategory::Job,
            _ => ReasonCategory::Render,
        }
    }
}

// -- authority → reason mappings (the single projection point) --------

/// Plugin verdict → reason. `Live` has no failure reason.
#[must_use]
pub fn reason_of_verdict(verdict: &PluginVerdict) -> Option<RenderReason> {
    Some(match verdict {
        PluginVerdict::Live => return None,
        PluginVerdict::Missing => RenderReason::PluginMissing,
        PluginVerdict::Denied { .. } => RenderReason::PluginDenied,
        PluginVerdict::UnsupportedFormat => RenderReason::PluginUnsupportedFormat,
        PluginVerdict::IdentityDrift { .. } => RenderReason::PluginIdentityDrift,
        PluginVerdict::PortLayoutMismatch { .. } => RenderReason::PluginPortLayoutMismatch,
        PluginVerdict::StateUnknown => RenderReason::PluginStateUnknown,
        PluginVerdict::StateUnsupported => RenderReason::PluginStateUnsupported,
        PluginVerdict::AutomationIdentityIncomplete => RenderReason::PluginAutomationIncomplete,
    })
}

/// Artifact eligibility → diagnostic reason. Exact/verified states
/// carry no reason; preview-class carries the explicit
/// non-equivalence marker; every failure state maps 1:1.
#[must_use]
pub fn reason_of_eligibility(eligibility: ArtifactEligibility) -> Option<RenderReason> {
    Some(match eligibility {
        ArtifactEligibility::EligibleExact | ArtifactEligibility::VerifiedMedia => return None,
        ArtifactEligibility::EligiblePreview => RenderReason::NonEquivalentPreview,
        ArtifactEligibility::ValidRecipe | ArtifactEligibility::Unverified => {
            RenderReason::ArtifactUnverified
        }
        ArtifactEligibility::Missing => RenderReason::ArtifactMissing,
        ArtifactEligibility::Corrupt => RenderReason::ArtifactCorrupt,
        ArtifactEligibility::Stale => RenderReason::ArtifactStale,
        ArtifactEligibility::Unsupported => RenderReason::ArtifactUnsupported,
        ArtifactEligibility::Unknown => RenderReason::ArtifactUnknown,
    })
}

/// Lookup outcome → reason. Outcome payloads (`Denied`/`Unknown`/
/// `Unsupported` detail strings) stay on the view row; the *code*
/// is the shared contract.
#[must_use]
pub fn reason_of_lookup(outcome: &LookupOutcome) -> Option<RenderReason> {
    Some(match outcome {
        LookupOutcome::EligibleExact { .. } | LookupOutcome::EligiblePreview { .. } => return None,
        LookupOutcome::Stale => RenderReason::ArtifactStale,
        LookupOutcome::Unverified => RenderReason::ArtifactUnverified,
        LookupOutcome::Corrupt => RenderReason::ArtifactCorrupt,
        LookupOutcome::Missing => RenderReason::ArtifactMissing,
        LookupOutcome::Denied { .. } => RenderReason::ArtifactDenied,
        LookupOutcome::Unknown { .. } => RenderReason::ArtifactUnknown,
        LookupOutcome::Unsupported { .. } => RenderReason::ArtifactUnsupported,
    })
}

/// Job failure reason → reason (1:1 vocabulary alignment).
#[must_use]
pub fn reason_of_job_failure(reason: JobFailureReason) -> RenderReason {
    match reason {
        JobFailureReason::PluginMissing => RenderReason::PluginMissing,
        JobFailureReason::StateCaptureUnsupported => RenderReason::PluginStateUnsupported,
        JobFailureReason::SourceUnavailable => RenderReason::SourceUnavailable,
        JobFailureReason::InsufficientStorage => RenderReason::InsufficientStorage,
        JobFailureReason::Cancelled => RenderReason::Cancelled,
        JobFailureReason::WorkerFailure => RenderReason::WorkerFailure,
        JobFailureReason::AuthDenied => RenderReason::AuthDenied,
        JobFailureReason::RemoteConsentDenied => RenderReason::RemoteConsentDenied,
        JobFailureReason::BudgetExceeded => RenderReason::BudgetExceeded,
        JobFailureReason::QuotaExceeded => RenderReason::QuotaExceeded,
        JobFailureReason::UnknownDependency => RenderReason::UnknownDependency,
        JobFailureReason::Unsupported => RenderReason::UnsupportedContract,
        JobFailureReason::IncompleteArtifacts => RenderReason::IncompleteArtifacts,
    }
}

/// Completeness gap kind → reason.
#[must_use]
pub fn reason_of_gap(kind: CompletenessGapKind) -> RenderReason {
    match kind {
        CompletenessGapKind::MissingRender => RenderReason::ArtifactMissing,
        CompletenessGapKind::InvalidArtifact => RenderReason::ArtifactCorrupt,
        CompletenessGapKind::StaleArtifact => RenderReason::ArtifactStale,
        CompletenessGapKind::UnverifiedArtifact => RenderReason::ArtifactUnverified,
        CompletenessGapKind::UnsupportedCapability => RenderReason::ArtifactUnsupported,
        CompletenessGapKind::PluginMissing => RenderReason::PluginMissing,
        CompletenessGapKind::Denied => RenderReason::ArtifactDenied,
        CompletenessGapKind::IncompleteContent => RenderReason::NonEquivalentPreview,
        CompletenessGapKind::UnresolvedDependency => RenderReason::RecipeUnknown,
    }
}

/// Unresolved-node reason → reason.
#[must_use]
pub fn reason_of_unresolved(node: &UnresolvedNode) -> RenderReason {
    match &node.reason {
        UnresolvedReason::NoEligibleSubstitution => RenderReason::NoEligibleSubstitution,
        UnresolvedReason::CutsetRejected => RenderReason::CutsetRejected,
        UnresolvedReason::PluginVerdict(verdict) => {
            reason_of_verdict(verdict).unwrap_or(RenderReason::PluginMissing)
        }
    }
}

/// Unknown-dependency cause → reason.
#[must_use]
pub fn reason_of_unknown(cause: &UnknownCause) -> RenderReason {
    match cause {
        UnknownCause::MissingEntity { .. } => RenderReason::ArtifactMissing,
        UnknownCause::UnresolvedPort { .. } | UnknownCause::UnresolvedProcessor { .. } => {
            RenderReason::RecipeUnknown
        }
        UnknownCause::PluginStateUnknown { .. } => RenderReason::PluginStateUnknown,
        UnknownCause::PluginStateDegraded { .. } => RenderReason::PluginStateUnsupported,
        UnknownCause::UnpinnedExternalAsset { .. } => RenderReason::AssetUnpinned,
        UnknownCause::WeakAssetDigest { .. } => RenderReason::AssetWeakDigest,
        UnknownCause::UnverifiedSource { .. } => RenderReason::SourceUnverified,
        UnknownCause::UnpinnedClock => RenderReason::LiveClock,
        UnknownCause::InvalidValue { .. } => RenderReason::InputRejected,
        UnknownCause::OpaqueDependency { .. } => RenderReason::UnknownDependency,
        UnknownCause::UnsupportedSchema { .. } => RenderReason::UnsupportedSchema,
    }
}

/// Unsupported-reason → reason.
#[must_use]
pub fn reason_of_unsupported(reason: &UnsupportedReason) -> RenderReason {
    match reason {
        UnsupportedReason::LiveClock => RenderReason::LiveClock,
        UnsupportedReason::CycleDetected => RenderReason::CycleDetected,
        UnsupportedReason::InvalidRouting => RenderReason::InvalidRouting,
        UnsupportedReason::UnsupportedAutomationScope => RenderReason::UnsupportedAutomationScope,
        UnsupportedReason::InputRejected { .. } => RenderReason::InputRejected,
        UnsupportedReason::Canonicalization { .. } => RenderReason::CanonicalizationFailed,
    }
}

// ---------------------------------------------------------------------------
// five presentation axes — independent, never collapsed
// ---------------------------------------------------------------------------

/// Semantic editability — whether the node's parameters/semantics can
/// be edited *now*. Missing plugins do NOT block this axis: the
/// project stays the canonical editable source.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Editability {
    /// Semantic edit permitted.
    Editable,
    /// Semantic edit blocked — carries the typed reason.
    Blocked {
        /// Why editing is blocked.
        reason: RenderReason,
    },
}

impl Editability {
    /// Stable axis label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Editable => "edit-possible",
            Self::Blocked { .. } => "edit-blocked",
        }
    }
}

/// Live executability — whether the node can execute *live* on this
/// host right now (plugin present, state restorable, layout matched).
/// Distinct from [`Editability`]: a missing plugin blocks execution
/// but never editing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Executability {
    /// Live execution available.
    Live,
    /// Live execution blocked — carries the typed reason.
    Blocked {
        /// Why live execution is unavailable.
        reason: RenderReason,
    },
}

impl Executability {
    /// Stable axis label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Blocked { .. } => "blocked",
        }
    }
}

/// Render-artifact validity — the RCP-02/RCP-04 eligibility state
/// machine projected for display. `Unknown`/`Unverified` are never
/// presented as valid.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RenderValidity {
    /// Verified artifact satisfies the request fitness.
    Valid,
    /// Only preview-class (non-equivalent) artifact coverage.
    Preview,
    /// Recipe/context/snapshot changed — revalidation needed.
    Stale,
    /// No artifact exists or is reachable.
    Missing,
    /// Verification failed — hash/manifest/decode contradiction.
    Corrupt,
    /// Verification incomplete.
    Unverified,
    /// Capability/format unsupported.
    Unsupported,
    /// Access/ACL/trust denied.
    Denied,
    /// Undecidable — never valid.
    Unknown,
}

impl RenderValidity {
    /// Stable axis label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Valid => "render-valid",
            Self::Preview => "render-preview",
            Self::Stale => "render-stale",
            Self::Missing => "render-missing",
            Self::Corrupt => "render-corrupt",
            Self::Unverified => "render-unverified",
            Self::Unsupported => "render-unsupported",
            Self::Denied => "render-denied",
            Self::Unknown => "render-unknown",
        }
    }
}

/// Maps RCP-04 [`ArtifactEligibility`] onto the display axis.
#[must_use]
pub fn render_validity_of(eligibility: ArtifactEligibility) -> RenderValidity {
    match eligibility {
        ArtifactEligibility::EligibleExact | ArtifactEligibility::VerifiedMedia => {
            RenderValidity::Valid
        }
        ArtifactEligibility::EligiblePreview => RenderValidity::Preview,
        ArtifactEligibility::ValidRecipe | ArtifactEligibility::Unverified => {
            RenderValidity::Unverified
        }
        ArtifactEligibility::Stale => RenderValidity::Stale,
        ArtifactEligibility::Missing => RenderValidity::Missing,
        ArtifactEligibility::Corrupt => RenderValidity::Corrupt,
        ArtifactEligibility::Unsupported => RenderValidity::Unsupported,
        ArtifactEligibility::Unknown => RenderValidity::Unknown,
    }
}

/// Maps a [`LookupOutcome`] onto the display axis — the outcome
/// vocabulary is already the Issue #291 contract; this is display
/// projection only, never a re-derivation.
#[must_use]
pub fn render_validity_of_lookup(outcome: &LookupOutcome) -> RenderValidity {
    match outcome {
        LookupOutcome::EligibleExact { .. } => RenderValidity::Valid,
        LookupOutcome::EligiblePreview { .. } => RenderValidity::Preview,
        LookupOutcome::Stale => RenderValidity::Stale,
        LookupOutcome::Unverified => RenderValidity::Unverified,
        LookupOutcome::Corrupt => RenderValidity::Corrupt,
        LookupOutcome::Missing => RenderValidity::Missing,
        LookupOutcome::Denied { .. } => RenderValidity::Denied,
        LookupOutcome::Unknown { .. } => RenderValidity::Unknown,
        LookupOutcome::Unsupported { .. } => RenderValidity::Unsupported,
    }
}

/// Playback quality — what the user can actually *hear*. Preview is
/// never labeled exact; `ExactArtifactReplay` requires an exact-
/// eligible artifact with certified/replayable reproducibility.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PlaybackQuality {
    /// Verified artifact replayed exactly.
    ExactArtifactReplay,
    /// Verified within a defined tolerance.
    VerifiedTolerance,
    /// Preview-class output — explicit non-equivalence.
    Preview,
    /// No playable output.
    Unavailable,
}

impl PlaybackQuality {
    /// Stable axis label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::ExactArtifactReplay => "quality-exact-artifact",
            Self::VerifiedTolerance => "quality-verified-tolerance",
            Self::Preview => "quality-preview",
            Self::Unavailable => "quality-unavailable",
        }
    }
}

/// Projects playback quality from eligibility + reproducibility +
/// preview opt-in. Fail-closed: anything not explicitly exact or
/// preview-eligible is `Unavailable`.
#[must_use]
pub fn playback_quality_of(
    eligibility: Option<ArtifactEligibility>,
    reproducibility: Option<ReproducibilityClass>,
    preview_class: bool,
) -> PlaybackQuality {
    if preview_class {
        return PlaybackQuality::Preview;
    }
    match eligibility {
        Some(ArtifactEligibility::EligibleExact) => match reproducibility {
            Some(
                ReproducibilityClass::BitExactCertified
                | ReproducibilityClass::ReplayOfVerifiedArtifact,
            ) => PlaybackQuality::ExactArtifactReplay,
            Some(ReproducibilityClass::VerifiedWithinTolerance) => {
                PlaybackQuality::VerifiedTolerance
            }
            _ => PlaybackQuality::Preview,
        },
        Some(ArtifactEligibility::EligiblePreview) => PlaybackQuality::Preview,
        _ => PlaybackQuality::Unavailable,
    }
}

/// Render runtime state — where the render pipeline/artifact stands
/// for this node or boundary.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RuntimeState {
    /// Policy/recipe configured, nothing produced yet.
    Configured,
    /// Prepared — resolution/spec built, render not run.
    Prepared,
    /// Verified artifact published.
    Published,
    /// Currently executing live.
    Active,
    /// A last-good artifact exists but the current revision diverges.
    LastGoodMismatch,
}

impl RuntimeState {
    /// Stable axis label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Configured => "runtime-configured",
            Self::Prepared => "runtime-prepared",
            Self::Published => "runtime-published",
            Self::Active => "runtime-active",
            Self::LastGoodMismatch => "runtime-last-good-mismatch",
        }
    }
}

// ---------------------------------------------------------------------------
// node presentation — Signal Flow / Inspector / Mixer overlay input
// ---------------------------------------------------------------------------

/// Accepted substitution overlay for one node — the RCP-04 cutset
/// evidence shown *on top of* the still-visible node entity.
#[derive(Clone, Debug, PartialEq)]
pub struct SubstitutionView {
    /// Artifact identity filling the cutset.
    pub artifact_id: String,
    /// Content hash prefix (`sha256:` + 12 hex).
    pub content_hash: String,
    /// `true` when the artifact is preview-class — non-equivalence
    /// must stay visible.
    pub preview_class: bool,
}

/// All authority evidence one node projection consumes — explicit
/// pins only, nothing re-resolved.
#[derive(Clone, Copy, Default)]
pub struct NodeRenderEvidence<'a> {
    /// Plugin executability verdict (`None` = not a plugin node or
    /// no capability evidence — never treated as `Live`).
    pub verdict: Option<&'a PluginVerdict>,
    /// Render eligibility covering this node, when evaluated.
    pub eligibility: Option<ArtifactEligibility>,
    /// Reproducibility class of the covering artifact.
    pub reproducibility: Option<ReproducibilityClass>,
    /// Accepted substitution (cutset + artifact).
    pub substitution: Option<&'a Substitution>,
    /// Unresolved-node evidence when selection could not execute it.
    pub unresolved: Option<&'a UnresolvedNode>,
    /// `true` when the node sits inside a required live region.
    pub in_live_region: bool,
    /// `true` when a last-good artifact exists for an older revision.
    pub last_good_mismatch: bool,
}

/// Five-axis presentation of one graph node — the overlay input for
/// Signal Flow / Inspector / Mixer / Arrangement surfaces. The node
/// entity itself is never hidden: unavailable plugins stay visible
/// with their blocked axes explained by reason codes.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderNodePresentation {
    /// Semantic node id.
    pub node: RcpNodeId,
    /// Semantic editability axis.
    pub editability: Editability,
    /// Live executability axis.
    pub executability: Executability,
    /// Render-artifact validity axis.
    pub render_validity: RenderValidity,
    /// Playback quality axis.
    pub playback_quality: PlaybackQuality,
    /// Runtime state axis.
    pub runtime_state: RuntimeState,
    /// Substitution overlay, when one was accepted.
    pub substitution: Option<SubstitutionView>,
    /// Deduped reason codes explaining the non-ideal axes.
    pub reasons: Vec<RenderReason>,
}

fn push_reason(reasons: &mut Vec<RenderReason>, reason: Option<RenderReason>) {
    if let Some(reason) = reason
        && !reasons.contains(&reason)
    {
        reasons.push(reason);
    }
}

/// Projects one node's five axes from pinned authority evidence.
/// Deterministic: identical evidence yields identical presentation.
#[must_use]
pub fn project_render_node(
    node: RcpNodeId,
    evidence: NodeRenderEvidence<'_>,
    _intent: &ExecutionIntent,
) -> RenderNodePresentation {
    let mut reasons = Vec::new();

    // Editability: only automation-identity incompleteness blocks
    // semantic editing. Missing/denied/drifted plugins stay editable —
    // the project is the canonical source (RCP-00 invariant).
    let editability = match evidence.verdict {
        Some(PluginVerdict::AutomationIdentityIncomplete) => {
            push_reason(&mut reasons, Some(RenderReason::PluginAutomationIncomplete));
            Editability::Blocked {
                reason: RenderReason::PluginAutomationIncomplete,
            }
        }
        _ => Editability::Editable,
    };

    // Executability: live iff the verdict says Live *or* an accepted
    // substitution covers the node (the region executes via artifact,
    // not via the missing plugin). Everything else fails closed with
    // the most specific reason available.
    let executability = if matches!(evidence.verdict, Some(PluginVerdict::Live))
        || evidence.substitution.is_some()
    {
        Executability::Live
    } else {
        let reason = evidence
            .unresolved
            .map(reason_of_unresolved)
            .or_else(|| evidence.verdict.and_then(reason_of_verdict))
            .or_else(|| evidence.eligibility.and_then(reason_of_eligibility))
            .unwrap_or(RenderReason::ArtifactUnknown);
        push_reason(&mut reasons, Some(reason));
        Executability::Blocked { reason }
    };

    // Render validity: eligibility when evaluated; a substitution
    // without eligibility evidence is preview-class at best; nothing
    // evaluated and nothing substituted is Missing render coverage.
    let render_validity = evidence
        .eligibility
        .map(render_validity_of)
        .or_else(|| {
            evidence.substitution.map(|s| {
                if s.preview_class {
                    RenderValidity::Preview
                } else {
                    RenderValidity::Valid
                }
            })
        })
        .unwrap_or(RenderValidity::Missing);
    push_reason(
        &mut reasons,
        evidence.eligibility.and_then(reason_of_eligibility),
    );

    let playback_quality = playback_quality_of(
        evidence.eligibility,
        evidence.reproducibility,
        evidence.substitution.is_some_and(|s| s.preview_class),
    );

    let runtime_state = if evidence.last_good_mismatch {
        RuntimeState::LastGoodMismatch
    } else if evidence.substitution.is_some() {
        RuntimeState::Published
    } else if matches!(evidence.verdict, Some(PluginVerdict::Live)) {
        RuntimeState::Active
    } else {
        RuntimeState::Configured
    };

    let substitution = evidence.substitution.map(|s| SubstitutionView {
        artifact_id: s.artifact_id.clone(),
        content_hash: digest_prefix_str(&s.content_hash),
        preview_class: s.preview_class,
    });

    RenderNodePresentation {
        node,
        editability,
        executability,
        render_validity,
        playback_quality,
        runtime_state,
        substitution,
        reasons,
    }
}

// ---------------------------------------------------------------------------
// selection-level projection — intent/verdict/pins for status surfaces
// ---------------------------------------------------------------------------

/// Display label for an [`ExecutionIntent`].
#[must_use]
pub fn intent_label(intent: &ExecutionIntent) -> &'static str {
    match intent {
        ExecutionIntent::MainPlayback => "main-playback",
        ExecutionIntent::EditingPreview { .. } => "editing-preview",
        ExecutionIntent::BranchAudition { .. } => "branch-audition",
        ExecutionIntent::OfflineRender { .. } => "offline-render",
        ExecutionIntent::Export => "export",
    }
}

/// Display label for a [`SelectionVerdict`].
#[must_use]
pub fn selection_verdict_label(verdict: &SelectionVerdict) -> &'static str {
    match verdict {
        SelectionVerdict::Exact => "exact",
        SelectionVerdict::NonEquivalent => "non-equivalent-preview",
        SelectionVerdict::Blocked => "blocked",
    }
}

/// The pinned generations a selection depended on — the snapshot pin
/// every surface must display identically (RCP-UX-02).
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionPins {
    /// Resolved-graph snapshot id.
    pub snapshot_id: u64,
    /// Project revision pinned.
    pub project_revision: ProjectRevision,
    /// Graph structure digest pinned.
    pub structure_digest: u64,
    /// Capability generation pinned.
    pub capability_generation: u64,
    /// Render index generation pinned.
    pub render_index_generation: u64,
    /// Digest of the selection decisions.
    pub selection_digest: u64,
}

/// Selection-level presentation — verdict, counts, and pins.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSelectionView {
    /// Intent label.
    pub intent: &'static str,
    /// Verdict label — `non-equivalent-preview` is never "exact".
    pub verdict: &'static str,
    /// Explicit non-equivalence flag carried verbatim.
    pub non_equivalent: bool,
    /// Live region count.
    pub live_regions: usize,
    /// Accepted substitution count.
    pub substitutions: usize,
    /// Skipped node count (preview opt-in only).
    pub skipped: usize,
    /// Unresolved node count.
    pub unresolved: usize,
    /// Degraded-but-tolerated node count.
    pub degraded: usize,
    /// Every unresolved node's reason codes.
    pub unresolved_reasons: Vec<RenderReason>,
    /// Pinned generations.
    pub pins: SelectionPins,
}

/// Projects an [`ExecutionSelection`] for display — read-only,
/// evidence-preserving.
#[must_use]
pub fn project_selection(selection: &ExecutionSelection) -> RenderSelectionView {
    let mut unresolved_reasons: Vec<RenderReason> = selection
        .unresolved
        .iter()
        .map(reason_of_unresolved)
        .collect();
    unresolved_reasons.sort_unstable();
    unresolved_reasons.dedup();
    let receipt = &selection.receipt;
    RenderSelectionView {
        intent: intent_label(&selection.intent),
        verdict: selection_verdict_label(&selection.verdict),
        non_equivalent: selection.non_equivalent,
        live_regions: selection.live_regions.len(),
        substitutions: selection.substitutions.len(),
        skipped: selection.skipped.len(),
        unresolved: selection.unresolved.len(),
        degraded: selection.degraded.len(),
        unresolved_reasons,
        pins: SelectionPins {
            snapshot_id: receipt.snapshot_id,
            project_revision: receipt.project_revision,
            structure_digest: receipt.structure_digest,
            capability_generation: receipt.capability_generation,
            render_index_generation: receipt.render_index_generation,
            selection_digest: receipt.selection_digest,
        },
    }
}

// ---------------------------------------------------------------------------
// snapshot pin display — identical text on every surface
// ---------------------------------------------------------------------------

/// Display form of one pinned snapshot identity — locator + digest +
/// capture generation. [`Self::pin_text`] is the canonical short pin
/// UI and CLI print side by side.
#[derive(Clone, Debug, PartialEq)]
pub struct SnapshotPinView {
    /// Canonical locator serialization.
    pub locator: String,
    /// Full `sha256:` content digest.
    pub digest: String,
    /// Resolver-local capture generation.
    pub capture_generation: u64,
}

impl SnapshotPinView {
    /// Projects a [`SnapshotIdentity`] for display.
    #[must_use]
    pub fn of(identity: &SnapshotIdentity) -> Self {
        Self {
            locator: serde_json::to_string(&identity.locator).unwrap_or_default(),
            digest: identity.digest.to_string(),
            capture_generation: identity.capture_generation,
        }
    }

    /// The canonical pin text — `locator @ sha256:<12-hex>` — shared
    /// verbatim by UI and headless output.
    #[must_use]
    pub fn pin_text(&self) -> String {
        format!("{} @ {}", self.locator, digest_prefix_str(&self.digest))
    }

    /// Digest prefix only (`sha256:` + 12 hex chars).
    #[must_use]
    pub fn digest_prefix(&self) -> String {
        digest_prefix_str(&self.digest)
    }
}

/// `sha256:` + first 12 hex chars of a digest string — the recipe-hash
/// / artifact-hash prefix every surface displays for boundary rows.
#[must_use]
pub fn digest_prefix_str(digest: &str) -> String {
    const PREFIX_LEN: usize = "sha256:".len() + 12;
    digest.chars().take(PREFIX_LEN).collect()
}

/// Prefix of a [`RecipeKey`] digest for boundary rows.
#[must_use]
pub fn recipe_key_prefix(key: &RecipeKey) -> String {
    digest_prefix_str(&key.digest().to_string())
}

/// Prefix of a [`ContentDigest`].
#[must_use]
pub fn content_digest_prefix(digest: &ContentDigest) -> String {
    digest_prefix_str(&digest.to_string())
}

// ---------------------------------------------------------------------------
// Branch Difference Explorer view model
// ---------------------------------------------------------------------------

/// Per-side boundary resolution state for diff rows.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BoundaryResolutionKind {
    /// Recipe resolved — carries key + provenance.
    Resolved,
    /// Resolution failed closed on typed unknowns.
    Unknown,
    /// The contract cannot express the demand.
    Unsupported,
}

impl BoundaryResolutionKind {
    /// Stable display label — never `Debug` formatting.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Unknown => "unknown",
            Self::Unsupported => "unsupported",
        }
    }
}

/// One side (left/right) of a boundary diff row.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundarySideView {
    /// Boundary label (output port summary).
    pub boundary: String,
    /// Resolution kind.
    pub resolution: BoundaryResolutionKind,
    /// Recipe key prefix when resolved.
    pub recipe_key: Option<String>,
    /// Renderer provenance label when resolved.
    pub renderer: Option<String>,
    /// Reason codes explaining non-resolved states.
    pub reasons: Vec<RenderReason>,
}

/// One changed-parameter row inside a boundary diff.
#[derive(Clone, Debug, PartialEq)]
pub struct ChangedParameterRow {
    /// Parameter/semantic path.
    pub path: String,
    /// Left value display.
    pub left: String,
    /// Right value display.
    pub right: String,
}

/// One boundary row of the Branch Difference Explorer.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundaryDiffRow {
    /// Demand index shared by both sides.
    pub index: usize,
    /// Left-side projection.
    pub left: BoundarySideView,
    /// Right-side projection.
    pub right: BoundarySideView,
    /// Changed parameters within this boundary.
    pub changed_parameters: Vec<ChangedParameterRow>,
    /// Missing plugins affecting this boundary.
    pub missing_plugins: Vec<String>,
    /// Port/state uncertainty notes (typed reasons already on sides;
    /// this carries free-form cutset/port evidence).
    pub uncertainty: Vec<String>,
    /// Coverage label (`covered`/`total` demands resolved).
    pub coverage: String,
    /// Non-equivalent preview markers for this boundary.
    pub preview_markers: Vec<String>,
}

/// Semantic (non-audio) diff row — used when source audio is
/// unavailable so the explorer still shows *what* changed.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticDiffRow {
    /// Semantic path (entity/param/port).
    pub path: String,
    /// Left display.
    pub left: String,
    /// Right display.
    pub right: String,
    /// Change kind.
    pub kind: SemanticChange,
}

/// Semantic change classification.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SemanticChange {
    /// Present only on the right.
    Added,
    /// Present only on the left.
    Removed,
    /// Present on both, values differ.
    Modified,
    /// Cannot be classified — displayed, never guessed.
    Unknown,
}

/// The Branch Difference Explorer view model — pinned A/B revisions,
/// per-boundary recipe/artifact/coverage/provenance/permission
/// evidence, missing-plugin lists, and semantic diffs. Stale
/// waveforms are never presented as current edit results: rows carry
/// validity/reasons, not audio.
#[derive(Clone, Debug, PartialEq)]
pub struct BranchDiffView {
    /// Left pin display.
    pub left_pin: SnapshotPinView,
    /// Right pin display.
    pub right_pin: SnapshotPinView,
    /// Pinned context label (variant set / branch revisions).
    pub context: String,
    /// Boundary diff rows (union of both sides' demands).
    pub rows: Vec<BoundaryDiffRow>,
    /// Union of missing plugins across boundaries.
    pub missing_plugins: Vec<String>,
    /// Semantic diffs (always available — never gated on audio).
    pub semantic_diffs: Vec<SemanticDiffRow>,
    /// Non-equivalent preview markers (selection-level).
    pub preview_markers: Vec<String>,
    /// Warnings (e.g. stale-render selection, undecidable sides).
    pub warnings: Vec<String>,
}

/// One side's inputs to [`project_branch_diff`].
pub struct BranchSideInput<'a> {
    /// Pinned snapshot identity.
    pub pin: &'a SnapshotIdentity,
    /// Demands (boundary + interval) in job order.
    pub demands: &'a [RenderDemand],
    /// Per-demand resolutions in the same order.
    pub resolutions: &'a [BoundaryResolution],
}

/// Inputs to [`project_branch_diff`].
pub struct BranchDiffInput<'a> {
    /// Left (base) side.
    pub left: BranchSideInput<'a>,
    /// Right (compare) side.
    pub right: BranchSideInput<'a>,
    /// Pinned context label (variant/branch revisions).
    pub context: String,
    /// Changed parameters per boundary index (semantic diff input —
    /// computed by the authority layer, displayed verbatim).
    pub changed_parameters: &'a [ChangedParameterRow],
    /// Missing plugin identities across the comparison.
    pub missing_plugins: &'a [String],
    /// Semantic diffs when source audio is unavailable.
    pub semantic_diffs: &'a [SemanticDiffRow],
    /// Selection-level non-equivalence markers.
    pub preview_markers: &'a [String],
    /// Stale-render / mismatch warnings from authority evidence.
    pub warnings: &'a [String],
}

fn boundary_label(boundary: &RenderBoundary) -> String {
    if boundary.ports.is_empty() {
        return "boundary".to_string();
    }
    boundary
        .ports
        .iter()
        .map(|p| format!("{p:?}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn side_view(
    demand: Option<&RenderDemand>,
    resolution: Option<&BoundaryResolution>,
) -> BoundarySideView {
    let boundary = demand.map_or_else(|| "boundary".to_string(), |d| boundary_label(&d.boundary));
    let (kind, recipe_key, renderer, mut reasons) = match resolution {
        Some(BoundaryResolution::Resolved { resolution, .. }) => match &**resolution {
            RecipeResolution::Resolved {
                key, provenance, ..
            } => (
                BoundaryResolutionKind::Resolved,
                Some(recipe_key_prefix(key)),
                provenance.snapshot.as_ref().map(|_| "pinned".to_string()),
                Vec::new(),
            ),
            RecipeResolution::Unknown { dependencies } => (
                BoundaryResolutionKind::Unknown,
                None,
                None,
                dependencies
                    .iter()
                    .map(|d| reason_of_unknown(&d.cause))
                    .collect(),
            ),
            RecipeResolution::Unsupported { reason } => (
                BoundaryResolutionKind::Unsupported,
                None,
                None,
                vec![reason_of_unsupported(reason)],
            ),
        },
        Some(BoundaryResolution::Unknown { dependencies, .. }) => (
            BoundaryResolutionKind::Unknown,
            None,
            None,
            dependencies
                .iter()
                .map(|d| reason_of_unknown(&d.cause))
                .collect(),
        ),
        Some(BoundaryResolution::Unsupported { reason, .. }) => (
            BoundaryResolutionKind::Unsupported,
            None,
            None,
            vec![reason_of_unsupported(reason)],
        ),
        None => (
            BoundaryResolutionKind::Unknown,
            None,
            None,
            vec![RenderReason::ArtifactUnknown],
        ),
    };
    reasons.sort_unstable();
    reasons.dedup();
    BoundarySideView {
        boundary,
        resolution: kind,
        recipe_key,
        renderer,
        reasons,
    }
}

/// Projects the Branch Difference Explorer — pure, deterministic,
/// evidence-preserving. Never fabricates audio/waveform data; rows
/// carry recipe pins, reasons, and semantic diffs only.
#[must_use]
pub fn project_branch_diff(input: BranchDiffInput<'_>) -> BranchDiffView {
    let total = input.left.demands.len().max(input.right.demands.len());
    let mut rows = Vec::with_capacity(total);
    for index in 0..total {
        let left_demand = input.left.demands.get(index);
        let right_demand = input.right.demands.get(index);
        let left = side_view(
            left_demand,
            input.left.resolutions.iter().find(|r| match r {
                BoundaryResolution::Resolved { demand, .. }
                | BoundaryResolution::Unknown { demand, .. }
                | BoundaryResolution::Unsupported { demand, .. } => *demand == index,
            }),
        );
        let right = side_view(
            right_demand,
            input.right.resolutions.iter().find(|r| match r {
                BoundaryResolution::Resolved { demand, .. }
                | BoundaryResolution::Unknown { demand, .. }
                | BoundaryResolution::Unsupported { demand, .. } => *demand == index,
            }),
        );
        let covered = usize::from(matches!(left.resolution, BoundaryResolutionKind::Resolved))
            + usize::from(matches!(right.resolution, BoundaryResolutionKind::Resolved));
        rows.push(BoundaryDiffRow {
            index,
            left,
            right,
            changed_parameters: input
                .changed_parameters
                .iter()
                .filter(|c| c.path.starts_with(&format!("boundary[{index}].")))
                .cloned()
                .collect(),
            missing_plugins: input.missing_plugins.to_vec(),
            uncertainty: Vec::new(),
            coverage: format!("{covered}/2 resolved"),
            preview_markers: input.preview_markers.to_vec(),
        });
    }
    BranchDiffView {
        left_pin: SnapshotPinView::of(input.left.pin),
        right_pin: SnapshotPinView::of(input.right.pin),
        context: input.context,
        rows,
        missing_plugins: input.missing_plugins.to_vec(),
        semantic_diffs: input.semantic_diffs.to_vec(),
        preview_markers: input.preview_markers.to_vec(),
        warnings: input.warnings.to_vec(),
    }
}

// ---------------------------------------------------------------------------
// Render Policy UI projection
// ---------------------------------------------------------------------------

/// Provider-side availability for a trigger — `OnCommit` is the only
/// trigger gated on an external capability today.
#[derive(Clone, Debug, PartialEq)]
pub enum TriggerProvider {
    /// Provider capability available.
    Available,
    /// Provider cannot support the trigger — same typed reason the
    /// CLI/headless surface reports as `Unsupported`.
    Unsupported {
        /// Shared reason code.
        reason: RenderReason,
        /// Provider detail prose.
        detail: String,
    },
}

/// One trigger row — effective enablement *and* provider availability
/// are separate facts (policy may enable `OnCommit` while the VCS
/// provider cannot commit).
#[derive(Clone, Debug, PartialEq)]
pub struct TriggerRow {
    /// The trigger.
    pub trigger: RenderTrigger,
    /// Stable trigger label.
    pub label: &'static str,
    /// Effective enablement under the resolved policy.
    pub enabled: bool,
    /// `Manual` is unconditionally enabled — display distinguishes it.
    pub unconditional: bool,
    /// Provider availability when the trigger needs one.
    pub provider: Option<TriggerProvider>,
    /// Why the row is not usable end-to-end, when it isn't.
    pub blocked_reason: Option<RenderReason>,
}

/// Display label for a [`RenderTrigger`].
#[must_use]
pub fn trigger_label(trigger: RenderTrigger) -> &'static str {
    match trigger {
        RenderTrigger::Manual => "manual",
        RenderTrigger::OnSave => "on-save",
        RenderTrigger::OnCommit => "on-commit",
        RenderTrigger::OnShare => "on-share",
        RenderTrigger::OnDemand => "on-demand",
    }
}

/// Display label for a [`RenderScope`].
#[must_use]
pub fn scope_label(scope: RenderScope) -> &'static str {
    match scope {
        RenderScope::PlaybackCompatibility => "playback-compatibility",
        RenderScope::CollaborativeEditing => "collaborative-editing",
        RenderScope::ExplicitBoundaries => "explicit-boundaries",
        RenderScope::MixSnapshot => "mix-snapshot",
    }
}

/// Display label for [`CompletionRequirement`].
#[must_use]
pub fn completion_label(completion: CompletionRequirement) -> &'static str {
    match completion {
        CompletionRequirement::BestEffort => "best-effort",
        CompletionRequirement::RequireComplete => "require-complete",
    }
}

/// Display label for [`PolicyField`].
#[must_use]
pub fn policy_field_label(field: PolicyField) -> &'static str {
    match field {
        PolicyField::Triggers => "triggers",
        PolicyField::Scope => "scope",
        PolicyField::QualityProfile => "quality-profile",
        PolicyField::Completion => "completion",
        PolicyField::ResourceBudget => "resource-budget",
        PolicyField::Retention => "retention",
    }
}

/// Display label for [`PolicyLayer`].
#[must_use]
pub fn policy_layer_label(layer: PolicyLayer) -> &'static str {
    match layer {
        PolicyLayer::ProjectDeclared => "project-declared",
        PolicyLayer::UserLocalOverride => "user-local-override",
        PolicyLayer::OperationIntent => "operation-intent",
    }
}

/// One provenance row — which layer supplied the adopted value.
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyFieldView {
    /// Field label.
    pub field: &'static str,
    /// Adopted-layer label.
    pub layer: &'static str,
}

/// Commit-capability display — `Unsupported` carries the same reason
/// code the CLI surface reports.
#[derive(Clone, Debug, PartialEq)]
pub enum CommitView {
    /// Provider can commit.
    Supported,
    /// Provider cannot commit — typed reason + detail.
    Unsupported {
        /// Shared reason code (`policy.commit-unsupported`).
        reason: RenderReason,
        /// Provider detail.
        detail: String,
    },
    /// Commit capability was not probed.
    Unknown,
}

/// The Render Policy UI projection — triggers (with provider
/// availability), scope, completeness, quality profile, budgets,
/// retention, per-field provenance, and the commit capability. The
/// `recipe_invariant` flag documents RCP-POL-02: trigger-only edits
/// never invalidate recipe keys/cache.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderPolicyView {
    /// All five triggers with effective enablement.
    pub triggers: Vec<TriggerRow>,
    /// Effective scope label.
    pub scope: &'static str,
    /// Effective completion requirement label.
    pub completion: &'static str,
    /// Quality profile display (`profile@rev digest-prefix`).
    pub quality_profile: String,
    /// Resource budget summary.
    pub resource_budget: String,
    /// Retention summary.
    pub retention: String,
    /// Per-field adopted-layer provenance.
    pub provenance: Vec<PolicyFieldView>,
    /// Commit capability display.
    pub commit: CommitView,
    /// Versioned resolution digest prefix (job-key component).
    pub resolution_digest: String,
    /// `true` — trigger-only policy edits leave every `RecipeKey`
    /// untouched (RCP-POL-02); displayed so users see cache is safe.
    pub recipe_invariant: bool,
}

const ALL_TRIGGERS: [RenderTrigger; 5] = [
    RenderTrigger::Manual,
    RenderTrigger::OnSave,
    RenderTrigger::OnCommit,
    RenderTrigger::OnShare,
    RenderTrigger::OnDemand,
];

/// Projects the effective policy + commit capability for display.
/// `policy` is the resolved [`EffectivePolicy`] — the GUI never
/// re-resolves layers itself.
#[must_use]
pub fn project_render_policy(
    effective: &EffectivePolicy,
    commit: &CommitCapability,
) -> RenderPolicyView {
    let policy: &RenderPolicy = &effective.policy;
    let commit_view = match commit {
        CommitCapability::Supported => CommitView::Supported,
        CommitCapability::Unsupported { reason } => CommitView::Unsupported {
            reason: RenderReason::CommitUnsupported,
            detail: reason.clone(),
        },
    };
    let triggers = ALL_TRIGGERS
        .iter()
        .map(|&trigger| {
            let enabled = policy.trigger_enabled(trigger);
            let unconditional = trigger == RenderTrigger::Manual;
            let provider = (trigger == RenderTrigger::OnCommit).then(|| match commit {
                CommitCapability::Supported => TriggerProvider::Available,
                CommitCapability::Unsupported { reason } => TriggerProvider::Unsupported {
                    reason: RenderReason::CommitUnsupported,
                    detail: reason.clone(),
                },
            });
            let blocked_reason = match (enabled, &provider) {
                (false, _) => Some(RenderReason::TriggerDisabled),
                (true, Some(TriggerProvider::Unsupported { reason, .. })) => Some(*reason),
                _ => None,
            };
            TriggerRow {
                trigger,
                label: trigger_label(trigger),
                enabled,
                unconditional,
                provider,
                blocked_reason,
            }
        })
        .collect();
    let provenance = effective
        .provenance
        .iter()
        .map(|FieldProvenance { field, layer }| PolicyFieldView {
            field: policy_field_label(*field),
            layer: policy_layer_label(*layer),
        })
        .collect();
    let budget = &policy.resource_budget;
    RenderPolicyView {
        triggers,
        scope: scope_label(policy.scope),
        completion: completion_label(policy.completion),
        quality_profile: format!(
            "{}@{} {}",
            policy.quality_profile.profile,
            policy.quality_profile.revision,
            content_digest_prefix(&policy.quality_profile.digest),
        ),
        resource_budget: format!(
            "cpu {}ms / wall {}ms / storage {}B / jobs {}",
            budget.max_cpu_ms,
            budget.max_wall_ms,
            budget.max_storage_bytes,
            budget.max_concurrent_jobs,
        ),
        retention: format!(
            "protect-referenced={} ttl={}",
            policy.retention.protect_referenced,
            policy
                .retention
                .unreferenced_ttl_ticks
                .map_or_else(|| "indefinite".to_string(), |t| t.to_string()),
        ),
        provenance,
        commit: commit_view,
        resolution_digest: content_digest_prefix(&effective.resolution_digest),
        recipe_invariant: true,
    }
}

// ---------------------------------------------------------------------------
// Render request preview
// ---------------------------------------------------------------------------

/// Action vocabulary — the typed candidates every surface offers.
/// These are *display/projection* ids; the authority lowering is
/// [`render_request_report`] / [`render_job_command`]. CLI/Workflow/
/// Script surfaces must reuse these exact ids so Target/Grant/Receipt
/// resolution is identical (RCP-UX-04).
pub const ACTION_RENDER_PLAN: &str = "synvelis.render.plan";
/// Read-only artifact/job inspection action id.
pub const ACTION_RENDER_INSPECT: &str = "synvelis.render.inspect";
/// Submit a render job action id.
pub const ACTION_RENDER_REQUEST: &str = "synvelis.render.request";
/// Cancel a render job action id (idempotent on [`RenderJobKey`]).
pub const ACTION_RENDER_CANCEL: &str = "synvelis.render.cancel";
/// Retry a render job action id (idempotent on [`RenderJobKey`]).
pub const ACTION_RENDER_RETRY: &str = "synvelis.render.retry";
/// Verify artifact integrity action id.
pub const ACTION_RENDER_VERIFY: &str = "synvelis.render.verify";
/// Attach a job's artifacts action id.
pub const ACTION_RENDER_ATTACH: &str = "synvelis.render.attach";
/// Publish artifacts action id — requires a grant.
pub const ACTION_RENDER_PUBLISH: &str = "synvelis.render.publish";
/// Remote render action id — requires a grant.
pub const ACTION_RENDER_REMOTE: &str = "synvelis.render.remote";

/// Action class — determines grant requirements and lowering path.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RenderActionKind {
    /// Read-only inspection — never mints work, never needs grant.
    Inspect,
    /// Read-only plan/preview resolution.
    Plan,
    /// Submit a render job (`TriggerReport` lowering).
    Request,
    /// Cancel a running job — idempotent on [`RenderJobKey`].
    Cancel,
    /// Retry a failed job — idempotent on [`RenderJobKey`].
    Retry,
    /// Re-verify artifact media — read-only.
    Verify,
    /// Attach external manifest post-publish — idempotent on key.
    Attach,
    /// Publish/share artifacts — requires grant.
    Publish,
    /// Remote-backend render — requires grant + consent.
    Remote,
}

/// `true` when `action_id` is a grant-requiring action — publish,
/// remote render (adopt is RCP-05 scope, owned elsewhere).
#[must_use]
pub fn render_action_requires_grant(action_id: &str) -> bool {
    matches!(action_id, ACTION_RENDER_PUBLISH | ACTION_RENDER_REMOTE)
}

/// One action candidate row — executable only when every gate
/// (capability, grant, target, revision) is satisfied; otherwise
/// `disabled_reason` explains why (fail-closed display).
#[derive(Clone, Debug, PartialEq)]
pub struct RenderActionCandidate {
    /// Stable action id (see `ACTION_RENDER_*`).
    pub id: &'static str,
    /// Display label.
    pub label: &'static str,
    /// Action class.
    pub kind: RenderActionKind,
    /// Whether the surface may offer this as an executable control.
    pub executable: bool,
    /// Why it is not executable, when it isn't.
    pub disabled_reason: Option<RenderReason>,
    /// Whether the action requires an authorization grant.
    pub requires_grant: bool,
}

fn candidate(
    id: &'static str,
    label: &'static str,
    kind: RenderActionKind,
    executable: bool,
    disabled_reason: Option<RenderReason>,
) -> RenderActionCandidate {
    RenderActionCandidate {
        id,
        label,
        kind,
        executable,
        disabled_reason,
        requires_grant: render_action_requires_grant(id),
    }
}

/// Action candidates for a render *request* preview — plan/inspect
/// are always readable; request needs a lawful selection; publish/
/// remote need grants.
#[must_use]
pub fn render_request_candidates(
    selection_ready: bool,
    grant: Option<&GrantRef>,
    remote_requested: bool,
) -> Vec<RenderActionCandidate> {
    let has_grant = grant.is_some();
    vec![
        candidate(
            ACTION_RENDER_PLAN,
            "Plan",
            RenderActionKind::Plan,
            true,
            None,
        ),
        candidate(
            ACTION_RENDER_INSPECT,
            "Inspect",
            RenderActionKind::Inspect,
            true,
            None,
        ),
        candidate(
            ACTION_RENDER_REQUEST,
            "Request render",
            RenderActionKind::Request,
            selection_ready,
            (!selection_ready).then_some(RenderReason::SelectionBlocked),
        ),
        candidate(
            ACTION_RENDER_PUBLISH,
            "Publish artifacts",
            RenderActionKind::Publish,
            has_grant,
            (!has_grant).then_some(RenderReason::GrantRequired),
        ),
        candidate(
            ACTION_RENDER_REMOTE,
            "Remote render",
            RenderActionKind::Remote,
            has_grant && remote_requested,
            if has_grant {
                (!remote_requested).then_some(RenderReason::TriggerDisabled)
            } else {
                Some(RenderReason::GrantRequired)
            },
        ),
    ]
}

/// Action candidates for one job row — cancel/retry/attach are
/// idempotent on the job key; verify is read-only.
#[must_use]
pub fn render_job_candidates(record: &JobRecord) -> Vec<RenderActionCandidate> {
    let active = !record.state.is_terminal() && !record.cancel_requested;
    let retryable = matches!(record.state, RenderJobState::Failed)
        && record.failure.as_ref().is_some_and(JobFailure::retryable);
    let attachable = matches!(
        record.state,
        RenderJobState::Published | RenderJobState::Partial
    ) && !record.external_manifests.is_empty();
    vec![
        candidate(
            ACTION_RENDER_CANCEL,
            "Cancel",
            RenderActionKind::Cancel,
            active,
            (!active).then_some(RenderReason::Cancelled),
        ),
        candidate(
            ACTION_RENDER_RETRY,
            "Retry",
            RenderActionKind::Retry,
            retryable,
            (!retryable).then_some(RenderReason::WorkerFailure),
        ),
        candidate(
            ACTION_RENDER_ATTACH,
            "Attach manifests",
            RenderActionKind::Attach,
            attachable,
            (!attachable).then_some(RenderReason::ArtifactMissing),
        ),
        candidate(
            ACTION_RENDER_VERIFY,
            "Verify",
            RenderActionKind::Verify,
            true,
            None,
        ),
    ]
}

/// One boundary candidate row in the request preview.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundaryCandidateView {
    /// Demand index.
    pub index: usize,
    /// Boundary label.
    pub boundary: String,
    /// Frame interval display (`start..end`).
    pub interval: String,
    /// Resolution kind.
    pub resolution: BoundaryResolutionKind,
    /// Recipe key prefix when resolved.
    pub recipe_key: Option<String>,
    /// Reason codes for non-resolved states.
    pub reasons: Vec<RenderReason>,
}

/// The render request preview — candidate boundaries, expected size,
/// coverage, missing plugins, owner-machine requirement, action
/// candidates, and invalidation reason. Read-only: previewing never
/// submits work.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderRequestPreview {
    /// Pinned snapshot display.
    pub pin: SnapshotPinView,
    /// Purpose label.
    pub purpose: &'static str,
    /// Boundary candidates.
    pub boundaries: Vec<BoundaryCandidateView>,
    /// Covered/total demand count.
    pub coverage: String,
    /// Expected output size estimate when declared.
    pub expected_bytes: Option<u64>,
    /// Missing plugins affecting this request.
    pub missing_plugins: Vec<String>,
    /// `true` when execution requires the owner's machine (remote
    /// backend or owner-only plugin) — displayed, never implied.
    pub requires_owner_machine: bool,
    /// Invalidation reason when the preview is stale/blocked.
    pub invalidation: Option<RenderReason>,
    /// Action candidates with executable gating.
    pub actions: Vec<RenderActionCandidate>,
}

/// Display label for [`JobPurpose`].
#[must_use]
pub fn job_purpose_label(purpose: &JobPurpose) -> &'static str {
    match purpose {
        JobPurpose::Scoped { .. } => "scoped",
        JobPurpose::Share { .. } => "share",
        JobPurpose::Selection => "selection",
    }
}

/// Projects a request preview from pinned selection + per-demand
/// resolutions. `missing_plugins`/`expected_bytes`/
/// `requires_owner_machine`/`invalidation` are authority-supplied
/// evidence — the projection never guesses them.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn project_render_request(
    selection: &RenderSelection,
    resolutions: &[BoundaryResolution],
    missing_plugins: &[String],
    expected_bytes: Option<u64>,
    requires_owner_machine: bool,
    invalidation: Option<RenderReason>,
    grant: Option<&GrantRef>,
    remote_requested: bool,
) -> RenderRequestPreview {
    let total = selection.demands.len();
    let mut covered = 0usize;
    let boundaries = selection
        .demands
        .iter()
        .enumerate()
        .map(|(index, demand)| {
            let resolution = resolutions.iter().find(|r| match r {
                BoundaryResolution::Resolved { demand: d, .. }
                | BoundaryResolution::Unknown { demand: d, .. }
                | BoundaryResolution::Unsupported { demand: d, .. } => *d == index,
            });
            let view = side_view(Some(demand), resolution);
            if view.resolution == BoundaryResolutionKind::Resolved {
                covered += 1;
            }
            BoundaryCandidateView {
                index,
                boundary: view.boundary,
                interval: format!("{}..{}", demand.interval.start, demand.interval.end),
                resolution: view.resolution,
                recipe_key: view.recipe_key,
                reasons: view.reasons,
            }
        })
        .collect();
    let selection_ready = covered == total && total > 0 && invalidation.is_none();
    RenderRequestPreview {
        pin: SnapshotPinView {
            locator: selection.snapshot.locator_key(),
            digest: selection.snapshot.identity.digest.to_string(),
            capture_generation: selection.snapshot.identity.capture_generation,
        },
        purpose: "selection",
        boundaries,
        coverage: format!("{covered}/{total} demands resolved"),
        expected_bytes,
        missing_plugins: missing_plugins.to_vec(),
        requires_owner_machine,
        invalidation,
        actions: render_request_candidates(selection_ready, grant, remote_requested),
    }
}

// ---------------------------------------------------------------------------
// render job view model — monitor/status surface
// ---------------------------------------------------------------------------

/// Display label for [`RenderJobState`].
#[must_use]
pub fn job_state_label(state: RenderJobState) -> &'static str {
    match state {
        RenderJobState::Resolve => "resolve",
        RenderJobState::Queue => "queue",
        RenderJobState::Preparing => "preparing",
        RenderJobState::Rendering => "rendering",
        RenderJobState::Verifying => "verifying",
        RenderJobState::Published => "published",
        RenderJobState::Partial => "partial",
        RenderJobState::Failed => "failed",
        RenderJobState::Cancelled => "cancelled",
    }
}

/// One completeness gap row.
#[derive(Clone, Debug, PartialEq)]
pub struct CompletenessGapView {
    /// Recipe key prefix when known.
    pub recipe_key: Option<String>,
    /// Boundary label.
    pub boundary: String,
    /// Gap kind label.
    pub kind: &'static str,
    /// Shared reason code.
    pub reason: RenderReason,
    /// Detail prose.
    pub detail: String,
}

/// Completeness verdict display.
#[derive(Clone, Debug, PartialEq)]
pub struct CompletenessView {
    /// `complete` / `incomplete` — `Partial` is never "complete".
    pub verdict: &'static str,
    /// Typed gaps.
    pub gaps: Vec<CompletenessGapView>,
}

/// Job failure display.
#[derive(Clone, Debug, PartialEq)]
pub struct JobFailureView {
    /// Shared reason code.
    pub reason: RenderReason,
    /// Reason label.
    pub label: &'static str,
    /// Whether retry may succeed.
    pub retryable: bool,
    /// Detail prose.
    pub detail: String,
}

/// Receipt-tail row — the independent render-job receipt stream,
/// never mixed with save/VCS receipts.
#[derive(Clone, Debug, PartialEq)]
pub struct JobReceiptView {
    /// Sequence within the job's receipt log.
    pub seq: u64,
    /// Receipt kind label.
    pub kind: String,
}

/// One render job's presentation — key summary, state, receipts,
/// completeness, failure, and gated action candidates.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderJobView {
    /// Job key summary (`action / purpose / grant / pin-prefix`).
    pub key_summary: String,
    /// Current state label.
    pub state: RenderJobState,
    /// State label string.
    pub state_label: &'static str,
    /// Attempt counter.
    pub attempt: u32,
    /// Environmental pending reason when queued.
    pub pending_reason: Option<RenderReason>,
    /// Cancel requested but not landed.
    pub cancel_requested: bool,
    /// Crash observed — resumable.
    pub crashed: bool,
    /// Terminal failure display.
    pub failure: Option<JobFailureView>,
    /// Completeness display.
    pub completeness: Option<CompletenessView>,
    /// Published/reused artifact ids.
    pub artifacts: Vec<String>,
    /// Trigger provenance labels.
    pub triggers: Vec<String>,
    /// Entry surface label.
    pub entry: &'static str,
    /// Receipt tail (newest last, bounded).
    pub receipts: Vec<JobReceiptView>,
    /// Action candidates with executable gating.
    pub actions: Vec<RenderActionCandidate>,
}

/// Entry surface label.
#[must_use]
pub fn entry_surface_label(surface: EntrySurface) -> &'static str {
    match surface {
        EntrySurface::Gui => "gui",
        EntrySurface::HeadlessCli => "headless-cli",
        EntrySurface::Workflow => "workflow",
        EntrySurface::Script => "script",
    }
}

fn receipt_kind_label(kind: &ReceiptKind) -> String {
    match kind {
        ReceiptKind::Transition { from, to } => {
            format!(
                "transition:{}->{}",
                job_state_label(*from),
                job_state_label(*to)
            )
        }
        ReceiptKind::Submitted { entry, .. } => {
            format!("submitted:{}", entry_surface_label(*entry))
        }
        ReceiptKind::Coalesced { entry, .. } => {
            format!("coalesced:{}", entry_surface_label(*entry))
        }
        ReceiptKind::CancelRequested => "cancel-requested".to_string(),
        ReceiptKind::Retrying { attempt } => format!("retrying:{attempt}"),
        ReceiptKind::CrashObserved => "crash-observed".to_string(),
        ReceiptKind::Resumed => "resumed".to_string(),
        ReceiptKind::Pending { reason, .. } => {
            format!("pending:{}", reason_of_job_failure(*reason).code())
        }
        ReceiptKind::PublishBlocked { gaps } => format!("publish-blocked:{gaps}"),
        ReceiptKind::ArtifactReady { demand, reused, .. } => {
            format!("artifact-ready:demand{demand}:reused={reused}")
        }
        ReceiptKind::BoundaryFailed {
            demand, failure, ..
        } => {
            format!(
                "boundary-failed:demand{demand}:{}",
                reason_of_job_failure(failure.reason).code()
            )
        }
        ReceiptKind::Terminal { state, .. } => format!("terminal:{}", job_state_label(*state)),
    }
}

fn completeness_view(report: &CompletenessReport) -> CompletenessView {
    CompletenessView {
        verdict: match report.verdict {
            CompletenessVerdict::Complete => "complete",
            CompletenessVerdict::Incomplete => "incomplete",
        },
        gaps: report
            .gaps
            .iter()
            .map(|gap| CompletenessGapView {
                recipe_key: gap.recipe_key.as_ref().map(recipe_key_prefix),
                boundary: format!("{} ports", gap.boundary.ports.len()),
                kind: match gap.kind {
                    CompletenessGapKind::MissingRender => "missing-render",
                    CompletenessGapKind::InvalidArtifact => "invalid-artifact",
                    CompletenessGapKind::StaleArtifact => "stale-artifact",
                    CompletenessGapKind::UnverifiedArtifact => "unverified-artifact",
                    CompletenessGapKind::UnsupportedCapability => "unsupported-capability",
                    CompletenessGapKind::PluginMissing => "plugin-missing",
                    CompletenessGapKind::Denied => "denied",
                    CompletenessGapKind::IncompleteContent => "incomplete-content",
                    CompletenessGapKind::UnresolvedDependency => "unresolved-dependency",
                },
                reason: reason_of_gap(gap.kind),
                detail: gap.detail.clone(),
            })
            .collect(),
    }
}

/// Projects one [`JobRecord`] for display — read-only; cancel/retry/
/// attach candidates carry the job key so duplicate intents coalesce
/// on the same [`RenderJobKey`] (idempotent by contract).
#[must_use]
pub fn project_render_job(record: &JobRecord, receipts: &[JobReceipt]) -> RenderJobView {
    let job_receipts: Vec<&JobReceipt> = receipts.iter().filter(|r| r.job == record.key).collect();
    let receipt_views = job_receipts
        .iter()
        .rev()
        .take(8)
        .map(|r| JobReceiptView {
            seq: r.seq,
            kind: receipt_kind_label(&r.kind),
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    RenderJobView {
        key_summary: format!(
            "{} / {} / {} / {}",
            record.action.0,
            job_purpose_label(&record_key_purpose(record)),
            record
                .grant
                .as_ref()
                .map_or_else(|| "no-grant".to_string(), |g| g.0.clone()),
            content_digest_prefix(&record.key.snapshot_digest),
        ),
        state: record.state,
        state_label: job_state_label(record.state),
        attempt: record.attempt,
        pending_reason: record.pending_reason.map(reason_of_job_failure),
        cancel_requested: record.cancel_requested,
        crashed: record.crashed,
        failure: record.failure.as_ref().map(|f| JobFailureView {
            reason: reason_of_job_failure(f.reason),
            label: reason_of_job_failure(f.reason).label(),
            retryable: f.retryable(),
            detail: f.detail.clone(),
        }),
        completeness: record.completeness.as_ref().map(completeness_view),
        artifacts: record.artifacts.iter().map(|a| a.0.to_string()).collect(),
        triggers: record
            .triggers
            .iter()
            .map(|t| format!("{}:{}", trigger_label(t.trigger), t.detail))
            .collect(),
        entry: entry_surface_label(record.entry),
        receipts: receipt_views,
        actions: render_job_candidates(record),
    }
}

/// The purpose lives inside the key — helper for display.
fn record_key_purpose(record: &JobRecord) -> JobPurpose {
    record.key.purpose.clone()
}

// ---------------------------------------------------------------------------
// view-local state + the single authority lowering
// ---------------------------------------------------------------------------

/// Which render surface a [`RenderViewState`] belongs to.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderSurface {
    /// Render status / policy / node axes surface.
    #[default]
    Status,
    /// Branch Difference Explorer.
    BranchDiff,
    /// Render request preview.
    RequestPreview,
    /// Render job monitor.
    Jobs,
}

/// View-local render state — selections and the *draft* policy intent
/// a user is composing. Never authority: the draft lowers to a
/// [`PolicyPatch`] (operation-intent layer) only through explicit
/// request actions.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RenderViewState {
    /// Owning surface.
    pub surface: RenderSurface,
    /// Selected boundary index (request preview / branch diff).
    pub selected_boundary: Option<usize>,
    /// Selected job index (monitor).
    pub selected_job: Option<usize>,
    /// Draft trigger set override (policy edit in progress).
    pub draft_triggers: Option<BTreeSet<RenderTrigger>>,
    /// Draft completion override.
    pub draft_completion: Option<CompletionRequirement>,
    /// Draft scope override.
    pub draft_scope: Option<RenderScope>,
}

/// View-local actions — mutate [`RenderViewState`] only.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RenderViewAction {
    /// Select a boundary row.
    SelectBoundary {
        /// Row index.
        index: usize,
    },
    /// Select a job row.
    SelectJob {
        /// Row index.
        index: usize,
    },
    /// Toggle a trigger in the policy draft.
    ToggleTrigger {
        /// The trigger.
        trigger: RenderTrigger,
        /// Desired enabled state.
        enabled: bool,
    },
    /// Set the draft completion requirement.
    SetCompletion {
        /// Desired completion.
        completion: CompletionRequirement,
    },
    /// Set the draft scope.
    SetScope {
        /// Desired scope.
        scope: RenderScope,
    },
    /// Clear all draft policy edits.
    ClearDraft,
}

/// Applies a view-local action — never touches authority state.
pub fn apply_render_view_action(state: &mut RenderViewState, action: RenderViewAction) {
    match action {
        RenderViewAction::SelectBoundary { index } => state.selected_boundary = Some(index),
        RenderViewAction::SelectJob { index } => state.selected_job = Some(index),
        RenderViewAction::ToggleTrigger { trigger, enabled } => {
            let draft = state
                .draft_triggers
                .get_or_insert_with(|| ALL_TRIGGERS.iter().copied().collect());
            // Manual is unconditionally enabled — it cannot be toggled off.
            if enabled {
                draft.insert(trigger);
            } else if trigger != RenderTrigger::Manual {
                draft.remove(&trigger);
            }
        }
        RenderViewAction::SetCompletion { completion } => {
            state.draft_completion = Some(completion);
        }
        RenderViewAction::SetScope { scope } => {
            state.draft_scope = Some(scope);
        }
        RenderViewAction::ClearDraft => {
            state.draft_triggers = None;
            state.draft_completion = None;
            state.draft_scope = None;
        }
    }
}

/// Builds the operation-intent [`synvelis_render::policy::PolicyPatch`]
/// from the draft — `None` when no draft exists. This is the *only*
/// path draft edits take toward authority; resolution stays with
/// `resolve_effective_policy`.
#[must_use]
pub fn render_policy_draft_patch(
    state: &RenderViewState,
) -> Option<synvelis_render::policy::PolicyPatch> {
    if state.draft_triggers.is_none()
        && state.draft_completion.is_none()
        && state.draft_scope.is_none()
    {
        return None;
    }
    Some(synvelis_render::policy::PolicyPatch {
        triggers: state.draft_triggers.clone(),
        scope: state.draft_scope,
        quality_profile: None,
        completion: state.draft_completion,
        resource_budget: None,
        retention: None,
    })
}

// -- typed command decode input (surface → authority) ------------------

/// A decoded render-surface command — mirrors the AUTX-07 split:
/// view-local mutations vs. domain requests the composition layer
/// lowers through [`render_request_report`] / orchestrator ports.
#[derive(Clone, Debug, PartialEq)]
pub enum RenderCommand {
    /// View-local mutation — [`RenderViewState`] only.
    ViewLocal(RenderViewAction),
    /// Domain request — the composition layer supplies the pinned
    /// selection/grant and calls [`render_request_report`].
    Request {
        /// Action id (`ACTION_RENDER_*`).
        action: String,
    },
    /// Job control on the selected job — the composition layer
    /// resolves the job key and calls the orchestrator's idempotent
    /// `cancel`/`retry`/attach ports.
    JobControl {
        /// `cancel` / `retry` / `attach`.
        op: JobControlOp,
    },
}

/// Job-control operation vocabulary.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JobControlOp {
    /// Cooperative cancel — idempotent on the job key.
    Cancel,
    /// Retry a retryable failure — idempotent on the job key.
    Retry,
    /// Attach external manifests post-publish — idempotent on the
    /// job key + artifact set.
    Attach,
}

/// Errors from render-surface lowering — fail-closed, typed.
#[derive(Clone, Debug, PartialEq)]
pub enum RenderUiError {
    /// Malformed/unknown request.
    InvalidRequest,
    /// Action is not executable under current evidence.
    NotExecutable {
        /// Shared reason code.
        reason: RenderReason,
    },
    /// Grant-requiring action without a grant.
    GrantRequired {
        /// Action id.
        action: String,
    },
    /// Target/revision mismatch — fail closed.
    Stale,
}

/// The *only* lowering from a surface request to the authority
/// [`TriggerReport`] — every entry surface (GUI/CLI/Workflow/Script/
/// Fake AI fixture) must produce the same `event`/`action`/`grant`
/// triple for identical requests so they resolve to the same
/// [`RenderJobKey`] and coalesce (RCP-POL-07 / RCP-UX-04).
///
/// Fail-closed rules enforced here at the projection boundary:
///
/// - `OnDemand`/`Demand` events always require a grant (authorization
///   is never implicit).
/// - Publish/remote action ids require a grant.
/// - `entry` is receipt provenance only — never part of the job key.
///
/// # Errors
/// [`RenderUiError::GrantRequired`] / [`RenderUiError::InvalidRequest`].
pub fn render_request_report(
    event: TriggerEvent,
    entry: EntrySurface,
    action: GatewayActionRef,
    grant: Option<GrantRef>,
) -> Result<TriggerReport, RenderUiError> {
    let demand_event = matches!(event, TriggerEvent::Demand(_));
    if (demand_event || render_action_requires_grant(&action.0)) && grant.is_none() {
        return Err(RenderUiError::GrantRequired {
            action: action.0.clone(),
        });
    }
    Ok(TriggerReport {
        event,
        entry,
        action,
        grant,
    })
}

/// Lowers a job-control command to its typed form — carries the job
/// key so cancel/retry/attach are idempotent by construction (same
/// key → same operation, never duplicate work). The composition
/// layer resolves *which* key the selected row refers to; this
/// function only validates the op vocabulary.
#[must_use]
pub fn render_job_command(op: JobControlOp, key: RenderJobKey) -> (JobControlOp, RenderJobKey) {
    (op, key)
}

/// Whether a job-state pair permits the control op — display-side
/// gate mirroring the orchestrator's own idempotent semantics
/// (cancel on terminal is a no-op `false`, retry only on retryable
/// failure, attach only post-publish). Used by both spec gating and
/// tests so UI and authority agree.
#[must_use]
pub fn job_control_allowed(op: JobControlOp, record: &JobRecord) -> bool {
    match op {
        JobControlOp::Cancel => !record.state.is_terminal() && !record.cancel_requested,
        JobControlOp::Retry => {
            matches!(record.state, RenderJobState::Failed)
                && record.failure.as_ref().is_some_and(JobFailure::retryable)
        }
        JobControlOp::Attach => {
            matches!(
                record.state,
                RenderJobState::Published | RenderJobState::Partial
            ) && !record.external_manifests.is_empty()
        }
    }
}
