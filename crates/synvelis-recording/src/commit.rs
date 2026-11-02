//! VS-D Recording Commit: finalized staging media → 単一冪等 Project
//! mutation への変換層。
//!
//! `FinalizedSourceRef`（VS-C の Project 非依存出力）を読み、Stable ID を
//! commit 前に `commit-manifest.json` へ確保・永続化し、
//! `RecordingOp::CommitRecording` が単一 mutation として検証・登録する。
//! `applied_commits` registry が `AlreadyApplied` / `IntegrityConflict` を
//! 判定し、`commit.json` marker が journal 側の commit 記録を担う。
//! PCM byte は manifest/command に乗らない（Project は参照と digest の
//! みを保持）。`FinalizedOutcome::Partial`・未 finalize・faulted stream
//! は commit 対象外として `recoverable` に分離し、Project へ自動昇格
//! しない。

use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};
use synvelis_project::{
    AppliedRecordingCommit, Asset, AssetId, CaptureSegment, CaptureSegmentId, CaptureStreamId,
    Clip, ClipId, ClockEpochId, Command, Error, IdGenerator, LaneId, MusicalRange, MusicalTick,
    Project, ProjectController, RecordingPass, RecordingPassId, RecordingSessionId, SessionFrame,
    SessionRange, SessionState, Source, SourceFrame, SourceId, SourceRange, SourceState, Take,
    TakeId, TakeState, TimeAnchor,
};

use crate::{
    crc32::crc32,
    ops::{RecordingOp, SynvelisOp},
    staging::{DirSyncCapability, StagingArea, StagingError, publish_atomic},
    writer::{FinalizedOutcome, FinalizedSourceRef, FinalizedSpan},
};

/// commit-manifest.json の schema version。
pub const COMMIT_SCHEMA_VERSION: u32 = 1;
/// staging session dir 内の commit manifest 名。
pub const COMMIT_MANIFEST_FILE: &str = "commit-manifest.json";
/// staging session dir 内の commit marker 名。
pub const COMMIT_MARKER_FILE: &str = "commit.json";
/// `StaleRevision` への recoverable retry 上限。
const MAX_REVISION_RETRIES: usize = 8;
/// PCM f32 の 1 frame あたり byte 数（per channel）。
const BYTES_PER_SAMPLE: u64 = 4;

// ---------- wire types (manifest file / command payload) ----------

/// Commit の冪等 identity。`session_id` が registry key、`content_hash` が
/// 内容一致/異内容の判定材料（ADR I-1）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitIdentity {
    /// `RecordingSessionId` の tagged 文字列。
    pub session_id: String,
    /// `payload` の canonical hash（hex16）。
    pub content_hash: String,
}

/// payload 内の pass 予約。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PassEntry {
    /// `RecordingPassId` tagged。
    pub id: String,
    /// pass ordinal（session 内一意）。
    pub index: u32,
}

/// payload 内の segment（merge 済み論理区間）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentEntry {
    /// `CaptureSegmentId` tagged。
    pub id: String,
    /// `RecordingPassId` tagged。
    pub pass_id: String,
    /// pass ordinal（fingerprint/検査用）。
    pub pass_index: u32,
    /// session 区間 `[start, end)`（frame）。
    pub session_start: i64,
    /// session 区間終端（exclusive）。
    pub session_end: i64,
    /// source 区間 `[start, end)`（frame）。
    pub source_start: i64,
    /// source 区間終端（exclusive）。
    pub source_end: i64,
    /// 区間先頭の project tick。
    pub project_in: i64,
    /// 区間終端（exclusive）の project tick。
    pub project_out: i64,
    /// media byte offset。
    pub media_offset: u64,
    /// media byte 長。
    pub media_len: u64,
}

/// payload 内の take（stream × pass の利用者向け結果）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TakeEntry {
    /// `TakeId` tagged。
    pub id: String,
    /// `RecordingPassId` tagged。
    pub pass_id: String,
    /// pass ordinal。
    pub pass_index: u32,
    /// 構成 segment の `CaptureSegmentId` tagged 列（session 順・非空）。
    pub segment_ids: Vec<String>,
}

/// payload 内の clip（空 lane への自動配置・segment 単位）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipEntry {
    /// `ClipId` tagged。
    pub id: String,
    /// 所有 take の `TakeId` tagged（traceability; project Clip は
    /// segment/source 経由で結び付く）。
    pub take_id: String,
    /// 対象 segment の `CaptureSegmentId` tagged。
    pub segment_id: String,
    /// timeline 開始 tick（`TimeAnchor::Musical`）。
    pub tick_in: i64,
    /// timeline 終端 tick（exclusive）。
    pub tick_out: i64,
    /// source 読み出し offset（frame）。
    pub source_offset: i64,
}

/// payload 内の stream（1 `FinalizedSourceRef` → 1 asset+source）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamEntry {
    /// `plan.targets` の添字。
    pub stream_index: u16,
    /// `CaptureStreamId` tagged（既登録 entity を参照・新規作成しない）。
    pub capture_stream_id: String,
    /// `LaneId` tagged。
    pub lane_id: String,
    /// `ClockEpochId` tagged（project の stream entity から確定）。
    pub clock_epoch_id: String,
    /// 新規 `AssetId` tagged（commit 前に確保）。
    pub asset_id: String,
    /// 新規 `SourceId` tagged（commit 前に確保）。
    pub source_id: String,
    /// PCM 形式識別子。
    pub pcm_format: String,
    /// media 内容 digest（hex・asset/source の `content_digest` と同一）。
    pub digest: String,
    /// media file の session 相対 path。
    pub media: String,
    /// durable media byte 長。
    pub media_byte_len: u64,
    /// durable source frame 総数。
    pub source_frames: u64,
    /// sample rate。
    pub sample_rate: u32,
    /// チャネル数。
    pub channels: u32,
    /// merge 済み segment 列。
    pub segments: Vec<SegmentEntry>,
    /// take 列（stream × pass）。
    pub takes: Vec<TakeEntry>,
    /// clip 列（初回空 lane のみ・segment 単位）。
    pub clips: Vec<ClipEntry>,
}

/// commit 本体（identity/新規 ID/参照/範囲を commit 前に確定）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitPayload {
    /// `RecordingSessionId` tagged。
    pub session_id: String,
    /// 新規 pass 予約（index 昇順）。
    pub passes: Vec<PassEntry>,
    /// commit 対象 stream（Complete のみ・`stream_index` 昇順）。
    pub streams: Vec<StreamEntry>,
}

/// `commit-manifest.json`（staging 原子公開・commit 前 ID 確保の正本）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordingCommitManifest {
    /// schema version。
    pub schema_version: u32,
    /// 冪等 identity。
    pub identity: CommitIdentity,
    /// commit 本体。
    pub payload: CommitPayload,
}

