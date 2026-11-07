//! Observed metadata facts.
//!
//! [`AssetObservation<T>`] separates facts that were *observed* (with
//! origin and time) from *inferred* values (with method) and values
//! that are *unknown* or *stale*. Duration, BPM, key, permissions and
//! licensing are never fabricated: when the provider cannot observe
//! them they are `Unknown`.

use std::fmt;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::refs::ObjectIdentity;

/// An observation timestamp.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObservedAt(pub SystemTime);

impl ObservedAt {
    /// The current wall-clock time.
    #[must_use]
    pub fn now() -> Self {
        Self(SystemTime::now())
    }
}

impl fmt::Display for ObservedAt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(d) => write!(f, "{}.{:09}Z", d.as_secs(), d.subsec_nanos()),
            Err(_) => f.write_str("before-epoch"),
        }
    }
}

/// Where an observed fact came from.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObservationOrigin {
    /// The provider backend itself reported it.
    Provider,
    /// The local filesystem reported it.
    Filesystem,
    /// A provider-side index or catalog reported it.
    Index,
    /// A sidecar document or embedded metadata reported it.
    Sidecar,
    /// The project aggregate itself is the source of truth.
    ProjectModel,
}

/// How an inferred value was produced.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InferenceMethod(pub String);

/// Why a fact is unknown.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case", deny_unknown_fields)]
pub enum UnavailableReason {
    /// The provider does not offer this fact for the asset.
    NotProvided,
    /// No index exists yet that would carry it.
    NotIndexed,
    /// It is only known after fetching bytes (e.g. a digest that was
    /// never computed).
    RequiresFetch,
    /// The principal cannot read it.
    PermissionDenied,
    /// The provider/backend was unreachable at observation time.
    Offline,
    /// The fact does not apply to this entry kind.
    NotApplicable,
    /// The underlying object is missing.
    Missing,
    /// Any other cause — details in `detail`.
    Other {
        /// Free-form detail.
        detail: String,
    },
}

/// Why a formerly-observed value is stale.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case", deny_unknown_fields)]
pub enum StaleReason {
    /// The value comes from a cached snapshot, not a live read.
    CachedSnapshot,
    /// The provider is unreachable so the value could not be
    /// refreshed.
    ProviderUnreachable,
    /// Access was revoked after the value was observed.
    Revoked,
}

/// One metadata fact with provenance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "observation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AssetObservation<T> {
    /// Directly observed — value, its origin, and when.
    Observed {
        /// The observed value.
        value: T,
        /// Where the observation came from.
        origin: ObservationOrigin,
        /// When it was observed.
        at: ObservedAt,
    },
    /// Derived by an algorithm — value, the method, and when.
    Inferred {
        /// The inferred value.
        value: T,
        /// The producing method/algorithm.
        method: InferenceMethod,
        /// When it was inferred.
        at: ObservedAt,
    },
    /// Not known — with a typed reason.
    Unknown {
        /// Why it is unknown.
        reason: UnavailableReason,
    },
    /// Previously observed but no longer current.
    Stale {
        /// The last observed value.
        value: T,
        /// When it was observed.
        at: ObservedAt,
        /// Why it is stale.
        reason: StaleReason,
    },
}

impl<T> AssetObservation<T> {
    /// The carried value when there is one (`Observed`/`Inferred`/
    /// `Stale`), `None` for `Unknown`.
    #[must_use]
    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Observed { value, .. }
            | Self::Inferred { value, .. }
            | Self::Stale { value, .. } => Some(value),
            Self::Unknown { .. } => None,
        }
    }

    /// `true` for a fresh `Observed` fact.
    #[must_use]
    pub const fn is_observed(&self) -> bool {
        matches!(self, Self::Observed { .. })
    }

    /// A fact observed just now via `origin`.
    #[must_use]
    pub fn observed(value: T, origin: ObservationOrigin) -> Self {
        Self::observed_at(value, origin, ObservedAt::now())
    }

    /// A fact observed via `origin` at `at` — providers pass their
    /// injected clock (`ProviderContext::now`) so observations are
    /// deterministic under test.
    #[must_use]
    pub fn observed_at(value: T, origin: ObservationOrigin, at: ObservedAt) -> Self {
        Self::Observed { value, origin, at }
    }

    /// An unknown fact.
    #[must_use]
    pub const fn unknown(reason: UnavailableReason) -> Self {
        Self::Unknown { reason }
    }
}

