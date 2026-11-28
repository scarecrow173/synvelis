//! TIM-04 (issue #128): real-ABI consumer-run evidence for plugin
//! state snapshots.
//!
//! These tests drive the pinned ruviform VST3/CLAP `test_stateful`
//! and `test_fault` cdylib fixtures through the production
//! `PluginHost` cell and `PluginStateHost` adapter. State mutation
//! uses the host's real `process` path and parameter events; the RFST
//! blob parser below is an independent oracle — no fixture or adapter
//! implementation is reused for verification.
//!
//! Evidence rows are written only when `TIM04_REAL_ABI_EVIDENCE_DIR`
//! points at a directory. Every row is one JSON object carrying an
//! explicit `Pass`/`Fail`/`NotRun`/`Unsupported` verdict plus the
//! environment metadata needed to interpret it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ruviform_audio_core::SampleRate;
use ruviform_plugin_clap::host::ClapLibrary;
use ruviform_plugin_core::{
    BufferView, BufferViewMut, Event, EventKind, EventList, HostError, HostInput, HostOutput,
    ParamEvent, ParamId, ParamValue, PluginDescriptor, PluginFactory, ProcessConfig,
};
use ruviform_plugin_host::{EndpointBounds, HostCommand, HostReply, PluginHandle, PluginHost};
use ruviform_plugin_vst3::host::Vst3Library;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use synvelis_player::host::PluginHostProcessors;
use synvelis_player::plugin_state::PluginStateHost;
use synvelis_project::{
    CaptureMode, Command, CounterIdGenerator, DirectoryPluginStateStore, PluginRef,
    PluginStateCaptureError, PluginStateOutcome, PluginStatePort, PluginStateRestoreOutcome,
    PluginStateRestoreRequest, PluginStateSaveRequest, PluginStateSession, ProcessorBinding,
    ProcessorId, Project, ProjectController, ProjectId, ProjectOp, StateCaptureQuality,
    StateQualityPolicy, digest_hex,
};

const FRAMES: usize = 64;
const FRAMES_U32: u32 = 64;
const MUTATED_NORMALIZED: f64 = 0.375;
const MUTATED_PLAIN: f64 = 0.75;
const DEFAULT_PLAIN: f64 = 1.0;
const DEFAULT_NORMALIZED: f64 = 0.5;
const PCM_TOLERANCE: f32 = 1.0e-6;

static STORE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

// ── Evidence ─────────────────────────────────────────────────────

fn evidence_root() -> Option<PathBuf> {
    let path = env::var_os("SYNVELIS_TIM04_REAL_ABI_EVIDENCE_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)?;
    if path.is_absolute() {
        Some(path)
    } else {
        Some(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(path),
        )
    }
}

fn write_case(
    case: &str,
    driver: &str,
    verdict: &str,
    detail: &str,
    observed: Value,
    environment: Value,
) {
    let Some(root) = evidence_root() else {
        return;
    };
    fs::create_dir_all(&root).expect("create TIM-04 evidence directory");
    let mut row = json!({
        "type": "case",
        "scenario": "tim04-real-abi-plugin-state",
        "run_id": "in-process-production-host-v1",
        "case": case,
        "driver": driver,
        "verdict": verdict,
        "detail": detail,
    });
    row["observed"] = observed;
    row["environment"] = environment;
    let text = serde_json::to_string(&row).expect("serialize TIM-04 evidence row");
    fs::write(root.join(format!("{case}.jsonl")), format!("{text}\n"))
        .expect("write TIM-04 evidence row");
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn ruviform_revision() -> Option<String> {
    let lock = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("Cargo.lock");
    let text = fs::read_to_string(lock).ok()?;
    text.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with("source = \"git+https://github.com/scarecrow173/ruviform") {
            line.rsplit_once('#')
                .map(|(_, revision)| revision.trim_end_matches('"').to_owned())
        } else {
            None
        }
    })
}

fn base_environment(
    format: Format,
    artifact: Option<&Artifact>,
    descriptor: Option<PluginDescriptor>,
) -> Value {
    let mut environment = json!({
        "os": env::consts::OS,
        "arch": env::consts::ARCH,
        "environment_kind": "test-plugin",
        "isolation": "in-process/l0",
        "format": format.label(),
        "ruviform_resolved_rev": ruviform_revision(),
    });
    if let Some(artifact) = artifact {
        environment["artifact"] = json!({
            "path": artifact.path,
            "file_name": artifact.path.file_name().map(|name| name.to_string_lossy().into_owned()),
            "sha256": artifact.sha256,
        });
    }
    if let Some(descriptor) = descriptor {
        environment["plugin"] = json!({
            "id": descriptor.id,
            "name": descriptor.name,
            "vendor": descriptor.vendor,
            "version": descriptor.version,
            "features": descriptor.features,
        });
    }
    environment
}

// ── cdylib artifact resolution ───────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Vst3,
    Clap,
}

impl Format {
    const ALL: [Self; 2] = [Self::Vst3, Self::Clap];

    const fn label(self) -> &'static str {
        match self {
            Self::Vst3 => "VST3",
            Self::Clap => "CLAP",
        }
    }

    const fn crate_stem(self) -> &'static str {
        match self {
            Self::Vst3 => "ruviform_plugin_test_vst3",
            Self::Clap => "ruviform_plugin_test_clap",
        }
    }

    const fn crate_dir(self) -> &'static str {
        match self {
            Self::Vst3 => "ruviform-plugin-test-vst3",
            Self::Clap => "ruviform-plugin-test-clap",
        }
    }

    const fn explicit_env(self) -> &'static str {
        match self {
            Self::Vst3 => "SYNVELIS_TIM04_TEST_VST3_PATH",
            Self::Clap => "SYNVELIS_TIM04_TEST_CLAP_PATH",
        }
    }

    const fn upstream_env(self) -> &'static str {
        match self {
            Self::Vst3 => "RUVIFORM_PLUGIN_DOCK_TEST_VST3",
            Self::Clap => "RUVIFORM_PLUGIN_DOCK_TEST_CLAP",
        }
    }
}

#[derive(Clone, Debug)]
struct Artifact {
    path: PathBuf,
    sha256: String,
}

fn hash_file(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    Ok(sha256_hex(&bytes))
}

