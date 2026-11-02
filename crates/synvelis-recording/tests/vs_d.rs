//! VS-D (E2E-R3): Recording Commit / Idempotency / Undo / Reload の
//! 受け入れ検証。Acceptance Matrix VS-001〜015 を全て実測する。
//!
//! Mock Input → gate → RT handoff → staging writer → finalize →
//! `RecordingOp::CommitRecording` 単一 mutation → save/reload/undo/redo
//! の決定的経路で、冪等 commit・crash 前後・fault 分離・partial apply
//! なしを確認する。実 DSP/GUI/device は扱わない。

use std::{
    fs::{self},
    path::{Path, PathBuf},
};

use synvelis_project::*;
use synvelis_recording::*;

const RATE: u32 = 48_000;
const CHANNELS: u32 = 2;

// ---------- helpers ----------

/// clip の musical placement を `[start, end)` tick として返す。
fn clip_musical_span(clip: &Clip) -> (i64, i64) {
    let TimeAnchor::Musical(start) = clip.placement.anchor else {
        panic!("clip anchor must be musical")
    };
    let ClipExtent::Musical(dur) = clip.placement.extent else {
        panic!("clip extent must be musical")
    };
    (start.value(), start.value() + dur.value())
}

/// controller + track + lanes + clock epoch を op 経由で構築する。
fn controller_with(lane_count: usize) -> (ProjectController, Vec<LaneId>, ClockEpochId) {
    let mut c = ProjectController::new(
        Project::new(ProjectId::new(0), "vs-d"),
        Box::new(CounterIdGenerator::new(1)),
    );
    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    let mut lanes = Vec::new();
    for _ in 0..lane_count {
        apply(&mut c, ProjectOp::AddLane { track_id: track });
        lanes.push(*c.live().lanes.keys().last().unwrap());
    }
    apply(
        &mut c,
        ProjectOp::RegisterClockEpoch {
            description: "mock".into(),
        },
    );
    let epoch = *c.live().clock_epochs.keys().next().unwrap();
    (c, lanes, epoch)
}

fn apply<O: Operation>(c: &mut ProjectController, op: O) -> ProjectRevision {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .expect("op applies")
}

fn rop(c: &mut ProjectController, op: RecordingOp) -> ProjectRevision {
    apply(c, SynvelisOp::Recording(op))
}

/// session 作成 → Open → lane 数分の stream 登録（staging 用 cfg を返す）。
fn open_session(
    c: &mut ProjectController,
    lanes: &[LaneId],
    epoch: ClockEpochId,
) -> (RecordingSessionId, Vec<StagingStreamConfig>) {
    rop(
        c,
        RecordingOp::CreateSession {
            name: "vs-d-session".into(),
        },
    );
    let session_id = *c.live().recording.sessions.keys().next().unwrap();
    rop(c, RecordingOp::OpenSession { session_id });
    let mut cfgs = Vec::new();
    for (i, &lane) in lanes.iter().enumerate() {
        rop(
            c,
            RecordingOp::RegisterStream {
                session_id,
                lane_id: lane,
                clock_epoch_id: epoch,
            },
        );
        let stream_id = *c.live().recording.streams.keys().last().unwrap();
        cfgs.push(StagingStreamConfig {
            stream_index: u16::try_from(i).unwrap(),
            capture_stream_id: stream_id.to_tagged(),
            lane_id: lane.to_tagged(),
            channels: CHANNELS,
        });
    }
    (session_id, cfgs)
}

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("synvelis-vsd-{}-{name}", std::process::id()));
    if d.exists() {
        fs::remove_dir_all(&d).unwrap();
    }
    fs::create_dir_all(&d).unwrap();
    d
}

fn request(
    lanes: &[LaneId],
    epoch: ClockEpochId,
    gate: GateSpec,
    stop_tick: i64,
    filled_capacity: usize,
) -> RecordingRequest {
    RecordingRequest {
        targets: lanes
            .iter()
            .map(|&lane_id| CaptureTarget {
                lane_id,
                channels: CHANNELS,
            })
            .collect(),
        clock_epoch: epoch,
        tempo_map: TempoMapRef { revision: 0 },
        transport_start: MusicalTick::ZERO,
        stop: MusicalTick::new(stop_tick),
        gate,
        budget: BufferBudget {
            chunks: 16,
            chunk_frames: 40_000,
            spans_per_chunk: 8,
            filled_capacity,
            lookahead_spans: 32,
        },
        max_block_frames: 40_000,
    }
}

/// mock 録音 → staging → checkpoint → finalize を実行し、
/// session を Closed にする。戻り値は staging area と finish refs。
fn record_and_finalize(
    c: &mut ProjectController,
    root: &Path,
    session_id: RecordingSessionId,
    cfgs: &[StagingStreamConfig],
    req: RecordingRequest,
    epoch: ClockEpochId,
    block_frames: u32,
) -> (StagingArea, FinishReport) {
    let area = StagingArea::create(
        root,
        &StagingSessionConfig {
            session_id,
            sample_rate: RATE,
            streams: cfgs.to_vec(),
        },
    )
    .unwrap();
    let plan = preflight(c.live(), req, RATE).expect("preflight");
    let a = area.clone();
    let mut sc: Scenario<StreamWriter> = Scenario::build_with(plan, move |i, ends, fault| {
        StreamWriter::create(&a, cfgs[i as usize].clone(), ends, fault).unwrap()
    });
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, block_frames);
    for rig in &mut sc.rigs {
        rig.writer.checkpoint().unwrap();
    }
    let tag = session_id.to_tagged();
    let writers: Vec<&mut StreamWriter> = sc.rigs.iter_mut().map(|r| &mut r.writer).collect();
    let report = finish_all(&tag, RATE, writers);
    (area, report)
}

