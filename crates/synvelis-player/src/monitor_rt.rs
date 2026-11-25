//! RCD-05 RT-safe monitor node primitive（synvelis#653 bounded first slice）。
//!
//! 責務:
//!
//! - [`MonitorRtNode`]: typed graph 上で駆動される RT-safe monitor
//!   processor。事前に route 判定済みの bounded schedule を適用し、
//!   playback input・monitor input を gain ramp 付きで monitor output
//!   へ混合する。route 判定（arm/monitor mode/transport/gate/mute/solo）
//!   は `synvelis-recording::decide` の権威に残し、本 module には
//!   複製しない。
//! - [`MonitorRtControl`]: 非 RT 側が固定容量 SPSC channel へ
//!   schedule を供給する producer。payload は `Copy` の固定サイズで、
//!   RT 側で所有権を持つオブジェクトや可変長 buffer を drop しない。
//! - [`MonitorRtStatus`]: fault・input missing・適用証拠を atomic
//!   counter/bit として非 RT 側へ公開する。
//!
//! 契約:
//!
//! - schedule は block 相対の半開区間 `[start, end)` を連続・非重複・
//!   空区間なしに tile し、`frames` を厳密に覆う。未供給・順序違反・
//!   gap/overlap・範囲外・容量超過ではその block を fail-closed に
//!   mute し、fault status を残す。
//! - monitor input は direct edge を前提とする。`NodeIo::input` が
//!   `None` の場合は PCM を捏造せず、route `Input` を effective mute
//!   に降格して `input_missing` を記録する。供給された無音 PCM と
//!   未供給は区別される。
//! - gain ramp は DSP のみであり、native placement・latency/PDC 補正・
//!   リサンプル・frame offset 補正は行わない。
//! - `process` は allocation-free・deallocation-free・non-blocking・
//!   bounded work を満たす。scratch は構築時に固定確保し、RT hot path
//!   では長さを変えない。

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_audio::typed::{GraphProcessor, InputView, NodeIo, OutputView};
use ruviform_audio_core::{AudioFormat, SampleFormat};
use ruviform_rt::{Consumer, Producer, RtContext, channel};
use synvelis_recording::MonitorRoute;

/// 1 block schedule が保持できる segment 数の固定上限。
pub const MONITOR_RT_SCHEDULE_CAPACITY: usize = 8;

/// schedule が RT block に供給されなかった。
pub const MONITOR_RT_FAULT_SCHEDULE_MISSING: u64 = 1 << 0;
/// schedule sequence が期待順序と一致しなかった。
pub const MONITOR_RT_FAULT_SEQUENCE: u64 = 1 << 1;
/// schedule の区間 tile・frame 数・容量契約が不正だった。
pub const MONITOR_RT_FAULT_SCHEDULE_MALFORMED: u64 = 1 << 2;
/// typed graph I/O shape が node 構成と一致しなかった。
pub const MONITOR_RT_FAULT_IO_CONTRACT: u64 = 1 << 3;
/// route `Input` に必要な monitor input PCM が未供給だった。
pub const MONITOR_RT_STATUS_INPUT_MISSING: u64 = 1 << 4;
/// playback input PCM が未供給だった。
pub const MONITOR_RT_STATUS_PLAYBACK_MISSING: u64 = 1 << 5;
/// control channel の容量超過を producer が観測した。
pub const MONITOR_RT_FAULT_CONTROL_OVERFLOW: u64 = 1 << 6;

/// route 判定済みの block 内半開区間 `[start_frame, end_frame)`。
///
/// `route` は非 RT 側で確定済みの値であり、RT node はこの値を再判定
/// しない。payload が untrusted boundary を越えるため、受信側は毎回
/// tile 契約を再検証する。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MonitorRtSegment {
    /// block 先頭からの開始 frame（inclusive）。
    pub start_frame: u32,
    /// block 先頭からの終了 frame（exclusive）。
    pub end_frame: u32,
    /// この区間で可聴にする route。
    pub route: MonitorRoute,
}

impl MonitorRtSegment {
    /// 無音の zero-length placeholder。未使用 slot の埋め込み専用で、
    /// 有効 segment としては validation を通らない。
    pub const ZERO: Self = Self {
        start_frame: 0,
        end_frame: 0,
        route: MonitorRoute::Muted,
    };