fn artifact_from_path(path: PathBuf) -> Result<Artifact, String> {
    if !path.is_file() {
        return Err(format!("artifact does not exist: {}", path.display()));
    }
    let sha256 = hash_file(&path)?;
    Ok(Artifact { path, sha256 })
}

fn depinfo_path(path: &Path) -> PathBuf {
    let depinfo = path.with_extension("d");
    let Some(file_name) = depinfo.file_name().and_then(|name| name.to_str()) else {
        return depinfo;
    };
    let Some(unprefixed) = file_name.strip_prefix("lib") else {
        return depinfo;
    };
    depinfo.with_file_name(unprefixed)
}

fn depinfo_proves_locked_ruviform(path: &Path, format: Format, revision: &str) -> bool {
    let Ok(depinfo) = fs::read_to_string(depinfo_path(path)) else {
        return false;
    };
    let normalized = depinfo.replace('\\', "/");
    let source = format!("/crates/{}/src/lib.rs", format.crate_dir());
    let segments: Vec<_> = normalized.split('/').collect();
    let locked_checkout = segments.windows(4).any(|window| {
        window[0] == "checkouts"
            && window[1].starts_with("ruviform-")
            && window[2].len() >= 7
            && revision.starts_with(window[2])
            && window[3] == "crates"
    });
    locked_checkout && normalized.contains(&source)
}

fn resolve_artifact(format: Format) -> Result<Artifact, String> {
    for variable in [format.explicit_env(), format.upstream_env()] {
        if let Some(path) = env::var_os(variable).filter(|value| !value.is_empty()) {
            return artifact_from_path(PathBuf::from(path))
                .map_err(|error| format!("{variable}: {error}"));
        }
    }

    let executable = env::current_exe().map_err(|error| format!("current_exe: {error}"))?;
    let deps_dir = executable
        .parent()
        .ok_or_else(|| format!("test executable has no parent: {}", executable.display()))?;
    let revision = ruviform_revision()
        .ok_or_else(|| "failed to resolve ruviform revision from Cargo.lock".to_owned())?;
    let library_name = format!(
        "{}{}{}",
        env::consts::DLL_PREFIX,
        format.crate_stem(),
        env::consts::DLL_SUFFIX
    );
    let mut candidates = vec![deps_dir.join(&library_name)];
    if let Some(profile_dir) = deps_dir.parent() {
        candidates.push(profile_dir.join(&library_name));
    }

    let stem_prefix = format!("{}{}", env::consts::DLL_PREFIX, format.crate_stem());
    let suffix = env::consts::DLL_SUFFIX.to_owned();
    let mut hashed = Vec::new();
    if let Ok(entries) = fs::read_dir(deps_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&stem_prefix) && name.ends_with(&suffix) {
                hashed.push(entry.path());
            }
        }
    }
    hashed.sort();
    candidates.extend(hashed);

    for candidate in &candidates {
        if candidate.is_file() && depinfo_proves_locked_ruviform(candidate, format, &revision) {
            return artifact_from_path(candidate.clone());
        }
    }

    Err(format!(
        "no {} cdylib proven by dep-info to come from locked ruviform rev {}; checked {}",
        format.label(),
        revision,
        candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", "),
    ))
}

// ── Host fixture ─────────────────────────────────────────────────

struct LoadedFixture {
    format: Format,
    artifact: Artifact,
    host: Rc<RefCell<PluginHost>>,
    descriptors: Vec<PluginDescriptor>,
    factory_create_sendable: BTreeMap<String, bool>,
}

fn load_fixture(format: Format) -> LoadedFixture {
    let artifact = resolve_artifact(format)
        .unwrap_or_else(|error| panic!("{} artifact resolution failed: {error}", format.label()));
    load_fixture_at(format, artifact)
}

fn load_fixture_at(format: Format, artifact: Artifact) -> LoadedFixture {
    let (processors, factory_create_sendable) = match format {
        Format::Vst3 => {
            let library = Vst3Library::load(&artifact.path).unwrap_or_else(|error| {
                panic!("load VST3 artifact {}: {error:?}", artifact.path.display())
            });
            let factories = library.factories();
            let capabilities = factories
                .iter()
                .map(|factory| {
                    (
                        factory.descriptor().id.to_owned(),
                        factory.create_sendable().is_some(),
                    )
                })
                .collect();
            let factories = factories
                .into_iter()
                .map(|factory| Box::new(factory) as Box<dyn PluginFactory>);
            (PluginHostProcessors::product(factories), capabilities)
        }
        Format::Clap => {
            let library = ClapLibrary::load(&artifact.path).unwrap_or_else(|error| {
                panic!("load CLAP artifact {}: {error:?}", artifact.path.display())
            });
            let factories = library.factories();
            let capabilities = factories
                .iter()
                .map(|factory| {
                    (
                        factory.descriptor().id.to_owned(),
                        factory.create_sendable().is_some(),
                    )
                })
                .collect();
            let factories = factories
                .into_iter()
                .map(|factory| Box::new(factory) as Box<dyn PluginFactory>);
            (PluginHostProcessors::product(factories), capabilities)
        }
    };
    let host = processors.host();
    let descriptors = match host.borrow_mut().execute(HostCommand::Scan) {
        Ok(HostReply::Scanned(descriptors)) => descriptors,
        other => panic!("scan failed: {other:?}"),
    };
    LoadedFixture {
        format,
        artifact,
        host,
        descriptors,
        factory_create_sendable,
    }
}

impl LoadedFixture {
    fn descriptor_named(&self, name: &str) -> PluginDescriptor {
        self.descriptors
            .iter()
            .copied()
            .find(|descriptor| descriptor.name == name)
            .unwrap_or_else(|| panic!("{} fixture has no plugin named {name}", self.format.label()))
    }

    fn environment(&self, descriptor: Option<PluginDescriptor>) -> Value {
        base_environment(self.format, Some(&self.artifact), descriptor)
    }
}

struct RunningPlugin {
    host: Rc<RefCell<PluginHost>>,
    handle: PluginHandle,
    descriptor: PluginDescriptor,
    processor_id: ProcessorId,
}

