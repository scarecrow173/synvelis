//! Typed Save/Commit/Share/OnDemand trigger ports (RCP-03,
//! synvelis#292; canonical contract `docs/specs/render-checkpoint.md`
//! §6).
//!
//! These are *ports*, not integrations: the events and receipts are
//! the typed contract every entry surface (GUI, headless CLI,
//! Workflow, Script — and later the real VCS provider of RCP-05)
//! projects into. Nothing here performs a save, a commit, a push, or
//! a package publish; it only classifies what an external receipt
//! lawfully triggers.
//!
//! Receipt separation is structural: external operation outcomes
//! ([`ExternalReceipt`]: save failure, commit provider/push failure,
//! share-gate verdicts) live in a stream disjoint from render-job
//! receipts ([`crate::render_job::JobReceipt`]). A provider failure
//! can never be mistaken for a render-job failure (RCP-POL-04).

use synvelis_project::{ProjectId, ProjectRevision};

use crate::policy::model::{PolicyPatch, RenderTrigger};
use crate::render_job::model::{
    EntrySurface, GatewayActionRef, GrantRef, JobCostEstimate, JobPurpose, PinnedSnapshot,
    RenderDemand, RenderSelection,
};

// ---------------------------------------------------------------------------
// pinned snapshot + render demand vocabulary (shared with render_job)
// ---------------------------------------------------------------------------
// `PinnedSnapshot`, `RenderDemand`, `RenderSelection`, `JobPurpose`,
// `EntrySurface`, `GatewayActionRef`, `GrantRef` are defined in
// `crate::render_job::model` — they are job-target concepts, not
// policy concepts. Re-imported above for this module's signatures.

// ---------------------------------------------------------------------------
// save trigger
// ---------------------------------------------------------------------------

/// Receipt of one *durable* save — the only lawful `OnSave` trigger
/// input. Produced exclusively by a successful durable save; a save
/// that failed produces [`SaveEvent::Failed`], which can never pin a
/// snapshot or spawn a render job (RCP-POL-03).
#[derive(Clone, Debug, PartialEq)]
pub struct SaveReceipt {
    /// The fixed snapshot the durable save returned — pinned by
    /// value, never re-resolved against live HEAD.
    pub snapshot: PinnedSnapshot,
    /// Project revision the save committed.
    pub saved_revision: ProjectRevision,
    /// Store head name observed at save time (provenance).
    pub head: String,
}

