//! Ruviform `device` port への入力 adapter（VS#115 RCD-01 slice）。
//!
//! `Device::open_input_stream` が返す `InputStream` の data callback
//! （planar `AudioBufferView<f32>` + `InputCallbackInfo`）を、既存の
//! `InputBlock` / `CaptureEngine` / SPSC handoff / writer 経路へ接続する。
//!
//! 責務分界:
//!
//! - Ruviform: generic device/driver/RT（`Device`/`InputStream`/
//!   `InputCallbackInfo`/`same_clock_domain`）。本モジュールは再実装しない。
//! - 本モジュール: channel→track `InputBinding`、Arm/Monitor 独立状態、
//!   channel map、callback→`InputBlock` 変換、planar→interleaved 有界
//!   コピー、clock domain 検証、`DeviceError`→`FAULT_*` 写像。
//!
//! RT 経路（data/error callback）では allocation/dealloc・blocking・
//! I/O・panic を行わない。callback は検証と interleave を事前確保の
//! work queue / PCM slot のみで行い、engine 処理は非 RT の `pump`
//! （または RT-safe な worker）が work queue を drain して行う。
//!
//! `ClockDomainId`（`ruviform-media-core`）は本モジュールの公開型に
//! 現れない。domain 判定は常に `same_clock_domain` を生値に対して呼び、
//! open 時に `Arc<OnceLock<_>>` へ stream の domain を一度だけ書き込む
//! （型は推論に委ね、名指ししない）。

#![allow(clippy::module_name_repetitions)]

use std::sync::{Arc, OnceLock};

use ruviform_audio_core::device::{
    DEVICE_CLOCK_TIMEBASE, Device, DeviceError, InputCallbackInfo, InputDataCallback,
    InputErrorCallback, InputStream, InputStreamConfig, same_clock_domain,
};
use ruviform_audio_core::{AudioBufferView, AudioFormat, SampleFormat};
use ruviform_media_core::{
    ClockAnchor, ClockEpochId as MediaClockEpochId, EngineFrame, MapRejection, MediaDuration,
    Rounding, TimeBase, TimestampProvenance, map_stamp_to_frame,
};
use ruviform_rt::{Consumer, Producer};
use synvelis_project::{ClockEpochId, SessionFrame};

use crate::{
    clock::InputBlock,
    engine::{CaptureEngine, EngineEndsBack},
    gate::GateScheduler,
    handoff::{
        FAULT_ARITHMETIC, FAULT_CONTRACT, FAULT_EPOCH_DISCONTINUITY, FAULT_INPUT_DISCONTINUITY,
        FAULT_QUEUE_FULL, FaultLatch, WriterEnds, build_handoff,
    },
    plan::BufferBudget,
};

/// 出力 channel → 入力 channel の写像（channel→track binding の中核）。
///
/// `channels[j]` は出力 channel `j` を供給する入力 channel 番号。
/// 長さが capture channel 数を決める。入力 channel 番号の実範囲は
/// stream open 時の format と照合して検証する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelMap {
    channels: Vec<u32>,
}

impl ChannelMap {
    /// `channels`（出力 channel 順の入力 channel 番号列）から構築する。
    ///
    /// # Errors
    /// 空の写像は [`InputBindError::EmptyChannelMap`]。
    pub fn new(channels: Vec<u32>) -> Result<Self, InputBindError> {
        if channels.is_empty() {
            return Err(InputBindError::EmptyChannelMap);
        }
        Ok(Self { channels })
    }

    /// `0..channels` の恒等写像（MultiChannel binding）。
    #[must_use]
    pub fn identity(channels: u32) -> Self {
        Self {
            channels: (0..channels).collect(),
        }
    }

    /// 単一入力 channel を取る写像（Mono binding）。
    #[must_use]
    pub fn mono(input_channel: u32) -> Self {
        Self {
            channels: vec![input_channel],
        }
    }

    /// 2 入力 channel を取る写像（Stereo binding）。
    #[must_use]
    pub fn stereo(left: u32, right: u32) -> Self {
        Self {
            channels: vec![left, right],
        }
    }

    /// capture channel 数（= 入力 channel 参照数）。
    #[must_use]
    pub fn len(&self) -> usize {
        self.channels.len()
    }

    /// 常に `false`（`new` が非空を保証）。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    /// 入力 channel 番号列（出力 channel 順）。
    #[must_use]
    pub fn channels(&self) -> &[u32] {
        &self.channels
    }
}

/// Record Arm の状態。
///
/// Arm Off の tap は capture path（engine/handoff/writer 接続）を持たず、
/// callback は raw PCM を記録しない。Arm は録音 pass 内で固定される
/// binding 時の状態であり、Monitor とは独立に評価される。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ArmState {
    /// Arm Off: raw capture を行わない（Writer を作成しない）。
    #[default]
    Off,
    /// Arm On: raw capture を engine→handoff→writer 経路へ流す。
    On,
}

/// Software Monitor の宣言モード（RCD-03 で `Auto` を追加）。
///
/// raw 録音とは独立した状態。Monitor On/Auto は capture/Writer を
/// 作らず、Arm Off + Monitor On でも Writer は作成されない。
/// `Auto` の region ごとの解決（Arm/Transport/Gate/Mute/Solo →
/// 可聴 route）は [`crate::monitor::decide`] が担い、本 enum は
/// binding 時の宣言モードのみを保持する。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum MonitorState {
    /// Monitor Off: Software Input Off。gate open 区間は既存
    /// Playback を抑制する（[`crate::monitor`] の判定規則）。
    #[default]
    Off,
    /// Monitor Auto: Arm 有効時は停止中 Input・gate open 中 Input・
    /// それ以外は既存 Playback。Arm Off では Input 無効。
    Auto,
    /// Monitor On: 入力 Binding/Output Capability が有効なら
    /// Arm なしでも Software Monitor 可（Recording は開始しない）。
    On,
}

/// 1 capture stream（track）への入力 binding。
///
/// channel→track 写像・Record Arm・Software Monitor を独立した状態と
/// して保持する。Arm/Monitor の遷移は binding（録音 pass）の単位で行い、
/// 本スライスでは構築後に変更しない。
#[derive(Clone, Debug)]
pub struct InputBinding {
    /// 出力 channel → 入力 channel の写像。
    pub channel_map: ChannelMap,
    /// Record Arm。
    pub arm: ArmState,
    /// Software Monitor。
    pub monitor: MonitorState,
}

