//! L2 acceptance: master sequential offline PCM WAV export →
//! independent reopen/verify → staged safe publish (Issue #25,
//! acceptance L2-01..08).
//!
//! Deterministic evidence: every expected sample is re-derived
//! analytically from `fixture_sample` through a model of the insert
//! chain and the render geometry (range/tail/preroll/PDC), each
//! failure class lands as a typed `ExportOutcome`, and `Completed`
//! is asserted only after a real io-wav file was reopened through an
//! independent reader and published.

use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use synvelis_player::EngineConfig;
use synvelis_player::fixture::{fixture_sample_f32, write_fixture};
use synvelis_player::import::{ImportOutcome, ImportRequest, import_local};
use synvelis_project::{
    ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, CompOp, CounterIdGenerator,
    MediaDuration, MediaTimeValue, MusicalDuration, MusicalTick, PlaybackMapping, Project,
    ProjectController, ProjectId, ProjectOp, ProjectRevision, SourceFrame, SourceId, SourceRange,
    TimeAnchor,
};
use synvelis_recording::ops::RecordingOp;

use ruviform_audio_core::SampleFormat;
use ruviform_media_io::{Error as MediaError, ErrorKind, MediaSink};

use synvelis_render::{
    ExportError, ExportJob, ExportOutcome, ExportRequest, ExportStage, Insert,
    OfflineRenderSupport, OutputHooks, PcmExpect, PreflightError, RenderSnapshot, TailPolicy,
    export,
};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-l2-{name}-{}-{}",
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(9), "l2"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn cmd(c: &ProjectController, op: ProjectOp) -> Command<ProjectOp> {
    Command {
        expected_revision: c.live_revision(),
        op,
    }
}

fn apply(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    c.apply(cmd(c, op)).unwrap()
}

fn media_placement(start: i64, end: i64, rate: u32) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Absolute(MediaTimeValue::new(start, 1, rate).unwrap()),
        extent: ClipExtent::Absolute(MediaDuration::new(end - start, 1, rate).unwrap()),
    }
}

fn natural_playback(offset: u64, frames: u64) -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::new(i64::try_from(offset).unwrap()).unwrap(),
            SourceFrame::new(i64::try_from(offset + frames).unwrap()).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

fn import(c: &mut ProjectController, root: &Path, path: &Path) -> ImportOutcome {
    import_local(
        c,
        &ImportRequest {
            path: path.to_path_buf(),
            staging_root: root.join("staging"),
            media_root: root.join("media"),
            resume: None,
        },
    )
    .unwrap()
}

fn lane(c: &mut ProjectController) -> synvelis_project::LaneId {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(c, ProjectOp::AddLane { track_id: track });
    *c.live().lanes.keys().next().unwrap()
}

fn add_clip(
    c: &mut ProjectController,
    lane: synvelis_project::LaneId,
    source_id: SourceId,
    start: i64,
    end: i64,
    rate: u32,
    offset: u64,
) -> ClipId {
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: media_placement(start, end, rate),
            playback: natural_playback(offset, u64::try_from(end - start).unwrap()),
        },
    );
    *c.live().clips.keys().next_back().unwrap()
}

fn engine(rate: u32, channels: u16, block: usize) -> EngineConfig {
    EngineConfig {
        sample_rate: rate,
        channels,
        block_frames: block,
    }
}

fn freeze(c: &ProjectController, root: &Path) -> RenderSnapshot {
    RenderSnapshot::freeze(c, root.join("media"))
}

#[allow(clippy::too_many_arguments)]
fn request(
    destination: PathBuf,
    engine: EngineConfig,
    range_start: u64,
    range_end: u64,
    tail: Option<TailPolicy>,
    preroll: u64,
    wire_format: SampleFormat,
    inserts: Vec<Insert>,
    max_block: u32,
) -> ExportRequest {
    ExportRequest {
        engine,
        range_start,
        range_end,
        tail,
        preroll,
        destination,
        replace_existing: false,
        wire_format,
        inserts,
        max_block,
        clip_support: OfflineRenderSupport::Verified,
    }
}

/// Leftover staged temp files in `dir` (`.rvf-tmp-` tag).
fn leftover_temps(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().contains(".rvf-tmp-"))
        })
        .collect()
}

// ------------------------------------------------------------------
// Analytic expectation model
// ------------------------------------------------------------------

/// A clip in engine-window terms — mirrors `ClipWindow` math from
/// `lowering`: negative anchor start clamps and shifts the source.
#[derive(Clone, Copy)]
struct ClipModel {
    engine_start: u64,
    engine_len: u64,
    source_start: u64,
    src_channels: usize,
}

fn clip_model(start: i64, end: i64, offset: u64, src_channels: usize) -> ClipModel {
    let engine_start = start.max(0);
    ClipModel {
        engine_start: u64::try_from(engine_start).unwrap(),
        engine_len: u64::try_from((end - engine_start).max(0)).unwrap(),
        source_start: offset + u64::try_from(engine_start - start).unwrap(),
        src_channels,
    }
}

fn mix_at(clips: &[ClipModel], ch: usize, pos: u64) -> f32 {
    clips
        .iter()
        .filter(|c| pos >= c.engine_start && pos < c.engine_start + c.engine_len)
        .map(|c| {
            let src = ch.min(c.src_channels - 1);
            fixture_sample_f32(
                src,
                usize::try_from(c.source_start + pos - c.engine_start).unwrap(),
            )
        })
        .sum()
}

#[allow(clippy::cast_possible_truncation)]
fn gain_lin(db: f64) -> f32 {
    10.0_f64.powf(db / 20.0) as f32
}

/// Per-channel sequential insert state.
enum St {
    Gain(f32),
    OnePole {
        a: f32,
        s: f32,
        line: Vec<f32>,
        cursor: usize,
    },
    Delay {
        ring: Vec<f32>,
        cursor: usize,
    },
}

fn states_for(inserts: &[Insert]) -> Vec<St> {
    inserts
        .iter()
        .map(|i| match *i {
            Insert::Gain { gain_db } => St::Gain(gain_lin(gain_db)),
            Insert::OnePole { a, latency } => St::OnePole {
                a,
                s: 0.0,
                line: vec![0.0; usize::try_from(latency).unwrap()],
                cursor: 0,
            },
            Insert::Delay { frames } => St::Delay {
                ring: vec![0.0; usize::try_from(frames).unwrap()],
                cursor: 0,
            },
        })
        .collect()
}

