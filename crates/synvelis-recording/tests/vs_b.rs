//! VS-B (E2E-R1): Mock Clock/Input → Preflight → Gate Schedule →
//! RT Capture → SPSC Handoff の受け入れ検証。
//!
//! 48 kHz / 120 BPM / PPQN 960 の既定 `TempoMap` では 1 tick = 25 frame。
//! すべて決定的 `MockPcm` で検算し、重複・欠落・順序・fault・
//! 所有権・RT 非割当てを確認する。

use synvelis_project::*;
use synvelis_recording::*;

const RATE: u32 = 48_000;

fn default_project() -> (Project, LaneId, ClockEpochId) {
    let lane = LaneId::new(11);
    let epoch = ClockEpochId::new(22);
    let mut p = Project::new(ProjectId::new(0), "vs-b");
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

fn budget() -> BufferBudget {
    BufferBudget {
        chunks: 8,
        chunk_frames: 512,
        spans_per_chunk: 8,
        filled_capacity: 8,
        lookahead_spans: 16,
    }
}

fn request(lane: LaneId, epoch: ClockEpochId, gate: GateSpec, stop_tick: i64) -> RecordingRequest {
    request_with(lane, epoch, gate, 0, stop_tick, budget())
}

fn request_with(
    lane: LaneId,
    epoch: ClockEpochId,
    gate: GateSpec,
    start_tick: i64,
    stop_tick: i64,
    budget: BufferBudget,
) -> RecordingRequest {
    RecordingRequest {
        targets: vec![CaptureTarget {
            lane_id: lane,
            channels: 2,
        }],
        clock_epoch: epoch,
        tempo_map: TempoMapRef { revision: 0 },
        transport_start: MusicalTick::new(start_tick),
        stop: MusicalTick::new(stop_tick),
        gate,
        budget,
        max_block_frames: 512,
    }
}

fn scenario(p: &Project, req: RecordingRequest) -> Scenario {
    let plan = preflight(p, req, RATE).expect("preflight must pass");
    Scenario::build(plan)
}

/// 全 chunk の span を受信順に集める。
fn all_spans(logs: &[ChunkLog]) -> Vec<&CaptureSpan> {
    logs.iter().flat_map(|l| l.spans.iter()).collect()
}

/// span 列が session 上で非重複・昇順（半開タイル）であることを検証。
fn assert_tiled(spans: &[&CaptureSpan]) {
    let mut prev_end = None;
    for s in spans {
        if let Some(e) = prev_end {
            assert!(
                s.session_start >= e,
                "overlap/order violation at {s:?} after {e:?}"
            );
        }
        assert!(s.frame_count > 0);
        prev_end = Some(
            s.session_start
                .checked_add(i64::from(s.frame_count))
                .unwrap(),
        );
    }
}

// ---------- Normal ----------

#[test]
fn normal_recording_exact_pcm_and_tiling() {
    let (p, lane, epoch) = default_project();
    // stop = 1920 ticks → 48000 frames, block 256 → 187 full + 128 tail.
    let mut sc = scenario(
        &p,
        request(
            lane,
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            1920,
        ),
    );
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 256);

    assert!(!sc.fault.is_faulted(), "fault bits: {:x}", sc.fault.bits());
    let log = &sc.rigs[0].writer.log;
    assert_ne!(log.as_slice(), []);
    for entry in log {
        assert_eq!(entry.validity, ChunkValidity::Complete);
        assert!(verify_log(entry, 0, 2), "pcm mismatch: {entry:?}");
    }
    // span 列は [0, 48000) を重複なく連続タイルする。
    let spans = all_spans(log);
    assert_tiled(&spans);
    assert_eq!(spans.first().unwrap().session_start, SessionFrame::ZERO);
    let covered: i64 = spans.iter().map(|s| i64::from(s.frame_count)).sum();
    assert_eq!(covered, 48_000);
    // header は単調に進み、SourceFrame は有効 capture で連続。
    let mut prev_src = None;
    let mut prev_session = None;
    for s in &spans {
        if let Some(ps) = prev_src {
            assert!(s.source_start >= ps);
        }
        prev_src = Some(s.source_start);
        if let Some(ps) = prev_session {
            assert!(s.session_start >= ps);
        }
        prev_session = Some(s.session_start);
    }
    // 最終 SourceFrame は 48000（全 frame 有効 capture）。
    assert_eq!(
        sc.rigs[0].engine.source_cursor(),
        SourceFrame::new(48_000).unwrap()
    );
    // 回収照合: provisioned == recovered。
    let ids = sc.recover_all();
    assert_eq!(ids, (0..8).collect::<Vec<u32>>());
}

