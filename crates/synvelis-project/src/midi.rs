//! The MIDI source model (issue #52): owned note sequences in
//! source-native time, referenced by clips through the shared
//! [`SourceId`] space.
//!
//! A [`MidiSource`] is the note-data analogue of a media [`Source`]:
//! it is identified by a `SourceId` from the same stable-ID space and
//! a [`crate::Clip`] references it through `Clip::source_id` exactly
//! like an audio source. Unlike `Source` it carries its payload inside
//! the project — no `Asset`, no `content_digest`, and no
//! `Imported → … → Saved` verification chain (those record *media
//! bytes* lifecycle, which a project-owned note sequence does not
//! have).
//!
//! **Linked-source semantics.** Several clips may name the same
//! `SourceId`; all of them read this one note set, so editing it
//! changes the meaning of every linked clip. Note editing ops are
//! addressed by the source itself (`source_id` + `note_id`) — the
//! model does not route an edit "through" one of the clips, so no
//! clip-ownership or clip-permission premise is embedded here.
//! Making one linked clip independent is an explicit op
//! ([`crate::ProjectOp::DetachClipSource`]) that copies the source and
//! repoints the clip; nothing performs an implicit copy-on-write.
//!
//! **Time fidelity.** Note positions are [`MidiTick`] values in the
//! source's *native* grid and the source's [`MidiDivision`] records
//! that grid's resolution, so a 480-division source stays 480 and a
//! note never gets requantized to the project PPQN (issue #19's
//! native-division contract). Musical positions derive exactly:
//! `beat = start / division` as a rational — the mapping is explicit,
//! never an implicit rewrite of the stored tick.
//!
//! [`Source`]: crate::Source

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    error::Error,
    id::{NoteId, SourceId},
    range::MidiRange,
    time::MidiTick,
};

/// The native resolution of a [`MidiSource`]'s tick grid.
///
/// Serialized with a `kind` tag so non-metrical divisions (SMF SMPTE
/// time bases) can join the format without a schema break.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "MidiDivisionRepr", into = "MidiDivisionRepr")]
#[non_exhaustive]
pub enum MidiDivision {
    /// Ticks per quarter note — the metrical SMF division (`> 0`).
    /// Construct through [`MidiDivision::ppqn`] so the `> 0` invariant
    /// holds at construction.
    Ppqn(u32),
}

/// Wire form for [`MidiDivision`]; validates on conversion.
#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", deny_unknown_fields)]
enum MidiDivisionRepr {
    /// Metrical division.
    Ppqn(u32),
}

impl MidiDivision {
    /// Creates a metrical division, rejecting a zero resolution.
    ///
    /// # Errors
    /// [`Error::Validation`] when `ticks_per_quarter == 0`.
    pub fn ppqn(ticks_per_quarter: u32) -> Result<Self, Error> {
        if ticks_per_quarter == 0 {
            return Err(Error::Validation("midi division is zero".into()));
        }
        Ok(Self::Ppqn(ticks_per_quarter))
    }

    /// Ticks per quarter note (`> 0`) for a metrical division.
    #[must_use]
    pub const fn ticks_per_quarter(&self) -> u32 {
        match self {
            Self::Ppqn(v) => *v,
        }
    }

    /// Enforces the `> 0` invariant for directly constructed values
    /// (the serde repr already rejects zero on load).
    ///
    /// # Errors
    /// [`Error::Validation`] when the resolution is zero.
    pub fn validate(&self) -> Result<(), Error> {
        if self.ticks_per_quarter() == 0 {
            return Err(Error::Validation("midi division is zero".into()));
        }
        Ok(())
    }
}

impl From<MidiDivision> for MidiDivisionRepr {
    fn from(d: MidiDivision) -> Self {
        match d {
            MidiDivision::Ppqn(v) => Self::Ppqn(v),
        }
    }
}

impl TryFrom<MidiDivisionRepr> for MidiDivision {
    type Error = Error;
    fn try_from(repr: MidiDivisionRepr) -> Result<Self, Self::Error> {
        match repr {
            MidiDivisionRepr::Ppqn(v) => Self::ppqn(v),
        }
    }
}

