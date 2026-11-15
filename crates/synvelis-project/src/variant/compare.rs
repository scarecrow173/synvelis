//! CB-08 Comparison Orchestrator (synvelis#285).
//!
//! Headless, read-only orchestration of the three Creative Branch
//! comparison layers — **Semantic** (Configured, VCS-02
//! [`crate::diff::compare_snapshots`]), **Signal** (Measured, provider
//! seam), and **Perceptual** (Inferred, interactive audition only) —
//! over two pinned [`BranchRevisionRef`] endpoints inside one
//! [`VariantSet`].
//!
//! # Contracts
//!
//! - **Pin, don't follow.** [`ComparisonOrchestrator::open_session`]
//!   resolves both branches through [`resolve_render_project`], verifies
//!   the caller-asserted revisions/content digests, captures the composed
//!   views as [`ImmutableSnapshot`]s, and fixes an
//!   [`EvaluationContext`] digest bundle (project revision, per-side
//!   scope/shared-context fingerprints, tempo/ppqn, clock-epoch
//!   time-mapping, plugin-instance, and source-material dimensions).
//!   The session never mutates the project and is non-persistent
//!   Derived state — save/reload discards it.
//! - **Stale is explicit.** [`ComparisonOrchestrator::freshness`]
//!   re-derives every pinned component against current authority and
//!   reports [`ComparisonFreshness::Stale`] with typed
//!   [`StaleDimension`]s. A stale session's evidence is pin-scoped
//!   derived data — never current proof — and aux layers are flagged
//!   `stale` so consumers cannot present old measurements as observed.
//! - **Layers never merge.** `Unavailable`/`Unknown`/`Unsupported`
//!   evidence is never collapsed into equality, `Aligned`, or `Pass`.
//!   Signal evidence additionally fails closed when either view carries
//!   non-`Known` opaque plugin state: headless cannot verify plugin
//!   state, so no provider output may claim exact measured status.
//! - **Cache is derived, never truth.** Semantic results reuse the
//!   bounded [`ChangeSetCache`] keyed by snapshot digests + spec — any
//!   content move (scope, shared context, tempo, plugin, source) changes
//!   a digest and misses. The cache is a reuse optimization only;
//!   [`ComparisonFreshness`] remains the validity oracle.
//!
//! Real signal rendering/probing, PCM delta, audition, and perceptual
//! acceptance are outside this slice: the signal layer is a provider
//! seam that reports honest gaps until a render/probe path lands, and
//! the perceptual layer is always `Unavailable` headlessly.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::controller::ProjectRevision;
use crate::diff::{
    ChangeSet, ChangeSetCache, ComparisonBudget, ComparisonDomains, ComparisonSpec,
    DIFF_POLICY_VERSION, DiffError, ObjectKind, SemanticObjectRef,
};
use crate::error::Error;
use crate::id::{BranchId, VariantSetId};
use crate::project::Project;
use crate::render_input::{
    BranchRenderPin, RenderBranchTarget, ResolvedRenderProject, all_semantic_objects,
    canonical_fingerprint, resolve_render_project,
};
use crate::snapshot::{
    BlobProbe, ContentDigest, ImmutableSnapshot, OpaqueStateStatus, SnapshotCancel, SnapshotError,
    SnapshotResolver,
};
use crate::variant::scope::scope_objects;
use crate::variant::{BranchContentStore, BranchRevision, VariantScope};

// ---------------------------------------------------------------------------
// pinned endpoints
// ---------------------------------------------------------------------------

/// Which side of a comparison a [`StaleDimension`] or error refers to.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompareSide {
    /// The context's left pin.
    Left,
    /// The context's right pin.
    Right,
}

impl CompareSide {
    /// Stable label for diagnostics.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// A caller-assertable pin on one branch endpoint: variant set, branch,
/// content revision, and the canonical scope-content fingerprint the
/// revision represents. Obtained from
/// [`ComparisonOrchestrator::pin_branch`] (or a prior session) and
/// verified fail-closed at [`ComparisonOrchestrator::open_session`] —
/// the same CAS discipline as [`crate::variant::AdoptIntent`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BranchRevisionRef {
    /// Variant set containing the branch.
    pub variant_set: VariantSetId,
    /// The pinned branch.
    pub branch: BranchId,
    /// Branch content revision asserted by the caller.
    pub revision: BranchRevision,
    /// Canonical (display-name-free) scope-content fingerprint asserted
    /// by the caller — recomputed during resolution, never trusted.
    pub content_digest: ContentDigest,
}

// ---------------------------------------------------------------------------
// session policy
// ---------------------------------------------------------------------------

/// Declared time-alignment posture for signal-layer evidence.
///
/// This is a *policy input*, not a measured state: it records what the
/// caller asserts about the two views' time bases. `Unaligned` blocks
/// signal evidence outright — delta claims over unmapped time bases
/// would be fabricated certainty.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AlignmentPolicy {
    /// Inputs share an identical time base.
    Aligned,
    /// Inputs share a declared non-identity mapping (e.g. tempo ratio)
    /// that a provider must apply before comparing.
    Mapped,
    /// No reliable time mapping exists — signal evidence is refused.
    Unaligned,
    /// Alignment cannot be determined (default — honest unknown).
    #[default]
    Unknown,
}

