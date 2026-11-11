//! Canonical semantic-diff value types (VCS-02, synvelis#93).
//!
//! These types describe *one* deterministic [`ChangeSet`] produced by
//! comparing two VCS-01 [`crate::snapshot::ImmutableSnapshot`]s. They are
//! deliberately independent of Git text diffs, file-tree diffs, PCM/render
//! comparisons, and runtime graph changes.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::id::{AssetId, ProcessorId, ProjectId};
use crate::range::{MediaSpan, MusicalRange};
use crate::snapshot::ContentDigest;
use crate::snapshot::{ContentStatus, OpaqueStateStatus, SnapshotIdentity};

/// Semantic domain a change belongs to. Adapters declare one domain each;
/// [`ComparisonSpec::domains`] selects which participate in a comparison.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComparisonDomain {
    /// Project-level scalars: name, ppqn, tempo map events.
    Project,
    /// Tracks, lanes, clips, clip-local notes.
    Arrangement,
    /// Assets, media/MIDI sources, SMF documents.
    Media,
    /// Parameter targets, automation lanes, modulation sources/assignments.
    Control,
    /// Routing model: buses, strips, processors, chains, ports, connections, sends.
    Routing,
    /// Recording registry and authored recording intent: sessions,
    /// passes, streams, segments, takes, epochs, applied records,
    /// monitor declarations, punch marks.
    Recording,
    /// Comping registry: take groups, comps, regions, transitions.
    Comping,
}

/// The set of domains included in one comparison.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComparisonDomains(pub BTreeSet<ComparisonDomain>);

impl ComparisonDomains {
    /// Every domain the engine knows how to compare.
    #[must_use]
    pub fn all() -> Self {
        Self(BTreeSet::from([
            ComparisonDomain::Project,
            ComparisonDomain::Arrangement,
            ComparisonDomain::Media,
            ComparisonDomain::Control,
            ComparisonDomain::Routing,
            ComparisonDomain::Recording,
            ComparisonDomain::Comping,
        ]))
    }

    /// `true` when `domain` participates in the comparison.
    #[must_use]
    pub fn contains(&self, domain: ComparisonDomain) -> bool {
        self.0.contains(&domain)
    }

    /// `true` when no domain participates (comparison is vacuous → Unknown).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Resource bounds for one comparison. Exceeding any bound fails the whole
/// comparison with [`DiffError::BudgetExceeded`] — never a partial silent
/// result.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonBudget {
    /// Maximum entity records visited across all adapters.
    pub max_entities: usize,
    /// Maximum confirmed changes emitted.
    pub max_changes: usize,
    /// Maximum unresolved issues recorded.
    pub max_unresolved: usize,
}

impl Default for ComparisonBudget {
    fn default() -> Self {
        Self {
            max_entities: 1_000_000,
            max_changes: 10_000,
            max_unresolved: 10_000,
        }
    }
}

/// Which snapshot an observation belongs to.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotSide {
    /// Present only in the before snapshot.
    Before,
    /// Present only in the after snapshot.
    After,
    /// Observed identically on both sides (deduplicated issues).
    Both,
}

/// The semantic kind of a compared object.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObjectKind {
    /// Project-level singleton (name/ppqn/tempo).
    Project,
    /// A track.
    Track,
    /// A lane inside a track.
    Lane,
    /// A clip on a lane.
    Clip,
    /// A clip-local note.
    Note,
    /// A media asset record.
    Asset,
    /// A media source.
    MediaSource,
    /// A MIDI source.
    MidiSource,
    /// A note inside a MIDI source.
    MidiNote,
    /// An SMF provenance document keyed by its MIDI source.
    SmfDocument,
    /// A clock epoch.
    ClockEpoch,
    /// A recording session.
    RecordingSession,
    /// A recording pass.
    RecordingPass,
    /// A capture stream.
    CaptureStream,
    /// A capture segment.
    CaptureSegment,
    /// A take.
    Take,
    /// An applied recording-commit registry entry (keyed by session).
    AppliedCommit,
    /// An applied import registry entry (keyed by import).
    AppliedImport,
    /// A comping take group.
    TakeGroup,
    /// A comp.
    Comp,
    /// A comp region.
    CompRegion,
    /// A comp transition.
    CompTransition,
    /// A parameter target.
    ParameterTarget,
    /// An automation lane.
    AutomationLane,
    /// A modulation source.
    ModulationSource,
    /// A modulation assignment.
    ModulationAssignment,
    /// Routing-model singleton fields (master pointer).
    RoutingModel,
    /// A routing bus.
    Bus,
    /// A channel strip.
    Strip,
    /// A plugin processor.
    Processor,
    /// A processor chain.
    ProcessorChain,
    /// A semantic port.
    SemanticPort,
    /// A routing connection (edge).
    RoutingConnection,
    /// A send configuration.
    SendConfig,
    /// A track-scoped authored monitor declaration, keyed by its
    /// `TrackId` (synvelis#117, schema v12). Appended last so every
    /// existing kind keeps its `as u8` discriminant — change IDs and
    /// issue dedup keys bake it in.
    TrackMonitorDecl,
    /// The project-wide punch-marks declaration (synvelis#117,
    /// schema v12) — a singleton object identified by the project tag.
    PunchMarks,
}

