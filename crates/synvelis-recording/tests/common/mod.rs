//! RCD-01 input adapter テスト共通基盤。
//!
//! scripted fake `Device`/`InputStream`（`ruviform_audio_core::device`
//! port 実装）と、plan/spec/PCM 検算の共有 helper。
//! `input_adapter.rs`（契約検証）と `input_adapter_rt_alloc.rs`
//! （RT no-alloc 計測）で共有する。
//!
//! これは fake port 契約の検証であり、実 Device Pass ではない
//! （実 backend は ruviform#251 待ちで `Unsupported`）。

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use ruviform_audio_core::device::{
    BufferSize, DEVICE_CLOCK_TIMEBASE, Device, DeviceEnumerator, DeviceError, DeviceId,
    DeviceResult, InputCallbackInfo, InputDataCallback, InputErrorCallback, InputStream,
    InputStreamConfig, InputTimestamp, OutputDataCallback, OutputErrorCallback, OutputStream,
    OutputStreamConfig, StreamLatency, SupportedInputFormat, SupportedOutputFormat,
};
use ruviform_audio_core::{
    AudioBufferView, AudioFormat, ChannelLayout, SampleFormat, SampleRate, SpeakerMask,
};
use ruviform_media_core::{ClockDomainId, MediaTime};
use synvelis_project::{
    CaptureStreamId, ClockEpoch, ClockEpochId, Lane, LaneId, MusicalTick, Project, ProjectId,
    RecordingSessionId, SessionFrame, TempoMapRef, Track, TrackId,
};
use synvelis_recording::{
    ArmState, BufferBudget, CaptureTarget, ChannelMap, ChunkLog, ChunkRecord, ChunkSink,
    FinishReport, FaultLatch, GateSpec, InputAdapter, InputBinding, InputSession, InputStreamSpec,
    InputTapSpec, InspectionWriter, JournalSpan, MockPcm, MonitorState, OpenedInput,
    PreparedRecordingPlan, RecordPayload, RecordingRequest, ReplayOutcome, StagingArea,
    StagingSessionConfig, StagingStreamConfig, StreamWriter, crc32, finish_all, preflight,
    replay_journal, stream_config,
};

pub const RATE: u32 = 48_000;
/// fake stream の PCM 生成 seed（stream ごとに変える）。
pub const SEED: u32 = 7;
/// fake slab の 1 channel あたり frame 上限。
pub const FRAMES_MAX: usize = 4096;

// ---------- fake port ----------

/// channel 数に対応する `ChannelLayout`（既知 layout 以外は custom mask）。
pub fn layout_for(channels: u32) -> ChannelLayout {
    match channels {
        1 => ChannelLayout::Mono,
        2 => ChannelLayout::Stereo,
        4 => ChannelLayout::Quad,
        6 => ChannelLayout::FivePointOne,
        8 => ChannelLayout::SevenPointOne,
        n if n < 32 => {
            ChannelLayout::custom(SpeakerMask::from_bits((1_u32 << n) - 1)).expect("non-empty mask")
        }
        _ => ChannelLayout::custom(SpeakerMask::FRONT_CENTER).expect("non-empty mask"),
    }
}

pub fn input_format(channels: u32) -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(RATE).expect("rate"),
        layout: layout_for(channels),
        sample_format: SampleFormat::F32,
    }
}

pub fn input_config(channels: u32) -> InputStreamConfig {
    InputStreamConfig {
        format: input_format(channels),
        buffer_size: BufferSize::Default,
    }
}

fn channels_of(format: AudioFormat) -> usize {
    usize::try_from(format.layout.channels()).unwrap_or(0)
}

/// scripted fake stream の共有状態（device handle と stream handle の双方から触る）。
pub struct FakeStreamShared {
    data_cb: Option<InputDataCallback>,
    err_cb: Option<InputErrorCallback>,
    /// `InputStream::clock_domain()` が返す固定 domain。
    domain: ClockDomainId,
    /// per-callback `InputCallbackInfo.clock_domain`（注入で変更可）。
    cb_domain: ClockDomainId,
    /// `stream.format()` が返す format（注入で変更可）。
    format: AudioFormat,
    /// 次に供給する device input frame 番号。
    position: u64,
    /// `frame_position` を報告するか（false で `None` 注入）。
    report_position: bool,
    /// callback に渡す channel 数の上書き（format 変更注入用）。
    deliver_channels: Option<usize>,
    /// planar PCM slab（channel-major、`channels * FRAMES_MAX` 確保済み）。
    slab: Vec<f32>,
    pcm: MockPcm,
    started: bool,
}