/// Monitor-path policy for audition/A-B evaluation. Applies to the
/// monitoring path only — it never modifies compared signal content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MonitoringPolicy {
    /// Monitor gain in dB applied on the audition path only.
    pub gain_db: f64,
    /// Whether loudness/level matching is requested on the monitor path.
    pub level_match: bool,
    /// Blind A/B presentation (labels hidden) for perceptual evaluation.
    pub blind: bool,
}

impl Default for MonitoringPolicy {
    fn default() -> Self {
        Self {
            gain_db: 0.0,
            level_match: false,
            blind: false,
        }
    }
}

impl MonitoringPolicy {
    fn validate(&self) -> Result<(), CompareError> {
        if !self.gain_db.is_finite() {
            return Err(CompareError::InvalidPolicy {
                detail: "monitoring gain_db must be finite".into(),
            });
        }
        Ok(())
    }
}

/// Domain/budget/alignment/monitor selection pinned at session open.
#[derive(Clone, Debug)]
pub struct SessionOptions {
    /// Semantic-diff domains participating in the comparison.
    pub domains: ComparisonDomains,
    /// Semantic-diff resource bounds.
    pub budget: ComparisonBudget,
    /// Declared signal-alignment policy.
    pub alignment: AlignmentPolicy,
    /// Monitor-path policy (audition surface only).
    pub monitor: MonitoringPolicy,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            domains: ComparisonDomains::all(),
            budget: ComparisonBudget::default(),
            alignment: AlignmentPolicy::Unknown,
            monitor: MonitoringPolicy::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// pinned evaluation context
// ---------------------------------------------------------------------------

/// The digest bundle fixed at session open. Every component is
/// recomputed during [`ComparisonOrchestrator::freshness`]; any drift is
/// a typed [`StaleDimension`], never silent reuse.
///
/// `left`/`right` [`BranchRenderPin`]s already carry the per-side
/// scope fingerprint and the shared-context complement digest; the
/// remaining fields partition the *shared* context into the dimensions
/// the issue names explicitly (tempo/ppqn, clock-epoch time mapping,
/// plugin-instance state, source material) so staleness can be
/// classified instead of reported as an opaque blob.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluationContext {
    /// Live project revision pinned at session open.
    pub project_revision: ProjectRevision,
    /// Resolved render pin of the left branch view.
    pub left: BranchRenderPin,
    /// Resolved render pin of the right branch view.
    pub right: BranchRenderPin,
    /// Canonical fingerprint of the project singleton (ppqn + tempo
    /// map + project id) inside shared context.
    pub tempo_digest: ContentDigest,
    /// Canonical fingerprint of clock-epoch objects inside shared
    /// context (musical↔session time mapping material).
    pub time_mapping_digest: ContentDigest,
    /// Canonical fingerprint of plugin-instance identity/binding
    /// records inside shared context.
    pub plugin_digest: ContentDigest,
    /// Canonical fingerprint of asset/media/MIDI source material
    /// inside shared context.
    pub source_digest: ContentDigest,
    /// Canonical fingerprint of shared-context objects *outside* the
    /// four named dimensions — the remainder that [`StaleDimension::SharedContext`]
    /// reports when it alone moves.
    pub remainder_digest: ContentDigest,
}

/// A pinned, non-persistent comparison session (design §4.1
/// `Comparing`). Read-only derived state: it holds the two composed
/// branch views captured at open plus the fixed [`EvaluationContext`].
/// Save/reload discards it; regeneration starts from staleness
/// detection.
#[derive(Clone, Debug)]
pub struct ComparisonSession {
    /// Left endpoint pin.
    pub left: BranchRevisionRef,
    /// Right endpoint pin.
    pub right: BranchRevisionRef,
    /// The fixed evaluation context.
    pub pinned_context: EvaluationContext,
    /// Declared signal-alignment policy.
    pub alignment: AlignmentPolicy,
    /// Monitor-path policy.
    pub monitor: MonitoringPolicy,
    /// Semantic-diff domains pinned at open.
    pub domains: ComparisonDomains,
    /// Semantic-diff budget pinned at open.
    pub budget: ComparisonBudget,
    /// Pinned composed view of the left branch (immutable snapshot).
    left_view: ImmutableSnapshot,
    /// Pinned composed view of the right branch (immutable snapshot).
    right_view: ImmutableSnapshot,
}

impl ComparisonSession {
    /// The pinned left composed view.
    #[must_use]
    pub fn left_view(&self) -> &ImmutableSnapshot {
        &self.left_view
    }

    /// The pinned right composed view.
    #[must_use]
    pub fn right_view(&self) -> &ImmutableSnapshot {
        &self.right_view
    }
}

// ---------------------------------------------------------------------------
// freshness / staleness
// ---------------------------------------------------------------------------

