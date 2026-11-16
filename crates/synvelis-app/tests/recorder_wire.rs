//! synvelis#114 SV-GUI-05 Option B — `syn.recorder.*` lifecycle wire
//! headless E2E.
//!
//! Every case drives the same `syn.*` string dispatch the product
//! host uses: `UiRuntime::dispatch_op` → `Service::call` →
//! `AppService` → `SynvelisApp` → `RecorderOwner` →
//! `synvelis-recording` (`preflight`/`InputSession`/`StagingArea`/
//! `StreamWriter`/`commit_session`). Input devices are test-local
//! fake `Device`/`InputStream` implementations injected through
//! `SynvelisApp::set_recorder_devices` — the same `&dyn Device` seam
//! a later slice will feed from real enumeration.
//!
//! Boundary: this is headless evidence only. It proves the app-layer
//! wire (ops registration, lifecycle ordering, live projections,
//! capabilities/close-veto consistency) — it does NOT prove real
//! device enumeration, real OS audio callbacks, real GUI windows,
//! or product acceptance. Those lanes stay `NotRun`.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use ruviform_audio_core::device::{
    BufferSize, DEVICE_CLOCK_TIMEBASE, Device, DeviceError, DeviceId, DeviceResult,
    InputCallbackInfo, InputDataCallback, InputErrorCallback, InputStream, InputStreamConfig,
    InputTimestamp, OutputDataCallback, OutputErrorCallback, OutputStream, OutputStreamConfig,
    StreamLatency, SupportedInputFormat, SupportedOutputFormat,
};
use ruviform_audio_core::{
    AudioBufferView, AudioFormat, ChannelLayout, SampleFormat, SampleRate, SpeakerMask,
};
use ruviform_media_core::{ClockDomainId, MediaTime};
use ruviform_ui::ipc::{ErrorBody, Event};
use ruviform_ui::{
    BackendReport, CallOutcome, DataRegistry, OpInfo, Service, ServiceContext, UiRuntime,
};

use synvelis_app::{AppService, SynvelisApp};
use synvelis_project::{
    ProjectOp, SessionFrame, SessionRange, SourceFrame, SourceId, SourceRange, SourceState, TakeId,
};
use synvelis_recording::{MockPcm, RecordingOp, SynvelisOp};

// ---------- test-local fake device port ----------

const RATE: u32 = 48_000;
/// fake slab の 1 channel あたり frame 上限。
const FRAMES_MAX: usize = 4096;

/// channel 数に対応する `ChannelLayout`（既知 layout 以外は custom mask）。
fn layout_for(channels: u32) -> ChannelLayout {
    match channels {
        1 => ChannelLayout::Mono,
        2 => ChannelLayout::Stereo,
        4 => ChannelLayout::Quad,
        6 => ChannelLayout::FivePointOne,
        8 => ChannelLayout::SevenPointOne,
        n if n < 32 => ChannelLayout::custom(SpeakerMask::from_bits((1_u32 << n) - 1))
            .unwrap_or(ChannelLayout::Mono),
        _ => ChannelLayout::custom(SpeakerMask::from_bits(u32::MAX)).unwrap_or(ChannelLayout::Mono),
    }
}

fn channels_of(format: AudioFormat) -> usize {
    usize::try_from(format.layout.channels()).unwrap_or(0)
}

/// scripted fake stream の共有状態（device handle と stream handle の双方から触る）。
struct FakeStreamShared {
    data_cb: Option<InputDataCallback>,
    err_cb: Option<InputErrorCallback>,
    domain: ClockDomainId,
    format: AudioFormat,
    /// 次に供給する device input frame 番号。
    position: u64,
    /// planar PCM slab（channel-major）。
    slab: Vec<f32>,
    pcm: MockPcm,
    started: bool,
    stop_calls: u32,
}

impl FakeStreamShared {
    /// `frames` 分の deterministic planar PCM を callback へ供給する
    /// （`frame_position` 追従・capture stamp 無し — position のみの
    /// placement 経路）。
    fn deliver(&mut self, frames: u32) {
        let frames_usize = usize::try_from(frames).expect("frames");
        assert!(frames_usize > 0 && frames_usize <= FRAMES_MAX);
        let ch = channels_of(self.format);
        let base = self.position;
        for c in 0..ch {
            for f in 0..frames_usize {
                self.slab[c * frames_usize + f] = self.pcm.sample(
                    base.wrapping_add(u64::try_from(f).unwrap_or(u64::MAX)),
                    u32::try_from(c).unwrap_or(u32::MAX),
                );
            }
        }
        let info = InputCallbackInfo {
            timestamp: InputTimestamp {
                callback: MediaTime::new(0, DEVICE_CLOCK_TIMEBASE),
                capture: None,
            },
            clock_domain: self.domain,
            frame_position: Some(self.position),
        };
        let view = AudioBufferView::try_new(&self.slab[..ch * frames_usize], ch, frames_usize)
            .expect("view");
        if let Some(cb) = &mut self.data_cb {
            cb(view, info);
        }
        self.position = self.position.wrapping_add(u64::from(frames));
    }

    /// error callback へ `DeviceError` を注入する。
    fn raise(&mut self, err: DeviceError) {
        if let Some(cb) = &mut self.err_cb {
            cb(err);
        }
    }

    fn is_started(&self) -> bool {
        self.started
    }

    fn stop_calls(&self) -> u32 {
        self.stop_calls
    }
}

/// scripted fake `InputStream`。
struct FakeInputStream {
    shared: Arc<Mutex<FakeStreamShared>>,
}

impl InputStream for FakeInputStream {
    fn start(&self) -> DeviceResult<()> {
        self.shared.lock().expect("lock").started = true;
        Ok(())
    }