impl FakeStreamShared {
    /// `frames` 分の deterministic planar PCM を callback へ供給する。
    pub fn deliver(&mut self, frames: u32) {
        let frames_usize = usize::try_from(frames).expect("frames");
        assert!(frames_usize > 0 && frames_usize <= FRAMES_MAX);
        let ch = self
            .deliver_channels
            .unwrap_or_else(|| channels_of(self.format));
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
            clock_domain: self.cb_domain,
            frame_position: if self.report_position {
                Some(self.position)
            } else {
                None
            },
        };
        let view = AudioBufferView::try_new(&self.slab[..ch * frames_usize], ch, frames_usize)
            .expect("view");
        if let Some(cb) = &mut self.data_cb {
            cb(view, info);
        }
        self.position = self.position.wrapping_add(u64::from(frames));
    }

    /// error callback へ `DeviceError` を注入する。
    pub fn raise(&mut self, err: DeviceError) {
        if let Some(cb) = &mut self.err_cb {
            cb(err);
        }
    }

    /// 位置を `pos` へ跳ばす（gap/dup 注入用）。
    pub fn jump_to(&mut self, pos: u64) {
        self.position = pos;
    }

    /// callback の clock domain を変更する（domain 変更注入用）。
    pub fn set_cb_domain(&mut self, domain: ClockDomainId) {
        self.cb_domain = domain;
    }

    /// `frame_position` 報告を止める/再開する。
    pub fn set_report_position(&mut self, report: bool) {
        self.report_position = report;
    }

    /// callback の channel 数を変更する（format 変更注入用）。
    pub fn set_deliver_channels(&mut self, channels: usize) {
        self.deliver_channels = Some(channels);
    }

    /// `stream.format()` の返り値を変更する（rate 変更注入用）。
    pub fn set_format(&mut self, format: AudioFormat) {
        self.format = format;
    }

    /// `start()` が呼ばれたか（lifecycle 検証用）。
    pub fn is_started(&self) -> bool {
        self.started
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
        self.shared.lock().expect("lock").started = false;
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
    seed: u32,
    open_error: Option<DeviceError>,
    streams: Vec<Arc<Mutex<FakeStreamShared>>>,
}

/// scripted fake `Device`（input のみ実装、output は `Unsupported`）。
#[derive(Clone)]
pub struct FakeInputDevice {
    shared: Arc<Mutex<FakeDeviceShared>>,
}

impl FakeInputDevice {
    /// `channels` ch・`domain` の fake input device を作る。
    pub fn new(channels: u32, domain: ClockDomainId) -> Self {
        Self {
            shared: Arc::new(Mutex::new(FakeDeviceShared {
                format: input_format(channels),
                domain,
                seed: SEED,
                open_error: None,
                streams: Vec::new(),
            })),
        }
    }

    /// PCM seed を変える（複数 stream の PCM を区別する検証用）。
    pub fn with_seed(self, seed: u32) -> Self {
        self.shared.lock().expect("lock").seed = seed;
        self
    }

    /// open 済み stream への scripted handle（test 駆動用）。
    pub fn stream(&self, index: usize) -> Arc<Mutex<FakeStreamShared>> {
        self.shared.lock().expect("lock").streams[index].clone()
    }

    /// `open_input_stream` を常に失敗させる注入。
    pub fn fail_open(&self, err: DeviceError) {
        self.shared.lock().expect("lock").open_error = Some(err);
    }

    /// 同一 fake device の識別（`open_input_stream` 呼出し順の
    /// stream handle 振り分け用）。
    pub fn instance_id(&self) -> usize {
        Arc::as_ptr(&self.shared) as usize
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
        let mut g = self.shared.lock().expect("lock");
        if let Some(err) = g.open_error {
            return Err(err);
        }
        let channels = channels_of(config.format);
        let shared = Arc::new(Mutex::new(FakeStreamShared {
            data_cb: Some(data_callback),
            err_cb: Some(error_callback),
            domain: g.domain,
            cb_domain: g.domain,
            format: config.format,
            position: 0,
            report_position: true,
            deliver_channels: None,
            slab: vec![0.0; channels.saturating_mul(FRAMES_MAX)],
            pcm: MockPcm { seed: g.seed },
            started: false,
        }));
        g.streams.push(Arc::clone(&shared));
        Ok(Box::new(FakeInputStream { shared }))
    }
}