/// One pinned-context component that no longer matches authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "dimension", rename_all = "kebab-case")]
pub enum StaleDimension {
    /// Live project revision advanced since the pin.
    ProjectRevision {
        /// Revision pinned at session open.
        pinned: ProjectRevision,
        /// Revision observed now.
        actual: ProjectRevision,
    },
    /// A branch's content revision moved.
    BranchRevision {
        /// Which endpoint moved.
        side: CompareSide,
        /// Revision pinned at session open.
        pinned: BranchRevision,
        /// Revision observed now.
        actual: BranchRevision,
    },
    /// A branch's canonical scope-content fingerprint moved.
    ScopeContent {
        /// Which endpoint moved.
        side: CompareSide,
    },
    /// A branch's stored payload digest moved while its revision stayed
    /// put — content-store corruption, reported as staleness rather than
    /// trusted.
    BranchPayload {
        /// Which endpoint moved.
        side: CompareSide,
    },
    /// The project singleton's ppqn/tempo-map content moved.
    Tempo,
    /// Clock-epoch time-mapping material moved.
    TimeMapping,
    /// Plugin-instance identity/binding records moved.
    Plugin,
    /// Asset/media/MIDI source material moved.
    Source,
    /// Other shared (non-scope) context moved — routing, strips,
    /// recording registry, monitoring, and any dimension without a
    /// finer classification.
    SharedContext,
}

/// Whether a session's pinned context still matches authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum ComparisonFreshness {
    /// Every pinned component still matches.
    Current,
    /// At least one pinned component moved; listed reasons are
    /// deterministic and complete for the observed drift.
    Stale {
        /// Typed stale dimensions, in check order.
        reasons: Vec<StaleDimension>,
    },
}

impl ComparisonFreshness {
    /// `true` when any pinned component moved.
    #[must_use]
    pub const fn is_stale(&self) -> bool {
        matches!(self, Self::Stale { .. })
    }
}

// ---------------------------------------------------------------------------
// layered result
// ---------------------------------------------------------------------------

/// Evidence availability of one comparison layer. `Unavailable`,
/// `Unknown`, and `Unsupported` are distinct honest outcomes — none of
/// them may be presented as equality or success.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum LayerState {
    /// Evidence was produced for the pinned context.
    Available,
    /// Evidence cannot be produced; `reason` says why.
    Unavailable {
        /// Why evidence is unavailable.
        reason: EvidenceReason,
    },
    /// Evidence exists but cannot support a verdict.
    Unknown {
        /// Why evidence is unknown.
        reason: EvidenceReason,
    },
    /// The layer is not supported in this context.
    Unsupported {
        /// Why the layer is unsupported.
        reason: EvidenceReason,
    },
}

impl LayerState {
    /// `true` only for [`Self::Available`].
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Why a layer's evidence is not `Available`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum EvidenceReason {
    /// No provider is configured for this layer.
    NoProvider,
    /// The session's declared alignment policy refuses this layer.
    AlignmentPolicy,
    /// Inputs share no reliable time mapping — delta evidence would
    /// misattribute time-base differences to signal differences.
    UnalignedInputs,
    /// Either view carries non-`Known` opaque plugin state; headless
    /// cannot verify plugin state so no measured-exact claim is allowed.
    PluginStateUnknown,
    /// The provider reported it cannot produce evidence.
    ProviderUnavailable,
    /// Provider output failed validation (e.g. non-finite metrics).
    EvidenceQuality,
    /// Perceptual evaluation requires interactive audition — outside
    /// the headless slice (CB-09/CB-10 surface).
    RequiresAudition,
    /// No defined time-domain mapping exists for this scope's signal.
    NoTimeMapping,
}

/// Provenance class of aux-layer evidence — real pipeline output must
/// never be conflated with fixtures.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceOrigin {
    /// Real render/probe pipeline output.
    Pipeline,
    /// Offline render or derived/cached analysis.
    Derived,
    /// Test/fixture provider output — visibly not real evidence.
    Mock,
}

/// Alignment quality a signal provider achieved, in the CB-09
/// vocabulary. `Aligned` is a strong claim: it requires identical time
/// bases *and* resolved phase/gain determinism.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignalAlignment {
    /// Frame-exact alignment established.
    Aligned,
    /// Approximate alignment — deltas are indicative, not exact.
    Approximate,
    /// Inputs could not be aligned.
    Unaligned,
    /// Alignment could not be determined.
    Unavailable,
}

/// Time domain a signal metric is expressed in. Metrics in different
/// domains must never be subtracted as if they shared a frame base —
/// a tempo/length change is not a PCM-frame delta.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignalDomain {
    /// Source/media frames of the underlying asset.
    SourceFrame,
    /// Musical ticks/beats of the project time base.
    Musical,
    /// Absolute session/project frames.
    ProjectFrame,
}

