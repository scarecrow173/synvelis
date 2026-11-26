//! TIM-03 L1 out-of-process production wiring and limited process oracle
//! (synvelis#127, slice `tim03-l1-oos-production-wiring-s1`).
//!
//! Two boundaries are deliberately separate:
//!
//! - the existing in-process L0 real-ABI negative boundary is owned by
//!   `tim03_real_abi_process_oracle.rs` and is not reused as L1 evidence;
//! - this file registers the ruviform#630 `test_gain` fixtures through the
//!   production composition seam as typed `OutOfProcessSpec` routes, then
//!   independently exercises an L1 endpoint with deterministic PCM.
//!
//! The oracle is intentionally limited. L1 `BusReport::Unavailable` is
//! recorded as unverified, lookahead/pass-through is not precision-exact
//! evidence, and no tempo/loop/automation/sample-accuracy matrix is claimed.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use ruviform_audio_core::{PlanarBufferView, PlanarBufferViewMut, SampleRate};
use ruviform_plugin_clap::host::ClapLibrary;
use ruviform_plugin_core::{
    Event, EventKind, EventList, HostInput, HostOutput, ParamEvent, ParamFlags, ParamId, ParamInfo,
    ParamValue, PluginDescriptor, ProcessConfig, ProcessResult,
};
use ruviform_plugin_dock_test::artifact::resolve_artifact_named;
use ruviform_plugin_dock_test::sha256_hex;
use ruviform_plugin_host::{
    BusReport, EndpointBounds, HostCommand, HostError, HostReply, OutOfProcessSpec, PluginEndpoint,
    PluginHandle, PluginHost,
};
use ruviform_plugin_vst3::host::Vst3Library;
use ruviform_ui::BackendReport;
use serde_json::json;
use synvelis_app::{
    ProductOutOfProcessPlugin, SessionEngineConfig, SynvelisApp,
    compose_routing_engine_with_out_of_process,
};
use synvelis_player::driver::PumpPolicy;
use synvelis_project::{
    BuildBudget, BuildMode, PluginRef, ProcessorBinding, ProjectController, ProjectOp,
    ProjectRevision, RoutingActivationState,
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
const FRAMES: usize = 512;
const FRAMES_U32: u32 = 512;
const EXPECTED_GAIN: f32 = 0.5;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct FixtureProbe {
    format: &'static str,
    path: PathBuf,
    sha256: String,
    descriptors: Vec<PluginDescriptor>,
    gain: PluginDescriptor,
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
        "L1 runner missing at {} — run `mise run test-tim03-l1` (or build synvelis-plugin-test-l1-runner)",
        path.display()
    );
    path.into_os_string()
}

fn route_spec(probe: &FixtureProbe, runner: OsString) -> OutOfProcessSpec {
    OutOfProcessSpec::new(probe.gain, runner)
}

/// Stages the resolved fixture under the runner's own directory using
/// the conventional un-hashed cdylib name. `ruviform-plugin-export`
/// owns the child argv contract and accepts no arguments, so this
/// sibling artifact is the reproducible provisioning channel for the
/// test-only runner. A mismatching pre-existing artifact is evidence of
/// a stale target directory and fails loudly instead of being hidden.
fn stage_fixture_sibling(probe: &FixtureProbe, runner: &Path) {
    let dir = runner.parent().expect("runner parent directory");
    let staged = dir.join(lib_name(match probe.format {
        "vst3" => VST3_STEM,
        "clap" => CLAP_STEM,
        other => panic!("unknown fixture format {other}"),
    }));
    if staged == probe.path {
        return;
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
        return;
    }
    fs::copy(&probe.path, &staged).unwrap_or_else(|error| {
        panic!(
            "failed to stage fixture {} as {}: {error}",
            probe.path.display(),
            staged.display()
        )
    });
}

fn process_config() -> ProcessConfig {
    ProcessConfig::new(SampleRate::new(48_000).unwrap(), 1, FRAMES_U32).unwrap()
}

fn execute_ok(host: &mut PluginHost, command: HostCommand, step: &str) {
    match host.execute(command) {
        Ok(HostReply::Ok) => {}
        outcome => panic!("{step} did not complete: {outcome:?}"),
    }
}

fn instantiate(host: &mut PluginHost, plugin_id: &str) -> PluginHandle {
    match host.execute(HostCommand::Instantiate {
        plugin_id: plugin_id.to_owned(),
    }) {
        Ok(HostReply::Instantiated { handle }) => handle,
        outcome => panic!("instantiate failed for {plugin_id}: {outcome:?}"),
    }
}

