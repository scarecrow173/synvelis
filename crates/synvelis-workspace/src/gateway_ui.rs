//! `gateway_ui` — the headless GW-09 Connection / Task / Grant /
//! Approval UI contract, state, and explanation surface (issue #60).
//!
//! # Trust boundary
//!
//! The UI is never authorization truth. It transmits *trusted human
//! intent* to the gateway: a [`CallerIdentity`] bound through an
//! [`AdapterSeal`]-issued adapter (the manual-UI control channel in
//! production, [`FixtureAdapter`][synvelis_gateway::fixture::FixtureAdapter]
//! in tests), plus explicit ids — never wire-declared identity, grants,
//! or approvals. GW-02 evaluates every request, GW-05 owns
//! proposal/approval/apply truth, GW-06 owns the commit boundary, and
//! the job coordinator owns cancellation truth.
//!
//! What the UI renders are *projections* of those authorities' records
//! plus its own bookkeeping:
//!
//! - **switches** — independent on/off for the AI runtime and the
//!   external MCP adapter (UX-01). Everything stays usable with both
//!   off.
//! - **grant editor + explanation** — scope kinds (explicit id set,
//!   issuance-time snapshot capture, dynamic rule), linked-source
//!   effects, expiry, epoch ceiling, and the apply budget (UX-02).
//! - **connection / task / grant surfaces** — explicit-id rows with a
//!   separate *revocation intent vs. confirmation* axis and per-id
//!   actions (UX-04, SYS-05).
//! - **approval surface** — a pinned `(proposal, digest)` review card:
//!   scope/effect presentation, bound grant/task, expiry, decision
//!   (approve/decline/refused) (UX-03). Selection, focus, and
//!   window-change races cannot move it.
//! - **apply mode** — `suggest` / `approve-to-apply` / `delegated-apply`
//!   are *derived from the issued grant's `ApprovalPolicy`*; the UI
//!   never declares a mode to the gateway (SYS-04).
//! - **activity feed** — one row per proposal with distinct Proposal /
//!   Approval / Apply / Commit / Engine / Saved axes, plus job rows
//!   that keep `phase` and `CancelLink` separate (UX-04, UX-05,
//!   SYS-06).
//!
//! Everything here is deliberately headless: projections for a host
//! UI to render, and a fixture-friendly seam layer the real shell
//! wires to the control channel. Real-device GUI verification stays a
//! manual gate.

#![allow(clippy::result_large_err)] // typed envelope is the contract

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use synvelis_gateway::mcp_permit::check_channel_kind;
use synvelis_gateway::{
    APPLY_ID, APPROVE_ID, Activation, AdapterId, AdminPort, ApplyStatus, ApprovalPolicy,
    ApprovalRecord, ApprovalStatus, AuthorizedTask, CallerIdentity, CancelLink, CommitOutcome,
    CommitPort, ContentDigest, DerivedEffect, EffectKind, ErrorEnvelope, Gateway, GatewayError,
    GatewayOutcome, Grant, GrantBinding, GrantRecord, GrantRef, JOB_CANCEL_ID, JobEngine, JobPhase,
    JobRecord, JobRef, McpBounds, McpChannel, McpConfig, OperationId, OperationTarget,
    OwnerConstraint, PermitIssue, PermitSpec, PermitStore, PolicyClock, PolicyInstant, PolicyStore,
    Principal, PrincipalKind, ProposalEngine, ProposalRef, ProposalStatus, ProposalStore,
    ProposalView, ResourceAttributes, ResourceKind, ResourceRef, ResourceScope, RetryOutcome,
    SaveRecord, SemanticCommand, TaskRef,
};
use synvelis_project::{ProjectId, ProjectRevision};

use crate::state::{EditingSelection, SemanticTarget};

// ══════════════════════════════════════════════════════════════════
// errors
// ══════════════════════════════════════════════════════════════════

/// UI-side failures: gateway/admin refusals, store errors, and the
/// surface-state guards that keep stale context from acting.
#[derive(Debug)]
#[non_exhaustive]
pub enum GatewayUiError {
    /// A typed refusal envelope — from the gateway or a spec-coherence
    /// check issued on the UI's behalf.
    Gateway(ErrorEnvelope),
    /// The host admin surface refused.
    Admin(ErrorEnvelope),
    /// The permit store failed.
    PermitStore(std::io::Error),
    /// The named proposal has no live view (unknown or evicted).
    ProposalGone(ProposalRef),
    /// The asserted digest does not match the stored changeset — a
    /// swapped or stale proposal must never open a review card.
    DigestMismatch {
        /// The proposal the assertion named.
        proposal: ProposalRef,
    },
    /// The approval-surface handle is unknown.
    UnknownSurface(u64),
    /// The selection capture carried no targets — an empty selection
    /// must never silently mint an empty scope.
    EmptyCapture,
    /// The apply needs an approval this caller cannot supply.
    NeedsApproval {
        /// The pending approval request ref.
        approval: String,
    },
}

impl fmt::Display for GatewayUiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gateway(e) => write!(f, "gateway refused: {e:?}"),
            Self::Admin(e) => write!(f, "admin refused: {e:?}"),
            Self::PermitStore(e) => write!(f, "permit store: {e}"),
            Self::ProposalGone(p) => write!(f, "proposal {} is not reviewable", p.as_str()),
            Self::DigestMismatch { proposal } => write!(
                f,
                "asserted digest does not match proposal {}",
                proposal.as_str()
            ),
            Self::UnknownSurface(id) => write!(f, "unknown approval surface {id}"),
            Self::EmptyCapture => write!(f, "selection capture carried no targets"),
            Self::NeedsApproval { approval } => {
                write!(f, "apply waits on approval request {approval}")
            }
        }
    }
}

impl std::error::Error for GatewayUiError {}

impl From<std::io::Error> for GatewayUiError {
    fn from(e: std::io::Error) -> Self {
        Self::PermitStore(e)
    }
}

// ══════════════════════════════════════════════════════════════════
// AI / MCP feature switches (UX-01)
// ══════════════════════════════════════════════════════════════════

/// An independently switchable AI/MCP capability.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum AiFeature {
    /// The AI runtime: proposals, approvals, applies, and non-RT jobs.
    /// Off ⇒ no agent-path calls are made; the project workspace stays
    /// fully usable.
    AiRuntime,
    /// The external MCP adapter (`synvelis.*` over MCP). Off ⇒
    /// `connect` fails closed; the rest of the UI (including AI-on-UI
    /// flows) is unaffected.
    ExternalMcp,
}

impl AiFeature {
    /// The display name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::AiRuntime => "AI runtime",
            Self::ExternalMcp => "External MCP adapter",
        }
    }

    /// What this switch gates — the honesty text the switch row shows.
    #[must_use]
    pub const fn scope(self) -> &'static str {
        match self {
            Self::AiRuntime => "agent proposals, approvals, applies, and background jobs",
            Self::ExternalMcp => "the `synvelis.*` MCP tool surface",
        }
    }
}

/// Independent AI/MCP feature switches (UX-01: separate master AI and
/// external-MCP settings, both defaulting OFF; no implicit global
/// "AI mode").
#[derive(Clone, Debug, Default)]
pub struct AiSwitches {
    ai_runtime: bool,
    external_mcp: bool,
}

impl AiSwitches {
    /// All-off.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `feature` is currently enabled.
    #[must_use]
    pub fn enabled(&self, feature: AiFeature) -> bool {
        match feature {
            AiFeature::AiRuntime => self.ai_runtime,
            AiFeature::ExternalMcp => self.external_mcp,
        }
    }

    /// Sets `feature` and returns the projected switch rows.
    pub fn set(&mut self, feature: AiFeature, enabled: bool) -> AiPanelView {
        match feature {
            AiFeature::AiRuntime => self.ai_runtime = enabled,
            AiFeature::ExternalMcp => self.external_mcp = enabled,
        }
        self.panel()
    }