/// One measured/derived signal comparison row reported by a provider.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignalMetric {
    /// Metric identity (e.g. `"pcm.rms"`, `"spectral.flatness"`).
    pub name: String,
    /// Time domain the values are expressed in.
    pub domain: SignalDomain,
    /// Left/pinned-before value.
    pub before: f64,
    /// Right/pinned-after value.
    pub after: f64,
    /// `after - before` as reported by the provider.
    pub delta: f64,
    /// Unit label (e.g. `"linear"`, `"db"`).
    pub unit: String,
}

/// Whether a semantic [`ChangeSet`] came from the derived cache or was
/// computed for this evaluation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CacheStatus {
    /// Returned by the bounded [`ChangeSetCache`] without recompute.
    Hit,
    /// Computed by the diff engine for this evaluation.
    Miss,
}

/// Semantic (Configured) layer result — the VCS-02 [`ChangeSet`] over
/// the two pinned composed views. Always produced on success; the
/// engine reports uncertainty *inside* the change set
/// (`overall`/`completeness`/`unresolved`), never as a layer-state
/// collapse.
#[derive(Clone, Debug)]
pub struct SemanticLayerResult {
    /// Layer state — `Available` whenever the diff engine produced a
    /// result (partial/unknown coverage lives inside the change set).
    pub state: LayerState,
    /// The canonical change set (Arc-shared with the cache).
    pub changeset: Arc<ChangeSet>,
    /// Whether the result was a cache hit.
    pub cache: CacheStatus,
    /// Digest of the [`crate::diff::ComparisonKey`] the result is stored
    /// under — the reuse identity callers can pin or log.
    pub key: ContentDigest,
}

/// Signal (Measured) layer result. Headless this slice ships no real
/// renderer, so the layer is `Unavailable{NoProvider}` unless a
/// [`SignalComparisonProvider`] is supplied — and even then the
/// orchestrator's plugin-state/alignment gates apply.
#[derive(Clone, Debug)]
pub struct SignalLayerResult {
    /// Layer state.
    pub state: LayerState,
    /// Evidence provenance class — `None` when no evidence exists.
    pub origin: Option<EvidenceOrigin>,
    /// Alignment quality of the evidence (or `Unavailable`).
    pub alignment: SignalAlignment,
    /// Provider-reported metric rows (empty unless `Available`).
    pub metrics: Vec<SignalMetric>,
    /// Provider-declared provenance digest (render recipe / probe id).
    pub provenance: Option<ContentDigest>,
    /// `true` when the session's pinned context was stale at
    /// evaluation time — the evidence is pin-scoped, never current.
    pub stale: bool,
}

/// Perceptual (Inferred) layer result. Human audition/A-B evaluation
/// is interactive and outside the headless slice — always an explicit
/// honest gap, never fabricated.
#[derive(Clone, Debug)]
pub struct PerceptualLayerResult {
    /// Layer state — `Unavailable{RequiresAudition}` headlessly.
    pub state: LayerState,
    /// `true` when the session's pinned context was stale.
    pub stale: bool,
}

/// The layered comparison result for one session evaluation.
#[derive(Clone, Debug)]
pub struct ComparisonResult {
    /// The session evaluated (pins + fixed context + pinned views).
    pub session: ComparisonSession,
    /// Freshness of the pinned context at evaluation time.
    pub freshness: ComparisonFreshness,
    /// Semantic (Configured) layer.
    pub semantic: SemanticLayerResult,
    /// Signal (Measured) layer.
    pub signal: SignalLayerResult,
    /// Perceptual (Inferred) layer.
    pub perceptual: PerceptualLayerResult,
}

// ---------------------------------------------------------------------------
// signal provider seam
// ---------------------------------------------------------------------------

/// Inputs handed to a [`SignalComparisonProvider`]: the session (pins,
/// policies) and its two pinned composed views. Providers must produce
/// evidence scoped to *these* snapshots — evaluating anything else and
/// reporting it as this comparison's evidence is a contract violation.
pub struct SignalCompareRequest<'a> {
    /// The comparison session being evaluated.
    pub session: &'a ComparisonSession,
    /// Pinned left composed view.
    pub left: &'a ImmutableSnapshot,
    /// Pinned right composed view.
    pub right: &'a ImmutableSnapshot,
}

/// What a signal provider reports for one evaluation. `state`/`origin`/
/// `alignment` are provider-declared; the orchestrator re-validates
/// them against its own gates (plugin-state, finite metrics) before
/// publishing the layer result.
#[derive(Clone, Debug)]
pub struct SignalProviderReport {
    /// Provider-declared layer state.
    pub state: LayerState,
    /// Provider-declared evidence provenance.
    pub origin: EvidenceOrigin,
    /// Provider-declared alignment quality.
    pub alignment: SignalAlignment,
    /// Provider-reported metric rows.
    pub metrics: Vec<SignalMetric>,
    /// Provider-declared provenance digest.
    pub provenance: Option<ContentDigest>,
}

