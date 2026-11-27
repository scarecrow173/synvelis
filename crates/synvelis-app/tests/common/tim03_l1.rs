//! Shared ruviform#630 L1 real-ABI harness helpers for TIM-03
//! integration tests.
//!
//! These helpers preserve the PR #668/#672 artifact contract: explicit
//! fixture/runner environment overrides win verbatim, otherwise cargo's
//! target-directory artifacts are resolved deterministically, the fixture
//! is staged beside the runner, and hash mismatches fail closed. They are
//! test harness code only; no production behavior lives here.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use ruviform_audio_plugin_bridge::DispatchStage;
use ruviform_media_core::TimeBase;
use ruviform_plugin_clap::host::ClapLibrary;
use ruviform_plugin_core::{ParamFlags, ParamId, ParamInfo, PluginDescriptor};
use ruviform_plugin_dock_test::artifact::resolve_artifact_named;
use ruviform_plugin_dock_test::sha256_hex;
use ruviform_plugin_vst3::host::Vst3Library;
use serde_json::json;
use synvelis_app::SessionEngineConfig;
use synvelis_player::driver::PumpPolicy;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_project::{
    BuildBudget, BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, LaneId,
    MediaDuration, MediaTimeValue, ParameterTargetId, PlaybackMapping, PluginRef, ProcessorBinding,
    ProcessorId, ProjectController, ProjectOp, ProjectRevision, RoutingActivationState,
    SourceFrame, SourceId, SourceRange, StripId, TimeAnchor, TrackId,
};

/// VST3 fixture artifact stem.
pub const VST3_STEM: &str = "ruviform_plugin_test_vst3";
/// CLAP fixture artifact stem.
pub const CLAP_STEM: &str = "ruviform_plugin_test_clap";
/// Explicit VST3 fixture path override.
pub const VST3_ENV: &str = "SYNVELIS_TIM03_REAL_ABI_VST3";
/// Explicit CLAP fixture path override.
pub const CLAP_ENV: &str = "SYNVELIS_TIM03_REAL_ABI_CLAP";
/// Explicit VST3 runner path override.
pub const VST3_RUNNER_ENV: &str = "SYNVELIS_TIM03_L1_VST3_RUNNER";
/// Explicit CLAP runner path override.
pub const CLAP_RUNNER_ENV: &str = "SYNVELIS_TIM03_L1_CLAP_RUNNER";
/// VST3 runner executable stem.
pub const VST3_RUNNER_STEM: &str = "synvelis-plugin-test-vst3-l1-runner";
/// CLAP runner executable stem.
pub const CLAP_RUNNER_STEM: &str = "synvelis-plugin-test-clap-l1-runner";
/// Gain fixture plugin name.
pub const TEST_GAIN: &str = "test_gain";
/// Instrument fixture plugin name.
pub const TEST_INSTRUMENT: &str = "test_instrument";
/// Harness sample rate.
pub const SAMPLE_RATE: u32 = 48_000;
/// Harness block size.
pub const BLOCK_FRAMES: usize = 256;
/// Generated media source length.
pub const SOURCE_FRAMES: usize = 19_200;
/// First clip end used by the shared project fixture.
pub const FIRST_CLIP_END: u64 = 9_600;
/// Constant input magnitude after 16-bit import normalization.
pub const INPUT: f32 = 0.25;
/// PCM comparison tolerance.
pub const TOLERANCE: f32 = 0.000_001;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