    fn stop(&self) -> DeviceResult<()> {
        let mut g = self.shared.lock().expect("lock");
        g.started = false;
        g.stop_calls += 1;
        Ok(())
    }

    fn format(&self) -> AudioFormat {
        self.shared.lock().expect("lock").format
    }

    fn buffer_size(&self) -> DeviceResult<BufferSize> {
        Ok(BufferSize::Default)
    }

    fn now(&self) -> DeviceResult<MediaTime> {
        Ok(MediaTime::new(0, DEVICE_CLOCK_TIMEBASE))
    }

    fn clock_domain(&self) -> ClockDomainId {
        self.shared.lock().expect("lock").domain
    }

    fn input_latency(&self) -> DeviceResult<Option<StreamLatency>> {
        Ok(None)
    }
}

/// scripted fake device の共有状態。
struct FakeDeviceShared {
    format: AudioFormat,
    domain: ClockDomainId,
    streams: Vec<Arc<Mutex<FakeStreamShared>>>,
}

/// scripted fake `Device`（input のみ実装、output は `Unsupported`）。
/// `synvelis-recording` の test fixture と同じ port 契約の
/// test-local 実装 — 別 crate の test 資産は再利用できないため
/// 本 crate 側に最小限を持つ。
#[derive(Clone)]
struct FakeInputDevice {
    shared: Arc<Mutex<FakeDeviceShared>>,
}

impl FakeInputDevice {
    /// `channels` ch の fake input device を作る。
    fn new(channels: u32) -> Self {
        Self {
            shared: Arc::new(Mutex::new(FakeDeviceShared {
                format: AudioFormat {
                    sample_rate: SampleRate::new(RATE).expect("rate"),
                    layout: layout_for(channels),
                    sample_format: SampleFormat::F32,
                },
                domain: ClockDomainId::MONOTONIC,
                streams: Vec::new(),
            })),
        }
    }

    /// open 済み stream への scripted handle（test 駆動用）。
    /// `syn.recorder.start` が `open_input_stream` を呼んだ後にのみ
    /// 有効。
    fn stream(&self, index: usize) -> Arc<Mutex<FakeStreamShared>> {
        self.shared.lock().expect("lock").streams[index].clone()
    }

    fn stream_count(&self) -> usize {
        self.shared.lock().expect("lock").streams.len()
    }
}

impl Device for FakeInputDevice {
    fn name(&self) -> DeviceResult<String> {
        Ok("fake-input".to_owned())
    }

    fn id(&self) -> DeviceResult<Option<DeviceId>> {
        Ok(Some(DeviceId::new("fake:0".to_owned())))
    }

    fn default_output_format(&self) -> DeviceResult<AudioFormat> {
        Err(DeviceError::Unsupported)
    }

    fn supported_output_formats(&self) -> DeviceResult<Vec<SupportedOutputFormat>> {
        Ok(Vec::new())
    }

    fn open_output_stream(
        &self,
        _config: &OutputStreamConfig,
        _data_callback: OutputDataCallback,
        _error_callback: OutputErrorCallback,
    ) -> DeviceResult<Box<dyn OutputStream>> {
        Err(DeviceError::Unsupported)
    }

    fn default_input_format(&self) -> DeviceResult<AudioFormat> {
        Ok(self.shared.lock().expect("lock").format)
    }

    fn supported_input_formats(&self) -> DeviceResult<Vec<SupportedInputFormat>> {
        Ok(Vec::new())
    }

    fn open_input_stream(
        &self,
        config: &InputStreamConfig,
        data_callback: InputDataCallback,
        error_callback: InputErrorCallback,
    ) -> DeviceResult<Box<dyn InputStream>> {
        let channels = channels_of(config.format);
        let mut g = self.shared.lock().expect("lock");
        let shared = Arc::new(Mutex::new(FakeStreamShared {
            data_cb: Some(data_callback),
            err_cb: Some(error_callback),
            domain: g.domain,
            format: config.format,
            position: 0,
            slab: vec![0.0; channels.saturating_mul(FRAMES_MAX)],
            pcm: MockPcm { seed: 7 },
            started: false,
            stop_calls: 0,
        }));
        g.streams.push(Arc::clone(&shared));
        Ok(Box::new(FakeInputStream { shared }))
    }
}

// ---------- shared headless harness ----------

/// Shares one [`AppService`] between the headless [`UiRuntime`]
/// (real `syn.*` string dispatch) and the test (fixture mutation via
/// `app_mut`). `UiRuntime` never exposes a registered service back,
/// so the service is wrapped in `Rc<RefCell>` and re-registered
/// through a delegating [`Service`] impl.
struct SharedService(Rc<RefCell<AppService>>);

impl Service for SharedService {
    fn ops(&self) -> &'static [OpInfo] {
        self.0.borrow().ops()
    }

    fn call(
        &mut self,
        op: &str,
        params: &Value,
        ctx: &mut ServiceContext<'_>,
    ) -> Result<CallOutcome, ErrorBody> {
        self.0.borrow_mut().call(op, params, ctx)
    }
}

struct Fixture {
    rt: UiRuntime,
    svc: Rc<RefCell<AppService>>,
}

fn fixture() -> Fixture {
    let app = SynvelisApp::new(BackendReport::available("webview2", "test-1.0".into()));
    let svc = Rc::new(RefCell::new(AppService::new(app)));
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(SharedService(svc.clone()))
        .expect("register AppService");
    Fixture { rt, svc }
}

fn call(rt: &mut UiRuntime, op: &str, params: Value) -> (bool, Option<Value>, Option<ErrorBody>) {
    let events = rt.dispatch_op("test-1", op, params);
    let result = events.iter().find_map(|e| match e {
        Event::Result {
            ok, data, error, ..
        } => Some((*ok, data.clone(), error.clone())),
        _ => None,
    });
    result.expect("every intent yields exactly one result event")
}