/// journal 側の commit 記録（`commit.json`）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitMarker {
    /// schema version。
    pub schema_version: u32,
    /// 記録対象の identity。
    pub identity: CommitIdentity,
    /// `"applied"` / `"already-applied"` / `"nothing-committed"`。
    pub outcome: String,
    /// 適用後の live revision（未適用時は現在 revision）。
    pub revision: u64,
    /// commit した各 stream の media digest（forensics 用）。
    #[serde(default)]
    pub stream_digests: Vec<String>,
}

// ---------- outcomes / errors ----------

/// `applied_commits` registry による commit 状態分類。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CommitStatus {
    /// 未適用。
    NotApplied,
    /// 同一 identity + 同一 content が適用済み（再適用しない）。
    AlreadyApplied,
    /// 同一 identity + 異内容（integrity 違反・報告して終了）。
    IntegrityConflict,
}

/// `commit_session` の結果分類。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitOutcome {
    /// 新規適用（新 live revision）。
    Applied {
        /// 適用後 revision。
        revision: u64,
    },
    /// registry が同一内容を検出（mutation なし・revision 不変）。
    AlreadyApplied,
    /// commit 対象 stream が無く mutation を発行しなかった。
    NothingCommitted,
    /// identity 一致・内容不一致で拒否（mutation なし）。
    IntegrityConflict,
    /// 適用失敗（部分適用なし）。
    Failed {
        /// 失敗理由（Error の display）。
        reason: String,
    },
}

/// commit 対象外の media（回復可能・Project 未登録のまま報告）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoverableStream {
    /// stream index。
    pub stream_index: u16,
    /// `CaptureStreamId` tagged。
    pub capture_stream_id: String,
    /// `LaneId` tagged。
    pub lane_id: String,
    /// 分類理由（`partial-finalize`/`missing-finalized`/
    /// `finalized-unreadable`/`empty-finalize`）。
    pub reason: String,
    /// media file の session 相対 path。
    pub media: String,
}

/// `commit_session` の最終報告（committed と recoverable を分離）。
#[derive(Clone, Debug)]
pub struct CommitReport {
    /// session。
    pub session_id: RecordingSessionId,
    /// manifest の content hash。
    pub content_hash: String,
    /// 結果分類。
    pub outcome: CommitOutcome,
    /// 登録 entity 数（Applied/AlreadyApplied 時に意味を持つ）。
    pub sources_committed: usize,
    /// 登録 take 数。
    pub takes_committed: usize,
    /// 登録 clip 数。
    pub clips_committed: usize,
    /// commit 対象外 media。
    pub recoverable: Vec<RecoverableStream>,
}

/// commit driver の失敗分類。
#[derive(Debug)]
pub enum CommitError {
    /// file I/O 失敗。
    Io {
        /// 失敗した操作。
        op: &'static str,
        /// 発生元。
        source: io::Error,
    },
    /// staging 層エラー。
    Staging(StagingError),
    /// project 層エラー（validate/apply/stale revision）。
    Project(Error),
    /// manifest/refs の構造不整合。
    Manifest(String),
}

impl std::fmt::Display for CommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { op, source } => write!(f, "commit io error ({op}): {source}"),
            Self::Staging(e) => write!(f, "{e}"),
            Self::Project(e) => write!(f, "project error: {e}"),
            Self::Manifest(m) => write!(f, "invalid commit manifest: {m}"),
        }
    }
}

impl std::error::Error for CommitError {}

impl From<StagingError> for CommitError {
    fn from(e: StagingError) -> Self {
        Self::Staging(e)
    }
}

impl From<Error> for CommitError {
    fn from(e: Error) -> Self {
        Self::Project(e)
    }
}

fn io_err(op: &'static str) -> impl FnOnce(io::Error) -> CommitError {
    move |source| CommitError::Io { op, source }
}

// ---------- paths / file IO ----------

/// `commit-manifest.json` の絶対 path。
#[must_use]
pub fn commit_manifest_path(area: &StagingArea) -> PathBuf {
    area.session_dir.join(COMMIT_MANIFEST_FILE)
}

/// `commit.json` の絶対 path。
#[must_use]
pub fn commit_marker_path(area: &StagingArea) -> PathBuf {
    area.session_dir.join(COMMIT_MARKER_FILE)
}

/// 既存 manifest の読み込み（無ければ `Ok(None)`）。
///
/// # Errors
/// [`CommitError::Io`]/[`CommitError::Manifest`]。
pub fn read_commit_manifest(
    area: &StagingArea,
) -> Result<Option<RecordingCommitManifest>, CommitError> {
    let path = commit_manifest_path(area);
    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_err("read commit manifest")(e)),
    };
    let manifest: RecordingCommitManifest = serde_json::from_slice(&bytes)
        .map_err(|e| CommitError::Manifest(format!("commit manifest decode: {e}")))?;
    Ok(Some(manifest))
}

