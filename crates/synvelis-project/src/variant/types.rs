//! Creative Branch variant model types (synvelis#278, CB-01).
//!
//! The model is the Project's versioned, project-authoritative record of
//! production candidates: a [`VariantSet`] groups typed [`Branch`]es that
//! share one [`VariantScope`]. Branch *content* is never embedded here —
//! [`BranchContentRef`] pins immutable candidate payloads by digest plus
//! locator kind, and shared `Source`/`RawAsset`/native-MIDI material is
//! referenced, never duplicated.
//!
//! Scope payloads stay domain-specific: each [`VariantScope`] variant
//! carries only the typed identity/mask needed to name its owned object
//! set — entity payloads live in the project (or in the referenced
//! candidate), not in the scope declaration.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    apply::ApplyApproval,
    controller::ProjectRevision,
    diff::SemanticObjectRef,
    id::{
        BranchId, ChainId, ClipId, ProcessorId, ProjectId, StripId, TakeGroupId, TrackId,
        VariantSetId,
    },
    range::MusicalRange,
    snapshot::{ContentDigest, SnapshotIdentity},
};

/// Monotonic per-branch content revision (`u64`, transparent on the wire).
///
/// Bumped whenever the branch's candidate content changes — a candidate
/// edit, an adopt, or the freeze of a formerly adopted branch. It is the
/// `expected_branch_revision` CAS token of [`crate::variant::AdoptIntent`].
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct BranchRevision(pub u64);

impl BranchRevision {
    /// Wraps a raw revision value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The raw revision value.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// The next revision (`self + 1`).
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// Monotonic per-set revision (`u64`, transparent on the wire).
///
/// Bumped by every variant-set mutation (fork adds the set at revision 1;
/// edit/adopt/discard advance it) so concurrent variant writers fail
/// closed instead of interleaving.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct VariantSetRevision(pub u64);

impl VariantSetRevision {
    /// Wraps a raw revision value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The raw revision value.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// The next revision (`self + 1`).
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// The comp-domain target of a [`VariantScope::TakeUsage`] scope: the
/// take group whose candidate selection is being branched.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TakeUsageTarget {
    /// Take group owning the comparable candidates.
    pub take_group: TakeGroupId,
}

/// A contiguous processor range inside one [`crate::routing::ProcessorChain`].
///
/// `start`/`end` are stable processor identities; their order inside the
/// chain defines the range (inclusive on both ends). The scope owns the
/// processors between them plus their ports and internal connections —
/// boundary ports/connections stay shared with the rest of the chain.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessingRangeRef {
    /// Owning processor chain.
    pub chain: ChainId,
    /// First processor of the range (inclusive).
    pub start: ProcessorId,
    /// Last processor of the range (inclusive).
    pub end: ProcessorId,
}

/// One channel-strip field family a [`VariantScope::StripFields`] scope
/// owns.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StripField {
    /// The strip fader parameter.
    Fader,
    /// The strip pan parameter.
    Pan,
    /// The strip mute parameter.
    Mute,
    /// The strip solo parameter.
    Solo,
    /// The strip's insert processor chain.
    Inserts,
}

/// Which channel-strip aspects a [`VariantScope::StripFields`] scope owns.
///
/// At least one field must be selected for the scope to be valid.
/// [`StripField::Inserts`] owns the strip's whole insert chain
/// (processors, ports, internal connections and their control
/// entities); the channel fields own the matching singleton control
/// targets plus their automation/modulation closure.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StripDomainMask(pub BTreeSet<StripField>);

impl StripDomainMask {
    /// `true` when `field` is selected.
    #[must_use]
    pub fn owns(&self, field: StripField) -> bool {
        self.0.contains(&field)
    }

    /// `true` when at least one field family is selected.
    #[must_use]
    pub fn any(&self) -> bool {
        !self.0.is_empty()
    }
}

