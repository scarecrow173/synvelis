//! `recording-poc`: deterministic headless F0 scenario.
//!
//! Builds a project through `ProjectController`, registers a recording
//! session's entities, saves through `DirectoryBundleStore`, reloads, and
//! prints a deterministic summary. Two runs must produce identical
//! stdout (see `tests/poc_determinism.rs`).

use std::path::PathBuf;

use synvelis_project::{
    Command, CounterIdGenerator, DirectoryBundleStore, MusicalTick, Project, ProjectId, ProjectOp,
    SessionFrame, SessionRange, SourceFrame, SourceRange, TempoMapRef,
};
use synvelis_recording::{
    BufferBudget, CaptureTarget, GateSpec, MockInput, RecordingOp, RecordingRequest, Scenario,
    StagingArea, StagingSessionConfig, StagingStreamConfig, StreamWriter, SynvelisOp, finish_all,
    preflight, recovery, stream_config,
};

fn cmd<C: synvelis_project::Operation>(
    controller: &synvelis_project::ProjectController,
    op: C,
) -> Command<C> {
    Command {
        expected_revision: controller.live_revision(),
        op,
    }
}

macro_rules! apply {
    ($c:expr, $op:expr) => {
        $c.apply(cmd(&$c, $op)).expect("command must apply")
    };
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("vsc") {
        std::process::exit(vsc(&args[2..]));
    }
    if args.get(1).map(String::as_str) == Some("vsd") {
        std::process::exit(vsd(&args[2..]));
    }
    if let Err(e) = run() {
        eprintln!("recording-poc failed: {e}");
        std::process::exit(1);
    }
}

// ---- VS-C: staging writer / journal / crash recovery ----
//
// `recording-poc vsc record <root> <session> [--crash]`
//   決定的 mock 録音 → staging。`--crash` 時は checkpoint 後に
//   `process::exit(3)`（finalize/drop なしの process 終了を再現）。
// `recording-poc vsc recover <root> <session>`
//   reopen → journal replay + Last Valid Checkpoint まで検証 →
//   `finalized.json` を（未作成なら）公開。exit 0。
fn vsc(args: &[String]) -> i32 {
    let (sub, root, session) = match args {
        [sub, root, session] => (sub.as_str(), PathBuf::from(root), session.clone()),
        [sub, root, session, flag] if flag == "--crash" => {
            (sub.as_str(), PathBuf::from(root), session.clone())
        }
        _ => {
            eprintln!("usage: recording-poc vsc record|recover <root> <session> [--crash]");
            return 2;
        }
    };
    let crash = args.last().map(String::as_str) == Some("--crash");
    let Ok(raw) = session.parse::<u128>() else {
        eprintln!("invalid session id");
        return 2;
    };
    let session_id = synvelis_project::RecordingSessionId::new(raw);
    match sub {
        "record" => vsc_record(&root, session_id, crash),
        "recover" => vsc_recover(&root, session_id),
        _ => {
            eprintln!("unknown vsc subcommand: {sub}");
            2
        }
    }
}

#[allow(clippy::too_many_lines)] // sequential poc scenario
fn vsc_record(
    root: &std::path::Path,
    session_id: synvelis_project::RecordingSessionId,
    crash: bool,
) -> i32 {
    use synvelis_project::{ClockEpoch, ClockEpochId, Lane, LaneId, Track, TrackId};
    let lane = LaneId::new(11);
    let epoch = ClockEpochId::new(22);
    let mut p = Project::new(ProjectId::new(0), "vsc-poc");
    p.clock_epochs.insert(
        epoch,
        ClockEpoch {
            id: epoch,
            description: "mock".into(),
        },
    );
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
    let area = match StagingArea::create(
        root,
        &StagingSessionConfig {
            session_id,
            sample_rate: 48_000,
            streams: vec![stream_config(
                0,
                synvelis_project::CaptureStreamId::new(100),
                lane,
                2,
            )],
        },
    ) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("staging create failed: {e}");
            return 1;
        }
    };
    let stream_cfgs: Vec<StagingStreamConfig> = area
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
    let Ok(plan) = preflight(
        &p,
        RecordingRequest {
            targets: vec![CaptureTarget {
                lane_id: lane,
                channels: 2,
            }],
            clock_epoch: epoch,
            tempo_map: TempoMapRef { revision: 0 },
            transport_start: MusicalTick::ZERO,
            stop: MusicalTick::new(1920),
            gate: GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            budget: BufferBudget {
                chunks: 8,
                chunk_frames: 512,
                spans_per_chunk: 8,
                filled_capacity: 8,
                lookahead_spans: 16,
            },
            max_block_frames: 512,
        },
        48_000,
    ) else {
        eprintln!("preflight failed");
        return 1;
    };
    let a = area.clone();
    let mut sc: Scenario<StreamWriter> = Scenario::build_with(plan, move |i, ends, fault| {
        StreamWriter::create(&a, stream_cfgs[i as usize].clone(), ends, fault)
            .expect("writer create")
    });
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 256);
    for rig in &mut sc.rigs {
        if let Err(e) = rig.writer.checkpoint() {
            eprintln!("checkpoint failed: {e}");
            return 1;
        }
    }
    if crash {
        // checkpoint 後の finalize/drop 無しに process 終了（crash 再現）。
        std::process::exit(3);
    }
    let tag = session_id.to_tagged();
    let writers: Vec<&mut StreamWriter> = sc.rigs.iter_mut().map(|r| &mut r.writer).collect();
    let report = finish_all(&tag, 48_000, writers);
    for (i, r) in &report.refs {
        match r {
            Ok(s) => println!(
                "vsc_finalize stream={i} outcome={:?} frames={} bytes={} digest={}",
                s.outcome, s.source_frames, s.media_byte_len, s.digest
            ),
            Err(e) => {
                eprintln!("vsc_finalize stream={i} error={e}");
                return 1;
            }
        }
    }
    println!("result=OK");
    0
}