/// 録音 → close → commit を一括で行う定番経路。
fn record_close_commit(
    c: &mut ProjectController,
    root: &Path,
    session_id: RecordingSessionId,
    cfgs: &[StagingStreamConfig],
    req: RecordingRequest,
    epoch: ClockEpochId,
) -> (StagingArea, CommitReport) {
    let (area, fin) = record_and_finalize(c, root, session_id, cfgs, req, epoch, 40_000);
    for (_, r) in &fin.refs {
        assert!(r.is_ok(), "finalize failed: {r:?}");
    }
    rop(c, RecordingOp::CloseSession { session_id });
    let report = commit_session(c, &area).expect("commit_session");
    (area, report)
}

fn lane_clips(c: &ProjectController, lane: LaneId) -> usize {
    c.live().lanes[&lane].clip_ids.len()
}

/// PCM 未混入検査用: 64 要素を超える配列を PCM 埋め込み疑いとして
/// 検出する（manifest の ID/span 配列はどれも小さい）。
fn has_large_array(v: &serde_json::Value, found: &mut bool) {
    match v {
        serde_json::Value::Array(a) if a.len() > 64 => *found = true,
        serde_json::Value::Array(a) => a.iter().for_each(|x| has_large_array(x, found)),
        serde_json::Value::Object(m) => m.values().for_each(|x| has_large_array(x, found)),
        _ => {}
    }
}

// ---------- VS-001: normal capture → verified commit ----------

#[test]
fn vs_d_normal_commit_verified() {
    let root = tmp_dir("normal");
    let (mut c, lanes, epoch) = controller_with(1);
    let lane = lanes[0];
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        1920,
        16,
    );
    let (area, report) = record_close_commit(&mut c, &root, session_id, &cfgs, req, epoch);

    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.sources_committed, 1);
    assert_eq!(report.takes_committed, 1);
    assert_eq!(report.clips_committed, 1);
    assert_eq!(report.recoverable.len(), 0);

    let p = c.live();
    let session = &p.recording.sessions[&session_id];
    assert_eq!(session.state, SessionState::Finalized);

    // 1 asset + 1 source(Committed) + 1 pass + 1 segment + 1 take + 1 clip。
    assert_eq!(p.assets.len(), 1);
    assert_eq!(p.sources.len(), 1);
    let source = p.sources.values().next().unwrap();
    assert_eq!(source.state, SourceState::Committed);
    let asset = &p.assets[&source.asset_id];
    assert_eq!(asset.digest, source.content_digest);

    assert_eq!(p.recording.passes.len(), 1);
    assert_eq!(p.recording.segments.len(), 1);
    let seg = p.recording.segments.values().next().unwrap();
    assert_eq!(
        seg.session_range.start().value(),
        0,
        "normal capture starts at session 0"
    );
    // 1 Track・全 Frame が staging media と一致（digest = finalized ref）。
    let fin = load_finalized(&area);
    let r = fin[0].finalized.as_ref().unwrap();
    assert_eq!(r.digest, source.content_digest);
    assert_eq!(
        seg.source_range.end().value() - seg.source_range.start().value(),
        r.source_frames.cast_signed()
    );
    let media_bytes = fs::read(area.media_path(0)).unwrap();
    assert_eq!(media_bytes.len() as u64, r.media_byte_len);

    let take = p.recording.takes.values().next().unwrap();
    assert_eq!(take.state, TakeState::Verified);
    assert_eq!(take.segment_ids.len(), 1);
    let clip = p.clips.values().next().unwrap();
    assert_eq!(clip.lane_id, lane);
    assert_eq!(clip.source_id, source.id);
    assert_eq!(lane_clips(&c, lane), 1);
    // clip placement は segment の project window と一致。
    assert_eq!(clip_musical_span(clip), (0, 1920));
    assert_eq!(clip.playback.source_region.start().value(), 0);

    // applied_commits registry が identity を記録。
    assert_eq!(
        commit_status(p, session_id, &report.content_hash),
        CommitStatus::AlreadyApplied
    );
    let marker = read_commit_marker(&area).expect("marker");
    assert_eq!(marker.outcome, "applied");
}

// ---------- VS-002: 2ch × 2 track same clock ----------

#[test]
fn vs_d_two_track_two_ch_clock() {
    let root = tmp_dir("two-track");
    let (mut c, lanes, epoch) = controller_with(2);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, report) = record_close_commit(&mut c, &root, session_id, &cfgs, req, epoch);

    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.sources_committed, 2);
    let p = c.live();
    assert_eq!(p.sources.len(), 2);
    assert_eq!(p.recording.takes.len(), 2);
    assert_eq!(p.clips.len(), 2);
    // 2 stream が同一 clock epoch を共有（clock evidence 整合）。
    let epochs: Vec<_> = p
        .recording
        .streams
        .values()
        .map(|s| s.clock_epoch_id)
        .collect();
    assert!(epochs.iter().all(|e| *e == epoch));
    // 各 lane に clip が 1 つ（初回空 lane → 自動 clip）。
    for &lane in &lanes {
        assert_eq!(lane_clips(&c, lane), 1);
    }
    // media は両 stream で同一 digest（同一 mock input・同一 epoch）。
    let fins = load_finalized(&area);
    assert_eq!(fins.len(), 2);
    let digests: Vec<_> = p.sources.values().map(|s| &s.content_digest).collect();
    assert!(
        digests
            .iter()
            .all(|d| **d == fins[0].finalized.as_ref().unwrap().digest
                || **d == fins[1].finalized.as_ref().unwrap().digest)
    );
}

