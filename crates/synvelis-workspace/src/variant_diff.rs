//! CB-09 Creative Branch Difference Explorer — headless view-model +
//! driver-neutral [`ViewSpec`] projection + CB-08/CB-10 dispatch
//! facade (synvelis#286).
//!
//! This module is the *presentation/read-model* layer for comparing two
//! branches of one [`VariantSet`], plus the workspace-side dispatch
//! facade ([`VariantDiffEngine`]) that keeps `synvelis-gateway` types
//! out of app code (AC-05). The view-model consumes already-produced
//! evidence — the canonical semantic diff (`synvelis_project::diff`,
//! VCS-02) for the Semantic (Configured) layer, and typed evidence
//! inputs for the Signal (Measured) and Perceptual (Inferred) layers —
//! and projects it into a deterministic, driver-neutral [`ViewSpec`]
//! tree that Fake, Mock, and (later) GPUI drivers can mount unchanged.
//!
//! Contract boundaries (creative-branch-design §7):
//!
//! - **Three layers stay separate.** Semantic Diff (what is configured),
//!   Signal Diff (what was measured), and Perceptual evaluation (what was
//!   inferred) are distinct sections with distinct state vocabularies.
//!   `Unavailable`/`Unknown`/`Unsupported` evidence is never folded into
//!   "zero difference", `Aligned`, or `Pass` (CB-CMP-02).
//! - **Honest typing.** Stale pins, unknown branches, and domains
//!   without a merged variant adapter are explicit machine codes
//!   ([`VariantDiffReason`]), never prose or `Debug` output. Every
//!   domain enumerated today has a merged CB-0x adapter (arrangement
//!   included — CB-07/#284); the `DomainAdapterUnmerged` verdict stays
//!   typed so a future unmerged domain reports `Unsupported` honestly.
//! - **Explicit targets only.** Compare/Adopt actions bind to fully
//!   pinned [`BranchPin`]s validated against read-only authority facts.
//!   There is no action variant that accepts an ambient/selection-derived
//!   target — implicit mutation is impossible by construction
//!   (CB-UX-01/04).
//! - **View-local state only.** [`VariantDiffViewState`] persists row
//!   selection, the audition (monitor) selection, and the last compare
//!   target per view. It never carries project/variant/diff authority;
//!   the view-model apply path returns authority intents as
//!   [`VariantDiffIntent`]s, and [`VariantDiffEngine`] is the single
//!   dispatch point that routes them through the CB-08 orchestrator
//!   (compare) or the gateway CAS/adopt path (adopt-preview/adopt).
//! - **Evidence seam.** [`AuxLayerEvidence`] is the input contract the
//!   CB-08 orchestrator fills with signal/perceptual evidence.
//!   Rows tagged [`EvidenceOrigin::Mock`] are Fake-provider output and
//!   are labelled as such — they are never real measurements.
//!
//! Real GPUI mount, OS/window/IME/pixel acceptance, real plugin dock,
//! and PCM/RT signal measurement are out of scope (CB-10/CB-11,
//! #114/#176/#288 gates) — see `evidence/sv286/`.

use std::collections::BTreeMap;
use std::sync::PoisonError;

use ruviform_ui::view::{
    A11yRole, A11ySpec, ButtonSpec, CellValue, ComponentKey, ComponentSpec, ComponentVisualState,
    LabelSpec, PanelSpec, TableSelectionSpec, TableSpec, ViewSpec,
};
use serde::{Deserialize, Serialize};
use synvelis_project::controller::ProjectController;
use synvelis_project::diff::{
    ChangeKind, ChangeSet, ChangeSetSummary, ComparisonCompleteness, ComparisonDomain,
    ComparisonEquality, ComparisonIssue, ObjectKind, SemanticChange,
};
use synvelis_project::project::Project;
use synvelis_project::snapshot::{BlobProbe, ContentDigest, SnapshotCancel};
use synvelis_project::variant::{
    AdoptIntent, BranchContentStore, BranchRevision, CompareError, ComparisonFreshness,
    ComparisonOrchestrator, ComparisonResult, EvidenceReason, LayerState, PerceptualLayerResult,
    RuntimePublishState, SessionOptions, SignalAlignment, SignalComparisonProvider,
    SignalLayerResult, VariantError, VariantScope, scope_digest,
};
use synvelis_project::{BranchId, ProjectRevision, VariantSetId};

// ── reason codes ────────────────────────────────────────────────────

/// Stable machine reason codes for the Difference Explorer. Labels are
/// prose for humans; these codes are the contract drivers/tests match on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum VariantDiffReason {
    /// A pinned branch revision no longer matches authority.
    StaleRevision,
    /// The pinned scope fingerprint no longer matches authority.
    StaleScope,
    /// The pinned project revision no longer matches authority.
    StaleProject,
    /// The pinned variant set is absent from authority.
    UnknownSet,
    /// The pinned scope is not the authority's scope — the context
    /// belongs to a different comparison entirely.
    UnknownScope,
    /// A pinned branch is absent from authority.
    UnknownBranch,
    /// The compared domain has no merged variant adapter. Kept typed
    /// for future unmerged domains — all current domains have adapters.
    DomainAdapterUnmerged,
    /// The layer produced no evidence for this compare.
    EvidenceUnavailable,
    /// The layer's evidence identity could not be resolved — never a
    /// zero-difference claim.
    EvidenceUnknown,
    /// The layer is not supported for this scope/build.
    LayerUnsupported,
    /// Evidence rows came from a Mock/Fake provider — not real
    /// measurement or evaluation.
    MockProvider,
    /// Evidence rows came from the real pipeline.
    PipelineProvider,
    /// No alignment evidence exists between the compared renders.
    NoAlignmentEvidence,
    /// Edit/adopt may proceed while exact (plugin-verified) audition is
    /// unavailable — independent axes (CB-UI-RCP-01, CB-CMP-03).
    MissingPlugin,
    /// Action rejected: the target was not fully explicit.
    ImplicitTarget,
    /// Action rejected: the pinned revision is stale.
    StaleTarget,
    /// Action rejected: the pinned branch is unknown to authority.
    UnknownTarget,
    /// Action rejected: the domain adapter is unmerged.
    UnsupportedDomain,
    /// Action rejected: compare needs two distinct branches.
    SameBranch,
    /// Action params failed the typed decode contract.
    Malformed,
}

impl VariantDiffReason {
    /// The kebab-case wire/stable code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::StaleRevision => "stale-revision",
            Self::StaleScope => "stale-scope",
            Self::StaleProject => "stale-project",
            Self::UnknownSet => "unknown-set",
            Self::UnknownScope => "unknown-scope",
            Self::UnknownBranch => "unknown-branch",
            Self::DomainAdapterUnmerged => "domain-adapter-unmerged",
            Self::EvidenceUnavailable => "evidence-unavailable",
            Self::EvidenceUnknown => "evidence-unknown",
            Self::LayerUnsupported => "layer-unsupported",
            Self::MockProvider => "mock-provider",
            Self::PipelineProvider => "pipeline-provider",
            Self::NoAlignmentEvidence => "no-alignment-evidence",
            Self::MissingPlugin => "missing-plugin",
            Self::ImplicitTarget => "implicit-target",
            Self::StaleTarget => "stale-target",
            Self::UnknownTarget => "unknown-target",
            Self::UnsupportedDomain => "unsupported-domain",
            Self::SameBranch => "same-branch",
            Self::Malformed => "malformed",
        }
    }
}

// ── domain classification ───────────────────────────────────────────

/// The variant-domain a [`VariantSet`] scope belongs to — the label the
/// Difference Explorer groups its diff rows under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum VariantDiffDomain {
    /// One audio clip (+ clip-local notes/control closure).
    AudioClip,
    /// Comping take usage for one take group.
    TakeUsage,
    /// One MIDI clip (+ its exclusively-owned MIDI source).
    MidiClip,
    /// One processor slot (instrument).
    InstrumentSlot,
    /// A contiguous processor range inside one chain.
    ProcessingRange,
    /// Selected fields of one channel strip.
    StripFields,
    /// A musical-time selection across tracks — CB-07 arrangement
    /// adapter merged (`scarecrow173/synvelis#284`).
    Arrangement,
}

impl VariantDiffDomain {
    /// Classifies a [`VariantScope`] into its explorer domain.
    #[must_use]
    pub fn for_scope(scope: &VariantScope) -> Self {
        match scope {
            VariantScope::AudioClip { .. } => Self::AudioClip,
            VariantScope::TakeUsage { .. } => Self::TakeUsage,
            VariantScope::MidiClip { .. } => Self::MidiClip,
            VariantScope::InstrumentSlot { .. } => Self::InstrumentSlot,
            VariantScope::ProcessingRange { .. } => Self::ProcessingRange,
            VariantScope::StripFields { .. } => Self::StripFields,
            VariantScope::Arrangement { .. } => Self::Arrangement,
        }
    }

    /// Stable kebab-case domain label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::AudioClip => "audio-clip",
            Self::TakeUsage => "take-usage",
            Self::MidiClip => "midi-clip",
            Self::InstrumentSlot => "instrument-slot",
            Self::ProcessingRange => "processing-range",
            Self::StripFields => "strip-fields",
            Self::Arrangement => "arrangement",
        }
    }

    /// Whether the domain's variant adapter is merged. Every domain
    /// enumerated here has a merged CB-0x adapter — the verdict stays
    /// typed so a future unmerged domain reports `Unsupported`
    /// honestly instead of projecting fabricated rows.
    #[must_use]
    pub fn supported(self) -> bool {
        match self {
            Self::AudioClip
            | Self::TakeUsage
            | Self::MidiClip
            | Self::InstrumentSlot
            | Self::ProcessingRange
            | Self::StripFields
            | Self::Arrangement => true,
        }
    }

    /// Human-readable domain name for labels/titles.
    #[must_use]
    pub fn display_name(self) -> &'static str {
        match self {
            Self::AudioClip => "Audio Clip",
            Self::TakeUsage => "Take Usage",
            Self::MidiClip => "MIDI Clip",
            Self::InstrumentSlot => "Instrument Slot",
            Self::ProcessingRange => "Processing Range",
            Self::StripFields => "Strip Fields",
            Self::Arrangement => "Arrangement",
        }
    }
}

// ── evidence input contract (mock seam) ─────────────────────────────

/// Availability of one evidence layer — the shared honest-state
/// vocabulary for semantic, signal, perceptual, and capability axes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum EvidenceState {
    /// Evidence is present and projectable.
    Available,
    /// The layer produced no evidence — `reason` says why.
    Unavailable {
        /// Machine reason code.
        reason: VariantDiffReason,
    },
    /// Evidence identity could not be resolved — never "no difference".
    Unknown {
        /// Machine reason code.
        reason: VariantDiffReason,
    },
    /// The layer is not supported for this scope/build.
    Unsupported {
        /// Machine reason code.
        reason: VariantDiffReason,
    },
}

impl Default for EvidenceState {
    fn default() -> Self {
        Self::Unavailable {
            reason: VariantDiffReason::EvidenceUnavailable,
        }
    }
}

impl EvidenceState {
    /// `true` only when evidence rows may be projected.
    #[must_use]
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Who produced a non-semantic layer's evidence rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum EvidenceOrigin {
    /// The real pipeline (CB-08 orchestrator / CB-11 measurement).
    Pipeline,
    /// A Mock/Fake provider — test seam output, never real evidence.
    Mock,
}

/// One verbatim evidence row supplied by a layer provider. The
/// view-model projects these strings unchanged — it does not interpret
/// or fabricate measurements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRow {
    /// Provider-supplied row label.
    pub label: String,
    /// Provider-supplied resolved value, when any.
    #[serde(default)]
    pub value: Option<String>,
    /// Provider-supplied detail note, when any.
    #[serde(default)]
    pub detail: Option<String>,
}

