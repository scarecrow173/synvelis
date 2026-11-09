//! Asynchronous event notification for the library cache.
//!
//! `LibraryCache` emits [`LibraryEvent`]s to registered
//! [`LibraryEventSink`]s — the port through which Player/Workspace
//! (later slices) observe catalog/preview activity. Every event is
//! stamped with the hub's current [`LibraryGeneration`]; invalidate
//! operations bump the generation first so consumers can suppress
//! results computed against older state.
//!
//! RT contract: an RT-safe sink must do queue insertion only — no
//! network, disk, hashing, blocking, or allocation inside the
//! callback. [`RtEventQueue`] is the reference implementation: a
//! bounded ring guarded by `try_lock`, where `emit` never blocks and
//! never allocates once warm (full/contended pushes are counted as
//! `dropped`, never queued).

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::IndexCompleteness;
use crate::observation::ObservedAt;

/// Monotonic cache-generation counter.
///
/// Bumped by every invalidating operation (principal retirement,
/// credential change, explicit clear, generation sweep). Events
/// emitted during/after the bump carry the new value; consumers use
/// [`SuppressingSink`] or filter [`RtEventQueue::drain`] output to
/// drop anything older than the newest generation they have seen.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LibraryGeneration(pub u64);

impl LibraryGeneration {
    /// The raw counter value.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Why a principal's private cache entries were retired.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RetireReason {
    /// The principal's credential epoch advanced (credential change).
    CredentialChanged,
    /// Access was explicitly revoked for the principal.
    PermissionRevoked,
    /// The provider reported `PermissionDenied`/`AuthExpired` for
    /// this principal — the cache retires defensively rather than
    /// risk serving private data under stale authority.
    ProviderDenied,
    /// The whole cache was explicitly cleared.
    ExplicitClear,
}

/// What happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryEventKind {
    /// A catalog index was built or refreshed.
    CatalogIndexed {
        /// Fingerprint of the catalog cache key (never the key
        /// itself — no principal/locator payloads in events).
        key_fp: String,
        /// The recorded completeness of the new index.
        completeness: IndexCompleteness,
    },
    /// A preview payload was stored.
    PreviewStored {
        /// Fingerprint of the preview cache key.
        key_fp: String,
        /// Cached payload bytes.
        byte_len: u64,
    },
    /// Entries were evicted by budget enforcement (batched).
    EntriesEvicted {
        /// Number of entries removed.
        count: u64,
        /// Payload bytes freed.
        bytes: u64,
    },
    /// All entries of one principal were retired.
    PrincipalRetired {
        /// Fingerprint of the retired principal scope.
        principal_fp: String,
        /// Why the retirement happened.
        reason: RetireReason,
    },
    /// The entire cache was cleared.
    CacheCleared,
}

/// One notification stamped with the emitting generation.
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryEvent {
    /// The hub generation at emit time.
    pub generation: LibraryGeneration,
    /// What happened.
    pub kind: LibraryEventKind,
    /// When the event was produced.
    pub at: ObservedAt,
}

/// A consumer of library events (Player/Workspace adapters later).
///
/// Implementations must be `Send + Sync`. RT callers must only use
/// sinks whose `emit` is queue-insertion-only — see
/// [`RtEventQueue`].
pub trait LibraryEventSink: Send + Sync {
    /// Receives one event. Called from non-RT worker threads in
    /// this slice; an RT-safe implementation must not block, do IO,
    /// hash, or allocate.
    fn emit(&self, event: LibraryEvent);
}

/// Fan-out hub owned by `LibraryCache`.
///
/// `generation` starts at 1 and is bumped by invalidating
/// operations. Sinks are `Arc` so consumers can subscribe/unsubscribe
/// independently of the cache's lifetime.
#[derive(Default)]
pub(crate) struct EventHub {
    generation: AtomicU64,
    sinks: Mutex<Vec<Arc<dyn LibraryEventSink>>>,
}

impl EventHub {
    pub(crate) fn new() -> Self {
        Self {
            generation: AtomicU64::new(1),
            sinks: Mutex::new(Vec::new()),
        }
    }

    /// The current generation (for stamping non-invalidating events).
    pub(crate) fn generation(&self) -> LibraryGeneration {
        LibraryGeneration(self.generation.load(Ordering::Acquire))
    }

    /// Advances the generation; returns the new value. Invalidating
    /// operations call this *before* emitting so the invalidation
    /// event itself carries the new boundary.
    pub(crate) fn bump(&self) -> LibraryGeneration {
        LibraryGeneration(self.generation.fetch_add(1, Ordering::AcqRel) + 1)
    }

    /// Registers a sink.
    pub(crate) fn add_sink(&self, sink: Arc<dyn LibraryEventSink>) {
        if let Ok(mut sinks) = self.sinks.lock() {
            sinks.push(sink);
        }
    }

