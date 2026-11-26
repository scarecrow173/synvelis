//! TIM-03 real-ABI consumer oracle slice (synvelis#127).
//!
//! This test reaches the real ruviform#630 VST3/CLAP fixtures through the
//! product host/composition path as far as the current architecture allows:
//! artifact resolution, ABI discovery, lifecycle, and the fail-closed endpoint
//! attach boundary. It deliberately does not claim a process oracle pass:
//! in-process real-ABI instances are `!Send`, so the typed-runtime endpoint is
//! unavailable until Synvelis wires the upstream L1 out-of-process host path.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_plugin_clap::host::ClapLibrary;
use ruviform_plugin_core::{
    BusDescriptor, BusDirection, BusRole, ParamFlags, ParamId, ParamInfo, PluginDescriptor,
    PluginFactory, ProcessConfig,
};
use ruviform_plugin_dock_test::artifact::resolve_artifact_named;
use ruviform_plugin_dock_test::sha256_hex;
use ruviform_plugin_host::{
    BusReport, EndpointBounds, HostCommand, HostError, HostReply, PluginHost,
};
use ruviform_plugin_vst3::host::Vst3Library;
use ruviform_ui::BackendReport;
use serde_json::json;
use synvelis_app::{ProductPlugin, SessionEngineConfig, SynvelisApp, compose_routing_engine};
use synvelis_player::driver::PumpPolicy;
use synvelis_player::{
    PluginHostProcessors, ProcessorConnectError, ProcessorConnectRequest, ProcessorHost,
};
use synvelis_project::{
    BuildBudget, BuildMode, PluginRef, PortBinding, PortDirection, PortOwner, PortRole, Processor,
    ProcessorBinding, ProcessorId, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, RoutingActivationState, RoutingBuildKey, RoutingFault, RuntimeClockEpochRef,
    RuntimeInstanceId, SemanticPort, SemanticPortId, SignalDomain, port_capability_generation,
    routing_fingerprint,
};

const VST3_STEM: &str = "ruviform_plugin_test_vst3";
const CLAP_STEM: &str = "ruviform_plugin_test_clap";
const VST3_ENV: &str = "SYNVELIS_TIM03_REAL_ABI_VST3";
const CLAP_ENV: &str = "SYNVELIS_TIM03_REAL_ABI_CLAP";
const TEST_GAIN: &str = "test_gain";
const EXPECTED_VST3_DESCRIPTORS: usize = 7;
const EXPECTED_CLAP_DESCRIPTORS: usize = 6;
// The fixture class id is built from one logical value —
// `vst3::uid(0x6D4F8A21, 0x3B7C9E05, 0xF1A24C68, 0xB9D37E51)` in
// `ruviform-plugin-test-vst3/src/component.rs` — but the `vst3` binding
// crate's `uid()` constructor is platform-dependent *by design* (its own
// doc: "the byte order of the resulting value will differ between Windows
// and other platforms"). On Windows the TUID uses the COM GUID
// mixed-endian memory layout (first u32 little-endian, next two u16
// fields little-endian, tail bytes as-is); everywhere else it uses the
// canonical Steinberg `INLINE_UID` big-endian order. The host scan renders
// the raw bytes as lowercase hex, so the descriptor id string legitimately
// differs per platform for the same logical class. Both layouts are
// asserted here — the oracle pins the platform-correct exact bytes rather
// than weakening the check.
#[cfg(target_os = "windows")]
const EXPECTED_VST3_TEST_GAIN_ID: &str = "218a4f6d7c3b059ef1a24c68b9d37e51";
#[cfg(not(target_os = "windows"))]
const EXPECTED_VST3_TEST_GAIN_ID: &str = "6d4f8a213b7c9e05f1a24c68b9d37e51";
const EXPECTED_CLAP_TEST_GAIN_ID: &str = "org.ruviform-test.clap-test-gain";