fn drive_to_processing(host: &mut PluginHost, handle: PluginHandle) {
    execute_ok(host, HostCommand::Initialize { handle }, "initialize");
    execute_ok(
        host,
        HostCommand::Activate {
            handle,
            config: process_config(),
        },
        "activate",
    );
    execute_ok(
        host,
        HostCommand::StartProcessing { handle },
        "start processing",
    );
}

fn sample_eq(actual: f32, expected: f32) -> bool {
    actual.to_bits() == expected.to_bits()
}

fn defined_output(sample: f32, sign: f32, inputs: &[f32]) -> bool {
    inputs
        .iter()
        .any(|&value| sample_eq(sample, EXPECTED_GAIN * sign * value))
}

#[derive(Default)]
struct ProcessObservations {
    delivered: u32,
    substituted: u32,
    dropped: u64,
    matched_examples: Vec<f32>,
}

fn endpoint_block(
    host: &PluginHost,
    handle: PluginHandle,
    endpoint: &mut PluginEndpoint,
    input: &[&[f32]; 2],
    out_left: &mut [f32],
    out_right: &mut [f32],
    events: EventList<'_>,
) -> (Result<ProcessResult, HostError>, u64, u64) {
    let counters = |host: &PluginHost| {
        host.audio_counters(handle)
            .map_or((0, 0), |c| (c.passthrough_submits, c.dropped_submits))
    };
    let before = counters(host);
    let input_view = PlanarBufferView::new(&input[..]).unwrap();
    let inputs = [HostInput(&input_view)];
    let mut out_list = [&mut *out_left, &mut *out_right];
    let mut output_view = PlanarBufferViewMut::new(&mut out_list).unwrap();
    let mut outputs = [HostOutput(&mut output_view)];
    let result = endpoint.process(&inputs, &mut outputs, events, None);
    let after = counters(host);
    (result, after.0 - before.0, after.1 - before.1)
}

fn observe_endpoint_process(
    host: &PluginHost,
    handle: PluginHandle,
    endpoint: &mut PluginEndpoint,
) -> ProcessObservations {
    let events = [Event::new(
        0,
        EventKind::ParamValue(ParamEvent {
            param_id: ParamId(0),
            value: ParamValue::new(0.25).unwrap(),
        }),
    )];
    let event_list = EventList::new(&events, FRAMES_U32).unwrap();
    let mut inputs = Vec::new();
    let mut observations = ProcessObservations::default();
    for call in 0..24_u32 {
        let value = 0.2 + 0.01 * f32::from(u8::try_from(call % 16).unwrap() + 1);
        let left = vec![value; FRAMES];
        let right = vec![-value; FRAMES];
        let input = [&left[..], &right[..]];
        let mut out_left = vec![0.0_f32; FRAMES];
        let mut out_right = vec![0.0_f32; FRAMES];
        thread::sleep(Duration::from_millis(5));
        let (result, passthrough, dropped) = endpoint_block(
            host,
            handle,
            endpoint,
            &input,
            &mut out_left,
            &mut out_right,
            event_list,
        );
        assert_eq!(result, Ok(ProcessResult::CONTINUE), "call {call}");
        if call < 2 {
            assert!(out_left.iter().all(|&sample| sample_eq(sample, 0.0)));
            assert!(out_right.iter().all(|&sample| sample_eq(sample, 0.0)));
        } else if passthrough > 0 {
            observations.substituted += 1;
            observations.dropped += dropped;
        } else {
            assert!(
                out_left
                    .iter()
                    .all(|&sample| defined_output(sample, 1.0, &inputs)),
                "call {call}: left channel outside defined-output membership"
            );
            assert!(
                out_right
                    .iter()
                    .all(|&sample| defined_output(sample, -1.0, &inputs)),
                "call {call}: right channel outside defined-output membership"
            );
            observations.delivered += 1;
            if observations.matched_examples.len() < 4 {
                observations.matched_examples.push(out_left[0]);
            }
        }
        inputs.push(value);
    }
    assert!(
        observations.delivered >= 6,
        "L1 endpoint barely delivered: {} matched, {} substituted",
        observations.delivered,
        observations.substituted
    );
    observations
}