/// scripted fake `DeviceEnumerator`。
pub struct FakeEnumerator {
    pub devices: Vec<FakeInputDevice>,
}

impl DeviceEnumerator for FakeEnumerator {
    fn output_devices(&self) -> DeviceResult<Vec<Box<dyn Device>>> {
        Ok(Vec::new())
    }

    fn default_output_device(&self) -> DeviceResult<Option<Box<dyn Device>>> {
        Ok(None)
    }

    fn input_devices(&self) -> DeviceResult<Vec<Box<dyn Device>>> {
        Ok(self
            .devices
            .iter()
            .cloned()
            .map(|d| Box::new(d) as Box<dyn Device>)
            .collect())
    }

    fn default_input_device(&self) -> DeviceResult<Option<Box<dyn Device>>> {
        Ok(self
            .devices
            .first()
            .cloned()
            .map(|d| Box::new(d) as Box<dyn Device>))
    }
}

// ---------- plan / spec helpers ----------

pub fn project_with_lanes(n: usize) -> (Project, Vec<LaneId>, ClockEpochId) {
    let epoch = ClockEpochId::new(22);
    let mut p = Project::new(ProjectId::new(0), "rcd-01");
    let mut lanes = Vec::new();
    for i in 0..n {
        let lane = LaneId::new(11 + u128::try_from(i).unwrap_or(u128::MAX));
        let track = TrackId::new(1 + u128::try_from(i).unwrap_or(u128::MAX));
        p.tracks.insert(
            track,
            Track {
                id: track,
                name: format!("t{i}"),
                lane_ids: vec![lane],
            },
        );
        p.lanes.insert(
            lane,
            Lane {
                id: lane,
                track_id: track,
                clip_ids: vec![],
            },
        );
        lanes.push(lane);
    }
    p.clock_epochs.insert(
        epoch,
        ClockEpoch {
            id: epoch,
            description: "fake".to_owned(),
        },
    );
    (p, lanes, epoch)
}

pub fn budget() -> BufferBudget {
    BufferBudget {
        chunks: 16,
        chunk_frames: 1024,
        spans_per_chunk: 8,
        filled_capacity: 16,
        lookahead_spans: 16,
    }
}

/// Normal gate・transport 0→`stop_tick` の plan（`project_with_lanes` と対で使う）。
pub fn plan_for(
    p: &Project,
    lanes: &[LaneId],
    channels: &[u32],
    epoch: ClockEpochId,
    stop_tick: i64,
) -> PreparedRecordingPlan {
    let req = RecordingRequest {
        targets: lanes
            .iter()
            .zip(channels.iter())
            .map(|(&lane_id, &ch)| CaptureTarget {
                lane_id,
                channels: ch,
            })
            .collect(),
        clock_epoch: epoch,
        tempo_map: TempoMapRef { revision: 0 },
        transport_start: MusicalTick::ZERO,
        stop: MusicalTick::new(stop_tick),
        gate: GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        budget: budget(),
        max_block_frames: 1024,
    };
    preflight(p, req, RATE).expect("preflight must pass")
}

/// 1 tap 分の spec を組み立てる（`session_end`/`budget` は plan 由来）。
pub fn tap(
    plan: &PreparedRecordingPlan,
    stream_index: u16,
    map: ChannelMap,
    arm: ArmState,
    monitor: MonitorState,
) -> InputTapSpec {
    InputTapSpec {
        binding: InputBinding {
            channel_map: map,
            arm,
            monitor,
        },
        stream_index,
        session_end: plan.session_end,
        budget: plan.budget,
    }
}

/// mono 1ch の armed spec を組み立てる定型。
pub fn mono_spec(
    plan: &PreparedRecordingPlan,
    epoch: ClockEpochId,
    callback_queue: usize,
) -> InputStreamSpec {
    InputStreamSpec {
        config: input_config(1),
        clock_epoch: epoch,
        max_block_frames: plan.max_block_frames,
        callback_queue,
        taps: vec![tap(
            plan,
            0,
            ChannelMap::mono(0),
            ArmState::On,
            MonitorState::Off,
        )],
    }
}