/// Which arrangement aspects a [`VariantScope::Arrangement`] scope owns.
///
/// `notes` implies clip ownership of the parent clips (a note cannot be
/// scoped without its coordinate frame).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArrangementDomainMask {
    /// Clips whose musical placement overlaps the scope range on the
    /// selected tracks.
    pub clips: bool,
    /// Clip-local notes of the scoped clips.
    pub notes: bool,
}

impl ArrangementDomainMask {
    /// Clips and notes enabled.
    pub const ALL: Self = Self {
        clips: true,
        notes: true,
    };

    /// `true` when at least one field family is selected.
    #[must_use]
    pub const fn any(self) -> bool {
        self.clips || self.notes
    }
}

/// The typed scope a [`VariantSet`] branches over.
///
/// Each variant names the *identity* of its domain object (or a bounded
/// selection of them) — never an entity payload. The owned object set is
/// derived by [`crate::variant::scope::scope_objects`]; domain adapters
/// decide how candidate content materializes inside that set.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum VariantScope {
    /// One audio clip: the clip entity, its clip-local notes and its
    /// clip-owned control closure.
    AudioClip {
        /// The scoped clip.
        clip: ClipId,
    },
    /// Comping take usage for one take group: the group, its comps,
    /// regions and transitions. Takes/sources stay shared raw material.
    TakeUsage {
        /// The scoped take-usage target.
        target: TakeUsageTarget,
    },
    /// One MIDI clip: the clip, its `MidiSource` note content (the
    /// source must be exclusively owned by this clip — shared MIDI
    /// sources are rejected at scope validation) and clip-local notes.
    MidiClip {
        /// The scoped clip.
        clip: ClipId,
    },
    /// One processor slot: the processor, its semantic ports and its
    /// processor-owned control closure (plugin params, automation,
    /// modulation).
    InstrumentSlot {
        /// The scoped processor.
        processor: ProcessorId,
    },
    /// A contiguous processor range inside one chain.
    ProcessingRange {
        /// The scoped range.
        range: ProcessingRangeRef,
    },
    /// Selected fields of one channel strip.
    StripFields {
        /// The scoped strip.
        strip: StripId,
        /// Which field families the scope owns.
        fields: StripDomainMask,
    },
    /// A musical-time selection across tracks.
    Arrangement {
        /// Tracks the selection covers (non-empty, deduplicated).
        tracks: Vec<TrackId>,
        /// Musical `[start, end)` range the selection covers.
        time: MusicalRange,
        /// Which arrangement field families the scope owns.
        fields: ArrangementDomainMask,
    },
}

/// Versioned, immutable reference to a branch's candidate content.
///
/// Two locator kinds exist:
///
/// - [`Self::Captured`] — immutable candidate payload bytes held by the
///   [`crate::variant::BranchContentStore`] under `payload_digest`.
///   `scope_digest` is the scope-content fingerprint: identical scoped
///   content yields identical digests across branches and adoptions,
///   which is the render-reuse identity (CB-MODEL-RCP-01).
/// - [`Self::Live`] — the adopted branch mirrors the live project for
///   the scope *by construction*; its content identity is the live
///   scope fingerprint computed on demand, so it can never desynchronize.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum BranchContentRef {
    /// Immutable candidate payload in the branch content store.
    Captured {
        /// Branch content revision this payload represents.
        revision: BranchRevision,
        /// SHA-256 of the exact stored payload bytes (the store key).
        payload_digest: ContentDigest,
        /// Scope-content fingerprint of the candidate.
        scope_digest: ContentDigest,
        /// Schema version the payload was written at.
        schema_version: u32,
    },
    /// The adopted branch's content is the live project scope.
    Live {
        /// Branch content revision of the last scoped mutation.
        revision: BranchRevision,
    },
}

impl BranchContentRef {
    /// The content revision this reference represents.
    #[must_use]
    pub const fn revision(&self) -> BranchRevision {
        match self {
            Self::Captured { revision, .. } | Self::Live { revision } => *revision,
        }
    }
}

