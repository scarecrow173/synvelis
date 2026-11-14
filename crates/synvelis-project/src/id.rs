//! Compile-time-distinct stable identifiers.
//!
//! Each ID is a `u128` newtype with a domain kind encoded at the type level.
//! Serialized form is the tagged string `"<kind>-<32 lowercase hex>"` so that
//! a mismatched kind fails at load time and `u128` never travels through a
//! lossy JSON number.
//!
//! Cross-kind assignment is rejected at compile time:
//!
//! ```compile_fail
//! use synvelis_project::{LaneId, TrackId};
//! let track: TrackId = TrackId::new(1);
//! let lane: LaneId = track; // ERROR: mismatched types
//! ```
//!
//! ```compile_fail
//! use synvelis_project::{ClipId, SourceId};
//! let a = ClipId::new(7);
//! let b = SourceId::new(7);
//! let _same = a == b; // ERROR: no implementation of `ClipId == SourceId`
//! ```

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};

use crate::error::Error;

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident, $kind:literal) => {
        $(#[$meta])*
        #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u128);

        impl $name {
            /// The serialization kind tag, e.g. `"track"`.
            pub const KIND: &'static str = $kind;

            /// Wraps a raw `u128` value.
            #[must_use]
            pub const fn new(raw: u128) -> Self {
                Self(raw)
            }

            /// The raw `u128` value.
            #[must_use]
            pub const fn raw(self) -> u128 {
                self.0
            }

            /// The canonical `"<kind>-<32hex>"` form.
            #[must_use]
            pub fn to_tagged(self) -> String {
                format!("{}-{:032x}", Self::KIND, self.0)
            }

            /// Parses the canonical tagged form, rejecting a mismatched kind.
            ///
            /// # Errors
            /// [`Error::InvalidId`] when the tag or hex payload is malformed.
            pub fn from_tagged(text: &str) -> Result<Self, Error> {
                // Split at the LAST '-': kind tags like "clock-epoch" and
                // "recording-session" themselves contain hyphens.
                let (kind, hex) = text
                    .rsplit_once('-')
                    .ok_or_else(|| Error::InvalidId(text.to_owned()))?;
                if kind != Self::KIND || hex.len() != 32 {
                    return Err(Error::InvalidId(text.to_owned()));
                }
                let raw = u128::from_str_radix(hex, 16)
                    .map_err(|_| Error::InvalidId(text.to_owned()))?;
                Ok(Self(raw))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.to_tagged())
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(&self.to_tagged())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let text = String::deserialize(d)?;
                Self::from_tagged(&text).map_err(D::Error::custom)
            }
        }
    };
}

