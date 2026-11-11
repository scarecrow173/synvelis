//! RCD-02 (synvelis#116): Durability Evidence / Recovery Review /
//! Commit Receipt / fault-matrix の受け入れ検証。
//!
//! checkpoint ごとの durability 証拠、scan → 明示 review
//! （Adopt/Hold/Discard・fail-closed 参照チェック）による確定、
//! commit receipt の publish-indeterminate 分類と冪等 reconcile、
//! および kill/short write/sync fail/DirSync/Permission/DiskFull/
//! indeterminate publish の fault 注入を決定的に検算する。
//! fake/mock FS の成功は実 OS/FS durability の証明にしない
//! （実機計測は `NotRun`）。

use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

use synvelis_project::*;
use synvelis_recording::*;

const RATE: u32 = 48_000;
const CHANNELS: u32 = 2;
const FRAME_BYTES: u64 = 8; // 2ch * f32

// ---------- staging helpers ----------

fn session_id(n: u128) -> RecordingSessionId {
    RecordingSessionId::new(n)
}

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("synvelis-rcd02-{}-{name}", std::process::id()));
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

fn finalized_path(root: &Path, session: u128, i: u16) -> PathBuf {
    session_dir(root, session).join(format!("streams/{}.finalized.json", stream_stem(i)))
}

fn review_path(root: &Path, session: u128, i: u16) -> PathBuf {
    session_dir(root, session).join(format!("streams/{}.review.json", stream_stem(i)))
}

/// crash 相当: finalize せず writer を落とした stream を手組みで作る
/// （media/journal は実 file、checkpoint まで durable）。
fn crashed_writer(
    area: &StagingArea,
    caps: Option<StagingCapabilities>,
) -> (StreamWriter, EngineEnds) {
    let (mut engine_ends, writer_ends) = build_handoff(0, 512, CHANNELS, 4, 8, 4);
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
            caps.unwrap_or_else(|| area.manifest.capabilities.clone()),
        ),
        engine_ends,
    )
}

fn manual_writer(area: &StagingArea) -> (StreamWriter, EngineEnds) {
    crashed_writer(area, None)
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

fn push_and_pump(w: &mut StreamWriter, ends: &mut EngineEnds, i: u32, start: i64) {
    assert!(
        ends.filled_tx
            .try_push(valid_chunk(i, 0, start, 64, 0))
            .is_ok()
    );
    w.pump();
}

/// media 末尾へ journal 未記録の byte を足す（unverified tail）。
fn append_unverified_media(path: &Path, bytes: usize) {
    let mut m = OpenOptions::new().append(true).open(path).unwrap();
    std::io::Write::write_all(&mut m, &vec![0xCDu8; bytes]).unwrap();
    m.sync_data().unwrap();
}

/// `path` を `len` へ truncate する。最終 sync frontier 以後に
/// `write_all` が成功した unsynced byte が、power-loss で OS の
/// dirty page と共に失われた状態の再現（file 長・内容ともに
/// frontier へ戻る）。append handle では truncate できないため
/// write handle を別途開く。
fn truncate_to(path: &Path, len: u64) {
    OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_len(len)
        .unwrap();
}

// ---------- review probe stubs ----------

struct ClearProbe;
impl MediaReferenceProbe for ClearProbe {
    fn check(&self, _c: &DiscardCandidate) -> ReferenceCheck {
        ReferenceCheck::Clear
    }
}

struct ReferencedProbe;
impl MediaReferenceProbe for ReferencedProbe {
    fn check(&self, _c: &DiscardCandidate) -> ReferenceCheck {
        ReferenceCheck::Referenced("project source".into())
    }
}

struct UnknownProbe;
impl MediaReferenceProbe for UnknownProbe {
    fn check(&self, _c: &DiscardCandidate) -> ReferenceCheck {
        ReferenceCheck::Unknown("probe cannot reach project".into())
    }
}

fn review_record(root: &Path, session: u128, i: u16) -> Option<ReviewRecord> {
    let path = review_path(root, session, i);
    fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}

// ---------- project/commit helpers ----------

fn controller_with(lane_count: usize) -> (ProjectController, Vec<LaneId>, ClockEpochId) {
    let mut c = ProjectController::new(
        Project::new(ProjectId::new(0), "rcd-02"),
        Box::new(CounterIdGenerator::new(1)),
    );
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::AddTrack { name: "t".into() },
    })
    .unwrap();
    let track = *c.live().tracks.keys().next().unwrap();
    let mut lanes = Vec::new();
    for _ in 0..lane_count {
        c.apply(Command {
            expected_revision: c.live_revision(),
            op: ProjectOp::AddLane { track_id: track },
        })
        .unwrap();
        lanes.push(*c.live().lanes.keys().last().unwrap());
    }
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::RegisterClockEpoch {
            description: "mock".into(),
        },
    })
    .unwrap();
    let epoch = *c.live().clock_epochs.keys().next().unwrap();
    (c, lanes, epoch)
}

