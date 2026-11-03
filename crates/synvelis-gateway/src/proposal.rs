//! GW-05 immutable prepared changesets and the proposal / approval /
//! apply state machines (GTC-007, GT-007..010, SEC-09..12).
//!
//! A [`PreparedChangeSet`] is the frozen semantic product of a GW-04
//! preparation: project, base revision, versioned commands,
//! preconditions, derived effects, unsupported-effect flags,
//! dependency identity, and expiry — with a deterministic
//! [`ContentDigest`] over the canonicalized semantic content.
//! Display-only data (proposal/operation/request refs, prepared-at
//! instants, retention ordering) never enters the digest, so two
//! preparations of the same semantic change against the same base
//! digest identically while any semantic drift — target, base
//! revision, effects, preconditions — changes it (GT-007).
//!
//! [`ProposalStore`] is the single authority for the three
//! independent lifecycles modeled here:
//!
//! ```text
//! proposal: Prepared ─▶ Invalidated
//!                 └─▶ Expired
//!
//! approval: Requested ─▶ Granted ─▶ Consumed
//!                │          ├─▶ Expired
//!                ├──────────┼─▶ Revoked
//!                ├──────────┴─▶ Invalidated
//!                └─▶ Declined
//!
//! apply:    AwaitingApproval ─▶ CommitRequested ─▶ Committed
//!               │                    ├─▶ Rejected
//!               ├─▶ Rejected         ├─▶ Conflict
//!               └─▶ Conflict         └─▶ Indeterminate
//! ```
//!
//! The transition tables are data, not per-caller logic — every
//! transition flows through [`ProposalStatus::transit`],
//! [`ApprovalStatus::transit`], or [`ApplyStatus::transit`], and the
//! table-driven tests prove every allowed and forbidden edge.
//!
//! The approval receipt ([`ApprovalReceipt`]) is minted only inside
//! this crate and bound to the proposal's *recorded* identity —
//! principal, project, grant, task, content digest, expected
//! revision, expiry — never to fields a request self-declares
//! (GT-008). An apply presents a receipt the gate verifies against
//! the stored record; consumption is one atomic
//! `Granted ─▶ Consumed` transition under the store lock, so a
//! second consume can never succeed — the double-spend contract
//! GW-06's commit fence relies on.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use synvelis_project::{ProjectId, ProjectRevision};

use crate::capability::EffectKind;
use crate::clock::{PolicyClock, PolicyInstant};
use crate::id::{ApprovalRef, GrantRef, OperationId, ProposalRef, TaskRef};
use crate::outcome::GatewayError;
use crate::prepare::{self, DerivedEffect, PreparationBasis, PreparedOperation, PreparedRecord};
use crate::request::{OperationRequest, Principal, PrincipalKind, ResourceKind};

/// The changeset schema identity carried in the digest input — a
/// format change is a semantic identity change.
pub const CHANGESET_SCHEMA: &str = "synvelis.prepared-changeset";

/// The changeset schema version carried in the digest input.
pub const CHANGESET_SCHEMA_VERSION: crate::capability::CapabilityVersion =
    crate::capability::CapabilityVersion::new(1, 0);

/// A deterministic content digest over a changeset's canonicalized
/// semantic content — `"sha256:<64 lowercase hex>"`.
///
/// Equality is semantic, not referential: two changesets with the
/// same project/base/commands/preconditions/effects digest equally
/// no matter when or for whom they were prepared.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct ContentDigest(String);

impl ContentDigest {
    /// The canonical `"sha256:<hex>"` string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ContentDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The frozen semantic product of one preparation — immutable at
/// the *store boundary*: the store holds the canonical instance and
/// hands out clones, so no caller mutates the retained changeset.
/// `verify()` recomputes the digest over the fields, catching any
/// in-process tampering of a clone as well.
///
/// *Dependency identity* (the base the change was derived against)
/// lives in [`Self::base`]; *expiry* is lifecycle metadata —
/// changing when a proposal dies does not change what it means —
/// so neither `expires_at` nor the snapshot *reference* enters the
/// digest: only (project, revision, tempo-map revision) identify
/// the semantic dependency.
#[derive(Clone, Debug)]
pub struct PreparedChangeSet {
    /// Deterministic digest over the semantic content.
    pub digest: ContentDigest,
    /// The pinned base — dependency identity.
    pub base: PreparationBasis,
    /// Versioned semantic commands, in application order.
    pub commands: Vec<crate::prepare::SemanticCommand>,
    /// Preconditions the pinned base satisfied.
    pub preconditions: Vec<crate::prepare::Precondition>,
    /// Effects derived against the base.
    pub effects: Vec<DerivedEffect>,
    /// Effect dimensions outside the observable model (fail-closed).
    pub unsupported: Vec<String>,
    /// Expiry on the policy clock — lifecycle metadata, excluded
    /// from the digest.
    pub expires_at: PolicyInstant,
}

/// Recursive canonicalization — object keys sort into a fixed order
/// and every value keeps its shape, so the digest input is byte-
/// deterministic regardless of the JSON map implementation in play
/// (`BTreeMap` vs insertion-ordered).
fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map
                .iter()
                .map(|(k, v)| (k.clone(), canonicalize(v)))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(entries.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

fn command_value(command: &crate::prepare::SemanticCommand) -> Value {
    serde_json::json!({
        "capability": command.capability,
        "op": command.op,
        "target": prepare::target_json(&command.target),
        "args": command.args,
    })
}

fn precondition_value(precondition: &crate::prepare::Precondition) -> Value {
    serde_json::json!({
        "code": precondition.code,
        "resource": precondition.resource.map(|r| prepare::ref_tagged(&r)),
        "detail": precondition.detail,
    })
}

fn effect_value(effect: &DerivedEffect) -> Value {
    serde_json::json!({
        "kind": prepare::effect_kind_name(effect.kind),
        "visibility": match effect.visibility {
            crate::prepare::EffectVisibility::Internal => "internal",
            crate::prepare::EffectVisibility::External => "external",
        },
        "resource": prepare::ref_tagged(&effect.resource),
        "before": effect.before.as_ref().map(prepare::attrs_json),
        "after": effect.after.as_ref().map(prepare::attrs_json),
        "before_detail": effect.before_detail,
        "after_detail": effect.after_detail,
        "references": effect.references.iter().map(prepare::ref_tagged).collect::<Vec<_>>(),
        "data_class": effect.data_class.map(|c| serde_json::to_value(c).expect("class serializes")),
    })
}

/// The canonical semantic identity of a changeset — everything that
/// defines *what the change is* and *what it stands on*. Excluded by
/// design: proposal/operation/request refs, caller identity, task
/// and grant refs, the pinned snapshot reference, timestamps, and
/// retention/display ordering.
fn digest_value(changeset: &PreparedChangeSet) -> Value {
    canonicalize(&serde_json::json!({
        "schema": CHANGESET_SCHEMA,
        "schema_version": CHANGESET_SCHEMA_VERSION,
        "base": {
            "project": changeset.base.project.to_tagged(),
            "revision": changeset.base.revision.0,
            "tempo_map_revision": changeset.base.tempo_map_revision,
        },
        "commands": changeset.commands.iter().map(command_value).collect::<Vec<_>>(),
        "preconditions": changeset.preconditions.iter().map(precondition_value).collect::<Vec<_>>(),
        "effects": changeset.effects.iter().map(effect_value).collect::<Vec<_>>(),
        "unsupported": changeset.unsupported,
    }))
}

fn compute_digest(value: &Value) -> ContentDigest {
    let bytes = serde_json::to_vec(value).expect("canonical value serializes");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    ContentDigest(format!("sha256:{:x}", hasher.finalize()))
}

impl PreparedChangeSet {
    /// Freezes a completed preparation into its immutable changeset —
    /// the digest is computed once, over the canonicalized content.
    #[must_use]
    pub fn freeze(prepared: &PreparedOperation, expires_at: PolicyInstant) -> Self {
        let mut changeset = Self {
            digest: ContentDigest(String::new()),
            base: prepared.base.clone(),
            commands: prepared.commands.clone(),
            preconditions: prepared.preconditions.clone(),
            effects: prepared.effects.clone(),
            unsupported: prepared.unsupported.clone(),
            expires_at,
        };
        changeset.digest = compute_digest(&digest_value(&changeset));
        changeset
    }

    /// Recomputes the digest over the stored fields — `false` means
    /// the changeset's content was altered after freeze (SEC-09).
    #[must_use]
    pub fn verify(&self) -> bool {
        compute_digest(&digest_value(self)) == self.digest
    }

    /// The changeset's schema identity — the same constant the
    /// digest binds, so a reader never guesses the format.
    #[must_use]
    pub fn schema(&self) -> &'static str {
        CHANGESET_SCHEMA
    }

    /// The changeset's schema version — bound into the digest.
    #[must_use]
    pub fn schema_version(&self) -> crate::capability::CapabilityVersion {
        CHANGESET_SCHEMA_VERSION
    }
}

// ── State machines ────────────────────────────────────────────

/// Proposal lifecycle (GTC-007). A proposal exists once preparation
/// completes — failed preparations never enter the store — so
/// `Prepared` is the only live state; `Invalidated`/`Expired` are
/// terminal.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProposalStatus {
    /// Frozen — previewable, approvable, applicable.
    Prepared,
    /// Explicitly dropped (evict, cascade) — terminal.
    Invalidated,
    /// Passed its expiry on the policy clock — terminal.
    Expired,
}