fn endpoint_process_oracle(probe: &FixtureProbe, spec: OutOfProcessSpec) -> serde_json::Value {
    let mut host = PluginHost::new();
    host.register_out_of_process(spec)
        .expect("register L1 route");
    let scanned = match host.execute(HostCommand::Scan).expect("scan") {
        HostReply::Scanned(descriptors) => descriptors,
        reply => panic!("scan returned unexpected reply: {reply:?}"),
    };
    assert!(
        scanned
            .iter()
            .any(|descriptor| descriptor.id == probe.gain.id)
    );
    let handle = instantiate(&mut host, probe.gain.id);
    drive_to_processing(&mut host, handle);
    let bus_report = host.buses(handle).expect("L1 bus report");
    assert!(
        matches!(bus_report, BusReport::Unavailable),
        "L1 bus enumeration must stay unverified, got {bus_report:?}"
    );
    assert_eq!(host.latency_frames(handle), Some(1024));
    let mut endpoint = host
        .prepare_endpoint(
            handle,
            EndpointBounds {
                max_frames: FRAMES_U32,
                channels: 2,
            },
        )
        .expect("L1 endpoint attach must succeed");
    assert_eq!(endpoint.latency_frames(), 1024);

    let observations = observe_endpoint_process(&host, handle, &mut endpoint);

    // Endpoint detach is explicit before control-plane teardown. The next
    // host entry points can then complete the ordinary lifecycle teardown.
    drop(endpoint);
    execute_ok(
        &mut host,
        HostCommand::StopProcessing { handle },
        "stop processing",
    );
    execute_ok(&mut host, HostCommand::Deactivate { handle }, "deactivate");
    execute_ok(&mut host, HostCommand::Destroy { handle }, "destroy");
    assert_eq!(host.lifecycle_state(handle), None);

    json!({
        "scan": "pass",
        "lifecycle_to_processing": "pass",
        "bus_report": "Unavailable/unverified",
        "endpoint_attach": "pass",
        "endpoint_detach": "pass",
        "latency_frames": 1024,
        "lookahead_blocks": 2,
        "precision_claim": "not exact — lookahead/substitution-capable L1 pipeline",
        "defined_output_membership": "pass",
        "delivered_matched_blocks": observations.delivered,
        "substituted_blocks": observations.substituted,
        "dropped_submit_deltas": observations.dropped,
        "matched_examples": observations.matched_examples,
    })
}