/// Seam for measured signal-layer evidence (PCM/render/probe paths).
/// Implementations live outside this slice; the orchestrator treats
/// every report as a claim to be gated, never as ground truth.
pub trait SignalComparisonProvider {
    /// Produces signal evidence for the pinned views, or an explicit
    /// gap state. Must not fabricate measurements.
    fn compare_signal(&self, request: &SignalCompareRequest<'_>) -> SignalProviderReport;
}

// ---------------------------------------------------------------------------
// orchestrator
// ---------------------------------------------------------------------------

/// Errors the orchestrator can fail with. Layer-level uncertainty is
/// reported inside [`ComparisonResult`]; these are contract failures —
/// unknown endpoints, CAS mismatches, corrupt content, capture/diff
/// engine failures.
#[derive(Clone, Debug)]
pub enum CompareError {
    /// The variant set does not exist in the live project.
    UnknownSet {
        /// The missing set.
        variant_set: VariantSetId,
    },
    /// The branch does not exist in the set.
    UnknownBranch {
        /// The missing branch.
        branch: BranchId,
    },
    /// Left/right endpoints belong to different variant sets — their
    /// scopes differ, so no shared-scope comparison exists.
    ScopeMismatch {
        /// Left endpoint's set.
        left: VariantSetId,
        /// Right endpoint's set.
        right: VariantSetId,
    },
    /// A caller-asserted branch revision no longer matches live content.
    StaleRevision {
        /// Which endpoint moved.
        side: CompareSide,
        /// Revision asserted by the caller.
        expected: BranchRevision,
        /// Revision observed now.
        actual: BranchRevision,
    },
    /// A caller-asserted scope-content digest no longer matches the
    /// recomputed canonical fingerprint.
    StaleScope {
        /// Which endpoint moved.
        side: CompareSide,
        /// Digest asserted by the caller.
        expected: ContentDigest,
        /// Digest recomputed now.
        actual: ContentDigest,
    },
    /// Session options failed validation (e.g. non-finite monitor gain).
    InvalidPolicy {
        /// What was invalid.
        detail: String,
    },
    /// Branch view resolution failed (unknown content, digest/schema/
    /// reference verification, or unmaterializable candidate content).
    Resolve(crate::render_input::RenderInputError),
    /// Snapshot capture of a composed view failed.
    Snapshot(SnapshotError),
    /// The semantic diff engine failed.
    Diff(DiffError),
    /// Shared-context object derivation failed.
    Project(Error),
}

impl std::fmt::Display for CompareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownSet { variant_set } => {
                write!(f, "unknown variant set {}", variant_set.to_tagged())
            }
            Self::UnknownBranch { branch } => {
                write!(f, "unknown branch {}", branch.to_tagged())
            }
            Self::ScopeMismatch { left, right } => write!(
                f,
                "compare endpoints span different variant sets: {} vs {}",
                left.to_tagged(),
                right.to_tagged()
            ),
            Self::StaleRevision {
                side,
                expected,
                actual,
            } => write!(
                f,
                "stale {} branch revision: pinned {:?}, actual {:?}",
                side.key(),
                expected,
                actual
            ),
            Self::StaleScope {
                side,
                expected,
                actual,
            } => write!(
                f,
                "stale {} scope content: pinned {expected}, actual {actual}",
                side.key()
            ),
            Self::InvalidPolicy { detail } => write!(f, "invalid session policy: {detail}"),
            Self::Resolve(err) => write!(f, "branch view resolution failed: {err}"),
            Self::Snapshot(err) => write!(f, "snapshot capture failed: {err}"),
            Self::Diff(err) => write!(f, "semantic diff failed: {err}"),
            Self::Project(err) => write!(f, "project scope derivation failed: {err}"),
        }
    }
}

impl std::error::Error for CompareError {}

impl From<crate::render_input::RenderInputError> for CompareError {
    fn from(err: crate::render_input::RenderInputError) -> Self {
        Self::Resolve(err)
    }
}

impl From<SnapshotError> for CompareError {
    fn from(err: SnapshotError) -> Self {
        Self::Snapshot(err)
    }
}

impl From<DiffError> for CompareError {
    fn from(err: DiffError) -> Self {
        Self::Diff(err)
    }
}

impl From<Error> for CompareError {
    fn from(err: Error) -> Self {
        Self::Project(err)
    }
}

/// The headless comparison orchestrator. Owns the bounded derived
/// [`ChangeSetCache`] (semantic layer reuse) and the
/// [`SnapshotResolver`] used to capture composed branch views. Holds
/// no project state — every method takes `&Project`/authority inputs
/// explicitly, so it can never mutate or go stale itself.
pub struct ComparisonOrchestrator {
    cache: ChangeSetCache,
    resolver: SnapshotResolver,
}

/// Canonical fingerprints of the shared-context dimensions the issue
/// pins explicitly, computed over shared (non-scope-owned) objects only.
struct ContextDigests {
    tempo: ContentDigest,
    time_mapping: ContentDigest,
    plugin: ContentDigest,
    source: ContentDigest,
    remainder: ContentDigest,
}

