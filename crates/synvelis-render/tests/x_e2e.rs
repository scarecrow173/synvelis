//! E2E-X acceptance: mock-recorded PCM → non-RT writer/journal/recovery
//! → finalized verified source → canonical publish → project
//! commit/save/reload → the SAME generic resolver/graph/offline
//! pipeline as external WAV import (L1) and the SAME master export
//! (L2) — Issue #26, acceptance X-01..X-06.
//!
//! Deterministic evidence: every asserted output sample is re-derived
//! from `MockPcm` through the committed segment map (source frame →
//! session frame → input frame), so per-pass/take/segment
//! correspondence, clipping clamps and summing are all verified
//! bit-exact. No recording-specific playback path is exercised.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_reader::PrefetchConfig;

use synvelis_player::import::{blob_path, content_digest};
use synvelis_player::lowering::{EngineConfig, NodeContent, lower};
use synvelis_player::publish::publish_recorded;
use synvelis_player::resolver::prepare;
use synvelis_player::{HeadlessDriver, PumpPolicy};
use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, ClockEpochId, Command, CounterIdGenerator,
    DirectoryBundleStore, LaneId, MusicalDuration, MusicalTick, Operation, PlaybackMapping,
    Project, ProjectController, ProjectId, ProjectOp, ProjectRevision, RecordingSessionId,
    SourceFrame, SourceId, TempoMapRef, TimeAnchor,
};
use synvelis_recording::commit::{commit_session, read_commit_manifest};
use synvelis_recording::mock::{MockInput, MockPcm, Scenario};
use synvelis_recording::ops::RecordingOp;
use synvelis_recording::plan::{
    BufferBudget, CaptureTarget, GateSpec, RecordingRequest, preflight,
};
use synvelis_recording::recovery;
use synvelis_recording::staging::{StagingArea, StagingSessionConfig, StagingStreamConfig};
use synvelis_recording::writer::{FinishReport, StreamWriter, finish_all};
use synvelis_recording::{CommitOutcome, FinalizedOutcome, SynvelisOp};
use synvelis_render::{
    ExportJob, ExportOutcome, ExportRequest, OfflineRenderSupport, OutputHooks, RenderSnapshot,
    TailPolicy, export,
};

const RATE: u32 = 48_000;
const CHANNELS: u32 = 2;
/// 120 BPM @ PPQN 960 → 0.5 s per quarter → 25 frames per tick @48k.
const FRAMES_PER_TICK: i64 = 25;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

// ---------- helpers (recording side mirrors vs_d.rs) ----------

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "synvelis-x-{}-{}-{}",
        name,
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    if d.exists() {
        fs::remove_dir_all(&d).unwrap();
    }
    fs::create_dir_all(&d).unwrap();
    d
}

