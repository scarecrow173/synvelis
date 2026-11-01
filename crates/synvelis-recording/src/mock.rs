//! Mock backend: 決定的 PCM 生成・入力ブロック供給・検査 Writer・
//! シナリオ driver・shutdown 回収照合。
//!
//! すべて非 RT 側の検証用コードで、RT 契約は engine/handoff 側に閉じる。
//! `MockPcm` は `(input_frame, channel, seed)` の純関数として PCM を
//! 生成するため、保存された chunk の内容を writer 側で完全に検算できる。

#![allow(clippy::module_name_repetitions)]

use ruviform_rt::{Consumer, Producer};
use synvelis_project::ClockEpochId;

use crate::{
    capture::{CaptureBlockHeader, CaptureChunk, CaptureSpan, ChunkId, ChunkValidity},
    clock::InputBlock,
    engine::CaptureEngine,
    handoff::{FaultLatch, WriterEnds, build_handoff},
    plan::{BufferBudget, PreparedRecordingPlan},
};

/// 決定的 PCM 生成器。`sample` は状態を持たない純関数。
#[derive(Copy, Clone, Debug)]
pub struct MockPcm {
    /// ストリームごとの seed。
    pub seed: u32,
}

impl MockPcm {
    /// `(input_frame, channel)` に対する決定的 sample。
    #[must_use]
    pub fn sample(&self, input_frame: u64, channel: u32) -> f32 {
        let v = input_frame.wrapping_mul(0x9E37_79B1)
            ^ u64::from(channel).wrapping_mul(0x85EB_CA6B)
            ^ u64::from(self.seed).wrapping_mul(0xC2B2_AE35);
        let mixed = (v ^ (v >> 29)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        let bits = u16::try_from(mixed >> 48).unwrap_or(u16::MAX);
        f32::from(bits) / 65536.0
    }

    /// `first_input_frame` から `frames` 分の interleaved PCM を生成する。
    #[must_use]
    pub fn block_pcm(&self, first_input_frame: u64, frames: u32, channels: u32) -> Vec<f32> {
        let mut out = Vec::with_capacity(
            usize::try_from(frames).unwrap_or(0) * usize::try_from(channels).unwrap_or(0),
        );
        for f in 0..frames {
            for ch in 0..channels {
                out.push(self.sample(first_input_frame.wrapping_add(u64::from(f)), ch));
            }
        }
        out
    }
}

/// 連続する device input frame 列を供給する Mock Clock/Input。
#[derive(Clone, Debug)]
pub struct MockInput {
    /// 現在の Clock Epoch。
    pub epoch: ClockEpochId,
    /// 次に供給する input frame 番号（0 起点とは限らない）。
    next_frame: u64,
}

impl MockInput {
    /// `first_frame` 起点の Mock input。
    #[must_use]
    pub fn new(epoch: ClockEpochId, first_frame: u64) -> Self {
        Self {
            epoch,
            next_frame: first_frame,
        }
    }

    /// `frames` 長の連続ブロックを払い出す。
    #[must_use]
    pub fn block(&mut self, frames: u32) -> InputBlock {
        let b = InputBlock {
            clock_epoch: self.epoch,
            first_input_frame: self.next_frame,
            frames,
        };
        self.next_frame = self.next_frame.wrapping_add(u64::from(frames));
        b
    }

    /// 供給位置を `frame` へ跳ばす（gap/dup 注入用）。
    pub fn jump_to(&mut self, frame: u64) {
        self.next_frame = frame;
    }

    /// Clock Epoch を切り替える（断絶注入用）。
    pub fn set_epoch(&mut self, epoch: ClockEpochId) {
        self.epoch = epoch;
    }
}

/// Writer が記録した chunk の検査ログ。
#[derive(Clone, Debug, PartialEq)]
pub struct ChunkLog {
    /// pool スロット番号。
    pub chunk_id: ChunkId,
    /// Block 文脈。
    pub header: CaptureBlockHeader,
    /// 内容の状態。
    pub validity: ChunkValidity,
    /// 保存区間列。
    pub spans: Vec<CaptureSpan>,
    /// `stored_samples` 分の PCM（検算用コピー）。
    pub pcm: Vec<f32>,
}

/// 検査 Writer: filled を非ブロックで消費し、ログ記録後に free へ戻す。
/// `hold` 注入で chunk を保持したままにでき、Queue 満杯・Free 枯渇を
/// 再現できる。
pub struct InspectionWriter {
    filled_rx: Consumer<CaptureChunk>,
    free_tx: Producer<CaptureChunk>,
    fault: FaultLatch,
    hold: usize,
    /// 解放せず保持中の chunk（回収照合対象）。
    pub held: Vec<CaptureChunk>,
    /// 受信済み chunk の記録（`held` と別勘定: 受信履歴 vs 現保有）。
    pub log: Vec<ChunkLog>,
}

impl InspectionWriter {
    /// `WriterEnds` と共有 `FaultLatch` から構築する。
    #[must_use]
    pub fn new(ends: WriterEnds, fault: FaultLatch) -> Self {
        Self {
            filled_rx: ends.filled_rx,
            free_tx: ends.free_tx,
            fault,
            hold: 0,
            held: Vec::new(),
            log: Vec::new(),
        }
    }