/// What kind of object an entry is.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AssetKind {
    /// A regular file.
    File,
    /// A directory / browsable container.
    Directory,
    /// A symbolic link (target facts travel separately).
    Symlink,
    /// A project content-addressed blob.
    Blob,
    /// Anything provider-specific.
    Other {
        /// Provider-defined kind label.
        label: String,
    },
}

/// Whether the asset's bytes are reachable right now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Availability {
    /// Reachable.
    Available,
    /// The underlying object is missing (dangling link, evicted blob).
    Missing {
        /// Detail for diagnostics.
        detail: String,
    },
    /// The backend is offline; bytes are not fetchable now.
    Offline,
    /// Reachable in principle but blocked for a typed reason.
    Unavailable {
        /// Why it is unavailable.
        reason: UnavailableReason,
    },
}

/// Media-domain facts. Every field defaults to `Unknown` — nothing is
/// guessed for unprobed content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaFacts {
    /// Duration in milliseconds.
    pub duration_ms: AssetObservation<u64>,
    /// Sample rate in Hz.
    pub sample_rate: AssetObservation<u32>,
    /// Channel count.
    pub channels: AssetObservation<u32>,
    /// Container family (`"wav"`, `"mp4"`, ...).
    pub container: AssetObservation<String>,
    /// Codec identifier.
    pub codec: AssetObservation<String>,
    /// Tempo estimate — never fabricate; analysis providers may infer.
    pub bpm: AssetObservation<f64>,
    /// Musical key estimate — same rule as `bpm`.
    pub key: AssetObservation<String>,
}

impl MediaFacts {
    /// All fields `Unknown { NotProvided }` — the honest default for
    /// content that was not probed.
    #[must_use]
    pub fn all_unknown() -> Self {
        Self {
            duration_ms: AssetObservation::unknown(UnavailableReason::NotProvided),
            sample_rate: AssetObservation::unknown(UnavailableReason::NotProvided),
            channels: AssetObservation::unknown(UnavailableReason::NotProvided),
            container: AssetObservation::unknown(UnavailableReason::NotProvided),
            codec: AssetObservation::unknown(UnavailableReason::NotProvided),
            bpm: AssetObservation::unknown(UnavailableReason::NotProvided),
            key: AssetObservation::unknown(UnavailableReason::NotProvided),
        }
    }
}

/// The per-entry fact bundle carried by browse entries and `stat`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryFacts {
    /// Byte length.
    pub byte_len: AssetObservation<u64>,
    /// Modification time.
    pub modified: AssetObservation<ObservedAt>,
    /// Entry kind.
    pub kind: AssetObservation<AssetKind>,
    /// Strong digest hint when one was already computed.
    pub checksum_hint: AssetObservation<ObjectIdentity>,
    /// Media facts — `Unknown` until probed.
    pub media: MediaFacts,
}

impl EntryFacts {
    /// A fact bundle with everything unknown.
    #[must_use]
    pub fn all_unknown() -> Self {
        Self {
            byte_len: AssetObservation::unknown(UnavailableReason::NotProvided),
            modified: AssetObservation::unknown(UnavailableReason::NotProvided),
            kind: AssetObservation::unknown(UnavailableReason::NotProvided),
            checksum_hint: AssetObservation::unknown(UnavailableReason::NotProvided),
            media: MediaFacts::all_unknown(),
        }
    }
}

/// One provenance record observed on an asset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceRecord {
    /// What produced the record (`"source"`, `"import"`, ...).
    pub kind: String,
    /// The stable identifier (tagged `SourceId`/`ImportId`).
    pub id: String,
    /// The recorded state (`"verified"`, `"committed"`, ...) when any.
    pub state: Option<String>,
}

/// The full stat result for one locator.
#[derive(Clone, Debug, PartialEq)]
pub struct AssetStat {
    /// The asset this report describes.
    pub asset: crate::RemoteAssetRef,
    /// Whether its bytes are reachable now.
    pub availability: Availability,
    /// Observed entry facts.
    pub facts: EntryFacts,
    /// Provenance observed on the asset (project sources/imports).
    pub provenance: Vec<ProvenanceRecord>,
}