impl ComparisonOrchestrator {
    /// An orchestrator whose semantic cache holds at most
    /// `cache_capacity` change sets (0 disables caching).
    #[must_use]
    pub fn new(cache_capacity: usize) -> Self {
        Self {
            cache: ChangeSetCache::new(cache_capacity),
            resolver: SnapshotResolver::new(),
        }
    }

    /// Pins one branch endpoint into a [`BranchRevisionRef`] — the
    /// assertable CAS surface for [`Self::open_session`].
    ///
    /// # Errors
    /// [`CompareError::UnknownSet`]/[`CompareError::UnknownBranch`]/
    /// [`CompareError::Resolve`] on missing endpoints or unverifiable
    /// branch content.
    pub fn pin_branch(
        &self,
        live: &Project,
        store: &dyn BranchContentStore,
        variant_set: VariantSetId,
        branch: BranchId,
    ) -> Result<BranchRevisionRef, CompareError> {
        let set = live
            .variant_sets
            .get(&variant_set)
            .ok_or(CompareError::UnknownSet { variant_set })?;
        if !set.branches.contains_key(&branch) {
            return Err(CompareError::UnknownBranch { branch });
        }
        let resolved = resolve_side(live, store, variant_set, branch)?;
        let pin = resolved.branch.as_ref().ok_or_else(|| {
            CompareError::Resolve(crate::render_input::RenderInputError::Corrupt {
                detail: "branch resolution produced no render pin".into(),
            })
        })?;
        Ok(BranchRevisionRef {
            variant_set,
            branch,
            revision: pin.branch_revision,
            content_digest: pin.scope_digest,
        })
    }

    /// Opens a [`ComparisonSession`]: verifies both asserted endpoints,
    /// resolves both composed views, fixes the [`EvaluationContext`],
    /// and captures both views as immutable snapshots. Read-only — the
    /// project, branches, and store are untouched.
    ///
    /// # Errors
    /// [`CompareError::UnknownSet`]/[`CompareError::UnknownBranch`]/
    /// [`CompareError::ScopeMismatch`]/[`CompareError::StaleRevision`]/
    /// [`CompareError::StaleScope`]/[`CompareError::InvalidPolicy`]/
    /// [`CompareError::Resolve`]/[`CompareError::Snapshot`].
    #[allow(clippy::too_many_arguments)]
    pub fn open_session(
        &self,
        live: &Project,
        store: &dyn BranchContentStore,
        left: BranchRevisionRef,
        right: BranchRevisionRef,
        project_revision: ProjectRevision,
        options: &SessionOptions,
        blobs: Option<&dyn BlobProbe>,
        cancel: &SnapshotCancel,
    ) -> Result<ComparisonSession, CompareError> {
        if left.variant_set != right.variant_set {
            return Err(CompareError::ScopeMismatch {
                left: left.variant_set,
                right: right.variant_set,
            });
        }
        options.monitor.validate()?;
        let set = live
            .variant_sets
            .get(&left.variant_set)
            .ok_or(CompareError::UnknownSet {
                variant_set: left.variant_set,
            })?;
        for (side, reference) in [(CompareSide::Left, &left), (CompareSide::Right, &right)] {
            let branch =
                set.branches
                    .get(&reference.branch)
                    .ok_or(CompareError::UnknownBranch {
                        branch: reference.branch,
                    })?;
            let actual = branch.content.revision();
            if actual != reference.revision {
                return Err(CompareError::StaleRevision {
                    side,
                    expected: reference.revision,
                    actual,
                });
            }
        }

        let left_resolved = resolve_side(live, store, left.variant_set, left.branch)?;
        let right_resolved = resolve_side(live, store, right.variant_set, right.branch)?;
        let left_pin = verified_pin(&left_resolved, &left, CompareSide::Left)?;
        let right_pin = verified_pin(&right_resolved, &right, CompareSide::Right)?;

        let digests = context_digests(live, &set.scope)?;
        let pinned_context = EvaluationContext {
            project_revision,
            left: left_pin,
            right: right_pin,
            tempo_digest: digests.tempo,
            time_mapping_digest: digests.time_mapping,
            plugin_digest: digests.plugin,
            source_digest: digests.source,
            remainder_digest: digests.remainder,
        };

        let left_view = self.resolver.capture_project(
            &left_resolved.project,
            project_revision,
            cancel,
            blobs,
        )?;
        let right_view = self.resolver.capture_project(
            &right_resolved.project,
            project_revision,
            cancel,
            blobs,
        )?;

        Ok(ComparisonSession {
            left,
            right,
            pinned_context,
            alignment: options.alignment,
            monitor: options.monitor.clone(),
            domains: options.domains.clone(),
            budget: options.budget,
            left_view,
            right_view,
        })
    }

