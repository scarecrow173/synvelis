//! Recovery Scan: process 再起動を想定して staging root を列挙し、
//! 各 stream の journal replay と media 実長を照合する。
//!
//! 復旧 frontier は **Last Valid Checkpoint**（intact prefix 内の最後の
//! Checkpoint record）。未検証末尾（intact record が無い media byte、
//! journal 自体が破損した suffix、journal 不在の orphan media）は
//! 「復元した」と報告せず、Finalized 候補へも混入しない。破損・
//! 未検証の byte は削除・切詰めしない（保護・報告のみ）。

use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

use crate::{
    crc32::{crc32, digest_fold},
    journal::{
        FinalizeOutcome, FinalizeRecord, JournalAppender, JournalRecord, RecordPayload,
        ReplayOutcome, StopReason, replay_journal,
    },
    staging::{DirSyncCapability, StagingArea, publish_atomic, sync_parent_dir},
    writer::{
        DurableFrontier, FinalizedOutcome, FinalizedSourceRef, FinalizedSpan, WriterError,
        stream_paths,
    },
};

/// stream 単位の復旧分類。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StreamClass {
    /// checkpoint があり frontier まで media/journal が整合。
    Healthy,
    /// frontier 以後に validated な chunk record 残、または
    /// unverified tail（media > journaled frontier）がある。
    Partial,
    /// replay が破損で停止（最後の intact checkpoint までしか信頼
    /// しない）。
    Corrupted(StopReason),
    /// media は存在するが journal が空/不在（orphan media）。
    Unjournaled,
}

/// journal replay + media 実長の照合結果（stream 単位）。
#[derive(Clone, Debug)]
pub struct StreamRecovery {
    /// stream index。
    pub stream_index: u16,
    /// 復旧分類。
    pub class: StreamClass,
    /// Last Valid Checkpoint（無ければ `None`）。
    pub durable: Option<DurableFrontier>,
    /// durable frontier までの検証済み span 列（session 順）。
    pub verified_spans: Vec<FinalizedSpan>,
    /// frontier 以後に残る validated な（intact record のある）
    /// media byte 長。
    pub validated_tail_bytes: u64,
    /// intact record が無い media byte 長（unverified tail）。
    pub unverified_tail_bytes: u64,
    /// replay の停止理由。
    pub stop: StopReason,
    /// 観測した fault の診断記録。
    pub faults: Vec<String>,
}

/// 1 session の復旧結果。
#[derive(Clone, Debug)]
pub struct SessionRecovery {
    /// session 識別子（dir 名）。
    pub session_id: String,
    /// stream 単位の結果。
    pub streams: Vec<StreamRecovery>,
    /// session 単位の fault（manifest 破損など）。
    pub faults: Vec<String>,
}

/// staging root 全体の復旧報告。
#[derive(Clone, Debug)]
pub struct RecoveryReport {
    /// session 単位の結果（dir 名列挙順）。
    pub sessions: Vec<SessionRecovery>,
}

/// media + journal を照合した内部結果。
struct StreamScan {
    class: StreamClass,
    durable: Option<DurableFrontier>,
    verified_spans: Vec<FinalizedSpan>,
    validated_tail_bytes: u64,
    unverified_tail_bytes: u64,
    stop: StopReason,
    faults: Vec<String>,
}