/// marker の読み込み（無ければ `Ok(None)`）。
#[must_use]
pub fn read_commit_marker(area: &StagingArea) -> Option<CommitMarker> {
    let path = commit_marker_path(area);
    if !path.exists() {
        return None;
    }
    fs::read(&path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
}

fn write_commit_marker(
    area: &StagingArea,
    manifest: &RecordingCommitManifest,
    outcome: &str,
    revision: u64,
) -> Result<(), CommitError> {
    let marker = CommitMarker {
        schema_version: COMMIT_SCHEMA_VERSION,
        identity: manifest.identity.clone(),
        outcome: outcome.to_string(),
        revision,
        stream_digests: manifest
            .payload
            .streams
            .iter()
            .map(|s| s.digest.clone())
            .collect(),
    };
    let bytes = serde_json::to_vec_pretty(&marker)
        .map_err(|e| CommitError::Manifest(format!("marker encode: {e}")))?;
    publish_atomic(&commit_marker_path(area), &bytes).map_err(io_err("publish commit marker"))?;
    // rename を crash で失わないよう parent dir も sync する
    // （manifest/marker が消えると reserved ID が失われ、
    // 再構築 hash が registry と衝突して IntegrityConflict に沈む）。
    if area.manifest.capabilities.directory_sync == DirSyncCapability::Supported {
        area.sync_dir().map_err(io_err("sync commit dir"))?;
    }
    Ok(())
}

// ---------- finalized ref loading / classification ----------

/// manifest 全 stream の `finalized.json` 読み取り結果。
#[derive(Clone, Debug)]
pub struct StreamFinalization {
    /// stream index。
    pub stream_index: u16,
    /// 解析済み ref（missing/破損は `None`）。
    pub finalized: Option<FinalizedSourceRef>,
    /// 読み取り/解析失敗の理由。
    pub error: Option<String>,
}

/// staging manifest 上の全 stream の finalized ref を読む。
/// read/parse 失敗は error に記録し panic しない。
#[must_use]
pub fn load_finalized(area: &StagingArea) -> Vec<StreamFinalization> {
    area.manifest
        .streams
        .iter()
        .map(|ms| {
            let path = area.finalized_path(ms.stream_index);
            if !path.exists() {
                return StreamFinalization {
                    stream_index: ms.stream_index,
                    finalized: None,
                    error: None,
                };
            }
            match fs::read(&path) {
                Ok(bytes) => match serde_json::from_slice::<FinalizedSourceRef>(&bytes) {
                    Ok(fin) => StreamFinalization {
                        stream_index: ms.stream_index,
                        finalized: Some(fin),
                        error: None,
                    },
                    Err(e) => StreamFinalization {
                        stream_index: ms.stream_index,
                        finalized: None,
                        error: Some(format!("decode: {e}")),
                    },
                },
                Err(e) => StreamFinalization {
                    stream_index: ms.stream_index,
                    finalized: None,
                    error: Some(format!("read: {e}")),
                },
            }
        })
        .collect()
}

// ---------- payload construction ----------

/// payload の内容 fingerprint（ID 文字列を除いた構造一致判定）。
/// 既存 manifest 再利用時に refs/project 由来の再構成と照合する。
fn fingerprint(payload: &CommitPayload) -> Vec<String> {
    let mut lines = vec![format!("session:{}", payload.session_id)];
    let mut passes: Vec<u32> = payload.passes.iter().map(|p| p.index).collect();
    passes.sort_unstable();
    for i in &passes {
        lines.push(format!("pass:{i}"));
    }
    for s in &payload.streams {
        lines.push(format!(
            "stream:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            s.stream_index,
            s.capture_stream_id,
            s.lane_id,
            s.clock_epoch_id,
            s.pcm_format,
            s.digest,
            s.media,
            s.media_byte_len,
            s.source_frames,
            s.sample_rate,
            s.channels
        ));
        for g in &s.segments {
            lines.push(format!(
                "seg:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                g.pass_index,
                g.session_start,
                g.session_end,
                g.source_start,
                g.source_end,
                g.project_in,
                g.project_out,
                g.media_offset,
                g.media_len
            ));
        }
        for t in &s.takes {
            lines.push(format!("take:{}:{}", t.pass_index, t.segment_ids.len()));
        }
        // clip の有無は commit 時点の lane 状態由来（refs 非依存）のため
        // fingerprint 対象外。clip↔segment の値整合は
        // `check_clip_consistency` が別途検査する。
    }
    lines
}

/// manifest 内の clip が参照 segment の配置・offset と整合するか。
/// （clip 生成自体は lane 状態由来だが、値は segment 由来で照合可能）
fn check_clip_consistency(payload: &CommitPayload) -> Result<(), CommitError> {
    for s in &payload.streams {
        for c in &s.clips {
            let seg = s
                .segments
                .iter()
                .find(|g| g.id == c.segment_id)
                .ok_or_else(|| CommitError::Manifest("clip references unknown segment".into()))?;
            if c.tick_in != seg.project_in
                || c.tick_out != seg.project_out
                || c.source_offset != seg.source_start
            {
                return Err(CommitError::Manifest(
                    "clip placement diverges from segment".into(),
                ));
            }
        }
    }
    Ok(())
}

/// payload の canonical hash（`identity.content_hash`）。
///
/// # Errors
/// [`CommitError::Manifest`]: encode 失敗時。
pub fn payload_hash(payload: &CommitPayload) -> Result<String, CommitError> {
    let bytes = serde_json::to_vec(payload)
        .map_err(|e| CommitError::Manifest(format!("payload encode: {e}")))?;
    let fwd = u64::from(crc32(&bytes));
    let mut rev = bytes.clone();
    rev.reverse();
    let bwd = u64::from(crc32(&rev));
    Ok(format!("{fwd:08x}{bwd:08x}"))
}

/// contiguous span を 1 論理 segment へ merge する。
/// 同一 pass・session/source/project/media の 4 系統全てが連続する
/// 場合のみ結合し、偽の連続区間を作らない。
fn merge_spans(spans: &[FinalizedSpan]) -> Vec<FinalizedSpan> {
    let mut merged: Vec<FinalizedSpan> = Vec::new();
    for span in spans {
        // 壊れた ref の巨大値で wrap-around 連続を偽造しないよう、
        // 連続判定・加算とも checked で行う（overflow → 非連続扱いで
        // merge せず、後段の media_len 整合検査が検出する）。
        let extend = merged.last_mut().is_some_and(|prev| {
            prev.pass_index == span.pass_index
                && prev.session_end == span.session_start
                && prev.source_end == span.source_start
                && prev.project_out == span.project_in
                && prev
                    .media_offset
                    .checked_add(prev.media_len)
                    .is_some_and(|end| end == span.media_offset)
                && prev.media_len.checked_add(span.media_len).is_some()
        });
        if extend {
            if let Some(prev) = merged.last_mut() {
                prev.session_end = span.session_end;
                prev.source_end = span.source_end;
                prev.project_out = span.project_out;
                prev.media_len = prev.media_len.saturating_add(span.media_len);
            }
        } else {
            merged.push(span.clone());
        }
    }
    merged
}

/// payload が要求する新規 ID 数（pass + segment + take + clip +
/// asset + source）。
fn count_new_ids(payload: &CommitPayload) -> usize {
    payload.passes.len()
        + payload
            .streams
            .iter()
            .map(|s| 2 + s.segments.len() + s.takes.len() + s.clips.len())
            .sum::<usize>()
}

/// `FinalizedSourceRef` 群 + staging manifest + project 状態から
/// commit payload を構成する。`draw` は新規 Stable ID の発行器
/// （通常 `ProjectController::draw_raw_id`）。Complete な ref のみ
/// commit 対象とし、他は `recoverable` に分離する。
///
/// # Errors
/// [`CommitError::Manifest`]: session/stream の参照不整合。
/// # Panics
/// take が存在しない segment id を指す内部不整合時（merge 後は起こらない）。
#[allow(clippy::too_many_lines)] // stream 単位の直列構成
pub fn build_payload(
    project: &Project,
    area: &StagingArea,
    session_id: RecordingSessionId,
    draw: &mut dyn FnMut() -> u128,
) -> Result<(CommitPayload, Vec<RecoverableStream>), CommitError> {
    let fins = load_finalized(area);
    // session スコープの capture fault marker（RT/engine 側 fault の
    // durable 記録）。存在すれば全 stream を recoverable 分離する
    // — capture が session として正常終了していないため「Complete
    // media」とは主張できない。marker が読めない場合も fail-closed。
    let capture_fault = if area.capture_fault_marker_exists() {
        Some(
            area.capture_fault()
                .unwrap_or_else(|| "unreadable capture-fault marker".to_string()),
        )
    } else {
        None
    };
    let mut recoverable = Vec::new();
    let mut pass_indexes: BTreeSet<u32> = BTreeSet::new();
    let mut stream_specs: Vec<(u16, FinalizedSourceRef, Vec<FinalizedSpan>)> = Vec::new();

    for ms in &area.manifest.streams {
        if let Some(detail) = &capture_fault {
            recoverable.push(RecoverableStream {
                stream_index: ms.stream_index,
                capture_stream_id: ms.capture_stream_id.clone(),
                lane_id: ms.lane_id.clone(),
                reason: format!("capture-fault:{detail}"),
                media: ms.media.clone(),
            });
            continue;
        }
        let fin = fins.iter().find(|f| f.stream_index == ms.stream_index);
        let entry = match fin {
            Some(StreamFinalization {
                finalized: Some(r), ..
            }) if r.outcome == FinalizedOutcome::Complete => {
                // ref の session/stream 帰属・format メタを staging
                // manifest と照合する。stale/流用 ref は commit しない。
                if r.capture_stream_id != ms.capture_stream_id
                    || r.lane_id != ms.lane_id
                    || r.session_id != area.manifest.session_id
                    || r.stream_index != ms.stream_index
                    || r.channels != ms.channels
                    || r.sample_rate != area.manifest.sample_rate
                    || r.media != ms.media
                    || r.schema_version != crate::staging::STAGING_SCHEMA_VERSION
                {
                    return Err(CommitError::Manifest(format!(
                        "finalized ref stream {} identity mismatch",
                        ms.stream_index
                    )));
                }
                if r.spans.is_empty() {
                    recoverable.push(RecoverableStream {
                        stream_index: ms.stream_index,
                        capture_stream_id: ms.capture_stream_id.clone(),
                        lane_id: ms.lane_id.clone(),
                        reason: "empty-finalize".to_string(),
                        media: r.media.clone(),
                    });
                    continue;
                }
                r.clone()
            }
            Some(StreamFinalization {
                finalized: Some(r), ..
            }) => {
                recoverable.push(RecoverableStream {
                    stream_index: ms.stream_index,
                    capture_stream_id: ms.capture_stream_id.clone(),
                    lane_id: ms.lane_id.clone(),
                    reason: "partial-finalize".to_string(),
                    media: r.media.clone(),
                });
                continue;
            }
            Some(StreamFinalization { error: Some(e), .. }) => {
                recoverable.push(RecoverableStream {
                    stream_index: ms.stream_index,
                    capture_stream_id: ms.capture_stream_id.clone(),
                    lane_id: ms.lane_id.clone(),
                    reason: format!("finalized-unreadable:{e}"),
                    media: ms.media.clone(),
                });
                continue;
            }
            _ => {
                recoverable.push(RecoverableStream {
                    stream_index: ms.stream_index,
                    capture_stream_id: ms.capture_stream_id.clone(),
                    lane_id: ms.lane_id.clone(),
                    reason: "missing-finalized".to_string(),
                    media: ms.media.clone(),
                });
                continue;
            }
        };
        let segments = merge_spans(&entry.spans);
        for g in &segments {
            pass_indexes.insert(g.pass_index);
        }
        stream_specs.push((ms.stream_index, entry, segments));
    }

    // pass index → reserved pass id（全 stream 共有・index 昇順に採番）。
    let pass_ids: BTreeMap<u32, String> = pass_indexes
        .iter()
        .map(|&i| (i, RecordingPassId::new(draw()).to_tagged()))
        .collect();

    // 同一 lane に複数 stream が流入する場合、自動 clip は先頭
    // stream のみが生成する（重複 clip set を lane に載せない）。
    let mut lanes_with_clips: BTreeSet<String> = BTreeSet::new();
    let mut streams = Vec::new();
    for (stream_index, r, segments) in stream_specs {
        let stream_id = CaptureStreamId::from_tagged(&r.capture_stream_id)
            .map_err(|_| CommitError::Manifest("bad capture_stream_id tag".into()))?;
        let stream = project.recording.streams.get(&stream_id).ok_or_else(|| {
            CommitError::Manifest(format!(
                "stream {} not registered in project",
                r.capture_stream_id
            ))
        })?;
        if stream.session_id != session_id {
            return Err(CommitError::Manifest(format!(
                "stream {} owned by different session",
                r.capture_stream_id
            )));
        }
        let lane_id = LaneId::from_tagged(&r.lane_id)
            .map_err(|_| CommitError::Manifest("bad lane_id tag".into()))?;
        if stream.lane_id != lane_id {
            return Err(CommitError::Manifest(format!(
                "stream {} lane mismatch",
                r.capture_stream_id
            )));
        }
        let lane = project
            .lanes
            .get(&lane_id)
            .ok_or_else(|| CommitError::Manifest(format!("lane {} missing", r.lane_id)))?;

        let mut seg_entries = Vec::new();
        for g in &segments {
            seg_entries.push(SegmentEntry {
                id: CaptureSegmentId::new(draw()).to_tagged(),
                pass_id: pass_ids[&g.pass_index].clone(),
                pass_index: g.pass_index,
                session_start: g.session_start,
                session_end: g.session_end,
                source_start: g.source_start,
                source_end: g.source_end,
                project_in: g.project_in,
                project_out: g.project_out,
                media_offset: g.media_offset,
                media_len: g.media_len,
            });
        }
        // take = (stream × pass)。segment を pass ごとに session 順で束ねる。
        let mut takes = Vec::new();
        let mut passes_in_stream: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        for g in &seg_entries {
            passes_in_stream
                .entry(g.pass_index)
                .or_default()
                .push(g.id.clone());
        }
        for (pass_index, segment_ids) in &passes_in_stream {
            takes.push(TakeEntry {
                id: TakeId::new(draw()).to_tagged(),
                pass_id: pass_ids[pass_index].clone(),
                pass_index: *pass_index,
                segment_ids: segment_ids.clone(),
            });
        }
        // 初回空 lane のみ、最低 pass の take から segment 単位で clip。
        // 同一 lane では先頭 stream のみ（同一 mutation 内の重複生成防止）。
        let mut clips = Vec::new();
        if lane.clip_ids.is_empty()
            && lanes_with_clips.insert(r.lane_id.clone())
            && let Some(first_take) = takes.iter().min_by_key(|t| t.pass_index)
        {
            for seg_id in &first_take.segment_ids {
                let seg = seg_entries
                    .iter()
                    .find(|g| &g.id == seg_id)
                    .expect("take segment exists");
                clips.push(ClipEntry {
                    id: ClipId::new(draw()).to_tagged(),
                    take_id: first_take.id.clone(),
                    segment_id: seg.id.clone(),
                    tick_in: seg.project_in,
                    tick_out: seg.project_out,
                    source_offset: seg.source_start,
                });
            }
        }
        streams.push(StreamEntry {
            stream_index,
            capture_stream_id: r.capture_stream_id.clone(),
            lane_id: r.lane_id.clone(),
            clock_epoch_id: stream.clock_epoch_id.to_tagged(),
            asset_id: AssetId::new(draw()).to_tagged(),
            source_id: SourceId::new(draw()).to_tagged(),
            pcm_format: r.pcm_format.clone(),
            digest: r.digest.clone(),
            media: r.media.clone(),
            media_byte_len: r.media_byte_len,
            source_frames: r.source_frames,
            sample_rate: r.sample_rate,
            channels: r.channels,
            segments: seg_entries,
            takes,
            clips,
        });
    }

    let payload = CommitPayload {
        session_id: session_id.to_tagged(),
        passes: pass_indexes
            .iter()
            .map(|&i| PassEntry {
                id: pass_ids[&i].clone(),
                index: i,
            })
            .collect(),
        streams,
    };
    Ok((payload, recoverable))
}

/// `prepare_manifest` の結果（manifest + recoverable 分離）。
#[derive(Clone, Debug)]
pub struct PreparedCommit {
    /// 確保済み manifest（既存照合再利用 or 新規作成・原子公開済み）。
    pub manifest: RecordingCommitManifest,
    /// commit 対象外 media。
    pub recoverable: Vec<RecoverableStream>,
}

/// `commit-manifest.json` を確保する。`applied_commits` registry が
/// authority: 既 commit なら staging drift に関わらず stored
/// manifest を返す。未適用なら既存 manifest を refs/project と照合
/// して再利用し（Restart/Retry で再採番しない）、不整合なら破棄して
/// 再構築する。無ければ全 ID を `draw_raw_id` で発行し、content
/// hash を確定して原子公開 + parent dir sync する。
///
/// # Errors
/// [`CommitError`]: manifest 不整合・I/O・参照エラー・build 中の
/// staging 変動。
pub fn prepare_manifest(
    controller: &mut ProjectController,
    area: &StagingArea,
) -> Result<PreparedCommit, CommitError> {
    let session_id = RecordingSessionId::from_tagged(&area.manifest.session_id)
        .map_err(|_| CommitError::Manifest("bad session_id tag".into()))?;

    if let Some(existing) = read_commit_manifest(area)? {
        if existing.schema_version != COMMIT_SCHEMA_VERSION {
            return Err(CommitError::Manifest(format!(
                "unsupported manifest schema {}",
                existing.schema_version
            )));
        }
        if existing.identity.session_id != session_id.to_tagged() {
            return Err(CommitError::Manifest(
                "commit manifest session mismatch".into(),
            ));
        }
        if payload_hash(&existing.payload)? != existing.identity.content_hash {
            return Err(CommitError::Manifest(
                "commit manifest content hash mismatch".into(),
            ));
        }
        // registry（applied_commits）は commit 判定の authority。
        // 既に commit 済みなら staging drift（finalized.json の
        // 削除・改ざん・再 finalize・marker 追記）に関わらず stored
        // manifest を返し、AlreadyApplied/IntegrityConflict の
        // 分類は `commit_session` 側の registry 照合へ委ねる。
        // refs 再構成はここでは不要なので best-effort で行う。
        match commit_status(
            controller.live(),
            session_id,
            &existing.identity.content_hash,
        ) {
            CommitStatus::AlreadyApplied | CommitStatus::IntegrityConflict => {
                let recoverable = build_payload(controller.live(), area, session_id, &mut || 0)
                    .map(|(_, r)| r)
                    .unwrap_or_default();
                return Ok(PreparedCommit {
                    manifest: existing,
                    recoverable,
                });
            }
            CommitStatus::NotApplied => {}
        }
        // 未適用 manifest は refs/project との整合を検査する。
        // fingerprint 照合用に ID 不使用で再構成（recoverable もここで得る）。
        let (probe, recoverable) = build_payload(controller.live(), area, session_id, &mut || 0)?;
        // 整合すれば再利用（Restart/Retry で再採番しない）。不整合
        // なら予約 ID は一度も Project に入っていないので stale
        // manifest を安全に破棄して再構築する（lane 非空化・undo 後の
        // ID 再発行・refs 回復後の再 commit・NothingCommitted 凍結を
        // 全てこの経路で回復可能にする）。
        // session が commit 可能状態でない間（Open 等）は project
        // 依存の staleness を評価しない（状態遷移後に再評価すれば
        // よく、manifest を無駄に採番し直さない）。
        let lifecycle_ok = matches!(
            controller
                .live()
                .recording
                .sessions
                .get(&session_id)
                .map(|s| s.state),
            Some(SessionState::Closed | SessionState::Finalized)
        );
        let stale = fingerprint(&existing.payload) != fingerprint(&probe)
            || check_clip_consistency(&existing.payload).is_err()
            || (lifecycle_ok && validate_commit(controller.live(), &existing).is_err());
        if !stale {
            return Ok(PreparedCommit {
                manifest: existing,
                recoverable,
            });
        }
        // stale → 再構築へ落ちる（下で上書き公開）。
    }

    // fingerprint 照合用に ID 不使用で再構成（recoverable もここで得る）。
    let (probe, recoverable) = build_payload(controller.live(), area, session_id, &mut || 0)?;

    // 新規 ID の必要数を probe（ID 不使用の fingerprint 用構成）から
    // 確定し、controller から先にまとめて採番して build へ供給する。
    // build_payload は project を読むだけで draw は generator のみを
    // 進めるため、二重借りを避けるこの順序で安全。
    let needed = count_new_ids(&probe);
    let mut reserved: Vec<u128> = Vec::with_capacity(needed);
    for _ in 0..needed {
        reserved.push(controller.draw_raw_id());
    }
    let mut iter = reserved.into_iter();
    let (payload, _) = build_payload(controller.live(), area, session_id, &mut || {
        // probe と build の間で staging が変化した場合、過剰 draw に
        // なり得る。sentinel は manifest validation で必ず失敗する
        // 値にし、build 後の個数検査で明示的に検出する。
        iter.next().unwrap_or(u128::MAX)
    })?;
    if iter.next().is_some() {
        return Err(CommitError::Manifest(
            "staging refs drifted during manifest build".into(),
        ));
    }
    let manifest = RecordingCommitManifest {
        schema_version: COMMIT_SCHEMA_VERSION,
        identity: CommitIdentity {
            session_id: session_id.to_tagged(),
            content_hash: payload_hash(&payload)?,
        },
        payload,
    };
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| CommitError::Manifest(format!("manifest encode: {e}")))?;
    // stale manifest の置換を許す（rename は既存 file を上書き
    // しない可能性があるため先に除去する）。
    let path = commit_manifest_path(area);
    if path.exists() {
        fs::remove_file(&path).map_err(io_err("remove stale commit manifest"))?;
    }
    publish_atomic(&path, &bytes).map_err(io_err("publish commit manifest"))?;
    // rename を crash で失わないよう parent dir も sync する
    // （manifest/marker が消えると reserved ID が失われ、
    // 再構築 hash が registry と衝突して IntegrityConflict に沈む）。
    if area.manifest.capabilities.directory_sync == DirSyncCapability::Supported {
        area.sync_dir().map_err(io_err("sync commit dir"))?;
    }
    Ok(PreparedCommit {
        manifest,
        recoverable,
    })
}