/// Events that move a proposal.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProposalEvent {
    /// Cross the proposal expiry.
    Expire,
    /// Invalidate (evict, cascade).
    Invalidate,
}

impl ProposalEvent {
    /// Every event, for table-driven transition tests.
    pub const ALL: [Self; 2] = [Self::Expire, Self::Invalidate];
}

impl ProposalStatus {
    /// Every status, for table-driven transition tests.
    pub const ALL: [Self; 3] = [Self::Prepared, Self::Invalidated, Self::Expired];

    /// The kebab-case name used in external messages.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Invalidated => "invalidated",
            Self::Expired => "expired",
        }
    }

    /// Whether the status is terminal (no legal outgoing edge).
    #[must_use]
    pub const fn terminal(self) -> bool {
        matches!(self, Self::Invalidated | Self::Expired)
    }

    /// The single authoritative transition table — `None` is an
    /// illegal transition.
    #[must_use]
    pub const fn transit(self, event: ProposalEvent) -> Option<Self> {
        Some(match (self, event) {
            (Self::Prepared, ProposalEvent::Expire) => Self::Expired,
            (Self::Prepared, ProposalEvent::Invalidate) => Self::Invalidated,
            _ => return None,
        })
    }
}

/// Approval lifecycle (GTC-007). `Consumed` seals the receipt: an
/// approval may authorize exactly one apply (GT-008, SEC-09/10).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalStatus {
    /// An apply awaits a trusted-user decision.
    Requested,
    /// A receipt is issued — consumable by one apply.
    Granted,
    /// The trusted user refused — terminal.
    Declined,
    /// Passed its expiry — terminal.
    Expired,
    /// Synvelis revoked the outstanding request/receipt — terminal.
    Revoked,
    /// The bound proposal was invalidated — terminal.
    Invalidated,
    /// The receipt was spent on an accepted apply — terminal.
    Consumed,
}

/// Events that move an approval.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ApprovalEvent {
    /// Issue the trusted-user receipt.
    Grant,
    /// The trusted user declined.
    Decline,
    /// Cross the request/receipt expiry.
    Expire,
    /// Synvelis-side revocation.
    Revoke,
    /// The bound proposal was invalidated.
    Invalidate,
    /// Spend the receipt on an accepted apply.
    Consume,
}

impl ApprovalEvent {
    /// Every event, for table-driven transition tests.
    pub const ALL: [Self; 6] = [
        Self::Grant,
        Self::Decline,
        Self::Expire,
        Self::Revoke,
        Self::Invalidate,
        Self::Consume,
    ];
}

impl ApprovalStatus {
    /// Every status, for table-driven transition tests.
    pub const ALL: [Self; 7] = [
        Self::Requested,
        Self::Granted,
        Self::Declined,
        Self::Expired,
        Self::Revoked,
        Self::Invalidated,
        Self::Consumed,
    ];

    /// The kebab-case name used in external messages.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Granted => "granted",
            Self::Declined => "declined",
            Self::Expired => "expired",
            Self::Revoked => "revoked",
            Self::Invalidated => "invalidated",
            Self::Consumed => "consumed",
        }
    }

    /// Whether the status is terminal.
    #[must_use]
    pub const fn terminal(self) -> bool {
        matches!(
            self,
            Self::Declined | Self::Expired | Self::Revoked | Self::Invalidated | Self::Consumed
        )
    }

    /// The single authoritative transition table.
    #[must_use]
    pub const fn transit(self, event: ApprovalEvent) -> Option<Self> {
        Some(match (self, event) {
            (Self::Requested, ApprovalEvent::Grant) => Self::Granted,
            (Self::Requested, ApprovalEvent::Decline) => Self::Declined,
            (Self::Requested | Self::Granted, ApprovalEvent::Expire) => Self::Expired,
            (Self::Requested | Self::Granted, ApprovalEvent::Revoke) => Self::Revoked,
            (Self::Requested | Self::Granted, ApprovalEvent::Invalidate) => Self::Invalidated,
            (Self::Granted, ApprovalEvent::Consume) => Self::Consumed,
            _ => return None,
        })
    }
}

/// Apply lifecycle (GTC-007). `CommitRequested` means the apply was
/// accepted onto the commit path — it is *not* a commit outcome:
/// `Committed`/`Rejected`/`Conflict`/`Indeterminate` are owned and
/// reported by GW-06 (ACG-004).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplyStatus {
    /// The apply waits on an approval decision.
    AwaitingApproval,
    /// Checks passed — handed to the commit boundary.
    CommitRequested,
    /// The commit landed — terminal (GW-06 reports).
    Committed,
    /// Refused — terminal.
    Rejected,
    /// Base revision moved under the proposal — terminal.
    Conflict,
    /// The outcome crossed a persistence boundary — terminal,
    /// reconcile before any retry (ACG-004).
    Indeterminate,
}