fn ok(rt: &mut UiRuntime, op: &str, params: Value) -> Value {
    let (ok, data, error) = call(rt, op, params);
    assert!(ok, "{op} failed: {error:?}");
    data.expect("successful result carries data")
}

fn err(rt: &mut UiRuntime, op: &str, params: Value) -> ErrorBody {
    let (ok, data, error) = call(rt, op, params);
    assert!(!ok, "{op} unexpectedly succeeded: {data:?}");
    assert!(data.is_none(), "{op} error must not carry data");
    error.expect("failed result carries an error")
}

// ---------- fixture mutation through the same app ----------

fn with_live<R>(f: &Fixture, g: impl FnOnce(&synvelis_project::Project) -> R) -> R {
    let svc = f.svc.borrow();
    g(svc.app().workspace().session().controller().live())
}

fn apply_synvelis(f: &Fixture, op: SynvelisOp) {
    let mut svc = f.svc.borrow_mut();
    let session = svc.app_mut().workspace_mut().session_mut();
    let rev = session.revision();
    session.apply(op, rev).expect("fixture op applies");
}

fn apply_project(f: &Fixture, op: ProjectOp) {
    apply_synvelis(f, SynvelisOp::Project(op));
}

fn apply_recording(f: &Fixture, op: RecordingOp) {
    apply_synvelis(f, SynvelisOp::Recording(op));
}

fn project_new(f: &mut Fixture, dir: &std::path::Path) -> Value {
    let bundle = dir.join("project");
    ok(
        &mut f.rt,
        "syn.project.new",
        json!({ "path": bundle.to_string_lossy(), "name": "recorder-wire" }),
    )
}

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("synvelis-recwire-{}-{name}", std::process::id()));
    if d.exists() {
        std::fs::remove_dir_all(&d).unwrap();
    }
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ---------- domain builders ----------

fn session_frames(start: i64, end: i64) -> SessionRange {
    SessionRange::try_new(
        SessionFrame::new(start).unwrap(),
        SessionFrame::new(end).unwrap(),
    )
    .unwrap()
}

fn source_frames(start: i64, end: i64) -> SourceRange {
    SourceRange::try_new(
        SourceFrame::new(start).unwrap(),
        SourceFrame::new(end).unwrap(),
    )
    .unwrap()
}

/// track + lane + clock epoch + committed media source — the minimum
/// substrate `preflight` (lane/epoch resolution) and take seeding
/// (segment `source_id`) require.
fn seed_substrate(f: &Fixture) {
    apply_project(f, ProjectOp::AddTrack { name: "rec".into() });
    let track = with_live(f, |p| *p.tracks.keys().next().unwrap());
    apply_project(f, ProjectOp::AddLane { track_id: track });
    apply_project(
        f,
        ProjectOp::RegisterClockEpoch {
            description: "mock".into(),
        },
    );
    apply_project(
        f,
        ProjectOp::RegisterAsset {
            digest: "aa".into(),
        },
    );
    let asset = with_live(f, |p| *p.assets.keys().next().unwrap());
    apply_project(
        f,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "aa".into(),
        },
    );
    let source = with_live(f, |p| *p.sources.keys().next().unwrap());
    for state in [SourceState::Verified, SourceState::Committed] {
        apply_project(
            f,
            ProjectOp::SetSourceState {
                source_id: source,
                state,
            },
        );
    }
}

fn first_lane(f: &Fixture) -> synvelis_project::LaneId {
    with_live(f, |p| *p.lanes.keys().next().unwrap())
}

/// An `Open` recording session carrying one `Raw` take — the
/// `syn.recorder.mark_take_verified` positive substrate (committed
/// sessions are `Finalized`, where the domain rejects the op).
fn seed_open_take(f: &Fixture) -> TakeId {
    let lane = first_lane(f);
    let source: SourceId = with_live(f, |p| *p.sources.keys().next().unwrap());
    let epoch = with_live(f, |p| *p.clock_epochs.keys().next().unwrap());
    apply_recording(
        f,
        RecordingOp::CreateSession {
            name: "open-take".into(),
        },
    );
    let session_id = with_live(f, |p| *p.recording.sessions.keys().next().unwrap());
    apply_recording(f, RecordingOp::OpenSession { session_id });
    apply_recording(
        f,
        RecordingOp::RegisterPass {
            session_id,
            index: 0,
        },
    );
    let pass_id = with_live(f, |p| *p.recording.passes.keys().next().unwrap());
    apply_recording(
        f,
        RecordingOp::RegisterStream {
            session_id,
            lane_id: lane,
            clock_epoch_id: epoch,
        },
    );
    let stream_id = with_live(f, |p| *p.recording.streams.keys().next().unwrap());
    apply_recording(
        f,
        RecordingOp::RegisterSegment {
            stream_id,
            pass_id,
            session_range: session_frames(0, 30_000),
            source_range: source_frames(0, 30_000),
            source_id: source,
        },
    );
    let seg = with_live(f, |p| *p.recording.segments.keys().last().unwrap());
    apply_recording(
        f,
        RecordingOp::RegisterTake {
            session_id,
            pass_id,
            stream_id,
            lane_id: lane,
            source_id: source,
            segment_ids: vec![seg],
        },
    );
    with_live(f, |p| *p.recording.takes.keys().last().unwrap())
}

// ---------- syn.recorder.* params ----------

