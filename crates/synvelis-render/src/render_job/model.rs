//! Render job model — pinned targets, idempotent job keys, the
//! non-RT state machine, independent receipts, failure taxonomy, and
//! completeness verdicts (RCP-03, synvelis#292).
//!
//! A render job is *non-real-time orchestration only*: it resolves
//! recipes against an immutable [`PinnedSnapshot`], queues behind
//! budget/grant/consent preflight, renders through an injected
//! [`crate::render_job::RenderExecutor`], publishes exclusively via
//! the RCP-02 verified pipeline, and records every transition in an
//! append-only [`JobReceipt`] stream kept disjoint from external
//! operation receipts ([`crate::policy::trigger::ExternalReceipt`]).
//!
//! Nothing here blocks a realtime lane, falls back to realtime
//! rendering on offline failure, or associates a stale completion
//! with a newer project revision — the pinned target is fixed at
//! submission and every receipt carries its identity.

use serde::{Deserialize, Serialize};

use synvelis_project::snapshot::SnapshotIdentity;
use synvelis_project::{ContentDigest, Project, ProjectRevision};

use crate::policy::model::RenderScope;
use crate::recipe::{
    RecipeKey, RecipeResolution, RenderBoundary, UnknownDependency, UnsupportedReason,
};

// ---------------------------------------------------------------------------
// pinned target — the only thing a job may write against
// ---------------------------------------------------------------------------

/// An immutable render target: the snapshot identity (locator +
/// content digest) plus the project view resolved from exactly those
/// captured bytes. Pinned by value at submission — later project
/// revisions can never re-associate this job's completion (RCP-POL-03).
#[derive(Clone, Debug, PartialEq)]
pub struct PinnedSnapshot {
    /// Content/provenance identity of the capture.
    pub identity: SnapshotIdentity,
    /// The project view resolved from the captured bytes.
    pub project: Project,
}

impl PinnedSnapshot {
    /// Canonical locator string used in the job key — the *fixed*
    /// locator, never a mutable reference.
    #[must_use]
    pub fn locator_key(&self) -> String {
        serde_json::to_string(&self.identity.locator).unwrap_or_default()
    }
}

/// One render demand — a boundary plus the frame interval it covers.
/// Demands are what a scope/purpose/package *requires*; each becomes
/// one resolved recipe inside the job.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderDemand {
    /// Output boundary.
    pub boundary: RenderBoundary,
    /// Frame interval `[start, end)` to render.
    pub interval: crate::recipe::FrameInterval,
}

/// Everything one job renders — one pinned snapshot plus the demand
/// set. Multiple demands share the snapshot and purpose but resolve
/// to distinct recipes.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSelection {
    /// Pinned target.
    pub snapshot: PinnedSnapshot,
    /// Required render demands (non-empty for a lawful job).
    pub demands: Vec<RenderDemand>,
}

// ---------------------------------------------------------------------------
// purpose / entry surface / gateway projection
// ---------------------------------------------------------------------------

/// Caller-declared cost estimate for one job — checked against the
/// effective [`crate::policy::model::ResourceBudget`] and
/// [`crate::policy::model::LocalGrants`] before any work is issued
/// (RCP-POL-06). Estimates are declarations, not measurements; the
/// budget gate is conservative by contract.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobCostEstimate {
    /// Estimated CPU time in milliseconds.
    pub cpu_ms: u64,
    /// Estimated wall time in milliseconds.
    pub wall_ms: u64,
    /// Estimated storage consumption in bytes (staged + published).
    pub storage_bytes: u64,
}

/// What a render job is *for* — part of the job key so identical
/// content rendered for different purposes never coalesces.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum JobPurpose {
    /// Policy scope-driven render (save/commit/manual under a scope).
    Scoped {
        /// The resolved policy scope.
        scope: RenderScope,
    },
    /// Render required by a share package.
    Share {
        /// Package identity.
        package_id: String,
    },
    /// Explicit selection render (approved on-demand, ad-hoc manual).
    Selection,
}

/// The surface that projected a trigger — receipt provenance only.
/// GUI, headless CLI, Workflow, and Script all produce the same
/// typed [`crate::policy::trigger::TriggerReport`]; the surface tag
/// exists so receipts can show where a request entered, and is
/// deliberately excluded from the job key (RCP-POL-07).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntrySurface {
    /// Graphical application surface.
    Gui,
    /// Headless command-line surface.
    HeadlessCli,
    /// Workflow/automation surface.
    Workflow,
    /// Scripting surface.
    Script,
}