    /// 半開区間 `[start_frame, end_frame)` の segment を作る。
    #[must_use]
    pub const fn new(start_frame: u32, end_frame: u32, route: MonitorRoute) -> Self {
        Self {
            start_frame,
            end_frame,
            route,
        }
    }
}

/// 1 RT block に適用する固定容量 schedule payload。
///
/// 全 field は非 RT → RT boundary を越える untrusted payload であり、
/// 公開されていても不変条件を型で保証しない。RT node は受信ごとに
/// sequence・frame 数・tile 契約を検証する。整備済み payload の構築には
/// [`MonitorRtSchedule::tile`] を使う。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct MonitorRtSchedule {
    /// 連続する RT block sequence。最初は 0、各 block で 1 増える。
    pub sequence: u64,
    /// この schedule が適用される block の frame 数。
    pub frames: u32,
    /// `segments` 先頭から有効な segment 数。
    pub len: u8,
    /// 固定容量の segment 配列。`len` 以降は無視される。
    pub segments: [MonitorRtSegment; MONITOR_RT_SCHEDULE_CAPACITY],
}

impl MonitorRtSchedule {
    /// 全 segment が未使用の payload を作る。これは valid schedule
    /// ではなく、raw fault-injection payload の初期値として使う。
    #[must_use]
    pub const fn empty(sequence: u64, frames: u32) -> Self {
        Self {
            sequence,
            frames,
            len: 0,
            segments: [MonitorRtSegment::ZERO; MONITOR_RT_SCHEDULE_CAPACITY],
        }
    }

    /// `segments` が `frames` を tile する payload を検証して作る。
    ///
    /// 受信側でも同じ契約を再検証するため、これは producer 側の早期
    /// エラー検出であり信頼境界ではない。
    ///
    /// # Errors
    /// [`MonitorRtScheduleError`] — segment 数超過、空 tile、gap/
    /// overlap、空区間、範囲外、frame 上限超過。
    pub fn tile(
        sequence: u64,
        frames: u32,
        segments: &[MonitorRtSegment],
        max_block_frames: usize,
    ) -> Result<Self, MonitorRtScheduleError> {
        if segments.len() > MONITOR_RT_SCHEDULE_CAPACITY {
            return Err(MonitorRtScheduleError::TooManySegments {
                len: segments.len(),
                capacity: MONITOR_RT_SCHEDULE_CAPACITY,
            });
        }
        let mut schedule = Self::empty(sequence, frames);
        schedule.len = u8::try_from(segments.len()).unwrap_or(u8::MAX);
        schedule.segments[..segments.len()].copy_from_slice(segments);
        validate_schedule_shape(&schedule, max_block_frames)?;
        Ok(schedule)
    }
}

/// schedule shape validation の拒否理由。
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MonitorRtScheduleError {
    /// frame 数が 0。
    ZeroFrames,
    /// frame 数が node の block 上限を超える。
    FramesExceedMax {
        /// schedule の frame 数。
        frames: u32,
        /// node 構築時の上限。
        max: usize,
    },
    /// segment 数が 0 で block を tile しない。
    NoSegments,
    /// segment 数が固定容量を超える。
    TooManySegments {
        /// 指定された segment 数。
        len: usize,
        /// 固定容量。
        capacity: usize,
    },
    /// payload の `len` が固定配列容量を超える。
    LengthExceedsCapacity {
        /// payload の len。
        len: usize,
        /// 固定容量。
        capacity: usize,
    },
    /// 次の期待開始位置と segment 開始位置が一致しない（gap/overlap）。
    NonContiguous {
        /// 違反 segment index。
        index: usize,
        /// tile 上の期待開始位置。
        expected_start: u32,
        /// 実際の開始位置。
        actual_start: u32,
    },
    /// `end <= start` の空区間。
    EmptySegment {
        /// 違反 segment index。
        index: usize,
    },
    /// segment 終端が block 長を超える。
    SegmentOutOfRange {
        /// 違反 segment index。
        index: usize,
        /// segment 終端。
        end: u32,
        /// block frame 数。
        frames: u32,
    },
    /// 最終 segment 終端が block 終端と一致しない。
    IncompleteCoverage {
        /// tile 済み frame 数。
        covered: u32,
        /// block frame 数。
        frames: u32,
    },
}