/// Alignment relationship between the compared renders (§7.1
/// vocabulary). `Unavailable` is the honest default when no alignment
/// evidence exists — it is never silently `Aligned`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum AlignmentState {
    /// Renders are sample-aligned.
    Aligned,
    /// Alignment is approximate.
    Approximate,
    /// Alignment failed or is unaligned.
    Unaligned,
    /// No alignment evidence exists.
    #[default]
    Unavailable,
}

/// Semantic (Configured) layer input — canonical `compare_snapshots`
/// output for the compared branch contents, or an honest gap.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticEvidence {
    /// Layer availability.
    pub state: EvidenceState,
    /// The diff payload — present only when `state` is `Available`.
    pub diff: Option<SemanticDiffEvidence>,
}

/// The canonical diff payload projected into domain rows.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticDiffEvidence {
    /// Confirmed changes in canonical order.
    pub changes: Vec<SemanticChange>,
    /// Unresolved comparison issues.
    pub unresolved: Vec<ComparisonIssue>,
    /// Tri-state equality.
    pub overall: ComparisonEquality,
    /// Coverage completeness.
    pub completeness: ComparisonCompleteness,
    /// Confirmed-change summary.
    pub summary: ChangeSetSummary,
}

impl From<&ChangeSet> for SemanticDiffEvidence {
    fn from(cs: &ChangeSet) -> Self {
        Self {
            changes: cs.changes.clone(),
            unresolved: cs.unresolved.clone(),
            overall: cs.overall.clone(),
            completeness: cs.completeness,
            summary: cs.summary,
        }
    }
}

/// Signal (Measured) or Perceptual (Inferred) layer input — the mock
/// seam the CB-08 orchestrator will replace with real producers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuxLayerEvidence {
    /// Layer availability.
    pub state: EvidenceState,
    /// Evidence provenance — meaningful only when `Available`.
    pub origin: Option<EvidenceOrigin>,
    /// Render alignment — meaningful only when `Available`.
    pub alignment: AlignmentState,
    /// Verbatim provider rows.
    pub rows: Vec<EvidenceRow>,
}

impl AuxLayerEvidence {
    /// An honestly absent layer.
    #[must_use]
    pub fn unavailable(reason: VariantDiffReason) -> Self {
        Self {
            state: EvidenceState::Unavailable { reason },
            ..Self::default()
        }
    }

    /// Provider output — `origin` must declare Mock or Pipeline
    /// provenance explicitly.
    #[must_use]
    pub fn available(
        origin: EvidenceOrigin,
        alignment: AlignmentState,
        rows: Vec<EvidenceRow>,
    ) -> Self {
        Self {
            state: EvidenceState::Available,
            origin: Some(origin),
            alignment,
            rows,
        }
    }
}

/// Edit/exact-audition capability split for the compared scope
/// (CB-UI-RCP-01, CB-CMP-03): "can edit/adopt" and "can audition
/// exactly" are independent axes — a missing plugin blocks exact
/// audition, never editability.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CapabilityEvidence {
    /// Whether edit/adopt intents may execute.
    pub editability: EvidenceState,
    /// Whether exact (plugin-verified) audition is available.
    pub exact_audition: EvidenceState,
    /// Machine reasons behind degraded states (e.g. `MissingPlugin`).
    pub reasons: Vec<VariantDiffReason>,
}

// ── pinned context + authority ──────────────────────────────────────

/// An explicit branch pin — identity + content revision. This is the
/// only target shape actions may carry; there is no ambient-selection
/// variant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BranchPin {
    /// Branch identity inside the variant set.
    pub branch: BranchId,
    /// Branch content revision the pin asserts.
    pub revision: BranchRevision,
}

/// The pinned comparison context — what the explorer claims to compare.
#[derive(Clone, Debug, PartialEq)]
pub struct CompareContext {
    /// The variant set under comparison.
    pub variant_set: VariantSetId,
    /// The set's typed scope (drives domain classification).
    pub scope: VariantScope,
    /// Left side of the A/B compare.
    pub left: BranchPin,
    /// Right side of the A/B compare.
    pub right: BranchPin,
    /// Pinned scope-content fingerprint (adopt CAS guard identity).
    pub scope_digest: ContentDigest,
    /// Pinned live project revision the context was taken at.
    pub project_revision: ProjectRevision,
}

/// Per-branch authority facts the view-model validates pins against.
/// Read-only — the view-model never mutates these.
#[derive(Clone, Debug, PartialEq)]
pub struct BranchAuthority {
    /// Current branch content revision.
    pub revision: BranchRevision,
    /// Host-observed "edit session open on this branch" flag (§7.3
    /// Editing label — session state, not project truth).
    pub editing: bool,
    /// Runtime publish observation from the last adopt receipt.
    pub publish: RuntimePublishState,
}

/// Read-only authority snapshot for one variant set.
#[derive(Clone, Debug, PartialEq)]
pub struct CompareAuthority {
    /// The variant set this authority describes.
    pub variant_set: VariantSetId,
    /// The set's typed scope.
    pub scope: VariantScope,
    /// The currently adopted branch.
    pub adopted: BranchId,
    /// Current live project revision.
    pub project_revision: ProjectRevision,
    /// Current live scope-content fingerprint.
    pub scope_digest: ContentDigest,
    /// Member branches by ID.
    pub branches: BTreeMap<BranchId, BranchAuthority>,
}

/// Everything the projection needs — evidence inputs plus the pinned
/// context and the authority facts it is validated against.
#[derive(Clone, Debug)]
pub struct VariantDiffInput {
    /// Pinned compare context (explicit targets only).
    pub context: CompareContext,
    /// Read-only authority facts.
    pub authority: CompareAuthority,
    /// Semantic (Configured) layer evidence.
    pub semantic: SemanticEvidence,
    /// Signal (Measured) layer evidence — mock seam.
    pub signal: AuxLayerEvidence,
    /// Perceptual (Inferred) layer evidence — mock seam.
    pub perceptual: AuxLayerEvidence,
    /// Edit/exact-audition capability split.
    pub capabilities: CapabilityEvidence,
}

// ── view-local persisted state ──────────────────────────────────────

/// The last compare target a view hosted — enough for the host to
/// rebuild [`CompareContext`] pins from current authority. Persisted so
/// the semantic target survives dock/window moves (CB-UX-05); stale
/// pins are re-validated honestly on the next projection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariantDiffTarget {
    /// Variant set under comparison.
    pub variant_set: VariantSetId,
    /// Left pin.
    pub left: BranchPin,
    /// Right pin.
    pub right: BranchPin,
}

/// Per-view Difference Explorer state — selections and the last target
/// only; never project/variant/diff authority.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VariantDiffViewState {
    /// Last compare target this view hosted.
    #[serde(default)]
    pub target: Option<VariantDiffTarget>,
    /// Audition (temporary monitor) selection — §7.3: independent of
    /// Adopted/Editing/Active; never a project write.
    #[serde(default)]
    pub audition: Option<BranchPin>,
    /// Selected semantic diff row index.
    #[serde(default)]
    pub selected_row: Option<usize>,
    /// Whether the Signal layer section is shown.
    #[serde(default = "default_true")]
    pub show_signal: bool,
    /// Whether the Perceptual layer section is shown.
    #[serde(default = "default_true")]
    pub show_perceptual: bool,
}

impl Default for VariantDiffViewState {
    fn default() -> Self {
        Self {
            target: None,
            audition: None,
            selected_row: None,
            show_signal: true,
            show_perceptual: true,
        }
    }
}

fn default_true() -> bool {
    true
}

// ── view model output ───────────────────────────────────────────────

/// Context integrity against authority.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ContextState {
    /// All pins match current authority — evidence is current.
    Current,
    /// Pins no longer match — the view is `Stale`, never current
    /// evidence. `reasons` lists every drift detected.
    Stale {
        /// Machine reason codes for each drift.
        reasons: Vec<VariantDiffReason>,
    },
    /// The pinned set/scope/branches cannot be found in authority.
    Rejected {
        /// Machine reason code.
        reason: VariantDiffReason,
    },
}

/// Independent branch status labels (§7.3 / CB-UX-02). These are NOT
/// mutually exclusive — a branch can be Adopted + Auditioning +
/// `RtPending` simultaneously, and each label is shown separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum BranchLabel {
    /// The formally adopted branch (project truth).
    Adopted,
    /// The view-local audition/monitor selection.
    Auditioning,
    /// A candidate edit session is open (host-observed).
    Editing,
    /// Runtime publish observed `Published` for the last adopt.
    RtActive,
    /// Runtime activation pending after commit — project committed but
    /// RT not yet active (never collapsed into Active).
    RtPending,
    /// Runtime publish failed after commit.
    RtFailed,
}

impl BranchLabel {
    /// Stable kebab-case code.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Adopted => "adopted",
            Self::Auditioning => "auditioning",
            Self::Editing => "editing",
            Self::RtActive => "rt-active",
            Self::RtPending => "rt-pending",
            Self::RtFailed => "rt-failed",
        }
    }

    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Adopted => "Adopted",
            Self::Auditioning => "Auditioning",
            Self::Editing => "Editing",
            Self::RtActive => "RT Active",
            Self::RtPending => "RT Pending",
            Self::RtFailed => "RT Publish Failed",
        }
    }
}

/// One side of the compare with its independent status labels.
#[derive(Clone, Debug, PartialEq)]
pub struct BranchView {
    /// The explicit pin this side asserts.
    pub pin: BranchPin,
    /// Whether this branch is the adopted one.
    pub adopted: bool,
    /// Whether an edit session is open (host-observed).
    pub editing: bool,
    /// Runtime publish observation (authority verbatim).
    pub publish: RuntimePublishState,
    /// Whether this branch is the view-local audition selection.
    pub auditioning: bool,
    /// Independent status labels — non-exclusive by contract.
    pub labels: Vec<BranchLabel>,
}

/// One field-level difference inside a [`DiffRow`].
#[derive(Clone, Debug, PartialEq)]
pub struct DiffFieldRow {
    /// Dotted field path (verbatim from the canonical diff).
    pub field: String,
    /// Canonical JSON before (verbatim).
    pub before: String,
    /// Canonical JSON after (verbatim).
    pub after: String,
}

/// One confirmed semantic change projected for display.
#[derive(Clone, Debug, PartialEq)]
pub struct DiffRow {
    /// Canonical object kind.
    pub object_kind: ObjectKind,
    /// Stable kebab-case kind label (serde contract, not `Debug`).
    pub object_kind_label: String,
    /// Canonical stable ID (verbatim).
    pub object_id: String,
    /// Semantic change kind.
    pub change: ChangeKind,
    /// Stable kebab-case change label.
    pub change_label: String,
    /// Total field diffs (may exceed `fields.len()` when capped).
    pub field_count: usize,
    /// Field diffs — capped at [`MAX_FIELDS_PER_ROW`].
    pub fields: Vec<DiffFieldRow>,
    /// Whether fields were capped.
    pub fields_truncated: bool,
}

/// One unresolved comparison issue — the `Unknown` side, kept visible
/// and never folded into "no changes".
#[derive(Clone, Debug, PartialEq)]
pub struct IssueRow {
    /// Issue variant label (serde contract).
    pub kind_label: String,
    /// Compact canonical JSON of the issue (verbatim detail).
    pub detail: String,
}