// ---------- VS-003: punch mid-block → exact placement ----------

#[test]
fn vs_d_punch_placement_exact() {
    let root = tmp_dir("punch");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    // block 途中に punch window が跨ぐ（chunk_frames=40_000 境界に
    // 一致しない tick/frames 位置）。
    let req = request(
        &lanes,
        epoch,
        GateSpec::Punch {
            punch_in: MusicalTick::new(7681),
            punch_out: MusicalTick::new(11521),
        },
        15_360,
        16,
    );
    let (area, report) = record_close_commit(&mut c, &root, session_id, &cfgs, req, epoch);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));

    let fin = load_finalized(&area);
    let r = fin[0].finalized.as_ref().unwrap();
    let p = c.live();
    // punch window 以外の frame は保存されない（保存長と placement 一致）。
    let stored: i64 = r.spans.iter().map(|s| s.source_end - s.source_start).sum();
    assert_eq!(stored, r.source_frames.cast_signed());
    assert!(stored > 0);
    for clip in p.clips.values() {
        let seg = p
            .recording
            .segments
            .values()
            .find(|s| s.source_id == clip.source_id)
            .unwrap();
        // clip は punch window の project tick と正確一致。
        assert_eq!(clip_musical_span(clip).0, seg_proj_in(&area, seg.id));
        assert_eq!(
            clip.playback.source_region.start(),
            seg.source_range.start()
        );
    }
    // punch は session の途中 → session_start > 0。
    let seg = p.recording.segments.values().next().unwrap();
    assert!(seg.session_range.start().value() > 0);
}

fn seg_proj_in(area: &StagingArea, seg_id: CaptureSegmentId) -> i64 {
    read_commit_manifest(area)
        .unwrap()
        .unwrap()
        .payload
        .streams
        .iter()
        .flat_map(|s| &s.segments)
        .find(|g| g.id == seg_id.to_tagged())
        .unwrap()
        .project_in
}

// ---------- VS-004: 3-pass loop ----------

#[test]
fn vs_d_loop_three_passes() {
    let root = tmp_dir("loop");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Loop {
            loop_start: MusicalTick::ZERO,
            loop_end: MusicalTick::new(15_360),
        },
        46_080, // 3 passes
        16,
    );
    let (_area, report) = record_close_commit(&mut c, &root, session_id, &cfgs, req, epoch);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));

    let p = c.live();
    assert_eq!(report.takes_committed, 3, "one take per loop pass");
    assert_eq!(p.recording.passes.len(), 3);
    assert_eq!(p.recording.takes.len(), 3);
    // session frame は単調増加（pass ごとに前進）。
    let mut session_bounds: Vec<i64> = p
        .recording
        .segments
        .values()
        .map(|s| s.session_range.end().value())
        .collect();
    session_bounds.sort_unstable();
    assert!(session_bounds.windows(2).all(|w| w[0] < w[1]));
    // 自動 clip は最低 pass の take のみ（comp 暗黙置換なし）。
    assert_eq!(report.clips_committed, 1);
    assert_eq!(p.clips.len(), 1);
    let clip = p.clips.values().next().unwrap();
    assert_eq!(
        clip_musical_span(clip),
        (0, 15_360),
        "clip covers pass-0 loop window"
    );
    // take → pass の mapping が各 pass に対応。
    let pass_indexes: std::collections::BTreeSet<u32> = p
        .recording
        .takes
        .values()
        .map(|t| p.recording.passes[&t.pass_id].index)
        .collect();
    assert_eq!(pass_indexes, [0, 1, 2].into_iter().collect());
}

// ---------- VS-005: punch-in-loop multi-segment ----------

#[test]
fn vs_d_punch_in_loop_segments() {
    let root = tmp_dir("punch-loop");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::PunchInLoop {
            loop_start: MusicalTick::ZERO,
            loop_end: MusicalTick::new(15_360),
            punch_in: MusicalTick::new(3_841),
            punch_out: MusicalTick::new(11_519),
        },
        30_720, // 2 passes
        16,
    );
    let (area, report) = record_close_commit(&mut c, &root, session_id, &cfgs, req, epoch);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));

    let p = c.live();
    assert_eq!(report.takes_committed, 2);
    assert_eq!(p.recording.passes.len(), 2);
    // 各 pass の segment は punch window のみ（source range 連続で
    // punch 区間長と一致）。
    let fin = load_finalized(&area);
    let r = fin[0].finalized.as_ref().unwrap();
    let expected_per_pass: i64 = r
        .spans
        .iter()
        .filter(|s| s.pass_index == 0)
        .map(|s| s.source_end - s.source_start)
        .sum();
    for take in p.recording.takes.values() {
        let take_frames: i64 = take
            .segment_ids
            .iter()
            .map(|id| {
                let s = &p.recording.segments[id];
                s.source_range.end().value() - s.source_range.start().value()
            })
            .sum();
        assert_eq!(take_frames, expected_per_pass);
    }
    // 全 segment の source frame が punch 区間内に収まる。
    for seg in p.recording.segments.values() {
        assert!(seg.source_range.end().value() <= r.source_frames.cast_signed());
    }
}

