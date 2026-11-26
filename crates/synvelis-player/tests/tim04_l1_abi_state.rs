//! TIM-04 L1 out-of-process real-ABI state/fault consumer run.
//!
//! These tests register the real ruviform `test_stateful`/`test_fault`
//! VST3/CLAP binaries with `PluginHost::register_out_of_process` and
//! drive lifecycle, audio ordering, state capture/restore, and fault
//! projection through production host/state-session contracts.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use ruviform_audio_core::{PlanarBufferView, PlanarBufferViewMut};
use ruviform_plugin_core::{
    Event, EventKind, EventList, HostInput, HostOutput, LifecycleState, ParamEvent, ParamId,
    ParamValue, PluginDescriptor, ProcessConfig, ProcessResult,
};
use ruviform_plugin_host::{
    DiagnosticQuery, FailureCause, HostCommand, HostError, HostNotification, HostReply,
    OutOfProcessIsolation, OutOfProcessSpec, PluginHandle, PluginHost, SpawnPhase,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use synvelis_player::plugin_state::PluginStateHost;
use synvelis_project::{
    CaptureMode, Command, CounterIdGenerator, DirectoryPluginStateStore, PluginRef,
    PluginStateError, PluginStateFailureKind, PluginStateOutcome, PluginStatePort,
    PluginStateRestoreRequest, PluginStateSaveRequest, PluginStateSession, ProcessorBinding,
    ProcessorId, Project, ProjectController, ProjectId, ProjectOp, StateCaptureQuality,
    StateQualityPolicy, StripId, digest_hex,
};

const RUVIFORM_REV: &str = "20dc41f71c47674b3e51d1eb0921124b94f8bfaf";
const EVIDENCE_ENV: &str = "SYNVELIS_TIM04_L1_EVIDENCE_DIR";
const FRAMES: usize = 512;
const FRAMES_U32: u32 = 512;
const MUTATION_RESEND_CALLS: u32 = 8;
const MAX_FOLLOW_UPS: u32 = 96;
const SPAWN_TIMEOUT_RETRIES: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Vst3,
    Clap,
}

impl Format {
    fn label(self) -> &'static str {
        match self {
            Self::Vst3 => "vst3",
            Self::Clap => "clap",
        }
    }

    fn artifact_env(self) -> &'static str {
        match self {
            Self::Vst3 => "SYNVELIS_TIM04_TEST_VST3_PATH",
            Self::Clap => "SYNVELIS_TIM04_TEST_CLAP_PATH",
        }
    }

    fn artifact_file(self) -> &'static str {
        match self {
            Self::Vst3 => "ruviform_plugin_test_vst3",
            Self::Clap => "ruviform_plugin_test_clap",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FixtureKind {
    Stateful,
    Fault,
}

fn hex_nibble(byte: u8) -> char {
    match byte {
        0..=9 => (b'0' + byte) as char,
        _ => (b'a' + byte - 10) as char,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(hex_nibble(byte >> 4));
        out.push(hex_nibble(byte & 0x0f));
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex_encode(&Sha256::digest(bytes))
}

fn file_sha256(path: &Path) -> String {
    sha256_hex(&fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display())))
}

#[cfg(target_os = "windows")]
fn vst3_uid_bytes(a: u32, b: u32, c: u32, d: u32) -> [u8; 16] {
    [
        (a & 0xff) as u8,
        ((a >> 8) & 0xff) as u8,
        ((a >> 16) & 0xff) as u8,
        ((a >> 24) & 0xff) as u8,
        ((b >> 16) & 0xff) as u8,
        ((b >> 24) & 0xff) as u8,
        (b & 0xff) as u8,
        ((b >> 8) & 0xff) as u8,
        ((c >> 24) & 0xff) as u8,
        ((c >> 16) & 0xff) as u8,
        ((c >> 8) & 0xff) as u8,
        (c & 0xff) as u8,
        ((d >> 24) & 0xff) as u8,
        ((d >> 16) & 0xff) as u8,
        ((d >> 8) & 0xff) as u8,
        (d & 0xff) as u8,
    ]
}

#[cfg(not(target_os = "windows"))]
fn vst3_uid_bytes(a: u32, b: u32, c: u32, d: u32) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[0..4].copy_from_slice(&a.to_be_bytes());
    bytes[4..8].copy_from_slice(&b.to_be_bytes());
    bytes[8..12].copy_from_slice(&c.to_be_bytes());
    bytes[12..16].copy_from_slice(&d.to_be_bytes());
    bytes
}