/// Events that move an apply.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ApplyEvent {
    /// The apply cleared the gate onto the commit path.
    Accept,
    /// The commit boundary committed.
    Commit,
    /// Refused/denied.
    Reject,
    /// Base revision drift detected.
    Conflict,
    /// The outcome cannot be determined.
    Indeterminate,
}

impl ApplyEvent {
    /// Every event, for table-driven transition tests.
    pub const ALL: [Self; 5] = [
        Self::Accept,
        Self::Commit,
        Self::Reject,
        Self::Conflict,
        Self::Indeterminate,
    ];
}

impl ApplyStatus {
    /// Every status, for table-driven transition tests.
    pub const ALL: [Self; 6] = [
        Self::AwaitingApproval,
        Self::CommitRequested,
        Self::Committed,
        Self::Rejected,
        Self::Conflict,
        Self::Indeterminate,
    ];

    /// The kebab-case name used in external messages.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AwaitingApproval => "awaiting-approval",
            Self::CommitRequested => "commit-requested",
            Self::Committed => "committed",
            Self::Rejected => "rejected",
            Self::Conflict => "conflict",
            Self::Indeterminate => "indeterminate",
        }
    }

    /// Whether the status is terminal.
    #[must_use]
    pub const fn terminal(self) -> bool {
        matches!(
            self,
            Self::Committed | Self::Rejected | Self::Conflict | Self::Indeterminate
        )
    }

    /// Whether a retry must reference this record rather than open a
    /// new apply — open or already-accepted states.
    #[must_use]
    pub const fn accepted_or_later(self) -> bool {
        matches!(
            self,
            Self::CommitRequested | Self::Committed | Self::Indeterminate
        )
    }

    /// The single authoritative transition table.
    #[must_use]
    pub const fn transit(self, event: ApplyEvent) -> Option<Self> {
        Some(match (self, event) {
            (Self::AwaitingApproval, ApplyEvent::Accept) => Self::CommitRequested,
            (Self::AwaitingApproval | Self::CommitRequested, ApplyEvent::Reject) => Self::Rejected,
            (Self::CommitRequested, ApplyEvent::Commit) => Self::Committed,
            (Self::AwaitingApproval | Self::CommitRequested, ApplyEvent::Conflict) => {
                Self::Conflict
            }
            (Self::CommitRequested, ApplyEvent::Indeterminate) => Self::Indeterminate,
            _ => return None,
        })
    }
}

// ── Records ───────────────────────────────────────────────────

/// The Synvelis-issued approval receipt — bound to the proposal's
/// *recorded* identity (GT-008), never to fields a request
/// self-declares. Only issued inside this crate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalReceipt {
    /// The trusted user that issued the receipt.
    pub approved_by: Principal,
    /// The principal authorized to apply with this receipt.
    pub for_principal: Principal,
    /// The bound project.
    pub project: ProjectId,
    /// The bound grant reference, when the proposal carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant: Option<GrantRef>,
    /// The bound task reference, when the proposal carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskRef>,
    /// The bound proposal.
    pub proposal: ProposalRef,
    /// The bound content digest — approving `D` never authorizes `D'`.
    pub digest: ContentDigest,
    /// The bound expected revision.
    pub expected_revision: ProjectRevision,
    /// Issuance instant on the policy clock.
    pub issued_at: PolicyInstant,
    /// Expiry instant on the policy clock.
    pub expires_at: PolicyInstant,
}

/// One proposal's approval lifecycle record — the request, its
/// status, and (once granted) the receipt.
#[derive(Clone, Debug)]
pub struct ApprovalRecord {
    /// The approval's stable reference.
    pub approval: ApprovalRef,
    /// Current lifecycle status.
    pub status: ApprovalStatus,
    /// The issued receipt (`Granted`/`Consumed` only).
    pub receipt: Option<ApprovalReceipt>,
    /// When the request was opened (policy clock).
    pub requested_at: PolicyInstant,
    /// Request-level expiry — an unanswered request dies on schedule.
    pub expires_at: PolicyInstant,
}

/// One proposal's apply lifecycle record — the latest attempt.
#[derive(Clone, Debug)]
pub struct ApplyRecord {
    /// The gateway operation that owns this attempt — the apply's
    /// durable handle (a retry references this exact operation).
    pub operation: OperationId,
    /// The bound proposal.
    pub proposal: ProposalRef,
    /// The principal whose apply this is.
    pub for_principal: Principal,
    /// Current lifecycle status.
    pub status: ApplyStatus,
    /// The receipt consumed by this attempt, when one was required.
    pub approval: Option<ApprovalRef>,
    /// When this attempt was opened (policy clock).
    pub requested_at: PolicyInstant,
}

/// A cloned view of one proposal's full lifecycle state — the read
/// surface the engine and gate work against without holding the lock.
#[derive(Clone)]
pub struct ProposalView {
    /// The proposal reference.
    pub proposal: ProposalRef,
    /// The frozen changeset.
    pub changeset: PreparedChangeSet,
    /// The proposal status.
    pub status: ProposalStatus,
    /// The retained preparation product (present only while `Prepared`).
    pub record: Option<Arc<PreparedRecord>>,
    /// The approval record, when a request exists.
    pub approval: Option<ApprovalRecord>,
    /// The apply record, when an attempt exists.
    pub apply: Option<ApplyRecord>,
    /// The principal that prepared the proposal.
    pub owner: Principal,
}

impl std::fmt::Debug for ProposalView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProposalView")
            .field("proposal", &self.proposal)
            .field("changeset", &self.changeset)
            .field("status", &self.status)
            .field("record", &self.record.is_some())
            .field("approval", &self.approval)
            .field("apply", &self.apply)
            .field("owner", &self.owner)
            .finish()
    }
}

/// The gate's read-only verdict on a presented receipt.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum ApprovalVerdict {
    /// A fresh receipt is bound to this exact apply — consumable.
    Granted,
    /// The apply waits on the user: no receipt, a live request, an
    /// expired request/receipt, or a granted receipt not yet
    /// presented.
    Pending,
    /// The presented or implied approval can never authorize this
    /// apply (bound elsewhere, declined, revoked, invalidated, or
    /// already consumed without a live apply to reference).
    Invalid(&'static str),
}

struct ProposalEntry {
    owner: Principal,
    changeset: PreparedChangeSet,
    record: Option<Arc<PreparedRecord>>,
    status: ProposalStatus,
    approval: Option<ApprovalRecord>,
    apply: Option<ApplyRecord>,
}

impl ProposalEntry {
    fn view(&self, proposal: &ProposalRef) -> ProposalView {
        ProposalView {
            proposal: proposal.clone(),
            changeset: self.changeset.clone(),
            status: self.status,
            record: self.record.clone(),
            approval: self.approval.clone(),
            apply: self.apply.clone(),
            owner: self.owner.clone(),
        }
    }
}

#[derive(Default)]
struct StoreState {
    entries: BTreeMap<String, ProposalEntry>,
    order: VecDeque<String>,
    next_ref: u128,
    next_approval: u128,
}

