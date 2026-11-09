//! VS-C (E2E-R2): Staging Writer / Journal / Crash Recovery の受け入れ検証。
//!
//! Raw f32 LE PCM + フレーム化 append-only journal + 明示 checkpoint の
//! 意味論を、決定的な Mock 経路と fault 注入で検算する。Captured →
//! Queued → Written → Validated → Durable Checkpoint → Finalized を
//! 混同せず、破損・未検証末尾が Finalized へ混入しないことを確認する。

use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

use synvelis_project::*;
use synvelis_recording::*;

const RATE: u32 = 48_000;
const CHANNELS: u32 = 2;
const FRAME_BYTES: u64 = 8; // 2ch * f32

// ---------- helpers ----------

fn default_project() -> (Project, LaneId, LaneId, ClockEpochId) {
    let lane1 = LaneId::new(11);
    let lane2 = LaneId::new(12);
    let epoch = ClockEpochId::new(22);
    let mut p = Project::new(ProjectId::new(0), "vs-c");
    let track = TrackId::new(1);
    p.tracks.insert(
        track,
        Track {
            id: track,
            name: "t".into(),
            lane_ids: vec![lane1, lane2],
        },
    );
    for lane in [lane1, lane2] {
        p.lanes.insert(
            lane,
            Lane {
                id: lane,
                track_id: track,
                clip_ids: vec![],
            },
        );
    }
    p.clock_epochs.insert(
        epoch,
        ClockEpoch {
            id: epoch,
            description: "mock".into(),
        },
    );
    (p, lane1, lane2, epoch)
}

fn request(
    lanes: &[LaneId],
    epoch: ClockEpochId,
    gate: GateSpec,
    stop_tick: i64,
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
            chunks: 8,
            chunk_frames: 512,
            spans_per_chunk: 8,
            filled_capacity: 8,
            lookahead_spans: 16,
        },
        max_block_frames: 512,
    }
}

fn session_id(n: u128) -> RecordingSessionId {
    RecordingSessionId::new(n)
}

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("synvelis-vsc-{}-{name}", std::process::id()));
    if d.exists() {
        fs::remove_dir_all(&d).unwrap();
    }
    fs::create_dir_all(&d).unwrap();
    d
}

fn stream_cfgs(lanes: &[LaneId]) -> Vec<StagingStreamConfig> {
    lanes
        .iter()
        .enumerate()
        .map(|(i, &lane)| {
            stream_config(
                u16::try_from(i).unwrap(),
                CaptureStreamId::new(100 + i as u128),
                lane,
                CHANNELS,
            )
        })
        .collect()
}

fn staging_area(root: &Path, session: u128, lanes: &[LaneId]) -> StagingArea {
    StagingArea::create(
        root,
        &StagingSessionConfig {
            session_id: session_id(session),
            sample_rate: RATE,
            streams: stream_cfgs(lanes),
        },
    )
    .unwrap()
}

fn session_dir(root: &Path, session: u128) -> PathBuf {
    root.join(session_id(session).to_tagged())
}

fn media_path(root: &Path, session: u128, i: u16) -> PathBuf {
    session_dir(root, session).join(format!("streams/{}.pcm", stream_stem(i)))
}

fn journal_path(root: &Path, session: u128, i: u16) -> PathBuf {
    session_dir(root, session).join(format!("streams/{}.journal", stream_stem(i)))
}

fn read_journal(root: &Path, session: u128, i: u16) -> Vec<u8> {
    fs::read(journal_path(root, session, i)).unwrap()
}

fn scenario_writer(
    p: &Project,
    req: RecordingRequest,
    area: &StagingArea,
) -> Scenario<StreamWriter> {
    let plan = preflight(p, req, RATE).expect("preflight must pass");
    let cfgs: Vec<StagingStreamConfig> = area
        .manifest
        .streams
        .iter()
        .map(|m| StagingStreamConfig {
            stream_index: m.stream_index,
            capture_stream_id: m.capture_stream_id.clone(),
            lane_id: m.lane_id.clone(),
            channels: m.channels,
        })
        .collect();
    let a = area.clone();
    Scenario::build_with(plan, move |i, ends, fault| {
        StreamWriter::create(&a, cfgs[i as usize].clone(), ends, fault).unwrap()
    })
}

fn checkpoint_all_rigs(sc: &mut Scenario<StreamWriter>) {
    for rig in &mut sc.rigs {
        rig.writer.checkpoint().unwrap();
    }
}