/// armed tap 1 件分の engine 構築パラメータ。
#[derive(Clone, Debug)]
pub struct InputTapSpec {
    /// channel→track binding。
    pub binding: InputBinding,
    /// capture stream 識別子（engine/writer 経路。armed tap 間で一意必須）。
    pub stream_index: u16,
    /// session 終端（`PreparedRecordingPlan::session_end` 由来）。
    pub session_end: SessionFrame,
    /// buffer 予算（handoff pool / engine lookahead / filled queue）。
    pub budget: BufferBudget,
}

/// 1 入力 stream 分の open 要求。
#[derive(Clone, Debug)]
pub struct InputStreamSpec {
    /// `Device::open_input_stream` に渡す config。`SampleFormat::F32` 必須
    /// （callback の `AudioBufferView<f32>` に対応するため）。
    pub config: InputStreamConfig,
    /// `InputBlock` に刻む録音 Clock Epoch（同一 stream の全 tap で共有）。
    pub clock_epoch: ClockEpochId,
    /// 1 callback の最大 frame 数。これを超える callback は CONTRACT fault。
    pub max_block_frames: u32,
    /// callback→pump の in-flight 上限（tap ごとの work queue 容量かつ
    /// PCM slot 数。1 以上）。
    pub callback_queue: usize,
    /// tap ごとの binding + engine パラメータ。
    pub taps: Vec<InputTapSpec>,
}

/// tap の binding 状態の記録（Arm/Monitor 独立状態の観測用）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TapRecord {
    /// capture stream 識別子。
    pub stream_index: u16,
    /// capture channel 数（`channel_map` 長）。
    pub channels: u32,
    /// Record Arm 状態。
    pub arm: ArmState,
    /// Monitor 状態。
    pub monitor: MonitorState,
    /// capture path が構築されたか（`arm == On`）。
    pub capturing: bool,
}

/// `InputAdapter::open` / `InputSession::add` の失敗。
#[derive(Clone, Debug, PartialEq)]
pub enum InputBindError {
    /// `spec.taps` が空。
    EmptyTaps,
    /// `max_block_frames` または `callback_queue` が 0。
    ZeroCapacity,
    /// `channel_map` が空。
    EmptyChannelMap,
    /// channel map が config の channel 数を超える入力 channel を参照。
    ChannelOutOfRange,
    /// armed tap 間で `stream_index` が重複。
    DuplicateStreamIndex,
    /// `config.format.sample_format` が `F32` でない。
    UnsupportedSampleFormat,
    /// PCM/work queue 容量の算術オーバーフロー。
    CapacityOverflow,
    /// open 後の `stream.format()` が要求 config と不一致。
    FormatMismatch,
    /// clock domain が `UNKNOWN`（`same_clock_domain` が常に false）。
    ClockDomainUnknown,
    /// session への追加 stream が既存 stream と別 clock domain
    /// （`same_clock_domain` が false。不一致・UNKNOWN を含む）。
    ClockDomainMismatch,
    /// session への追加 stream が先頭 stream と別の Clock Epoch を
    /// 要求（1 recording pass の全 stream は同一 epoch を共有する）。
    ClockEpochMismatch,
    /// device 側の `open_input_stream` 失敗。
    Device(DeviceError),
}

impl core::fmt::Display for InputBindError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyTaps => f.write_str("input stream spec has no taps"),
            Self::ZeroCapacity => f.write_str("max_block_frames/callback_queue must be >= 1"),
            Self::EmptyChannelMap => f.write_str("channel map must not be empty"),
            Self::ChannelOutOfRange => {
                f.write_str("channel map references a channel outside the stream format")
            }
            Self::DuplicateStreamIndex => f.write_str("duplicate stream_index among armed taps"),
            Self::UnsupportedSampleFormat => {
                f.write_str("input adapter requires SampleFormat::F32")
            }
            Self::CapacityOverflow => f.write_str("input adapter capacity overflow"),
            Self::FormatMismatch => {
                f.write_str("opened stream format does not match the requested config")
            }
            Self::ClockDomainUnknown => f.write_str("input stream clock domain is UNKNOWN"),
            Self::ClockDomainMismatch => {
                f.write_str("input stream clock domain differs from the session's first stream")
            }
            Self::ClockEpochMismatch => {
                f.write_str("input stream clock epoch differs from the session's first stream")
            }
            Self::Device(e) => write!(f, "device open failed: {e}"),
        }
    }
}

impl std::error::Error for InputBindError {}

/// callback→pump 間を流れる 1 block 分の仕事。
struct WorkItem {
    /// device input frame 由来の block 記述。
    block: InputBlock,
    /// interleave 済み PCM。容量は `max_block_frames * tap channels` で、
    /// 実使用長は `block.frames * channels`（pump が slice して渡す）。
    /// pump 完了後に free queue へ返却され再利用される。
    pcm: Box<[f32]>,
}

/// data callback 側が保持する tap 状態（closure capture 用）。
struct TapCallback {
    /// pump 側への work queue producer。
    work_tx: Producer<WorkItem>,
    /// pump 側から返却される PCM slot の consumer。
    pcm_free_rx: Consumer<Box<[f32]>>,
    /// push 失敗時の一時退避（RT 上で dealloc しないため）。
    spare: Option<Box<[f32]>>,
    /// 出力 channel → 入力 channel の写像。
    channel_map: Vec<u32>,
    /// capture channel 数（`channel_map.len()`）。
    out_channels: usize,
}

/// pump 側が保持する tap 状態。
struct TapPump {
    /// RT capture engine。
    engine: CaptureEngine,
    /// capture stream 識別子。
    stream_index: u16,
    /// capture channel 数。
    channels: usize,
    /// session 終端（lookahead `until` の clamp 用）。
    session_end: SessionFrame,
    /// callback 側からの work queue consumer。
    work_rx: Consumer<WorkItem>,
    /// 消費済み PCM slot の返却 producer。
    pcm_free_tx: Producer<Box<[f32]>>,
}

/// `InputAdapter::open` の戻り値。
pub struct OpenedInput {
    /// pump/制御側 adapter。
    pub adapter: InputAdapter,
    /// armed tap ごとの Writer endpoint（spec 順）。
    pub writer_ends: Vec<TapWriterEnds>,
}

/// armed tap の Writer 側 endpoint。
pub struct TapWriterEnds {
    /// capture stream 識別子。
    pub stream_index: u16,
    /// WriterEnds（filled consumer + free producer）。
    pub ends: WriterEnds,
}

/// `InputAdapter::into_parts` の戻り値（非 RT teardown）。
#[derive(Default)]
pub struct InputAdapterParts {
    /// 開いた stream（start/stop/Drop 管理は呼び出し側）。
    pub stream: Option<Box<dyn InputStream>>,
    /// armed tap ごとの engine endpoint。
    pub taps: Vec<TapEngineEnds>,
    /// 共有 Fault latch。
    pub fault: Option<FaultLatch>,
}

