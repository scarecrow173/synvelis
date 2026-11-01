//! 事前確保 Chunk と、その内容の正規表現。
//!
//! `CaptureChunk` は「PCM 領域 + Block 内保存区間列 + Block header +
//! Validity」を一つの所有権単位として RT → 非 RT へ渡す。
//! PCM/span は構築時に確保した固定容量を再利用し、RT パスでは
//! 再割当てしない。`ChunkId` は pool スロット番号で、shutdown 時の
//! 総数照合（provisioned == recovered）に使う。

#![allow(clippy::module_name_repetitions)]

use synvelis_project::{ClockEpochId, MusicalTick, SessionFrame, SourceFrame};

use crate::bounded::BoundedVec;

/// Chunk の安定識別子（pool スロット index）。
pub type ChunkId = u32;

/// 1 Block の文脈（非 PCM メタ）。
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CaptureBlockHeader {
    /// この block の Clock Epoch。
    pub clock_epoch: ClockEpochId,
    /// block 先頭の `SessionFrame`。
    pub session_start: SessionFrame,
    /// block 先頭の device input frame。
    pub first_input_frame: u64,
    /// block の frame 数。
    pub block_frames: u32,
    /// block 内に valid な capture が存在するか。
    pub has_capture: bool,
}

/// Chunk 内容の状態。Chunk 自体の所有権は常に一意。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum ChunkValidity {
    /// 完全データ。
    #[default]
    Complete,
    /// Gate が全 frame で閉じていたため PCM は無効（header のみ意味を持つ）。
    GateClosed,
    /// Fault 発生時に放出された Chunk。`header.block_frames` までの
    /// 内容は部分データであり、Fault フラグと併せて扱う。
    Faulted,
}

/// Chunk 内の 1 保存区間（半開 session 区間）。
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CaptureSpan {
    /// Loop pass 番号。
    pub pass_index: u32,
    /// 区間先頭の `SessionFrame`。
    pub session_start: SessionFrame,
    /// 区間長（frame）。`> 0`。
    pub frame_count: u32,
    /// `pcm` 内の packed 先頭 sample offset（interleaved index）。
    pub packed_pcm_offset: usize,
    /// 区間先頭の `SourceFrame`。
    pub source_start: SourceFrame,
    /// 区間先頭に対応する project tick。
    pub project_in: MusicalTick,
    /// 区間終端（exclusive）に対応する project tick。
    pub project_out: MusicalTick,
}

/// 事前確保された 1 Chunk。`pcm` は `frames * channels` 個の
/// interleaved sample 領域で、`spans` がその中の保存区間を指す。
pub struct CaptureChunk {
    /// pool スロット番号。
    pub id: ChunkId,
    /// 属する capture stream（多 stream 時の経路識別）。
    pub stream_index: u16,
    /// Block 文脈。
    pub header: CaptureBlockHeader,
    /// 内容の状態。
    pub validity: ChunkValidity,
    /// Block 内保存区間（有界）。
    pub spans: BoundedVec<CaptureSpan>,
    /// interleaved PCM 領域（`frames * channels`）。
    pub pcm: Box<[f32]>,
    /// `spans` が指す有効 sample 数（≤ `pcm.len()`）。
    pub stored_samples: usize,
    /// `spans` が指す総 frame 数。
    pub stored_frames: u32,
}

impl CaptureChunk {
    /// `frames * channels` の PCM と `spans_per_chunk` の span 領域を
    /// 確保する（この構築でのみ割当て）。
    #[must_use]
    pub fn new(
        id: ChunkId,
        stream_index: u16,
        frames: u32,
        channels: u32,
        spans_per_chunk: usize,
    ) -> Self {
        let samples = usize::try_from(frames)
            .unwrap_or(usize::MAX)
            .saturating_mul(usize::try_from(channels).unwrap_or(usize::MAX));
        Self {
            id,
            stream_index,
            header: CaptureBlockHeader {
                clock_epoch: ClockEpochId::new(0),
                session_start: SessionFrame::ZERO,
                first_input_frame: 0,
                block_frames: 0,
                has_capture: false,
            },
            validity: ChunkValidity::Complete,
            spans: BoundedVec::with_capacity(spans_per_chunk),
            pcm: vec![0.0; samples].into_boxed_slice(),
            stored_samples: 0,
            stored_frames: 0,
        }
    }

    /// PCM の frame 容量（`pcm.len() / channels`）。
    #[must_use]
    pub fn pcm_frame_capacity(&self, channels: u32) -> u32 {
        let ch = usize::try_from(channels).unwrap_or(usize::MAX).max(1);
        u32::try_from(self.pcm.len() / ch).unwrap_or(u32::MAX)
    }

    /// 再利用前の reset。span・カウンタ・オフセット・header を初期化する。
    /// （PCM バイト列は書き込みで上書きされるため非 zeroed でよい。）
    pub fn reset(&mut self) {
        self.spans.clear();
        self.stored_samples = 0;
        self.stored_frames = 0;
        self.validity = ChunkValidity::Complete;
        self.header.has_capture = false;
    }
}