/// Semantic (Configured) layer section.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SemanticSection {
    /// Effective layer state — the domain-support gate is already
    /// applied (arrangement → `Unsupported`).
    pub state: EvidenceState,
    /// Tri-state equality — `Some` only when evidence was available.
    pub equality: Option<ComparisonEquality>,
    /// Coverage completeness — `Some` only when available.
    pub completeness: Option<ComparisonCompleteness>,
    /// Change summary — `Some` only when available.
    pub summary: Option<ChangeSetSummary>,
    /// Domain diff rows in canonical order, capped at
    /// [`MAX_CONTENT_ROWS`]. `Variants`-domain changes (the compare's
    /// own set/branch bookkeeping) are suppressed from rows and counted
    /// in `variants_bookkeeping` instead — they are not scoped content.
    pub rows: Vec<DiffRow>,
    /// Unresolved issues, capped at [`MAX_ISSUE_ROWS`].
    pub issues: Vec<IssueRow>,
    /// Confirmed `Variants`-domain changes suppressed from `rows`.
    pub variants_bookkeeping: usize,
    /// Content rows dropped by the [`MAX_CONTENT_ROWS`] cap.
    pub omitted_rows: usize,
    /// Issues dropped by the [`MAX_ISSUE_ROWS`] cap.
    pub omitted_issues: usize,
}

/// Signal or Perceptual layer section.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuxSection {
    /// Layer availability.
    pub state: EvidenceState,
    /// Evidence provenance — `Some` only when `Available`.
    pub origin: Option<EvidenceOrigin>,
    /// Alignment state — `Unavailable` unless evidence is `Available`.
    pub alignment: AlignmentState,
    /// Verbatim provider rows, capped at [`MAX_AUX_ROWS`].
    pub rows: Vec<EvidenceRow>,
    /// Rows dropped by the [`MAX_AUX_ROWS`] cap.
    pub omitted_rows: usize,
}

/// Capability axes display.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CapabilityView {
    /// Edit/adopt axis.
    pub editability: EvidenceState,
    /// Exact-audition axis.
    pub exact_audition: EvidenceState,
    /// Machine reasons (e.g. `MissingPlugin`).
    pub reasons: Vec<VariantDiffReason>,
}

/// Authority-bound action kinds the explorer can request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum VariantDiffActionKind {
    /// Run the orchestrated compare over two explicit pins.
    Compare,
    /// Request the adopt-preview (consequence) view for a branch.
    AdoptPreview,
    /// Request the adopt CAS for a branch (post-preview).
    Adopt,
}

/// A validated, executable authority intent — the host routes these to
/// the existing variant engine; the view-model never executes them.
/// Intents are minted against the apply-time [`CompareAuthority`]: a
/// consumer routing one detached (e.g. over a command channel) must
/// pair it with that authority's `variant_set`/`project_revision` to
/// build the engine-level adopt request — authority fields are
/// deliberately not copied into the intent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "intent", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum VariantDiffIntent {
    /// Compare two explicitly pinned branches.
    Compare {
        /// Left pin.
        left: BranchPin,
        /// Right pin.
        right: BranchPin,
    },
    /// Preview adopt consequences for an explicitly pinned branch.
    AdoptPreview {
        /// Target pin.
        target: BranchPin,
        /// Current scope fingerprint — the CAS guard the host asserts.
        scope_digest: ContentDigest,
    },
    /// Adopt an explicitly pinned branch.
    Adopt {
        /// Target pin.
        target: BranchPin,
        /// Current scope fingerprint — the CAS guard the host asserts.
        scope_digest: ContentDigest,
    },
}

/// Which side of the A/B compare an adopt candidate targets — keeps
/// component keys unique when both compared branches are non-adopted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompareSide {
    /// The context's left pin.
    Left,
    /// The context's right pin.
    Right,
}

impl CompareSide {
    fn key(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// One action the view offers — executable candidates become Buttons;
/// blocked candidates stay visible as labelled reasons (fail-closed,
/// never silently dropped or faked executable).
#[derive(Clone, Debug, PartialEq)]
pub struct ActionCandidate {
    /// Action kind.
    pub kind: VariantDiffActionKind,
    /// Display label (prose).
    pub label: String,
    /// The typed intent this candidate would emit when executable.
    pub intent: VariantDiffIntent,
    /// Which compare side this candidate acts on — `None` for the
    /// whole-compare action.
    pub side: Option<CompareSide>,
    /// Whether the action may execute now.
    pub executable: bool,
    /// Why it is blocked — `None` when executable.
    pub block_reason: Option<VariantDiffReason>,
}

/// The complete projected view.
#[derive(Clone, Debug, PartialEq)]
pub struct VariantDiffView {
    /// The pinned comparison context this view was projected from —
    /// displayed verbatim so drivers can show exactly what is pinned.
    pub context: CompareContext,
    /// Classified domain.
    pub domain: VariantDiffDomain,
    /// Domain support state (`Unsupported` for unmerged adapters).
    pub domain_state: EvidenceState,
    /// Context integrity.
    pub context_state: ContextState,
    /// Left/right branch views with independent labels.
    pub branches: Vec<BranchView>,
    /// Semantic (Configured) section.
    pub semantic: SemanticSection,
    /// Signal (Measured) section.
    pub signal: AuxSection,
    /// Perceptual (Inferred) section.
    pub perceptual: AuxSection,
    /// Capability axes.
    pub capabilities: CapabilityView,
    /// Action candidates (executable + honestly blocked).
    pub actions: Vec<ActionCandidate>,
}

// ── caps ────────────────────────────────────────────────────────────

/// Field diffs shown per row before truncation.
pub const MAX_FIELDS_PER_ROW: usize = 16;

/// Content diff rows kept per semantic section — bounds the projected
/// `TableSpec` well under the driver contract's row cap and matches the
/// `vcs_ui` diff-row convention.
pub const MAX_CONTENT_ROWS: usize = 256;

/// Unresolved issues kept per semantic section.
pub const MAX_ISSUE_ROWS: usize = 128;

/// Evidence rows kept per aux (signal/perceptual) section.
pub const MAX_AUX_ROWS: usize = 256;

// ── projection ──────────────────────────────────────────────────────

/// Projects one compare input + view-local state into the deterministic
/// view model. Pure: same inputs → same view.
#[must_use]
pub fn project_variant_diff(
    input: &VariantDiffInput,
    view: &VariantDiffViewState,
) -> VariantDiffView {
    let domain = VariantDiffDomain::for_scope(&input.context.scope);
    let domain_state = if domain.supported() {
        EvidenceState::Available
    } else {
        EvidenceState::Unsupported {
            reason: VariantDiffReason::DomainAdapterUnmerged,
        }
    };
    let context_state = context_state(&input.context, &input.authority);

    let branches = [input.context.left.clone(), input.context.right.clone()]
        .into_iter()
        .map(|pin| branch_view(pin, &input.authority, view))
        .collect();

    let semantic = semantic_section(input, &domain_state);
    let signal = aux_section(&input.signal);
    let perceptual = aux_section(&input.perceptual);
    let capabilities = CapabilityView {
        editability: input.capabilities.editability.clone(),
        exact_audition: input.capabilities.exact_audition.clone(),
        reasons: input.capabilities.reasons.clone(),
    };
    let actions = action_candidates(input, &domain_state, &context_state);

    VariantDiffView {
        context: input.context.clone(),
        domain,
        domain_state,
        context_state,
        branches,
        semantic,
        signal,
        perceptual,
        capabilities,
        actions,
    }
}

fn context_state(context: &CompareContext, authority: &CompareAuthority) -> ContextState {
    if authority.variant_set != context.variant_set {
        return ContextState::Rejected {
            reason: VariantDiffReason::UnknownSet,
        };
    }
    if authority.scope != context.scope {
        return ContextState::Rejected {
            reason: VariantDiffReason::UnknownScope,
        };
    }
    for pin in [&context.left, &context.right] {
        if !authority.branches.contains_key(&pin.branch) {
            return ContextState::Rejected {
                reason: VariantDiffReason::UnknownBranch,
            };
        }
    }
    let mut reasons = Vec::new();
    if context.scope_digest != authority.scope_digest {
        reasons.push(VariantDiffReason::StaleScope);
    }
    if context.project_revision != authority.project_revision {
        reasons.push(VariantDiffReason::StaleProject);
    }
    for pin in [&context.left, &context.right] {
        if authority.branches[&pin.branch].revision != pin.revision {
            reasons.push(VariantDiffReason::StaleRevision);
            break;
        }
    }
    if reasons.is_empty() {
        ContextState::Current
    } else {
        ContextState::Stale { reasons }
    }
}

fn branch_view(
    pin: BranchPin,
    authority: &CompareAuthority,
    view: &VariantDiffViewState,
) -> BranchView {
    let adopted = pin.branch == authority.adopted;
    let (editing, publish) = authority
        .branches
        .get(&pin.branch)
        .map_or((false, RuntimePublishState::NotRequested), |b| {
            (b.editing, b.publish.clone())
        });
    let auditioning = view
        .audition
        .as_ref()
        .is_some_and(|a| a.branch == pin.branch);
    let mut labels = Vec::new();
    if adopted {
        labels.push(BranchLabel::Adopted);
    }
    if auditioning {
        labels.push(BranchLabel::Auditioning);
    }
    if editing {
        labels.push(BranchLabel::Editing);
    }
    match &publish {
        RuntimePublishState::Published => labels.push(BranchLabel::RtActive),
        RuntimePublishState::Pending { .. } => labels.push(BranchLabel::RtPending),
        RuntimePublishState::Failed { .. } => labels.push(BranchLabel::RtFailed),
        RuntimePublishState::NotRequested => {}
    }
    BranchView {
        pin,
        adopted,
        editing,
        publish,
        auditioning,
        labels,
    }
}

fn semantic_section(input: &VariantDiffInput, domain_state: &EvidenceState) -> SemanticSection {
    // The domain gate wins: an unmerged adapter never shows fabricated
    // rows even if a caller supplies diff evidence for it.
    if !domain_state.is_available() {
        return SemanticSection {
            state: domain_state.clone(),
            ..SemanticSection::default()
        };
    }
    match (&input.semantic.state, input.semantic.diff.as_ref()) {
        (EvidenceState::Available, Some(diff)) => {
            let content: Vec<&SemanticChange> = diff
                .changes
                .iter()
                .filter(|c| c.object.kind.domain() != ComparisonDomain::Variants)
                .collect();
            let variants_bookkeeping = diff.changes.len() - content.len();
            let omitted_rows = content.len().saturating_sub(MAX_CONTENT_ROWS);
            let issues_total = diff.unresolved.len();
            let omitted_issues = issues_total.saturating_sub(MAX_ISSUE_ROWS);
            SemanticSection {
                state: EvidenceState::Available,
                equality: Some(diff.overall.clone()),
                completeness: Some(diff.completeness),
                summary: Some(diff.summary),
                rows: content
                    .into_iter()
                    .take(MAX_CONTENT_ROWS)
                    .map(diff_row)
                    .collect(),
                issues: diff
                    .unresolved
                    .iter()
                    .take(MAX_ISSUE_ROWS)
                    .map(|issue| IssueRow {
                        kind_label: issue_kind_label(issue),
                        detail: serde_json::to_string(issue).unwrap_or_default(),
                    })
                    .collect(),
                variants_bookkeeping,
                omitted_rows,
                omitted_issues,
            }
        }
        // `Available` without a payload is a contract violation — report
        // it as Unknown, never as "no changes".
        (EvidenceState::Available, None) => SemanticSection {
            state: EvidenceState::Unknown {
                reason: VariantDiffReason::EvidenceUnknown,
            },
            ..SemanticSection::default()
        },
        (state, _) => SemanticSection {
            state: state.clone(),
            ..SemanticSection::default()
        },
    }
}

fn diff_row(change: &SemanticChange) -> DiffRow {
    let total = change.evidence.fields.len();
    let fields: Vec<DiffFieldRow> = change
        .evidence
        .fields
        .iter()
        .take(MAX_FIELDS_PER_ROW)
        .map(|f| DiffFieldRow {
            field: f.field.clone(),
            before: serde_json::to_string(&f.before).unwrap_or_default(),
            after: serde_json::to_string(&f.after).unwrap_or_default(),
        })
        .collect();
    DiffRow {
        object_kind: change.object.kind,
        object_kind_label: serde_label(&change.object.kind),
        object_id: change.object.stable_id.clone(),
        change: change.kind,
        change_label: serde_label(&change.kind),
        field_count: total,
        fields,
        fields_truncated: total > MAX_FIELDS_PER_ROW,
    }
}

fn aux_section(input: &AuxLayerEvidence) -> AuxSection {
    if !input.state.is_available() {
        return AuxSection {
            state: input.state.clone(),
            alignment: AlignmentState::Unavailable,
            ..AuxSection::default()
        };
    }
    // `Available` without provenance is a contract violation — report
    // it as Unknown, never as unattributed "measured" rows (same rule
    // as the semantic layer's `Available`-without-payload coercion).
    let Some(origin) = input.origin else {
        return AuxSection {
            state: EvidenceState::Unknown {
                reason: VariantDiffReason::EvidenceUnknown,
            },
            alignment: AlignmentState::Unavailable,
            ..AuxSection::default()
        };
    };
    let omitted_rows = input.rows.len().saturating_sub(MAX_AUX_ROWS);
    AuxSection {
        state: EvidenceState::Available,
        origin: Some(origin),
        alignment: input.alignment,
        rows: input.rows.iter().take(MAX_AUX_ROWS).cloned().collect(),
        omitted_rows,
    }
}

fn action_candidates(
    input: &VariantDiffInput,
    domain_state: &EvidenceState,
    context_state: &ContextState,
) -> Vec<ActionCandidate> {
    let domain_ok = domain_state.is_available();
    let current = matches!(context_state, ContextState::Current);
    let scope_digest = input.authority.scope_digest;
    let mut out = Vec::new();

    let compare_intent = VariantDiffIntent::Compare {
        left: input.context.left.clone(),
        right: input.context.right.clone(),
    };
    let compare_block = if input.context.left.branch == input.context.right.branch {
        Some(VariantDiffReason::SameBranch)
    } else if !domain_ok {
        Some(VariantDiffReason::UnsupportedDomain)
    } else if !current {
        context_block_reason(context_state)
    } else {
        None
    };
    out.push(ActionCandidate {
        kind: VariantDiffActionKind::Compare,
        label: "Compare A/B".to_string(),
        intent: compare_intent,
        side: None,
        executable: compare_block.is_none(),
        block_reason: compare_block,
    });

    // Adopt (the mutating intent) additionally requires the supplied
    // editability capability — `AdoptPreview` stays available so the
    // consequence view remains inspectable when edit is gated.
    let adopt_edit_block = if input.capabilities.editability.is_available() {
        None
    } else {
        Some(evidence_block_reason(&input.capabilities.editability))
    };

    for (pin, side, side_name) in [
        (&input.context.left, CompareSide::Left, "A"),
        (&input.context.right, CompareSide::Right, "B"),
    ] {
        if pin.branch == input.authority.adopted {
            continue;
        }
        // Adopt candidates are per-branch — a same-branch context
        // (already `SameBranch`-blocked for Compare) must not mint
        // duplicate adopt intents under two side labels.
        if side == CompareSide::Right && pin.branch == input.context.left.branch {
            continue;
        }
        let exists = input.authority.branches.contains_key(&pin.branch);
        let stale = exists && input.authority.branches[&pin.branch].revision != pin.revision;
        let block = if !exists {
            Some(VariantDiffReason::UnknownTarget)
        } else if stale {
            Some(VariantDiffReason::StaleTarget)
        } else if !domain_ok {
            Some(VariantDiffReason::UnsupportedDomain)
        } else if !current {
            context_block_reason(context_state)
        } else {
            None
        };
        for (kind, label) in [
            (
                VariantDiffActionKind::AdoptPreview,
                format!("Adopt Preview {side_name}"),
            ),
            (VariantDiffActionKind::Adopt, format!("Adopt {side_name}")),
        ] {
            let intent = if kind == VariantDiffActionKind::AdoptPreview {
                VariantDiffIntent::AdoptPreview {
                    target: pin.clone(),
                    scope_digest,
                }
            } else {
                VariantDiffIntent::Adopt {
                    target: pin.clone(),
                    scope_digest,
                }
            };
            // The editability gate applies only to the mutating Adopt
            // intent and only when no earlier block already applies.
            let kind_block = if kind == VariantDiffActionKind::Adopt {
                block.or(adopt_edit_block)
            } else {
                block
            };
            out.push(ActionCandidate {
                kind,
                label,
                intent,
                side: Some(side),
                executable: kind_block.is_none(),
                block_reason: kind_block,
            });
        }
    }
    out
}

/// The machine reason carried by a non-`Available` evidence state —
/// used as the block reason when capability evidence gates an action.
fn evidence_block_reason(state: &EvidenceState) -> VariantDiffReason {
    match state {
        EvidenceState::Available => VariantDiffReason::EvidenceUnavailable,
        EvidenceState::Unavailable { reason }
        | EvidenceState::Unknown { reason }
        | EvidenceState::Unsupported { reason } => *reason,
    }
}

fn context_block_reason(state: &ContextState) -> Option<VariantDiffReason> {
    match state {
        ContextState::Current => None,
        ContextState::Stale { reasons } => reasons.first().copied(),
        ContextState::Rejected { reason } => Some(*reason),
    }
}

// ── actions ─────────────────────────────────────────────────────────

/// Decoded view-local or authority-bound action.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum VariantDiffViewAction {
    /// Semantic row selection changed (view-local).
    SelectRow {
        /// Selected row index.
        row: usize,
    },
    /// Audition selection changed — `None` clears (view-local, never a
    /// project write).
    SetAudition {
        /// Branch to audition, or `None` to stop.
        branch: Option<BranchPin>,
    },
    /// Compare target changed (view-local persistence only).
    SetTarget {
        /// New target.
        target: VariantDiffTarget,
    },
    /// Compare two explicitly pinned branches.
    Compare {
        /// Left pin.
        left: BranchPin,
        /// Right pin.
        right: BranchPin,
    },
    /// Adopt preview for an explicitly pinned branch.
    AdoptPreview {
        /// Target pin.
        target: BranchPin,
    },
    /// Adopt an explicitly pinned branch.
    Adopt {
        /// Target pin.
        target: BranchPin,
    },
}

/// Typed rejection — fail-closed; nothing mutates on `Err`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum VariantDiffReject {
    /// The action lacked fully explicit pins.
    ImplicitTarget,
    /// The pinned branch revision is stale.
    StaleRevision,
    /// The pinned branch is unknown to authority.
    UnknownBranch,
    /// The pinned comparison context is stale or rejected — authority
    /// intents are only minted against a `Current` context.
    StaleContext,
    /// The authority's scope domain has no merged adapter.
    UnsupportedDomain,
    /// The supplied editability evidence does not permit adopt.
    CapabilityUnavailable,
    /// The target is already the adopted branch — adopt is a no-op.
    AlreadyAdopted,
    /// Compare needs two distinct branches.
    SameBranch,
    /// Params failed the typed decode contract.
    Malformed,
}