/// Simulates the sequential render analytically: processes positions
/// `[play_pos, last_pos]` through the insert chain per channel and
/// collects the emitted window `[range_start, range_start + emitted)`
/// where emitted position = processed position − `total_latency`.
fn simulate(
    clips: &[ClipModel],
    inserts: &[Insert],
    channels: usize,
    play_pos: u64,
    last_pos: u64,
    total_latency: u64,
    range_start: u64,
) -> Vec<Vec<f32>> {
    let mut out = vec![Vec::new(); channels];
    let mut states: Vec<Vec<St>> = (0..channels).map(|_| states_for(inserts)).collect();
    for pos in play_pos..=last_pos {
        for ch in 0..channels {
            let mut x = mix_at(clips, ch, pos);
            for st in &mut states[ch] {
                match st {
                    St::Gain(g) => x *= *g,
                    St::OnePole { a, s, line, cursor } => {
                        *s = *s * *a + x;
                        x = if line.is_empty() {
                            *s
                        } else {
                            let d = line[*cursor];
                            line[*cursor] = *s;
                            *cursor = (*cursor + 1) % line.len();
                            d
                        };
                    }
                    St::Delay { ring, cursor } => {
                        if !ring.is_empty() {
                            let d = ring[*cursor];
                            ring[*cursor] = x;
                            *cursor = (*cursor + 1) % ring.len();
                            x = d;
                        }
                    }
                }
            }
            let emitted =
                i64::try_from(pos).unwrap_or(i64::MAX) - i64::try_from(total_latency).unwrap();
            if emitted >= i64::try_from(range_start).unwrap() {
                out[ch].push(x);
            }
        }
    }
    out
}

/// Applies the wire conversion to expected planes: integer PCM wire
/// formats saturate to the representable range — `S16` clamps to
/// `[-1.0, 32767/32768]` (and every test value sits on the i16 grid,
/// so the round-trip stays exact). `F32` is lossless.
fn wire_apply(planes: &mut [Vec<f32>], wire: SampleFormat) {
    if wire == SampleFormat::S16 {
        for p in planes.iter_mut() {
            for s in p.iter_mut() {
                *s = s.clamp(-1.0, 32767.0 / 32768.0);
            }
        }
    }
}

/// Total latency a request's inserts will report (only `OnePole`
/// declares processing latency; `Delay` is an effect).
fn insert_latency(inserts: &[Insert]) -> u64 {
    inserts
        .iter()
        .map(|i| match *i {
            Insert::OnePole { latency, .. } => u64::from(latency),
            _ => 0,
        })
        .sum()
}

fn assert_planes(got: &[Vec<f32>], want: &[Vec<f32>], eps: f32, label: &str) {
    assert_eq!(got.len(), want.len(), "{label}: channel count");
    for (c, (g, w)) in got.iter().zip(want.iter()).enumerate() {
        assert_eq!(g.len(), w.len(), "{label}: ch{c} len");
        for (i, (a, b)) in g.iter().zip(w.iter()).enumerate() {
            assert!(
                (a - b).abs() <= eps,
                "{label}: ch{c} frame{i}: got {a}, want {b}"
            );
        }
    }
}

/// `expect_pcm` hook that captures the reopened planes and compares
/// them to `expected` when set.
fn pcm_hook(
    expected: Option<Vec<Vec<f32>>>,
    eps: f32,
    capture: Arc<Mutex<Option<Vec<Vec<f32>>>>>,
) -> PcmExpect {
    Box::new(move |planes| {
        *capture.lock().unwrap() = Some(planes.to_vec());
        if let Some(want) = &expected {
            if planes.len() != want.len() {
                eprintln!("PCM mismatch: channels {} != {}", planes.len(), want.len());
                return false;
            }
            for (ch, (g, w)) in planes.iter().zip(want.iter()).enumerate() {
                if g.len() != w.len() {
                    eprintln!("PCM mismatch: ch{ch} len {} != {}", g.len(), w.len());
                    return false;
                }
                for (i, (a, b)) in g.iter().zip(w.iter()).enumerate() {
                    if (a - b).abs() > eps {
                        eprintln!("PCM mismatch: ch{ch} frame{i}: got {a}, want {b}");
                        return false;
                    }
                }
            }
        }
        true
    })
}

fn completed(outcome: ExportOutcome) -> Box<synvelis_render::ExportReceipt> {
    match outcome {
        ExportOutcome::Completed { receipt } => receipt,
        other => panic!("expected Completed, got {other:?}"),
    }
}

fn failed(outcome: ExportOutcome) -> (ExportStage, ExportError) {
    match outcome {
        ExportOutcome::Failed { stage, error } => (stage, error),
        other => panic!("expected Failed, got {other:?}"),
    }
}

// ---------------------------------------------------------------- L2-01

/// L2-01: 48 kHz stereo + mono deterministic fixtures render to a
/// real io-wav file; the independent reopen matches rate, layout,
/// frame count and every PCM sample. Fixture hash and geometry are
/// recorded on the receipt.
#[test]
fn l2_01_master_export_reopen_pcm_match() {
    let root = scratch("l201");
    let wav = root.join("stereo.wav");
    let bytes = write_fixture(&wav, 2, 48_000, 4_800).unwrap();
    let mut c = controller();
    let out = import(&mut c, &root, &wav);
    let source_id = out.commit.source_id;
    let lane = lane(&mut c);
    // Clip window [64, 764) reading source [13, 713).
    add_clip(&mut c, lane, source_id, 64, 764, 48_000, 13);

    let snapshot = freeze(&c, &root);
    assert_eq!(
        snapshot.ids.source_digests[&source_id],
        synvelis_player::import::content_digest(&bytes)
    );
    let clips = vec![clip_model(64, 764, 13, 2)];
    let dest = root.join("out-stereo.wav");
    let req = request(
        dest.clone(),
        engine(48_000, 2, 256),
        0,
        1_536,
        Some(TailPolicy::None),
        0,
        SampleFormat::S16,
        Vec::new(),
        256,
    );
    let mut expected = simulate(&clips, &[], 2, 0, 1_535, 0, 0);
    wire_apply(&mut expected, req.wire_format);
    let capture = Arc::new(Mutex::new(None));
    let hooks = OutputHooks {
        expect_pcm: Some(pcm_hook(Some(expected.clone()), 0.0, Arc::clone(&capture))),
        ..OutputHooks::default()
    };
    let job = ExportJob::new();
    let receipt = completed(export(&snapshot, &req, hooks, &job));
    assert_eq!(receipt.render.frames_emitted, 1_536);
    assert_eq!(receipt.render.range_start, 0);
    assert_eq!(receipt.render.range_end_with_tail, 1_536);
    assert!(receipt.verified.reopened);
    assert!(receipt.verified.pcm_checked);
    assert_eq!(receipt.verified.frames, 1_536);
    assert_eq!(receipt.verified.channels, 2);
    assert_eq!(receipt.verified.sample_rate, 48_000);
    assert_eq!(receipt.snapshot.live_revision, snapshot.ids.live_revision);
    assert_eq!(receipt.snapshot.source_digests, snapshot.ids.source_digests);
    assert!(dest.exists());
    let planes = capture.lock().unwrap().clone().unwrap();
    assert_planes(&planes, &expected, 0.0, "stereo");
    assert_eq!(leftover_temps(&root), Vec::<PathBuf>::new());

    // Mono path: mono source on a mono engine.
    let mwav = root.join("mono.wav");
    write_fixture(&mwav, 1, 48_000, 2_400).unwrap();
    let out2 = import(&mut c, &root, &mwav);
    add_clip(&mut c, lane, out2.commit.source_id, 0, 1_024, 48_000, 96);
    let snapshot2 = freeze(&c, &root);
    let clips2 = vec![clip_model(64, 764, 13, 2), clip_model(0, 1_024, 96, 1)];
    let mdest = root.join("out-mono.wav");
    let mreq = request(
        mdest.clone(),
        engine(48_000, 1, 256),
        0,
        1_536,
        Some(TailPolicy::None),
        0,
        SampleFormat::S16,
        Vec::new(),
        256,
    );
    // Mono engine: stereo clip maps channel 0 → source channel 0.
    let mut expected2 = simulate(&clips2, &[], 1, 0, 1_535, 0, 0);
    wire_apply(&mut expected2, mreq.wire_format);
    let hooks2 = OutputHooks {
        expect_pcm: Some(pcm_hook(Some(expected2), 0.0, Arc::new(Mutex::new(None)))),
        ..OutputHooks::default()
    };
    let job2 = ExportJob::new();
    let receipt2 = completed(export(&snapshot2, &mreq, hooks2, &job2));
    assert_eq!(receipt2.verified.channels, 1);
    assert_eq!(receipt2.render.frames_emitted, 1_536);
    assert!(mdest.exists());
}

