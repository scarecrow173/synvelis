//! GW-10 causal event trace (test/PoC evidence only).
//!
//! The trace records *what the driver observed*, in the order it was
//! observed, so an evidence record can show the causal order behind a
//! verdict — e.g. "grant revoked before commit fence entered" — instead
//! of a bare pass/fail. Trace entries are sequence-numbered; the
//! sequence is the ordering authority within one process.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// One observed event in a scenario's causal order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TraceEvent {
    /// Monotonic sequence number — the ordering authority.
    pub seq: u64,
    /// The actor the event is attributed to (thread/driver label).
    pub actor: String,
    /// The event kind (`"fence-parked"`, `"commit-confirmed"`, ...).
    pub kind: String,
    /// Free-form detail carried by the event.
    pub detail: Value,
}

/// A process-local causal trace — cheap to clone; clones share one
/// sequence and one event log.
#[derive(Clone, Default)]
pub struct CausalTrace {
    inner: Arc<TraceInner>,
}

#[derive(Default)]
struct TraceInner {
    next: AtomicU64,
    events: Mutex<Vec<TraceEvent>>,
}

impl CausalTrace {
    /// A fresh trace.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Records `actor`/`kind`/`detail` at the next sequence position
    /// and returns the recorded event.
    ///
    /// # Panics
    /// The event log lock is poisoned.
    // The recorded event is returned for callers that branch on it —
    // the log itself is the contract, discarding is fine.
    #[allow(clippy::must_use_candidate)]
    pub fn record(&self, actor: &str, kind: &str, detail: Value) -> TraceEvent {
        let seq = self.inner.next.fetch_add(1, Ordering::Relaxed);
        let event = TraceEvent {
            seq,
            actor: actor.to_owned(),
            kind: kind.to_owned(),
            detail,
        };
        self.inner.events.lock().unwrap().push(event.clone());
        event
    }

    /// Convenience for `record(actor, kind, json!(msg))`.
    ///
    /// # Panics
    /// The event log lock is poisoned.
    #[allow(clippy::must_use_candidate)]
    pub fn mark(&self, actor: &str, kind: &str, msg: &str) -> TraceEvent {
        self.record(actor, kind, json!(msg))
    }

    /// The recorded events in sequence order.
    ///
    /// # Panics
    /// The event log lock is poisoned.
    #[must_use]
    pub fn events(&self) -> Vec<TraceEvent> {
        self.inner.events.lock().unwrap().clone()
    }

    /// The seq order of the first event of `kind_a` precedes the first
    /// event of `kind_b` — the assertion ordering evidence rests on.
    #[must_use]
    pub fn precedes(&self, kind_a: &str, kind_b: &str) -> bool {
        let events = self.events();
        let a = events.iter().find(|e| e.kind == kind_a);
        let b = events.iter().find(|e| e.kind == kind_b);
        match (a, b) {
            (Some(a), Some(b)) => a.seq < b.seq,
            _ => false,
        }
    }
}