// ---------- VS-006: queue overflow → recoverable, not committed ----------

#[test]
fn vs_d_queue_overflow_not_committed() {
    let root = tmp_dir("queue-overflow");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let area = StagingArea::create(
        &root,
        &StagingSessionConfig {
            session_id,
            sample_rate: RATE,
            streams: cfgs.clone(),
        },
    )
    .unwrap();
    // filled_capacity=2 で writer drain なしに feed_engine_only →
    // FAULT_QUEUE_FULL を誘発。
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        3840,
        2,
    );
    let plan = preflight(c.live(), req, RATE).unwrap();
    let a = area.clone();
    let cfgs2 = cfgs.clone();
    let mut sc: Scenario<StreamWriter> = Scenario::build_with(plan, move |i, ends, fault| {
        StreamWriter::create(&a, cfgs2[i as usize].clone(), ends, fault).unwrap()
    });
    let mut input = MockInput::new(epoch, 0);
    for _ in 0..32 {
        let block = input.block(40_000);
        sc.feed_engine_only(&block);
        if sc.fault.is_faulted() {
            break;
        }
    }
    assert!(
        sc.fault.bits() & FAULT_QUEUE_FULL != 0,
        "queue overflow must latch FAULT_QUEUE_FULL (bits={:#x})",
        sc.fault.bits()
    );
    // driver は latch を観測して session スコープの fault を durable
    // に記録する（writer 個別失敗と分離する層）。
    area.record_capture_fault(&format!("bits={:#06x}", sc.fault.bits()))
        .unwrap();
    // engine fault 後に drain → finalize は Partial/欠落として現れる。
    let tag = session_id.to_tagged();
    let writers: Vec<&mut StreamWriter> = sc.rigs.iter_mut().map(|r| &mut r.writer).collect();
    let _ = finish_all(&tag, RATE, writers);
    rop(&mut c, RecordingOp::CloseSession { session_id });
    let report = commit_session(&mut c, &area).expect("commit_session");

    // 部分適用なし: faulted stream は committed へ昇格せず
    // recoverable へ分類される。
    assert!(matches!(
        report.outcome,
        CommitOutcome::NothingCommitted | CommitOutcome::Applied { .. }
    ));
    assert_eq!(report.recoverable.len(), 1);
    let p = c.live();
    assert_eq!(p.sources.len(), 0, "faulted media must not commit");
    assert_eq!(p.recording.takes.len(), 0);
    assert_eq!(p.clips.len(), 0);
}

// ---------- VS-007: writer error → partial recovery handoff ----------

#[test]
fn vs_d_writer_error_recoverable() {
    let root = tmp_dir("writer-error");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let area = StagingArea::create(
        &root,
        &StagingSessionConfig {
            session_id,
            sample_rate: RATE,
            streams: cfgs.clone(),
        },
    )
    .unwrap();
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        3840,
        16,
    );
    let plan = preflight(c.live(), req, RATE).unwrap();
    let a = area.clone();
    let cfgs2 = cfgs.clone();
    let mut sc: Scenario<StreamWriter> = Scenario::build_with(plan, move |i, ends, fault| {
        StreamWriter::create(&a, cfgs2[i as usize].clone(), ends, fault).unwrap()
    });
    let mut input = MockInput::new(epoch, 0);
    // 1 block 分は正常書込み → checkpoint、以後 write 失敗を注入。
    let block = input.block(40_000);
    sc.feed(&block);
    sc.rigs[0].writer.checkpoint().unwrap();
    sc.rigs[0]
        .writer
        .inject_media(Inject::FailWrite { call: u64::MAX / 2 });
    sc.rigs[0]
        .writer
        .inject_media(Inject::ByteLimit { limit: 500_000 });
    sc.run_to_end(&mut input, 40_000);

    let tag = session_id.to_tagged();
    let writers: Vec<&mut StreamWriter> = sc.rigs.iter_mut().map(|r| &mut r.writer).collect();
    let _ = finish_all(&tag, RATE, writers);
    rop(&mut c, RecordingOp::CloseSession { session_id });
    let report = commit_session(&mut c, &area).expect("commit_session");

    // Writer error 後も確認済 media は recoverable として報告され、
    // committed へ自動昇格しない。
    assert_eq!(report.recoverable.len(), 1);
    assert_eq!(
        report.recoverable[0].reason.split(':').next().unwrap(),
        "partial-finalize"
    );
    assert_eq!(c.live().sources.len(), 0);
    assert!(area.media_path(0).exists(), "verified media preserved");
}

// ---------- VS-008: journal truncation → last checkpoint ----------