/// Provenance of entity IDs minted inside a branch's candidate content.
///
/// Candidate edits draw IDs from the *live* ID space (reservations are
/// monotonic and can never collide with live or sibling-branch
/// entities). `local_ids` records which tagged stable IDs this branch
/// minted and at which content revision — the adopt-time lineage check
/// requires every newly-scoped entity to be provable here (or to come
/// from the fork origin). Unknown lineage is rejected, never guessed;
/// duplicate/undo-reissued identities stay distinct because the live
/// generator never reissues a raw value.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BranchLineage {
    /// Tagged stable ID → branch content revision that minted it.
    #[serde(default)]
    pub local_ids: BTreeMap<String, BranchRevision>,
}

/// One production candidate inside a [`VariantSet`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Branch {
    /// Stable branch identity.
    pub id: BranchId,
    /// The branch this one was forked from (`None` = fork origin).
    pub parent: Option<BranchId>,
    /// Fixed snapshot identity the branch was created from — locator +
    /// exact-byte digest + schema/projection identity, never just a
    /// project revision number.
    pub created_from: SnapshotIdentity,
    /// Versioned reference to the candidate content.
    pub content: BranchContentRef,
    /// Lineage of branch-minted entity IDs.
    #[serde(default)]
    pub lineage: BranchLineage,
}

/// Project-authoritative group of branches over one typed scope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariantSet {
    /// Stable set identity.
    pub id: VariantSetId,
    /// The shared scope every branch in the set covers.
    pub scope: VariantScope,
    /// Fixed snapshot identity of the fork origin.
    pub origin: SnapshotIdentity,
    /// Member branches by ID (at least one: the adopted branch).
    pub branches: BTreeMap<BranchId, Branch>,
    /// The branch currently adopted — its content mirrors live for the
    /// scope. Exactly one branch is adopted at a time.
    pub adopted: BranchId,
    /// Per-set mutation revision.
    pub revision: VariantSetRevision,
    /// Scope object set captured at fork — the baseline adopt uses to
    /// detect candidate-side entity removal (unsupported in CB-01).
    #[serde(default)]
    pub origin_objects: Vec<SemanticObjectRef>,
    /// Evidence record of the most recent adopt (project-side receipt).
    /// Runtime-publish evidence is *not* persisted here — it lives on
    /// the returned [`AdoptReceipt`] only (runtime state is not project
    /// truth).
    #[serde(default)]
    pub last_adopt: Option<AdoptRecord>,
}

/// Persisted project-side evidence of one adopt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptRecord {
    /// Branch that was adopted.
    pub branch: BranchId,
    /// The branch content revision that was adopted.
    pub branch_revision: BranchRevision,
    /// Number of scope objects materialized into live.
    pub objects: u32,
    /// Whether the adopt changed any scoped entity.
    pub outcome: AdoptOutcome,
}

/// Adopt outcome classification.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdoptOutcome {
    /// Scoped entities were materialized into live.
    Applied,
    /// The adopt was a semantic no-op (same candidate re-adopted, or
    /// the branch was already adopted) — no CAS, no undo step.
    NoChange,
}

/// A request to adopt one branch's scoped content into live.
///
/// Carries the full CAS surface: project revision, branch content
/// revision, and the live scope-content fingerprint the caller observed.
/// Any drift fails closed — there is no automatic rebase.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptIntent {
    /// Set to adopt from.
    pub variant_set: VariantSetId,
    /// Branch whose candidate becomes live for the scope.
    pub branch: BranchId,
    /// Live project revision the caller captured the intent at.
    pub expected_project_revision: ProjectRevision,
    /// Branch content revision the caller observed.
    pub expected_branch_revision: BranchRevision,
    /// [`crate::variant::scope_digest`] of the *live* project for the
    /// set's scope at intent time — the target hash.
    pub expected_scope_digest: ContentDigest,
    /// Explicit approval token (Gateway-compatible targeted intent);
    /// empty tickets are rejected.
    pub approval: ApplyApproval,
}

