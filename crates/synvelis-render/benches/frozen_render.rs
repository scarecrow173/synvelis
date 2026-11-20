//! sv602 E-layer frozen two-arm benchmark harness (Issue #602).
//!
//! `harness = false` custom main. Two arms, one process, one session:
//!
//! - **Arm K** (`mix/<clips>x<block>`) — A/B interleaved timing of the
//!   FROZEN naive mixer below (side A) against the production
//!   `fixture WAV → import_local → resolver::prepare → lowering::lower
//!   → HeadlessDriver::activate → run(blocks, PumpPolicy::Every(1))`
//!   path (side B). The gate metric is `median(A)/median(B)` per-call
//!   wall — machine-independent because both sides share the session.
//! - **Arm P** (`export/<clips>x<range>`) — `RenderSnapshot → export()`
//!   pipeline E2E, interleaved in the same schedule. e2e wall is
//!   normalized by this session's first Arm-K production median; the
//!   published WAV bytes' sha256 fingerprint is the fail-closed gate.
//!
//! FROZEN: the naive mixer, the cell matrix and the measurement
//! procedure are part of the pinned contract — changing them
//! invalidates `benches/frozen/baseline.sv602.json` and requires a
//! `--write-baseline` candidate → manual promote re-pin
//! (`docs/test-specs/issue-602-frozen-render-bench.md`).

use std::alloc::System;
use std::env;
use std::fs;
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use core::hint::black_box;
use core::sync::atomic::{AtomicU64, Ordering};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_io_file::StagedFile;
use ruviform_media_io::MediaSink;
use ruviform_media_reader::PrefetchConfig;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};

use synvelis_player::driver::HeadlessDriver;
use synvelis_player::fixture::{fixture_sample_f32, write_fixture};
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::lowering::{EngineConfig, FadeWindow, Lowered, lower};
use synvelis_player::resolver::prepare;
use synvelis_player::{PlayReport, PumpPolicy};
use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, Command, CounterIdGenerator, MediaDuration,
    MediaTimeValue, PlaybackMapping, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, SourceFrame, SourceId, SourceRange, TimeAnchor,
};
use synvelis_render::spec::WrapSink;
use synvelis_render::{
    ExportJob, ExportOutcome, ExportRequest, Insert, OfflineRenderSupport, OutputHooks,
    RenderSnapshot, TailPolicy, export,
};

#[global_allocator]
static ALLOC: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

// ------------------------------------------------------------------
// Frozen constants — part of the pin.
// ------------------------------------------------------------------

/// naive→prod rep count per Arm-K cell (`N >= 5` contract).
const REPS_K: usize = 7;
/// Arm-P rep count, interleaved inside the same schedule.
const REPS_P: usize = 5;
/// Per-rep wall target for `inner` calibration (~0.25 s).
const TARGET_REP_NS: u128 = 250_000_000;
/// Upper bound for the calibrated per-rep call count.
const MAX_INNER: usize = 64;
/// Fixture engine geometry: stereo / 48 kHz.
const RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
/// Prefetch ring depth for the production playback path (L1 test
/// convention).
const PREFETCH_BLOCKS: usize = 8;
/// Each clip's source read is offset by `i * CLIP_STRIDE` frames so
/// overlapping clips mix distinct content from one shared source.
const CLIP_STRIDE: u64 = 64;
/// `export()` physical-call bound (l2 test convention).
const EXPORT_MAX_BLOCK: u32 = 256;

struct KCell {
    name: &'static str,
    clips: usize,
    block: usize,
    /// Engine frames each clip's window covers (a multiple of `block`
    /// so `run(blocks)` covers it exactly).
    window: u64,
}

const K_CELLS: &[KCell] = &[
    KCell {
        name: "mix/4x256",
        clips: 4,
        block: 256,
        window: 24_576,
    },
    KCell {
        name: "mix/8x512",
        clips: 8,
        block: 512,
        window: 49_152,
    },
    KCell {
        name: "mix/16x256",
        clips: 16,
        block: 256,
        window: 24_576,
    },
];

struct PCell {
    name: &'static str,
    clips: usize,
    range_end: u64,
}

const P_CELLS: &[PCell] = &[
    PCell {
        name: "export/4x48000",
        clips: 4,
        range_end: 48_000,
    },
    PCell {
        name: "export/8x96000",
        clips: 8,
        range_end: 96_000,
    },
];

/// Pin path for the promoted baseline (repo-relative).
fn baseline_pin_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("frozen")
        .join("baseline.sv602.json")
}

/// `evidence/sv602/decision.tsv` — hillclimb-schema trial log.
fn decision_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("evidence")
        .join("sv602")
        .join("decision.tsv")
}

// ------------------------------------------------------------------
// FROZEN naive mixer (Arm K side A) — pin contract, do not modify.
//
// Semantics: per lowered playback binding, the clip's engine window
// `[engine_start, engine_start + engine_len)` maps 1:1 onto source
// frames `[source_start, source_start + engine_len)`. Output frame =
// `Σ_clips fixture_sample(src_pos) * fade(pos)`, then `* master_gain`.
// Channel selection mirrors the production lowering's `ChannelMap`
// clamp (`src_ch = ch.min(src_channels - 1)`). Fades come from the
// binding's `FadeWindow` envelope (`None` = unity).
// ------------------------------------------------------------------