/// teardown 時に回収する 1 tap 分の engine endpoint。
pub struct TapEngineEnds {
    /// capture stream 識別子。
    pub stream_index: u16,
    /// `CaptureEngine::into_ends` の戻り（filled/free endpoint + retained）。
    pub ends: EngineEndsBack,
}

/// 1 入力 stream 分の adapter。
///
/// data callback（RT）→ work queue → `pump` → `CaptureEngine` →
/// handoff → writer の経路を束ねる。`pump` は work queue を drain し、
/// engine の lookahead schedule を補充してから `process` を呼ぶ
/// （`Scenario::feed` と同じ順序を queue 越しに再現する）。
pub struct InputAdapter {
    stream: Box<dyn InputStream>,
    opened_format: AudioFormat,
    clock_epoch: ClockEpochId,
    taps: Vec<TapPump>,
    records: Vec<TapRecord>,
    fault: FaultLatch,
}

impl InputAdapter {
    /// `device` から入力 stream を開き、`spec` の binding に従って
    /// callback を接続する。
    ///
    /// - 全 tap の `channel_map` が config channel 数の範囲内であること、
    ///   armed tap の `stream_index` が一意であることを検証する。
    /// - `config.format.sample_format` は `F32` 必須。
    /// - open 後に `stream.format()` が config と一致すること、clock
    ///   domain が `UNKNOWN` でないことを検証する（fail closed）。
    /// - Arm Off の tap は engine/handoff/writer endpoint を構築せず、
    ///   callback 対象にも含めない（Monitor On でも raw capture しない）。
    ///
    /// `fault` は engine・callback・error callback で共有する latch。
    ///
    /// # Errors
    /// spec 不正・format 不一致・clock domain UNKNOWN・device open 失敗。
    pub fn open(
        device: &dyn Device,
        spec: &InputStreamSpec,
        fault: FaultLatch,
    ) -> Result<OpenedInput, InputBindError> {
        let expected_channels = validate_spec(spec)?;

        // --- tap 記録（全 tap。Arm/Monitor 独立状態の観測用）---
        let records = tap_records(&spec.taps)?;

        // --- armed tap の resource 構築（engine/handoff/queue/PCM pool）---
        let resources = build_tap_resources(
            &spec.taps,
            spec.max_block_frames,
            spec.callback_queue,
            &fault,
        )?;
        let mut taps_cb = resources.taps_cb;

        // --- callback 構築（RT。確保済み storage のみ使用）---
        // `domain_cell` は open 後に `stream.clock_domain()` を一度だけ
        // 書き込む。`ClockDomainId` の型名は書かず推論に委ねる。
        let domain_cell = Arc::new(OnceLock::new());
        let cell_for_cb = Arc::clone(&domain_cell);
        // stamp placement は stream open 後に `now()` で較正するため、
        // callback 側は `OnceLock` 経由で読む（callback は start 後に
        // しか発火しないので較正完了が保証される）。
        let stamp_cell = Arc::new(OnceLock::new());
        let cell_for_stamp = Arc::clone(&stamp_cell);
        let fault_for_cb = fault.clone();
        let clock_epoch = spec.clock_epoch;
        let max_frames = usize::try_from(spec.max_block_frames).unwrap_or(usize::MAX);
        let exp_channels = expected_channels;
        let frame_base = spec.config.format.sample_rate.time_base();
        // stamp 経路の累積配置状態（callback 内でのみ更新）。
        let mut stamp_state = StampState::default();

        let data_callback: InputDataCallback = Box::new(
            move |view: AudioBufferView<'_, f32>, info: InputCallbackInfo| {
                if fault_for_cb.is_faulted() {
                    return;
                }
                // clock domain: stream open 時の値と per-callback 値を
                // `same_clock_domain` で照合（UNKNOWN は常に false）。
                let domain_ok = cell_for_cb
                    .get()
                    .is_some_and(|d| same_clock_domain(info.clock_domain, *d));
                if !domain_ok {
                    fault_for_cb.set(FAULT_EPOCH_DISCONTINUITY);
                    return;
                }
                // frame 位置: `frame_position` を優先し、無ければ
                // capture stamp 経路（初回 seed + 累積 count 配置 +
                // stamp 連続性検証）。両方無い・写像不能は fail closed
                // （値の捏造はしない）。
                let frames = view.frames();
                let frames_u64 = u64::try_from(frames).unwrap_or(u64::MAX);
                let first_frame = match resolve_first_frame(
                    &info,
                    cell_for_stamp.get().copied(),
                    frames_u64,
                    &mut stamp_state,
                ) {
                    Ok(frame) => frame,
                    Err(bit) => {
                        fault_for_cb.set(bit);
                        return;
                    }
                };
                if frames == 0 || frames > max_frames || view.channels() != exp_channels {
                    fault_for_cb.set(FAULT_CONTRACT);
                    return;
                }
                let Ok(frames_u32) = u32::try_from(frames) else {
                    fault_for_cb.set(FAULT_CONTRACT);
                    return;
                };
                let block = InputBlock {
                    clock_epoch,
                    first_input_frame: first_frame,
                    frames: frames_u32,
                };
                for tap in &mut taps_cb {
                    feed_tap(tap, &view, block, &fault_for_cb);
                    if fault_for_cb.is_faulted() {
                        return;
                    }
                }
            },
        );

        // --- stream open + format/domain 検証 ---
        let stream = device
            .open_input_stream(
                &spec.config,
                data_callback,
                input_error_callback(fault.clone()),
            )
            .map_err(InputBindError::Device)?;
        let domain = stream.clock_domain();
        // `same_clock_domain(d, d)` は d == UNKNOWN のときのみ false。
        // UNKNOWN domain は per-callback 照合が常に失敗するため open で拒否。
        if !same_clock_domain(domain, domain) {
            return Err(InputBindError::ClockDomainUnknown);
        }
        let _ = domain_cell.set(domain);
        if stream.format() != spec.config.format {
            return Err(InputBindError::FormatMismatch);
        }
        // stamp→frame anchor の較正（非 RT）。構築できない backend
        // では `None` のまま — stamp 経路は callback で fail-stop、
        // `frame_position` 経路は影響を受けない。
        if let Some(placement) = calibrate_stamp_placement(&*stream, frame_base) {
            let _ = stamp_cell.set(placement);
        }