impl fmt::Display for MonitorRtScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::ZeroFrames => f.write_str("monitor rt schedule has zero frames"),
            Self::FramesExceedMax { frames, max } => {
                write!(f, "monitor rt schedule frames {frames} exceed max {max}")
            }
            Self::NoSegments => f.write_str("monitor rt schedule has no segments"),
            Self::TooManySegments { len, capacity } => {
                write!(
                    f,
                    "monitor rt schedule len {len} exceeds capacity {capacity}"
                )
            }
            Self::LengthExceedsCapacity { len, capacity } => {
                write!(
                    f,
                    "monitor rt schedule len field {len} exceeds capacity {capacity}"
                )
            }
            Self::NonContiguous {
                index,
                expected_start,
                actual_start,
            } => write!(
                f,
                "monitor rt schedule segment {index} starts at {actual_start}, expected {expected_start}"
            ),
            Self::EmptySegment { index } => {
                write!(f, "monitor rt schedule segment {index} is empty")
            }
            Self::SegmentOutOfRange { index, end, frames } => write!(
                f,
                "monitor rt schedule segment {index} ends at {end}, beyond {frames}"
            ),
            Self::IncompleteCoverage { covered, frames } => write!(
                f,
                "monitor rt schedule covers {covered} frames, expected {frames}"
            ),
        }
    }
}

impl std::error::Error for MonitorRtScheduleError {}

fn validate_schedule_shape(
    schedule: &MonitorRtSchedule,
    max_block_frames: usize,
) -> Result<(), MonitorRtScheduleError> {
    if schedule.frames == 0 {
        return Err(MonitorRtScheduleError::ZeroFrames);
    }
    if usize::try_from(schedule.frames).unwrap_or(usize::MAX) > max_block_frames {
        return Err(MonitorRtScheduleError::FramesExceedMax {
            frames: schedule.frames,
            max: max_block_frames,
        });
    }
    let len = usize::from(schedule.len);
    if len == 0 {
        return Err(MonitorRtScheduleError::NoSegments);
    }
    if len > MONITOR_RT_SCHEDULE_CAPACITY {
        return Err(MonitorRtScheduleError::LengthExceedsCapacity {
            len,
            capacity: MONITOR_RT_SCHEDULE_CAPACITY,
        });
    }
    let mut expected_start = 0_u32;
    for index in 0..len {
        let segment = schedule.segments[index];
        if segment.start_frame != expected_start {
            return Err(MonitorRtScheduleError::NonContiguous {
                index,
                expected_start,
                actual_start: segment.start_frame,
            });
        }
        if segment.end_frame <= segment.start_frame {
            return Err(MonitorRtScheduleError::EmptySegment { index });
        }
        if segment.end_frame > schedule.frames {
            return Err(MonitorRtScheduleError::SegmentOutOfRange {
                index,
                end: segment.end_frame,
                frames: schedule.frames,
            });
        }
        expected_start = segment.end_frame;
    }
    if expected_start != schedule.frames {
        return Err(MonitorRtScheduleError::IncompleteCoverage {
            covered: expected_start,
            frames: schedule.frames,
        });
    }
    Ok(())
}

/// [`MonitorRtNode`] 構築設定。
#[derive(Copy, Clone, Debug)]
pub struct MonitorRtNodeConfig {
    /// 全 port に要求する exact audio format。channel 数と sample format
    /// は RT node の PCM contract を決める。
    pub format: AudioFormat,
    /// 1 callback の最大 frame 数。scratch 長と schedule 上限を固定する。
    pub max_block_frames: usize,
    /// route 遷移の線形 ramp 幅。0 は即時切替。
    pub ramp_frames: u32,
    /// 非 RT → RT control channel に保持できる block schedule 数。
    pub control_capacity: usize,
}