    /// Re-derives every pinned context component against current
    /// authority. Cheap relative to [`Self::evaluate`] — no diff is
    /// recomputed; resolution and fingerprinting only.
    ///
    /// # Errors
    /// [`CompareError::UnknownSet`]/[`CompareError::UnknownBranch`] when
    /// an endpoint vanished (deletion is not staleness — the session
    /// can no longer be verified at all), plus resolve/project errors.
    pub fn freshness(
        &self,
        session: &ComparisonSession,
        live: &Project,
        store: &dyn BranchContentStore,
        project_revision: ProjectRevision,
    ) -> Result<ComparisonFreshness, CompareError> {
        let ctx = &session.pinned_context;
        let set =
            live.variant_sets
                .get(&session.left.variant_set)
                .ok_or(CompareError::UnknownSet {
                    variant_set: session.left.variant_set,
                })?;

        let mut reasons = Vec::new();
        for (side, reference) in [
            (CompareSide::Left, &session.left),
            (CompareSide::Right, &session.right),
        ] {
            let branch =
                set.branches
                    .get(&reference.branch)
                    .ok_or(CompareError::UnknownBranch {
                        branch: reference.branch,
                    })?;
            let actual = branch.content.revision();
            if actual != reference.revision {
                reasons.push(StaleDimension::BranchRevision {
                    side,
                    pinned: reference.revision,
                    actual,
                });
            }
        }

        let mut shared_moved = false;
        for (side, reference, pin) in [
            (CompareSide::Left, &session.left, &ctx.left),
            (CompareSide::Right, &session.right, &ctx.right),
        ] {
            let resolved = resolve_side(live, store, reference.variant_set, reference.branch)?;
            let fresh = resolved.branch.as_ref().ok_or_else(|| {
                CompareError::Resolve(crate::render_input::RenderInputError::Corrupt {
                    detail: "branch resolution produced no render pin".into(),
                })
            })?;
            if fresh.scope_digest != pin.scope_digest {
                reasons.push(StaleDimension::ScopeContent { side });
            }
            if fresh.payload_digest != pin.payload_digest {
                reasons.push(StaleDimension::BranchPayload { side });
            }
            if fresh.shared_context_digest != pin.shared_context_digest {
                shared_moved = true;
            }
        }

        if shared_moved {
            let digests = context_digests(live, &set.scope)?;
            if digests.tempo != ctx.tempo_digest {
                reasons.push(StaleDimension::Tempo);
            }
            if digests.time_mapping != ctx.time_mapping_digest {
                reasons.push(StaleDimension::TimeMapping);
            }
            if digests.plugin != ctx.plugin_digest {
                reasons.push(StaleDimension::Plugin);
            }
            if digests.source != ctx.source_digest {
                reasons.push(StaleDimension::Source);
            }
            if digests.remainder != ctx.remainder_digest {
                reasons.push(StaleDimension::SharedContext);
            }
        }

        if project_revision != ctx.project_revision {
            reasons.push(StaleDimension::ProjectRevision {
                pinned: ctx.project_revision,
                actual: project_revision,
            });
        }

        if reasons.is_empty() {
            Ok(ComparisonFreshness::Current)
        } else {
            Ok(ComparisonFreshness::Stale { reasons })
        }
    }

    /// Evaluates the session into a layered [`ComparisonResult`]:
    /// freshness against current authority, the semantic
    /// [`ChangeSet`] through the bounded cache, signal evidence through
    /// the optional provider seam (gated), and the always-honest
    /// perceptual gap.
    ///
    /// # Errors
    /// Same contract-failure surface as [`Self::freshness`] plus
    /// [`CompareError::Diff`] on engine failure.
    pub fn evaluate(
        &mut self,
        session: &ComparisonSession,
        live: &Project,
        store: &dyn BranchContentStore,
        project_revision: ProjectRevision,
        signal: Option<&dyn SignalComparisonProvider>,
        cancel: &SnapshotCancel,
    ) -> Result<ComparisonResult, CompareError> {
        let freshness = self.freshness(session, live, store, project_revision)?;
        let stale = freshness.is_stale();

        let spec = ComparisonSpec {
            before: session.left_view.identity.clone(),
            after: session.right_view.identity.clone(),
            policy_version: DIFF_POLICY_VERSION,
            domains: session.domains.clone(),
            budget: session.budget,
        };
        let cache_hit = self
            .cache
            .contains(&session.left_view, &session.right_view, &spec);
        let changeset =
            self.cache
                .get_or_compare(&session.left_view, &session.right_view, &spec, cancel)?;
        let semantic = SemanticLayerResult {
            state: LayerState::Available,
            key: changeset.key.digest(),
            cache: if cache_hit {
                CacheStatus::Hit
            } else {
                CacheStatus::Miss
            },
            changeset: Arc::clone(&changeset),
        };

        let signal_layer = Self::signal_layer(session, signal, stale);
        let perceptual = PerceptualLayerResult {
            state: LayerState::Unavailable {
                reason: EvidenceReason::RequiresAudition,
            },
            stale,
        };

        Ok(ComparisonResult {
            session: session.clone(),
            freshness,
            semantic,
            signal: signal_layer,
            perceptual,
        })
    }