    /// The projected switch panel — rows plus the assurance note.
    #[must_use]
    pub fn panel(&self) -> AiPanelView {
        AiPanelView {
            switches: [AiFeature::AiRuntime, AiFeature::ExternalMcp]
                .into_iter()
                .map(|feature| SwitchRow {
                    feature,
                    enabled: self.enabled(feature),
                    scope: feature.scope(),
                })
                .collect(),
            note: "Each switch is independent: the external MCP adapter \
                   does not imply the AI runtime, and neither is required \
                   for the project workspace to stay fully usable.",
        }
    }
}

/// One projected switch row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchRow {
    /// Which feature.
    pub feature: AiFeature,
    /// Its current state.
    pub enabled: bool,
    /// What the switch gates (honesty text).
    pub scope: &'static str,
}

/// The projected AI/MCP settings panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiPanelView {
    /// One row per feature.
    pub switches: Vec<SwitchRow>,
    /// The independence assurance text.
    pub note: &'static str,
}

// ══════════════════════════════════════════════════════════════════
// revocation — intent vs. confirmation (SYS-05)
// ══════════════════════════════════════════════════════════════════

/// The UI-side revocation axis, kept separate from the authority's
/// `revoked` flag: the user asked (`Requested`), the store confirmed
/// (`Confirmed`), or nothing was asked (`None`). A failed confirmation
/// stays `Requested` — the row shows "revoke requested" while the
/// authority still reports live (SYS-05).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Revocation {
    /// No revocation has been requested through this surface.
    #[default]
    None,
    /// The user requested revocation; the authority has not confirmed.
    Requested,
    /// The authority store confirmed the revocation.
    Confirmed,
}

impl Revocation {
    /// The display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "live",
            Self::Requested => "revoke-requested",
            Self::Confirmed => "revoked",
        }
    }
}

/// The revocation target key — permits, tasks, and grants live in
/// different namespaces, so the axis is keyed by kind+id.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RevocationTarget {
    /// A connection permit id.
    Permit(String),
    /// An authorized task.
    Task(TaskRef),
    /// A grant.
    Grant(GrantRef),
}

/// The result of a revocation action.
#[derive(Clone, Debug)]
pub struct RevocationReport {
    /// Where the intent-versus-confirmation axis landed.
    pub state: Revocation,
    /// The job refs a grant revocation sent cancel *requests* to —
    /// these are requests, not confirmed cancellations (SYS-05). Empty
    /// for permit/task revocations and for grant revocations that
    /// reached no live jobs.
    pub cancel_requests: Vec<JobRef>,
}

// ══════════════════════════════════════════════════════════════════
// apply mode — derived, never declared (SYS-04)
// ══════════════════════════════════════════════════════════════════

/// The apply-mode label the UI *derives* from a grant's issued
/// `ApprovalPolicy`. The wire cannot carry a mode; the grant's
/// issuance-time policy is the single source.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ApplyMode {
    /// Proposals prepare and preview only — agent apply is denied; the
    /// user may still apply a proposal manually.
    Suggest,
    /// Apply needs a fresh human approval receipt bound to this exact
    /// proposal.
    ApproveToApply,
    /// Apply proceeds on grant+commit checks alone — no receipt.
    DelegatedApply,
}

impl ApplyMode {
    /// Derives the label from the issued policy.
    #[must_use]
    pub const fn of(policy: ApprovalPolicy) -> Self {
        match policy {
            ApprovalPolicy::Suggest => Self::Suggest,
            ApprovalPolicy::ApproveToApply => Self::ApproveToApply,
            ApprovalPolicy::DelegatedApply => Self::DelegatedApply,
        }
    }

    /// The display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Suggest => "suggest",
            Self::ApproveToApply => "approve-to-apply",
            Self::DelegatedApply => "delegated-apply",
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// grant + scope explanation (UX-02)
// ══════════════════════════════════════════════════════════════════

/// The projected scope text for one grant (UX-02: Fixed vs Dynamic
/// plus linked-source clarity).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ScopeExplanation {
    /// An explicit id set — membership is exactly these tagged ids.
    Exact {
        /// Member count.
        members: usize,
        /// The tagged member ids (stable order).
        resources: Vec<String>,
    },
    /// An issuance-time snapshot capture — the set was copied from the
    /// issuance-time context at `captured_revision`; it cannot change.
    Snapshot {
        /// Member count.
        members: usize,
        /// The tagged member ids.
        resources: Vec<String>,
        /// The committed revision the capture pinned.
        captured_revision: u64,
    },
    /// A rule-constrained set — membership is whatever satisfies the
    /// constraints (and optionally future members). The member list is
    /// *evaluated at use time*, never trusted from the UI.
    Dynamic {
        /// Kinds the set may contain.
        kinds: Vec<ResourceKind>,
        /// The owner constraint.
        owners: OwnerExplanation,
        /// Whether a range constraint bounds members.
        range_bound: bool,
        /// The member-count cap, when declared.
        max_members: Option<u32>,
        /// Whether resources created after issuance may join.
        include_future: bool,
    },
}

/// The projected owner constraint.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum OwnerExplanation {
    /// Any owner.
    Any,
    /// One of these explicit owner refs.
    OneOf {
        /// Member count.
        count: usize,
        /// The tagged owner ids.
        refs: Vec<String>,
    },
}

/// The projected expiry axis.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ExpiryExplanation {
    /// The expiry instant (`None` = none declared).
    pub expires_at: Option<PolicyInstant>,
    /// Whether the record is revoked.
    pub revoked: bool,
    /// Whether the record is live *now* (`!expired && !revoked`).
    pub live: bool,
}

/// The projected budget axis.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct BudgetExplanation {
    /// The declared apply budget (`None` = unlimited).
    pub apply_budget: Option<u64>,
    /// Applies consumed so far (from the authority store).
    pub applies_consumed: u64,
    /// The declared job budget, when shown.
    pub job_budget: Option<u64>,
}

/// The full projected grant explanation (UX-02): scope + linked-source
/// clarity + expiry + epoch + budget + mode.
#[derive(Clone, Debug)]
pub struct GrantExplanation {
    /// The grant this explains.
    pub grant: GrantRef,
    /// The scope projection.
    pub scope: ScopeExplanation,
    /// The derived apply mode.
    pub mode: ApplyMode,
    /// Expiry/revocation/liveness.
    pub expiry: ExpiryExplanation,
    /// The apply/job budget.
    pub budget: BudgetExplanation,
    /// Whether an epoch ceiling is declared.
    pub epoch_ceiling: bool,
    /// Honesty notes the card shows verbatim — linked-source effect,
    /// snapshot provenance, future-membership.
    pub notes: Vec<&'static str>,
}

/// Projects a grant record into the approval-facing explanation
/// (UX-02). `now` is the caller's policy instant — liveness is
/// evaluated against it, never assumed.
#[must_use]
pub fn explain_grant(record: &GrantRecord, now: PolicyInstant) -> GrantExplanation {
    let grant = &record.grant;
    let scope = scope_explanation(&grant.scope);
    let mut notes = vec![
        "Scope is evaluated by the policy engine at use time — this \
         surface never decides membership itself.",
        "Linked sources are separate resources: clip scope never covers \
         the sources a clip reads, and a shared-source write lands on \
         every clip referencing it.",
    ];
    match &grant.scope {
        ResourceScope::Snapshot { .. } => {
            notes.push(
                "This scope was captured from the issuance-time context — \
                 its membership cannot change.",
            );
        }
        ResourceScope::Dynamic(d) => {
            notes.push(if d.include_future {
                "Members are whichever resources satisfy the constraints \
                 at use time — including resources created after issuance."
            } else {
                "Members are fixed to resources that satisfied the \
                 constraints at issuance; nothing joins later."
            });
        }
        ResourceScope::Exact { .. } => {}
    }
    if grant.epoch_ceiling.is_some() {
        notes.push(
            "An epoch ceiling is declared — a policy-epoch advance past \
             it retires this grant even before expiry.",
        );
    }
    GrantExplanation {
        grant: grant.id.clone(),
        scope,
        mode: ApplyMode::of(grant.approval_policy),
        expiry: ExpiryExplanation {
            expires_at: grant.validity.expires_at,
            revoked: grant.validity.revoked,
            live: grant.validity.live(now),
        },
        budget: BudgetExplanation {
            apply_budget: grant.limits.apply_budget,
            applies_consumed: record.consumed_applies,
            job_budget: grant.limits.job_budget,
        },
        epoch_ceiling: grant.epoch_ceiling.is_some(),
        notes,
    }
}