// ---------------------------------------------------------------- L2-02

/// L2-02: clip starting before zero (clamped head, shifted source),
/// silent gaps, mid-source offsets and a muted clip render exact
/// output frames — no off-by-one, no fabricated content.
#[test]
fn l2_02_clip_boundaries_gaps_muted() {
    let root = scratch("l202");
    let wav = root.join("s.wav");
    write_fixture(&wav, 2, 48_000, 4_800).unwrap();
    let mut c = controller();
    let out = import(&mut c, &root, &wav);
    let source_id = out.commit.source_id;
    let lane = lane(&mut c);
    // Before-zero start: anchor [-100,400) → engine [0,400), src [100,500).
    add_clip(&mut c, lane, source_id, -100, 400, 48_000, 0);
    // Mid-source: engine [600,900) reading source [96,396).
    add_clip(&mut c, lane, source_id, 600, 900, 48_000, 96);
    // Muted clip in [900,1200) — lowers to silence.
    let muted = add_clip(&mut c, lane, source_id, 900, 1_200, 48_000, 0);
    apply(
        &mut c,
        ProjectOp::SetClipMuted {
            clip_id: muted,
            muted: true,
        },
    );

    let snapshot = freeze(&c, &root);
    let clips = vec![clip_model(-100, 400, 0, 2), clip_model(600, 900, 96, 2)];
    let dest = root.join("out.wav");
    let req = request(
        dest,
        engine(48_000, 2, 256),
        0,
        1_536,
        Some(TailPolicy::None),
        0,
        SampleFormat::S16,
        Vec::new(),
        256,
    );
    let mut expected = simulate(&clips, &[], 2, 0, 1_535, 0, 0);
    wire_apply(&mut expected, req.wire_format);
    let hooks = OutputHooks {
        expect_pcm: Some(pcm_hook(Some(expected), 0.0, Arc::new(Mutex::new(None)))),
        ..OutputHooks::default()
    };
    let job = ExportJob::new();
    let receipt = completed(export(&snapshot, &req, hooks, &job));
    assert_eq!(receipt.render.frames_emitted, 1_536);
}

/// L2-02 (range edges): a clip ending exactly at `range_end` emits its
/// last frame at `range_end - 1`; clips wholly outside the range emit
/// silence; a mid-stream range with preroll clamped at zero stays exact.
#[test]
fn l2_02_range_edges_and_preroll_clamp() {
    let root = scratch("l202e");
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, 48_000, 6_000).unwrap();
    let mut c = controller();
    let out = import(&mut c, &root, &wav);
    let source_id = out.commit.source_id;
    let lane = lane(&mut c);
    // Ends exactly at range_end = 1536: last content frame at 1535.
    add_clip(&mut c, lane, source_id, 1_400, 1_536, 48_000, 7);
    // Wholly past the range — processed but never emitted.
    add_clip(&mut c, lane, source_id, 2_000, 2_400, 48_000, 0);

    let snapshot = freeze(&c, &root);
    let clips = vec![
        clip_model(1_400, 1_536, 7, 1),
        clip_model(2_000, 2_400, 0, 1),
    ];
    let dest = root.join("edge.wav");
    let req = request(
        dest,
        engine(48_000, 1, 256),
        0,
        1_536,
        Some(TailPolicy::None),
        0,
        SampleFormat::S16,
        Vec::new(),
        256,
    );
    let mut expected = simulate(&clips, &[], 1, 0, 1_535, 0, 0);
    wire_apply(&mut expected, req.wire_format);
    let hooks = OutputHooks {
        expect_pcm: Some(pcm_hook(Some(expected), 0.0, Arc::new(Mutex::new(None)))),
        ..OutputHooks::default()
    };
    let job = ExportJob::new();
    completed(export(&snapshot, &req, hooks, &job));

    // Preroll larger than range_start clamps the play position at 0:
    // a negative play position is never issued.
    let dest2 = root.join("mid.wav");
    let req2 = request(
        dest2,
        engine(48_000, 1, 256),
        100,
        700,
        Some(TailPolicy::None),
        400, // preroll > range_start → play_pos clamps to 0
        SampleFormat::S16,
        Vec::new(),
        256,
    );
    let mut expected2 = simulate(&clips, &[], 1, 0, 699, 0, 100);
    wire_apply(&mut expected2, req2.wire_format);
    let hooks2 = OutputHooks {
        expect_pcm: Some(pcm_hook(Some(expected2), 0.0, Arc::new(Mutex::new(None)))),
        ..OutputHooks::default()
    };
    let job2 = ExportJob::new();
    let receipt2 = completed(export(&snapshot, &req2, hooks2, &job2));
    assert_eq!(receipt2.render.frames_emitted, 600);
    assert_eq!(receipt2.render.range_start, 100);
}

// ---------------------------------------------------------------- L2-03

/// L2-03: gain + stateful one-pole + delay on the master bus are
/// sample-identical across 64/128/256/whole-range block partitions —
/// sequential state, never chunked or parallelized. Wire format F32
/// keeps the comparison exact.
#[test]
fn l2_03_stateful_inserts_block_invariant() {
    let root = scratch("l203");
    let wav = root.join("s.wav");
    write_fixture(&wav, 2, 48_000, 4_800).unwrap();
    let mut c = controller();
    let out = import(&mut c, &root, &wav);
    let lane = lane(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 128, 1_408, 48_000, 0);
    let snapshot = freeze(&c, &root);

    let inserts = vec![
        Insert::Gain { gain_db: -6.0 },
        Insert::OnePole { a: 0.5, latency: 0 },
        Insert::Delay { frames: 64 },
    ];
    let clips = vec![clip_model(128, 1_408, 0, 2)];
    let latency = insert_latency(&inserts);
    // Delay tail: content ends at 1408, delayed 64 → tail frames to 1472.
    let emit_end = 1_536_u64 + 64;
    let expected = simulate(&clips, &inserts, 2, 0, emit_end - 1 + latency, latency, 0);

    let mut reference: Option<Vec<Vec<f32>>> = None;
    for block in [2_048_u32, 64, 128, 256] {
        let dest = root.join(format!("b{block}.wav"));
        let req = request(
            dest,
            engine(48_000, 2, 256),
            0,
            1_536,
            Some(TailPolicy::Known(64)),
            0,
            SampleFormat::F32,
            inserts.clone(),
            block,
        );
        let capture = Arc::new(Mutex::new(None));
        let hooks = OutputHooks {
            expect_pcm: Some(pcm_hook(Some(expected.clone()), 1e-7, Arc::clone(&capture))),
            ..OutputHooks::default()
        };
        let job = ExportJob::new();
        let receipt = completed(export(&snapshot, &req, hooks, &job));
        assert_eq!(receipt.render.frames_emitted, emit_end);
        let planes = capture.lock().unwrap().clone().unwrap();
        if let Some(want) = &reference {
            assert_planes(&planes, want, 0.0, &format!("block {block}"));
        } else {
            reference = Some(planes);
        }
    }
}