    /// 次に受け取る `n` chunk を free へ戻さず保持する（注入）。
    pub fn hold_next(&mut self, n: usize) {
        self.hold = self.hold.saturating_add(n);
    }

    /// filled を空になるまで非ブロックで消費する（非 RT pump）。
    pub fn pump(&mut self) {
        while let Ok(chunk) = self.filled_rx.try_pop() {
            self.log.push(ChunkLog {
                chunk_id: chunk.id,
                header: chunk.header,
                validity: chunk.validity,
                spans: chunk.spans.as_slice().to_vec(),
                pcm: chunk.pcm[..chunk.stored_samples].to_vec(),
            });
            if self.hold > 0 {
                self.hold -= 1;
                self.held.push(chunk);
            } else if let Err(e) = self.free_tx.try_push(chunk) {
                // free 側が満杯（契約上発生しないが所有権は保持する）。
                self.fault.set(crate::handoff::FAULT_WRITER_ERROR);
                self.held.push(e.into_inner());
            }
        }
    }

    /// endpoint と保持 chunk を返す（teardown）。
    #[must_use]
    pub fn into_ends(
        self,
    ) -> (
        Consumer<CaptureChunk>,
        Producer<CaptureChunk>,
        Vec<CaptureChunk>,
    ) {
        (self.filled_rx, self.free_tx, self.held)
    }
}

impl crate::handoff::ChunkSink for InspectionWriter {
    fn pump(&mut self) {
        InspectionWriter::pump(self);
    }

    fn into_ends(
        self,
    ) -> (
        Consumer<CaptureChunk>,
        Producer<CaptureChunk>,
        Vec<CaptureChunk>,
    ) {
        InspectionWriter::into_ends(self)
    }
}

/// 1 stream の engine + writer。
pub struct StreamRig<W = InspectionWriter> {
    /// RT engine。
    pub engine: CaptureEngine,
    /// 非 RT writer。
    pub writer: W,
}

/// 計画から構築されたシナリオ。stream ごとに独立した SPSC 対を持つ。
/// `W` は `ChunkSink` を満たす非 RT 消費者（`InspectionWriter` のほか
/// `StreamWriter` 等の staging 実装を差し替えられる）。
pub struct Scenario<W = InspectionWriter> {
    /// 凍結済み計画（scheduler は共有・無状態）。
    pub plan: PreparedRecordingPlan,
    /// stream ごとの rig。
    pub rigs: Vec<StreamRig<W>>,
    /// 共有 Fault latch。
    pub fault: FaultLatch,
    pcms: Vec<MockPcm>,
}

impl Scenario<InspectionWriter> {
    /// `plan` から pool・SPSC・engine・`InspectionWriter` を構築し、
    /// 全 chunk を free pool に流した状態で返す。
    #[must_use]
    pub fn build(plan: PreparedRecordingPlan) -> Self {
        Self::build_with(plan, |_i, ends, fault| InspectionWriter::new(ends, fault))
    }
}

impl<W: crate::handoff::ChunkSink> Scenario<W> {
    /// `plan` から pool・SPSC・engine と、stream ごとに `make` が作る
    /// writer を構築する。`make` には `stream_index`・`WriterEnds`・共有
    /// `FaultLatch` が渡される。
    pub fn build_with(
        plan: PreparedRecordingPlan,
        make: impl Fn(u16, WriterEnds, FaultLatch) -> W,
    ) -> Self {
        let fault = FaultLatch::new();
        let mut rigs = Vec::new();
        let mut pcms = Vec::new();
        for (i, target) in plan.targets.iter().enumerate() {
            let stream_index = u16::try_from(i).unwrap_or(u16::MAX);
            let b: BufferBudget = plan.budget;
            let (engine_ends, writer_ends) = build_handoff(
                stream_index,
                b.chunk_frames,
                target.channels,
                b.spans_per_chunk,
                b.chunks,
                b.filled_capacity,
            );
            let engine = CaptureEngine::new(
                stream_index,
                target.channels,
                plan.session_end,
                engine_ends,
                fault.clone(),
                b.lookahead_spans,
            );
            let writer = make(stream_index, writer_ends, fault.clone());
            rigs.push(StreamRig { engine, writer });
            pcms.push(MockPcm {
                seed: stream_index.into(),
            });
        }
        Self {
            plan,
            rigs,
            fault,
            pcms,
        }
    }

