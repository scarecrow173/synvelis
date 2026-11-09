//! Staging 領域: `RecordingSessionId` ごとに隔離された
//! Manifest + stream 別 Raw PCM / Journal / Finalized marker。
//!
//! 配置:
//! ```text
//! <staging_root>/<session-tag>/manifest.json
//! <staging_root>/<session-tag>/streams/stream-NNNN.{pcm,journal,finalized.json}
//! ```
//!
//! Manifest は作成時に 1 度だけ atomic publish（tmp→sync→rename）し、
//! 以後 immutable。`capabilities` は作成時に検出した FS 保証範囲を
//! 記録する（directory sync が非対応の環境では `unsupported` を正直に
//! 記録し、保証を過剰主張しない）。

use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use synvelis_project::{CaptureStreamId, LaneId, RecordingSessionId};

use crate::faultinj::PublishFaults;

/// manifest/finalized JSON の schema version。
pub const STAGING_SCHEMA_VERSION: u32 = 1;
/// media の PCM 形式識別子。
pub const PCM_FORMAT: &str = "f32le-interleaved";
/// canonical receipt (`canonical.json`) の schema version。
pub const CANONICAL_SCHEMA_VERSION: u32 = 1;
/// canonical media の container/format 識別子（PCM f32 WAV）。
pub const CANONICAL_MEDIA_FORMAT: &str = "wav-f32";

/// 1 stream の staging 設定（呼び出し側が plan/project から供給する
/// Project 非依存の identity/format 解決）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagingStreamConfig {
    /// `plan.targets` の添字。
    pub stream_index: u16,
    /// この stream の安定識別子（tagged 文字列）。
    pub capture_stream_id: String,
    /// 録音先 lane の安定識別子（tagged 文字列）。
    pub lane_id: String,
    /// チャネル数（1 以上）。
    pub channels: u32,
}

/// session 全体の staging 設定。
#[derive(Clone, Debug)]
pub struct StagingSessionConfig {
    /// session の安定識別子。
    pub session_id: RecordingSessionId,
    /// sample rate（Hz）。
    pub sample_rate: u32,
    /// stream 設定（`stream_index` 順・一意）。
    pub streams: Vec<StagingStreamConfig>,
}

/// FS capability の観測値。
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DirSyncCapability {
    /// dir を open して sync できた。
    Supported,
    /// dir open/sync が非対応（Windows 等）。
    Unsupported,
    /// 検出自体が失敗した。
    Failed,
}

/// manifest.json に書かれる FS 保証範囲の記録。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagingCapabilities {
    /// tmp→rename の原子公開を利用（常に試行）。
    pub atomic_rename: bool,
    /// file sync（`sync_data`）を利用。
    pub file_sync: bool,
    /// directory sync の観測値。
    pub directory_sync: DirSyncCapability,
}

/// manifest.json の stream エントリ。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestStream {
    /// stream index。
    pub stream_index: u16,
    /// capture stream 識別子（tagged）。
    pub capture_stream_id: String,
    /// lane 識別子（tagged）。
    pub lane_id: String,
    /// チャネル数。
    pub channels: u32,
    /// media file の session 相対 path。
    pub media: String,
    /// journal file の session 相対 path。
    pub journal: String,
    /// finalized marker の session 相対 path。
    pub finalized: String,
}

/// manifest.json（session 単位・作成時に確定）。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagingManifest {
    /// schema version。
    pub schema_version: u32,
    /// session 識別子（tagged）。
    pub session_id: String,
    /// sample rate。
    pub sample_rate: u32,
    /// PCM 形式識別子。
    pub pcm_format: String,
    /// stream 一覧。
    pub streams: Vec<ManifestStream>,
    /// 検出した FS capability。
    pub capabilities: StagingCapabilities,
}

/// `streams/stream-0000` 形式のファイル stem。
#[must_use]
pub fn stream_stem(stream_index: u16) -> String {
    format!("stream-{stream_index:04}")
}

/// media file の session 相対 path。
#[must_use]
pub fn media_rel(stream_index: u16) -> String {
    format!("streams/{}.pcm", stream_stem(stream_index))
}
fn journal_rel(stream_index: u16) -> String {
    format!("streams/{}.journal", stream_stem(stream_index))
}
fn finalized_rel(stream_index: u16) -> String {
    format!("streams/{}.finalized.json", stream_stem(stream_index))
}