fn scope_explanation(scope: &ResourceScope) -> ScopeExplanation {
    match scope {
        ResourceScope::Exact { resources } => ScopeExplanation::Exact {
            members: resources.len(),
            resources: resources.iter().map(tagged_resource).collect(),
        },
        ResourceScope::Snapshot {
            resources,
            captured_revision,
        } => ScopeExplanation::Snapshot {
            members: resources.len(),
            resources: resources.iter().map(tagged_resource).collect(),
            captured_revision: captured_revision.0,
        },
        ResourceScope::Dynamic(d) => ScopeExplanation::Dynamic {
            kinds: d.kinds.iter().copied().collect(),
            owners: owner_explanation(&d.owners),
            range_bound: d.range.is_some(),
            max_members: d.max_members,
            include_future: d.include_future,
        },
    }
}

fn owner_explanation(owners: &OwnerConstraint) -> OwnerExplanation {
    match owners {
        OwnerConstraint::Any => OwnerExplanation::Any,
        OwnerConstraint::OneOf { refs } => OwnerExplanation::OneOf {
            count: refs.len(),
            refs: refs.iter().map(tagged_resource).collect(),
        },
    }
}

/// The selection-assist capture: copies `selection` into `grant`'s
/// scope as an issuance-time `Snapshot` set with its provenance
/// revision (UX-02 — an explicit capture is a Snapshot, never live
/// context). This is a *one-time copy*: afterwards, selection / focus
/// / window changes cannot move the grant's membership.
///
/// # Errors
/// [`GatewayUiError::EmptyCapture`] on an empty selection — never
/// silently mints an empty scope.
pub fn capture_scope(
    grant: &mut Grant,
    selection: &EditingSelection,
    revision: ProjectRevision,
) -> Result<usize, GatewayUiError> {
    let resources: std::collections::BTreeSet<ResourceRef> = selection
        .targets
        .iter()
        .map(|t| match t {
            SemanticTarget::Track(id) => ResourceRef::Track(*id),
            SemanticTarget::Lane(id) => ResourceRef::Lane(*id),
            SemanticTarget::Clip(id) => ResourceRef::Clip(*id),
            SemanticTarget::Source(id) => ResourceRef::Source(*id),
        })
        .collect();
    if resources.is_empty() {
        return Err(GatewayUiError::EmptyCapture);
    }
    let members = resources.len();
    grant.scope = ResourceScope::Snapshot {
        resources,
        captured_revision: revision,
    };
    Ok(members)
}

// ══════════════════════════════════════════════════════════════════
// connection / task / grant rows (explicit ids, UX-04)
// ══════════════════════════════════════════════════════════════════

/// A projected connection-permit row. A permit binds *identity and
/// lifetime only* — no capabilities, no data access (SYS-05).
#[derive(Clone, Debug)]
pub struct ConnectionRow {
    /// The permit id.
    pub permit: String,
    /// The channel it binds.
    pub channel: McpChannel,
    /// The bound principal's display id.
    pub principal: String,
    /// The principal kind (human/agent).
    pub kind: PrincipalKind,
    /// The bound project.
    pub project: ProjectId,
    /// The optional task binding.
    pub task: Option<TaskRef>,
    /// The optional grant binding.
    pub grant: Option<GrantRef>,
    /// The permit expiry, when declared.
    pub expires_at: Option<PolicyInstant>,
    /// The authority's revoked flag.
    pub revoked: bool,
    /// `!revoked && !expired` at read time.
    pub live: bool,
    /// The UI's revocation axis for this permit.
    pub revocation: Revocation,
}

/// A projected authorized-task row.
#[derive(Clone, Debug)]
pub struct TaskRow {
    /// The task id.
    pub task: TaskRef,
    /// The bound principal's display id.
    pub principal: String,
    /// The task's activation class (`user-issued` / `proactive`).
    pub activation: &'static str,
    /// The expiry instant, when declared.
    pub expires_at: Option<PolicyInstant>,
    /// The authority's revoked flag.
    pub revoked: bool,
    /// `!expired && !revoked` at read time.
    pub live: bool,
    /// The UI's revocation axis.
    pub revocation: Revocation,
}

/// A projected grant row — the authority record plus the UI's
/// revocation axis.
#[derive(Clone, Debug)]
pub struct GrantRow {
    /// The grant id.
    pub grant: GrantRef,
    /// The bound principal's display id.
    pub principal: String,
    /// The binding summary (`task:*` / `session` / `persistent`).
    pub binding: String,
    /// The derived apply mode.
    pub mode: ApplyMode,
    /// The scope projection.
    pub scope: ScopeExplanation,
    /// Capability count.
    pub capabilities: usize,
    /// Applies consumed against the budget.
    pub consumed_applies: u64,
    /// The expiry instant, when declared.
    pub expires_at: Option<PolicyInstant>,
    /// The authority's revoked flag.
    pub revoked: bool,
    /// `!expired && !revoked` at read time.
    pub live: bool,
    /// The UI's revocation axis.
    pub revocation: Revocation,
}

fn binding_summary(binding: &GrantBinding) -> String {
    match binding {
        GrantBinding::Task { task } => format!("task:{}", task.as_str()),
        GrantBinding::Session { .. } => "session".to_owned(),
        GrantBinding::PersistentProject => "persistent-project".to_owned(),
    }
}

fn activation_name(a: &Activation) -> &'static str {
    match a {
        Activation::UserIssued => "user-issued",
        Activation::Proactive { .. } => "proactive",
    }
}

// ══════════════════════════════════════════════════════════════════
// approval surface (UX-03) — a pinned review card
// ══════════════════════════════════════════════════════════════════

/// One normalized command row in the approval surface.
#[derive(Clone, Debug)]
pub struct CommandRow {
    /// The semantic capability ref.
    pub capability: String,
    /// The lowered `ProjectOp` variant tag.
    pub op: &'static str,
    /// The bound explicit target (display string).
    pub target: String,
    /// The schema-validated args.
    pub args: Value,
}

/// One derived effect row — reads are disclosure-only, writes show
/// before→after.
#[derive(Clone, Debug)]
pub struct EffectRow {
    /// The effect kind.
    pub kind: EffectKind,
    /// The affected resource (tagged).
    pub resource: String,
    /// `created` / `modified` / `deleted` — or `read` for disclosure.
    pub change: &'static str,
    /// The pinned before-state, when visible.
    pub before: Option<ResourceAttributes>,
    /// The candidate after-state, when visible.
    pub after: Option<ResourceAttributes>,
    /// Related resources the effect touches (linked clips/sources —
    /// display-only provenance).
    pub references: Vec<String>,
}

/// A linked-source effect row (UX-02/UX-03): a shared-source write is
/// shown with the clips it lands on, and the card makes explicit that
/// a clip-scope grant never authorizes source writes on its own.
#[derive(Clone, Debug)]
pub struct LinkedEffect {
    /// The shared source (tagged).
    pub source: String,
    /// The clips the effect references.
    pub clips: Vec<String>,
}

/// One MIDI-note diff row — recovered from a source effect's stored
/// detail (the GW-04 observation contract, projected for review).
#[derive(Clone, Debug, PartialEq)]
pub struct MidiNoteRow {
    /// The source (tagged).
    pub source: String,
    /// The note key.
    pub note: String,
    /// `added` / `removed` / `modified`.
    pub change: &'static str,
}

/// The approval-surface decision state.
#[derive(Clone, Debug, Default)]
pub enum SurfaceDecision {
    /// Awaiting the user's choice.
    #[default]
    Pending,
    /// Approved — carries the issued receipt record.
    Granted(ApprovalRecord),
    /// The user declined.
    Declined,
    /// The gateway refused the action (proposal dead, grant expired,
    /// digest mismatch…).
    Refused {
        /// The typed refusal code.
        code: GatewayError,
        /// The safe-to-disclose message.
        message: String,
    },
}