/// [`MonitorRtNode::new`] の構築エラー。
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MonitorRtNodeError {
    /// `max_block_frames` が 0。
    ZeroBlockFrames,
    /// `max_block_frames` が schedule payload の `u32` frame 表現を超える。
    BlockFramesTooLarge {
        /// 指定された上限。
        max_block_frames: usize,
    },
    /// `control_capacity` が 0。
    ZeroControlCapacity,
    /// `ramp_frames` が `f32` exact conversion 契約の `u16` 上限を超える。
    RampFramesTooLarge {
        /// 指定された ramp 幅。
        ramp_frames: u32,
    },
    /// `format.sample_format` が canonical RT `f32` ではない。
    UnsupportedSampleFormat,
}

impl fmt::Display for MonitorRtNodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::ZeroBlockFrames => f.write_str("monitor rt node requires non-zero block frames"),
            Self::BlockFramesTooLarge { max_block_frames } => write!(
                f,
                "monitor rt node block frames {max_block_frames} exceed u32 schedule contract"
            ),
            Self::ZeroControlCapacity => {
                f.write_str("monitor rt node requires non-zero control capacity")
            }
            Self::RampFramesTooLarge { ramp_frames } => write!(
                f,
                "monitor rt node ramp frames {ramp_frames} exceed u16 exact conversion contract"
            ),
            Self::UnsupportedSampleFormat => {
                f.write_str("monitor rt node requires f32 audio format")
            }
        }
    }
}

impl std::error::Error for MonitorRtNodeError {}

/// control producer からの供給エラー。
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MonitorRtControlError {
    /// producer 側の事前 validation に失敗した。
    InvalidSchedule(MonitorRtScheduleError),
    /// 固定容量 control channel が full。
    ChannelFull,
}

impl fmt::Display for MonitorRtControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchedule(error) => write!(f, "invalid monitor rt schedule: {error}"),
            Self::ChannelFull => f.write_str("monitor rt control channel is full"),
        }
    }
}

impl std::error::Error for MonitorRtControlError {}

/// RT node の観測 status。全 field は atomic で、hot path は blocking
/// lock や文字列生成を行わず counter/bit のみを更新する。
#[derive(Debug)]
pub struct MonitorRtStatus {
    fault_flags: AtomicU64,
    schedule_missing_blocks: AtomicU64,
    schedule_contract_violations: AtomicU64,
    control_overflows: AtomicU64,
    input_missing_blocks: AtomicU64,
    playback_missing_blocks: AtomicU64,
    applied_blocks: AtomicU64,
    applied_segments: AtomicU64,
    output_frames: AtomicU64,
    last_applied_sequence: AtomicU64,
}

impl Default for MonitorRtStatus {
    fn default() -> Self {
        Self {
            fault_flags: AtomicU64::new(0),
            schedule_missing_blocks: AtomicU64::new(0),
            schedule_contract_violations: AtomicU64::new(0),
            control_overflows: AtomicU64::new(0),
            input_missing_blocks: AtomicU64::new(0),
            playback_missing_blocks: AtomicU64::new(0),
            applied_blocks: AtomicU64::new(0),
            applied_segments: AtomicU64::new(0),
            output_frames: AtomicU64::new(0),
            last_applied_sequence: AtomicU64::new(u64::MAX),
        }
    }
}

impl MonitorRtStatus {
    fn flag(&self, bit: u64) {
        self.fault_flags.fetch_or(bit, Ordering::Relaxed);
    }

    fn add(counter: &AtomicU64, value: u64) {
        counter.fetch_add(value, Ordering::Relaxed);
    }

    /// latch 済み fault/status bit 集合。
    #[must_use]
    pub fn fault_flags(&self) -> u64 {
        self.fault_flags.load(Ordering::Relaxed)
    }

    /// schedule 未供給で mute した block 数。
    #[must_use]
    pub fn schedule_missing_blocks(&self) -> u64 {
        self.schedule_missing_blocks.load(Ordering::Relaxed)
    }

    /// sequence/tile/frame/control overflow の contract violation 数。
    #[must_use]
    pub fn schedule_contract_violations(&self) -> u64 {
        self.schedule_contract_violations.load(Ordering::Relaxed)
    }

    /// producer が観測した control channel overflow 数。
    #[must_use]
    pub fn control_overflows(&self) -> u64 {
        self.control_overflows.load(Ordering::Relaxed)
    }