/// One clip's mix parameters, extracted from a `PlaybackBinding`.
struct NaiveClip {
    engine_start: u64,
    engine_len: u64,
    source_start: u64,
    src_channels: usize,
    fade: Option<FadeWindow>,
}

/// The frozen reference: clip-window sum × fade → master gain.
/// Returns planar `f32` PCM, `channels × frames`.
fn naive_mix(
    clips: &[NaiveClip],
    channels: usize,
    frames: usize,
    master_gain: f32,
) -> Vec<Vec<f32>> {
    let mut out = vec![vec![0.0_f32; frames]; channels];
    for clip in clips {
        let start = usize::try_from(clip.engine_start).unwrap_or(usize::MAX);
        let end = usize::try_from(clip.engine_start.saturating_add(clip.engine_len))
            .unwrap_or(usize::MAX)
            .min(frames);
        if start >= end {
            continue;
        }
        for (ch, channel) in out.iter_mut().enumerate().take(channels) {
            let src_ch = ch.min(clip.src_channels.saturating_sub(1));
            for (i, sample) in channel[start..end].iter_mut().enumerate() {
                let pos = start + i;
                let src = usize::try_from(clip.source_start)
                    .unwrap_or(0)
                    .saturating_add(pos - start);
                let fade = clip
                    .fade
                    .map_or(1.0, |f| f.gain_at(i64::try_from(pos).unwrap_or(i64::MAX)));
                *sample += fixture_sample_f32(src_ch, src) * fade;
            }
        }
    }
    if master_gain.to_bits() != 1.0_f32.to_bits() {
        for channel in &mut out {
            for sample in channel.iter_mut() {
                *sample *= master_gain;
            }
        }
    }
    out
}

// ------------------------------------------------------------------
// CLI
// ------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
enum Mode {
    Run,
    Test,
    Selftest,
    WriteBaseline(PathBuf),
    Help,
}

struct Cli {
    mode: Mode,
    baseline: Option<PathBuf>,
    filters: Vec<String>,
}

const USAGE: &str = "frozen_render — sv602 two-arm frozen bench harness

USAGE:
    cargo bench -p synvelis-render --bench frozen_render -- [CELL FILTER...] [MODE]

CELL FILTERS (positional, substring match on the cell name):
    mix/4x256  mix/8x512  mix/16x256  export/4x48000  export/8x96000
    No filter runs every cell. A filter matching 0 cells is gate not_run
    (exit 0), never an error.

MODES (mutually exclusive):
    (none)                    real run — JSONL run/cell/gate records on
                              stdout, gate vs the pinned baseline, a
                              decision.tsv row appended under
                              evidence/sv602/. exit 0 pass / 1 fail.
    --test                    1-rep smoke: gate not_run (report_only),
                              exit 0; perf thresholds are not evaluated.
                              Only a correctness defect (output/fingerprint
                              mismatch or a failed call) still exits 1.
    --selftest                regression injection — Arm-K B side gains a
                              ~4x estimated-call spin inside the timed
                              region, Arm-P sink flips one WAV data byte —
                              proves the gate fails closed (nonzero exit
                              expected).
    --write-baseline <path>   run the measurement and emit a baseline
                              CANDIDATE (approval:null) at <path>. Never
                              promotes; promote is manual.
    --baseline <path>         use <path> instead of the pinned baseline
                              as the gate input.
    --help                    this text (exit 0)

EXIT: 0 pass / 1 gate-or-correctness fail / 2 CLI misuse.";

fn parse_cli(args: &[String]) -> Result<Cli, i32> {
    let mut mode: Option<Mode> = None;
    let mut baseline: Option<PathBuf> = None;
    let mut filters = Vec::new();
    let mut i = 0;
    macro_rules! set_mode {
        ($m:expr) => {{
            if mode.is_some() {
                eprintln!("frozen_render: modes are mutually exclusive\n{USAGE}");
                return Err(2);
            }
            mode = Some($m);
        }};
    }
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--test" => set_mode!(Mode::Test),
            "--selftest" => set_mode!(Mode::Selftest),
            "--help" | "-h" => set_mode!(Mode::Help),
            // `cargo bench` appends `--bench` to every bench binary;
            // `cargo test --benches` passes `--test`/`--nocapture`.
            "--bench" | "--nocapture" | "--exact" | "--quiet" | "-q" | "--ignored"
            | "--include-ignored" => {}
            "--write-baseline" => {
                i += 1;
                let Some(path) = args.get(i) else {
                    eprintln!("frozen_render: --write-baseline needs a path\n{USAGE}");
                    return Err(2);
                };
                set_mode!(Mode::WriteBaseline(PathBuf::from(path)));
            }
            "--baseline" => {
                i += 1;
                let Some(path) = args.get(i) else {
                    eprintln!("frozen_render: --baseline needs a path\n{USAGE}");
                    return Err(2);
                };
                baseline = Some(PathBuf::from(path));
            }
            _ if arg.starts_with("--") => {
                eprintln!("frozen_render: unknown flag {arg}\n{USAGE}");
                return Err(2);
            }
            _ => filters.push(arg.to_string()),
        }
        i += 1;
    }
    Ok(Cli {
        mode: mode.unwrap_or(Mode::Run),
        baseline,
        filters,
    })
}

// ------------------------------------------------------------------
// Environment metadata (informational only — never a gate value).
// ------------------------------------------------------------------