/// Creates a unique temporary scratch directory for one test.
pub fn scratch(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "synvelis-tim03l1-{}-{}-{}",
        name,
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub struct FixtureProbe {
    pub format: &'static str,
    pub path: PathBuf,
    pub sha256: String,
    pub descriptors: Vec<PluginDescriptor>,
    pub gain: PluginDescriptor,
}

pub fn lib_name(stem: &str) -> String {
    format!(
        "{}{}{}",
        env::consts::DLL_PREFIX,
        stem,
        env::consts::DLL_SUFFIX
    )
}

pub fn resolve_fixture(explicit: Option<&Path>, stem: &str) -> Result<PathBuf, String> {
    let name = lib_name(stem);
    match resolve_artifact_named(explicit, &name) {
        Err(reason) if explicit.is_none() => find_built_fixture(stem).ok_or(reason),
        result => result,
    }
}

pub fn find_built_fixture(stem: &str) -> Option<PathBuf> {
    let dir = env::current_exe().ok()?.parent()?.to_path_buf();
    let name = lib_name(stem);
    if let Some(root) = dir.parent() {
        let candidate = root.join(&name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let prefix = format!("{}{}-", env::consts::DLL_PREFIX, stem);
    let mut names: Vec<String> = fs::read_dir(&dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .filter(|name| name.starts_with(&prefix) && name.ends_with(env::consts::DLL_SUFFIX))
        .collect();
    names.sort_unstable();
    names.first().map(|name| dir.join(name))
}

pub fn probe_fixture(
    format: &'static str,
    path: PathBuf,
    descriptors: Vec<PluginDescriptor>,
) -> FixtureProbe {
    let sha256 = sha256_hex(&path).expect("resolved fixture must hash");
    let gain = descriptors
        .iter()
        .find(|descriptor| descriptor.name == TEST_GAIN)
        .copied()
        .unwrap_or_else(|| panic!("{format} fixture does not expose {TEST_GAIN}"));
    FixtureProbe {
        format,
        path,
        sha256,
        descriptors,
        gain,
    }
}

pub fn load_vst3() -> (FixtureProbe, Vst3Library) {
    let explicit = env::var_os(VST3_ENV).map(PathBuf::from);
    let path = resolve_fixture(explicit.as_deref(), VST3_STEM)
        .unwrap_or_else(|reason| panic!("VST3 fixture artifact unresolved: {reason}"));
    let library = Vst3Library::load(&path).expect("resolved VST3 fixture must load");
    let probe = probe_fixture("vst3", path, library.descriptors().to_vec());
    (probe, library)
}

pub fn load_clap() -> (FixtureProbe, ClapLibrary) {
    let explicit = env::var_os(CLAP_ENV).map(PathBuf::from);
    let path = resolve_fixture(explicit.as_deref(), CLAP_STEM)
        .unwrap_or_else(|reason| panic!("CLAP fixture artifact unresolved: {reason}"));
    let library = ClapLibrary::load(&path).expect("resolved CLAP fixture must load");
    let probe = probe_fixture("clap", path, library.descriptors().to_vec());
    (probe, library)
}

pub fn runner_program(env_name: &str, stem: &str) -> OsString {
    if let Some(path) = env::var_os(env_name).map(PathBuf::from) {
        assert!(
            path.is_file(),
            "{env_name} does not name a runner: {}",
            path.display()
        );
        return path.into_os_string();
    }
    let exe = env::current_exe().expect("current_exe");
    let deps = exe.parent().expect("test executable directory");
    let profile = deps.parent().expect("cargo profile directory");
    let path = profile.join(format!("{stem}{}", env::consts::EXE_SUFFIX));
    assert!(
        path.is_file(),
        "L1 runner missing at {} — build `synvelis-plugin-test-l1-runner` through the canonical mise L1 task",
        path.display()
    );
    path.into_os_string()
}

pub fn stage_fixture_sibling(probe: &FixtureProbe, runner: &Path) -> PathBuf {
    let dir = runner.parent().expect("runner parent directory");
    let staged = dir.join(lib_name(match probe.format {
        "vst3" => VST3_STEM,
        "clap" => CLAP_STEM,
        other => panic!("unknown fixture format {other}"),
    }));
    if staged == probe.path {
        return staged;
    }
    if staged.exists() {
        let existing = sha256_hex(&staged).expect("staged fixture must hash");
        assert_eq!(
            existing,
            probe.sha256,
            "stale staged fixture {} does not match resolved artifact {}",
            staged.display(),
            probe.path.display()
        );
        return staged;
    }
    fs::copy(&probe.path, &staged).unwrap_or_else(|error| {
        panic!(
            "failed to stage fixture {} as {}: {error}",
            probe.path.display(),
            staged.display()
        )
    });
    staged
}

pub fn descriptor_named(probe: &FixtureProbe, name: &str) -> PluginDescriptor {
    probe
        .descriptors
        .iter()
        .find(|descriptor| descriptor.name == name)
        .copied()
        .unwrap_or_else(|| panic!("{} fixture does not expose {name}", probe.format))
}

pub fn file_facts(path: &Path) -> (u64, String) {
    (fs::metadata(path).unwrap().len(), sha256_hex(path).unwrap())
}

pub fn ruviform_revision() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("Cargo.lock");
    let text = fs::read_to_string(&root).expect("workspace Cargo.lock");
    text.lines()
        .find_map(|line| {
            line.strip_prefix("source = \"git+https://github.com/scarecrow173/ruviform.git#")
                .and_then(|rest| rest.strip_suffix('"'))
                .map(str::to_owned)
        })
        .expect("ruviform git revision in Cargo.lock")
}

pub fn gain_param() -> ParamInfo {
    ParamInfo {
        id: ParamId(0),
        name: "Gain",
        unit: "",
        min_value: 0.0,
        max_value: 2.0,
        default_value: 1.0,
        flags: ParamFlags::AUTOMATABLE,
    }
}

pub fn frame_base() -> TimeBase {
    TimeBase::sample_rate(SAMPLE_RATE).unwrap()
}

pub fn apply(controller: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    controller
        .apply(Command {
            expected_revision: controller.live_revision(),
            op,
        })
        .unwrap()
}

pub fn media_placement(start_frame: u64, end_frame: u64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Absolute(
            MediaTimeValue::new(i64::try_from(start_frame).unwrap(), 1, SAMPLE_RATE).unwrap(),
        ),
        extent: ClipExtent::Absolute(
            MediaDuration::new(
                i64::try_from(end_frame - start_frame).unwrap(),
                1,
                SAMPLE_RATE,
            )
            .unwrap(),
        ),
    }
}

pub fn natural_playback(offset: i64, frames: i64) -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::new(offset).unwrap(),
            SourceFrame::new(offset + frames).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

pub fn session_config(media_root: PathBuf) -> SessionEngineConfig {
    SessionEngineConfig {
        media_root,
        engine: synvelis_player::lowering::EngineConfig {
            sample_rate: SAMPLE_RATE,
            channels: 2,
            block_frames: BLOCK_FRAMES,
        },
        prefetch: ruviform_media_reader::PrefetchConfig {
            block_frames: BLOCK_FRAMES,
            block_count: 8,
            channels: 2,
            sample_rate: SAMPLE_RATE,
        },
        budget: BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
        mode: BuildMode::Realtime,
        runtime_instance: synvelis_project::RuntimeInstanceId(1),
        device_clock_epoch: synvelis_project::RuntimeClockEpochRef(1),
        engine_config_generation: 1,
    }
}

pub fn write_constant_wav(path: &Path, frames: usize) {
    let channels = 2_u16;
    let bits = 16_u16;
    let block_align = channels * (bits / 8);
    let byte_rate = SAMPLE_RATE * u32::from(block_align);
    let data_len = u32::try_from(frames * usize::from(block_align)).unwrap();
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&bits.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for _ in 0..frames {
        bytes.extend_from_slice(&8_192_i16.to_le_bytes());
        bytes.extend_from_slice(&(-8_192_i16).to_le_bytes());
    }
    fs::write(path, bytes).unwrap();
}

pub fn import_constant_stereo(
    controller: &mut ProjectController,
    root: &Path,
) -> synvelis_player::ImportOutcome {
    let wav = root.join("constant.wav");
    write_constant_wav(&wav, SOURCE_FRAMES);
    import_local(
        controller,
        &ImportRequest {
            path: wav,
            staging_root: root.join("staging"),
            media_root: root.join("media"),
            resume: None,
        },
    )
    .unwrap()
}

pub fn add_track_strip(controller: &mut ProjectController) -> (TrackId, StripId, LaneId) {
    apply(
        controller,
        ProjectOp::AddTrack {
            name: "track".into(),
        },
    );
    let track = *controller.live().tracks.keys().next_back().unwrap();
    apply(controller, ProjectOp::AddTrackStrip { track_id: track });
    let strip = controller.live().routing.strip_of_track(track).unwrap().id;
    apply(controller, ProjectOp::AddLane { track_id: track });
    let lane = *controller.live().tracks[&track].lane_ids.last().unwrap();
    (track, strip, lane)
}

pub fn add_clip(
    controller: &mut ProjectController,
    lane: LaneId,
    source: SourceId,
    start: u64,
    end: u64,
    offset: i64,
) -> ClipId {
    apply(
        controller,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: media_placement(start, end),
            playback: natural_playback(offset, i64::try_from(end - start).unwrap()),
        },
    );
    *controller.live().clips.keys().next_back().unwrap()
}

pub fn insert_gain_processor(
    controller: &mut ProjectController,
    strip: StripId,
    descriptor: &str,
) -> ProcessorId {
    insert_processor(controller, strip, TEST_GAIN, descriptor)
}

/// Inserts one resolved plugin processor under `name`.
pub fn insert_processor(
    controller: &mut ProjectController,
    strip: StripId,
    name: &str,
    descriptor: &str,
) -> ProcessorId {
    apply(
        controller,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: name.into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: descriptor.to_owned(),
                version: "0.0.0".into(),
                capability_revision: 0,
            }),
            sidechain_input: false,
        },
    );
    controller
        .live()
        .routing
        .processors
        .keys()
        .next_back()
        .copied()
        .expect("inserted processor id")
}