// ---------- PCM 検算 ----------

/// `verify_log` の channel map 対応版: 出力 channel j は入力
/// `channel_map[j]` の sample と一致することを確認する。
pub fn verify_tap_log(log: &ChunkLog, channel_map: &[u32], seed: u32) -> bool {
    let gen_ = MockPcm { seed };
    let out_ch = channel_map.len();
    for span in &log.spans {
        let frame_off = span
            .session_start
            .checked_sub(log.header.session_start.value())
            .map(SessionFrame::value);
        let Ok(Some(input_base)) = frame_off.map(|o| {
            log.header
                .first_input_frame
                .checked_add(u64::try_from(o).unwrap_or(u64::MAX))
        }) else {
            return false;
        };
        let start = span.packed_pcm_offset;
        let need = usize::try_from(span.frame_count).unwrap_or(usize::MAX) * out_ch;
        if start + need > log.pcm.len() {
            return false;
        }
        for f in 0..span.frame_count {
            for (j, &src_ch) in channel_map.iter().enumerate() {
                let idx = start + usize::try_from(f).unwrap_or(usize::MAX) * out_ch + j;
                if log.pcm[idx].to_bits()
                    != gen_
                        .sample(input_base.wrapping_add(u64::from(f)), src_ch)
                        .to_bits()
                {
                    return false;
                }
            }
        }
    }
    true
}

// ---------- test helpers ----------

/// `OpenedInput` を adapter + `(stream_index, InspectionWriter)` 列に分解する。
pub fn into_writers(
    opened: OpenedInput,
    fault: &FaultLatch,
) -> (InputAdapter, Vec<(u16, InspectionWriter)>) {
    let adapter = opened.adapter;
    let writers = opened
        .writer_ends
        .into_iter()
        .map(|w| (w.stream_index, InspectionWriter::new(w.ends, fault.clone())))
        .collect();
    (adapter, writers)
}

/// `frames` を `block` 刻みで deliver → pump → writer.pump する。
pub fn drive(
    stream: &Arc<Mutex<FakeStreamShared>>,
    adapter: &mut InputAdapter,
    writers: &mut [(u16, InspectionWriter)],
    plan: &PreparedRecordingPlan,
    frames: u32,
    block: u32,
) {
    let mut remaining = frames;
    while remaining > 0 {
        let n = block.min(remaining);
        stream.lock().expect("lock").deliver(n);
        remaining -= n;
        adapter.pump(&plan.scheduler);
        for (_, w) in writers.iter_mut() {
            w.pump();
        }
    }
}

// ---------- device-recording rig（fake port → 実 StreamWriter） ----------
//
// `input_adapter.rs` が in-memory `InspectionWriter` で検証するのに対し、
// こちらは `TapWriterEnds` を実 `StreamWriter`（staging media/journal）
// へ接続し、checkpoint/finalize まで駆動する統合 fixture 用 harness。
// これも fake port 契約の検証であり、実 Device Pass ではない
// （実 backend は ruviform#251 待ちで `Unsupported`）。

/// `GateSpec` 指定の plan（`plan_for` の Gate 一般化版）。
pub fn plan_gated(
    p: &Project,
    lanes: &[LaneId],
    channels: &[u32],
    epoch: ClockEpochId,
    gate: GateSpec,
    stop_tick: i64,
) -> PreparedRecordingPlan {
    let req = RecordingRequest {
        targets: lanes
            .iter()
            .zip(channels.iter())
            .map(|(&lane_id, &ch)| CaptureTarget {
                lane_id,
                channels: ch,
            })
            .collect(),
        clock_epoch: epoch,
        tempo_map: TempoMapRef { revision: 0 },
        transport_start: MusicalTick::ZERO,
        stop: MusicalTick::new(stop_tick),
        gate,
        budget: budget(),
        max_block_frames: 1024,
    };
    preflight(p, req, RATE).expect("preflight must pass")
}