fn cmd_stdout(cmd: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(cmd).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn ruviform_sha() -> String {
    let lock = repo_root().join("Cargo.lock");
    let Ok(text) = fs::read_to_string(&lock) else {
        return "unknown".to_string();
    };
    for line in text.lines() {
        if let Some(pos) = line.find("ruviform.git#") {
            return line[pos + "ruviform.git#".len()..]
                .trim_matches('"')
                .to_string();
        }
    }
    "unknown".to_string()
}

fn env_json() -> Value {
    let synvelis_sha =
        cmd_stdout("git", &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let rustc = cmd_stdout("rustc", &["--version"]).unwrap_or_else(|| "unknown".into());
    let cpu = env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "unknown".into());
    let cores = std::thread::available_parallelism().map_or(0, std::num::NonZeroUsize::get);
    json!({
        "synvelis_sha": synvelis_sha,
        "ruviform_sha": ruviform_sha(),
        "rustc": rustc,
        "os": env::consts::OS,
        "arch": env::consts::ARCH,
        "cpu": cpu,
        "cores": cores,
    })
}

// ------------------------------------------------------------------
// Deterministic project construction (shared fixture machinery).
// ------------------------------------------------------------------

static SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "synvelis-sv602-bench-{name}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

const fn engine(block: usize) -> EngineConfig {
    EngineConfig {
        sample_rate: RATE,
        channels: CHANNELS,
        block_frames: block,
    }
}

fn engine_format() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(RATE).expect("non-zero rate"),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn apply(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .expect("project op")
}

fn media_placement(end_frame: u64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Absolute(MediaTimeValue::new(0, 1, RATE).expect("time value")),
        extent: ClipExtent::Absolute(
            MediaDuration::new(i64::try_from(end_frame).expect("extent"), 1, RATE)
                .expect("duration"),
        ),
    }
}

fn natural_playback(offset: u64, frames: u64) -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::new(i64::try_from(offset).expect("offset")).expect("source frame"),
            SourceFrame::new(i64::try_from(offset + frames).expect("end")).expect("source frame"),
        )
        .expect("source range"),
        mapping: PlaybackMapping::Natural,
    }
}

/// Builds one deterministic project: one stereo fixture source +
/// `clips` tracks, each with one lane holding a clip over `[0, window)`,
/// reading the source at `index * CLIP_STRIDE`. `semantic` also adds
/// the `AddTrackStrip` wiring the offline export lowering requires.
fn build_project(
    root: &Path,
    clips: usize,
    window: u64,
    semantic: bool,
) -> (ProjectController, SourceId) {
    let frames = usize::try_from(window + CLIP_STRIDE * clips as u64).expect("frames");
    let wav = root.join("fixture.wav");
    write_fixture(&wav, CHANNELS, RATE, frames).expect("fixture wav");
    let mut c = ProjectController::new(
        Project::new(ProjectId::new(602), "sv602-bench"),
        Box::new(CounterIdGenerator::new(1)),
    );
    let out = import_local(
        &mut c,
        &ImportRequest {
            path: wav,
            staging_root: root.join("staging"),
            media_root: root.join("media"),
            resume: None,
        },
    )
    .expect("import_local");
    let source_id = out.commit.source_id;
    for index in 0..clips {
        apply(
            &mut c,
            ProjectOp::AddTrack {
                name: format!("t{index}"),
            },
        );
        let track = *c.live().tracks.keys().next_back().expect("track");
        if semantic {
            apply(&mut c, ProjectOp::AddTrackStrip { track_id: track });
        }
        apply(&mut c, ProjectOp::AddLane { track_id: track });
        let lane = *c.live().lanes.keys().next_back().expect("lane");
        apply(
            &mut c,
            ProjectOp::AddClip {
                lane_id: lane,
                source_id,
                placement: media_placement(window),
                playback: natural_playback(index as u64 * CLIP_STRIDE, window),
            },
        );
    }
    (c, source_id)
}

// ------------------------------------------------------------------
// Arm K — production path and its measured call.
// ------------------------------------------------------------------

struct KSetup {
    root: PathBuf,
    controller: ProjectController,
    lowered: Lowered,
    engine_cfg: EngineConfig,
    prefetch: PrefetchConfig,
    blocks: usize,
    frames: usize,
    naive_clips: Vec<NaiveClip>,
    mix_samples: u64,
}

fn k_setup(cell: &KCell) -> KSetup {
    let root = scratch(cell.name.replace('/', "-").as_str());
    let (controller, _source) = build_project(&root, cell.clips, cell.window, false);
    let engine_cfg = engine(cell.block);
    let lowered = lower(controller.live(), &|_| Some(engine_format()), &engine_cfg);
    assert!(
        !lowered.playbacks.is_empty(),
        "cell {0} lowered no playbacks",
        cell.name
    );
    let prefetch = PrefetchConfig {
        block_frames: cell.block,
        block_count: PREFETCH_BLOCKS,
        channels: usize::from(CHANNELS),
        sample_rate: RATE,
    };
    let max_end = lowered
        .playbacks
        .iter()
        .map(|b| b.window.engine_start + b.window.engine_len)
        .max()
        .expect("windows");
    let blocks = usize::try_from(max_end).expect("end") / cell.block;
    let naive_clips = lowered
        .playbacks
        .iter()
        .map(|b| NaiveClip {
            engine_start: b.window.engine_start,
            engine_len: b.window.engine_len,
            source_start: b.window.source_start,
            src_channels: usize::try_from(b.canonical.layout.channels()).unwrap_or(usize::MAX),
            fade: b.fade,
        })
        .collect::<Vec<_>>();
    let mix_samples = lowered
        .playbacks
        .iter()
        .map(|b| b.window.engine_len)
        .sum::<u64>()
        * u64::from(CHANNELS);
    KSetup {
        root,
        controller,
        lowered,
        engine_cfg,
        prefetch,
        blocks,
        frames: blocks * cell.block,
        naive_clips,
        mix_samples,
    }
}