/// L2-03 (semantics): `Insert::Delay { frames: 0 }` is an honest
/// pass-through — `y[n] = x[n−0]` — not a silent one-frame shift.
#[test]
fn l2_03_delay_zero_is_identity() {
    let root = scratch("l203zero");
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, 48_000, 4_800).unwrap();
    let mut c = controller();
    let out = import(&mut c, &root, &wav);
    let lane = lane(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 512, 48_000, 0);
    let snapshot = freeze(&c, &root);

    let inserts = vec![Insert::Delay { frames: 0 }];
    let clips = vec![clip_model(0, 512, 0, 1)];
    let expected = simulate(&clips, &inserts, 1, 0, 511, 0, 0);
    let dest = root.join("id.wav");
    let req = request(
        dest,
        engine(48_000, 1, 256),
        0,
        512,
        Some(TailPolicy::None),
        0,
        SampleFormat::F32,
        inserts,
        256,
    );
    let capture = Arc::new(Mutex::new(None));
    let hooks = OutputHooks {
        expect_pcm: Some(pcm_hook(Some(expected), 0.0, Arc::clone(&capture))),
        ..OutputHooks::default()
    };
    let job = ExportJob::new();
    let receipt = completed(export(&snapshot, &req, hooks, &job));
    assert_eq!(receipt.render.frames_emitted, 512);
    assert_planes(
        &capture.lock().unwrap().clone().unwrap(),
        &[(0..512)
            .map(|i| fixture_sample_f32(0, i))
            .collect::<Vec<f32>>()],
        0.0,
        "delay(0) identity",
    );
}

// ---------------------------------------------------------------- L2-04

/// L2-04: PDC realignment — a one-pole declaring 96-frame latency is
/// delayed 96 frames in output space, and the emit shift relabels it
/// onto its input positions: `emitted[p] = filtered[p]`, the emitted
/// window is not inflated, and `total_latency` is on the receipt.
#[test]
fn l2_04_pdc_realigns_not_inflates() {
    let root = scratch("l204pdc");
    let wav = root.join("s.wav");
    write_fixture(&wav, 2, 48_000, 4_800).unwrap();
    let mut c = controller();
    let out = import(&mut c, &root, &wav);
    let lane = lane(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 512, 48_000, 0);
    let snapshot = freeze(&c, &root);

    let inserts = vec![Insert::OnePole {
        a: 0.5,
        latency: 96,
    }];
    let clips = vec![clip_model(0, 512, 0, 2)];
    let expected = simulate(&clips, &inserts, 2, 0, 511 + 96, 96, 0);
    let dest = root.join("pdc.wav");
    let req = request(
        dest,
        engine(48_000, 2, 256),
        0,
        512,
        Some(TailPolicy::None),
        0,
        SampleFormat::F32,
        inserts,
        256,
    );
    let hooks = OutputHooks {
        expect_pcm: Some(pcm_hook(Some(expected), 1e-7, Arc::new(Mutex::new(None)))),
        ..OutputHooks::default()
    };
    let job = ExportJob::new();
    let receipt = completed(export(&snapshot, &req, hooks, &job));
    assert_eq!(receipt.capability.total_latency, 96);
    // Emitted window is exactly [0,512) — PDC never inflates it.
    assert_eq!(receipt.render.frames_emitted, 512);
    assert_eq!(receipt.render.range_end_with_tail, 512);
}

/// L2-04 (preroll): warm-up is a semantic — with a stateful filter,
/// `preroll = range_start` reproduces the from-zero render; `preroll =
/// 0` starts the filter cold at `range_start` and must differ.
#[test]
fn l2_04_preroll_is_state_semantics() {
    let root = scratch("l204pre");
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, 48_000, 4_800).unwrap();
    let mut c = controller();
    let out = import(&mut c, &root, &wav);
    let lane = lane(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 800, 48_000, 0);
    let snapshot = freeze(&c, &root);

    let inserts = vec![Insert::OnePole { a: 0.5, latency: 0 }];
    let clips = vec![clip_model(0, 800, 0, 1)];
    // preroll 200 == play_pos 0: full warm-up before [200,600).
    let warm = simulate(&clips, &inserts, 1, 0, 599, 0, 200);
    let dest = root.join("warm.wav");
    let req = request(
        dest,
        engine(48_000, 1, 256),
        200,
        600,
        Some(TailPolicy::None),
        200,
        SampleFormat::F32,
        inserts.clone(),
        256,
    );
    let hooks = OutputHooks {
        expect_pcm: Some(pcm_hook(
            Some(warm.clone()),
            1e-7,
            Arc::new(Mutex::new(None)),
        )),
        ..OutputHooks::default()
    };
    let job = ExportJob::new();
    completed(export(&snapshot, &req, hooks, &job));

    // preroll 0 → play_pos 200: filter state starts cold at 200.
    let cold = simulate(&clips, &inserts, 1, 200, 599, 0, 200);
    let dest2 = root.join("cold.wav");
    let req2 = request(
        dest2,
        engine(48_000, 1, 256),
        200,
        600,
        Some(TailPolicy::None),
        0,
        SampleFormat::F32,
        inserts,
        256,
    );
    let capture = Arc::new(Mutex::new(None));
    let hooks2 = OutputHooks {
        expect_pcm: Some(pcm_hook(Some(cold.clone()), 1e-7, Arc::clone(&capture))),
        ..OutputHooks::default()
    };
    let job2 = ExportJob::new();
    completed(export(&snapshot, &req2, hooks2, &job2));
    let planes = capture.lock().unwrap().clone().unwrap();
    assert_planes(&planes, &cold, 1e-7, "cold");
    // Warm-up genuinely changed the filter state → outputs differ.
    assert_ne!(planes, warm);
}

