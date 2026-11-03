//! Policy time source (GW-02).
//!
//! Grant/task/permit validity is evaluated against a [`PolicyInstant`]
//! supplied by a [`PolicyClock`]. The evaluator never calls
//! `SystemTime::now` directly — a deterministic clock keeps expiry
//! decisions testable and lets a host pin evaluation to a fixed point
//! (e.g. inside a commit fence).

use std::sync::Mutex;

/// An opaque instant on the policy clock — milliseconds, ordered.
///
/// The scale is deliberately unspecified beyond ordering and equality:
/// it marks *positions* in policy time, not wall-clock meaning.
/// Deterministic `now` sources (manual clocks, a fence-pinned instant)
/// and wall clocks produce comparable values only by convention; the
/// evaluator only compares instants from the same configured clock.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct PolicyInstant(pub u64);

impl PolicyInstant {
    /// Wraps a raw instant value.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw instant value.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// The time source policy validity is evaluated against.
pub trait PolicyClock: Send + Sync {
    /// The current policy instant.
    fn now(&self) -> PolicyInstant;
}

/// Wall-clock source (milliseconds since the Unix epoch).
#[derive(Copy, Clone, Debug, Default)]
pub struct SystemClock;

impl PolicyClock for SystemClock {
    fn now(&self) -> PolicyInstant {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or_default();
        PolicyInstant(millis)
    }
}

/// Deterministic clock for tests and fence-pinned evaluation.
///
/// Time advances only when explicitly moved, so a test can place a
/// request exactly at an expiry boundary.
#[derive(Debug)]
pub struct ManualClock {
    now: Mutex<PolicyInstant>,
}

impl ManualClock {
    /// A clock stopped at `start`.
    #[must_use]
    pub const fn new(start: PolicyInstant) -> Self {
        Self {
            now: Mutex::new(start),
        }
    }

    /// Sets the current instant.
    ///
    /// # Panics
    ///
    /// Panics if the clock's mutex is poisoned.
    pub fn set(&self, instant: PolicyInstant) {
        *self.now.lock().expect("manual clock poisoned") = instant;
    }

    /// Advances the clock by `millis`.
    ///
    /// # Panics
    ///
    /// Panics if the clock's mutex is poisoned.
    pub fn advance(&self, millis: u64) {
        let mut now = self.now.lock().expect("manual clock poisoned");
        *now = PolicyInstant(now.0.saturating_add(millis));
    }
}

impl PolicyClock for ManualClock {
    fn now(&self) -> PolicyInstant {
        *self.now.lock().expect("manual clock poisoned")
    }
}