/// One production-path call: a fresh driver resolves, lowers-in and
/// plays the whole window — `new → prepare×N → activate → run`.
/// `slow_ns` is the selftest's injected extra work inside the timed
/// region.
fn prod_mix_call(setup: &KSetup, slow_ns: u128) -> Result<PlayReport, String> {
    let mut driver = HeadlessDriver::new(setup.engine_cfg);
    let prepared = setup
        .lowered
        .playbacks
        .iter()
        .map(|b| {
            prepare(
                setup.controller.live(),
                b.source_id,
                &setup.root.join("media"),
                &setup.prefetch,
            )
            .map_err(|e| format!("prepare: {e:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    driver
        .activate(&setup.lowered, prepared)
        .map_err(|e| format!("activate: {e:?}"))?;
    let report = driver
        .run(setup.blocks, PumpPolicy::Every(1))
        .map_err(|e| format!("run: {e:?}"))?;
    if slow_ns > 0 {
        spin(slow_ns);
    }
    Ok(report)
}

/// Deterministic busy wait — the selftest's injected slowdown.
fn spin(ns: u128) {
    let start = Instant::now();
    let mut acc = 0_u64;
    while start.elapsed().as_nanos() < ns {
        acc = acc.wrapping_mul(31).wrapping_add(1);
    }
    black_box(acc);
}

// ------------------------------------------------------------------
// Arm P — export() E2E call.
// ------------------------------------------------------------------

struct PSetup {
    root: PathBuf,
    snapshot: RenderSnapshot,
    engine_cfg: EngineConfig,
    range_end: u64,
}

fn p_setup(cell: &PCell) -> PSetup {
    let root = scratch(cell.name.replace('/', "-").as_str());
    let (controller, _source) = build_project(&root, cell.clips, cell.range_end, true);
    let snapshot = RenderSnapshot::freeze(&controller, root.join("media"));
    PSetup {
        root,
        snapshot,
        engine_cfg: engine(usize::try_from(EXPORT_MAX_BLOCK).unwrap_or(256)),
        range_end: cell.range_end,
    }
}

fn export_request(setup: &PSetup) -> ExportRequest {
    ExportRequest {
        engine: setup.engine_cfg,
        range_start: 0,
        range_end: setup.range_end,
        tail: Some(TailPolicy::Known(64)),
        preroll: 0,
        destination: setup.root.join("dest.wav"),
        replace_existing: true,
        wire_format: SampleFormat::S16,
        inserts: vec![
            Insert::Gain { gain_db: -6.0 },
            Insert::OnePole { a: 0.5, latency: 0 },
            Insert::Delay { frames: 64 },
        ],
        max_block: EXPORT_MAX_BLOCK,
        clip_support: OfflineRenderSupport::Verified,
        plugin_state: None,
    }
}

/// Selftest sink wrapper — a plain `fn` so it coerces to the
/// higher-ranked `WrapSink` trait object.
fn corrupt_wrap(staged: &mut StagedFile) -> Box<dyn MediaSink + '_> {
    Box::new(CorruptSink::new(staged))
}

/// One Arm-P call: `export()` with the deterministic insert chain —
/// returns the published WAV sha256 hex, byte length and emitted
/// frame count.
fn prod_export_call(setup: &PSetup, corrupt: bool) -> Result<(String, u64, u64), String> {
    let request = export_request(setup);
    let hooks = OutputHooks {
        wrap_sink: corrupt.then(|| Box::new(corrupt_wrap) as WrapSink),
        ..OutputHooks::default()
    };
    let job = ExportJob::new();
    match export(&setup.snapshot, &request, hooks, &job) {
        ExportOutcome::Completed { receipt } => {
            let bytes = fs::read(&request.destination).map_err(|e| format!("readback: {e}"))?;
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            Ok((
                format!("{:x}", hasher.finalize()),
                u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                receipt.render.frames_emitted,
            ))
        }
        other => Err(format!("export outcome: {other:?}")),
    }
}

/// Selftest sink: flips one byte at offset 100 (inside the WAV data
/// chunk — the 44-byte canonical header stays intact, so the file
/// still verifies and publishes; only the content bytes differ).
struct CorruptSink<'a> {
    inner: &'a mut StagedFile,
    pos: u64,
    flipped: bool,
}

impl<'a> CorruptSink<'a> {
    fn new(inner: &'a mut StagedFile) -> Self {
        Self {
            inner,
            pos: 0,
            flipped: false,
        }
    }
}

impl Write for CorruptSink<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        const TARGET: u64 = 100;
        if self.flipped || self.pos.saturating_add(buf.len() as u64) <= TARGET {
            self.pos = self.pos.saturating_add(buf.len() as u64);
            return self.inner.write(buf);
        }
        let mut owned = buf.to_vec();
        let index = usize::try_from(TARGET - self.pos).unwrap_or(0);
        owned[index] ^= 0x5A;
        self.flipped = true;
        self.pos = self.pos.saturating_add(buf.len() as u64);
        self.inner.write(&owned)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl Seek for CorruptSink<'_> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match self.inner.seek(pos) {
            Ok(now) => {
                self.pos = now;
                Ok(now)
            }
            Err(e) => Err(e),
        }
    }
}