fn finish(sc: &mut Scenario<StreamWriter>, session: u128) -> FinishReport {
    let tag = session_id(session).to_tagged();
    let writers: Vec<&mut StreamWriter> = sc.rigs.iter_mut().map(|r| &mut r.writer).collect();
    finish_all(&tag, RATE, writers)
}

/// journal 上の chunk record 列が media 実 byte（CRC）と `MockPcm` 期待値に
/// 一致するか検算する。戻り値は (chunk record 数, media 被覆 byte 長)。
fn verify_journal_against_media(
    root: &Path,
    session: u128,
    stream: u16,
    seed: u32,
) -> (usize, u64) {
    let pcm = MockPcm { seed };
    let media_bytes = fs::read(media_path(root, session, stream)).unwrap();
    let media = media_bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect::<Vec<_>>();
    let outcome = replay_journal(&read_journal(root, session, stream));
    assert!(
        matches!(outcome.stop, StopReason::CleanEnd | StopReason::TornTail),
        "journal corrupted: {:?}",
        outcome.stop
    );
    let mut chunk_count = 0usize;
    let mut covered = 0u64;
    for rec in &outcome.records {
        let RecordPayload::Chunk(c) = &rec.payload else {
            continue;
        };
        chunk_count += 1;
        let start = usize::try_from(c.media_offset).unwrap();
        let end = start + usize::try_from(c.media_len).unwrap();
        assert_eq!(
            crc32(&media_bytes[start..end]),
            c.pcm_crc32,
            "seq {}",
            rec.seq
        );
        covered = covered.max(c.media_offset + c.media_len);
        for s in &c.spans {
            let m_off = usize::try_from(s.media_offset).unwrap() / 4;
            let input_base = c
                .first_input_frame
                .wrapping_add(u64::try_from(s.session_start - c.session_start).unwrap_or(0));
            for f in 0..s.frame_count {
                let input_frame = input_base.wrapping_add(u64::from(f));
                for ch in 0..CHANNELS {
                    let actual = media[m_off + f as usize * CHANNELS as usize + ch as usize];
                    assert_eq!(
                        actual,
                        pcm.sample(input_frame, ch),
                        "sample mismatch seq {} span {:?} f {f} ch {ch}",
                        rec.seq,
                        s
                    );
                }
            }
        }
    }
    (chunk_count, covered)
}

/// 手組みの有効 chunk（単一 span・source=session=input 連続）。
fn valid_chunk(id: u32, stream_index: u16, start: i64, frames: u32, seed: u32) -> CaptureChunk {
    let pcm = MockPcm { seed };
    let mut c = CaptureChunk::new(id, stream_index, frames.max(64), CHANNELS, 4);
    c.header = CaptureBlockHeader {
        clock_epoch: ClockEpochId::new(22),
        session_start: SessionFrame::new(start).unwrap(),
        first_input_frame: u64::try_from(start).unwrap(),
        block_frames: frames,
        has_capture: true,
    };
    c.spans
        .push(CaptureSpan {
            pass_index: 0,
            session_start: SessionFrame::new(start).unwrap(),
            frame_count: frames,
            packed_pcm_offset: 0,
            source_start: SourceFrame::new(start).unwrap(),
            project_in: MusicalTick::new(0),
            project_out: MusicalTick::new(0),
        })
        .unwrap();
    c.stored_frames = frames;
    c.stored_samples = frames as usize * CHANNELS as usize;
    for f in 0..frames {
        for ch in 0..CHANNELS {
            c.pcm[f as usize * CHANNELS as usize + ch as usize] = pcm.sample(u64::from(f), ch);
        }
    }
    c
}

fn manual_writer(area: &StagingArea) -> (StreamWriter, EngineEnds) {
    let (mut engine_ends, writer_ends) = build_handoff(0, 512, CHANNELS, 4, 8, 4);
    // pool chunk を drain して free queue を空にし、chunk 返却を成功させる。
    while engine_ends.free_rx.try_pop().is_ok() {}
    let fault = FaultLatch::new();
    let m = &area.manifest.streams[0];
    let cfg = StagingStreamConfig {
        stream_index: m.stream_index,
        capture_stream_id: m.capture_stream_id.clone(),
        lane_id: m.lane_id.clone(),
        channels: m.channels,
    };
    let paths = stream_paths(area, 0);
    let media = FaultWriter::new(OpenOptions::new().append(true).open(&paths.media).unwrap());
    let journal = FaultWriter::new(
        OpenOptions::new()
            .append(true)
            .open(&paths.journal)
            .unwrap(),
    );
    (
        StreamWriter::with_writers(
            cfg,
            paths,
            media,
            journal,
            writer_ends,
            fault,
            area.manifest.capabilities.clone(),
        ),
        engine_ends,
    )
}

