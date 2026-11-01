//! Staging journal: 追記専用の binary framed record。
//!
//! Layout（全て little-endian）:
//! ```text
//! +0   u16  magic = 0x4A52 ("JR")
//! +2   u8   version = 1
//! +3   u8   kind  (1=Chunk, 2=Checkpoint, 3=Finalize)
//! +4   u64  seq   （stream 内で 0 から連番）
//! +12  u32  payload_len
//! +16  [payload_len bytes payload]
//! +..  u32  crc32  （magic を除く header+payload、offset 2..crc-1）
//! ```
//!
//! Replay は header 不完全・magic/version 不整合・長さ不正・crc 不一致・
//! seq 欠番/逆転のいずれかで停止し、それまでを検証可能 prefix として
//! 返す。破損以降の byte は削除・切詰めしない（保護・報告のみ）。

use crate::crc32::crc32;

/// record magic（"JR"）。
pub const MAGIC: u16 = 0x4A52;
/// 現行 format version。
pub const VERSION: u8 = 1;
/// 固定 header 長（byte）。
pub const HEADER_LEN: usize = 16;
/// payload 上限（chunk record の span 列は有界。これを超える record は
/// 破損として扱う）。
pub const MAX_PAYLOAD: u32 = 1 << 20;

const KIND_CHUNK: u8 = 1;
const KIND_CHECKPOINT: u8 = 2;
const KIND_FINALIZE: u8 = 3;

/// replay の停止理由。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// 末尾まで全 record が intact。
    CleanEnd,
    /// header/payload が byte 途中で切れた（torn tail）。
    TornTail,
    /// magic が一致しない。
    BadMagic,
    /// version が未対応。
    BadVersion,
    /// `payload_len` が上限超過または残り不足。
    BadLength,
    /// crc32 不一致。
    BadChecksum,
    /// seq が期待値と一致しない（欠番・逆転・重複）。
    SequenceGap,
}

/// Finalize record の outcome。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FinalizeOutcome {
    /// 全 source が frontier に含まれる完全録音。
    Complete,
    /// 途中で失敗した stream の verified prefix。
    Partial,
}

impl FinalizeOutcome {
    fn to_u8(self) -> u8 {
        match self {
            Self::Complete => 0,
            Self::Partial => 1,
        }
    }

    fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Complete),
            1 => Some(Self::Partial),
            _ => None,
        }
    }
}

/// journal に記録される 1 保存区間（`CaptureSpan` の永続形）。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct JournalSpan {
    /// Loop pass 番号。
    pub pass_index: u32,
    /// 区間先頭の session frame。
    pub session_start: i64,
    /// 区間長（frame）。
    pub frame_count: u32,
    /// 区間先頭の source frame。
    pub source_start: i64,
    /// 区間先頭の project tick。
    pub project_in: i64,
    /// 区間終端（exclusive）の project tick。
    pub project_out: i64,
    /// media file 内の byte offset（self-describing）。
    pub media_offset: u64,
}

/// kind=1: 1 chunk の書込み記録。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkRecord {
    /// pool スロット番号（診断用）。
    pub chunk_id: u32,
    /// block 先頭の device input frame。
    pub first_input_frame: u64,
    /// block 先頭の session frame。
    pub session_start: i64,
    /// block の frame 数。
    pub block_frames: u32,
    /// 保存 frame 総数（= `sum(spans.frame_count)`）。
    pub stored_frames: u32,
    /// media への書込み byte 範囲。
    pub media_offset: u64,
    /// media への書込み byte 長。
    pub media_len: u64,
    /// 書込み byte 列全体の CRC32。
    pub pcm_crc32: u32,
    /// この record 時点の digest fold 状態。
    pub digest_fold: u64,
    /// 保存区間列。
    pub spans: Vec<JournalSpan>,
}

/// kind=2: media+journal sync 済みの durable frontier 宣言。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CheckpointRecord {
    /// checkpoint 番号（stream 内で連番）。
    pub checkpoint_index: u64,
    /// sync した media byte 長。
    pub media_len: u64,
    /// sync した source frame 総数。
    pub source_frames: u64,
    /// この時点の digest fold 状態。
    pub digest_fold: u64,
    /// この checkpoint までに発行した chunk record 数。
    pub chunk_records: u32,
}

/// kind=3: finalize 実行の記録。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FinalizeRecord {
    /// 確定した media byte 長（durable frontier）。
    pub media_len: u64,
    /// 確定した source frame 総数。
    pub source_frames: u64,
    /// 確定した digest fold。
    pub digest_fold: u64,
    /// complete/partial。
    pub outcome: FinalizeOutcome,
}

