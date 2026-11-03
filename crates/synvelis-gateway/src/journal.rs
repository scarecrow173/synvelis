//! Commit journal: 追記専用の binary framed record（GW-06、REC-07〜09）。
//!
//! GW-05 は proposal/approval/apply の不変 record をメモリ上の
//! [`crate::ProposalStore`] に保持するだけで、process 再起動で全て失われる。
//! GW-06 は commit fence が下した *決定済み* 結果を本 journal に永続化し、
//! crash/restart 後に何が確定し、何が確定できなかったかを復元する。
//!
//! Layout は recording staging journal（`synvelis-recording/src/journal.rs`）
//! と同一の framing を採用する（全て little-endian）:
//!
//! ```text
//! +0   u16  magic = 0x4A4A ("JJ")
//! +2   u8   version = 1
//! +3   u8   reserved = 0
//! +4   u64  seq   （journal 内で 0 から連番）
//! +12  u32  payload_len
//! +16  [payload_len bytes payload]  (serde_json 可視表現)
//! +..  u32  crc32  （magic 除く header+payload、offset 2..crc-1）
//! ```
//!
//! Replay は header 不完全・magic/version 不整合・長さ不正・crc 不一致・
//! seq 欠番/逆転のいずれかで停止し、それまでを検証可能 prefix として返す。
//! 破損以降の byte は復元時に切詰めて報告する（single writer の append-only
//! なので、torn tail は fsync 前に死んだ不完全 record でしかあり得ない）。
//!
//! ## 耐久性の約束
//!
//! - `FileJournal::append` は frame を書いて `sync_data` してから戻る。
//!   戻った record は crash/power loss（storage が約束する範囲）を越えて残る。
//! - `OpenOptions` による open は既存内容を scan し、intact prefix までを
//!   検証可能とみなした上で torn tail を切詰めてから追記を再開する。
//!   切詰め事実は [`OpenReport`] で呼出し側へ報告し、成功を誤認しない。
//! - fsync/FD レベルより深い障害（虚偽報告するストレージ、bit rot）は
//!   本 contract では保証外とし、実装記録に限界として明記する。

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// record magic（"JJ"）。
pub const MAGIC: u16 = 0x4A4A;
/// 現行 format version。
pub const VERSION: u8 = 1;
/// 固定 header 長（byte）。
pub const HEADER_LEN: usize = 16;
/// payload 上限（commit record は project 全体の snapshot を含むため、
/// staging journal より大きな上限を置く）。
pub const MAX_PAYLOAD: u32 = 1 << 24;

/// IEEE CRC32（polynomial `0xEDB8_8320`、reflected）。
///
/// `synvelis-recording/src/crc32.rs` と同一アルゴリズムの独立実装。
/// gateway が recording へ依存しないよう、frame 用の最小限のみここに持つ。
#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// replay の停止理由。
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum StopReason {
    /// 末尾まで全 record が intact。
    #[default]
    CleanEnd,
    /// header/payload が byte 途中で切れた（torn tail）。
    TornTail,
    /// magic が一致しない。
    BadMagic,
    /// version が未対応。
    BadVersion,
    /// reserved byte が 0 ではない（未知の format 拡張）。
    BadReserved,
    /// `payload_len` が上限超過または残り不足。
    BadLength,
    /// crc32 不一致。
    BadChecksum,
    /// seq が期待値と一致しない（欠番・逆転・重複）。
    SequenceGap,
}

/// 1 record（header の seq + JSON payload byte 列）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalRecord {
    /// journal 内連番（0 起点）。
    pub seq: u64,
    /// `serde_json` で encode された payload byte 列。
    pub payload: Vec<u8>,
}

/// replay の結果。
#[derive(Clone, Debug)]
pub struct ReplayOutcome {
    /// 検証可能 prefix の intact record。
    pub records: Vec<JournalRecord>,
    /// 検証可能 prefix の byte 長。
    pub prefix_len: u64,
    /// 停止理由。
    pub stop: StopReason,
}