/// `syn.recorder.start` の標準 params — mono target on `lane`、
/// stereo fake device の channel 0 を mono で capture。
fn start_params(lane: synvelis_project::LaneId, staging_root: &std::path::Path) -> Value {
    json!({
        "name": "wire-session",
        "transport_start": 0,
        "stop": 3840,
        "sample_rate": RATE,
        "staging_root": staging_root.to_string_lossy(),
        "targets": [{ "lane": lane.to_tagged(), "channels": 1 }],
        "streams": [{
            "channels": 2,
            "taps": [{ "stream_index": 0, "channel_map": [0] }],
        }],
    })
}

fn inject_device(f: &Fixture, channels: u32) -> FakeInputDevice {
    let device = FakeInputDevice::new(channels);
    f.svc
        .borrow_mut()
        .app_mut()
        .set_recorder_devices(vec![Box::new(device.clone())]);
    device
}

fn recorder_status(f: &mut Fixture) -> Value {
    ok(&mut f.rt, "syn.recorder.status", json!({}))
}

fn recording_info(f: &mut Fixture) -> Value {
    ok(&mut f.rt, "syn.recording.info", json!({}))
}

fn close_guard_reasons(f: &mut Fixture) -> Vec<String> {
    let data = ok(&mut f.rt, "syn.window.close_guard", json!({}));
    data["reasons"]
        .as_array()
        .expect("reasons array")
        .iter()
        .map(|r| r.as_str().expect("reason string").to_owned())
        .collect()
}

fn capabilities_recorder(f: &mut Fixture) -> Value {
    let data = ok(&mut f.rt, "syn.app.capabilities", json!({}));
    data["recorder"].clone()
}

// ---------- cases ----------

/// OPS 登録・初期投影・device 未注入の honest unavailable。
#[test]
fn recorder_wire_ops_registered_and_idle_without_device() {
    let mut f = fixture();
    // OPS 一覧に syn.recorder.* が登録されている。
    let ops = f.svc.borrow().ops();
    for name in [
        "syn.recorder.start",
        "syn.recorder.pump",
        "syn.recorder.stop",
        "syn.recorder.commit",
        "syn.recorder.discard",
        "syn.recorder.status",
        "syn.recorder.mark_take_verified",
    ] {
        assert!(
            ops.iter().any(|o| o.name == name),
            "{name} missing from OPS"
        );
    }
    // 初期状態は idle・device 0。
    let status = recorder_status(&mut f);
    assert_eq!(status["state"], "idle");
    assert_eq!(status["active"], false);
    assert_eq!(status["devices"], 0);
    // device 未注入は unavailable + reason（unavailable/idle 固定の
    // stub ではなく実態投影）。
    let cap = capabilities_recorder(&mut f);
    assert_eq!(cap["state"], "unavailable");
    assert_eq!(cap["status"], "idle");
    assert_eq!(cap["active"], false);
    assert!(cap["reason"].as_str().is_some_and(|r| !r.is_empty()));
    // session 無しの lifecycle op は typed error。
    err(&mut f.rt, "syn.recorder.pump", json!({}));
    err(&mut f.rt, "syn.recorder.stop", json!({}));
    err(&mut f.rt, "syn.recorder.commit", json!({}));
    err(&mut f.rt, "syn.recorder.discard", json!({}));
    err(
        &mut f.rt,
        "syn.recorder.mark_take_verified",
        json!({ "take_id": "take-0" }),
    );
}

/// device 未注入での `syn.recorder.start` は `NoDevice` →
/// `invalid_params` エラー。project を汚さない。
#[test]
fn recorder_wire_start_without_device_is_typed_error() {
    let dir = tmp_dir("nodevice");
    let mut f = fixture();
    project_new(&mut f, &dir);
    seed_substrate(&f);
    let lane = first_lane(&f);
    let staging = tmp_dir("staging-nodevice");
    let e = err(
        &mut f.rt,
        "syn.recorder.start",
        start_params(lane, &staging),
    );
    assert!(
        e.message.contains("no recorder input device"),
        "unexpected error: {e:?}"
    );
    // 失敗後も idle・project に session は残らない。
    assert_eq!(recorder_status(&mut f)["state"], "idle");
    let info = recording_info(&mut f);
    assert_eq!(info["counts"]["sessions"], 0);
}