/// The proposal/approval/apply store — the single authority for the
/// three lifecycles. In-memory like GW-02's policy store: every
/// method is one lock-bounded operation, and the check-and-transition
/// pairs (receipt verify+consume, apply accept) are atomic under the
/// same lock — the seam GW-06's fence composes.
///
/// Entries are bounded FIFO; a retained proposal's heavy state
/// ([`PreparedRecord`], which carries the candidate project) is
/// dropped when the proposal leaves `Prepared`, while the changeset
/// and lifecycle records stay for audit until capacity evicts the
/// whole entry.
pub struct ProposalStore {
    clock: Arc<dyn PolicyClock>,
    proposal_ttl_ms: u64,
    approval_ttl_ms: u64,
    max_entries: usize,
    state: Mutex<StoreState>,
}

#[allow(clippy::result_large_err)] // typed errors are the contract
impl ProposalStore {
    /// A bounded store over `clock`; `proposal_ttl_ms` bounds a
    /// changeset's applicability, `approval_ttl_ms` bounds both the
    /// request and the issued receipt.
    #[must_use]
    pub fn new(
        clock: Arc<dyn PolicyClock>,
        proposal_ttl_ms: u64,
        approval_ttl_ms: u64,
        max_entries: usize,
    ) -> Self {
        Self {
            clock,
            proposal_ttl_ms,
            approval_ttl_ms,
            max_entries: max_entries.max(1),
            state: Mutex::new(StoreState::default()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, StoreState> {
        self.state.lock().expect("proposal store poisoned")
    }

    /// Clock-derived transitions — run inside the lock at the top of
    /// every operation, so a stale `Prepared` or live-looking receipt
    /// is never observed after its deadline.
    fn refresh(&self, state: &mut StoreState, key: &str) {
        let now = self.clock.now();
        let Some(entry) = state.entries.get_mut(key) else {
            return;
        };
        if entry.status == ProposalStatus::Prepared && now > entry.changeset.expires_at {
            entry.status = ProposalStatus::Expired;
            entry.record = None;
        }
        if entry.status.terminal() {
            // A dead proposal kills a live approval with it — the
            // receipt bound to dropped content can never be spent.
            if let Some(approval) = &mut entry.approval {
                let event = match entry.status {
                    ProposalStatus::Expired => ApprovalEvent::Expire,
                    ProposalStatus::Invalidated => ApprovalEvent::Invalidate,
                    ProposalStatus::Prepared => {
                        unreachable!("terminal guard excludes Prepared")
                    }
                };
                if let Some(next) = approval.status.transit(event) {
                    approval.status = next;
                }
            }
            // An apply still *waiting* on a dead proposal is refused.
            // An apply already accepted onto the commit path keeps
            // its record: its outcome is GW-06's to report, and a
            // dead proposal must never falsify it to `Rejected`.
            if let Some(apply) = &mut entry.apply
                && apply.status == ApplyStatus::AwaitingApproval
                && let Some(next) = apply.status.transit(ApplyEvent::Reject)
            {
                apply.status = next;
            }
        }
        if let Some(approval) = &mut entry.approval {
            let deadline = approval
                .receipt
                .as_ref()
                .map_or(approval.expires_at, |r| r.expires_at);
            if matches!(
                approval.status,
                ApprovalStatus::Requested | ApprovalStatus::Granted
            ) && now > deadline
                && let Some(next) = approval.status.transit(ApprovalEvent::Expire)
            {
                approval.status = next;
            }
        }
    }

    fn view_of(&self, state: &mut StoreState, proposal: &ProposalRef) -> Option<ProposalView> {
        self.refresh(state, proposal.as_str());
        state
            .entries
            .get(proposal.as_str())
            .map(|e| e.view(proposal))
    }

    /// Records a completed preparation — freezes the changeset,
    /// mints the proposal ref, and retains the record under the
    /// `Prepared` status. Returns the ref, the retained record, and
    /// the frozen changeset.
    ///
    /// # Errors
    /// [`GatewayError::LimitExceeded`] — the store is at capacity and
    /// every retained entry is live: a live proposal (and any granted
    /// receipt or in-flight apply it carries) is never expendable, so
    /// the bound applies as backpressure on preparation rather than a
    /// silent loss of an open lifecycle.
    ///
    /// # Panics
    /// Panics if the ref space is exhausted (2^128 proposals).
    pub fn insert(
        &self,
        request: OperationRequest,
        op: synvelis_project::ProjectOp,
        prepared: PreparedOperation,
    ) -> Result<(ProposalRef, Arc<PreparedRecord>, PreparedChangeSet), GatewayError> {
        let mut state = self.lock();
        let reference = ProposalRef::new(format!("proposal-{:032x}", state.next_ref))
            .expect("proposal ref is bounded");
        state.next_ref = state.next_ref.checked_add(1).expect("ref space exhausted");
        // Bounded FIFO — dead entries evict first: a finished
        // lifecycle's tombstone is expendable where a live proposal
        // (and any granted receipt or in-flight apply it carries)
        // is not. When only live entries remain the cap refuses the
        // insert — a `LimitExceeded` is honest; dropping an open
        // apply's record is not.
        while state.entries.len() >= self.max_entries {
            let Some(victim) = state
                .order
                .iter()
                .position(|key| state.entries.get(key).is_some_and(|e| e.status.terminal()))
                .and_then(|i| state.order.remove(i))
            else {
                return Err(GatewayError::LimitExceeded {
                    limit: "proposals".to_owned(),
                    max: self.max_entries as u64,
                });
            };
            state.entries.remove(&victim);
        }
        let expires_at = PolicyInstant(self.clock.now().0.saturating_add(self.proposal_ttl_ms));
        let changeset = PreparedChangeSet::freeze(&prepared, expires_at);
        let owner = request.caller().principal().clone();
        let record = Arc::new(PreparedRecord {
            proposal: reference.clone(),
            request,
            op,
            prepared,
        });
        state.entries.insert(
            reference.as_str().to_owned(),
            ProposalEntry {
                owner,
                changeset: changeset.clone(),
                record: Some(record.clone()),
                status: ProposalStatus::Prepared,
                approval: None,
                apply: None,
            },
        );
        state.order.push_back(reference.as_str().to_owned());
        Ok((reference, record, changeset))
    }

    /// The entry `caller` may observe: the proposal's owner, or any
    /// trusted user (the approval authority reviews delegated
    /// proposals). Unknown, foreign-project, and foreign-principal
    /// proposals share `None` — existence blinding, same contract as
    /// the cursor store (SEC-10).
    #[must_use]
    pub fn resolve(
        &self,
        proposal: &ProposalRef,
        caller: &Principal,
        project: ProjectId,
    ) -> Option<ProposalView> {
        let mut state = self.lock();
        let view = self.view_of(&mut state, proposal)?;
        if view.changeset.base.project != project {
            return None;
        }
        if view.owner != *caller && caller.kind() != PrincipalKind::Human {
            return None;
        }
        Some(view)
    }

    /// The frozen changeset, regardless of lifecycle status.
    #[must_use]
    pub fn changeset(&self, proposal: &ProposalRef) -> Option<PreparedChangeSet> {
        let mut state = self.lock();
        self.view_of(&mut state, proposal)
            .map(|view| view.changeset)
    }

    /// The retained preparation product — the record GW-06
    /// revalidates against; `None` once the proposal left `Prepared`.
    #[must_use]
    pub fn record(&self, proposal: &ProposalRef) -> Option<Arc<PreparedRecord>> {
        let mut state = self.lock();
        self.view_of(&mut state, proposal).and_then(|v| v.record)
    }

    /// The full lifecycle view (internal/test surface).
    #[must_use]
    pub fn view(&self, proposal: &ProposalRef) -> Option<ProposalView> {
        let mut state = self.lock();
        self.view_of(&mut state, proposal)
    }

    /// Number of retained proposal entries (live + dead, up to the cap).
    #[must_use]
    pub fn count(&self) -> usize {
        self.lock().entries.len()
    }

    /// Explicit invalidation — the retention-control evict and the
    /// cascade owner for "this proposal is no longer usable". The
    /// entry stays for audit; the heavy record drops.
    pub fn evict(&self, proposal: &ProposalRef) -> bool {
        let mut state = self.lock();
        self.refresh(&mut state, proposal.as_str());
        let Some(entry) = state.entries.get_mut(proposal.as_str()) else {
            return false;
        };
        if let Some(next) = entry.status.transit(ProposalEvent::Invalidate) {
            entry.status = next;
        }
        entry.record = None;
        if let Some(approval) = &mut entry.approval
            && let Some(next) = approval.status.transit(ApprovalEvent::Invalidate)
        {
            approval.status = next;
        }
        // Only an apply still waiting on the proposal is refused —
        // an accepted apply's outcome stays GW-06's to report.
        if let Some(apply) = &mut entry.apply
            && apply.status == ApplyStatus::AwaitingApproval
            && let Some(next) = apply.status.transit(ApplyEvent::Reject)
        {
            apply.status = next;
        }
        true
    }

    /// Opens (or keeps) the proposal's approval request on behalf of
    /// the apply attempt `req` — idempotent: a live request is
    /// returned as-is; an expired or spent-on-a-finished-apply record
    /// is (re)opened so the apply path can wait on a fresh decision.
    /// A dead proposal never grows a request (`None`).
    ///
    /// While the request is `Requested` the record's `approval` ref
    /// tracks the wire ref this attempt was told
    /// (`approval-for-<operation>`) — last-told wins. Once the
    /// receipt exists the ref is frozen: the presented ref is
    /// correlation, never authority; the receipt binding is.
    pub(crate) fn request_approval(
        &self,
        proposal: &ProposalRef,
        req: &OperationRequest,
    ) -> Option<ApprovalRecord> {
        let mut state = self.lock();
        self.refresh(&mut state, proposal.as_str());
        let entry = state.entries.get_mut(proposal.as_str())?;
        if entry.status != ProposalStatus::Prepared {
            return None;
        }
        if let Some(record) = &mut entry.approval {
            // An expired record or a receipt spent on a finished
            // apply reopens; anything else keeps its record.
            let reopen = record.status == ApprovalStatus::Expired
                || (record.status == ApprovalStatus::Consumed
                    && entry.apply.as_ref().is_some_and(|a| a.status.terminal()));
            if !reopen {
                if record.status == ApprovalStatus::Requested {
                    record.approval = crate::gateway::approval_ref_for(req.operation_id());
                }
                return Some(record.clone());
            }
        }
        let now = self.clock.now();
        let record = ApprovalRecord {
            approval: crate::gateway::approval_ref_for(req.operation_id()),
            status: ApprovalStatus::Requested,
            receipt: None,
            requested_at: now,
            expires_at: PolicyInstant(now.0.saturating_add(self.approval_ttl_ms)),
        };
        entry.approval = Some(record.clone());
        Some(record)
    }

    /// Issues a trusted-user receipt bound to the proposal's
    /// *recorded* identity — principal, project, grant, task, digest,
    /// expected revision — never to self-declared fields (GT-008).
    /// `approved_by` is the approving user from the trusted caller,
    /// verified human by the engine before this runs.
    ///
    /// # Errors
    /// [`GatewayError::ProposalUnavailable`] — the proposal is dead.
    /// [`GatewayError::ApprovalInvalid`] — an existing approval is in
    /// a terminal state that must not be re-granted.
    pub(crate) fn grant_approval(
        &self,
        proposal: &ProposalRef,
        approved_by: &Principal,
    ) -> Result<ApprovalRecord, GatewayError> {
        let mut state = self.lock();
        self.refresh(&mut state, proposal.as_str());
        // Phase 1 — inspect and clone what the receipt binds to; the
        // entry borrow must end before the approval mint touches
        // `state` again.
        let (needs_fresh, grant, task) = {
            let Some(entry) = state.entries.get_mut(proposal.as_str()) else {
                return Err(GatewayError::ProposalUnavailable {
                    reason: "unknown proposal".to_owned(),
                });
            };
            if entry.status != ProposalStatus::Prepared {
                return Err(GatewayError::ProposalUnavailable {
                    reason: format!("proposal {}", entry.status.name()),
                });
            }
            // `Prepared` entries always retain their record.
            let record = entry.record.as_ref().expect("prepared entry keeps record");
            let grant = record.request.grant().cloned();
            let task = record.request.task().cloned();
            match entry.approval.as_ref().map(|a| a.status) {
                Some(ApprovalStatus::Granted) => {
                    return Ok(entry.approval.clone().expect("approval present"));
                }
                Some(ApprovalStatus::Consumed) => {
                    // A receipt spent on a finished apply ends its
                    // story — a fresh request may open for the retry;
                    // a live apply's receipt stays sealed.
                    if entry.apply.as_ref().is_some_and(|a| a.status.terminal()) {
                        (true, grant, task)
                    } else {
                        return Err(GatewayError::ApprovalInvalid {
                            reason: "approval already consumed".to_owned(),
                        });
                    }
                }
                Some(
                    status @ (ApprovalStatus::Declined
                    | ApprovalStatus::Revoked
                    | ApprovalStatus::Invalidated),
                ) => {
                    return Err(GatewayError::ApprovalInvalid {
                        reason: format!("approval {}", status.name()),
                    });
                }
                Some(ApprovalStatus::Requested) => (false, grant, task),
                Some(ApprovalStatus::Expired) | None => (true, grant, task),
            }
        };
        // Phase 2 — mint/open if needed, then grant.
        let now = self.clock.now();
        if needs_fresh {
            let aref = mint_approval(&mut state);
            let entry = state
                .entries
                .get_mut(proposal.as_str())
                .expect("entry existence checked");
            entry.approval = Some(ApprovalRecord {
                approval: aref,
                status: ApprovalStatus::Requested,
                receipt: None,
                requested_at: now,
                expires_at: PolicyInstant(now.0.saturating_add(self.approval_ttl_ms)),
            });
        }
        let entry = state
            .entries
            .get_mut(proposal.as_str())
            .expect("entry existence checked");
        let approval = entry.approval.as_mut().expect("request opened above");
        approval.status = approval
            .status
            .transit(ApprovalEvent::Grant)
            .expect("requested grants");
        approval.receipt = Some(ApprovalReceipt {
            approved_by: approved_by.clone(),
            for_principal: entry.owner.clone(),
            project: entry.changeset.base.project,
            grant,
            task,
            proposal: proposal.clone(),
            digest: entry.changeset.digest.clone(),
            expected_revision: entry.changeset.base.revision,
            issued_at: now,
            expires_at: PolicyInstant(now.0.saturating_add(self.approval_ttl_ms)),
        });
        Ok(approval.clone())
    }

    /// Drives one approval event — the Synvelis-side decline /
    /// revoke / invalidate transitions.
    ///
    /// # Errors
    /// [`GatewayError::Internal`] — the transition is not legal, or
    /// no approval exists.
    pub(crate) fn transit_approval(
        &self,
        proposal: &ProposalRef,
        event: ApprovalEvent,
    ) -> Result<ApprovalRecord, GatewayError> {
        let mut state = self.lock();
        self.refresh(&mut state, proposal.as_str());
        let Some(approval) = state
            .entries
            .get_mut(proposal.as_str())
            .and_then(|e| e.approval.as_mut())
        else {
            return Err(GatewayError::Internal {
                reason: "no approval for proposal".to_owned(),
            });
        };
        approval.status = approval
            .status
            .transit(event)
            .ok_or_else(|| GatewayError::Internal {
                reason: format!(
                    "illegal approval transition {} on {}",
                    event_name(event),
                    approval.status.name()
                ),
            })?;
        Ok(approval.clone())
    }

    /// The read-only receipt verdict for the apply gate — no state
    /// change beyond lazy expiry. `presented` is the request's
    /// `approval` argument.
    pub(crate) fn approval_verdict(
        &self,
        proposal: &ProposalRef,
        req: &OperationRequest,
        presented: Option<&str>,
    ) -> ApprovalVerdict {
        let mut state = self.lock();
        self.refresh(&mut state, proposal.as_str());
        let Some(view) = self.view_of(&mut state, proposal) else {
            return ApprovalVerdict::Pending;
        };
        let record = view.approval;
        match (presented, record) {
            (Some(_), None) => ApprovalVerdict::Invalid("approval does not match this proposal"),
            (Some(aref), Some(record)) => {
                if record.approval.as_str() != aref {
                    return ApprovalVerdict::Invalid("approval does not match this proposal");
                }
                match record.status {
                    ApprovalStatus::Granted => {
                        let receipt = record.receipt.expect("granted carries a receipt");
                        if let Some(reason) = binding_mismatch(&receipt, req, &view.changeset) {
                            ApprovalVerdict::Invalid(reason)
                        } else {
                            ApprovalVerdict::Granted
                        }
                    }
                    ApprovalStatus::Requested | ApprovalStatus::Expired => ApprovalVerdict::Pending,
                    ApprovalStatus::Declined => ApprovalVerdict::Invalid("approval declined"),
                    ApprovalStatus::Revoked => ApprovalVerdict::Invalid("approval revoked"),
                    ApprovalStatus::Invalidated => ApprovalVerdict::Invalid("approval invalidated"),
                    ApprovalStatus::Consumed => {
                        if view
                            .apply
                            .as_ref()
                            .is_some_and(|a| a.status.accepted_or_later())
                        {
                            // The apply this receipt funded is still
                            // open — the retry references it, not a
                            // second consume.
                            ApprovalVerdict::Granted
                        } else if view.apply.as_ref().is_some_and(|a| a.status.terminal()) {
                            // The apply the receipt funded is over —
                            // the request reopens for a fresh decision.
                            ApprovalVerdict::Pending
                        } else {
                            ApprovalVerdict::Invalid("approval already consumed")
                        }
                    }
                }
            }
            (None, Some(record)) => match record.status {
                ApprovalStatus::Granted => {
                    let receipt = record.receipt.expect("granted carries a receipt");
                    if binding_mismatch(&receipt, req, &view.changeset).is_none() {
                        // The bound receipt funds this apply on the
                        // record's own ref — the agent never declares
                        // receipt identity.
                        ApprovalVerdict::Granted
                    } else {
                        // Bound to another principal/grant — this
                        // caller must wait on its own approval.
                        ApprovalVerdict::Pending
                    }
                }
                ApprovalStatus::Requested | ApprovalStatus::Expired => ApprovalVerdict::Pending,
                ApprovalStatus::Declined => ApprovalVerdict::Invalid("approval declined"),
                ApprovalStatus::Revoked => ApprovalVerdict::Invalid("approval revoked"),
                ApprovalStatus::Invalidated => ApprovalVerdict::Invalid("approval invalidated"),
                ApprovalStatus::Consumed => {
                    if view.apply.is_some_and(|a| a.status.terminal()) {
                        ApprovalVerdict::Pending
                    } else {
                        ApprovalVerdict::Invalid("approval already consumed")
                    }
                }
            },
            (None, None) => ApprovalVerdict::Pending,
        }
    }

    /// Atomically verifies and spends the receipt — `Granted` to
    /// `Consumed` under the store lock, so a second consume can never
    /// succeed even if two applies race the gate (the GW-06
    /// double-spend contract). For the apply that already spent the
    /// receipt the call is an idempotent *verify* — the commit fence
    /// can re-confirm that this exact attempt consumed it.
    ///
    /// Consumption is bound to a *recorded, accepted* attempt: the
    /// request's operation must be the `CommitRequested` apply's —
    /// a receipt can never be spent into the void (a `Consumed`
    /// approval with no bound apply is a permanent dead-end the
    /// lifecycle cannot explain), and an unaccepted attempt never
    /// gets to pre-spend a receipt.
    ///
    /// # Errors
    /// [`GatewayError::ApprovalInvalid`] — the receipt is not
    /// consumable for this request, or the request is not the apply
    /// attempt on record; [`GatewayError::ProposalUnavailable`] —
    /// the proposal is gone.
    pub(crate) fn consume_approval(
        &self,
        proposal: &ProposalRef,
        approval: &ApprovalRef,
        req: &OperationRequest,
    ) -> Result<ApprovalReceipt, GatewayError> {
        let mut state = self.lock();
        self.refresh(&mut state, proposal.as_str());
        let Some(entry) = state.entries.get_mut(proposal.as_str()) else {
            return Err(GatewayError::ProposalUnavailable {
                reason: "unknown proposal".to_owned(),
            });
        };
        if !entry.apply.as_ref().is_some_and(|a| {
            a.status == ApplyStatus::CommitRequested && a.operation == req.operation_id()
        }) {
            return Err(GatewayError::ApprovalInvalid {
                reason: "no accepted apply attempt binds this consumption".to_owned(),
            });
        }
        consume_in_entry(entry, approval, req)
    }

    /// The atomic gate-to-commit accept — liveness, the retry
    /// reference, receipt consumption, and the `CommitRequested`
    /// record all commit under one lock, so a proposal that dies or a
    /// racing apply between the gate's read and this call can never
    /// produce an `Accepted` without a committable record.
    /// Returns the operation the `Accepted` outcome names — the
    /// existing record's on a retry, this request's on a fresh
    /// accept.
    ///
    /// When `receipt_required` and no ref is presented, a granted
    /// record funds the apply on the record's own ref — the agent
    /// never declares receipt identity.
    ///
    /// # Errors
    /// [`GatewayError::ProposalUnavailable`] — the proposal is dead
    /// or gone; [`GatewayError::ApprovalInvalid`] — the receipt is
    /// not consumable for this request.
    pub(crate) fn accept_apply(
        &self,
        proposal: &ProposalRef,
        req: &OperationRequest,
        receipt_required: bool,
        presented: Option<&str>,
    ) -> Result<OperationId, GatewayError> {
        let mut state = self.lock();
        self.refresh(&mut state, proposal.as_str());
        let Some(entry) = state.entries.get_mut(proposal.as_str()) else {
            return Err(GatewayError::ProposalUnavailable {
                reason: "unknown proposal".to_owned(),
            });
        };
        if entry.status != ProposalStatus::Prepared {
            return Err(GatewayError::ProposalUnavailable {
                reason: format!("proposal {}", entry.status.name()),
            });
        }
        // The retry reference under the same lock that accepts — a
        // racing apply sees the winner's record, never an overwrite.
        if let Some(apply) = &entry.apply
            && apply.status.accepted_or_later()
        {
            return Ok(apply.operation);
        }
        let spent = if receipt_required {
            let reference = match presented {
                Some(text) => ApprovalRef::new(text)?,
                None => entry
                    .approval
                    .as_ref()
                    .filter(|a| a.status == ApprovalStatus::Granted)
                    .map(|a| a.approval.clone())
                    .ok_or_else(|| GatewayError::ApprovalInvalid {
                        reason: "an approval receipt is required".to_owned(),
                    })?,
            };
            consume_in_entry(entry, &reference, req)?;
            Some(reference)
        } else {
            None
        };
        let now = self.clock.now();
        match entry.apply.as_mut() {
            Some(record) if record.status == ApplyStatus::AwaitingApproval => {
                record.status = record
                    .status
                    .transit(ApplyEvent::Accept)
                    .expect("awaiting accepts");
                record.operation = req.operation_id();
                record.approval = spent;
            }
            _ => {
                entry.apply = Some(ApplyRecord {
                    operation: req.operation_id(),
                    proposal: proposal.clone(),
                    for_principal: req.caller().principal().clone(),
                    status: ApplyStatus::CommitRequested,
                    approval: spent,
                    requested_at: now,
                });
            }
        }
        Ok(req.operation_id())
    }

    /// Opens the apply's waiting record when the gate reports
    /// `ApprovalRequired` — a live open attempt is kept as-is, and a
    /// dead proposal never grows an apply record.
    pub(crate) fn mark_apply_awaiting(&self, proposal: &ProposalRef, req: &OperationRequest) {
        let mut state = self.lock();
        self.refresh(&mut state, proposal.as_str());
        let Some(entry) = state.entries.get_mut(proposal.as_str()) else {
            return;
        };
        if entry.status != ProposalStatus::Prepared {
            return;
        }
        let keep = entry.apply.as_ref().is_some_and(|a| !a.status.terminal());
        if keep {
            return;
        }
        entry.apply = Some(ApplyRecord {
            operation: req.operation_id(),
            proposal: proposal.clone(),
            for_principal: req.caller().principal().clone(),
            status: ApplyStatus::AwaitingApproval,
            approval: None,
            requested_at: self.clock.now(),
        });
    }

    /// Records a terminal apply state observed before the apply ever
    /// reached the commit path (revision drift, …) — evidence of the
    /// attempt, not a transition of a stored one. A dead proposal
    /// keeps the lifecycle it already recorded, and a live attempt
    /// record is never clobbered by a different attempt's failure.
    pub(crate) fn fail_apply(
        &self,
        proposal: &ProposalRef,
        req: &OperationRequest,
        status: ApplyStatus,
    ) {
        debug_assert!(status.terminal());
        let mut state = self.lock();
        self.refresh(&mut state, proposal.as_str());
        let Some(entry) = state.entries.get_mut(proposal.as_str()) else {
            return;
        };
        if entry.status != ProposalStatus::Prepared {
            return;
        }
        if entry.apply.as_ref().is_some_and(|a| !a.status.terminal()) {
            return;
        }
        let prior_approval = entry.apply.as_ref().and_then(|a| a.approval.clone());
        entry.apply = Some(ApplyRecord {
            operation: req.operation_id(),
            proposal: proposal.clone(),
            for_principal: req.caller().principal().clone(),
            status,
            approval: prior_approval,
            requested_at: self.clock.now(),
        });
    }

    /// Drives one apply event — GW-06's reporting seam for the
    /// commit-side outcomes (`Commit`, `Reject`, `Conflict`,
    /// `Indeterminate`) on the accepted apply. A settle that names
    /// the record's current status is idempotent — the fence may
    /// re-report an outcome it cannot prove it already reported.
    ///
    /// # Errors
    /// [`GatewayError::Internal`] — no apply exists, or the
    /// transition is not legal from its current status.
    pub(crate) fn transit_apply(
        &self,
        proposal: &ProposalRef,
        event: ApplyEvent,
    ) -> Result<ApplyRecord, GatewayError> {
        let mut state = self.lock();
        self.refresh(&mut state, proposal.as_str());
        let Some(apply) = state
            .entries
            .get_mut(proposal.as_str())
            .and_then(|e| e.apply.as_mut())
        else {
            return Err(GatewayError::Internal {
                reason: "no apply for proposal".to_owned(),
            });
        };
        if apply.status == event_target(event) {
            return Ok(apply.clone());
        }
        apply.status = apply
            .status
            .transit(event)
            .ok_or_else(|| GatewayError::Internal {
                reason: format!(
                    "illegal apply transition {} on {}",
                    apply_event_name(event),
                    apply.status.name()
                ),
            })?;
        Ok(apply.clone())
    }
}

/// The status each apply event lands on — the same-target check the
/// settle seam uses for idempotent re-reports.
fn event_target(event: ApplyEvent) -> ApplyStatus {
    match event {
        ApplyEvent::Accept => ApplyStatus::CommitRequested,
        ApplyEvent::Commit => ApplyStatus::Committed,
        ApplyEvent::Reject => ApplyStatus::Rejected,
        ApplyEvent::Conflict => ApplyStatus::Conflict,
        ApplyEvent::Indeterminate => ApplyStatus::Indeterminate,
    }
}

fn event_name(event: ApprovalEvent) -> &'static str {
    match event {
        ApprovalEvent::Grant => "grant",
        ApprovalEvent::Decline => "decline",
        ApprovalEvent::Expire => "expire",
        ApprovalEvent::Revoke => "revoke",
        ApprovalEvent::Invalidate => "invalidate",
        ApprovalEvent::Consume => "consume",
    }
}

fn apply_event_name(event: ApplyEvent) -> &'static str {
    match event {
        ApplyEvent::Accept => "accept",
        ApplyEvent::Commit => "commit",
        ApplyEvent::Reject => "reject",
        ApplyEvent::Conflict => "conflict",
        ApplyEvent::Indeterminate => "indeterminate",
    }
}

/// Mints the next approval reference — kept as a `StoreState` helper
/// so minting never races an open entry borrow.
fn mint_approval(state: &mut StoreState) -> ApprovalRef {
    let aref = ApprovalRef::new(format!("approval-{:032x}", state.next_approval))
        .expect("approval ref is bounded");
    state.next_approval = state
        .next_approval
        .checked_add(1)
        .expect("approval ref space exhausted");
    aref
}

/// The entry-level verify+consume shared by the seam
/// (`consume_approval`) and the atomic accept — under the caller's
/// lock. A `Consumed` receipt re-verifies for the exact apply that
/// spent it (the fence's consumption/commit consistency check); any
/// other caller sees the spent receipt's `Invalid`.
fn consume_in_entry(
    entry: &mut ProposalEntry,
    approval: &ApprovalRef,
    req: &OperationRequest,
) -> Result<ApprovalReceipt, GatewayError> {
    let invalid = |reason: &str| GatewayError::ApprovalInvalid {
        reason: reason.to_owned(),
    };
    let Some(record) = entry.approval.as_mut() else {
        return Err(invalid("no approval bound to this proposal"));
    };
    if record.approval != *approval {
        return Err(invalid("approval does not match this proposal"));
    }
    if record.status != ApprovalStatus::Granted {
        if record.status == ApprovalStatus::Consumed
            && entry.apply.as_ref().is_some_and(|a| {
                a.status == ApplyStatus::CommitRequested
                    && a.operation == req.operation_id()
                    && a.approval.as_ref() == Some(approval)
            })
        {
            // This attempt spent the receipt — re-confirming is not
            // a second spend.
            return Ok(record.receipt.clone().expect("consumed carries a receipt"));
        }
        return Err(invalid(match record.status {
            ApprovalStatus::Requested => "approval is still pending",
            ApprovalStatus::Expired => "approval expired",
            ApprovalStatus::Declined => "approval declined",
            ApprovalStatus::Revoked => "approval revoked",
            ApprovalStatus::Invalidated => "approval invalidated",
            ApprovalStatus::Consumed => "approval already consumed",
            ApprovalStatus::Granted => unreachable!("matched above"),
        }));
    }
    let receipt = record.receipt.clone().expect("granted carries a receipt");
    if let Some(reason) = binding_mismatch(&receipt, req, &entry.changeset) {
        return Err(invalid(reason));
    }
    record.status = record
        .status
        .transit(ApprovalEvent::Consume)
        .expect("granted consumes");
    // The spend is bound to the attempt it funds — whichever path
    // consumed, the apply record names the receipt that paid for it.
    if let Some(apply) = &mut entry.apply
        && apply.operation == req.operation_id()
        && apply.approval.is_none()
    {
        apply.approval = Some(approval.clone());
    }
    Ok(receipt)
}

/// The receipt-binding check shared by the read-only verdict and the
/// atomic consume — every bound dimension must match the request
/// (SEC-10/11). The changeset-digest comparison additionally pins the
/// receipt to the stored semantic content.
fn binding_mismatch(
    receipt: &ApprovalReceipt,
    req: &OperationRequest,
    changeset: &PreparedChangeSet,
) -> Option<&'static str> {
    if receipt.for_principal != *req.caller().principal() {
        return Some("approval bound to another principal");
    }
    if receipt.project != req.project() {
        return Some("approval bound to another project");
    }
    if receipt.grant.as_ref() != req.grant() {
        return Some("approval bound to another grant");
    }
    if receipt.task.as_ref() != req.task() {
        return Some("approval bound to another task");
    }
    if receipt.digest != changeset.digest {
        return Some("approval bound to another changeset");
    }
    if req.expected_revision() != Some(receipt.expected_revision) {
        return Some("approval bound to another revision");
    }
    None
}

