//! Canonical Render Recipe — types, canonical bytes and `RecipeKey`
//! (synvelis#290, RCP-01; contract: `docs/specs/render-checkpoint.md`).
//!
//! A [`RenderRecipeV1`] is the deterministic, versioned description of
//! *processing conditions* for one render boundary: which typed
//! semantic dependencies feed the output, under which temporal domain,
//! output contract and renderer profile. [`RecipeKey`] is
//! `SHA-256(domain_tag || versioned_canonical_bytes)` — a
//! processing-condition identity, **not** a PCM-equality proof
//! (reproducibility evidence is a separate concern, RCP-02+).
//!
//! Canonicalization rules (RCP-00 contract):
//!
//! - Entity identity is the typed [`SemanticObjectRef`] (`kind` +
//!   `"<kind>-<32hex>"` stable ID); maps/sets serialize in sorted
//!   order — never insertion order.
//! - Entity *values* come from
//!   [`synvelis_project::render_input::canonical_entity_value`]:
//!   display `name` fields are stripped (UI rename must not move a
//!   key) and non-finite floats are rejected at extraction, never
//!   normalized to `0`/`null`.
//! - Display names, view state, creation timestamps, render-trigger
//!   policy, artifact locations, project/branch *revisions* and
//!   unrelated subgraphs are excluded — revisions are authority pins,
//!   not acoustic identity.
//! - [`RecipeResolution`] is tri-state: `Resolved` carries the recipe
//!   and key; `Unknown` carries typed [`UnknownDependency`]s;
//!   `Unsupported` names a contract the slice cannot express. Neither
//!   non-`Resolved` state may ever be treated as an Exact cache hit.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use synvelis_project::{
    BranchId, ProjectId, SemanticPortId, SnapshotIdentity, VariantSetId, snapshot::ContentDigest,
};

use crate::dependency::DependencyClosure;

/// Wire/schema version of [`RenderRecipeV1`]. A version change yields
/// a different canonical byte stream — old keys are never overwritten
/// or reinterpreted (RCP-REC-06).
pub const RECIPE_SCHEMA_VERSION: u32 = 1;

/// Canonicalization rule version — bump when byte layout or ordering
/// rules change even if the schema version stays.
pub const CANONICALIZATION_VERSION: u32 = 1;

/// Audio-semantics contract version the recipe was resolved under.
pub const AUDIO_SEMANTICS_VERSION: u32 = 1;

/// Domain separator prepended to canonical bytes before hashing —
/// cross-protocol digest collisions are impossible by construction.
pub const RECIPE_HASH_DOMAIN: &[u8] = b"synvelis/render-recipe/v1\0";

/// Human/machine tag of the hash domain (recorded in recipes; the
/// hashed form is [`RECIPE_HASH_DOMAIN`]).
pub const RECIPE_DOMAIN_TAG: &str = "synvelis/render-recipe/v1";

/// Hash algorithm identifier recorded in every recipe.
pub const RECIPE_HASH_ALGORITHM: &str = "sha-256";

// ---------------------------------------------------------------------------
// frame intervals / boundary / temporal domain
// ---------------------------------------------------------------------------

/// Half-open frame interval `[start, end)` in render-output frames.
/// `end > start` is enforced at construction; an empty interval is not
/// a valid render coverage.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameInterval {
    /// First covered frame (inclusive).
    pub start: u64,
    /// One past the last covered frame (exclusive).
    pub end: u64,
}

impl FrameInterval {
    /// Creates an interval, rejecting `end <= start`.
    ///
    /// # Errors
    /// `Err` when the interval is empty or inverted.
    pub fn new(start: u64, end: u64) -> Result<Self, String> {
        if end <= start {
            return Err(format!("empty/inverted frame interval [{start}, {end})"));
        }
        Ok(Self { start, end })
    }

    /// `true` when `frame` lies inside `[start, end)`.
    #[must_use]
    pub fn contains(self, frame: u64) -> bool {
        self.start <= frame && frame < self.end
    }

    /// Union with `other` when the intervals overlap or touch.
    #[must_use]
    pub fn overlaps_or_touches(self, other: Self) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    /// The merged interval covering both (caller guarantees overlap).
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// The render boundary: which output ports the recipe covers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderBoundary {
    /// Output [`SemanticPortId`]s, canonically sorted, deduplicated.
    pub ports: Vec<SemanticPortId>,
}

