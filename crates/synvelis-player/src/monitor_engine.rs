//! RCD-03 engine 搭載 monitor bus（synvelis#117 slice）。
//!
//! 責務:
//!
//! - [`MonitorSession`]: recording 側 [`MonitorProbe`]（`GateScheduler`
//!   を共有する gate 観測）と player 側 [`MonitorAdapter`]（PCM
//!   gain-ramp 混合）を合成し、block 単位で route 判定→PCM 生成を
//!   行う非 RT monitor bus。gate ロジックの第二実装は持たない
//!   （segment の権威は `GateScheduler`/`evaluate_gate`、route の
//!   権威は `decide`）。
//! - [`MonitorEngine`]: headless engine driver（[`HeadlessDriver`]/
//!   [`RoutingEngine`]）を 1 block ずつ実 run 経路で駆動し、各
//!   block の実 master tap PCM（`PlayReport.pcm`）を session の
//!   playback 側として供給する wrapper。monitor 出力は engine
//!   通常出力・raw capture・export を変更しない独立 bus。
//!
//! 契約:
//!
//! - engine frame ↔ session frame は 1:1（`plan.sample_rate ==
//!   engine.sample_rate` を構築時に検証）。attach 時の engine
//!   cursor を frame offset とし、以後の block で session 位置と
//!   のずれ（seek 等）を検出した場合は fail-closed（無音 +
//!   `faulted` 観測可能）。
//! - `TransportPhase`・入力 PCM は block 単位で caller が供給
//!   する（block 内相変化は対象外）。入力 PCM 未供給の block で
//!   route `Input` が選ばれた segment は `Muted` へ降格 +
//!   `input_missing` notice（PCM を捏造しない）。
//! - monitor 出力 PCM は native placement を保ち、latency/PDC
//!   補正・リサンプル・オフセットを placement に適用しない
//!   （gain ramp は DSP のみ）。
//! - 本 module は RT callback 内では動かない（`MonitorAdapter`
//!   の `Vec` 確保は非 RT）。RT graph への monitor node 搭載は
//!   別 slice。

#![allow(clippy::module_name_repetitions)]

use std::collections::VecDeque;
use std::fmt;

use ruviform_audio::typed::ProcessError;
use synvelis_project::{SessionFrame, SessionRange};
use synvelis_recording::{
    BoundedVec, MonitorNotices, MonitorProbe, MonitorRoute, MonitorSegment, PreparedRecordingPlan,
    TrackMonitorContext, TransportPhase,
};

use crate::driver::{HeadlessDriver, PlayReport, PumpPolicy};
use crate::lowering::EngineConfig;
use crate::monitor::{MonitorAdapter, MonitorBlock};
use crate::routing_engine::RoutingEngine;

// ---------- engine blocks 抽象 ----------

/// headless engine driver の block 実行経路を抽象化する trait。
///
/// [`HeadlessDriver`] と [`RoutingEngine`] の既存 `run`/`cursor`
/// へ委譲する。両者は同一 `ProcessError`/`PumpPolicy`/
/// `PlayReport`/`EngineConfig` 型を共有するため 1 trait で両対応
/// できる。`run_blocks(1, ...)` の `PlayReport.pcm` はその block
/// の master tap PCM（tap 未接続時は空 channel）であり、
/// [`MonitorEngine`] が session へ供給する実経路 PCM となる。
pub trait EngineBlocks {
    /// `blocks` block を実 run 経路で処理し、master tap PCM を
    /// 蓄積した [`PlayReport`] を返す。
    ///
    /// # Errors
    /// engine runtime の [`ProcessError`]（over-budget frames 等）。
    fn run_blocks(&mut self, blocks: usize, policy: PumpPolicy)
    -> Result<PlayReport, ProcessError>;

    /// 処理済み engine frame 数。
    fn cursor(&self) -> u64;
}

impl EngineBlocks for HeadlessDriver {
    fn run_blocks(
        &mut self,
        blocks: usize,
        policy: PumpPolicy,
    ) -> Result<PlayReport, ProcessError> {
        self.run(blocks, policy)
    }

    fn cursor(&self) -> u64 {
        self.cursor()
    }
}

