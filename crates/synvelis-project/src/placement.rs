//! Normalized clip placement/playback contract (#10 §3.2).
//!
//! [`ClipPlacement`] separates the saved timeline **anchor point**
//! (the authority that survives tempo-map edits) from the **extent**,
//! and [`ClipPlayback`] separates the source read window and mapping
//! from the placement. Anchors and extents each live in a mixed
//! musical/absolute domain; [`ClipPlacement::resolve`] re-derives the
//! concrete media span through the tempo map — the stored anchor is
//! re-evaluated, never the source-native frame window.

use serde::{Deserialize, Serialize};

use crate::{
    error::Error,
    range::{MediaSpan, MusicalRange, SourceRange},
    tempo::TickTimeMapping,
    time::{MediaDuration, MediaTimeValue, MusicalDuration, MusicalTick, SourceFrame},
};

/// The saved timeline anchor point — the placement authority.
///
/// After a tempo-map edit the stored anchor is re-evaluated through
/// the new map; the clip's source read window is never rewritten.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", deny_unknown_fields)]
pub enum TimeAnchor {
    /// Musical position in project PPQN ticks (negative = pre-roll).
    Musical(MusicalTick),
    /// Absolute media position in its declared time base.
    Absolute(MediaTimeValue),
}

/// Clip length measured from the anchor, in a mixed domain.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", deny_unknown_fields)]
pub enum ClipExtent {
    /// Musical length in project PPQN ticks.
    Musical(MusicalDuration),
    /// Absolute length in its declared time base.
    Absolute(MediaDuration),
}

impl ClipExtent {
    /// `true` when the extent has zero length.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Musical(d) => d.value() == 0,
            Self::Absolute(d) => d.value() == 0,
        }
    }
}

/// Where a clip sits on the timeline: a point anchor plus a length.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipPlacement {
    /// Saved position anchor (the authority for re-derivation).
    pub anchor: TimeAnchor,
    /// Length measured from the anchor.
    pub extent: ClipExtent,
}

/// Constant-ratio project→source mapping factor.
///
/// `source_frames = media_frames * numerator / denominator` (floor at
/// the final frame boundary). `1/1` is a natural 1:1 read; `2/1`
/// consumes the source twice as fast. Fields are private so the `> 0`
/// invariant cannot be bypassed by a literal or a crafted snapshot.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "StretchRatioRepr", into = "StretchRatioRepr")]
#[serde(deny_unknown_fields)]
pub struct StretchRatio {
    /// Ratio numerator (`> 0`).
    numerator: u32,
    /// Ratio denominator (`> 0`).
    denominator: u32,
}

/// Wire form for [`StretchRatio`]; validates on conversion.
#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StretchRatioRepr {
    numerator: u32,
    denominator: u32,
}

impl StretchRatio {
    /// Creates a ratio, rejecting zero components.
    ///
    /// # Errors
    /// [`Error::Validation`] when either component is zero.
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, Error> {
        if numerator == 0 || denominator == 0 {
            return Err(Error::Validation("stretch ratio component is zero".into()));
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    /// The ratio numerator (`> 0`).
    #[must_use]
    pub const fn numerator(&self) -> u32 {
        self.numerator
    }

    /// The ratio denominator (`> 0`).
    #[must_use]
    pub const fn denominator(&self) -> u32 {
        self.denominator
    }

    /// `floor(frames * num / den)` — exact `i128` math.
    ///
    /// # Errors
    /// [`Error::Overflow`] when the result exceeds `u64`.
    pub fn apply(&self, frames: u64) -> Result<u64, Error> {
        let n = i128::from(frames) * i128::from(self.numerator) / i128::from(self.denominator);
        u64::try_from(n).map_err(|_| Error::Overflow)
    }
}

impl From<StretchRatio> for StretchRatioRepr {
    fn from(r: StretchRatio) -> Self {
        Self {
            numerator: r.numerator,
            denominator: r.denominator,
        }
    }
}

impl TryFrom<StretchRatioRepr> for StretchRatio {
    type Error = Error;
    fn try_from(repr: StretchRatioRepr) -> Result<Self, Self::Error> {
        Self::new(repr.numerator, repr.denominator)
    }
}

/// How project time inside the extent maps onto the source window.
/// Non-linear marker warping is a later stage and not expressible here.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum PlaybackMapping {
    /// 1:1 natural read — media elapsed == source frames elapsed.
    Natural,
    /// Repeat the whole `source_region` window to fill the extent.
    Loop,
    /// Constant-ratio stretch: source consumed at `ratio × media` rate.
    SimpleStretch(StretchRatio),
}

/// The source side of a clip: which window it may read and how the
/// extent maps onto it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipPlayback {
    /// Source-native frame window the clip may consume
    /// (`start` = the former `source_offset` semantics).
    pub source_region: SourceRange,
    /// Project→source mapping across the extent.
    pub mapping: PlaybackMapping,
}