impl Drop for RunningPlugin {
    fn drop(&mut self) {
        if let Ok(mut host) = self.host.try_borrow_mut() {
            let _ = host.execute(HostCommand::StopProcessing {
                handle: self.handle,
            });
            let _ = host.execute(HostCommand::Deactivate {
                handle: self.handle,
            });
            let _ = host.execute(HostCommand::Destroy {
                handle: self.handle,
            });
        }
    }
}

fn execute_ok(host: &Rc<RefCell<PluginHost>>, command: HostCommand) {
    match host.borrow_mut().execute(command) {
        Ok(HostReply::Ok) => {}
        other => panic!("host command failed: {other:?}"),
    }
}

fn start_plugin(fixture: &LoadedFixture, name: &str, processor_id: ProcessorId) -> RunningPlugin {
    let descriptor = fixture.descriptor_named(name);
    let handle = match fixture.host.borrow_mut().execute(HostCommand::Instantiate {
        plugin_id: descriptor.id.to_owned(),
    }) {
        Ok(HostReply::Instantiated { handle }) => handle,
        other => panic!("instantiate {name} failed: {other:?}"),
    };
    execute_ok(&fixture.host, HostCommand::Initialize { handle });
    execute_ok(
        &fixture.host,
        HostCommand::Activate {
            handle,
            config: ProcessConfig::new(SampleRate::new(48_000).unwrap(), 1, FRAMES_U32).unwrap(),
        },
    );
    execute_ok(&fixture.host, HostCommand::StartProcessing { handle });
    RunningPlugin {
        host: Rc::clone(&fixture.host),
        handle,
        descriptor,
        processor_id,
    }
}

fn stop_processing(plugin: &RunningPlugin) {
    execute_ok(
        &plugin.host,
        HostCommand::StopProcessing {
            handle: plugin.handle,
        },
    );
}

fn start_processing(plugin: &RunningPlugin) {
    execute_ok(
        &plugin.host,
        HostCommand::StartProcessing {
            handle: plugin.handle,
        },
    );
}

// ── Real host PCM driver ─────────────────────────────────────────

struct SliceIn<'a>(&'a [&'a [f32]]);

impl BufferView for SliceIn<'_> {
    fn channels(&self) -> usize {
        self.0.len()
    }

    fn frames(&self) -> usize {
        self.0.first().map_or(0, |channel| channel.len())
    }

    fn channel(&self, index: usize) -> Option<&[f32]> {
        self.0.get(index).copied()
    }
}

struct SliceOut<'a>(&'a mut [&'a mut [f32]]);

impl BufferView for SliceOut<'_> {
    fn channels(&self) -> usize {
        self.0.len()
    }

    fn frames(&self) -> usize {
        self.0.first().map_or(0, |channel| channel.len())
    }

    fn channel(&self, index: usize) -> Option<&[f32]> {
        self.0.get(index).map(|channel| &channel[..])
    }
}

impl BufferViewMut for SliceOut<'_> {
    fn channel_mut(&mut self, index: usize) -> Option<&mut [f32]> {
        self.0.get_mut(index).map(|channel| &mut channel[..])
    }
}

fn gain_event(normalized: f64) -> Event {
    Event::new(
        0,
        EventKind::ParamValue(ParamEvent {
            param_id: ParamId(0),
            value: ParamValue::new(normalized).unwrap(),
        }),
    )
}

fn process_stereo(
    plugin: &RunningPlugin,
    input_left: &[f32],
    input_right: &[f32],
    events: &[Event],
) -> (Vec<f32>, Vec<f32>) {
    assert_eq!(input_left.len(), input_right.len());
    let frames = input_left.len();
    let input_channels = [input_left, input_right];
    let input_view = SliceIn(&input_channels);
    let inputs = [HostInput(&input_view)];
    let mut output_left = vec![0.0; frames];
    let mut output_right = vec![0.0; frames];
    {
        let mut output_channels: [&mut [f32]; 2] = [&mut output_left[..], &mut output_right[..]];
        let mut output_view = SliceOut(&mut output_channels);
        let mut outputs = [HostOutput(&mut output_view)];
        let event_list = EventList::new(events, u32::try_from(frames).unwrap()).unwrap();
        plugin
            .host
            .borrow_mut()
            .process(plugin.handle, &inputs, &mut outputs, event_list, None)
            .unwrap_or_else(|error| panic!("{} process failed: {error:?}", plugin.descriptor.name));
    }
    (output_left, output_right)
}

fn constant_block(value: f32) -> Vec<f32> {
    vec![value; FRAMES]
}

fn pcm_close(actual: &[f32], expected: f32) -> bool {
    actual
        .iter()
        .all(|sample| (sample - expected).abs() <= PCM_TOLERANCE)
}

fn pcm_summary(left: &[f32], right: &[f32]) -> Value {
    json!({
        "frames": left.len(),
        "left_first": left.first(),
        "right_first": right.first(),
        "left_sha256": sha256_hex(bytemuck_f32(left).as_slice()),
        "right_sha256": sha256_hex(bytemuck_f32(right).as_slice()),
    })
}

fn bytemuck_f32(samples: &[f32]) -> Vec<u8> {
    samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect()
}

// ── RFST independent oracle ──────────────────────────────────────

#[derive(Clone, Copy, Debug)]
enum RfstPayload {
    Plain(f64),
    Pair {
        component_plain: f64,
        controller_normalized: f64,
    },
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    const FNV_OFFSET: u32 = 0x811C_9DC5;
    const FNV_PRIME: u32 = 0x0100_0193;
    let mut hash = FNV_OFFSET;
    for &byte in bytes {
        hash = (hash ^ u32::from(byte)).wrapping_mul(FNV_PRIME);
    }
    hash
}

fn parse_rfst_v2_block(block: &[u8]) -> Result<f64, String> {
    if block.len() != 20 {
        return Err(format!("RFST block length {}, expected 20", block.len()));
    }
    if block[0..4] != *b"RFST" {
        return Err("bad RFST magic".to_owned());
    }
    let version = u32::from_le_bytes(block[4..8].try_into().unwrap());
    if version != 2 {
        return Err(format!("unsupported RFST version {version}"));
    }
    let checksum = u32::from_le_bytes(block[16..20].try_into().unwrap());
    let expected = fnv1a32(&block[0..16]);
    if checksum != expected {
        return Err(format!(
            "RFST checksum mismatch: stored {checksum:#x}, expected {expected:#x}"
        ));
    }
    let value = f64::from_le_bytes(block[8..16].try_into().unwrap());
    if !value.is_finite() {
        return Err("non-finite RFST value".to_owned());
    }
    Ok(value)
}