/// The pinned approval-review surface (UX-03). At open time it pins
/// `(proposal, digest)` and snapshots what the user reviews — the
/// scope/effect presentation, bound grant/task, expiry. Selection,
/// focus, other-window changes, and proposal replacement cannot
/// silently retarget it: the pinned digest is asserted on every action
/// (SYS-03).
pub struct ApprovalSurface {
    /// The pinned proposal.
    proposal: ProposalRef,
    /// The asserted changeset digest.
    digest: ContentDigest,
    /// The proposal owner (whose preparation produced it).
    owner: Principal,
    /// The project under review.
    project: ProjectId,
    /// The committed base revision asserted.
    base_revision: ProjectRevision,
    /// The tempo-map revision asserted.
    tempo_map_revision: u64,
    /// The proposal expiry.
    expires_at: PolicyInstant,
    /// The proposal status at last refresh — `None` when the proposal
    /// is no longer in the store (evicted or never recorded); a gone
    /// view is never presented as a specific status.
    status: Option<ProposalStatus>,
    /// The normalized command list.
    commands: Vec<CommandRow>,
    /// The derived effect rows.
    effects: Vec<EffectRow>,
    /// The linked-source effects.
    linked_sources: Vec<LinkedEffect>,
    /// The MIDI-note diff rows recovered from source effects.
    midi_notes: Vec<MidiNoteRow>,
    /// Effects that could not be lowered.
    unsupported: Vec<String>,
    /// Whether the entity diff projection is complete (no unsupported
    /// effects).
    diff_complete: bool,
    /// The bound grant (from the prepared request), when known.
    grant: Option<GrantRef>,
    /// The bound task, when declared.
    task: Option<TaskRef>,
    /// The bound grant's explanation at open/refresh time.
    grant_explanation: Option<GrantExplanation>,
    /// The approval status at refresh time.
    approval_status: Option<ApprovalStatus>,
    /// The apply status at refresh time.
    apply_status: Option<ApplyStatus>,
    /// The decision state.
    decision: SurfaceDecision,
}

impl ApprovalSurface {
    /// The pinned proposal id.
    #[must_use]
    pub fn proposal(&self) -> &ProposalRef {
        &self.proposal
    }

    /// The asserted changeset digest — the approval pin.
    #[must_use]
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    /// Whose preparation this reviews.
    #[must_use]
    pub fn owner(&self) -> &Principal {
        &self.owner
    }

    /// The project under review.
    #[must_use]
    pub fn project(&self) -> ProjectId {
        self.project
    }

    /// The committed base revision asserted.
    #[must_use]
    pub fn base_revision(&self) -> ProjectRevision {
        self.base_revision
    }

    /// The tempo-map revision asserted — part of the dependency
    /// identity the digest binds.
    #[must_use]
    pub fn tempo_map_revision(&self) -> u64 {
        self.tempo_map_revision
    }

    /// The proposal expiry instant.
    #[must_use]
    pub fn expires_at(&self) -> PolicyInstant {
        self.expires_at
    }

    /// The proposal status at last refresh (`None` = no longer in the
    /// store).
    #[must_use]
    pub fn status(&self) -> Option<ProposalStatus> {
        self.status
    }

    /// The normalized command rows.
    #[must_use]
    pub fn commands(&self) -> &[CommandRow] {
        &self.commands
    }

    /// The derived effect rows.
    #[must_use]
    pub fn effects(&self) -> &[EffectRow] {
        &self.effects
    }

    /// The linked-source effect rows.
    #[must_use]
    pub fn linked_sources(&self) -> &[LinkedEffect] {
        &self.linked_sources
    }

    /// The MIDI-note diff rows.
    #[must_use]
    pub fn midi_notes(&self) -> &[MidiNoteRow] {
        &self.midi_notes
    }

    /// Effects that could not be lowered.
    #[must_use]
    pub fn unsupported(&self) -> &[String] {
        &self.unsupported
    }

    /// Whether the entity-diff projection is complete.
    #[must_use]
    pub fn diff_complete(&self) -> bool {
        self.diff_complete
    }

    /// The bound grant ref, when the prepared request carried one.
    #[must_use]
    pub fn grant(&self) -> Option<&GrantRef> {
        self.grant.as_ref()
    }

    /// The bound task ref.
    #[must_use]
    pub fn task(&self) -> Option<&TaskRef> {
        self.task.as_ref()
    }

    /// The bound grant's explanation — scope/expiry/budget/mode at
    /// approval-review time. `None` when the proposal is no longer
    /// bound to a retrievable grant.
    #[must_use]
    pub fn grant_explanation(&self) -> Option<&GrantExplanation> {
        self.grant_explanation.as_ref()
    }

    /// The approval status at last refresh.
    #[must_use]
    pub fn approval_status(&self) -> Option<ApprovalStatus> {
        self.approval_status
    }

    /// The apply status at last refresh.
    #[must_use]
    pub fn apply_status(&self) -> Option<ApplyStatus> {
        self.apply_status
    }

    /// The decision state.
    #[must_use]
    pub fn decision(&self) -> &SurfaceDecision {
        &self.decision
    }
}

// ══════════════════════════════════════════════════════════════════
// activity feed — distinct status axes (UX-04/05, SYS-05/06)
// ══════════════════════════════════════════════════════════════════

/// The commit-axis verdict for one apply operation.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum CommitVerdict {
    /// No commit outcome yet (in flight or commit-requested).
    Pending,
    /// Durable commit landed.
    Committed {
        /// The commit identity.
        commit: String,
        /// The committed revision.
        revision: u64,
        /// The journal sequence.
        journal_seq: u64,
    },
    /// Authorization/validation refused.
    Rejected {
        /// The reason.
        reason: String,
    },
    /// Expected-revision conflict at the commit boundary.
    Conflicted {
        /// The asserted revision.
        expected: u64,
        /// The live revision.
        actual: u64,
    },
    /// Outcome cannot be determined — never report as success.
    Indeterminate {
        /// The reason.
        reason: String,
    },
    /// Nothing is recorded for this operation (no commit boundary, or
    /// a pre-boundary failure).
    Unknown,
}

impl CommitVerdict {
    /// Projects a durable [`CommitOutcome`] into the display verdict.
    #[must_use]
    pub fn of(outcome: &CommitOutcome) -> Self {
        match outcome {
            CommitOutcome::Committed(r) => Self::Committed {
                commit: r.commit.to_tagged(),
                revision: r.revision.0,
                journal_seq: r.journal_seq,
            },
            CommitOutcome::Rejected { reason } => Self::Rejected {
                reason: reason.clone(),
            },
            CommitOutcome::Conflicted { expected, actual } => Self::Conflicted {
                expected: expected.0,
                actual: actual.0,
            },
            CommitOutcome::Indeterminate { reason } => Self::Indeterminate {
                reason: reason.clone(),
            },
        }
    }
}

/// The engine-application axis — separate from commit (SYS-06): a
/// committed mutation is journal-durable *before* the engine consumes
/// it. The workspace reports engine events through `note_engine`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum EngineApplyState {
    /// No engine-side application reported yet.
    #[default]
    Pending,
    /// The engine graph consumed the mutation.
    Applied {
        /// The engine-visible revision.
        at_revision: u64,
    },
    /// The engine reported a failure — project state stays committed;
    /// this row carries the honest axis.
    Failed {
        /// The reported reason.
        reason: String,
    },
}

/// One projected activity row — the proposal's full lifecycle with
/// every status axis kept distinct (UX-04/UX-05). `None` fields mean
/// "not applicable yet", never "assumed".
#[derive(Clone, Debug)]
pub struct OperationRow {
    /// The proposal id.
    pub proposal: ProposalRef,
    /// The proposal status (`None` = evicted from the store).
    pub status: Option<ProposalStatus>,
    /// The approval status, when the proposal has an approval record.
    pub approval: Option<ApprovalStatus>,
    /// The apply status, when an apply exists.
    pub apply: Option<ApplyStatus>,
    /// The apply's operation id.
    pub operation: Option<OperationId>,
    /// The commit-axis verdict.
    pub commit: Option<CommitVerdict>,
    /// The engine-application axis.
    pub engine: Option<EngineApplyState>,
    /// The earliest save/publication covering the committed revision —
    /// `None` means "committed but not yet published".
    pub saved: Option<SaveRecord>,
}

