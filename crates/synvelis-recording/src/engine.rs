//! RT capture engine（1 stream につき 1 つ）。
//!
//! `process` は 1 入力ブロックを受け、input の連続性検査 → Gate 評価 →
//! free chunk 取得 → PCM/span 書き込み → filled push を行う。
//! RT パスでは heap 割当て・dealloc・lock・OS I/O を行わない。
//! 失敗した `try_push` が返した heap-owned chunk は固定 `retained`
//! スロットへ move して保持し、非 RT の teardown で回収する。
//! すべての Fault は `FaultLatch` に立てて fail-stop（以後 no-op）とする。

use ruviform_rt::{Consumer, Producer};
use synvelis_project::{ClockEpochId, SessionFrame, SessionRange, SourceFrame};

use crate::{
    bounded::BoundedVec,
    capture::{CaptureBlockHeader, CaptureChunk, CaptureSpan, ChunkValidity},
    clock::InputBlock,
    gate::{CapturePiece, GateError, GateSchedule, evaluate_gate},
    handoff::{
        EngineEnds, FAULT_ARITHMETIC, FAULT_CONTRACT, FAULT_EPOCH_DISCONTINUITY,
        FAULT_FREE_EXHAUSTED, FAULT_INPUT_DISCONTINUITY, FAULT_PCM_OVERFLOW, FAULT_QUEUE_FULL,
        FAULT_SCHEDULE_UNDERRUN, FAULT_SPAN_OVERFLOW, FaultLatch,
    },
};

/// 1 stream の RT capture engine。endpoint は `new` で所有権を受け、
/// `into_ends` で teardown 用に返す。
pub struct CaptureEngine {
    stream_index: u16,
    channels: u32,
    /// 初ブロックで採種する Clock Epoch baseline。
    expected_epoch: Option<ClockEpochId>,
    /// 次に期待する device input frame（gate 閉塞でも進める）。
    next_input_frame: u64,
    /// session 上の現在位置（fault/finish で凍結）。
    session_cursor: SessionFrame,
    /// 次に割り当てる `SourceFrame`（有効 capture のみで進む）。
    next_source_frame: SourceFrame,
    session_end: SessionFrame,
    schedule: GateSchedule,
    /// engine 所有の piece 用 scratch（per-call 構築禁止のため保有）。
    scratch: BoundedVec<CapturePiece>,
    filled_tx: Producer<CaptureChunk>,
    free_rx: Consumer<CaptureChunk>,
    /// push 失敗・経路不整合 chunk の固定駐車場。fail-stop で ≤1。
    retained: Option<CaptureChunk>,
    fault: FaultLatch,
}

/// `into_ends` の戻り。engine が保持する endpoint と保留 chunk。
pub struct EngineEndsBack {
    /// Filled Producer（Writer が対になる Consumer を持つ）。
    pub filled_tx: Producer<CaptureChunk>,
    /// Free Consumer（Writer が対になる Producer を持つ）。
    pub free_rx: Consumer<CaptureChunk>,
    /// 失敗 push / 経路不整合で RT 上に保持された chunk（≤1）。
    pub retained: Option<CaptureChunk>,
}

impl CaptureEngine {
    /// 構築。`lookahead_spans` は schedule window と piece scratch の
    /// 双方の容量（1 block 内 piece 数は window 内 span 数を超えない）。
    #[must_use]
    pub fn new(
        stream_index: u16,
        channels: u32,
        session_end: SessionFrame,
        ends: EngineEnds,
        fault: FaultLatch,
        lookahead_spans: usize,
    ) -> Self {
        Self {
            stream_index,
            channels,
            expected_epoch: None,
            next_input_frame: 0,
            session_cursor: SessionFrame::ZERO,
            next_source_frame: SourceFrame::ZERO,
            session_end,
            schedule: GateSchedule::new(lookahead_spans),
            scratch: BoundedVec::with_capacity(lookahead_spans),
            filled_tx: ends.filled_tx,
            free_rx: ends.free_rx,
            retained: None,
            fault,
        }
    }

    /// lookahead schedule（`extend` 用の可変参照）。
    pub fn schedule_mut(&mut self) -> &mut GateSchedule {
        &mut self.schedule
    }