fn unframe_vst3_state(blob: &[u8]) -> Result<(&[u8], &[u8]), String> {
    if blob.len() < 8 {
        return Err(format!(
            "VST3 adapter frame length {}, minimum 8",
            blob.len()
        ));
    }
    let component_len = u32::from_le_bytes(blob[0..4].try_into().unwrap()) as usize;
    let component_end = 4usize
        .checked_add(component_len)
        .filter(|end| *end + 4 <= blob.len())
        .ok_or_else(|| "VST3 component length exceeds frame".to_owned())?;
    let controller_len_end = component_end + 4;
    let controller_len =
        u32::from_le_bytes(blob[component_end..controller_len_end].try_into().unwrap()) as usize;
    let controller_end = controller_len_end
        .checked_add(controller_len)
        .filter(|end| *end == blob.len())
        .ok_or_else(|| "VST3 controller length does not end the frame exactly".to_owned())?;
    Ok((
        &blob[4..component_end],
        &blob[controller_len_end..controller_end],
    ))
}

fn parse_state_blob(blob: &[u8], format: Format, plugin_name: &str) -> Result<RfstPayload, String> {
    if format != Format::Vst3 {
        return parse_rfst_v2_block(blob).map(RfstPayload::Plain);
    }
    let (component, controller) = unframe_vst3_state(blob)?;
    let component_plain = parse_rfst_v2_block(component)?;
    if plugin_name == "test_stateful" {
        let controller_normalized = parse_rfst_v2_block(controller)?;
        Ok(RfstPayload::Pair {
            component_plain,
            controller_normalized,
        })
    } else {
        if !controller.is_empty() {
            return Err(format!(
                "single-component VST3 controller payload length {}, expected 0",
                controller.len()
            ));
        }
        Ok(RfstPayload::Plain(component_plain))
    }
}

fn rfst_plain(payload: RfstPayload) -> f64 {
    match payload {
        RfstPayload::Plain(plain)
        | RfstPayload::Pair {
            component_plain: plain,
            ..
        } => plain,
    }
}

fn rfst_normalized(payload: RfstPayload) -> Option<f64> {
    match payload {
        RfstPayload::Plain(_) => None,
        RfstPayload::Pair {
            controller_normalized,
            ..
        } => Some(controller_normalized),
    }
}

fn expected_blob_len(format: Format, plugin_name: &str) -> usize {
    if format != Format::Vst3 {
        20
    } else if plugin_name == "test_stateful" {
        // `[u32 comp_len][20-byte comp][u32 ctrl_len][20-byte ctrl]`.
        48
    } else {
        // `[u32 comp_len][20-byte comp][u32 ctrl_len = 0]`.
        28
    }
}

// ── Project/session helpers ──────────────────────────────────────

fn plugin_ref(descriptor: PluginDescriptor) -> PluginRef {
    PluginRef {
        descriptor: descriptor.id.into(),
        version: descriptor.version.into(),
        capability_revision: 1,
    }
}

fn project_with_processor(reference: &PluginRef) -> (ProjectController, ProcessorId) {
    let mut controller = ProjectController::new(
        Project::new(ProjectId::new(0), "tim04-real-abi"),
        Box::new(CounterIdGenerator::new(1)),
    );
    controller
        .apply(Command {
            expected_revision: controller.live_revision(),
            op: ProjectOp::AddTrack {
                name: "track".into(),
            },
        })
        .unwrap();
    let track = *controller.live().tracks.keys().next_back().unwrap();
    controller
        .apply(Command {
            expected_revision: controller.live_revision(),
            op: ProjectOp::AddTrackStrip { track_id: track },
        })
        .unwrap();
    let strip = controller.live().routing.strip_of_track(track).unwrap().id;
    controller
        .apply(Command {
            expected_revision: controller.live_revision(),
            op: ProjectOp::InsertProcessor {
                strip_id: strip,
                index: 0,
                name: "plugin-state".into(),
                binding: ProcessorBinding::Resolved(reference.clone()),
                sidechain_input: false,
            },
        })
        .unwrap();
    let processor_id = *controller
        .live()
        .routing
        .processors
        .keys()
        .next_back()
        .unwrap();
    (controller, processor_id)
}

fn temp_store_root(case: &str) -> PathBuf {
    let sequence = STORE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!(
        "synvelis-tim04-real-abi-{case}-{}-{sequence}",
        std::process::id()
    ))
}

type RealSession = PluginStateSession<PluginStateHost, DirectoryPluginStateStore>;

fn project_and_session(
    plugin: &RunningPlugin,
    case: &str,
) -> (ProjectController, RealSession, PathBuf) {
    let reference = plugin_ref(plugin.descriptor);
    let (controller, processor_id) = project_with_processor(&reference);
    let mut port = PluginStateHost::new(Rc::clone(&plugin.host));
    port.bind(processor_id, plugin.handle, &reference, 1);
    let root = temp_store_root(case);
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    let store = DirectoryPluginStateStore::create(&root).unwrap();
    (controller, PluginStateSession::new(port, store), root)
}

fn save_request(controller: &ProjectController) -> PluginStateSaveRequest<'_> {
    PluginStateSaveRequest {
        project: controller.live(),
        project_revision: controller.live_revision().0,
        project_head: None,
        mode: CaptureMode::Coordinated,
        policy: StateQualityPolicy::Strict,
        stored_at: "tim04-real-abi".into(),
    }
}

fn captured_digest(
    receipt: &synvelis_project::PluginStateReceipt,
) -> (synvelis_project::ContentDigest, u64) {
    assert_eq!(receipt.records.len(), 1);
    match &receipt.records[0].outcome {
        PluginStateOutcome::Captured {
            blob_digest,
            blob_len,
            ..
        } => (*blob_digest, *blob_len),
        other => panic!("save did not capture state: {other:?}"),
    }
}