impl EngineBlocks for RoutingEngine {
    fn run_blocks(
        &mut self,
        blocks: usize,
        policy: PumpPolicy,
    ) -> Result<PlayReport, ProcessError> {
        self.run(blocks, policy)
    }

    fn cursor(&self) -> u64 {
        self.cursor()
    }
}

// ---------- 構築エラー ----------

/// [`MonitorEngine`] 構築時の検証エラー。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MonitorEngineError {
    /// engine sample rate が plan 構築 rate と不一致 — engine
    /// frame と session frame の 1:1 対応が崩れるため構築を拒否。
    SampleRateMismatch {
        /// `PreparedRecordingPlan` 構築時の sample rate。
        plan: u32,
        /// engine の sample rate。
        engine: u32,
    },
    /// `block_frames` が 0 または `u32` 範囲外 — session feed の
    /// frame 数契約（`u32`）を満たせない。
    InvalidBlockFrames {
        /// 渡された block frame 数。
        block_frames: usize,
    },
    /// `channels` が 0 — monitor 出力 channel を構成できない。
    ZeroChannels,
}

impl fmt::Display for MonitorEngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::SampleRateMismatch { plan, engine } => {
                write!(
                    f,
                    "monitor engine sample rate mismatch: plan={plan} engine={engine}"
                )
            }
            Self::InvalidBlockFrames { block_frames } => {
                write!(f, "monitor engine invalid block_frames: {block_frames}")
            }
            Self::ZeroChannels => f.write_str("monitor engine zero channels"),
        }
    }
}

impl std::error::Error for MonitorEngineError {}

// ---------- session ----------

/// `MonitorSession` が block 内で適用した 1 segment の証拠。
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MonitorSegmentRecord {
    /// segment の session-frame 半開区間 `[in, out)`。
    pub session: SessionRange,
    /// この region で Record/Punch Gate が開いていたか。
    pub gate_open: bool,
    /// `decide` が返した route（adapter fail-closed 降格前）。
    pub route: MonitorRoute,
    /// adapter 適用後の実効 route（input 未供給の `Muted` 降格等）。
    pub effective_route: MonitorRoute,
    /// `decide` の notice 集合。
    pub notices: MonitorNotices,
    /// adapter の `input_missing` notice。
    pub input_missing: bool,
    /// この block に入力 PCM が供給されていたか。
    pub input_supplied: bool,
}

/// channel-major PCM の `[start, end)` frame slice を複写する。
/// channel/frame が不足する分は空（adapter が無音として扱う）。
fn slice_block(pcm: &[Vec<f32>], channels: usize, start: usize, end: usize) -> Vec<Vec<f32>> {
    (0..channels)
        .map(|c| {
            pcm.get(c).map_or_else(Vec::new, |ch| {
                let s = start.min(ch.len());
                let e = end.min(ch.len()).max(s);
                ch[s..e].to_vec()
            })
        })
        .collect()
}

/// `channels × frames` の無音 PCM。
fn silence(channels: usize, frames: usize) -> Vec<Vec<f32>> {
    vec![vec![0.0; frames]; channels]
}

/// recording `MonitorProbe` と player `MonitorAdapter` を合成した
/// 非 RT monitor bus。
///
/// block ごとに [`Self::on_block`] が (1) probe で gate segment
/// を session-frame 精度で取得、(2) session 終端を越える分を
/// `Playback` で補完、(3) segment を隙間・重複なく tile する
/// ことを検証、(4) segment ごとに `decide` route で adapter を
/// 駆動し monitor PCM を連結する。契約違反は fail-closed
/// （無音出力 + `faulted`）。
pub struct MonitorSession<'a> {
    probe: MonitorProbe<'a>,
    adapter: MonitorAdapter,
    channels: usize,
    phase: TransportPhase,
    session_end: SessionFrame,
    position: SessionFrame,
    input_fifo: VecDeque<Vec<Vec<f32>>>,
    probe_segments: BoundedVec<MonitorSegment>,
    block_segments: Vec<MonitorSegment>,
    faulted: bool,
    segment_log: Vec<MonitorSegmentRecord>,
}