/// L2-04 (tail): `Delay` content past `range_end` only lands when a
/// known tail extends the emitted window — `Known(64)` captures the
/// delayed tail exactly.
#[test]
fn l2_04_known_tail_captures_delay() {
    let root = scratch("l204tail");
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, 48_000, 4_800).unwrap();
    let mut c = controller();
    let out = import(&mut c, &root, &wav);
    let lane = lane(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 512, 48_000, 0);
    let snapshot = freeze(&c, &root);

    let inserts = vec![Insert::Delay { frames: 64 }];
    let clips = vec![clip_model(0, 512, 0, 1)];
    let expected = simulate(&clips, &inserts, 1, 0, 512 + 64 - 1, 0, 0);
    let dest = root.join("tail.wav");
    let req = request(
        dest,
        engine(48_000, 1, 256),
        0,
        512,
        Some(TailPolicy::Known(64)),
        0,
        SampleFormat::F32,
        inserts,
        256,
    );
    let hooks = OutputHooks {
        expect_pcm: Some(pcm_hook(Some(expected), 1e-7, Arc::new(Mutex::new(None)))),
        ..OutputHooks::default()
    };
    let job = ExportJob::new();
    let receipt = completed(export(&snapshot, &req, hooks, &job));
    assert_eq!(receipt.render.frames_emitted, 576);
    assert_eq!(receipt.render.range_end_with_tail, 576);
    assert_eq!(
        receipt.capability.tail_evidence,
        ruviform_audio::render::TailEvidence::Known
    );

    // An *estimated* tail emits the same bounded window but the
    // receipt honestly reports `Estimated`, not `Known`.
    let req = request(
        root.join("tail_est.wav"),
        engine(48_000, 1, 256),
        0,
        512,
        Some(TailPolicy::Estimated(64)),
        0,
        SampleFormat::F32,
        vec![Insert::Delay { frames: 64 }],
        256,
    );
    let job = ExportJob::new();
    let receipt = completed(export(&snapshot, &req, OutputHooks::default(), &job));
    assert_eq!(receipt.render.frames_emitted, 576);
    assert_eq!(
        receipt.capability.tail_evidence,
        ruviform_audio::render::TailEvidence::Estimated
    );
}

/// L2-04 (rejects): unclassified tail and below-Verified processor
/// support are explicit preflight failures — never a degraded render.
#[test]
#[allow(clippy::too_many_lines)] // one table of refusal cases
fn l2_04_unknown_tail_and_support_rejected() {
    let root = scratch("l204rej");
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, 48_000, 1_024).unwrap();
    let mut c = controller();
    let out = import(&mut c, &root, &wav);
    let lane = lane(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 512, 48_000, 0);
    let snapshot = freeze(&c, &root);

    // Unknown tail — the export cannot bound the emitted window.
    let dest = root.join("u.wav");
    let mut req = request(
        dest.clone(),
        engine(48_000, 1, 256),
        0,
        512,
        None,
        0,
        SampleFormat::S16,
        Vec::new(),
        256,
    );
    let job = ExportJob::new();
    let (stage, err) = failed(export(&snapshot, &req, OutputHooks::default(), &job));
    assert_eq!(stage, ExportStage::Preflight);
    assert!(matches!(
        err,
        ExportError::Preflight(PreflightError::UnknownTail)
    ));
    assert!(!dest.exists());
    assert_eq!(leftover_temps(&root), Vec::<PathBuf>::new());

    // Declared/Unknown offline support → the capability gate refuses.
    for support in [
        OfflineRenderSupport::Declared,
        OfflineRenderSupport::Unknown,
    ] {
        let dest = root.join(format!("s{support:?}.wav"));
        req.destination = dest.clone();
        req.tail = Some(TailPolicy::None);
        req.clip_support = support;
        let job = ExportJob::new();
        let (stage, err) = failed(export(&snapshot, &req, OutputHooks::default(), &job));
        assert_eq!(stage, ExportStage::Preflight);
        assert!(
            matches!(
                err,
                ExportError::Render(ruviform_audio::render::RenderError::Preflight(_))
            ),
            "support {support:?}: {err}"
        );
        assert!(!dest.exists());
    }

    // Channel geometry outside the mono/stereo slice refuses — never
    // a silent stereo coercion.
    for channels in [0_u16, 4] {
        let dest = root.join(format!("ch{channels}.wav"));
        req.destination = dest.clone();
        req.tail = Some(TailPolicy::None);
        req.clip_support = OfflineRenderSupport::Verified;
        req.engine.channels = channels;
        let job = ExportJob::new();
        let (stage, err) = failed(export(&snapshot, &req, OutputHooks::default(), &job));
        assert_eq!(stage, ExportStage::Preflight);
        assert!(
            matches!(
                err,
                ExportError::Preflight(PreflightError::InsufficientCapability { .. })
            ),
            "channels {channels}: {err}"
        );
        assert!(!dest.exists());
    }
    req.engine.channels = 1;

    // `TailPolicy::None` against an insert chain that provably carries
    // a tail is a self-inconsistent request — refuse, never drop the
    // delayed tail while reporting `None`.
    let dest = root.join("droptail.wav");
    req.destination = dest.clone();
    req.inserts = vec![Insert::Delay { frames: 64 }];
    let job = ExportJob::new();
    let (stage, err) = failed(export(&snapshot, &req, OutputHooks::default(), &job));
    assert_eq!(stage, ExportStage::Preflight);
    assert!(
        matches!(
            err,
            ExportError::Preflight(PreflightError::InvalidRange { .. })
        ),
        "{err}"
    );
    assert!(!dest.exists());
    req.inserts = Vec::new();

    // Unbounded magnitudes refuse instead of aborting the allocator.
    for (block, inserts) in [
        (0_u32, Vec::new()),
        ((1 << 20) + 1, Vec::new()),
        (
            256_u32,
            vec![Insert::Delay {
                frames: (1 << 20) + 1,
            }],
        ),
        (
            256_u32,
            vec![Insert::OnePole {
                a: 0.5,
                latency: (1 << 20) + 1,
            }],
        ),
    ] {
        let dest = root.join(format!("mag{block}-{}.wav", inserts.len()));
        req.destination = dest.clone();
        req.max_block = block;
        req.inserts = inserts;
        let job = ExportJob::new();
        let (stage, err) = failed(export(&snapshot, &req, OutputHooks::default(), &job));
        assert_eq!(stage, ExportStage::Preflight);
        assert!(
            matches!(
                err,
                ExportError::Preflight(PreflightError::InvalidRange { .. })
            ),
            "block {block}: {err}"
        );
        assert!(!dest.exists());
    }
    req.max_block = 256;
    req.inserts = Vec::new();
}

/// L2-02 (rejects): a non-muted clip that cannot be lowered — a
/// musical anchor whose tick conversion overflows — is an explicit
/// preflight failure, never a clip silently absent from the output.
#[test]
fn l2_02_unresolvable_anchor_refused() {
    let root = scratch("l202anchor");
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, 48_000, 1_024).unwrap();
    let mut c = controller();
    let out = import(&mut c, &root, &wav);
    let lane = lane(&mut c);
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: out.commit.source_id,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(0)),
                extent: ClipExtent::Musical(MusicalDuration::new(i64::MAX).unwrap()),
            },
            playback: natural_playback(0, 1_024),
        },
    );
    let snapshot = freeze(&c, &root);
    let dest = root.join("out.wav");
    let req = request(
        dest.clone(),
        engine(48_000, 1, 256),
        0,
        512,
        Some(TailPolicy::None),
        0,
        SampleFormat::S16,
        Vec::new(),
        256,
    );
    let job = ExportJob::new();
    let (stage, err) = failed(export(&snapshot, &req, OutputHooks::default(), &job));
    assert_eq!(stage, ExportStage::Preflight);
    assert!(
        matches!(
            err,
            ExportError::Preflight(PreflightError::UnrenderableClip { .. })
        ),
        "{err}"
    );
    assert!(!dest.exists());
    assert_eq!(leftover_temps(&root), Vec::<PathBuf>::new());
}