/// Action names carried on the wire/`ButtonSpec.action`.
/// Row-selection intents reuse the `ruviform-ui` table contract's
/// `"select"` action name.
pub const ACTION_VARIANT_DIFF_SELECT: &str = "select";
/// Set/clear the view-local audition (monitor) selection. Params:
/// `{"branch": <BranchPin|null>}`.
pub const ACTION_VARIANT_DIFF_AUDITION: &str = "audition";
/// Persist a new compare target for this view (view-local). Params:
/// a serialized [`VariantDiffTarget`].
pub const ACTION_VARIANT_DIFF_SET_TARGET: &str = "set-target";
/// Compare two explicitly pinned branches. Params:
/// `{"left": <BranchPin>, "right": <BranchPin>}`.
pub const ACTION_VARIANT_DIFF_COMPARE: &str = "compare";
/// Adopt preview for an explicitly pinned branch. Params: `<BranchPin>`.
pub const ACTION_VARIANT_DIFF_ADOPT_PREVIEW: &str = "adopt-preview";
/// Adopt an explicitly pinned branch. Params: `<BranchPin>`.
pub const ACTION_VARIANT_DIFF_ADOPT: &str = "adopt";

/// Logical command target every Difference Explorer intent is emitted
/// on — the single binding point drivers route back through
/// [`decode_variant_diff_action`].
pub const VARIANT_DIFF_TARGET: &str = "cb.variant-diff";

/// Decodes one wire action + params into the typed action contract.
/// Fail-closed: unknown actions, missing pins, and malformed payloads
/// are rejected — an implicit or selection-derived target can never be
/// constructed here.
///
/// # Errors
/// [`VariantDiffReject::ImplicitTarget`] when a pin-carrying action has
/// no params or lacks a required pin key, [`VariantDiffReject::Malformed`]
/// on undecodable params; unknown action names also reject as
/// `ImplicitTarget` — the caller's target routing is expected to have
/// already selected this decoder.
pub fn decode_variant_diff_action(
    action: &str,
    params: Option<&serde_json::Value>,
) -> Result<VariantDiffViewAction, VariantDiffReject> {
    match action {
        ACTION_VARIANT_DIFF_SELECT => {
            let row = params
                .and_then(|p| p.get("row"))
                .and_then(serde_json::Value::as_u64)
                .ok_or(VariantDiffReject::Malformed)?;
            let row = usize::try_from(row).map_err(|_| VariantDiffReject::Malformed)?;
            Ok(VariantDiffViewAction::SelectRow { row })
        }
        ACTION_VARIANT_DIFF_AUDITION => {
            let p = params.ok_or(VariantDiffReject::ImplicitTarget)?;
            if !p.is_object() {
                return Err(VariantDiffReject::Malformed);
            }
            let branch = match p.get("branch") {
                None => return Err(VariantDiffReject::ImplicitTarget),
                Some(serde_json::Value::Null) => None,
                Some(v) => Some(
                    serde_json::from_value::<BranchPin>(v.clone())
                        .map_err(|_| VariantDiffReject::Malformed)?,
                ),
            };
            Ok(VariantDiffViewAction::SetAudition { branch })
        }
        ACTION_VARIANT_DIFF_SET_TARGET => {
            let target = params
                .ok_or(VariantDiffReject::ImplicitTarget)
                .and_then(|p| {
                    serde_json::from_value::<VariantDiffTarget>(p.clone())
                        .map_err(|_| VariantDiffReject::Malformed)
                })?;
            Ok(VariantDiffViewAction::SetTarget { target })
        }
        ACTION_VARIANT_DIFF_COMPARE => {
            let (left, right) = pin_pair(params)?;
            Ok(VariantDiffViewAction::Compare { left, right })
        }
        ACTION_VARIANT_DIFF_ADOPT_PREVIEW | ACTION_VARIANT_DIFF_ADOPT => {
            let target = params
                .ok_or(VariantDiffReject::ImplicitTarget)
                .and_then(|p| {
                    serde_json::from_value::<BranchPin>(p.clone())
                        .map_err(|_| VariantDiffReject::Malformed)
                })?;
            Ok(if action == ACTION_VARIANT_DIFF_ADOPT_PREVIEW {
                VariantDiffViewAction::AdoptPreview { target }
            } else {
                VariantDiffViewAction::Adopt { target }
            })
        }
        _ => Err(VariantDiffReject::ImplicitTarget),
    }
}

