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
    io::{self, Read},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    commit::commit_manifest_path,
    crc32::{IntegrityDigest, crc32},
    faultinj::PublishFaults,
    journal::{
        FinalizeOutcome, FinalizeRecord, JournalAppender, JournalRecord, RecordPayload,
        ReplayOutcome, StopReason, replay_journal,
    },
    staging::{DirSyncCapability, StagingArea, StagingCapabilities, review_rel, sync_parent_dir},
    writer::{
        DurabilityEvidence, DurableFrontier, FinalizedOutcome, FinalizedSourceRef, FinalizedSpan,
        WriterError, stream_paths,
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

/// stream の recovery review 状態（`scan` の分類 + 判定記録）。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReviewState {
    /// `finalized.json` 公開済み（commit 可能・review 不要）。
    Finalized,
    /// 未 finalize・判定記録なし — 明示的な Adopt/Hold/Discard が必要。
    Pending,
    /// review で採用済みと記録されている（finalized が後に失われた
    /// 場合にのみ `scan` へ現れる。通常は `Finalized` が優先）。
    Adopted,
    /// review で保留と記録されている（file はそのまま保持）。
    Held,
    /// review で破棄済みと記録されている。
    Discarded,
    /// 判定記録が破損している（fail-closed: 未判定として扱う）。
    Unreadable,
}

/// review 判定記録の決定値（`stream-NNNN.review.json` の wire 表現）。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewDecision {
    /// verified prefix を finalize して採用。
    Adopted,
    /// 保留（追加証拠・人間判断待ち）。file は変更しない。
    Held,
    /// 参照なし確認の上で media/journal/finalized/canonical を破棄。
    Discarded,
}

/// `review_stream` へ渡す判定要求。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ReviewAction {
    /// verified prefix を finalize して採用（冪等）。
    Adopt,
    /// 保留を記録（file 不変）。
    Hold,
    /// 参照チェック通過時のみ media/journal/finalized/canonical を削除。
    Discard,
}

/// `streams/stream-NNNN.review.json` の wire 型。review 判定を
/// durable に記録し、再 scan/再起動で冪等に再評価できるようにする。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewRecord {
    /// schema version（staging schema に合わせる）。
    pub schema_version: u32,
    /// session 識別子（tagged）。
    pub session_id: String,
    /// stream index。
    pub stream_index: u16,
    /// 判定。
    pub decision: ReviewDecision,
    /// Adopt 時の確定 media fold digest（hex・診断/冪等照合用）。
    /// [`crate::crc32::IntegrityDigest`] 系の integrity 値であり、
    /// content identity ではない。
    #[serde(default)]
    pub finalized_digest: Option<String>,
    /// Adopt 時の確定 media byte 長。
    #[serde(default)]
    pub media_byte_len: Option<u64>,
    /// Adopt 時の確定 source frame 数。
    #[serde(default)]
    pub source_frames: Option<u64>,
    /// Adopt 時の finalize outcome。
    #[serde(default)]
    pub outcome: Option<FinalizedOutcome>,
    /// Hold/Discard の根拠（任意）。
    #[serde(default)]
    pub reason: Option<String>,
}

/// `Discard` 判定時に参照チェックへ渡す候補情報。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscardCandidate {
    /// session 識別子（tagged）。
    pub session_id: String,
    /// stream index。
    pub stream_index: u16,
    /// capture stream 識別子（tagged）。
    pub capture_stream_id: String,
    /// lane 識別子（tagged）。
    pub lane_id: String,
    /// media file の session 相対 path。
    pub media: String,
    /// journal file の session 相対 path。
    pub journal: String,
    /// media file の観測 byte 長（不存在は 0）。
    pub media_byte_len: u64,
}

/// `MediaReferenceProbe` の判定結果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReferenceCheck {
    /// どこからも参照されていない（Discard 可）。
    Clear,
    /// 参照が存在する（詳細付き・Discard 拒否）。
    Referenced(String),
    /// 参照状態を判定できない（fail-closed・Discard 拒否）。
    Unknown(String),
}