// ---------- Punch ----------

#[test]
fn punch_in_at_frame_113_of_257_block() {
    let (p, lane, epoch) = default_project();
    // 先頭 200 frame の block の後 257 frame 刻み → block [4312, 4569)。
    // punch_in = tick 177 → session frame 4425 → block 内 offset 113。
    let gate = GateSpec::Punch {
        punch_in: MusicalTick::new(177),
        punch_out: MusicalTick::new(277), // frame 6925
    };
    let mut sc = scenario(&p, request(lane, epoch, gate, 400)); // stop: frame 10000
    let mut input = MockInput::new(epoch, 0);
    sc.feed(&input.block(200));
    while !sc.fault.is_faulted() && sc.rigs[0].engine.session_cursor().value() < 10_000 {
        let b = input.block(257);
        sc.feed(&b);
    }
    assert!(!sc.fault.is_faulted(), "fault bits: {:x}", sc.fault.bits());
    let spans = all_spans(&sc.rigs[0].writer.log);
    assert_tiled(&spans);
    // 最初の保存区間は [4425, 4569): 257-frame block 内 113 offset → 144 frame。
    assert_eq!(spans[0].session_start.value(), 4_425);
    assert_eq!(spans[0].frame_count, 144);
    // punch 総量 = 6925 - 4425 = 2500 frame。
    let total: i64 = spans.iter().map(|s| i64::from(s.frame_count)).sum();
    assert_eq!(total, 2_500);
    // 最後の保存区間の終端は punch_out (frame 6925)。
    let last = spans.last().unwrap();
    assert_eq!(
        last.session_start.value() + i64::from(last.frame_count),
        6_925
    );
    for entry in &sc.rigs[0].writer.log {
        assert!(verify_log(entry, 0, 2));
    }
    // SourceFrame は punch 区間分のみ進む。
    assert_eq!(
        sc.rigs[0].engine.source_cursor(),
        SourceFrame::new(2_500).unwrap()
    );
}

// ---------- Loop ----------

#[test]
fn three_loop_passes_and_wrap() {
    let (p, lane, epoch) = default_project();
    // 4-bar loop = 15360 ticks = 384000 frames。stop = 3 pass 分。
    let gate = GateSpec::Loop {
        loop_start: MusicalTick::ZERO,
        loop_end: MusicalTick::new(15_360),
    };
    // block 40000 は loop 長 (384000) を割り切れない → wrap を跨ぐ
    // block で 1 chunk に 2 piece 入る（spans_per_chunk >= 2 が必要）。
    let mut req = request(lane, epoch, gate, 46_080);
    req.budget = BufferBudget {
        chunk_frames: 40_000,
        ..budget()
    };
    req.max_block_frames = 40_000;
    let mut sc = scenario(&p, req);
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 40_000);

    assert!(!sc.fault.is_faulted(), "fault bits: {:x}", sc.fault.bits());
    let spans = all_spans(&sc.rigs[0].writer.log);
    assert_tiled(&spans);
    // 3 pass が [0,384000) [384000,768000) [768000,1152000) を覆う。
    let pass_bounds = [
        (0, 0, 384_000),
        (1, 384_000, 768_000),
        (2, 768_000, 1_152_000),
    ];
    for (pass, lo, hi) in pass_bounds {
        let pass_spans: Vec<_> = spans.iter().filter(|s| s.pass_index == pass).collect();
        assert!(!pass_spans.is_empty(), "pass {pass} missing");
        let cov: i64 = pass_spans.iter().map(|s| i64::from(s.frame_count)).sum();
        assert_eq!(cov, hi - lo, "pass {pass} coverage");
        assert_eq!(pass_spans[0].session_start.value(), lo);
        let last = pass_spans.last().unwrap();
        assert_eq!(last.session_start.value() + i64::from(last.frame_count), hi);
        // 同一 project 区間へ戻る。
        for s in &pass_spans {
            assert!(s.project_in.value() >= 0 && s.project_out.value() <= 15_360);
        }
    }
    for entry in &sc.rigs[0].writer.log {
        assert!(verify_log(entry, 0, 2));
    }
    assert_eq!(
        sc.rigs[0].engine.source_cursor(),
        SourceFrame::new(1_152_000).unwrap()
    );
}