/// preflight→open/add→writer/staging→start→pump→stop→commit の
/// 全 lifecycle を `UiRuntime` dispatch で駆動し、live 投影・
/// capabilities・close-veto が同じ owner を読むことを pin する。
#[test]
#[allow(clippy::too_many_lines)] // sequential e2e scenario
fn recorder_wire_full_lifecycle_headless() {
    let dir = tmp_dir("lifecycle");
    let mut f = fixture();
    project_new(&mut f, &dir);
    seed_substrate(&f);
    let lane = first_lane(&f);
    let device = inject_device(&f, 2);
    let staging = tmp_dir("staging-lifecycle");

    // capabilities: device 注入後は available（session はまだ idle）。
    let cap = capabilities_recorder(&mut f);
    assert_eq!(cap["state"], "available");
    assert_eq!(cap["status"], "idle");
    assert_eq!(cap["devices"], 1);

    // --- start ---
    let started = ok(
        &mut f.rt,
        "syn.recorder.start",
        start_params(lane, &staging),
    );
    let session_tag = started["session_id"]
        .as_str()
        .expect("session_id tagged")
        .to_owned();
    assert_eq!(started["phase"], "recording");
    assert_eq!(device.stream_count(), 1, "one input stream opened");
    {
        let stream = device.stream(0);
        let g = stream.lock().expect("stream");
        assert!(g.is_started(), "recorder.start must start the stream");
    }

    // live 投影: recorder owner + persisted session/stream。
    let status = recorder_status(&mut f);
    assert_eq!(status["state"], "recording");
    assert_eq!(status["active"], true);
    assert_eq!(status["session_id"], session_tag.as_str());
    assert_eq!(status["devices"], 1);
    let info = recording_info(&mut f);
    assert_eq!(info["recorder"]["status"], "recording");
    assert_eq!(info["recorder"]["active"], true);
    assert_eq!(info["counts"]["sessions"], 1);
    assert_eq!(info["counts"]["streams"], 1);
    assert_eq!(info["sessions"][0]["state"], "open");
    assert_eq!(info["sessions"][0]["id"], session_tag.as_str());

    // capabilities + close veto が同じ owner を読む。
    let cap = capabilities_recorder(&mut f);
    assert_eq!(cap["state"], "available");
    assert_eq!(cap["status"], "recording");
    assert_eq!(cap["active"], true);
    assert!(
        close_guard_reasons(&mut f).contains(&"recorder_active".to_owned()),
        "active recorder session must veto close"
    );

    // Busy: 稼働中の二重 start は typed error。
    err(
        &mut f.rt,
        "syn.recorder.start",
        start_params(lane, &staging),
    );

    // --- capture: fake deliver → pump ---
    {
        let stream = device.stream(0);
        let mut g = stream.lock().expect("stream");
        g.deliver(512);
        g.deliver(512);
    }
    let pumped = ok(&mut f.rt, "syn.recorder.pump", json!({}));
    assert_eq!(pumped["faulted"], false);

    // --- stop: stream 停止 + finalize ---
    let stopped = ok(&mut f.rt, "syn.recorder.stop", json!({}));
    assert_eq!(stopped["phase"], "stopped");
    assert_eq!(stopped["finalized"], 1);
    {
        let stream = device.stream(0);
        let g = stream.lock().expect("stream");
        assert!(!g.is_started(), "stop must stop the stream");
        assert_eq!(g.stop_calls(), 1);
    }
    let status = recorder_status(&mut f);
    assert_eq!(status["state"], "stopped");
    assert_eq!(status["active"], true, "stopped session still owns media");
    // 位相違反: Recording 中のみ pump、Stopped のみ commit。
    err(&mut f.rt, "syn.recorder.pump", json!({}));
    err(&mut f.rt, "syn.recorder.stop", json!({}));

    // --- commit: CloseSession + commit_session → Finalized ---
    let committed = ok(&mut f.rt, "syn.recorder.commit", json!({}));
    assert_eq!(committed["session_id"], session_tag.as_str());
    assert_eq!(committed["outcome"], "applied");
    assert!(
        committed["takes_committed"].as_u64().unwrap_or(0) >= 1,
        "commit must register takes: {committed:?}"
    );

    // commit 後は idle・session は Finalized。commit は finalized
    // media を検証済み take として登録する（`commit_session` の
    // payload は `TakeState::Verified` で構築される — 手動の
    // Raw→Verified は `syn.recorder.mark_take_verified` が担う）。
    let status = recorder_status(&mut f);
    assert_eq!(status["state"], "idle");
    assert_eq!(status["active"], false);
    assert_eq!(status["session_id"], Value::Null);
    let info = recording_info(&mut f);
    assert_eq!(info["recorder"]["status"], "idle");
    assert_eq!(info["recorder"]["active"], false);
    assert_eq!(info["sessions"][0]["state"], "finalized");
    assert!(
        info["takes"]
            .as_array()
            .is_some_and(|t| t.iter().any(|t| t["state"] == "verified")),
        "committed take must be verified: {info:?}"
    );
    // close veto は recorder 分が消える（unsaved_changes は残り得る）。
    assert!(
        !close_guard_reasons(&mut f).contains(&"recorder_active".to_owned()),
        "committed session must release recorder veto"
    );
    // capabilities は device 保持のまま available・idle。
    let cap = capabilities_recorder(&mut f);
    assert_eq!(cap["state"], "available");
    assert_eq!(cap["status"], "idle");
    assert_eq!(cap["active"], false);

    // Finalized session の take（既に Verified）は domain が拒否
    // する — `mark_take_verified` は Open|Closed session の
    // Raw→Verified のみ。
    let take_id = info["takes"][0]["id"].as_str().expect("take id");
    err(
        &mut f.rt,
        "syn.recorder.mark_take_verified",
        json!({ "take_id": take_id }),
    );
}

/// device error callback → `FaultLatch` → `syn.recorder.pump` の
/// `faulted` 投影 → `syn.recorder.stop` が capture-fault marker を
/// staging に記録する経路。fault は fail-stop ではなく latch で
/// 観測され、stop/commit の typed 経路は維持される。
#[test]
fn recorder_wire_device_fault_latches_and_marks() {
    let dir = tmp_dir("fault");
    let mut f = fixture();
    project_new(&mut f, &dir);
    seed_substrate(&f);
    let lane = first_lane(&f);
    let device = inject_device(&f, 2);
    let staging = tmp_dir("staging-fault");

    ok(
        &mut f.rt,
        "syn.recorder.start",
        start_params(lane, &staging),
    );
    {
        let stream = device.stream(0);
        let mut g = stream.lock().expect("stream");
        g.deliver(256);
        g.raise(DeviceError::StreamInvalidated);
    }
    // faulted latch が pump 結果に投影される。
    let pumped = ok(&mut f.rt, "syn.recorder.pump", json!({}));
    assert_eq!(pumped["faulted"], true);
    let status = recorder_status(&mut f);
    assert_ne!(status["fault_bits"], "0x0000");

    // stop は fault marker を記録した上で finalize まで進む。
    let stopped = ok(&mut f.rt, "syn.recorder.stop", json!({}));
    assert_eq!(stopped["finalized"], 1);
    // staging session dir に capture-fault marker が残る。
    let session_dir = std::fs::read_dir(&staging)
        .expect("staging root")
        .filter_map(std::result::Result::ok)
        .find(|e| e.path().is_dir())
        .expect("session dir")
        .path();
    let markers: Vec<_> = std::fs::read_dir(&session_dir)
        .expect("session dir read")
        .filter_map(std::result::Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("fault"))
        .collect();
    assert!(
        !markers.is_empty(),
        "capture-fault marker must be recorded: {markers:?}"
    );

    // capture-fault marker は全 stream を recoverable に分類するため
    // commit outcome は `nothing-committed`。commit 確定でない
    // outcome では runtime session を保持し retry/discard を
    // 可能にする — drop すると project session が孤立する。
    let committed = ok(&mut f.rt, "syn.recorder.commit", json!({}));
    assert_eq!(committed["outcome"], "nothing-committed");
    let status = recorder_status(&mut f);
    assert_eq!(status["state"], "stopped");
    assert_eq!(status["active"], true);
    assert!(
        close_guard_reasons(&mut f).contains(&"recorder_active".to_owned()),
        "uncommitted stopped session must keep recorder veto"
    );
    // discard が session を畳み、veto が解放される。
    let discarded = ok(&mut f.rt, "syn.recorder.discard", json!({}));
    assert_eq!(discarded["session_closed"], true);
    assert_eq!(recorder_status(&mut f)["state"], "idle");
}

