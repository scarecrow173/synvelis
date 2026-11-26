//! RCD-01 consumer virtual-WASAPI EXT-L3 harness (synvelis#115).
//!
//! `Ruviform Cable Input`（render/output endpoint）へ deterministic
//! impulse train を production output callback から供給し、
//! `Ruviform Cable Output`（capture/input endpoint）から入る PCMを
//! production `InputSession`/`InputAdapter`/`RealDeviceRig`/実
//! `StreamWriter`/journal/staging/project commit→save→reload で検証する。
//!
//! endpoint 名の一致は候補であって証拠ではない。実際に open/start
//! して stream 経由で観測できた run だけが EXT-L3 Pass を名乗る。
//! endpoint pair 不在や host 列挙不能は fail-closed に `not-run` /
//! `NOT_PROVISIONED` を記録して終了し、候補 pair があるのに
//! open/start/callback/oracle/commit/reload が失敗した場合は Fail。
//!
//! これは virtual cable の SRC を通る case なので、render stimulusと
//! capture PCM の bit-exact 一致は要求しない。信号内容は marker
//! 間隔/個数・RMS band・非 finite sample の独立 oracle で判定し、
//! journal 内 bytes/CRC/SHA-256 は記録媒体の内部整合として独立に
//! 再計算する。

#[allow(dead_code)]
mod common;

use std::path::{Path, PathBuf};
use std::process;

use ruviform_audio_test::clock::{JitterSpec, VirtualClockConfig};
use ruviform_audio_test::metadata::{
    CaseRequirement, CaseResult, EnvironmentKind, RunMetadata, RunParameters, VerificationLevel,
};

const RUN_SEED: u64 = 0x115;
const FIXTURE_VERSION: u32 = 1;
const CASE_ID: &str = "sv115.rcd01.virtual-endpoint.render-capture-recorder";
const ORACLE_ID: &str = "marker-rms-journal-commit-reload-v1";

const fn run_parameters(sample_rate: u32) -> RunParameters {
    RunParameters {
        seed: RUN_SEED,
        fixture_version: FIXTURE_VERSION,
        clock_config: VirtualClockConfig {
            sample_rate,
            epoch: 0,
            offset_ticks: 0,
            drift_ppm: 0,
            jitter: JitterSpec::DISABLED,
        },
    }
}

fn repo_name() -> String {
    if let Ok(repo) =
        std::env::var("SYNVELIS_RUN_REPO").or_else(|_| std::env::var("GITHUB_REPOSITORY"))
    {
        return repo;
    }
    let output = process::Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .output();
    let Ok(output) = output else {
        return "unknown".to_owned();
    };
    if !output.status.success() {
        return "unknown".to_owned();
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    for prefix in ["https://github.com/", "git@github.com:"] {
        if let Some(rest) = url.strip_prefix(prefix) {
            return rest.trim_end_matches(".git").to_owned();
        }
    }
    "unknown".to_owned()
}

fn repo_sha() -> String {
    if let Ok(sha) = std::env::var("SYNVELIS_RUN_SHA")
        .or_else(|_| std::env::var("CI_COMMIT_SHA"))
        .or_else(|_| std::env::var("GITHUB_SHA"))
    {
        return sha;
    }
    let output = process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output();
    let Ok(output) = output else {
        return "unknown".to_owned();
    };
    if output.status.success() {
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    } else {
        "unknown".to_owned()
    }
}

fn github_run_evidence() -> Option<String> {
    let repo = std::env::var("GITHUB_REPOSITORY").ok()?;
    let run_id = std::env::var("GITHUB_RUN_ID").ok()?;
    let attempt = std::env::var("GITHUB_RUN_ATTEMPT").ok()?;
    if repo.is_empty()
        || !run_id.chars().all(|c| c.is_ascii_digit())
        || !attempt.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some(format!("github-run:{repo}:{run_id}:{attempt}"))
}

fn revision_evidence(env_key: &str, label: &str) -> Option<String> {
    let value = std::env::var(env_key).ok()?;
    let revision = value.trim();
    if revision.len() == 40 && revision.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(format!("{label}:{revision}"))
    } else {
        None
    }
}

fn endpoint_record_at(case_id: &str, oracle: &str, sample_rate: u32) -> RunMetadata {
    let mut meta = RunMetadata::for_environment(
        case_id,
        CaseRequirement::Required,
        VerificationLevel::ExtL3,
        EnvironmentKind::VirtualOsEndpoint,
        oracle,
        run_parameters(sample_rate),
        repo_name(),
        repo_sha(),
    );
    if let Some(run) = github_run_evidence() {
        meta = meta.with_evidence(run);
    }
    for (env_key, label) in [
        ("SYNVELIS_RUVIFORM_LOCK_REV", "ruviform-lock-rev"),
        ("SYNVELIS_RUVIFORM_CHECKOUT_REV", "ruviform-checkout-rev"),
    ] {
        if let Some(revision) = revision_evidence(env_key, label) {
            meta = meta.with_evidence(revision);
        }
    }
    meta.with_platform(std::env::consts::OS, std::env::consts::ARCH)
}

fn endpoint_record(case_id: &str, oracle: &str) -> RunMetadata {
    endpoint_record_at(case_id, oracle, 48_000)
}

/// Direction words stripped before stem matching. Endpoint directionは
/// OS enumeration side（render/output list と capture/input list）が
/// 権威であり、名前の input/output は `SysVAD` 由来の cable 配線語。
const DIRECTION_TOKENS: &[&str] = &[
    "input",
    "output",
    "render",
    "capture",
    "in",
    "out",
    "speaker",
    "speakers",
    "microphone",
    "mic",
    "headphone",
    "headphones",
];