    /// Fault latch。
    #[must_use]
    pub fn fault(&self) -> &FaultLatch {
        &self.fault
    }

    /// いずれかの Fault が立っているか（以後 `process` は no-op）。
    #[must_use]
    pub fn is_faulted(&self) -> bool {
        self.fault.is_faulted()
    }

    /// 現在の session 位置（fail-stop で凍結）。
    #[must_use]
    pub fn session_cursor(&self) -> SessionFrame {
        self.session_cursor
    }

    /// 次の `SourceFrame`（有効 capture 分のみ進む）。
    #[must_use]
    pub fn source_cursor(&self) -> SourceFrame {
        self.next_source_frame
    }

    /// session 終端に到達したか。
    #[must_use]
    pub fn finished(&self) -> bool {
        self.session_cursor >= self.session_end
    }

    /// 保留 chunk（shutdown 回収対象）。
    #[must_use]
    pub fn retained(&self) -> Option<&CaptureChunk> {
        self.retained.as_ref()
    }

    fn fail(&mut self, bit: u32) {
        self.fault.set(bit);
    }

    /// 1 入力ブロックを処理する（RT パス）。
    ///
    /// `pcm` は `block.frames * channels` の interleaved sample で、
    /// 長さが一致しない場合は CONTRACT fault。Gate が全区間閉の
    /// ブロックは chunk を生成しないが、session/input カーソルは
    /// 必ず進める（次ブロックの連続性検査を破らないため）。
    ///
    /// 進行は「検査 → Gate 評価 → chunk 確保 → 書き込み → push →
    /// カーソル前進」。Fault を立てた時点でそれ以降を行わず
    /// fail-stop し、session cursor は凍結（以後の capture を捏造しない）。
    #[allow(clippy::too_many_lines)] // sequential fail-stop checks
    pub fn process(&mut self, block: &InputBlock, pcm: &[f32]) {
        if self.is_faulted() || self.finished() {
            return;
        }
        // --- input 契約 ---
        if block.frames == 0 {
            self.fail(FAULT_CONTRACT);
            return;
        }
        let expected_samples = usize::try_from(block.frames)
            .ok()
            .and_then(|f| f.checked_mul(usize::try_from(self.channels).ok()?));
        if expected_samples != Some(pcm.len()) {
            self.fail(FAULT_CONTRACT);
            return;
        }
        match self.expected_epoch {
            None => {
                // 初ブロックで baseline を採種（0 起点とは限らない）。
                self.expected_epoch = Some(block.clock_epoch);
                self.next_input_frame = block.first_input_frame;
            }
            Some(epoch) => {
                if block.clock_epoch != epoch {
                    self.fail(FAULT_EPOCH_DISCONTINUITY);
                    return;
                }
                if block.first_input_frame != self.next_input_frame {
                    self.fail(FAULT_INPUT_DISCONTINUITY);
                    return;
                }
            }
        }
        let Some(next_input) = block.first_input_frame.checked_add(u64::from(block.frames)) else {
            self.fail(FAULT_ARITHMETIC);
            return;
        };

        // --- block 範囲（session_end を超える尻尾は評価対象を切る）---
        let Ok(full_end) = self.session_cursor.checked_add(i64::from(block.frames)) else {
            self.fail(FAULT_ARITHMETIC);
            return;
        };
        let eval_end = full_end.min(self.session_end);
        let Ok(block_range) = SessionRange::try_new(self.session_cursor, eval_end) else {
            self.fail(FAULT_CONTRACT);
            return;
        };

        // --- Gate 評価 ---
        // scratch を engine から取り出す（`with_capacity(0)` は割当て無し）。
        // fill ループで piece を読みつつ self を変更できるようにするため。
        let mut scratch = core::mem::replace(&mut self.scratch, BoundedVec::with_capacity(0));
        scratch.clear();
        if let Err(e) = evaluate_gate(&mut self.schedule, block_range, &mut scratch) {
            self.scratch = scratch;
            self.fail(match e {
                GateError::Underrun => FAULT_SCHEDULE_UNDERRUN,
                GateError::SpanCapacity => FAULT_SPAN_OVERFLOW,
            });
            return;
        }

        if !scratch.is_empty() {
            // --- chunk 確保 ---
            let Ok(mut chunk) = self.free_rx.try_pop() else {
                self.scratch = scratch;
                self.fail(FAULT_FREE_EXHAUSTED);
                return;
            };
            chunk.reset();
            if chunk.stream_index != self.stream_index {
                // 経路不整合: RT 上で drop も他 stream への返却もできない。
                self.retained = Some(chunk);
                self.scratch = scratch;
                self.fail(FAULT_CONTRACT);
                return;
            }
            // --- PCM + span 書き込み ---
            let block_start_v = self.session_cursor.value();
            for piece in scratch.as_slice() {
                let Ok(frame_count) = piece.session_out.frames_since(piece.session_in) else {
                    self.fail(FAULT_ARITHMETIC);
                    break;
                };
                let Ok(frame_count_u32) = u32::try_from(frame_count) else {
                    self.fail(FAULT_ARITHMETIC);
                    break;
                };
                let samples = usize::try_from(frame_count)
                    .ok()
                    .and_then(|f| f.checked_mul(usize::try_from(self.channels).ok()?));
                let Some(samples) = samples else {
                    self.fail(FAULT_ARITHMETIC);
                    break;
                };
                if samples > chunk.pcm.len() - chunk.stored_samples {
                    self.fail(FAULT_PCM_OVERFLOW);
                    break;
                }
                let src_frame_off = piece.session_in.value() - block_start_v;
                let Some(src_sample_off) = usize::try_from(src_frame_off)
                    .ok()
                    .and_then(|o| o.checked_mul(usize::try_from(self.channels).ok()?))
                else {
                    self.fail(FAULT_ARITHMETIC);
                    break;
                };
                chunk.pcm[chunk.stored_samples..chunk.stored_samples + samples]
                    .copy_from_slice(&pcm[src_sample_off..src_sample_off + samples]);
                let span = CaptureSpan {
                    pass_index: piece.pass_index,
                    session_start: piece.session_in,
                    frame_count: frame_count_u32,
                    packed_pcm_offset: chunk.stored_samples,
                    source_start: self.next_source_frame,
                    project_in: piece.project_in,
                    project_out: piece.project_out,
                };
                if chunk.spans.push(span).is_err() {
                    self.fail(FAULT_SPAN_OVERFLOW);
                    break;
                }
                chunk.stored_samples += samples;
                chunk.stored_frames = chunk.stored_frames.saturating_add(frame_count_u32);
                let Ok(next_src) = self.next_source_frame.checked_add(frame_count) else {
                    self.fail(FAULT_ARITHMETIC);
                    break;
                };
                self.next_source_frame = next_src;
            }
            if self.is_faulted() {
                // 書き込み途中の fault: 部分データ chunk は所有権を失わない
                // よう Faulted validity で保持（Writer へは送らない）。
                chunk.validity = ChunkValidity::Faulted;
                if self.retained.is_none() {
                    self.retained = Some(chunk);
                }
                self.scratch = scratch;
                return;
            }
            chunk.header = CaptureBlockHeader {
                clock_epoch: block.clock_epoch,
                session_start: self.session_cursor,
                first_input_frame: block.first_input_frame,
                block_frames: block.frames,
                has_capture: true,
            };
            if let Err(e) = self.filled_tx.try_push(chunk) {
                // push 失敗が返す heap-owned chunk は drop せず保持。
                self.retained = Some(e.into_inner());
                self.scratch = scratch;
                self.fail(FAULT_QUEUE_FULL);
                return;
            }
        }

        // --- カーソル前進（gate 閉塞でも必ず進める）---
        self.next_input_frame = next_input;
        // ブロック全体が session 時間を消費する。session_end 以降の
        // 尻尾は session 外なので終端で切る。
        self.session_cursor = full_end.min(self.session_end);
        self.scratch = scratch;
    }

    /// engine を分解して endpoint と保留 chunk を返す（非 RT teardown）。
    #[must_use]
    pub fn into_ends(self) -> EngineEndsBack {
        EngineEndsBack {
            filled_tx: self.filled_tx,
            free_rx: self.free_rx,
            retained: self.retained,
        }
    }
}