    /// monitor input 未供給を含む block 数。
    #[must_use]
    pub fn input_missing_blocks(&self) -> u64 {
        self.input_missing_blocks.load(Ordering::Relaxed)
    }

    /// playback input 未供給を含む block 数。
    #[must_use]
    pub fn playback_missing_blocks(&self) -> u64 {
        self.playback_missing_blocks.load(Ordering::Relaxed)
    }

    /// valid schedule を適用した block 数。
    #[must_use]
    pub fn applied_blocks(&self) -> u64 {
        self.applied_blocks.load(Ordering::Relaxed)
    }

    /// valid schedule で適用した segment 数の累計。
    #[must_use]
    pub fn applied_segments(&self) -> u64 {
        self.applied_segments.load(Ordering::Relaxed)
    }

    /// monitor output に書き込んだ frame 数の累計。
    #[must_use]
    pub fn output_frames(&self) -> u64 {
        self.output_frames.load(Ordering::Relaxed)
    }

    /// 最後に適用した schedule sequence。未適用時は `u64::MAX`。
    #[must_use]
    pub fn last_applied_sequence(&self) -> u64 {
        self.last_applied_sequence.load(Ordering::Relaxed)
    }
}

/// 非 RT 側の schedule producer。
///
/// `push_tile` は早期 validation を行う。`push_schedule` は raw payload
/// をそのまま送り、RT 受信側の再検証を試す fault-injection 経路としても
/// 使える。channel full では呼び出し側へエラーを返し、atomic overflow
/// counter を進める。RT node は次にその変化を観測した block を
/// fail-closed に mute する。
pub struct MonitorRtControl {
    producer: Producer<MonitorRtSchedule>,
    max_block_frames: usize,
    status: Arc<MonitorRtStatus>,
}

impl MonitorRtControl {
    /// tile 済み segment 列から schedule を構築して push する。
    ///
    /// # Errors
    /// [`MonitorRtControlError::InvalidSchedule`] または
    /// [`MonitorRtControlError::ChannelFull`]。
    pub fn push_tile(
        &mut self,
        sequence: u64,
        frames: u32,
        segments: &[MonitorRtSegment],
    ) -> Result<(), MonitorRtControlError> {
        let schedule = MonitorRtSchedule::tile(sequence, frames, segments, self.max_block_frames)
            .map_err(MonitorRtControlError::InvalidSchedule)?;
        self.push_schedule(schedule)
    }

    /// raw schedule payload をそのまま push する。RT node が必ず再検証
    /// するため、producer 側 validation を通らない payload の fault
    /// injection にも使える。
    ///
    /// # Errors
    /// [`MonitorRtControlError::ChannelFull`]。full 時は overflow を
    /// status に記録する。
    pub fn push_schedule(
        &mut self,
        schedule: MonitorRtSchedule,
    ) -> Result<(), MonitorRtControlError> {
        if self.producer.try_push(schedule).is_ok() {
            Ok(())
        } else {
            MonitorRtStatus::add(&self.status.control_overflows, 1);
            self.status.flag(MONITOR_RT_FAULT_CONTROL_OVERFLOW);
            Err(MonitorRtControlError::ChannelFull)
        }
    }

    /// node と共有する status。
    #[must_use]
    pub fn status(&self) -> &Arc<MonitorRtStatus> {
        &self.status
    }
}

/// typed graph 上の RT-safe monitor node。
///
/// input port 0 は playback、input port 1 は monitor input、output port 0
/// は monitor output。direct edge で供給されない input は `None` として
/// 観測し、zero-filled PCM と混同しない。route 判定済み schedule の適用と
/// PCM mixing だけを行い、route 判定・transport 判定・PDC/latency 補正は
/// 持たない。
pub struct MonitorRtNode {
    consumer: Consumer<MonitorRtSchedule>,
    status: Arc<MonitorRtStatus>,
    channels: usize,
    max_block_frames: usize,
    ramp_step: f32,
    input_gain: f32,
    playback_gain: f32,
    in_gain_scratch: Vec<f32>,
    pb_gain_scratch: Vec<f32>,
    next_sequence: u64,
    seen_control_overflows: u64,
}