/// `payload`（JSON byte 列）を 1 frame に encode する。
/// `payload_len > MAX_PAYLOAD` なら `None`。
#[must_use]
pub fn encode_frame(seq: u64, payload: &[u8]) -> Option<Vec<u8>> {
    let payload_len = u32::try_from(payload.len()).ok()?;
    if payload_len > MAX_PAYLOAD {
        return None;
    }
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len() + 4);
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.push(VERSION);
    out.push(0);
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&payload_len.to_le_bytes());
    let crc = {
        let mut framed = out.clone();
        framed.extend_from_slice(payload);
        crc32(&framed[2..])
    };
    out.extend_from_slice(payload);
    out.extend_from_slice(&crc.to_le_bytes());
    Some(out)
}

/// `bytes` を先頭から replay し、最初の破損点で停止する。
#[must_use]
pub fn replay(bytes: &[u8]) -> ReplayOutcome {
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
        if u16::from_le_bytes([header[0], header[1]]) != MAGIC {
            break StopReason::BadMagic;
        }
        if header[2] != VERSION {
            break StopReason::BadVersion;
        }
        if header[3] != 0 {
            break StopReason::BadReserved;
        }
        let seq = u64::from_le_bytes(header[4..12].try_into().unwrap_or_default());
        let payload_len = u32::from_le_bytes(header[12..16].try_into().unwrap_or_default());
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
        let crc_field = &bytes[pos + HEADER_LEN + plen..record_end];
        let crc_stored = u32::from_le_bytes(crc_field.try_into().unwrap_or_default());
        if crc32(&bytes[pos + 2..pos + HEADER_LEN + plen]) != crc_stored {
            break StopReason::BadChecksum;
        }
        if seq != next_seq {
            break StopReason::SequenceGap;
        }
        records.push(JournalRecord {
            seq,
            payload: bytes[pos + HEADER_LEN..pos + HEADER_LEN + plen].to_vec(),
        });
        next_seq = next_seq.saturating_add(1);
        pos = record_end;
    };
    ReplayOutcome {
        records,
        prefix_len: u64::try_from(pos).unwrap_or(u64::MAX),
        stop,
    }
}

/// `FileJournal::open` の報告 — torn tail の切詰めなど、再起動時の修復を
/// 呼出し側が黙殺しないよう明示する。
#[derive(Clone, Debug, Default)]
pub struct OpenReport {
    /// replay 結果（intact record 一覧と停止理由）。
    pub replay: Option<ReplayOutcome>,
    /// torn tail として切詰めた byte 数。
    pub truncated_bytes: u64,
}

/// append-only file journal。`append` は書込み + `sync_data` を行う。
pub struct FileJournal {
    path: PathBuf,
    file: File,
    next_seq: u64,
}

impl FileJournal {
    /// `path` を開き、既存内容を replay 検証した上で追記を再開する。
    /// torn tail は切詰める（修復）。なければ新規作成。
    ///
    /// # Errors
    /// ファイルの open/scan/repair における IO 失敗。
    pub fn open(path: impl AsRef<Path>) -> io::Result<(Self, OpenReport)> {
        let path = path.as_ref().to_path_buf();
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let out = replay(&bytes);
        let mut report = OpenReport {
            truncated_bytes: u64::try_from(bytes.len())
                .unwrap_or(u64::MAX)
                .saturating_sub(out.prefix_len),
            replay: Some(out),
        };
        let next_seq = report
            .replay
            .as_ref()
            .and_then(|r| r.records.last())
            .map_or(0, |r| r.seq.saturating_add(1));
        if report.truncated_bytes > 0 {
            let prefix_len = report.replay.as_ref().map_or(0, |r| r.prefix_len);
            file.set_len(prefix_len)?;
            file.sync_data()?;
        }
        file.seek(SeekFrom::End(0))?;
        if bytes.is_empty() {
            report.replay = None;
        }
        Ok((
            Self {
                path,
                file,
                next_seq,
            },
            report,
        ))
    }