impl ClipPlayback {
    /// Source frames the mapping consumes for `media_frames` of
    /// project media time. `Loop` consumes the same logical span with
    /// wrap-around inside the window.
    ///
    /// # Errors
    /// [`Error::Overflow`] on stretched lengths beyond `u64`.
    pub fn consumed_len(&self, media_frames: u64) -> Result<u64, Error> {
        match self.mapping {
            PlaybackMapping::Natural | PlaybackMapping::Loop => Ok(media_frames),
            PlaybackMapping::SimpleStretch(ratio) => ratio.apply(media_frames),
        }
    }

    /// Checks that `media_frames` of playback stays inside
    /// `source_region` — `Loop` needs only a non-empty window.
    ///
    /// # Errors
    /// [`Error::Validation`] on an empty window or insufficient handle.
    pub fn validate_window(&self, media_frames: u64) -> Result<(), Error> {
        let window = u64::try_from(
            self.source_region
                .end()
                .frames_since(self.source_region.start())?,
        )
        .map_err(|_| Error::Overflow)?;
        if window == 0 {
            return Err(Error::Validation("empty source region window".into()));
        }
        if matches!(self.mapping, PlaybackMapping::Loop) {
            return Ok(());
        }
        let needed = self.consumed_len(media_frames)?;
        if needed > window {
            return Err(Error::Validation(format!(
                "playback consumes {needed} source frames, window is {window}"
            )));
        }
        Ok(())
    }
}

/// Clip placement+playback contract checks.
///
/// Only the *rate-independent* contract is enforced here — anchor
/// resolvability is tempo-map dependent (an anchor may re-derive
/// differently or fail after a tempo edit; consumers surface that as
/// `UnresolvableAnchor`, so it is not an op-time error).
///
/// `Absolute` extents carry their own time base: `frames_at(denominator)`
/// treats the declared base as the source's native frame rate — the
/// unit-consistent convention for absolute-extent clips (`1/rate`
/// bases check exactly). `Musical` extents are tempo-derived and
/// `Source` carries no native rate, so only the non-empty window is
/// checked; their handle fit is enforced at resolve time.
///
/// The same check runs at op validation and inside
/// `Project::validate_references`, so a snapshot that violates the
/// persisted contract is rejected at load instead of resurrecting an
/// invalid clip.
///
/// # Errors
/// [`Error::Validation`]/[`Error::Negative`]/[`Error::Overflow`].
pub(crate) fn validate_clip_contract(
    _project: &crate::project::Project,
    placement: &ClipPlacement,
    playback: &ClipPlayback,
) -> Result<(), Error> {
    if placement.extent.is_empty() {
        return Err(Error::Validation("clip extent is empty".into()));
    }
    match &placement.extent {
        ClipExtent::Absolute(dur) => {
            let media_frames =
                u64::try_from(dur.frames_at(dur.denominator())).map_err(|_| Error::Negative)?;
            playback.validate_window(media_frames)
        }
        ClipExtent::Musical(_) => playback.validate_window(0),
    }
}

/// Enforces the defined clip-contract subset over a
/// [`MidiSource`](crate::midi::MidiSource): a `Musical` extent and a
/// `Natural` mapping. Frame-domain `Absolute` extents compare media
/// frames against native ticks, and media-rate mappings (`Loop` /
/// `SimpleStretch`) have no defined meaning over a note sequence —
/// persisting either would record a contract no consumer can evaluate
/// (issue #52). Media-source clips are unconstrained here.
///
/// # Errors
/// [`Error::Validation`] when a MIDI-source clip leaves that subset.
pub(crate) fn validate_midi_clip_contract(
    project: &crate::project::Project,
    source_id: &crate::id::SourceId,
    placement: &ClipPlacement,
    playback: &ClipPlayback,
) -> Result<(), Error> {
    if !project.midi_sources.contains_key(source_id) {
        return Ok(());
    }
    if !matches!(placement.extent, ClipExtent::Musical(_)) {
        return Err(Error::Validation(format!(
            "clip on midi source {source_id} requires a Musical extent"
        )));
    }
    if !matches!(playback.mapping, PlaybackMapping::Natural) {
        return Err(Error::Validation(format!(
            "clip on midi source {source_id} requires a Natural playback mapping"
        )));
    }
    Ok(())
}