/// One projected job row — `phase` (provider truth) and `cancel`
/// (request linkage) are distinct axes (SYS-05).
#[derive(Clone, Debug)]
pub struct JobRow {
    /// The operation id.
    pub operation: OperationId,
    /// The provider job ref.
    pub job: JobRef,
    /// The last provider-reported phase.
    pub phase: JobPhase,
    /// The cancel linkage — `NotRequested` / `Requested` / `Confirmed`
    /// / `Unconfirmed`. A `Requested` cancel never displays as
    /// cancelled.
    pub cancel: CancelLink,
}

impl JobRow {
    fn of(record: &JobRecord) -> Self {
        Self {
            operation: record.operation,
            job: record.job.clone(),
            phase: record.last_phase.clone(),
            cancel: record.cancel,
        }
    }
}

/// The result of a UI-driven apply.
#[derive(Clone, Debug)]
pub struct ApplyReport {
    /// The apply operation id.
    pub operation: OperationId,
    /// The commit verdict reached by driving the commit seam.
    pub verdict: CommitVerdict,
}

// ══════════════════════════════════════════════════════════════════
// the session composer
// ══════════════════════════════════════════════════════════════════

/// The seams `GatewayUi` drives — the UI never reaches store internals
/// itself; it works through the same typed boundaries the MCP adapter
/// uses (plus the proposal engine's decline seam).
pub struct GatewayUiDeps {
    /// The evaluation-time policy surface (read rows).
    pub policy: Arc<dyn PolicyStore>,
    /// The host administration surface (issue/revoke, live revision).
    pub admin: Arc<dyn AdminPort>,
    /// The connection-permit store.
    pub permits: Box<dyn PermitStore>,
    /// The proposal/approval/apply store (read projections).
    pub proposals: Arc<ProposalStore>,
    /// The GW-05 proposal engine (decline seam).
    pub proposal_engine: Arc<ProposalEngine>,
    /// The commit-boundary seam driven after an `Accepted` apply.
    pub committer: Arc<dyn CommitPort>,
    /// The job engine for grant-revocation cancel linkage and job
    /// rows (`None` = no job coordinator wired).
    pub jobs: Option<Arc<JobEngine>>,
    /// The policy clock.
    pub clock: Arc<dyn PolicyClock>,
}

/// The headless GW-09 session — one per manual-UI workspace session.
///
/// Owns the caller binding (trusted human intent), the AI/MCP
/// switches, the explicit-id row projections, the approval surfaces,
/// and the activity feed. Every gateway-facing action goes through a
/// typed `Gateway` call with the bound caller — never a fabricated
/// request.
pub struct GatewayUi {
    caller: CallerIdentity,
    gateway: Gateway,
    policy: Arc<dyn PolicyStore>,
    admin: Arc<dyn AdminPort>,
    permits: Mutex<Box<dyn PermitStore>>,
    proposals: Arc<ProposalStore>,
    proposal_engine: Arc<ProposalEngine>,
    committer: Arc<dyn CommitPort>,
    jobs: Option<Arc<JobEngine>>,
    clock: Arc<dyn PolicyClock>,

    switches: AiSwitches,
    connections: Vec<String>,
    tasks: Vec<TaskRef>,
    grants: Vec<GrantRef>,
    known_proposals: Vec<ProposalRef>,
    surfaces: BTreeMap<u64, ApprovalSurface>,
    next_surface: u64,
    revocations: BTreeMap<RevocationTarget, Revocation>,
    job_operations: Vec<OperationId>,
    engine_states: BTreeMap<OperationId, EngineApplyState>,
    publications: Vec<SaveRecord>,
    next_request: u64,
}

impl GatewayUi {
    /// Binds a UI session to a trusted `caller` and its seams.
    #[must_use]
    pub fn new(caller: CallerIdentity, gateway: Gateway, deps: GatewayUiDeps) -> Self {
        Self {
            caller,
            gateway,
            policy: deps.policy,
            admin: deps.admin,
            permits: Mutex::new(deps.permits),
            proposals: deps.proposals,
            proposal_engine: deps.proposal_engine,
            committer: deps.committer,
            jobs: deps.jobs,
            clock: deps.clock,
            switches: AiSwitches::new(),
            connections: Vec::new(),
            tasks: Vec::new(),
            grants: Vec::new(),
            known_proposals: Vec::new(),
            surfaces: BTreeMap::new(),
            next_surface: 0,
            revocations: BTreeMap::new(),
            job_operations: Vec::new(),
            engine_states: BTreeMap::new(),
            publications: Vec::new(),
            next_request: 0,
        }
    }

    // ── seams the fixture/host needs ──────────────────────────────

    /// The bound caller (for fixture-side calls that reuse the same
    /// trusted identity).
    #[must_use]
    pub fn caller(&self) -> &CallerIdentity {
        &self.caller
    }

    /// The gateway — mutable access for fixture-side agent calls.
    pub fn gateway_mut(&mut self) -> &mut Gateway {
        &mut self.gateway
    }

    /// The proposal store the UI reads projections from.
    #[must_use]
    pub fn proposals(&self) -> &Arc<ProposalStore> {
        &self.proposals
    }

    /// The policy store.
    #[must_use]
    pub fn policy(&self) -> &Arc<dyn PolicyStore> {
        &self.policy
    }

    /// The policy clock.
    #[must_use]
    pub fn clock(&self) -> &Arc<dyn PolicyClock> {
        &self.clock
    }

    // ── switches ──────────────────────────────────────────────────

    /// The current switch panel (UX-01).
    #[must_use]
    pub fn switch_panel(&self) -> AiPanelView {
        self.switches.panel()
    }

    /// Toggles a feature; returns the projected panel.
    pub fn set_switch(&mut self, feature: AiFeature, enabled: bool) -> AiPanelView {
        self.switches.set(feature, enabled)
    }

    /// The MCP config derived from the switches — `enabled` follows
    /// the independent external-MCP toggle (never the AI master).
    #[must_use]
    pub fn mcp_config(&self, adapter: AdapterId, bounds: McpBounds) -> McpConfig {
        McpConfig {
            adapter,
            bounds,
            enabled: self.switches.enabled(AiFeature::ExternalMcp),
        }
    }

    // ── connections (permits) ─────────────────────────────────────

    /// Issues a connection permit through the trusted store —
    /// channel/kind coherence is checked before issuance
    /// (`check_channel_kind`), and the returned secret is the one-time
    /// credential (the UI stores only the id).
    ///
    /// # Errors
    /// [`GatewayUiError::Gateway`] on channel/kind incoherence;
    /// [`GatewayUiError::PermitStore`] on store failure.
    pub fn issue_connection(&mut self, spec: PermitSpec) -> Result<PermitIssue, GatewayUiError> {
        check_channel_kind(&spec)
            .map_err(|code| GatewayUiError::Gateway(ErrorEnvelope::from(code)))?;
        let mut permits = self.permits.lock().unwrap_or_else(PoisonError::into_inner);
        let issue = permits.issue(spec, self.clock.now())?;
        self.connections.push(issue.record.permit.clone());
        Ok(issue)
    }

    /// Requests revocation of a connection permit — records intent,
    /// then confirms against the store (SYS-05). A store `false`
    /// (unknown permit) leaves the axis `Requested`, honestly.
    ///
    /// # Errors
    /// [`GatewayUiError::PermitStore`] — the axis stays `Requested`.
    pub fn revoke_connection(&mut self, permit: &str) -> Result<RevocationReport, GatewayUiError> {
        let target = RevocationTarget::Permit(permit.to_owned());
        self.revocations
            .insert(target.clone(), Revocation::Requested);
        let mut permits = self.permits.lock().unwrap_or_else(PoisonError::into_inner);
        let confirmed = permits.revoke(permit)?;
        drop(permits);
        let state = if confirmed {
            Revocation::Confirmed
        } else {
            Revocation::Requested
        };
        self.revocations.insert(target, state);
        Ok(RevocationReport {
            state,
            cancel_requests: Vec::new(),
        })
    }