/// record payload。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordPayload {
    /// chunk 書込み。
    Chunk(ChunkRecord),
    /// checkpoint。
    Checkpoint(CheckpointRecord),
    /// finalize。
    Finalize(FinalizeRecord),
}

/// 1 record（header の seq + payload）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalRecord {
    /// stream 内連番（0 起点）。
    pub seq: u64,
    /// payload。
    pub payload: RecordPayload,
}

// ---- encode helpers ----

fn put_u8(buf: &mut Vec<u8>, v: u8) {
    buf.push(v);
}
fn put_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn put_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn put_u64(buf: &mut Vec<u8>, v: u64) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn put_i64(buf: &mut Vec<u8>, v: i64) {
    buf.extend_from_slice(&v.to_le_bytes());
}

// ---- decode cursor ----

struct Cursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let s = self.buf.get(self.pos..end)?;
        self.pos = end;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|s| u16::from_le_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4)
            .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn u64(&mut self) -> Option<u64> {
        self.take(8)
            .map(|s| u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
    }
    fn i64(&mut self) -> Option<i64> {
        self.u64().map(u64::cast_signed)
    }
}

fn encode_payload(payload: &RecordPayload) -> Vec<u8> {
    let mut p = Vec::new();
    match payload {
        RecordPayload::Chunk(c) => {
            put_u32(&mut p, c.chunk_id);
            put_u64(&mut p, c.first_input_frame);
            put_i64(&mut p, c.session_start);
            put_u32(&mut p, c.block_frames);
            put_u32(&mut p, c.stored_frames);
            put_u16(&mut p, u16::try_from(c.spans.len()).unwrap_or(u16::MAX));
            put_u64(&mut p, c.media_offset);
            put_u64(&mut p, c.media_len);
            put_u32(&mut p, c.pcm_crc32);
            put_u64(&mut p, c.digest_fold);
            for s in &c.spans {
                put_u32(&mut p, s.pass_index);
                put_i64(&mut p, s.session_start);
                put_u32(&mut p, s.frame_count);
                put_i64(&mut p, s.source_start);
                put_i64(&mut p, s.project_in);
                put_i64(&mut p, s.project_out);
                put_u64(&mut p, s.media_offset);
            }
        }
        RecordPayload::Checkpoint(c) => {
            put_u64(&mut p, c.checkpoint_index);
            put_u64(&mut p, c.media_len);
            put_u64(&mut p, c.source_frames);
            put_u64(&mut p, c.digest_fold);
            put_u32(&mut p, c.chunk_records);
        }
        RecordPayload::Finalize(f) => {
            put_u64(&mut p, f.media_len);
            put_u64(&mut p, f.source_frames);
            put_u64(&mut p, f.digest_fold);
            put_u8(&mut p, f.outcome.to_u8());
        }
    }
    p
}

fn decode_payload(kind: u8, buf: &[u8]) -> Option<RecordPayload> {
    let mut c = Cursor::new(buf);
    match kind {
        KIND_CHUNK => {
            let chunk_id = c.u32()?;
            let first_input_frame = c.u64()?;
            let session_start = c.i64()?;
            let block_frames = c.u32()?;
            let stored_frames = c.u32()?;
            let span_count = usize::from(c.u16()?);
            let media_offset = c.u64()?;
            let media_len = c.u64()?;
            let pcm_crc32 = c.u32()?;
            let digest_fold = c.u64()?;
            let mut spans = Vec::with_capacity(span_count);
            for _ in 0..span_count {
                spans.push(JournalSpan {
                    pass_index: c.u32()?,
                    session_start: c.i64()?,
                    frame_count: c.u32()?,
                    source_start: c.i64()?,
                    project_in: c.i64()?,
                    project_out: c.i64()?,
                    media_offset: c.u64()?,
                });
            }
            // payload 全体を消費しなければ破損。
            if c.pos != buf.len() {
                return None;
            }
            Some(RecordPayload::Chunk(ChunkRecord {
                chunk_id,
                first_input_frame,
                session_start,
                block_frames,
                stored_frames,
                media_offset,
                media_len,
                pcm_crc32,
                digest_fold,
                spans,
            }))
        }
        KIND_CHECKPOINT => {
            let payload = RecordPayload::Checkpoint(CheckpointRecord {
                checkpoint_index: c.u64()?,
                media_len: c.u64()?,
                source_frames: c.u64()?,
                digest_fold: c.u64()?,
                chunk_records: c.u32()?,
            });
            (c.pos == buf.len()).then_some(payload)
        }
        KIND_FINALIZE => {
            let payload = RecordPayload::Finalize(FinalizeRecord {
                media_len: c.u64()?,
                source_frames: c.u64()?,
                digest_fold: c.u64()?,
                outcome: FinalizeOutcome::from_u8(c.u8()?)?,
            });
            (c.pos == buf.len()).then_some(payload)
        }
        _ => None,
    }
}