fn controller_with(lane_count: usize) -> (ProjectController, Vec<LaneId>, ClockEpochId) {
    let mut c = ProjectController::new(
        Project::new(ProjectId::new(0), "x"),
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

fn open_session(
    c: &mut ProjectController,
    lanes: &[LaneId],
    epoch: ClockEpochId,
) -> (RecordingSessionId, Vec<StagingStreamConfig>) {
    rop(
        c,
        RecordingOp::CreateSession {
            name: "x-session".into(),
        },
    );
    let session_id = *c.live().recording.sessions.keys().last().unwrap();
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

fn request(
    lanes: &[LaneId],
    epoch: ClockEpochId,
    gate: GateSpec,
    transport_start: i64,
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
        transport_start: MusicalTick::new(transport_start),
        stop: MusicalTick::new(stop_tick),
        gate,
        budget: BufferBudget {
            chunks: 16,
            chunk_frames: 40_000,
            spans_per_chunk: 8,
            filled_capacity: 16,
            lookahead_spans: 32,
        },
        max_block_frames: 40_000,
    }
}

/// `Normal{open:0}` gate, `transport_start=0` request shorthand.
fn normal_req(lanes: &[LaneId], epoch: ClockEpochId, stop_tick: i64) -> RecordingRequest {
    request(
        lanes,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        0,
        stop_tick,
    )
}

/// mock record → staging → checkpoint → finalize. Session left Open.
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

/// record → finalize → close → publish canonical → commit.
fn publish_and_commit(
    c: &mut ProjectController,
    root: &Path,
    media_root: &Path,
    session_id: RecordingSessionId,
    cfgs: &[StagingStreamConfig],
    req: RecordingRequest,
    epoch: ClockEpochId,
) -> (StagingArea, FinishReport) {
    let (area, fin) = record_and_finalize(c, root, session_id, cfgs, req, epoch, 40_000);
    for (_, r) in &fin.refs {
        assert!(r.is_ok(), "finalize failed: {r:?}");
    }
    rop(c, RecordingOp::CloseSession { session_id });
    let pub_report = publish_recorded(&area, media_root).expect("publish_recorded");
    assert_eq!(
        pub_report.published.len(),
        cfgs.len(),
        "all streams publish"
    );
    let report = commit_session(c, &area).expect("commit_session");
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    (area, fin)
}

fn save_reload(c: &mut ProjectController, root: &Path) -> ProjectController {
    let mut store = DirectoryBundleStore::create(root.join("bundle")).unwrap();
    c.save(&mut store).unwrap();
    let mut c2 = ProjectController::new(
        Project::new(ProjectId::new(0), "x"),
        Box::new(CounterIdGenerator::new(1)),
    );
    c2.load(&store).unwrap();
    c2
}

// ---------- playback/export helpers (mirror l1/l2 paths) ----------

fn prefetch(channels: u16, rate: u32) -> PrefetchConfig {
    PrefetchConfig {
        block_frames: 256,
        block_count: 8,
        channels: usize::from(channels),
        sample_rate: rate,
    }
}

fn engine(rate: u32, channels: u16, block: usize) -> EngineConfig {
    EngineConfig {
        sample_rate: rate,
        channels,
        block_frames: block,
    }
}

fn canonical() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(RATE).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn musical_placement(start_tick: i64, end_tick: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(start_tick)),
        extent: ClipExtent::Musical(MusicalDuration::new(end_tick - start_tick).unwrap()),
    }
}

/// clip の musical placement を `[start, end)` tick として返す。
fn clip_musical_span(clip: &synvelis_project::Clip) -> (i64, i64) {
    let TimeAnchor::Musical(start) = clip.placement.anchor else {
        panic!("recorded clips use Musical anchors")
    };
    let ClipExtent::Musical(dur) = clip.placement.extent else {
        panic!("recorded clips use Musical extents")
    };
    (start.value(), start.value() + dur.value())
}

/// Runs the generic playback path for `frames` engine frames and
/// returns the captured master planes.
fn play(c: &ProjectController, media_root: &Path, frames: u64) -> Vec<Vec<f32>> {
    let eng = engine(RATE, 2, 1024);
    let lowered = lower(c.live(), &|_| Some(canonical()), &eng);
    let prepared: Vec<_> = lowered
        .playbacks
        .iter()
        .map(|b| prepare(c.live(), b.source_id, media_root, &prefetch(2, RATE)).unwrap())
        .collect();
    let mut driver = HeadlessDriver::new(eng);
    driver.activate(&lowered, prepared).unwrap();
    let blocks = usize::try_from(frames.div_ceil(1024)).unwrap();
    let report = driver.run(blocks, PumpPolicy::Every(1)).unwrap();
    report
        .pcm
        .into_iter()
        .map(|ch| ch[..usize::try_from(frames).unwrap()].to_vec())
        .collect()
}

// ---------- deterministic evidence ----------

/// Maps a source frame of `source_id` to the mock input frame through
/// the committed segment map. Input frames equal session frames
/// because `MockInput::new(epoch, 0)` feeds frames 0..N continuously.
fn segment_input_frame(
    p: &synvelis_project::Project,
    source_id: SourceId,
    source_frame: i64,
) -> Option<i64> {
    let sf = SourceFrame::new(source_frame).ok()?;
    p.recording
        .segments
        .values()
        .find(|s| s.source_id == source_id && s.source_range.contains(sf))
        .map(|s| s.session_range.start().value() + (source_frame - s.source_range.start().value()))
}