        Ok(OpenedInput {
            adapter: InputAdapter {
                stream,
                opened_format: spec.config.format,
                clock_epoch: spec.clock_epoch,
                taps: resources.pumps,
                records,
                fault,
            },
            writer_ends: resources.writer_ends,
        })
    }

    /// work queue を drain し、各 tap の engine lookahead を補充してから
    /// `process` を呼ぶ（非 RT pump。RT thread から呼んでも各操作は
    /// bounded だが、意図する呼び出し側は worker/control thread）。
    ///
    /// 消費した PCM slot は free queue へ返却して再利用する。
    pub fn pump(&mut self, scheduler: &GateScheduler) {
        for tap in &mut self.taps {
            while let Ok(item) = tap.work_rx.try_pop() {
                let until = tap
                    .engine
                    .session_cursor()
                    .checked_add(i64::from(item.block.frames))
                    .unwrap_or(tap.session_end)
                    .min(tap.session_end);
                let _ = scheduler.extend(tap.engine.schedule_mut(), until);
                let need = usize::try_from(item.block.frames)
                    .ok()
                    .and_then(|f| f.checked_mul(tap.channels));
                match need {
                    Some(n) if n <= item.pcm.len() => {
                        tap.engine.process(&item.block, &item.pcm[..n]);
                    }
                    _ => {
                        // enqueue 時に検証済みのはずの不変条件違反。
                        self.fault.set(FAULT_CONTRACT);
                    }
                }
                if tap.pcm_free_tx.try_push(item.pcm).is_err() {
                    // pool 会計の不変条件違反（容量 == queue 容量）。
                    self.fault.set(FAULT_CONTRACT);
                }
            }
        }
    }

    /// `stream.format()` を再照合し、不一致（rate/format 変更の検出）なら
    /// `FAULT_CONTRACT` を立てる。非 RT の control path から呼ぶ。
    pub fn verify_format(&self) {
        if self.stream.format() != self.opened_format {
            self.fault.set(FAULT_CONTRACT);
        }
    }

    /// 開いた stream への参照（`clock_domain()`/`format()` 等の照合用）。
    #[must_use]
    pub fn stream(&self) -> &dyn InputStream {
        &*self.stream
    }

    /// 共有 Fault latch。
    #[must_use]
    pub fn fault(&self) -> &FaultLatch {
        &self.fault
    }

    /// いずれかの Fault が立っているか。
    #[must_use]
    pub fn is_faulted(&self) -> bool {
        self.fault.is_faulted()
    }

    /// tap ごとの binding 状態記録（Arm/Monitor 独立状態の観測用）。
    #[must_use]
    pub fn tap_records(&self) -> &[TapRecord] {
        &self.records
    }

    /// `InputBlock` に刻む Clock Epoch（`InputStreamSpec::clock_epoch`）。
    #[must_use]
    pub fn clock_epoch(&self) -> ClockEpochId {
        self.clock_epoch
    }

    /// adapter を分解して stream・engine endpoint・latch を返す
    /// （非 RT teardown。work queue に残った item は drop されるため、
    /// teardown 前に `pump` で drain すること）。
    #[must_use]
    pub fn into_parts(self) -> InputAdapterParts {
        let Self {
            stream,
            taps,
            fault,
            ..
        } = self;
        InputAdapterParts {
            stream: Some(stream),
            taps: taps
                .into_iter()
                .map(|t| TapEngineEnds {
                    stream_index: t.stream_index,
                    ends: t.engine.into_ends(),
                })
                .collect(),
            fault: Some(fault),
        }
    }
}

/// `capture_stamp` → device input frame の較正済み写像。
///
/// `InputAdapter::open`（非 RT）で `stream.now()` から構築し、RT
/// callback では読むだけ。`Copy` で callback closure へ move する。
#[derive(Copy, Clone)]
struct StampPlacement {
    /// `map_stamp_to_frame` の権威となる stamp↔frame 対応。
    anchor: ClockAnchor,
    /// engine frame の timebase（`1/sample_rate`）。
    frame_base: TimeBase,
    /// `map_stamp_to_frame` の `now` — epoch 整合検査専用の定数
    /// （較正後に epoch は変わらない）。
    now: EngineFrame,
}

/// stamp→frame 写像で使う `ruviform_media_core::ClockEpochId`。
/// 本 adapter 内で完結する固定値（`InputBlock.clock_epoch` の
/// `synvelis_project::ClockEpochId` とは別型。epoch 整合は
/// `map_stamp_to_frame` 内部の stale-anchor 検査専用）。
const STAMP_FRAME_EPOCH: MediaClockEpochId = MediaClockEpochId::new(0);

/// `stream.now()` から stamp→frame anchor を較正する（非 RT）。
///
/// `now()` が `DEVICE_CLOCK_TIMEBASE` の非負値を返せない場合は
/// `None` — stamp 経路は callback で fail-stop に落ち、
/// `frame_position` 経路には影響しない。
fn calibrate_stamp_placement(
    stream: &dyn InputStream,
    frame_base: TimeBase,
) -> Option<StampPlacement> {
    let now = stream.now().ok()?;
    if now.base() != DEVICE_CLOCK_TIMEBASE {
        return None;
    }
    let ticks = u64::try_from(now.value()).ok()?;
    Some(StampPlacement {
        anchor: ClockAnchor {
            domain: stream.clock_domain(),
            stamp_ticks: ticks,
            stamp_base: DEVICE_CLOCK_TIMEBASE,
            frame: EngineFrame::new(STAMP_FRAME_EPOCH, 0),
            calibration: TimestampProvenance::Derived,
        },
        frame_base,
        now: EngineFrame::new(STAMP_FRAME_EPOCH, 0),
    })
}

/// stamp 経路の per-callback 進行検証の許容誤差（frame 単位）。
///
/// 実測（WASAPI/Audient iD4）では連続 capture stamp の tick 差が
/// 真値の frame 時間から ±1 frame 未満ずれる sub-ms jitter を示す
/// （10 ms block で ±20 µs ≈ ±1 frame @44.1 kHz）。256 frame は
/// 48 kHz で約 5.3 ms — jitter を十分に吸収しつつ、1 buffer 規模の
/// xrun/gap（数百 frame 以上）を検出する。
const STAMP_DELTA_TOLERANCE_FRAMES: u64 = 256;

/// stamp 経路の anchor からの累積乖離許容（frame 単位）。
///
/// 48 kHz で約 0.34 秒相当。~100 ppm の clock drift を 1 時間超
/// 吸収し、持続的な divergence/xrun 列を検出する。単発の timestamp
/// jitter は累積しない（±数 frame に留まる）ため誤検出しない。
const STAMP_ABS_TOLERANCE_FRAMES: u64 = 16_384;