/// armed tap から staging stream 設定を組み立てる。
/// `stream_index` は `plan.targets` の添字と一致する前提で、lane は
/// target から、channels は `channel_map` 長との一致を検証して取る。
/// `stream_ids` を与えると `stream_ids[stream_index]` を
/// `capture_stream_id` に使う（commit 層検証で project 側の採番 ID と
/// 一致させる場合）。
fn staging_streams(
    plan: &PreparedRecordingPlan,
    specs: &[&InputStreamSpec],
    stream_ids: Option<&[CaptureStreamId]>,
) -> Vec<StagingStreamConfig> {
    let mut out = Vec::new();
    for spec in specs {
        for t in &spec.taps {
            if t.binding.arm != ArmState::On {
                continue;
            }
            let idx = usize::from(t.stream_index);
            let target = plan
                .targets
                .get(idx)
                .unwrap_or_else(|| panic!("stream_index {} has no plan target", t.stream_index));
            let map_channels = u32::try_from(t.binding.channel_map.len()).unwrap_or(u32::MAX);
            assert_eq!(
                target.channels, map_channels,
                "stream_index {} channel count mismatch",
                t.stream_index
            );
            let capture_stream_id = stream_ids
                .and_then(|ids| ids.get(idx).copied())
                .unwrap_or_else(|| CaptureStreamId::new(10_000 + u128::from(t.stream_index)));
            out.push(stream_config(
                t.stream_index,
                capture_stream_id,
                target.lane_id,
                target.channels,
            ));
        }
    }
    out.sort_by_key(|c| c.stream_index);
    out
}

/// fake device → `InputAdapter` → 実 `StreamWriter` → staging
/// media/journal を結合した録音 rig（`device_recording.rs` fixture 用）。
///
/// これは fake port 契約の検証であり、実 Device Pass ではない。
pub struct DeviceRig {
    /// same-clock 検証済みの入力 session（全 stream で共有する `FaultLatch`）。
    pub session: InputSession,
    /// `(stream_index, StreamWriter)` を `stream_index` 昇順で保持。
    pub writers: Vec<(u16, StreamWriter)>,
    /// staging 領域（manifest/media/journal/finalized/capture-fault）。
    pub area: StagingArea,
    /// open 順の fake stream handle（deliver/注入駆動用）。
    pub streams: Vec<Arc<Mutex<FakeStreamShared>>>,
    /// 駆動に使う確定 plan（`pump_all` の scheduler 供給元）。
    pub plan: PreparedRecordingPlan,
}

impl DeviceRig {
    /// `inputs`（open 順の `(device, spec)` 列）から rig を構築する。
    /// 最初の spec は `InputSession::open`、以降は `add` で same-clock
    /// 検証を通す。armed tap ごとに staging stream + `StreamWriter` を接続する。
    pub fn open(
        root: &Path,
        session_id: RecordingSessionId,
        plan: PreparedRecordingPlan,
        inputs: &[(&FakeInputDevice, &InputStreamSpec)],
        stream_ids: Option<&[CaptureStreamId]>,
    ) -> Self {
        assert!(!inputs.is_empty(), "at least one input required");
        let specs: Vec<&InputStreamSpec> = inputs.iter().map(|(_, s)| *s).collect();
        let streams_cfg = staging_streams(&plan, &specs, stream_ids);
        let area = StagingArea::create(
            root,
            &StagingSessionConfig {
                session_id,
                sample_rate: RATE,
                streams: streams_cfg,
            },
        )
        .expect("staging create");

        let (first_dev, first_spec) = inputs[0];
        let (mut session, first_ends) =
            InputSession::open(first_dev, first_spec).expect("session open");
        let mut writer_ends = first_ends;
        let mut handles = Vec::with_capacity(inputs.len());
        let mut per_device: HashMap<usize, usize> = HashMap::new();
        let first_k = per_device.entry(first_dev.instance_id()).or_insert(0);
        handles.push(first_dev.stream(*first_k));
        *first_k += 1;
        for (dev, spec) in &inputs[1..] {
            let ends = session.add(*dev, spec).expect("session add");
            writer_ends.extend(ends);
            let k = per_device.entry(dev.instance_id()).or_insert(0);
            handles.push(dev.stream(*k));
            *k += 1;
        }

        let fault = session.fault().clone();
        let mut writers = Vec::with_capacity(writer_ends.len());
        for w in writer_ends {
            let cfg = area
                .manifest
                .streams
                .iter()
                .find(|m| m.stream_index == w.stream_index)
                .map(|m| StagingStreamConfig {
                    stream_index: m.stream_index,
                    capture_stream_id: m.capture_stream_id.clone(),
                    lane_id: m.lane_id.clone(),
                    channels: m.channels,
                })
                .expect("manifest stream for tap");
            writers.push((
                w.stream_index,
                StreamWriter::create(&area, cfg, w.ends, fault.clone())
                    .expect("stream writer create"),
            ));
        }
        writers.sort_by_key(|(i, _)| *i);
        Self {
            session,
            writers,
            area,
            streams: handles,
            plan,
        }
    }