/// Save-side trigger events.
#[derive(Clone, Debug)]
pub enum SaveEvent {
    /// Durable save succeeded — carries its receipt.
    Completed(Box<SaveReceipt>),
    /// Save failed — recorded as an external receipt; launches
    /// nothing.
    Failed {
        /// Project the failed save targeted.
        project_id: ProjectId,
        /// Revision the save attempted.
        revision: ProjectRevision,
        /// Machine-readable failure detail.
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// commit trigger
// ---------------------------------------------------------------------------

/// Capability of the VCS provider behind the commit trigger.
/// `Unsupported` is a first-class typed result — never a false
/// success and never a silent skip (RCP-POL-04).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitCapability {
    /// The provider can create commits.
    Supported,
    /// The provider cannot commit — carries the reason.
    Unsupported {
        /// Why commit is unavailable.
        reason: String,
    },
}

/// Receipt of one *successful* VCS commit — the only lawful
/// `OnCommit` trigger input. The commit locator (SHA or provider
/// equivalent) is immutable: post-commit artifacts attach through
/// the external manifest registry, never by rewriting the commit.
#[derive(Clone, Debug, PartialEq)]
pub struct CommitReceipt {
    /// Provider-side immutable commit locator.
    pub commit_locator: String,
    /// The pinned snapshot captured at commit time.
    pub snapshot: PinnedSnapshot,
}

/// Commit-side trigger events. Provider and push failures are
/// recorded on the external receipt stream — they are *not* render
/// job failures and never spawn one.
#[derive(Clone, Debug)]
pub enum CommitEvent {
    /// Commit succeeded — carries its receipt.
    Committed(Box<CommitReceipt>),
    /// Provider has no commit capability — typed unsupported.
    Unsupported {
        /// Why commit is unavailable.
        reason: String,
    },
    /// Provider-side failure (e.g. hook rejection, write error).
    ProviderFailed {
        /// Failure detail.
        detail: String,
    },
    /// Commit succeeded but the push failed — the commit itself is
    /// still immutable and valid; only propagation failed.
    PushFailed {
        /// The immutable commit locator that was created.
        commit_locator: String,
        /// Push failure detail.
        detail: String,
    },
}

// ---------------------------------------------------------------------------
// share trigger
// ---------------------------------------------------------------------------

/// A share/package publish request — `OnShare` inspects package
/// completeness *before* publish; under `RequireComplete` an
/// incomplete package is blocked with typed reasons and a retry
/// hint, under `BestEffort` it proceeds with explicit partial
/// marking (RCP-POL-05).
#[derive(Clone, Debug, PartialEq)]
pub struct ShareRequest {
    /// Package identity being shared.
    pub package_id: String,
    /// Pinned snapshot the package renders from.
    pub snapshot: PinnedSnapshot,
    /// Render demands the package requires.
    pub demands: Vec<RenderDemand>,
    /// Optional explicit policy intent for this operation.
    pub policy_intent: Option<PolicyPatch>,
}

// ---------------------------------------------------------------------------
// on-demand trigger
// ---------------------------------------------------------------------------

/// An `OnDemand` render proposal — a suggestion only. Nothing is
/// rendered, queued, or granted CPU/GPU/network until an
/// [`DemandAuthorizer`] explicitly approves (RCP-POL-06).
#[derive(Clone, Debug, PartialEq)]
pub struct DemandProposal {
    /// Proposal identity (idempotency/provenance).
    pub proposal_id: String,
    /// What would be rendered, pinned.
    pub selection: RenderSelection,
    /// What the render is for.
    pub purpose: JobPurpose,
    /// Why the render is proposed (free-form rationale for audit).
    pub rationale: String,
    /// Optional explicit policy intent carried by the approval.
    pub policy_intent: Option<PolicyPatch>,
}

/// Authorization decision for an [`DemandProposal`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DemandAuthorization {
    /// Explicit approval — carries the grant identity under which
    /// the job may consume resources.
    Granted {
        /// Grant the approval was issued under.
        grant: GrantRef,
    },
    /// Denied — typed reason, no resources may be consumed.
    Denied {
        /// Why the proposal was denied.
        reason: String,
    },
}

/// Port deciding whether an `OnDemand` proposal may consume resources.
/// Implementations are the consent boundary: configured user
/// approval, automation grant, or policy engine — never implicit.
pub trait DemandAuthorizer {
    /// Authorizes (or denies) `proposal`.
    fn authorize(&self, proposal: &DemandProposal) -> DemandAuthorization;
}

// ---------------------------------------------------------------------------
// trigger events + routing vocabulary
// ---------------------------------------------------------------------------

/// One external event presented to the render pipeline. Every entry
/// surface (GUI/CLI/Workflow/Script) projects into this same typed
/// vocabulary (RCP-POL-07).
#[derive(Clone, Debug)]
pub enum TriggerEvent {
    /// Explicit manual render request — always enabled.
    Manual {
        /// Pinned target + demands.
        selection: RenderSelection,
        /// What the render is for.
        purpose: JobPurpose,
        /// Optional operation-specific policy intent.
        policy_intent: Option<PolicyPatch>,
    },
    /// Save-side event.
    Save(SaveEvent),
    /// Commit-side event.
    Commit(CommitEvent),
    /// Share/package request.
    Share(ShareRequest),
    /// `OnDemand` proposal — routed to the authorizer first.
    Demand(DemandProposal),
}

/// Provenance of one trigger that contributed to a job — recorded on
/// the job record so coalesced duplicate events keep their intent
/// visible without duplicating work (RCP-POL-01).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriggerProvenance {
    /// The trigger kind.
    pub trigger: RenderTrigger,
    /// Human/machine detail (save head, commit locator, package id,
    /// proposal id, …).
    pub detail: String,
}

