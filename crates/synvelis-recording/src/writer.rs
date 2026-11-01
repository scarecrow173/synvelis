//! 非 RT `StreamWriter`: VS-B の `CaptureChunk` 契約を消費し、stream
//! 単位の Raw PCM + Journal へ決定的順序で追記する staging writer。
//!
//! 保存状態 `Captured/Enqueued/Written/Validated/Durable Checkpoint/
//! Finalized` を厳密に区別する。OS の `write_all` 成功を durable と
//! 呼ばず、sync 失敗は `SyncFailed` として `Io` と区別して報告する。
//!
//! 1 chunk あたりの順序: `filled` 受信 → 連続整合検証 → media 追記
//! （Written）→ journal record 追記（Validated）→ `free` 返却。
//! `checkpoint()` は「media `sync_data` → Checkpoint record 追記 →
//! journal `sync_data` → dir sync（capability 時のみ）」の順序で、
//! それまでの Validated prefix を durable frontier として宣言する。

use std::{
    fs::{File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    capture::{CaptureChunk, ChunkValidity},
    crc32::digest_fold,
    faultinj::{FaultWriter, Inject, StorageWriter},
    handoff::{ChunkSink, FAULT_CONTRACT, FAULT_WRITER_ERROR, FaultLatch, WriterEnds},
    journal::{
        CheckpointRecord, ChunkRecord, FinalizeOutcome, FinalizeRecord, JournalAppender,
        JournalSpan, RecordPayload,
    },
    staging::{DirSyncCapability, StagingArea, publish_atomic, sync_parent_dir},
};

/// stream の writer 状態機械。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StreamState {
    /// 追記可能。
    Active,
    /// Writer エラー/契約違反で停止（以後書込まない。Validated prefix は
    /// 保護される）。
    Failed,
    /// Finalize 済み（`finalized.json` 公開済み）。
    Finalized,
}

/// stream 単位の file 配置（`StagingArea` から導出）。
#[derive(Clone, Debug)]
pub struct StreamPaths {
    /// raw PCM media file。
    pub media: PathBuf,
    /// journal file。
    pub journal: PathBuf,
    /// finalized marker。
    pub finalized: PathBuf,
}

/// durable frontier（最後に sync 成功した checkpoint）。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct DurableFrontier {
    /// checkpoint record の journal seq。
    pub seq: u64,
    /// checkpoint 番号。
    pub checkpoint_index: u64,
    /// sync した media byte 長。
    pub media_len: u64,
    /// sync した source frame 総数。
    pub source_frames: u64,
    /// その時点の digest fold。
    pub digest_fold: u64,
}

/// `FinalizedSourceRef` の outcome。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FinalizedOutcome {
    /// 全 source が frontier に含まれる完全録音。
    Complete,
    /// 途中失敗/中断した stream の verified prefix。
    Partial,
}

/// `FinalizedSourceRef` 内の 1 保存区間。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalizedSpan {
    /// Loop pass 番号。
    pub pass_index: u32,
    /// session 区間（半開）。
    pub session_start: i64,
    /// session 区間終端（exclusive）。
    pub session_end: i64,
    /// source 区間（半開）。
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

/// VS-D へ渡す Project 非依存の確定 source 参照。
/// `SourceId`/`AssetId`/`TakeId` 等の Project 参照を持たない。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FinalizedSourceRef {
    /// schema version。
    pub schema_version: u32,
    /// session 識別子（tagged）。
    pub session_id: String,
    /// stream index。
    pub stream_index: u16,
    /// capture stream 識別子（tagged）。
    pub capture_stream_id: String,
    /// lane 識別子（tagged）。
    pub lane_id: String,
    /// sample rate。
    pub sample_rate: u32,
    /// PCM 形式識別子。
    pub pcm_format: String,
    /// チャネル数。
    pub channels: u32,
    /// media file の session 相対 path。
    pub media: String,
    /// 確定 media byte 長（durable frontier）。
    pub media_byte_len: u64,
    /// 確定 source frame 総数。
    pub source_frames: u64,
    /// complete/partial。
    pub outcome: FinalizedOutcome,
    /// 確定 span 列（session 順）。
    pub spans: Vec<FinalizedSpan>,
    /// media 内容の fold digest（hex）。
    pub digest: String,
    /// frontier の journal seq。
    pub journal_seq: u64,
    /// frontier の checkpoint index。
    pub checkpoint_index: u64,
    /// 観測した fault 記録（診断用）。
    pub faults: Vec<String>,
}