/// canonical receipt の session 相対 path。
#[must_use]
pub fn canonical_rel(stream_index: u16) -> String {
    format!("streams/{}.canonical.json", stream_stem(stream_index))
}

/// recovery review 判定記録の session 相対 path。
#[must_use]
pub fn review_rel(stream_index: u16) -> String {
    format!("streams/{}.review.json", stream_stem(stream_index))
}

/// canonical publication receipt（`streams/stream-NNNN.canonical.json`）。
/// recovery 確定済みの raw media を canonical verified-media 形式
/// （decodable container・`media_root/blobs/<digest>` で resolve 可能）
/// へ publish した記録。commit は `digest` を `Source.content_digest`
/// へ採用し、raw media provenance は `raw_digest` で保持する。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalSourceRef {
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
    /// 対象 `FinalizedSourceRef` の raw media fold digest（hex）。
    pub raw_digest: String,
    /// canonical blob の content digest（import と同一 scheme・hex16）。
    pub digest: String,
    /// canonical media の container/format 識別子。
    pub media_format: String,
    /// canonical blob byte 長。
    pub byte_len: u64,
    /// 確定 source frame 総数（`FinalizedSourceRef` と一致必須）。
    pub source_frames: u64,
    /// sample rate。
    pub sample_rate: u32,
    /// チャネル数。
    pub channels: u32,
}

/// open 済みの staging session 領域。
#[derive(Clone, Debug)]
pub struct StagingArea {
    /// staging root。
    pub root: PathBuf,
    /// session dir（`root/<session-tag>`）。
    pub session_dir: PathBuf,
    /// 読み込み/作成した manifest。
    pub manifest: StagingManifest,
}

/// staging 固有のエラー（writer の `WriterError` へ変換される）。
#[derive(Debug)]
pub enum StagingError {
    /// 下位 I/O エラー。
    Io {
        /// 失敗した操作。
        op: &'static str,
        /// 発生元。
        source: io::Error,
    },
    /// manifest の schema/内容不正。
    ManifestInvalid(String),
    /// 指定した session/stream が存在しない。
    NotFound(String),
    /// 既に存在するものを再作成しようとした。
    AlreadyExists(String),
}

impl StagingError {
    fn io(op: &'static str) -> impl FnOnce(io::Error) -> Self {
        move |source| Self::Io { op, source }
    }
}

impl std::fmt::Display for StagingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { op, source } => write!(f, "staging io error ({op}): {source}"),
            Self::ManifestInvalid(m) => write!(f, "invalid staging manifest: {m}"),
            Self::NotFound(m) => write!(f, "staging not found: {m}"),
            Self::AlreadyExists(m) => write!(f, "staging already exists: {m}"),
        }
    }
}

impl std::error::Error for StagingError {}

/// directory sync の capability を検出する（open して sync できれば
/// `Supported`、open 自体が失敗すれば `Unsupported`、その他の失敗は
/// `Failed`）。
#[must_use]
pub fn detect_dir_sync(dir: &Path) -> DirSyncCapability {
    match File::open(dir) {
        Ok(f) => match f.sync_all() {
            Ok(()) => DirSyncCapability::Supported,
            Err(e) if e.kind() == io::ErrorKind::Unsupported => DirSyncCapability::Unsupported,
            Err(_) => DirSyncCapability::Failed,
        },
        Err(e) if e.kind() == io::ErrorKind::Unsupported => DirSyncCapability::Unsupported,
        // Windows では dir の File::open が PermissionDenied/IsADirectory
        // 系のエラーになり得る。sync 保証を主張できないので Unsupported。
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::PermissionDenied | io::ErrorKind::IsADirectory
            ) =>
        {
            DirSyncCapability::Unsupported
        }
        Err(_) => DirSyncCapability::Failed,
    }
}

/// `path` の親 dir へ sync を試みる（capability が `Supported` のとき
/// のみ呼ぶ。失敗は呼び出し側で記録する）。
///
/// # Errors
/// dir の open/sync が失敗した場合の `io::Error`。
pub fn sync_parent_dir(path: &Path) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    File::open(parent)?.sync_all()
}

/// `bytes` を `path` へ tmp→sync→rename で原子公開する。
/// tmp 残留時は既存のものを上書きして進める（idempotent）。
/// fault 注入が必要な場合は [`publish_atomic_faultable`] を使う。
///
/// # Errors
/// open/write/sync/rename のいずれかの失敗。
pub fn publish_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    publish_atomic_faultable(path, bytes, &mut PublishFaults::new())
}