fn vsc_recover(root: &std::path::Path, session_id: synvelis_project::RecordingSessionId) -> i32 {
    let area = match StagingArea::open(root, session_id) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("staging open failed: {e}");
            return 1;
        }
    };
    let report = match recovery::scan(root) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("scan failed: {e}");
            return 1;
        }
    };
    for session in &report.sessions {
        for s in &session.streams {
            println!(
                "vsc_scan stream={} class={:?} durable={:?} tail_v={} tail_u={}",
                s.stream_index,
                s.class,
                s.durable.map(|d| d.media_len),
                s.validated_tail_bytes,
                s.unverified_tail_bytes
            );
        }
    }
    for (i, r) in recovery::recover_finalize(&area) {
        match r {
            Ok(s) => println!(
                "vsc_recover stream={i} outcome={:?} frames={} bytes={} digest={}",
                s.outcome, s.source_frames, s.media_byte_len, s.digest
            ),
            Err(synvelis_recording::WriterError::AlreadyFinalized) => {
                println!("vsc_recover stream={i} already_finalized");
            }
            Err(e) => {
                eprintln!("vsc_recover stream={i} error={e}");
                return 1;
            }
        }
    }
    println!("result=OK");
    0
}

// ---- VS-D: recording commit / idempotency / undo / reload ----
//
// `recording-poc vsd <root> <session>`
//   project ops → mock 録音 → staging finalize → commit_session →
//   save → reload → AlreadyApplied retry → undo/redo の決定的実演。
//   `vsd-report.json` を root 直下に出力し、stdout は key=value。
fn vsd(args: &[String]) -> i32 {
    let (root, session) = if let [root, session] = args {
        (PathBuf::from(root), session.clone())
    } else {
        eprintln!("usage: recording-poc vsd <root> <session>");
        return 2;
    };
    let Ok(raw) = session.parse::<u128>() else {
        eprintln!("invalid session id");
        return 2;
    };
    vsd_run(&root, synvelis_project::RecordingSessionId::new(raw))
}