/// stamp 経路の累積配置状態（callback 内でのみ更新・非共有）。
///
/// frame 位置は累積 frame count で導出する（sample-exact・block 間
/// 連続性を保証）。capture stamp は (i) 初回 callback の seed と
/// (ii) 以後の連続性/ドリフト検証に使い、block 毎の独立写像は
/// 行わない（timestamp jitter で非連続になるため — 実機実測済み）。
#[derive(Default)]
struct StampState {
    /// seed 済みか（初回 stamp callback で `map_stamp_to_frame` の
    /// 結果を初期位置として一度だけ較正する）。
    seeded: bool,
    /// 次の callback に期待される先頭 device input frame
    /// （前 block の `first_input_frame + frames`）。
    next_first_frame: u64,
    /// 直前 callback の stamp tick（逆行検出用）。
    prev_stamp_ticks: u64,
    /// 直前 callback の frame 数（stamp 進行量の期待値）。
    prev_frames: u64,
}

/// stamp 経路で block 先頭の device input frame を解決する。
///
/// `frame_position` があればそちらを優先する（上流が device frame
/// counter を供給する将来経路）。stamp 経路では:
///
/// 1. `capture_stamp()` → `map_stamp_to_frame` で domain/base/epoch
///    を検証しつつ初回 seed を較正（配置値としては jitter を含む
///    ため以後は使わない）。
/// 2. 以後は `next_first_frame`（累積 count）を配置に使い、stamp の
///    逆行・per-callback 進行乖離・累積乖離を `FAULT_*` へ写像する。
///
/// 配置不能・契約違反は `Err(FAULT_* bit)` — 値を捏造しない。
/// RT-safe（純粋整数演算のみ、alloc なし）。
fn resolve_first_frame(
    info: &InputCallbackInfo,
    placement: Option<StampPlacement>,
    frames: u64,
    state: &mut StampState,
) -> Result<u64, u32> {
    if let Some(position) = info.frame_position {
        return Ok(position);
    }
    let Some(placement) = placement else {
        return Err(FAULT_INPUT_DISCONTINUITY);
    };
    let Some(stamp) = info.capture_stamp() else {
        return Err(FAULT_INPUT_DISCONTINUITY);
    };
    // domain/base/epoch の検証を兼ねた stamp→frame 写像。戻り値の
    // frame は jitter を含むため配置には使わず、rejection 検出と
    // 初回 seed・累積乖離の観測に使う。
    let mapped = map_stamp_to_frame(
        &stamp,
        &placement.anchor,
        placement.frame_base,
        placement.now,
    )
    .map_err(stamp_rejection_fault)?;
    let mapped_frame = mapped.frame.frame();

    if !state.seeded {
        state.seeded = true;
        state.next_first_frame = mapped_frame.checked_add(frames).ok_or(FAULT_ARITHMETIC)?;
        state.prev_stamp_ticks = stamp.ticks;
        state.prev_frames = frames;
        return Ok(mapped_frame);
    }

    // stamp の逆行は capture 時刻の物理的不可逆性に反する。
    let Some(delta_ticks) = stamp.ticks.checked_sub(state.prev_stamp_ticks) else {
        return Err(FAULT_INPUT_DISCONTINUITY);
    };
    let delta_i64 = i64::try_from(delta_ticks).map_err(|_| FAULT_ARITHMETIC)?;
    let delta_frames = MediaDuration::new(delta_i64, stamp.base)
        .convert(placement.frame_base, Rounding::NearestEven)
        .map_err(|_| FAULT_ARITHMETIC)?
        .value();
    let prev_frames = i64::try_from(state.prev_frames).map_err(|_| FAULT_ARITHMETIC)?;
    if delta_frames.abs_diff(prev_frames) > STAMP_DELTA_TOLERANCE_FRAMES {
        return Err(FAULT_INPUT_DISCONTINUITY);
    }
    // 累積乖離（anchor からの絶対ドリフト）。配置は累積 count の
    // まま — jitter 範囲内の絶対差は許容する。
    if mapped_frame.abs_diff(state.next_first_frame) > STAMP_ABS_TOLERANCE_FRAMES {
        return Err(FAULT_INPUT_DISCONTINUITY);
    }

    let first = state.next_first_frame;
    state.next_first_frame = first.checked_add(frames).ok_or(FAULT_ARITHMETIC)?;
    state.prev_stamp_ticks = stamp.ticks;
    state.prev_frames = frames;
    Ok(first)
}

/// input stream の error callback を構築する。
///
/// fail-stop: error callback は Fault latch に写像するだけ
/// （確保・ブロック・panic なし）。
fn input_error_callback(fault: FaultLatch) -> InputErrorCallback {
    Box::new(move |err: DeviceError| {
        let bit = match err {
            DeviceError::ClockDiscontinuity => FAULT_EPOCH_DISCONTINUITY,
            DeviceError::StreamInvalidated | DeviceError::DeviceNotFound | DeviceError::Xrun => {
                FAULT_INPUT_DISCONTINUITY
            }
            _ => FAULT_CONTRACT,
        };
        fault.set(bit);
    })
}

/// `map_stamp_to_frame` の rejection を既存 `FAULT_*` bit へ写像する。
///
/// domain/base/epoch の不一致は clock 系列の同一性問題
/// （`same_clock_domain` 不一致・`DeviceError::ClockDiscontinuity` と
/// 同じ分類）。`BeforeAnchor` は配置不能な時刻逆行、`Overflow` は
/// チェック済み演算失敗。未知の rejection（`MapRejection` は
/// `non_exhaustive`）は契約違反として fail-stop。
fn stamp_rejection_fault(rejection: MapRejection) -> u32 {
    match rejection {
        MapRejection::UnmappedDomain | MapRejection::BaseMismatch | MapRejection::EpochMismatch => {
            FAULT_EPOCH_DISCONTINUITY
        }
        MapRejection::BeforeAnchor => FAULT_INPUT_DISCONTINUITY,
        MapRejection::Overflow => FAULT_ARITHMETIC,
        _ => FAULT_CONTRACT,
    }
}

/// `build_tap_resources` の戻り値（callback/pump/writer の 3 系列）。
struct TapResources {
    /// callback 側 tap 状態。
    taps_cb: Vec<TapCallback>,
    /// pump 側 tap 状態。
    pumps: Vec<TapPump>,
    /// armed tap の Writer endpoint（spec 順）。
    writer_ends: Vec<TapWriterEnds>,
}