    /// Emits `kind` stamped with `generation` to all sinks. Sink
    /// panics are contained — a broken consumer must not break the
    /// cache operation that produced the event.
    pub(crate) fn emit(
        &self,
        generation: LibraryGeneration,
        kind: LibraryEventKind,
        at: ObservedAt,
    ) {
        let event = LibraryEvent {
            generation,
            kind,
            at,
        };
        let sinks = match self.sinks.lock() {
            Ok(sinks) => sinks.clone(),
            Err(_) => return,
        };
        for sink in sinks {
            // A panicking sink is a consumer bug; keep serving the
            // remaining sinks and the cache operation.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sink.emit(event.clone());
            }));
        }
    }
}

/// Wraps a sink and drops events older than the newest generation
/// already observed — the consumer-side stale-generation suppressor.
///
/// Events with `generation >= latest seen` pass through and advance
/// the watermark; strictly older events are discarded. The watermark
/// is monotonic (`fetch_max`), so out-of-order delivery is handled.
pub struct SuppressingSink<S: LibraryEventSink> {
    inner: S,
    latest: AtomicU64,
}

impl<S: LibraryEventSink> SuppressingSink<S> {
    /// Wraps `inner`, starting the watermark at `initial` (usually
    /// the generation the consumer already knows is current).
    #[must_use]
    pub fn new(inner: S, initial: LibraryGeneration) -> Self {
        Self {
            inner,
            latest: AtomicU64::new(initial.raw()),
        }
    }

    /// The newest generation observed so far.
    #[must_use]
    pub fn latest(&self) -> LibraryGeneration {
        LibraryGeneration(self.latest.load(Ordering::Acquire))
    }
}

impl<S: LibraryEventSink> LibraryEventSink for SuppressingSink<S> {
    fn emit(&self, event: LibraryEvent) {
        let observed = event.generation.raw();
        let prev = self.latest.fetch_max(observed, Ordering::AcqRel);
        if observed < prev {
            // Stale: produced before a newer generation was already
            // observed — drop it.
            return;
        }
        self.inner.emit(event);
    }
}

/// A bounded, non-blocking, allocation-free event queue for RT
/// contexts.
///
/// `emit` performs a `try_lock` on a preallocated slot ring: it
/// never blocks, never performs IO/hashing, and never allocates once
/// constructed (events are moved into pre-sized `Option` slots).
/// When the queue is full or the lock is contended the event is
/// dropped and counted — bounded memory with honest loss reporting
/// via [`Self::dropped`]. Drain with [`Self::drain`] on a non-RT
/// thread.
pub struct RtEventQueue {
    slots: Mutex<Vec<Option<LibraryEvent>>>,
    head: AtomicUsize,
    tail: AtomicUsize,
    capacity: usize,
    dropped: AtomicUsize,
}

impl RtEventQueue {
    /// A queue holding at most `capacity` pending events.
    ///
    /// # Panics
    /// Panics when `capacity == 0` — an RT queue must have room.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "RtEventQueue capacity must be > 0");
        Self {
            slots: Mutex::new(vec![None; capacity]),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            capacity,
            dropped: AtomicUsize::new(0),
        }
    }

    /// Queue-insertion-only emit. Returns `true` when the event was
    /// queued, `false` when it was dropped (full or contended).
    /// Never blocks and never allocates after construction.
    #[must_use]
    pub fn try_emit(&self, event: LibraryEvent) -> bool {
        let Ok(mut slots) = self.slots.try_lock() else {
            self.dropped.fetch_add(1, Ordering::AcqRel);
            return false;
        };
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= self.capacity {
            self.dropped.fetch_add(1, Ordering::AcqRel);
            return false;
        }
        slots[head % self.capacity] = Some(event);
        self.head.store(head.wrapping_add(1), Ordering::Release);
        true
    }

    /// Drains all pending events in FIFO order. Non-RT only.
    #[must_use]
    pub fn drain(&self) -> Vec<LibraryEvent> {
        let mut out = Vec::new();
        let Ok(mut slots) = self.slots.lock() else {
            return out;
        };
        loop {
            let tail = self.tail.load(Ordering::Acquire);
            if tail >= self.head.load(Ordering::Acquire) {
                break;
            }
            if let Some(event) = slots[tail % self.capacity].take() {
                out.push(event);
            }
            self.tail.store(tail.wrapping_add(1), Ordering::Release);
        }
        out
    }

    /// Events dropped because the queue was full or contended.
    #[must_use]
    pub fn dropped(&self) -> usize {
        self.dropped.load(Ordering::Acquire)
    }

    /// Pending (queued, not yet drained) event count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.head
            .load(Ordering::Acquire)
            .wrapping_sub(self.tail.load(Ordering::Acquire))
    }

    /// `true` when no events are pending.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl LibraryEventSink for RtEventQueue {
    fn emit(&self, event: LibraryEvent) {
        let _ = self.try_emit(event);
    }
}