impl<'a> MonitorSession<'a> {
    /// `plan` の gate scheduler を共有する session を構築する。
    ///
    /// `channels`/`block_frames`/`ramp_frames` は [`MonitorAdapter`]
    /// の構成と同一契約（`block_frames` は 1 block の最大 frame、
    /// `0 < block_frames <= u32::MAX`）。
    ///
    /// # Errors
    /// [`MonitorEngineError::ZeroChannels`]/
    /// [`MonitorEngineError::InvalidBlockFrames`]。
    pub fn new(
        plan: &'a PreparedRecordingPlan,
        ctx: TrackMonitorContext,
        channels: usize,
        block_frames: usize,
        ramp_frames: u32,
    ) -> Result<Self, MonitorEngineError> {
        if channels == 0 {
            return Err(MonitorEngineError::ZeroChannels);
        }
        match u32::try_from(block_frames) {
            Ok(v) if v > 0 => {}
            _ => return Err(MonitorEngineError::InvalidBlockFrames { block_frames }),
        }
        let lookahead = plan.budget.lookahead_spans;
        // 1 block あたりの segment 数は piece 数に比例する。piece は
        // lookahead span 数で有界だが、閉/開の補完で最大
        // `2 * pieces + 1` になり得るため余裕を持たせる。溢れた場合は
        // probe.faulted() → fail-closed（無音）となる。
        let segment_capacity = lookahead.saturating_mul(2).saturating_add(4);
        Ok(Self {
            probe: MonitorProbe::new(plan, ctx),
            adapter: MonitorAdapter::new(channels, block_frames, ramp_frames),
            channels,
            phase: TransportPhase::Playing,
            session_end: plan.session_end,
            position: SessionFrame::ZERO,
            input_fifo: VecDeque::new(),
            probe_segments: BoundedVec::with_capacity(segment_capacity),
            block_segments: Vec::with_capacity(segment_capacity),
            faulted: false,
            segment_log: Vec::new(),
        })
    }

    /// 次 block 以降に適用する transport 相を設定する
    /// （block 単位供給。block 内相変化は対象外）。
    pub const fn set_phase(&mut self, phase: TransportPhase) {
        self.phase = phase;
    }

    /// 現在の transport 相。
    #[must_use]
    pub const fn phase(&self) -> TransportPhase {
        self.phase
    }

    /// 次の engine block に対応する入力 PCM を staging する
    /// （channel-major、`channels × block_frames`）。block ごとに
    /// 1 回・順序通りに呼ぶ契約 — `on_block` が block ごとに
    /// 先頭を消費する。route `Input` の segment はこの block の
    /// 対応 frame slice を使い、未供給なら `Muted` へ降格する。
    pub fn push_input_block(&mut self, pcm: Vec<Vec<f32>>) {
        self.input_fifo.push_back(pcm);
    }

    /// 契約違反で fail-closed したか（以後の `on_block` は無音）。
    #[must_use]
    pub const fn faulted(&self) -> bool {
        self.faulted
    }

    /// fail-closed を外部から記録する（engine frame と session
    /// frame のずれを検出した呼出し側が使う）。以後の `on_block`
    /// は無音を返す。
    pub const fn fault(&mut self) {
        self.faulted = true;
    }

    /// session 位置（次 block 先頭の `SessionFrame`）。session
    /// 終端を越えても飽和せず進む（engine frame との 1:1 対応を
    /// 保つため）。
    #[must_use]
    pub const fn position(&self) -> SessionFrame {
        self.position
    }

    /// `session_end` に到達したか（probe の契約に従う）。
    #[must_use]
    pub fn finished(&self) -> bool {
        self.probe.finished()
    }

    /// 適用済み segment の証拠列。
    #[must_use]
    pub fn segment_log(&self) -> &[MonitorSegmentRecord] {
        &self.segment_log
    }

    /// segment 証拠列を消去する。
    pub fn clear_segment_log(&mut self) {
        self.segment_log.clear();
    }

