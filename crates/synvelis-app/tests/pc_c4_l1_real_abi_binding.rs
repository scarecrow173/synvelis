//! PC-C4 L1 real ABI binding acceptance slice (synvelis#101).
//!
//! This test drives the production composition seam with the ruviform#630
//! `test_gain` VST3/CLAP fixtures as typed L1 out-of-process routes. It is
//! deliberately a bounded slice: binding pins, verified frame base,
//! generation replacement, control-ledger frame offsets, and a
//! normalized/plain PCM value oracle are in scope; the full automation,
//! tempo/loop, GUI, fault, and RT/offline parity matrices are not.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use ruviform_audio_plugin_bridge::DispatchStage;
use ruviform_media_core::{
    ClockEpochId, EngineFrame, LoopOccurrence, MediaTime, PlanGeneration, SpanContext,
    TempoMapRevision, TimeBase, TransportGeneration, TransportSpan, TransportStatus,
};
use ruviform_plugin_clap::host::ClapLibrary;
use ruviform_plugin_core::{ParamFlags, ParamId, ParamInfo, ParamWriteMeans, PluginDescriptor};
use ruviform_plugin_dock_test::artifact::resolve_artifact_named;
use ruviform_plugin_dock_test::sha256_hex;
use ruviform_plugin_host::OutOfProcessSpec;
use ruviform_plugin_vst3::host::Vst3Library;
use ruviform_ui::BackendReport;
use serde_json::json;
use synvelis_app::{
    ProductOutOfProcessPlugin, SessionEngineConfig, SynvelisApp,
    compose_routing_engine_with_out_of_process,
};
use synvelis_player::driver::PumpPolicy;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::{
    FastControlOutcome, FastControlWrite, PluginParameterBinding, SurfaceReport, TickReport,
};
use synvelis_project::{
    AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange, BuildBudget,
    BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, ConflictPolicy,
    ControlOwner, Interpolation, LaneId, MappingResolution, MediaDuration, MediaTimeValue,
    ParamMapping, ParameterRole, ParameterTargetId, PlaybackMapping, PluginInstanceId, PluginRef,
    ProcessorBinding, ProcessorId, Project, ProjectController, ProjectOp, ProjectRevision,
    RoutingActivationState, SemanticNodeRef, SourceFrame, SourceId, SourceRange, StripId,
    TargetCapability, TimeAnchor, TrackId, ValueKind, port_capability_generation,
};

const VST3_STEM: &str = "ruviform_plugin_test_vst3";
const CLAP_STEM: &str = "ruviform_plugin_test_clap";
const VST3_ENV: &str = "SYNVELIS_TIM03_REAL_ABI_VST3";
const CLAP_ENV: &str = "SYNVELIS_TIM03_REAL_ABI_CLAP";
const VST3_RUNNER_ENV: &str = "SYNVELIS_TIM03_L1_VST3_RUNNER";
const CLAP_RUNNER_ENV: &str = "SYNVELIS_TIM03_L1_CLAP_RUNNER";
const VST3_RUNNER_STEM: &str = "synvelis-plugin-test-vst3-l1-runner";
const CLAP_RUNNER_STEM: &str = "synvelis-plugin-test-clap-l1-runner";
const TEST_GAIN: &str = "test_gain";
const SAMPLE_RATE: u32 = 48_000;
const BLOCK_FRAMES: usize = 256;
const SOURCE_FRAMES: usize = 19_200;
const FIRST_CLIP_END: u64 = 9_600;
const FAST_FRAME: i64 = 900;
const FAST_VALUE: f64 = 0.25;
const AUTOMATION_FRAME: i64 = 4_900;
const AUTOMATION_VALUE: f64 = 0.375;
const INPUT: f32 = 0.25;
const TOLERANCE: f32 = 0.000_001;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct FixtureProbe {
    format: &'static str,
    path: PathBuf,
    sha256: String,
    descriptors: Vec<PluginDescriptor>,
    gain: PluginDescriptor,
}