#[test]
fn loop_partial_final_pass_is_capped() {
    let (p, lane, epoch) = default_project();
    let gate = GateSpec::Loop {
        loop_start: MusicalTick::ZERO,
        loop_end: MusicalTick::new(15_360),
    };
    // stop tick 20000 → session_end 500000: pass0 全 + pass1 の頭 116000。
    let mut req = request(lane, epoch, gate, 20_000);
    req.budget = BufferBudget {
        chunk_frames: 40_000,
        ..budget()
    };
    req.max_block_frames = 40_000;
    let mut sc = scenario(&p, req);
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 40_000);
    assert!(!sc.fault.is_faulted());
    let spans = all_spans(&sc.rigs[0].writer.log);
    assert_tiled(&spans);
    let last = spans.last().unwrap();
    assert_eq!(last.pass_index, 1);
    assert_eq!(
        last.session_start.value() + i64::from(last.frame_count),
        500_000
    );
    let total: i64 = spans.iter().map(|s| i64::from(s.frame_count)).sum();
    assert_eq!(total, 500_000);
}

#[test]
fn punch_in_loop_captures_only_punch_region() {
    let (p, lane, epoch) = default_project();
    // loop = 3840 ticks (96000f)、punch = [960, 2880) (24000..72000f)。
    let gate = GateSpec::PunchInLoop {
        loop_start: MusicalTick::ZERO,
        loop_end: MusicalTick::new(3_840),
        punch_in: MusicalTick::new(960),
        punch_out: MusicalTick::new(2_880),
    };
    // stop tick 10000 → session_end 250000 (2.6 pass)。
    let mut req = request(lane, epoch, gate, 10_000);
    req.budget = BufferBudget {
        chunk_frames: 9_600,
        ..budget()
    };
    req.max_block_frames = 9_600;
    let mut sc = scenario(&p, req);
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 9_600);
    assert!(!sc.fault.is_faulted(), "fault {:x}", sc.fault.bits());
    let spans = all_spans(&sc.rigs[0].writer.log);
    assert_tiled(&spans);
    // pass0: [24000,72000), pass1: [120000,168000), pass2: [216000,250000)。
    let expect = [
        (0u32, 24_000i64, 72_000i64),
        (1, 120_000, 168_000),
        (2, 216_000, 250_000),
    ];
    // span は block 境界でさらに分割されうるため、pass ごとの被覆で検証。
    for (pass, lo, hi) in expect {
        let cov: i64 = spans
            .iter()
            .filter(|s| s.pass_index == pass)
            .map(|s| i64::from(s.frame_count))
            .sum();
        assert_eq!(cov, hi - lo, "pass {pass}");
        let first = spans.iter().find(|s| s.pass_index == pass).unwrap();
        assert_eq!(first.session_start.value(), lo);
    }
    // 保存総量 = 48000*2 + 34000。
    let total: i64 = spans.iter().map(|s| i64::from(s.frame_count)).sum();
    assert_eq!(total, 130_000);
    // project 区間は全て punch 内。
    for s in &spans {
        assert!(s.project_in.value() >= 960 && s.project_out.value() <= 2_880);
    }
    for entry in &sc.rigs[0].writer.log {
        assert!(verify_log(entry, 0, 2));
    }
}

// ---------- Tempo boundary ----------

#[test]
fn tempo_boundary_splits_span_and_block() {
    let (p, lane, epoch) = default_project();
    // tick 480 で 120→60 BPM: f(1920) = 12000 + 1440*50 = 84000。
    let mut proj = p.clone();
    proj.tempo_map.tempo_events.push(TempoEventDto {
        tick: 480,
        bpm_milli: 60_000,
    });
    proj.tempo_map.revision = 1;
    let mut req = request(
        lane,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        1920,
    );
    req.tempo_map = TempoMapRef { revision: 1 };
    let mut sc = scenario(&proj, req);
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 256);
    assert!(!sc.fault.is_faulted(), "fault {:x}", sc.fault.bits());
    let spans = all_spans(&sc.rigs[0].writer.log);
    assert_tiled(&spans);
    // template は [0,480tick) と [480,1920) の 2 本。境界 frame = 12000。
    // frame 12000 を跨ぐ block で 2 piece が 1 chunk に入る。
    let boundary = spans.iter().any(|s| {
        s.session_start.value() < 12_000
            && s.session_start.value() + i64::from(s.frame_count) <= 12_000
    });
    assert!(boundary, "no span ends at tempo boundary");
    let after = spans.iter().any(|s| s.project_in.value() == 480);
    assert!(after, "no span starts at tempo tick 480");
    // project_in=480 の span の session_start は 12000。
    let s = spans.iter().find(|s| s.project_in.value() == 480).unwrap();
    assert_eq!(s.session_start.value(), 12_000);
    let total: i64 = spans.iter().map(|s| i64::from(s.frame_count)).sum();
    assert_eq!(total, 84_000);
    for entry in &sc.rigs[0].writer.log {
        assert!(verify_log(entry, 0, 2));
    }
}

