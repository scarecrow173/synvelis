//! RCD-01 input adapter テスト共通基盤。
//!
//! scripted fake `Device`/`InputStream`（`ruviform_audio_core::device`
//! port 実装）と、plan/spec/PCM 検算の共有 helper。
//! `input_adapter.rs`（契約検証）と `input_adapter_rt_alloc.rs`
//! （RT no-alloc 計測）で共有する。
//!
//! これは fake port 契約の検証であり、実 Device Pass ではない
//! （実 backend は ruviform#251 待ちで `Unsupported`）。

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
    ClockEpoch, ClockEpochId, Lane, LaneId, MusicalTick, Project, ProjectId, SessionFrame,
    TempoMapRef, Track, TrackId,
};
use synvelis_recording::{
    ArmState, BufferBudget, CaptureTarget, ChannelMap, ChunkLog, FaultLatch, GateSpec,
    InputAdapter, InputBinding, InputStreamSpec, InputTapSpec, InspectionWriter, MockPcm,
    MonitorState, OpenedInput, PreparedRecordingPlan, RecordingRequest, preflight,
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
                        .sample(
                            input_base.wrapping_add(u64::from(f)),
                            src_ch,
                        )
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