/// staging media が project/他 session から参照されていないかを
/// 答える port。synvelis-project/library 接続の adapter が実装する
/// 境界で、本スライスでは trait 定義と fail-closed 判定のみ。
/// `Clear` 以外（`Referenced`/`Unknown`）は全て Discard 拒否。
pub trait MediaReferenceProbe {
    /// `candidate` の参照状態を返す。
    fn check(&self, candidate: &DiscardCandidate) -> ReferenceCheck;
}

/// `review_stream` の結果。
#[derive(Clone, Debug, PartialEq)]
pub enum ReviewOutcome {
    /// verified prefix を finalize して採用（`FinalizedSourceRef`）。
    Adopted(Box<FinalizedSourceRef>),
    /// 保留を記録した。
    Held,
    /// 参照なし確認の上で破棄した。
    Discarded,
}

/// `review_stream` の失敗分類。
#[derive(Debug)]
pub enum ReviewError {
    /// staging 層エラー。
    Staging(crate::staging::StagingError),
    /// file I/O 失敗。
    Io {
        /// 失敗した操作。
        op: &'static str,
        /// 発生元。
        source: io::Error,
    },
    /// Adopt 内部の finalize 検証/公開の失敗。
    Writer(WriterError),
    /// 判定が fail-closed で拒否された（Discard の参照未確認等）。
    Refused(String),
    /// 指定 stream が manifest に存在しない。
    NotFound(String),
    /// 確定済み ref/判定記録の decode 失敗（fail-closed）。
    Corrupt(String),
}

impl std::fmt::Display for ReviewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Staging(e) => write!(f, "{e}"),
            Self::Io { op, source } => write!(f, "review io error ({op}): {source}"),
            Self::Writer(e) => write!(f, "review finalize failed: {e}"),
            Self::Refused(m) => write!(f, "review refused: {m}"),
            Self::NotFound(m) => write!(f, "review target not found: {m}"),
            Self::Corrupt(m) => write!(f, "review corrupt data: {m}"),
        }
    }
}

impl std::error::Error for ReviewError {}