// ---------- status / commit driver ----------

/// `applied_commits` registry に基づく 3 分類。
#[must_use]
pub fn commit_status(
    project: &Project,
    session_id: RecordingSessionId,
    content_hash: &str,
) -> CommitStatus {
    match project.recording.applied_commits.get(&session_id) {
        None => CommitStatus::NotApplied,
        Some(applied) if applied.content_hash == content_hash => CommitStatus::AlreadyApplied,
        Some(_) => CommitStatus::IntegrityConflict,
    }
}

/// record → finalize 済み session を Project へ冪等 commit する。
/// 全工程は非 RT。`AlreadyApplied`/`IntegrityConflict`/`NothingCommitted`
/// は mutation を発行しない。`StaleRevision` は bounded retry する
/// （recoverable conflict）。
///
/// # Errors
/// [`CommitError`]: manifest・I/O・project 層の非 recoverable 失敗。
pub fn commit_session(
    controller: &mut ProjectController,
    area: &StagingArea,
) -> Result<CommitReport, CommitError> {
    let session_id = RecordingSessionId::from_tagged(&area.manifest.session_id)
        .map_err(|_| CommitError::Manifest("bad session_id tag".into()))?;
    let prepared = prepare_manifest(controller, area)?;
    let manifest = prepared.manifest;
    let hash = manifest.identity.content_hash.clone();

    let status = commit_status(controller.live(), session_id, &hash);
    let outcome = match status {
        CommitStatus::AlreadyApplied => CommitOutcome::AlreadyApplied,
        CommitStatus::IntegrityConflict => CommitOutcome::IntegrityConflict,
        CommitStatus::NotApplied => {
            if manifest.payload.streams.is_empty() {
                CommitOutcome::NothingCommitted
            } else {
                let mut result = CommitOutcome::Failed {
                    reason: "revision conflict retries exhausted".into(),
                };
                for _ in 0..MAX_REVISION_RETRIES {
                    let expected = controller.live_revision();
                    let op = SynvelisOp::Recording(RecordingOp::CommitRecording {
                        manifest: Box::new(manifest.clone()),
                    });
                    match controller.apply(Command {
                        expected_revision: expected,
                        op,
                    }) {
                        Ok(rev) => {
                            result = CommitOutcome::Applied { revision: rev.0 };
                            break;
                        }
                        Err(Error::StaleRevision { .. }) => {}
                        Err(e) => {
                            result = CommitOutcome::Failed {
                                reason: e.to_string(),
                            };
                            break;
                        }
                    }
                }
                result
            }
        }
    };

    // marker は protocol 完了の journal 側記録。crash で書けなくても
    // registry の AlreadyApplied が二重登録を防ぐ。
    match outcome {
        CommitOutcome::Applied { revision } => {
            write_commit_marker(area, &manifest, "applied", revision)?;
        }
        CommitOutcome::AlreadyApplied => {
            write_commit_marker(
                area,
                &manifest,
                "already-applied",
                controller.live_revision().0,
            )?;
        }
        CommitOutcome::NothingCommitted => {
            write_commit_marker(
                area,
                &manifest,
                "nothing-committed",
                controller.live_revision().0,
            )?;
        }
        _ => {}
    }

    let sources_committed = manifest.payload.streams.len();
    let takes_committed = manifest.payload.streams.iter().map(|s| s.takes.len()).sum();
    let clips_committed = manifest.payload.streams.iter().map(|s| s.clips.len()).sum();
    Ok(CommitReport {
        session_id,
        content_hash: hash,
        outcome,
        sources_committed,
        takes_committed,
        clips_committed,
        recoverable: prepared.recoverable,
    })
}

