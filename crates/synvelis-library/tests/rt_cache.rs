//! ALIB-03 RT 契約証拠: `RtEventQueue::try_emit` と
//! `SuppressingSink::emit`（RT callback が使う queue 投入経路のみ）
//! が heap 割当ても dealloc も行わないことを `stats_alloc` の
//! グローバルアロケータで計測する。`drain` は非 RT 側の消費経路
//! なので計測対象外。
//!
//! `cargo test` は同一バイナリ内のテストを並列スレッドで実行し、
//! グローバルアロケータを共有するため、計測はこのファイル内の
//! 単一テストに集約する（他スレッドの割当てを窓に混入させない）。
//! 計測の前に warm-up を行い、初期化系の遅延割当てを窓から除外する。

use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use synvelis_library::{
    LibraryEvent, LibraryEventKind, LibraryEventSink, LibraryGeneration, ObservedAt, RtEventQueue,
    SuppressingSink,
};

use std::alloc::System;
use std::time::SystemTime;

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

fn ev(n: u64) -> LibraryEvent {
    LibraryEvent {
        generation: LibraryGeneration(n),
        kind: LibraryEventKind::CacheCleared,
        at: ObservedAt(SystemTime::UNIX_EPOCH),
    }
}

#[test]
fn rt_event_enqueue_is_alloc_and_dealloc_free() {
    // Warm-up on the measured instance itself: fill the slot ring,
    // drain, confirm empty — first-touch mutex/ring paths and lazy
    // allocator init land outside the measured window (same pattern
    // as the other rt_* contract tests).
    let queue = RtEventQueue::new(8);
    for i in 1..=8u64 {
        assert!(queue.try_emit(ev(i)));
    }
    assert_eq!(queue.drain().len(), 8);
    assert!(queue.is_empty());

    // Queue-insertion-only emit: pre-built events move into the
    // preallocated slot ring — no heap traffic on the RT path.
    let events = [ev(1), ev(2), ev(3), ev(4), ev(5), ev(6), ev(7), ev(8)];
    let reg = Region::new(ALLOC);
    for event in events {
        assert!(queue.try_emit(event));
    }
    let stats = reg.change();
    assert_eq!(stats.allocations, 0, "allocations: {stats:?}");
    assert_eq!(stats.deallocations, 0, "deallocations: {stats:?}");
    assert_eq!(queue.len(), 8);
    let _ = queue.drain();

    // The composed RT path — SuppressingSink generation gate in
    // front of the queue — is equally alloc/dealloc free, including
    // the stale-generation drop branch.
    let sink = SuppressingSink::new(RtEventQueue::new(8), LibraryGeneration(0));
    // Warm-up on the measured sink: one emit passes the gate
    // (generation 0 does not advance the watermark) and occupies a
    // queue slot. SuppressingSink exposes no drain, so the warm
    // event is undrainable and leaves 7 free slots; the nine
    // gate-passing events below then additionally exercise the
    // full-queue drop branch (ev(8) and ev(9) are dropped there).
    sink.emit(ev(0));
    let events = [
        ev(1),
        ev(2),
        ev(3),
        ev(4),
        ev(5),
        ev(6),
        ev(7),
        ev(8),
        ev(3), // stale vs watermark 8 → dropped before the queue
        ev(9),
    ];
    let reg = Region::new(ALLOC);
    for event in events {
        sink.emit(event);
    }
    let stats = reg.change();
    assert_eq!(stats.allocations, 0, "allocations: {stats:?}");
    assert_eq!(stats.deallocations, 0, "deallocations: {stats:?}");
    assert_eq!(sink.latest(), LibraryGeneration(9));
}