/// `records`（intact prefix）と media 実長から scan 結果を作る。
/// `channels` は media byte ↔ frame の換算に使う。
fn scan_stream(outcome: &ReplayOutcome, media_len: u64, channels: u32) -> StreamScan {
    let mut faults = Vec::new();
    // 最後の intact Checkpoint record を durable frontier とする。
    let mut durable: Option<DurableFrontier> = None;
    let mut verified_spans: Vec<FinalizedSpan> = Vec::new();
    let mut journaled_end = 0u64;
    let frame_bytes = u64::from(channels) * 4;
    for rec in &outcome.records {
        match &rec.payload {
            RecordPayload::Chunk(c) => {
                let end = c.media_offset.saturating_add(c.media_len);
                journaled_end = journaled_end.max(end);
                // frontier 到達分のみ verified span に含める（checkpoint
                // record は後から来るため、一旦全 span を保持し、最後の
                // checkpoint で打切る）。
                for s in &c.spans {
                    verified_spans.push(FinalizedSpan {
                        pass_index: s.pass_index,
                        session_start: s.session_start,
                        session_end: s.session_start.saturating_add(i64::from(s.frame_count)),
                        source_start: s.source_start,
                        source_end: s.source_start.saturating_add(i64::from(s.frame_count)),
                        project_in: s.project_in,
                        project_out: s.project_out,
                        media_offset: s.media_offset,
                        media_len: u64::from(s.frame_count) * frame_bytes,
                    });
                }
            }
            RecordPayload::Checkpoint(cp) => {
                durable = Some(DurableFrontier {
                    seq: rec.seq,
                    checkpoint_index: cp.checkpoint_index,
                    media_len: cp.media_len,
                    source_frames: cp.source_frames,
                    digest_fold: cp.digest_fold,
                });
            }
            RecordPayload::Finalize(_) => {}
        }
    }
    // frontier 到達分の verified spans = durable.media_len までの span。
    let verified_spans: Vec<FinalizedSpan> = verified_spans
        .into_iter()
        .filter(|s| {
            durable.is_none_or(|d| s.media_offset.saturating_add(s.media_len) <= d.media_len)
        })
        .collect();
    let durable_len = durable.map_or(0, |d| d.media_len);
    let validated_tail_bytes = journaled_end.saturating_sub(durable_len);
    let unverified_tail_bytes = media_len.saturating_sub(journaled_end);
    // media が journal 被覆に満たない（外部からの切詰め・消失）。
    let media_short_bytes = journaled_end.saturating_sub(media_len);
    if media_short_bytes > 0 {
        faults.push(format!(
            "media shorter than journaled coverage: {media_len} < {journaled_end}"
        ));
    }
    let class = match (&outcome.stop, durable.is_some(), journaled_end) {
        // 破損停止（TornTail 以外）は intact prefix までしか信頼しない。
        (reason, _, _) if !matches!(reason, StopReason::CleanEnd | StopReason::TornTail) => {
            StreamClass::Corrupted(*reason)
        }
        // journal record が 1 件も無い（空 journal/欠落）→ orphan media。
        (_, _, 0) => StreamClass::Unjournaled,
        // checkpoint あり・clean・tail/short 無し → Healthy。
        (StopReason::CleanEnd, true, _)
            if validated_tail_bytes == 0
                && unverified_tail_bytes == 0
                && media_short_bytes == 0 =>
        {
            StreamClass::Healthy
        }
        _ => StreamClass::Partial,
    };
    StreamScan {
        class,
        durable,
        verified_spans,
        validated_tail_bytes,
        unverified_tail_bytes,
        stop: outcome.stop,
        faults,
    }
}

/// staging root 配下の session dir を列挙して復旧報告を作る。
/// manifest の無い dir は session 領域とみなさずスキップする。
///
/// # Errors
/// root 列挙・manifest 読込み・file 読込みの `io::Error`。
pub fn scan(root: &Path) -> std::io::Result<RecoveryReport> {
    let mut sessions = Vec::new();
    if !root.exists() {
        return Ok(RecoveryReport { sessions });
    }
    let mut dirs: Vec<PathBuf> = fs::read_dir(root)?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    for dir in dirs {
        let session_id = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let manifest_path = dir.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }
        // manifest 破損は session 全体を見えなくするのではなく、
        // fault 付きで列挙する（他 session への影響を隔離）。
        let manifest: Option<crate::staging::StagingManifest> = fs::read(&manifest_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let Some(manifest) = manifest else {
            sessions.push(SessionRecovery {
                session_id,
                streams: Vec::new(),
                faults: vec!["manifest unreadable/invalid".to_string()],
            });
            continue;
        };
        let mut streams = Vec::new();
        for s in &manifest.streams {
            let journal_path = dir.join(&s.journal);
            let media_path = dir.join(&s.media);
            let journal_bytes = if journal_path.exists() {
                fs::read(&journal_path).unwrap_or_default()
            } else {
                Vec::new()
            };
            let media_len = fs::metadata(&media_path).map_or(0, |m| m.len());
            let outcome = replay_journal(&journal_bytes);
            let scan = scan_stream(&outcome, media_len, s.channels);
            streams.push(StreamRecovery {
                stream_index: s.stream_index,
                class: scan.class,
                durable: scan.durable,
                verified_spans: scan.verified_spans,
                validated_tail_bytes: scan.validated_tail_bytes,
                unverified_tail_bytes: scan.unverified_tail_bytes,
                stop: scan.stop,
                faults: scan.faults,
            });
        }
        sessions.push(SessionRecovery {
            session_id,
            streams,
            faults: Vec::new(),
        });
    }
    Ok(RecoveryReport { sessions })
}