/// [`publish_atomic`] の fault 注入対応版。`faults` の注入種別は
/// [`crate::faultinj::PublishInject`] を参照。`Indeterminate` は
/// rename 成功後に `Err` を返すため、呼出し側は公開成否を file の
/// 再読込で照合する必要がある（応答喪失相当）。
///
/// # Errors
/// open/write/sync/rename の失敗、または注入された失敗。
pub fn publish_atomic_faultable(
    path: &Path,
    bytes: &[u8],
    faults: &mut PublishFaults,
) -> io::Result<()> {
    let call = faults.next_call();
    if faults.denied(call) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "injected publish denied",
        ));
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        if faults.sync_fail(call) {
            return Err(io::Error::other("injected publish sync failure"));
        }
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    if faults.indeterminate(call) {
        return Err(io::Error::other("injected indeterminate publish"));
    }
    Ok(())
}

impl StagingArea {
    /// `<root>/<session-tag>` を作成し、manifest と空の stream file を
    /// 配置する。`root` 自体は無ければ作る。
    ///
    /// manifest が公開済みなら `AlreadyExists`。session dir だけが
    /// 残る場合（作成途中の crash 等）は manifest 未公開の partial
    /// state とみなし、作成を冪等に完了する（既存 file は truncate
    /// しない）。
    ///
    /// # Errors
    /// dir 作成・manifest 書込み・stream file 作成の失敗。
    /// manifest が既に存在する場合 `AlreadyExists`。
    pub fn create(root: &Path, cfg: &StagingSessionConfig) -> Result<Self, StagingError> {
        let session_dir = root.join(cfg.session_id.to_tagged());
        let manifest_path = session_dir.join("manifest.json");
        if manifest_path.exists() {
            return Err(StagingError::AlreadyExists(
                session_dir.display().to_string(),
            ));
        }
        let streams_dir = session_dir.join("streams");
        fs::create_dir_all(&streams_dir).map_err(StagingError::io("create_dir_all"))?;
        let capabilities = StagingCapabilities {
            atomic_rename: true,
            file_sync: true,
            directory_sync: detect_dir_sync(&session_dir),
        };
        let streams: Vec<ManifestStream> = cfg
            .streams
            .iter()
            .map(|s| ManifestStream {
                stream_index: s.stream_index,
                capture_stream_id: s.capture_stream_id.clone(),
                lane_id: s.lane_id.clone(),
                channels: s.channels,
                media: media_rel(s.stream_index),
                journal: journal_rel(s.stream_index),
                finalized: finalized_rel(s.stream_index),
            })
            .collect();
        // 空の stream file を先に配置（manifest を最後に公開）。
        // partial create の retry では file が残っていることがあるので
        // truncate せず create_new にする。
        for s in &streams {
            for rel in [&s.media, &s.journal] {
                match File::create_new(session_dir.join(rel)) {
                    Ok(_) => {}
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(e) => return Err(StagingError::io("create stream")(e)),
                }
            }
        }
        let manifest = StagingManifest {
            schema_version: STAGING_SCHEMA_VERSION,
            session_id: cfg.session_id.to_tagged(),
            sample_rate: cfg.sample_rate,
            pcm_format: PCM_FORMAT.to_string(),
            streams,
            capabilities,
        };
        let bytes = serde_json::to_vec_pretty(&manifest)
            .map_err(|e| StagingError::ManifestInvalid(e.to_string()))?;
        publish_atomic(&manifest_path, &bytes).map_err(StagingError::io("publish manifest"))?;
        let area = Self {
            root: root.to_path_buf(),
            session_dir,
            manifest,
        };
        if area.manifest.capabilities.directory_sync == DirSyncCapability::Supported {
            let _ = area.sync_dir();
        }
        Ok(area)
    }

    /// 既存の session dir を manifest から復元する。
    ///
    /// # Errors
    /// manifest 不在/不正・`NotFound`/`ManifestInvalid`。
    pub fn open(root: &Path, session_id: RecordingSessionId) -> Result<Self, StagingError> {
        let session_dir = root.join(session_id.to_tagged());
        let manifest_path = session_dir.join("manifest.json");
        let bytes = fs::read(&manifest_path)
            .map_err(|_| StagingError::NotFound(manifest_path.display().to_string()))?;
        let manifest: StagingManifest = serde_json::from_slice(&bytes)
            .map_err(|e| StagingError::ManifestInvalid(e.to_string()))?;
        if manifest.schema_version != STAGING_SCHEMA_VERSION {
            return Err(StagingError::ManifestInvalid(format!(
                "schema_version {} unsupported",
                manifest.schema_version
            )));
        }
        if manifest.session_id != session_id.to_tagged() {
            return Err(StagingError::ManifestInvalid(format!(
                "session_id mismatch: {} vs {}",
                manifest.session_id,
                session_id.to_tagged()
            )));
        }
        Ok(Self {
            root: root.to_path_buf(),
            session_dir,
            manifest,
        })
    }