/// B1 で対象にする Ruviform virtual endpoint だけを候補化する stem。
/// `Ruviform Cable Input`/`Ruviform Cable Output` と
/// `Ruviform Virtual Audio Device ...` の表記揺れを許容するが、
/// VB-CABLE/Voicemeeter/物理 device は候補にしない。
fn ruviform_virtual_stem(name: &str) -> Option<String> {
    let tokens: Vec<String> = name
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect();
    if !tokens.iter().any(|token| token == "ruviform") {
        return None;
    }
    let cable = tokens.iter().any(|token| token == "cable");
    let virtual_audio_device = tokens.iter().any(|token| token == "virtual")
        && tokens.iter().any(|token| token == "audio")
        && tokens.iter().any(|token| token == "device");
    if !cable && !virtual_audio_device {
        return None;
    }
    let stem = tokens
        .iter()
        .filter(|token| !DIRECTION_TOKENS.contains(&token.as_str()))
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" ");
    if stem.is_empty() { None } else { Some(stem) }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum PairSelection {
    NotProvisioned,
    Candidates(usize),
}

fn classify_pair_selection(pair_count: usize) -> PairSelection {
    if pair_count == 0 {
        PairSelection::NotProvisioned
    } else {
        PairSelection::Candidates(pair_count)
    }
}

fn metadata_file_name(case_id: &str) -> String {
    format!(
        "run-metadata-{}.json",
        case_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
    )
}

fn metadata_output_dir_from(
    run_metadata_dir: Option<&str>,
    cargo_target_tmpdir: Option<&str>,
) -> Option<PathBuf> {
    run_metadata_dir
        .filter(|dir| !dir.trim().is_empty())
        .or_else(|| cargo_target_tmpdir.filter(|dir| !dir.trim().is_empty()))
        .map(PathBuf::from)
}

fn write_run_metadata_file(meta: &RunMetadata, dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(metadata_file_name(&meta.case_id));
    meta.save_json(&path)?;
    Ok(path)
}

#[cfg(feature = "real-device")]
fn emit_record(meta: &RunMetadata) {
    let json = meta
        .to_json()
        .unwrap_or_else(|error| format!("{{\"serialize_error\":\"{error}\"}}"));
    eprintln!("run_metadata: {json}");
    let run_metadata_dir = std::env::var("SYNVELIS_RUN_METADATA_DIR").ok();
    let cargo_target_tmpdir = std::env::var("CARGO_TARGET_TMPDIR").ok();
    if let Some(dir) =
        metadata_output_dir_from(run_metadata_dir.as_deref(), cargo_target_tmpdir.as_deref())
    {
        match write_run_metadata_file(meta, &dir) {
            Ok(path) => eprintln!("run_metadata_path: {}", path.display()),
            Err(error) => eprintln!("run_metadata_path_error: {error}"),
        }
    }
}

#[test]
fn ruviform_stem_matches_only_ruviform_virtual_endpoints() {
    assert_eq!(
        ruviform_virtual_stem("Ruviform Cable Input"),
        Some("ruviform cable".to_owned())
    );
    assert_eq!(
        ruviform_virtual_stem("Ruviform Cable Output"),
        Some("ruviform cable".to_owned())
    );
    assert_eq!(
        ruviform_virtual_stem("Ruviform Virtual Audio Device Input"),
        Some("ruviform virtual audio device".to_owned())
    );
    assert_eq!(
        ruviform_virtual_stem("Speakers (Ruviform Virtual Audio Device)"),
        Some("ruviform virtual audio device".to_owned())
    );
}

#[test]
fn ruviform_stem_rejects_other_virtual_and_physical_names() {
    for name in [
        "CABLE Input (VB-Audio Virtual Cable)",
        "CABLE Output (VB-Audio Virtual Cable)",
        "Voicemeeter Input (VB-Audio Voicemeeter VAIO)",
        "Virtual Audio Cable Input",
        "Microphone (Audient iD4)",
        "Ruviform Microphone",
    ] {
        assert_eq!(ruviform_virtual_stem(name), None, "{name}");
    }
}

#[test]
fn endpoint_metadata_round_trips_case_schema_vocabulary() {
    let meta = endpoint_record(CASE_ID, ORACLE_ID)
        .with_expected("kebab-case vocabulary round-trip")
        .with_result(CaseResult::NotRun, "constructed only — never executed")
        .with_evidence("crates/synvelis-recording/tests/virtual_endpoint.rs");
    let json = meta.to_json().expect("metadata json");
    assert!(json.contains("\"environment_kind\":\"virtual-os-endpoint\""));
    assert!(json.contains("\"level\":\"ext-l3\""));
    assert!(json.contains("\"result\":\"not-run\""));
    let back = RunMetadata::from_json(&json).expect("metadata round-trip");
    assert_eq!(back, meta);
}

#[test]
fn pair_selection_distinguishes_not_provisioned_from_candidates() {
    assert_eq!(classify_pair_selection(0), PairSelection::NotProvisioned);
    assert_eq!(classify_pair_selection(1), PairSelection::Candidates(1));
    assert_eq!(classify_pair_selection(3), PairSelection::Candidates(3));
}

#[test]
fn metadata_output_dir_prefers_run_metadata_dir() {
    let selected =
        metadata_output_dir_from(Some("D:/runs/sv115/metadata"), Some("D:/cargo-target/tmp"));
    assert_eq!(
        selected,
        Some(std::path::PathBuf::from("D:/runs/sv115/metadata"))
    );
    assert_eq!(
        metadata_output_dir_from(None, Some("D:/cargo-target/tmp")),
        Some(std::path::PathBuf::from("D:/cargo-target/tmp"))
    );
    assert_eq!(metadata_output_dir_from(Some(" "), None), None);
}