// ------------------------------------------------------------------
// Measurement plumbing.
// ------------------------------------------------------------------

#[derive(Default)]
struct Side {
    ns: Vec<u64>,
    allocs: Vec<u64>,
    deallocs: Vec<u64>,
}

fn elapsed_ns(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn median(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted[sorted.len() / 2]
}

fn min_max(values: &[u64]) -> (u64, u64) {
    (
        values.iter().min().copied().unwrap_or(0),
        values.iter().max().copied().unwrap_or(0),
    )
}

/// Times `inner` calls of `f` once; returns (elapsed ns, allocs,
/// deallocs) measured under the instrumented allocator.
fn time_calls<A>(inner: usize, alloc_delta: &mut (u64, u64), mut f: impl FnMut() -> A) -> u64 {
    let region = Region::new(ALLOC);
    let start = Instant::now();
    let mut last = None;
    for _ in 0..inner {
        last = Some(f());
    }
    let ns = elapsed_ns(start);
    let stats = region.change();
    black_box(last);
    *alloc_delta = (stats.allocations as u64, stats.deallocations as u64);
    ns
}

/// Calibrate `inner` so one rep lands near `TARGET_REP_NS`; returns
/// (inner, estimated single-call ns).
fn calibrate<A>(mut f: impl FnMut() -> A) -> (usize, u64) {
    let start = Instant::now();
    black_box(f());
    let est = elapsed_ns(start).max(1);
    let inner = usize::try_from(TARGET_REP_NS / u128::from(est))
        .unwrap_or(MAX_INNER)
        .clamp(1, MAX_INNER);
    (inner, est)
}

struct KOut {
    cell: &'static str,
    naive: Side,
    prod: Side,
    inner_naive: usize,
    inner_prod: usize,
    /// Selftest slowdown injected inside each timed prod call.
    spin_ns: u128,
    errors: u64,
    call_failures: u64,
    frames_per_call: usize,
    mix_samples_per_call: u64,
}

struct POut {
    cell: &'static str,
    wall: Vec<u64>,
    allocs: Vec<u64>,
    deallocs: Vec<u64>,
    sha: String,
    sha_mismatch: u64,
    errors: u64,
    wav_bytes: u64,
    export_frames: u64,
    normalized: Option<f64>,
}

// ------------------------------------------------------------------
// Baseline + gate.
// ------------------------------------------------------------------

struct Baseline {
    cells: Value,
    env: Value,
}

/// Reads a baseline file; `Err(())` = unreadable for gating
/// (missing, malformed, wrong schema, or `approval: null` — an
/// unpromoted candidate can never gate).
fn read_baseline(path: &Path) -> Result<Baseline, ()> {
    let text = fs::read_to_string(path).map_err(|_| ())?;
    let json: Value = serde_json::from_str(&text).map_err(|_| ())?;
    if !matches!(
        json.get("schema").and_then(Value::as_str),
        Some("sv.bench.baseline/v1")
    ) {
        return Err(());
    }
    match json.get("approval") {
        Some(Value::Object(_)) => {}
        _ => return Err(()),
    }
    let cells = json.get("cells").cloned().ok_or(())?;
    let env = json.get("env").cloned().unwrap_or(Value::Null);
    Ok(Baseline { cells, env })
}

/// Appends one hillclimb-schema row to `evidence/sv602/decision.tsv`.
fn decision_append(
    hypothesis: &str,
    change: &str,
    after: &str,
    delta: &str,
    tests: &str,
    note: &str,
) {
    let path = decision_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let exists = path.exists();
    let seq = fs::read_to_string(&path).map_or(0, |t| {
        t.lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with("id\t"))
            .count()
    }) + 1;
    let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    if !exists {
        let _ = writeln!(
            file,
            "id\thypothesis\tchange\tbefore\tafter\tdelta\ttests\tverdict\tnote"
        );
    }
    let _ = writeln!(
        file,
        "sv602-r{seq}\t{hypothesis}\t{change}\t-\t{after}\t{delta}\t{tests}\tkept\t{note}"
    );
}

// ------------------------------------------------------------------
// Main measurement + gate.
// ------------------------------------------------------------------

