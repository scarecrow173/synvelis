//! RCD-02 (synvelis#116) project/library adapter slice:
//! verified-prefix SHA-256 identity・`ProjectMediaProbe`/`CompositeProbe`
//! ・saved-HEAD revision adapter の受け入れ検証。
//!
//! `verified_sha256`（`sha256:<64hex>`）は `digest`/`digest_scheme`
//! の CRC32 integrity fold・canonical digest とは別 namespace の
//! verified-blob identity で、#86 の `ContentDigest` と同一形式。
//! unverified/journaled tail は digest 対象に含めない。
//!
//! `ProjectMediaProbe` は live project・undo/redo 履歴・saved HEAD
//! のいずれかが session/stream を参照する場合に Discard を拒否し、
//! 判定不能（HEAD schema 不一致・decode 失敗・store error・
//! `project_id` 不一致）は全て `Unknown` で fail-closed にする。
//! fake/mock FS・InMemoryStore の成功は実 OS/FS durability の
//! 証明にしない（実機計測は `NotRun`）。

use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

use sha2::Digest as _;
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
    let d = std::env::temp_dir().join(format!("synvelis-rcd02pa-{}-{name}", std::process::id()));
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

/// project 登録済み session id を使う staging area（`record_close`
/// と同じ構成）。
fn project_staging_area(
    root: &Path,
    sid: RecordingSessionId,
    cfgs: &[StagingStreamConfig],
) -> StagingArea {
    StagingArea::create(
        root,
        &StagingSessionConfig {
            session_id: sid,
            sample_rate: RATE,
            streams: cfgs.to_vec(),
        },
    )
    .unwrap()
}

fn tag_paths(root: &Path, tag: &str, i: u16) -> (PathBuf, PathBuf, PathBuf) {
    let dir = root.join(tag);
    (
        dir.join(format!("streams/{}.pcm", stream_stem(i))),
        dir.join(format!("streams/{}.journal", stream_stem(i))),
        dir.join(format!("streams/{}.finalized.json", stream_stem(i))),
    )
}

fn media_path(root: &Path, tag: &str, i: u16) -> PathBuf {
    tag_paths(root, tag, i).0
}

fn journal_path(root: &Path, tag: &str, i: u16) -> PathBuf {
    tag_paths(root, tag, i).1
}

fn finalized_path(root: &Path, tag: &str, i: u16) -> PathBuf {
    tag_paths(root, tag, i).2
}

/// crash 相当: finalize せず writer を落とした stream を手組みで作る
/// （media/journal は実 file、checkpoint まで durable）。
fn crashed_writer(area: &StagingArea) -> (StreamWriter, EngineEnds) {
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
            area.manifest.capabilities.clone(),
        ),
        engine_ends,
    )
}

fn manual_writer(area: &StagingArea) -> (StreamWriter, EngineEnds) {
    crashed_writer(area)
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

fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{:x}", sha2::Sha256::digest(bytes))
}

/// media 末尾へ journal 未記録の byte を足す（unverified tail）。
fn append_unverified_media(path: &Path, bytes: usize) {
    let mut m = OpenOptions::new().append(true).open(path).unwrap();
    std::io::Write::write_all(&mut m, &vec![0xCDu8; bytes]).unwrap();
    m.sync_data().unwrap();
}