#[test]
fn vs_d_journal_truncation_recovery() {
    let root = tmp_dir("journal-trunc");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        1920,
        16,
    );
    let (area, fin) = record_and_finalize(&mut c, &root, session_id, &cfgs, req, epoch, 40_000);
    assert!(fin.refs[0].1.is_ok());

    // journal 末尾の Finalize record を torn write 相当に削る
    // （末尾数 byte を落とす）。最後の valid checkpoint までしか
    // 戻らないことを recover_finalize で確認する。journal 前半の
    // checkpoint は残るため Partial frontier として復帰する。
    let jpath = area.journal_path(0);
    let len = fs::metadata(&jpath).unwrap().len();
    let trunc = fs::read(&jpath).unwrap();
    fs::write(&jpath, &trunc[..usize::try_from(len).unwrap() - 8]).unwrap();
    // finalized.json が既にある場合、recovery は AlreadyFinalized。
    // 削除して recover_finalize を走らせ、frontier が戻ることを確認。
    let fpath = area.finalized_path(0);
    let _ = fs::remove_file(&fpath);
    let results = recovery::recover_finalize(&area);
    let outcome = &results[0].1;
    match outcome {
        Ok(r) => assert_eq!(r.outcome, FinalizedOutcome::Partial),
        Err(e) => panic!("recover_finalize failed: {e}"),
    }
    rop(&mut c, RecordingOp::CloseSession { session_id });
    let report = commit_session(&mut c, &area).expect("commit_session");
    assert_eq!(report.recoverable.len(), 1);
    assert_eq!(c.live().sources.len(), 0, "partial media not committed");
}

// ---------- VS-009: crash before commit → retry applies ----------

#[test]
fn vs_d_crash_before_commit_retry() {
    let root = tmp_dir("crash-before");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, fin) = record_and_finalize(&mut c, &root, session_id, &cfgs, req, epoch, 40_000);
    assert!(fin.refs[0].1.is_ok());
    rop(&mut c, RecordingOp::CloseSession { session_id });

    // manifest を書いて「crash」（driver state を捨てる＝実プロセス
    // 再起動は manifest ファイルだけが残る状態と同型）。
    let prepared = prepare_manifest(&mut c, &area).expect("prepare");
    let manifest_before = read_commit_manifest(&area).unwrap().unwrap();
    assert_eq!(prepared.manifest, manifest_before);
    assert!(read_commit_marker(&area).is_none());
    let live_rev_before = c.live_revision();

    // 再起動相当: manifest が残っているので ID 再採番せず commit 成功。
    let report = commit_session(&mut c, &area).expect("commit retry");
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    let manifest_after = read_commit_manifest(&area).unwrap().unwrap();
    assert_eq!(manifest_after, manifest_before, "manifest must be reused");
    assert_eq!(
        c.live_revision().0,
        live_rev_before.0 + 1,
        "exactly one new revision"
    );
    assert_eq!(c.live().sources.len(), 1);
}

// ---------- VS-010: crash after commit → AlreadyApplied, no dup ----------

#[test]
fn vs_d_crash_after_commit_retry_no_dup() {
    let root = tmp_dir("crash-after");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, report) = record_close_commit(&mut c, &root, session_id, &cfgs, req, epoch);
    let CommitOutcome::Applied { revision: rev } = report.outcome else {
        panic!("expected applied")
    };
    let entity_counts = (
        c.live().assets.len(),
        c.live().sources.len(),
        c.live().recording.passes.len(),
        c.live().recording.segments.len(),
        c.live().recording.takes.len(),
        c.live().clips.len(),
    );

    // marker 未記録 crash を再現（project 側は結果あり）。
    fs::remove_file(commit_marker_path(&area)).unwrap();
    let report2 = commit_session(&mut c, &area).expect("retry");
    assert!(matches!(report2.outcome, CommitOutcome::AlreadyApplied));
    let now = (
        c.live().assets.len(),
        c.live().sources.len(),
        c.live().recording.passes.len(),
        c.live().recording.segments.len(),
        c.live().recording.takes.len(),
        c.live().clips.len(),
    );
    assert_eq!(now, entity_counts, "no duplicate registration");
    assert_eq!(
        c.live_revision().0,
        rev,
        "no new revision for AlreadyApplied"
    );
    let marker = read_commit_marker(&area).expect("marker rewritten");
    assert_eq!(marker.outcome, "already-applied");

    // save→reload 経路でも同一: registry 永続化で AlreadyApplied。
    let mut store = DirectoryBundleStore::create(root.join("bundle")).unwrap();
    c.save(&mut store).unwrap();
    let mut c2 = ProjectController::new(
        Project::new(ProjectId::new(0), "x"),
        Box::new(CounterIdGenerator::new(1)),
    );
    c2.load(&store).unwrap();
    fs::remove_file(commit_marker_path(&area)).unwrap();
    let report3 = commit_session(&mut c2, &area).expect("retry after reload");
    assert!(matches!(report3.outcome, CommitOutcome::AlreadyApplied));
}

// ---------- VS-011: undo/redo restores ids, media untouched ----------