fn chunk_rec(offset: u64, len: u64) -> ChunkRecord {
    ChunkRecord {
        chunk_id: 0,
        first_input_frame: 0,
        session_start: 0,
        block_frames: 32,
        stored_frames: 32,
        media_offset: offset,
        media_len: len,
        pcm_crc32: crc32(&vec![0u8; usize::try_from(len).unwrap()]),
        digest_fold: 0,
        spans: vec![],
    }
}

fn cp_rec(index: u64, media_len: u64, frames: u64, digest: u64) -> CheckpointRecord {
    CheckpointRecord {
        checkpoint_index: index,
        media_len,
        source_frames: frames,
        digest_fold: digest,
        chunk_records: 0,
    }
}

// ---------- 1. layout / manifest ----------

#[test]
fn vsc_staging_layout_manifest() {
    let (_p, l1, l2, _e) = default_project();
    let root = tmp_dir("layout");
    let area = staging_area(&root, 7, &[l1, l2]);
    let dir = session_dir(&root, 7);
    assert!(dir.join("manifest.json").exists());
    assert!(dir.join("streams/stream-0000.pcm").exists());
    assert!(dir.join("streams/stream-0001.journal").exists());
    assert_eq!(area.manifest.sample_rate, RATE);
    assert_eq!(area.manifest.streams.len(), 2);
    // capability は実環境観測として記録される（値は環境依存）。
    assert!(matches!(
        area.manifest.capabilities.directory_sync,
        DirSyncCapability::Supported | DirSyncCapability::Unsupported
    ));
    let reopened = StagingArea::open(&root, session_id(7)).unwrap();
    assert_eq!(reopened.manifest, area.manifest);
    assert!(matches!(
        StagingArea::open(&root, session_id(999)),
        Err(StagingError::NotFound(_))
    ));
    fs::remove_dir_all(&root).ok();
}

// ---------- 2. normal: 単一 stream / bit-exact ----------

#[test]
fn vsc_normal_stereo_pcm_bit_exact() {
    let (p, lane, _l2, epoch) = default_project();
    let root = tmp_dir("normal");
    let area = staging_area(&root, 8, &[lane]);
    let mut sc = scenario_writer(
        &p,
        request(
            &[lane],
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            1920,
        ),
        &area,
    );
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 256);
    checkpoint_all_rigs(&mut sc);

    assert!(!sc.fault.is_faulted(), "fault: {:x}", sc.fault.bits());
    let outcome = replay_journal(&read_journal(&root, 8, 0));
    assert_eq!(outcome.stop, StopReason::CleanEnd);
    // seq は 0 起点連番、先頭は chunk record。
    for (i, r) in outcome.records.iter().enumerate() {
        assert_eq!(r.seq, i as u64);
    }
    assert!(matches!(
        outcome.records[0].payload,
        RecordPayload::Chunk(_)
    ));
    assert!(
        outcome
            .records
            .iter()
            .any(|r| matches!(r.payload, RecordPayload::Checkpoint(_)))
    );
    // PCM bit-exact + 48000 frame * 8 byte。
    let (chunks, covered) = verify_journal_against_media(&root, 8, 0, 0);
    assert!(chunks > 0);
    assert_eq!(covered, 48_000 * FRAME_BYTES);
    // finalize → Complete ref。
    let report = finish(&mut sc, 8);
    let Ok(ref s) = report.refs[0].1 else {
        panic!("finalize failed: {:?}", report.refs[0].1)
    };
    assert_eq!(s.outcome, FinalizedOutcome::Complete);
    assert_eq!(s.source_frames, 48_000);
    assert_eq!(s.media_byte_len, 48_000 * FRAME_BYTES);
    assert_eq!(s.media, "streams/stream-0000.pcm");
    let fin = session_dir(&root, 8).join("streams/stream-0000.finalized.json");
    assert!(fin.exists());
    let parsed: FinalizedSourceRef = serde_json::from_slice(&fs::read(&fin).unwrap()).unwrap();
    assert_eq!(parsed.source_frames, 48_000);
    fs::remove_dir_all(&root).ok();
}

// ---------- 3. multi-stream / punch loop ----------