impl RenderBoundary {
    /// Creates a boundary, sorting and deduplicating ports.
    #[must_use]
    pub fn new(mut ports: Vec<SemanticPortId>) -> Self {
        ports.sort();
        ports.dedup();
        Self { ports }
    }

    /// The master bus main output port, when the project carries the
    /// bootstrap routing model.
    #[must_use]
    pub fn master(project: &synvelis_project::Project) -> Option<Self> {
        let master = project.routing.master;
        let bus = project.routing.buses.get(&master)?;
        let strip = project.routing.strips.get(&bus.strip)?;
        let port = strip
            .port_ids
            .iter()
            .copied()
            .filter(|p| {
                project.routing.ports.get(p).is_some_and(|port| {
                    port.direction == synvelis_project::PortDirection::Output
                        && port.role == synvelis_project::PortRole::Main
                })
            })
            .min()?;
        Some(Self::new(vec![port]))
    }
}

/// External clock/transport dependency state. Anything not explicitly
/// pinned prevents an Exact-resolution claim (RCP-REC-02).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "clock", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ClockDependency {
    /// No external clock participates in this render.
    None,
    /// A pinned clock epoch identity (content digest).
    Pinned {
        /// Digest of the pinned clock-epoch record.
        epoch: ContentDigest,
    },
    /// A live device/clock — cannot be pinned, resolution must fail
    /// closed (`Unsupported`).
    Live,
    /// Referenced but not pinned — resolution must fail closed
    /// (`Unknown`).
    Unpinned,
}

/// Deterministic temporal domain of one render — every field is an
/// integer/enum so canonical bytes stay exact (no floats, no
/// non-finite risk).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporalDomain {
    /// Output sample rate in Hz.
    pub sample_rate_hz: u32,
    /// Output channel count.
    pub channels: u32,
    /// Output PCM layout.
    pub format: PcmFormat,
    /// Rendered frame interval `[start, end)`.
    pub interval: FrameInterval,
    /// Pre-roll frames consumed before `interval.start`.
    pub preroll_frames: u64,
    /// Warmup frames before signal is considered settled.
    pub warmup_frames: u64,
    /// Tail frames rendered after `interval.end`.
    pub tail_frames: u64,
    /// Declared DSP lookahead (plugin/PDC) in frames.
    pub lookahead_frames: u64,
    /// Reported plugin-delay-compensation total in frames.
    pub pdc_frames: u64,
    /// External clock/transport dependency.
    pub clock: ClockDependency,
    /// Loop region inside the render interval, when looping applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_region: Option<FrameInterval>,
    /// Declared seek boundaries (sorted, deduplicated).
    #[serde(default)]
    pub seek_boundaries: Vec<u64>,
}

/// PCM sample layout of the render output.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PcmFormat {
    /// 32-bit float.
    F32,
    /// 64-bit float.
    F64,
    /// 16-bit integer.
    I16,
    /// 24-bit integer.
    I24,
    /// 32-bit integer.
    I32,
}

// ---------------------------------------------------------------------------
// renderer profile / output contract / target
// ---------------------------------------------------------------------------

/// Renderer compute precision.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderPrecision {
    /// Single-precision float DSP.
    F32,
    /// Double-precision float DSP.
    F64,
    /// Fixed-point DSP.
    Fixed,
}

/// Denormal/flush-to-zero policy — denormal handling changes PCM bits.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DenormalMode {
    /// Hardware denormals preserved.
    Preserve,
    /// Flush denormals to zero.
    FlushToZero,
}

/// Renderer identity for recipe purposes. `compatibility_profile` is a
/// *digest* of the renderer compatibility contract — a version string
/// alone is never sufficient identity (RCP-00).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RendererProfile {
    /// Renderer implementation identity (e.g. `"ruviform-offline"`).
    pub renderer_id: String,
    /// Renderer version string (informational; identity is the
    /// compatibility profile digest).
    pub renderer_version: String,
    /// Digest of the compatibility profile — required, never optional.
    pub compatibility_profile: ContentDigest,
    /// DSP precision.
    pub precision: RenderPrecision,
    /// Backend/scheduler identity.
    pub backend: String,
    /// Deterministic seed (0 = none declared).
    pub seed: u64,
    /// Denormal policy.
    pub denormal_mode: DenormalMode,
}

/// One output port's contract inside [`OutputContract`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputPortContract {
    /// The boundary output port.
    pub port: SemanticPortId,
    /// Channel count at this port.
    pub channels: u32,
    /// PCM layout written for this port.
    pub format: PcmFormat,
}