#[allow(clippy::too_many_lines)] // sequential poc scenario
fn vsd_run(root: &std::path::Path, session_raw: synvelis_project::RecordingSessionId) -> i32 {
    use synvelis_project::{ClockEpochId, CounterIdGenerator, DirectoryBundleStore, LaneId};
    use synvelis_recording::{CommitOutcome, commit_session, commit_status, read_commit_marker};

    let _ = session_raw; // session 実体は controller が採番（決定的）。
    let bundle = root.join("project.synvelis");
    if bundle.exists() {
        let _ = std::fs::remove_dir_all(&bundle);
    }

    let mut controller = synvelis_project::ProjectController::new(
        Project::new(ProjectId::new(0), "vsd-poc"),
        Box::new(CounterIdGenerator::new(1)),
    );
    apply!(
        controller,
        SynvelisOp::Project(ProjectOp::AddTrack { name: "vsd".into() })
    );
    let track_id = *controller.live().tracks.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Project(ProjectOp::AddLane { track_id })
    );
    let lane_id: LaneId = *controller.live().lanes.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Project(ProjectOp::RegisterClockEpoch {
            description: "mock".into(),
        })
    );
    let epoch: ClockEpochId = *controller.live().clock_epochs.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::CreateSession {
            name: "vsd-session".into(),
        })
    );
    let session_id = *controller.live().recording.sessions.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::OpenSession { session_id })
    );
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::RegisterStream {
            session_id,
            lane_id,
            clock_epoch_id: epoch,
        })
    );
    let stream_id = *controller.live().recording.streams.keys().next().unwrap();
    let session_dir = root.join(session_id.to_tagged());
    if session_dir.exists() {
        let _ = std::fs::remove_dir_all(&session_dir);
    }

    let area = match StagingArea::create(
        root,
        &StagingSessionConfig {
            session_id,
            sample_rate: 48_000,
            streams: vec![stream_config(0, stream_id, lane_id, 2)],
        },
    ) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("staging create failed: {e}");
            return 1;
        }
    };
    let stream_cfgs: Vec<StagingStreamConfig> = area
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
    let Ok(plan) = preflight(
        controller.live(),
        RecordingRequest {
            targets: vec![CaptureTarget {
                lane_id,
                channels: 2,
            }],
            clock_epoch: epoch,
            tempo_map: TempoMapRef { revision: 0 },
            transport_start: MusicalTick::ZERO,
            stop: MusicalTick::new(1920),
            gate: GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            budget: BufferBudget {
                chunks: 8,
                chunk_frames: 512,
                spans_per_chunk: 8,
                filled_capacity: 8,
                lookahead_spans: 16,
            },
            max_block_frames: 512,
        },
        48_000,
    ) else {
        eprintln!("preflight failed");
        return 1;
    };
    let a = area.clone();
    let mut sc: Scenario<StreamWriter> = Scenario::build_with(plan, move |i, ends, fault| {
        StreamWriter::create(&a, stream_cfgs[i as usize].clone(), ends, fault)
            .expect("writer create")
    });
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 256);
    // capture 中に latch された engine 側 fault（queue 溢れ・epoch
    // 断絶等）は session 全体に属するため、driver が durable marker
    // として staging に記録する。commit は marker で全 stream を
    // recoverable に分類する（FaultLatch は in-memory のため）。
    if sc.fault.is_faulted()
        && let Err(e) = area.record_capture_fault(&format!("fault bits {}", sc.fault.bits()))
    {
        eprintln!("capture fault marker failed: {e}");
        return 1;
    }
    for rig in &mut sc.rigs {
        if let Err(e) = rig.writer.checkpoint() {
            eprintln!("checkpoint failed: {e}");
            return 1;
        }
    }
    let tag = session_id.to_tagged();
    let writers: Vec<&mut StreamWriter> = sc.rigs.iter_mut().map(|r| &mut r.writer).collect();
    let fin = finish_all(&tag, 48_000, writers);
    for (i, r) in &fin.refs {
        match r {
            Ok(s) => println!(
                "vsd_finalize stream={i} outcome={:?} frames={} digest={}",
                s.outcome, s.source_frames, s.digest
            ),
            Err(e) => {
                eprintln!("vsd_finalize stream={i} error={e}");
                return 1;
            }
        }
    }
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::CloseSession { session_id })
    );

    let report = match commit_session(&mut controller, &area) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("commit failed: {e}");
            return 1;
        }
    };
    match &report.outcome {
        CommitOutcome::Applied { revision } => {
            println!("vsd_commit outcome=applied rev={revision}");
        }
        other => {
            eprintln!("vsd_commit unexpected outcome: {other:?}");
            return 1;
        }
    }
    println!(
        "vsd_committed sources={} takes={} clips={} recoverable={}",
        report.sources_committed,
        report.takes_committed,
        report.clips_committed,
        report.recoverable.len()
    );
    println!("vsd_hash={}", report.content_hash);

    let mut store = match DirectoryBundleStore::create(&bundle) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("store create failed: {e}");
            return 1;
        }
    };
    if let Err(e) = controller.save(&mut store) {
        eprintln!("save failed: {e}");
        return 1;
    }

    let mut reloaded = synvelis_project::ProjectController::new(
        Project::new(ProjectId::new(0), "placeholder"),
        Box::new(CounterIdGenerator::new(1)),
    );
    if let Err(e) = reloaded.load(&store) {
        eprintln!("reload failed: {e}");
        return 1;
    }
    let roundtrip = serde_json::to_value(controller.live()).unwrap()
        == serde_json::to_value(reloaded.live()).unwrap();
    println!("vsd_reload_equal={roundtrip}");
    if !roundtrip {
        return 1;
    }
    // reload 後の retry: persisted registry が AlreadyApplied を返し、
    // 同一 manifest（同一 ID・同一 hash）で重複登録しない。
    let report2 = match commit_session(&mut reloaded, &area) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("retry failed: {e}");
            return 1;
        }
    };
    match &report2.outcome {
        CommitOutcome::AlreadyApplied => println!("vsd_retry outcome=already-applied"),
        other => {
            eprintln!("vsd_retry unexpected outcome: {other:?}");
            return 1;
        }
    }
    let committed_ids: Vec<String> = controller
        .live()
        .recording
        .takes
        .keys()
        .map(|t| t.to_tagged())
        .collect();
    println!("vsd_take_ids={}", committed_ids.join(","));

    // undo/redo（in-memory history）: Project 参照のみ戻し、staging
    // media は残る。
    let media_path = area.media_path(0);
    let media_bytes_before = media_path.metadata().map_or(0, |m| m.len());
    if !controller.undo().unwrap_or(false) {
        eprintln!("undo failed");
        return 1;
    }
    let undone = controller.live();
    println!(
        "vsd_undo sources={} takes={} clips={} sessions_finalized={} media_present={}",
        undone.sources.len(),
        undone.recording.takes.len(),
        undone.clips.len(),
        usize::from(
            undone
                .recording
                .sessions
                .values()
                .any(|s| s.state == synvelis_project::SessionState::Finalized)
        ),
        media_path.exists()
    );
    if !controller.redo().unwrap_or(false) {
        eprintln!("redo failed");
        return 1;
    }
    let redone_ids: Vec<String> = controller
        .live()
        .recording
        .takes
        .keys()
        .map(|t| t.to_tagged())
        .collect();
    let ids_same = committed_ids == redone_ids;
    println!(
        "vsd_redo sources={} ids_same={ids_same}",
        controller.live().sources.len()
    );
    if !ids_same {
        return 1;
    }
    let media_bytes_after = media_path.metadata().map_or(0, |m| m.len());
    println!(
        "vsd_media_preserved={}",
        media_bytes_before == media_bytes_after
    );
    if media_bytes_before != media_bytes_after {
        return 1;
    }
    let status = commit_status(controller.live(), session_id, &report.content_hash);
    println!("vsd_status_after_redo={status:?}");
    if let Some(m) = read_commit_marker(&area) {
        println!("vsd_marker outcome={} rev={}", m.outcome, m.revision);
    } else {
        eprintln!("commit marker missing");
        return 1;
    }

    // durable report（結果比較の正本）。
    let report_doc = serde_json::json!({
        "session": session_id.to_tagged(),
        "content_hash": report.content_hash,
        "sources_committed": report.sources_committed,
        "takes_committed": report.takes_committed,
        "clips_committed": report.clips_committed,
        "recoverable": report.recoverable.iter().map(|r| {
            serde_json::json!({
                "stream": r.stream_index, "reason": r.reason, "media": r.media,
            })
        }).collect::<Vec<_>>(),
        "retry_outcome": "already-applied",
        "reload_equal": roundtrip,
        "undo_redo_ids_same": ids_same,
        "media_preserved_bytes": media_bytes_after,
    });
    if let Err(e) = std::fs::write(
        root.join("vsd-report.json"),
        serde_json::to_vec_pretty(&report_doc).unwrap(),
    ) {
        eprintln!("report write failed: {e}");
        return 1;
    }
    println!("result=OK");
    0
}

