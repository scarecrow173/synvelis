//! RCD-01 input adapter テスト共通基盤。
//!
//! scripted fake `Device`/`InputStream`（`ruviform_audio_core::device`
//! port 実装）と、plan/spec/PCM 検算の共有 helper。
//! `input_adapter.rs`（契約検証）と `input_adapter_rt_alloc.rs`
//! （RT no-alloc 計測）で共有する。
//!
//! これは fake port 契約の検証であり、実 Device Pass ではない
//! （実 backend は `real-device` feature の opt-in lane
//! `tests/real_input.rs` で別途検証する）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use ruviform_audio_core::device::{
    BufferSize, DEVICE_CLOCK_TIMEBASE, Device, DeviceEnumerator, DeviceError, DeviceId,
    DeviceResult, InputCallbackInfo, InputDataCallback, InputErrorCallback, InputStream,
    InputStreamConfig, InputTimestamp, OutputDataCallback, OutputErrorCallback, OutputStream,
    OutputStreamConfig, StreamLatency, SupportedInputFormat, SupportedOutputFormat,
};
use ruviform_audio_core::{
    AudioBufferView, AudioFormat, ChannelLayout, SampleFormat, SampleRate, SpeakerMask,
};
use ruviform_media_core::{
    ClockDomainId, MediaTime, MediaTimestamp, TimeBase, TimestampProvenance,
};
use synvelis_project::{
    CaptureStreamId, ClockEpoch, ClockEpochId, Lane, LaneId, MusicalTick, Project, ProjectId,
    RecordingSessionId, SessionFrame, TempoMapRef, Track, TrackId,
};
use synvelis_recording::{
    ArmState, BufferBudget, CaptureTarget, ChannelMap, ChunkLog, ChunkRecord, ChunkSink,
    FaultLatch, FinishReport, GateSpec, InputAdapter, InputBinding, InputSession, InputStreamSpec,
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

/// `RIG_DEBUG` env が立っていれば `RealDeviceRig::pump_all` が
/// writer 状態を `eprintln` する診断フラグ。hot loop 内で env を
/// 走査しないよう `OnceLock` で初回のみ評価する。
static RIG_DEBUG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

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
    /// `InputTimestamp.capture` を報告するか（false で `capture: None`）。
    report_capture: bool,
    /// capture stamp の position 0 に対応する device-clock tick。
    stamp_epoch_ticks: u64,
    /// capture stamp の timebase（foreign base 注入用）。
    stamp_base: TimeBase,
    /// `Some` のとき position 追従ではなく固定 tick を報告する
    /// （stamp が進まない注入 — stamp 経路なら不連続 fault になる）。
    stamp_fixed_ticks: Option<u64>,
    /// stamp tick へ加える決定的 jitter（ns）。deliver ごとに
    /// `[0,+1,+2,+3,-3,-2,-1]` cycle で `±3*jitter_ns` を与える
    /// （実機 timestamp jitter の再現。0 で jitter 無し）。
    stamp_jitter_ns: i64,
    /// `deliver` 呼出し回数（jitter cycle 用）。
    deliver_count: u64,
    /// `stream.now()` が返す値（anchor 較正注入用。device level で
    /// open 前に設定する）。
    now_value: i64,
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
        let capture = if self.report_capture {
            // position 追従 stamp: tick = epoch_ticks + position を
            // stream rate の ns へ換算した値。`position * 1e9 / rate`
            // の順で計算し、frame 単位の量子化誤差を 1ns 未満に抑える
            // （`map_stamp_to_frame` の NearestEven で真値の整数
            // frame へ正確に戻る）。
            let ticks = self.stamp_fixed_ticks.unwrap_or_else(|| {
                let rate = u128::from(self.format.sample_rate.get().max(1));
                let base = i128::from(self.stamp_epoch_ticks)
                    + i128::from(
                        u64::try_from(u128::from(self.position) * 1_000_000_000 / rate)
                            .unwrap_or(u64::MAX),
                    );
                // 決定的 jitter: cycle [0,+1,+2,+3,-3,-2,-1] * jitter_ns
                // （先頭は 0 — seed が anchor と一致し PCM 検算を保つ）。
                let step = (self.deliver_count + 3) % 7;
                let jitter = (i64::try_from(step).unwrap_or(0) - 3) * self.stamp_jitter_ns;
                u64::try_from((base + i128::from(jitter)).max(0)).unwrap_or(u64::MAX)
            });
            Some(MediaTimestamp {
                time: MediaTime::new(i64::try_from(ticks).unwrap_or(i64::MAX), self.stamp_base),
                provenance: TimestampProvenance::Carried,
            })
        } else {
            None
        };
        let info = InputCallbackInfo {
            timestamp: InputTimestamp {
                callback: MediaTime::new(0, DEVICE_CLOCK_TIMEBASE),
                capture,
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
        self.deliver_count = self.deliver_count.wrapping_add(1);
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

    /// `capture` stamp の報告を止める/再開する。
    pub fn set_report_capture(&mut self, report: bool) {
        self.report_capture = report;
    }

    /// capture stamp の epoch tick（position 0 の stamp 値）を設定する。
    pub fn set_stamp_epoch_ticks(&mut self, ticks: u64) {
        self.stamp_epoch_ticks = ticks;
    }

    /// capture stamp の timebase を変更する（foreign base 注入用）。
    pub fn set_stamp_base(&mut self, base: TimeBase) {
        self.stamp_base = base;
    }

    /// capture stamp を固定 tick にする（position 非追従の注入用。
    /// `None` で position 追従へ戻す）。
    pub fn set_stamp_fixed_ticks(&mut self, ticks: Option<u64>) {
        self.stamp_fixed_ticks = ticks;
    }

    /// capture stamp へ決定的 jitter（ns）を加える。deliver ごとに
    /// `[0,+1,+2,+3,-3,-2,-1]` cycle で `±3*jitter_ns` が変化する。
    pub fn set_stamp_jitter_ns(&mut self, jitter_ns: i64) {
        self.stamp_jitter_ns = jitter_ns;
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
        let value = self.shared.lock().expect("lock").now_value;
        Ok(MediaTime::new(value, DEVICE_CLOCK_TIMEBASE))
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
    /// open 前に設定する `stream.now()` の返り値（anchor 較正注入用）。
    now_value: i64,
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
                now_value: 0,
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

    /// open 前に `stream.now()` の返り値を設定する（anchor 較正注入用。
    /// 以後 open される全 stream に適用される）。
    pub fn set_now_value(&self, value: i64) {
        self.shared.lock().expect("lock").now_value = value;
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
            report_capture: false,
            stamp_epoch_ticks: 0,
            stamp_base: DEVICE_CLOCK_TIMEBASE,
            stamp_fixed_ticks: None,
            stamp_jitter_ns: 0,
            deliver_count: 0,
            now_value: g.now_value,
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

/// `RecordingRequest` → `preflight` の共通部。session sample rate・
/// buffer budget・`max_block_frames` を明示指定する（実機 lane が
/// negotiated rate と可変 callback block に対応するため）。
/// `plan_for`/`plan_gated` は本関数へ `RATE`/`budget()`/1024 で
/// 委譲する。
#[allow(clippy::too_many_arguments)]
pub fn plan_at(
    p: &Project,
    lanes: &[LaneId],
    channels: &[u32],
    epoch: ClockEpochId,
    gate: GateSpec,
    stop_tick: i64,
    sample_rate: u32,
    budget: BufferBudget,
    max_block_frames: u32,
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
        budget,
        max_block_frames,
    };
    preflight(p, req, sample_rate).expect("preflight must pass")
}

/// Normal gate・transport 0→`stop_tick` の plan（`project_with_lanes` と対で使う）。
pub fn plan_for(
    p: &Project,
    lanes: &[LaneId],
    channels: &[u32],
    epoch: ClockEpochId,
    stop_tick: i64,
) -> PreparedRecordingPlan {
    plan_at(
        p,
        lanes,
        channels,
        epoch,
        GateSpec::Normal {
            open: MusicalTick::ZERO,
        },
        stop_tick,
        RATE,
        budget(),
        1024,
    )
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
// （実 backend は `real-device` feature の opt-in lane
// `tests/real_input.rs` で別途検証する）。

/// `GateSpec` 指定の plan（`plan_for` の Gate 一般化版）。
pub fn plan_gated(
    p: &Project,
    lanes: &[LaneId],
    channels: &[u32],
    epoch: ClockEpochId,
    gate: GateSpec,
    stop_tick: i64,
) -> PreparedRecordingPlan {
    plan_at(
        p,
        lanes,
        channels,
        epoch,
        gate,
        stop_tick,
        RATE,
        budget(),
        1024,
    )
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

/// rig 共通の構築部: staging stream 設定 → `StagingArea` →
/// `InputSession::open`/`add`（same-clock 検証込み）→ armed tap ごとの
/// `StreamWriter` 接続。`DeviceRig`（fake handle 追跡）と
/// `RealDeviceRig`（実 device）で共有する。`sample_rate` は
/// staging manifest / finalize の rate 権威。
#[allow(clippy::type_complexity)]
fn open_rig_core(
    root: &Path,
    session_id: RecordingSessionId,
    plan: &PreparedRecordingPlan,
    sample_rate: u32,
    inputs: &[(&dyn Device, &InputStreamSpec)],
    stream_ids: Option<&[CaptureStreamId]>,
) -> (InputSession, Vec<(u16, StreamWriter)>, StagingArea) {
    assert!(!inputs.is_empty(), "at least one input required");
    let specs: Vec<&InputStreamSpec> = inputs.iter().map(|(_, s)| *s).collect();
    let streams_cfg = staging_streams(plan, &specs, stream_ids);
    let area = StagingArea::create(
        root,
        &StagingSessionConfig {
            session_id,
            sample_rate,
            streams: streams_cfg,
        },
    )
    .expect("staging create");

    let (first_dev, first_spec) = inputs[0];
    let (mut session, first_ends) =
        InputSession::open(first_dev, first_spec).expect("session open");
    let mut writer_ends = first_ends;
    for (dev, spec) in &inputs[1..] {
        let ends = session.add(*dev, spec).expect("session add");
        writer_ends.extend(ends);
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
            StreamWriter::create(&area, cfg, w.ends, fault.clone()).expect("stream writer create"),
        ));
    }
    writers.sort_by_key(|(i, _)| *i);
    (session, writers, area)
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
        let dyn_inputs: Vec<(&dyn Device, &InputStreamSpec)> = inputs
            .iter()
            .map(|&(d, s)| {
                let dev: &dyn Device = d;
                (dev, s)
            })
            .collect();
        let (session, writers, area) =
            open_rig_core(root, session_id, &plan, RATE, &dyn_inputs, stream_ids);

        // scripted handle は open 順の per-device ordinal で引く
        // （`open_input_stream` 呼出し順に `streams` へ push される）。
        let mut handles = Vec::with_capacity(inputs.len());
        let mut per_device: HashMap<usize, usize> = HashMap::new();
        for (dev, _) in inputs {
            let k = per_device.entry(dev.instance_id()).or_insert(0);
            handles.push(dev.stream(*k));
            *k += 1;
        }
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

/// 実 `Device` → `InputSession`/`InputAdapter` → 実 `StreamWriter` →
/// staging media/journal を結合した録音 rig（`tests/real_input.rs`
/// の `real-device` opt-in lane 用）。
///
/// fake `DeviceRig` と異なり scripted handle を持たない — PCM は
/// device の非同期 callback が供給し、test は wall-clock と fault
/// latch のみを観測する。`inputs` は `&dyn Device` のため、1 spec
/// の複数 armed tap（2 track）・same-clock 複数 device の双方を
/// 表現できる。`sample_rate` は negotiated format 由来で、
/// staging manifest / finalize ref の rate 権威と一致させる。
pub struct RealDeviceRig {
    /// same-clock 検証済みの入力 session（全 stream で共有する `FaultLatch`）。
    pub session: InputSession,
    /// `(stream_index, StreamWriter)` を `stream_index` 昇順で保持。
    pub writers: Vec<(u16, StreamWriter)>,
    /// staging 領域（manifest/media/journal/finalized/capture-fault）。
    pub area: StagingArea,
    /// 駆動に使う確定 plan（`pump_all` の scheduler 供給元）。
    pub plan: PreparedRecordingPlan,
    /// session sample rate（staging/finalize に使う negotiated 値）。
    pub sample_rate: u32,
}

impl RealDeviceRig {
    /// `inputs`（open 順の `(device, spec)` 列）から rig を構築する。
    /// `DeviceRig::open` と同じ same-clock 検証・staging/writer 接続を
    /// `open_rig_core` 経由で行う。
    pub fn open(
        root: &Path,
        session_id: RecordingSessionId,
        plan: PreparedRecordingPlan,
        sample_rate: u32,
        inputs: &[(&dyn Device, &InputStreamSpec)],
        stream_ids: Option<&[CaptureStreamId]>,
    ) -> Self {
        let (session, writers, area) =
            open_rig_core(root, session_id, &plan, sample_rate, inputs, stream_ids);
        Self {
            session,
            writers,
            area,
            plan,
            sample_rate,
        }
    }

    /// 単一 `(device, spec)` の簡易 open。
    pub fn open_one(
        root: &Path,
        session_id: RecordingSessionId,
        plan: PreparedRecordingPlan,
        sample_rate: u32,
        device: &dyn Device,
        spec: &InputStreamSpec,
    ) -> Self {
        Self::open(root, session_id, plan, sample_rate, &[(device, spec)], None)
    }

    /// 全 stream を `start` する（非 RT control path）。最初の失敗で
    /// return（起動済み stream は Drop/stop で回収される）。
    pub fn start_all(&self) -> DeviceResult<()> {
        for a in self.session.adapters() {
            a.stream().start()?;
        }
        Ok(())
    }

    /// 全 stream を `stop` する。失敗があっても残りを止めて最初の
    /// error を返す（部分停止を残さない）。
    pub fn stop_all(&self) -> DeviceResult<()> {
        let mut first_err = None;
        for a in self.session.adapters() {
            if let Err(e) = a.stream().stop()
                && first_err.is_none()
            {
                first_err = Some(e);
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// `duration` の間 `interval` 毎に `pump_all` を回す wall-clock
    /// 駆動。fault latch が立てば早期 return（fail-stop 後の入力は
    /// callback 側で捨てられる）。戻り値は pump 実行回数（観測記録用）。
    ///
    /// 待機は `thread::sleep` — OS の sleep 粒度（Windows 既定
    /// ~15 ms）で実効 cadence は ~15 ms になるが、実機 lane の
    /// buffer はそれを吸収するサイズに取る（`real_budget` の
    /// chunk pool 64 ≈ 640 ms、長時間 lane は 256 ≈ 2.5 s の
    /// stall 耐性）。busy-wait/spin は WASAPI worker thread を
    /// 飢餓させて `DATA_DISCONTINUITY` を誘発し得るため使わない
    /// （実機 lane での実測に基づく）。
    pub fn run_for(&mut self, duration: Duration, interval: Duration) -> usize {
        let deadline = Instant::now() + duration;
        let mut pumps = 0usize;
        while Instant::now() < deadline {
            if self.session.fault().is_faulted() {
                break;
            }
            self.pump_all();
            pumps += 1;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            thread::sleep(interval.min(remaining));
        }
        pumps
    }

    /// `pump_all` → 全 writer `pump`（RT→non-RT の drain）。
    pub fn pump_all(&mut self) {
        self.session.pump_all(&self.plan.scheduler);
        for (_, w) in &mut self.writers {
            w.pump();
        }
        if *RIG_DEBUG.get_or_init(|| std::env::var("RIG_DEBUG").is_ok()) {
            for (i, w) in &self.writers {
                eprintln!(
                    "  [rig] stream {i}: media_len={} state={:?} reason={:?} fault={:#x}",
                    w.media_len(),
                    w.state(),
                    w.failure_reason(),
                    self.session.fault().bits()
                );
            }
        }
    }

    /// engine のみ pump（writer を止める filled-queue 溢れ注入用）。
    pub fn pump_engines(&mut self) {
        self.session.pump_all(&self.plan.scheduler);
    }

    /// control path の format 照合を全 adapter に実行する。
    pub fn verify_formats(&self) {
        for a in self.session.adapters() {
            a.verify_format();
        }
    }

    /// fault latch が立っていれば session スコープの capture-fault
    /// marker を durable に記録し、checkpoint → finalize を実行する。
    /// `finish_all` の `sample_rate` は negotiated `self.sample_rate`
    /// （`DeviceRig::finish` の `RATE` 固定と異なり実機 rate）。
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
        finish_all(&tag, self.sample_rate, writers)
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
    spans.iter().map(|s| i64::from(s.frame_count)).sum()
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

/// 実機 PCM 用の構造検証（`verify_capture_pcm` の deterministic
/// 期待値比較を除いた版）。各 chunk の `pcm_crc32` を media 実
/// bytes と照合し、media offset 連続性・span の media 範囲・
/// PCM の有限値を確認する。実機 PCM は `MockPcm` 期待値を持たない
/// ため、構造と観測値のみで Pass を構成する。
///
/// `channels` は stream の出力 channel 数（`channel_map.len()`）。
/// 戻り値は検証した出力 PCM sample 数（観測記録用）。
pub fn verify_capture_integrity(cap: &StreamCapture, channels: u32) -> usize {
    let ch = usize::try_from(channels).expect("channels");
    assert!(ch > 0, "channels must be >= 1");
    let mut cursor = 0u64;
    let mut samples = 0usize;
    for c in &cap.chunks {
        let start = usize::try_from(c.media_offset).expect("media offset");
        let len = usize::try_from(c.media_len).expect("media len");
        assert_eq!(
            u64::try_from(start).unwrap_or(u64::MAX),
            cursor,
            "chunk {} media_offset gap/overlap",
            c.chunk_id
        );
        let end = start.checked_add(len).expect("media range");
        assert!(
            end <= cap.media.len(),
            "chunk {} media out of bounds",
            c.chunk_id
        );
        assert_eq!(
            crc32(&cap.media[start..end]),
            c.pcm_crc32,
            "chunk {} pcm_crc32 mismatch",
            c.chunk_id
        );
        cursor = u64::try_from(end).unwrap_or(u64::MAX);
        for s in &c.spans {
            let span_start = usize::try_from(s.media_offset).expect("span offset");
            let span_frames = usize::try_from(s.frame_count).expect("span frames");
            let span_bytes = span_frames
                .checked_mul(ch)
                .and_then(|f| f.checked_mul(4))
                .expect("span bytes");
            assert!(
                span_start >= start && span_start + span_bytes <= end,
                "span media range outside chunk {} ({s:?})",
                c.chunk_id
            );
            samples += span_frames * ch;
        }
    }
    // journaled prefix の PCM が全て有限値であること（NaN/Inf 検出）。
    // media file 末尾の unjournaled tail は検証対象外。
    let journaled = usize::try_from(cursor)
        .unwrap_or(usize::MAX)
        .min(cap.media.len());
    for (i, bytes) in cap.media[..journaled].as_chunks::<4>().0.iter().enumerate() {
        let v = f32::from_le_bytes(*bytes);
        assert!(v.is_finite(), "non-finite PCM sample at index {i}");
    }
    samples
}

/// test 用一時 directory（process id + name で一意、事前に消す）。
pub fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("synvelis-rcd01-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}