#[test]
fn vs_d_undo_redo_restores_ids() {
    let root = tmp_dir("undo-redo");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, _) = record_close_commit(&mut c, &root, session_id, &cfgs, req, epoch);

    let takes: Vec<TakeId> = c.live().recording.takes.keys().copied().collect();
    let clips: Vec<ClipId> = c.live().clips.keys().copied().collect();
    let sources: Vec<SourceId> = c.live().sources.keys().copied().collect();
    assert!(!takes.is_empty() && !clips.is_empty() && !sources.is_empty());

    // undo = 1 session = 1 undo 単位（commit は単一 command）。
    assert!(c.undo().unwrap());
    {
        let p = c.live();
        assert_eq!(p.sources.len(), 0);
        assert_eq!(p.recording.takes.len(), 0);
        assert_eq!(p.clips.len(), 0);
        assert_eq!(p.recording.passes.len(), 0);
        assert_eq!(p.recording.segments.len(), 0);
        assert!(!p.recording.applied_commits.contains_key(&session_id));
        assert_eq!(
            p.recording.sessions[&session_id].state,
            SessionState::Closed,
            "undo restores pre-commit session state"
        );
    }
    // staging/finalized media・manifest・marker は削除されない。
    assert!(area.media_path(0).exists());
    assert!(area.finalized_path(0).exists());
    assert!(commit_manifest_path(&area).exists());

    // redo で同一 Stable ID・mapping を復元。
    assert!(c.redo().unwrap());
    let p = c.live();
    let takes2: Vec<TakeId> = p.recording.takes.keys().copied().collect();
    let clips2: Vec<ClipId> = p.clips.keys().copied().collect();
    let sources2: Vec<SourceId> = p.sources.keys().copied().collect();
    assert_eq!(takes, takes2);
    assert_eq!(clips, clips2);
    assert_eq!(sources, sources2);
    assert_eq!(
        p.recording.sessions[&session_id].state,
        SessionState::Finalized
    );
    assert!(p.recording.applied_commits.contains_key(&session_id));
}

// ---------- VS-012: save/reload integrity ----------

#[test]
fn vs_d_save_reload_integrity() {
    let root = tmp_dir("save-reload");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, _) = record_close_commit(&mut c, &root, session_id, &cfgs, req, epoch);

    let mut store = DirectoryBundleStore::create(root.join("bundle")).unwrap();
    c.save(&mut store).unwrap();
    let mut c2 = ProjectController::new(
        Project::new(ProjectId::new(0), "x"),
        Box::new(CounterIdGenerator::new(1)),
    );
    c2.load(&store).unwrap();

    let before = serde_json::to_value(c.live()).unwrap();
    let after = serde_json::to_value(c2.live()).unwrap();
    assert_eq!(before, after, "project snapshot roundtrip");
    // session/source/take/clip/placement + registry が永続化。
    let p = c2.live();
    assert_eq!(
        p.recording.sessions[&session_id].state,
        SessionState::Finalized
    );
    assert_eq!(p.sources.len(), 1);
    assert_eq!(p.recording.takes.len(), 1);
    assert_eq!(p.clips.len(), 1);
    assert!(p.recording.applied_commits.contains_key(&session_id));
    // reload 後の再試行は AlreadyApplied（重複登録なし）。
    let report = commit_session(&mut c2, &area).expect("retry after reload");
    assert!(matches!(report.outcome, CommitOutcome::AlreadyApplied));
}

// ---------- VS-013: clock epoch discontinuity → no fake continuity ----------

#[test]
fn vs_d_epoch_discontinuity_not_committed() {
    let root = tmp_dir("epoch-disc");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let area = StagingArea::create(
        &root,
        &StagingSessionConfig {
            session_id,
            sample_rate: RATE,
            streams: cfgs.clone(),
        },
    )
    .unwrap();
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        3840,
        16,
    );
    let plan = preflight(c.live(), req, RATE).unwrap();
    let a = area.clone();
    let cfgs2 = cfgs.clone();
    let mut sc: Scenario<StreamWriter> = Scenario::build_with(plan, move |i, ends, fault| {
        StreamWriter::create(&a, cfgs2[i as usize].clone(), ends, fault).unwrap()
    });
    let mut input = MockInput::new(epoch, 0);
    let block = input.block(40_000);
    sc.feed(&block);
    // epoch 断絶: 次 block で別 epoch を差す → fail-stop。
    input.set_epoch(ClockEpochId::new(9999));
    let block = input.block(40_000);
    sc.feed(&block);
    assert!(
        sc.fault.bits() & FAULT_EPOCH_DISCONTINUITY != 0,
        "epoch discontinuity must latch (bits={:#x})",
        sc.fault.bits()
    );
    area.record_capture_fault(&format!("bits={:#06x}", sc.fault.bits()))
        .unwrap();
    let tag = session_id.to_tagged();
    let writers: Vec<&mut StreamWriter> = sc.rigs.iter_mut().map(|r| &mut r.writer).collect();
    let _ = finish_all(&tag, RATE, writers);
    rop(&mut c, RecordingOp::CloseSession { session_id });
    let report = commit_session(&mut c, &area).expect("commit_session");

    // 断絶後の stream は「連続録音」として commit されず、
    // recoverable として分離される。
    assert_eq!(report.recoverable.len(), 1);
    assert_eq!(c.live().sources.len(), 0);
    assert_eq!(c.live().recording.takes.len(), 0);
}

// ---------- VS-014: revision conflict → no partial apply, retry ok ----------

