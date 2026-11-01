//! Half-open `[start, end)` ranges over domain position types.
//!
//! `end < start` is rejected at construction ([`Error::Inverted`]).
//! Empty ranges (`end == start`) are legal at the type level; domains that
//! require non-empty ranges (e.g. capture segments) validate separately.

use serde::{Deserialize, Serialize};

use crate::{
    error::Error,
    time::{MediaTimeValue, MusicalTick, SessionFrame, SourceFrame},
};

/// Half-open range over an ordered position type.
///
/// Serialized as `{start, end}`. Construction goes through
/// [`Range::try_new`] — the direct fields are private so the `end >=
/// start` invariant cannot be bypassed.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(
    try_from = "RangeRepr<F>",
    into = "RangeRepr<F>",
    bound(
        serialize = "F: Copy + Serialize",
        deserialize = "F: Copy + PartialOrd + Deserialize<'de>"
    )
)]
#[serde(deny_unknown_fields)]
pub struct Range<F> {
    start: F,
    end: F,
}

/// Wire form for [`Range`]; validates on conversion.
#[derive(Copy, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RangeRepr<F> {
    start: F,
    end: F,
}

impl<F: Copy + PartialOrd> Range<F> {
    /// Creates a range, rejecting `end < start`.
    ///
    /// # Errors
    /// [`Error::Inverted`] when `end < start`.
    pub fn try_new(start: F, end: F) -> Result<Self, Error> {
        if end < start {
            return Err(Error::Inverted);
        }
        Ok(Self { start, end })
    }

    /// Inclusive start of the range.
    #[must_use]
    pub fn start(&self) -> F {
        self.start
    }

    /// Exclusive end of the range.
    #[must_use]
    pub fn end(&self) -> F {
        self.end
    }

    /// `true` when the range contains no position (`start == end`).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// `true` when `pos` is inside `[start, end)`.
    #[must_use]
    pub fn contains(&self, pos: F) -> bool {
        self.start <= pos && pos < self.end
    }

    /// `true` when the two ranges share at least one position.
    /// Adjacent ranges (`a.end == b.start`) do **not** overlap.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// The shared sub-range, or `None` when the ranges are disjoint or
    /// merely adjacent.
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Option<Self>
    where
        F: Ord,
    {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        (start < end).then_some(Self { start, end })
    }
}

impl<F: Copy> From<Range<F>> for RangeRepr<F> {
    fn from(r: Range<F>) -> Self {
        Self {
            start: r.start,
            end: r.end,
        }
    }
}

impl<F: Copy + PartialOrd> TryFrom<RangeRepr<F>> for Range<F> {
    type Error = Error;
    fn try_from(repr: RangeRepr<F>) -> Result<Self, Self::Error> {
        Self::try_new(repr.start, repr.end)
    }
}

/// `[start, end)` over [`SessionFrame`] positions.
pub type SessionRange = Range<SessionFrame>;
/// `[start, end)` over [`SourceFrame`] positions.
pub type SourceRange = Range<SourceFrame>;
/// `[start, end)` over [`MusicalTick`] positions.
pub type MusicalRange = Range<MusicalTick>;

/// Half-open span between two media instants (possibly different bases).
///
/// Distinct from ruviform `MediaRange` (which is `start + duration`,
/// unvalidated and saturating). Ordering uses exact cross-multiplied
/// comparison via [`MediaTimeValue::ge`]/[`MediaTimeValue::lt`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "MediaSpanRepr", into = "MediaSpanRepr")]
#[serde(deny_unknown_fields)]
pub struct MediaSpan {
    start: MediaTimeValue,
    end: MediaTimeValue,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MediaSpanRepr {
    start: MediaTimeValue,
    end: MediaTimeValue,
}

impl MediaSpan {
    /// Creates a span, rejecting `end < start`.
    ///
    /// # Errors
    /// [`Error::Inverted`] when `end` precedes `start`.
    pub fn try_new(start: MediaTimeValue, end: MediaTimeValue) -> Result<Self, Error> {
        if end.lt(&start) {
            return Err(Error::Inverted);
        }
        Ok(Self { start, end })
    }

    /// Inclusive start instant.
    #[must_use]
    pub fn start(&self) -> MediaTimeValue {
        self.start
    }

    /// Exclusive end instant.
    #[must_use]
    pub fn end(&self) -> MediaTimeValue {
        self.end
    }

    /// `true` when `start == end`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start.ge(&self.end) && self.end.ge(&self.start)
    }
}

impl From<MediaSpan> for MediaSpanRepr {
    fn from(s: MediaSpan) -> Self {
        Self {
            start: s.start,
            end: s.end,
        }
    }
}

impl TryFrom<MediaSpanRepr> for MediaSpan {
    type Error = Error;
    fn try_from(repr: MediaSpanRepr) -> Result<Self, Self::Error> {
        Self::try_new(repr.start, repr.end)
    }
}