#[test]
fn run_metadata_file_writes_one_machine_readable_json() {
    let dir = common::tmp_dir("virtual-endpoint-metadata-output");
    let meta = endpoint_record(CASE_ID, ORACLE_ID)
        .with_expected("metadata file round-trip")
        .with_result(CaseResult::NotRun, "constructed only — never executed")
        .with_evidence("crates/synvelis-recording/tests/virtual_endpoint.rs");
    let path = write_run_metadata_file(&meta, &dir).expect("write metadata");
    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("run-metadata-sv115-rcd01-virtual-endpoint-render-capture-recorder.json")
    );
    let entries = std::fs::read_dir(&dir)
        .expect("metadata dir")
        .collect::<Result<Vec<_>, _>>()
        .expect("metadata entries");
    assert_eq!(entries.len(), 1, "exactly one metadata file");
    let loaded = RunMetadata::load_json(&path).expect("metadata round-trip");
    assert_eq!(loaded, meta);
    std::fs::remove_dir_all(&dir).expect("cleanup metadata dir");
}

#[cfg(feature = "real-device")]
mod virtual_lane {
    use std::any::Any;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
    use std::thread;
    use std::time::Duration;

    use super::*;
    use common::{
        RealDeviceRig, all_journal_spans, assert_session_coverage, assert_source_contiguous,
        capture_of, plan_at, span_frame_total, tap, tmp_dir, verify_capture_integrity,
    };
    use ruviform_audio_core::device::{
        BufferSize, Device, DeviceEnumerator, DeviceError, OutputCallbackInfo, OutputDataCallback,
        OutputErrorCallback, OutputStream, OutputStreamConfig, SupportedOutputFormat,
        same_clock_domain,
    };
    use ruviform_audio_core::{
        AudioBufferViewMut, AudioFormat, ChannelLayout, SampleFormat, SampleRate,
    };
    use ruviform_audio_device::Host;
    use ruviform_audio_test::pcm::{PcmFixture, PcmPattern};
    use sha2::Digest as _;
    use synvelis_project::{
        CaptureStreamId, ClockEpochId, Command, CounterIdGenerator, DirectoryBundleStore, LaneId,
        MusicalTick, Project, ProjectController, ProjectId, ProjectOp, RecordingSessionId,
        SourceId,
    };
    use synvelis_recording::real_input::{default_host, negotiate_input_format, real_input_config};
    use synvelis_recording::{
        ArmState, ChannelMap, CommitOutcome, FinalizedOutcome, FinalizedSourceRef, GateSpec,
        InputStreamSpec, MonitorState, RecordingOp, StopReason, SynvelisOp, commit_session,
        load_finalized, read_commit_manifest,
    };

    const CAPTURE_STOP_TICK: i64 = 6_000;
    const PUMP_INTERVAL: Duration = Duration::from_millis(2);
    const MAX_RUN: Duration = Duration::from_secs(60);
    const REQUESTED_FRAMES: usize = 480;
    const REAL_MAX_BLOCK: u32 = 8192;
    const REAL_CALLBACK_QUEUE: usize = 256;
    const IMPULSE_AMPLITUDE: f32 = 0.8;
    const IMPULSE_PERIOD_MS: u64 = 100;
    const MARKER_THRESHOLD: f32 = 0.5;
    const MARKER_MIN_COUNT: usize = 8;
    const MARKER_INTERVAL_TOLERANCE: f64 = 0.10;
    const MARKER_REFRACTORY_MS: u64 = 20;
    const MIN_LEVEL_RATIO: f64 = 0.2;
    const MAX_LEVEL_RATIO: f64 = 3.0;
    const MAX_FIRST_MARKER_SECONDS: f64 = 1.0;

    #[derive(Copy, Clone)]
    enum Direction {
        Render,
        Capture,
    }

    struct EndpointCandidate {
        name: String,
        id: String,
        stem: Option<String>,
        format: Option<AudioFormat>,
        device: Box<dyn Device>,
    }

    struct EndpointPair {
        render: EndpointCandidate,
        capture: EndpointCandidate,
    }

    #[derive(Default)]
    struct OutputProbe {
        callbacks: AtomicU64,
        frames: AtomicU64,
        channel_mismatches: AtomicU64,
        timestamp_regressions: AtomicU64,
        last_callback_ns: AtomicI64,
        fill_errors: AtomicU64,
        errors: AtomicU64,
        render_cursor: AtomicU64,
        expected_channels: usize,
    }

    impl OutputProbe {
        fn new(format: AudioFormat) -> Self {
            Self {
                last_callback_ns: AtomicI64::new(i64::MIN),
                expected_channels: usize::try_from(format.layout.channels()).unwrap_or(usize::MAX),
                ..Self::default()
            }
        }
    }

    struct RoundTripObserved {
        input_rate: u32,
        output_rate: u32,
        session_frames: i64,
        pumps: usize,
        output_callbacks: u64,
        output_frames: u64,
        marker_summary: String,
        commit_summary: String,
        latency_summary: String,
    }

    fn spawn_watchdog() {
        thread::spawn(|| {
            thread::sleep(MAX_RUN);
            eprintln!("virtual_endpoint: watchdog timeout after {MAX_RUN:?} — aborting");
            process::exit(3);
        });
    }

    const fn layout_for_channels(channels: u32) -> Option<ChannelLayout> {
        match channels {
            1 => Some(ChannelLayout::Mono),
            2 => Some(ChannelLayout::Stereo),
            4 => Some(ChannelLayout::Quad),
            6 => Some(ChannelLayout::FivePointOne),
            8 => Some(ChannelLayout::SevenPointOne),
            _ => None,
        }
    }

    fn select_f32_output_format(supported: &[SupportedOutputFormat]) -> Option<AudioFormat> {
        supported
            .iter()
            .filter(|range| range.sample_format == SampleFormat::F32)
            .filter(|range| {
                range.min_sample_rate <= range.max_sample_rate && range.max_sample_rate != 0
            })
            .filter(|range| layout_for_channels(range.channels).is_some())
            .max_by_key(|range| {
                (
                    range.channels == 2,
                    range.min_sample_rate <= 48_000 && 48_000 <= range.max_sample_rate,
                )
            })
            .and_then(|range| {
                Some(AudioFormat {
                    sample_rate: SampleRate::new(
                        48_000.clamp(range.min_sample_rate, range.max_sample_rate),
                    )
                    .ok()?,
                    layout: layout_for_channels(range.channels)?,
                    sample_format: SampleFormat::F32,
                })
            })
    }