/// `InputStreamSpec` の静的検証。config の channel 数を返す。
fn validate_spec(spec: &InputStreamSpec) -> Result<usize, InputBindError> {
    if spec.taps.is_empty() {
        return Err(InputBindError::EmptyTaps);
    }
    if spec.max_block_frames == 0 || spec.callback_queue == 0 {
        return Err(InputBindError::ZeroCapacity);
    }
    if spec.config.format.sample_format != SampleFormat::F32 {
        return Err(InputBindError::UnsupportedSampleFormat);
    }
    let expected_channels =
        usize::try_from(spec.config.format.layout.channels()).unwrap_or(usize::MAX);
    if expected_channels == 0 {
        return Err(InputBindError::EmptyChannelMap);
    }
    // 全 tap の map 非空・範囲、armed tap の stream_index 一意。
    let mut armed_indices: Vec<u16> = Vec::new();
    for tap in &spec.taps {
        if tap.binding.channel_map.is_empty() {
            return Err(InputBindError::EmptyChannelMap);
        }
        for &src in tap.binding.channel_map.channels() {
            if usize::try_from(src).unwrap_or(usize::MAX) >= expected_channels {
                return Err(InputBindError::ChannelOutOfRange);
            }
        }
        if tap.binding.arm == ArmState::On {
            if armed_indices.contains(&tap.stream_index) {
                return Err(InputBindError::DuplicateStreamIndex);
            }
            armed_indices.push(tap.stream_index);
        }
    }
    Ok(expected_channels)
}

/// 全 tap の binding 状態記録を spec 順に構築する。
fn tap_records(taps: &[InputTapSpec]) -> Result<Vec<TapRecord>, InputBindError> {
    taps.iter()
        .map(|tap_spec| {
            Ok(TapRecord {
                stream_index: tap_spec.stream_index,
                channels: u32::try_from(tap_spec.binding.channel_map.len())
                    .map_err(|_| InputBindError::CapacityOverflow)?,
                arm: tap_spec.binding.arm,
                monitor: tap_spec.binding.monitor,
                capturing: tap_spec.binding.arm == ArmState::On,
            })
        })
        .collect()
}

/// armed tap ごとの capture resource（engine/handoff/work queue/PCM
/// pool）を構築する。Arm Off の tap は capture path を持たないため
/// 対象外（Monitor On でも raw capture しない）。
fn build_tap_resources(
    taps: &[InputTapSpec],
    max_block_frames: u32,
    callback_queue: usize,
    fault: &FaultLatch,
) -> Result<TapResources, InputBindError> {
    let mut taps_cb: Vec<TapCallback> = Vec::new();
    let mut pumps: Vec<TapPump> = Vec::new();
    let mut writer_ends: Vec<TapWriterEnds> = Vec::new();
    for tap_spec in taps {
        if tap_spec.binding.arm != ArmState::On {
            continue;
        }
        let map_len = tap_spec.binding.channel_map.len();
        let channels_u32 = u32::try_from(map_len).map_err(|_| InputBindError::CapacityOverflow)?;
        let slot_len = usize::try_from(max_block_frames)
            .unwrap_or(usize::MAX)
            .checked_mul(map_len)
            .ok_or(InputBindError::CapacityOverflow)?;
        let (work_tx, work_rx) = ruviform_rt::channel::<WorkItem>(callback_queue);
        let (mut pcm_free_tx, pcm_free_rx) = ruviform_rt::channel::<Box<[f32]>>(callback_queue);
        for _ in 0..callback_queue {
            // pool 容量 == queue 容量なので全 slot が収まる。
            let _ = pcm_free_tx.try_push(vec![0.0; slot_len].into_boxed_slice());
        }
        let b = tap_spec.budget;
        let (engine_ends, writer_end) = build_handoff(
            tap_spec.stream_index,
            b.chunk_frames,
            channels_u32,
            b.spans_per_chunk,
            b.chunks,
            b.filled_capacity,
        );
        let engine = CaptureEngine::new(
            tap_spec.stream_index,
            channels_u32,
            tap_spec.session_end,
            engine_ends,
            fault.clone(),
            b.lookahead_spans,
        );
        writer_ends.push(TapWriterEnds {
            stream_index: tap_spec.stream_index,
            ends: writer_end,
        });
        taps_cb.push(TapCallback {
            work_tx,
            pcm_free_rx,
            spare: None,
            channel_map: tap_spec.binding.channel_map.channels().to_vec(),
            out_channels: map_len,
        });
        pumps.push(TapPump {
            engine,
            stream_index: tap_spec.stream_index,
            channels: map_len,
            session_end: tap_spec.session_end,
            work_rx,
            pcm_free_tx,
        });
    }
    Ok(TapResources {
        taps_cb,
        pumps,
        writer_ends,
    })
}

/// `InputAdapter::open` で callback に渡す 1 tap 分の処理。
///
/// planar `view` を channel map に従って interleave し、事前確保の
/// PCM slot に書き込んで work queue へ積む。すべて確保済み storage の
/// みを使い、allocation/blocking/I/O/panic を行わない。
fn feed_tap(
    tap: &mut TapCallback,
    view: &AudioBufferView<'_, f32>,
    block: InputBlock,
    fault: &FaultLatch,
) {
    // PCM slot 取得（spare 優先、なければ free queue から）。
    let mut pcm = tap.spare.take();
    if pcm.is_none() {
        pcm = tap.pcm_free_rx.try_pop().ok();
    }
    let Some(mut pcm) = pcm else {
        fault.set(FAULT_QUEUE_FULL);
        return;
    };
    let out_ch = tap.out_channels;
    let need = view.frames().saturating_mul(out_ch);
    if need > pcm.len() {
        // enqueue 前検証で排除済みのはずの不変条件違反。
        tap.spare = Some(pcm);
        fault.set(FAULT_CONTRACT);
        return;
    }
    // planar → interleaved: 出力 channel j は入力 channel_map[j] の
    // sample を frame ごとに並べる。channel index は open 時に範囲検証
    // 済みだが、backend の format 変更に備え再検証する。
    for (j, &src) in tap.channel_map.iter().enumerate() {
        let Ok(src_idx) = usize::try_from(src) else {
            tap.spare = Some(pcm);
            fault.set(FAULT_CONTRACT);
            return;
        };
        let Some(samples) = view.channel(src_idx) else {
            tap.spare = Some(pcm);
            fault.set(FAULT_CONTRACT);
            return;
        };
        for f in 0..view.frames() {
            pcm[f * out_ch + j] = samples[f];
        }
    }
    if let Err(e) = tap.work_tx.try_push(WorkItem { block, pcm }) {
        // queue full: 値を回収して spare へ退避（RT dealloc 回避）。
        let item = e.into_inner();
        tap.spare = Some(item.pcm);
        fault.set(FAULT_QUEUE_FULL);
    }
}