fn blob_bytes(root: &Path, digest: &synvelis_project::ContentDigest) -> Vec<u8> {
    fs::read(root.join("blobs").join(digest_hex(digest))).expect("read captured RFST blob")
}

fn restore_explicit(
    session: &mut RealSession,
    controller: &ProjectController,
    manifest_name: &str,
) -> synvelis_project::PluginStateRestoreReceipt {
    session
        .restore(&PluginStateRestoreRequest {
            manifest: Some(manifest_name.to_owned()),
            project: controller.live(),
        })
        .expect("restore manifest")
}

fn first_restore_outcome(
    receipt: &synvelis_project::PluginStateRestoreReceipt,
) -> &PluginStateRestoreOutcome {
    assert_eq!(receipt.records.len(), 1);
    &receipt.records[0].outcome
}

// ── Typed-runtime capability probe ───────────────────────────────

fn endpoint_gap(plugin: &RunningPlugin) -> (bool, Value) {
    let result = plugin.host.borrow_mut().prepare_endpoint(
        plugin.handle,
        EndpointBounds {
            max_frames: FRAMES_U32,
            channels: 2,
        },
    );
    match result {
        Err(HostError::EndpointUnavailable) => (
            true,
            json!({
                "error_kind": "EndpointUnavailable",
                "location": "PluginHost::prepare_endpoint",
                "root_cause": "factory create_sendable() is None for !Send real ABI plugin wrappers",
            }),
        ),
        Err(error) => (
            false,
            json!({
                "error_kind": format!("{error:?}"),
                "location": "PluginHost::prepare_endpoint",
            }),
        ),
        Ok(endpoint) => {
            let latency = endpoint.latency_frames();
            drop(endpoint);
            (
                false,
                json!({
                    "unexpected": "endpoint minted",
                    "latency_frames": latency,
                    "location": "PluginHost::prepare_endpoint",
                }),
            )
        }
    }
}

fn record_endpoint_gap(fixture: &LoadedFixture, plugin: &RunningPlugin) {
    let (is_gap, mut observed) = endpoint_gap(plugin);
    observed["factory_create_sendable"] = json!(
        fixture
            .factory_create_sendable
            .get(plugin.descriptor.id)
            .copied()
    );
    let verdict = if is_gap { "Unsupported" } else { "Fail" };
    write_case(
        &format!(
            "{}-typed-runtime-endpoint",
            fixture.format.label().to_lowercase()
        ),
        "PluginHost::prepare_endpoint",
        verdict,
        "typed-graph PCM attachment capability for a real ABI in-process instance",
        observed.clone(),
        fixture.environment(Some(plugin.descriptor)),
    );
    assert!(
        is_gap,
        "expected EndpointUnavailable from prepare_endpoint, observed {observed}"
    );
}

// ── Artifact and capability matrix cases ─────────────────────────

fn assert_depinfo_provenance_filter() {
    assert_eq!(
        depinfo_path(Path::new("target/libruviform_plugin_test_vst3.so")),
        Path::new("target/ruviform_plugin_test_vst3.d")
    );
    assert_eq!(
        depinfo_path(Path::new("target/ruviform_plugin_test_vst3.dll")),
        Path::new("target/ruviform_plugin_test_vst3.d")
    );

    let probe_root = temp_store_root("artifact-depinfo");
    if probe_root.exists() {
        fs::remove_dir_all(&probe_root).unwrap();
    }
    fs::create_dir_all(&probe_root).unwrap();
    let probe_artifact = probe_root.join("ruviform_plugin_test_vst3.dll");
    fs::write(&probe_artifact, b"probe").unwrap();
    let probe_depinfo = probe_artifact.with_extension("d");
    let probe_revision = "abcdef1234567890";
    fs::write(
        &probe_depinfo,
        r"target: C:\Users\probe\.cargo\git\checkouts\ruviform-deadbeef\abcdef1\crates\ruviform-plugin-test-vst3\src\lib.rs",
    )
    .unwrap();
    assert!(depinfo_proves_locked_ruviform(
        &probe_artifact,
        Format::Vst3,
        probe_revision
    ));
    fs::write(
        &probe_depinfo,
        r"target: C:\Users\probe\.cargo\git\checkouts\ruviform-deadbeef\0000000\crates\ruviform-plugin-test-vst3\src\lib.rs",
    )
    .unwrap();
    assert!(!depinfo_proves_locked_ruviform(
        &probe_artifact,
        Format::Vst3,
        probe_revision
    ));
    fs::write(
        &probe_depinfo,
        r"target: G:\orca\workspaces\ruviform\rf253-midi-e2e\target\debug\deps\ruviform_plugin_test_vst3.d: crates\ruviform-plugin-test-vst3\src\lib.rs",
    )
    .unwrap();
    assert!(!depinfo_proves_locked_ruviform(
        &probe_artifact,
        Format::Vst3,
        probe_revision
    ));
    fs::remove_file(&probe_depinfo).unwrap();
    assert!(!depinfo_proves_locked_ruviform(
        &probe_artifact,
        Format::Vst3,
        probe_revision
    ));
    fs::remove_dir_all(&probe_root).unwrap();
}