    /// Resolves the signal layer for one evaluation: no provider →
    /// `Unavailable{NoProvider}`; `Unaligned` policy → refused; provider
    /// report → gated on plugin-state knowledge and metric validity.
    fn signal_layer(
        session: &ComparisonSession,
        provider: Option<&dyn SignalComparisonProvider>,
        stale: bool,
    ) -> SignalLayerResult {
        let gap = |reason: EvidenceReason| SignalLayerResult {
            state: LayerState::Unavailable { reason },
            origin: None,
            alignment: SignalAlignment::Unavailable,
            metrics: Vec::new(),
            provenance: None,
            stale,
        };
        let Some(provider) = provider else {
            return gap(EvidenceReason::NoProvider);
        };
        if session.alignment == AlignmentPolicy::Unaligned {
            return gap(EvidenceReason::AlignmentPolicy);
        }

        let request = SignalCompareRequest {
            session,
            left: &session.left_view,
            right: &session.right_view,
        };
        let report = provider.compare_signal(&request);

        // Plugin-state gate: either view carrying non-`Known` opaque
        // state means headless cannot verify plugin state — no report
        // may claim `Available` measured evidence.
        let plugin_unknown = session
            .left_view
            .opaque_state
            .iter()
            .chain(session.right_view.opaque_state.iter())
            .any(|opaque| opaque.status != OpaqueStateStatus::Known);

        let gated = match report.state {
            LayerState::Available if plugin_unknown => LayerState::Unavailable {
                reason: EvidenceReason::PluginStateUnknown,
            },
            LayerState::Available
                if report.metrics.iter().any(|metric| {
                    !metric.before.is_finite()
                        || !metric.after.is_finite()
                        || !metric.delta.is_finite()
                }) =>
            {
                LayerState::Unknown {
                    reason: EvidenceReason::EvidenceQuality,
                }
            }
            other => other,
        };

        SignalLayerResult {
            state: gated,
            origin: Some(report.origin),
            alignment: report.alignment,
            metrics: report.metrics,
            provenance: report.provenance,
            stale,
        }
    }
}

// ---------------------------------------------------------------------------
// internals
// ---------------------------------------------------------------------------

/// Resolves one branch endpoint to its composed render view.
fn resolve_side(
    live: &Project,
    store: &dyn BranchContentStore,
    variant_set: VariantSetId,
    branch: BranchId,
) -> Result<ResolvedRenderProject, CompareError> {
    resolve_render_project(
        live,
        Some(RenderBranchTarget {
            variant_set,
            branch,
            store,
        }),
    )
    .map_err(CompareError::Resolve)
}

/// Extracts the render pin and verifies the caller-asserted content
/// digest against the recomputed canonical scope fingerprint.
fn verified_pin(
    resolved: &ResolvedRenderProject,
    reference: &BranchRevisionRef,
    side: CompareSide,
) -> Result<BranchRenderPin, CompareError> {
    let pin = resolved.branch.clone().ok_or_else(|| {
        CompareError::Resolve(crate::render_input::RenderInputError::Corrupt {
            detail: "branch resolution produced no render pin".into(),
        })
    })?;
    if pin.scope_digest != reference.content_digest {
        return Err(CompareError::StaleScope {
            side,
            expected: reference.content_digest,
            actual: pin.scope_digest,
        });
    }
    Ok(pin)
}

/// Partitions the shared (non-scope-owned) semantic objects into the
/// explicitly pinned context dimensions and fingerprints each.
fn context_digests(
    project: &Project,
    scope: &VariantScope,
) -> Result<ContextDigests, CompareError> {
    let all = all_semantic_objects(project);
    let owned = scope_objects(project, scope)?;
    let shared: BTreeSet<SemanticObjectRef> = all.difference(&owned).cloned().collect();
    let pick = |kinds: &[ObjectKind]| -> BTreeSet<SemanticObjectRef> {
        shared
            .iter()
            .filter(|object| kinds.contains(&object.kind))
            .cloned()
            .collect()
    };
    let fingerprint = |objects: &BTreeSet<SemanticObjectRef>| {
        canonical_fingerprint(project, objects).map_err(CompareError::Resolve)
    };
    let named_kinds = [
        ObjectKind::Project,
        ObjectKind::ClockEpoch,
        ObjectKind::PluginInstance,
        ObjectKind::Asset,
        ObjectKind::MediaSource,
        ObjectKind::MidiSource,
        ObjectKind::MidiNote,
        ObjectKind::SmfDocument,
    ];
    let remainder: BTreeSet<SemanticObjectRef> = shared
        .iter()
        .filter(|object| !named_kinds.contains(&object.kind))
        .cloned()
        .collect();
    Ok(ContextDigests {
        tempo: fingerprint(&pick(&[ObjectKind::Project]))?,
        time_mapping: fingerprint(&pick(&[ObjectKind::ClockEpoch]))?,
        plugin: fingerprint(&pick(&[ObjectKind::PluginInstance]))?,
        source: fingerprint(&pick(&[
            ObjectKind::Asset,
            ObjectKind::MediaSource,
            ObjectKind::MidiSource,
            ObjectKind::MidiNote,
            ObjectKind::SmfDocument,
        ]))?,
        remainder: fingerprint(&remainder)?,
    })
}
