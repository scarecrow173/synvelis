//! Domain time types.
//!
//! Three distinct positions are never interchangeable:
//! [`SessionFrame`] (timeline of a recording session), [`SourceFrame`]
//! (offset inside a media source) — both non-negative — and
//! [`ProjectTime`] (transport position, negative pre-roll allowed).
//!
//! All arithmetic is `checked`; [`Error::Negative`]/[`Error::Overflow`]
//! are returned instead of panicking or wrapping. Ruviform `Tick`/
//! `TickDuration` `Add`/`Sub` are deliberately not used for domain math
//! (they are unchecked); `MusicalTick` carries the musical domain.

use serde::{Deserialize, Serialize};

use crate::error::Error;

macro_rules! define_frame {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        ///
        /// Invariant: the wrapped value is `>= 0`. Construction and every
        /// operation are checked; a negative input or result is an error.
        #[derive(
            Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash,
            Serialize, Deserialize,
        )]
        #[serde(try_from = "i64", into = "i64")]
        pub struct $name(i64);

        impl $name {
            /// The zero position.
            pub const ZERO: Self = Self(0);

            /// Creates a position, rejecting negative values.
            ///
            /// # Errors
            /// [`Error::Negative`] when `value < 0`.
            pub const fn new(value: i64) -> Result<Self, Error> {
                if value < 0 {
                    return Err(Error::Negative);
                }
                Ok(Self(value))
            }

            /// The raw frame count (`>= 0`).
            #[must_use]
            pub const fn value(self) -> i64 {
                self.0
            }

            /// Checked `self + delta`, rejecting overflow and negative results.
            ///
            /// # Errors
            /// [`Error::Overflow`] or [`Error::Negative`].
            pub fn checked_add(self, delta: i64) -> Result<Self, Error> {
                let v = self.0.checked_add(delta).ok_or(Error::Overflow)?;
                Self::new(v)
            }

            /// Checked `self - delta`, rejecting underflow/negative results.
            ///
            /// # Errors
            /// [`Error::Overflow`] or [`Error::Negative`].
            pub fn checked_sub(self, delta: i64) -> Result<Self, Error> {
                let v = self.0.checked_sub(delta).ok_or(Error::Overflow)?;
                Self::new(v)
            }

            /// Checked distance `self - earlier` as `i64`.
            ///
            /// # Errors
            /// [`Error::Overflow`] or [`Error::Negative`] when `earlier > self`.
            pub fn frames_since(self, earlier: Self) -> Result<i64, Error> {
                self.0
                    .checked_sub(earlier.0)
                    .ok_or(Error::Overflow)
                    .and_then(|v| if v < 0 { Err(Error::Negative) } else { Ok(v) })
            }
        }

        impl TryFrom<i64> for $name {
            type Error = Error;
            fn try_from(value: i64) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for i64 {
            fn from(v: $name) -> i64 {
                v.0
            }
        }
    };
}

define_frame!(
    /// Position on a recording session's timeline. `0` is session start;
    /// pre-session positions do not exist (negative is an error).
    SessionFrame
);
define_frame!(
    /// Offset inside a media source file. `0` is the first captured frame.
    SourceFrame
);

/// Musical position in project PPQN ticks (default 960 per quarter note).
/// Negative values are legal (pre-roll before tick 0).
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct MusicalTick(i64);

impl MusicalTick {
    /// The zero downbeat.
    pub const ZERO: Self = Self(0);

    /// Creates a tick position; negative values are legal.
    #[must_use]
    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    /// The raw tick count.
    #[must_use]
    pub const fn value(self) -> i64 {
        self.0
    }

    /// Checked `self + delta` (delta may be negative).
    ///
    /// # Errors
    /// [`Error::Overflow`].
    pub fn checked_add(self, delta: i64) -> Result<Self, Error> {
        self.0.checked_add(delta).map(Self).ok_or(Error::Overflow)
    }

    /// Checked `self - delta`.
    ///
    /// # Errors
    /// [`Error::Overflow`].
    pub fn checked_sub(self, delta: i64) -> Result<Self, Error> {
        self.0.checked_sub(delta).map(Self).ok_or(Error::Overflow)
    }

    /// Checked `self - other` as a tick span.
    ///
    /// # Errors
    /// [`Error::Overflow`].
    pub fn ticks_since(self, earlier: Self) -> Result<i64, Error> {
        self.0.checked_sub(earlier.0).ok_or(Error::Overflow)
    }
}

impl From<MusicalTick> for i64 {
    fn from(v: MusicalTick) -> i64 {
        v.0
    }
}

/// Serde-stable DTO for a Ruviform `MediaTime`.
///
/// `ruviform-media-core` types carry no serde impls, so persisted project
/// state stores this value/TimeBase triple and converts at the boundary.
/// `numerator`/`denominator` are the `TimeBase` rational (seconds =
/// `value * numerator / denominator`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaTimeValue {
    /// Instant in base ticks (negative allowed, e.g. preroll).
    pub value: i64,
    /// `TimeBase` numerator (> 0).
    pub numerator: u32,
    /// `TimeBase` denominator (> 0).
    pub denominator: u32,
}

impl MediaTimeValue {
    /// Creates a value, rejecting zero base components.
    ///
    /// # Errors
    /// [`Error::Validation`] when either base component is zero.
    pub fn new(value: i64, numerator: u32, denominator: u32) -> Result<Self, Error> {
        if numerator == 0 || denominator == 0 {
            return Err(Error::Validation("time base component is zero".into()));
        }
        Ok(Self {
            value,
            numerator,
            denominator,
        })
    }

    /// Compares two instants exactly via `v*n/d` cross-multiplication.
    #[must_use]
    fn scaled_num(self, other: &Self) -> (i128, i128) {
        let lhs =
            i128::from(self.value) * i128::from(self.numerator) * i128::from(other.denominator);
        let rhs =
            i128::from(other.value) * i128::from(other.numerator) * i128::from(self.denominator);
        (lhs, rhs)
    }

    /// `self < other` on the absolute time axis.
    #[must_use]
    pub fn lt(self, other: &Self) -> bool {
        let (a, b) = self.scaled_num(other);
        a < b
    }

    /// `self >= other` on the absolute time axis.
    #[must_use]
    pub fn ge(self, other: &Self) -> bool {
        !self.lt(other)
    }
}

/// Transport position: either musical (project ticks) or media (a time-base
/// instant). Negative positions are legal in both variants (pre-roll).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", deny_unknown_fields)]
pub enum ProjectTime {
    /// Musical position in PPQN ticks.
    Musical(MusicalTick),
    /// Media position as a value/base instant.
    Media(MediaTimeValue),
}