    /// journal ファイルの path。
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 次に発行される seq。
    #[must_use]
    pub const fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// `payload`（JSON byte 列）を append し、同期してから seq を返す。
    /// `sync_data` 完了後に戻るため、返った record は crash を越えて残る。
    ///
    /// # Errors
    /// frame 上限超過時は `InvalidInput`、書込み/同期失敗時は IO error。
    /// 失敗した append は seq を消費しない — 部分的に書き込まれた
    /// tail は即座に切詰める。放置すると後続の *成功した* record が
    /// torn byte の後ろに積まれ、replay がそこで止まって確認済みの
    /// commit まで失われるため。
    pub fn append(&mut self, payload: &[u8]) -> io::Result<u64> {
        self.append_inner(payload, None)
    }

    /// `append` と同じだが、`fail_after` byte 書込んだ時点で write が
    /// 失敗したものとして扱う — torn-append の rollback path を試す
    /// fault-injection seam。
    ///
    /// # Errors
    /// `append` と同じ。`fail_after` が frame 長未満なら必ず失敗する。
    #[doc(hidden)]
    pub fn append_fail_after(&mut self, payload: &[u8], fail_after: usize) -> io::Result<u64> {
        self.append_inner(payload, Some(fail_after))
    }

    fn append_inner(&mut self, payload: &[u8], fail_after: Option<usize>) -> io::Result<u64> {
        let frame = encode_frame(self.next_seq, payload).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "journal record too large")
        })?;
        let start = self.file.stream_position()?;
        let write = match fail_after {
            Some(n) if n < frame.len() => self.file.write_all(&frame[..n]).and_then(|()| {
                Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "injected write failure",
                ))
            }),
            _ => self
                .file
                .write_all(&frame)
                .and_then(|()| self.file.sync_data()),
        };
        if let Err(err) = write {
            let _ = self.file.set_len(start);
            let _ = self.file.seek(SeekFrom::End(0));
            return Err(err);
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.saturating_add(1);
        Ok(seq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_vector() {
        // IEEE CRC32 of "123456789" = 0xCBF43926
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn roundtrip_and_torn_tail() {
        let mut file = encode_frame(0, b"{}").unwrap();
        let second = encode_frame(1, b"{\"a\":1}").unwrap();
        file.extend_from_slice(&second);
        let out = replay(&file);
        assert_eq!(out.stop, StopReason::CleanEnd);
        assert_eq!(out.records.len(), 2);

        let mut torn = file.clone();
        torn.truncate(file.len() - 5);
        let out = replay(&torn);
        assert_eq!(out.stop, StopReason::TornTail);
        assert_eq!(out.records.len(), 1);
        assert_eq!(
            out.prefix_len,
            u64::try_from(file.len() - second.len()).unwrap()
        );
    }

    #[test]
    fn failed_append_rolls_back_so_the_next_record_stays_replayable() {
        // P0 regression: a write that dies mid-frame must not leave torn
        // bytes the *next* successful append stacks records behind —
        // replay would stop at the tear and lose confirmed records.
        let path =
            std::env::temp_dir().join(format!("synvelis-journal-rollback-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let (mut journal, _) = FileJournal::open(&path).unwrap();
        journal.append(br#"{"ok":1}"#).unwrap();
        assert!(journal.append_fail_after(br#"{"dead":1}"#, 4).is_err());
        assert_eq!(journal.next_seq(), 1, "failed append consumes no seq");

        journal.append(br#"{"ok":2}"#).unwrap();
        drop(journal);

        let (journal, report) = FileJournal::open(&path).unwrap();
        let out = report.replay.unwrap();
        assert_eq!(out.stop, StopReason::CleanEnd);
        assert_eq!(out.records.len(), 2, "both confirmed records replay");
        assert_eq!(journal.next_seq(), 2);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn bad_checksum_and_seq_gap() {
        let mut file = encode_frame(0, b"a").unwrap();
        let mut corrupted = encode_frame(1, b"b").unwrap();
        let flip = HEADER_LEN;
        corrupted[flip] ^= 0xFF;
        file.extend_from_slice(&corrupted);
        assert_eq!(replay(&file).stop, StopReason::BadChecksum);

        let mut gap = encode_frame(0, b"a").unwrap();
        gap.extend_from_slice(&encode_frame(2, b"b").unwrap());
        assert_eq!(replay(&gap).stop, StopReason::SequenceGap);
    }
}