    /// 内部の [`MonitorProbe`]（検査用）。
    #[must_use]
    pub const fn probe(&self) -> &MonitorProbe<'a> {
        &self.probe
    }

    /// 1 block 分の monitor PCM を生成する。
    ///
    /// `session_frame` はこの block 先頭の session frame
    /// （`MonitorEngine` では `engine_cursor - attach_offset`）。
    /// `self.position` と一致しない場合は session 位置ずれと
    /// みなし fail-closed。`frames` は engine block の frame 数、
    /// `playback` は master tap PCM（channel-major。tap 未接続・
    /// channel/frame 不足は無音として扱い、PCM を捏造しない）。
    ///
    /// 戻り値は `channels × frames` の monitor PCM。`faulted`
    /// 時・契約違反時は `channels × frames` の無音。
    pub fn on_block(
        &mut self,
        session_frame: u64,
        frames: usize,
        playback: &[Vec<f32>],
    ) -> Vec<Vec<f32>> {
        if self.faulted {
            return silence(self.channels, frames);
        }
        let Some((block_start, block_end)) = self.prepare_block(session_frame, frames) else {
            return silence(self.channels, frames);
        };
        self.render_block(block_start, block_end, frames, playback)
    }

    /// block の session 位置を照合し、gate segment を取得して
    /// `[block_start, block_end)` を tile することを検証する。
    /// 契約違反時は `faulted` を立てて `None` を返す。
    fn prepare_block(
        &mut self,
        session_frame: u64,
        frames: usize,
    ) -> Option<(SessionFrame, SessionFrame)> {
        // 変換不能・範囲外はすべて契約違反 — fail-closed を
        // `faulted` で観測可能にする（無音だけ返して黙殺しない）。
        let expected = i64::try_from(session_frame)
            .ok()
            .and_then(|v| SessionFrame::new(v).ok());
        let Some(expected) = expected else {
            self.faulted = true;
            return None;
        };
        if expected != self.position {
            self.faulted = true;
            return None;
        }
        let block_start = self.position;
        let block_end = i64::try_from(frames)
            .ok()
            .and_then(|f| block_start.checked_add(f).ok());
        let feed_frames = u32::try_from(frames).ok();
        let (Some(block_end), Some(feed_frames)) = (block_end, feed_frames) else {
            self.faulted = true;
            return None;
        };
        // gate segment を session-frame 精度で取得（probe が内部
        // cursor で [block_start, min(block_end, session_end)) を
        // tile する）。session 終端を越える分は Playback で補完。
        self.probe
            .feed(feed_frames, self.phase, &mut self.probe_segments);
        if self.probe.faulted() {
            self.faulted = true;
            return None;
        }
        self.block_segments.clear();
        self.block_segments
            .extend(self.probe_segments.as_slice().iter().copied());
        // probe が tile するのは [block_start, covered_end) のみ。
        // covered_end = max(block_start, min(block_end, session_end))。
        let covered_end = block_end.min(self.session_end).max(block_start);
        if covered_end < block_end
            && let Ok(range) = SessionRange::try_new(covered_end, block_end)
        {
            self.block_segments.push(MonitorSegment {
                session: range,
                gate_open: false,
                route: MonitorRoute::Playback,
                notices: MonitorNotices::default(),
            });
        }
        // segment が block を隙間・重複・空区間なく tile することを
        // 検証する（契約違反は fail-closed）。
        let mut pos = block_start;
        let tiles = self.block_segments.iter().all(|seg| {
            let ok = !seg.session.is_empty() && seg.session.start() == pos;
            pos = seg.session.end();
            ok
        }) && pos == block_end;
        if !tiles {
            self.faulted = true;
            return None;
        }
        Some((block_start, block_end))
    }

    /// 検証済み segment 列を adapter で駆動し monitor PCM を連結
    /// する。`position` を `block_end`（session 終端を越えても
    /// 飽和しない engine 対応位置）へ進める。
    fn render_block(
        &mut self,
        block_start: SessionFrame,
        block_end: SessionFrame,
        frames: usize,
        playback: &[Vec<f32>],
    ) -> Vec<Vec<f32>> {
        // 入力 PCM は block 単位で 1 つ消費し、Input segment ごとに
        // 対応 frame slice を使う（block 内に複数 Input segment が
        // あっても frame 対応は保たれる）。shape が `channels ×
        // frames` と一致しない block は未供給と同じく fail-closed
        // （部分的な無音を Input route のまま見せない）。
        let input_block = self
            .input_fifo
            .pop_front()
            .filter(|pcm| pcm.len() == self.channels && pcm.iter().all(|ch| ch.len() == frames));
        let input_supplied = input_block.is_some();
        let block_start_i64 = i64::from(block_start);
        let mut out: Vec<Vec<f32>> = vec![Vec::with_capacity(frames); self.channels];
        for i in 0..self.block_segments.len() {
            let seg = self.block_segments[i];
            let Ok(seg_start) = usize::try_from(i64::from(seg.session.start()) - block_start_i64)
            else {
                self.faulted = true;
                return silence(self.channels, frames);
            };
            let Ok(seg_end) = usize::try_from(i64::from(seg.session.end()) - block_start_i64)
            else {
                self.faulted = true;
                return silence(self.channels, frames);
            };
            let input = if seg.route == MonitorRoute::Input {
                input_block
                    .as_ref()
                    .map(|pcm| slice_block(pcm, self.channels, seg_start, seg_end))
            } else {
                None
            };
            let output = self.adapter.process_block(&MonitorBlock {
                route: seg.route,
                frames: seg_end - seg_start,
                input,
                playback: slice_block(playback, self.channels, seg_start, seg_end),
            });
            if output.pcm.len() != self.channels
                || output.pcm.iter().any(|ch| ch.len() != seg_end - seg_start)
            {
                self.faulted = true;
                return silence(self.channels, frames);
            }
            for (c, ch) in output.pcm.iter().enumerate() {
                out[c].extend_from_slice(ch);
            }
            self.segment_log.push(MonitorSegmentRecord {
                session: seg.session,
                gate_open: seg.gate_open,
                route: seg.route,
                effective_route: output.effective_route,
                notices: seg.notices,
                input_missing: output.notices.input_missing,
                input_supplied,
            });
        }
        self.position = block_end;
        out
    }
}