impl From<crate::staging::StagingError> for ReviewError {
    fn from(e: crate::staging::StagingError) -> Self {
        Self::Staging(e)
    }
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
    /// manifest が記録した FS capability（作成時観測値）。
    pub fs_capability: StagingCapabilities,
    /// recovery review 状態（finalized/判定記録/未判定）。
    pub review_state: ReviewState,
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
                    digest_fold: IntegrityDigest(cp.digest_fold),
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

/// `streams/stream-NNNN.review.json` を読む。
/// `None` = 不在。`Some(Err)` = 破損または session/stream 帰属の
/// 不一致（fail-closed で `Unreadable` に分類する）。
fn read_review(
    dir: &Path,
    session_id: &str,
    stream_index: u16,
) -> Option<Result<ReviewRecord, ()>> {
    let path = dir.join(review_rel(stream_index));
    if !path.exists() {
        return None;
    }
    fs::read(&path).ok().map(|b| {
        serde_json::from_slice::<ReviewRecord>(&b)
            .map_err(|_| ())
            .and_then(|rec| {
                if rec.session_id == session_id && rec.stream_index == stream_index {
                    Ok(rec)
                } else {
                    Err(())
                }
            })
    })
}

/// `dir`（session dir）内の `stream_index` の review 状態を分類する。
/// `finalized.json` が authority: 存在すれば review 記録より優先して
/// `Finalized` とする。
fn classify_review(
    dir: &Path,
    session_id: &str,
    stream_index: u16,
    finalized_rel: &str,
) -> ReviewState {
    if dir.join(finalized_rel).exists() {
        return ReviewState::Finalized;
    }
    match read_review(dir, session_id, stream_index) {
        Some(Ok(rec)) => match rec.decision {
            ReviewDecision::Adopted => ReviewState::Adopted,
            ReviewDecision::Held => ReviewState::Held,
            ReviewDecision::Discarded => ReviewState::Discarded,
        },
        Some(Err(())) => ReviewState::Unreadable,
        None => ReviewState::Pending,
    }
}

/// staging root 配下の session dir を列挙して復旧報告を作る。
/// manifest の無い dir は session 領域とみなさずスキップする。
/// 列挙は dir 名 sort の決定的順で、各 stream の処理量は file 実長に
/// 比例する（無制限の探索・再帰は行わない）。
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
            let review_state =
                classify_review(&dir, &manifest.session_id, s.stream_index, &s.finalized);
            streams.push(StreamRecovery {
                stream_index: s.stream_index,
                class: scan.class,
                durable: scan.durable,
                verified_spans: scan.verified_spans,
                validated_tail_bytes: scan.validated_tail_bytes,
                unverified_tail_bytes: scan.unverified_tail_bytes,
                stop: scan.stop,
                faults: scan.faults,
                fs_capability: manifest.capabilities.clone(),
                review_state,
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
    let mut digest = IntegrityDigest::ZERO;
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
        digest = digest.fold(u64::from(c.pcm_crc32));
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
            "digest {} != frontier {}",
            digest.to_hex(),
            frontier.digest_fold.to_hex()
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
    recover_stream_finalize_faultable(area, s, &mut PublishFaults::new())
}

/// [`recover_stream_finalize`] の publish fault 注入対応版。
/// `finalized.json` の原子公開が `publish_atomic_faultable` を通る
/// ため、review Adopt 経路の indeterminate publish を検証できる。
#[allow(clippy::too_many_lines)] // 直列の検証→公開→record 追記
fn recover_stream_finalize_faultable(
    area: &StagingArea,
    s: &crate::staging::ManifestStream,
    publish_faults: &mut PublishFaults,
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
                digest_fold: IntegrityDigest(cp.digest_fold),
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
        // journal に Finalize record があっても outcome が Complete を
        // 名乗るだけでは採用しない。Complete は「record が Complete を
        // 主張」「journal stop が CleanEnd」「validated/unverified
        // tail がともに 0」を全て満たす場合に限る — それ以外は
        // unverified tail を Complete として昇格させないため Partial
        // へ降格する。record が無い/Partial も同じく Partial。
        let fin = outcome.records.iter().rev().find_map(|r| match &r.payload {
            RecordPayload::Finalize(f) => Some(f.outcome),
            _ => None,
        });
        let clean = outcome.stop == StopReason::CleanEnd;
        let no_tail = verified.validated_tail_bytes == 0 && verified.unverified_tail_bytes == 0;
        let outcome_kind = match fin {
            Some(FinalizeOutcome::Complete) if clean && no_tail => FinalizedOutcome::Complete,
            _ => FinalizedOutcome::Partial,
        };
        if !clean {
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
            digest: durable.digest_fold.to_hex(),
            journal_seq: durable.seq,
            checkpoint_index: durable.checkpoint_index,
            faults,
            durability_evidence: vec![DurabilityEvidence::recovered(
                &area.manifest.capabilities,
                durable,
            )],
        };
        let bytes = serde_json::to_vec_pretty(&source_ref)
            .map_err(|e| WriterError::Corrupt(format!("ref encode: {e}")))?;
        crate::staging::publish_atomic_faultable(&paths.finalized, &bytes, publish_faults)
            .map_err(|e| WriterError::Io {
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
        digest_fold: u64::from(durable.digest_fold),
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

/// review 判定記録を原子公開する。
fn publish_review(
    area: &StagingArea,
    stream_index: u16,
    rec: &ReviewRecord,
    faults: &mut PublishFaults,
) -> Result<(), ReviewError> {
    let bytes = serde_json::to_vec_pretty(rec)
        .map_err(|e| ReviewError::Corrupt(format!("review encode: {e}")))?;
    crate::staging::publish_atomic_faultable(&area.review_path(stream_index), &bytes, faults)
        .map_err(|source| ReviewError::Io {
            op: "publish review record",
            source,
        })
}

/// 既存の `finalized.json` を decode して返す（review Adopt の冪等
/// 経路・破損時は `Corrupt` で fail-closed）。
fn read_finalized(
    area: &StagingArea,
    stream_index: u16,
) -> Result<FinalizedSourceRef, ReviewError> {
    let path = stream_paths(area, stream_index).finalized;
    let bytes = fs::read(&path).map_err(|source| ReviewError::Io {
        op: "read finalized",
        source,
    })?;
    serde_json::from_slice(&bytes)
        .map_err(|e| ReviewError::Corrupt(format!("finalized decode: {e}")))
}

/// review 判定記録の雛形（decision ごとの差分は caller が埋める）。
fn review_record(session_id: &str, stream_index: u16, decision: ReviewDecision) -> ReviewRecord {
    ReviewRecord {
        schema_version: crate::staging::STAGING_SCHEMA_VERSION,
        session_id: session_id.to_string(),
        stream_index,
        decision,
        finalized_digest: None,
        media_byte_len: None,
        source_frames: None,
        outcome: None,
        reason: None,
    }
}

/// Adopted 判定記録を finalized ref の値で埋める。
fn adopted_record(session_id: &str, stream_index: u16, r: &FinalizedSourceRef) -> ReviewRecord {
    let mut rec = review_record(session_id, stream_index, ReviewDecision::Adopted);
    rec.finalized_digest = Some(r.digest.clone());
    rec.media_byte_len = Some(r.media_byte_len);
    rec.source_frames = Some(r.source_frames);
    rec.outcome = Some(r.outcome);
    rec
}

/// `Adopt`: verified prefix を finalize して採用（冪等）。
fn review_adopt(
    area: &StagingArea,
    s: &crate::staging::ManifestStream,
    stream_index: u16,
    publish_faults: &mut PublishFaults,
) -> Result<ReviewOutcome, ReviewError> {
    let r =
        recover_stream_finalize_faultable(area, s, publish_faults).map_err(ReviewError::Writer)?;
    let rec = adopted_record(&area.manifest.session_id, stream_index, &r);
    publish_review(area, stream_index, &rec, publish_faults)?;
    Ok(ReviewOutcome::Adopted(Box::new(r)))
}

/// `Hold`: 判定のみ記録し file を変更しない。
fn review_hold(
    area: &StagingArea,
    stream_index: u16,
    publish_faults: &mut PublishFaults,
) -> Result<ReviewOutcome, ReviewError> {
    let rec = review_record(
        &area.manifest.session_id,
        stream_index,
        ReviewDecision::Held,
    );
    publish_review(area, stream_index, &rec, publish_faults)?;
    Ok(ReviewOutcome::Held)
}

/// `Discard`: 参照チェック `Clear` のみ file を削除して記録する
/// （fail-closed・冪等）。
fn review_discard(
    area: &StagingArea,
    s: &crate::staging::ManifestStream,
    stream_index: u16,
    state: ReviewState,
    probe: Option<&dyn MediaReferenceProbe>,
    publish_faults: &mut PublishFaults,
) -> Result<ReviewOutcome, ReviewError> {
    // Discarded 済みなら冪等に成功を返す（probe 不要）。
    if state == ReviewState::Discarded {
        return Ok(ReviewOutcome::Discarded);
    }
    // commit 済み session の media は project 側から参照され得る
    // ため、staging 内 commit manifest があれば拒否。
    if commit_manifest_path(area).exists() {
        return Err(ReviewError::Refused(format!(
            "session {} has a commit manifest",
            area.manifest.session_id
        )));
    }
    let probe = probe
        .ok_or_else(|| ReviewError::Refused("discard requires a MediaReferenceProbe".into()))?;
    let paths = stream_paths(area, stream_index);
    let candidate = DiscardCandidate {
        session_id: area.manifest.session_id.clone(),
        stream_index,
        capture_stream_id: s.capture_stream_id.clone(),
        lane_id: s.lane_id.clone(),
        media: s.media.clone(),
        journal: s.journal.clone(),
        media_byte_len: fs::metadata(&paths.media).map_or(0, |m| m.len()),
    };
    match probe.check(&candidate) {
        ReferenceCheck::Clear => {}
        ReferenceCheck::Referenced(why) => {
            return Err(ReviewError::Refused(format!("media referenced: {why}")));
        }
        ReferenceCheck::Unknown(why) => {
            return Err(ReviewError::Refused(format!(
                "reference state unknown: {why}"
            )));
        }
    }
    // 参照なし確認済み: file を削除してから判定を記録する。
    // 削除は冪等（NotFound は無視）。finalized/canonical はこの
    // 時点で存在しないはずだが、念のため対象に含める。
    for p in [
        paths.media,
        paths.journal,
        paths.finalized,
        area.canonical_path(stream_index),
    ] {
        match fs::remove_file(&p) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(ReviewError::Io {
                    op: "discard stream file",
                    source: e,
                });
            }
        }
    }
    let rec = review_record(
        &area.manifest.session_id,
        stream_index,
        ReviewDecision::Discarded,
    );
    publish_review(area, stream_index, &rec, publish_faults)?;
    Ok(ReviewOutcome::Discarded)
}

/// stream の review 判定を明示的に記録・実行する。
///
/// - `Adopt`: Last Valid Checkpoint までを検証して `finalized.json`
///   を公開し `Adopted` を記録する。既に finalized 済みなら既存 ref
///   を返す（冪等・重複発行しない）。unverified tail は Complete に
///   ならない。
/// - `Hold`: `Held` を記録するだけで file を変更しない。
/// - `Discard`: `MediaReferenceProbe` が `Clear` を返した場合のみ
///   media/journal/finalized/canonical を削除し `Discarded` を記録
///   する。`Referenced`/`Unknown`/probe 未指定・commit 済み manifest
///   存在は全て拒否（fail-closed）。削除は冪等。
///
/// `scan()` の `ReviewState::Pending`（および `Unreadable` 以外の
/// 未確定状態）に対して呼ぶ。`finalized.json` が存在する stream の
/// `Hold`/`Discard` は拒否する。
///
/// # Errors
/// stream 不在・fail-closed 拒否・I/O/検証失敗。
pub fn review_stream(
    area: &StagingArea,
    stream_index: u16,
    action: ReviewAction,
    probe: Option<&dyn MediaReferenceProbe>,
    publish_faults: &mut PublishFaults,
) -> Result<ReviewOutcome, ReviewError> {
    let s = area
        .manifest
        .streams
        .iter()
        .find(|s| s.stream_index == stream_index)
        .ok_or_else(|| ReviewError::NotFound(format!("stream-{stream_index:04}")))?;
    let session_id = &area.manifest.session_id;

    // 既存 review 記録の分類（finalized.json 優先は classify_review
    // と同じ規則）。
    let state = classify_review(&area.session_dir, session_id, stream_index, &s.finalized);

    // 既に finalized 済み: Adopt は既存 ref を冪等に返し、
    // Hold/Discard は確定済みデータへの操作として拒否する。
    if stream_paths(area, stream_index).finalized.exists() {
        return match action {
            ReviewAction::Adopt => {
                let r = read_finalized(area, stream_index)?;
                // review 記録が無い/失われている場合に備えて冪等に
                // 再公開する（同一 bytes のため no-op 相当）。
                let rec = adopted_record(session_id, stream_index, &r);
                publish_review(area, stream_index, &rec, publish_faults)?;
                Ok(ReviewOutcome::Adopted(Box::new(r)))
            }
            _ => Err(ReviewError::Refused(format!(
                "stream-{stream_index:04} is already finalized"
            ))),
        };
    }

    // 破損した判定記録は fail-closed（上書きで証拠を失わせない）。
    if state == ReviewState::Unreadable {
        return Err(ReviewError::Corrupt(format!(
            "stream-{stream_index:04} review record unreadable"
        )));
    }

    match action {
        ReviewAction::Adopt => review_adopt(area, s, stream_index, publish_faults),
        ReviewAction::Hold => review_hold(area, stream_index, publish_faults),
        ReviewAction::Discard => {
            review_discard(area, s, stream_index, state, probe, publish_faults)
        }
    }
}

#[cfg(test)]
mod tests {
    // 受け入れ試験は tests/vs_c.rs に集約する。
}