fn vst3_id(a: u32, b: u32, c: u32, d: u32) -> String {
    hex_encode(&vst3_uid_bytes(a, b, c, d))
}

fn descriptor(format: Format, kind: FixtureKind) -> PluginDescriptor {
    match (format, kind) {
        (Format::Vst3, FixtureKind::Stateful) => PluginDescriptor {
            id: Box::leak(
                vst3_id(0x8D2F_6B94, 0xA1C5_3E07, 0xF4B8_9D26, 0xC7E3_5A81).into_boxed_str(),
            ),
            name: "test_stateful",
            vendor: "Ruviform Test",
            version: "0.0.0",
            features: &["Fx"],
        },
        (Format::Vst3, FixtureKind::Fault) => PluginDescriptor {
            id: Box::leak(
                vst3_id(0x7C3A_91E4, 0x5F2B_8D61, 0xA4E7_0C93, 0xD8B5_6F2A).into_boxed_str(),
            ),
            name: "test_fault",
            vendor: "Ruviform Test",
            version: "0.0.0",
            features: &["Fx"],
        },
        (Format::Clap, FixtureKind::Stateful) => PluginDescriptor {
            id: "org.ruviform-test.clap-test-stateful",
            name: "test_stateful",
            vendor: "Ruviform Test",
            version: "0.0.0",
            features: &["audio-effect", "stereo"],
        },
        (Format::Clap, FixtureKind::Fault) => PluginDescriptor {
            id: "org.ruviform-test.clap-test-fault",
            name: "test_fault",
            vendor: "Ruviform Test",
            version: "0.0.0",
            features: &["audio-effect", "stereo"],
        },
    }
}

fn resolve_artifact(format: Format) -> PathBuf {
    if let Some(path) = env::var_os(format.artifact_env()) {
        let path = PathBuf::from(path);
        assert!(
            path.is_file(),
            "{} artifact from {} is missing: {}",
            format.label(),
            format.artifact_env(),
            path.display()
        );
        return path;
    }
    let exe = env::current_exe().expect("current_exe");
    let deps = exe
        .parent()
        .expect("test executable lives in target deps")
        .to_path_buf();
    let stem = format!("{}-", format.artifact_file());
    let suffix = env::consts::DLL_SUFFIX;
    let mut matches: Vec<PathBuf> = fs::read_dir(&deps)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&stem) && name.ends_with(suffix))
        })
        .collect();
    matches.sort();
    assert_eq!(
        matches.len(),
        1,
        "{} real fixture artifact resolution in {} was not unique: {matches:?}",
        format.label(),
        deps.display()
    );
    matches.remove(0)
}

fn runner_program() -> PathBuf {
    let path = option_env!("CARGO_BIN_EXE_tim04-l1-plugin-runner")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::current_exe()
                .expect("current_exe")
                .parent()
                .and_then(Path::parent)
                .expect("test executable lives under target/<profile>/deps")
                .join(if cfg!(windows) {
                    "tim04-l1-plugin-runner.exe"
                } else {
                    "tim04-l1-plugin-runner"
                })
        });
    assert!(
        path.is_file(),
        "TIM-04 L1 runner missing at {}; cargo must build the explicit bin target",
        path.display()
    );
    path
}

fn config() -> ProcessConfig {
    ProcessConfig::new(
        ruviform_audio_core::SampleRate::new(48_000).unwrap(),
        1,
        FRAMES_U32,
    )
    .unwrap()
}

#[derive(Clone, Copy, Default)]
struct LegDeltas {
    passthrough: u64,
    dropped: u64,
}
fn host_block(
    host: &Rc<RefCell<PluginHost>>,
    handle: PluginHandle,
    input: &[&[f32]; 2],
    out_left: &mut [f32],
    out_right: &mut [f32],
    events: EventList<'_>,
) -> (Result<ProcessResult, HostError>, LegDeltas) {
    let mut host = host.borrow_mut();
    let counters = |h: &PluginHost| {
        h.audio_counters(handle)
            .map_or((0, 0), |c| (c.passthrough_submits, c.dropped_submits))
    };
    let before = counters(&host);
    let iv = PlanarBufferView::new(&input[..]).unwrap();
    let inputs = [HostInput(&iv)];
    let mut ol = [&mut *out_left, &mut *out_right];
    let mut ov = PlanarBufferViewMut::new(&mut ol).unwrap();
    let mut outputs = [HostOutput(&mut ov)];
    let result = host.process(handle, &inputs, &mut outputs, events, None);
    let after = counters(&host);
    (
        result,
        LegDeltas {
            passthrough: after.0 - before.0,
            dropped: after.1 - before.1,
        },
    )
}