const GAIN_PARAM: ParamInfo = ParamInfo {
    id: ParamId(0),
    name: "Gain",
    unit: "",
    min_value: 0.0,
    max_value: 2.0,
    default_value: 1.0,
    flags: ParamFlags::AUTOMATABLE,
};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct ArtifactProbe {
    format: &'static str,
    path: PathBuf,
    sha256: String,
    descriptors: Vec<PluginDescriptor>,
    gain: PluginDescriptor,
    expected_descriptors: usize,
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

fn artifact_probe(
    format: &'static str,
    path: PathBuf,
    descriptors: Vec<PluginDescriptor>,
    expected_descriptors: usize,
) -> ArtifactProbe {
    let sha256 = sha256_hex(&path).expect("resolved artifact must hash");
    let gain = descriptors
        .iter()
        .find(|descriptor| descriptor.name == TEST_GAIN)
        .copied()
        .unwrap_or_else(|| panic!("{format} fixture does not expose {TEST_GAIN}: {descriptors:?}"));
    ArtifactProbe {
        format,
        path,
        sha256,
        descriptors,
        gain,
        expected_descriptors,
    }
}

fn load_vst3() -> (ArtifactProbe, Vst3Library) {
    let explicit = env::var_os(VST3_ENV).map(PathBuf::from);
    let path = resolve_fixture(explicit.as_deref(), VST3_STEM)
        .unwrap_or_else(|reason| panic!("VST3 fixture artifact unresolved: {reason}"));
    let library = Vst3Library::load(&path).expect("resolved VST3 fixture must load");
    let probe = artifact_probe(
        "vst3",
        path,
        library.descriptors().to_vec(),
        EXPECTED_VST3_DESCRIPTORS,
    );
    (probe, library)
}

fn load_clap() -> (ArtifactProbe, ClapLibrary) {
    let explicit = env::var_os(CLAP_ENV).map(PathBuf::from);
    let path = resolve_fixture(explicit.as_deref(), CLAP_STEM)
        .unwrap_or_else(|reason| panic!("CLAP fixture artifact unresolved: {reason}"));
    let library = ClapLibrary::load(&path).expect("resolved CLAP fixture must load");
    let probe = artifact_probe(
        "clap",
        path,
        library.descriptors().to_vec(),
        EXPECTED_CLAP_DESCRIPTORS,
    );
    (probe, library)
}

fn take_test_gain_factory<T: PluginFactory + 'static>(factories: Vec<T>) -> Box<dyn PluginFactory> {
    let factory = factories
        .into_iter()
        .find(|factory| factory.descriptor().name == TEST_GAIN)
        .unwrap_or_else(|| panic!("factory scan does not expose {TEST_GAIN}"));
    Box::new(factory)
}

fn execute_ok(host: &mut PluginHost, command: HostCommand, step: &str) {
    match host.execute(command) {
        Ok(HostReply::Ok) => {}
        outcome => panic!("{step} did not complete: {outcome:?}"),
    }
}

fn teardown(host: &mut PluginHost, handle: ruviform_plugin_core::PluginHandle) {
    let _ = host.execute(HostCommand::StopProcessing { handle });
    let _ = host.execute(HostCommand::Deactivate { handle });
    let _ = host.execute(HostCommand::Destroy { handle });
}

/// Drives the production host lifecycle up to `Processing`, then measures the
/// exact endpoint rejection independently of the adapter's earlier bus-table
/// validation ordering.
fn measure_endpoint_boundary(
    factory: Box<dyn PluginFactory>,
) -> (Vec<PluginDescriptor>, BusReport, HostError, bool) {
    let plugin_id = factory.descriptor().id.to_owned();
    let create_sendable_available = factory.create_sendable().is_some();
    assert!(
        !create_sendable_available,
        "real-ABI factory unexpectedly exposed a Send endpoint instance"
    );
    let mut host = PluginHost::new();
    host.register_factory(factory);
    let scanned = match host.execute(HostCommand::Scan).expect("scan command") {
        HostReply::Scanned(descriptors) => descriptors,
        reply => panic!("scan returned unexpected reply: {reply:?}"),
    };
    let handle = match host.execute(HostCommand::Instantiate {
        plugin_id: plugin_id.clone(),
    }) {
        Ok(HostReply::Instantiated { handle }) => handle,
        outcome => panic!("instantiate failed for {plugin_id}: {outcome:?}"),
    };
    execute_ok(&mut host, HostCommand::Initialize { handle }, "initialize");
    let config = ProcessConfig::new(SampleRate::new(48_000).unwrap(), 1, 256).unwrap();
    execute_ok(
        &mut host,
        HostCommand::Activate { handle, config },
        "activate",
    );
    execute_ok(
        &mut host,
        HostCommand::StartProcessing { handle },
        "start processing",
    );
    let bus_report = host.buses(handle).expect("bus report");
    let Err(outcome) = host.prepare_endpoint(
        handle,
        EndpointBounds {
            max_frames: 256,
            channels: 2,
        },
    ) else {
        panic!("in-process real-ABI endpoint attach unexpectedly succeeded")
    };
    teardown(&mut host, handle);
    (scanned, bus_report, outcome, create_sendable_available)
}