    /// `stream_index` の manifest エントリ。
    #[must_use]
    pub fn stream(&self, stream_index: u16) -> Option<&ManifestStream> {
        self.manifest
            .streams
            .iter()
            .find(|s| s.stream_index == stream_index)
    }

    /// media file の絶対 path。
    #[must_use]
    pub fn media_path(&self, stream_index: u16) -> PathBuf {
        self.session_dir.join(media_rel(stream_index))
    }

    /// journal file の絶対 path。
    #[must_use]
    pub fn journal_path(&self, stream_index: u16) -> PathBuf {
        self.session_dir.join(journal_rel(stream_index))
    }

    /// finalized marker の絶対 path。
    #[must_use]
    pub fn finalized_path(&self, stream_index: u16) -> PathBuf {
        self.session_dir.join(finalized_rel(stream_index))
    }

    /// canonical receipt (`canonical.json`) の絶対 path。
    #[must_use]
    pub fn canonical_path(&self, stream_index: u16) -> PathBuf {
        self.session_dir.join(canonical_rel(stream_index))
    }

    /// recovery review 判定記録の絶対 path。
    #[must_use]
    pub fn review_path(&self, stream_index: u16) -> PathBuf {
        self.session_dir.join(review_rel(stream_index))
    }

    /// `session_dir` 直下の dir sync を試みる（capability が
    /// `Supported` のときのみ効果。結果は呼び出し側で記録する）。
    ///
    /// # Errors
    /// `sync_parent_dir` の失敗をそのまま返す。
    pub fn sync_dir(&self) -> io::Result<()> {
        sync_parent_dir(&self.session_dir.join("manifest.json"))
    }

    /// capture 側 fault（queue 満杯・epoch 断絶等、session 全体を
    /// 中断させた RT/engine 側 fault）を session スコープの durable
    /// marker として記録する。writer 個別の失敗は stream ごとの
    /// journal/finalized ref が担うため、この marker は「capture が
    /// 全 stream で正常終了しなかった」ことだけを示す。
    /// commit 層は marker の存在を全 stream の `recoverable` 分離
    /// として解釈する。
    ///
    /// # Errors
    /// marker 書込みの I/O 失敗。
    pub fn record_capture_fault(&self, detail: &str) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(&serde_json::json!({
            "capture_fault": detail,
        }))?;
        publish_atomic(&self.capture_fault_path(), &bytes)?;
        if self.manifest.capabilities.directory_sync == DirSyncCapability::Supported {
            self.sync_dir()?;
        }
        Ok(())
    }

    /// 記録済み capture fault marker の内容。未記録・読めない場合は
    /// `None`（未記録と読取失敗は同一視しないため、読取失敗は
    /// commit 層側で別途扱う）。
    #[must_use]
    pub fn capture_fault(&self) -> Option<String> {
        let bytes = fs::read(self.capture_fault_path()).ok()?;
        serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()?
            .get("capture_fault")?
            .as_str()
            .map(str::to_string)
    }

    /// capture fault marker が読めない形で存在するか（commit 層は
    /// fail-closed に recoverable 分離する）。
    #[must_use]
    pub fn capture_fault_marker_exists(&self) -> bool {
        self.capture_fault_path().exists()
    }

    /// capture fault marker の絶対 path。
    #[must_use]
    pub fn capture_fault_path(&self) -> PathBuf {
        self.session_dir.join("capture-fault.json")
    }
}

/// `LaneId`/`CaptureStreamId` から `StagingStreamConfig` を作る補助
/// （tagged 文字列で保持し Project 型への依存を manifest に残さない）。
#[must_use]
pub fn stream_config(
    stream_index: u16,
    capture_stream_id: CaptureStreamId,
    lane_id: LaneId,
    channels: u32,
) -> StagingStreamConfig {
    StagingStreamConfig {
        stream_index,
        capture_stream_id: capture_stream_id.to_tagged(),
        lane_id: lane_id.to_tagged(),
        channels,
    }
}
