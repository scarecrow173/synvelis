//! RCD-01 RT 契約証拠: data callback（planar→interleave→work queue
//! push）が heap 割当ても dealloc も行わないことを `stats_alloc` の
//! グローバルアロケータで計測する。
//!
//! `cargo test` は同一バイナリ内のテストを並列スレッドで実行し、
//! グローバルアロケータを共有するため、計測はこのファイル内の
//! 単一テストに集約する（他スレッドの割当てを窓に混入させない）。
//! 計測の前に warm-up を行い、初期化系の遅延割当てを窓から除外する。

// 各 test binary が common の subset だけを使うため module 単位で許容
// （synvelis-library/tests/common と同じ運用）。
#[allow(dead_code)]
mod common;

use std::alloc::System;
use std::sync::{Arc, Mutex};

use common::*;
use ruviform_media_core::ClockDomainId;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use synvelis_project::ClockEpochId;
use synvelis_recording::{
    ArmState, ChannelMap, FaultLatch, InputAdapter, InputStreamSpec, MonitorState,
    PreparedRecordingPlan,
};

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

/// armed stereo tap の spec（2 track 構成ではなく 1 track stereo）。
fn stereo_spec(plan: &PreparedRecordingPlan, epoch: ClockEpochId) -> InputStreamSpec {
    InputStreamSpec {
        config: input_config(2),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue: 16,
        taps: vec![tap(
            plan,
            0,
            ChannelMap::stereo(0, 1),
            ArmState::On,
            MonitorState::Off,
        )],
    }
}

#[test]
fn rt_callback_is_alloc_and_drop_free() {
    let (p, lanes, epoch) = project_with_lanes(1);
    let plan = plan_for(&p, &lanes, &[2], epoch, 200);
    let device = FakeInputDevice::new(2, ClockDomainId::SAMPLE_CLOCK);
    let fault = FaultLatch::new();
    let opened =
        InputAdapter::open(&device, &stereo_spec(&plan, epoch), fault.clone()).expect("open");
    let (mut adapter, mut writers) = into_writers(opened, &fault);
    let stream: Arc<Mutex<FakeStreamShared>> = device.stream(0);

    // warm-up: queue/pool の初回経路と pump/writer の遅延初期化を
    // 窓の外で済ませる。
    for _ in 0..8 {
        stream.lock().expect("lock").deliver(256);
        adapter.pump(&plan.scheduler);
        for (_, w) in &mut writers {
            w.pump();
        }
    }
    assert!(!fault.is_faulted());

    // data callback 1 回分（planar gather → interleave → work queue
    // push）だけを Region で計る。Mutex guard は窓の外で取得する。
    let mut guard = stream.lock().expect("lock");
    let reg = Region::new(ALLOC);
    guard.deliver(256);
    let stats = reg.change();
    drop(guard);
    assert_eq!(stats.allocations, 0, "allocations: {stats:?}");
    assert_eq!(stats.deallocations, 0, "deallocations: {stats:?}");

    // 計測した block を pump/writer まで流し、経路全体が健全なことを確認。
    adapter.pump(&plan.scheduler);
    for (_, w) in &mut writers {
        w.pump();
    }
    assert!(!fault.is_faulted(), "fault bits: {:x}", fault.bits());
    assert!(
        writers[0]
            .1
            .log
            .iter()
            .all(|l| verify_tap_log(l, &[0, 1], SEED))
    );

    // stamp fallback 経路（frame_position: None + capture_stamp: Some）
    // も同じ no-alloc 契約を持つことを確認する。warm-up を挟んでから
    // 1 block を計測する。
    {
        let mut s = stream.lock().expect("lock");
        s.set_report_position(false);
        s.set_report_capture(true);
    }
    for _ in 0..4 {
        stream.lock().expect("lock").deliver(256);
        adapter.pump(&plan.scheduler);
        for (_, w) in &mut writers {
            w.pump();
        }
    }
    assert!(!fault.is_faulted(), "fault bits: {:x}", fault.bits());

    let mut guard = stream.lock().expect("lock");
    let reg = Region::new(ALLOC);
    guard.deliver(256);
    let stats = reg.change();
    drop(guard);
    assert_eq!(stats.allocations, 0, "stamp-path allocations: {stats:?}");
    assert_eq!(
        stats.deallocations, 0,
        "stamp-path deallocations: {stats:?}"
    );

    adapter.pump(&plan.scheduler);
    for (_, w) in &mut writers {
        w.pump();
    }
    assert!(!fault.is_faulted(), "fault bits: {:x}", fault.bits());
}