fn finalized_ref(root: &Path, tag: &str, i: u16) -> FinalizedSourceRef {
    let bytes = fs::read(finalized_path(root, tag, i)).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

// ---------- project/commit helpers ----------

fn controller_with(lane_count: usize) -> (ProjectController, Vec<LaneId>, ClockEpochId) {
    let mut c = ProjectController::new(
        Project::new(ProjectId::new(0), "rcd-02-adapter"),
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
            name: "rcd-02-adapter-session".into(),
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
    let area = project_staging_area(root, session_id, cfgs);
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

/// commit 済み session の staging から finalized/manifest/marker を
/// 取り除き、「project 側に参照が残るが staging metadata は失われた」
/// 状態を再現する（probe の Referenced/Unknown 経路を試すために
/// 必要 — manifest が残る限り `review_discard` は probe 到達前に拒否
/// する）。
fn strip_commit_artifacts(area: &StagingArea) {
    for p in [commit_manifest_path(area), commit_marker_path(area)] {
        let _ = fs::remove_file(p);
    }
    for s in &area.manifest.streams {
        let _ = fs::remove_file(stream_paths(area, s.stream_index).finalized);
    }
}

// ---------- probe stubs ----------

struct ClearProbe;
impl MediaReferenceProbe for ClearProbe {
    fn check(&self, _c: &DiscardCandidate) -> ReferenceCheck {
        ReferenceCheck::Clear
    }
}

struct ReferencedProbe;
impl MediaReferenceProbe for ReferencedProbe {
    fn check(&self, _c: &DiscardCandidate) -> ReferenceCheck {
        ReferenceCheck::Referenced("stub".into())
    }
}

struct UnknownProbe;
impl MediaReferenceProbe for UnknownProbe {
    fn check(&self, _c: &DiscardCandidate) -> ReferenceCheck {
        ReferenceCheck::Unknown("stub".into())
    }
}

fn candidate() -> DiscardCandidate {
    DiscardCandidate {
        session_id: session_id(1).to_tagged(),
        stream_index: 0,
        capture_stream_id: CaptureStreamId::new(2).to_tagged(),
        lane_id: LaneId::new(3).to_tagged(),
        media: "streams/s000.pcm".into(),
        journal: "streams/s000.journal".into(),
        media_byte_len: 0,
    }
}

// ---------- verified-prefix SHA-256 ----------

#[test]
fn rcd02pa_finalize_records_verified_prefix_sha256() {
    let lane = LaneId::new(11);
    let root = tmp_dir("sha-complete");
    let area = staging_area(&root, 301, &[lane]);
    let tag = session_id(301).to_tagged();
    let (mut w, mut ends) = manual_writer(&area);
    for i in 0..3u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    w.checkpoint().unwrap();
    let r = w.finalize(&tag, RATE).unwrap();
    assert_eq!(r.outcome, FinalizedOutcome::Complete);
    drop(w);

    // verified prefix 全体の SHA-256（`sha256:<64hex>`）。
    let media = fs::read(media_path(&root, &tag, 0)).unwrap();
    let expect = sha256_prefixed(&media[..usize::try_from(r.media_byte_len).unwrap()]);
    assert_eq!(r.verified_sha256.as_deref(), Some(expect.as_str()));
    let on_disk = finalized_ref(&root, &tag, 0);
    assert_eq!(on_disk.verified_sha256.as_deref(), Some(expect.as_str()));
    fs::remove_dir_all(&root).ok();

    // unverified tail（journal 未記録の media byte）がある Partial
    // では、digest は durable frontier までの prefix のみを覆う。
    // （journaled tail は finalize の暗黙 checkpoint で durable に
    // 昇格するため、Partial を作るには未検証 byte が必要。）
    let root = tmp_dir("sha-partial");
    let area = staging_area(&root, 302, &[lane]);
    let tag = session_id(302).to_tagged();
    let (mut w, mut ends) = manual_writer(&area);
    for i in 0..3u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    w.checkpoint().unwrap();
    append_unverified_media(&media_path(&root, &tag, 0), 512);
    let r = w.finalize(&tag, RATE).unwrap();
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    assert_eq!(r.media_byte_len, 3 * 64 * FRAME_BYTES);
    drop(w);
    let media = fs::read(media_path(&root, &tag, 0)).unwrap();
    assert_eq!(media.len() as u64, 3 * 64 * FRAME_BYTES + 512);
    let expect = sha256_prefixed(&media[..usize::try_from(r.media_byte_len).unwrap()]);
    assert_eq!(r.verified_sha256.as_deref(), Some(expect.as_str()));
    // tail を含む全体の hash とは一致しない（prefix 限定の証明）。
    assert_ne!(expect, sha256_prefixed(&media));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02pa_recovery_adopt_records_verified_prefix_sha256() {
    let lane = LaneId::new(11);
    let root = tmp_dir("sha-adopt");
    let area = staging_area(&root, 303, &[lane]);
    let tag = session_id(303).to_tagged();
    let (mut w, mut ends) = manual_writer(&area);
    for i in 0..3u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    w.checkpoint().unwrap();
    for i in 3..5u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    drop(w);

    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults).unwrap();
    let ReviewOutcome::Adopted(r) = out else {
        panic!("expected Adopted")
    };
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    let media = fs::read(media_path(&root, &tag, 0)).unwrap();
    let expect = sha256_prefixed(&media[..usize::try_from(r.media_byte_len).unwrap()]);
    assert_eq!(r.verified_sha256.as_deref(), Some(expect.as_str()));
    let on_disk = finalized_ref(&root, &tag, 0);
    assert_eq!(on_disk.verified_sha256.as_deref(), Some(expect.as_str()));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02pa_commit_manifest_carries_verified_sha256() {
    let (mut c, lanes, epoch) = controller_with(1);
    let (sid, cfgs) = open_session(&mut c, &lanes, epoch);
    let root = tmp_dir("sha-manifest");
    let area = record_close(&mut c, &root, sid, &cfgs, &lanes, epoch);
    let report = commit_session(&mut c, &area).unwrap();
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));

    let manifest = read_commit_manifest(&area)
        .unwrap()
        .expect("commit manifest");
    assert_eq!(manifest.payload.streams.len(), 1);
    let entry = &manifest.payload.streams[0];
    let sha = entry.verified_sha256.as_deref().expect("verified_sha256");
    assert!(sha.starts_with("sha256:") && sha.len() == 71);

    // finalized ref と manifest で同一の verified-prefix identity。
    let tag = sid.to_tagged();
    let fin = finalized_ref(&root, &tag, 0);
    assert_eq!(fin.verified_sha256.as_deref(), Some(sha));
    // 実 media の verified prefix と独立に一致する。
    let media = fs::read(media_path(&root, &tag, 0)).unwrap();
    let expect = sha256_prefixed(&media[..usize::try_from(entry.media_byte_len).unwrap()]);
    assert_eq!(sha, expect);

    // payload hash は verified_sha256 を含む（identity の一部）。
    let mut stripped = manifest.payload.clone();
    for s in &mut stripped.streams {
        s.verified_sha256 = None;
    }
    // 旧形式（field 無し）は serialize から key 自体を省略し、
    // 旧 manifest との payload hash 互換を保つ。
    let json = serde_json::to_string(&stripped).unwrap();
    assert!(!json.contains("verified_sha256"));
    assert_ne!(
        payload_hash(&stripped).unwrap(),
        manifest.identity.content_hash
    );
    // 旧形式 JSON（key 欠落）は default で decode できる。
    let mut v = serde_json::to_value(&manifest).unwrap();
    for s in v["payload"]["streams"].as_array_mut().unwrap() {
        s.as_object_mut().unwrap().remove("verified_sha256");
    }
    let old: RecordingCommitManifest = serde_json::from_value(v).unwrap();
    assert!(old.payload.streams[0].verified_sha256.is_none());
    fs::remove_dir_all(&root).ok();
}

// ---------- ProjectMediaProbe / CompositeProbe ----------

#[test]
fn rcd02pa_probe_clear_allows_discard() {
    // project が一度も登録していない orphan staging — live・履歴・
    // HEAD のいずれにも参照が無いため Clear。
    let (c, _lanes, _epoch) = controller_with(1);
    let root = tmp_dir("probe-clear");
    let area = staging_area(&root, 999, &[LaneId::new(999)]);
    let tag = session_id(999).to_tagged();
    let (mut w, mut ends) = manual_writer(&area);
    push_and_pump(&mut w, &mut ends, 0, 0);
    w.checkpoint().unwrap();
    drop(w);

    let store = InMemoryStore::new();
    let probe = ProjectMediaProbe::new(&c, &store);
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Discard, Some(&probe), &mut faults).unwrap();
    assert_eq!(out, ReviewOutcome::Discarded);
    assert!(!media_path(&root, &tag, 0).exists());
    assert!(!journal_path(&root, &tag, 0).exists());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02pa_probe_blocks_open_state_in_history() {
    // live は Closed でも、undo 履歴に session Open/Created 状態が
    // 残る限り undo で「録音中」状態へ戻り得る = media は参照中。
    // fail-closed で Discard を拒否する。
    let (mut c, lanes, epoch) = controller_with(1);
    let (sid, cfgs) = open_session(&mut c, &lanes, epoch);
    let root = tmp_dir("probe-hist-open");
    let area = project_staging_area(&root, sid, &cfgs);
    let tag = sid.to_tagged();
    let (mut w, mut ends) = manual_writer(&area);
    push_and_pump(&mut w, &mut ends, 0, 0);
    w.checkpoint().unwrap();
    drop(w);
    rop(&mut c, RecordingOp::CloseSession { session_id: sid });
    assert!(matches!(
        c.live().recording.sessions[&sid].state,
        SessionState::Closed
    ));

    let store = InMemoryStore::new();
    let probe = ProjectMediaProbe::new(&c, &store);
    let mut faults = PublishFaults::new();
    let err =
        review_stream(&area, 0, ReviewAction::Discard, Some(&probe), &mut faults).unwrap_err();
    let ReviewError::Refused(msg) = err else {
        panic!("expected Refused")
    };
    assert!(msg.contains("project history"), "{msg}");
    assert!(media_path(&root, &tag, 0).exists());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02pa_probe_open_session_blocks_discard() {
    let (mut c, lanes, epoch) = controller_with(1);
    let (sid, cfgs) = open_session(&mut c, &lanes, epoch);
    let root = tmp_dir("probe-open");
    let area = project_staging_area(&root, sid, &cfgs);
    let tag = sid.to_tagged();
    let (mut w, mut ends) = manual_writer(&area);
    push_and_pump(&mut w, &mut ends, 0, 0);
    w.checkpoint().unwrap();
    drop(w);
    // session は Open のまま — 録音継続中の media を recovery が
    // 破棄してはならない。

    let store = InMemoryStore::new();
    let probe = ProjectMediaProbe::new(&c, &store);
    let mut faults = PublishFaults::new();
    let err =
        review_stream(&area, 0, ReviewAction::Discard, Some(&probe), &mut faults).unwrap_err();
    let ReviewError::Refused(msg) = err else {
        panic!("expected Refused")
    };
    assert!(msg.contains("media referenced"), "{msg}");
    assert!(media_path(&root, &tag, 0).exists());
    assert!(journal_path(&root, &tag, 0).exists());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02pa_probe_blocks_committed_media_in_live() {
    let (mut c, lanes, epoch) = controller_with(1);
    let (sid, cfgs) = open_session(&mut c, &lanes, epoch);
    let root = tmp_dir("probe-live");
    let area = record_close(&mut c, &root, sid, &cfgs, &lanes, epoch);
    let report = commit_session(&mut c, &area).unwrap();
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    let tag = sid.to_tagged();
    // staging metadata 喪失を再現（project 側の参照は残る）。
    strip_commit_artifacts(&area);

    let store = InMemoryStore::new();
    let probe = ProjectMediaProbe::new(&c, &store);
    let mut faults = PublishFaults::new();
    let err =
        review_stream(&area, 0, ReviewAction::Discard, Some(&probe), &mut faults).unwrap_err();
    assert!(matches!(err, ReviewError::Refused(_)), "{err}");
    assert!(media_path(&root, &tag, 0).exists());
    assert!(journal_path(&root, &tag, 0).exists());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02pa_probe_blocks_via_undo_redo_history() {
    let (mut c, lanes, epoch) = controller_with(1);
    let (sid, cfgs) = open_session(&mut c, &lanes, epoch);
    let root = tmp_dir("probe-history");
    let area = record_close(&mut c, &root, sid, &cfgs, &lanes, epoch);
    let report = commit_session(&mut c, &area).unwrap();
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    let tag = sid.to_tagged();
    strip_commit_artifacts(&area);
    // commit を undo → live は参照を持たないが、redo stack の
    // commit 済み状態から復帰可能 = 参照が残る。
    assert!(c.undo().unwrap());
    assert!(c.live().recording.applied_commits.is_empty());

    let store = InMemoryStore::new();
    let probe = ProjectMediaProbe::new(&c, &store);
    let mut faults = PublishFaults::new();
    let err =
        review_stream(&area, 0, ReviewAction::Discard, Some(&probe), &mut faults).unwrap_err();
    let ReviewError::Refused(msg) = err else {
        panic!("expected Refused")
    };
    assert!(msg.contains("project history"), "{msg}");
    assert!(media_path(&root, &tag, 0).exists());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02pa_probe_blocks_via_saved_head() {
    let (mut c, lanes, epoch) = controller_with(1);
    let (sid, cfgs) = open_session(&mut c, &lanes, epoch);
    let root = tmp_dir("probe-head");
    let area = record_close(&mut c, &root, sid, &cfgs, &lanes, epoch);
    // commit 前の clean な project 状態を別 controller に移す
    // （live 参照なし・履歴なし・saved HEAD のみが参照源になる）。
    let pre_commit = c.live().clone();
    let report = commit_session(&mut c, &area).unwrap();
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    let tag = sid.to_tagged();
    strip_commit_artifacts(&area);

    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let other = ProjectController::new(pre_commit, Box::new(CounterIdGenerator::new(1)));
    let probe = ProjectMediaProbe::new(&other, &store);
    let mut faults = PublishFaults::new();
    let err =
        review_stream(&area, 0, ReviewAction::Discard, Some(&probe), &mut faults).unwrap_err();
    let ReviewError::Refused(msg) = err else {
        panic!("expected Refused")
    };
    assert!(msg.contains("saved HEAD"), "{msg}");
    assert!(media_path(&root, &tag, 0).exists());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02pa_probe_unknown_head_states_fail_closed() {
    // orphan staging（project 未登録）+ clean live/履歴 —
    // saved HEAD の状態だけが判定を分ける。
    let (c, _lanes, _epoch) = controller_with(1);
    let root = tmp_dir("probe-unknown");
    let area = staging_area(&root, 999, &[LaneId::new(999)]);
    let tag = session_id(999).to_tagged();
    let (mut w, mut ends) = manual_writer(&area);
    push_and_pump(&mut w, &mut ends, 0, 0);
    w.checkpoint().unwrap();
    drop(w);

    // (a) decode 不能な HEAD bytes。
    // (b) schema 不一致の HEAD（migration 未対応 → Unknown）。
    let old_env = SnapshotEnvelope {
        schema_version: SCHEMA_VERSION - 1,
        project_id: ProjectId::new(77),
        revision: ProjectRevision(1),
        project: Project::new(ProjectId::new(77), "old"),
    };
    // (c) 別 project の HEAD。
    let foreign_env = SnapshotEnvelope {
        schema_version: SCHEMA_VERSION,
        project_id: ProjectId::new(88),
        revision: ProjectRevision(1),
        project: Project::new(ProjectId::new(88), "foreign"),
    };
    // (d) envelope/body の project_id 不整合。
    let mismatch_env = SnapshotEnvelope {
        schema_version: SCHEMA_VERSION,
        project_id: ProjectId::new(99),
        revision: ProjectRevision(1),
        project: Project::new(ProjectId::new(100), "mismatch"),
    };

    for (name, env) in [
        ("corrupt", None),
        ("old-schema", Some(old_env)),
        ("foreign", Some(foreign_env)),
        ("mismatch", Some(mismatch_env)),
    ] {
        let mut store = InMemoryStore::new();
        match env {
            Some(e) => {
                store
                    .save("p", ProjectRevision(1), &serde_json::to_vec(&e).unwrap())
                    .unwrap();
            }
            None => {
                store.save("p", ProjectRevision(1), b"not-json").unwrap();
            }
        }
        let probe = ProjectMediaProbe::new(&c, &store);
        let mut faults = PublishFaults::new();
        let err =
            review_stream(&area, 0, ReviewAction::Discard, Some(&probe), &mut faults).unwrap_err();
        let ReviewError::Refused(msg) = err else {
            panic!("{name}: expected Refused")
        };
        assert!(msg.contains("reference state unknown"), "{name}: {msg}");
        assert!(media_path(&root, &tag, 0).exists(), "{name}");
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd02pa_composite_probe_semantics() {
    let c = candidate();
    // Referenced は Unknown より優先（fail-closed で拒否理由を残す）。
    let p = CompositeProbe::new(vec![&UnknownProbe, &ReferencedProbe]);
    assert!(matches!(p.check(&c), ReferenceCheck::Referenced(_)));
    // Clear が混ざっても Unknown は残る（判定不能を握り潰さない）。
    let p = CompositeProbe::new(vec![&ClearProbe, &UnknownProbe]);
    assert!(matches!(p.check(&c), ReferenceCheck::Unknown(_)));
    // 全 Clear → Clear。
    let p = CompositeProbe::new(vec![&ClearProbe, &ClearProbe]);
    assert_eq!(p.check(&c), ReferenceCheck::Clear);
    // 空列 → Clear。
    let p = CompositeProbe::new(vec![]);
    assert_eq!(p.check(&c), ReferenceCheck::Clear);
}

// ---------- Finalized → Commit → Save → Reload → Undo chain ----------

/// commit 由来 entity の tagged ID 集合（重複 take/asset 検出用
/// snapshot）。BTreeMap の key 順で決定的。
fn entity_snapshot(p: &Project) -> serde_json::Value {
    serde_json::json!({
        "sources": p.sources.keys().map(|k| k.to_tagged()).collect::<Vec<_>>(),
        "assets": p.assets.keys().map(|k| k.to_tagged()).collect::<Vec<_>>(),
        "passes": p.recording.passes.keys().map(|k| k.to_tagged()).collect::<Vec<_>>(),
        "segments": p.recording.segments.keys().map(|k| k.to_tagged()).collect::<Vec<_>>(),
        "takes": p.recording.takes.keys().map(|k| k.to_tagged()).collect::<Vec<_>>(),
        "clips": p.clips.keys().map(|k| k.to_tagged()).collect::<Vec<_>>(),
        "applied_commits": p.recording.applied_commits.keys().map(|k| k.to_tagged()).collect::<Vec<_>>(),
    })
}

#[test]
fn rcd02pa_commit_save_reload_undo_restore_ids() {
    // Finalized(Complete + verified_sha256) → Commit → Undo/Redo →
    // Save → Reload（別 Start 相当）→ 再 Commit が AlreadyApplied で
    // 二重 Asset/Take を生成しないことを通しで検証する。
    let (mut c, lanes, epoch) = controller_with(1);
    let (sid, cfgs) = open_session(&mut c, &lanes, epoch);
    let root = tmp_dir("e2e-chain");
    let area = record_close(&mut c, &root, sid, &cfgs, &lanes, epoch);
    let report = commit_session(&mut c, &area).unwrap();
    let CommitOutcome::Applied { .. } = report.outcome else {
        panic!("expected applied: {:?}", report.outcome)
    };
    let hash = report.content_hash.clone();
    let committed = entity_snapshot(c.live());
    assert_ne!(committed["takes"], serde_json::json!([]));
    assert_ne!(committed["sources"], serde_json::json!([]));

    // Undo: commit 由来 entity が消え、staging media/manifest は残る。
    assert!(c.undo().unwrap());
    let undone = entity_snapshot(c.live());
    for key in ["sources", "assets", "passes", "segments", "takes", "clips"] {
        assert!(
            undone[key].as_array().unwrap().is_empty(),
            "{key} must be empty after undo"
        );
    }
    assert_eq!(undone["applied_commits"], serde_json::json!([]));
    let tag = sid.to_tagged();
    assert!(media_path(&root, &tag, 0).exists());
    assert!(commit_manifest_path(&area).exists());

    // Redo: 同一 Stable ID が復元される（別 ID での再作成ではない）。
    assert!(c.redo().unwrap());
    assert_eq!(entity_snapshot(c.live()), committed);

    // Save → Reload（別 Start 相当: persisted HEAD からの load）。
    let mut store = DirectoryBundleStore::create(root.join("bundle")).unwrap();
    c.save(&mut store).unwrap();
    let mut c2 = ProjectController::new(
        Project::new(ProjectId::new(0), "x"),
        Box::new(CounterIdGenerator::new(1)),
    );
    c2.load(&store).unwrap();
    assert_eq!(
        serde_json::to_value(c2.live()).unwrap(),
        serde_json::to_value(c.live()).unwrap(),
        "project snapshot roundtrip"
    );

    // 別 Start 後の同一 Session/Stream 再 commit → AlreadyApplied・
    // entity ID 集合も content hash も不変（誤重複 Take なし）。
    let before = entity_snapshot(c2.live());
    let report2 = commit_session(&mut c2, &area).unwrap();
    assert!(matches!(report2.outcome, CommitOutcome::AlreadyApplied));
    assert_eq!(report2.content_hash, hash);
    assert_eq!(entity_snapshot(c2.live()), before);

    // receipt: registry 適用 + marker 一致 + saved HEAD が commit
    // revision を含む → CommittedConfirmed / project_saved=true。
    let r = commit_receipt(c2.live(), &area, head_revision(&store).unwrap()).unwrap();
    assert_eq!(r.stage, ReceiptStage::CommittedConfirmed);
    assert_eq!(r.project_saved, Some(true));
    fs::remove_dir_all(&root).ok();
}

// ---------- saved-HEAD revision adapter ----------

#[test]
fn rcd02pa_head_revision_and_commit_receipt_saved() {
    let (mut c, lanes, epoch) = controller_with(1);
    let (sid, cfgs) = open_session(&mut c, &lanes, epoch);
    let root = tmp_dir("head-rev");
    let area = record_close(&mut c, &root, sid, &cfgs, &lanes, epoch);
    let report = commit_session(&mut c, &area).unwrap();
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));

    // snapshot_revision: canonical 名のみ受理。
    assert_eq!(snapshot_revision("r12.json"), Some(ProjectRevision(12)));
    for bad in [
        "r01.json",
        "r1.JSON",
        "x1.json",
        "r-1.json",
        "r1.json.bak",
        "12.json",
    ] {
        assert_eq!(snapshot_revision(bad), None, "{bad}");
    }

    let mut store = InMemoryStore::new();
    // save 前: HEAD 無し → saved_revision None → project_saved 不明。
    assert_eq!(head_revision(&store).unwrap(), None);
    let r = commit_receipt(c.live(), &area, head_revision(&store).unwrap()).unwrap();
    assert_eq!(r.project_saved, None);

    // save 後: HEAD revision が commit revision を含む → saved=true。
    let saved = c.save(&mut store).unwrap();
    let head = head_revision(&store).unwrap();
    assert_eq!(head, Some(saved.revision));
    let r = commit_receipt(c.live(), &area, head).unwrap();
    assert_eq!(r.stage, ReceiptStage::CommittedConfirmed);
    assert_eq!(r.project_saved, Some(true));
    fs::remove_dir_all(&root).ok();
}