impl ObjectKind {
    /// The domain this object kind belongs to.
    #[must_use]
    pub const fn domain(self) -> ComparisonDomain {
        match self {
            Self::Project => ComparisonDomain::Project,
            Self::Track | Self::Lane | Self::Clip | Self::Note => ComparisonDomain::Arrangement,
            Self::Asset
            | Self::MediaSource
            | Self::MidiSource
            | Self::MidiNote
            | Self::SmfDocument => ComparisonDomain::Media,
            Self::ParameterTarget
            | Self::AutomationLane
            | Self::ModulationSource
            | Self::ModulationAssignment => ComparisonDomain::Control,
            Self::RoutingModel
            | Self::Bus
            | Self::Strip
            | Self::Processor
            | Self::ProcessorChain
            | Self::SemanticPort
            | Self::RoutingConnection
            | Self::SendConfig => ComparisonDomain::Routing,
            Self::ClockEpoch
            | Self::RecordingSession
            | Self::RecordingPass
            | Self::CaptureStream
            | Self::CaptureSegment
            | Self::Take
            | Self::AppliedCommit
            | Self::AppliedImport
            | Self::TrackMonitorDecl
            | Self::PunchMarks => ComparisonDomain::Recording,
            Self::TakeGroup | Self::Comp | Self::CompRegion | Self::CompTransition => {
                ComparisonDomain::Comping
            }
        }
    }
}

/// Optional creation/fork lineage of an object. No persisted lineage
/// schema exists yet (synvelis#7 defers it to a Project schema
/// extension), so the engine always emits `None` — it never fabricates
/// lineage for old snapshots.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LineageRef {
    /// Opaque lineage record reference (e.g. restore/fork record ID).
    pub record: String,
}

/// Stable reference to one compared object.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SemanticObjectRef {
    /// Project the object belongs to.
    pub project_id: ProjectId,
    /// Semantic object kind.
    pub kind: ObjectKind,
    /// Canonical `"<kind>-<32hex>"` stable ID (or a namespaced key for
    /// registry entries that reuse another entity's ID space).
    pub stable_id: String,
    /// Creation/fork lineage — always `None` until a lineage schema exists.
    pub lineage: Option<LineageRef>,
}

/// Reference to an entity's canonical serialized value. The digest covers
/// the entity's canonical JSON form — a content identity, not a VCS blob.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SemanticValueRef {
    /// SHA-256 of the entity's canonical JSON serialization.
    pub digest: ContentDigest,
}

/// Affected time range of a change, when it is cleanly expressible in a
/// single stored domain (musical ticks or absolute media span).
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "domain", content = "range", rename_all = "kebab-case")]
pub enum SemanticTimeRange {
    /// Musical `[start, end)` in project PPQN ticks.
    Musical(MusicalRange),
    /// Absolute media `[start, end)` span.
    Media(MediaSpan),
}

/// One field-level difference inside a [`SemanticChange`]'s evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemanticFieldDiff {
    /// Dotted field path (e.g. `placement.anchor`, `name`).
    pub field: String,
    /// Canonical JSON value before the change.
    pub before: serde_json::Value,
    /// Canonical JSON value after the change.
    pub after: serde_json::Value,
}

/// Evidence attached to a [`SemanticChange`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ChangeEvidence {
    /// Aggregated field-level differences (empty for Added/Removed).
    pub fields: Vec<SemanticFieldDiff>,
}

/// Deterministic change identifier, bound to the comparison key, object,
/// kind, and field set via SHA-256.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChangeId(pub String);