/// L2-02 (rejects): an all-muted project lowers to zero clip nodes —
/// refusing is honest; publishing a silent file would pretend content
/// was rendered.
#[test]
fn l2_02_all_muted_refused() {
    let root = scratch("l202muted");
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, 48_000, 1_024).unwrap();
    let mut c = controller();
    let out = import(&mut c, &root, &wav);
    let lane = lane(&mut c);
    let clip = add_clip(&mut c, lane, out.commit.source_id, 0, 512, 48_000, 0);
    apply(
        &mut c,
        ProjectOp::SetClipMuted {
            clip_id: clip,
            muted: true,
        },
    );
    let snapshot = freeze(&c, &root);
    let dest = root.join("out.wav");
    let req = request(
        dest.clone(),
        engine(48_000, 1, 256),
        0,
        512,
        Some(TailPolicy::None),
        0,
        SampleFormat::S16,
        Vec::new(),
        256,
    );
    let job = ExportJob::new();
    let (stage, err) = failed(export(&snapshot, &req, OutputHooks::default(), &job));
    assert_eq!(stage, ExportStage::Preflight);
    assert!(
        matches!(err, ExportError::Preflight(PreflightError::NothingToRender)),
        "{err}"
    );
    assert!(!dest.exists());
    assert_eq!(leftover_temps(&root), Vec::<PathBuf>::new());
}

// ---------------------------------------------------------------- L2-05

/// L2-05 (destination policy): an existing destination refuses when
/// `replace_existing` is false — before any staging — and the file is
/// preserved byte-for-byte; `replace_existing` publishes over it.
#[test]
fn l2_05_destination_policy() {
    let root = scratch("l205");
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, 48_000, 1_024).unwrap();
    let mut c = controller();
    let out = import(&mut c, &root, &wav);
    let lane = lane(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 512, 48_000, 0);
    let snapshot = freeze(&c, &root);

    let dest = root.join("out.wav");
    fs::write(&dest, b"original bytes").unwrap();
    let mut req = request(
        dest.clone(),
        engine(48_000, 1, 256),
        0,
        512,
        Some(TailPolicy::None),
        0,
        SampleFormat::S16,
        Vec::new(),
        256,
    );
    let job = ExportJob::new();
    let (stage, err) = failed(export(&snapshot, &req, OutputHooks::default(), &job));
    assert_eq!(stage, ExportStage::Preflight);
    assert!(matches!(
        err,
        ExportError::Preflight(PreflightError::DestinationConflict { .. })
    ));
    assert_eq!(fs::read(&dest).unwrap(), b"original bytes");
    assert_eq!(leftover_temps(&root), Vec::<PathBuf>::new());

    req.replace_existing = true;
    let job2 = ExportJob::new();
    let receipt = completed(export(&snapshot, &req, OutputHooks::default(), &job2));
    assert_eq!(receipt.render.frames_emitted, 512);
    assert_ne!(fs::read(&dest).unwrap(), b"original bytes");
}

/// A write-budget sink: writes succeed until `budget` bytes, then fail
/// — the "disk full"/"writer full" class.
struct BoundedSink {
    file: fs::File,
    budget: u64,
}

impl Write for BoundedSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.budget == 0 {
            return Err(std::io::Error::other("disk full"));
        }
        let take = usize::try_from(
            self.budget
                .min(u64::try_from(buf.len()).unwrap_or(u64::MAX)),
        )
        .unwrap_or(usize::MAX);
        let n = self.file.write(&buf[..take])?;
        self.budget -= u64::try_from(n).unwrap_or(u64::MAX);
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

impl Seek for BoundedSink {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(pos)
    }
}

/// A sink whose `flush` (close/sync half of finalize) always fails.
struct FailFlushSink {
    file: fs::File,
}

impl Write for FailFlushSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("flush failed"))
    }
}

impl Seek for FailFlushSink {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(pos)
    }
}

fn basic_fixture(root: &Path) -> (ProjectController, SourceId) {
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, 48_000, 1_024).unwrap();
    let mut c = controller();
    let out = import(&mut c, root, &wav);
    let lane = lane(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 512, 48_000, 0);
    (c, out.commit.source_id)
}

fn basic_request(dest: PathBuf) -> ExportRequest {
    request(
        dest,
        engine(48_000, 1, 256),
        0,
        512,
        Some(TailPolicy::None),
        0,
        SampleFormat::S16,
        Vec::new(),
        256,
    )
}

/// L2-05 (fault injection): sink-full, flush/close failure, verify
/// failure, publish failure and pre-start cancellation all land as
/// typed non-`Completed` outcomes; every staged temp is cleaned and an
/// existing destination stays byte-for-byte intact.
#[test]
fn l2_05_fault_injection_partial_never_completed() {
    let root = scratch("l205f");
    let (c, _src) = basic_fixture(&root);
    let snapshot = freeze(&c, &root);

    // Writer full mid-stream — the staged file dies, no destination.
    let dest = root.join("full.wav");
    let req = basic_request(dest.clone());
    let hooks = OutputHooks {
        wrap_sink: Some(Box::new(|file| {
            Box::new(BoundedSink { file, budget: 128 }) as Box<dyn MediaSink>
        })),
        ..OutputHooks::default()
    };
    let job = ExportJob::new();
    let (stage, _e) = failed(export(&snapshot, &req, hooks, &job));
    assert!(matches!(
        stage,
        ExportStage::Output | ExportStage::Finalize | ExportStage::Drain
    ));
    assert!(!dest.exists());
    assert_eq!(leftover_temps(&root), Vec::<PathBuf>::new());

    // Flush/close failure at finalize.
    let dest = root.join("close.wav");
    let req = basic_request(dest.clone());
    let hooks = OutputHooks {
        wrap_sink: Some(Box::new(|file| {
            Box::new(FailFlushSink { file }) as Box<dyn MediaSink>
        })),
        ..OutputHooks::default()
    };
    let job = ExportJob::new();
    let (stage, _e) = failed(export(&snapshot, &req, hooks, &job));
    assert!(
        matches!(stage, ExportStage::Finalize | ExportStage::Publish),
        "flush failure stage {stage:?}"
    );
    assert!(!dest.exists());
    assert_eq!(leftover_temps(&root), Vec::<PathBuf>::new());

    // Reopen verify failure — PCM expectation refuses the file.
    let dest = root.join("verify.wav");
    let req = basic_request(dest.clone());
    let hooks = OutputHooks {
        expect_pcm: Some(Box::new(|_| false)),
        ..OutputHooks::default()
    };
    let job = ExportJob::new();
    let (stage, err) = failed(export(&snapshot, &req, hooks, &job));
    assert_eq!(stage, ExportStage::Verify);
    assert!(matches!(
        err,
        ExportError::Render(ruviform_audio::render::RenderError::Verify { .. })
    ));
    assert!(!dest.exists());
    assert_eq!(leftover_temps(&root), Vec::<PathBuf>::new());

    // Publish failure — everything succeeded until the commit step.
    let dest = root.join("publish.wav");
    fs::write(&dest, b"keep me").unwrap();
    let mut req = basic_request(dest.clone());
    req.replace_existing = true;
    let hooks = OutputHooks {
        publish: Some(Box::new(|_, _| Err(MediaError::from_kind(ErrorKind::Io)))),
        ..OutputHooks::default()
    };
    let job = ExportJob::new();
    let (stage, err) = failed(export(&snapshot, &req, hooks, &job));
    assert_eq!(stage, ExportStage::Publish);
    assert!(matches!(
        err,
        ExportError::Render(ruviform_audio::render::RenderError::Publish { .. })
    ));
    assert_eq!(fs::read(&dest).unwrap(), b"keep me");
    assert_eq!(leftover_temps(&root), Vec::<PathBuf>::new());

    // Cancellation requested before the export runs — honored at the
    // earliest boundary, the source-preparation pass.
    let dest = root.join("cancel.wav");
    let req = basic_request(dest.clone());
    let job = ExportJob::new();
    job.cancel.cancel();
    match export(&snapshot, &req, OutputHooks::default(), &job) {
        ExportOutcome::Cancelled { phase } => assert_eq!(phase, "prepare"),
        other => panic!("expected Cancelled, got {other:?}"),
    }
    assert!(!dest.exists());
    assert_eq!(leftover_temps(&root), Vec::<PathBuf>::new());
}