#[test]
fn vs_d_revision_conflict_recoverable() {
    let root = tmp_dir("rev-conflict");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, fin) = record_and_finalize(&mut c, &root, session_id, &cfgs, req, epoch, 40_000);
    assert!(fin.refs[0].1.is_ok());
    rop(&mut c, RecordingOp::CloseSession { session_id });
    let prepared = prepare_manifest(&mut c, &area).expect("prepare");

    // stale expected_revision で直接 op 適用 → StaleRevision・部分適用なし。
    let stale = ProjectRevision(c.live_revision().0 - 1);
    let err = c
        .apply(Command {
            expected_revision: stale,
            op: SynvelisOp::Recording(RecordingOp::CommitRecording {
                manifest: Box::new(prepared.manifest.clone()),
            }),
        })
        .unwrap_err();
    assert!(matches!(err, Error::StaleRevision { .. }));
    assert_eq!(
        c.live().sources.len(),
        0,
        "no partial apply on stale revision"
    );
    assert_eq!(c.live().recording.takes.len(), 0);

    // driver の commit_session は最新 revision で retry し成功する。
    let report = commit_session(&mut c, &area).expect("commit");
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(c.live().sources.len(), 1);

    // commit 後の再適用は session.state == Finalized で拒否される
    // （partial apply なし・entities は成功時のまま）。
    let rev = c.live_revision();
    let err2 = c
        .apply(Command {
            expected_revision: rev,
            op: SynvelisOp::Recording(RecordingOp::CommitRecording {
                manifest: Box::new(prepared.manifest),
            }),
        })
        .unwrap_err();
    assert!(matches!(err2, Error::IllegalTransition(_)));
    assert_eq!(
        c.live().sources.len(),
        1,
        "duplicate apply rejected cleanly"
    );
}

// ---------- integrity conflict: same identity + different content ----------

#[test]
fn vs_d_integrity_conflict_same_identity() {
    let root = tmp_dir("integrity");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, _) = record_close_commit(&mut c, &root, session_id, &cfgs, req, epoch);
    let counts = c.live().sources.len();

    // manifest を書き換え（同一 session・異内容）→ prepare が検出。
    // payload digest を改ざんし identity.content_hash は据え置くので
    // payload 再計算 hash と identity hash が不整合になる。
    let mut manifest = read_commit_manifest(&area).unwrap().unwrap();
    manifest.payload.streams[0].digest = "tampered".to_string();
    fs::write(
        commit_manifest_path(&area),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();

    // prepare が hash 不整合を検出する（manifest payload hash ≠
    // identity.content_hash）。
    let err = commit_session(&mut c, &area).unwrap_err();
    assert!(matches!(err, CommitError::Manifest(_)));

    // registry 経由の IntegrityConflict: 同一 session に対し異なる
    // content_hash で到達した commit は Conflict 分類になる。
    let status = commit_status(c.live(), session_id, "ffffffffffffffff");
    assert_eq!(status, CommitStatus::IntegrityConflict);
    assert_eq!(
        c.live().sources.len(),
        counts,
        "conflict leaves project unchanged"
    );
}

// ---------- VS-015: commit path is non-RT + PCM never on command ----------

#[test]
fn vs_d_commit_path_non_rt() {
    let root = tmp_dir("non-rt");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, _) = record_close_commit(&mut c, &root, session_id, &cfgs, req, epoch);

    // commit manifest / command は PCM を運ばない: manifest JSON に
    // sample 配列や base64 ブロックは存在しない。
    let manifest_text = fs::read_to_string(commit_manifest_path(&area)).unwrap();
    assert!(!manifest_text.contains("samples"));
    let manifest = read_commit_manifest(&area).unwrap().unwrap();
    let json = serde_json::to_value(&manifest).unwrap();
    let mut found_pcm = false;
    has_large_array(&json, &mut found_pcm);
    assert!(!found_pcm, "manifest must carry references, not pcm");

    // commit 後も staging media は実 byte を持つ（command が media を
    // 抱え込んでいないことの対偶確認）。
    let media_len = fs::metadata(area.media_path(0)).unwrap().len();
    assert!(media_len > 0);
}

// ---------- review fixes: registry authority / stale manifest / lifecycle ----------

#[test]
fn vs_d_retry_after_staging_drift_is_already_applied() {
    // commit 済み session で staging refs が消失・改変されても、
    // `applied_commits` registry が authority として AlreadyApplied
    // を返す（finalize ref 再構成に依存しない）。
    let root = tmp_dir("drift-retry");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, report) = record_close_commit(&mut c, &root, session_id, &cfgs, req, epoch);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    let fin = load_finalized(&area);
    let digest = fin[0].finalized.as_ref().unwrap().digest.clone();

    // staging drift: finalized ref・media を消す。media は project
    // 登録済み source が digest を持つため復元不能でも semantic に
    // commit 済みのまま扱う（registry 正本）。
    fs::remove_file(area.finalized_path(0)).unwrap();
    fs::remove_file(area.media_path(0)).unwrap();
    let report2 = commit_session(&mut c, &area).expect("retry after drift");
    assert!(matches!(report2.outcome, CommitOutcome::AlreadyApplied));
    assert_eq!(c.live().sources.len(), 1);
    let marker = read_commit_marker(&area).expect("marker");
    assert_eq!(marker.outcome, "already-applied");
    assert_eq!(marker.stream_digests, vec![digest]);
}

#[test]
fn vs_d_commit_after_finalize_session_op() {
    // FinalizeSession で Finalized 化済み・registry 未登録の session
    // も commit 可能（commit 専用遷移に lifecycle を縛らない）。
    let root = tmp_dir("fin-op-commit");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, fin) = record_and_finalize(&mut c, &root, session_id, &cfgs, req, epoch, 40_000);
    assert!(fin.refs[0].1.is_ok());
    rop(&mut c, RecordingOp::CloseSession { session_id });
    rop(&mut c, RecordingOp::FinalizeSession { session_id });
    assert_eq!(
        c.live().recording.sessions[&session_id].state,
        SessionState::Finalized
    );

    let report = commit_session(&mut c, &area).expect("commit after FinalizeSession");
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(c.live().sources.len(), 1);
    assert_eq!(c.live().recording.takes.len(), 1);
}

