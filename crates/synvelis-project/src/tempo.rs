//! Tempo map persistence values and tick↔media mapping.
//!
//! Ruviform `TempoMap` has private internals and no serde impl, so the
//! project persists [`TempoMapData`]: the event lists plus a `revision`
//! counter (the snapshot identity a [`TempoMapRef`] points at). At runtime
//! the map is rebuilt via `TempoMapBuilder` into a [`TickTimeMapping`].
//!
//! Musical positions are persisted as `tick + ppqn` context; the
//! project PPQN is configurable in `1..=32767` (issue #125 / TIM-01)
//! and changes go through `ProjectOp::SetProjectPpqn`, which
//! re-projects every stored tick through exact rational beats —
//! see [`crate::ppqn`].

use num_traits::ToPrimitive;
use ruviform_media_core::{
    BeatPosition, MediaTime, Ppqn, TempoEvent, TempoMap, TempoMapBuilder, Tick, TickPosition,
    TimeBase, TimeSignatureEvent,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::Error,
    time::{MediaTimeValue, MusicalTick},
};

/// Seconds-domain resolution of the transport position contract: the
/// view-facing seconds value is an integer count of microseconds.
const SECONDS_DEN: u32 = 1_000_000;

fn seconds_base() -> Result<TimeBase, Error> {
    TimeBase::new(1, SECONDS_DEN).map_err(|e| Error::Validation(format!("time base: {e:?}")))
}

/// Project PPQN setting — the tick-grid context every `MusicalTick`
/// in this project counts against (default 960; configurable
/// `1..=32767` via `ProjectOp::SetProjectPpqn`, issue #125).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectPpqn(pub u32);

impl Default for ProjectPpqn {
    fn default() -> Self {
        Self(960)
    }
}

/// Wire form of a tempo change (milli-BPM fixed point, matching Ruviform's
/// `TempoEvent::bpm_milli`: 120.000 BPM → `120_000`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TempoEventDto {
    /// Tick at which the tempo takes effect.
    pub tick: i64,
    /// Tempo in milli-BPM (valid range `1..=999_999_999`).
    pub bpm_milli: u32,
}

/// Wire form of a time-signature change.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignatureDto {
    /// Tick at which the signature takes effect.
    pub tick: i64,
    /// Beats per bar (>= 1).
    pub numerator: u8,
    /// Beat unit as a power-of-two denominator.
    pub denominator: u8,
}

/// Persisted tempo map snapshot: event lists plus a monotonic `revision`
/// bumped on every tempo-map mutation. [`TempoMapRef`] values refer to a
/// revision of this snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TempoMapData {
    /// Snapshot revision; `0` is the default map.
    pub revision: u64,
    /// Project PPQN carried with the snapshot.
    pub ppqn: u32,
    /// Tempo change events (must be non-empty for a valid map).
    pub tempo_events: Vec<TempoEventDto>,
    /// Time-signature change events.
    pub signature_events: Vec<SignatureDto>,
}

impl Default for TempoMapData {
    /// 120.000 BPM, 4/4 from tick 0, PPQN 960.
    fn default() -> Self {
        Self {
            revision: 0,
            ppqn: 960,
            tempo_events: vec![TempoEventDto {
                tick: 0,
                bpm_milli: 120_000,
            }],
            signature_events: vec![SignatureDto {
                tick: 0,
                numerator: 4,
                denominator: 4,
            }],
        }
    }
}

impl TempoMapData {
    /// Compiles the persisted events into a Ruviform `TempoMap`.
    ///
    /// # Errors
    /// [`Error::Validation`] when the event lists fail `TempoMapBuilder`
    /// validation (empty tempo, out-of-range BPM, bad signature, dup ticks).
    pub fn compile(&self) -> Result<TempoMap, Error> {
        let ppqn = Ppqn::new(
            u16::try_from(self.ppqn).map_err(|_| Error::Validation("ppqn out of range".into()))?,
        )
        .map_err(|e| Error::Validation(format!("ppqn: {e:?}")))?;
        let mut builder = TempoMapBuilder::new();
        for e in &self.tempo_events {
            builder.add_tempo_event(TempoEvent::at_tick(
                TickPosition::new(Tick::new(e.tick), ppqn),
                e.bpm_milli,
            ));
        }
        for e in &self.signature_events {
            builder.add_time_signature_event(TimeSignatureEvent::at_tick(
                TickPosition::new(Tick::new(e.tick), ppqn),
                e.numerator,
                e.denominator,
            ));
        }
        builder
            .build()
            .map_err(|e| Error::Validation(format!("tempo map compile: {e:?}")))
    }
}