fn scratch(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "synvelis-tim03-l1-{}-{}-{}",
        name,
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn apply(controller: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    controller
        .apply(synvelis_project::Command {
            expected_revision: controller.live_revision(),
            op,
        })
        .unwrap()
}

fn session_config(media_root: PathBuf) -> SessionEngineConfig {
    SessionEngineConfig {
        media_root,
        engine: synvelis_player::lowering::EngineConfig {
            sample_rate: 48_000,
            channels: 2,
            block_frames: 256,
        },
        prefetch: ruviform_media_reader::PrefetchConfig {
            block_frames: 256,
            block_count: 8,
            channels: 2,
            sample_rate: 48_000,
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

/// Builds the single-track/single-processor production project and returns
/// the inserted processor id.
fn build_project_with_processor(
    app: &mut SynvelisApp,
    descriptor: &str,
) -> synvelis_project::ProcessorId {
    let controller = app.workspace_mut().session_mut().controller_mut();
    apply(
        controller,
        ProjectOp::AddTrack {
            name: "track".into(),
        },
    );
    let track = *controller.live().tracks.keys().next_back().unwrap();
    apply(controller, ProjectOp::AddTrackStrip { track_id: track });
    let strip = controller.live().routing.strip_of_track(track).unwrap().id;
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

fn active_generation(engine: &synvelis_app::SessionRoutingEngine, context: &str) -> u64 {
    engine.run(2, PumpPolicy::Every(1)).unwrap();
    match engine.receipt().expect("routing activation receipt").state {
        RoutingActivationState::Active { generation, .. } => generation,
        other => panic!("{context}: {other:?}"),
    }
}

/// Retires the processor generation through the same production
/// session/engine path. The RT-observed generation switch is the
/// engine's defined point at which the old generation's retained
/// connection vector is dropped; dropping each lease owns the
/// StopProcessing/Deactivate/Destroy lifecycle teardown. Endpoint
/// object detach itself is ownership-based (`Drop`) upstream and
/// exposes no detach receipt, so this oracle independently proves
/// generation retirement rather than re-attachability of the same
/// instance.
fn retire_generation(
    app: &mut SynvelisApp,
    engine: &synvelis_app::SessionRoutingEngine,
    processor_id: synvelis_project::ProcessorId,
    generation: u64,
) -> u64 {
    let revision = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(ProjectOp::RemoveProcessor { processor_id }, revision)
        .unwrap();
    let retired_generation = active_generation(engine, "production retirement did not activate");
    assert_ne!(
        generation, retired_generation,
        "processor removal did not publish a replacement generation"
    );
    assert!(
        engine.plugin_state_handles(generation).is_empty(),
        "retired generation {generation} still exposes plugin state handles"
    );
    retired_generation
}

fn production_activation(probe: &FixtureProbe, spec: OutOfProcessSpec) -> serde_json::Value {
    let root = scratch("production");
    fs::create_dir_all(root.join("media")).unwrap();
    let mut app = SynvelisApp::new(BackendReport::available(
        "gpui",
        "tim03-l1-production".to_owned(),
    ));
    let processor_id = build_project_with_processor(&mut app, probe.gain.id);
    let engine = compose_routing_engine_with_out_of_process(
        &mut app,
        session_config(root.join("media")),
        Vec::new(),
        vec![ProductOutOfProcessPlugin {
            spec,
            params: vec![gain_param()],
        }],
    )
    .expect("production L1 route registration");
    let revision = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "tim03-l1-production".into(),
            },
            revision,
        )
        .unwrap();
    let generation = active_generation(&engine, "production composition did not activate");
    let state_handles = engine.plugin_state_handles(generation);
    assert!(
        state_handles
            .iter()
            .any(|endpoint| endpoint.processor == processor_id),
        "production activation exposed no state handle for {processor_id:?}"
    );
    let retired_generation = retire_generation(&mut app, &engine, processor_id, generation);
    json!({
        "composition_seam": "compose_routing_engine_with_out_of_process",
        "activation": "Active",
        "generation": generation,
        "replacement_generation": retired_generation,
        "processor": format!("{processor_id:?}"),
        "state_handle_present": true,
        "lifecycle_attach": "pass — Scan/Instantiate/Initialize/Activate/StartProcessing/attach reached through PluginHostProcessors",
        "generation_retirement": "pass — RemoveProcessor rebuild activated a replacement generation and retired-generation plugin_state_handles is empty",
        "endpoint_detach": "limited — production endpoint detach is ownership Drop during generation/application teardown; no upstream detach receipt or same-instance re-attach oracle exists after teardown",
    })
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

fn exercise_format(probe: &FixtureProbe, runner_env: &str, runner_stem: &str) {
    assert!(probe.path.is_file());
    assert_eq!(probe.sha256.len(), 64);
    let runner = runner_program(runner_env, runner_stem);
    let runner_path = PathBuf::from(&runner);
    stage_fixture_sibling(probe, &runner_path);
    let endpoint_spec = route_spec(probe, runner.clone());
    let production_spec = route_spec(probe, runner.clone());
    let endpoint_evidence = endpoint_process_oracle(probe, endpoint_spec);
    let production_evidence = production_activation(probe, production_spec);
    println!(
        "tim03-l1-process-oracle {}",
        json!({
            "format": probe.format,
            "artifact": probe.path,
            "sha256": probe.sha256,
            "descriptor_count": probe.descriptors.len(),
            "test_gain_descriptor_id": probe.gain.id,
            "runner": PathBuf::from(runner),
            "runner_resolution": runner_env,
            "host_route": "L1 OutOfProcessSpec",
            "production": production_evidence,
            "endpoint_oracle": endpoint_evidence,
            "tempo_loop_automation_sample_accuracy": "NotRun — later TIM-03 slices",
            "crash_hang_fault_isolation": "NotRun — outside this slice",
            "process_oracle_status": "initial re-evaluation complete, limited"
        })
    );
}

#[test]
fn tim03_l1_vst3_production_wiring_and_limited_process_oracle() {
    let (probe, _library) = load_vst3();
    exercise_format(&probe, VST3_RUNNER_ENV, VST3_RUNNER_STEM);
}

#[test]
fn tim03_l1_clap_production_wiring_and_limited_process_oracle() {
    let (probe, _library) = load_clap();
    exercise_format(&probe, CLAP_RUNNER_ENV, CLAP_RUNNER_STEM);
}