#[test]
fn vs_d_stale_manifest_rebuild_after_lane_change() {
    // manifest 作成後に lane が非空化した場合、旧 manifest は
    // implicit comp 置換になるため破棄し、clip 無しで再構築して
    // commit する（reserved ID は未適用なので安全に再採番）。
    let root = tmp_dir("stale-lane");
    let (mut c, lanes, epoch) = controller_with(1);
    let lane = lanes[0];
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, fin) = record_and_finalize(&mut c, &root, session_id, &cfgs, req, epoch, 40_000);
    assert!(fin.refs[0].1.is_ok());
    rop(&mut c, RecordingOp::CloseSession { session_id });
    let prepared = prepare_manifest(&mut c, &area).expect("prepare");
    assert!(
        !prepared.manifest.payload.streams[0].clips.is_empty(),
        "initial manifest carries auto clip"
    );

    // 別経路で lane に clip を置く → manifest が stale 化。
    apply(
        &mut c,
        ProjectOp::RegisterAsset {
            digest: "ext".into(),
        },
    );
    let asset = *c.live().assets.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "ext".into(),
        },
    );
    let source = *c.live().sources.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(0)),
                extent: ClipExtent::Musical(MusicalDuration::new(480).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(480 * 50).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    assert_eq!(lane_clips(&c, lane), 1);

    let report = commit_session(&mut c, &area).expect("commit rebuilds manifest");
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.sources_committed, 1);
    assert_eq!(
        report.clips_committed, 0,
        "rebuilt manifest drops auto clips on non-empty lane"
    );
    assert_eq!(c.live().recording.takes.len(), 1);
    assert_eq!(lane_clips(&c, lane), 1, "existing comp untouched");
}

#[test]
fn vs_d_recommit_after_undo_recovers() {
    // undo は commit 前 snapshot（registry 記録・session state・
    // reserved ID 全て）を復元する。manifest は残るので再 commit は
    // 同内容で再適用できる（再採番・再適用いずれも受理）。
    let root = tmp_dir("undo-recommit");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, report) = record_close_commit(&mut c, &root, session_id, &cfgs, req, epoch);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));

    assert!(c.undo().unwrap());
    assert_eq!(c.live().sources.len(), 0);
    assert!(!c.live().recording.applied_commits.contains_key(&session_id));

    let report2 = commit_session(&mut c, &area).expect("re-commit after undo");
    assert!(matches!(report2.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(c.live().sources.len(), 1);
    assert_eq!(c.live().recording.takes.len(), 1);
    assert!(c.live().recording.applied_commits.contains_key(&session_id));
}

#[test]
fn vs_d_validate_rejects_intra_manifest_dup_pass_index() {
    // manifest 内の pass index 重複は検査で拒否する（既存 session
    // との衝突検査だけでは incoming payload 内の重複を捕捉できない）。
    let root = tmp_dir("dup-pass-index");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, fin) = record_and_finalize(&mut c, &root, session_id, &cfgs, req, epoch, 40_000);
    assert!(fin.refs[0].1.is_ok());
    rop(&mut c, RecordingOp::CloseSession { session_id });
    let prepared = prepare_manifest(&mut c, &area).expect("prepare");

    let mut m = prepared.manifest;
    let mut dup = m.payload.passes[0].clone();
    dup.id = RecordingPassId::new(u128::MAX).to_tagged();
    m.payload.passes.push(dup);
    m.identity.content_hash = payload_hash(&m.payload).unwrap();
    let err = validate_commit(c.live(), &m).unwrap_err();
    assert!(
        matches!(err, Error::Validation(_)),
        "duplicate pass index must be rejected: {err:?}"
    );
}

#[test]
fn vs_d_validate_rejects_negative_ranges() {
    // 負の frame 値は validate で拒否する（apply 層の Negative 検査に
    // 到達する前に total validation で捕捉）。
    let root = tmp_dir("neg-range");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = request(
        &lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        960,
        16,
    );
    let (area, fin) = record_and_finalize(&mut c, &root, session_id, &cfgs, req, epoch, 40_000);
    assert!(fin.refs[0].1.is_ok());
    rop(&mut c, RecordingOp::CloseSession { session_id });
    let prepared = prepare_manifest(&mut c, &area).expect("prepare");

    // 各負値が個別に拒否される（部分適用なし）。
    let reject = |mut m: RecordingCommitManifest, f: &dyn Fn(&mut RecordingCommitManifest)| {
        f(&mut m);
        m.identity.content_hash = payload_hash(&m.payload).unwrap();
        validate_commit(c.live(), &m).unwrap_err()
    };
    let err = reject(prepared.manifest.clone(), &|m| {
        m.payload.streams[0].segments[0].source_start = -5;
    });
    assert!(matches!(err, Error::Validation(_)), "{err:?}");
    let err = reject(prepared.manifest.clone(), &|m| {
        m.payload.streams[0].segments[0].session_start = -5;
    });
    assert!(matches!(err, Error::Validation(_)), "{err:?}");
    let err = reject(prepared.manifest.clone(), &|m| {
        m.payload.streams[0].clips[0].source_offset = -1;
    });
    assert!(matches!(err, Error::Validation(_)), "{err:?}");
}
