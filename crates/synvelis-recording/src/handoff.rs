//! RT → 非 RT の chunk handoff（双方向 SPSC）と Fault 通知。
//!
//! 各 capture stream は 1 対の `ruviform-rt` SPSC を持つ。
//! Filled は RT Producer → Writer Consumer、Free は Writer Producer →
//! RT Consumer で、各端点はちょうど 1 producer / 1 consumer の
//! 単一所有権。Fault 通知は Atomic Latch（side channel）で行い、
//! 満杯のデータ Queue に通知経路を依存させない。

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use ruviform_rt::{Consumer, Producer};

use crate::capture::CaptureChunk;

/// schedule lookahead 不足（`extend` が追いつかなかった）。
pub const FAULT_SCHEDULE_UNDERRUN: u32 = 1 << 0;
/// Filled Queue 満杯（Writer が消費不足）。
pub const FAULT_QUEUE_FULL: u32 = 1 << 1;
/// Free Pool 枯渇（Chunk が戻らない）。
pub const FAULT_FREE_EXHAUSTED: u32 = 1 << 2;
/// `CaptureSpan` 記録容量超過（1 block の piece 数 > `spans_per_chunk`）。
pub const FAULT_SPAN_OVERFLOW: u32 = 1 << 3;
/// PCM 容量超過（piece の frame 数が残容量を超えた）。
pub const FAULT_PCM_OVERFLOW: u32 = 1 << 4;
/// Clock Epoch の不連続。
pub const FAULT_EPOCH_DISCONTINUITY: u32 = 1 << 5;
/// Device input frame の不連続（重複・欠落を含む）。
pub const FAULT_INPUT_DISCONTINUITY: u32 = 1 << 6;
/// Writer 側異常（Free への返却失敗・不正 chunk）。
pub const FAULT_WRITER_ERROR: u32 = 1 << 7;
/// 契約違反（`stream_index` 不整合・PCM 長不整合・chunk 不正等）。
pub const FAULT_CONTRACT: u32 = 1 << 8;
/// チェック済み演算の失敗（`SessionFrame`/`SourceFrame`/`u64` overflow）。
pub const FAULT_ARITHMETIC: u32 = 1 << 9;

/// 全 bit の論理和（診断用）。
pub const FAULT_ANY: u32 = (1 << 10) - 1;

/// Atomic な単一方向 Fault latch。`set` は RT/非 RT のどちらからでも
/// 待たずに呼べ、いったん立った bit は `clear` しない限り残る。
/// 満杯のデータ Queue に通知を依存しない side channel。
#[derive(Clone, Debug)]
pub struct FaultLatch {
    bits: Arc<AtomicU32>,
}

impl FaultLatch {
    /// 初期状態 0 の latch。
    #[must_use]
    pub fn new() -> Self {
        Self {
            bits: Arc::new(AtomicU32::new(0)),
        }
    }

    /// `bit` を立てる（冪等・non-blocking）。
    pub fn set(&self, bit: u32) {
        self.bits.fetch_or(bit, Ordering::Release);
    }

    /// 現在の bit 集合。
    #[must_use]
    pub fn bits(&self) -> u32 {
        self.bits.load(Ordering::Acquire)
    }

    /// いずれかの fault が立っているか。
    #[must_use]
    pub fn is_faulted(&self) -> bool {
        self.bits() != 0
    }
}

impl Default for FaultLatch {
    fn default() -> Self {
        Self::new()
    }
}

/// RT 側 endpoint（Filled Producer + Free Consumer）。
pub struct EngineEnds {
    /// RT → Writer 方向（filled chunk の producer）。
    pub filled_tx: Producer<CaptureChunk>,
    /// Writer → RT 方向（free chunk の consumer）。
    pub free_rx: Consumer<CaptureChunk>,
}

/// 非 RT（Writer）側 endpoint（Filled Consumer + Free Producer）。
pub struct WriterEnds {
    /// RT → Writer 方向の consumer。
    pub filled_rx: Consumer<CaptureChunk>,
    /// Writer → RT 方向の producer。
    pub free_tx: Producer<CaptureChunk>,
}

/// Chunk pool + SPSC 対を構築し、全 chunk を Free 側へ流した状態で返す。
///
/// 割当てはここで一括して行い、以後の RT パスでは再割当てしない。
/// `chunks` は `free_capacity` と一致しなければならない（preflight で
/// 強制）ので、pool は必ず全 chunk を free に流せる。
///
/// # Errors
/// なし（構築時の容量は事前検証済みを前提。不正容量は panic ではなく
/// 空の pool を返す）。
#[must_use]
pub fn build_handoff(
    stream_index: u16,
    chunk_frames: u32,
    channels: u32,
    spans_per_chunk: usize,
    chunks: usize,
    filled_capacity: usize,
) -> (EngineEnds, WriterEnds) {
    let (filled_tx, filled_rx) = ruviform_rt::channel::<CaptureChunk>(filled_capacity.max(1));
    let (mut free_tx, free_rx) = ruviform_rt::channel::<CaptureChunk>(chunks.max(1));
    for id in 0..chunks {
        let chunk = CaptureChunk::new(
            u32::try_from(id).unwrap_or(u32::MAX),
            stream_index,
            chunk_frames,
            channels,
            spans_per_chunk,
        );
        // free_capacity == chunks が保証されているので失敗しない。
        debug_assert!(free_tx.try_push(chunk).is_ok());
    }
    (
        EngineEnds { filled_tx, free_rx },
        WriterEnds { filled_rx, free_tx },
    )
}