fn submit_block(
    host: &Rc<RefCell<PluginHost>>,
    handle: PluginHandle,
    events: EventList<'_>,
) -> ([f32; FRAMES], [f32; FRAMES], LegDeltas) {
    let l = [0.5f32; FRAMES];
    let r = [-0.25f32; FRAMES];
    let input = [&l[..], &r[..]];
    let mut ol = [0.0f32; FRAMES];
    let mut or = [0.0f32; FRAMES];
    thread::sleep(Duration::from_millis(5));
    let (result, d) = host_block(host, handle, &input, &mut ol, &mut or, events);
    assert_eq!(result, Ok(ProcessResult::CONTINUE));
    (ol, or, d)
}
fn param_event(normalized: f64) -> Event {
    Event::new(
        0,
        EventKind::ParamValue(ParamEvent {
            param_id: ParamId(0),
            value: ParamValue::new(normalized).unwrap(),
        }),
    )
}
fn submit_event_until_landed(
    host: &Rc<RefCell<PluginHost>>,
    handle: PluginHandle,
    normalized: f64,
) -> u32 {
    let events = [param_event(normalized)];
    for attempt in 1..=MUTATION_RESEND_CALLS {
        let (_l, _r, d) = submit_block(host, handle, EventList::new(&events, FRAMES_U32).unwrap());
        if d.dropped == 0 {
            return attempt;
        }
    }
    panic!("parameter event normalized={normalized} never landed")
}
fn sample_eq(a: f32, e: f32) -> bool {
    (a - e).abs() <= f32::EPSILON
}
fn wait_for_gain_output(
    host: &Rc<RefCell<PluginHost>>,
    handle: PluginHandle,
    el: f32,
    er: f32,
) -> (bool, u32, u32) {
    let mut substituted = 0;
    for call in 1..=MAX_FOLLOW_UPS {
        let (l, r, d) = submit_block(host, handle, EventList::empty(FRAMES_U32));
        if d.passthrough > 0 {
            substituted += 1;
            continue;
        }
        if l.iter().all(|&s| sample_eq(s, el)) && r.iter().all(|&s| sample_eq(s, er)) {
            return (true, call, substituted);
        }
    }
    (false, MAX_FOLLOW_UPS, substituted)
}
fn arm_fault(host: &Rc<RefCell<PluginHost>>, handle: PluginHandle, normalized: f64) -> u32 {
    let events = [param_event(normalized)];
    for attempt in 1..=MUTATION_RESEND_CALLS {
        let li = [0.5f32; FRAMES];
        let ri = [-0.25f32; FRAMES];
        let input = [&li[..], &ri[..]];
        let mut ol = [0.0f32; FRAMES];
        let mut or = [0.0f32; FRAMES];
        let (_r, d) = host_block(
            host,
            handle,
            &input,
            &mut ol,
            &mut or,
            EventList::new(&events, FRAMES_U32).unwrap(),
        );
        if d.dropped == 0 {
            return attempt;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("fault event normalized={normalized} never landed")
}
fn wait_failed(host: &Rc<RefCell<PluginHost>>, handle: PluginHandle) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if host.borrow().lifecycle_state(handle) == Some(LifecycleState::Failed) {
            return true;
        }
        thread::sleep(Duration::from_millis(5));
    }
    false
}
fn instantiate(host: &Rc<RefCell<PluginHost>>, plugin_id: &str) -> PluginHandle {
    for attempt in 0..=SPAWN_TIMEOUT_RETRIES {
        let result = {
            host.borrow_mut().execute(HostCommand::Instantiate {
                plugin_id: plugin_id.to_owned(),
            })
        };
        match result {
            Ok(HostReply::Instantiated { handle }) => return handle,
            Ok(other) => panic!("unexpected instantiate reply: {other:?}"),
            Err(HostError::SpawnFailed {
                cause,
                diagnostics_id,
            }) => {
                let query = host.borrow().diagnostics_query(diagnostics_id);
                let sig = cause == FailureCause::Timeout
                    && matches!(&query,DiagnosticQuery::Found(record) if matches!(record.phase,SpawnPhase::PreSpawn|SpawnPhase::Spawn|SpawnPhase::DrainInit|SpawnPhase::Handshake|SpawnPhase::Live)&&record.cause==Some(FailureCause::Timeout));
                assert!(
                    sig,
                    "instantiate failed outside bounded pre-mint timeout signature"
                );
                assert!(
                    attempt < SPAWN_TIMEOUT_RETRIES,
                    "pre-mint spawn timeout persisted through retry bound"
                );
            }
            Err(error) => panic!("instantiate failed: {error:?}"),
        }
    }
    unreachable!()
}
struct L1Instance {
    host: Rc<RefCell<PluginHost>>,
    handle: PluginHandle,
}
impl Drop for L1Instance {
    fn drop(&mut self) {
        let mut h = self.host.borrow_mut();
        let _ = h.execute(HostCommand::StopProcessing {
            handle: self.handle,
        });
        let _ = h.execute(HostCommand::Deactivate {
            handle: self.handle,
        });
        let _ = h.execute(HostCommand::Destroy {
            handle: self.handle,
        });
    }
}
fn drive_to_processing(host: &Rc<RefCell<PluginHost>>, handle: PluginHandle) {
    let mut h = host.borrow_mut();
    h.execute(HostCommand::Initialize { handle }).unwrap();
    h.execute(HostCommand::Activate {
        handle,
        config: config(),
    })
    .unwrap();
    h.execute(HostCommand::StartProcessing { handle }).unwrap();
}
fn prepare_host(
    routes: &[(Format, FixtureKind)],
    command_timeout: Duration,
) -> (Rc<RefCell<PluginHost>>, Vec<PluginDescriptor>) {
    let runner = runner_program();
    assert!(resolve_artifact(Format::Vst3).is_file());
    assert!(resolve_artifact(Format::Clap).is_file());
    let host = Rc::new(RefCell::new(PluginHost::new()));
    let mut expected = Vec::new();
    for &(format, kind) in routes {
        let d = descriptor(format, kind);
        let mut spec = OutOfProcessSpec::new(d, runner.clone().into_os_string());
        spec.isolation = OutOfProcessIsolation::Dedicated;
        spec.command_timeout = command_timeout;
        host.borrow_mut().register_out_of_process(spec).unwrap();
        expected.push(d);
    }
    let scanned = match host.borrow_mut().execute(HostCommand::Scan).unwrap() {
        HostReply::Scanned(d) => d,
        other => panic!("unexpected scan reply: {other:?}"),
    };
    for d in &expected {
        assert!(scanned.contains(d), "scan did not contain {}", d.id);
    }
    (host, expected)
}
fn spawn_instance(host: &Rc<RefCell<PluginHost>>, descriptor: PluginDescriptor) -> L1Instance {
    let handle = instantiate(host, descriptor.id);
    drive_to_processing(host, handle);
    assert_eq!(
        host.borrow().lifecycle_state(handle),
        Some(LifecycleState::Processing)
    );
    L1Instance {
        host: host.clone(),
        handle,
    }
}
fn plugin_ref(d: PluginDescriptor) -> PluginRef {
    PluginRef {
        descriptor: d.id.to_owned(),
        version: d.version.to_owned(),
        capability_revision: 0,
    }
}
fn evidence_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
fn environment(format: Format, d: PluginDescriptor, timeout_ms: u64) -> Value {
    let artifact = resolve_artifact(format);
    let runner = runner_program();
    json!({"os":env::consts::OS,"arch":env::consts::ARCH,"environment_kind":"test-plugin","isolation":"out-of-process/l1","format":format.label(),"fixture_artifact_path":artifact.display().to_string(),"fixture_artifact_sha256":file_sha256(&artifact),"runner_path":runner.display().to_string(),"runner_sha256":file_sha256(&runner),"ruviform_resolved_rev":RUVIFORM_REV,"descriptor":{"id":d.id,"name":d.name,"vendor":d.vendor,"version":d.version,"features":d.features},"command_timeout_ms":timeout_ms,"spawn_timeout_ms":timeout_ms,"isolation_mode":"Dedicated"})
}
fn write_evidence(case: &str, verdict: &str, detail: &str, environment: &Value, observed: &Value) {
    let Some(dir) = env::var_os(EVIDENCE_ENV) else {
        return;
    };
    let dir = PathBuf::from(dir);
    fs::create_dir_all(&dir).unwrap();
    let row = json!({"case":case,"driver":"tim04_l1_abi_state/PluginHost+PluginStateSession","verdict":verdict,"detail":detail,"observed":observed,"environment":environment});
    let _guard = evidence_lock().lock().unwrap();
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("tim04_l1_abi_state.jsonl"))
        .unwrap();
    writeln!(f, "{}", serde_json::to_string(&row).unwrap()).unwrap();
}
struct RfstBlob {
    gain: f64,
    checksum: u32,
}
fn parse_rfst(blob: &[u8]) -> RfstBlob {
    let framed = u32::from_le_bytes(blob[0..4].try_into().unwrap()) as usize;
    let stream =
        if (framed == 16 || framed == 20) && blob.len() >= 4 + framed && &blob[4..8] == b"RFST" {
            &blob[4..4 + framed]
        } else {
            blob
        };
    assert!(
        stream.len() == 16 || stream.len() == 20,
        "unexpected RFST length"
    );
    assert_eq!(&stream[0..4], b"RFST");
    assert_eq!(u32::from_le_bytes(stream[4..8].try_into().unwrap()), 2);
    let gain = f64::from_le_bytes(stream[8..16].try_into().unwrap());
    let checksum = if stream.len() == 20 {
        u32::from_le_bytes(stream[16..20].try_into().unwrap())
    } else {
        stream[8..16].iter().fold(0u32, |a, b| a + u32::from(*b))
    };
    RfstBlob { gain, checksum }
}
type RealSession = PluginStateSession<PluginStateHost, DirectoryPluginStateStore>;
fn apply(c: &mut ProjectController, op: ProjectOp) {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .unwrap();
}
fn add_track_and_strip(c: &mut ProjectController) -> (synvelis_project::TrackId, StripId) {
    apply(
        c,
        ProjectOp::AddTrack {
            name: "tim04-l1".into(),
        },
    );
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    (track, strip)
}
fn insert_processor(
    c: &mut ProjectController,
    strip: StripId,
    index: usize,
    plugin: PluginRef,
    name: &str,
) -> ProcessorId {
    apply(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index,
            name: name.into(),
            binding: ProcessorBinding::Resolved(plugin),
            sidechain_input: false,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}
fn project_with_processor(
    d: PluginDescriptor,
) -> (
    ProjectController,
    synvelis_project::TrackId,
    StripId,
    ProcessorId,
) {
    let mut c = ProjectController::new(
        Project::new(ProjectId::new(0), "tim04-l1"),
        Box::new(CounterIdGenerator::new(1)),
    );
    let (track, strip) = add_track_and_strip(&mut c);
    let p = insert_processor(&mut c, strip, 0, plugin_ref(d), "stateful");
    (c, track, strip, p)
}
fn temp_store_root(case: &str) -> PathBuf {
    env::temp_dir().join(format!("synvelis-tim04-l1-{case}-{}", std::process::id()))
}
fn session_for(
    root: &Path,
    host: Rc<RefCell<PluginHost>>,
    bindings: &[(ProcessorId, PluginHandle, PluginDescriptor)],
) -> RealSession {
    let mut port = PluginStateHost::new(host);
    for &(p, h, d) in bindings {
        port.bind(p, h, &plugin_ref(d), 1);
    }
    if root.exists() {
        fs::remove_dir_all(root).unwrap();
    }
    let store = DirectoryPluginStateStore::create(root).unwrap();
    PluginStateSession::new(port, store)
}
fn save_request(c: &ProjectController) -> PluginStateSaveRequest<'_> {
    PluginStateSaveRequest {
        project: c.live(),
        project_revision: c.live_revision().0,
        project_head: None,
        mode: CaptureMode::Coordinated,
        policy: StateQualityPolicy::Strict,
        stored_at: "tim04-l1".into(),
    }
}
fn save_strict(
    session: &mut RealSession,
    c: &ProjectController,
) -> synvelis_project::PluginStateReceipt {
    session.save(&save_request(c)).unwrap()
}
fn captured_digest(
    receipt: &synvelis_project::PluginStateReceipt,
    p: ProcessorId,
) -> (synvelis_project::ContentDigest, u64, StateCaptureQuality) {
    let record = receipt
        .records
        .iter()
        .find(|r| r.processor_id == p)
        .expect("processor record");
    match record.outcome {
        PluginStateOutcome::Captured {
            quality,
            blob_digest,
            blob_len,
        } => (blob_digest, blob_len, quality),
        ref other => panic!("save did not capture state: {other:?}"),
    }
}
fn blob_bytes(root: &Path, digest: &synvelis_project::ContentDigest) -> Vec<u8> {
    fs::read(root.join("blobs").join(digest_hex(digest))).expect("read captured RFST blob")
}
#[test]
fn l1_scan_lifecycle_and_capture_quality_matrix() {
    for format in [Format::Vst3, Format::Clap] {
        let (host, ds) = prepare_host(
            &[
                (format, FixtureKind::Stateful),
                (format, FixtureKind::Fault),
            ],
            Duration::from_secs(5),
        );
        let sd = ds[0];
        let fd = ds[1];
        let stateful = spawn_instance(&host, sd);
        let fault = spawn_instance(&host, fd);
        let (mut c, track, strip, sp) = project_with_processor(sd);
        let root = temp_store_root("matrix");
        let fp = insert_processor(&mut c, strip, 1, plugin_ref(fd), "fault");
        assert_ne!(sp, fp);
        let _ = track;
        let mut session = session_for(
            &root,
            host.clone(),
            &[(sp, stateful.handle, sd), (fp, fault.handle, fd)],
        );
        let processing = session
            .port_mut()
            .capture(sp, CaptureMode::Coordinated)
            .unwrap();
        assert_eq!(processing.achieved, StateCaptureQuality::BestEffort);
        assert!(processing.observation.processing);
        parse_rfst(&processing.blob);
        write_evidence(
            &format!("{}_stateful_processing_capture", format.label()),
            "Pass",
            "processing capture remains BestEffort",
            &environment(format, sd, 5000),
            &json!({"quality":format!("{:?}",processing.achieved),"blob_len":processing.blob.len(),"blob_sha256":sha256_hex(&processing.blob),"observation":format!("{:?}",processing.observation)}),
        );
        host.borrow_mut()
            .execute(HostCommand::StopProcessing {
                handle: stateful.handle,
            })
            .unwrap();
        let stopped = session
            .port_mut()
            .capture(sp, CaptureMode::Coordinated)
            .unwrap();
        assert_eq!(stopped.achieved, StateCaptureQuality::Coordinated);
        assert!(!stopped.observation.processing);
        assert!(stopped.observation.stable_boundary);
        parse_rfst(&stopped.blob);
        write_evidence(
            &format!("{}_stateful_stopped_capture", format.label()),
            "Pass",
            "stable stopped boundary captures Coordinated",
            &environment(format, sd, 5000),
            &json!({"quality":format!("{:?}",stopped.achieved),"blob_len":stopped.blob.len(),"blob_sha256":sha256_hex(&stopped.blob),"observation":format!("{:?}",stopped.observation)}),
        );
        let fault_err = session
            .port_mut()
            .capture(fp, CaptureMode::Coordinated)
            .unwrap_err();
        assert_eq!(
            fault_err,
            synvelis_project::PluginStateCaptureError::Unsupported
        );
        write_evidence(
            &format!("{}_fault_state_capability", format.label()),
            "Unsupported",
            "test_fault honestly exposes no state capability",
            &environment(format, fd, 5000),
            &json!({"capture_error":format!("{fault_err:?}")}),
        );
    }
}