// ---------------------------------------------------------------- L2-06

/// L2-06: the render input is a frozen snapshot — edits after
/// `freeze` are invisible to the render, export mutates neither the
/// live revision nor undo/redo history, and the source digest on the
/// receipt is the freeze-time content version.
#[test]
fn l2_06_snapshot_freeze_and_immutability() {
    let root = scratch("l206");
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, 48_000, 4_800).unwrap();
    let mut c = controller();
    let out = import(&mut c, &root, &wav);
    let source_id = out.commit.source_id;
    let lane = lane(&mut c);
    add_clip(&mut c, lane, source_id, 0, 512, 48_000, 0);

    // Freeze BEFORE adding a second clip — the render must not see it.
    let snapshot = freeze(&c, &root);
    let frozen_rev = snapshot.ids.live_revision;
    add_clip(&mut c, lane, source_id, 600, 900, 48_000, 400);
    assert_ne!(c.live_revision(), frozen_rev);

    let clips = vec![clip_model(0, 512, 0, 1)]; // snapshot geometry only
    let dest = root.join("snap.wav");
    let req = basic_request(dest);
    let mut expected = simulate(&clips, &[], 1, 0, 511, 0, 0);
    wire_apply(&mut expected, req.wire_format);
    let hooks = OutputHooks {
        expect_pcm: Some(pcm_hook(Some(expected), 0.0, Arc::new(Mutex::new(None)))),
        ..OutputHooks::default()
    };
    let live_before = c.live_revision();
    let job = ExportJob::new();
    let receipt = completed(export(&snapshot, &req, hooks, &job));

    // Export mutated nothing: live revision and source digests intact.
    assert_eq!(c.live_revision(), live_before);
    assert_eq!(
        receipt.snapshot.source_digests[&source_id],
        snapshot.ids.source_digests[&source_id]
    );
    assert_eq!(receipt.snapshot.live_revision, frozen_rev);
    // Undo history unchanged — the post-freeze clip still undoes
    // (undo applies forward, it does not rewind the counter), leaving
    // exactly the clip the snapshot froze.
    assert!(c.undo().unwrap());
    assert_eq!(c.live().clips.len(), 1);
}

// ---------------------------------------------------------------- L2-07

/// L2-07: a completed export carries the capability evidence —
/// `wav_capabilities()` wire formats, per-node `offline_support` all
/// Verified, total latency, tail evidence and container facts.
#[test]
fn l2_07_capability_evidence_on_receipt() {
    let root = scratch("l207");
    let (c, _src) = basic_fixture(&root);
    let snapshot = freeze(&c, &root);
    let dest = root.join("cap.wav");
    let req = basic_request(dest);
    let job = ExportJob::new();
    let receipt = completed(export(&snapshot, &req, OutputHooks::default(), &job));

    let caps = &receipt.capability;
    assert!(caps.wav_sample_formats.iter().any(|f| f == "S16"));
    assert!(caps.wav_sample_formats.iter().any(|f| f == "F32"));
    assert!(!caps.wav_rf64);
    assert!(caps.wav_custom_layouts);
    assert!(!caps.node_support.is_empty());
    assert!(
        caps.node_support
            .values()
            .all(|s| *s == OfflineRenderSupport::Verified)
    );
    assert_eq!(caps.total_latency, 0);
    assert_eq!(
        caps.tail_evidence,
        ruviform_audio::render::TailEvidence::None
    );
    // Backend boundary capability: io-wav muxes/encodes PCM only.
    let backend = ruviform_io_wav::capabilities();
    assert!(backend.can_mux());
    assert!(backend.can_encode(ruviform_media_io::CodecId::Pcm));
    assert!(!backend.can_demux());
}

// ------------------------------------------------------------------
// MD-B: Effective Comp through the offline render path (issue #50)
// ------------------------------------------------------------------

fn gapply<O: synvelis_project::Operation>(c: &mut ProjectController, op: O) -> ProjectRevision {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .expect("op applies")
}

fn mr(start: i64, end: i64) -> synvelis_project::ProjectRange {
    synvelis_project::ProjectRange::Musical(
        synvelis_project::MusicalRange::try_new(MusicalTick::new(start), MusicalTick::new(end))
            .unwrap(),
    )
}