// ---------- engine wrapper ----------

/// [`MonitorEngine::run`] の結果。master tap PCM と monitor bus
/// PCM を block 連結で保持する。
#[derive(Clone, Debug, Default)]
pub struct MonitorRunReport {
    /// master tap PCM（channel-major）。tap 未接続時は空。
    pub playback: Vec<Vec<f32>>,
    /// monitor bus PCM（channel-major、`channels × frames`）。
    pub monitor: Vec<Vec<f32>>,
    /// 処理した engine frame 数。
    pub frames: u64,
}

/// headless engine driver を block 単位で駆動し、各 block の実
/// master tap PCM を [`MonitorSession`] へ供給する wrapper。
///
/// `run` は 1 block ずつ `EngineBlocks::run_blocks` を呼び、
/// pump cadence は呼出し内 `index` ではなく `blocks_run`
/// counter で `index % n == 0` と同一になるよう emulation する
/// （`run(1, ...)` 反復で pump が毎 block 化するのを防ぐ）。
/// session 位置と engine cursor のずれ・probe/adapter の契約
/// 違反は fail-closed（monitor 出力は無音、`session.faulted()`
/// で観測可能）。engine 自体の `ProcessError` はそのまま伝播する。
pub struct MonitorEngine<'a, D: EngineBlocks> {
    driver: D,
    session: MonitorSession<'a>,
    engine: EngineConfig,
    frame_offset: u64,
    blocks_run: u64,
}

impl<'a, D: EngineBlocks> MonitorEngine<'a, D> {
    /// `driver` に monitor bus を搭載する。
    ///
    /// `plan` の構築 sample rate と `engine.sample_rate` の一致、
    /// `engine.block_frames` が `1..=u32::MAX`、`engine.channels`
    /// が非 0 であることを検証する。`frame_offset` は構築時の
    /// `driver.cursor()` — 以後 session 位置は
    /// `cursor - frame_offset` として照合される。
    ///
    /// # Errors
    /// [`MonitorEngineError::SampleRateMismatch`]/
    /// [`MonitorEngineError::InvalidBlockFrames`]/
    /// [`MonitorEngineError::ZeroChannels`]。
    pub fn new(
        driver: D,
        plan: &'a PreparedRecordingPlan,
        ctx: TrackMonitorContext,
        engine: EngineConfig,
        ramp_frames: u32,
    ) -> Result<Self, MonitorEngineError> {
        if plan.sample_rate != engine.sample_rate {
            return Err(MonitorEngineError::SampleRateMismatch {
                plan: plan.sample_rate,
                engine: engine.sample_rate,
            });
        }
        let session = MonitorSession::new(
            plan,
            ctx,
            usize::from(engine.channels),
            engine.block_frames,
            ramp_frames,
        )?;
        let frame_offset = driver.cursor();
        Ok(Self {
            driver,
            session,
            engine,
            frame_offset,
            blocks_run: 0,
        })
    }