fn pin_pair(
    params: Option<&serde_json::Value>,
) -> Result<(BranchPin, BranchPin), VariantDiffReject> {
    let p = params.ok_or(VariantDiffReject::ImplicitTarget)?;
    if !p.is_object() {
        return Err(VariantDiffReject::Malformed);
    }
    let left = p
        .get("left")
        .ok_or(VariantDiffReject::ImplicitTarget)
        .and_then(|v| {
            serde_json::from_value::<BranchPin>(v.clone()).map_err(|_| VariantDiffReject::Malformed)
        })?;
    let right = p
        .get("right")
        .ok_or(VariantDiffReject::ImplicitTarget)
        .and_then(|v| {
            serde_json::from_value::<BranchPin>(v.clone()).map_err(|_| VariantDiffReject::Malformed)
        })?;
    Ok((left, right))
}

/// Applies one decoded action.
///
/// View-local actions (`SelectRow`, `SetAudition`, `SetTarget`) mutate
/// only [`VariantDiffViewState`]. Authority-bound actions are validated
/// against [`CompareAuthority`] and returned as [`VariantDiffIntent`]s
/// for the host to route — `Ok(None)` means the action was consumed
/// view-locally.
///
/// # Errors
/// [`VariantDiffReject`] on stale/unknown/unsupported/implicit targets —
/// fail-closed, nothing mutates. Capability evidence (e.g. editability)
/// is evaluated as-of-projection: authority facts are re-validated
/// against `authority`, but a plugin-state drift inside the
/// projection→apply window is the downstream adopt engine's CAS
/// preflight boundary, not this view-model's.
pub fn apply_variant_diff_action(
    state: &mut VariantDiffViewState,
    view: &VariantDiffView,
    authority: &CompareAuthority,
    action: &VariantDiffViewAction,
) -> Result<Option<VariantDiffIntent>, VariantDiffReject> {
    match action {
        VariantDiffViewAction::SelectRow { row } => {
            state.selected_row = Some(*row);
            Ok(None)
        }
        VariantDiffViewAction::SetAudition { branch } => {
            state.audition.clone_from(branch);
            Ok(None)
        }
        VariantDiffViewAction::SetTarget { target } => {
            state.target = Some(target.clone());
            Ok(None)
        }
        VariantDiffViewAction::Compare { left, right } => {
            if left.branch == right.branch {
                return Err(VariantDiffReject::SameBranch);
            }
            validate_pin(authority, left)?;
            validate_pin(authority, right)?;
            require_domain(authority)?;
            require_current(view, authority)?;
            Ok(Some(VariantDiffIntent::Compare {
                left: left.clone(),
                right: right.clone(),
            }))
        }
        VariantDiffViewAction::AdoptPreview { target }
        | VariantDiffViewAction::Adopt { target } => {
            validate_pin(authority, target)?;
            require_domain(authority)?;
            require_current(view, authority)?;
            if target.branch == authority.adopted {
                return Err(VariantDiffReject::AlreadyAdopted);
            }
            let is_adopt = matches!(action, VariantDiffViewAction::Adopt { .. });
            if is_adopt && !view.capabilities.editability.is_available() {
                return Err(VariantDiffReject::CapabilityUnavailable);
            }
            let intent = if is_adopt {
                VariantDiffIntent::Adopt {
                    target: target.clone(),
                    scope_digest: authority.scope_digest,
                }
            } else {
                VariantDiffIntent::AdoptPreview {
                    target: target.clone(),
                    scope_digest: authority.scope_digest,
                }
            };
            Ok(Some(intent))
        }
    }
}

/// Applies the view-local subset of [`apply_variant_diff_action`] —
/// `SelectRow`/`SetAudition`/`SetTarget` — directly to persisted
/// state. These actions never consult authority or the projection by
/// contract, so hosts may apply them on a view that has no compare
/// target yet (`set-target` is the bootstrap action). Returns `false`
/// for authority-bound actions, which must go through the full apply
/// path.
#[must_use]
pub fn apply_variant_diff_view_local(
    state: &mut VariantDiffViewState,
    action: &VariantDiffViewAction,
) -> bool {
    match action {
        VariantDiffViewAction::SelectRow { row } => state.selected_row = Some(*row),
        VariantDiffViewAction::SetAudition { branch } => state.audition.clone_from(branch),
        VariantDiffViewAction::SetTarget { target } => state.target = Some(target.clone()),
        _ => return false,
    }
    true
}

fn validate_pin(authority: &CompareAuthority, pin: &BranchPin) -> Result<(), VariantDiffReject> {
    let Some(branch) = authority.branches.get(&pin.branch) else {
        return Err(VariantDiffReject::UnknownBranch);
    };
    if branch.revision != pin.revision {
        return Err(VariantDiffReject::StaleRevision);
    }
    Ok(())
}

/// Domain support is derived from the *authority's* scope — the scope
/// the intent would actually execute against — never from the view's
/// display context, which a caller could pair with a different
/// authority.
fn require_domain(authority: &CompareAuthority) -> Result<(), VariantDiffReject> {
    if VariantDiffDomain::for_scope(&authority.scope).supported() {
        Ok(())
    } else {
        Err(VariantDiffReject::UnsupportedDomain)
    }
}

/// Authority intents are only minted while the pinned context is
/// `Current` *against the apply-time authority* — the projection-time
/// `view.context_state` verdict may be stale if authority drifted
/// between projection and apply, so the verdict is re-derived here.
fn require_current(
    view: &VariantDiffView,
    authority: &CompareAuthority,
) -> Result<(), VariantDiffReject> {
    if matches!(
        context_state(&view.context, authority),
        ContextState::Current
    ) {
        Ok(())
    } else {
        Err(VariantDiffReject::StaleContext)
    }
}

// ── ViewSpec projection ─────────────────────────────────────────────

fn key(name: &str) -> ComponentKey {
    ComponentKey(name.to_string())
}

fn a11y(role: A11yRole, label: &str) -> A11ySpec {
    A11ySpec {
        role: Some(role),
        label: Some(label.to_string()),
        ..A11ySpec::default()
    }
}

fn label(key_name: &str, text: &str) -> ComponentSpec {
    ComponentSpec::Label(LabelSpec {
        key: key(key_name),
        text: text.to_string(),
        visual: ComponentVisualState::default(),
        a11y: a11y(A11yRole::Text, text),
    })
}

fn panel(key_name: &str, title: &str, children: Vec<ComponentSpec>) -> ComponentSpec {
    ComponentSpec::Panel(PanelSpec {
        key: key(key_name),
        title: Some(title.to_string()),
        children,
        visual: ComponentVisualState::default(),
        a11y: a11y(A11yRole::Group, title),
    })
}

/// Builds the driver-neutral [`ViewSpec`] for a projected view. Pure and
/// deterministic — identical `(view, state)` inputs produce identical
/// specs, and component keys are stable across projections.
#[must_use]
pub fn variant_diff_spec(view: &VariantDiffView, state: &VariantDiffViewState) -> ViewSpec {
    let mut children = vec![
        context_panel(view),
        branches_panel(view),
        semantic_panel(view, state),
        capabilities_panel(view),
        actions_panel(view),
    ];
    if state.show_signal {
        children.push(aux_panel(
            "variant-diff.signal",
            "Signal Diff (Measured)",
            &view.signal,
        ));
    }
    if state.show_perceptual {
        children.push(aux_panel(
            "variant-diff.perceptual",
            "Perceptual Evaluation (Inferred)",
            &view.perceptual,
        ));
    }
    ViewSpec {
        root: panel("variant-diff", "Variant Difference Explorer", children),
    }
}

fn context_panel(view: &VariantDiffView) -> ComponentSpec {
    let state_text = match &view.context_state {
        ContextState::Current => "context: current".to_string(),
        ContextState::Stale { reasons } => format!(
            "context: stale ({})",
            reasons
                .iter()
                .map(|r| r.code())
                .collect::<Vec<_>>()
                .join(",")
        ),
        ContextState::Rejected { reason } => format!("context: rejected ({})", reason.code()),
    };
    let domain_text = match &view.domain_state {
        EvidenceState::Available => format!("domain: {}", view.domain.label()),
        EvidenceState::Unsupported { reason } => {
            format!(
                "domain: {} unsupported ({})",
                view.domain.label(),
                reason.code()
            )
        }
        EvidenceState::Unavailable { reason } => {
            format!(
                "domain: {} unavailable ({})",
                view.domain.label(),
                reason.code()
            )
        }
        EvidenceState::Unknown { reason } => {
            format!(
                "domain: {} unknown ({})",
                view.domain.label(),
                reason.code()
            )
        }
    };
    // The pinned context is shown verbatim — set identity, live
    // revision, and scope fingerprint are the authority anchors the
    // compare claims, not derivable display state.
    let digest_text = view.context.scope_digest.to_string();
    let digest_short: String = digest_text.chars().take(23).collect();
    let digest_short = format!("{digest_short}…");
    panel(
        "variant-diff.context",
        "Compare Context",
        vec![
            label("variant-diff.context.domain", &domain_text),
            label("variant-diff.context.state", &state_text),
            label(
                "variant-diff.context.set",
                &format!("variant set: {}", view.context.variant_set.to_tagged()),
            ),
            label(
                "variant-diff.context.revision",
                &format!("project revision: {}", view.context.project_revision.0),
            ),
            label(
                "variant-diff.context.digest",
                &format!("scope digest: {digest_short}"),
            ),
        ],
    )
}

fn branches_panel(view: &VariantDiffView) -> ComponentSpec {
    let mut children = Vec::new();
    for (i, branch) in view.branches.iter().enumerate() {
        let side = if i == 0 { "left" } else { "right" };
        let labels = if branch.labels.is_empty() {
            "—".to_string()
        } else {
            branch
                .labels
                .iter()
                .map(|l| l.label())
                .collect::<Vec<_>>()
                .join(" · ")
        };
        children.push(label(
            &format!("variant-diff.branch.{side}"),
            &format!(
                "{side}: {} rev {} — {}",
                branch.pin.branch.to_tagged(),
                branch.pin.revision.raw(),
                labels
            ),
        ));
    }
    panel("variant-diff.branches", "Branches", children)
}