/// Gateway action identity — the single projection every entry
/// surface maps its render request onto (RCP-POL-07). Part of the
/// job key: identical requests under different actions do not
/// coalesce.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GatewayActionRef(pub String);

/// Authorization grant identity supplied by the gateway/consent
/// layer — carried opaquely; verification is the gateway's job
/// (RCP-06), this slice only keeps it in the job identity so grant
/// boundaries never share jobs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GrantRef(pub String);

// ---------------------------------------------------------------------------
// job key — idempotency / coalescing / duplicate-work suppression
// ---------------------------------------------------------------------------

/// The idempotent job identity: pinned snapshot locator + content
/// digest + sorted resolved recipe keys + purpose + versioned policy
/// resolution + gateway action/grant. Two requests with the same key
/// are the same work — the second coalesces, never duplicates
/// render or publish (RCP-POL-01).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RenderJobKey {
    /// Canonical serialization of the pinned *fixed* locator.
    pub snapshot_locator: String,
    /// Content digest of the pinned snapshot — provenance
    /// disambiguator alongside the locator.
    pub snapshot_digest: ContentDigest,
    /// Sorted resolved recipe keys (the resolved recipe set).
    pub recipe_keys: Vec<RecipeKey>,
    /// What the render is for.
    pub purpose: JobPurpose,
    /// Digest of the effective policy resolution — the "policy
    /// versioned resolution" component.
    pub policy_resolution: ContentDigest,
    /// Gateway action identity.
    pub action: String,
    /// Grant identity when present.
    pub grant: Option<String>,
}

// ---------------------------------------------------------------------------
// state machine + receipts
// ---------------------------------------------------------------------------

/// Non-RT job states — every transition is recorded as a
/// [`JobReceipt`]. `Partial` is a lawful terminal state: the job
/// finished with typed completeness gaps (never reported as
/// complete).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderJobState {
    /// Resolving recipes against the pinned snapshot.
    Resolve,
    /// Resolved, awaiting/preflight-passed, waiting for a slot.
    Queue,
    /// Building per-recipe render specs.
    Preparing,
    /// Rendering + verified publishing in flight.
    Rendering,
    /// Post-render completeness evaluation.
    Verifying,
    /// All required artifacts verified-published, no gaps.
    Published,
    /// Finished with typed completeness gaps — explicitly
    /// incomplete, never "complete".
    Partial,
    /// Terminal failure with a typed reason.
    Failed,
    /// Cooperative cancellation landed.
    Cancelled,
}

impl RenderJobState {
    /// `true` for states that can still advance.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Published | Self::Partial | Self::Failed | Self::Cancelled
        )
    }
}

/// One recorded job transition or event — the independent render-job
/// receipt stream (never mixed with save/VCS receipts).
#[derive(Clone, Debug, PartialEq)]
pub struct JobReceipt {
    /// Job key this receipt belongs to.
    pub job: RenderJobKey,
    /// Monotonic sequence within the job's receipt log.
    pub seq: u64,
    /// Store-logical tick at recording time.
    pub tick: u64,
    /// What happened.
    pub kind: ReceiptKind,
}