impl MonitorRtNode {
    /// node と control producer の pair を構築する。
    ///
    /// channel ring と gain scratch はここで固定確保する。構築は非 RT
    /// 専用であり、RT hot path での allocation は行わない。
    ///
    /// # Errors
    /// [`MonitorRtNodeError`] — zero/過大 block、zero control capacity、
    /// non-f32 format。
    pub fn new(
        config: MonitorRtNodeConfig,
    ) -> Result<(Self, MonitorRtControl), MonitorRtNodeError> {
        if config.max_block_frames == 0 {
            return Err(MonitorRtNodeError::ZeroBlockFrames);
        }
        if config.max_block_frames > usize::try_from(u32::MAX).unwrap_or(usize::MAX) {
            return Err(MonitorRtNodeError::BlockFramesTooLarge {
                max_block_frames: config.max_block_frames,
            });
        }
        if config.control_capacity == 0 {
            return Err(MonitorRtNodeError::ZeroControlCapacity);
        }
        if config.format.sample_format != SampleFormat::F32 {
            return Err(MonitorRtNodeError::UnsupportedSampleFormat);
        }
        let (producer, consumer) = channel(config.control_capacity);
        let status = Arc::new(MonitorRtStatus::default());
        let ramp_frames = u16::try_from(config.ramp_frames).map_err(|_| {
            MonitorRtNodeError::RampFramesTooLarge {
                ramp_frames: config.ramp_frames,
            }
        })?;
        let ramp_step = if ramp_frames == 0 {
            f32::INFINITY
        } else {
            1.0 / f32::from(ramp_frames)
        };
        let channels = usize::try_from(config.format.layout.channels()).unwrap_or(0);
        let node = Self {
            consumer,
            status: Arc::clone(&status),
            channels,
            max_block_frames: config.max_block_frames,
            ramp_step,
            input_gain: 0.0,
            playback_gain: 0.0,
            in_gain_scratch: vec![0.0; config.max_block_frames],
            pb_gain_scratch: vec![0.0; config.max_block_frames],
            next_sequence: 0,
            seen_control_overflows: 0,
        };
        let control = MonitorRtControl {
            producer,
            max_block_frames: config.max_block_frames,
            status,
        };
        Ok((node, control))
    }

    /// node と共有する status。control producer 側でも同じ値を読める。
    #[must_use]
    pub fn status(&self) -> &Arc<MonitorRtStatus> {
        &self.status
    }

    /// 現在の input gain（検査用）。
    #[must_use]
    pub fn input_gain(&self) -> f32 {
        self.input_gain
    }

    /// 現在の playback gain（検査用）。
    #[must_use]
    pub fn playback_gain(&self) -> f32 {
        self.playback_gain
    }

    fn record_schedule_fault(&self, bit: u64) {
        self.status.flag(bit);
        MonitorRtStatus::add(&self.status.schedule_contract_violations, 1);
    }

    fn write_zero_output(&mut self, output: &mut OutputView<'_>, frames: usize) {
        for channel_index in 0..output.channels() {
            if let Some(channel) = output.channel_mut(channel_index) {
                channel[..frames].fill(0.0);
            }
        }
        MonitorRtStatus::add(
            &self.status.output_frames,
            u64::try_from(frames).unwrap_or(u64::MAX),
        );
    }

    fn write_mixed(
        &mut self,
        output: &mut OutputView<'_>,
        frames: usize,
        playback: Option<&InputView<'_>>,
        monitor_input: Option<&InputView<'_>>,
    ) {
        for channel_index in 0..self.channels {
            let playback_channel = playback.and_then(|view| view.channel(channel_index));
            let input_channel = monitor_input.and_then(|view| view.channel(channel_index));
            let Some(output_channel) = output.channel_mut(channel_index) else {
                self.status.flag(MONITOR_RT_FAULT_IO_CONTRACT);
                return;
            };
            for frame in 0..frames {
                let playback_sample = playback_channel.map_or(0.0, |channel| channel[frame]);
                let input_sample = input_channel.map_or(0.0, |channel| channel[frame]);
                output_channel[frame] = mix(
                    self.pb_gain_scratch[frame],
                    playback_sample,
                    self.in_gain_scratch[frame],
                    input_sample,
                );
            }
        }
        MonitorRtStatus::add(
            &self.status.output_frames,
            u64::try_from(frames).unwrap_or(u64::MAX),
        );
    }