/// 同一 clock domain を強制する複数入力 stream の集合。
///
/// 先頭 stream の `clock_domain()` を基準とし、`add` で追加する stream は
/// `same_clock_domain` が真の場合のみ受理する（不一致・UNKNOWN は
/// `ClockDomainMismatch` で拒否。受理判定は open 済みの生値に対して
/// `same_clock_domain` を呼ぶ）。
pub struct InputSession {
    adapters: Vec<InputAdapter>,
    fault: FaultLatch,
}

impl InputSession {
    /// 最初の入力 stream を開く（以降の `add` の clock domain 基準）。
    ///
    /// # Errors
    /// `InputAdapter::open` と同じ。
    pub fn open(
        device: &dyn Device,
        spec: &InputStreamSpec,
    ) -> Result<(Self, Vec<TapWriterEnds>), InputBindError> {
        let fault = FaultLatch::new();
        let opened = InputAdapter::open(device, spec, fault.clone())?;
        let writer_ends = opened.writer_ends;
        let adapter = opened.adapter;
        Ok((
            Self {
                adapters: vec![adapter],
                fault,
            },
            writer_ends,
        ))
    }

    /// 2 本目以降の入力 stream を追加する。
    ///
    /// 追加 stream を open した上で、先頭 stream との
    /// `same_clock_domain` を検証する。不一致なら open 済み stream を
    /// drop（未 start のため副作用なし）して `ClockDomainMismatch` を返す。
    ///
    /// # Errors
    /// `InputAdapter::open` の失敗、または clock domain 不一致。
    pub fn add(
        &mut self,
        device: &dyn Device,
        spec: &InputStreamSpec,
    ) -> Result<Vec<TapWriterEnds>, InputBindError> {
        let Some(first) = self.adapters.first() else {
            return Err(InputBindError::EmptyTaps);
        };
        // 1 recording pass の全 stream は同一 Clock Epoch を共有する。
        if spec.clock_epoch != first.clock_epoch() {
            return Err(InputBindError::ClockEpochMismatch);
        }
        // stream_index は session 全体で一意（writer 経路識別のため）。
        for tap in &spec.taps {
            if tap.binding.arm == ArmState::On
                && self
                    .adapters
                    .iter()
                    .flat_map(InputAdapter::tap_records)
                    .any(|r| r.capturing && r.stream_index == tap.stream_index)
            {
                return Err(InputBindError::DuplicateStreamIndex);
            }
        }
        let opened = InputAdapter::open(device, spec, self.fault.clone())?;
        if !same_clock_domain(
            opened.adapter.stream().clock_domain(),
            first.stream().clock_domain(),
        ) {
            // open 済み stream は drop で停止（未 start）。engine/writer
            // endpoint は WriterEnds 未返却のまま drop される。
            return Err(InputBindError::ClockDomainMismatch);
        }
        let writer_ends = opened.writer_ends;
        self.adapters.push(opened.adapter);
        Ok(writer_ends)
    }

    /// 全 adapter の work queue を drain する（`pump` の一括版）。
    pub fn pump_all(&mut self, scheduler: &GateScheduler) {
        for adapter in &mut self.adapters {
            adapter.pump(scheduler);
        }
    }

    /// 各 stream の adapter（`verify_format`/`stream()`/`tap_records()` 用）。
    #[must_use]
    pub fn adapters(&self) -> &[InputAdapter] {
        &self.adapters
    }

    /// 共有 Fault latch（全 stream で共有）。
    #[must_use]
    pub fn fault(&self) -> &FaultLatch {
        &self.fault
    }

    /// session を adapter 列へ分解する（非 RT teardown）。
    #[must_use]
    pub fn into_adapters(self) -> Vec<InputAdapter> {
        self.adapters
    }
}

#[cfg(test)]
mod tests {
    use ruviform_audio_core::device::InputTimestamp;
    use ruviform_media_core::{ClockDomainId, MediaTime, MediaTimestamp};

    use super::*;

    const TEST_DOMAIN: ClockDomainId = ClockDomainId::MONOTONIC;
    const TEST_EPOCH: MediaClockEpochId = MediaClockEpochId::new(7);

    fn placement() -> StampPlacement {
        StampPlacement {
            anchor: ClockAnchor {
                domain: TEST_DOMAIN,
                stamp_ticks: 1_000_000,
                stamp_base: DEVICE_CLOCK_TIMEBASE,
                frame: EngineFrame::new(TEST_EPOCH, 0),
                calibration: TimestampProvenance::Derived,
            },
            frame_base: TimeBase::sample_rate(48_000).expect("base"),
            now: EngineFrame::new(TEST_EPOCH, 0),
        }
    }

    fn info(position: Option<u64>, capture_ticks: Option<u64>) -> InputCallbackInfo {
        InputCallbackInfo {
            timestamp: InputTimestamp {
                callback: MediaTime::new(0, DEVICE_CLOCK_TIMEBASE),
                capture: capture_ticks.map(|ticks| MediaTimestamp {
                    time: MediaTime::new(
                        i64::try_from(ticks).unwrap_or(i64::MAX),
                        DEVICE_CLOCK_TIMEBASE,
                    ),
                    provenance: TimestampProvenance::Carried,
                }),
            },
            clock_domain: TEST_DOMAIN,
            frame_position: position,
        }
    }

    /// 500 frame @48kHz ≈ 10,416,667 ns（stamp tick の block 刻み）。
    const BLOCK_NS: u64 = 10_416_667;

    fn resolve(
        info: &InputCallbackInfo,
        placement: Option<StampPlacement>,
        frames: u64,
        state: &mut StampState,
    ) -> Result<u64, u32> {
        resolve_first_frame(info, placement, frames, state)
    }

    #[test]
    fn position_wins_over_stamp() {
        // frame_position: Some は stamp の有無・状態に関わらず優先。
        let mut state = StampState::default();
        let got = resolve(
            &info(Some(500), Some(9_999_999_999)),
            Some(placement()),
            500,
            &mut state,
        );
        assert_eq!(got, Ok(500));
        assert!(!state.seeded, "position 経路は stamp 状態を更新しない");
    }