fn scratch(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "synvelis-pcc4l1-{}-{}-{}",
        name,
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn lib_name(stem: &str) -> String {
    format!(
        "{}{}{}",
        env::consts::DLL_PREFIX,
        stem,
        env::consts::DLL_SUFFIX
    )
}

fn resolve_fixture(explicit: Option<&Path>, stem: &str) -> Result<PathBuf, String> {
    let name = lib_name(stem);
    match resolve_artifact_named(explicit, &name) {
        Err(reason) if explicit.is_none() => find_built_fixture(stem).ok_or(reason),
        result => result,
    }
}

fn find_built_fixture(stem: &str) -> Option<PathBuf> {
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

fn probe_fixture(
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

fn load_vst3() -> (FixtureProbe, Vst3Library) {
    let explicit = env::var_os(VST3_ENV).map(PathBuf::from);
    let path = resolve_fixture(explicit.as_deref(), VST3_STEM)
        .unwrap_or_else(|reason| panic!("VST3 fixture artifact unresolved: {reason}"));
    let library = Vst3Library::load(&path).expect("resolved VST3 fixture must load");
    let probe = probe_fixture("vst3", path, library.descriptors().to_vec());
    (probe, library)
}

fn load_clap() -> (FixtureProbe, ClapLibrary) {
    let explicit = env::var_os(CLAP_ENV).map(PathBuf::from);
    let path = resolve_fixture(explicit.as_deref(), CLAP_STEM)
        .unwrap_or_else(|reason| panic!("CLAP fixture artifact unresolved: {reason}"));
    let library = ClapLibrary::load(&path).expect("resolved CLAP fixture must load");
    let probe = probe_fixture("clap", path, library.descriptors().to_vec());
    (probe, library)
}

fn runner_program(env_name: &str, stem: &str) -> OsString {
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
        "L1 runner missing at {} — run `mise run test-pc-c4-l1` (or build synvelis-plugin-test-l1-runner)",
        path.display()
    );
    path.into_os_string()
}

fn stage_fixture_sibling(probe: &FixtureProbe, runner: &Path) -> PathBuf {
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

fn gain_param() -> ParamInfo {
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

fn frame_base() -> TimeBase {
    TimeBase::sample_rate(SAMPLE_RATE).unwrap()
}

fn apply(controller: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    controller
        .apply(Command {
            expected_revision: controller.live_revision(),
            op,
        })
        .unwrap()
}

fn media_placement(start_frame: u64, end_frame: u64) -> ClipPlacement {
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

fn natural_playback(offset: i64, frames: i64) -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::new(offset).unwrap(),
            SourceFrame::new(offset + frames).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

fn session_config(media_root: PathBuf) -> SessionEngineConfig {
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

fn write_constant_wav(path: &Path, frames: usize) {
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

fn import_constant_stereo(
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

fn add_track_strip(controller: &mut ProjectController) -> (TrackId, StripId, LaneId) {
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

fn add_clip(
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

fn insert_gain_processor(
    controller: &mut ProjectController,
    strip: StripId,
    descriptor: &str,
) -> ProcessorId {
    apply(
        controller,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: TEST_GAIN.into(),
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

fn add_gain_target(
    controller: &mut ProjectController,
    processor: ProcessorId,
) -> ParameterTargetId {
    apply(
        controller,
        ProjectOp::AddParameterTarget {
            name: "gain".into(),
            owner: ControlOwner::Processor(processor),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(TargetCapability {
                param_id: 0,
                flags_bits: ParamFlags::AUTOMATABLE.bits(),
                min_value: 0.0,
                max_value: 2.0,
                default_value: 1.0,
            }),
            base: 1.0,
        },
    );
    let target = controller
        .live()
        .control
        .targets
        .values()
        .find(|target| target.owner == ControlOwner::Processor(processor))
        .unwrap()
        .id;
    apply(controller, ProjectOp::AddAutomationLane { target });
    let lane = *controller
        .live()
        .control
        .lanes
        .values()
        .find(|lane| lane.target == target)
        .map(|lane| &lane.id)
        .unwrap();
    apply(
        controller,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(0, 1).unwrap(),
                    BeatPoint::new(5, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Linear,
                points: vec![
                    AutomationPoint {
                        pos: BeatPoint::new(0, 1).unwrap(),
                        value: AUTOMATION_VALUE,
                    },
                    AutomationPoint {
                        pos: BeatPoint::new(4, 1).unwrap(),
                        value: AUTOMATION_VALUE,
                    },
                ],
            },
            policy: ConflictPolicy::Replace,
        },
    );
    target
}

fn build_project(
    app: &mut SynvelisApp,
    root: &Path,
    descriptor: &str,
) -> (ProcessorId, LaneId, ParameterTargetId, SourceId) {
    let controller = app.workspace_mut().session_mut().controller_mut();
    let (_, strip, lane) = add_track_strip(controller);
    let processor = insert_gain_processor(controller, strip, descriptor);
    let target = add_gain_target(controller, processor);
    let imported = import_constant_stereo(controller, root);
    let source = imported.commit.source_id;
    add_clip(controller, lane, source, 0, FIRST_CLIP_END, 0);
    (processor, lane, target, source)
}

fn activate_generation(
    engine: &synvelis_app::SessionRoutingEngine,
    context: &str,
) -> (u64, synvelis_player::driver::PlayReport) {
    let report = engine.run(2, PumpPolicy::Every(1)).unwrap();
    match engine.receipt().expect("routing activation receipt").state {
        RoutingActivationState::Active { generation, .. } => (generation, report),
        other => panic!("{context}: {other:?}"),
    }
}

/// Runs one block at a time with deliberate wall-clock pacing. L1 execution
/// is asynchronous: a tight headless loop can retire every lookahead window
/// before the child has had CPU time, which would measure scheduling
/// starvation rather than the plugin's DSP law.
fn run_paced(
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

fn span(end: u64) -> TransportSpan {
    TransportSpan {
        start: EngineFrame::new(ClockEpochId::new(0), 0),
        end: EngineFrame::new(ClockEpochId::new(0), end),
        ctx: SpanContext {
            status: TransportStatus::Playing,
            position_ticks: 0,
            position_base: frame_base(),
            loop_ticks: None,
            occurrence: LoopOccurrence::new(0),
            clock_epoch: ClockEpochId::new(0),
            transport_generation: TransportGeneration::new(0),
            plan_generation: PlanGeneration::new(1),
            tempo_revision: TempoMapRevision::new(0),
        },
        deliverable: true,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockClass {
    Silence,
    Unity,
    Half,
    WrongQuarter,
    Other,
}

impl BlockClass {
    fn as_str(self) -> &'static str {
        match self {
            Self::Silence => "silence",
            Self::Unity => "input*1.0",
            Self::Half => "input*0.5",
            Self::WrongQuarter => "input*0.25",
            Self::Other => "other",
        }
    }
}

fn classify_sample(left: f32, right: f32, gain: f32) -> bool {
    (left - INPUT * gain).abs() <= TOLERANCE && (right + INPUT * gain).abs() <= TOLERANCE
}

fn classify_blocks(
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

fn find_dispatched(
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

fn file_facts(path: &Path) -> (u64, String) {
    (
        fs::metadata(path).expect("artifact metadata").len(),
        sha256_hex(path).expect("artifact sha256"),
    )
}

fn ruviform_revision() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
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

struct ArtifactEvidence {
    runner_path: PathBuf,
    staged_fixture: PathBuf,
    fixture_size: u64,
    runner_size: u64,
    runner_sha256: String,
    staged_size: u64,
    staged_sha256: String,
}

struct InitialBindingEvidence {
    report: SurfaceReport,
    generation: u64,
    activation_report: synvelis_player::driver::PlayReport,
    control_frame_base: TimeBase,
    capability_generation: u64,
    persisted_instance: PluginInstanceId,
    binding: PluginParameterBinding,
}

struct ControlEvidence {
    automation_tick: TickReport,
    fast_call_start: u64,
    fast_frame_offset: u32,
    automation_call_start: u64,
    automation_frame_offset: u32,
    pcm_classes: Vec<serde_json::Value>,
    unity_index: usize,
    half_index: usize,
}

struct GenerationEvidence {
    replacement: u64,
    initial_handles_after_replacement: usize,
    replacement_handles: usize,
}

fn block_offset(frame: i64) -> u32 {
    let block_frames = i64::try_from(BLOCK_FRAMES).unwrap();
    u32::try_from(frame % block_frames).unwrap()
}

fn verify_initial_binding(
    engine: &synvelis_app::SessionRoutingEngine,
    project: &Project,
    processor: ProcessorId,
    target: ParameterTargetId,
    probe: &FixtureProbe,
) -> InitialBindingEvidence {
    let report = engine.control_report().expect("control surface report");
    assert_eq!(report.bound_targets, 1, "target did not bind on publish");
    assert!(
        report.unbound.is_empty(),
        "target unexpectedly unbound: {:?}",
        report.unbound
    );
    assert!(
        report.without_control.is_empty(),
        "processor has no control surface: {:?}",
        report.without_control
    );

    let (generation, activation_report) =
        activate_generation(engine, "initial production activation failed");
    assert_eq!(engine.control_generation(), Some(generation));
    let control_frame_base = engine
        .control_frame_base()
        .expect("verified control frame base");
    assert_eq!(control_frame_base, frame_base());

    let capability_generation = port_capability_generation(project);
    let authored_plugin = PluginRef {
        descriptor: probe.gain.id.to_owned(),
        version: "0.0.0".into(),
        capability_revision: 0,
    };
    let persisted_instance = project
        .plugin_instances
        .resolve(processor, &authored_plugin)
        .expect("persisted plugin instance identity");
    let binding = engine
        .with_control_dispatch(|dispatch| dispatch.binding(target))
        .flatten()
        .expect("target must expose a verified PluginParameterBinding");
    assert_eq!(binding.synvelis_target, target);
    assert_eq!(binding.plugin, authored_plugin);
    assert_eq!(binding.instance_id.get(), persisted_instance.get());
    assert_eq!(binding.plugin_param, ParamId(0));
    assert_eq!(binding.graph_generation, generation);
    assert_eq!(binding.capability_generation, capability_generation);
    assert_eq!(
        binding.attach_clock_context.frame_base(),
        control_frame_base
    );
    assert!(
        binding
            .attach_clock_context
            .check_frame_base(TimeBase::sample_rate(44_100).unwrap())
            .is_err(),
        "foreign frame base must fail the verified-context check"
    );
    let source_map = engine.source_map().expect("active routing source map");
    let node_resolution = source_map
        .semantic_nodes
        .get(&SemanticNodeRef::Processor(processor))
        .expect("processor source-map entry");
    assert!(
        matches!(node_resolution, MappingResolution::Resolved(node) if *node == binding.node),
        "binding node does not match the generation-pinned source map entry"
    );

    InitialBindingEvidence {
        report,
        generation,
        activation_report,
        control_frame_base,
        capability_generation,
        persisted_instance,
        binding,
    }
}

fn assert_pcm_oracle(pcm_classes: &[serde_json::Value]) -> (usize, usize) {
    assert!(
        pcm_classes
            .iter()
            .all(|class| class["class"] != "input*0.25"),
        "normalized host value was treated as plain plugin gain: {pcm_classes:?}"
    );
    let unity_index = pcm_classes
        .iter()
        .position(|class| class["class"] == "input*1.0");
    let half_index = pcm_classes
        .iter()
        .position(|class| class["class"] == "input*0.5");
    let (Some(unity_index), Some(half_index)) = (unity_index, half_index) else {
        panic!("PCM oracle did not observe unity and half-gain blocks: {pcm_classes:?}");
    };
    assert!(
        unity_index < half_index,
        "expected input*1.0 before input*0.5, got {pcm_classes:?}"
    );
    (unity_index, half_index)
}

fn exercise_control_pcm(
    engine: &synvelis_app::SessionRoutingEngine,
    project: &Project,
    target: ParameterTargetId,
    write_revision: ProjectRevision,
    activation_report: &synvelis_player::driver::PlayReport,
) -> ControlEvidence {
    let _ = engine.control_drain_observations();
    let outcome = engine
        .control_fast_control(
            project,
            FastControlWrite {
                target,
                value: FAST_VALUE,
                means: ParamWriteMeans::TimedEvent,
                position: MediaTime::new(FAST_FRAME, frame_base()),
                project_revision: write_revision,
                transport_generation: None,
            },
        )
        .expect("attached control surface");
    assert!(
        matches!(outcome, FastControlOutcome::Delivered),
        "fast control was not delivered on the active generation: {outcome:?}"
    );
    let mut pcm_classes = Vec::new();
    classify_blocks(
        0,
        &activation_report.pcm[0],
        &activation_report.pcm[1],
        &mut pcm_classes,
    );
    let fast_report = run_paced(engine, 16);
    classify_blocks(
        2,
        &fast_report.pcm[0],
        &fast_report.pcm[1],
        &mut pcm_classes,
    );

    let fast_cycles = engine.control_drain_observations();
    let transport_anchor = fast_cycles
        .iter()
        .flat_map(|cycle| &cycle.receipts)
        .filter_map(|receipt| receipt.applied_frame)
        .map(EngineFrame::frame)
        .min()
        .expect("control receipts must expose the transport anchor frame");
    assert_eq!(transport_anchor, activation_report.frames);
    let (fast_call_start, fast_frame_offset, fast_position) =
        find_dispatched(&fast_cycles, target, FAST_VALUE);
    assert_eq!(fast_position, FAST_FRAME);
    assert_eq!(fast_frame_offset, block_offset(FAST_FRAME));
    assert_eq!(
        fast_call_start + u64::from(fast_frame_offset),
        u64::try_from(FAST_FRAME).unwrap() + transport_anchor,
        "control ledger frame projection does not reconstruct the transport-anchored write frame"
    );
    assert!(
        fast_frame_offset > 0,
        "write must carry an explicit in-block frame offset"
    );

    let automation_tick = engine
        .control_tick(
            project,
            BeatPoint::new(1, 1).unwrap(),
            MediaTime::new(AUTOMATION_FRAME, frame_base()),
        )
        .expect("attached control surface");
    assert!(automation_tick.attached);
    assert!(!automation_tick.gated);
    assert!(!automation_tick.stale);
    assert_eq!(automation_tick.evaluated, 1);
    assert_eq!(automation_tick.events, 1);
    assert_eq!(automation_tick.dispatch_errors, 0);
    let automation_report = run_paced(engine, 16);
    classify_blocks(
        18,
        &automation_report.pcm[0],
        &automation_report.pcm[1],
        &mut pcm_classes,
    );
    let automation_cycles = engine.control_drain_observations();
    let (automation_call_start, automation_frame_offset, automation_position) =
        find_dispatched(&automation_cycles, target, AUTOMATION_VALUE);
    assert_eq!(automation_position, AUTOMATION_FRAME);
    assert_eq!(automation_frame_offset, block_offset(AUTOMATION_FRAME));
    assert_eq!(
        automation_call_start + u64::from(automation_frame_offset),
        u64::try_from(AUTOMATION_FRAME).unwrap() + transport_anchor,
        "automation frame projection does not reconstruct the transport-anchored write frame"
    );

    let (unity_index, half_index) = assert_pcm_oracle(&pcm_classes);

    ControlEvidence {
        automation_tick,
        fast_call_start,
        fast_frame_offset,
        automation_call_start,
        automation_frame_offset,
        pcm_classes,
        unity_index,
        half_index,
    }
}

fn exercise_generation(
    app: &mut SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor: ProcessorId,
    lane: LaneId,
    source: SourceId,
    initial: &InitialBindingEvidence,
) -> GenerationEvidence {
    let revision = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::AddClip {
                lane_id: lane,
                source_id: source,
                placement: media_placement(FIRST_CLIP_END, SOURCE_FRAMES as u64),
                playback: natural_playback(
                    0,
                    i64::try_from(SOURCE_FRAMES as u64 - FIRST_CLIP_END).unwrap(),
                ),
            },
            revision,
        )
        .unwrap();
    assert_eq!(engine.control_generation(), Some(initial.generation + 1));
    let (replacement, _) = activate_generation(engine, "replacement generation did not activate");
    assert_ne!(initial.generation, replacement);
    let initial_handles_after_replacement = engine.plugin_state_handles(initial.generation).len();
    let replacement_handles = engine.plugin_state_handles(replacement).len();
    assert_eq!(initial_handles_after_replacement, 0);
    assert!(
        engine
            .plugin_state_handles(replacement)
            .iter()
            .any(|endpoint| endpoint.processor == processor)
    );
    assert!(
        engine
            .control_observe_cycle(
                processor,
                initial.generation,
                &span(4_096),
                EngineFrame::new(ClockEpochId::new(0), 0),
                256
            )
            .is_none(),
        "stale-generation observe must fail closed"
    );
    let project2 = app.workspace().session().project();
    let capability_generation2 = port_capability_generation(project2);
    let binding2 = engine
        .with_control_dispatch(|dispatch| dispatch.binding(initial.binding.synvelis_target))
        .flatten()
        .expect("replacement target binding");
    assert_eq!(binding2.graph_generation, replacement);
    assert_eq!(binding2.capability_generation, capability_generation2);
    let source_map2 = engine.source_map().expect("replacement routing source map");
    assert_eq!(source_map2.generation, replacement);
    let node_resolution2 = source_map2
        .semantic_nodes
        .get(&SemanticNodeRef::Processor(processor))
        .expect("replacement processor source-map entry");
    assert!(
        matches!(node_resolution2, MappingResolution::Resolved(node) if *node == binding2.node),
        "replacement binding node does not match the replacement generation source map entry"
    );
    assert_eq!(
        binding2.attach_clock_context.frame_base(),
        initial.control_frame_base
    );
    assert_eq!(binding2.instance_id, initial.binding.instance_id);

    GenerationEvidence {
        replacement,
        initial_handles_after_replacement,
        replacement_handles,
    }
}

fn artifact_json(
    probe: &FixtureProbe,
    artifacts: &ArtifactEvidence,
    runner_env: &str,
) -> serde_json::Value {
    json!({
        "fixture": {
            "path": &probe.path,
            "size": artifacts.fixture_size,
            "sha256": &probe.sha256,
        },
        "staged_fixture": {
            "path": &artifacts.staged_fixture,
            "size": artifacts.staged_size,
            "sha256": &artifacts.staged_sha256,
        },
        "runner": {
            "path": &artifacts.runner_path,
            "size": artifacts.runner_size,
            "sha256": &artifacts.runner_sha256,
            "resolution_env": runner_env,
        },
        "ruviform_resolved_revision": ruviform_revision(),
    })
}

fn binding_json(
    initial: &InitialBindingEvidence,
    processor: ProcessorId,
    target: ParameterTargetId,
) -> serde_json::Value {
    json!({
        "target": format!("{target:?}"),
        "processor": format!("{processor:?}"),
        "plugin_param": 0,
        "stable_plugin_instance_id": initial.binding.instance_id.get(),
        "persisted_plugin_instance_id": initial.persisted_instance.get(),
        "graph_generation": initial.binding.graph_generation,
        "capability_generation": initial.capability_generation,
        "frame_base": "48000/1",
        "verified_frame_base_foreign_base_rejected": true,
        "report": {
            "bound_targets": initial.report.bound_targets,
            "unbound": initial.report.unbound.len(),
            "without_control": initial.report.without_control.len(),
        },
    })
}

fn control_json(control: &ControlEvidence) -> serde_json::Value {
    json!({
        "fast": {
            "value": FAST_VALUE,
            "position_frame": FAST_FRAME,
            "outcome": "Delivered",
            "call_start": control.fast_call_start,
            "frame_offset": control.fast_frame_offset,
            "stage": "Dispatched",
        },
        "automation": {
            "value": AUTOMATION_VALUE,
            "position_frame": AUTOMATION_FRAME,
            "evaluated": control.automation_tick.evaluated,
            "events": control.automation_tick.events,
            "call_start": control.automation_call_start,
            "frame_offset": control.automation_frame_offset,
            "stage": "Dispatched",
        },
        "sample_exact_pcm_frame_offset": "not claimed — frame precision evidence is control-ledger projection; PCM evidence is value/law evidence inside L1 latency",
    })
}

fn pcm_json(control: &ControlEvidence) -> serde_json::Value {
    json!({
        "input": { "left": INPUT, "right": -INPUT },
        "normalized_write": FAST_VALUE,
        "expected_plain_gain": 0.5,
        "wrong_plain_gain_if_normalized_confused": 0.25,
        "unity_block_index": control.unity_index,
        "half_block_index": control.half_index,
        "wrong_quarter_blocks": 0,
        "block_classes": &control.pcm_classes,
        "lookahead_blocks": 2,
        "latency_bound_frames": 1024,
        "pass_through_caveat": "L1 may emit silence or repeat the last completed endpoint output while lookahead/completion is unavailable; this evidence does not label substituted blocks as fresh plugin delivery",
    })
}

fn generation_json(
    initial: &InitialBindingEvidence,
    generation: &GenerationEvidence,
) -> serde_json::Value {
    json!({
        "initial": initial.generation,
        "replacement": generation.replacement,
        "initial_handles_after_replacement": generation.initial_handles_after_replacement,
        "replacement_handles": generation.replacement_handles,
        "stale_generation_observe": "fail-closed None",
        "wrong_generation_delivery": "no stale-generation write path exists through the public surface; stale observation is refused and the retired generation exposes no plugin state handles",
    })
}

struct EvidenceInput<'a> {
    probe: &'a FixtureProbe,
    artifacts: &'a ArtifactEvidence,
    runner_env: &'a str,
    initial: &'a InitialBindingEvidence,
    control: &'a ControlEvidence,
    generation: &'a GenerationEvidence,
    processor: ProcessorId,
    target: ParameterTargetId,
}

fn print_evidence(input: &EvidenceInput<'_>) {
    println!(
        "pc-c4-l1-real-abi-binding {}",
        json!({
            "schema": "pc-c4-l1-real-abi-binding/v1",
            "format": input.probe.format,
            "production_wiring": {
                "seam": "compose_routing_engine_with_out_of_process",
                "route": "L1 OutOfProcessSpec",
                "activation": "Active",
                "descriptor": input.probe.gain.id,
                "descriptor_count": input.probe.descriptors.len(),
            },
            "artifacts": artifact_json(input.probe, input.artifacts, input.runner_env),
            "binding": binding_json(input.initial, input.processor, input.target),
            "control_oracle": control_json(input.control),
            "pcm_oracle": pcm_json(input.control),
            "generation": generation_json(input.initial, input.generation),
            "commands": [
                "cargo build --locked -p synvelis-plugin-test-l1-runner",
                "cargo test --locked -p synvelis-app --test pc_c4_l1_real_abi_binding -- --ignored --test-threads=1 --nocapture"
            ],
        })
    );
}

fn exercise_format(probe: &FixtureProbe, runner_env: &str, runner_stem: &str) {
    assert!(probe.path.is_file());
    assert_eq!(probe.sha256.len(), 64);
    let runner = runner_program(runner_env, runner_stem);
    let runner_path = PathBuf::from(&runner);
    let staged_fixture = stage_fixture_sibling(probe, &runner_path);
    let (runner_size, runner_sha256) = file_facts(&runner_path);
    let (staged_size, staged_sha256) = file_facts(&staged_fixture);
    assert_eq!(staged_sha256, probe.sha256);
    let artifacts = ArtifactEvidence {
        runner_path,
        staged_fixture,
        fixture_size: fs::metadata(&probe.path).unwrap().len(),
        runner_size,
        runner_sha256,
        staged_size,
        staged_sha256,
    };

    let root = scratch(probe.format);
    fs::create_dir_all(root.join("media")).unwrap();
    let mut app = SynvelisApp::new(BackendReport::available(
        "gpui",
        "pc-c4-l1-binding".to_owned(),
    ));
    let (processor, lane, target, source) = build_project(&mut app, &root, probe.gain.id);
    let engine = compose_routing_engine_with_out_of_process(
        &mut app,
        session_config(root.join("media")),
        Vec::new(),
        vec![ProductOutOfProcessPlugin {
            spec: OutOfProcessSpec::new(probe.gain, runner.clone()),
            params: vec![gain_param()],
        }],
    )
    .expect("production L1 route registration");

    let revision = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "pc-c4-l1-binding".into(),
            },
            revision,
        )
        .unwrap();

    let initial = {
        let project = app.workspace().session().project();
        verify_initial_binding(&engine, project, processor, target, probe)
    };
    let write_revision = app.workspace().session().revision();
    let control = {
        let project = app.workspace().session().project();
        exercise_control_pcm(
            &engine,
            project,
            target,
            write_revision,
            &initial.activation_report,
        )
    };
    let generation = exercise_generation(&mut app, &engine, processor, lane, source, &initial);
    print_evidence(&EvidenceInput {
        probe,
        artifacts: &artifacts,
        runner_env,
        initial: &initial,
        control: &control,
        generation: &generation,
        processor,
        target,
    });
}

#[test]
#[ignore = "artifact-dependent L1 lane: requires prebuilt test-only runner (run `mise run test-pc-c4-l1`)"]
fn pc_c4_l1_vst3_real_abi_binding() {
    let (probe, _library) = load_vst3();
    exercise_format(&probe, VST3_RUNNER_ENV, VST3_RUNNER_STEM);
}

#[test]
#[ignore = "artifact-dependent L1 lane: requires prebuilt test-only runner (run `mise run test-pc-c4-l1`)"]
fn pc_c4_l1_clap_real_abi_binding() {
    let (probe, _library) = load_clap();
    exercise_format(&probe, CLAP_RUNNER_ENV, CLAP_RUNNER_STEM);
}