fn fmt() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn processor(processor_id: ProcessorId, descriptor: &str) -> Processor {
    Processor {
        id: processor_id,
        name: TEST_GAIN.into(),
        binding: ProcessorBinding::Resolved(PluginRef {
            descriptor: descriptor.to_owned(),
            version: "0.0.0".into(),
            capability_revision: 0,
        }),
        port_ids: vec![SemanticPortId::new(1), SemanticPortId::new(2)],
    }
}

fn semantic_port(id: SemanticPortId, owner: ProcessorId, direction: PortDirection) -> SemanticPort {
    SemanticPort {
        id,
        owner: PortOwner::Processor(owner),
        direction,
        role: PortRole::Main,
        domain: SignalDomain::Audio,
        binding: PortBinding::Internal,
        channels: 2,
    }
}

fn connect_request(processor_id: ProcessorId, descriptor: &str) -> ProcessorConnectRequest {
    let port_in = SemanticPortId::new(1);
    let port_out = SemanticPortId::new(2);
    // `ProcessorConnectRequest::node` names ruviform-graph's type, which is
    // deliberately not a direct synvelis-app dependency. Mint a real graph ID
    // through the player's public lowering instead of fabricating one; connect
    // fails before the value can be installed into a runtime plan.
    let project = Project::new(ProjectId::new(1), "tim03-node-id-mint");
    let key = RoutingBuildKey {
        project_id: project.project_id,
        project_revision: ProjectRevision(0),
        routing_fingerprint: routing_fingerprint(&project),
        capability_generation: port_capability_generation(&project),
        engine_config_generation: 1,
        source_format_generation: 1,
        lowering_policy_version: synvelis_project::LOWERING_POLICY_VERSION,
        build_mode: BuildMode::Realtime,
        device_clock_epoch: RuntimeClockEpochRef(1),
        runtime_instance: RuntimeInstanceId(1),
    };
    let engine = synvelis_player::lowering::EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 256,
    };
    let lowered = synvelis_player::routing_lower::lower_semantic(
        &project,
        &|_| None,
        &engine,
        &key,
        1,
        &BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
    )
    .expect("empty semantic lowering mints the device node");
    let node = lowered.spec.nodes()[0].id();
    ProcessorConnectRequest {
        processor: processor(processor_id, descriptor),
        ports: vec![
            semantic_port(port_in, processor_id, PortDirection::Input),
            semantic_port(port_out, processor_id, PortDirection::Output),
        ],
        mode: BuildMode::Realtime,
        generation: 1,
        node,
        port_format: fmt(),
        max_frames: 256,
    }
}

fn adapter_connect_error(
    factory: Box<dyn PluginFactory>,
    descriptor: &str,
) -> ProcessorConnectError {
    let mut processors = PluginHostProcessors::product(vec![factory]);
    processors.register_params(descriptor, vec![GAIN_PARAM]);
    match processors.connect(connect_request(ProcessorId::new(1), descriptor)) {
        Ok(_) => panic!("real-ABI in-process connect unexpectedly succeeded"),
        Err(error) => error,
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "synvelis-tim03-real-abi-{}-{}-{}",
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

fn production_failure(factory: Box<dyn PluginFactory>, descriptor: &str) -> RoutingActivationState {
    let root = scratch("production");
    fs::create_dir_all(root.join("media")).unwrap();
    let mut app = SynvelisApp::new(BackendReport::available(
        "gpui",
        "tim03-real-abi".to_owned(),
    ));
    {
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
    }
    let engine = compose_routing_engine(
        &mut app,
        session_config(root.join("media")),
        vec![ProductPlugin {
            factory,
            params: vec![GAIN_PARAM],
        }],
    );
    // The engine is attached only after the track/strip/processor mutations
    // above. Commit one more ordinary project edit through the session so the
    // production EnginePort notification path performs the build under test.
    let revision = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "tim03-real-abi".into(),
            },
            revision,
        )
        .unwrap();
    let _ = engine.run(1, PumpPolicy::Every(1));
    engine.receipt().expect("routing activation receipt").state
}