// ---------- op validation / application ----------

fn parse_id<T>(
    tag: &str,
    parse: impl Fn(&str) -> Result<T, Error>,
    what: &str,
) -> Result<T, Error> {
    parse(tag).map_err(|_| Error::Validation(format!("bad {what} tag: {tag}")))
}

/// `CommitRecording` の全検査（純粋・副作用なし）。manifest の hash・
/// session 状態・参照解決・新規 ID 衝突・範囲・Clock Evidence・
/// lane 空判定を全て確認する。失敗は project を不変のまま拒否する。
///
/// # Errors
/// [`Error`]: 任意の不正（`NotFound`/`WrongOwner`/`IllegalTransition`/
/// `Validation`）。
#[allow(clippy::too_many_lines)] // sequential total checks
pub fn validate_commit(project: &Project, manifest: &RecordingCommitManifest) -> Result<(), Error> {
    if manifest.schema_version != COMMIT_SCHEMA_VERSION {
        return Err(Error::Validation(format!(
            "unsupported commit schema {}",
            manifest.schema_version
        )));
    }
    let recomputed = payload_hash(&manifest.payload)
        .map_err(|_| Error::Validation("commit payload hash failure".into()))?;
    if recomputed != manifest.identity.content_hash {
        return Err(Error::Validation(
            "commit manifest content hash mismatch".into(),
        ));
    }
    let session_id = parse_id(
        &manifest.payload.session_id,
        RecordingSessionId::from_tagged,
        "session_id",
    )?;
    if manifest.identity.session_id != manifest.payload.session_id {
        return Err(Error::Validation("commit identity session mismatch".into()));
    }
    let session = project
        .recording
        .sessions
        .get(&session_id)
        .ok_or_else(|| Error::NotFound(format!("session {session_id}")))?;
    // commit 可能なのは Closed、または FinalizeSession で Finalized
    // 化されたが media 登録は未実施の session（registry に記録が
    // 無い場合のみ）。registry 有り = commit 済みとして拒否する。
    let registered = project.recording.applied_commits.contains_key(&session_id);
    match session.state {
        SessionState::Closed => {}
        SessionState::Finalized if !registered => {}
        _ => {
            return Err(Error::IllegalTransition(format!(
                "session {session_id} is {:?}; commit requires Closed",
                session.state
            )));
        }
    }
    if registered {
        return Err(Error::Validation(format!(
            "session {session_id} commit already applied"
        )));
    }

    // 新規 ID は全 map・applied registry・payload 内で一意であること。
    let mut new_ids: BTreeSet<u128> = BTreeSet::new();
    let mut claim = |raw: u128, what: &str| -> Result<(), Error> {
        if !new_ids.insert(raw) {
            return Err(Error::Validation(format!("duplicate {what} id {raw}")));
        }
        Ok(())
    };

    let mut seen_pass_indexes: BTreeSet<u32> = BTreeSet::new();
    for p in &manifest.payload.passes {
        let pass_id = parse_id(&p.id, RecordingPassId::from_tagged, "pass_id")?;
        claim(pass_id.raw(), "pass")?;
        if project.recording.passes.contains_key(&pass_id) {
            return Err(Error::Validation(format!("pass id {pass_id} collides")));
        }
        // pass index は manifest 内・既存 session 内ともに一意。
        if !seen_pass_indexes.insert(p.index) {
            return Err(Error::Validation(format!(
                "duplicate pass index {} in manifest",
                p.index
            )));
        }
        if project
            .recording
            .passes
            .values()
            .any(|e| e.session_id == session_id && e.index == p.index)
        {
            return Err(Error::Validation(format!(
                "pass index {} already exists in session {session_id}",
                p.index
            )));
        }
    }
    let pass_ids: BTreeSet<String> = manifest
        .payload
        .passes
        .iter()
        .map(|p| p.id.clone())
        .collect();
    let pass_indexes: BTreeSet<u32> = manifest.payload.passes.iter().map(|p| p.index).collect();
    let pass_index_by_id: BTreeMap<String, u32> = manifest
        .payload
        .passes
        .iter()
        .map(|p| (p.id.clone(), p.index))
        .collect();

    let mut seen_stream_indexes: BTreeSet<u16> = BTreeSet::new();
    let mut seen_stream_ids: BTreeSet<&str> = BTreeSet::new();
    for s in &manifest.payload.streams {
        // stream entry は manifest 内で一意。
        if !seen_stream_indexes.insert(s.stream_index)
            || !seen_stream_ids.insert(s.capture_stream_id.as_str())
        {
            return Err(Error::Validation(format!(
                "duplicate stream entry {} in manifest",
                s.stream_index
            )));
        }
        let stream_id = parse_id(
            &s.capture_stream_id,
            CaptureStreamId::from_tagged,
            "capture_stream_id",
        )?;
        let stream = project
            .recording
            .streams
            .get(&stream_id)
            .ok_or_else(|| Error::NotFound(format!("stream {stream_id}")))?;
        if stream.session_id != session_id {
            return Err(Error::WrongOwner(format!("stream {stream_id}")));
        }
        let lane_id = parse_id(&s.lane_id, LaneId::from_tagged, "lane_id")?;
        let lane = project
            .lanes
            .get(&lane_id)
            .ok_or_else(|| Error::NotFound(format!("lane {lane_id}")))?;
        if stream.lane_id != lane_id {
            return Err(Error::WrongOwner(format!(
                "stream {stream_id} lane mismatch"
            )));
        }
        let epoch_id = parse_id(
            &s.clock_epoch_id,
            ClockEpochId::from_tagged,
            "clock_epoch_id",
        )?;
        if stream.clock_epoch_id != epoch_id {
            return Err(Error::WrongOwner(format!(
                "stream {stream_id} clock epoch mismatch"
            )));
        }
        let asset_id = parse_id(&s.asset_id, AssetId::from_tagged, "asset_id")?;
        let source_id = parse_id(&s.source_id, SourceId::from_tagged, "source_id")?;
        claim(asset_id.raw(), "asset")?;
        claim(source_id.raw(), "source")?;
        if project.assets.contains_key(&asset_id) {
            return Err(Error::Validation(format!("asset id {asset_id} collides")));
        }
        if project.sources.contains_key(&source_id) {
            return Err(Error::Validation(format!("source id {source_id} collides")));
        }
        let frame_bytes = u64::from(s.channels) * BYTES_PER_SAMPLE;
        if frame_bytes == 0 {
            return Err(Error::Validation("stream channels must be >= 1".into()));
        }

        if s.segments.is_empty() || s.takes.is_empty() {
            return Err(Error::Validation(format!(
                "stream {} has empty segment/take payload",
                s.stream_index
            )));
        }
        let seg_ids: BTreeSet<String> = s.segments.iter().map(|g| g.id.clone()).collect();
        let take_ids: BTreeSet<String> = s.takes.iter().map(|t| t.id.clone()).collect();
        let mut pass_session_ranges: BTreeMap<u32, Vec<(i64, i64)>> = BTreeMap::new();
        for g in &s.segments {
            let seg_id = parse_id(&g.id, CaptureSegmentId::from_tagged, "segment_id")?;
            claim(seg_id.raw(), "segment")?;
            if project.recording.segments.contains_key(&seg_id) {
                return Err(Error::Validation(format!("segment id {seg_id} collides")));
            }
            if !pass_ids.contains(&g.pass_id) {
                return Err(Error::Validation(format!(
                    "segment {seg_id} references unknown pass"
                )));
            }
            if !pass_indexes.contains(&g.pass_index) {
                return Err(Error::Validation(format!(
                    "segment {seg_id} pass_index not in manifest"
                )));
            }
            // pass_id と pass_index は同一 pass を指すこと。
            if pass_index_by_id.get(&g.pass_id) != Some(&g.pass_index) {
                return Err(Error::Validation(format!(
                    "segment {seg_id} pass_id/pass_index mismatch"
                )));
            }
            let Some(frames) = g.source_end.checked_sub(g.source_start) else {
                return Err(Error::Validation(format!(
                    "segment {seg_id} has inverted source range"
                )));
            };
            if g.session_start < 0
                || g.source_start < 0
                || g.session_end <= g.session_start
                || frames <= 0
                || g.project_out <= g.project_in
            {
                return Err(Error::Validation(format!(
                    "segment {seg_id} has negative/empty/inverted range"
                )));
            }
            if u64::try_from(g.source_end).unwrap_or(u64::MAX) > s.source_frames {
                return Err(Error::Validation(format!(
                    "segment {seg_id} source range exceeds finalized media"
                )));
            }
            if u64::try_from(frames)
                .ok()
                .and_then(|f| f.checked_mul(frame_bytes))
                != Some(g.media_len)
            {
                return Err(Error::Validation(format!(
                    "segment {seg_id} media_len inconsistent with source range"
                )));
            }
            if g.media_offset.saturating_add(g.media_len) > s.media_byte_len {
                return Err(Error::Validation(format!(
                    "segment {seg_id} media range exceeds finalized bytes"
                )));
            }
            pass_session_ranges
                .entry(g.pass_index)
                .or_default()
                .push((g.session_start, g.session_end));
        }
        // 同一 pass の session 範囲は重複しない。
        for (pass, mut ranges) in pass_session_ranges {
            ranges.sort_unstable();
            for w in ranges.windows(2) {
                if w[0].1 > w[1].0 {
                    return Err(Error::Validation(format!(
                        "overlapping session ranges in pass {pass}"
                    )));
                }
            }
        }
        for t in &s.takes {
            let take_id = parse_id(&t.id, TakeId::from_tagged, "take_id")?;
            claim(take_id.raw(), "take")?;
            if project.recording.takes.contains_key(&take_id) {
                return Err(Error::Validation(format!("take id {take_id} collides")));
            }
            if !pass_ids.contains(&t.pass_id) {
                return Err(Error::Validation(format!(
                    "take {take_id} references unknown pass"
                )));
            }
            // take.pass_index も pass_id と一致する index を指すこと。
            if pass_index_by_id.get(&t.pass_id) != Some(&t.pass_index) {
                return Err(Error::Validation(format!(
                    "take {take_id} pass_id/pass_index mismatch"
                )));
            }
            if t.segment_ids.is_empty() {
                return Err(Error::Validation(format!("take {take_id} has no segments")));
            }
            for seg_id in &t.segment_ids {
                if !seg_ids.contains(seg_id) {
                    return Err(Error::Validation(format!(
                        "take {take_id} references foreign segment"
                    )));
                }
            }
        }
        for c in &s.clips {
            let clip_id = parse_id(&c.id, ClipId::from_tagged, "clip_id")?;
            claim(clip_id.raw(), "clip")?;
            if project.clips.contains_key(&clip_id) {
                return Err(Error::Validation(format!("clip id {clip_id} collides")));
            }
            if !take_ids.contains(&c.take_id) {
                return Err(Error::Validation(format!(
                    "clip {clip_id} references unknown take"
                )));
            }
            let seg = s
                .segments
                .iter()
                .find(|g| g.id == c.segment_id)
                .ok_or_else(|| {
                    Error::Validation(format!("clip {clip_id} references unknown segment"))
                })?;
            if c.tick_out <= c.tick_in || c.source_offset < 0 {
                return Err(Error::Validation(format!(
                    "clip {clip_id} has negative/empty/inverted anchor"
                )));
            }
            if c.tick_in != seg.project_in
                || c.tick_out != seg.project_out
                || c.source_offset != seg.source_start
            {
                return Err(Error::Validation(format!(
                    "clip {clip_id} placement diverges from segment"
                )));
            }
        }
        // 自動 clip は初回空 lane のみ。manifest 作成後に lane が非空化
        // した場合は暗黙置換せず commit を拒否する（driver が manifest
        // 再構築を報告する）。
        if !s.clips.is_empty() && !lane.clip_ids.is_empty() {
            return Err(Error::Validation(format!(
                "lane {lane_id} already has clips; refusing implicit comp replacement"
            )));
        }
    }
    Ok(())
}