/// `syn.recorder.discard` — commit せず session を破棄し、
/// project session は best-effort `CloseSession` で `closed` に
/// 留まる（Finalized にはならない）。
#[test]
fn recorder_wire_discard_closes_without_commit() {
    let dir = tmp_dir("discard");
    let mut f = fixture();
    project_new(&mut f, &dir);
    seed_substrate(&f);
    let lane = first_lane(&f);
    let device = inject_device(&f, 2);
    let staging = tmp_dir("staging-discard");

    ok(
        &mut f.rt,
        "syn.recorder.start",
        start_params(lane, &staging),
    );
    {
        let stream = device.stream(0);
        stream.lock().expect("stream").deliver(256);
    }
    ok(&mut f.rt, "syn.recorder.pump", json!({}));

    let discarded = ok(&mut f.rt, "syn.recorder.discard", json!({}));
    assert_eq!(discarded["session_closed"], true);
    assert_eq!(discarded["active"], false);
    assert_eq!(recorder_status(&mut f)["state"], "idle");
    // Recording 中 discard は stream を停止する。
    let stream = device.stream(0);
    assert!(!stream.lock().expect("stream").is_started());
    // project session は CloseSession 済み（closed）で commit されない。
    let info = recording_info(&mut f);
    assert_eq!(info["counts"]["sessions"], 1);
    assert_eq!(info["sessions"][0]["state"], "closed");
    assert_eq!(info["counts"]["takes"], 0);
    assert!(
        !close_guard_reasons(&mut f).contains(&"recorder_active".to_owned()),
        "discarded session must release recorder veto"
    );
    // discard 後は session 無し — commit/stop は typed error。
    err(&mut f.rt, "syn.recorder.commit", json!({}));
}

/// `project.new` は稼働 recorder session を破棄する
/// （旧 project の session/stream ID は無効）。注入 device は
/// host resource として保持され、capabilities は available のまま。
#[test]
fn recorder_wire_project_new_resets_active_session() {
    let dir = tmp_dir("reset");
    let mut f = fixture();
    project_new(&mut f, &dir);
    seed_substrate(&f);
    let lane = first_lane(&f);
    let device = inject_device(&f, 2);
    let staging = tmp_dir("staging-reset");

    ok(
        &mut f.rt,
        "syn.recorder.start",
        start_params(lane, &staging),
    );
    assert_eq!(recorder_status(&mut f)["state"], "recording");
    assert!(
        close_guard_reasons(&mut f).contains(&"recorder_active".to_owned()),
        "active session must veto before reset"
    );

    // project 再生成 → recorder session は破棄・stream 停止。
    let dir2 = tmp_dir("reset-2");
    project_new(&mut f, &dir2);
    assert_eq!(recorder_status(&mut f)["state"], "idle");
    assert_eq!(recorder_status(&mut f)["active"], false);
    assert_eq!(
        device.stream(0).lock().expect("stream").stop_calls(),
        1,
        "reset must stop the running stream"
    );
    let info = recording_info(&mut f);
    assert_eq!(info["counts"]["sessions"], 0, "new project has no sessions");
    assert!(
        !close_guard_reasons(&mut f).contains(&"recorder_active".to_owned()),
        "reset session must release recorder veto"
    );
    // device は保持 — capabilities は available のまま。
    let cap = capabilities_recorder(&mut f);
    assert_eq!(cap["state"], "available");
    assert_eq!(cap["devices"], 1);
}

/// `syn.recorder.mark_take_verified` — `Open` session の `Raw` take
/// を Verified に遷移させ、`syn.recording.info` に反映される。
/// `Finalized` session の take は domain が拒否する。
#[test]
fn recorder_wire_mark_take_verified() {
    let dir = tmp_dir("verify");
    let mut f = fixture();
    project_new(&mut f, &dir);
    seed_substrate(&f);
    let take = seed_open_take(&f);

    let marked = ok(
        &mut f.rt,
        "syn.recorder.mark_take_verified",
        json!({ "take_id": take.to_tagged() }),
    );
    assert_eq!(marked["state"], "verified");
    let info = recording_info(&mut f);
    let row = info["takes"]
        .as_array()
        .and_then(|t| {
            t.iter()
                .find(|t| t["id"].as_str() == Some(take.to_tagged().as_str()))
        })
        .expect("take row");
    assert_eq!(row["state"], "verified");

    // 未知 take / Finalized session の take は typed error。
    err(
        &mut f.rt,
        "syn.recorder.mark_take_verified",
        json!({ "take_id": "take-999999" }),
    );
}