/// What the render writes — part of recipe identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputContract {
    /// Per-port output declarations, sorted by port ID.
    pub ports: Vec<OutputPortContract>,
}

/// Acoustic identity of a resolved variant branch — the *only*
/// branch data inside the hashed recipe. RCP-REC-01 requires
/// different branches carrying identical acoustic graphs to produce
/// identical keys, so set/branch IDs, content revisions and payload
/// digests are authority pins recorded on [`ResolutionProvenance`],
/// never hashed here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeBranchPin {
    /// Scope-content fingerprint (CB-MODEL-RCP-01 reuse identity).
    pub scope_digest: ContentDigest,
    /// Digest of the resolved shared (non-scope) live context.
    pub shared_context_digest: ContentDigest,
}

/// What the recipe is *for*: boundary + resolved-input identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeTarget {
    /// Output boundary.
    pub boundary: RenderBoundary,
    /// Owning project identity.
    pub project_id: ProjectId,
    /// Project schema version the view was resolved at.
    pub project_schema_version: u32,
    /// Render-input adapter schema version.
    pub render_input_schema_version: u32,
    /// Branch pin, when the input resolved through a variant branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<RecipeBranchPin>,
}

// ---------------------------------------------------------------------------
// the recipe
// ---------------------------------------------------------------------------

/// Canonical Render Recipe v1 — processing-condition identity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenderRecipeV1 {
    /// [`RECIPE_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// [`CANONICALIZATION_VERSION`].
    pub canonicalization_version: u32,
    /// [`AUDIO_SEMANTICS_VERSION`].
    pub audio_semantics_version: u32,
    /// Hash algorithm identifier ([`RECIPE_HASH_ALGORITHM`]).
    pub hash_algorithm: String,
    /// Domain separator ([`RECIPE_HASH_DOMAIN`] as UTF-8 lossy-free tag).
    pub domain_separator: String,
    /// What is being rendered.
    pub target: RecipeTarget,
    /// Digest over the typed upstream dependency closure.
    pub inputs_digest: ContentDigest,
    /// Digest over processor/plugin-state/external-pin evidence.
    pub processor_digest: ContentDigest,
    /// Digest over the temporal domain.
    pub temporal_digest: ContentDigest,
    /// The temporal domain itself — carried so invalidation can
    /// expand dirty intervals without re-resolving (it is part of
    /// the canonical bytes the key covers).
    pub temporal: TemporalDomain,
    /// Digest over the renderer profile.
    pub renderer_digest: ContentDigest,
    /// Output contract.
    pub output_contract: OutputContract,
    /// The typed dependency closure the digests summarize — carried
    /// for inspection and temporal-invalidation mapping.
    pub closure: DependencyClosure,
}

impl RenderRecipeV1 {
    /// Deterministic canonical bytes: `serde_json` of the recipe with
    /// every collection already in canonical order. Serialization of
    /// [`serde_json::Value`] is `BTreeMap`-ordered; all `Vec`s are
    /// sorted at construction; no `f64` fields exist in the recipe
    /// (entity floats were finite-checked at extraction).
    ///
    /// # Errors
    /// `Err` on serialization failure (unreachable for this type —
    /// surfaced anyway so callers never unwrap a digest input).
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// `RecipeKey = SHA-256(domain_tag || versioned_canonical_bytes)`.
    ///
    /// # Errors
    /// `Err` when canonical serialization fails.
    pub fn key(&self) -> Result<RecipeKey, serde_json::Error> {
        let bytes = self.canonical_bytes()?;
        let mut hasher_input = Vec::with_capacity(RECIPE_HASH_DOMAIN.len() + bytes.len());
        hasher_input.extend_from_slice(RECIPE_HASH_DOMAIN);
        hasher_input.extend_from_slice(&bytes);
        Ok(RecipeKey(ContentDigest::sha256(&hasher_input)))
    }
}

/// Processing-condition identity — `SHA-256(domain || canonical)`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RecipeKey(pub ContentDigest);

impl RecipeKey {
    /// The wrapped content digest.
    #[must_use]
    pub const fn digest(&self) -> ContentDigest {
        self.0
    }
}

// ---------------------------------------------------------------------------
// resolution / validity
// ---------------------------------------------------------------------------