fn semantic_panel(view: &VariantDiffView, state: &VariantDiffViewState) -> ComponentSpec {
    let mut children = Vec::new();
    let state_text = evidence_state_text(&view.semantic.state);
    children.push(label("variant-diff.semantic.state", &state_text));
    if let Some(eq) = &view.semantic.equality {
        children.push(label(
            "variant-diff.semantic.equality",
            &format!("equality: {}", equality_label(eq)),
        ));
    }
    if let Some(comp) = &view.semantic.completeness {
        children.push(label(
            "variant-diff.semantic.completeness",
            &format!("completeness: {}", completeness_label(*comp)),
        ));
    }
    if let Some(summary) = &view.semantic.summary {
        children.push(label(
            "variant-diff.semantic.summary",
            &format!(
                "changes: {} (+{} added -{} removed ~{} modified, {} moved, {} reordered, {} reconnected, {} replaced; unresolved: {})",
                summary.total,
                summary.added,
                summary.removed,
                summary.modified,
                summary.moved,
                summary.reordered,
                summary.reconnected,
                summary.replaced,
                summary.unresolved
            ),
        ));
    }
    for (i, issue) in view.semantic.issues.iter().enumerate() {
        children.push(label(
            &format!("variant-diff.semantic.issue.{i}"),
            &format!("unresolved: {} {}", issue.kind_label, issue.detail),
        ));
    }
    if view.semantic.omitted_issues > 0 {
        children.push(label(
            "variant-diff.semantic.issues-omitted",
            &format!(
                "unresolved: …and {} more issue(s) omitted",
                view.semantic.omitted_issues
            ),
        ));
    }
    if view.semantic.variants_bookkeeping > 0 {
        children.push(label(
            "variant-diff.semantic.bookkeeping",
            &format!(
                "variant bookkeeping: {} change(s) excluded from content rows",
                view.semantic.variants_bookkeeping
            ),
        ));
    }
    if view.semantic.omitted_rows > 0 {
        children.push(label(
            "variant-diff.semantic.rows-omitted",
            &format!(
                "rows: showing {} of {} content change(s)",
                view.semantic.rows.len(),
                view.semantic.rows.len() + view.semantic.omitted_rows
            ),
        ));
    }
    if !view.semantic.rows.is_empty() {
        let rows: Vec<Vec<Option<CellValue>>> = view
            .semantic
            .rows
            .iter()
            .map(|row| {
                vec![
                    Some(CellValue::Text(format!(
                        "{} {}",
                        row.object_kind_label, row.object_id
                    ))),
                    Some(CellValue::Text(row.change_label.clone())),
                    Some(CellValue::Text(format!("{} field(s)", row.field_count))),
                ]
            })
            .collect();
        children.push(ComponentSpec::Table(TableSpec {
            key: key("variant-diff.semantic.rows"),
            columns: vec!["object".into(), "change".into(), "fields".into()],
            rows,
            selection: Some(TableSelectionSpec {
                target: VARIANT_DIFF_TARGET.to_string(),
            }),
            visual: ComponentVisualState::default(),
            a11y: a11y(A11yRole::Table, "semantic diff rows"),
        }));
    }
    // Selected-row field detail — makes the persisted view-local
    // selection observable and surfaces the bounded per-field
    // before/after evidence the view model already computed.
    children.extend(selected_detail(view, state));
    panel(
        "variant-diff.semantic",
        "Semantic Diff (Configured)",
        children,
    )
}

/// Labels for the selected semantic row's bounded field diffs. An
/// out-of-range selection (crafted payload) yields nothing.
fn selected_detail(view: &VariantDiffView, state: &VariantDiffViewState) -> Vec<ComponentSpec> {
    let Some(sel) = state.selected_row else {
        return Vec::new();
    };
    let Some(row) = view.semantic.rows.get(sel) else {
        return Vec::new();
    };
    let mut children = vec![label(
        "variant-diff.semantic.detail.header",
        &format!("selected: {} {}", row.object_kind_label, row.object_id),
    )];
    for (j, field) in row.fields.iter().enumerate() {
        children.push(label(
            &format!("variant-diff.semantic.detail.field.{j}"),
            &format!("{}: {} -> {}", field.field, field.before, field.after),
        ));
    }
    if row.fields_truncated {
        children.push(label(
            "variant-diff.semantic.detail.fields-omitted",
            &format!(
                "fields: showing {} of {} field diff(s)",
                row.fields.len(),
                row.field_count
            ),
        ));
    }
    children
}

fn aux_panel(key_name: &str, title: &str, section: &AuxSection) -> ComponentSpec {
    let mut children = vec![label(
        &format!("{key_name}.state"),
        &evidence_state_text(&section.state),
    )];
    if section.state.is_available() {
        if let Some(origin) = section.origin {
            let text = match origin {
                EvidenceOrigin::Pipeline => "origin: pipeline".to_string(),
                EvidenceOrigin::Mock => "origin: mock (not real measurement)".to_string(),
            };
            children.push(label(&format!("{key_name}.origin"), &text));
        }
        children.push(label(
            &format!("{key_name}.alignment"),
            &format!("alignment: {}", alignment_label(section.alignment)),
        ));
        if section.omitted_rows > 0 {
            children.push(label(
                &format!("{key_name}.rows-omitted"),
                &format!(
                    "rows: showing {} of {} evidence row(s)",
                    section.rows.len(),
                    section.rows.len() + section.omitted_rows
                ),
            ));
        }
        if !section.rows.is_empty() {
            let rows: Vec<Vec<Option<CellValue>>> = section
                .rows
                .iter()
                .map(|row| {
                    vec![
                        Some(CellValue::Text(row.label.clone())),
                        row.value.clone().map(CellValue::Text),
                        row.detail.clone().map(CellValue::Text),
                    ]
                })
                .collect();
            children.push(ComponentSpec::Table(TableSpec {
                key: key(&format!("{key_name}.rows")),
                columns: vec!["label".into(), "value".into(), "detail".into()],
                rows,
                selection: None,
                visual: ComponentVisualState::default(),
                a11y: a11y(A11yRole::Table, title),
            }));
        }
    }
    panel(key_name, title, children)
}

/// Display bound for capability reason labels — the view model keeps
/// every reason verbatim; only the spec projection truncates, with an
/// explicit omitted count (same honesty rule as row caps).
const MAX_DISPLAY_REASONS: usize = 32;

fn capabilities_panel(view: &VariantDiffView) -> ComponentSpec {
    let mut children = vec![
        label(
            "variant-diff.capabilities.editability",
            &format!(
                "editability: {}",
                evidence_state_text(&view.capabilities.editability)
            ),
        ),
        label(
            "variant-diff.capabilities.exact-audition",
            &format!(
                "exact audition: {}",
                evidence_state_text(&view.capabilities.exact_audition)
            ),
        ),
    ];
    for (i, reason) in view
        .capabilities
        .reasons
        .iter()
        .take(MAX_DISPLAY_REASONS)
        .enumerate()
    {
        children.push(label(
            &format!("variant-diff.capabilities.reason.{i}"),
            &format!("reason: {}", reason.code()),
        ));
    }
    if view.capabilities.reasons.len() > MAX_DISPLAY_REASONS {
        children.push(label(
            "variant-diff.capabilities.reasons-omitted",
            &format!(
                "reasons: …and {} more not shown",
                view.capabilities.reasons.len() - MAX_DISPLAY_REASONS
            ),
        ));
    }
    panel("variant-diff.capabilities", "Capabilities", children)
}

fn actions_panel(view: &VariantDiffView) -> ComponentSpec {
    let mut children = Vec::new();
    for candidate in &view.actions {
        let (base_key, action_name) = match candidate.kind {
            VariantDiffActionKind::Compare => (
                "variant-diff.action.compare".to_string(),
                ACTION_VARIANT_DIFF_COMPARE,
            ),
            VariantDiffActionKind::AdoptPreview => (
                "variant-diff.action.adopt-preview".to_string(),
                ACTION_VARIANT_DIFF_ADOPT_PREVIEW,
            ),
            VariantDiffActionKind::Adopt => (
                "variant-diff.action.adopt".to_string(),
                ACTION_VARIANT_DIFF_ADOPT,
            ),
        };
        // Side-suffixed keys keep adopt candidates unique when both
        // compared branches are non-adopted.
        let key_name = match candidate.side {
            Some(side) => format!("{base_key}.{}", side.key()),
            None => base_key,
        };
        if candidate.executable {
            children.push(action_button(
                &key_name,
                &candidate.label,
                action_name,
                action_params(candidate.kind, &candidate.intent),
            ));
        } else {
            children.push(label(
                &format!("{key_name}.blocked"),
                &format!(
                    "{} — blocked: {}",
                    candidate.label,
                    candidate.block_reason.map_or("unknown", |r| r.code())
                ),
            ));
        }
    }
    // Audition toggles are view-local and always executable — they never
    // touch project truth (§7.3 monitor selection).
    for (i, branch) in view.branches.iter().enumerate() {
        let side = if i == 0 { "left" } else { "right" };
        let (label_text, branch_param) = if branch.auditioning {
            ("Stop Audition".to_string(), serde_json::Value::Null)
        } else {
            (
                format!("Audition {}", side.to_uppercase()),
                serde_json::to_value(&branch.pin).unwrap_or_default(),
            )
        };
        let a11y_spec = a11y(A11yRole::Button, &label_text);
        children.push(ComponentSpec::Button(ButtonSpec {
            key: key(&format!("variant-diff.audition.{side}")),
            label: label_text,
            target: VARIANT_DIFF_TARGET.to_string(),
            action: ACTION_VARIANT_DIFF_AUDITION.to_string(),
            params: Some(serde_json::json!({ "branch": branch_param })),
            visual: ComponentVisualState::default(),
            a11y: a11y_spec,
        }));
    }
    panel("variant-diff.actions", "Actions", children)
}

/// The action-params payload a button carries — the exact shape
/// [`decode_variant_diff_action`] accepts (pins only; authority-derived
/// fields like `scope_digest` are never client-asserted).
fn action_params(kind: VariantDiffActionKind, intent: &VariantDiffIntent) -> serde_json::Value {
    match (kind, intent) {
        (VariantDiffActionKind::Compare, VariantDiffIntent::Compare { left, right }) => {
            serde_json::json!({ "left": left, "right": right })
        }
        (
            VariantDiffActionKind::AdoptPreview,
            VariantDiffIntent::AdoptPreview { target, .. }
            | VariantDiffIntent::Adopt { target, .. },
        )
        | (
            VariantDiffActionKind::Adopt,
            VariantDiffIntent::AdoptPreview { target, .. }
            | VariantDiffIntent::Adopt { target, .. },
        ) => serde_json::to_value(target).unwrap_or_default(),
        _ => serde_json::Value::Null,
    }
}

fn action_button(
    key_name: &str,
    label_text: &str,
    action: &str,
    params: serde_json::Value,
) -> ComponentSpec {
    ComponentSpec::Button(ButtonSpec {
        key: key(key_name),
        label: label_text.to_string(),
        target: VARIANT_DIFF_TARGET.to_string(),
        action: action.to_string(),
        params: Some(params),
        visual: ComponentVisualState::default(),
        a11y: a11y(A11yRole::Button, label_text),
    })
}

// ── label helpers ───────────────────────────────────────────────────

/// Kebab-case label for a unit serde enum (`ObjectKind`, `ChangeKind`).
fn serde_label<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}

/// Issue-variant label — `ComparisonIssue` is a tagged enum whose serde
/// contract carries the variant under `"issue"`.
fn issue_kind_label(issue: &ComparisonIssue) -> String {
    serde_json::to_value(issue)
        .ok()
        .and_then(|v| v.get("issue").and_then(|i| i.as_str()).map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}

fn evidence_state_text(state: &EvidenceState) -> String {
    match state {
        EvidenceState::Available => "available".to_string(),
        EvidenceState::Unavailable { reason } => format!("unavailable ({})", reason.code()),
        EvidenceState::Unknown { reason } => format!("unknown ({})", reason.code()),
        EvidenceState::Unsupported { reason } => format!("unsupported ({})", reason.code()),
    }
}

fn equality_label(eq: &ComparisonEquality) -> &'static str {
    match eq {
        ComparisonEquality::Equal => "equal",
        ComparisonEquality::Different => "different",
        ComparisonEquality::Unknown(_) => "unknown",
    }
}

fn completeness_label(c: ComparisonCompleteness) -> &'static str {
    match c {
        ComparisonCompleteness::Complete => "complete",
        ComparisonCompleteness::Partial => "partial",
    }
}

fn alignment_label(a: AlignmentState) -> &'static str {
    match a {
        AlignmentState::Aligned => "aligned",
        AlignmentState::Approximate => "approximate",
        AlignmentState::Unaligned => "unaligned",
        AlignmentState::Unavailable => "unavailable",
    }
}

