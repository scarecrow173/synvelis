//! RCD-04 Recovery partial + cancel + retry fixture
//! (synvelis#118 crate fixture slice)。
//!
//! unverified tail を持つ crashed stream を手組みし、
//! `recovery::scan` → `review_stream` の Hold(cancel/defer) →
//! Adopt(retry) 系列で、verified prefix のみが確定し tail が
//! Complete に昇格しないこと・file/revision 非改変・冪等性を
//! 検証する。2 stream の partial/held 混在で commit payload が
//! 全 stream を recoverable に分離する(all-or-nothing)こと、
//! corrupt journal の fail-closed を確認する。
//!
//! fake/mock FS 上の契約検証であり、実 OS/FS/Device durability
//! の Pass を意味しない(実機は `NotRun`)。

use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

use synvelis_project::*;
use synvelis_recording::*;

const RATE: u32 = 48_000;
const CHANNELS: u32 = 2;
const FRAME_BYTES: u64 = 8;

// ---------- staging helpers (recovery_review.rs conventions) ----------

fn session_id(n: u128) -> RecordingSessionId {
    RecordingSessionId::new(n)
}

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("synvelis-rcd04-{}-{name}", std::process::id()));
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

/// crash 相当: finalize せず writer を落とした stream を手組みで作る。
fn manual_writer(area: &StagingArea) -> (StreamWriter, EngineEnds) {
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

/// stream `i` 用の手組み writer（multi-stream 用）。
fn manual_writer_at(area: &StagingArea, i: u16) -> (StreamWriter, EngineEnds) {
    let (mut engine_ends, writer_ends) = build_handoff(0, 512, CHANNELS, 4, 8, 4);
    while engine_ends.free_rx.try_pop().is_ok() {}
    let fault = FaultLatch::new();
    let m = area
        .manifest
        .streams
        .iter()
        .find(|s| s.stream_index == i)
        .unwrap();
    let cfg = StagingStreamConfig {
        stream_index: m.stream_index,
        capture_stream_id: m.capture_stream_id.clone(),
        lane_id: m.lane_id.clone(),
        channels: m.channels,
    };
    let paths = stream_paths(area, i);
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
            project_in: MusicalTick::ZERO,
            project_out: MusicalTick::ZERO,
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

fn review_record(root: &Path, session: u128, i: u16) -> Option<ReviewRecord> {
    let path = review_path(root, session, i);
    fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}

/// checkpoint 済み prefix + journaled tail の partial stream を作る
/// （戻り値は検証済み frame 数と unverified tail byte 数）。
fn partial_stream(area: &StagingArea, root: &Path, session: u128, stream: u16) -> (u64, usize) {
    let (mut w, mut ends) = manual_writer_at(area, stream);
    for i in 0..3u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    w.pump();
    w.checkpoint().unwrap();
    // journaled だが frontier 外の tail（validated tail → Partial）。
    for i in 3..5u32 {
        push_and_pump(&mut w, &mut ends, i, i64::from(i) * 64);
    }
    drop(w);
    // media 末尾に未検証 byte（unverified tail は Complete にしない）。
    let tail = 512usize;
    append_unverified_media(&media_path(root, session, stream), tail);
    (3 * 64, tail)
}

// ---------------------------------------------------------------------
// F4: partial + cancel(Hold) + retry(Adopt)
// ---------------------------------------------------------------------

#[test]
fn rcd04_partial_hold_cancel_then_adopt_retry() {
    let lane = LaneId::new(11);
    let root = tmp_dir("partial-hold-adopt");
    let area = staging_area(&root, 220, &[lane]);
    let (prefix_frames, _tail) = partial_stream(&area, &root, 220, 0);

    // scan: Pending・validated tail を報告。
    let report = recovery::scan(&root).unwrap();
    let s = &report.sessions[0].streams[0];
    assert_eq!(s.review_state, ReviewState::Pending);
    assert!(s.validated_tail_bytes > 0);

    // --- cancel (Hold): file/revision 不変・decision durable ---
    let media_before = fs::read(media_path(&root, 220, 0)).unwrap();
    let journal_before = fs::read(journal_path(&root, 220, 0)).unwrap();
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Hold, None, &mut faults).unwrap();
    assert_eq!(out, ReviewOutcome::Held);
    assert_eq!(fs::read(media_path(&root, 220, 0)).unwrap(), media_before);
    assert_eq!(
        fs::read(journal_path(&root, 220, 0)).unwrap(),
        journal_before
    );
    assert!(!finalized_path(&root, 220, 0).exists());
    let rec = review_record(&root, 220, 0).expect("review record");
    assert_eq!(rec.decision, ReviewDecision::Held);

    // --- retry (Adopt): verified prefix のみ Partial で確定 ---
    let mut faults = PublishFaults::new();
    let out = review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults).unwrap();
    let ReviewOutcome::Adopted(r) = out else {
        panic!("expected Adopted")
    };
    // unverified tail は Complete に昇格しない。
    assert_eq!(r.outcome, FinalizedOutcome::Partial);
    assert_eq!(r.source_frames, prefix_frames);
    assert_eq!(r.media_byte_len, prefix_frames * FRAME_BYTES);
    assert!(finalized_path(&root, 220, 0).exists());
    let rec = review_record(&root, 220, 0).expect("review record");
    assert_eq!(rec.decision, ReviewDecision::Adopted);
    assert_eq!(rec.finalized_digest.as_deref(), Some(r.digest.as_str()));

    // 冪等: 再 scan は Finalized、再 Adopt は同一 digest。
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
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd04_partial_discard_requires_clear_probe() {
    let lane = LaneId::new(11);
    let root = tmp_dir("partial-discard");
    let area = staging_area(&root, 221, &[lane]);
    partial_stream(&area, &root, 221, 0);

    // probe 未指定 → fail-closed 拒否。
    let mut faults = PublishFaults::new();
    let err = review_stream(&area, 0, ReviewAction::Discard, None, &mut faults).unwrap_err();
    assert!(matches!(err, ReviewError::Refused(_)));
    assert!(media_path(&root, 221, 0).exists());

    // Referenced → 拒否（参照中 media を消さない）。
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
    assert!(media_path(&root, 221, 0).exists());

    // Clear → staging artifact のみ削除（project/commit 面は不変）。
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
    assert!(!media_path(&root, 221, 0).exists());
    assert!(!journal_path(&root, 221, 0).exists());
    let rec = review_record(&root, 221, 0).expect("review record");
    assert_eq!(rec.decision, ReviewDecision::Discarded);
    fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------
// Group commit 境界: partial/held stream は all-or-nothing で
// commit 対象外（recoverable 分離・部分 commit なし）
// ---------------------------------------------------------------------

#[test]
fn rcd04_group_commit_separates_partial_and_held_streams() {
    let lanes = [LaneId::new(11), LaneId::new(12)];
    let root = tmp_dir("group-partial");
    let area = staging_area(&root, 222, &lanes);
    // stream0: partial prefix + tail → Adopt で Partial 確定。
    partial_stream(&area, &root, 222, 0);
    let mut faults = PublishFaults::new();
    review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults).unwrap();
    // stream1: partial のまま Hold（cancel）→ finalized なし。
    partial_stream(&area, &root, 222, 1);
    let mut faults = PublishFaults::new();
    review_stream(&area, 1, ReviewAction::Hold, None, &mut faults).unwrap();

    // commit payload 構成: Partial・missing ともに recoverable へ
    // 分離され、commit 対象 stream は 0 件（部分 group commit なし）。
    let mut p = Project::new(ProjectId::new(0), "rcd-04-group");
    for &lane in &lanes {
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
    }
    let area2 = StagingArea::open(&root, session_id(222)).unwrap();
    let mut next = 1_000u128;
    let (payload, recoverable) = build_payload(&p, &area2, session_id(222), &mut || {
        next += 1;
        next
    })
    .expect("build_payload");
    assert!(payload.streams.is_empty(), "no stream may commit partially");
    assert!(
        payload.passes.is_empty(),
        "no passes without committed streams"
    );
    assert_eq!(recoverable.len(), 2);
    assert!(
        recoverable
            .iter()
            .all(|r| r.reason == "partial-finalize" || r.reason == "missing-finalized")
    );

    // held stream の file は不変のまま。
    assert!(media_path(&root, 222, 1).exists());
    assert!(journal_path(&root, 222, 1).exists());
    assert!(!finalized_path(&root, 222, 1).exists());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rcd04_corrupt_journal_is_fail_closed() {
    let lane = LaneId::new(11);
    let root = tmp_dir("corrupt-journal");
    let area = staging_area(&root, 223, &[lane]);
    let (mut w, mut ends) = manual_writer(&area);
    push_and_pump(&mut w, &mut ends, 0, 0);
    w.pump();
    w.checkpoint().unwrap();
    drop(w);
    // journal 途中の byte を破壊（corrupt record）。
    let jp = journal_path(&root, 223, 0);
    let mut jbytes = fs::read(&jp).unwrap();
    let mid = jbytes.len() / 2;
    jbytes[mid] ^= 0xFF;
    fs::write(&jp, &jbytes).unwrap();

    let report = recovery::scan(&root).unwrap();
    let s = &report.sessions[0].streams[0];
    // corrupt journal は Finalized にならない。Unreadable なら
    // 全 review action を fail-closed で拒否する。
    assert_ne!(
        s.review_state,
        ReviewState::Finalized,
        "corrupt journal must not finalize: {s:?}"
    );
    if s.review_state == ReviewState::Unreadable {
        let mut faults = PublishFaults::new();
        for action in [
            ReviewAction::Adopt,
            ReviewAction::Hold,
            ReviewAction::Discard,
        ] {
            let err = review_stream(&area, 0, action, Some(&ClearProbe), &mut faults);
            assert!(err.is_err(), "{action:?} must fail closed: {err:?}");
        }
        assert!(media_path(&root, 223, 0).exists());
    } else {
        // Pending 系: Adopt が通る場合でも verified prefix のみ
        // Partial であり、corrupt 以降を Complete に昇格しない。
        let mut faults = PublishFaults::new();
        if let Ok(ReviewOutcome::Adopted(r)) =
            review_stream(&area, 0, ReviewAction::Adopt, None, &mut faults)
        {
            assert_eq!(r.outcome, FinalizedOutcome::Partial);
            assert!(r.source_frames <= 64);
        }
    }
    fs::remove_dir_all(&root).ok();
}