/// `syn.recorder.commit` の retry 契約 — `commit_session` が
/// staging IO 失敗で `Err` を返した場合、`CloseSession` は既に
/// 適用済みで project session は `Closed` に残る。`CloseSession`
/// の validate は `Open` のみを要求するため、retry は適用済み
/// `CloseSession` を skip して `commit_session` の冪等経路
/// （`commit_status` 照合）に入らなければならない — さもないと
/// commit が永久に wedge する。
#[test]
fn recorder_wire_commit_retries_after_staging_io_failure() {
    let dir = tmp_dir("commit-retry");
    let mut f = fixture();
    project_new(&mut f, &dir);
    seed_substrate(&f);
    let lane = first_lane(&f);
    let device = inject_device(&f, 2);
    let staging = tmp_dir("staging-commit-retry");

    ok(
        &mut f.rt,
        "syn.recorder.start",
        start_params(lane, &staging),
    );
    {
        let stream = device.stream(0);
        stream.lock().expect("stream").deliver(512);
    }
    ok(&mut f.rt, "syn.recorder.pump", json!({}));
    ok(&mut f.rt, "syn.recorder.stop", json!({}));

    // `commit-manifest.json` を破損 seed → `commit_session` の
    // `read_commit_manifest` が decode error で `Err`（build_payload
    // 到達前）。`CloseSession` は適用済みで project session は
    // `Closed`。dir rename は open handle で Windows に拒否される
    // ため、commit が最初に読む file を直接壊す。
    let session_dir = std::fs::read_dir(&staging)
        .expect("staging root")
        .filter_map(std::result::Result::ok)
        .find(|e| e.path().is_dir())
        .expect("session dir")
        .path();
    let manifest_path = session_dir.join("commit-manifest.json");
    std::fs::write(&manifest_path, b"{corrupt").expect("seed corrupt commit manifest");

    let e = err(&mut f.rt, "syn.recorder.commit", json!({}));
    // staging/commit の infrastructure 失敗は `service_error`。
    assert_eq!(e.code, "service_error", "{e:?}");

    // session は `stopped` のまま保持され、project session は
    // `closed`（`CloseSession` 済み・commit 未適用）。
    let status = recorder_status(&mut f);
    assert_eq!(status["state"], "stopped");
    assert_eq!(status["active"], true);
    let info = recording_info(&mut f);
    assert_eq!(info["sessions"][0]["state"], "closed");

    // 破損 manifest を除去して retry — `CloseSession` 再適用
    // （`Closed` への validate 失敗）ではなく `commit_session` の
    // 冪等経路で commit が完了しなければならない。
    std::fs::remove_file(&manifest_path).expect("remove corrupt commit manifest");
    let committed = ok(&mut f.rt, "syn.recorder.commit", json!({}));
    assert_eq!(committed["outcome"], "applied");
    assert_eq!(recorder_status(&mut f)["state"], "idle");
    let info = recording_info(&mut f);
    assert_eq!(info["sessions"][0]["state"], "finalized");
}

/// `syn.project.undo`/`redo` は recorder session 稼働中に拒否
/// される — undo は snapshot 復元で per-op validation を通らず、
/// recorder が登録した session/stream entity を巻き戻して
/// commit を永久に不能にし得るため。`stopped`（commit 待ち）でも
/// 同じく拒否し、commit 確定後に再び使えることを pin する。
#[test]
fn recorder_wire_undo_redo_rejected_while_active() {
    let dir = tmp_dir("undo-gate");
    let mut f = fixture();
    project_new(&mut f, &dir);
    seed_substrate(&f);
    let lane = first_lane(&f);
    let device = inject_device(&f, 2);
    let staging = tmp_dir("staging-undo-gate");

    ok(
        &mut f.rt,
        "syn.recorder.start",
        start_params(lane, &staging),
    );
    // recording 中は undo/redo が typed error。
    let e = err(&mut f.rt, "syn.project.undo", json!({}));
    assert_eq!(e.code, "invalid_params", "{e:?}");
    let e = err(&mut f.rt, "syn.project.redo", json!({}));
    assert_eq!(e.code, "invalid_params", "{e:?}");
    // session は recording のまま — lifecycle を完走できる。
    {
        let stream = device.stream(0);
        stream.lock().expect("stream").deliver(256);
    }
    ok(&mut f.rt, "syn.recorder.pump", json!({}));
    ok(&mut f.rt, "syn.recorder.stop", json!({}));
    // stopped（未 commit）でも gate は残る。
    let e = err(&mut f.rt, "syn.project.undo", json!({}));
    assert_eq!(e.code, "invalid_params", "{e:?}");
    let committed = ok(&mut f.rt, "syn.recorder.commit", json!({}));
    assert_eq!(committed["outcome"], "applied");
    // commit 確定で runtime session が畳まれ、undo が再び使える。
    let undone = ok(&mut f.rt, "syn.project.undo", json!({}));
    assert_eq!(undone["undone"], true);
}

/// `targets[].channels` は wire 上限を超えると `invalid_params`
/// — tap resource の callback queue（`max_block_frames ×
/// channel_map.len()`）と PCM pool が eager 確保されるため、
/// 上限なしでは巨大確保で process が abort し得る。
#[test]
fn recorder_wire_start_rejects_oversized_target_channels() {
    let dir = tmp_dir("channels-bound");
    let mut f = fixture();
    project_new(&mut f, &dir);
    seed_substrate(&f);
    let lane = first_lane(&f);
    let _device = inject_device(&f, 2);
    let staging = tmp_dir("staging-channels");

    // channel_map 長 257 = target channels 257 > 上限 256。
    let channel_map = vec![0_u32; 257];
    let e = err(
        &mut f.rt,
        "syn.recorder.start",
        json!({
            "transport_start": 0,
            "stop": 3840,
            "sample_rate": RATE,
            "staging_root": staging.to_string_lossy(),
            "targets": [{ "lane": lane.to_tagged(), "channels": 257 }],
            "streams": [{
                "channels": 2,
                "taps": [{ "stream_index": 0, "channel_map": channel_map }],
            }],
        }),
    );
    assert_eq!(e.code, "invalid_params", "{e:?}");
    // project は汚れない — session 未作成・runtime session 無し。
    assert_eq!(recorder_status(&mut f)["state"], "idle");
    assert_eq!(recording_info(&mut f)["counts"]["sessions"], 0);
}