// ── CB-08 provider adapter + intent dispatch facade ─────────────────
//
// This section wires the merged CB-08 `ComparisonOrchestrator` and the
// CB-10 variant ports into the explorer's evidence/intent contracts.
// It lives in this module (not a new file) because `synvelis-workspace`
// already depends on `synvelis-project` and `synvelis-gateway`, and the
// gateway types stay *inside* this facade — `synvelis-app` never names
// them (AC-05).
//
// Honesty rules carried across the boundary:
//
// - Layer states map one-to-one (`Available`/`Unavailable`/`Unknown`/
//   `Unsupported`); a CB-08 gap is never folded into "zero difference".
// - Evidence provenance is preserved: pipeline/derived output maps to
//   [`EvidenceOrigin::Pipeline`], fixture output to
//   [`EvidenceOrigin::Mock`] — mock rows stay labelled mock.
// - Freshness is pin-scoped: [`VariantDiffLayers::freshness`] reports
//   the orchestrator's verdict at evaluation time, and
//   [`project_variant_diff`] independently re-derives staleness as
//   [`ContextState`] from *live* authority on every projection. Callers
//   must build [`CompareAuthority`] from live project state, never from
//   a session snapshot, or stale evidence would launder as current.
// - Adopt preview/execute go through the same `run_intent` dispatch the
//   gateway commit gate uses (`simulate`/`execute`), so preview and
//   commit cannot diverge; the CAS guards (project revision, branch
//   revision, scope digest, approval ticket) are re-checked by the
//   domain engine at commit time.

/// Bounded semantic-diff cache for the facade's orchestrator — same
/// order as the CB-08 test rig.
const VARIANT_DIFF_CACHE_CAPACITY: usize = 16;

/// Host-observed per-branch facts [`variant_diff_authority`] cannot
/// derive from [`Project`] alone: open edit sessions (view-local
/// observation, §7.3 Editing label) and the runtime publish state from
/// the last adopt receipt. Absent entries default to `false` /
/// [`RuntimePublishState::NotRequested`] — honest "not observed",
/// never a fabricated `Published`.
#[derive(Clone, Debug, Default)]
pub struct VariantDiffBranchObservations {
    /// Branch id → host-observed "edit session open" flag.
    pub editing: BTreeMap<BranchId, bool>,
    /// Branch id → runtime publish observation from the last adopt.
    pub publish: BTreeMap<BranchId, RuntimePublishState>,
}

/// The three CB-09 evidence layers plus the orchestrator's freshness
/// verdict for one CB-08 evaluation.
#[derive(Clone, Debug)]
pub struct VariantDiffLayers {
    /// Semantic (Configured) layer — the canonical `ChangeSet`.
    pub semantic: SemanticEvidence,
    /// Signal (Measured) layer — provider-gated honest states only.
    pub signal: AuxLayerEvidence,
    /// Perceptual (Inferred) layer — `RequiresAudition` stays a typed
    /// gap headlessly, never fabricated.
    pub perceptual: AuxLayerEvidence,
    /// Freshness of the session's pinned context at evaluation time.
    /// Pin-scoped evidence freshness — see the module note above for
    /// how it relates to [`ContextState`].
    pub freshness: ComparisonFreshness,
}

/// Dispatch failures for the adopt preview/execute facade. The inner
/// errors stay typed for callers that can name them; the app maps them
/// to wire error bodies through [`Self::code`] without naming gateway
/// types (AC-05).
#[derive(Debug)]
#[non_exhaustive]
pub enum VariantDiffDispatchError {
    /// Intent/authority validation failed — same typed rejects the
    /// view-model apply path produces.
    Reject(VariantDiffReject),
    /// The variant engine rejected the operation (stale CAS, missing
    /// adapter, preflight refusal, store failure).
    Variant(VariantError),
    /// The adopt commit path failed — determined rejection or
    /// project-CAS conflict, never indeterminate.
    Commit(synvelis_gateway::variant::VariantCommitError),
}

impl VariantDiffDispatchError {
    /// Stable wire-facing reason code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Reject(VariantDiffReject::ImplicitTarget) => "implicit-target",
            Self::Reject(VariantDiffReject::StaleRevision) => "stale-revision",
            Self::Reject(VariantDiffReject::UnknownBranch) => "unknown-branch",
            Self::Reject(VariantDiffReject::StaleContext) => "stale-context",
            Self::Reject(VariantDiffReject::UnsupportedDomain) => "unsupported-domain",
            Self::Reject(VariantDiffReject::CapabilityUnavailable) => "capability-unavailable",
            Self::Reject(VariantDiffReject::AlreadyAdopted) => "already-adopted",
            Self::Reject(VariantDiffReject::SameBranch) => "same-branch",
            Self::Reject(VariantDiffReject::Malformed) => "malformed",
            Self::Variant(_) => "variant-error",
            Self::Commit(_) => "commit-error",
        }
    }

    /// Finer-grained wire reason than [`Self::code`] — commit
    /// rejections surface the gateway's `variant-<kind>` slug and
    /// project-CAS conflicts report `conflicted`, so drivers can
    /// distinguish a stale-CAS conflict from a missing-plugin
    /// refusal without parsing prose.
    #[must_use]
    pub fn detail_reason(&self) -> String {
        match self {
            Self::Commit(synvelis_gateway::variant::VariantCommitError::Rejected { reason }) => {
                reason
                    .split(':')
                    .next()
                    .unwrap_or("commit-error")
                    .to_owned()
            }
            Self::Commit(synvelis_gateway::variant::VariantCommitError::Conflicted { .. }) => {
                "conflicted".to_owned()
            }
            _ => self.code().to_owned(),
        }
    }
}

impl std::fmt::Display for VariantDiffDispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reject(reject) => write!(f, "variant-diff reject: {reject:?}"),
            Self::Variant(err) => write!(f, "variant engine error: {err}"),
            Self::Commit(synvelis_gateway::variant::VariantCommitError::Rejected { reason }) => {
                write!(f, "variant commit rejected: {reason}")
            }
            Self::Commit(synvelis_gateway::variant::VariantCommitError::Conflicted {
                expected,
                actual,
            }) => write!(
                f,
                "variant commit conflicted: expected revision {expected}, actual {actual}"
            ),
        }
    }
}

impl std::error::Error for VariantDiffDispatchError {}

/// CB-08 [`EvidenceReason`] → CB-09 [`VariantDiffReason`] — one honest
/// reason per gap kind; nothing is promoted to `Available`.
fn diff_reason(reason: &EvidenceReason) -> VariantDiffReason {
    match reason {
        EvidenceReason::NoProvider | EvidenceReason::ProviderUnavailable => {
            VariantDiffReason::EvidenceUnavailable
        }
        EvidenceReason::AlignmentPolicy | EvidenceReason::NoTimeMapping => {
            VariantDiffReason::LayerUnsupported
        }
        EvidenceReason::UnalignedInputs => VariantDiffReason::NoAlignmentEvidence,
        EvidenceReason::PluginStateUnknown => VariantDiffReason::MissingPlugin,
        EvidenceReason::EvidenceQuality => VariantDiffReason::EvidenceUnknown,
        EvidenceReason::RequiresAudition => VariantDiffReason::EvidenceUnavailable,
    }
}

/// CB-08 [`LayerState`] → CB-09 [`EvidenceState`].
fn diff_evidence_state(state: &LayerState, reason: &EvidenceReason) -> EvidenceState {
    match state {
        LayerState::Available => EvidenceState::Available,
        LayerState::Unavailable { .. } => EvidenceState::Unavailable {
            reason: diff_reason(reason),
        },
        LayerState::Unknown { .. } => EvidenceState::Unknown {
            reason: diff_reason(reason),
        },
        LayerState::Unsupported { .. } => EvidenceState::Unsupported {
            reason: diff_reason(reason),
        },
    }
}

/// The `EvidenceReason` a non-`Available` [`LayerState`] carries.
fn layer_reason(state: &LayerState) -> EvidenceReason {
    match state {
        LayerState::Available => EvidenceReason::NoProvider,
        LayerState::Unavailable { reason }
        | LayerState::Unknown { reason }
        | LayerState::Unsupported { reason } => reason.clone(),
    }
}

/// CB-08 signal provenance → CB-09 [`EvidenceOrigin`]. Pipeline and
/// derived (offline render/cached analysis) output are both real
/// measured evidence — only fixture output stays `Mock`.
fn signal_origin(origin: synvelis_project::variant::EvidenceOrigin) -> EvidenceOrigin {
    match origin {
        synvelis_project::variant::EvidenceOrigin::Pipeline
        | synvelis_project::variant::EvidenceOrigin::Derived => EvidenceOrigin::Pipeline,
        synvelis_project::variant::EvidenceOrigin::Mock => EvidenceOrigin::Mock,
    }
}

/// CB-08 [`SignalAlignment`] → CB-09 [`AlignmentState`].
fn signal_alignment(alignment: SignalAlignment) -> AlignmentState {
    match alignment {
        SignalAlignment::Aligned => AlignmentState::Aligned,
        SignalAlignment::Approximate => AlignmentState::Approximate,
        SignalAlignment::Unaligned => AlignmentState::Unaligned,
        SignalAlignment::Unavailable => AlignmentState::Unavailable,
    }
}

/// `SignalDomain` label for evidence-row detail — kebab-case, matching
/// the serde vocabulary.
fn signal_domain_label(domain: synvelis_project::variant::SignalDomain) -> &'static str {
    match domain {
        synvelis_project::variant::SignalDomain::SourceFrame => "source-frame",
        synvelis_project::variant::SignalDomain::Musical => "musical",
        synvelis_project::variant::SignalDomain::ProjectFrame => "project-frame",
    }
}

/// Projects one CB-08 [`SignalLayerResult`] into [`AuxLayerEvidence`].
/// Metric rows are carried verbatim — labels are the provider's metric
/// names and values/details format the provider's own numbers; nothing
/// is interpreted, normalized, or fabricated.
fn aux_from_signal(result: &SignalLayerResult) -> AuxLayerEvidence {
    if !result.state.is_available() {
        return AuxLayerEvidence {
            state: diff_evidence_state(&result.state, &layer_reason(&result.state)),
            ..AuxLayerEvidence::default()
        };
    }
    let origin = result
        .origin
        .map_or(EvidenceOrigin::Pipeline, signal_origin);
    AuxLayerEvidence::available(
        origin,
        signal_alignment(result.alignment),
        result
            .metrics
            .iter()
            .map(|metric| EvidenceRow {
                label: metric.name.clone(),
                value: Some(format!(
                    "{} → {} (Δ {})",
                    metric.before, metric.after, metric.delta
                )),
                detail: Some(format!(
                    "{} {}",
                    signal_domain_label(metric.domain),
                    metric.unit
                )),
            })
            .collect(),
    )
}

/// Projects one CB-08 [`PerceptualLayerResult`] into
/// [`AuxLayerEvidence`]. Headless the layer is always an honest gap
/// (`RequiresAudition`); an `Available` claim without provenance still
/// meets [`AuxLayerEvidence`]'s origin gate downstream.
fn aux_from_perceptual(result: &PerceptualLayerResult) -> AuxLayerEvidence {
    AuxLayerEvidence {
        state: diff_evidence_state(&result.state, &layer_reason(&result.state)),
        ..AuxLayerEvidence::default()
    }
}

/// Converts one CB-08 [`ComparisonResult`] into the CB-09 evidence
/// bundle. Pure projection — the freshness verdict is carried through
/// for the caller; [`project_variant_diff`] re-derives context
/// staleness independently from live authority.
#[must_use]
pub fn variant_diff_layers(result: &ComparisonResult) -> VariantDiffLayers {
    let semantic = if result.semantic.state.is_available() {
        SemanticEvidence {
            state: EvidenceState::Available,
            diff: Some(SemanticDiffEvidence::from(
                result.semantic.changeset.as_ref(),
            )),
        }
    } else {
        SemanticEvidence {
            state: diff_evidence_state(
                &result.semantic.state,
                &layer_reason(&result.semantic.state),
            ),
            diff: None,
        }
    };
    VariantDiffLayers {
        semantic,
        signal: aux_from_signal(&result.signal),
        perceptual: aux_from_perceptual(&result.perceptual),
        freshness: result.freshness.clone(),
    }
}