    /// `blocks` block を実 run 経路で駆動し、master tap PCM と
    /// monitor PCM を [`MonitorRunReport`] で返す。
    ///
    /// `pump_every` は `PumpPolicy::Every(n)` 相当 — `n > 0` で
    /// `blocks_run % n == 0` の block で producer を pump する
    /// （`n == 0` は never）。呼出しをまたいだ連続 cadence。
    /// block の合間に [`Self::set_phase`]/[`Self::push_input_block`]
    /// で session 入力を供給できる（`run(1, ...)` を繰り返す形）。
    ///
    /// # Errors
    /// engine runtime の [`ProcessError`]。
    pub fn run(
        &mut self,
        blocks: usize,
        pump_every: u64,
    ) -> Result<MonitorRunReport, ProcessError> {
        let channels = usize::from(self.engine.channels);
        let block_frames = self.engine.block_frames;
        let mut playback: Vec<Vec<f32>> = vec![Vec::new(); channels];
        let mut monitor: Vec<Vec<f32>> = vec![Vec::new(); channels];
        let mut frames_total = 0_u64;
        for _ in 0..blocks {
            let cursor = self.driver.cursor();
            // `run(1, ...)` 反復では呼出し内 index が常に 0 に戻り
            // `index % n` の cadence が狂うため、pump 要否を
            // `blocks_run` で判定し Every(1)/Every(0) を切替える
            // （Every(1) は index 0 で必ず pump、Every(0) は never
            // — 単一 run(N, Every(n)) と同一 cadence）。
            let policy = if pump_every > 0 && self.blocks_run.is_multiple_of(pump_every) {
                PumpPolicy::Every(1)
            } else {
                PumpPolicy::Every(0)
            };
            let report = self.driver.run_blocks(1, policy)?;
            self.blocks_run += 1;
            frames_total = frames_total.saturating_add(report.frames);
            for (c, ch) in report.pcm.iter().enumerate().take(channels) {
                playback[c].extend_from_slice(ch);
            }
            let monitor_pcm = if let Some(session_frame) = cursor.checked_sub(self.frame_offset) {
                self.session
                    .on_block(session_frame, block_frames, &report.pcm)
            } else {
                // engine cursor が attach offset を下回った
                // （後方 seek 等）— session 位置を照合不能とし
                // fail-closed。
                self.session.fault();
                silence(channels, block_frames)
            };
            for (c, ch) in monitor_pcm.iter().enumerate().take(channels) {
                monitor[c].extend_from_slice(ch);
            }
        }
        Ok(MonitorRunReport {
            playback,
            monitor,
            frames: frames_total,
        })
    }

    /// 内包する engine driver。
    #[must_use]
    pub const fn driver(&self) -> &D {
        &self.driver
    }

    /// 内包する engine driver（mutable）。
    pub const fn driver_mut(&mut self) -> &mut D {
        &mut self.driver
    }

    /// monitor session。
    #[must_use]
    pub const fn session(&self) -> &MonitorSession<'a> {
        &self.session
    }

    /// monitor session（mutable）。phase・入力 PCM の供給に使う。
    pub const fn session_mut(&mut self) -> &mut MonitorSession<'a> {
        &mut self.session
    }

    /// 次 block 以降の transport 相を設定する。
    pub const fn set_phase(&mut self, phase: TransportPhase) {
        self.session.set_phase(phase);
    }

    /// 次の engine block の入力 PCM を staging する。
    pub fn push_input_block(&mut self, pcm: Vec<Vec<f32>>) {
        self.session.push_input_block(pcm);
    }

    /// engine cursor（処理済み frame 数）。
    #[must_use]
    pub fn cursor(&self) -> u64 {
        self.driver.cursor()
    }
}