// ── Semantic preview ──────────────────────────────────────────

/// The semantic preview of a frozen changeset — the approval UX's
/// review document. It is a *projection of the derived effects*:
/// entity-level before/after attributes come from the same
/// `DerivedEffect` set the digest binds, and the MIDI note pairs are
/// recovered from each source effect's stored detail — the existing
/// GW-04 observation contract, not a second diff engine.
#[must_use]
pub fn preview_value(view: &ProposalView) -> Value {
    let changeset = &view.changeset;
    let mut entities = Vec::new();
    let mut midi_notes = Vec::new();
    for effect in &changeset.effects {
        let change = match effect.kind {
            EffectKind::Create => "created",
            EffectKind::Delete => "deleted",
            EffectKind::Write => "modified",
            _ => continue,
        };
        entities.push(serde_json::json!({
            "resource": prepare::ref_tagged(&effect.resource),
            "change": change,
            "before": effect.before.as_ref().map(prepare::attrs_json),
            "after": effect.after.as_ref().map(prepare::attrs_json),
        }));
        if effect.resource.kind() == ResourceKind::Source {
            note_pairs(effect, &mut midi_notes);
        }
    }
    serde_json::json!({
        "proposal": view.proposal.as_str(),
        "digest": changeset.digest.as_str(),
        "status": view.status.name(),
        "base": {
            "project": changeset.base.project.to_tagged(),
            "revision": changeset.base.revision.0,
            "snapshot": changeset.base.snapshot.as_str(),
            "tempo_map_revision": changeset.base.tempo_map_revision,
        },
        "expires_at": changeset.expires_at.0,
        "commands": changeset.commands.iter().map(command_value).collect::<Vec<_>>(),
        "preconditions": changeset.preconditions.iter().map(precondition_value).collect::<Vec<_>>(),
        "effects": changeset.effects.iter().map(effect_value).collect::<Vec<_>>(),
        "unsupported": changeset.unsupported,
        "diff": {
            "entities": entities,
            "midi_notes": midi_notes,
            "complete": changeset.unsupported.is_empty(),
        },
    })
}

/// Pairs a source effect's `before`/`after` note maps into per-note
/// changes — `added`/`removed`/`modified`. The note payloads are the
/// verbatim `MidiNote` serializations GW-04 stored as effect detail.
fn note_pairs(effect: &DerivedEffect, out: &mut Vec<Value>) {
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
    let source = prepare::ref_tagged(&effect.resource);
    for (note, after_note) in after {
        match before.get(note) {
            None => out.push(serde_json::json!({
                "source": source,
                "note": note,
                "change": "added",
                "before": Value::Null,
                "after": after_note,
            })),
            Some(before_note) if before_note != after_note => out.push(serde_json::json!({
                "source": source,
                "note": note,
                "change": "modified",
                "before": before_note,
                "after": after_note,
            })),
            Some(_) => {}
        }
    }
    for (note, before_note) in before {
        if !after.contains_key(note) {
            out.push(serde_json::json!({
                "source": source,
                "note": note,
                "change": "removed",
                "before": before_note,
                "after": Value::Null,
            }));
        }
    }
}