#[test]
fn vsc_multi_stream_all_ranges() {
    let (p, l1, l2, epoch) = default_project();
    let root = tmp_dir("multi");
    let area = staging_area(&root, 9, &[l1, l2]);
    let mut sc = scenario_writer(
        &p,
        request(
            &[l1, l2],
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            960,
        ),
        &area,
    );
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 256);
    checkpoint_all_rigs(&mut sc);
    assert!(!sc.fault.is_faulted());
    for i in 0..2u16 {
        let (chunks, covered) = verify_journal_against_media(&root, 9, i, u32::from(i));
        assert!(chunks > 0, "stream {i}");
        assert_eq!(covered, 24_000 * FRAME_BYTES, "stream {i}");
    }
    let report = finish(&mut sc, 9);
    assert_eq!(report.refs.len(), 2);
    for (_i, r) in &report.refs {
        assert!(
            matches!(r, Ok(s) if s.outcome == FinalizedOutcome::Complete),
            "{r:?}"
        );
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn vsc_punch_loop_spans_match() {
    let (p, lane, _l2, epoch) = default_project();
    let root = tmp_dir("punch");
    let area = staging_area(&root, 10, &[lane]);
    // punch [480, 960) loop=2 → source 2 pass。
    let mut sc = scenario_writer(
        &p,
        request(
            &[lane],
            epoch,
            GateSpec::PunchInLoop {
                loop_start: MusicalTick::ZERO,
                loop_end: MusicalTick::new(960),
                punch_in: MusicalTick::new(480),
                punch_out: MusicalTick::new(960),
            },
            1920,
        ),
        &area,
    );
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 256);
    checkpoint_all_rigs(&mut sc);
    assert!(!sc.fault.is_faulted());
    let outcome = replay_journal(&read_journal(&root, 10, 0));
    let passes: std::collections::BTreeSet<u32> = outcome
        .records
        .iter()
        .flat_map(|r| match &r.payload {
            RecordPayload::Chunk(c) => c.spans.iter().map(|s| s.pass_index).collect::<Vec<_>>(),
            _ => vec![],
        })
        .collect();
    assert_eq!(passes, [0, 1].into_iter().collect());
    verify_journal_against_media(&root, 10, 0, 0);
    let report = finish(&mut sc, 10);
    assert!(matches!(report.refs[0].1, Ok(ref s) if s.outcome == FinalizedOutcome::Complete));
    fs::remove_dir_all(&root).ok();
}

// ---------- 4. fault 注入 ----------