    /// 単一 `(device, spec)` の簡易 open。
    pub fn open_one(
        root: &Path,
        session_id: RecordingSessionId,
        plan: PreparedRecordingPlan,
        device: &FakeInputDevice,
        spec: &InputStreamSpec,
    ) -> Self {
        Self::open(root, session_id, plan, &[(device, spec)], None)
    }

    /// 全 fake stream に `frames` frame を deliver する（pump しない）。
    pub fn deliver_all(&mut self, frames: u32) {
        for s in &self.streams {
            s.lock().expect("lock").deliver(frames);
        }
    }

    /// `pump_all` → 全 writer `pump`（RT→non-RT の drain）。
    pub fn pump_all(&mut self) {
        self.session.pump_all(&self.plan.scheduler);
        for (_, w) in &mut self.writers {
            w.pump();
        }
    }

    /// engine のみ pump（writer を止める filled-queue 溢れ注入用）。
    pub fn pump_engines(&mut self) {
        self.session.pump_all(&self.plan.scheduler);
    }

    /// `frames` を `block` 刻みで deliver→`pump_all`（fault 観測は呼び出し側）。
    pub fn drive(&mut self, frames: u32, block: u32) {
        let mut remaining = frames;
        while remaining > 0 {
            let n = block.min(remaining);
            self.deliver_all(n);
            self.pump_all();
            remaining -= n;
        }
    }

    /// control path の format 照合を全 adapter に実行する。
    pub fn verify_formats(&self) {
        for a in self.session.adapters() {
            a.verify_format();
        }
    }

    /// fault latch が立っていれば session スコープの capture-fault
    /// marker を durable に記録し、checkpoint → finalize を実行する。
    pub fn finish(&mut self) -> FinishReport {
        if self.session.fault().is_faulted() {
            let detail = format!("fault bits {:#06x}", self.session.fault().bits());
            self.area
                .record_capture_fault(&detail)
                .expect("capture fault marker");
        }
        for (_, w) in &mut self.writers {
            w.checkpoint().expect("checkpoint");
        }
        let tag = self.area.manifest.session_id.clone();
        let writers = self.writers.iter_mut().map(|(_, w)| w);
        finish_all(&tag, RATE, writers)
    }
}

/// 1 stream の journal replay + media bytes + record/chunk 列。
pub struct StreamCapture {
    /// journal replay 結果（intact prefix + stop reason）。
    pub replay: ReplayOutcome,
    /// intact record 列。
    pub records: Vec<synvelis_recording::JournalRecord>,
    /// chunk record のみ抽出（journal 順）。
    pub chunks: Vec<ChunkRecord>,
    /// media 実 bytes。
    pub media: Vec<u8>,
}

/// `stream_index` の journal/media を読み出して束ねる。
pub fn capture_of(area: &StagingArea, stream_index: u16) -> StreamCapture {
    let journal = std::fs::read(area.journal_path(stream_index)).expect("read journal");
    let replay = replay_journal(&journal);
    let records = replay.records.clone();
    let chunks = records
        .iter()
        .filter_map(|r| match &r.payload {
            RecordPayload::Chunk(c) => Some(c.clone()),
            _ => None,
        })
        .collect();
    let media = std::fs::read(area.media_path(stream_index)).expect("read media");
    StreamCapture {
        replay,
        records,
        chunks,
        media,
    }
}

