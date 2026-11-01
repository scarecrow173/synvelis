//! VS-B RT 契約証拠: `process` が heap 割当ても dealloc も行わない
//! ことを `stats_alloc` のグローバルアロケータで計測する。
//!
//! `cargo test` は同一バイナリ内のテストを並列スレッドで実行し、
//! グローバルアロケータを共有するため、計測はこのファイル内の
//! 単一テストに集約する（他スレッドの割当てを窓に混入させない）。
//! 計測の前に warm-up を行い、初期化系の遅延割当てを窓から除外する。

use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use synvelis_project::*;
use synvelis_recording::*;

use std::alloc::System;

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

const RATE: u32 = 48_000;

fn default_project() -> (Project, LaneId, ClockEpochId) {
    let lane = LaneId::new(11);
    let epoch = ClockEpochId::new(22);
    let mut p = Project::new(ProjectId::new(0), "vs-b-rt");
    let track = TrackId::new(1);
    p.tracks.insert(
        track,
        Track {
            id: track,
            name: "t".into(),
            lane_ids: vec![lane],
        },
    );
    p.lanes.insert(
        lane,
        Lane {
            id: lane,
            track_id: track,
            clip_ids: vec![],
        },
    );
    p.clock_epochs.insert(
        epoch,
        ClockEpoch {
            id: epoch,
            description: "mock".into(),
        },
    );
    (p, lane, epoch)
}

fn request(lane: LaneId, epoch: ClockEpochId, b: BufferBudget) -> RecordingRequest {
    RecordingRequest {
        targets: vec![CaptureTarget {
            lane_id: lane,
            channels: 2,
        }],
        clock_epoch: epoch,
        tempo_map: TempoMapRef { revision: 0 },
        transport_start: MusicalTick::ZERO,
        stop: MusicalTick::new(1_920),
        gate: GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        budget: b,
        max_block_frames: 512,
    }
}

/// engine の RT パス 1 回分（`process`）だけを Region で計る。
fn measured_process(sc: &mut Scenario, input: &mut MockInput) -> stats_alloc::Stats {
    let rig = &mut sc.rigs[0];
    let block = input.block(128);
    let until = rig
        .engine
        .session_cursor()
        .checked_add(i64::from(block.frames))
        .unwrap()
        .min(sc.plan.session_end);
    // extend（非 RT）と PCM 生成（非 RT）は計測窓の外。
    sc.plan
        .scheduler
        .extend(rig.engine.schedule_mut(), until)
        .unwrap();
    let pcm = MockPcm { seed: 0 }.block_pcm(block.first_input_frame, block.frames, 2);
    let reg = Region::new(ALLOC);
    rig.engine.process(&block, &pcm);
    reg.change()
}

#[test]
fn rt_path_is_alloc_and_drop_free() {
    let budget = BufferBudget {
        chunks: 8,
        chunk_frames: 512,
        spans_per_chunk: 8,
        filled_capacity: 8,
        lookahead_spans: 16,
    };
    let (p, lane, epoch) = default_project();
    let plan = preflight(&p, request(lane, epoch, budget), RATE).unwrap();
    let mut sc = Scenario::build(plan);
    let mut input = MockInput::new(epoch, 0);
    // warm-up: 遅延初期化を窓の外で済ませる。
    for _ in 0..8 {
        sc.feed(&input.block(128));
    }
    assert!(!sc.fault.is_faulted());
    // 通常 capture の process: 割当て 0 / dealloc 0。
    let stats = measured_process(&mut sc, &mut input);
    assert_eq!(stats.allocations, 0, "allocations: {stats:?}");
    assert_eq!(stats.deallocations, 0, "deallocations: {stats:?}");
    assert!(!sc.fault.is_faulted());

    // Filled Queue 満杯パス: 失敗 push が返した heap-owned chunk を
    // drop せず retained に move するだけ → 割当て 0 / dealloc 0。
    let budget2 = BufferBudget {
        chunks: 4,
        chunk_frames: 512,
        spans_per_chunk: 8,
        filled_capacity: 2,
        lookahead_spans: 16,
    };
    let plan2 = preflight(&p, request(lane, epoch, budget2), RATE).unwrap();
    let mut sc2 = Scenario::build(plan2);
    let mut input2 = MockInput::new(epoch, 0);
    // filled (cap 2) を埋める（pump しない warm-up）。
    for _ in 0..2 {
        sc2.feed_engine_only(&input2.block(128));
    }
    assert!(!sc2.fault.is_faulted());
    let stats2 = measured_process(&mut sc2, &mut input2);
    assert_eq!(stats2.allocations, 0, "allocations: {stats2:?}");
    assert_eq!(stats2.deallocations, 0, "deallocations: {stats2:?}");
    assert_eq!(sc2.fault.bits() & FAULT_QUEUE_FULL, FAULT_QUEUE_FULL);
    assert!(sc2.rigs[0].engine.retained().is_some());
    // GateClosed パス（chunk を取らない）も計測。
    let plan3 = preflight(
        &p,
        RecordingRequest {
            gate: GateSpec::Punch {
                punch_in: MusicalTick::new(1_000),
                punch_out: MusicalTick::new(1_500),
            },
            ..request(lane, epoch, budget)
        },
        RATE,
    )
    .unwrap();
    let mut sc3 = Scenario::build(plan3);
    let mut input3 = MockInput::new(epoch, 0);
    for _ in 0..2 {
        sc3.feed(&input3.block(128));
    }
    let stats3 = measured_process(&mut sc3, &mut input3);
    assert_eq!(stats3.allocations, 0, "allocations: {stats3:?}");
    assert_eq!(stats3.deallocations, 0, "deallocations: {stats3:?}");
    assert!(!sc3.fault.is_faulted());
}