    fn negotiate_output_format(device: &dyn Device) -> Option<AudioFormat> {
        match device.default_output_format() {
            Ok(format) if format.sample_format == SampleFormat::F32 => Some(format),
            _ => select_f32_output_format(&device.supported_output_formats().ok()?),
        }
    }

    fn describe_candidate(
        device: Box<dyn Device>,
        direction: Direction,
    ) -> Option<EndpointCandidate> {
        let name = device.name().ok()?;
        let id = device
            .id()
            .ok()
            .flatten()
            .map_or_else(|| "?".to_owned(), |id| format!("{id:?}"));
        let format = match direction {
            Direction::Render => negotiate_output_format(device.as_ref()),
            Direction::Capture => negotiate_input_format(device.as_ref()),
        };
        let stem = ruviform_virtual_stem(&name);
        Some(EndpointCandidate {
            name,
            id,
            stem,
            format,
            device,
        })
    }

    fn enumerate_devices(
        host: &Host,
        direction: Direction,
    ) -> Result<Vec<EndpointCandidate>, String> {
        let (devices, label) = match direction {
            Direction::Render => (host.output_devices(), "output"),
            Direction::Capture => (host.input_devices(), "input"),
        };
        let devices = devices.map_err(|error| format!("ENUMERATION_FAILED {label}: {error}"))?;
        if devices.is_empty() {
            return Err(format!("ENUMERATION_EMPTY no {label} devices on this host"));
        }
        Ok(devices
            .into_iter()
            .filter_map(|device| describe_candidate(device, direction))
            .collect())
    }

    fn pair_candidates(
        renders: Vec<EndpointCandidate>,
        captures: Vec<EndpointCandidate>,
    ) -> Vec<EndpointPair> {
        let mut free: Vec<Option<EndpointCandidate>> = captures.into_iter().map(Some).collect();
        let mut pairs = Vec::new();
        for render in renders {
            let Some(stem) = &render.stem else { continue };
            let Some(index) = free.iter().position(|candidate| {
                candidate
                    .as_ref()
                    .is_some_and(|capture| capture.stem.as_deref() == Some(stem.as_str()))
            }) else {
                continue;
            };
            if let Some(capture) = free[index].take() {
                pairs.push(EndpointPair { render, capture });
            }
        }
        pairs
    }

    fn open_output_with_fallback(
        device: &dyn Device,
        format: AudioFormat,
        probe: &Arc<OutputProbe>,
        fixture: PcmFixture,
    ) -> Result<Box<dyn OutputStream>, DeviceError> {
        let requested = OutputStreamConfig {
            format,
            buffer_size: BufferSize::Fixed(REQUESTED_FRAMES),
        };
        match device.open_output_stream(
            &requested,
            output_data_callback(probe, fixture),
            output_error_callback(probe),
        ) {
            Ok(stream) => Ok(stream),
            Err(DeviceError::UnsupportedConfig) => {
                let fallback = OutputStreamConfig {
                    format,
                    buffer_size: BufferSize::Default,
                };
                device.open_output_stream(
                    &fallback,
                    output_data_callback(probe, fixture),
                    output_error_callback(probe),
                )
            }
            Err(error) => Err(error),
        }
    }

    fn output_data_callback(probe: &Arc<OutputProbe>, fixture: PcmFixture) -> OutputDataCallback {
        let probe = Arc::clone(probe);
        Box::new(move |mut out, info| observe_output(&probe, &mut out, info, fixture))
    }