#[test]
#[ignore = "artifact-dependent real ABI lane: requires prebuilt fixture artifacts (run `mise run test-tim04-real-abi`)"]
fn artifact_resolution_and_scan_surface_fail_closed() {
    assert_depinfo_provenance_filter();

    for format in Format::ALL {
        let artifact = match resolve_artifact(format) {
            Ok(artifact) => artifact,
            Err(error) => {
                write_case(
                    &format!("{}-artifact", format.label().to_lowercase()),
                    "cargo test-plugin artifact resolution",
                    "Fail",
                    "required test-plugin cdylib artifact is absent",
                    json!({ "error": error }),
                    base_environment(format, None, None),
                );
                panic!("{} artifact resolution failed", format.label());
            }
        };
        let artifact_ok = artifact.path.is_file() && artifact.sha256.len() == 64;
        write_case(
            &format!("{}-artifact", format.label().to_lowercase()),
            "cargo test-plugin artifact resolution",
            if artifact_ok { "Pass" } else { "Fail" },
            "resolved exact test-plugin cdylib and recorded its SHA-256",
            json!({
                "path": artifact.path,
                "sha256": artifact.sha256,
                "is_file": artifact.path.is_file(),
            }),
            base_environment(format, Some(&artifact), None),
        );
        assert!(artifact_ok);

        let fixture = load_fixture_at(format, artifact);
        let names: Vec<_> = fixture
            .descriptors
            .iter()
            .map(|descriptor| descriptor.name)
            .collect();
        let required = match format {
            Format::Vst3 => vec!["test_fault", "test_stateful", "test_stateful_single"],
            Format::Clap => vec!["test_fault", "test_stateful"],
        };
        let scan_ok = required.iter().all(|name| names.contains(name));
        write_case(
            &format!("{}-scan", format.label().to_lowercase()),
            "Vst3Library/ClapLibrary factories + PluginHost Scan",
            if scan_ok { "Pass" } else { "Fail" },
            "required fixture descriptors are visible through the production host scan surface",
            json!({
                "required": required,
                "descriptors": fixture.descriptors.iter().map(|descriptor| json!({
                    "id": descriptor.id,
                    "name": descriptor.name,
                    "vendor": descriptor.vendor,
                    "version": descriptor.version,
                })).collect::<Vec<_>>(),
            }),
            fixture.environment(None),
        );
        assert!(
            scan_ok,
            "{} scan missing required fixture descriptors",
            format.label()
        );
    }
}

fn expected_quality(processing: bool, mode: CaptureMode) -> StateCaptureQuality {
    if processing {
        StateCaptureQuality::BestEffort
    } else if mode == CaptureMode::Coordinated {
        StateCaptureQuality::Coordinated
    } else {
        StateCaptureQuality::Quiescent
    }
}

#[test]
#[ignore = "artifact-dependent real ABI lane: requires prebuilt fixture artifacts (run `mise run test-tim04-real-abi`)"]
fn capability_matrix_is_measured_for_real_test_plugins() {
    for format in Format::ALL {
        let fixture = load_fixture(format);
        let plugin_names: &[&str] = match format {
            Format::Vst3 => &["test_stateful", "test_stateful_single", "test_fault"],
            Format::Clap => &["test_stateful", "test_fault"],
        };
        let mut endpoint_recorded = false;
        for (index, plugin_name) in plugin_names.iter().enumerate() {
            let plugin = start_plugin(&fixture, plugin_name, ProcessorId::new((index + 1) as u128));
            if !endpoint_recorded {
                record_endpoint_gap(&fixture, &plugin);
                endpoint_recorded = true;
            }
            for processing in [true, false] {
                if !processing {
                    stop_processing(&plugin);
                }
                for mode in [
                    CaptureMode::Coordinated,
                    CaptureMode::Quiescent,
                    CaptureMode::BestEffort,
                ] {
                    let reference = plugin_ref(plugin.descriptor);
                    let mut port = PluginStateHost::new(Rc::clone(&plugin.host));
                    port.bind(plugin.processor_id, plugin.handle, &reference, 1);
                    let result = port.capture(plugin.processor_id, mode);
                    let lifecycle = if processing { "processing" } else { "stopped" };
                    let case = format!(
                        "{}-matrix-{plugin_name}-{lifecycle}-{mode:?}",
                        format.label().to_lowercase()
                    )
                    .replace(' ', "-");
                    let is_stateful = plugin_name.starts_with("test_stateful");
                    let (verdict, detail, mut observed) = match &result {
                        Ok(capture) => {
                            let parsed = parse_state_blob(&capture.blob, format, plugin_name);
                            let expected_len = expected_blob_len(format, plugin_name);
                            let quality_expected = expected_quality(processing, mode);
                            let pass = is_stateful
                                && capture.achieved == quality_expected
                                && capture.blob.len() == expected_len
                                && parsed.is_ok();
                            (
                                if pass { "Pass" } else { "Fail" },
                                "stateful capture quality, RFST shape, checksum, and length",
                                json!({
                                    "capture_outcome": "Captured",
                                    "achieved_quality": format!("{:?}", capture.achieved),
                                    "expected_quality": format!("{quality_expected:?}"),
                                    "processing_observed": capture.observation.processing,
                                    "stable_boundary_observed": capture.observation.stable_boundary,
                                    "blob_len": capture.blob.len(),
                                    "expected_blob_len": expected_len,
                                    "blob_sha256": sha256_hex(&capture.blob),
                                    "rfst_parse": parsed.map(|payload| json!({
                                        "plain": rfst_plain(payload),
                                        "normalized": rfst_normalized(payload),
                                    })).map_err(|error| json!({ "error": error })),
                                }),
                            )
                        }
                        Err(error) => {
                            let expected_unsupported =
                                !is_stateful && *error == PluginStateCaptureError::Unsupported;
                            (
                                if expected_unsupported {
                                    "Unsupported"
                                } else {
                                    "Fail"
                                },
                                "stateless fixture reports the typed Unsupported capability verdict",
                                json!({
                                    "capture_outcome": format!("{error:?}"),
                                    "expected_unsupported": expected_unsupported,
                                }),
                            )
                        }
                    };
                    observed["mode"] = json!(format!("{mode:?}"));
                    observed["processing_lifecycle"] = json!(processing);
                    write_case(
                        &case,
                        "PluginStateHost::capture",
                        verdict,
                        detail,
                        observed,
                        fixture.environment(Some(plugin.descriptor)),
                    );
                    if is_stateful {
                        assert_eq!(verdict, "Pass", "{case} failed");
                    } else {
                        assert_eq!(verdict, "Unsupported", "{case} failed");
                    }
                }
            }
        }
    }
}

// ── Save → mutate → restore → save oracle ────────────────────────

struct RoundtripEvidence {
    initial_len: u64,
    mutated_len: u64,
    restored_len: u64,
    expected_len: usize,
    initial_digest: synvelis_project::ContentDigest,
    mutated_digest: synvelis_project::ContentDigest,
    restored_digest: synvelis_project::ContentDigest,
    initial_blob: Vec<u8>,
    mutated_blob: Vec<u8>,
    restored_blob: Vec<u8>,
    initial_payload: RfstPayload,
    mutated_payload: RfstPayload,
    restored_payload: RfstPayload,
    restore_outcome: PluginStateRestoreOutcome,
    initial_left: Vec<f32>,
    initial_right: Vec<f32>,
    mutated_left: Vec<f32>,
    mutated_right: Vec<f32>,
    restored_left: Vec<f32>,
    restored_right: Vec<f32>,
    pass: bool,
}