/// Why a render input cannot be resolved — the typed `Unknown` side.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "cause", rename_all = "kebab-case", deny_unknown_fields)]
pub enum UnknownCause {
    /// A referenced semantic entity does not exist.
    MissingEntity {
        /// Object locator (`kind|stable_id`).
        locator: String,
    },
    /// A port binding is unresolved (never auto-rebound).
    UnresolvedPort {
        /// Port stable ID.
        locator: String,
    },
    /// A processor's plugin binding is unresolved.
    UnresolvedProcessor {
        /// Processor stable ID.
        locator: String,
    },
    /// Plugin state evidence is absent — Exact cannot be claimed.
    PluginStateUnknown {
        /// Processor stable ID.
        locator: String,
    },
    /// Plugin state quality is not exact-eligible
    /// (`BestEffort`/`Unsupported`/failed capture).
    PluginStateDegraded {
        /// Processor stable ID.
        locator: String,
        /// Observed quality name.
        quality: String,
    },
    /// An external asset is not pinned to verified content.
    UnpinnedExternalAsset {
        /// Asset stable ID.
        locator: String,
    },
    /// An asset digest is not a strong content address and no verified
    /// pin was supplied.
    WeakAssetDigest {
        /// Asset stable ID.
        locator: String,
    },
    /// Source content is not verified (`Imported` state only).
    UnverifiedSource {
        /// Source stable ID.
        locator: String,
    },
    /// External clock/transport is referenced but unpinned.
    UnpinnedClock,
    /// A non-finite or otherwise invalid value was encountered.
    InvalidValue {
        /// Locator of the offending entity.
        locator: String,
    },
    /// An opaque/unsupported dependency kind was encountered.
    OpaqueDependency {
        /// Locator of the offending entity.
        locator: String,
    },
    /// The snapshot/input schema version is outside the supported
    /// ladder — old schemas stay typed `Unknown`, never coerced.
    UnsupportedSchema {
        /// Schema version found.
        found: u32,
    },
}

/// A typed unknown dependency — preserved, never inferred away.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnknownDependency {
    /// Why the dependency is unknown.
    pub cause: UnknownCause,
    /// Human/machine locator of the dependency.
    pub locator: String,
}

/// Why resolution is `Unsupported` — the contract cannot express it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case", deny_unknown_fields)]
pub enum UnsupportedReason {
    /// A live (unpinnable) external clock/device participates.
    LiveClock,
    /// The routing graph contains a cycle the contract rejects.
    CycleDetected,
    /// The routing model failed validation.
    InvalidRouting,
    /// An automation scope the contract cannot evaluate
    /// (`ProjectAbsolute` — declared but resolution unavailable).
    UnsupportedAutomationScope,
    /// The render-input read adapter rejected the input
    /// (corrupt pin, unsupported composition, invalid project).
    InputRejected {
        /// Adapter error detail.
        detail: String,
    },
    /// Canonicalization failed.
    Canonicalization {
        /// Detail.
        detail: String,
    },
}

/// Tri-state recipe resolution — `Unknown`/`Unsupported` are never
/// Exact cache hits.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RecipeResolution {
    /// Fully resolved recipe + key.
    Resolved {
        /// The canonical recipe (boxed — the payload dwarfs the
        /// `Unknown`/`Unsupported` variants).
        recipe: Box<RenderRecipeV1>,
        /// Its `RecipeKey`.
        key: RecipeKey,
        /// Resolution provenance (snapshot locator, revisions, pins) —
        /// recorded for audit, deliberately outside the hashed recipe.
        provenance: Box<ResolutionProvenance>,
    },
    /// Resolution failed closed with typed unknown dependencies.
    Unknown {
        /// Every unknown dependency found (sorted, deduplicated).
        dependencies: Vec<UnknownDependency>,
    },
    /// The contract cannot express this input.
    Unsupported {
        /// Why.
        reason: UnsupportedReason,
    },
}

/// Non-hashed provenance of one resolution — authority pins and
/// locators that must never move the recipe key.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolutionProvenance {
    /// Snapshot identity the input was resolved from, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<SnapshotIdentity>,
    /// Live project revision observed (pin only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_revision: Option<u64>,
    /// Variant set the branch belongs to (pin only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant_set: Option<VariantSetId>,
    /// Branch identity observed (pin only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<BranchId>,
    /// Branch content revision observed (pin only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_revision: Option<u64>,
    /// Captured payload digest (pin only — acoustic identity is
    /// `scope_digest` + `shared_context_digest` inside the recipe).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_digest: Option<ContentDigest>,
}