    fn ramp_muted(&mut self, output: &mut OutputView<'_>, frames: usize) {
        for frame in 0..frames {
            self.input_gain = approach(self.input_gain, 0.0, self.ramp_step);
            self.playback_gain = approach(self.playback_gain, 0.0, self.ramp_step);
            self.in_gain_scratch[frame] = self.input_gain;
            self.pb_gain_scratch[frame] = self.playback_gain;
        }
        self.write_mixed(output, frames, None, None);
    }

    fn input_shape_valid(view: &InputView<'_>, channels: usize, frames: usize) -> bool {
        view.channels() == channels
            && view.frames() == frames
            && (0..channels).all(|channel| view.channel(channel).is_some())
    }

    fn render_valid_schedule(
        &mut self,
        output: &mut OutputView<'_>,
        frames: usize,
        schedule: &MonitorRtSchedule,
        playback: Option<&InputView<'_>>,
        monitor_input: Option<&InputView<'_>>,
    ) {
        let len = usize::from(schedule.len);
        let input_supplied = monitor_input.is_some();
        let playback_supplied = playback.is_some();
        let input_needed = schedule.segments[..len]
            .iter()
            .any(|segment| segment.route == MonitorRoute::Input);
        let playback_needed = schedule.segments[..len]
            .iter()
            .any(|segment| segment.route == MonitorRoute::Playback);
        if input_needed && !input_supplied {
            self.status.flag(MONITOR_RT_STATUS_INPUT_MISSING);
            MonitorRtStatus::add(&self.status.input_missing_blocks, 1);
        }
        if playback_needed && !playback_supplied {
            self.status.flag(MONITOR_RT_STATUS_PLAYBACK_MISSING);
            MonitorRtStatus::add(&self.status.playback_missing_blocks, 1);
        }

        let mut segment_index = 0_usize;
        for frame in 0..frames {
            while segment_index + 1 < len
                && frame
                    >= usize::try_from(schedule.segments[segment_index].end_frame)
                        .unwrap_or(usize::MAX)
            {
                segment_index += 1;
            }
            let route = schedule.segments[segment_index].route;
            let input_target = if route == MonitorRoute::Input && input_supplied {
                1.0
            } else {
                0.0
            };
            let playback_target = if route == MonitorRoute::Playback && playback_supplied {
                1.0
            } else {
                0.0
            };
            self.input_gain = approach(self.input_gain, input_target, self.ramp_step);
            self.playback_gain = approach(self.playback_gain, playback_target, self.ramp_step);
            self.in_gain_scratch[frame] = self.input_gain;
            self.pb_gain_scratch[frame] = self.playback_gain;
        }
        self.write_mixed(output, frames, playback, monitor_input);
        MonitorRtStatus::add(&self.status.applied_blocks, 1);
        MonitorRtStatus::add(
            &self.status.applied_segments,
            u64::try_from(len).unwrap_or(u64::MAX),
        );
        self.status
            .last_applied_sequence
            .store(schedule.sequence, Ordering::Relaxed);
    }
}