fn kind_of(payload: &RecordPayload) -> u8 {
    match payload {
        RecordPayload::Chunk(_) => KIND_CHUNK,
        RecordPayload::Checkpoint(_) => KIND_CHECKPOINT,
        RecordPayload::Finalize(_) => KIND_FINALIZE,
    }
}

/// `seq`/`payload` から 1 record の byte 列を encode する。
/// `payload_len > MAX_PAYLOAD` なら `None`。
#[must_use]
pub fn encode_record(seq: u64, payload: &RecordPayload) -> Option<Vec<u8>> {
    let body = encode_payload(payload);
    let payload_len = u32::try_from(body.len()).ok()?;
    if payload_len > MAX_PAYLOAD {
        return None;
    }
    let mut out = Vec::with_capacity(HEADER_LEN + body.len() + 4);
    put_u16(&mut out, MAGIC);
    put_u8(&mut out, VERSION);
    put_u8(&mut out, kind_of(payload));
    put_u64(&mut out, seq);
    put_u32(&mut out, payload_len);
    let crc = {
        let mut framed = out.clone();
        framed.extend_from_slice(&body);
        crc32(&framed[2..])
    };
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc.to_le_bytes());
    Some(out)
}

/// replay の結果。
#[derive(Clone, Debug)]
pub struct ReplayOutcome {
    /// 検証可能 prefix の intact record。
    pub records: Vec<JournalRecord>,
    /// 検証可能 prefix の byte 長。
    pub prefix_len: usize,
    /// 停止理由。
    pub stop: StopReason,
    /// 最後の intact record の seq（0 record なら `None`）。
    pub last_seq: Option<u64>,
}

/// `bytes` を先頭から replay し、最初の破損点で停止する。
/// `expected_first_seq` は通常 0（reopen 後の途中再開は将来拡張）。
#[must_use]
pub fn replay_journal(bytes: &[u8]) -> ReplayOutcome {
    let mut pos = 0usize;
    let mut next_seq = 0u64;
    let mut records = Vec::new();
    let stop = loop {
        let Some(header) = bytes.get(pos..pos.saturating_add(HEADER_LEN)) else {
            break if pos == bytes.len() {
                StopReason::CleanEnd
            } else {
                StopReason::TornTail
            };
        };
        let mut h = Cursor::new(header);
        let Some(magic) = h.u16() else {
            break StopReason::TornTail;
        };
        if magic != MAGIC {
            break StopReason::BadMagic;
        }
        let Some(version) = h.u8() else {
            break StopReason::TornTail;
        };
        if version != VERSION {
            break StopReason::BadVersion;
        }
        let Some(kind) = h.u8() else {
            break StopReason::TornTail;
        };
        let Some(seq) = h.u64() else {
            break StopReason::TornTail;
        };
        let Some(payload_len) = h.u32() else {
            break StopReason::TornTail;
        };
        if payload_len > MAX_PAYLOAD {
            break StopReason::BadLength;
        }
        let plen = usize::try_from(payload_len).unwrap_or(usize::MAX);
        let Some(record_end) = pos
            .checked_add(HEADER_LEN)
            .and_then(|v| v.checked_add(plen))
            .and_then(|v| v.checked_add(4))
        else {
            break StopReason::BadLength;
        };
        if record_end > bytes.len() {
            break StopReason::TornTail;
        }
        let body = &bytes[pos + HEADER_LEN..pos + HEADER_LEN + plen];
        let crc_field = &bytes[pos + HEADER_LEN + plen..record_end];
        let crc_stored =
            u32::from_le_bytes([crc_field[0], crc_field[1], crc_field[2], crc_field[3]]);
        if crc32(&bytes[pos + 2..pos + HEADER_LEN + plen]) != crc_stored {
            break StopReason::BadChecksum;
        }
        if seq != next_seq {
            break StopReason::SequenceGap;
        }
        let Some(payload) = decode_payload(kind, body) else {
            break StopReason::BadLength;
        };
        records.push(JournalRecord { seq, payload });
        next_seq = next_seq.saturating_add(1);
        pos = record_end;
    };
    ReplayOutcome {
        last_seq: records.last().map(|r| r.seq),
        records,
        prefix_len: pos,
        stop,
    }
}

/// journal への追記器（seq 採番を管理）。
pub struct JournalAppender {
    next_seq: u64,
}