fn rop(c: &mut ProjectController, op: RecordingOp) {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: SynvelisOp::Recording(op),
    })
    .unwrap();
}

fn open_session(
    c: &mut ProjectController,
    lanes: &[LaneId],
    epoch: ClockEpochId,
) -> (RecordingSessionId, Vec<StagingStreamConfig>) {
    rop(
        c,
        RecordingOp::CreateSession {
            name: "rcd-02-session".into(),
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

fn record_request(lanes: &[LaneId], epoch: ClockEpochId) -> RecordingRequest {
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
        stop: MusicalTick::new(4096),
        gate: GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        budget: BufferBudget {
            chunks: 16,
            chunk_frames: 40_000,
            spans_per_chunk: 8,
            filled_capacity: 8,
            lookahead_spans: 32,
        },
        max_block_frames: 40_000,
    }
}

/// mock 録音 → staging → checkpoint → finalize → `CloseSession`。
fn record_close(
    c: &mut ProjectController,
    root: &Path,
    session_id: RecordingSessionId,
    cfgs: &[StagingStreamConfig],
    lanes: &[LaneId],
    epoch: ClockEpochId,
) -> StagingArea {
    let area = StagingArea::create(
        root,
        &StagingSessionConfig {
            session_id,
            sample_rate: RATE,
            streams: cfgs.to_vec(),
        },
    )
    .unwrap();
    let req = record_request(lanes, epoch);
    let plan = preflight(c.live(), req, RATE).expect("preflight");
    let a = area.clone();
    let mut sc: Scenario<StreamWriter> = Scenario::build_with(plan, move |i, ends, fault| {
        StreamWriter::create(&a, cfgs[i as usize].clone(), ends, fault).unwrap()
    });
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 40_000);
    for rig in &mut sc.rigs {
        rig.writer.checkpoint().unwrap();
    }
    let tag = session_id.to_tagged();
    let writers: Vec<&mut StreamWriter> = sc.rigs.iter_mut().map(|r| &mut r.writer).collect();
    let report = finish_all(&tag, RATE, writers);
    for (_, r) in &report.refs {
        assert!(r.is_ok(), "finalize failed: {r:?}");
    }
    rop(c, RecordingOp::CloseSession { session_id });
    area
}

// ---------- durability evidence ----------

#[test]
fn rcd02_checkpoint_evidence_records_each_stage() {
    let lane = LaneId::new(11);
    let root = tmp_dir("evidence");
    let area = staging_area(&root, 201, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    push_and_pump(&mut w, &mut ends, 0, 0);

    // journal sync 失敗: media sync 済み・journal sync 未達の
    // evidence が失敗地点まで記録され、frontier は立たない。
    w.inject_journal(Inject::FailSync { call: 0 });
    let err = w.checkpoint().unwrap_err();
    assert!(matches!(err, WriterError::SyncFailed { .. }));
    let ev = w.durability_evidence();
    assert_eq!(ev.len(), 1);
    assert!(ev[0].media_synced);
    assert!(!ev[0].journal_synced);
    assert_eq!(ev[0].dir_sync, DirSyncState::NotAttempted);
    assert!(ev[0].frontier.is_none());
    assert!(w.durable().is_none());

    // fault 除去後の retry は独立した evidence を記録する。
    let ev2 = w.checkpoint().unwrap();
    assert!(ev2.media_synced && ev2.journal_synced);
    assert!(ev2.frontier.is_some());
    assert_eq!(w.durability_evidence().len(), 2);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02_evidence_dir_sync_unsupported_is_honest() {
    let lane = LaneId::new(11);
    let root = tmp_dir("dirsync");
    let area = staging_area(&root, 202, &[lane]);
    let caps = StagingCapabilities {
        atomic_rename: true,
        file_sync: true,
        directory_sync: DirSyncCapability::Unsupported,
    };
    let (mut w, mut ends) = crashed_writer(&area, Some(caps));
    push_and_pump(&mut w, &mut ends, 0, 0);
    let ev = w.checkpoint().unwrap();
    // dir sync 非対応は「試行しなかった」ではなく Unsupported と
    // 観測され、durable 保証を誇張しない。
    assert_eq!(ev.dir_sync, DirSyncState::Unsupported);
    assert!(ev.frontier.is_some());
    assert_eq!(
        ev.fs_capability.directory_sync,
        DirSyncCapability::Unsupported
    );
    fs::remove_dir_all(&root).ok();
}

// ---------- fault matrix ----------

#[test]
fn rcd02_fault_matrix_write_sync_permission_diskfull() {
    // media write permission denied → stream Failed・validated prefix 保持。
    let lane = LaneId::new(11);
    let root = tmp_dir("perm-write");
    let area = staging_area(&root, 203, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    w.inject_media(Inject::PermissionDenied { call: 0 });
    push_and_pump(&mut w, &mut ends, 0, 0);
    assert_eq!(w.state(), StreamState::Failed);
    fs::remove_dir_all(&root).ok();

    // journal sync permission denied → checkpoint SyncFailed・
    // durable 不成立（write 成功 ≠ durable）。
    let root = tmp_dir("perm-sync");
    let area = staging_area(&root, 204, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    push_and_pump(&mut w, &mut ends, 0, 0);
    w.inject_journal(Inject::SyncPermissionDenied { call: 0 });
    let err = w.checkpoint().unwrap_err();
    assert!(matches!(err, WriterError::SyncFailed { .. }));
    assert!(w.durable().is_none());
    fs::remove_dir_all(&root).ok();

    // media short write → 部分 byte は unverified tail として残る。
    let root = tmp_dir("short-write");
    let area = staging_area(&root, 205, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    w.inject_media(Inject::ShortWrite {
        call: 0,
        bytes: 100,
    });
    push_and_pump(&mut w, &mut ends, 0, 0);
    assert_eq!(w.state(), StreamState::Failed);
    drop(w);
    let report = recovery::scan(&root).unwrap();
    let s = &report.sessions[0].streams[0];
    assert!(s.unverified_tail_bytes > 0 || s.class == StreamClass::Partial);
    fs::remove_dir_all(&root).ok();

    // disk full (byte limit) → 失敗までの validated prefix は保護。
    let root = tmp_dir("diskfull");
    let area = staging_area(&root, 206, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    w.inject_media(Inject::ByteLimit { limit: 600 });
    for i in 0..4u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
        if w.state() == StreamState::Failed {
            break;
        }
    }
    assert_eq!(w.state(), StreamState::Failed);
    drop(w);
    // 破損・未検証 byte は削除されず報告される。
    let report = recovery::scan(&root).unwrap();
    let s = &report.sessions[0].streams[0];
    assert!(s.unverified_tail_bytes > 0 || s.validated_tail_bytes > 0 || s.durable.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02_fault_matrix_journal_torn_tail_review_adopt() {
    let lane = LaneId::new(11);
    let root = tmp_dir("torn-review");
    let area = staging_area(&root, 207, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    for i in 0..3u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    w.pump();
    w.checkpoint().unwrap();
    drop(w);
    // journal 末尾に半分だけ書かれた record（kill/power-loss 相当の
    // torn tail）を注入する。
    let jp = journal_path(&root, 207, 0);
    let mut jbytes = fs::read(&jp).unwrap();
    let partial = encode_record(
        4,
        &RecordPayload::Chunk(ChunkRecord {
            chunk_id: 0,
            first_input_frame: 0,
            session_start: 0,
            block_frames: 32,
            stored_frames: 32,
            media_offset: 0,
            media_len: 8,
            pcm_crc32: 0,
            digest_fold: 0,
            spans: vec![],
        }),
    )
    .unwrap();
    jbytes.extend(&partial[..partial.len() / 2]);
    fs::write(&jp, &jbytes).unwrap();

    let report = recovery::scan(&root).unwrap();
    let s = &report.sessions[0].streams[0];
    assert_eq!(s.stop, StopReason::TornTail);
    assert_eq!(s.review_state, ReviewState::Pending);
    // review Adopt は verified prefix のみを Partial で確定する。
    let area = StagingArea::open(&root, session_id(207)).unwrap();
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults).unwrap();
    let ReviewOutcome::Adopted(r) = out else {
        panic!("expected Adopted")
    };
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    assert_eq!(r.source_frames, 192);
    fs::remove_dir_all(&root).ok();
}

// ---------- recovery review ----------

#[test]
fn rcd02_review_adopt_verified_prefix_and_idempotent() {
    let lane = LaneId::new(11);
    let root = tmp_dir("review-adopt");
    let area = staging_area(&root, 208, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    // checkpoint までの 3 chunk が durable、以後 2 chunk は
    // journaled だが frontier 外（validated tail）。
    for i in 0..3u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    w.pump();
    w.checkpoint().unwrap();
    for i in 3..5u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    drop(w);

    let report = recovery::scan(&root).unwrap();
    let s = &report.sessions[0].streams[0];
    assert_eq!(s.review_state, ReviewState::Pending);
    assert_eq!(s.validated_tail_bytes, 2 * 64 * FRAME_BYTES);

    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults).unwrap();
    let ReviewOutcome::Adopted(r) = out else {
        panic!("expected Adopted")
    };
    // verified prefix のみ確定 — tail は Complete に昇格しない。
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    assert_eq!(r.media_byte_len, 3 * 64 * FRAME_BYTES);
    assert_eq!(r.source_frames, 192);
    assert_eq!(r.spans.len(), 3);
    assert!(finalized_path(&root, 208, 0).exists());
    let rec = review_record(&root, 208, 0).expect("review record");
    assert_eq!(rec.decision, ReviewDecision::Adopted);
    assert_eq!(rec.finalized_digest.as_deref(), Some(r.digest.as_str()));

    // 再 scan は Finalized、再 Adopt は同一 ref を冪等に返す
    // （finalized 重複発行・digest 変化なし）。
    let report = recovery::scan(&root).unwrap();
    assert_eq!(
        report.sessions[0].streams[0].review_state,
        ReviewState::Finalized
    );
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults).unwrap();
    let ReviewOutcome::Adopted(r2) = out else {
        panic!("expected Adopted")
    };
    assert_eq!(r2.digest, r.digest);
    assert_eq!(r2.media_byte_len, r.media_byte_len);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02_review_hold_persists_without_touching_files() {
    let lane = LaneId::new(11);
    let root = tmp_dir("review-hold");
    let area = staging_area(&root, 209, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    push_and_pump(&mut w, &mut ends, 0, 0);
    w.pump();
    w.checkpoint().unwrap();
    drop(w);
    let media_before = fs::read(media_path(&root, 209, 0)).unwrap();

    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Hold, None, &mut faults).unwrap();
    assert_eq!(out, ReviewOutcome::Held);
    assert_eq!(fs::read(media_path(&root, 209, 0)).unwrap(), media_before);
    assert!(!finalized_path(&root, 209, 0).exists());
    let rec = review_record(&root, 209, 0).expect("review record");
    assert_eq!(rec.decision, ReviewDecision::Held);

    let report = recovery::scan(&root).unwrap();
    assert_eq!(
        report.sessions[0].streams[0].review_state,
        ReviewState::Held
    );
    // 再 Hold は冪等。
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Hold, None, &mut faults).unwrap();
    assert_eq!(out, ReviewOutcome::Held);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02_review_discard_fail_closed_and_idempotent() {
    let lane = LaneId::new(11);
    let root = tmp_dir("review-discard");
    let area = staging_area(&root, 210, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    push_and_pump(&mut w, &mut ends, 0, 0);
    w.pump();
    w.checkpoint().unwrap();
    drop(w);

    // probe 未指定 → 拒否（fail-closed）。
    let mut faults = PublishFaults::new();
    let err = review_stream(&area, 0, ReviewAction::Discard, None, &mut faults).unwrap_err();
    assert!(matches!(err, ReviewError::Refused(_)));
    assert!(media_path(&root, 210, 0).exists());

    // Referenced → 拒否。参照中 media を消さない。
    let mut faults = PublishFaults::new();
    let err = review_stream(
        &area,
        0,
        ReviewAction::Discard,
        Some(&ReferencedProbe),
        &mut faults,
    )
    .unwrap_err();
    assert!(matches!(err, ReviewError::Refused(_)));
    assert!(media_path(&root, 210, 0).exists());

    // Unknown → 拒否（判定不能を Clear にしない）。
    let mut faults = PublishFaults::new();
    let err = review_stream(
        &area,
        0,
        ReviewAction::Discard,
        Some(&UnknownProbe),
        &mut faults,
    )
    .unwrap_err();
    assert!(matches!(err, ReviewError::Refused(_)));
    assert!(media_path(&root, 210, 0).exists());

    // Clear → media/journal を削除し Discarded を記録。
    let mut faults = PublishFaults::new();
    let out = review_stream(
        &area,
        0,
        ReviewAction::Discard,
        Some(&ClearProbe),
        &mut faults,
    )
    .unwrap();
    assert_eq!(out, ReviewOutcome::Discarded);
    assert!(!media_path(&root, 210, 0).exists());
    assert!(!journal_path(&root, 210, 0).exists());
    let rec = review_record(&root, 210, 0).expect("review record");
    assert_eq!(rec.decision, ReviewDecision::Discarded);

    // 再 Discard は probe 無しでも冪等に成功（副作用の重複なし）。
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Discard, None, &mut faults).unwrap();
    assert_eq!(out, ReviewOutcome::Discarded);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02_review_record_unreadable_is_fail_closed() {
    // 判定記録が存在するが読めない（ここでは dir を置いて read を
    // 失敗させる）場合、Pending に潰さず Unreadable として
    // fail-closed にし、review 操作を拒否する。
    let lane = LaneId::new(11);
    let root = tmp_dir("review-unreadable");
    let area = staging_area(&root, 213, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    push_and_pump(&mut w, &mut ends, 0, 0);
    w.pump();
    w.checkpoint().unwrap();
    drop(w);
    fs::create_dir(review_path(&root, 213, 0)).unwrap();

    let report = recovery::scan(&root).unwrap();
    assert_eq!(
        report.sessions[0].streams[0].review_state,
        ReviewState::Unreadable
    );
    let mut faults = PublishFaults::new();
    for action in [
        ReviewAction::Adopt,
        ReviewAction::Hold,
        ReviewAction::Discard,
    ] {
        let err = review_stream(&area, 0, action, Some(&ClearProbe), &mut faults).unwrap_err();
        assert!(matches!(err, ReviewError::Corrupt(_)), "{action:?}: {err}");
    }
    // media/journal は変更されない。
    assert!(media_path(&root, 213, 0).exists());
    assert!(journal_path(&root, 213, 0).exists());
    fs::remove_dir_all(&root).ok();
}

// ---------- complete 強化 ----------

#[test]
fn rcd02_finalize_record_complete_with_tail_downgrades_to_partial() {
    // journal の Finalize record が Complete を主張しても、
    // unverified tail が残る限り recovery は Partial に降格する
    // （unverified tail を Complete にしない）。
    let lane = LaneId::new(11);
    let root = tmp_dir("complete-tail");
    let area = staging_area(&root, 211, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    for i in 0..2u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    w.pump();
    w.checkpoint().unwrap();
    // writer 経路の finalize は Complete を記録する。
    let r = w.finalize(&session_id(211).to_tagged(), RATE).unwrap();
    assert_eq!(r.outcome, FinalizedOutcome::Complete);
    drop(w);
    // finalized marker が失われ、media 末尾に未検証 byte が残った
    // 状態を再現する（crash での marker 喪失 + 未同期 tail）。
    fs::remove_file(finalized_path(&root, 211, 0)).unwrap();
    append_unverified_media(&media_path(&root, 211, 0), 512);

    let area = StagingArea::open(&root, session_id(211)).unwrap();
    let results = recovery::recover_finalize(&area);
    let Ok(s) = &results[0].1 else {
        panic!("recover failed: {:?}", results[0].1)
    };
    // Complete record + CleanEnd でも tail があれば Partial。
    assert_eq!(s.outcome, FinalizedOutcome::Partial);
    assert_eq!(s.media_byte_len, r.media_byte_len);
    fs::remove_dir_all(&root).ok();
}

// ---------- commit receipt / indeterminate publish ----------

#[test]
fn rcd02_commit_receipt_lifecycle_and_reconcile() {
    let (mut c, lanes, epoch) = controller_with(1);
    let (sid, cfgs) = open_session(&mut c, &lanes, epoch);
    let root = tmp_dir("receipt");
    let area = record_close(&mut c, &root, sid, &cfgs, &lanes, epoch);

    // manifest 不在 → Empty。
    let r = commit_receipt(c.live(), &area, None).unwrap();
    assert_eq!(r.stage, ReceiptStage::Empty);

    // manifest 確保済み・未適用 → Reserved。
    prepare_manifest(&mut c, &area).unwrap();
    let r = commit_receipt(c.live(), &area, None).unwrap();
    assert_eq!(r.stage, ReceiptStage::Reserved);
    assert!(r.content_hash.is_some());
    assert!(
        r.journal_finalize
            .iter()
            .all(|e| e.finalized_present && e.finalize_record.is_some())
    );

    // commit 適用 + marker 公開 → CommittedConfirmed。
    let report = commit_session(&mut c, &area).unwrap();
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    let r = commit_receipt(c.live(), &area, Some(c.live_revision())).unwrap();
    assert_eq!(r.stage, ReceiptStage::CommittedConfirmed);
    assert_eq!(r.marker_outcome.as_deref(), Some("applied"));
    assert_eq!(r.project_saved, Some(true));
    // save 未了（commit revision より前）なら project_saved=false。
    let committed_rev = r.committed_revision.unwrap();
    let r = commit_receipt(c.live(), &area, Some(ProjectRevision(committed_rev - 1))).unwrap();
    assert_eq!(r.project_saved, Some(false));

    // marker 喪失 → registry 適用済み + marker 不在 =
    // PublishIndeterminate（commit 済みを未 commit と誤認しない）。
    fs::remove_file(commit_marker_path(&area)).unwrap();
    let r = commit_receipt(c.live(), &area, None).unwrap();
    assert_eq!(r.stage, ReceiptStage::PublishIndeterminate);
    assert!(r.marker_outcome.is_none());

    // reconcile が marker を冪等に再公開 → CommittedConfirmed。
    let r = reconcile_commit_receipt(&c, &area, None).unwrap();
    assert_eq!(r.stage, ReceiptStage::CommittedConfirmed);
    assert_eq!(r.marker_outcome.as_deref(), Some("already-applied"));

    // commit_session 再実行も AlreadyApplied で二重登録しない
    // （別 Start 後の再起動相当: applied_commits が authority）。
    let sources_before = c.live().sources.len();
    let takes_before = c.live().recording.takes.len();
    let clips_before = c.live().clips.len();
    let report = commit_session(&mut c, &area).unwrap();
    assert_eq!(report.outcome, CommitOutcome::AlreadyApplied);
    assert_eq!(c.live().sources.len(), sources_before);
    assert_eq!(c.live().recording.takes.len(), takes_before);
    assert_eq!(c.live().clips.len(), clips_before);
    let r = commit_receipt(c.live(), &area, None).unwrap();
    assert_eq!(r.stage, ReceiptStage::CommittedConfirmed);
    fs::remove_dir_all(&root).ok();
}

// ---------- publish fault injection ----------

#[test]
fn rcd02_publish_indeterminate_adopt_recovers() {
    // finalized.json publish が rename 成功後に応答喪失
    // （Indeterminate）を起こしても、再試行は既存 ref を冪等に
    // 採用して review record を完成させる。
    let lane = LaneId::new(11);
    let root = tmp_dir("pub-indet");
    let area = staging_area(&root, 212, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    for i in 0..2u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    w.pump();
    w.checkpoint().unwrap();
    drop(w);

    // 1 回目の faultable publish（finalized.json）で indeterminate。
    let mut faults = PublishFaults::new();
    faults.inject(PublishInject::Indeterminate { call: 0 });
    let err = review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults).unwrap_err();
    assert!(matches!(err, ReviewError::Writer(_)));
    // file は公開済み（rename 成功）だが呼出し側は失敗を見た。
    assert!(finalized_path(&root, 212, 0).exists());
    assert!(review_record(&root, 212, 0).is_none());

    // 再試行: 既存 finalized を decode して Adopted を返し、
    // review record を冪等に公開する（重複 finalize なし）。
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults).unwrap();
    let ReviewOutcome::Adopted(r) = out else {
        panic!("expected Adopted")
    };
    // crash 由来（Finalize record 無し）は Partial で確定する。
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    let rec = review_record(&root, 212, 0).expect("review record");
    assert_eq!(rec.decision, ReviewDecision::Adopted);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02_fault_matrix_journal_short_write_torn_append() {
    // journal への short write（torn append）: 部分 byte が file に
    // 残り writer は Failed。replay は intact prefix で停止し、
    // review Adopt は verified prefix のみを Partial で確定する
    // （torn/corrupt tail を Complete にしない）。
    let lane = LaneId::new(11);
    let root = tmp_dir("j-short");
    let area = staging_area(&root, 214, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    for i in 0..2u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    w.checkpoint().unwrap();
    // journal write: chunk0(0), chunk1(1), checkpoint(2) を消費済み。
    // chunk2 の record append（call=3）が途中で切れる。
    w.inject_journal(Inject::ShortWrite { call: 3, bytes: 17 });
    push_and_pump(&mut w, &mut ends, 2, 128);
    assert_eq!(w.state(), StreamState::Failed);
    drop(w);

    let report = recovery::scan(&root).unwrap();
    let s = &report.sessions[0].streams[0];
    // torn tail は CleanEnd に到達せず、verified prefix のみが残る。
    assert_ne!(s.stop, StopReason::CleanEnd);
    assert_eq!(s.class, StreamClass::Partial);
    assert_eq!(s.review_state, ReviewState::Pending);
    // media には chunk2 分の byte があるが journaled coverage は
    // checkpoint の 2 chunk 分に留まる（unverified tail）。
    assert_eq!(s.unverified_tail_bytes, 64 * FRAME_BYTES);

    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults).unwrap();
    let ReviewOutcome::Adopted(r) = out else {
        panic!("expected Adopted")
    };
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    assert_eq!(r.media_byte_len, 2 * 64 * FRAME_BYTES);
    assert_eq!(r.source_frames, 128);
    fs::remove_dir_all(&root).ok();
}

// ---------- unsynced-write loss（sync frontier truncation） ----------

#[test]
fn rcd02_unsynced_loss_truncates_to_sync_frontier() {
    // `write_all` が成功し file に見えていた byte が、sync 未達の
    // ため power-loss で失われる喪失形を、media/journal を最終
    // sync frontier（checkpoint 直後の実 file 長）へ truncate
    // して再現する。
    let lane = LaneId::new(11);
    let root = tmp_dir("unsynced-loss");
    let area = staging_area(&root, 215, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    for i in 0..3u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    w.pump();
    w.checkpoint().unwrap();
    // checkpoint 直後の実 file 長 = 最終 sync frontier。
    let mp = media_path(&root, 215, 0);
    let jp = journal_path(&root, 215, 0);
    let synced_media_len = fs::metadata(&mp).unwrap().len();
    let synced_journal_len = fs::metadata(&jp).unwrap().len();
    // 以後の write は成功するが sync 未達（unsynced tail）。
    for i in 3..5u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    drop(w);
    // unsynced tail が実際に file 上に存在することを pin する
    // （喪失の再現が vacuous でないことの保証）。
    assert!(fs::metadata(&mp).unwrap().len() > synced_media_len);
    assert!(fs::metadata(&jp).unwrap().len() > synced_journal_len);
    // power-loss 相当: OS が捨てた dirty page 分を truncate で再現。
    truncate_to(&mp, synced_media_len);
    truncate_to(&jp, synced_journal_len);

    let report = recovery::scan(&root).unwrap();
    let s = &report.sessions[0].streams[0];
    // chunk 3-4 の record も喪失し、journal は checkpoint で
    // clean に終わる。tail も無く Healthy に見えるが、finalize
    // は未実施（Pending）。
    assert_eq!(s.stop, StopReason::CleanEnd);
    assert_eq!(s.class, StreamClass::Healthy);
    assert_eq!(s.review_state, ReviewState::Pending);
    assert_eq!(s.durable.map(|d| d.media_len), Some(synced_media_len));
    assert_eq!(s.validated_tail_bytes, 0);
    assert_eq!(s.unverified_tail_bytes, 0);

    let area = StagingArea::open(&root, session_id(215)).unwrap();
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults).unwrap();
    let ReviewOutcome::Adopted(r) = out else {
        panic!("expected Adopted")
    };
    // verified prefix（sync frontier）のみ確定。Finalize record
    // 不在の crash 由来なので Complete には昇格せず、喪失
    // chunk は ref/digest に一切混入しない。
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    assert_eq!(r.media_byte_len, synced_media_len);
    assert_eq!(r.source_frames, 192);
    assert_eq!(r.spans.len(), 3);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02_unsynced_loss_asymmetric_tail() {
    let lane = LaneId::new(11);
    // --- media 残留 / journal 喪失 ---
    // media の unsynced byte は OS が flush 済みで残り、journal
    // 側の record だけ失われた状態（unverified tail）。
    let root = tmp_dir("unsynced-media-tail");
    let area = staging_area(&root, 216, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    for i in 0..3u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    w.pump();
    w.checkpoint().unwrap();
    let mp = media_path(&root, 216, 0);
    let jp = journal_path(&root, 216, 0);
    let synced_media_len = fs::metadata(&mp).unwrap().len();
    let synced_journal_len = fs::metadata(&jp).unwrap().len();
    for i in 3..5u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    drop(w);
    truncate_to(&jp, synced_journal_len);

    let report = recovery::scan(&root).unwrap();
    let s = &report.sessions[0].streams[0];
    // journal は checkpoint で clean、media のみ unverified tail。
    assert_eq!(s.stop, StopReason::CleanEnd);
    assert_eq!(s.class, StreamClass::Partial);
    assert_eq!(s.unverified_tail_bytes, 2 * 64 * FRAME_BYTES);

    let area = StagingArea::open(&root, session_id(216)).unwrap();
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults).unwrap();
    let ReviewOutcome::Adopted(r) = out else {
        panic!("expected Adopted")
    };
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    assert_eq!(r.media_byte_len, synced_media_len);
    assert_eq!(r.source_frames, 192);
    // unverified byte は削除・切詰めされず file 上に保存される。
    assert_eq!(
        fs::metadata(&mp).unwrap().len(),
        synced_media_len + 2 * 64 * FRAME_BYTES
    );
    fs::remove_dir_all(&root).ok();

    // --- journal 残留 / media 喪失 ---
    // journal 側の chunk record は残るが media の unsynced byte
    // が失われ、journaled 被覆が media 実長を超える状態。
    let root = tmp_dir("unsynced-journal-tail");
    let area = staging_area(&root, 217, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    for i in 0..3u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    w.pump();
    w.checkpoint().unwrap();
    let mp = media_path(&root, 217, 0);
    let synced_media_len = fs::metadata(&mp).unwrap().len();
    for i in 3..5u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    drop(w);
    // media の unsynced tail が喪失前に存在していたことを pin する。
    assert!(fs::metadata(&mp).unwrap().len() > synced_media_len);
    truncate_to(&mp, synced_media_len);

    let report = recovery::scan(&root).unwrap();
    let s = &report.sessions[0].streams[0];
    // journal は chunk4 まで clean だが media が journaled 被覆
    // に満たず、fault として報告され Partial に分類される。
    assert_eq!(s.stop, StopReason::CleanEnd);
    assert_eq!(s.class, StreamClass::Partial);
    assert_eq!(s.validated_tail_bytes, 2 * 64 * FRAME_BYTES);
    assert!(
        s.faults
            .iter()
            .any(|f| f.contains("media shorter than journaled coverage"))
    );

    let area = StagingArea::open(&root, session_id(217)).unwrap();
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults).unwrap();
    let ReviewOutcome::Adopted(r) = out else {
        panic!("expected Adopted")
    };
    // journaled tail は frontier を越えるため Complete にならず、
    // media が物理的に欠けた分も verified prefix に混入しない。
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    assert_eq!(r.media_byte_len, synced_media_len);
    assert_eq!(r.source_frames, 192);
    fs::remove_dir_all(&root).ok();
}

// ---------- commit receipt: finalize 段階の区別 ----------

#[test]
fn rcd02_commit_receipt_finalize_publish_lost_is_distinct() {
    // finalized.json の公開が失われた（journal の Finalize record は
    // 残る）状態は、commit 未実行の Reserved と区別して
    // `journal_finalize.finalized_present=false` +
    // `finalize_record=Some(Complete)` として観測される。
    // commit は missing-finalized を recoverable に分離し、
    // 未 finalize stream を commit しない。
    let (mut c, lanes, epoch) = controller_with(1);
    let (sid, cfgs) = open_session(&mut c, &lanes, epoch);
    let root = tmp_dir("fin-lost");
    let area = record_close(&mut c, &root, sid, &cfgs, &lanes, epoch);
    let finalized = finalized_path(&root, sid.raw(), 0);
    fs::remove_file(&finalized).unwrap();

    let report = commit_session(&mut c, &area).unwrap();
    assert!(matches!(report.outcome, CommitOutcome::NothingCommitted));
    assert_eq!(report.recoverable.len(), 1);
    assert_eq!(report.recoverable[0].reason, "missing-finalized");
    assert_eq!(c.live().sources.len(), 0);
    assert_eq!(c.live().recording.takes.len(), 0);

    // receipt: commit 未適用 + marker nothing-committed = Reserved。
    // journal_finalize が finalize publish 喪失を区別して示す。
    let r = commit_receipt(c.live(), &area, None).unwrap();
    assert_eq!(r.stage, ReceiptStage::Reserved);
    assert_eq!(r.marker_outcome.as_deref(), Some("nothing-committed"));
    assert_eq!(r.journal_finalize.len(), 1);
    let e = &r.journal_finalize[0];
    assert!(!e.finalized_present);
    assert!(matches!(e.finalize_record, Some(FinalizeOutcome::Complete)));
    assert_eq!(e.journal_stop, Some(StopReason::CleanEnd));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02_commit_receipt_adopted_partial_finalize_record_absent() {
    // review Adopt で確定した Partial（torn journal 由来）は
    // finalized.json を持つが journal に Finalize record を持たない
    // （torn tail への append は行われない）。receipt の
    // `journal_finalize` がこの非対称を区別して観測し、
    // Partial は commit 対象外（recoverable）に留まる。
    let (mut c, lanes, epoch) = controller_with(1);
    let (sid, cfgs) = open_session(&mut c, &lanes, epoch);
    let root = tmp_dir("fin-adopted");
    let area = StagingArea::create(
        &root,
        &StagingSessionConfig {
            session_id: sid,
            sample_rate: RATE,
            streams: cfgs.clone(),
        },
    )
    .unwrap();
    let (mut w, mut ends) = crashed_writer(&area, None);
    for i in 0..3u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    w.pump();
    w.checkpoint().unwrap();
    drop(w);
    // journal 末尾に torn tail を注入（kill/power-loss 相当）。
    let jp = journal_path(&root, sid.raw(), 0);
    let mut jbytes = fs::read(&jp).unwrap();
    let partial = encode_record(
        4,
        &RecordPayload::Chunk(ChunkRecord {
            chunk_id: 0,
            first_input_frame: 0,
            session_start: 0,
            block_frames: 32,
            stored_frames: 32,
            media_offset: 0,
            media_len: 8,
            pcm_crc32: 0,
            digest_fold: 0,
            spans: vec![],
        }),
    )
    .unwrap();
    jbytes.extend(&partial[..partial.len() / 2]);
    fs::write(&jp, &jbytes).unwrap();
    rop(&mut c, RecordingOp::CloseSession { session_id: sid });

    // review Adopt → verified prefix のみ Partial で確定。
    // journal stop が CleanEnd でないため Finalize record は
    // 追記されない（finalized.json が authority）。
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults).unwrap();
    let ReviewOutcome::Adopted(r) = out else {
        panic!("expected Adopted")
    };
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    assert!(finalized_path(&root, sid.raw(), 0).exists());

    // Partial は commit 対象外 — payload は空で NothingCommitted、
    // stream は recoverable に分離される。
    let report = commit_session(&mut c, &area).unwrap();
    assert!(matches!(report.outcome, CommitOutcome::NothingCommitted));
    assert_eq!(report.recoverable.len(), 1);
    assert_eq!(report.recoverable[0].reason, "partial-finalize");
    assert_eq!(c.live().sources.len(), 0);
    assert_eq!(c.live().recording.takes.len(), 0);

    // receipt: finalized 有り + Finalize record 無し + TornTail を
    // 区別して観測する（journal finalize append 未記録の状態）。
    let r = commit_receipt(c.live(), &area, None).unwrap();
    assert_eq!(r.stage, ReceiptStage::Reserved);
    assert_eq!(r.journal_finalize.len(), 1);
    let e = &r.journal_finalize[0];
    assert!(e.finalized_present);
    assert!(e.finalize_record.is_none());
    assert_eq!(e.journal_stop, Some(StopReason::TornTail));
    fs::remove_dir_all(&root).ok();
}