/// A placement+playback pair resolved to concrete frames under one
/// tempo-map revision.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ResolvedClip {
    /// Project media-time span `[start, end)` in `1/rate` base.
    pub media_span: MediaSpan,
    /// Source frame read at the clip start — tempo-map invariant.
    pub source_start: SourceFrame,
    /// Source frames the mapping consumes across `media_span`
    /// (before any engine-side clamping).
    pub source_len: u64,
    /// The mapping that produced `source_len`.
    pub mapping: PlaybackMapping,
}

impl ClipPlacement {
    /// The anchor's media position in frames at `rate`.
    fn anchor_media_frame(&self, map: Option<&TickTimeMapping>, rate: u32) -> Result<i64, Error> {
        match self.anchor {
            TimeAnchor::Musical(tick) => {
                let map = map
                    .ok_or_else(|| Error::Validation("musical anchor needs a tempo map".into()))?;
                Ok(map.tick_to_media(tick, 1, rate)?.value())
            }
            TimeAnchor::Absolute(media) => Ok(media.frames_at(rate)),
        }
    }

    /// The anchor's musical tick. An absolute anchor is re-derived
    /// through the tempo map once — the stored anchor stays authoritative.
    fn anchor_tick(&self, map: &TickTimeMapping) -> Result<MusicalTick, Error> {
        match self.anchor {
            TimeAnchor::Musical(tick) => Ok(tick),
            TimeAnchor::Absolute(media) => map.media_to_tick(media),
        }
    }

    /// Re-derives the concrete clip under `map` at `rate`.
    ///
    /// Musical conversions go through `map` exactly once per endpoint;
    /// `source_start` is tempo-map invariant. `map` may be `None` only
    /// when neither anchor nor extent touches the musical domain.
    ///
    /// # Errors
    /// [`Error::Validation`]/[`Error::Overflow`]/[`Error::Inverted`].
    pub fn resolve(
        &self,
        playback: &ClipPlayback,
        map: Option<&TickTimeMapping>,
        rate: u32,
    ) -> Result<ResolvedClip, Error> {
        let media_span = self.resolve_span(map, rate)?;
        let media_len = u64::try_from(
            media_span
                .end()
                .value()
                .checked_sub(media_span.start().value())
                .ok_or(Error::Overflow)?,
        )
        .map_err(|_| Error::Negative)?;
        playback.validate_window(media_len)?;
        Ok(ResolvedClip {
            media_span,
            source_start: playback.source_region.start(),
            source_len: playback.consumed_len(media_len)?,
            mapping: playback.mapping,
        })
    }

    /// Resolves only the project media span — no playback window
    /// check. `resolve` is `resolve_span` plus the window contract.
    ///
    /// # Errors
    /// [`Error::Validation`]/[`Error::Overflow`]/[`Error::Inverted`].
    pub fn resolve_span(
        &self,
        map: Option<&TickTimeMapping>,
        rate: u32,
    ) -> Result<MediaSpan, Error> {
        let start = self.anchor_media_frame(map, rate)?;
        let end = match self.extent {
            ClipExtent::Musical(dur) => {
                let map = map
                    .ok_or_else(|| Error::Validation("musical extent needs a tempo map".into()))?;
                let end_tick = self.anchor_tick(map)?.checked_add(dur.value())?;
                map.tick_to_media(end_tick, 1, rate)?.value()
            }
            ClipExtent::Absolute(dur) => start
                .checked_add(dur.frames_at(rate))
                .ok_or(Error::Overflow)?,
        };
        MediaSpan::try_new(
            MediaTimeValue::new(start, 1, rate)?,
            MediaTimeValue::new(end, 1, rate)?,
        )
    }

    /// Re-derives the musical `[start, end)` range under `map`.
    /// Absolute extents require `rate` to reach the media domain first.
    ///
    /// # Errors
    /// [`Error::Validation`]/[`Error::Overflow`]/[`Error::Inverted`].
    pub fn resolve_musical_range(
        &self,
        map: &TickTimeMapping,
        rate: u32,
    ) -> Result<MusicalRange, Error> {
        let start = self.anchor_tick(map)?;
        let end = match self.extent {
            ClipExtent::Musical(dur) => start.checked_add(dur.value())?,
            ClipExtent::Absolute(dur) => {
                let start_frame = self.anchor_media_frame(Some(map), rate)?;
                let end_media = MediaTimeValue::new(
                    start_frame
                        .checked_add(dur.frames_at(rate))
                        .ok_or(Error::Overflow)?,
                    1,
                    rate,
                )?;
                map.media_to_tick(end_media)?
            }
        };
        MusicalRange::try_new(start, end)
    }
}