    /// 1 入力ブロックを全 stream に配る。
    ///
    /// 各 stream について「lookahead 補充（非 RT）→ process（RT）→
    /// writer.pump（非 RT）」を順に行う。
    pub fn feed(&mut self, block: &InputBlock) {
        debug_assert!(
            block.frames <= self.plan.max_block_frames,
            "block.frames {} exceeds max_block_frames {}",
            block.frames,
            self.plan.max_block_frames
        );
        for i in 0..self.rigs.len() {
            self.feed_engine(i, block);
            self.rigs[i].writer.pump();
        }
    }

    /// `feed` の engine 半分だけを行い writer.pump を呼ばない
    /// （Filled Queue 満杯の再現用）。
    pub fn feed_engine_only(&mut self, block: &InputBlock) {
        for i in 0..self.rigs.len() {
            self.feed_engine(i, block);
        }
    }

    fn feed_engine(&mut self, i: usize, block: &InputBlock) {
        let rig = &mut self.rigs[i];
        let until = rig
            .engine
            .session_cursor()
            .checked_add(i64::from(block.frames))
            .unwrap_or(self.plan.session_end)
            .min(self.plan.session_end);
        // 補充失敗は設計上発生しない（水位は bounded）。
        let _ = self.plan.scheduler.extend(rig.engine.schedule_mut(), until);
        let pcm = self.pcms[i].block_pcm(
            block.first_input_frame,
            block.frames,
            self.plan.targets[i].channels,
        );
        rig.engine.process(block, &pcm);
    }

    /// `session_end` まで `block_frames` 刻みで供給する。
    /// Fault が立った時点で停止する。
    pub fn run_to_end(&mut self, input: &mut MockInput, block_frames: u32) {
        while !self.fault.is_faulted() && self.rigs.iter().any(|r| !r.engine.finished()) {
            let block = input.block(block_frames);
            self.feed(&block);
        }
    }

    /// 全 stream の chunk を回収し、pool スロット ID の列を返す
    /// （非 RT teardown。`retained` + free キュー + filled 残り + held）。
    #[must_use]
    pub fn recover_all(self) -> Vec<ChunkId> {
        let mut ids = Vec::new();
        for rig in self.rigs {
            let ends = rig.engine.into_ends();
            if let Some(c) = ends.retained {
                ids.push(c.id);
            }
            let mut free_rx = ends.free_rx;
            while let Ok(c) = free_rx.try_pop() {
                ids.push(c.id);
            }
            drop(ends.filled_tx);
            let (mut filled_rx, _free_tx, held) = rig.writer.into_ends();
            while let Ok(c) = filled_rx.try_pop() {
                ids.push(c.id);
            }
            for c in held {
                ids.push(c.id);
            }
        }
        ids.sort_unstable();
        ids
    }
}

/// `ChunkLog` の内容を `MockPcm` で検算する。
/// 各 span の packed PCM 区間が、対応する input frame 列の生成値と
/// 完全一致することを確認する（重複・欠落は span 列の session 順序と
/// この検算の双方で検出される）。
///
/// 検算に使う input frame は `header.first_input_frame` と
/// `span.session_start - header.session_start` から復元する。
#[must_use]
pub fn verify_log(log: &ChunkLog, pcm_seed: u32, channels: u32) -> bool {
    let gen_ = MockPcm { seed: pcm_seed };
    for span in &log.spans {
        let frame_off = span
            .session_start
            .checked_sub(log.header.session_start.value())
            .map(synvelis_project::SessionFrame::value);
        let Ok(Some(input_base)) = frame_off.map(|o| {
            log.header
                .first_input_frame
                .checked_add(u64::try_from(o).unwrap_or(u64::MAX))
        }) else {
            return false;
        };
        let start = span.packed_pcm_offset;
        let need = usize::try_from(span.frame_count).unwrap_or(usize::MAX)
            * usize::try_from(channels).unwrap_or(usize::MAX);
        if start + need > log.pcm.len() {
            return false;
        }
        for f in 0..span.frame_count {
            for ch in 0..channels {
                let idx = start
                    + usize::try_from(f).unwrap_or(usize::MAX)
                        * usize::try_from(channels).unwrap_or(usize::MAX)
                    + usize::try_from(ch).unwrap_or(usize::MAX);
                if log.pcm[idx].to_bits()
                    != gen_
                        .sample(input_base.wrapping_add(u64::from(f)), ch)
                        .to_bits()
                {
                    return false;
                }
            }
        }
    }
    true
}