#[test]
fn l1_stateful_save_mutate_restore_save_roundtrip() {
    for format in [Format::Vst3, Format::Clap] {
        let (host, ds) = prepare_host(&[(format, FixtureKind::Stateful)], Duration::from_secs(5));
        let sd = ds[0];
        let instance = spawn_instance(&host, sd);
        let (c, _track, _strip, processor) = project_with_processor(sd);
        let root = temp_store_root("roundtrip");
        let mut session = session_for(&root, host.clone(), &[(processor, instance.handle, sd)]);
        host.borrow_mut()
            .execute(HostCommand::StopProcessing {
                handle: instance.handle,
            })
            .unwrap();
        let initial = save_strict(&mut session, &c);
        let (initial_digest, initial_len, initial_quality) = captured_digest(&initial, processor);
        let initial_bytes = blob_bytes(&root, &initial_digest);
        let parsed = parse_rfst(&initial_bytes);
        assert_eq!(parsed.gain, 1.0);
        assert_eq!(initial_len, initial_bytes.len() as u64);
        assert!(matches!(
            initial_quality,
            StateCaptureQuality::Coordinated | StateCaptureQuality::Quiescent
        ));
        host.borrow_mut()
            .execute(HostCommand::StartProcessing {
                handle: instance.handle,
            })
            .unwrap();
        let resend = submit_event_until_landed(&host, instance.handle, 0.375);
        let (seen, calls, substituted) =
            wait_for_gain_output(&host, instance.handle, 0.375, -0.1875);
        assert!(seen, "delivered audio never showed the x0.75 mutation");
        host.borrow_mut()
            .execute(HostCommand::StopProcessing {
                handle: instance.handle,
            })
            .unwrap();
        let mutated = save_strict(&mut session, &c);
        let (mutated_digest, _, _) = captured_digest(&mutated, processor);
        let mutated_bytes = blob_bytes(&root, &mutated_digest);
        assert_ne!(mutated_bytes, initial_bytes);
        assert_eq!(parse_rfst(&mutated_bytes).gain, 0.75);
        session
            .restore(&PluginStateRestoreRequest {
                project: c.live(),
                manifest: initial.manifest_name.clone(),
            })
            .unwrap();
        host.borrow_mut()
            .execute(HostCommand::StartProcessing {
                handle: instance.handle,
            })
            .unwrap();
        let (restored_seen, restore_calls, restore_substituted) =
            wait_for_gain_output(&host, instance.handle, 0.5, -0.25);
        assert!(
            restored_seen,
            "delivered audio never showed the restored unity gain"
        );
        host.borrow_mut()
            .execute(HostCommand::StopProcessing {
                handle: instance.handle,
            })
            .unwrap();
        let resaved = save_strict(&mut session, &c);
        let (resaved_digest, _, _) = captured_digest(&resaved, processor);
        let resaved_bytes = blob_bytes(&root, &resaved_digest);
        assert_eq!(resaved_bytes, initial_bytes);
        assert_eq!(resaved_digest, initial_digest);
        write_evidence(
            &format!("{}_stateful_save_restore_roundtrip", format.label()),
            "Pass",
            "save mutate restore save returns to initial blob bytes and digest",
            &environment(format, sd, 5000),
            &json!({"initial":{"len":initial_bytes.len(),"sha256":sha256_hex(&initial_bytes),"gain":parse_rfst(&initial_bytes).gain,"checksum":parse_rfst(&initial_bytes).checksum,"receipt_digest":digest_hex(&initial_digest),"receipt_len":initial_len},"mutated":{"len":mutated_bytes.len(),"sha256":sha256_hex(&mutated_bytes),"gain":parse_rfst(&mutated_bytes).gain,"receipt_digest":digest_hex(&mutated_digest)},"resaved":{"sha256":sha256_hex(&resaved_bytes),"receipt_digest":digest_hex(&resaved_digest)},"ordering":{"mutation_resend_attempt":resend,"matched_call":calls,"substituted_calls":substituted,"restore_matched_call":restore_calls,"restore_substituted_calls":restore_substituted},"manifest_receipt":{"manifest_name":initial.manifest_name,"manifest_digest":format!("{:?}",initial.manifest_digest)}}),
        );
    }
}
#[expect(
    clippy::too_many_lines,
    reason = "the fault matrix keeps each mode's setup, oracle, and evidence in one consumer-run sequence"
)]
#[test]
fn l1_fault_projection_and_old_blob_protection() {
    for format in [Format::Vst3, Format::Clap] {
        for (mode, normalized, expected) in [
            ("abort", 0.125, PluginStateFailureKind::Crashed),
            ("exit42", 0.3025, PluginStateFailureKind::Crashed),
            ("hang", 0.375, PluginStateFailureKind::Timeout),
        ] {
            let timeout = Duration::from_millis(500);
            let (host, ds) = prepare_host(
                &[
                    (format, FixtureKind::Stateful),
                    (format, FixtureKind::Fault),
                ],
                timeout,
            );
            let sd = ds[0];
            let fd = ds[1];
            let stateful = spawn_instance(&host, sd);
            let fault = spawn_instance(&host, fd);
            let (mut c, _track, strip, sp) = project_with_processor(sd);
            let root = temp_store_root("fault");
            let mut session = session_for(&root, host.clone(), &[(sp, stateful.handle, sd)]);
            host.borrow_mut()
                .execute(HostCommand::StopProcessing {
                    handle: stateful.handle,
                })
                .unwrap();
            let initial = save_strict(&mut session, &c);
            let (initial_digest, _, _) = captured_digest(&initial, sp);
            let blob_path = root.join("blobs").join(digest_hex(&initial_digest));
            let before_blob = fs::read(&blob_path).unwrap();
            let before_head = fs::read_to_string(root.join("HEAD")).unwrap();
            let before_manifests: BTreeSet<_> = fs::read_dir(root.join("manifests"))
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect();
            assert_eq!(initial.records.len(), 1);
            assert_eq!(digest_hex(&initial_digest), sha256_hex(&before_blob));
            let fp = insert_processor(&mut c, strip, 1, plugin_ref(fd), "fault");
            session
                .port_mut()
                .bind(fp, fault.handle, &plugin_ref(fd), 1);
            let arm_attempt = arm_fault(&host, fault.handle, normalized);
            if mode == "hang" {
                for _ in 0..6 {
                    submit_block(&host, fault.handle, EventList::empty(FRAMES_U32));
                }
            } else {
                assert!(
                    wait_failed(&host, fault.handle),
                    "fault child did not converge to Failed for {mode}"
                );
            }
            let capture_err = session
                .port_mut()
                .capture(fp, CaptureMode::Coordinated)
                .unwrap_err();
            let capture_kind = match capture_err {
                synvelis_project::PluginStateCaptureError::Failed(kind) => kind,
                other @ synvelis_project::PluginStateCaptureError::Unsupported => {
                    panic!("unexpected capture error {other:?}")
                }
            };
            assert_eq!(
                capture_kind, expected,
                "{mode} must project to {expected:?}"
            );
            assert!(
                wait_failed(&host, fault.handle),
                "fault child did not converge to Failed after the state command for {mode}"
            );
            let restore_err = session.port_mut().restore(fp, &before_blob).unwrap_err();
            let restore_kind = match restore_err {
                synvelis_project::PluginStateCaptureError::Failed(kind) => kind,
                other @ synvelis_project::PluginStateCaptureError::Unsupported => {
                    panic!("unexpected restore error {other:?}")
                }
            };
            assert_eq!(
                restore_kind, expected,
                "faulted instance restore must fail closed"
            );
            let notification = session.port().failure_notification(fault.handle);
            let diagnostic = host.borrow().diagnostics_for_handle(fault.handle);
            let save_err = session.save(&save_request(&c)).unwrap_err();
            let summary = match save_err {
                PluginStateError::QualityRejected { summary } => summary,
                other => panic!("strict save must be quality-rejected, got {other:?}"),
            };
            let after_blob = fs::read(&blob_path).unwrap();
            let after_head = fs::read_to_string(root.join("HEAD")).unwrap();
            let after_manifests: BTreeSet<_> = fs::read_dir(root.join("manifests"))
                .unwrap()
                .map(|e| e.unwrap().file_name())
                .collect();
            assert_eq!(after_blob, before_blob, "old blob bytes changed");
            assert_eq!(sha256_hex(&after_blob), digest_hex(&initial_digest));
            assert_eq!(after_head, before_head, "HEAD changed");
            assert_eq!(
                after_manifests, before_manifests,
                "a new manifest published"
            );
            let (exit_text, cause_text) = match notification {
                Some(HostNotification::InstanceFailed { exit, cause, .. }) => {
                    (format!("{exit:?}"), format!("{cause:?}"))
                }
                other => (format!("{other:?}"), "missing".into()),
            };
            write_evidence(
                &format!("{}_fault_{mode}_projection_old_blob", format.label()),
                "Pass",
                "fault projects typed failure and strict save protects the old blob",
                &environment(format, fd, 500),
                &json!({"fault_mode":mode,"normalized":normalized,"arm_attempt":arm_attempt,"capture_failure_kind":format!("{capture_kind:?}"),"restore_failure_kind":format!("{restore_kind:?}"),"notification_exit":exit_text,"failure_cause":cause_text,"diagnostics":diagnostic.map(|r|json!({"diagnostics_id":r.diagnostics_id,"phase":format!("{:?}",r.phase),"cause":format!("{:?}",r.cause)})),"strict_rejection_summary":format!("{summary:?}"),"old_blob":{"sha256":digest_hex(&initial_digest),"len":before_blob.len(),"bytes_hex":hex_encode(&before_blob)},"head_unchanged":after_head==before_head,"manifest_set_unchanged":after_manifests==before_manifests}),
            );
        }
    }
}