pub fn activate_generation(
    engine: &synvelis_app::SessionRoutingEngine,
    context: &str,
) -> (u64, synvelis_player::driver::PlayReport) {
    let report = engine.run(2, PumpPolicy::Every(1)).unwrap();
    match engine.receipt().expect("routing activation receipt").state {
        RoutingActivationState::Active { generation, .. } => (generation, report),
        other => panic!("{context}: {other:?}"),
    }
}

pub fn run_paced(
    engine: &synvelis_app::SessionRoutingEngine,
    blocks: usize,
) -> synvelis_player::driver::PlayReport {
    let mut pcm = vec![Vec::new(), Vec::new()];
    let mut crc = 0_u64;
    for _ in 0..blocks {
        thread::sleep(Duration::from_millis(20));
        let report = engine.run(1, PumpPolicy::Every(1)).unwrap();
        assert_eq!(report.pcm.len(), 2);
        for (out, input) in pcm.iter_mut().zip(&report.pcm) {
            out.extend_from_slice(input);
        }
        crc = crc.wrapping_mul(31).wrapping_add(report.crc);
    }
    synvelis_player::driver::PlayReport {
        pcm,
        crc,
        frames: (blocks * BLOCK_FRAMES) as u64,
    }
}

pub fn block_offset(frame: i64) -> u32 {
    let block_frames = i64::try_from(BLOCK_FRAMES).unwrap();
    u32::try_from(frame % block_frames).unwrap()
}