#[test]
fn vsc_partial_write_stream_failed() {
    let (_p, l1, _l2, _e) = default_project();
    let root = tmp_dir("partial");
    let area = staging_area(&root, 11, &[l1]);
    let (mut w, mut ends) = manual_writer(&area);
    assert!(ends.filled_tx.try_push(valid_chunk(0, 0, 0, 64, 0)).is_ok());
    w.pump();
    assert_eq!(w.state(), StreamState::Active);
    // 2 回目の media write（call=1）を失敗させる。
    w.inject_media(Inject::FailWrite { call: 1 });
    assert!(
        ends.filled_tx
            .try_push(valid_chunk(1, 0, 64, 64, 0))
            .is_ok()
    );
    w.pump();
    assert_eq!(w.state(), StreamState::Failed);
    assert_ne!(w.fault().bits() & FAULT_WRITER_ERROR, 0);
    // 失敗 chunk は保持（free 返却しない）。
    assert_eq!(w.held().len(), 1);
    // 成功分の journal record・media byte は残る。
    let outcome = replay_journal(&read_journal(&root, 11, 0));
    assert!(
        outcome
            .records
            .iter()
            .any(|r| matches!(r.payload, RecordPayload::Chunk(_)))
    );
    assert_eq!(
        fs::metadata(media_path(&root, 11, 0)).unwrap().len(),
        64 * FRAME_BYTES
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn vsc_failed_chunk_preserves_validated_prefix() {
    // 回帰: chunk0 を checkpoint まで確定 → chunk1 の media write が
    // 失敗しても、chunk0 の journaled frontier は失われず Partial で
    // finalize できる。失敗 chunk の cursor/digest が checkpoint に
    // 混入する実装では frontier 不整合（Err(Corrupt)）になる。
    let (_p, l1, _l2, _e) = default_project();
    let root = tmp_dir("prefix");
    let area = staging_area(&root, 118, &[l1]);
    let (mut w, mut ends) = manual_writer(&area);
    assert!(ends.filled_tx.try_push(valid_chunk(0, 0, 0, 64, 0)).is_ok());
    w.pump();
    w.checkpoint().unwrap();
    assert_eq!(w.durable().unwrap().media_len, 64 * FRAME_BYTES);
    w.inject_media(Inject::FailWrite { call: 1 });
    assert!(
        ends.filled_tx
            .try_push(valid_chunk(1, 0, 64, 64, 0))
            .is_ok()
    );
    w.pump();
    assert_eq!(w.state(), StreamState::Failed);
    // finalize は chunk0 の frontier を再検証して Partial を返す。
    let r = w.finalize(&session_id(118).to_tagged(), RATE).unwrap();
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    assert_eq!(r.source_frames, 64);
    assert_eq!(r.media_byte_len, 64 * FRAME_BYTES);
    assert_eq!(r.spans.len(), 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn vsc_failed_journal_append_preserves_prefix() {
    // journal append 失敗: media byte は書かれたが chunk record は
    // 無い → unverified tail として保持され、frontier は進まない。
    let (_p, l1, _l2, _e) = default_project();
    let root = tmp_dir("jfail");
    let area = staging_area(&root, 119, &[l1]);
    let (mut w, mut ends) = manual_writer(&area);
    assert!(ends.filled_tx.try_push(valid_chunk(0, 0, 0, 64, 0)).is_ok());
    w.pump();
    w.checkpoint().unwrap();
    // journal write 呼出し: chunk0 record(0), checkpoint record(1) を
    // 消費済み → chunk1 の record append が call=2。
    w.inject_journal(Inject::FailWrite { call: 2 });
    assert!(
        ends.filled_tx
            .try_push(valid_chunk(1, 0, 64, 64, 0))
            .is_ok()
    );
    w.pump();
    assert_eq!(w.state(), StreamState::Failed);
    // media は 2 chunk 分あるが journaled は 1 分のみ。
    assert_eq!(
        fs::metadata(media_path(&root, 119, 0)).unwrap().len(),
        128 * FRAME_BYTES
    );
    let r = w.finalize(&session_id(119).to_tagged(), RATE).unwrap();
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    assert_eq!(r.source_frames, 64);
    assert_eq!(r.media_byte_len, 64 * FRAME_BYTES);
    // unverified tail は fault として記録され、削除はしない。
    let report = recovery::scan(&root).unwrap();
    let s = &report.sessions[0].streams[0];
    assert_eq!(s.unverified_tail_bytes, 64 * FRAME_BYTES);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn vsc_disk_full_byte_limit() {
    let (_p, l1, _l2, _e) = default_project();
    let root = tmp_dir("diskfull");
    let area = staging_area(&root, 111, &[l1]);
    let (mut w, mut ends) = manual_writer(&area);
    // 累積 600 byte 以後の media write を失敗（途中 byte は残る）。
    w.inject_media(Inject::ByteLimit { limit: 600 });
    for i in 0..3u32 {
        assert!(
            ends.filled_tx
                .try_push(valid_chunk(i, 0, i64::from(i) * 64, 64, 0))
                .is_ok()
        );
        w.pump();
        if w.state() == StreamState::Failed {
            break;
        }
    }
    assert_eq!(w.state(), StreamState::Failed);
    assert_ne!(w.fault().bits() & FAULT_WRITER_ERROR, 0);
    // 成功した 1 chunk 分は journal に残る（Validated prefix 保護）。
    let outcome = replay_journal(&read_journal(&root, 111, 0));
    assert!(
        outcome
            .records
            .iter()
            .any(|r| matches!(r.payload, RecordPayload::Chunk(_)))
    );
    // 残った media byte は消えない（unverified tail として保持）。
    assert!(fs::metadata(media_path(&root, 111, 0)).unwrap().len() >= 64 * FRAME_BYTES);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn vsc_sync_failed_distinct_from_write() {
    let (_p, l1, _l2, _e) = default_project();
    let root = tmp_dir("syncfail");
    let area = staging_area(&root, 12, &[l1]);
    let (mut w, mut ends) = manual_writer(&area);
    assert!(ends.filled_tx.try_push(valid_chunk(0, 0, 0, 64, 0)).is_ok());
    w.pump();
    // journal.sync_data（checkpoint 内の最初の sync）を失敗。
    w.inject_journal(Inject::FailSync { call: 0 });
    let err = w.checkpoint().unwrap_err();
    assert!(matches!(err, WriterError::SyncFailed { .. }), "{err:?}");
    // sync 失敗 → durable 不成立（write 成功 ≠ durable）。
    assert!(w.durable().is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn vsc_journal_torn_tail_and_checksum_and_gap() {
    // codec level: torn tail。
    let mut bytes = Vec::new();
    bytes.extend(encode_record(0, &RecordPayload::Chunk(chunk_rec(0, 100))).unwrap());
    bytes.extend(encode_record(1, &RecordPayload::Checkpoint(cp_rec(0, 100, 10, 7))).unwrap());
    let next = encode_record(2, &RecordPayload::Chunk(chunk_rec(100, 50))).unwrap();
    bytes.extend(&next[..next.len() / 2]);
    let outcome = replay_journal(&bytes);
    assert_eq!(outcome.stop, StopReason::TornTail);
    assert_eq!(outcome.records.len(), 2);
    // checksum 破損（record 内 byte を反転）。
    let mut bad = encode_record(0, &RecordPayload::Chunk(chunk_rec(0, 100))).unwrap();
    bad[20] ^= 0xFF;
    let outcome = replay_journal(&bad);
    assert_eq!(outcome.stop, StopReason::BadChecksum);
    assert_eq!(outcome.records.len(), 0);
    // seq gap。
    let mut gap = encode_record(0, &RecordPayload::Chunk(chunk_rec(0, 100))).unwrap();
    gap.extend(encode_record(2, &RecordPayload::Chunk(chunk_rec(100, 50))).unwrap());
    let outcome = replay_journal(&gap);
    assert_eq!(outcome.stop, StopReason::SequenceGap);
    assert_eq!(outcome.records.len(), 1);

    // file level: 書き込んだ journal の末尾を途中で切る。
    let (_p, l1, _l2, _e) = default_project();
    let root = tmp_dir("tornfile");
    let area = staging_area(&root, 13, &[l1]);
    let (mut w, mut ends) = manual_writer(&area);
    for i in 0..3u32 {
        assert!(
            ends.filled_tx
                .try_push(valid_chunk(i, 0, i64::from(i) * 64, 64, 0))
                .is_ok()
        );
    }
    w.pump();
    w.checkpoint().unwrap();
    drop(w);
    let jp = journal_path(&root, 13, 0);
    let mut jbytes = fs::read(&jp).unwrap();
    // torn tail: checkpoint 以後に半分だけ書かれた record（crash で
    // 追記が途切れた状態）。
    let partial = encode_record(4, &RecordPayload::Chunk(chunk_rec(0, 8))).unwrap();
    jbytes.extend(&partial[..partial.len() / 2]);
    fs::write(&jp, &jbytes).unwrap();
    let outcome = replay_journal(&fs::read(&jp).unwrap());
    assert_eq!(outcome.stop, StopReason::TornTail);
    // recover_finalize は Last Valid Checkpoint まで Partial ref を出す。
    let area = StagingArea::open(&root, session_id(13)).unwrap();
    let results = recovery::recover_finalize(&area);
    assert_eq!(results.len(), 1);
    let Ok(s) = &results[0].1 else {
        panic!("recover failed: {:?}", results[0].1)
    };
    assert_eq!(s.outcome, FinalizedOutcome::Partial);
    assert_eq!(s.source_frames, 192);
    fs::remove_dir_all(&root).ok();
}

// ---------- 5. checkpoint / recovery ----------

#[test]
fn vsc_recover_last_checkpoint() {
    let (_p, l1, _l2, _e) = default_project();
    let root = tmp_dir("recover");
    let area = staging_area(&root, 14, &[l1]);
    let (mut w, mut ends) = manual_writer(&area);
    for i in 0..3u32 {
        assert!(
            ends.filled_tx
                .try_push(valid_chunk(i, 0, i64::from(i) * 64, 64, 0))
                .is_ok()
        );
    }
    w.pump();
    w.checkpoint().unwrap();
    // checkpoint 以後の 2 chunk（crash で durable にならない）。
    for i in 3..5u32 {
        assert!(
            ends.filled_tx
                .try_push(valid_chunk(i, 0, i64::from(i) * 64, 64, 0))
                .is_ok()
        );
    }
    w.pump();
    drop(w);
    // scan: durable frontier = checkpoint 時点。
    let report = recovery::scan(&root).unwrap();
    assert_eq!(report.sessions.len(), 1);
    let s = &report.sessions[0].streams[0];
    let d = s.durable.expect("durable frontier");
    assert_eq!(d.media_len, 3 * 64 * FRAME_BYTES);
    assert_eq!(d.source_frames, 192);
    assert_eq!(s.validated_tail_bytes, 2 * 64 * FRAME_BYTES);
    // recover_finalize は Partial ref を frontier まで公開。
    let area = StagingArea::open(&root, session_id(14)).unwrap();
    let results = recovery::recover_finalize(&area);
    let Ok(s) = &results[0].1 else {
        panic!("{:?}", results[0].1)
    };
    assert_eq!(s.outcome, FinalizedOutcome::Partial);
    assert_eq!(s.source_frames, 192);
    assert_eq!(s.media_byte_len, d.media_len);
    assert_eq!(s.spans.len(), 3);
    assert!(
        s.spans
            .iter()
            .all(|sp| sp.media_offset + sp.media_len <= d.media_len)
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn vsc_unverified_tail_never_finalized() {
    let (_p, l1, _l2, _e) = default_project();
    let root = tmp_dir("tail");
    let area = staging_area(&root, 15, &[l1]);
    let (mut w, mut ends) = manual_writer(&area);
    assert!(ends.filled_tx.try_push(valid_chunk(0, 0, 0, 64, 0)).is_ok());
    w.pump();
    // journal record の無い byte を media 末尾へ直接追加（unverified tail）。
    {
        let mut m = OpenOptions::new()
            .append(true)
            .open(media_path(&root, 15, 0))
            .unwrap();
        std::io::Write::write_all(&mut m, &[0xAB; 512]).unwrap();
        m.sync_data().unwrap();
    }
    drop(w);
    let report = recovery::scan(&root).unwrap();
    let s = &report.sessions[0].streams[0];
    assert_eq!(s.unverified_tail_bytes, 512);
    // durable frontier 無し → finalize 不可（Corrupt）。
    let area = StagingArea::open(&root, session_id(15)).unwrap();
    let results = recovery::recover_finalize(&area);
    assert!(matches!(results[0].1, Err(WriterError::Corrupt(_))));
    fs::remove_dir_all(&root).ok();
}

// ---------- 6. multi-stream isolation ----------

#[test]
fn vsc_multi_stream_fault_isolation() {
    let (_p, l1, l2, _e) = default_project();
    let root = tmp_dir("isolation");
    let area = staging_area(&root, 16, &[l1, l2]);
    let shared = FaultLatch::new();
    let (mut ee0, we0) = build_handoff(0, 512, CHANNELS, 4, 8, 4);
    let (mut ee1, we1) = build_handoff(1, 512, CHANNELS, 4, 8, 4);
    let to_cfg = |i: usize| {
        let m = &area.manifest.streams[i];
        StagingStreamConfig {
            stream_index: m.stream_index,
            capture_stream_id: m.capture_stream_id.clone(),
            lane_id: m.lane_id.clone(),
            channels: m.channels,
        }
    };
    let mut w0 = StreamWriter::create(&area, to_cfg(0), we0, shared.clone()).unwrap();
    let mut w1 = StreamWriter::create(&area, to_cfg(1), we1, shared.clone()).unwrap();
    assert!(ee0.filled_tx.try_push(valid_chunk(0, 0, 0, 64, 0)).is_ok());
    w0.pump();
    w0.inject_media(Inject::FailWrite { call: 1 });
    assert!(ee0.filled_tx.try_push(valid_chunk(1, 0, 64, 64, 0)).is_ok());
    w0.pump();
    assert_eq!(w0.state(), StreamState::Failed);
    assert_ne!(shared.bits() & FAULT_WRITER_ERROR, 0);
    // stream1 は健全に継続。
    for i in 0..3u32 {
        assert!(
            ee1.filled_tx
                .try_push(valid_chunk(i, 1, i64::from(i) * 64, 64, 1))
                .is_ok()
        );
    }
    w1.pump();
    w1.checkpoint().unwrap();
    let tag = session_id(16).to_tagged();
    let writers: Vec<&mut StreamWriter> = vec![&mut w0, &mut w1];
    let report = finish_all(&tag, RATE, writers);
    // stream0 は Failed だが chunk0 の validated prefix は失われない:
    // finalize は journaled frontier（chunk0 のみ）を検証して Partial
    // ref を公開する。失敗 chunk の cursor/digest は checkpoint に
    // 混入しない（混入すれば frontier 不整合で Err になる → 回帰検出）。
    let Ok(s0) = &report.refs[0].1 else {
        panic!("validated prefix lost: {:?}", report.refs[0].1)
    };
    assert_eq!(s0.outcome, FinalizedOutcome::Partial);
    assert_eq!(s0.source_frames, 64);
    assert_eq!(s0.media_byte_len, 64 * FRAME_BYTES);
    assert_eq!(s0.spans.len(), 1);
    // stream1 は Complete。
    let Ok(s1) = &report.refs[1].1 else {
        panic!("{:?}", report.refs[1].1)
    };
    assert_eq!(s1.outcome, FinalizedOutcome::Complete);
    assert_eq!(s1.source_frames, 192);
    let outcome = replay_journal(&read_journal(&root, 16, 0));
    assert!(
        outcome
            .records
            .iter()
            .any(|r| matches!(r.payload, RecordPayload::Chunk(_)))
    );
    fs::remove_dir_all(&root).ok();
}

// ---------- 7. idempotent ----------

#[test]
fn vsc_finalize_idempotent() {
    let (p, lane, _l2, epoch) = default_project();
    let root = tmp_dir("idem");
    let area = staging_area(&root, 17, &[lane]);
    let mut sc = scenario_writer(
        &p,
        request(
            &[lane],
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            480,
        ),
        &area,
    );
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 256);
    checkpoint_all_rigs(&mut sc);
    let r1 = finish(&mut sc, 17);
    assert!(r1.refs[0].1.is_ok());
    // 2 回目 → AlreadyFinalized。
    let r2 = finish(&mut sc, 17);
    assert!(matches!(r2.refs[0].1, Err(WriterError::AlreadyFinalized)));
    // recovery 経由も AlreadyFinalized（二重発行しない）。
    let area = StagingArea::open(&root, session_id(17)).unwrap();
    let results = recovery::recover_finalize(&area);
    assert!(matches!(results[0].1, Err(WriterError::AlreadyFinalized)));
    // .tmp 残滓無し。
    let entries: Vec<String> = fs::read_dir(session_dir(&root, 17).join("streams"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(entries.iter().any(|n| n == "stream-0000.finalized.json"));
    assert!(!entries.iter().any(|n| {
        std::path::Path::new(n)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("tmp"))
    }));
    fs::remove_dir_all(&root).ok();
}

// ---------- 8. writer stall / queue full ----------

#[test]
fn vsc_writer_stall_queue_full_then_drain() {
    let (p, lane, _l2, epoch) = default_project();
    let root = tmp_dir("stall");
    let area = staging_area(&root, 18, &[lane]);
    let mut sc = scenario_writer(
        &p,
        request(
            &[lane],
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            1920,
        ),
        &area,
    );
    let mut input = MockInput::new(epoch, 0);
    // stall: writer を回さず queue を埋める。
    for _ in 0..64 {
        let block = input.block(512);
        sc.feed_engine_only(&block);
        if sc.fault.is_faulted() {
            break;
        }
    }
    assert_ne!(
        sc.fault.bits() & (FAULT_QUEUE_FULL | FAULT_FREE_EXHAUSTED),
        0,
        "stall did not saturate queues"
    );
    // pump で drain → journal は一貫。
    for rig in &mut sc.rigs {
        rig.writer.pump();
        rig.writer.checkpoint().unwrap();
    }
    let outcome = replay_journal(&read_journal(&root, 18, 0));
    assert_eq!(outcome.stop, StopReason::CleanEnd);
    assert!(
        outcome
            .records
            .iter()
            .any(|r| matches!(r.payload, RecordPayload::Chunk(_)))
    );
    fs::remove_dir_all(&root).ok();
}

// ---------- 9. contract violation ----------

#[test]
fn vsc_contract_violation_source_gap() {
    let (_p, l1, _l2, _e) = default_project();
    let root = tmp_dir("contract");
    let area = staging_area(&root, 19, &[l1]);
    let (mut w, mut ends) = manual_writer(&area);
    assert!(ends.filled_tx.try_push(valid_chunk(0, 0, 0, 64, 0)).is_ok());
    // source 連続性違反: 2 個目の chunk の source_start をずらす。
    let mut bad = valid_chunk(1, 0, 64, 64, 0);
    let mut s = bad.spans.as_slice()[0];
    s.source_start = SourceFrame::new(128).unwrap();
    bad.spans.clear();
    bad.spans.push(s).unwrap();
    assert!(ends.filled_tx.try_push(bad).is_ok());
    w.pump();
    assert_eq!(w.state(), StreamState::Failed);
    assert_ne!(w.fault().bits() & FAULT_CONTRACT, 0);
    // 最初の chunk は journal に残る（validated range 保護）。
    let outcome = replay_journal(&read_journal(&root, 19, 0));
    assert_eq!(
        outcome
            .records
            .iter()
            .filter(|r| matches!(r.payload, RecordPayload::Chunk(_)))
            .count(),
        1
    );
    fs::remove_dir_all(&root).ok();
}