    /// The connection rows (authority + UI axis).
    #[must_use]
    pub fn connection_rows(&self) -> Vec<ConnectionRow> {
        let permits = self.permits.lock().unwrap_or_else(PoisonError::into_inner);
        let now = self.clock.now();
        self.connections
            .iter()
            .filter_map(|id| {
                permits.record(id).map(|r| ConnectionRow {
                    permit: r.permit.clone(),
                    channel: r.channel,
                    principal: r.principal.id().to_owned(),
                    kind: r.principal.kind(),
                    project: r.project,
                    task: r.task.clone(),
                    grant: r.grant.clone(),
                    expires_at: r.expires_at,
                    revoked: r.revoked,
                    live: r.live(now),
                    revocation: self
                        .revocations
                        .get(&RevocationTarget::Permit(r.permit.clone()))
                        .copied()
                        .unwrap_or_default(),
                })
            })
            .collect()
    }

    // ── tasks ─────────────────────────────────────────────────────

    /// Issues an authorized task through the admin surface.
    ///
    /// # Errors
    /// [`GatewayUiError::Admin`] on admin refusal.
    pub fn issue_task(&mut self, task: &AuthorizedTask) -> Result<(), GatewayUiError> {
        self.admin
            .issue_task(task.clone())
            .map_err(GatewayUiError::Admin)?;
        self.tasks.push(task.task.clone());
        Ok(())
    }

    /// Requests task revocation (intent → confirmation).
    ///
    /// # Errors
    /// [`GatewayUiError::Admin`] — the axis stays `Requested`.
    pub fn revoke_task(&mut self, task: &TaskRef) -> Result<RevocationReport, GatewayUiError> {
        let target = RevocationTarget::Task(task.clone());
        self.revocations
            .insert(target.clone(), Revocation::Requested);
        self.admin
            .revoke_task(task)
            .map_err(GatewayUiError::Admin)?;
        self.revocations.insert(target, Revocation::Confirmed);
        Ok(RevocationReport {
            state: Revocation::Confirmed,
            cancel_requests: Vec::new(),
        })
    }

    /// The task rows (authority + UI axis).
    #[must_use]
    pub fn task_rows(&self) -> Vec<TaskRow> {
        let now = self.clock.now();
        self.tasks
            .iter()
            .filter_map(|id| {
                self.policy.task(id).map(|task| TaskRow {
                    task: task.task.clone(),
                    principal: task.principal.id().to_owned(),
                    activation: activation_name(&task.activation),
                    expires_at: task.validity.expires_at,
                    revoked: task.validity.revoked,
                    live: task.validity.live(now),
                    revocation: self
                        .revocations
                        .get(&RevocationTarget::Task(task.task.clone()))
                        .copied()
                        .unwrap_or_default(),
                })
            })
            .collect()
    }

    // ── grants ────────────────────────────────────────────────────

    /// Issues a grant through the admin surface — the `Grant` arrives
    /// fully formed (a `capture_scope` assist has already fixed any
    /// snapshot membership).
    ///
    /// # Errors
    /// [`GatewayUiError::Admin`] on admin refusal.
    pub fn issue_grant(&mut self, grant: &Grant) -> Result<(), GatewayUiError> {
        self.admin
            .issue_grant(grant.clone())
            .map_err(GatewayUiError::Admin)?;
        self.grants.push(grant.id.clone());
        Ok(())
    }

    /// Requests grant revocation: records intent, confirms via the
    /// admin surface, then forwards the revocation to the job
    /// coordinator so live jobs admitted under the grant get a cancel
    /// *request* (SYS-05 — never a fabricated `Cancelled`).
    ///
    /// # Errors
    /// [`GatewayUiError::Admin`] — the axis stays `Requested` and no
    /// cancel request is sent.
    pub fn revoke_grant(&mut self, grant: &GrantRef) -> Result<RevocationReport, GatewayUiError> {
        let target = RevocationTarget::Grant(grant.clone());
        self.revocations
            .insert(target.clone(), Revocation::Requested);
        self.admin
            .revoke_grant(grant)
            .map_err(GatewayUiError::Admin)?;
        self.revocations.insert(target, Revocation::Confirmed);
        let cancel_requests = self
            .jobs
            .as_ref()
            .map(|j| j.note_grant_revoked(grant))
            .unwrap_or_default();
        Ok(RevocationReport {
            state: Revocation::Confirmed,
            cancel_requests,
        })
    }

    /// The grant rows (authority + UI axis).
    #[must_use]
    pub fn grant_rows(&self) -> Vec<GrantRow> {
        let now = self.clock.now();
        self.grants
            .iter()
            .filter_map(|id| {
                self.policy.grant(id).map(|record| GrantRow {
                    grant: record.grant.id.clone(),
                    principal: record.grant.principal.id().to_owned(),
                    binding: binding_summary(&record.grant.binding),
                    mode: ApplyMode::of(record.grant.approval_policy),
                    scope: scope_explanation(&record.grant.scope),
                    capabilities: record.grant.capabilities.len(),
                    consumed_applies: record.consumed_applies,
                    expires_at: record.grant.validity.expires_at,
                    revoked: record.grant.validity.revoked,
                    live: record.grant.validity.live(now),
                    revocation: self
                        .revocations
                        .get(&RevocationTarget::Grant(record.grant.id.clone()))
                        .copied()
                        .unwrap_or_default(),
                })
            })
            .collect()
    }

    /// The explanation for one grant — `None` when unknown.
    #[must_use]
    pub fn grant_explanation(&self, grant: &GrantRef) -> Option<GrantExplanation> {
        self.policy
            .grant(grant)
            .map(|r| explain_grant(&r, self.clock.now()))
    }

    // ── proposals / approval surfaces ─────────────────────────────

    /// Records a proposal the UI should track in the activity feed
    /// (the host reports the prepare's `ProposalReady` ref).
    pub fn note_proposal(&mut self, proposal: ProposalRef) {
        if !self.known_proposals.contains(&proposal) {
            self.known_proposals.push(proposal);
        }
    }

    /// Opens the pinned approval-review surface for `proposal`,
    /// asserting the caller's `asserted_digest` — a swapped or stale
    /// proposal fails before a card exists (SYS-03).
    ///
    /// # Errors
    /// [`GatewayUiError::ProposalGone`] / [`GatewayUiError::DigestMismatch`].
    pub fn open_approval(
        &mut self,
        proposal: &ProposalRef,
        asserted_digest: &str,
    ) -> Result<u64, GatewayUiError> {
        let view = self
            .proposals
            .view(proposal)
            .ok_or_else(|| GatewayUiError::ProposalGone(proposal.clone()))?;
        if view.changeset.digest.as_str() != asserted_digest {
            return Err(GatewayUiError::DigestMismatch {
                proposal: proposal.clone(),
            });
        }
        self.next_surface += 1;
        let id = self.next_surface;
        let surface = self.build_surface(&view);
        self.surfaces.insert(id, surface);
        Ok(id)
    }

    /// The surface, if open.
    #[must_use]
    pub fn surface(&self, id: u64) -> Option<&ApprovalSurface> {
        self.surfaces.get(&id)
    }

    /// Re-reads the authority state for a surface — proposal status,
    /// approval/apply status, grant liveness at approval-review time.
    ///
    /// # Errors
    /// [`GatewayUiError::UnknownSurface`].
    ///
    /// # Panics
    /// Only if a surface validated by the `UnknownSurface` check
    /// disappears before the same call mutates it — unreachable in
    /// this single-threaded mutation path.
    pub fn refresh_surface(&mut self, id: u64) -> Result<(), GatewayUiError> {
        let (proposal, grant) = {
            let surface = self
                .surfaces
                .get(&id)
                .ok_or(GatewayUiError::UnknownSurface(id))?;
            (surface.proposal.clone(), surface.grant.clone())
        };
        let view = self.proposals.view(&proposal);
        let grant_explanation = grant
            .as_ref()
            .and_then(|g| self.policy.grant(g))
            .map(|r| explain_grant(&r, self.clock.now()));
        let surface = self.surfaces.get_mut(&id).expect("surface present");
        surface.status = view.as_ref().map(|v| v.status);
        surface.approval_status = view
            .as_ref()
            .and_then(|v| v.approval.as_ref())
            .map(|a| a.status);
        surface.apply_status = view
            .as_ref()
            .and_then(|v| v.apply.as_ref())
            .map(|a| a.status);
        surface.grant_explanation = grant_explanation;
        Ok(())
    }