define_id!(
    /// Identifies a [`crate::Project`].
    ProjectId, "project"
);
define_id!(
    /// Identifies a [`crate::Track`].
    TrackId, "track"
);
define_id!(
    /// Identifies a [`crate::Lane`].
    LaneId, "lane"
);
define_id!(
    /// Identifies a [`crate::Clip`].
    ClipId, "clip"
);
define_id!(
    /// Identifies a readable source — a media [`crate::Source`] or a
    /// project-owned [`crate::MidiSource`]; the two share one ID space.
    SourceId, "source"
);
define_id!(
    /// Identifies a stored [`crate::Asset`].
    AssetId, "asset"
);
define_id!(
    /// Identifies a [`crate::Take`].
    TakeId, "take"
);
define_id!(
    /// Identifies a [`crate::RecordingSession`].
    RecordingSessionId, "recording-session"
);
define_id!(
    /// Identifies a [`crate::RecordingPass`].
    RecordingPassId, "recording-pass"
);
define_id!(
    /// Identifies a [`crate::CaptureStream`].
    CaptureStreamId, "capture-stream"
);
define_id!(
    /// Identifies a [`crate::CaptureSegment`].
    CaptureSegmentId, "capture-segment"
);
define_id!(
    /// Identifies a [`crate::ClockEpoch`].
    ClockEpochId, "clock-epoch"
);
define_id!(
    /// Identifies one safe local media import operation.
    ImportId, "import"
);
define_id!(
    /// Identifies a [`crate::TakeGroup`].
    TakeGroupId, "take-group"
);
define_id!(
    /// Identifies a [`crate::Comp`].
    CompId, "comp"
);
define_id!(
    /// Identifies a [`crate::CompRegion`].
    CompRegionId, "comp-region"
);
define_id!(
    /// Identifies a [`crate::CompTransition`].
    CompTransitionId, "comp-transition"
);
define_id!(
    /// Identifies a [`crate::control::ParameterTarget`].
    ParameterTargetId, "param"
);
define_id!(
    /// Identifies a [`crate::control::AutomationLane`].
    AutomationLaneId, "automation-lane"
);
define_id!(
    /// Identifies a [`crate::control::ModulationSource`].
    ModulationSourceId, "mod-source"
);
define_id!(
    /// Identifies a [`crate::control::ModulationAssignment`].
    ModulationAssignmentId, "mod-assign"
);
define_id!(
    /// Identifies a note entity (placeholder for clip-local note scope).
    NoteId, "note"
);
define_id!(
    /// Identifies a [`crate::routing::Bus`] — a logical route
    /// (group/return/master), not an audio device endpoint.
    BusId, "bus"
);
define_id!(
    /// Identifies a [`crate::routing::ChannelStrip`] — the owner of
    /// DSP/fader/pan/insert/send, distinct from `TrackId`.
    StripId, "strip"
);
define_id!(
    /// Identifies a [`crate::routing::Processor`].
    ProcessorId, "processor"
);
define_id!(
    /// Identifies a [`crate::routing::ProcessorChain`].
    ChainId, "chain"
);
define_id!(
    /// Identifies a [`crate::routing::SemanticPort`] — the project
    /// semantic port, a different type from the runtime graph `PortId`
    /// and from any plugin-local port identifier.
    SemanticPortId, "semantic-port"
);
define_id!(
    /// Identifies a [`crate::routing::RoutingConnection`] — the single
    /// authority for where a signal goes.
    RoutingConnectionId, "connection"
);
define_id!(
    /// Identifies a [`crate::routing::SendConfig`].
    SendId, "send"
);
define_id!(
    /// Identifies a [`crate::midi_comp::MidiPhraseRegion`] inside a
    /// plan-level MIDI comp selection (RCD-04 fixture semantics —
    /// not a persisted project entity).
    MidiCompRegionId, "midi-comp-region"
);
define_id!(
    /// Identifies a [`crate::variant::VariantSet`] — a group of
    /// production-candidate branches over one typed scope
    /// (synvelis#278).
    VariantSetId, "variant-set"
);
define_id!(
    /// Identifies a [`crate::variant::Branch`] — one versioned
    /// candidate inside a variant set (synvelis#278).
    BranchId, "branch"
);

/// Source of fresh stable-ID raw values. Injected into
/// [`crate::ProjectController`] so tests are fully deterministic.
/// The `Send` bound lets the single writer live behind a mutex on a
/// multithreaded host — GW-06's commit fence serializes callers from
/// different worker threads onto one `ProjectController`.
pub trait IdGenerator: Send {
    /// Returns the next raw value without exposing the next-but-one.
    fn next(&mut self) -> u128;
    /// Returns the value that the next call to [`IdGenerator::next`] yields.
    fn peek(&self) -> u128;
    /// Resets the generator so the next issued value is `seed`. Used when
    /// a snapshot (save/load/undo) restores `Project::id_watermark`.
    fn reseed(&mut self, seed: u128);
}

/// Deterministic monotonic counter generator (the default strategy).
///
/// The counter is seeded from `Project::id_watermark` so a save/load or an
/// undo restores the exact issuance position recorded in the snapshot.
#[derive(Debug, Clone)]
pub struct CounterIdGenerator {
    next: u128,
}

impl CounterIdGenerator {
    /// Creates a generator whose next issued value is `seed`.
    #[must_use]
    pub const fn new(seed: u128) -> Self {
        Self { next: seed }
    }
}

impl IdGenerator for CounterIdGenerator {
    fn next(&mut self) -> u128 {
        let value = self.next;
        self.next += 1;
        value
    }

    fn peek(&self) -> u128 {
        self.next
    }

    fn reseed(&mut self, seed: u128) {
        self.next = seed;
    }
}
