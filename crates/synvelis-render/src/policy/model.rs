//! Render Policy model — the typed declaration of *when* and *how*
//! non-real-time renders are requested (RCP-03, synvelis#292;
//! canonical contract `docs/specs/render-checkpoint.md` §6).
//!
//! A [`RenderPolicy`] declares a non-exclusive trigger set, a render
//! scope, a quality-profile reference, a completion requirement, a
//! resource budget, and a retention policy. Policy fields are
//! *requests and caps*, never authorization: CPU/GPU/network/remote
//! permission lives in [`LocalGrants`], which is supplied by the
//! local environment and can never be populated from a shared
//! project's declared policy.
//!
//! Policy is deliberately **not** a recipe input: trigger, scope,
//! completion, budget, retention, and local-override changes must
//! leave every [`crate::recipe::RecipeKey`] untouched (RCP-POL-02).
//! Only quality-affecting render settings — resolved through
//! [`crate::policy::RenderSettingsProbe`] into `TemporalDomain` /
//! `RendererProfile` / `OutputContract` — move the key.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use synvelis_project::ContentDigest;

/// Policy document schema version recorded on every [`RenderPolicy`].
pub const POLICY_SCHEMA_VERSION: u32 = 1;

/// Domain separator for the effective-policy resolution digest —
/// the "policy versioned resolution" component of a render job key.
pub const POLICY_RESOLUTION_DOMAIN: &str = "synvelis-render/policy-resolution/v1";

// ---------------------------------------------------------------------------
// declared policy
// ---------------------------------------------------------------------------

/// Non-exclusive render trigger. `Manual` is always enabled — it is
/// listed for completeness of the vocabulary but its presence in a
/// [`RenderPolicy::triggers`] set changes nothing
/// ([`RenderPolicy::effective_triggers`] always contains it).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderTrigger {
    /// Explicit user/operation request — always available.
    Manual,
    /// After a durable Save receipt pins a fixed snapshot.
    OnSave,
    /// After a successful VCS commit receipt pins an immutable
    /// locator; unsupported providers yield typed `Unsupported`.
    OnCommit,
    /// Before share/package publish, gated on completeness.
    OnShare,
    /// Proposal-only until explicit approval/authorization.
    OnDemand,
}

/// The scope a render policy targets — what the produced artifacts
/// are *for*. Scope selects which boundaries a [`crate::render_job`]
/// must cover; it is policy metadata, never recipe identity.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderScope {
    /// Artifacts needed for ordinary playback compatibility.
    PlaybackCompatibility,
    /// Artifacts enabling collaborative editing hand-off.
    CollaborativeEditing,
    /// Caller-declared explicit boundaries only.
    ExplicitBoundaries,
    /// The final mix snapshot boundary set.
    MixSnapshot,
}

/// Share/publish completeness requirement (RCP-POL-05).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompletionRequirement {
    /// Save/commit proceed; incomplete output is explicitly marked
    /// and share is allowed only with the partial disclosure.
    BestEffort,
    /// Fail closed: share/public is blocked until every required
    /// artifact verifies complete.
    RequireComplete,
}

/// Declared resource caps for render jobs — a *budget*, not a grant.
/// Preflight rejects or pends jobs whose estimate exceeds these caps;
/// authorization is decided separately by [`LocalGrants`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceBudget {
    /// Per-job CPU-time ceiling in milliseconds.
    pub max_cpu_ms: u64,
    /// Per-job wall-time ceiling in milliseconds.
    pub max_wall_ms: u64,
    /// Per-job staged+published storage ceiling in bytes.
    pub max_storage_bytes: u64,
    /// Maximum concurrently executing render jobs.
    pub max_concurrent_jobs: u32,
}

impl Default for ResourceBudget {
    /// Fail-closed-but-unobtrusive default: a single in-flight job,
    /// otherwise unbounded per-job caps (real budgets come from
    /// declared policy).
    fn default() -> Self {
        Self {
            max_cpu_ms: u64::MAX,
            max_wall_ms: u64::MAX,
            max_storage_bytes: u64::MAX,
            max_concurrent_jobs: 1,
        }
    }
}

/// Artifact retention declaration — consumed by GC policy later;
/// recorded here so policy edits are versioned with the resolution.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetentionPolicy {
    /// Referenced artifacts are protected from collection.
    pub protect_referenced: bool,
    /// Store-logical ticks an unreferenced artifact is retained;
    /// `None` retains indefinitely.
    pub unreferenced_ttl_ticks: Option<u64>,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            protect_referenced: true,
            unreferenced_ttl_ticks: None,
        }
    }
}

/// Content-addressed reference to the versioned render-settings
/// document that carries quality-affecting parameters. The document
/// itself is resolved by [`crate::policy::RenderSettingsProbe`] into
/// recipe inputs; the policy only pins *which* document applies.
/// Two references are equal only when profile, revision, and digest
/// all match — a name alone never identifies settings.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderSettingsRef {
    /// Stable profile identity (e.g. `"mixdown-default"`).
    pub profile: String,
    /// Document revision claimed by the referral.
    pub revision: u64,
    /// Content digest of the referenced settings document.
    pub digest: ContentDigest,
}