/// Builds the canonical MD-B comp project for render verification:
/// one imported mono source, one lane, one take over `source
/// [0,60_000)`, an active comp `[0,960) Take@0` / `[960,1920)
/// ExistingSource@2000` / `[1920,2000) Silence` with a `[900,1020)`
/// transition, plus a lane clip inside the target range that must be
/// superseded — never double-played into the export.
#[allow(clippy::too_many_lines)] // sequential comp construction
fn comp_fixture(root: &Path) -> (ProjectController, SourceId) {
    let wav = root.join("comp.wav");
    write_fixture(&wav, 1, 48_000, 60_000).unwrap();
    let mut c = controller();
    let out = import(&mut c, root, &wav);
    let source_id = out.commit.source_id;
    let lane = lane(&mut c);
    gapply(
        &mut c,
        ProjectOp::RegisterClockEpoch {
            description: "clk".into(),
        },
    );
    let epoch = *c.live().clock_epochs.keys().next().unwrap();

    gapply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: media_placement(0, 48_000, 48_000),
            playback: natural_playback(12_000, 48_000),
        },
    );

    gapply(&mut c, RecordingOp::CreateSession { name: "s".into() });
    let session = *c.live().recording.sessions.keys().next().unwrap();
    gapply(
        &mut c,
        RecordingOp::OpenSession {
            session_id: session,
        },
    );
    gapply(
        &mut c,
        RecordingOp::RegisterPass {
            session_id: session,
            index: 0,
        },
    );
    let pass = *c.live().recording.passes.keys().next().unwrap();
    gapply(
        &mut c,
        RecordingOp::RegisterStream {
            session_id: session,
            lane_id: lane,
            clock_epoch_id: epoch,
        },
    );
    let stream = *c.live().recording.streams.keys().next().unwrap();
    gapply(
        &mut c,
        RecordingOp::RegisterSegment {
            stream_id: stream,
            pass_id: pass,
            session_range: synvelis_project::SessionRange::try_new(
                synvelis_project::SessionFrame::new(0).unwrap(),
                synvelis_project::SessionFrame::new(60_000).unwrap(),
            )
            .unwrap(),
            source_range: SourceRange::try_new(
                SourceFrame::ZERO,
                SourceFrame::new(60_000).unwrap(),
            )
            .unwrap(),
            source_id,
        },
    );
    let segment = *c.live().recording.segments.keys().next().unwrap();
    gapply(
        &mut c,
        RecordingOp::RegisterTake {
            session_id: session,
            pass_id: pass,
            stream_id: stream,
            lane_id: lane,
            source_id,
            segment_ids: vec![segment],
        },
    );
    let take_id = *c.live().recording.takes.keys().next().unwrap();
    gapply(&mut c, RecordingOp::MarkTakeVerified { take_id });

    gapply(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: lane,
            target_range: mr(0, 2_000),
            take_ids: vec![take_id],
            source_rate: 48_000,
        },
    );
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    gapply(
        &mut c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();
    for (range, candidate) in [
        (
            mr(0, 960),
            synvelis_project::CompCandidateRef::Take {
                take_id,
                source_offset: SourceFrame::ZERO,
            },
        ),
        (
            mr(960, 1_920),
            synvelis_project::CompCandidateRef::ExistingSource {
                source_id,
                handle: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(60_000).unwrap())
                    .unwrap(),
                play_from: SourceFrame::new(2_000).unwrap(),
            },
        ),
        (
            mr(1_920, 2_000),
            synvelis_project::CompCandidateRef::Silence,
        ),
    ] {
        gapply(
            &mut c,
            CompOp::AddCompRegion {
                comp_id: comp,
                project_range: range,
                candidate,
                mapping: PlaybackMapping::Natural,
            },
        );
    }
    gapply(
        &mut c,
        CompOp::SetActiveComp {
            take_group_id: group,
            comp_id: Some(comp),
        },
    );
    let regions = c.live().comp.comps[&comp].region_ids.clone();
    gapply(
        &mut c,
        CompOp::AddCompTransition {
            left_region_id: regions[0],
            right_region_id: regions[1],
            overlap_range: mr(900, 1_020),
            fade_out: synvelis_project::FadeShape::EqualPower,
            fade_in: synvelis_project::FadeShape::Linear,
        },
    );
    (c, source_id)
}

/// Expected mono mix of the comp fixture at engine frame `f`
/// (25 frames/tick): r0 base at unity inside the overlap pre-side, the
/// right overhang fading in, r1 base plus the left overhang fading
/// out, silence region and beyond at zero.
#[allow(clippy::cast_possible_truncation)]
#[allow(clippy::cast_precision_loss)] // engine frames stay far under 2^53
fn comp_expected(f: i64) -> f32 {
    let (o_start, o_end, boundary) = (22_500_i64, 25_500_i64, 24_000_i64);
    let fx = |frame: i64| fixture_sample_f32(0, usize::try_from(frame).unwrap());
    if f < o_start {
        fx(f)
    } else if f < boundary {
        let t = (f - o_start) as f64 / (boundary - o_start) as f64;
        fx(f) + fx(500 + (f - o_start)) * (t as f32)
    } else if f < o_end {
        let t = (f - boundary) as f64 / (o_end - boundary) as f64;
        fx(2_000 + (f - boundary))
            + fx(f) * (((1.0 - t) * std::f64::consts::FRAC_PI_2).sin() as f32)
    } else if f < 48_000 {
        fx(2_000 + (f - boundary))
    } else {
        0.0
    }
}

/// MD-B: the active comp's Effective projection exports through the
/// offline path — adopted segments at unity, faded transition
/// overhangs, the silence region as silence, and the superseded lane
/// clip absent from the mix.
#[test]
fn md_b_comp_export_pcm_exact() {
    let root = scratch("mdb");
    let (c, _source_id) = comp_fixture(&root);
    let snapshot = freeze(&c, &root);
    let dest = root.join("comp-out.wav");
    let frames = 51_200_u64; // covers the whole target range
    let req = request(
        dest.clone(),
        engine(48_000, 1, 256),
        0,
        frames,
        Some(TailPolicy::None),
        0,
        SampleFormat::F32,
        Vec::new(),
        256,
    );
    let mut expected = vec![Vec::with_capacity(usize::try_from(frames).unwrap()); 1];
    for f in 0..i64::try_from(frames).unwrap() {
        expected[0].push(comp_expected(f));
    }
    let capture = Arc::new(Mutex::new(None));
    let hooks = OutputHooks {
        expect_pcm: Some(pcm_hook(Some(expected.clone()), 1e-5, Arc::clone(&capture))),
        ..OutputHooks::default()
    };
    let job = ExportJob::new();
    let receipt = completed(export(&snapshot, &req, hooks, &job));
    assert!(receipt.verified.reopened);
    assert!(receipt.verified.pcm_checked);
    assert_eq!(receipt.render.frames_emitted, frames);
    assert!(dest.exists());
    let planes = capture.lock().unwrap().clone().unwrap();
    assert_planes(&planes, &expected, 1e-5, "comp export");
}

/// MD-B: a comp node that cannot be lowered for the render engine
/// fails the export as an explicit `UnrenderableComp` — never a
/// silently dropped region.
#[test]
fn md_b_comp_unlowerable_fails_export() {
    let root = scratch("mdbfail");
    let (mut c, source_id) = comp_fixture(&root);
    // Swap the silence region for a `Loop`-mapped one: legal in the
    // comp contract but not lowerable by the 1:1 playback graph, so
    // export must refuse honestly.
    let comp = *c.live().comp.comps.keys().next().unwrap();
    let regions = c.live().comp.comps[&comp].region_ids.clone();
    gapply(
        &mut c,
        CompOp::RemoveCompRegion {
            region_id: regions[2],
        },
    );
    gapply(
        &mut c,
        CompOp::AddCompRegion {
            comp_id: comp,
            project_range: mr(1_920, 2_000),
            candidate: synvelis_project::CompCandidateRef::ExistingSource {
                source_id,
                handle: SourceRange::try_new(SourceFrame::ZERO, SourceFrame::new(60_000).unwrap())
                    .unwrap(),
                play_from: SourceFrame::ZERO,
            },
            mapping: PlaybackMapping::Loop,
        },
    );
    let snapshot = freeze(&c, &root);
    let dest = root.join("comp-fail.wav");
    let req = request(
        dest,
        engine(48_000, 1, 256),
        0,
        51_200,
        Some(TailPolicy::None),
        0,
        SampleFormat::F32,
        Vec::new(),
        256,
    );
    let job = ExportJob::new();
    let (stage, error) = failed(export(&snapshot, &req, OutputHooks::default(), &job));
    assert_eq!(stage, ExportStage::Preflight);
    assert!(
        matches!(
            error,
            ExportError::Preflight(PreflightError::UnrenderableComp { .. })
        ),
        "expected UnrenderableComp, got {error:?}"
    );
}