fn measure_stateful_roundtrip(
    plugin: &RunningPlugin,
    session: &mut RealSession,
    controller: &ProjectController,
    store_root: &Path,
    format: Format,
    plugin_name: &str,
) -> RoundtripEvidence {
    let initial_receipt = session
        .save(&save_request(controller))
        .expect("initial save");
    let (initial_digest, initial_len) = captured_digest(&initial_receipt);
    let initial_blob = blob_bytes(store_root, &initial_digest);
    let initial_payload =
        parse_state_blob(&initial_blob, format, plugin_name).expect("initial RFST parses");
    let initial_name = initial_receipt
        .manifest_name
        .clone()
        .expect("initial manifest name");

    start_processing(plugin);
    let input_left = constant_block(0.5);
    let input_right = constant_block(-0.25);
    let (initial_left, initial_right) = process_stereo(plugin, &input_left, &input_right, &[]);
    let events = [gain_event(MUTATED_NORMALIZED)];
    let (mutated_left, mutated_right) = process_stereo(plugin, &input_left, &input_right, &events);

    stop_processing(plugin);
    let mutated_receipt = session
        .save(&save_request(controller))
        .expect("mutated save");
    let (mutated_digest, mutated_len) = captured_digest(&mutated_receipt);
    let mutated_blob = blob_bytes(store_root, &mutated_digest);
    let mutated_payload =
        parse_state_blob(&mutated_blob, format, plugin_name).expect("mutated RFST parses");

    let initial_restore_receipt = restore_explicit(session, controller, &initial_name);
    let restore_outcome = first_restore_outcome(&initial_restore_receipt).clone();
    let restored_receipt = session
        .save(&save_request(controller))
        .expect("restored save");
    let (restored_digest, restored_len) = captured_digest(&restored_receipt);
    let restored_blob = blob_bytes(store_root, &restored_digest);
    let restored_payload =
        parse_state_blob(&restored_blob, format, plugin_name).expect("restored RFST parses");

    start_processing(plugin);
    let (restored_left, restored_right) = process_stereo(plugin, &input_left, &input_right, &[]);

    let mut evidence = RoundtripEvidence {
        initial_len,
        mutated_len,
        restored_len,
        expected_len: expected_blob_len(format, plugin_name),
        initial_digest,
        mutated_digest,
        restored_digest,
        initial_blob,
        mutated_blob,
        restored_blob,
        initial_payload,
        mutated_payload,
        restored_payload,
        restore_outcome,
        initial_left,
        initial_right,
        mutated_left,
        mutated_right,
        restored_left,
        restored_right,
        pass: false,
    };
    evidence.pass = roundtrip_pass(&evidence);
    evidence
}

fn payload_state_matches(payload: RfstPayload, plain: f64) -> bool {
    (rfst_plain(payload) - plain).abs() <= 1.0e-12
        && rfst_normalized(payload)
            .is_none_or(|value| (value - DEFAULT_NORMALIZED).abs() <= 1.0e-12)
}

fn roundtrip_check_values(evidence: &RoundtripEvidence) -> [bool; 10] {
    let blob_shape = evidence.initial_blob.len() == evidence.expected_len
        && evidence.mutated_blob.len() == evidence.expected_len
        && evidence.restored_blob.len() == evidence.expected_len;
    let digest_matches = digest_hex(&evidence.initial_digest) == sha256_hex(&evidence.initial_blob)
        && digest_hex(&evidence.mutated_digest) == sha256_hex(&evidence.mutated_blob)
        && digest_hex(&evidence.restored_digest) == sha256_hex(&evidence.restored_blob);
    [
        pcm_close(&evidence.initial_left, 0.5) && pcm_close(&evidence.initial_right, -0.25),
        pcm_close(&evidence.mutated_left, 0.375) && pcm_close(&evidence.mutated_right, -0.1875),
        blob_shape,
        digest_matches,
        payload_state_matches(evidence.initial_payload, DEFAULT_PLAIN),
        payload_state_matches(evidence.mutated_payload, MUTATED_PLAIN),
        evidence.mutated_blob != evidence.initial_blob,
        evidence.restore_outcome == PluginStateRestoreOutcome::Restored,
        evidence.restored_blob == evidence.initial_blob
            && evidence.restored_digest == evidence.initial_digest
            && (rfst_plain(evidence.restored_payload) - DEFAULT_PLAIN).abs() <= 1.0e-12,
        pcm_close(&evidence.restored_left, 0.5) && pcm_close(&evidence.restored_right, -0.25),
    ]
}

fn roundtrip_checks(evidence: &RoundtripEvidence) -> Value {
    let [
        pcm_initial,
        pcm_mutated,
        blob_shape,
        digest_matches,
        initial_state,
        mutated_state,
        mutation_visible,
        restore,
        restored_state,
        pcm_restored,
    ] = roundtrip_check_values(evidence);
    json!({
        "pcm_initial": pcm_initial,
        "pcm_mutated": pcm_mutated,
        "blob_shape": blob_shape,
        "digest_matches": digest_matches,
        "initial_state": initial_state,
        "mutated_state": mutated_state,
        "mutation_visible": mutation_visible,
        "restore": restore,
        "restored_state": restored_state,
        "pcm_restored": pcm_restored,
    })
}

fn roundtrip_pass(evidence: &RoundtripEvidence) -> bool {
    roundtrip_check_values(evidence)
        .into_iter()
        .all(|check| check)
}