    #[test]
    fn stamp_seeds_then_places_cumulative_frames() {
        // 初回 stamp で seed（anchor=1ms → stamp=anchor で frame 0）、
        // 以後は累積 count で sample-exact に連続配置する。
        let p = placement();
        let mut state = StampState::default();
        // block1: stamp = anchor → mapped 0 → first=0, next=500。
        let got = resolve(&info(None, Some(1_000_000)), Some(p), 500, &mut state);
        assert_eq!(got, Ok(0));
        // block2: stamp = anchor+500frames → first=500（累積）。
        let got = resolve(
            &info(None, Some(1_000_000 + BLOCK_NS)),
            Some(p),
            500,
            &mut state,
        );
        assert_eq!(got, Ok(500));
        // block3: block2 の 500 frame 分進んだ stamp → first=1000
        // （block3 自身は 300 frames で next=1300）。
        let got = resolve(
            &info(None, Some(1_000_000 + 2 * BLOCK_NS)),
            Some(p),
            300,
            &mut state,
        );
        assert_eq!(got, Ok(1000));
    }

    #[test]
    fn stamp_jitter_is_absorbed_not_placed() {
        // timestamp jitter（±2 frame 相当）は配置に反映しない —
        // 累積 count が sample-exact を保つ（実機 WASAPI 実測の
        // ±3 frame jitter を許容する設計）。
        let p = placement();
        let mut state = StampState::default();
        assert_eq!(
            resolve(&info(None, Some(1_000_000)), Some(p), 500, &mut state),
            Ok(0)
        );
        // stamp が真値より +50µs（≈+2.4 frame）進んでいても、
        // delta 乖離 2 frame・累積乖離 ~2 frame は許容内 → first=500。
        let jittered = 1_000_000 + BLOCK_NS + 50_000;
        let got = resolve(&info(None, Some(jittered)), Some(p), 500, &mut state);
        assert_eq!(got, Ok(500));
        // jitter が戻っても累積位置は継続する。
        let got = resolve(
            &info(None, Some(1_000_000 + 2 * BLOCK_NS)),
            Some(p),
            500,
            &mut state,
        );
        assert_eq!(got, Ok(1000));
    }

    #[test]
    fn stamp_delta_gap_faults() {
        // stamp の進行が frame 進行から許容を超えて乖離（xrun/gap
        // 相当）→ INPUT_DISCONTINUITY。
        let p = placement();
        let mut state = StampState::default();
        assert_eq!(
            resolve(&info(None, Some(1_000_000)), Some(p), 500, &mut state),
            Ok(0)
        );
        // 1 秒分（48000 frame 相当）の stamp 進行 vs 500 frame。
        let got = resolve(
            &info(None, Some(1_000_000_000 + BLOCK_NS)),
            Some(p),
            500,
            &mut state,
        );
        assert_eq!(got, Err(FAULT_INPUT_DISCONTINUITY));
    }

    #[test]
    fn stamp_accumulated_drift_faults() {
        // per-callback の乖離は許容内（200 frame < 256）だが累積で
        // STAMP_ABS_TOLERANCE_FRAMES を超える持続 drift → fault。
        let p = placement();
        let mut state = StampState::default();
        let mut ticks = 1_000_000u64;
        assert_eq!(
            resolve(&info(None, Some(ticks)), Some(p), 500, &mut state),
            Ok(0)
        );
        // 毎 callback 200 frame 分（4,166,667 ns）多く進む stamp。
        let mut last = Ok(0);
        for _ in 0..100 {
            ticks += BLOCK_NS + 4_166_667;
            last = resolve(&info(None, Some(ticks)), Some(p), 500, &mut state);
            if last.is_err() {
                break;
            }
        }
        assert_eq!(last, Err(FAULT_INPUT_DISCONTINUITY));
    }

    #[test]
    fn stamp_regression_faults() {
        // stamp tick の逆行 → INPUT_DISCONTINUITY。
        let p = placement();
        let mut state = StampState::default();
        assert_eq!(
            resolve(&info(None, Some(2_000_000)), Some(p), 500, &mut state),
            Ok(48)
        );
        let got = resolve(&info(None, Some(1_500_000)), Some(p), 500, &mut state);
        assert_eq!(got, Err(FAULT_INPUT_DISCONTINUITY));
    }

    #[test]
    fn missing_position_and_stamp_faults() {
        let mut state = StampState::default();
        // 両方無い → fail-stop。
        let got = resolve(&info(None, None), Some(placement()), 500, &mut state);
        assert_eq!(got, Err(FAULT_INPUT_DISCONTINUITY));
        // anchor 無し（now() 非対応 backend）でも fail-stop。
        let got = resolve(&info(None, Some(2_000_000)), None, 500, &mut state);
        assert_eq!(got, Err(FAULT_INPUT_DISCONTINUITY));
        // anchor 有りでも stamp が検証不能（foreign base 等で
        // capture_stamp() が None）なら fail-stop。
        let mut foreign = info(None, Some(2_000_000));
        foreign.timestamp.capture = foreign.timestamp.capture.map(|mut ts| {
            ts.time = MediaTime::new(ts.time.value(), TimeBase::new(1, 1_000).expect("base"));
            ts
        });
        let got = resolve(&foreign, Some(placement()), 500, &mut state);
        assert_eq!(got, Err(FAULT_INPUT_DISCONTINUITY));
    }

    #[test]
    fn stamp_rejections_map_to_faults() {
        let mut state = StampState::default();
        // UnmappedDomain: stamp の clock domain が anchor と不一致。
        let mut p = placement();
        p.anchor.domain = ClockDomainId::new(99);
        let got = resolve(&info(None, Some(2_000_000)), Some(p), 500, &mut state);
        assert_eq!(got, Err(FAULT_EPOCH_DISCONTINUITY));

        // BaseMismatch: anchor の stamp base が device clock でない。
        let mut p = placement();
        p.anchor.stamp_base = TimeBase::new(1, 1_000).expect("base");
        let got = resolve(&info(None, Some(2_000_000)), Some(p), 500, &mut state);
        assert_eq!(got, Err(FAULT_EPOCH_DISCONTINUITY));

        // EpochMismatch: now の epoch が anchor frame の epoch と不一致。
        let mut p = placement();
        p.now = EngineFrame::new(MediaClockEpochId::new(8), 0);
        let got = resolve(&info(None, Some(2_000_000)), Some(p), 500, &mut state);
        assert_eq!(got, Err(FAULT_EPOCH_DISCONTINUITY));

        // BeforeAnchor: stamp が anchor より過去。
        let mut p = placement();
        p.anchor.stamp_ticks = 3_000_000;
        let got = resolve(&info(None, Some(2_000_000)), Some(p), 500, &mut state);
        assert_eq!(got, Err(FAULT_INPUT_DISCONTINUITY));

        // Overflow: tick delta が i64 に収まらない極端な stamp。
        // （実経路では capture_stamp() が i64 範囲に clamp するため
        // 到達不能だが、写像自体は直接検証する。）
        assert_eq!(
            stamp_rejection_fault(MapRejection::Overflow),
            FAULT_ARITHMETIC
        );
    }
}