fn exercise_real_abi_format(
    probe: &ArtifactProbe,
    endpoint_factory: Box<dyn PluginFactory>,
    adapter_factory: Box<dyn PluginFactory>,
    production_factory: Box<dyn PluginFactory>,
) {
    assert_eq!(probe.descriptors.len(), probe.expected_descriptors);
    assert_eq!(
        probe.gain.id,
        if probe.format == "vst3" {
            EXPECTED_VST3_TEST_GAIN_ID
        } else {
            EXPECTED_CLAP_TEST_GAIN_ID
        }
    );
    assert_eq!(probe.sha256.len(), 64);
    assert!(probe.path.is_file());

    let (scanned, bus_report, endpoint_error, create_sendable_available) =
        measure_endpoint_boundary(endpoint_factory);
    assert!(
        scanned
            .iter()
            .any(|descriptor| descriptor.id == probe.gain.id)
    );
    assert_eq!(
        bus_report,
        BusReport::Reported(vec![
            BusDescriptor {
                direction: BusDirection::Input,
                layout: ChannelLayout::Stereo,
                name: "",
                role: BusRole::Main,
                enabled: true,
            },
            BusDescriptor {
                direction: BusDirection::Output,
                layout: ChannelLayout::Stereo,
                name: "",
                role: BusRole::Main,
                enabled: true,
            },
        ])
    );
    assert_eq!(endpoint_error, HostError::EndpointUnavailable);

    // The in-process adapters at the pinned ruviform revision enumerate the
    // same stereo main input/output table the semantic processor declares.
    // Bus validation therefore reaches endpoint preparation, whose defined
    // refusal is the architectural boundary: real-ABI in-process instances do
    // not expose a Send endpoint. The direct host measurement above is the
    // independent evidence for that endpoint refusal.
    let adapter_error = adapter_connect_error(adapter_factory, probe.gain.id);
    assert_eq!(adapter_error, ProcessorConnectError::Unavailable);

    let production_state = production_failure(production_factory, probe.gain.id);
    let production_fault = match &production_state {
        RoutingActivationState::Failed { fault, .. }
        | RoutingActivationState::Blocked { fault, .. } => *fault,
        state => panic!("production composition unexpectedly reached {state:?}"),
    };
    // `PluginHostProcessors` maps the endpoint refusal (`Unavailable`) to the
    // routing fault for plugin unavailability. This is typed fail-closed
    // evidence; it must not be rewritten into a format/time-map claim the
    // current production path did not make.
    assert_eq!(production_fault, RoutingFault::PluginUnavailable);

    println!(
        "tim03-real-abi-oracle {}",
        json!({
            "format": probe.format,
            "artifact": probe.path,
            "sha256": probe.sha256,
            "descriptor_count": probe.descriptors.len(),
            "descriptors": probe
                .descriptors
                .iter()
                .map(|descriptor| json!({
                    "id": descriptor.id,
                    "name": descriptor.name,
                    "vendor": descriptor.vendor,
                    "version": descriptor.version,
                }))
                .collect::<Vec<_>>(),
            "test_gain_descriptor_id": probe.gain.id,
            "factory_class_id": probe.gain.id,
            "factory_class_id_evidence": if probe.format == "vst3" {
                "VST3 descriptor id is the raw component TUID as lowercase hex; the TUID byte order is platform-dependent by vst3 crate design (COM GUID mixed-endian on Windows, INLINE_UID big-endian elsewhere)"
            } else {
                "CLAP descriptor id is the plugin factory instantiation id"
            },
            "scan": "pass",
            "lifecycle_to_processing": "pass",
            "reported_buses": format!("{bus_report:?}"),
            "create_sendable_available": create_sendable_available,
            "endpoint_prepare": format!("{endpoint_error:?}"),
            "adapter_connect": format!("{adapter_error:?}"),
            "production_activation_fault": format!("{production_fault:?}"),
            "process_oracle": "NotRun — in-process endpoint unavailable; re-evaluate after L1 out-of-process hosting is wired"
        })
    );
}

#[test]
fn tim03_real_abi_vst3_discovery_lifecycle_and_fail_closed_boundary() {
    let (probe, library) = load_vst3();
    exercise_real_abi_format(
        &probe,
        take_test_gain_factory(library.factories()),
        take_test_gain_factory(library.factories()),
        take_test_gain_factory(library.factories()),
    );
}

#[test]
fn tim03_real_abi_clap_discovery_lifecycle_and_fail_closed_boundary() {
    let (probe, library) = load_clap();
    exercise_real_abi_format(
        &probe,
        take_test_gain_factory(library.factories()),
        take_test_gain_factory(library.factories()),
        take_test_gain_factory(library.factories()),
    );
}