impl GraphProcessor for MonitorRtNode {
    /// 1 block の schedule を適用し、monitor output を書く。
    ///
    /// control channel は 1 block につき 1 payload。未供給・sequence 不
    /// 一致・malformed payload・control overflow 観測では PCM を捏造せず
    /// mute し、fault counter/bit を残す。direct input の `None` は未供給
    /// として区別する。
    fn process(&mut self, io: &mut NodeIo<'_>, _ctx: &RtContext) {
        let frames = io.frames();
        if frames == 0 {
            return;
        }
        let Some(mut output) = io.output(0) else {
            self.status.flag(MONITOR_RT_FAULT_IO_CONTRACT);
            return;
        };
        if frames > self.max_block_frames {
            self.status.flag(MONITOR_RT_FAULT_IO_CONTRACT);
            self.write_zero_output(&mut output, frames);
            return;
        }
        if output.channels() != self.channels || output.frames() != frames {
            self.status.flag(MONITOR_RT_FAULT_IO_CONTRACT);
            self.write_zero_output(&mut output, frames);
            return;
        }

        let overflows = self.status.control_overflows.load(Ordering::Relaxed);
        let overflow_pending = overflows != self.seen_control_overflows;
        self.seen_control_overflows = overflows;
        if overflow_pending {
            self.record_schedule_fault(MONITOR_RT_FAULT_CONTROL_OVERFLOW);
        }

        let mut valid_schedule = None;
        if let Ok(schedule) = self.consumer.try_pop() {
            if schedule.sequence != self.next_sequence {
                self.record_schedule_fault(MONITOR_RT_FAULT_SEQUENCE);
                self.next_sequence = if schedule.sequence > self.next_sequence {
                    schedule.sequence.saturating_add(1)
                } else {
                    self.next_sequence.saturating_add(1)
                };
            } else if validate_schedule_shape(&schedule, self.max_block_frames).is_err()
                || usize::try_from(schedule.frames).unwrap_or(usize::MAX) != frames
            {
                self.record_schedule_fault(MONITOR_RT_FAULT_SCHEDULE_MALFORMED);
                self.next_sequence = self.next_sequence.saturating_add(1);
            } else {
                self.next_sequence = self.next_sequence.saturating_add(1);
                if !overflow_pending {
                    valid_schedule = Some(schedule);
                }
            }
        } else {
            self.status.flag(MONITOR_RT_FAULT_SCHEDULE_MISSING);
            MonitorRtStatus::add(&self.status.schedule_missing_blocks, 1);
            self.next_sequence = self.next_sequence.saturating_add(1);
        }

        let Some(schedule) = valid_schedule else {
            self.ramp_muted(&mut output, frames);
            return;
        };

        let playback = io.input(0);
        let monitor_input = io.input(1);
        if playback
            .as_ref()
            .is_some_and(|view| !Self::input_shape_valid(view, self.channels, frames))
            || monitor_input
                .as_ref()
                .is_some_and(|view| !Self::input_shape_valid(view, self.channels, frames))
        {
            self.status.flag(MONITOR_RT_FAULT_IO_CONTRACT);
            self.ramp_muted(&mut output, frames);
            return;
        }
        self.render_valid_schedule(
            &mut output,
            frames,
            &schedule,
            playback.as_ref(),
            monitor_input.as_ref(),
        );
    }
}

/// gain を `target` へ `step` ずつ近づける（線形 ramp）。
fn approach(gain: f32, target: f32, step: f32) -> f32 {
    if gain < target {
        (gain + step).min(target)
    } else if gain > target {
        (gain - step).max(target)
    } else {
        gain
    }
}

/// gain が bit レベルで 1.0 かを返す。ramp は target に厳密に
/// 到達するため、近似比較ではなく bit 判定が意図した契約である。
#[inline]
fn gain_is_one(gain: f32) -> bool {
    gain.to_bits() == 1.0_f32.to_bits()
}

/// gain が bit レベルで +0.0 かを返す。
#[inline]
fn gain_is_zero(gain: f32) -> bool {
    gain.to_bits() == 0.0_f32.to_bits()
}

/// gain 1.0/0.0 を乗算せず通し、安定区間の bit 一致を保つ。
#[inline]
fn apply(gain: f32, sample: f32) -> f32 {
    if gain_is_one(gain) {
        sample
    } else if gain_is_zero(gain) {
        0.0
    } else {
        sample * gain
    }
}

/// 片側だけが可聴な安定区間では加算を挟まず sample をそのまま返す。
/// `-0.0 + 0.0` が `+0.0` になる IEEE 加算を避け、passthrough の
/// bit 一致を保つ。crossfade 区間だけ加算する。
#[inline]
fn mix(playback_gain: f32, playback_sample: f32, input_gain: f32, input_sample: f32) -> f32 {
    if gain_is_one(playback_gain) && gain_is_zero(input_gain) {
        playback_sample
    } else if gain_is_one(input_gain) && gain_is_zero(playback_gain) {
        input_sample
    } else {
        apply(playback_gain, playback_sample) + apply(input_gain, input_sample)
    }
}