#[expect(
    clippy::too_many_lines,
    reason = "single measurement+gate pass over a fixed schedule"
)]
#[expect(
    clippy::cast_precision_loss,
    reason = "ns-scale u64/usize values (<2^53) → f64 for ratio/normalized reporting; precision loss is irrelevant at bench scale"
)]
fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let cli = match parse_cli(&args) {
        Ok(c) => c,
        Err(code) => std::process::exit(code),
    };
    if cli.mode == Mode::Help {
        println!("{USAGE}");
        return;
    }

    let selftest = cli.mode == Mode::Selftest;
    let smoke = cli.mode == Mode::Test;
    let reps_k = if smoke { 1 } else { REPS_K };
    let reps_p = if smoke { 1 } else { REPS_P };
    let select =
        |name: &str| cli.filters.is_empty() || cli.filters.iter().any(|f| name.contains(f));
    let k_cells: Vec<&KCell> = K_CELLS.iter().filter(|c| select(c.name)).collect();
    let p_cells: Vec<&PCell> = P_CELLS.iter().filter(|c| select(c.name)).collect();
    let injected = selftest.then_some(
        "arm-k prod side +4x estimated-call spin; arm-p corrupt sink flips one WAV data byte @100",
    );

    let mut records: Vec<Value> = Vec::new();
    records.push(json!({
        "type": "run",
        "schema": "syn.bench.run/v1",
        "issue": "synvelis#602",
        "mode": match cli.mode {
            Mode::Run => "run",
            Mode::Test => "test",
            Mode::Selftest => "selftest",
            Mode::WriteBaseline(_) => "write-baseline",
            Mode::Help => "help",
        },
        "reps_k": reps_k,
        "reps_p": reps_p,
        "baseline": cli.baseline.clone().unwrap_or_else(baseline_pin_path),
        "env": env_json(),
        "injected": injected,
    }));

    // ---- per-cell setups (untimed, shared by both arms' calls) ----
    let k_setups: Vec<KSetup> = k_cells.iter().map(|c| k_setup(c)).collect();
    let p_setups: Vec<PSetup> = p_cells.iter().map(|c| p_setup(c)).collect();

    let mut k_outs: Vec<KOut> = k_cells
        .iter()
        .zip(&k_setups)
        .map(|(cell, setup)| {
            let (inner_naive, _naive_est) =
                calibrate(|| naive_mix(&setup.naive_clips, 2, setup.frames, 1.0));
            let (inner_prod, prod_est) = calibrate(|| prod_mix_call(setup, 0).ok());
            KOut {
                cell: cell.name,
                naive: Side::default(),
                prod: Side::default(),
                inner_naive,
                inner_prod,
                spin_ns: if selftest {
                    u128::from(prod_est) * 4
                } else {
                    0
                },
                errors: 0,
                call_failures: 0,
                frames_per_call: setup.frames,
                mix_samples_per_call: setup.mix_samples,
            }
        })
        .collect();
    let mut p_outs: Vec<POut> = p_cells
        .iter()
        .map(|cell| POut {
            cell: cell.name,
            wall: Vec::new(),
            allocs: Vec::new(),
            deallocs: Vec::new(),
            sha: String::new(),
            sha_mismatch: 0,
            errors: 0,
            wav_bytes: 0,
            export_frames: 0,
            normalized: None,
        })
        .collect();

    // ---- interleaved schedule: per rep, K cells naive then prod,
    // P cells while rep < reps_p — same session, shared normalizer.
    for rep in 0..reps_k.max(reps_p) {
        if rep < reps_k {
            for (setup, out) in k_setups.iter().zip(k_outs.iter_mut()) {
                let mut last_naive: Option<Vec<Vec<f32>>> = None;
                let mut delta = (0, 0);
                let inner = out.inner_naive;
                out.naive.ns.push(time_calls(inner, &mut delta, || {
                    last_naive = Some(naive_mix(&setup.naive_clips, 2, setup.frames, 1.0));
                }));
                out.naive.allocs.push(delta.0);
                out.naive.deallocs.push(delta.1);

                let mut last_prod: Option<PlayReport> = None;
                let mut failed = false;
                let inner = out.inner_prod;
                let spin_ns = out.spin_ns;
                out.prod.ns.push(time_calls(inner, &mut delta, || {
                    match prod_mix_call(setup, spin_ns) {
                        Ok(report) => last_prod = Some(report),
                        Err(_) => failed = true,
                    }
                }));
                out.prod.allocs.push(delta.0);
                out.prod.deallocs.push(delta.1);
                if failed {
                    out.call_failures += 1;
                }
                if let (Some(a), Some(b)) = (&last_naive, &last_prod) {
                    for (naive_ch, prod_ch) in a.iter().zip(b.pcm.iter()) {
                        let len = naive_ch.len().max(prod_ch.len());
                        for i in 0..len {
                            let av = naive_ch.get(i).copied().unwrap_or(f32::NAN);
                            let bv = prod_ch.get(i).copied().unwrap_or(f32::NAN);
                            if av.to_bits() != bv.to_bits() {
                                out.errors += 1;
                            }
                        }
                    }
                }
            }
        }
        if rep < reps_p {
            for (setup, out) in p_setups.iter().zip(p_outs.iter_mut()) {
                let mut delta = (0, 0);
                let mut result: Option<Result<(String, u64, u64), String>> = None;
                out.wall.push(time_calls(1, &mut delta, || {
                    result = Some(prod_export_call(setup, selftest));
                }));
                out.allocs.push(delta.0);
                out.deallocs.push(delta.1);
                match result {
                    Some(Ok((sha, bytes, frames))) => {
                        if out.sha.is_empty() {
                            out.sha = sha;
                        } else if out.sha != sha {
                            out.sha_mismatch += 1;
                        }
                        out.wav_bytes = bytes;
                        out.export_frames = frames;
                    }
                    _ => out.errors += 1,
                }
            }
        }
    }

    // ---- per-cell records ----
    let mut k_ratios: Vec<f64> = Vec::new();
    let mut prod_medians: Vec<Option<f64>> = Vec::new();
    for out in &k_outs {
        let med_naive = median(&out.naive.ns);
        let med_prod = median(&out.prod.ns);
        let naive_call = med_naive as f64 / out.inner_naive.max(1) as f64;
        let prod_call = med_prod as f64 / out.inner_prod.max(1) as f64;
        let ratio = if prod_call > 0.0 {
            naive_call / prod_call
        } else {
            0.0
        };
        k_ratios.push(ratio);
        prod_medians.push((prod_call > 0.0).then_some(prod_call));
        let (naive_min, naive_max) = min_max(&out.naive.ns);
        let (prod_min, prod_max) = min_max(&out.prod.ns);
        records.push(json!({
            "type": "cell",
            "cell": out.cell,
            "arm": "k",
            "reps": out.naive.ns.len(),
            "inner_a": out.inner_naive,
            "inner_b": out.inner_prod,
            "a": {"median_ns": med_naive, "min_ns": naive_min, "max_ns": naive_max, "alloc_median": median(&out.naive.allocs), "dealloc_median": median(&out.naive.deallocs)},
            "b": {"median_ns": med_prod, "min_ns": prod_min, "max_ns": prod_max, "alloc_median": median(&out.prod.allocs), "dealloc_median": median(&out.prod.deallocs)},
            "per_call_ns": {"a": naive_call, "b": prod_call},
            "ratio": ratio,
            "errors": out.errors,
            "call_failures": out.call_failures,
            "work": {"frames_per_call": out.frames_per_call, "mix_samples_per_call": out.mix_samples_per_call},
            "limiter": "B: decode/prefetch + compute/mix (graph process + pump); A: compute/mix only",
        }));
    }
    // Normalizer = first measured Arm-K production per-call median.
    let normalizer = prod_medians.iter().flatten().next().copied();
    let normalizer_cell = k_outs
        .iter()
        .zip(&prod_medians)
        .find_map(|(o, m)| m.map(|_| o.cell));
    for out in &mut p_outs {
        let w_med = median(&out.wall);
        out.normalized = normalizer.map(|n| w_med as f64 / n);
        let (w_min, w_max) = min_max(&out.wall);
        records.push(json!({
            "type": "cell",
            "cell": out.cell,
            "arm": "p",
            "reps": out.wall.len(),
            "wall": {"median_ns": w_med, "min_ns": w_min, "max_ns": w_max, "alloc_median": median(&out.allocs), "dealloc_median": median(&out.deallocs)},
            "normalized": out.normalized,
            "normalizer": normalizer_cell,
            "normalizer_median_ns": normalizer,
            "pcm_sha256": out.sha,
            "sha_mismatch": out.sha_mismatch,
            "errors": out.errors,
            "work": {"export_frames": out.export_frames, "wav_bytes": out.wav_bytes},
            "limiter": "decode + compute/mix + encode + fs-publish (incl. reopen verify)",
        }));
    }

    // ---- gate ----
    let gate_input = cli.baseline.clone().unwrap_or_else(baseline_pin_path);
    let baseline = if matches!(cli.mode, Mode::Run | Mode::Selftest) {
        Some(read_baseline(&gate_input))
    } else {
        None
    };
    // In run/selftest an unreadable baseline fails the whole gate
    // closed; per-cell verdicts degrade to not_run.
    let baseline_bad = matches!(baseline, Some(Err(())));

    let mut gate_rows: Vec<Value> = Vec::new();
    let mut correctness_fail = false;
    let mut gate_fail = baseline_bad;
    for (out, ratio) in k_outs.iter().zip(&k_ratios) {
        let mut reasons: Vec<String> = Vec::new();
        if out.call_failures > 0 {
            reasons.push(format!("call_failed x{}", out.call_failures));
        }
        if out.errors > 0 {
            reasons.push(format!("output_mismatch samples={}", out.errors));
        }
        if !reasons.is_empty() {
            correctness_fail = true;
        }
        let mut verdict = if reasons.is_empty() { "pass" } else { "fail" };
        if let Some(Ok(b)) = &baseline {
            if let Some(spec) = b.cells.get(out.cell) {
                let min_ratio = spec.get("min_ratio").and_then(Value::as_f64).unwrap_or(0.0);
                if *ratio < min_ratio {
                    reasons.push(format!("ratio_below_threshold {ratio:.6} < {min_ratio:.6}"));
                    verdict = "fail";
                }
            } else {
                verdict = "not_run";
                reasons.push("baseline_incomplete: no cell entry".to_string());
                gate_fail = true;
            }
        } else {
            if verdict == "pass" {
                verdict = "not_run";
            }
            if baseline_bad {
                reasons.push("baseline_unreadable".to_string());
            }
        }
        gate_fail |= verdict == "fail";
        gate_rows.push(
            json!({"cell": out.cell, "arm": "k", "verdict": verdict, "reasons": reasons, "ratio": ratio}),
        );
    }
    for out in &p_outs {
        let mut reasons: Vec<String> = Vec::new();
        if out.errors > 0 {
            reasons.push(format!("export_failed x{}", out.errors));
        }
        if out.sha_mismatch > 0 {
            reasons.push(format!("fingerprint_unstable x{}", out.sha_mismatch));
        }
        if !reasons.is_empty() {
            correctness_fail = true;
        }
        let mut verdict = if reasons.is_empty() { "pass" } else { "fail" };
        if let Some(Ok(b)) = &baseline {
            if let Some(spec) = b.cells.get(out.cell) {
                let pinned = spec.get("pcm_sha256").and_then(Value::as_str).unwrap_or("");
                if out.sha != pinned {
                    reasons.push(format!(
                        "fingerprint_mismatch {0} != {pinned}",
                        &out.sha[..out.sha.len().min(16)]
                    ));
                    verdict = "fail";
                    correctness_fail = true;
                }
                let max_norm = spec
                    .get("max_normalized")
                    .and_then(Value::as_f64)
                    .unwrap_or(f64::INFINITY);
                match out.normalized {
                    Some(n) if n > max_norm => {
                        reasons.push(format!("normalized_above_threshold {n:.4} > {max_norm:.4}"));
                        verdict = "fail";
                    }
                    None => {
                        reasons
                            .push("normalized not_run: no arm-k normalizer in session".to_string());
                    }
                    _ => {}
                }
            } else {
                verdict = "not_run";
                reasons.push("baseline_incomplete: no cell entry".to_string());
                gate_fail = true;
            }
        } else {
            if verdict == "pass" {
                verdict = "not_run";
            }
            if baseline_bad {
                reasons.push("baseline_unreadable".to_string());
            }
        }
        gate_fail |= verdict == "fail";
        gate_rows.push(
            json!({"cell": out.cell, "arm": "p", "verdict": verdict, "reasons": reasons, "normalized": out.normalized, "pcm_sha256": out.sha}),
        );
    }

    let no_cells = k_outs.is_empty() && p_outs.is_empty();
    let gate_verdict = if no_cells || matches!(cli.mode, Mode::Test | Mode::WriteBaseline(_)) {
        "not_run"
    } else if gate_fail {
        "fail"
    } else {
        "pass"
    };

    records.push(json!({
        "type": "gate",
        "verdict": gate_verdict,
        "baseline": gate_input,
        "baseline_state": match &baseline {
            Some(Ok(b)) => json!({"state": "promoted", "env": b.env}),
            Some(Err(())) => json!({"state": "unreadable-or-unpromoted"}),
            None => json!({"state": "not-consulted"}),
        },
        "cells": gate_rows,
    }));

    for record in &records {
        println!("{}", serde_json::to_string(record).expect("json"));
    }

    // ---- side outputs: candidate / decision row / exit ----
    if let Mode::WriteBaseline(path) = &cli.mode {
        let mut cells = serde_json::Map::new();
        for (out, ratio) in k_outs.iter().zip(&k_ratios) {
            cells.insert(
                out.cell.to_string(),
                json!({
                    "arm": "k",
                    "min_ratio": ratio * 0.5,
                }),
            );
        }
        for out in &p_outs {
            cells.insert(
                out.cell.to_string(),
                json!({
                    "arm": "p",
                    "max_normalized": out.normalized.unwrap_or(0.0) * 2.0,
                    "pcm_sha256": out.sha,
                }),
            );
        }
        let candidate = json!({
            "schema": "sv.bench.baseline/v1",
            "issue": "synvelis#602",
            "approval": null,
            "env": env_json(),
            "cells": cells,
        });
        let text = serde_json::to_string_pretty(&candidate).expect("json");
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match fs::write(path, format!("{text}\n")) {
            Ok(()) => eprintln!("wrote baseline candidate -> {}", path.display()),
            Err(e) => {
                eprintln!("failed to write candidate {}: {e}", path.display());
                std::process::exit(1);
            }
        }
    }

    if !matches!(cli.mode, Mode::Test) {
        let delta = k_outs
            .iter()
            .zip(&k_ratios)
            .map(|(o, r)| format!("{cell} r={r:.4}", cell = o.cell))
            .chain(p_outs.iter().map(|o| {
                format!(
                    "{cell} n={n:.2}",
                    cell = o.cell,
                    n = o.normalized.unwrap_or(f64::NAN)
                )
            }))
            .collect::<Vec<_>>()
            .join(";");
        let verdict_count = |v: &str| {
            gate_rows
                .iter()
                .filter(|r| r.get("verdict").and_then(Value::as_str) == Some(v))
                .count()
        };
        decision_append(
            "frozen two-arm bench measurement",
            match cli.mode {
                Mode::Run => "none (run)",
                Mode::Selftest => "injected: prod spin x4 + corrupt-sink byte@100",
                Mode::WriteBaseline(_) => "baseline candidate emit (approval:null)",
                _ => "-",
            },
            &format!("gate={gate_verdict}"),
            &delta,
            &format!(
                "cells pass={p} fail={f} not_run={n}",
                p = verdict_count("pass"),
                f = verdict_count("fail"),
                n = verdict_count("not_run")
            ),
            &format!(
                "env={}/{}",
                env_json()
                    .get("synvelis_sha")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .chars()
                    .take(8)
                    .collect::<String>(),
                env_json()
                    .get("ruviform_sha")
                    .and_then(Value::as_str)
                    .unwrap_or("?")
                    .chars()
                    .take(8)
                    .collect::<String>()
            ),
        );
    }

    let exit = match cli.mode {
        Mode::Test => i32::from(correctness_fail),
        Mode::Help | Mode::WriteBaseline(_) => 0,
        Mode::Run | Mode::Selftest => i32::from(gate_verdict == "fail"),
    };
    std::process::exit(exit);
}