/// writer 側エラー。write 系と sync 系を区別する。
#[derive(Debug)]
pub enum WriterError {
    /// media/journal/公開の write 失敗（durable とは限らない）。
    Io {
        /// 失敗した操作。
        op: &'static str,
        /// 発生元。
        source: io::Error,
    },
    /// `sync_data` の失敗（write 成功とは別種。durable 未到達）。
    SyncFailed {
        /// 失敗した操作。
        op: &'static str,
        /// 発生元。
        source: io::Error,
    },
    /// Chunk 契約違反（`FAULT_CONTRACT` を latch する）。
    ContractViolation(&'static str),
    /// 既に finalize 済み（二重 Finalize 防止）。
    AlreadyFinalized,
    /// journal/media の整合破損（finalizer/recovery 検査で検出）。
    Corrupt(String),
    /// staging 側のエラー。
    Staging(crate::staging::StagingError),
}

impl std::fmt::Display for WriterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { op, source } => write!(f, "io error ({op}): {source}"),
            Self::SyncFailed { op, source } => write!(f, "sync failed ({op}): {source}"),
            Self::ContractViolation(m) => write!(f, "chunk contract violation: {m}"),
            Self::AlreadyFinalized => write!(f, "stream already finalized"),
            Self::Corrupt(m) => write!(f, "corrupt staging data: {m}"),
            Self::Staging(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for WriterError {}

impl From<crate::staging::StagingError> for WriterError {
    fn from(e: crate::staging::StagingError) -> Self {
        Self::Staging(e)
    }
}

fn io_err(op: &'static str) -> impl FnOnce(io::Error) -> WriterError {
    move |source| WriterError::Io { op, source }
}
fn sync_err(op: &'static str) -> impl FnOnce(io::Error) -> WriterError {
    move |source| WriterError::SyncFailed { op, source }
}

/// `stream_index` の file 配置を `area` から導出する。
#[must_use]
pub fn stream_paths(area: &StagingArea, stream_index: u16) -> StreamPaths {
    StreamPaths {
        media: area.media_path(stream_index),
        journal: area.journal_path(stream_index),
        finalized: area.finalized_path(stream_index),
    }
}

/// 追記用に file を open する（無ければ作成）。
fn open_append(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

/// 1 stream の staging writer。`WriterEnds` を所有し、
/// [`ChunkSink`] として scenario driver から駆動される。
pub struct StreamWriter {
    cfg: crate::staging::StagingStreamConfig,
    paths: StreamPaths,
    media: FaultWriter,
    journal: FaultWriter,
    journal_path: PathBuf,
    ends: WriterEnds,
    fault: FaultLatch,
    dir_sync: DirSyncCapability,
    state: StreamState,
    state_reason: Option<String>,
    appender: JournalAppender,
    /// journal record まで確定した cursor（source 連続検証に使う）。
    source_cursor: i64,
    last_session_end: i64,
    last_input_end: u64,
    /// media file の次回書込み位置（実 file end。journaled とは別）。
    media_len: u64,
    /// journal record が発行済みの media frontier（checkpoint が記録
    /// する値。書込み途中失敗で実 file が先行しても journaled
    /// frontier だけが durable 候補になる）。
    journaled_media_len: u64,
    /// journal record が発行済みの digest fold 状態。
    journaled_digest: u64,
    /// journal record が発行済みの source frame 総数。
    journaled_source_frames: u64,
    /// fold 中の digest（journal append 成功で `journaled_digest` へ）。
    digest: u64,
    chunk_records: u32,
    checkpoint_index: u64,
    durable: Option<DurableFrontier>,
    held: Vec<CaptureChunk>,
    /// 観測した fault の診断記録（finalized ref へ転記）。
    pub notes: Vec<String>,
}

impl StreamWriter {
    /// `StagingArea` 上に `stream_index` の writer を構築する
    /// （media/journal を append open。`manifest_entry` は manifest の
    /// stream 行）。
    ///
    /// # Errors
    /// file open の失敗。
    pub fn create(
        area: &StagingArea,
        cfg: crate::staging::StagingStreamConfig,
        ends: WriterEnds,
        fault: FaultLatch,
    ) -> Result<Self, WriterError> {
        let paths = stream_paths(area, cfg.stream_index);
        if paths.finalized.exists() {
            return Err(WriterError::AlreadyFinalized);
        }
        // 既に journal がある stream への追記継続は resume 対応を要する。
        // seq 二重発番・未検証 tail 混入を防ぐため create は空 journal
        // のみを許容する（既存内容の検証は recovery の責務）。
        if std::fs::metadata(&paths.journal)
            .map_err(io_err("journal metadata"))?
            .len()
            > 0
        {
            return Err(WriterError::Corrupt(
                "journal not empty; use recovery::scan/recover_finalize".into(),
            ));
        }
        let media = open_append(&paths.media).map_err(io_err("open media"))?;
        let journal = open_append(&paths.journal).map_err(io_err("open journal"))?;
        Ok(Self::with_writers(
            cfg,
            paths,
            FaultWriter::new(media),
            FaultWriter::new(journal),
            ends,
            fault,
            area.manifest.capabilities.directory_sync,
        ))
    }

    /// `FaultWriter` 包みの media/journal から構築する（fault 注入用）。
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn with_writers(
        cfg: crate::staging::StagingStreamConfig,
        paths: StreamPaths,
        media: FaultWriter,
        journal: FaultWriter,
        ends: WriterEnds,
        fault: FaultLatch,
        dir_sync: DirSyncCapability,
    ) -> Self {
        Self {
            cfg,
            journal_path: paths.journal.clone(),
            paths,
            media,
            journal,
            ends,
            fault,
            dir_sync,
            state: StreamState::Active,
            state_reason: None,
            appender: JournalAppender::new(),
            source_cursor: 0,
            last_session_end: 0,
            last_input_end: 0,
            media_len: 0,
            journaled_media_len: 0,
            journaled_digest: 0,
            journaled_source_frames: 0,
            digest: 0,
            chunk_records: 0,
            checkpoint_index: 0,
            durable: None,
            held: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// 現在の状態。
    #[must_use]
    pub fn state(&self) -> StreamState {
        self.state
    }

    /// 失敗理由（`Failed` の場合）。
    #[must_use]
    pub fn failure_reason(&self) -> Option<&str> {
        self.state_reason.as_deref()
    }

    /// stream index。
    #[must_use]
    pub fn stream_index(&self) -> u16 {
        self.cfg.stream_index
    }

    /// 書込み済み media byte 長（Written frontier）。
    #[must_use]
    pub fn media_len(&self) -> u64 {
        self.media_len
    }

    /// 確定した source frame 総数（Validated 済みのものに限る）。
    #[must_use]
    pub fn source_frames(&self) -> u64 {
        self.journaled_source_frames
    }

    /// 最後の durable frontier。
    #[must_use]
    pub fn durable(&self) -> Option<DurableFrontier> {
        self.durable
    }

    /// 保持中の chunk（所有権照合用）。
    #[must_use]
    pub fn held(&self) -> &[CaptureChunk] {
        &self.held
    }

    /// 共有 fault latch（診断・検証用）。
    #[must_use]
    pub fn fault(&self) -> &FaultLatch {
        &self.fault
    }

    /// media への fault 注入を予約する。
    pub fn inject_media(&mut self, inj: Inject) {
        self.media.inject(inj);
    }

    /// journal への fault 注入を予約する。
    pub fn inject_journal(&mut self, inj: Inject) {
        self.journal.inject(inj);
    }

    fn fail(&mut self, reason: String, bit: u32) {
        if self.state == StreamState::Active {
            self.state = StreamState::Failed;
            self.state_reason = Some(reason.clone());
            self.notes.push(reason);
        }
        self.fault.set(bit);
    }

    /// Chunk の連続整合を検証する。失敗は `ContractViolation`。
    /// 副作用は持たず、cursor の commit は journal record の append
    /// 成功後に `process_chunk` が行う。
    fn verify_chunk(&self, chunk: &CaptureChunk) -> Result<ChunkCursors, WriterError> {
        if chunk.validity != ChunkValidity::Complete {
            return Err(WriterError::ContractViolation("non-complete chunk"));
        }
        if chunk.stream_index != self.cfg.stream_index {
            return Err(WriterError::ContractViolation("stream_index mismatch"));
        }
        if !chunk.header.has_capture {
            return Err(WriterError::ContractViolation("has_capture unset"));
        }
        let channels = usize::try_from(self.cfg.channels).unwrap_or(usize::MAX);
        let expected_samples = usize::try_from(chunk.stored_frames)
            .ok()
            .and_then(|f| f.checked_mul(channels));
        if expected_samples != Some(chunk.stored_samples) {
            return Err(WriterError::ContractViolation(
                "stored_samples != stored_frames*channels",
            ));
        }
        if chunk.stored_samples > chunk.pcm.len() || chunk.spans.is_empty() {
            return Err(WriterError::ContractViolation("invalid span/pcm state"));
        }
        // block 間の重複なし（gate-off で chunk が出ない区間の gap は許容）。
        let session_start = chunk.header.session_start.value();
        let block_end = session_start
            .checked_add(i64::from(chunk.header.block_frames))
            .ok_or(WriterError::ContractViolation("session overflow"))?;
        if session_start < self.last_session_end {
            return Err(WriterError::ContractViolation("session overlap"));
        }
        let input_end = chunk
            .header
            .first_input_frame
            .checked_add(u64::from(chunk.header.block_frames))
            .ok_or(WriterError::ContractViolation("input overflow"))?;
        if chunk.header.first_input_frame < self.last_input_end {
            return Err(WriterError::ContractViolation("input overlap"));
        }
        // span 列: session 順・非重複・packed 範囲内・source 連続・
        // `sum(frame_count) == stored_frames`。
        let mut span_cursor = session_start;
        let mut packed_end = 0usize;
        let mut next_source = self.source_cursor;
        let mut span_frames = 0u64;
        for span in chunk.spans.as_slice() {
            if span.frame_count == 0 {
                return Err(WriterError::ContractViolation("empty span"));
            }
            let s_start = span.session_start.value();
            let s_end = s_start
                .checked_add(i64::from(span.frame_count))
                .ok_or(WriterError::ContractViolation("span overflow"))?;
            if s_start < span_cursor || s_end > block_end {
                return Err(WriterError::ContractViolation("span out of order"));
            }
            let span_samples = usize::try_from(span.frame_count)
                .ok()
                .and_then(|f| f.checked_mul(channels));
            let Some(span_samples) = span_samples else {
                return Err(WriterError::ContractViolation("span size overflow"));
            };
            if span.packed_pcm_offset < packed_end
                || span
                    .packed_pcm_offset
                    .checked_add(span_samples)
                    .is_none_or(|e| e > chunk.stored_samples)
            {
                return Err(WriterError::ContractViolation("packed range invalid"));
            }
            if span.source_start.value() != next_source {
                return Err(WriterError::ContractViolation("source discontinuity"));
            }
            span_cursor = s_end;
            packed_end = span.packed_pcm_offset + span_samples;
            next_source = s_end_of_source(span)?;
            span_frames = span_frames.saturating_add(u64::from(span.frame_count));
        }
        if span_frames != u64::from(chunk.stored_frames) {
            return Err(WriterError::ContractViolation(
                "stored_frames != sum(span.frame_count)",
            ));
        }
        Ok(ChunkCursors {
            source: next_source,
            session_end: block_end,
            input_end,
        })
    }

    fn process_chunk(&mut self, chunk: CaptureChunk) {
        if self.state != StreamState::Active {
            // Finalized/Failed への到着は契約違反として記録し、
            // 所有権だけ保持する（何も書き込まない）。
            self.fault.set(FAULT_CONTRACT);
            self.held.push(chunk);
            return;
        }
        let cursors = match self.verify_chunk(&chunk) {
            Ok(c) => c,
            Err(WriterError::ContractViolation(m)) => {
                self.fail(format!("contract: {m}"), FAULT_CONTRACT);
                self.held.push(chunk);
                return;
            }
            Err(_) => unreachable!("verify_chunk returns only ContractViolation"),
        };
        // ---- media 追記（Written）----
        let channels = usize::try_from(self.cfg.channels).unwrap_or(usize::MAX);
        let media_offset = self.media_len;
        let mut write_offset = media_offset;
        let mut pcm_crc = crate::crc32::Crc32::new();
        let mut span_records = Vec::with_capacity(chunk.spans.len());
        for span in chunk.spans.as_slice() {
            let start = span.packed_pcm_offset;
            let len = usize::try_from(span.frame_count).unwrap_or(0) * channels;
            let mut bytes = Vec::with_capacity(len * 4);
            for s in &chunk.pcm[start..start + len] {
                bytes.extend_from_slice(&s.to_le_bytes());
            }
            if let Err(source) = self.media.write_all(&bytes) {
                self.fail(format!("media write: {source}"), FAULT_WRITER_ERROR);
                self.held.push(chunk);
                return;
            }
            pcm_crc.update(&bytes);
            let written = u64::try_from(bytes.len()).unwrap_or(0);
            self.media_len = self.media_len.saturating_add(written);
            span_records.push(JournalSpan {
                pass_index: span.pass_index,
                session_start: span.session_start.value(),
                frame_count: span.frame_count,
                source_start: span.source_start.value(),
                project_in: span.project_in.value(),
                project_out: span.project_out.value(),
                media_offset: write_offset,
            });
            write_offset = write_offset.saturating_add(written);
        }
        // ---- journal record 追記（Validated）----
        let pcm_crc32 = pcm_crc.finish();
        self.digest = digest_fold(self.digest, u64::from(pcm_crc32));
        let media_len = self.media_len - media_offset;
        let written_end = write_offset;
        let rec = ChunkRecord {
            chunk_id: chunk.id,
            first_input_frame: chunk.header.first_input_frame,
            session_start: chunk.header.session_start.value(),
            block_frames: chunk.header.block_frames,
            stored_frames: chunk.stored_frames,
            media_offset,
            media_len,
            pcm_crc32,
            digest_fold: self.digest,
            spans: span_records,
        };
        let Some(bytes) = self.appender.encode_next(&RecordPayload::Chunk(rec)) else {
            self.fail(
                "journal encode: payload too large".into(),
                FAULT_WRITER_ERROR,
            );
            self.held.push(chunk);
            return;
        };
        if let Err(source) = self.journal.write_all(&bytes) {
            self.fail(format!("journal write: {source}"), FAULT_WRITER_ERROR);
            self.held.push(chunk);
            return;
        }
        // ---- journaled frontier commit（journal append 成功。ここ
        // より前の失敗は media byte のみ残り、unverified tail として
        // 保護される。checkpoint/durable は journaled 側を記録する）----
        self.chunk_records = self.chunk_records.saturating_add(1);
        self.journaled_media_len = written_end;
        self.journaled_digest = self.digest;
        self.journaled_source_frames = u64::try_from(cursors.source).unwrap_or(0);
        self.source_cursor = cursors.source;
        self.last_session_end = cursors.session_end;
        self.last_input_end = cursors.input_end;
        // ---- free 返却 ----
        if let Err(e) = self.ends.free_tx.try_push(chunk) {
            self.fault.set(FAULT_WRITER_ERROR);
            self.held.push(e.into_inner());
        }
    }

    /// durable frontier を 1 つ進める。
    /// media `sync_data` → Checkpoint record 追記 → journal `sync_data`
    /// → dir sync（capability が `Supported` の場合のみ）。
    /// sync 失敗は `SyncFailed`（write 失敗と別種）で、record は発行しない。
    ///
    /// # Errors
    /// `SyncFailed`/`Io`。`Finalized` 後は no-op で `Ok`。
    pub fn checkpoint(&mut self) -> Result<(), WriterError> {
        if self.state == StreamState::Finalized {
            return Ok(());
        }
        self.media
            .sync_data()
            .map_err(sync_err("media.sync_data"))?;
        let rec = CheckpointRecord {
            checkpoint_index: self.checkpoint_index,
            media_len: self.journaled_media_len,
            source_frames: self.journaled_source_frames,
            digest_fold: self.journaled_digest,
            chunk_records: self.chunk_records,
        };
        let Some(bytes) = self.appender.encode_next(&RecordPayload::Checkpoint(rec)) else {
            return Err(WriterError::Corrupt("checkpoint encode failed".into()));
        };
        let seq = self.appender.next_seq().saturating_sub(1);
        if let Err(source) = self.journal.write_all(&bytes) {
            // append 失敗後の追記は seq gap で unreachable になるため
            // stream を Failed にして dead-end を防ぐ。
            self.fail(format!("journal append: {source}"), FAULT_WRITER_ERROR);
            return Err(WriterError::Io {
                op: "journal.append checkpoint",
                source,
            });
        }
        // record は file 上に存在する（index を消費し、sync 失敗の
        // retry で重複 index にならないよう append 時点で繰り上げ）。
        self.checkpoint_index = self.checkpoint_index.saturating_add(1);
        self.journal
            .sync_data()
            .map_err(sync_err("journal.sync_data"))?;
        if self.dir_sync == DirSyncCapability::Supported
            && let Err(e) = sync_parent_dir(&self.journal_path)
        {
            self.notes.push(format!("dir sync failed: {e}"));
        }
        self.durable = Some(DurableFrontier {
            seq,
            checkpoint_index: rec.checkpoint_index,
            media_len: self.journaled_media_len,
            source_frames: self.journaled_source_frames,
            digest_fold: self.journaled_digest,
        });
        Ok(())
    }

    /// Byte Length/Format/Frame Count/Checksum・Journal 整合性を再検証し
    /// `finalized.json` を原子公開する。既存なら `AlreadyFinalized`。
    ///
    /// # Errors
    /// `SyncFailed`（暗黙 checkpoint の sync 失敗）/`Corrupt`（検証
    /// 不一致）/`Io`/`AlreadyFinalized`。
    pub fn finalize(
        &mut self,
        session_id: &str,
        sample_rate: u32,
    ) -> Result<FinalizedSourceRef, WriterError> {
        if self.state == StreamState::Finalized || self.paths.finalized.exists() {
            return Err(WriterError::AlreadyFinalized);
        }
        // journaled frontier が最後の checkpoint 以後に伸びていれば
        // 最終 checkpoint を取る（失敗時は SyncFailed で finalize 自体
        // が失敗）。unjournaled tail（失敗 chunk の部分 byte）は
        // journaled_media_len に含まれず、checkpoint に混入しない。
        if self
            .durable
            .is_none_or(|d| d.media_len != self.journaled_media_len)
        {
            self.checkpoint()?;
        }
        let Some(frontier) = self.durable else {
            return Err(WriterError::Corrupt("no durable frontier".into()));
        };
        let verified = crate::recovery::verify_stream(
            &self.paths.media,
            &self.paths.journal,
            frontier,
            self.cfg.channels,
        )?;
        let outcome = if self.state == StreamState::Failed
            || verified.validated_tail_bytes > 0
            || verified.unverified_tail_bytes > 0
        {
            FinalizedOutcome::Partial
        } else {
            FinalizedOutcome::Complete
        };
        let source_ref = FinalizedSourceRef {
            schema_version: crate::staging::STAGING_SCHEMA_VERSION,
            session_id: session_id.to_string(),
            stream_index: self.cfg.stream_index,
            capture_stream_id: self.cfg.capture_stream_id.clone(),
            lane_id: self.cfg.lane_id.clone(),
            sample_rate,
            pcm_format: crate::staging::PCM_FORMAT.to_string(),
            channels: self.cfg.channels,
            media: crate::staging::media_rel(self.cfg.stream_index),
            media_byte_len: frontier.media_len,
            source_frames: frontier.source_frames,
            outcome,
            spans: verified.spans,
            digest: format!("{:016x}", frontier.digest_fold),
            journal_seq: frontier.seq,
            checkpoint_index: frontier.checkpoint_index,
            faults: self.notes.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&source_ref)
            .map_err(|e| WriterError::Corrupt(format!("ref encode: {e}")))?;
        publish_atomic(&self.paths.finalized, &bytes).map_err(io_err("publish finalized"))?;
        let rec = FinalizeRecord {
            media_len: frontier.media_len,
            source_frames: frontier.source_frames,
            digest_fold: frontier.digest_fold,
            outcome: match outcome {
                FinalizedOutcome::Complete => FinalizeOutcome::Complete,
                FinalizedOutcome::Partial => FinalizeOutcome::Partial,
            },
        };
        if let Some(bytes) = self.appender.encode_next(&RecordPayload::Finalize(rec)) {
            // finalized.json が authority。journal の Finalize record は
            // 診断用で、失敗しても公開済み ref は取り消さない（notes
            // に記録するのみ）。
            if let Err(e) = self.journal.write_all(&bytes) {
                self.notes.push(format!("journal finalize append: {e}"));
            } else if let Err(e) = self.journal.sync_data() {
                self.notes.push(format!("journal finalize sync: {e}"));
            }
        }
        if self.dir_sync == DirSyncCapability::Supported {
            let _ = sync_parent_dir(&self.journal_path);
        }
        self.state = StreamState::Finalized;
        Ok(source_ref)
    }
}

/// `verify_chunk` が検算した cursor。journal append 成功後に
/// `process_chunk` が commit する。
struct ChunkCursors {
    source: i64,
    session_end: i64,
    input_end: u64,
}

fn s_end_of_source(span: &crate::capture::CaptureSpan) -> Result<i64, WriterError> {
    span.source_start
        .value()
        .checked_add(i64::from(span.frame_count))
        .ok_or(WriterError::ContractViolation("source overflow"))
}

impl ChunkSink for StreamWriter {
    fn pump(&mut self) {
        while let Ok(chunk) = self.ends.filled_rx.try_pop() {
            self.process_chunk(chunk);
        }
    }

    fn into_ends(
        self,
    ) -> (
        ruviform_rt::Consumer<CaptureChunk>,
        ruviform_rt::Producer<CaptureChunk>,
        Vec<CaptureChunk>,
    ) {
        (self.ends.filled_rx, self.ends.free_tx, self.held)
    }
}

/// 全 stream に checkpoint を掛ける。各 stream の結果を index 順に返す。
pub fn checkpoint_all(streams: &mut [StreamWriter]) -> Vec<Result<(), WriterError>> {
    streams.iter_mut().map(StreamWriter::checkpoint).collect()
}

/// `finish` の結果（stream index 順）。
pub struct FinishReport {
    /// stream ごとの finalize 結果。
    pub refs: Vec<(u16, Result<FinalizedSourceRef, WriterError>)>,
}

/// 残りの filled を drain し、全 stream に最終 checkpoint → finalize を
/// 掛ける（各 stream の結果を index 順に返す）。
///
/// `streams` は scenario の writer と同一順序。`ends` の free 返却は
/// `pump` 内で行われる。
pub fn finish_all<'a>(
    session_id: &str,
    sample_rate: u32,
    streams: impl IntoIterator<Item = &'a mut StreamWriter>,
) -> FinishReport {
    let mut refs = Vec::new();
    for s in streams {
        s.pump();
        let r = s.finalize(session_id, sample_rate);
        refs.push((s.stream_index(), r));
    }
    FinishReport { refs }
}