/// Receipt payload — the transition or notable event.
#[derive(Clone, Debug, PartialEq)]
pub enum ReceiptKind {
    /// State transition `from → to`.
    Transition {
        /// Previous state.
        from: RenderJobState,
        /// New state.
        to: RenderJobState,
    },
    /// Job submitted (first receipt).
    Submitted {
        /// Entry surface provenance.
        entry: EntrySurface,
        /// Trigger contributors.
        triggers: Vec<crate::policy::trigger::TriggerProvenance>,
        /// Pinned target locator (canonical string).
        target_locator: String,
        /// Pinned snapshot digest.
        snapshot_digest: ContentDigest,
        /// Saved revision when the pin came from a durable save.
        saved_revision: Option<ProjectRevision>,
    },
    /// A duplicate request coalesced into this job — intent kept,
    /// no duplicate work.
    Coalesced {
        /// Entry surface of the duplicate.
        entry: EntrySurface,
        /// Trigger contributors of the duplicate.
        triggers: Vec<crate::policy::trigger::TriggerProvenance>,
    },
    /// Cancel requested.
    CancelRequested,
    /// Retry scheduled for a retryable failure.
    Retrying {
        /// Attempt number being started.
        attempt: u32,
    },
    /// Crash observed mid-render — job remains resumable.
    CrashObserved,
    /// Resume requested after crash/pending.
    Resumed,
    /// Job admitted to the queue but blocked on an environmental
    /// gate (concurrency quota, storage headroom) — retryable,
    /// never a failure.
    Pending {
        /// Typed pending reason.
        reason: JobFailureReason,
        /// Machine-readable detail.
        detail: String,
    },
    /// Publish blocked by completeness gate (share path).
    PublishBlocked {
        /// Typed gap count.
        gaps: usize,
    },
    /// One demand produced (or reused) a verified artifact.
    ArtifactReady {
        /// Demand index within the selection.
        demand: usize,
        /// Recipe key rendered.
        recipe_key: RecipeKey,
        /// Artifact identity.
        artifact: crate::artifact::manifest::RenderArtifactId,
        /// `true` when an existing verified artifact was reused
        /// instead of a fresh render+publish.
        reused: bool,
    },
    /// One demand failed — recorded even when the job later
    /// retries or terminates, so per-boundary evidence is never
    /// lost.
    BoundaryFailed {
        /// Demand index within the selection.
        demand: usize,
        /// Recipe key when resolution succeeded.
        recipe_key: Option<RecipeKey>,
        /// Typed failure.
        failure: JobFailure,
    },
    /// Terminal outcome.
    Terminal {
        /// Final state.
        state: RenderJobState,
        /// Published/reused artifact identities.
        artifacts: Vec<crate::artifact::manifest::RenderArtifactId>,
        /// Completeness verdict summary (`None` when resolution
        /// failed before any artifact work).
        completeness: Option<CompletenessVerdict>,
        /// Typed failure when `state == Failed`.
        failure: Option<JobFailure>,
    },
}

// ---------------------------------------------------------------------------
// failure taxonomy — the RCP-POL-06 matrix vocabulary
// ---------------------------------------------------------------------------

/// Why a render job failed or pended — every variant is typed so a
/// receipt never collapses distinct causes into "error".
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JobFailureReason {
    /// A required plugin/processor binding is missing.
    PluginMissing,
    /// Plugin state could not be captured exactly.
    StateCaptureUnsupported,
    /// Referenced source content is unavailable/unverified.
    SourceUnavailable,
    /// Storage quota/headroom exhausted — retryable once space
    /// frees.
    InsufficientStorage,
    /// Cooperative cancellation.
    Cancelled,
    /// Worker/render backend failure — retryable.
    WorkerFailure,
    /// OS/permission denial.
    AuthDenied,
    /// Remote render/backend use without consent.
    RemoteConsentDenied,
    /// Declared resource budget exceeded — non-retryable under the
    /// same policy.
    BudgetExceeded,
    /// Rate/concurrency quota — retryable when capacity frees.
    QuotaExceeded,
    /// Typed unknown dependency blocked exact resolution.
    UnknownDependency,
    /// The contract cannot express the request.
    Unsupported,
    /// Required artifacts incomplete at a `RequireComplete` gate.
    IncompleteArtifacts,
}

impl JobFailureReason {
    /// `true` when the failure class may succeed on retry.
    #[must_use]
    pub const fn retryable(self) -> bool {
        matches!(
            self,
            JobFailureReason::InsufficientStorage
                | JobFailureReason::WorkerFailure
                | JobFailureReason::QuotaExceeded
        )
    }

    /// `true` when an environmental gate (concurrency/storage
    /// headroom) owns resume — `retry` must not bypass it; `drive`
    /// re-evaluates the gate automatically.
    #[must_use]
    pub const fn gate_managed(self) -> bool {
        matches!(
            self,
            JobFailureReason::InsufficientStorage | JobFailureReason::QuotaExceeded
        )
    }
}

/// A typed job failure — `retryable` separates transient causes
/// (storage/quota/worker) from permanent ones (auth/consent/
/// unsupported/budget-under-policy).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobFailure {
    /// Typed reason.
    pub reason: JobFailureReason,
    /// Machine-readable detail.
    pub detail: String,
}

impl JobFailure {
    /// `true` when the failure class may succeed on retry.
    #[must_use]
    pub const fn retryable(&self) -> bool {
        self.reason.retryable()
    }
}