#[allow(clippy::too_many_lines)] // sequential poc scenario
fn run() -> Result<(), synvelis_project::Error> {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("poc-out");
    let _ = std::fs::remove_dir_all(&out_dir);
    let mut store = DirectoryBundleStore::create(&out_dir)?;

    let mut controller = synvelis_project::ProjectController::new(
        Project::new(ProjectId::new(0), "vs-a-poc"),
        Box::new(CounterIdGenerator::new(1)),
    );

    apply!(
        controller,
        SynvelisOp::Project(ProjectOp::AddTrack {
            name: "drums".into(),
        })
    );
    let track_id = *controller.live().tracks.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Project(ProjectOp::AddLane { track_id })
    );
    let lane_id = *controller.live().lanes.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Project(ProjectOp::RegisterAsset {
            digest: "sha256:drums-take".into(),
        })
    );
    let asset_id = *controller.live().assets.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Project(ProjectOp::RegisterSource {
            asset_id,
            content_digest: "sha256:drums-take".into(),
        })
    );
    let source_id = *controller.live().sources.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Project(ProjectOp::RegisterClockEpoch {
            description: "audio-device-clock".into(),
        })
    );
    let clock_epoch_id = *controller.live().clock_epochs.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::CreateSession {
            name: "take-1".into(),
        })
    );
    let session_id = *controller.live().recording.sessions.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::OpenSession { session_id })
    );
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::RegisterPass {
            session_id,
            index: 0,
        })
    );
    let pass_id = *controller.live().recording.passes.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::RegisterStream {
            session_id,
            lane_id,
            clock_epoch_id,
        })
    );
    let stream_id = *controller.live().recording.streams.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::RegisterSegment {
            stream_id,
            pass_id,
            session_range: SessionRange::try_new(
                SessionFrame::new(0)?,
                SessionFrame::new(96_000)?,
            )?,
            source_range: SourceRange::try_new(SourceFrame::new(0)?, SourceFrame::new(96_000)?,)?,
            source_id,
        })
    );
    let segment_id = *controller.live().recording.segments.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::RegisterTake {
            session_id,
            pass_id,
            stream_id,
            lane_id,
            source_id,
            segment_ids: vec![segment_id],
        })
    );
    let take_id = *controller.live().recording.takes.keys().next().unwrap();
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::MarkTakeVerified { take_id })
    );
    apply!(
        controller,
        SynvelisOp::Recording(RecordingOp::CloseSession { session_id })
    );

    let saved = controller.save(&mut store)?;

    let mut reloaded = synvelis_project::ProjectController::new(
        Project::new(ProjectId::new(0), "placeholder"),
        Box::new(CounterIdGenerator::new(1)),
    );
    reloaded.load(&store)?;

    let same = serde_json::to_value(controller.live()).unwrap()
        == serde_json::to_value(reloaded.live()).unwrap();

    println!("project_id={}", controller.live().project_id);
    println!("saved_revision={}", saved.revision.0);
    println!("saved_head={}", saved.head);
    println!("reloaded_revision={}", reloaded.live_revision().0);
    println!("tracks={}", controller.live().tracks.len());
    println!("sessions={}", controller.live().recording.sessions.len());
    println!("takes={}", controller.live().recording.takes.len());
    println!("roundtrip_equal={same}");
    assert!(same, "reload mismatch");

    // ---- VS-B: Mock Clock/Input -> Gate -> RT Capture -> SPSC ----
    // 4-bar loop (15360 ticks = 384000f @48k/120bpm) を 3 pass 録音する。
    let plan = preflight(
        controller.live(),
        RecordingRequest {
            targets: vec![CaptureTarget {
                lane_id,
                channels: 2,
            }],
            clock_epoch: clock_epoch_id,
            tempo_map: TempoMapRef { revision: 0 },
            transport_start: MusicalTick::ZERO,
            stop: MusicalTick::new(46_080),
            gate: GateSpec::Loop {
                loop_start: MusicalTick::ZERO,
                loop_end: MusicalTick::new(15_360),
            },
            budget: BufferBudget {
                chunks: 8,
                chunk_frames: 40_000,
                spans_per_chunk: 8,
                filled_capacity: 8,
                lookahead_spans: 16,
            },
            max_block_frames: 40_000,
        },
        48_000,
    )?;
    let mut sc = Scenario::build(plan);
    let mut input = MockInput::new(clock_epoch_id, 0);
    sc.run_to_end(&mut input, 40_000);

    let (n_chunks, n_spans, frames, pass_sum, checksum, faulted, fault_bits) = {
        let log = &sc.rigs[0].writer.log;
        let spans: Vec<_> = log.iter().flat_map(|e| e.spans.iter()).collect();
        let frames: i64 = spans.iter().map(|s| i64::from(s.frame_count)).sum();
        let pass_sum: u64 = spans.iter().map(|s| u64::from(s.pass_index)).sum();
        // packed PCM の全 bit を fold した決定的チェックサム。
        let checksum: u64 = log
            .iter()
            .flat_map(|e| e.pcm.iter())
            .fold(0x9E37_79B9_7F4A_7C15, |acc, s| {
                acc.wrapping_mul(1_000_003) ^ u64::from(s.to_bits())
            });
        (
            log.len(),
            spans.len(),
            frames,
            pass_sum,
            checksum,
            sc.fault.is_faulted(),
            sc.fault.bits(),
        )
    };
    let recovered = sc.recover_all();
    println!("vsb_chunks={n_chunks}");
    println!("vsb_spans={n_spans}");
    println!("vsb_frames={frames}");
    println!("vsb_pass_sum={pass_sum}");
    println!("vsb_fault_bits={fault_bits:#x}");
    println!("vsb_recovered={}", recovered.len());
    println!("vsb_checksum={checksum:#x}");
    assert!(!faulted, "vs-b fault");
    println!("result=OK");
    Ok(())
}