fn write_roundtrip_case(case: &str, evidence: &RoundtripEvidence, environment: Value) {
    write_case(
        case,
        "PluginHost::process + PluginStateSession save/restore",
        if evidence.pass { "Pass" } else { "Fail" },
        "real ABI save → PCM/parameter mutation → restore initial blob → save round-trip oracle",
        json!({
            "blob_len": {
                "initial": evidence.initial_len,
                "mutated": evidence.mutated_len,
                "restored": evidence.restored_len,
                "expected": evidence.expected_len,
            },
            "blob_sha256": {
                "initial": sha256_hex(&evidence.initial_blob),
                "mutated": sha256_hex(&evidence.mutated_blob),
                "restored": sha256_hex(&evidence.restored_blob),
            },
            "receipt_blob_digest": {
                "initial": digest_hex(&evidence.initial_digest),
                "mutated": digest_hex(&evidence.mutated_digest),
                "restored": digest_hex(&evidence.restored_digest),
            },
            "rfst": {
                "initial": {
                    "plain": rfst_plain(evidence.initial_payload),
                    "normalized": rfst_normalized(evidence.initial_payload),
                },
                "mutated": {
                    "plain": rfst_plain(evidence.mutated_payload),
                    "normalized": rfst_normalized(evidence.mutated_payload),
                },
                "restored": {
                    "plain": rfst_plain(evidence.restored_payload),
                    "normalized": rfst_normalized(evidence.restored_payload),
                },
            },
            "restore_outcome": format!("{:?}", evidence.restore_outcome),
            "pcm": {
                "initial": pcm_summary(&evidence.initial_left, &evidence.initial_right),
                "mutated": pcm_summary(&evidence.mutated_left, &evidence.mutated_right),
                "restored": pcm_summary(&evidence.restored_left, &evidence.restored_right),
            },
            "checks": roundtrip_checks(evidence),
        }),
        environment,
    );
}

fn stateful_roundtrip(format: Format, plugin_name: &str) {
    let fixture = load_fixture(format);
    let plugin = start_plugin(&fixture, plugin_name, ProcessorId::new(1));
    stop_processing(&plugin);
    let case_prefix = format!("{}-{plugin_name}-roundtrip", format.label().to_lowercase());
    let (controller, mut session, store_root) = project_and_session(&plugin, &case_prefix);
    let evidence = measure_stateful_roundtrip(
        &plugin,
        &mut session,
        &controller,
        &store_root,
        format,
        plugin_name,
    );
    let pass = evidence.pass;
    write_roundtrip_case(
        &case_prefix,
        &evidence,
        fixture.environment(Some(plugin.descriptor)),
    );
    drop(session);
    let _ = fs::remove_dir_all(&store_root);
    assert!(pass, "{case_prefix} oracle failed");
}

#[test]
#[ignore = "artifact-dependent real ABI lane: requires prebuilt fixture artifacts (run `mise run test-tim04-real-abi`)"]
fn vst3_distributed_stateful_roundtrip_oracle() {
    stateful_roundtrip(Format::Vst3, "test_stateful");
}

#[test]
#[ignore = "artifact-dependent real ABI lane: requires prebuilt fixture artifacts (run `mise run test-tim04-real-abi`)"]
fn vst3_single_stateful_roundtrip_oracle() {
    stateful_roundtrip(Format::Vst3, "test_stateful_single");
}

#[test]
#[ignore = "artifact-dependent real ABI lane: requires prebuilt fixture artifacts (run `mise run test-tim04-real-abi`)"]
fn clap_stateful_roundtrip_oracle() {
    stateful_roundtrip(Format::Clap, "test_stateful");
}

// ── Corrupt/missing blob integrity ───────────────────────────────

#[test]
#[ignore = "artifact-dependent real ABI lane: requires prebuilt fixture artifacts (run `mise run test-tim04-real-abi`)"]
fn corrupt_and_missing_blobs_restore_as_typed_failures() {
    for format in Format::ALL {
        let fixture = load_fixture(format);
        let plugin = start_plugin(&fixture, "test_stateful", ProcessorId::new(1));
        stop_processing(&plugin);
        let case_prefix = format!("{}-integrity", format.label().to_lowercase());
        let (controller, mut session, store_root) = project_and_session(&plugin, &case_prefix);
        let receipt = session
            .save(&save_request(&controller))
            .expect("integrity save");
        let (digest, _) = captured_digest(&receipt);
        let manifest_name = receipt
            .manifest_name
            .clone()
            .expect("integrity manifest name");
        let blob_path = store_root.join("blobs").join(digest_hex(&digest));

        let mut corrupted = fs::read(&blob_path).expect("read blob before corruption");
        corrupted[0] ^= 0x01;
        fs::write(&blob_path, corrupted).expect("write corrupted blob");
        let corrupt_receipt = restore_explicit(&mut session, &controller, &manifest_name);
        let corrupt_outcome = first_restore_outcome(&corrupt_receipt);
        let corrupt_ok = matches!(
            corrupt_outcome,
            PluginStateRestoreOutcome::Failed {
                kind: synvelis_project::PluginStateFailureKind::Corrupt
            }
        );
        write_case(
            &format!("{case_prefix}-corrupt"),
            "PluginStateSession::restore blob digest precheck",
            if corrupt_ok { "Pass" } else { "Fail" },
            "bit-flipped content-addressed blob is typed Corrupt before the restore port is reached",
            json!({
                "blob_digest": digest.to_string(),
                "restore_outcome": format!("{corrupt_outcome:?}"),
                "blob_path": blob_path,
            }),
            fixture.environment(Some(plugin.descriptor)),
        );
        assert!(corrupt_ok, "corrupt blob did not restore as typed Corrupt");

        fs::remove_file(&blob_path).expect("remove blob for missing case");
        let missing_receipt = restore_explicit(&mut session, &controller, &manifest_name);
        let missing_outcome = first_restore_outcome(&missing_receipt);
        let missing_ok = matches!(
            missing_outcome,
            PluginStateRestoreOutcome::Failed {
                kind: synvelis_project::PluginStateFailureKind::Missing
            }
        );
        write_case(
            &format!("{case_prefix}-missing"),
            "PluginStateSession::restore blob existence precheck",
            if missing_ok { "Pass" } else { "Fail" },
            "absent content-addressed blob is typed Missing and is never passed to the plugin",
            json!({
                "blob_digest": digest.to_string(),
                "restore_outcome": format!("{missing_outcome:?}"),
                "blob_path": blob_path,
            }),
            fixture.environment(Some(plugin.descriptor)),
        );
        assert!(missing_ok, "missing blob did not restore as typed Missing");

        drop(session);
        let _ = fs::remove_dir_all(&store_root);
    }
}