    /// Human approval — `synvelis.proposal.approve` over the gateway
    /// wire with the bound caller. The surface asserts the pinned
    /// proposal + digest + reviewed base revision; a refused call
    /// records `Refused` on the surface (SYS-03).
    ///
    /// # Errors
    /// [`GatewayUiError::Gateway`] on refusal — the surface records the
    /// typed `Refused` decision for display.
    pub fn approve(&mut self, id: u64) -> Result<SurfaceDecision, GatewayUiError> {
        let (project, proposal, digest, base) = {
            let s = self
                .surfaces
                .get(&id)
                .ok_or(GatewayUiError::UnknownSurface(id))?;
            (
                s.project,
                s.proposal.clone(),
                s.digest.as_str().to_owned(),
                s.base_revision,
            )
        };
        let wire = self.wire(
            APPROVE_ID,
            project,
            &json!({"proposal": proposal.as_str(), "digest": digest}),
            &expected_revision(base),
        );
        match self.gateway.handle(&wire, self.caller.clone()) {
            Ok(GatewayOutcome::Completed { .. }) => {
                let approval = self.proposals.view(&proposal).and_then(|v| v.approval);
                let decision = match approval {
                    Some(record) => SurfaceDecision::Granted(record),
                    None => SurfaceDecision::Refused {
                        code: GatewayError::Internal {
                            reason: "approve completed but no receipt is recorded".to_owned(),
                        },
                        message: "receipt missing".to_owned(),
                    },
                };
                self.set_decision(id, decision.clone());
                Ok(decision)
            }
            Ok(other) => {
                let decision = SurfaceDecision::Refused {
                    code: GatewayError::Internal {
                        reason: "approve produced a non-completed outcome".to_owned(),
                    },
                    message: format!("{other:?}"),
                };
                self.set_decision(id, decision.clone());
                Err(GatewayUiError::Gateway(internal(
                    "unexpected approve outcome",
                )))
            }
            Err(env) => {
                let decision = SurfaceDecision::Refused {
                    code: env.code.clone(),
                    message: env.message.clone(),
                };
                self.set_decision(id, decision);
                Err(GatewayUiError::Gateway(env))
            }
        }
    }

    /// Human decline — the approval store records the terminal
    /// `Declined` status through the proposal engine.
    ///
    /// # Errors
    /// [`GatewayUiError::Gateway`] on refusal.
    pub fn decline(&mut self, id: u64) -> Result<SurfaceDecision, GatewayUiError> {
        let proposal = {
            let s = self
                .surfaces
                .get(&id)
                .ok_or(GatewayUiError::UnknownSurface(id))?;
            s.proposal.clone()
        };
        match self.proposal_engine.decline_approval(&proposal) {
            Ok(_record) => {
                let decision = SurfaceDecision::Declined;
                self.set_decision(id, decision.clone());
                Ok(decision)
            }
            Err(env) => {
                let decision = SurfaceDecision::Refused {
                    code: env.code.clone(),
                    message: env.message.clone(),
                };
                self.set_decision(id, decision);
                Err(GatewayUiError::Gateway(env))
            }
        }
    }

    /// Human apply — `synvelis.proposal.apply` over the wire with the
    /// bound caller, then the commit seam drives the `Accepted`
    /// operation to its verdict (SYS-06: an `Accepted` apply is *not*
    /// committed until the gate settles it).
    ///
    /// # Errors
    /// [`GatewayUiError::Gateway`] on refusal; the surface's decision
    /// records the typed `Refused` for display.
    pub fn apply(&mut self, id: u64) -> Result<ApplyReport, GatewayUiError> {
        let (project, proposal, digest, base) = {
            let s = self
                .surfaces
                .get(&id)
                .ok_or(GatewayUiError::UnknownSurface(id))?;
            (
                s.project,
                s.proposal.clone(),
                s.digest.as_str().to_owned(),
                s.base_revision,
            )
        };
        let wire = self.wire(
            APPLY_ID,
            project,
            &json!({"proposal": proposal.as_str(), "digest": digest}),
            &expected_revision(base),
        );
        match self.gateway.handle(&wire, self.caller.clone()) {
            Ok(GatewayOutcome::Accepted { operation }) => {
                let verdict = self.drive_commit(&proposal, operation);
                Ok(ApplyReport { operation, verdict })
            }
            Ok(GatewayOutcome::ApprovalRequired { approval }) => {
                Err(GatewayUiError::NeedsApproval {
                    approval: approval.as_str().to_owned(),
                })
            }
            Ok(GatewayOutcome::Completed { .. } | GatewayOutcome::ProposalReady { .. }) => Err(
                GatewayUiError::Gateway(internal("unexpected apply outcome")),
            ),
            Err(env) => {
                let decision = SurfaceDecision::Refused {
                    code: env.code.clone(),
                    message: env.message.clone(),
                };
                self.set_decision(id, decision);
                Err(GatewayUiError::Gateway(env))
            }
        }
    }

    /// Requests job cancellation for one operation — a *request*,
    /// recorded honestly as `Requested`/`Confirmed`/`Unconfirmed`
    /// (SYS-05); never fabricates `Cancelled`.
    ///
    /// # Errors
    /// [`GatewayUiError::Gateway`] on refusal or when `operation` is
    /// not a tracked job.
    pub fn cancel_job(
        &mut self,
        operation: OperationId,
    ) -> Result<GatewayOutcome<Value>, GatewayUiError> {
        let Some(project) = self.project_of_job(operation) else {
            return Err(GatewayUiError::Gateway(internal("unknown job operation")));
        };
        let wire = self.wire(
            JOB_CANCEL_ID,
            project,
            &json!({"operation": operation.to_tagged()}),
            &json!({"kind": "unconditional"}),
        );
        self.gateway
            .handle(&wire, self.caller.clone())
            .map_err(GatewayUiError::Gateway)
    }

    // ── activity feed ─────────────────────────────────────────────

    /// The projected activity rows — one per tracked proposal, with
    /// every axis distinct (UX-04/UX-05). A proposal evicted from the
    /// store still appears (`status: None`).
    #[must_use]
    pub fn operation_rows(&self) -> Vec<OperationRow> {
        self.known_proposals
            .iter()
            .map(|p| {
                let view = self.proposals.view(p);
                let apply = view.as_ref().and_then(|v| v.apply.as_ref());
                let (operation, commit, engine) = match apply {
                    Some(a) => {
                        let op = a.operation;
                        (
                            Some(op),
                            Some(self.commit_verdict(op)),
                            Some(self.engine_states.get(&op).cloned().unwrap_or_default()),
                        )
                    }
                    None => (None, None, None),
                };
                let saved = match (&view, &commit) {
                    (
                        Some(v),
                        Some(CommitVerdict::Committed {
                            revision: committed,
                            ..
                        }),
                    ) => self
                        .publications
                        .iter()
                        .filter(|s| {
                            s.project == v.changeset.base.project && s.revision.0 >= *committed
                        })
                        .min_by_key(|s| s.revision.0)
                        .cloned(),
                    _ => None,
                };
                OperationRow {
                    proposal: p.clone(),
                    status: view.as_ref().map(|v| v.status),
                    approval: view
                        .as_ref()
                        .and_then(|v| v.approval.as_ref())
                        .map(|a| a.status),
                    apply: apply.map(|a| a.status),
                    operation,
                    commit,
                    engine,
                    saved,
                }
            })
            .collect()
    }

    /// Records a job admission the host observed (the fixture calls
    /// this after an `Accepted` `synvelis.job.submit`).
    pub fn note_job(&mut self, operation: OperationId) {
        if !self.job_operations.contains(&operation) {
            self.job_operations.push(operation);
        }
    }

    /// The projected job rows (SYS-05: `phase` and `cancel` stay
    /// distinct).
    #[must_use]
    pub fn job_rows(&self) -> Vec<JobRow> {
        self.job_operations
            .iter()
            .filter_map(|op| self.jobs.as_ref().and_then(|j| j.record(*op)))
            .map(|r| JobRow::of(&r))
            .collect()
    }