/// The semantic classification of one change.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeKind {
    /// Object exists only after.
    Added,
    /// Object exists only before.
    Removed,
    /// Scalar/structural attribute change.
    Modified,
    /// Position/membership move (clip anchor or lane, lane order context).
    Moved,
    /// Meaningful order change of an ordered collection.
    Reordered,
    /// Edge endpoint rewired on the same connection identity.
    Reconnected,
    /// Content/source identity swapped (clip source, asset digest).
    Replaced,
}

/// One canonical semantic change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SemanticChange {
    /// Deterministic change ID.
    pub id: ChangeId,
    /// The object the change applies to.
    pub object: SemanticObjectRef,
    /// Semantic change kind.
    pub kind: ChangeKind,
    /// Entity value reference before (absent for `Added`).
    pub before: Option<SemanticValueRef>,
    /// Entity value reference after (absent for `Removed`).
    pub after: Option<SemanticValueRef>,
    /// Affected range when cleanly expressible.
    pub affected_range: Option<SemanticTimeRange>,
    /// Field-level evidence.
    pub evidence: ChangeEvidence,
    /// Causal dependencies — empty in this slice (snapshots carry no
    /// operation provenance).
    pub dependencies: Vec<ChangeId>,
}

/// Why an object's identity could not be confirmed.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AmbiguityReason {
    /// The two snapshots belong to different projects.
    CrossProject,
    /// The same raw ID is claimed by different object kinds.
    CrossKindId,
    /// An owner/lineage reference field differs on the same stable ID.
    OwnerConflict,
}

/// An unresolved comparison issue — the `Unknown` side of the tri-state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "issue", rename_all = "kebab-case")]
pub enum ComparisonIssue {
    /// The source schema version is outside the supported ladder.
    UnsupportedSchema {
        /// Schema version found in the snapshot identity.
        found: u32,
        /// Which snapshot carried it.
        side: SnapshotSide,
    },
    /// A referenced asset's content is affirmatively missing.
    MissingAsset {
        /// The missing asset.
        asset_id: AssetId,
        /// Which snapshot reported it.
        side: SnapshotSide,
        /// Probe status.
        status: ContentStatus,
    },
    /// A processor's opaque state could not be verified.
    MissingOpaqueState {
        /// The processor.
        processor_id: ProcessorId,
        /// Which snapshot reported it.
        side: SnapshotSide,
        /// Verification status.
        status: OpaqueStateStatus,
    },
    /// Object identity could not be confirmed — never coerced to Equal,
    /// zero-change, or a rename.
    AmbiguousIdentity {
        /// The ambiguous object reference.
        object: SemanticObjectRef,
        /// Why identity is ambiguous.
        reason: AmbiguityReason,
    },
    /// A domain (or part of it) is a migration projection rather than
    /// authored state, so it cannot be compared as authored content.
    PartialProjection {
        /// The affected domain.
        domain: ComparisonDomain,
        /// Machine-readable detail (e.g. `pre-v6-routing`).
        detail: String,
    },
    /// Referenced source content presence could not be determined.
    UnresolvedSource {
        /// The unresolved asset.
        asset_id: AssetId,
        /// Which snapshot reported it.
        side: SnapshotSide,
    },
    /// A persisted reference does not resolve (defensive; snapshots are
    /// validated at resolve time).
    InvalidReference {
        /// Human-readable detail.
        detail: String,
    },
}

/// Whole-comparison equality — the tri-state result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "kebab-case")]
pub enum ComparisonEquality {
    /// Zero confirmed changes and zero unresolved issues on complete
    /// snapshots only.
    Equal,
    /// At least one confirmed change (unresolved issues may coexist;
    /// `completeness` then reports `Partial`).
    Different,
    /// No confirmed changes but at least one unresolved issue — equality
    /// cannot be claimed.
    Unknown(ComparisonIssue),
}

/// Whether the comparison covered every relevant region.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComparisonCompleteness {
    /// No unresolved issues.
    Complete,
    /// At least one unresolved issue exists.
    Partial,
}

/// Confirmed-change counts only — unresolved issues are counted
/// separately and never folded into change counts.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSetSummary {
    /// `Added` count.
    pub added: usize,
    /// `Removed` count.
    pub removed: usize,
    /// `Modified` count.
    pub modified: usize,
    /// `Moved` count.
    pub moved: usize,
    /// `Reordered` count.
    pub reordered: usize,
    /// `Reconnected` count.
    pub reconnected: usize,
    /// `Replaced` count.
    pub replaced: usize,
    /// Total confirmed changes.
    pub total: usize,
    /// Unresolved issue count.
    pub unresolved: usize,
}