/// Project-commit evidence — distinct from runtime activation by
/// contract (CB-MODEL-08).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectCommitEvidence {
    /// Whether the project CAS committed.
    pub committed: bool,
    /// Live revision after the attempt.
    pub revision: ProjectRevision,
    /// Undo steps this attempt pushed (1 on `Applied`, 0 on `NoChange`).
    pub undo_steps: u32,
}

/// Runtime-publish outcome evidence — kept as a *separate* receipt from
/// the project commit so a publish failure never masquerades as (or
/// rolls back) committed project state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimePublishEvidence {
    /// The publish state the runtime port reported.
    pub state: RuntimePublishState,
    /// Human-readable detail (failure reason, pending cause, …).
    pub detail: Option<String>,
}

/// Runtime publish state reported through [`RuntimePublish`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum RuntimePublishState {
    /// The runtime accepted and activated the committed revision.
    Published,
    /// The runtime acknowledged but activation is still pending —
    /// project state stays committed; last-good runtime is *not*
    /// reported as current.
    Pending {
        /// Why activation is pending.
        reason: String,
    },
    /// The runtime publish failed after the project commit.
    Failed {
        /// Why the publish failed.
        reason: String,
    },
    /// No runtime publish was requested/performed.
    NotRequested,
}

/// Everything an adopt tells the runtime port — identity of the commit,
/// never runtime-specific types inside the domain model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePublishRequest {
    /// Project the commit landed on.
    pub project_id: ProjectId,
    /// Live revision the commit produced.
    pub revision: ProjectRevision,
    /// Set that was adopted.
    pub variant_set: VariantSetId,
    /// Branch that was adopted.
    pub branch: BranchId,
    /// Scope-content fingerprint that is now live.
    pub scope_digest: ContentDigest,
}

/// Port the engine calls *after* a successful project CAS to request
/// runtime activation. Implementations are host/runtime concerns; the
/// project crate only records the returned evidence on the receipt.
pub trait RuntimePublish {
    /// Requests activation of `request.revision`; returns evidence.
    fn publish(&self, request: &RuntimePublishRequest) -> RuntimePublishEvidence;
}

/// Receipt returned by [`crate::variant::adopt_branch`].
///
/// `project` and `runtime` are deliberately separate evidence blocks:
/// a committed project with a failed/pending publish is a valid,
/// honest outcome — never reported as a runtime success.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdoptReceipt {
    /// Set that was adopted into.
    pub variant_set: VariantSetId,
    /// Branch that was adopted.
    pub branch: BranchId,
    /// Live revision before the attempt.
    pub before_revision: ProjectRevision,
    /// Live revision after the attempt.
    pub after_revision: ProjectRevision,
    /// Scope objects materialized (0 on `NoChange`).
    pub objects: u32,
    /// Applied vs no-change classification.
    pub outcome: AdoptOutcome,
    /// Project-commit evidence.
    pub project: ProjectCommitEvidence,
    /// Runtime-publish evidence (separate receipt domain).
    pub runtime: RuntimePublishEvidence,
}

/// Receipt returned by [`crate::variant::fork_variant_set`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForkReceipt {
    /// The created set.
    pub variant_set: VariantSetId,
    /// The adopted (live-mirror) branch.
    pub adopted: BranchId,
    /// The candidate branch.
    pub candidate: BranchId,
    /// Live revision after the fork commit.
    pub project_revision: ProjectRevision,
}

/// Receipt returned by [`crate::variant::edit_branch`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditReceipt {
    /// Set containing the branch.
    pub variant_set: VariantSetId,
    /// Branch that was edited.
    pub branch: BranchId,
    /// The branch's new content revision (`None` when the edit did not
    /// change scoped content — e.g. an adopted-branch edit outside the
    /// scope).
    pub content_revision: Option<BranchRevision>,
    /// Live revision after the edit commit.
    pub project_revision: ProjectRevision,
}

/// Receipt returned by [`crate::variant::discard_branch`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscardReceipt {
    /// Set the branch was discarded from.
    pub variant_set: VariantSetId,
    /// Branch that was discarded.
    pub branch: BranchId,
    /// Live revision after the discard commit.
    pub project_revision: ProjectRevision,
}