/// An event wrapped with its entry-surface projection — the same
/// [`TriggerEvent`] + [`GatewayActionRef`] + [`GrantRef`] triple
/// every surface must produce for identical requests (RCP-POL-07).
/// `entry` is receipt provenance only; it is deliberately *not* part
/// of the job key so cross-surface duplicates coalesce.
#[derive(Clone, Debug)]
pub struct TriggerReport {
    /// The typed event.
    pub event: TriggerEvent,
    /// Which surface projected this event.
    pub entry: EntrySurface,
    /// Gateway action identity — part of the job key.
    pub action: GatewayActionRef,
    /// Authorization grant identity — part of the job key when
    /// present (`OnDemand` approvals always carry one).
    pub grant: Option<GrantRef>,
}

/// Why an event produced no job — typed, never silent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InertReason {
    /// The effective policy does not enable this trigger.
    TriggerDisabled,
    /// The event carried no lawful pin (e.g. save failure has no
    /// snapshot to pin, commit provider/push failure has no usable
    /// receipt for rendering).
    NoLawfulPin,
    /// The scope resolver produced no render demands for this
    /// purpose — nothing to render.
    NoDemands,
    /// An external operation failed — recorded on the external
    /// receipt stream; no render work is lawful from it.
    ExternalFailed,
}

/// What the trigger router decided for one [`TriggerReport`].
#[derive(Clone, Debug)]
pub enum RoutedTrigger {
    /// A render job should be submitted (or coalesced) — carries the
    /// assembled job intent.
    Job(JobIntent),
    /// Event is explicitly inert under the effective policy.
    Inert {
        /// Typed reason.
        reason: InertReason,
    },
    /// The required capability is unsupported (commit API absent) —
    /// typed, never a false success.
    Unsupported {
        /// Which capability.
        capability: &'static str,
        /// Why.
        reason: String,
    },
    /// Fail-closed denial — authorization refused before any
    /// CPU/GPU/network consumption.
    Denied {
        /// Why.
        reason: String,
    },
    /// Share request routed to the completeness gate — the
    /// orchestrator evaluates the package before any publish.
    ShareGate {
        /// The share request.
        request: ShareRequest,
        /// Entry/action/grant projection.
        entry: EntrySurface,
        /// Gateway action.
        action: GatewayActionRef,
        /// Grant, when present.
        grant: Option<GrantRef>,
    },
}

/// Everything needed to submit one render job — assembled by the
/// router from a lawful trigger event.
#[derive(Clone, Debug)]
pub struct JobIntent {
    /// Pinned target + demands.
    pub selection: RenderSelection,
    /// What the render is for.
    pub purpose: JobPurpose,
    /// Resolved effective policy for this operation.
    pub policy: crate::policy::model::EffectivePolicy,
    /// Trigger provenance contributors (may be several after
    /// coalescing information is merged by the caller).
    pub triggers: Vec<TriggerProvenance>,
    /// Commit locators whose artifacts must be attached via the
    /// external manifest registry after publish (SHA stays
    /// immutable).
    pub external_manifests: Vec<String>,
    /// Caller-declared cost estimate for budget/grant preflight.
    pub estimate: JobCostEstimate,
    /// Saved revision when the pin came from a durable save —
    /// recorded on receipts so a completion can never be
    /// re-associated with a newer revision (RCP-POL-03).
    pub saved_revision: Option<ProjectRevision>,
    /// Entry surface (receipt provenance only).
    pub entry: EntrySurface,
    /// Gateway action identity.
    pub action: GatewayActionRef,
    /// Grant identity, when present.
    pub grant: Option<GrantRef>,
}