/// Validity of a stored recipe record against a fresh resolution.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Validity {
    /// Keys match and the stored record re-verified — Exact reuse may
    /// be claimed *subject to separate reproducibility evidence*.
    Match,
    /// Both sides resolved but keys differ — known-stale, safe to
    /// re-render.
    Stale,
    /// Either side is `Unknown`/`Unsupported`, or the stored record
    /// fails re-verification (schema drift, hash-collision injection,
    /// corrupt index) — never a valid hit.
    Unknown,
}

/// A stored recipe record — the lookup/migration unit of RCP-REC-06.
/// Verification re-derives the key from the stored canonical bytes and
/// re-checks every version field; a record that fails is `Corrupt`,
/// never silently valid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeRecord {
    /// The stored key claim.
    pub key: RecipeKey,
    /// Recipe schema version the bytes were written at.
    pub schema_version: u32,
    /// Canonicalization version.
    pub canonicalization_version: u32,
    /// Audio-semantics version.
    pub audio_semantics_version: u32,
    /// The exact canonical bytes the key was computed over.
    pub canonical_bytes: Vec<u8>,
}

/// Why a [`RecipeRecord`] fails re-verification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecipeRecordError {
    /// Schema/canonicalization/semantics version is not current —
    /// old records are never reinterpreted under new rules.
    VersionMismatch {
        /// Which version drifted.
        field: &'static str,
        /// Found value.
        found: u32,
    },
    /// Recomputed key differs from the stored claim — hash-collision
    /// injection or corruption; fail closed.
    KeyMismatch,
}

impl RecipeRecord {
    /// Re-verifies the record: versions must be current and
    /// `key == SHA-256(domain || canonical_bytes)`.
    ///
    /// # Errors
    /// [`RecipeRecordError`] on any mismatch.
    pub fn verify(&self) -> Result<(), RecipeRecordError> {
        if self.schema_version != RECIPE_SCHEMA_VERSION {
            return Err(RecipeRecordError::VersionMismatch {
                field: "schema_version",
                found: self.schema_version,
            });
        }
        if self.canonicalization_version != CANONICALIZATION_VERSION {
            return Err(RecipeRecordError::VersionMismatch {
                field: "canonicalization_version",
                found: self.canonicalization_version,
            });
        }
        if self.audio_semantics_version != AUDIO_SEMANTICS_VERSION {
            return Err(RecipeRecordError::VersionMismatch {
                field: "audio_semantics_version",
                found: self.audio_semantics_version,
            });
        }
        let mut input = Vec::with_capacity(RECIPE_HASH_DOMAIN.len() + self.canonical_bytes.len());
        input.extend_from_slice(RECIPE_HASH_DOMAIN);
        input.extend_from_slice(&self.canonical_bytes);
        let recomputed = RecipeKey(ContentDigest::sha256(&input));
        if recomputed != self.key {
            return Err(RecipeRecordError::KeyMismatch);
        }
        Ok(())
    }
}

/// Evaluates stored-record vs fresh-resolution validity. A record that
/// fails [`RecipeRecord::verify`] can never produce `Match` —
/// corruption and collision injection fail closed to `Unknown`.
#[must_use]
pub fn evaluate_validity(record: &RecipeRecord, current: &RecipeResolution) -> Validity {
    let RecipeResolution::Resolved { key, .. } = current else {
        return Validity::Unknown;
    };
    if record.verify().is_err() {
        return Validity::Unknown;
    }
    if *key == record.key {
        Validity::Match
    } else {
        Validity::Stale
    }
}

/// Convenience: build a [`RecipeRecord`] from a resolved recipe.
///
/// # Errors
/// `Err` when canonical serialization fails.
pub fn recipe_record(
    recipe: &RenderRecipeV1,
    key: RecipeKey,
) -> Result<RecipeRecord, serde_json::Error> {
    Ok(RecipeRecord {
        key,
        schema_version: recipe.schema_version,
        canonicalization_version: recipe.canonicalization_version,
        audio_semantics_version: recipe.audio_semantics_version,
        canonical_bytes: recipe.canonical_bytes()?,
    })
}

/// Lookup helper: find a stored record by key. Returns the record
/// *unverified* — callers must [`RecipeRecord::verify`] before any
/// validity claim (lookup itself never implies validity).
#[must_use]
pub fn lookup_record<'a>(
    records: &'a BTreeMap<RecipeKey, RecipeRecord>,
    key: &RecipeKey,
) -> Option<&'a RecipeRecord> {
    records.get(key)
}