/// `loop_start < transport_start` は session frame が負になり
/// `GateScheduler` が構成できないため、runtime fault ではなく
/// start 時に `invalid_params` で拒否する。
#[test]
fn recorder_wire_start_rejects_loop_before_transport() {
    let dir = tmp_dir("loop-gate");
    let mut f = fixture();
    project_new(&mut f, &dir);
    seed_substrate(&f);
    let lane = first_lane(&f);
    let _device = inject_device(&f, 2);
    let staging = tmp_dir("staging-loop-gate");

    let mut params = start_params(lane, &staging);
    params["loop_start"] = json!(-100);
    params["loop_end"] = json!(100);
    let e = err(&mut f.rt, "syn.recorder.start", params);
    assert_eq!(e.code, "invalid_params", "{e:?}");
    assert_eq!(recorder_status(&mut f)["state"], "idle");
    assert_eq!(recording_info(&mut f)["counts"]["sessions"], 0);
}

/// `syn.recorder.stop` の retry 契約 — 複数 armed stream で一部の
/// finalize が失敗した場合、成功済み stream は `Finalized` のまま
/// 残る。`StreamWriter::finalize` は非冪等（`AlreadyFinalized`）
/// なので、retry は `AlreadyFinalized` を成功として扱い残りを
/// finalize して `Stopped` に到達しなければならない — さもないと
/// commit 不能の wedge になる。
#[test]
fn recorder_wire_stop_retries_after_partial_finalize_failure() {
    let dir = tmp_dir("stop-retry");
    let mut f = fixture();
    project_new(&mut f, &dir);
    seed_substrate(&f);
    // 2 本目の lane — 2 armed tap = 2 staging stream。
    let track = with_live(&f, |p| *p.tracks.keys().next().unwrap());
    apply_project(&f, ProjectOp::AddLane { track_id: track });
    let lane_ids: Vec<_> = with_live(&f, |p| p.lanes.keys().copied().collect());
    assert_eq!(lane_ids.len(), 2);
    let device = inject_device(&f, 2);
    let staging = tmp_dir("staging-stop-retry");

    let params = json!({
        "name": "wire-2stream",
        "transport_start": 0,
        "stop": 3840,
        "sample_rate": RATE,
        "staging_root": staging.to_string_lossy(),
        "targets": [
            { "lane": lane_ids[0].to_tagged(), "channels": 1 },
            { "lane": lane_ids[1].to_tagged(), "channels": 1 },
        ],
        "streams": [{
            "channels": 2,
            "taps": [
                { "stream_index": 0, "channel_map": [0] },
                { "stream_index": 1, "channel_map": [1] },
            ],
        }],
    });
    ok(&mut f.rt, "syn.recorder.start", params);
    {
        let stream = device.stream(0);
        stream.lock().expect("stream").deliver(512);
    }
    ok(&mut f.rt, "syn.recorder.pump", json!({}));

    // stream-0001 の journal を一時的に破損 — finalize の
    // `verify_stream` が失敗し、stream-0000 は finalize 済みの
    // まま残る（部分 finalize 失敗）。offset は file 先頭の
    // record 領域で、append される checkpoint record には触れない。
    let session_dir = std::fs::read_dir(&staging)
        .expect("staging root")
        .filter_map(std::result::Result::ok)
        .find(|e| e.path().is_dir())
        .expect("session dir")
        .path();
    let journal_path = session_dir.join("streams").join("stream-0001.journal");
    assert!(
        std::fs::metadata(&journal_path)
            .expect("journal meta")
            .len()
            > 24,
        "journal must hold chunk records"
    );
    let mut bytes = std::fs::read(&journal_path).expect("journal read");
    let saved: Vec<u8> = bytes[8..24].to_vec();
    for b in &mut bytes[8..24] {
        *b ^= 0xFF;
    }
    std::fs::write(&journal_path, &bytes).expect("corrupt journal");

    // 1 回目の stop — stream-0001 の finalize が失敗し位相は
    // recording のまま。
    err(&mut f.rt, "syn.recorder.stop", json!({}));
    let status = recorder_status(&mut f);
    assert_eq!(status["state"], "recording");
    assert_eq!(status["active"], true);

    // journal を復元（XOR で同じ offset を戻す — 破損中に append
    // された checkpoint record は保持される）。
    let mut bytes = std::fs::read(&journal_path).expect("journal read");
    bytes[8..24].copy_from_slice(&saved);
    std::fs::write(&journal_path, &bytes).expect("restore journal");

    // retry — stream-0000 は `AlreadyFinalized` を成功として扱い、
    // stream-0001 が finalize されて `stopped` に到達する。
    let stopped = ok(&mut f.rt, "syn.recorder.stop", json!({}));
    assert_eq!(stopped["finalized"], 2, "{stopped:?}");
    assert_eq!(recorder_status(&mut f)["state"], "stopped");

    // commit も完走する。
    let committed = ok(&mut f.rt, "syn.recorder.commit", json!({}));
    assert_eq!(committed["outcome"], "applied");
    assert_eq!(recorder_status(&mut f)["state"], "idle");
}