/// The declared render policy — see module docs for the
/// policy-vs-recipe and policy-vs-grant boundaries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderPolicy {
    /// [`POLICY_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Declared automatic triggers — non-exclusive; `Manual` is
    /// always effective regardless of membership.
    pub triggers: BTreeSet<RenderTrigger>,
    /// Render scope.
    pub scope: RenderScope,
    /// Quality-profile reference.
    pub quality_profile: RenderSettingsRef,
    /// Completion requirement for share/public gates.
    pub completion: CompletionRequirement,
    /// Resource caps.
    pub resource_budget: ResourceBudget,
    /// Retention declaration.
    pub retention: RetentionPolicy,
}

impl Default for RenderPolicy {
    /// Conservative default: manual-only triggers, playback scope,
    /// a deterministic placeholder settings ref, `RequireComplete`
    /// (fail closed — incomplete output is never presented as
    /// complete), single-flight budget, referenced-protected
    /// retention.
    fn default() -> Self {
        Self {
            schema_version: POLICY_SCHEMA_VERSION,
            triggers: BTreeSet::from([RenderTrigger::Manual]),
            scope: RenderScope::PlaybackCompatibility,
            quality_profile: RenderSettingsRef {
                profile: "synvelis-render/default".to_owned(),
                revision: 0,
                digest: ContentDigest::sha256(b"synvelis-render/default-quality-settings"),
            },
            completion: CompletionRequirement::RequireComplete,
            resource_budget: ResourceBudget::default(),
            retention: RetentionPolicy::default(),
        }
    }
}

impl RenderPolicy {
    /// The trigger set actually in force — declared triggers plus
    /// `Manual`, which is unconditionally enabled.
    #[must_use]
    pub fn effective_triggers(&self) -> BTreeSet<RenderTrigger> {
        let mut set = self.triggers.clone();
        set.insert(RenderTrigger::Manual);
        set
    }

    /// `true` when `trigger` may fire under this policy.
    #[must_use]
    pub fn trigger_enabled(&self, trigger: RenderTrigger) -> bool {
        trigger == RenderTrigger::Manual || self.triggers.contains(&trigger)
    }
}

// ---------------------------------------------------------------------------
// three-layer resolution with provenance
// ---------------------------------------------------------------------------

/// Which policy layer supplied an adopted value.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyLayer {
    /// The project's declared preference (shared — may describe what
    /// the project wants, never what this user permits).
    ProjectDeclared,
    /// This user's local override — private to the local principal.
    UserLocalOverride,
    /// The operation's explicit intent — highest precedence.
    OperationIntent,
}

/// A policy field that can carry per-layer provenance.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyField {
    /// Trigger set.
    Triggers,
    /// Render scope.
    Scope,
    /// Quality-profile reference.
    QualityProfile,
    /// Completion requirement.
    Completion,
    /// Resource budget.
    ResourceBudget,
    /// Retention policy.
    Retention,
}

/// Per-field record of which layer supplied the adopted value.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldProvenance {
    /// The field.
    pub field: PolicyField,
    /// The layer the value was adopted from.
    pub layer: PolicyLayer,
}

/// A partial policy layer — every field optional; `None` defers to
/// the next-lower-precedence layer (then to the built-in default).
/// This is what project declarations, user overrides, and explicit
/// intents actually exchange; a full [`RenderPolicy`] can be
/// converted via [`PolicyPatch::from_policy`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyPatch {
    /// Trigger set override.
    pub triggers: Option<BTreeSet<RenderTrigger>>,
    /// Scope override.
    pub scope: Option<RenderScope>,
    /// Quality-profile override.
    pub quality_profile: Option<RenderSettingsRef>,
    /// Completion override.
    pub completion: Option<CompletionRequirement>,
    /// Budget override.
    pub resource_budget: Option<ResourceBudget>,
    /// Retention override.
    pub retention: Option<RetentionPolicy>,
}

impl PolicyPatch {
    /// Views a complete policy as a patch (all fields `Some`).
    #[must_use]
    pub fn from_policy(policy: &RenderPolicy) -> Self {
        Self {
            triggers: Some(policy.triggers.clone()),
            scope: Some(policy.scope),
            quality_profile: Some(policy.quality_profile.clone()),
            completion: Some(policy.completion),
            resource_budget: Some(policy.resource_budget),
            retention: Some(policy.retention),
        }
    }
}

/// The resolved effective policy plus the provenance of every field.
/// `resolution_digest` is the versioned-resolution identity used in
/// [`crate::render_job::RenderJobKey`]: identical resolutions always
/// produce identical digests; any adopted-value change moves it.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectivePolicy {
    /// The fully resolved policy.
    pub policy: RenderPolicy,
    /// Per-field adopted-layer record, in [`PolicyField`] order.
    pub provenance: Vec<FieldProvenance>,
    /// Digest over the canonical resolution (domain-separated).
    pub resolution_digest: ContentDigest,
}