/// Re-derives the expected master planes from `MockPcm` through the
/// committed clip→segment→session→input chain — the X-02 evidence.
fn expected_output(p: &synvelis_project::Project, frames: u64, channels: u32) -> Vec<Vec<f32>> {
    let n = usize::try_from(frames).unwrap();
    let mut out = vec![vec![0.0f32; n]; usize::try_from(channels).unwrap()];
    let pcm = MockPcm { seed: 0 };
    for clip in p.clips.values() {
        if clip.muted {
            continue;
        }
        let (tick_in, tick_out) = clip_musical_span(clip);
        let win_start = (tick_in * FRAMES_PER_TICK).max(0);
        let win_end = (tick_out * FRAMES_PER_TICK).max(0);
        for e in win_start.max(0)..win_end.min(i64::try_from(frames).unwrap()) {
            let sf = clip.playback.source_region.start().value() + (e - tick_in * FRAMES_PER_TICK);
            let Some(input_frame) = segment_input_frame(p, clip.source_id, sf) else {
                continue;
            };
            for (ch, plane) in out.iter_mut().enumerate() {
                plane[usize::try_from(e).unwrap()] += pcm.sample(
                    u64::try_from(input_frame).unwrap(),
                    u32::try_from(ch).unwrap(),
                );
            }
        }
    }
    out
}

fn assert_planes(got: &[Vec<f32>], want: &[Vec<f32>], label: &str) {
    assert_eq!(got.len(), want.len(), "{label}: channels");
    for (c, (g, w)) in got.iter().zip(want.iter()).enumerate() {
        assert_eq!(g.len(), w.len(), "{label}: ch{c} len");
        for (i, (a, b)) in g.iter().zip(w.iter()).enumerate() {
            assert_eq!(a, b, "{label}: ch{c} frame{i}: got {a}, want {b}");
        }
    }
}

/// Exports `[0, frames)` to F32 WAV and returns the reopened planes.
fn export_and_reopen(
    c: &ProjectController,
    media_root: &Path,
    dest: &Path,
    frames: u64,
) -> (Vec<Vec<f32>>, Box<synvelis_render::ExportReceipt>) {
    let snapshot = RenderSnapshot::freeze(c, media_root);
    let req = ExportRequest {
        engine: engine(RATE, 2, 1024),
        range_start: 0,
        range_end: frames,
        tail: Some(TailPolicy::None),
        preroll: 0,
        destination: dest.to_path_buf(),
        replace_existing: false,
        wire_format: SampleFormat::F32,
        inserts: Vec::new(),
        max_block: 1024,
        clip_support: OfflineRenderSupport::Verified,
        plugin_state: None,
    };
    let captured: Arc<Mutex<Option<Vec<Vec<f32>>>>> = Arc::new(Mutex::new(None));
    let cap = captured.clone();
    let hooks = OutputHooks {
        expect_pcm: Some(Box::new(move |planes: &[Vec<f32>]| {
            *cap.lock().unwrap() = Some(planes.to_vec());
            true
        })),
        ..OutputHooks::default()
    };
    let outcome = export(&snapshot, &req, hooks, &ExportJob::default());
    let ExportOutcome::Completed { receipt } = outcome else {
        panic!("export must complete");
    };
    assert!(receipt.verified.reopened, "independent reopen ran");
    (captured.lock().unwrap().clone().unwrap(), receipt)
}

// ---------- X-01: end-to-end bit-exact playback + export ----------