/// A note inside a [`MidiSource`]: a stable-ID timed entity in
/// source-native ticks occupying `[start, start + len)`.
///
/// Distinct from the clip-local [`crate::Note`], which anchors
/// `ControlOwner::Note` evaluation domains in clip-relative beats —
/// a `MidiNote` is source payload shared by every clip that reads the
/// source. Both draw IDs from the same [`NoteId`] space, but note
/// addressing is always the `(SourceId, NoteId)` pair: editing ops
/// name the owning source explicitly.
///
/// `key`/`velocity` follow the MIDI wire domains (`0–127` / `1–127`;
/// velocity `0` encodes note-off on the wire, so a stored note rejects
/// it). Overlapping same-key notes are legal data — the minimal
/// contract imposes no uniqueness policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiNote {
    /// Stable ID (unique within the owning source).
    pub id: NoteId,
    /// Musical key identity (0–127).
    pub key: u8,
    /// Note-on velocity (1–127).
    pub velocity: u8,
    /// Source-native start tick (`>= 0`).
    pub start: MidiTick,
    /// Note length in source-native ticks (`> 0`).
    pub len: MidiTick,
}

impl MidiNote {
    /// Exclusive end tick (`start + len`).
    ///
    /// # Errors
    /// [`Error::Overflow`] when the sum exceeds `i64`.
    pub fn end(&self) -> Result<MidiTick, Error> {
        self.start.checked_add(self.len.value())
    }

    /// The half-open `[start, end)` native-tick span the note occupies.
    ///
    /// # Errors
    /// [`Error::Overflow`]/[`Error::Inverted`].
    pub fn range(&self) -> Result<MidiRange, Error> {
        MidiRange::try_new(self.start, self.end()?)
    }

    /// Enforces the note field contract independent of identity:
    /// `key <= 127`, `1 <= velocity <= 127`, `len > 0`, and a
    /// representable `start + len` end. Ops call this before an ID is
    /// drawn, so violations name the offending values, not an ID.
    ///
    /// # Errors
    /// [`Error::Validation`]/[`Error::Overflow`].
    pub fn validate_fields(
        key: u8,
        velocity: u8,
        start: MidiTick,
        len: MidiTick,
    ) -> Result<(), Error> {
        if key > 127 {
            return Err(Error::Validation(format!("midi note key {key} > 127")));
        }
        if velocity == 0 {
            return Err(Error::Validation(
                "midi note velocity is 0 (note-off, not a stored note)".into(),
            ));
        }
        if velocity > 127 {
            return Err(Error::Validation(format!(
                "midi note velocity {velocity} > 127"
            )));
        }
        if len.value() <= 0 {
            return Err(Error::Validation(
                "midi note has non-positive length".into(),
            ));
        }
        start.checked_add(len.value())?;
        Ok(())
    }

    /// Enforces the note's value contract — [`MidiNote::validate_fields`]
    /// plus this note's identity in the error context.
    ///
    /// # Errors
    /// [`Error::Validation`]/[`Error::Overflow`].
    pub fn validate(&self) -> Result<(), Error> {
        Self::validate_fields(self.key, self.velocity, self.start, self.len).map_err(|e| match e {
            Error::Validation(m) => Error::Validation(format!("midi note {}: {m}", self.id)),
            other => other,
        })
    }
}

/// An owned note sequence in source-native time — the MIDI member of
/// the [`SourceId`] space.
///
/// Clips reference the source by [`SourceId`] and read the half-open
/// `ClipPlayback::source_region` window of this tick grid; the frame
/// unit of that window is the source's native tick. The minimal
/// contract bounds notes only by their own fields — a source has no
/// declared length, so notes are never clipped against a source extent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MidiSource {
    /// Stable ID (same `SourceId` space as media [`crate::Source`]s).
    pub id: SourceId,
    /// Native tick resolution.
    pub division: MidiDivision,
    /// Notes by stable ID, sorted for canonical output.
    pub notes: BTreeMap<NoteId, MidiNote>,
}

impl MidiSource {
    /// Enforces the source's internal contract: a valid division, map
    /// keys matching `note.id`, and every note individually valid.
    ///
    /// # Errors
    /// [`Error::Validation`]/[`Error::WrongOwner`]/[`Error::Overflow`].
    pub fn validate(&self) -> Result<(), Error> {
        self.division.validate()?;
        for (key, note) in &self.notes {
            if *key != note.id {
                return Err(Error::WrongOwner(format!(
                    "midi source {}: note {} stored under foreign key {key}",
                    self.id, note.id
                )));
            }
            note.validate()?;
        }
        Ok(())
    }
}