/// What one [`TriggerReport`] produced — the observable outcome of
/// routing plus submission/gate evaluation.
#[derive(Clone, Debug)]
pub enum TriggerOutcome {
    /// A render job was submitted or coalesced into an existing one.
    Job {
        /// The job key.
        key: crate::render_job::model::RenderJobKey,
        /// `true` when an existing job absorbed the request.
        coalesced: bool,
    },
    /// Share request evaluated at the completeness gate.
    ShareGate {
        /// The fill job key when one was submitted.
        job: Option<crate::render_job::model::RenderJobKey>,
        /// The gate verdict.
        verdict: crate::render_job::model::ShareGateVerdict,
    },
    /// `OnDemand` proposal denied — typed reason, nothing consumed.
    Denied {
        /// Why.
        reason: String,
    },
    /// Capability unsupported (e.g. commit API) — typed, not a
    /// render failure.
    Unsupported {
        /// Which capability.
        capability: &'static str,
        /// Why.
        reason: String,
    },
    /// Event inert under the effective policy or carrying no lawful
    /// pin — typed reason, recorded.
    Inert {
        /// Why.
        reason: InertReason,
    },
}

// ---------------------------------------------------------------------------
// external receipt stream — disjoint from render-job receipts
// ---------------------------------------------------------------------------

/// Outcomes of *external* operations (save/commit/share gate) —
/// recorded separately from [`crate::render_job::JobReceipt`] so a
/// provider failure is never conflated with a render-job failure
/// (RCP-POL-04).
#[derive(Clone, Debug, PartialEq)]
pub enum ExternalReceipt {
    /// A save attempt failed — no render job was or will be spawned
    /// for it.
    SaveFailed {
        /// Project.
        project_id: ProjectId,
        /// Attempted revision.
        revision: ProjectRevision,
        /// Failure detail.
        reason: String,
    },
    /// The commit capability is unsupported — typed, not a failure
    /// of the render pipeline.
    CommitUnsupported {
        /// Why.
        reason: String,
    },
    /// The VCS provider failed to create the commit.
    CommitProviderFailed {
        /// Failure detail.
        detail: String,
    },
    /// The commit was created but pushing it failed — the commit
    /// locator remains valid and immutable.
    CommitPushFailed {
        /// Immutable commit locator.
        commit_locator: String,
        /// Push failure detail.
        detail: String,
    },
    /// Share-gate verdict for a package — the pre-publish
    /// completeness decision.
    ShareGate {
        /// Package identity.
        package_id: String,
        /// Allowed / allowed-partial / blocked summary.
        verdict: String,
    },
    /// Post-publish external manifest attach failed — the commit
    /// locator stays immutable and the render job's own receipts
    /// are unaffected, but the rejection (unsafe locator, unknown
    /// or quarantined artifact, store error) is recorded here so
    /// it is never silent (RCP-POL-04).
    ExternalManifestAttachFailed {
        /// Commit locator the attach targeted.
        commit_locator: String,
        /// Artifact the attach targeted.
        artifact: crate::artifact::manifest::RenderArtifactId,
        /// Failure detail.
        detail: String,
    },
}

// ---------------------------------------------------------------------------
// ports
// ---------------------------------------------------------------------------

/// Supplies the policy layers for a project — the declared project
/// preference and this user's local override. The operation-specific
/// explicit intent arrives on the trigger event itself.
///
/// Implementations must keep `local_override` private to the local
/// principal: a shared project's declared policy is visible to every
/// collaborator, but one user's override never leaks into another's
/// resolution.
pub trait PolicyResolver {
    /// The project's declared preference, when any.
    fn declared(&self, project_id: ProjectId) -> Option<PolicyPatch>;
    /// This user's local override, when any.
    fn local_override(&self) -> Option<PolicyPatch>;
}

/// Resolves which render demands a scope/purpose requires for a
/// pinned project — the scope→boundary/interval mapping owned by the
/// render-intent layer (consumed read-only by player/CLI surfaces).
pub trait ScopeDemandResolver {
    /// Demands for `purpose` on `project`. May be empty — the
    /// trigger then routes `Inert`.
    fn demands(
        &self,
        project: &synvelis_project::Project,
        purpose: &JobPurpose,
    ) -> Vec<RenderDemand>;
}