/// X-01: recorded source plays through the generic resolver/graph and
/// exports through master export with bit-exact sample integrity.
/// X-03 (identity/digest), X-06 (no recording-specific path) ride along.
#[test]
fn x_01_recorded_playback_export_bit_exact() {
    let root = scratch("x01");
    let media_root = root.join("media");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = normal_req(&lanes, epoch, 1920);
    let (area, fin) = publish_and_commit(&mut c, &root, &media_root, session_id, &cfgs, req, epoch);
    let raw_digest = fin.refs[0].1.as_ref().unwrap().digest.clone();

    // X-03: stable IDs + digest identity — the registered source names
    // the canonical blob; the manifest keeps raw provenance.
    let p = c.live();
    assert_eq!(p.sources.len(), 1);
    let (&source_id, source) = p.sources.iter().next().unwrap();
    let asset = &p.assets[&source.asset_id];
    assert_eq!(asset.digest, source.content_digest);
    let manifest = read_commit_manifest(&area).unwrap().unwrap();
    let entry = &manifest.payload.streams[0];
    assert_eq!(
        entry.digest, source.content_digest,
        "canonical digest in manifest"
    );
    assert_eq!(
        entry.raw_digest.as_deref(),
        Some(raw_digest.as_str()),
        "raw provenance kept"
    );
    let blob = blob_path(&media_root, &source.content_digest);
    assert!(blob.exists(), "canonical blob materialized");
    assert_eq!(
        content_digest(&fs::read(&blob).unwrap()),
        source.content_digest
    );
    assert!(
        area.canonical_path(0).exists(),
        "canonical receipt persisted"
    );

    // publish idempotency: a consistent receipt reuses — never
    // re-encodes; a missing blob re-materializes from durable raw
    // media with the identical canonical digest.
    let again = publish_recorded(&area, &media_root).unwrap();
    assert_eq!(again.reused.len(), 1);
    assert_eq!(again.published, Vec::new());
    fs::remove_file(&blob).unwrap();
    let again2 = publish_recorded(&area, &media_root).unwrap();
    assert_eq!(again2.reused.len(), 1);
    assert_eq!(again2.reused[0].digest, source.content_digest);
    assert!(blob.exists());
    assert_eq!(
        content_digest(&fs::read(&blob).unwrap()),
        source.content_digest
    );

    // save/reload — committed identities survive the store round trip.
    let takes_before: Vec<_> = p.recording.takes.keys().copied().collect();
    let clips_before: Vec<_> = p.clips.keys().copied().collect();
    let digest = source.content_digest.clone();
    let live_rev = c.live_revision();
    let c2 = &mut save_reload(&mut c, &root);
    assert_eq!(c2.live().sources[&source_id].content_digest, digest);
    assert_eq!(c2.live().recording.takes.len(), takes_before.len());
    assert_eq!(c2.live().clips.len(), clips_before.len());
    assert_eq!(c2.live_revision(), live_rev);

    // X-01 playback: 48_000 stored frames → engine window [0, 48_000).
    let got = play(c2, &media_root, 48_000);
    let want = expected_output(c2.live(), 48_000, CHANNELS);
    assert_planes(&got, &want, "x01 playback");
    // zero-fill check: not a single wrongly-flattened sample — the
    // generator never emits 0.0 here, so bit equality suffices.
    assert!(got.iter().flatten().any(|s| *s != 0.0));

    // Underrun is counted, never filled with content: starve the pump.
    let eng = engine(RATE, 2, 1024);
    let lowered = lower(c2.live(), &|_| Some(canonical()), &eng);
    let prepared: Vec<_> = lowered
        .playbacks
        .iter()
        .map(|b| prepare(c2.live(), b.source_id, &media_root, &prefetch(2, RATE)).unwrap())
        .collect();
    let NodeContent::Clip(clip_id) = lowered.playbacks[0].content else {
        panic!("first playback node is the committed clip")
    };
    let mut driver = HeadlessDriver::new(eng);
    driver.activate(&lowered, prepared).unwrap();
    let starved = driver.run(2, PumpPolicy::Every(0)).unwrap();
    assert!(starved.pcm.iter().flatten().all(|s| *s == 0.0));
    assert!(driver.stats_of(clip_id).unwrap().snapshot().underrun > 0);

    // X-01 export: same source, master export, reopened planes equal.
    let dest = root.join("export.wav");
    let (planes, _receipt) = export_and_reopen(c2, &media_root, &dest, 48_000);
    assert_planes(&planes, &want, "x01 export");
    // playback path and export path agree bit-exact (no special casing).
    assert_eq!(planes, got, "export == headless playback");

    // Export must not mutate the project (X-03 revision stability).
    assert_eq!(c2.live_revision(), live_rev);
}