/// Builds read-only [`CompareAuthority`] from live project state for
/// `variant_set`. `project_revision` is the controller's live
/// revision; `observations` carries host-only facts (edit sessions,
/// last publish receipt). Fails closed on an unknown set or a scope
/// whose fingerprint cannot be computed.
///
/// # Errors
/// [`synvelis_project::Error`] when the set is absent or the scope
/// digest fails.
pub fn variant_diff_authority(
    project: &Project,
    variant_set: VariantSetId,
    project_revision: ProjectRevision,
    observations: &VariantDiffBranchObservations,
) -> Result<CompareAuthority, synvelis_project::Error> {
    let set = project
        .variant_sets
        .get(&variant_set)
        .ok_or_else(|| synvelis_project::Error::NotFound(format!("variant set {variant_set}")))?;
    let branches = set
        .branches
        .iter()
        .map(|(id, branch)| {
            (
                *id,
                BranchAuthority {
                    revision: branch.content.revision(),
                    editing: observations.editing.get(id).copied().unwrap_or(false),
                    publish: observations
                        .publish
                        .get(id)
                        .cloned()
                        .unwrap_or(RuntimePublishState::NotRequested),
                },
            )
        })
        .collect();
    Ok(CompareAuthority {
        variant_set,
        scope: set.scope.clone(),
        adopted: set.adopted,
        project_revision,
        scope_digest: scope_digest(project, &set.scope)?,
        branches,
    })
}

/// Builds a [`CompareContext`] for two explicit pins against live
/// project state — scope identity and the scope-content fingerprint
/// are re-derived, never trusted from the caller.
///
/// # Errors
/// [`synvelis_project::Error`] when the set is absent or the scope
/// digest fails.
pub fn variant_diff_context(
    project: &Project,
    variant_set: VariantSetId,
    left: BranchPin,
    right: BranchPin,
    project_revision: ProjectRevision,
) -> Result<CompareContext, synvelis_project::Error> {
    let set = project
        .variant_sets
        .get(&variant_set)
        .ok_or_else(|| synvelis_project::Error::NotFound(format!("variant set {variant_set}")))?;
    Ok(CompareContext {
        variant_set,
        scope: set.scope.clone(),
        left,
        right,
        scope_digest: scope_digest(project, &set.scope)?,
        project_revision,
    })
}

/// The CB-08/CB-10 dispatch facade. Owns the comparison orchestrator
/// (bounded semantic-diff cache) and the variant ports (shared branch
/// store + runtime publish + asset/plugin probes) so callers —
/// `synvelis-app` included — never name `synvelis-gateway` types.
///
/// Every method takes live authority explicitly; the only mutating
/// path is [`Self::adopt`], which runs the same CAS/approval dispatch
/// the gateway commit gate uses.
pub struct VariantDiffEngine {
    orchestrator: ComparisonOrchestrator,
    ports: synvelis_gateway::variant::VariantPorts,
}

impl VariantDiffEngine {
    /// Wires the standard variant ports under `content_root` (the
    /// project bundle directory — `None` only for project-less
    /// contexts) plus the CB-07
    /// [`synvelis_project::variant::ArrangementAdapter`] the gateway's
    /// standard table predates. The arrangement adapter is merged in
    /// `synvelis-project`; inserting it here keeps arrangement scopes
    /// resolving honestly instead of failing closed on a stale table
    /// (gateway files are consume-only).
    #[must_use]
    pub fn new(content_root: Option<&std::path::Path>) -> Self {
        let mut ports = synvelis_gateway::variant::VariantPorts::standard(content_root);
        ports.adapters.insert(
            synvelis_gateway::variant::VariantScopeKind::Arrangement,
            std::sync::Arc::new(synvelis_project::variant::ArrangementAdapter::new()),
        );
        Self {
            orchestrator: ComparisonOrchestrator::new(VARIANT_DIFF_CACHE_CAPACITY),
            ports,
        }
    }

    /// Compares two explicitly pinned branches through the CB-08
    /// orchestrator and returns the CB-09 evidence bundle.
    ///
    /// The explorer's asserted pin revisions are CAS-checked against
    /// live content *before* the session opens — a stale pin fails
    /// [`CompareError::StaleRevision`] rather than silently comparing
    /// whatever the branch currently holds. `signal` is the optional
    /// measured-evidence provider; `None` yields the honest
    /// `Unavailable{NoProvider}` layer.
    ///
    /// # Errors
    /// [`CompareError`] on unknown endpoints, stale asserted pins,
    /// scope mismatch, or capture/diff engine failures.
    #[allow(clippy::too_many_arguments)]
    pub fn compare(
        &mut self,
        live: &Project,
        variant_set: VariantSetId,
        left: &BranchPin,
        right: &BranchPin,
        project_revision: ProjectRevision,
        options: &SessionOptions,
        signal: Option<&dyn SignalComparisonProvider>,
    ) -> Result<VariantDiffLayers, CompareError> {
        let store_guard = self
            .ports
            .store
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let store: &dyn BranchContentStore = &**store_guard;
        let left_ref = self
            .orchestrator
            .pin_branch(live, store, variant_set, left.branch)?;
        let right_ref = self
            .orchestrator
            .pin_branch(live, store, variant_set, right.branch)?;
        for (side, asserted, actual) in [
            (
                synvelis_project::variant::CompareSide::Left,
                left,
                &left_ref,
            ),
            (
                synvelis_project::variant::CompareSide::Right,
                right,
                &right_ref,
            ),
        ] {
            if asserted.revision != actual.revision {
                return Err(CompareError::StaleRevision {
                    side,
                    expected: asserted.revision,
                    actual: actual.revision,
                });
            }
        }
        // Asset availability probing is the same port the commit gate
        // uses — `None` fails closed inside capture, never fabricates.
        let blobs: Option<&dyn BlobProbe> = self
            .ports
            .assets
            .as_ref()
            .map(|probe| -> &dyn BlobProbe { probe.as_ref() });
        let session = self.orchestrator.open_session(
            live,
            store,
            left_ref,
            right_ref,
            project_revision,
            options,
            blobs,
            &SnapshotCancel::new(),
        )?;
        let result = self.orchestrator.evaluate(
            &session,
            live,
            store,
            project_revision,
            signal,
            &SnapshotCancel::new(),
        )?;
        Ok(variant_diff_layers(&result))
    }

    /// Runs the adopt *preview* for a validated
    /// [`VariantDiffIntent::AdoptPreview`] — the same `run_intent`
    /// dispatch commit uses, on a scratch controller with a
    /// read-through store, so the previewed post-state is exactly what
    /// commit would produce (barring later drift, which the CAS
    /// guards re-check honestly).
    ///
    /// # Errors
    /// [`VariantDiffDispatchError::Reject`] on intent/authority
    /// mismatch, [`VariantDiffDispatchError::Variant`] on engine
    /// failure.
    pub fn adopt_preview(
        &self,
        base: &Project,
        authority: &CompareAuthority,
        intent: &VariantDiffIntent,
    ) -> Result<Project, VariantDiffDispatchError> {
        if !matches!(intent, VariantDiffIntent::AdoptPreview { .. }) {
            return Err(VariantDiffDispatchError::Reject(
                VariantDiffReject::Malformed,
            ));
        }
        let adopt = Self::adopt_intent(authority, intent)?;
        let variant_intent = synvelis_gateway::variant::VariantIntent::Adopt { intent: adopt };
        synvelis_gateway::variant::simulate(base, &self.ports, &variant_intent)
            .map_err(VariantDiffDispatchError::Variant)
    }

    /// Runs the adopt CAS for a validated [`VariantDiffIntent::Adopt`]
    /// — the same `run_intent` dispatch the gateway commit gate uses:
    /// adapter preflight, asset/plugin/port preflight, project CAS,
    /// undo step, then runtime publish whose evidence stays a
    /// separate receipt block.
    ///
    /// `approval_ticket` is the host's explicit-approval token — the
    /// GUI fills it only after the user confirms this exact intent
    /// (same contract as [`synvelis_project::apply::ApplyApproval`]).
    ///
    /// # Errors
    /// [`VariantDiffDispatchError::Reject`] on intent/authority
    /// mismatch, [`VariantDiffDispatchError::Variant`] on determined
    /// engine rejection, [`VariantDiffDispatchError::Commit`] on
    /// commit-path failure.
    pub fn adopt(
        &self,
        controller: &mut ProjectController,
        authority: &CompareAuthority,
        intent: &VariantDiffIntent,
        approval_ticket: &str,
    ) -> Result<serde_json::Value, VariantDiffDispatchError> {
        // A preview intent must never reach the commit path — the
        // intent variant is part of the authority contract, not a
        // hint the callee may reinterpret.
        if !matches!(intent, VariantDiffIntent::Adopt { .. }) {
            return Err(VariantDiffDispatchError::Reject(
                VariantDiffReject::Malformed,
            ));
        }
        let adopt = Self::adopt_intent(authority, intent)?;
        let variant_intent = synvelis_gateway::variant::VariantIntent::Adopt { intent: adopt };
        synvelis_gateway::variant::execute(
            controller,
            &self.ports,
            &variant_intent,
            approval_ticket,
        )
        .map_err(VariantDiffDispatchError::Commit)
    }

    /// Validates an adopt-family intent against fresh authority and
    /// lowers it to the engine-level [`AdoptIntent`]. Re-checks the
    /// same facts the view-model apply path enforced — branch known,
    /// revision current, scope digest matching, target not already
    /// adopted — so a detached or replayed intent cannot slip past a
    /// drifted view.
    fn adopt_intent(
        authority: &CompareAuthority,
        intent: &VariantDiffIntent,
    ) -> Result<AdoptIntent, VariantDiffDispatchError> {
        let (target, scope_digest) = match intent {
            VariantDiffIntent::AdoptPreview {
                target,
                scope_digest,
            }
            | VariantDiffIntent::Adopt {
                target,
                scope_digest,
            } => (target, *scope_digest),
            VariantDiffIntent::Compare { .. } => {
                return Err(VariantDiffDispatchError::Reject(
                    VariantDiffReject::Malformed,
                ));
            }
        };
        let branch =
            authority
                .branches
                .get(&target.branch)
                .ok_or(VariantDiffDispatchError::Reject(
                    VariantDiffReject::UnknownBranch,
                ))?;
        if branch.revision != target.revision {
            return Err(VariantDiffDispatchError::Reject(
                VariantDiffReject::StaleRevision,
            ));
        }
        if target.branch == authority.adopted {
            return Err(VariantDiffDispatchError::Reject(
                VariantDiffReject::AlreadyAdopted,
            ));
        }
        if scope_digest != authority.scope_digest {
            return Err(VariantDiffDispatchError::Reject(
                VariantDiffReject::StaleContext,
            ));
        }
        Ok(AdoptIntent {
            variant_set: authority.variant_set,
            branch: target.branch,
            expected_project_revision: authority.project_revision,
            expected_branch_revision: target.revision,
            expected_scope_digest: scope_digest,
            // Placeholder — `execute` substitutes the consumed
            // approval receipt's id; `simulate` its scratch ticket.
            approval: synvelis_project::apply::ApplyApproval {
                ticket: String::new(),
            },
        })
    }
}