/// media bytes を `channel_map`+`MockPcm(seed)` の期待 interleaved PCM
/// と照合し、各 chunk の `pcm_crc32` も検算する。
/// 戻り値は照合した sample 数。
pub fn verify_capture_pcm(cap: &StreamCapture, channel_map: &[u32], seed: u32) -> usize {
    let gen_ = MockPcm { seed };
    let out_ch = channel_map.len();
    let mut checked = 0usize;
    for c in &cap.chunks {
        let start = usize::try_from(c.media_offset).expect("media offset");
        let end = start + usize::try_from(c.media_len).expect("media len");
        assert_eq!(
            crc32(&cap.media[start..end]),
            c.pcm_crc32,
            "chunk {} pcm_crc32 mismatch",
            c.chunk_id
        );
        for s in &c.spans {
            let m_off = usize::try_from(s.media_offset).expect("span offset") / 4;
            let input_base = c
                .first_input_frame
                .wrapping_add(u64::try_from(s.session_start - c.session_start).unwrap_or(0));
            for f in 0..s.frame_count {
                let input_frame = input_base.wrapping_add(u64::from(f));
                for (j, &src_ch) in channel_map.iter().enumerate() {
                    let idx = m_off + usize::try_from(f).unwrap_or(usize::MAX) * out_ch + j;
                    let bytes = cap.media.get(idx * 4..idx * 4 + 4).expect("pcm bytes");
                    let actual = f32::from_le_bytes(bytes.try_into().expect("f32 le"));
                    assert_eq!(
                        actual,
                        gen_.sample(input_frame, src_ch),
                        "pcm mismatch chunk {} f {f} out_ch {j} (in_ch {src_ch})",
                        c.chunk_id
                    );
                    checked += 1;
                }
            }
        }
    }
    checked
}

/// 全 chunk の journal span を `session_start` 昇順で平坦化する。
pub fn all_journal_spans(cap: &StreamCapture) -> Vec<JournalSpan> {
    let mut spans: Vec<JournalSpan> = cap
        .chunks
        .iter()
        .flat_map(|c| c.spans.iter().copied())
        .collect();
    spans.sort_by_key(|s| s.session_start);
    spans
}

/// span 列の `(session_start, session_end, pass_index, source_start,
/// project_in, project_out)` を取り出す（Expected/Observed 比較用）。
pub fn span_obs(spans: &[JournalSpan]) -> Vec<(i64, i64, u32, i64, i64, i64)> {
    spans
        .iter()
        .map(|s| {
            (
                s.session_start,
                s.session_start + i64::from(s.frame_count),
                s.pass_index,
                s.source_start,
                s.project_in,
                s.project_out,
            )
        })
        .collect()
}

/// `spans`（`session_start` 昇順）が `expected` の各 session 領域を
/// 隙間・重複・はみ出しなく cover することを検証する。
pub fn assert_session_coverage(spans: &[JournalSpan], expected: &[(i64, i64)]) {
    let mut idx = 0usize;
    for &(start, end) in expected {
        let mut cur = start;
        while cur < end {
            let Some(s) = spans.get(idx) else {
                panic!("missing span covering session {cur}..{end}");
            };
            assert_eq!(
                s.session_start, cur,
                "gap/overlap before session {cur} (span {s:?})"
            );
            cur = s.session_start + i64::from(s.frame_count);
            assert!(cur <= end, "span overruns region end {end}: {s:?}");
            idx += 1;
        }
    }
    assert_eq!(idx, spans.len(), "unexpected extra spans: {spans:?}");
}

/// span 列の source 範囲が `0..total` を連続 cover することを検証する。
pub fn assert_source_contiguous(spans: &[JournalSpan], total: i64) {
    let mut expect = 0i64;
    for s in spans {
        assert_eq!(
            s.source_start, expect,
            "source gap/overlap at session {} (span {s:?})",
            s.session_start
        );
        expect = s.source_start + i64::from(s.frame_count);
    }
    assert_eq!(expect, total, "source total mismatch");
}

/// span 列の frame 総数。
pub fn span_frame_total(spans: &[JournalSpan]) -> i64 {
    spans
        .iter()
        .map(|s| i64::from(s.frame_count))
        .sum()
}

/// span 列が session 0 からの連続 prefix（隙間・重複・再開なし）で
/// あることを検証する — fail-stop の verified prefix 性質。
pub fn assert_prefix_contiguous(spans: &[JournalSpan]) {
    let mut cur = 0i64;
    for s in spans {
        assert_eq!(s.session_start, cur, "prefix discontinuity at {s:?}");
        cur = s.session_start + i64::from(s.frame_count);
    }
}