/// Reference to a tempo-map snapshot revision. Carried by values that need
/// to pin which mapping produced them.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TempoMapRef {
    /// The `TempoMapData::revision` this reference names.
    pub revision: u64,
}

/// Runtime tick↔media mapper built from [`TempoMapData`].
pub struct TickTimeMapping {
    map: TempoMap,
    ppqn: u32,
}

impl TickTimeMapping {
    /// Builds the mapping, compiling the persisted event lists.
    ///
    /// # Errors
    /// [`Error::Validation`] when the events fail to compile.
    pub fn new(data: &TempoMapData) -> Result<Self, Error> {
        Ok(Self {
            map: data.compile()?,
            ppqn: data.ppqn,
        })
    }

    /// The PPQN context this mapping was built under.
    #[must_use]
    pub fn ppqn(&self) -> u32 {
        self.ppqn
    }

    /// Converts a musical tick to a media instant at `base`.
    ///
    /// # Errors
    /// [`Error::Overflow`] when the instant is out of range.
    pub fn tick_to_media(
        &self,
        tick: MusicalTick,
        numerator: u32,
        denominator: u32,
    ) -> Result<MediaTimeValue, Error> {
        let base = TimeBase::new(numerator, denominator)
            .map_err(|e| Error::Validation(format!("time base: {e:?}")))?;
        let ppqn = self.ppqn_value()?;
        let media: MediaTime = self
            .map
            .tick_to_media(TickPosition::new(Tick::new(tick.value()), ppqn), base)
            .map_err(|_| Error::Overflow)?;
        MediaTimeValue::new(media.value(), numerator, denominator)
    }

    /// Converts a media instant back to a musical tick.
    ///
    /// # Errors
    /// [`Error::Overflow`] when the tick is out of range.
    pub fn media_to_tick(&self, media: MediaTimeValue) -> Result<MusicalTick, Error> {
        let base = TimeBase::new(media.numerator(), media.denominator())
            .map_err(|e| Error::Validation(format!("time base: {e:?}")))?;
        let tick = self
            .map
            .media_to_tick(MediaTime::new(media.value(), base), self.ppqn_value()?)
            .map_err(|_| Error::Overflow)?;
        Ok(MusicalTick::new(tick.tick().value()))
    }

    /// Converts a beat position to seconds under this tempo map — the
    /// transport position's seconds display value. Exact rational math
    /// in ruviform, quantized to microseconds (`SECONDS_DEN`).
    ///
    /// # Errors
    /// [`Error::Overflow`] when the instant is out of range.
    pub fn beat_to_seconds(&self, beat: BeatPosition) -> Result<f64, Error> {
        let media = self
            .map
            .beat_to_media(beat, seconds_base()?)
            .map_err(|_| Error::Overflow)?;
        let micros = media.value().to_f64().ok_or(Error::Overflow)?;
        Ok(micros / f64::from(SECONDS_DEN))
    }

    /// Converts seconds back to a beat position under this tempo map —
    /// the inverse of [`Self::beat_to_seconds`] for the transport commit.
    /// The input is rounded to the nearest microsecond.
    ///
    /// # Errors
    /// [`Error::Overflow`] when `seconds` is non-finite or out of range.
    pub fn seconds_to_beat(&self, seconds: f64) -> Result<BeatPosition, Error> {
        let micros = (seconds * f64::from(SECONDS_DEN))
            .round()
            .to_i64()
            .ok_or(Error::Overflow)?;
        self.map
            .media_to_beat(MediaTime::new(micros, seconds_base()?))
            .map_err(|_| Error::Overflow)
    }

    /// The stored PPQN as a validated Ruviform `Ppqn`.
    fn ppqn_value(&self) -> Result<Ppqn, Error> {
        Ppqn::new(u16::try_from(self.ppqn).map_err(|_| Error::Overflow)?)
            .map_err(|e| Error::Validation(format!("ppqn: {e:?}")))
    }
}