/// Deterministic identity of one comparison request. Bound to both
/// snapshot content digests, source schema versions, projection
/// versions, policy version, and included domains — the key under which
/// a [`ChangeSet`] (or its cache entry) is stored and reused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonKey {
    /// Before snapshot content digest.
    pub before_digest: ContentDigest,
    /// After snapshot content digest.
    pub after_digest: ContentDigest,
    /// Before source schema version (pre-migration).
    pub before_schema_version: u32,
    /// After source schema version.
    pub after_schema_version: u32,
    /// Before projection schema version.
    pub before_projection_version: u32,
    /// After projection schema version.
    pub after_projection_version: u32,
    /// Semantic-diff policy version.
    pub policy_version: u32,
    /// Included domains (sorted).
    pub domains: Vec<ComparisonDomain>,
}

impl ComparisonKey {
    /// Builds the key from the two snapshot identities and the spec.
    #[must_use]
    pub fn new(before: &SnapshotIdentity, after: &SnapshotIdentity, spec: &ComparisonSpec) -> Self {
        Self {
            before_digest: before.digest,
            after_digest: after.digest,
            before_schema_version: before.schema_version,
            after_schema_version: after.schema_version,
            before_projection_version: before.projection_version,
            after_projection_version: after.projection_version,
            policy_version: spec.policy_version,
            domains: spec.domains.0.iter().copied().collect(),
        }
    }

    /// Content digest of the canonical key serialization — the cache/index
    /// identity for a [`ChangeSet`].
    #[must_use]
    pub fn digest(&self) -> ContentDigest {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        ContentDigest::sha256(&bytes)
    }
}

/// One comparison request: endpoints, policy, domains, and budget.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonSpec {
    /// Declared before endpoint identity (must be the before snapshot's).
    pub before: SnapshotIdentity,
    /// Declared after endpoint identity (must be the after snapshot's).
    pub after: SnapshotIdentity,
    /// Semantic-diff policy version ([`DIFF_POLICY_VERSION`]).
    pub policy_version: u32,
    /// Domains participating in the comparison.
    pub domains: ComparisonDomains,
    /// Resource bounds.
    pub budget: ComparisonBudget,
}

/// Current semantic-diff policy version.
///
/// v2 (synvelis#117): monitor declarations
/// (`TrackMonitorDecl`/`PunchMarks`) joined the compared entity set —
/// a monitor-only difference that reported `Equal` under v1 now
/// reports `Different`. The version lives inside `ComparisonKey`, so
/// cached/persisted results computed under an older policy can never
/// collide with new-policy keys.
pub const DIFF_POLICY_VERSION: u32 = 2;

impl ComparisonSpec {
    /// A spec comparing `before` → `after` over all domains.
    #[must_use]
    pub fn new(before: SnapshotIdentity, after: SnapshotIdentity) -> Self {
        Self {
            before,
            after,
            policy_version: DIFF_POLICY_VERSION,
            domains: ComparisonDomains::all(),
            budget: ComparisonBudget::default(),
        }
    }
}

/// The canonical, deterministic result of one semantic comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChangeSet {
    /// The comparison specification that produced this result.
    pub comparison: ComparisonSpec,
    /// Deterministic comparison key.
    pub key: ComparisonKey,
    /// Confirmed changes in deterministic order.
    pub changes: Vec<SemanticChange>,
    /// Unresolved issues in deterministic order.
    pub unresolved: Vec<ComparisonIssue>,
    /// Tri-state equality.
    pub overall: ComparisonEquality,
    /// Coverage completeness.
    pub completeness: ComparisonCompleteness,
    /// Confirmed-change summary.
    pub summary: ChangeSetSummary,
}

/// Errors the diff engine can fail with. Everything else is reported
/// inside the [`ChangeSet`] as unresolved issues — the engine never
/// fabricates certainty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffError {
    /// Cooperative cancellation via [`crate::snapshot::SnapshotCancel`].
    Cancelled,
    /// A [`ComparisonBudget`] bound was exceeded.
    BudgetExceeded {
        /// Which bound.
        limit: &'static str,
        /// Observed count.
        actual: usize,
    },
    /// Internal serialization failure (canonical JSON could not be
    /// produced for a compared value).
    Internal(String),
}

impl std::fmt::Display for DiffError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("comparison cancelled"),
            Self::BudgetExceeded { limit, actual } => {
                write!(f, "comparison budget exceeded: {limit} ({actual})")
            }
            Self::Internal(detail) => write!(f, "diff internal error: {detail}"),
        }
    }
}

impl std::error::Error for DiffError {}