impl JournalAppender {
    /// seq 0 から開始。
    #[must_use]
    pub fn new() -> Self {
        Self { next_seq: 0 }
    }

    /// `next_seq` から再開（reopen 後の追記用）。
    #[must_use]
    pub fn resumed(next_seq: u64) -> Self {
        Self { next_seq }
    }

    /// 次に発行される seq。
    #[must_use]
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// `payload` を encode して返し、seq を進める。
    /// `payload_len > MAX_PAYLOAD` なら `None`（seq は進めない）。
    pub fn encode_next(&mut self, payload: &RecordPayload) -> Option<Vec<u8>> {
        let bytes = encode_record(self.next_seq, payload)?;
        self.next_seq = self.next_seq.saturating_add(1);
        Some(bytes)
    }
}

impl Default for JournalAppender {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk_record() -> RecordPayload {
        RecordPayload::Chunk(ChunkRecord {
            chunk_id: 3,
            first_input_frame: 1024,
            session_start: 96,
            block_frames: 48,
            stored_frames: 32,
            media_offset: 512,
            media_len: 256,
            pcm_crc32: 0xDEAD_BEEF,
            digest_fold: 0x1234_5678,
            spans: vec![
                JournalSpan {
                    pass_index: 0,
                    session_start: 96,
                    frame_count: 16,
                    source_start: 64,
                    project_in: 0,
                    project_out: 5120,
                    media_offset: 512,
                },
                JournalSpan {
                    pass_index: 1,
                    session_start: 112,
                    frame_count: 16,
                    source_start: 80,
                    project_in: 0,
                    project_out: 5120,
                    media_offset: 640,
                },
            ],
        })
    }

    #[test]
    fn roundtrip_all_kinds() {
        let mut ap = JournalAppender::new();
        let payloads = [
            chunk_record(),
            RecordPayload::Checkpoint(CheckpointRecord {
                checkpoint_index: 0,
                media_len: 256,
                source_frames: 32,
                digest_fold: 0x1234_5678,
                chunk_records: 1,
            }),
            RecordPayload::Finalize(FinalizeRecord {
                media_len: 256,
                source_frames: 32,
                digest_fold: 0x1234_5678,
                outcome: FinalizeOutcome::Partial,
            }),
        ];
        let mut file = Vec::new();
        for p in &payloads {
            file.extend_from_slice(&ap.encode_next(p).unwrap());
        }
        let out = replay_journal(&file);
        assert_eq!(out.stop, StopReason::CleanEnd);
        assert_eq!(out.records.len(), 3);
        assert_eq!(out.prefix_len, file.len());
        for (i, r) in out.records.iter().enumerate() {
            assert_eq!(r.seq, u64::try_from(i).unwrap());
            assert_eq!(&r.payload, &payloads[i]);
        }
    }

    #[test]
    fn torn_tail_detected() {
        let mut ap = JournalAppender::new();
        let mut file = ap.encode_next(&chunk_record()).unwrap();
        let partial = ap.encode_next(&chunk_record()).unwrap();
        let keep = partial.len() - 7;
        file.extend_from_slice(&partial[..keep]);
        let out = replay_journal(&file);
        assert_eq!(out.stop, StopReason::TornTail);
        assert_eq!(out.records.len(), 1);
    }

    #[test]
    fn bad_checksum_detected() {
        let mut ap = JournalAppender::new();
        let mut file = ap.encode_next(&chunk_record()).unwrap();
        let good = ap.encode_next(&chunk_record()).unwrap();
        file.extend_from_slice(&good);
        // 2 record 目の payload を反転。
        let pos = file.len() - good.len() + HEADER_LEN + 4;
        file[pos] ^= 0xFF;
        let out = replay_journal(&file);
        assert_eq!(out.stop, StopReason::BadChecksum);
        assert_eq!(out.records.len(), 1);
    }

    #[test]
    fn seq_gap_detected() {
        let mut file = encode_record(0, &chunk_record()).unwrap();
        file.extend_from_slice(&encode_record(2, &chunk_record()).unwrap());
        let out = replay_journal(&file);
        assert_eq!(out.stop, StopReason::SequenceGap);
        assert_eq!(out.records.len(), 1);
    }

    #[test]
    fn bad_magic_detected() {
        let mut file = encode_record(0, &chunk_record()).unwrap();
        file[0] = 0xFF;
        let out = replay_journal(&file);
        assert_eq!(out.stop, StopReason::BadMagic);
        assert_eq!(out.records.len(), 0);
    }

    #[test]
    fn empty_is_clean() {
        let out = replay_journal(&[]);
        assert_eq!(out.stop, StopReason::CleanEnd);
        assert_eq!(out.records.len(), 0);
    }
}