    /// Reports an engine-application event for an applied operation —
    /// the honest axis separate from commit (SYS-06).
    pub fn note_engine(&mut self, operation: OperationId, state: EngineApplyState) {
        self.engine_states.insert(operation, state);
    }

    /// Records a save/publication event (SYS-06 — `Saved` is a
    /// distinct receipt from `Committed`).
    pub fn note_saved(&mut self, record: SaveRecord) {
        self.publications.push(record);
    }

    /// The recorded publications.
    #[must_use]
    pub fn publications(&self) -> &[SaveRecord] {
        &self.publications
    }

    // ── internals ─────────────────────────────────────────────────

    fn set_decision(&mut self, id: u64, decision: SurfaceDecision) {
        if let Some(s) = self.surfaces.get_mut(&id) {
            s.decision = decision;
        }
    }

    fn build_surface(&self, view: &ProposalView) -> ApprovalSurface {
        let cs = &view.changeset;
        let grant = view
            .record
            .as_ref()
            .and_then(|r| r.request.grant().cloned());
        let task = view.record.as_ref().and_then(|r| r.request.task().cloned());
        let grant_explanation = grant
            .as_ref()
            .and_then(|g| self.policy.grant(g))
            .map(|r| explain_grant(&r, self.clock.now()));
        ApprovalSurface {
            proposal: view.proposal.clone(),
            digest: cs.digest.clone(),
            owner: view.owner.clone(),
            project: cs.base.project,
            base_revision: cs.base.revision,
            tempo_map_revision: cs.base.tempo_map_revision,
            expires_at: cs.expires_at,
            status: Some(view.status),
            commands: cs.commands.iter().map(command_row).collect(),
            effects: cs.effects.iter().map(effect_row).collect(),
            linked_sources: linked_effects(&cs.effects),
            midi_notes: midi_note_rows(&cs.effects),
            unsupported: cs.unsupported.clone(),
            diff_complete: cs.unsupported.is_empty(),
            grant,
            task,
            grant_explanation,
            approval_status: view.approval.as_ref().map(|a| a.status),
            apply_status: view.apply.as_ref().map(|a| a.status),
            decision: SurfaceDecision::Pending,
        }
    }

    fn commit_verdict(&self, operation: OperationId) -> CommitVerdict {
        match self.committer.reconcile(operation) {
            RetryOutcome::Settled(outcome) => CommitVerdict::of(&outcome),
            RetryOutcome::InFlight => CommitVerdict::Pending,
            RetryOutcome::Unknown => CommitVerdict::Unknown,
        }
    }

    fn drive_commit(&self, proposal: &ProposalRef, operation: OperationId) -> CommitVerdict {
        match self.committer.commit(proposal) {
            Ok(receipt) => CommitVerdict::Committed {
                commit: receipt.commit.to_tagged(),
                revision: receipt.revision.0,
                journal_seq: receipt.journal_seq,
            },
            Err(_) => self.commit_verdict(operation),
        }
    }

    fn project_of_job(&self, operation: OperationId) -> Option<ProjectId> {
        self.jobs
            .as_ref()
            .and_then(|j| j.record(operation))
            .map(|r| r.project)
    }

    fn wire(
        &mut self,
        capability: &str,
        project: ProjectId,
        args: &Value,
        consistency: &Value,
    ) -> Value {
        self.next_request += 1;
        json!({
            "request_id": format!("ui-{}", self.next_request),
            "capability": {"id": capability, "version": {"major": 1, "minor": 0}},
            "schema": {"name": format!("{capability}.input"), "version": {"major": 1, "minor": 0}},
            "project": project.to_tagged(),
            "args": args,
            "consistency": consistency,
        })
    }
}

// ── row projections ────────────────────────────────────────────────

fn command_row(c: &SemanticCommand) -> CommandRow {
    CommandRow {
        capability: c.capability.id.as_str().to_owned(),
        op: c.op,
        target: target_display(&c.target),
        args: c.args.clone(),
    }
}

fn target_display(target: &OperationTarget) -> String {
    match target {
        OperationTarget::None => "none".to_owned(),
        OperationTarget::Resource { resource } => tagged_resource(resource),
        OperationTarget::ResourceSet { resources } => resources
            .iter()
            .map(tagged_resource)
            .collect::<Vec<_>>()
            .join(", "),
        OperationTarget::Create { parent, .. } => {
            format!("create under {}", tagged_resource(parent))
        }
    }
}

fn effect_row(e: &DerivedEffect) -> EffectRow {
    EffectRow {
        kind: e.kind,
        resource: tagged_resource(&e.resource),
        change: match e.kind {
            EffectKind::Create => "created",
            EffectKind::Write => "modified",
            EffectKind::Delete => "deleted",
            _ => "read",
        },
        before: e.before.clone(),
        after: e.after.clone(),
        references: e.references.iter().map(tagged_resource).collect(),
    }
}

/// The linked-source projection: any effect on a `Source` resource
/// whose references include clips becomes a `LinkedEffect` row — the
/// approval card shows the shared-source write alongside the clips it
/// lands on (UX-02/UX-03, SEC-07).
fn linked_effects(effects: &[DerivedEffect]) -> Vec<LinkedEffect> {
    effects
        .iter()
        .filter(|e| e.resource.kind() == ResourceKind::Source && !e.references.is_empty())
        .map(|e| LinkedEffect {
            source: tagged_resource(&e.resource),
            clips: e
                .references
                .iter()
                .filter(|r| r.kind() == ResourceKind::Clip)
                .map(tagged_resource)
                .collect(),
        })
        .filter(|l| !l.clips.is_empty())
        .collect()
}

/// The MIDI-note diff projection — the same note-pair derivation
/// `preview_value` performs (`added`/`removed`/`modified` per note
/// key), recovered from each source effect's stored detail.
fn midi_note_rows(effects: &[DerivedEffect]) -> Vec<MidiNoteRow> {
    let mut rows = Vec::new();
    for effect in effects {
        if effect.resource.kind() != ResourceKind::Source {
            continue;
        }
        let source = tagged_resource(&effect.resource);
        let empty = serde_json::Map::new();
        let before = effect
            .before_detail
            .as_ref()
            .and_then(|d| d.get("notes"))
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        let after = effect
            .after_detail
            .as_ref()
            .and_then(|d| d.get("notes"))
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        for (note, after_note) in after {
            match before.get(note) {
                None => rows.push(MidiNoteRow {
                    source: source.clone(),
                    note: note.clone(),
                    change: "added",
                }),
                Some(before_note) if before_note != after_note => rows.push(MidiNoteRow {
                    source: source.clone(),
                    note: note.clone(),
                    change: "modified",
                }),
                Some(_) => {}
            }
        }
        for note in before.keys() {
            if !after.contains_key(note) {
                rows.push(MidiNoteRow {
                    source: source.clone(),
                    note: note.clone(),
                    change: "removed",
                });
            }
        }
    }
    rows
}

fn tagged_resource(r: &ResourceRef) -> String {
    match r {
        ResourceRef::Project(id) => id.to_tagged(),
        ResourceRef::Track(id) => id.to_tagged(),
        ResourceRef::Lane(id) => id.to_tagged(),
        ResourceRef::Clip(id) => id.to_tagged(),
        ResourceRef::Source(id) => id.to_tagged(),
        ResourceRef::Asset(id) => id.to_tagged(),
        ResourceRef::Note(id) => id.to_tagged(),
        ResourceRef::TakeGroup(id) => id.to_tagged(),
        ResourceRef::Comp(id) => id.to_tagged(),
        ResourceRef::ParameterTarget(id) => id.to_tagged(),
        ResourceRef::AutomationLane(id) => id.to_tagged(),
        ResourceRef::Strip(id) => id.to_tagged(),
        ResourceRef::Send(id) => id.to_tagged(),
        ResourceRef::Processor(id) => id.to_tagged(),
    }
}

fn expected_revision(revision: ProjectRevision) -> Value {
    json!({"kind": "expected-revision", "revision": revision.0})
}

fn internal(reason: &str) -> ErrorEnvelope {
    GatewayError::Internal {
        reason: reason.to_owned(),
    }
    .into()
}