/// `verify_stream` の結果: frontier まで再検証した span 列と、
/// frontier 以後の tail 情報。
pub(crate) struct VerifiedStream {
    pub spans: Vec<FinalizedSpan>,
    pub validated_tail_bytes: u64,
    pub unverified_tail_bytes: u64,
}

/// Finalizer/Recovery の共通検査: journal を replay し、`frontier` と
/// 照合した上で media の実 byte を再読して各 chunk の CRC32・fold
/// digest を再検証する。
///
/// # Errors
/// journal 破損・frontier 不一致・media 実長不足・CRC/digest 不一致は
/// `WriterError::Corrupt`、file 読込み失敗は `Io`。
#[allow(clippy::too_many_lines)]
pub(crate) fn verify_stream(
    media_path: &Path,
    journal_path: &Path,
    frontier: DurableFrontier,
    channels: u32,
) -> Result<VerifiedStream, WriterError> {
    let journal_bytes = fs::read(journal_path).map_err(|e| WriterError::Io {
        op: "read journal",
        source: e,
    })?;
    let outcome = replay_journal(&journal_bytes);
    // frontier.seq までの record が intact でなければ破損。
    let upto: Vec<&JournalRecord> = outcome
        .records
        .iter()
        .take_while(|r| r.seq <= frontier.seq)
        .collect();
    let Some(last) = upto.last() else {
        return Err(WriterError::Corrupt("no records before frontier".into()));
    };
    if last.seq != frontier.seq {
        return Err(WriterError::Corrupt(
            "frontier seq beyond intact prefix".into(),
        ));
    }
    if !matches!(last.payload, RecordPayload::Checkpoint(_)) {
        return Err(WriterError::Corrupt(
            "frontier record is not a checkpoint".into(),
        ));
    }
    // chunk record 列（frontier.seq まで）: media 範囲・source frames・
    // digest を検算する。
    let mut media_end = 0u64;
    let mut source_frames = 0u64;
    let mut digest = 0u64;
    let mut spans = Vec::new();
    let mut media = File::open(media_path).map_err(|e| WriterError::Io {
        op: "open media",
        source: e,
    })?;
    let actual_media_len = media
        .metadata()
        .map_err(|e| WriterError::Io {
            op: "media metadata",
            source: e,
        })?
        .len();
    if actual_media_len < frontier.media_len {
        return Err(WriterError::Corrupt(format!(
            "media shorter than frontier: {actual_media_len} < {}",
            frontier.media_len
        )));
    }
    for rec in &upto {
        let RecordPayload::Chunk(c) = &rec.payload else {
            continue;
        };
        let end = c.media_offset.saturating_add(c.media_len);
        if end > frontier.media_len {
            return Err(WriterError::Corrupt(
                "chunk record beyond frontier media_len".into(),
            ));
        }
        // media byte を再読して CRC32 を検算。
        let mut buf = vec![0u8; usize::try_from(c.media_len).unwrap_or(usize::MAX)];
        {
            use std::io::{Seek, SeekFrom};
            media
                .seek(SeekFrom::Start(c.media_offset))
                .map_err(|e| WriterError::Io {
                    op: "media seek",
                    source: e,
                })?;
            media.read_exact(&mut buf).map_err(|e| WriterError::Io {
                op: "media read",
                source: e,
            })?;
        }
        if crc32(&buf) != c.pcm_crc32 {
            return Err(WriterError::Corrupt(format!(
                "pcm crc mismatch at seq {}",
                rec.seq
            )));
        }
        digest = digest_fold(digest, u64::from(c.pcm_crc32));
        media_end = media_end.max(end);
        source_frames = source_frames.saturating_add(u64::from(c.stored_frames));
        let frame_bytes = u64::from(channels) * 4;
        for s in &c.spans {
            spans.push(FinalizedSpan {
                pass_index: s.pass_index,
                session_start: s.session_start,
                session_end: s.session_start.saturating_add(i64::from(s.frame_count)),
                source_start: s.source_start,
                source_end: s.source_start.saturating_add(i64::from(s.frame_count)),
                project_in: s.project_in,
                project_out: s.project_out,
                media_offset: s.media_offset,
                media_len: u64::from(s.frame_count) * frame_bytes,
            });
        }
    }
    if media_end != frontier.media_len {
        return Err(WriterError::Corrupt(format!(
            "chunk media coverage {media_end} != frontier {}",
            frontier.media_len
        )));
    }
    if source_frames != frontier.source_frames {
        return Err(WriterError::Corrupt(format!(
            "source frames {source_frames} != frontier {}",
            frontier.source_frames
        )));
    }
    if digest != frontier.digest_fold {
        return Err(WriterError::Corrupt(format!(
            "digest {digest:#x} != frontier {:#x}",
            frontier.digest_fold
        )));
    }
    // frontier 以後の tail 情報（report のみ。Finalized へは混入しない）。
    let journaled_end = outcome
        .records
        .iter()
        .filter_map(|r| match &r.payload {
            RecordPayload::Chunk(c) => Some(c.media_offset.saturating_add(c.media_len)),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    Ok(VerifiedStream {
        spans,
        validated_tail_bytes: journaled_end.saturating_sub(frontier.media_len),
        unverified_tail_bytes: actual_media_len.saturating_sub(journaled_end),
    })
}

/// reopen した `area` の各 stream について、Last Valid Checkpoint
/// までの検証を行い `finalized.json` を原子公開する。
/// durable frontier の無い stream・既に finalized 済みの streamは
/// `WriterError` を返す（`NotFound`/`AlreadyFinalized`）。
///
/// `frontier` の無い stream は `Err(WriterError::Corrupt)` として
/// 報告し、file は変更しない。
#[must_use]
pub fn recover_finalize(area: &StagingArea) -> Vec<(u16, Result<FinalizedSourceRef, WriterError>)> {
    area.manifest
        .streams
        .iter()
        .map(|s| (s.stream_index, recover_stream_finalize(area, s)))
        .collect()
}

/// stream 1 本の Last Valid Checkpoint 検証 → finalized 公開。
fn recover_stream_finalize(
    area: &StagingArea,
    s: &crate::staging::ManifestStream,
) -> Result<FinalizedSourceRef, WriterError> {
    let i = s.stream_index;
    let paths = stream_paths(area, i);
    {
        if paths.finalized.exists() {
            return Err(WriterError::AlreadyFinalized);
        }
        let journal_bytes = fs::read(&paths.journal).map_err(|e| WriterError::Io {
            op: "read journal",
            source: e,
        })?;
        let outcome = replay_journal(&journal_bytes);
        // Last Valid Checkpoint: 新しい checkpoint から順に再検証し、
        // 検証を通る最初の frontier を採用する。最新 checkpoint が
        // 破損（media/CRC/digest 不整合）でも、それ以前の整合した
        // checkpoint へフォールバックして validated prefix を救う。
        let mut faults = Vec::new();
        let mut durable = None;
        let mut verified = None;
        for rec in outcome.records.iter().rev() {
            let RecordPayload::Checkpoint(cp) = &rec.payload else {
                continue;
            };
            let cand = DurableFrontier {
                seq: rec.seq,
                checkpoint_index: cp.checkpoint_index,
                media_len: cp.media_len,
                source_frames: cp.source_frames,
                digest_fold: cp.digest_fold,
            };
            match verify_stream(&paths.media, &paths.journal, cand, s.channels) {
                Ok(v) => {
                    durable = Some(cand);
                    verified = Some(v);
                    break;
                }
                Err(e) => {
                    faults.push(format!("checkpoint {} rejected: {e}", cp.checkpoint_index));
                }
            }
        }
        let (durable, verified) = durable
            .zip(verified)
            .ok_or_else(|| WriterError::Corrupt("no valid checkpoint".into()))?;
        // journal に Finalize record があればその outcome、無ければ
        // 中断由来の Partial。非 CleanEnd の tail は fault として
        // ref に記録する。
        let fin = outcome.records.iter().rev().find_map(|r| match &r.payload {
            RecordPayload::Finalize(f) => Some(f.outcome),
            _ => None,
        });
        let outcome_kind = match fin {
            Some(FinalizeOutcome::Complete) => FinalizedOutcome::Complete,
            _ => FinalizedOutcome::Partial,
        };
        if outcome.stop != StopReason::CleanEnd {
            faults.push(format!("journal tail not clean: {:?}", outcome.stop));
        }
        if verified.unverified_tail_bytes > 0 {
            faults.push(format!(
                "unverified media tail preserved: {} bytes",
                verified.unverified_tail_bytes
            ));
        }
        if verified.validated_tail_bytes > 0 {
            faults.push(format!(
                "journaled tail beyond frontier excluded: {} bytes",
                verified.validated_tail_bytes
            ));
        }
        let source_ref = FinalizedSourceRef {
            schema_version: crate::staging::STAGING_SCHEMA_VERSION,
            session_id: area.manifest.session_id.clone(),
            stream_index: i,
            capture_stream_id: s.capture_stream_id.clone(),
            lane_id: s.lane_id.clone(),
            sample_rate: area.manifest.sample_rate,
            pcm_format: crate::staging::PCM_FORMAT.to_string(),
            channels: s.channels,
            media: crate::staging::media_rel(i),
            media_byte_len: durable.media_len,
            source_frames: durable.source_frames,
            outcome: outcome_kind,
            spans: verified.spans,
            digest: format!("{:016x}", durable.digest_fold),
            journal_seq: durable.seq,
            checkpoint_index: durable.checkpoint_index,
            faults,
        };
        let bytes = serde_json::to_vec_pretty(&source_ref)
            .map_err(|e| WriterError::Corrupt(format!("ref encode: {e}")))?;
        publish_atomic(&paths.finalized, &bytes).map_err(|e| WriterError::Io {
            op: "publish finalized",
            source: e,
        })?;
        // Finalize record を追記（既にあれば二重発行しない）。
        // tail が clean でない journal への append は、破損 byte の
        // 後ろに record を残して以後の replay を更に混乱させるため
        // 行わない（finalized.json が authority で十分）。
        if fin.is_none() && outcome.stop == StopReason::CleanEnd {
            append_finalize_record(&paths.journal, &outcome, durable, outcome_kind)?;
        }
        if area.manifest.capabilities.directory_sync == DirSyncCapability::Supported {
            let _ = sync_parent_dir(&paths.journal);
        }
        Ok(source_ref)
    }
}

/// replay 済み journal の末尾へ Finalize record を追記して sync する。
fn append_finalize_record(
    journal_path: &Path,
    outcome: &ReplayOutcome,
    durable: DurableFrontier,
    kind: FinalizedOutcome,
) -> Result<(), WriterError> {
    let mut appender = JournalAppender::resumed(outcome.last_seq.map_or(0, |s| s + 1));
    let rec = FinalizeRecord {
        media_len: durable.media_len,
        source_frames: durable.source_frames,
        digest_fold: durable.digest_fold,
        outcome: match kind {
            FinalizedOutcome::Complete => FinalizeOutcome::Complete,
            FinalizedOutcome::Partial => FinalizeOutcome::Partial,
        },
    };
    let Some(bytes) = appender.encode_next(&RecordPayload::Finalize(rec)) else {
        return Err(WriterError::Corrupt("finalize record encode failed".into()));
    };
    let mut j = OpenOptions::new()
        .append(true)
        .open(journal_path)
        .map_err(|e| WriterError::Io {
            op: "append finalize",
            source: e,
        })?;
    std::io::Write::write_all(&mut j, &bytes).map_err(|e| WriterError::Io {
        op: "append finalize",
        source: e,
    })?;
    File::sync_data(&j).map_err(|e| WriterError::SyncFailed {
        op: "journal.sync_data",
        source: e,
    })
}

#[cfg(test)]
mod tests {
    // 受け入れ試験は tests/vs_c.rs に集約する。
}