// ---------------------------------------------------------------------------
// completeness verdict — BestEffort vs RequireComplete input
// ---------------------------------------------------------------------------

/// Why a required render is not completely satisfied — the typed
/// gap vocabulary of RCP-POL-05, aligned with RCP-04
/// `ArtifactEligibility`/`LookupOutcome` and RCP-01 `UnknownCause`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompletenessGapKind {
    /// No artifact exists for the recipe.
    MissingRender,
    /// Artifact failed verification (hash/manifest/decode
    /// contradiction).
    InvalidArtifact,
    /// Artifact is stale relative to current resolution.
    StaleArtifact,
    /// Artifact exists but verification is incomplete.
    UnverifiedArtifact,
    /// Required capability/format unsupported.
    UnsupportedCapability,
    /// A required plugin/processor is missing or its state is
    /// unverifiable.
    PluginMissing,
    /// Access/ACL/trust denied.
    Denied,
    /// Only a non-exact (preview-class) artifact exists.
    IncompleteContent,
    /// Recipe resolution failed closed on typed unknown
    /// dependencies.
    UnresolvedDependency,
}

/// One typed completeness gap.
#[derive(Clone, Debug, PartialEq)]
pub struct CompletenessGap {
    /// Recipe key the gap belongs to (`None` for boundaries whose
    /// resolution never produced a key).
    pub recipe_key: Option<RecipeKey>,
    /// The boundary that could not be satisfied.
    pub boundary: RenderBoundary,
    /// Typed gap kind.
    pub kind: CompletenessGapKind,
    /// Machine-readable detail.
    pub detail: String,
}

/// Whole-job completeness verdict — `Complete` is claimed only when
/// every required recipe has an Exact-eligible verified artifact.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompletenessVerdict {
    /// Every required artifact verified-exact.
    Complete,
    /// At least one typed gap — never reported as complete.
    Incomplete,
}

/// Full completeness evaluation result.
#[derive(Clone, Debug, PartialEq)]
pub struct CompletenessReport {
    /// The verdict.
    pub verdict: CompletenessVerdict,
    /// Typed gaps (empty iff `Complete`).
    pub gaps: Vec<CompletenessGap>,
}

// ---------------------------------------------------------------------------
// share gate verdict
// ---------------------------------------------------------------------------

/// Pre-publish share/package decision under the effective
/// completion policy.
#[derive(Clone, Debug, PartialEq)]
pub enum ShareGateVerdict {
    /// Every required artifact verified — share may proceed.
    Allowed {
        /// Satisfied artifact identities.
        artifacts: Vec<crate::artifact::manifest::RenderArtifactId>,
    },
    /// `BestEffort`: share proceeds with explicit partial marking —
    /// the gaps are disclosed, never hidden.
    AllowedPartial {
        /// Typed gaps being disclosed.
        gaps: Vec<CompletenessGap>,
        /// Satisfied artifact identities.
        artifacts: Vec<crate::artifact::manifest::RenderArtifactId>,
    },
    /// `RequireComplete` (or undecidable pending work): publish is
    /// blocked — typed reasons plus what to retry.
    Blocked {
        /// Typed gaps.
        gaps: Vec<CompletenessGap>,
        /// Jobs still in flight that may close the gaps.
        pending_jobs: Vec<RenderJobKey>,
    },
}

// ---------------------------------------------------------------------------
// boundary resolution record (per-demand recipe outcome)
// ---------------------------------------------------------------------------

/// Per-demand recipe resolution outcome stored on the job record.
#[derive(Clone, Debug)]
pub enum BoundaryResolution {
    /// Recipe resolved — carries the full `Resolved` resolution
    /// (recipe + key + provenance) so publish drafts and lookups
    /// consume the same authority.
    Resolved {
        /// Demand index within the selection.
        demand: usize,
        /// The `Resolved` recipe resolution.
        resolution: Box<RecipeResolution>,
    },
    /// Resolution failed closed — typed unknown dependencies.
    Unknown {
        /// Demand index.
        demand: usize,
        /// Every unknown dependency found.
        dependencies: Vec<UnknownDependency>,
    },
    /// The contract cannot express this demand.
    Unsupported {
        /// Demand index.
        demand: usize,
        /// Why.
        reason: UnsupportedReason,
    },
}