// ---------- Negative pre-roll ----------

#[test]
fn negative_preroll_maps_to_positive_session() {
    let (p, lane, epoch) = default_project();
    // transport_start = -1920 ticks (-48000f)、stop = 0 → 48000f session。
    let mut req = request(
        lane,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::new(-1920),
        },
        0,
    );
    req.transport_start = MusicalTick::new(-1920);
    let mut sc = scenario(&p, req);
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 256);
    assert!(!sc.fault.is_faulted(), "fault {:x}", sc.fault.bits());
    let spans = all_spans(&sc.rigs[0].writer.log);
    // project 位置は負 pre-roll で -1920..0。
    assert!(spans[0].project_in.value() < 0);
    assert_eq!(spans[0].project_in.value(), -1920);
    let total: i64 = spans.iter().map(|s| i64::from(s.frame_count)).sum();
    assert_eq!(total, 48_000);
}

// ---------- Clock / Input faults ----------

#[test]
fn epoch_discontinuity_latches_and_no_fabrication() {
    let (p, lane, epoch) = default_project();
    let mut sc = scenario(
        &p,
        request(
            lane,
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            1920,
        ),
    );
    let mut input = MockInput::new(epoch, 0);
    sc.feed(&input.block(256));
    sc.feed(&input.block(256));
    input.set_epoch(ClockEpochId::new(99));
    sc.feed(&input.block(256));
    assert_eq!(
        sc.fault.bits() & FAULT_EPOCH_DISCONTINUITY,
        FAULT_EPOCH_DISCONTINUITY
    );
    // 断絶後の process は no-op（連続を捏造しない）。
    let blocks_before = sc.rigs[0].writer.log.len();
    sc.feed(&input.block(256));
    assert_eq!(sc.rigs[0].writer.log.len(), blocks_before);
    // 断絶前の chunk は正しい。
    for entry in &sc.rigs[0].writer.log {
        assert!(verify_log(entry, 0, 2));
    }
}

#[test]
fn input_gap_latches_discontinuity() {
    let (p, lane, epoch) = default_project();
    let mut sc = scenario(
        &p,
        request(
            lane,
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            1920,
        ),
    );
    let mut input = MockInput::new(epoch, 0);
    sc.feed(&input.block(256));
    input.jump_to(512); // 256 frame スキップ（欠落）
    sc.feed(&input.block(256));
    assert_eq!(
        sc.fault.bits() & FAULT_INPUT_DISCONTINUITY,
        FAULT_INPUT_DISCONTINUITY
    );
}

#[test]
fn input_duplicate_latches_discontinuity() {
    let (p, lane, epoch) = default_project();
    let mut sc = scenario(
        &p,
        request(
            lane,
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            1920,
        ),
    );
    let mut input = MockInput::new(epoch, 0);
    sc.feed(&input.block(256));
    input.jump_to(128); // 巻き戻り（重複）
    sc.feed(&input.block(256));
    assert_eq!(
        sc.fault.bits() & FAULT_INPUT_DISCONTINUITY,
        FAULT_INPUT_DISCONTINUITY
    );
}

// ---------- Handoff faults ----------

#[test]
fn queue_full_failstop_and_chunk_retained() {
    let (p, lane, epoch) = default_project();
    let b = BufferBudget {
        chunks: 4,
        filled_capacity: 2,
        ..budget()
    };
    let mut req = request(
        lane,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        1920,
    );
    req.budget = b;
    let mut sc = scenario(&p, req);
    // pump しない → filled (cap 2) が 3 個目の push で満杯になる。
    let mut input = MockInput::new(epoch, 0);
    for _ in 0..4 {
        let blk = input.block(128);
        sc.feed_engine_only(&blk);
        if sc.fault.is_faulted() {
            break;
        }
    }
    assert_eq!(
        sc.fault.bits() & FAULT_QUEUE_FULL,
        FAULT_QUEUE_FULL,
        "bits {:x}",
        sc.fault.bits()
    );
    // 失敗 push が返した heap-owned chunk は drop されず保持される。
    assert!(sc.rigs[0].engine.retained().is_some());
    // fail-stop: backlog を吐き切った後は engine が何も生まない。
    sc.rigs[0].writer.pump();
    let n = sc.rigs[0].writer.log.len();
    sc.feed(&input.block(128));
    assert_eq!(sc.rigs[0].writer.log.len(), n);
    // 全 chunk 回収（retained + held + 残キュー）。
    let ids = sc.recover_all();
    assert_eq!(ids, vec![0, 1, 2, 3]);
}