// ---------- X-02: loop 3-pass + punch + pre-roll + offsets ----------

/// X-02: `PunchInLoop` with pre-roll transport commits per-pass segments;
/// each committed take reads back through the SAME source at its own
/// offset/placement with exact source→session→input correspondence.
#[test]
fn x_02_punch_in_loop_per_segment_correspondence() {
    let root = scratch("x02");
    let media_root = root.join("media");
    let (mut c, lanes, epoch) = controller_with(1);
    let lane = lanes[0];
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    // loop [0,15360) ×3 passes, punch window [3841,11519) stored,
    // transport rolls 384 ticks before tick 0 (pre-roll).
    let req = request(
        &lanes,
        epoch,
        GateSpec::PunchInLoop {
            loop_start: MusicalTick::new(0),
            loop_end: MusicalTick::new(15_360),
            punch_in: MusicalTick::new(3_841),
            punch_out: MusicalTick::new(11_519),
        },
        -384,
        46_080,
    );
    let (_area, _fin) =
        publish_and_commit(&mut c, &root, &media_root, session_id, &cfgs, req, epoch);

    // 1 source, 3 passes, 3 segments, 3 takes, auto-clip for pass 0.
    let p = c.live();
    assert_eq!(p.sources.len(), 1);
    assert_eq!(p.recording.passes.len(), 3);
    assert_eq!(p.recording.segments.len(), 3);
    assert_eq!(p.recording.takes.len(), 3);
    assert_eq!(p.clips.len(), 1);
    let (&source_id, _) = p.sources.iter().next().unwrap();

    // Segment structure: session windows are 384_000 frames apart
    // (one 15_360-tick pass each), source ranges tile contiguously.
    let mut segs: Vec<_> = p.recording.segments.values().collect();
    segs.sort_by_key(|s| s.source_range.start().value());
    for (i, s) in segs.iter().enumerate() {
        let (ss, se) = (
            s.session_range.start().value(),
            s.session_range.end().value(),
        );
        let (fs, fe) = (s.source_range.start().value(), s.source_range.end().value());
        let i = i64::try_from(i).unwrap();
        assert_eq!(fs, i * 191_950, "pass{i} source start tiles contiguously");
        assert_eq!(fe - fs, 191_950, "punch window is (11519-3841)*25 frames");
        // session start = punch tick → session frame, plus pass stride.
        assert_eq!(ss, (3_841 + 384) * FRAMES_PER_TICK + i * 384_000);
        assert_eq!(se - ss, 191_950);
    }

    // Auto-clip covers the pass-0 segment at its project placement.
    let auto_clip = p.clips.values().next().unwrap();
    assert_eq!(clip_musical_span(auto_clip), (3_841, 11_519));
    // The playback window is the whole captured segment range.
    assert_eq!(auto_clip.playback.source_region, segs[0].source_range);

    // X-02 composition: pass-1 take placed at [20000,27678), pass-2
    // take placed in pre-roll [-1920,5758) → negative clamp exercises
    // clip source offset + project placement.
    let (win1, win2) = (segs[1].source_range, segs[2].source_range);
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: musical_placement(20_000, 27_678),
            playback: ClipPlayback {
                source_region: win1,
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: musical_placement(-1_920, 5_758),
            playback: ClipPlayback {
                source_region: win2,
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    let c2 = &mut save_reload(&mut c, &root);
    assert_eq!(c2.live().clips.len(), 3);

    // Play [0, 27678*25) — pass-0 + pass-1 + clamped pass-2 regions.
    let frames = 27_678u64 * 25;
    let got = play(c2, &media_root, frames);
    let want = expected_output(c2.live(), frames, CHANNELS);
    assert_planes(&got, &want, "x02 playback");
    // each pass region non-silent → real per-pass evidence, not zeros.
    for (lo, hi, tag) in [
        (96_025usize, 120_000, "pass0"),
        (500_000usize, 520_000, "pass1"),
        (0usize, 60_000, "pass2-clamped"),
    ] {
        assert!(
            got[0][lo..hi].iter().any(|s| *s != 0.0),
            "{tag} region non-silent"
        );
    }

    // Same composition through master export — identical output.
    let dest = root.join("x02.wav");
    let (planes, _receipt) = export_and_reopen(c2, &media_root, &dest, frames);
    assert_planes(&planes, &want, "x02 export");
}

// ---------- X-03: revision/hash/commit identity + undo/redo ----------

/// X-03: commit identity, undo/redo and save/reload keep stable IDs and
/// digest linkage; playback+export never mutate project state.
#[test]
fn x_03_identity_revision_undo_save_reload() {
    let root = scratch("x03");
    let media_root = root.join("media");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = normal_req(&lanes, epoch, 960);
    let (_area, _fin) =
        publish_and_commit(&mut c, &root, &media_root, session_id, &cfgs, req, epoch);

    let p = c.live();
    let source_id = *p.sources.keys().next().unwrap();
    let digest = p.sources[&source_id].content_digest.clone();
    let take_ids: Vec<_> = p.recording.takes.keys().copied().collect();
    let clip_ids: Vec<_> = p.clips.keys().copied().collect();
    let applied_hash = p.recording.applied_commits[&session_id]
        .content_hash
        .clone();

    // undo → whole commit reverts; redo → identical stable IDs return.
    assert!(c.undo().unwrap());
    assert!(c.live().sources.is_empty());
    assert!(c.live().clips.is_empty());
    assert!(c.redo().unwrap());
    assert_eq!(c.live().sources[&source_id].content_digest, digest);
    assert_eq!(
        c.live().recording.takes.keys().copied().collect::<Vec<_>>(),
        take_ids
    );
    assert_eq!(c.live().clips.keys().copied().collect::<Vec<_>>(), clip_ids);

    // save/reload — registry hash identical → commit idempotent identity.
    let c2 = &mut save_reload(&mut c, &root);
    assert_eq!(
        c2.live().recording.applied_commits[&session_id].content_hash,
        applied_hash
    );
    assert_eq!(c2.live().sources[&source_id].content_digest, digest);

    // Resolve + play through the restored project — media untouched.
    let got = play(c2, &media_root, 24_000);
    assert_planes(
        &got,
        &expected_output(c2.live(), 24_000, CHANNELS),
        "x03 replay",
    );
}

/// Records a session that flows data through the writer but never
/// runs `finish_all` — no finalized ref exists (WIP media).
fn record_wip(
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
    let plan = preflight(c.live(), normal_req(lanes, epoch, 480), RATE).unwrap();
    let a = area.clone();
    let cfgs = cfgs.to_vec();
    let mut sc: Scenario<StreamWriter> = Scenario::build_with(plan, move |i, ends, fault| {
        StreamWriter::create(&a, cfgs[i as usize].clone(), ends, fault).unwrap()
    });
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 40_000);
    for rig in &mut sc.rigs {
        rig.writer.checkpoint().unwrap();
    }
    drop(sc);
    rop(c, RecordingOp::CloseSession { session_id });
    area
}

/// Records + finalizes, then torn-tails the journal and re-runs
/// `recover_finalize` — producing a Partial finalized ref.
fn record_truncated(
    c: &mut ProjectController,
    root: &Path,
    session_id: RecordingSessionId,
    cfgs: &[StagingStreamConfig],
    req: RecordingRequest,
    epoch: ClockEpochId,
) -> StagingArea {
    let (area, fin) = record_and_finalize(c, root, session_id, cfgs, req, epoch, 40_000);
    assert!(fin.refs[0].1.is_ok());
    let jpath = area.journal_path(0);
    let bytes = fs::read(&jpath).unwrap();
    fs::write(&jpath, &bytes[..bytes.len() - 8]).unwrap();
    fs::remove_file(area.finalized_path(0)).unwrap();
    let results = recovery::recover_finalize(&area);
    match &results[0].1 {
        Ok(r) => assert_eq!(r.outcome, FinalizedOutcome::Partial),
        Err(e) => panic!("recover_finalize failed: {e}"),
    }
    rop(c, RecordingOp::CloseSession { session_id });
    area
}

// ---------- X-04: finalized-only visibility + crash/truncation ----------

/// X-04: nothing but finalized+published+committed media can resolve —
/// uncommitted sessions, WIP streams, partial finalize after journal
/// truncation and crash-before-commit all fail closed.
#[test]
fn x_04_finalized_only_visibility() {
    let root = scratch("x04");
    let media_root = root.join("media");
    let (mut c, lanes, epoch) = controller_with(1);

    // (a) crash-before-commit: media published but never committed.
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = normal_req(&lanes, epoch, 960);
    let (area, fin) = record_and_finalize(&mut c, &root, session_id, &cfgs, req, epoch, 40_000);
    assert!(fin.refs[0].1.is_ok());
    rop(&mut c, RecordingOp::CloseSession { session_id });
    let pub1 = publish_recorded(&area, &media_root).expect("publish");
    assert_eq!(pub1.published.len(), 1);
    // blob exists, but no project source → resolve fails closed.
    assert_eq!(c.live().sources.len(), 0);
    let eng = engine(RATE, 2, 1024);
    let lowered = lower(c.live(), &|_| Some(canonical()), &eng);
    assert!(lowered.playbacks.is_empty(), "no clips without commit");

    // (b) WIP stream: a second session is recorded but never finalized.
    let (session2, cfgs2) = open_session(&mut c, &lanes, epoch);
    let area2 = record_wip(&mut c, &root.join("wip"), session2, &cfgs2, &lanes, epoch);
    let pub2 = publish_recorded(&area2, &media_root).expect("publish wip");
    assert_eq!(
        pub2.published.len() + pub2.reused.len(),
        0,
        "WIP never publishes"
    );
    assert_eq!(pub2.skipped.len(), 1);
    assert_eq!(pub2.skipped[0].reason, "missing-finalized");
    let report2 = commit_session(&mut c, &area2).expect("commit wip");
    assert_eq!(report2.recoverable.len(), 1);
    assert_eq!(c.live().sources.len(), 0, "WIP not committed");

    // (c) journal truncation → partial finalize → skipped, uncommitted.
    let (session3, cfgs3) = open_session(&mut c, &lanes, epoch);
    let req3 = normal_req(&lanes, epoch, 1920);
    let area3 = record_truncated(&mut c, &root.join("trunc"), session3, &cfgs3, req3, epoch);
    let pub3 = publish_recorded(&area3, &media_root).expect("publish partial");
    assert_eq!(
        pub3.published.len() + pub3.reused.len(),
        0,
        "partial never publishes"
    );
    assert_eq!(pub3.skipped.len(), 1);
    assert!(pub3.skipped[0].reason.starts_with("partial-finalize"));
    let report3 = commit_session(&mut c, &area3).expect("commit partial");
    assert_eq!(report3.recoverable.len(), 1);
    assert_eq!(c.live().sources.len(), 0, "partial media not committed");

    // (d) crash-after-commit + reload: only finalized media is exposed;
    // commit retry is idempotent and playback still works.
    let (session4, cfgs4) = open_session(&mut c, &lanes, epoch);
    let req4 = normal_req(&lanes, epoch, 960);
    let (area4, _fin4) = publish_and_commit(
        &mut c,
        &root.join("c4"),
        &media_root,
        session4,
        &cfgs4,
        req4,
        epoch,
    );
    assert_eq!(c.live().sources.len(), 1);
    let c2 = &mut save_reload(&mut c, &root.join("c4"));
    fs::remove_file(synvelis_recording::commit::commit_marker_path(&area4)).unwrap();
    let retry = commit_session(c2, &area4).expect("retry after reload");
    assert!(matches!(retry.outcome, CommitOutcome::AlreadyApplied));
    let got = play(c2, &media_root, 24_000);
    assert_planes(
        &got,
        &expected_output(c2.live(), 24_000, CHANNELS),
        "x04 replay",
    );

    // blobs/ holds only canonical digests — never raw staging media.
    // (a) and (d) recorded identical content → one deduplicated blob.
    let mut blob_names = Vec::new();
    for entry in fs::read_dir(media_root.join("blobs")).unwrap().flatten() {
        blob_names.push(entry.file_name().to_string_lossy().into_owned());
    }
    blob_names.sort();
    let committed_digest = c2
        .live()
        .sources
        .values()
        .next()
        .unwrap()
        .content_digest
        .clone();
    assert_eq!(
        blob_names,
        vec![committed_digest],
        "only canonical blobs published, content-address deduped"
    );
}

// ---------- X-05: fault classes stay typed/separate ----------

/// X-05: capture faults, engine underruns and export failures are
/// distinct typed outcomes — none collapse into fake success.
#[test]
fn x_05_fault_classes_separate() {
    let root = scratch("x05");
    let media_root = root.join("media");
    let (mut c, lanes, epoch) = controller_with(1);

    // capture fault → publish skips with fault reason, commit recoverable.
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    let req = normal_req(&lanes, epoch, 960);
    let (area, fin) = record_and_finalize(&mut c, &root, session_id, &cfgs, req, epoch, 40_000);
    assert!(fin.refs[0].1.is_ok());
    area.record_capture_fault("synthetic-capture-fault")
        .unwrap();
    rop(&mut c, RecordingOp::CloseSession { session_id });
    let pub_report = publish_recorded(&area, &media_root).expect("publish");
    assert_eq!(pub_report.published.len() + pub_report.reused.len(), 0);
    assert_eq!(pub_report.skipped.len(), 1);
    assert!(
        pub_report.skipped[0].reason.starts_with("capture-fault"),
        "typed fault reason, got {}",
        pub_report.skipped[0].reason
    );
    let report = commit_session(&mut c, &area).expect("commit");
    assert_eq!(report.recoverable.len(), 1);
    assert_eq!(c.live().sources.len(), 0);

    // export failure class: existing destination is a typed failure,
    // never a silent overwrite.
    let (session2, cfgs2) = open_session(&mut c, &lanes, epoch);
    let req2 = normal_req(&lanes, epoch, 480);
    let (_area2, _fin2) = publish_and_commit(
        &mut c,
        &root.join("x5b"),
        &media_root,
        session2,
        &cfgs2,
        req2,
        epoch,
    );
    let dest = root.join("exists.wav");
    fs::write(&dest, b"occupied").unwrap();
    let snapshot = RenderSnapshot::freeze(&c, &media_root);
    let req_export = ExportRequest {
        engine: engine(RATE, 2, 1024),
        range_start: 0,
        range_end: 12_000,
        tail: Some(TailPolicy::None),
        preroll: 0,
        destination: dest.clone(),
        replace_existing: false,
        wire_format: SampleFormat::F32,
        inserts: Vec::new(),
        max_block: 1024,
        clip_support: OfflineRenderSupport::Verified,
        plugin_state: None,
    };
    let outcome = export(
        &snapshot,
        &req_export,
        OutputHooks::default(),
        &ExportJob::default(),
    );
    match outcome {
        ExportOutcome::Failed { .. } => {}
        other => panic!("existing destination must fail typed, got {other:?}"),
    }
    assert_eq!(fs::read(&dest).unwrap(), b"occupied", "never overwritten");
}