impl EffectivePolicy {
    /// `true` when `trigger` may fire under the resolved policy.
    #[must_use]
    pub fn trigger_enabled(&self, trigger: RenderTrigger) -> bool {
        self.policy.trigger_enabled(trigger)
    }
}

/// Resolves the effective policy from the three layers in precedence
/// order: operation explicit intent > user local override > project
/// declared preference > built-in default. Each field independently
/// adopts the highest-precedence `Some` value; the adopting layer is
/// recorded in [`EffectivePolicy::provenance`].
///
/// A project-declared layer can *request* triggers/budgets but can
/// never grant CPU/GPU/network/remote permission — authorization is
/// [`LocalGrants`]' job, evaluated at job preflight, so a shared
/// project's policy cannot force another user's resources
/// (RCP-POL-06).
#[must_use]
pub fn resolve_effective_policy(
    declared: Option<&PolicyPatch>,
    local_override: Option<&PolicyPatch>,
    explicit_intent: Option<&PolicyPatch>,
) -> EffectivePolicy {
    let defaults = RenderPolicy::default();
    let mut provenance = Vec::with_capacity(6);

    // Layers in precedence order (highest first); the first `Some`
    // wins per field. When no layer specifies a field the built-in
    // default is adopted and the lowest-precedence layer is recorded
    // — the default is the project's implicit declaration.
    let layers = [
        (PolicyLayer::OperationIntent, explicit_intent),
        (PolicyLayer::UserLocalOverride, local_override),
        (PolicyLayer::ProjectDeclared, declared),
    ];

    macro_rules! adopt_field {
        ($field:expr, $patch_field:ident, $default:expr) => {{
            let mut value = $default;
            let mut layer = PolicyLayer::ProjectDeclared;
            for (candidate_layer, patch) in layers {
                if let Some(patch) = patch {
                    if let Some(v) = patch.$patch_field.clone() {
                        value = v;
                        layer = candidate_layer;
                        break;
                    }
                }
            }
            provenance.push(FieldProvenance {
                field: $field,
                layer,
            });
            value
        }};
    }

    let triggers = adopt_field!(PolicyField::Triggers, triggers, defaults.triggers.clone());
    let scope = adopt_field!(PolicyField::Scope, scope, defaults.scope);
    let quality_profile = adopt_field!(
        PolicyField::QualityProfile,
        quality_profile,
        defaults.quality_profile.clone()
    );
    let completion = adopt_field!(PolicyField::Completion, completion, defaults.completion);
    let resource_budget = adopt_field!(
        PolicyField::ResourceBudget,
        resource_budget,
        defaults.resource_budget
    );
    let retention = adopt_field!(PolicyField::Retention, retention, defaults.retention);

    let policy = RenderPolicy {
        schema_version: POLICY_SCHEMA_VERSION,
        triggers,
        scope,
        quality_profile,
        completion,
        resource_budget,
        retention,
    };

    // Canonical resolution bytes: domain tag + schema version +
    // resolved policy + provenance — deterministic because every
    // component serializes canonically (BTree structures, sorted
    // provenance).
    let resolution_bytes = serde_json::to_vec(&serde_json::json!({
        "domain": POLICY_RESOLUTION_DOMAIN,
        "schema_version": POLICY_SCHEMA_VERSION,
        "policy": &policy,
        "provenance": &provenance,
    }))
    .unwrap_or_default();
    let resolution_digest = ContentDigest::sha256(&resolution_bytes);

    EffectivePolicy {
        policy,
        provenance,
        resolution_digest,
    }
}

// ---------------------------------------------------------------------------
// local grants — authorization, never policy
// ---------------------------------------------------------------------------

/// What *this* environment permits a render job to consume.
/// Supplied by the local host (OS permission, plugin consent,
/// remote-backend approval) — never derived from a project's
/// declared policy, so a shared project cannot force another
/// user's CPU/GPU/network spend (RCP-POL-06).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalGrants {
    /// Local CPU rendering permitted.
    pub cpu_render: bool,
    /// Local GPU rendering permitted.
    pub gpu_render: bool,
    /// Network use permitted (remote backends, remote consent).
    pub network: bool,
    /// Approved remote backend identifiers — a remote render may
    /// only target one of these *and* requires `network`.
    pub remote_backends: BTreeSet<String>,
    /// Local artifact-store storage ceiling in bytes the job may
    /// consume; `u64::MAX` declares no local cap.
    pub storage_quota_bytes: u64,
}

impl LocalGrants {
    /// Grants permitting local CPU render only — the conservative
    /// fixture default for tests that must opt into more.
    #[must_use]
    pub fn cpu_only() -> Self {
        Self {
            cpu_render: true,
            gpu_render: false,
            network: false,
            remote_backends: BTreeSet::new(),
            storage_quota_bytes: u64::MAX,
        }
    }
}