/// `CommitRecording` の適用。validate 通過後にのみ呼ばれ、全 entity
/// を一括登録し、session を Finalized 化し、registry へ記録し、
/// generator を使用済み最大 raw ID 以後へ reseed する。
///
/// # Errors
/// [`Error::Validation`]: 不正 tag（validate 通過後は起こらない）。
#[allow(clippy::too_many_lines)] // per-stream 直列登録
pub fn apply_commit(
    project: &mut Project,
    manifest: &RecordingCommitManifest,
    ids: &mut dyn IdGenerator,
) -> Result<(), Error> {
    let session_id = RecordingSessionId::from_tagged(&manifest.payload.session_id)
        .map_err(|e| Error::Validation(format!("{e}")))?;
    let mut max_raw = 0u128;
    let mut note = |raw: u128| {
        if raw > max_raw {
            max_raw = raw;
        }
    };

    let mut source_ids = Vec::new();
    let mut take_ids = Vec::new();
    let mut clip_ids = Vec::new();

    for p in &manifest.payload.passes {
        let pass_id =
            RecordingPassId::from_tagged(&p.id).map_err(|e| Error::Validation(format!("{e}")))?;
        note(pass_id.raw());
        project.recording.passes.insert(
            pass_id,
            RecordingPass {
                id: pass_id,
                session_id,
                index: p.index,
            },
        );
    }

    for s in &manifest.payload.streams {
        let stream_id = CaptureStreamId::from_tagged(&s.capture_stream_id)
            .map_err(|e| Error::Validation(format!("{e}")))?;
        let lane_id =
            LaneId::from_tagged(&s.lane_id).map_err(|e| Error::Validation(format!("{e}")))?;
        let asset_id =
            AssetId::from_tagged(&s.asset_id).map_err(|e| Error::Validation(format!("{e}")))?;
        let source_id =
            SourceId::from_tagged(&s.source_id).map_err(|e| Error::Validation(format!("{e}")))?;
        note(asset_id.raw());
        note(source_id.raw());
        source_ids.push(source_id);
        project.assets.insert(
            asset_id,
            Asset {
                id: asset_id,
                digest: s.digest.clone(),
            },
        );
        // Verified（media 検証済み）→ Committed（本 mutation が project
        // 登録）の両段をこの原子 mutation 内で確定させる。
        project.sources.insert(
            source_id,
            Source {
                id: source_id,
                asset_id,
                content_digest: s.digest.clone(),
                state: SourceState::Committed,
            },
        );
        for g in &s.segments {
            let seg_id = CaptureSegmentId::from_tagged(&g.id)
                .map_err(|e| Error::Validation(format!("{e}")))?;
            let pass_id = RecordingPassId::from_tagged(&g.pass_id)
                .map_err(|e| Error::Validation(format!("{e}")))?;
            note(seg_id.raw());
            project.recording.segments.insert(
                seg_id,
                CaptureSegment {
                    id: seg_id,
                    stream_id,
                    pass_id,
                    session_range: SessionRange::try_new(
                        SessionFrame::new(g.session_start)?,
                        SessionFrame::new(g.session_end)?,
                    )?,
                    source_range: SourceRange::try_new(
                        SourceFrame::new(g.source_start)?,
                        SourceFrame::new(g.source_end)?,
                    )?,
                    source_id,
                },
            );
        }
        for t in &s.takes {
            let take_id =
                TakeId::from_tagged(&t.id).map_err(|e| Error::Validation(format!("{e}")))?;
            let pass_id = RecordingPassId::from_tagged(&t.pass_id)
                .map_err(|e| Error::Validation(format!("{e}")))?;
            note(take_id.raw());
            take_ids.push(take_id);
            let segment_ids = t
                .segment_ids
                .iter()
                .map(|s| {
                    CaptureSegmentId::from_tagged(s).map_err(|e| Error::Validation(format!("{e}")))
                })
                .collect::<Result<Vec<_>, _>>()?;
            project.recording.takes.insert(
                take_id,
                Take {
                    id: take_id,
                    session_id,
                    pass_id,
                    stream_id,
                    lane_id,
                    source_id,
                    segment_ids,
                    state: TakeState::Verified,
                },
            );
        }
        for c in &s.clips {
            let clip_id =
                ClipId::from_tagged(&c.id).map_err(|e| Error::Validation(format!("{e}")))?;
            note(clip_id.raw());
            clip_ids.push(clip_id);
            project.clips.insert(
                clip_id,
                Clip {
                    id: clip_id,
                    lane_id,
                    source_id,
                    anchor: TimeAnchor::Musical(MusicalRange::try_new(
                        MusicalTick::new(c.tick_in),
                        MusicalTick::new(c.tick_out),
                    )?),
                    source_offset: SourceFrame::new(c.source_offset)?,
                },
            );
            if let Some(lane) = project.lanes.get_mut(&lane_id) {
                lane.clip_ids.push(clip_id);
            }
        }
    }

    if let Some(session) = project.recording.sessions.get_mut(&session_id) {
        session.state = SessionState::Finalized;
    }
    project.recording.applied_commits.insert(
        session_id,
        AppliedRecordingCommit {
            content_hash: manifest.identity.content_hash.clone(),
            source_ids,
            take_ids,
            clip_ids,
        },
    );
    // manifest 予約 ID が既存 watermark を超えていても、適用後は
    // generator がそれ以後から発行する（reload 後の再発行防止）。
    let next = max_raw.saturating_add(1);
    if next > ids.peek() {
        ids.reseed(next);
    }
    Ok(())
}