#[test]
fn free_pool_exhaustion_failstop() {
    let (p, lane, epoch) = default_project();
    let b = BufferBudget {
        chunks: 3,
        filled_capacity: 3,
        ..budget()
    };
    let mut req = request(
        lane,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        1920,
    );
    req.budget = b;
    let mut sc = scenario(&p, req);
    sc.rigs[0].writer.hold_next(usize::MAX); // 返却しない → free pool が尽きる
    let mut input = MockInput::new(epoch, 0);
    for _ in 0..5 {
        let blk = input.block(128);
        sc.feed(&blk);
        if sc.fault.is_faulted() {
            break;
        }
    }
    assert_eq!(
        sc.fault.bits() & FAULT_FREE_EXHAUSTED,
        FAULT_FREE_EXHAUSTED,
        "bits {:x}",
        sc.fault.bits()
    );
    let ids = sc.recover_all();
    assert_eq!(ids, vec![0, 1, 2]);
}

#[test]
fn span_capacity_overflow_failstop() {
    let (p, lane, epoch) = default_project();
    // 小 loop (2400f) を 4800f block で走査 → 1 block に 2 piece。
    // spans_per_chunk=1 で 2 本目が記録できない。
    let b = BufferBudget {
        chunks: 2,
        chunk_frames: 4_800,
        spans_per_chunk: 1,
        filled_capacity: 2,
        lookahead_spans: 8,
    };
    let mut req = request(
        lane,
        epoch,
        GateSpec::Loop {
            loop_start: MusicalTick::ZERO,
            loop_end: MusicalTick::new(96),
        },
        96 * 4,
    );
    req.budget = b;
    req.max_block_frames = 4_800;
    let mut sc = scenario(&p, req);
    let mut input = MockInput::new(epoch, 0);
    sc.feed(&input.block(4_800));
    assert_eq!(
        sc.fault.bits() & FAULT_SPAN_OVERFLOW,
        FAULT_SPAN_OVERFLOW,
        "bits {:x}",
        sc.fault.bits()
    );
    // 部分書き込み chunk は Faulted validity で保持（drop されない）。
    let retained = sc.rigs[0]
        .engine
        .retained()
        .expect("chunk must be retained");
    assert_eq!(retained.validity, ChunkValidity::Faulted);
    let ids = sc.recover_all();
    assert_eq!(ids, vec![0, 1]);
}

// ---------- Schedule watermark / ownership ----------

#[test]
fn underrun_when_schedule_not_extended() {
    let (p, lane, epoch) = default_project();
    let mut sc = scenario(
        &p,
        request(
            lane,
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            1920,
        ),
    );
    // feed を通さず schedule を補充しないまま process する。
    let block = InputBlock {
        clock_epoch: epoch,
        first_input_frame: 0,
        frames: 128,
    };
    let pcm = MockPcm { seed: 0 }.block_pcm(0, 128, 2);
    sc.rigs[0].engine.process(&block, &pcm);
    assert_eq!(
        sc.fault.bits() & FAULT_SCHEDULE_UNDERRUN,
        FAULT_SCHEDULE_UNDERRUN
    );
}

#[test]
fn spsc_endpoints_are_single_owner_pairs() {
    let (engine_ends, writer_ends) = build_handoff(0, 8, 2, 2, 4, 2);
    // free pool にはちょうど chunks 個が入っている。
    let mut free = engine_ends.free_rx;
    let mut ids = Vec::new();
    while let Ok(c) = free.try_pop() {
        ids.push(c.id);
    }
    ids.sort_unstable();
    assert_eq!(ids, vec![0, 1, 2, 3]);
    // 空の consumer は Empty を返す（non-blocking）。
    assert!(free.try_pop().is_err());
    drop(engine_ends.filled_tx);
    drop(writer_ends);
}

// RT 非割当ての計測は `vs_b_rt_alloc.rs` に分離（並列テストが
// グローバルアロケータを共有するため、別バイナリで単独に計測する）。