pub fn find_dispatched(
    cycles: &[synvelis_player::DispatchCycle],
    target: ParameterTargetId,
    value: f64,
) -> (u64, u32, i64) {
    for cycle in cycles {
        for (observed_target, observation) in &cycle.observations {
            if *observed_target != target {
                continue;
            }
            let observed_value = observation.value.map(ruviform_plugin_core::ParamValue::get);
            if observed_value != Some(value) {
                continue;
            }
            if let DispatchStage::Dispatched {
                call_start,
                frame_offset,
            } = observation.stage
            {
                return (call_start, frame_offset, observation.position.value());
            }
        }
    }
    panic!("no Dispatched observation for target {target:?} value {value}");
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockClass {
    Silence,
    Unity,
    Half,
    ThreeQuarter,
    WrongQuarter,
    Other,
}

impl BlockClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::Silence => "silence",
            Self::Unity => "input*1.0",
            Self::Half => "input*0.5",
            Self::ThreeQuarter => "input*0.75",
            Self::WrongQuarter => "input*0.25",
            Self::Other => "other",
        }
    }
}

fn classify_sample(left: f32, right: f32, gain: f32) -> bool {
    (left - INPUT * gain).abs() <= TOLERANCE && (right + INPUT * gain).abs() <= TOLERANCE
}

pub fn classify_blocks(
    start_index: usize,
    left: &[f32],
    right: &[f32],
    classes: &mut Vec<serde_json::Value>,
) {
    assert_eq!(left.len(), right.len());
    assert_eq!(left.len() % BLOCK_FRAMES, 0);
    for (block, (left, right)) in left
        .as_chunks::<BLOCK_FRAMES>()
        .0
        .iter()
        .zip(right.as_chunks::<BLOCK_FRAMES>().0)
        .enumerate()
    {
        let class = if left.iter().all(|sample| sample.abs() <= TOLERANCE)
            && right.iter().all(|sample| sample.abs() <= TOLERANCE)
        {
            BlockClass::Silence
        } else if left
            .iter()
            .zip(right)
            .all(|(left, right)| classify_sample(*left, *right, 1.0))
        {
            BlockClass::Unity
        } else if left
            .iter()
            .zip(right)
            .all(|(left, right)| classify_sample(*left, *right, 0.5))
        {
            BlockClass::Half
        } else if left
            .iter()
            .zip(right)
            .all(|(left, right)| classify_sample(*left, *right, 0.75))
        {
            BlockClass::ThreeQuarter
        } else if left
            .iter()
            .zip(right)
            .all(|(left, right)| classify_sample(*left, *right, 0.25))
        {
            BlockClass::WrongQuarter
        } else {
            BlockClass::Other
        };
        classes.push(json!({
            "block": start_index + block,
            "class": class.as_str(),
            "left_first": left[0],
            "right_first": right[0],
        }));
    }
}