    fn observe_output(
        probe: &OutputProbe,
        out: &mut AudioBufferViewMut<'_, f32>,
        info: OutputCallbackInfo,
        fixture: PcmFixture,
    ) {
        let frames = out.frames();
        probe.callbacks.fetch_add(1, Ordering::Relaxed);
        probe.frames.fetch_add(frames as u64, Ordering::Relaxed);
        if out.channels() != probe.expected_channels {
            probe.channel_mismatches.fetch_add(1, Ordering::Relaxed);
        }
        let callback_ns = info.timestamp.callback.value();
        let previous = probe
            .last_callback_ns
            .fetch_max(callback_ns, Ordering::Relaxed);
        if callback_ns < previous {
            probe.timestamp_regressions.fetch_add(1, Ordering::Relaxed);
        }
        let base = probe
            .render_cursor
            .fetch_add(frames as u64, Ordering::Relaxed);
        if fixture.fill_view(base, out).is_err() {
            probe.fill_errors.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn output_error_callback(probe: &Arc<OutputProbe>) -> OutputErrorCallback {
        let probe = Arc::clone(probe);
        Box::new(move |_error: DeviceError| {
            probe.errors.fetch_add(1, Ordering::Relaxed);
        })
    }

    fn analyze_output_probe(probe: &OutputProbe) -> Vec<String> {
        let mut failures = Vec::new();
        if probe.callbacks.load(Ordering::Relaxed) == 0 {
            failures.push("output stream delivered zero data callbacks".to_owned());
        }
        if probe.frames.load(Ordering::Relaxed) == 0 {
            failures.push("output stream delivered zero frames".to_owned());
        }
        for (label, count) in [
            (
                "output channel mismatches",
                probe.channel_mismatches.load(Ordering::Relaxed),
            ),
            (
                "output timestamp regressions",
                probe.timestamp_regressions.load(Ordering::Relaxed),
            ),
            (
                "output fixture fill errors",
                probe.fill_errors.load(Ordering::Relaxed),
            ),
            ("output stream errors", probe.errors.load(Ordering::Relaxed)),
        ] {
            if count > 0 {
                failures.push(format!("{label}: {count}"));
            }
        }
        failures
    }

    fn controller_with_tracks(n: usize) -> (ProjectController, Vec<LaneId>, ClockEpochId) {
        let mut c = ProjectController::new(
            Project::new(ProjectId::new(0), "sv115-virtual-endpoint"),
            Box::new(CounterIdGenerator::new(1)),
        );
        let mut lanes = Vec::with_capacity(n);
        for i in 0..n {
            c.apply(Command {
                expected_revision: c.live_revision(),
                op: ProjectOp::AddTrack {
                    name: format!("virtual-t{i}"),
                },
            })
            .expect("add track");
            let track = *c.live().tracks.keys().last().expect("track");
            c.apply(Command {
                expected_revision: c.live_revision(),
                op: ProjectOp::AddLane { track_id: track },
            })
            .expect("add lane");
            lanes.push(*c.live().lanes.keys().last().expect("lane"));
        }
        c.apply(Command {
            expected_revision: c.live_revision(),
            op: ProjectOp::RegisterClockEpoch {
                description: "virtual-endpoint".into(),
            },
        })
        .expect("register epoch");
        let epoch = *c.live().clock_epochs.keys().next().expect("epoch");
        (c, lanes, epoch)
    }

    fn rop(c: &mut ProjectController, op: RecordingOp) {
        c.apply(Command {
            expected_revision: c.live_revision(),
            op: SynvelisOp::Recording(op),
        })
        .expect("recording op");
    }

    fn open_session_ids(
        c: &mut ProjectController,
        lanes: &[LaneId],
        epoch: ClockEpochId,
    ) -> (RecordingSessionId, Vec<CaptureStreamId>) {
        rop(
            c,
            RecordingOp::CreateSession {
                name: "virtual-endpoint-session".into(),
            },
        );
        let session_id = *c.live().recording.sessions.keys().next().expect("session");
        rop(c, RecordingOp::OpenSession { session_id });
        let mut ids = Vec::with_capacity(lanes.len());
        for &lane in lanes {
            rop(
                c,
                RecordingOp::RegisterStream {
                    session_id,
                    lane_id: lane,
                    clock_epoch_id: epoch,
                },
            );
            ids.push(*c.live().recording.streams.keys().last().expect("stream"));
        }
        (session_id, ids)
    }

    fn real_budget() -> (synvelis_recording::BufferBudget, u32) {
        let mut b = common::budget();
        b.chunk_frames = REAL_MAX_BLOCK;
        b.chunks = 64;
        b.filled_capacity = 64;
        (b, REAL_MAX_BLOCK)
    }

    fn run_duration(session_frames: i64, rate: u32, margin: f64) -> Duration {
        let frames = u32::try_from(session_frames).expect("session frames fit u32");
        Duration::from_secs_f64(f64::from(frames) / f64::from(rate) + margin)
    }

    struct SignalAudit {
        markers: usize,
        mean_interval_ms: f64,
        first_marker_frame: u64,
        first_marker_latency_ms: f64,
        rms: f64,
        level_ratio: f64,
    }

    #[expect(clippy::cast_precision_loss)]
    fn analyze_signal(samples: &[f32], rate_hz: u32) -> Result<SignalAudit, String> {
        let minimum = usize::try_from(u64::from(rate_hz) * 2).unwrap_or(usize::MAX);
        if samples.len() < minimum {
            return Err(format!(
                "insufficient recorded audio for oracle: {} frames at {rate_hz} Hz",
                samples.len()
            ));
        }
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err("recorded PCM contains non-finite samples".to_owned());
        }
        let refractory = (u64::from(rate_hz) * MARKER_REFRACTORY_MS / 1_000).max(1);
        let mut markers: Vec<u64> = Vec::new();
        let mut next_allowed = 0_u64;
        for (index, &sample) in samples.iter().enumerate() {
            let frame = index as u64;
            if sample.abs() > MARKER_THRESHOLD && frame >= next_allowed {
                markers.push(frame);
                next_allowed = frame + refractory;
            }
        }
        if markers.len() < MARKER_MIN_COUNT {
            return Err(format!(
                "marker oracle: only {} markers detected (need {MARKER_MIN_COUNT})",
                markers.len()
            ));
        }
        let intervals: Vec<f64> = markers
            .windows(2)
            .map(|pair| (pair[1] - pair[0]) as f64 / f64::from(rate_hz))
            .collect();
        let mean = intervals.iter().sum::<f64>() / intervals.len() as f64;
        let expected_interval = IMPULSE_PERIOD_MS as f64 / 1_000.0;
        if (mean - expected_interval).abs() > expected_interval * MARKER_INTERVAL_TOLERANCE {
            return Err(format!(
                "marker oracle: mean interval {mean:.4}s deviates from {expected_interval:.4}s"
            ));
        }
        let sum_sq: f64 = samples
            .iter()
            .map(|&sample| f64::from(sample) * f64::from(sample))
            .sum();
        let rms = (sum_sq / samples.len() as f64).sqrt();
        let expected_rms =
            f64::from(IMPULSE_AMPLITUDE) / (f64::from(rate_hz) * expected_interval).sqrt();
        let ratio = if expected_rms > 0.0 {
            rms / expected_rms
        } else {
            0.0
        };
        if !(MIN_LEVEL_RATIO..=MAX_LEVEL_RATIO).contains(&ratio) {
            return Err(format!(
                "level oracle: rms ratio {ratio:.3} outside [{MIN_LEVEL_RATIO}, {MAX_LEVEL_RATIO}]"
            ));
        }
        let first = markers[0];
        let first_latency = first as f64 / f64::from(rate_hz);
        if first_latency > MAX_FIRST_MARKER_SECONDS {
            return Err(format!(
                "latency oracle: first marker at {first_latency:.3}s exceeds {MAX_FIRST_MARKER_SECONDS:.3}s"
            ));
        }
        Ok(SignalAudit {
            markers: markers.len(),
            mean_interval_ms: mean * 1_000.0,
            first_marker_frame: first,
            first_marker_latency_ms: first_latency * 1_000.0,
            rms,
            level_ratio: ratio,
        })
    }

    fn sha256_prefixed(bytes: &[u8]) -> String {
        format!("sha256:{:x}", sha2::Sha256::digest(bytes))
    }

    fn finalized_for(
        area: &synvelis_recording::StagingArea,
        stream_index: u16,
    ) -> FinalizedSourceRef {
        load_finalized(area)
            .into_iter()
            .find(|f| f.stream_index == stream_index)
            .and_then(|f| f.finalized)
            .unwrap_or_else(|| panic!("stream {stream_index} finalized ref"))
    }

    fn audit_virtual_stream(
        rig: &RealDeviceRig,
        stream_index: u16,
        session_frames: i64,
    ) -> SignalAudit {
        let fin = finalized_for(&rig.area, stream_index);
        assert_eq!(fin.outcome, FinalizedOutcome::Complete);
        assert_eq!(
            fin.source_frames,
            u64::try_from(session_frames).expect("session frames")
        );
        assert_eq!(fin.media_byte_len, fin.source_frames * 4);

        let cap = capture_of(&rig.area, stream_index);
        assert!(
            matches!(cap.replay.stop, StopReason::CleanEnd | StopReason::TornTail),
            "stream {stream_index} journal stop: {:?}",
            cap.replay.stop
        );
        let spans = all_journal_spans(&cap);
        assert_session_coverage(&spans, &[(0, session_frames)]);
        assert_source_contiguous(&spans, session_frames);
        assert_eq!(span_frame_total(&spans), session_frames);
        let checked = verify_capture_integrity(&cap, 1);
        assert_eq!(
            checked,
            usize::try_from(session_frames).expect("session frames")
        );

        let prefix_len = usize::try_from(fin.media_byte_len).expect("media len");
        let prefix = &cap.media[..prefix_len];
        assert_eq!(
            fin.verified_sha256.as_deref(),
            Some(sha256_prefixed(prefix).as_str()),
            "stream {stream_index} verified_sha256"
        );
        let samples: Vec<f32> = prefix
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes))
            .collect();
        analyze_signal(&samples, rig.sample_rate)
            .unwrap_or_else(|error| panic!("stream {stream_index} signal oracle: {error}"))
    }

    fn assert_digest_chain(rig: &RealDeviceRig, project: &ProjectController, stream_index: u16) {
        let fin = finalized_for(&rig.area, stream_index);
        let cap = capture_of(&rig.area, stream_index);
        let prefix = &cap.media[..usize::try_from(fin.media_byte_len).expect("media len")];
        assert_eq!(
            fin.verified_sha256.as_deref(),
            Some(sha256_prefixed(prefix).as_str()),
            "stream {stream_index} verified_sha256 recompute"
        );
        let manifest = read_commit_manifest(&rig.area)
            .expect("manifest read")
            .expect("commit manifest");
        let entry = manifest
            .payload
            .streams
            .iter()
            .find(|entry| entry.stream_index == stream_index)
            .unwrap_or_else(|| panic!("manifest entry for stream {stream_index}"));
        assert_eq!(entry.digest, fin.digest, "stream {stream_index} digest");
        assert_eq!(entry.verified_sha256, fin.verified_sha256);
        assert_eq!(entry.media_byte_len, fin.media_byte_len);
        let source_id = SourceId::from_tagged(&entry.source_id).expect("source id");
        let source = project
            .live()
            .sources
            .get(&source_id)
            .unwrap_or_else(|| panic!("source for stream {stream_index}"));
        assert_eq!(source.content_digest, entry.digest);
        let asset = project.live().assets.get(&source.asset_id).expect("asset");
        assert_eq!(asset.digest, entry.digest);
        assert_eq!(asset.digest, source.content_digest);
    }

    #[allow(clippy::too_many_lines)] // sequential end-to-end fixture scenario; splitting would obscure the stream lifecycle
    fn run_virtual_round_trip(pair: EndpointPair) -> Result<RoundTripObserved, Vec<String>> {
        let EndpointPair { render, capture } = pair;
        eprintln!(
            "  render: \"{}\" id={} | capture: \"{}\" id={} (candidate, unconfirmed)",
            render.name, render.id, capture.name, capture.id
        );
        let Some(input_format) = capture.format else {
            return Err(vec!["no F32-openable input format".to_owned()]);
        };
        let Some(output_format) = render.format else {
            return Err(vec!["no F32-openable output format".to_owned()]);
        };
        eprintln!("  negotiated in={input_format:?} out={output_format:?}");
        if input_format.layout.channels() < 2 || output_format.layout.channels() < 2 {
            return Err(vec![format!(
                "multi-track stereo split requires >=2 channels (in={} out={})",
                input_format.layout.channels(),
                output_format.layout.channels()
            )]);
        }

        let output_channels = u16::try_from(output_format.layout.channels()).unwrap_or(u16::MAX);
        let output_rate = output_format.sample_rate.get();
        let period_frames = (u64::from(output_rate) * IMPULSE_PERIOD_MS / 1_000).max(1);
        let fixture = PcmFixture::new(
            output_format.sample_rate,
            output_channels,
            PcmPattern::Impulse {
                period_frames: u32::try_from(period_frames).unwrap_or(u32::MAX),
                amplitude: IMPULSE_AMPLITUDE,
            },
        )
        .map_err(|error| vec![format!("fixture construction failed: {error}")])?;

        let probe = Arc::new(OutputProbe::new(output_format));
        let out_stream =
            match open_output_with_fallback(render.device.as_ref(), output_format, &probe, fixture)
            {
                Ok(stream) => stream,
                Err(error) => return Err(vec![format!("output open failed: {error}")]),
            };
        if out_stream.format() != output_format {
            return Err(vec![format!(
                "opened output format mismatch: requested {output_format:?} got {:?}",
                out_stream.format()
            )]);
        }
        if !same_clock_domain(out_stream.clock_domain(), out_stream.clock_domain()) {
            return Err(vec!["output stream clock domain is UNKNOWN".to_owned()]);
        }
        let output_latency = out_stream.output_latency();

        let input_rate = input_format.sample_rate.get();
        let (mut controller, lanes, epoch) = controller_with_tracks(2);
        let (session_id, stream_ids) = open_session_ids(&mut controller, &lanes, epoch);
        let (budget, max_block) = real_budget();
        let plan = plan_at(
            controller.live(),
            &lanes,
            &[1, 1],
            epoch,
            GateSpec::Normal {
                open: MusicalTick::ZERO,
            },
            CAPTURE_STOP_TICK,
            input_rate,
            budget,
            max_block,
        );
        let session_frames = plan.session_end.value();
        let spec = InputStreamSpec {
            config: real_input_config(input_format),
            clock_epoch: epoch,
            max_block_frames: plan.max_block_frames,
            callback_queue: REAL_CALLBACK_QUEUE,
            taps: vec![
                tap(
                    &plan,
                    0,
                    ChannelMap::mono(0),
                    ArmState::On,
                    MonitorState::Off,
                ),
                tap(
                    &plan,
                    1,
                    ChannelMap::mono(1),
                    ArmState::On,
                    MonitorState::Off,
                ),
            ],
        };
        let root = tmp_dir("virtual-endpoint");
        let mut rig = RealDeviceRig::open(
            &root,
            session_id,
            plan,
            input_rate,
            &[(capture.device.as_ref(), &spec)],
            Some(stream_ids.as_slice()),
        );
        let input_latency = rig
            .session
            .adapters()
            .first()
            .map(|adapter| adapter.stream().input_latency());
        eprintln!(
            "  latency queries: input={input_latency:?} output={output_latency:?} in_domain={:?} out_domain={:?}",
            rig.session.adapters()[0].stream().clock_domain(),
            out_stream.clock_domain()
        );

        let run = run_duration(session_frames, input_rate, 0.8);
        let (finish_report, fault_bits, pumps, stop_errors) = thread::scope(|scope| {
            let rig_ref = &mut rig;
            scope
                .spawn(move || {
                    rig_ref
                        .start_all()
                        .expect("input start on a present candidate must succeed");
                    out_stream
                        .start()
                        .expect("output start on a present candidate must succeed");
                    let pumps = rig_ref.run_for(run, PUMP_INTERVAL);
                    let mut stop_errors = Vec::new();
                    if let Err(error) = out_stream.stop() {
                        stop_errors.push(format!("output stop failed: {error}"));
                    }
                    if let Err(error) = rig_ref.stop_all() {
                        stop_errors.push(format!("input stop failed: {error}"));
                    }
                    rig_ref.pump_all();
                    let bits = rig_ref.session.fault().bits();
                    let report = rig_ref.finish();
                    (report, bits, pumps, stop_errors)
                })
                .join()
                .expect("drive thread panicked")
        });

        let mut failures = analyze_output_probe(&probe);
        failures.extend(stop_errors);
        if fault_bits != 0 {
            failures.push(format!(
                "input capture path faulted: bits {fault_bits:#06x}"
            ));
        }
        for (stream_index, result) in &finish_report.refs {
            match result {
                Ok(fin) if fin.outcome == FinalizedOutcome::Complete => {}
                Ok(fin) => failures.push(format!(
                    "stream {stream_index} finalize outcome: {:?}",
                    fin.outcome
                )),
                Err(error) => {
                    failures.push(format!("stream {stream_index} finalize error: {error:?}"));
                }
            }
        }
        if !failures.is_empty() {
            return Err(failures);
        }

        let audit0 = audit_virtual_stream(&rig, 0, session_frames);
        let audit1 = audit_virtual_stream(&rig, 1, session_frames);

        rop(&mut controller, RecordingOp::CloseSession { session_id });
        let report = commit_session(&mut controller, &rig.area).expect("commit_session");
        assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
        assert_eq!(report.sources_committed, 2);
        assert_eq!(report.recoverable.len(), 0);
        let mut store = DirectoryBundleStore::create(root.join("bundle")).expect("bundle store");
        controller.save(&mut store).expect("save");
        let mut reloaded = ProjectController::new(
            Project::new(ProjectId::new(0), "sv115-virtual-reload"),
            Box::new(CounterIdGenerator::new(1)),
        );
        reloaded.load(&store).expect("load");
        for stream_index in 0..2_u16 {
            assert_digest_chain(&rig, &controller, stream_index);
            assert_digest_chain(&rig, &reloaded, stream_index);
        }
        assert_eq!(controller.live().tracks.len(), 2);
        assert_eq!(controller.live().lanes.len(), 2);
        assert_eq!(controller.live().clips.len(), 2);
        assert_eq!(reloaded.live().sources.len(), 2);
        let report2 = commit_session(&mut reloaded, &rig.area).expect("recommit");
        assert!(matches!(report2.outcome, CommitOutcome::AlreadyApplied));

        Ok(RoundTripObserved {
            input_rate,
            output_rate,
            session_frames,
            pumps,
            output_callbacks: probe.callbacks.load(Ordering::Relaxed),
            output_frames: probe.frames.load(Ordering::Relaxed),
            marker_summary: format!(
                "s0 markers={} interval={:.2}ms first={} latency={:.1}ms rms={:.5} ratio={:.2}; s1 markers={} interval={:.2}ms first={} latency={:.1}ms rms={:.5} ratio={:.2}",
                audit0.markers,
                audit0.mean_interval_ms,
                audit0.first_marker_frame,
                audit0.first_marker_latency_ms,
                audit0.rms,
                audit0.level_ratio,
                audit1.markers,
                audit1.mean_interval_ms,
                audit1.first_marker_frame,
                audit1.first_marker_latency_ms,
                audit1.rms,
                audit1.level_ratio,
            ),
            commit_summary: format!(
                "commit sources={} takes={} clips={} reload_sources={} recommit=AlreadyApplied",
                report.sources_committed,
                report.takes_committed,
                report.clips_committed,
                reloaded.live().sources.len()
            ),
            latency_summary: format!(
                "input_latency={input_latency:?} output_latency={output_latency:?}"
            ),
        })
    }

    fn panic_message(payload: &Box<dyn Any + Send>) -> String {
        if let Some(message) = payload.downcast_ref::<&str>() {
            (*message).to_owned()
        } else if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else {
            "non-string panic payload".to_owned()
        }
    }

    fn run_pair_candidates(pairs: Vec<EndpointPair>, meta: &RunMetadata) {
        match classify_pair_selection(pairs.len()) {
            PairSelection::NotProvisioned => {
                let record = meta.clone().with_result(
                    CaseResult::NotRun,
                    "NOT_PROVISIONED — no Ruviform render↔capture virtual endpoint stem pair",
                );
                emit_record(&record);
                eprintln!(
                    "verdict: NotRun — NOT_PROVISIONED (no Ruviform Cable Input/Output endpoint pair; other virtual drivers are not provisioning evidence)"
                );
                return;
            }
            PairSelection::Candidates(count) => eprintln!("  Ruviform pair candidates: {count}"),
        }

        let mut pair_failures: Vec<String> = Vec::new();
        for (index, pair) in pairs.into_iter().enumerate() {
            eprintln!(
                "pair[{index}]: render=\"{}\" capture=\"{}\" (candidate, unconfirmed)",
                pair.render.name, pair.capture.name
            );
            let outcome = catch_unwind(AssertUnwindSafe(|| run_virtual_round_trip(pair)));
            match outcome {
                Ok(Ok(observed)) => {
                    let record = endpoint_record_at(CASE_ID, ORACLE_ID, observed.output_rate)
                        .with_expected(
                            "impulse train 0.8/100ms supplied via PcmFixture::fill_view; production Recorder stereo→2 mono tracks; marker/RMS/latency, journal verified prefix, commit→save→reload digest chain",
                        )
                        .with_result(
                            CaseResult::Pass,
                            format!(
                                "pair[{index}] EXT-L3 observed through real OS streams (in={} Hz out={} Hz session_frames={} pumps={} out_cb={} out_frames={}); {}; {}; {}",
                                observed.input_rate,
                                observed.output_rate,
                                observed.session_frames,
                                observed.pumps,
                                observed.output_callbacks,
                                observed.output_frames,
                                observed.marker_summary,
                                observed.latency_summary,
                                observed.commit_summary,
                            ),
                        )
                        .with_evidence("crates/synvelis-recording/tests/virtual_endpoint.rs")
                        .with_evidence("name match is a candidate, not proof; this run opened and streamed the endpoints");
                    emit_record(&record);
                    eprintln!("verdict: Pass — virtual endpoint Recorder round-trip observed");
                    return;
                }
                Ok(Err(failures)) => {
                    eprintln!("  pair[{index}] failed:");
                    for failure in &failures {
                        eprintln!("    - {failure}");
                    }
                    pair_failures.push(format!("pair[{index}]: {}", failures.join("; ")));
                }
                Err(payload) => {
                    let message = panic_message(&payload);
                    eprintln!("  pair[{index}] panicked: {message}");
                    pair_failures.push(format!("pair[{index}]: panic: {message}"));
                }
            }
        }
        let record = meta.clone().with_result(
            CaseResult::Fail,
            format!(
                "CANDIDATE_FAILED — no Ruviform pair candidate completed the Recorder round-trip: {}",
                pair_failures.join("; ")
            ),
        );
        emit_record(&record);
        panic!(
            "verdict: Fail — no Ruviform pair candidate completed the Recorder round-trip:\n{}",
            pair_failures.join("\n")
        );
    }

    #[test]
    #[ignore = "requires provisioned Ruviform Cable Input/Output endpoints (run `mise run test-recording-virtual-endpoint`)"]
    fn virtual_endpoint_records_through_production_recorder() {
        spawn_watchdog();
        let meta = endpoint_record(CASE_ID, ORACLE_ID);
        eprintln!("virtual_endpoint: Ruviform render→capture production Recorder verifier");

        let host = match default_host() {
            Ok(host) => host,
            Err(error) => {
                let record = meta.clone().with_result(
                    CaseResult::NotRun,
                    format!("HOST_ENUMERATION_FAILED default_host(): {error}"),
                );
                emit_record(&record);
                eprintln!("verdict: NotRun — default_host() failed: {error}");
                return;
            }
        };
        eprintln!("  host: {}", host.name());

        let renders = match enumerate_devices(&host, Direction::Render) {
            Ok(candidates) => candidates,
            Err(reason) => {
                let record = meta.clone().with_result(CaseResult::NotRun, reason.clone());
                emit_record(&record);
                eprintln!("verdict: NotRun — {reason}");
                return;
            }
        };
        let captures = match enumerate_devices(&host, Direction::Capture) {
            Ok(candidates) => candidates,
            Err(reason) => {
                let record = meta.clone().with_result(CaseResult::NotRun, reason.clone());
                emit_record(&record);
                eprintln!("verdict: NotRun — {reason}");
                return;
            }
        };
        for candidate in &renders {
            eprintln!(
                "  render candidate scan: \"{}\" stem={:?} format={:?}",
                candidate.name, candidate.stem, candidate.format
            );
        }
        for candidate in &captures {
            eprintln!(
                "  capture candidate scan: \"{}\" stem={:?} format={:?}",
                candidate.name, candidate.stem, candidate.format
            );
        }
        let pairs = pair_candidates(renders, captures);
        run_pair_candidates(pairs, &meta);
    }
}
