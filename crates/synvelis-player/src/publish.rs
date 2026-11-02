//! Finalized recording media → canonical verified-media publication.
//!
//! Non-RT step between VS-C recovery finalize and VS-D `commit_session`:
//! for each `FinalizedOutcome::Complete` stream, the durable frontier of
//! the raw `f32le-interleaved` media is encoded to a PCM f32 WAV,
//! digest-addressed into `media_root/blobs/<digest>` via the same
//! [`content_digest`] scheme as [`crate::import`], and a
//! `canonical.json` receipt is atomically published into the session
//! staging dir. `commit_session` then registers the canonical digest as
//! `Source.content_digest` (see `synvelis_recording::commit`).
//!
//! **Finalized-only:** partial/orphan/WIP media is never published —
//! streams without a `Complete` finalized ref land in
//! [`PublishReport::skipped`]. This module does not touch playback or
//! render code: published blobs are indistinguishable from imported
//! ones and resolve through the exact same verified-source boundary.

use std::fmt;
use std::fs;
use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use ruviform_audio_core::{AudioBuffer, BufferSpec, ChannelLayout, SampleFormat, SampleRate};
use ruviform_io_wav::{WavOutputSpec, make_encoder, open_muxer};
use ruviform_media_core::{MediaTime, MediaTimestamp};
use ruviform_media_io::{AudioFrame, Encoder, Frame, Muxer};
use synvelis_recording::load_finalized;
use synvelis_recording::staging::{
    CANONICAL_MEDIA_FORMAT, CANONICAL_SCHEMA_VERSION, CanonicalSourceRef, StagingArea,
    publish_atomic,
};
use synvelis_recording::writer::FinalizedOutcome;

use crate::import::{blob_path, content_digest, content_digest_stream};

/// One stream whose canonical media was published (or re-materialized).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishedStream {
    /// stream index (`plan.targets` の添字)。
    pub stream_index: u16,
    /// canonical blob の content digest。
    pub digest: String,
    /// 対象 `FinalizedSourceRef` の raw media fold digest。
    pub raw_digest: String,
    /// 公開先 blob path。
    pub blob: PathBuf,
    /// canonical byte 長。
    pub byte_len: u64,
    /// 確定 source frame 総数。
    pub source_frames: u64,
}

/// publish 対象外となった stream と理由。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedStream {
    /// stream index。
    pub stream_index: u16,
    /// 分離理由（`missing-finalized` / `partial-finalize` /
    /// `finalized-unreadable` / `capture-fault` 等）。
    pub reason: String,
}

/// `publish_recorded` の結果分類。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PublishReport {
    /// 新規に canonical media + receipt を公開した stream。
    pub published: Vec<PublishedStream>,
    /// 既存 receipt と整合し再公開が不要だった stream
    /// （blob 欠損時は blob のみ再公開してここに入る）。
    pub reused: Vec<PublishedStream>,
    /// publish 対象外（Complete でない・fault 分離）stream。
    pub skipped: Vec<SkippedStream>,
}

/// `publish_recorded` の失敗分類。全て fail-closed — 部分 publish は
/// atomic publish 単位でのみ起こり、中途半端な receipt を残さない。
#[derive(Debug)]
#[non_exhaustive]
pub enum PublishError {
    /// staging/media/blob/receipt の I/O 失敗。
    Io {
        /// 失敗した操作。
        op: &'static str,
        /// 診断。
        detail: String,
    },
    /// finalized ref / media bytes / receipt の整合破損
    /// （frontier 不足・frame 数不一致・receipt 破損）。
    CorruptMedia {
        /// stream index。
        stream_index: u16,
        /// 診断。
        detail: String,
    },
    /// staging manifest と finalized ref の identity 不一致、
    /// または既存 canonical receipt が別 media を指す。
    /// silent な付替えを行わず報告する。
    Conflict {
        /// stream index。
        stream_index: u16,
        /// 診断。
        detail: String,
    },
    /// canonical WAV の speaker-mask へ対応づけられない channel 数
    /// （capability gap — Mono/Stereo のみ対応）。
    UnsupportedChannels {
        /// stream index。
        stream_index: u16,
        /// 要求された channel 数。
        channels: u32,
    },
    /// WAV encode/mux の失敗。
    Encode {
        /// stream index。
        stream_index: u16,
        /// 診断。
        detail: String,
    },
}

impl fmt::Display for PublishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { op, detail } => write!(f, "publish I/O failed ({op}): {detail}"),
            Self::CorruptMedia {
                stream_index,
                detail,
            } => write!(f, "stream {stream_index}: corrupt media/receipt: {detail}"),
            Self::Conflict {
                stream_index,
                detail,
            } => write!(f, "stream {stream_index}: identity conflict: {detail}"),
            Self::UnsupportedChannels {
                stream_index,
                channels,
            } => write!(f, "stream {stream_index}: {channels} channels unsupported"),
            Self::Encode {
                stream_index,
                detail,
            } => write!(f, "stream {stream_index}: wav encode failed: {detail}"),
        }
    }
}

impl std::error::Error for PublishError {}

fn io_err(op: &'static str) -> impl FnOnce(std::io::Error) -> PublishError {
    move |e| PublishError::Io {
        op,
        detail: e.to_string(),
    }
}

/// raw media (f32le interleaved) の durable frontier を canonical
/// PCM f32 WAV bytes へ encode する。
fn encode_canonical_wav(
    stream_index: u16,
    r: &synvelis_recording::FinalizedSourceRef,
    raw: &[u8],
) -> Result<Vec<u8>, PublishError> {
    let channels = usize::try_from(r.channels).unwrap_or(usize::MAX);
    let frame_bytes = channels.saturating_mul(4);
    let want = usize::try_from(r.media_byte_len).unwrap_or(usize::MAX);
    let frames = usize::try_from(r.source_frames).unwrap_or(usize::MAX);
    if frame_bytes == 0 || want != frames.saturating_mul(frame_bytes) {
        return Err(PublishError::CorruptMedia {
            stream_index,
            detail: format!(
                "frontier {} bytes != {} frames * {} frame-bytes",
                r.media_byte_len, r.source_frames, frame_bytes
            ),
        });
    }
    if raw.len() < want {
        return Err(PublishError::CorruptMedia {
            stream_index,
            detail: format!("media holds {} bytes, frontier needs {want}", raw.len()),
        });
    }
    let layout = match r.channels {
        1 => ChannelLayout::Mono,
        2 => ChannelLayout::Stereo,
        n => {
            return Err(PublishError::UnsupportedChannels {
                stream_index,
                channels: n,
            });
        }
    };
    let rate = SampleRate::new(r.sample_rate).map_err(|e| PublishError::CorruptMedia {
        stream_index,
        detail: format!("bad sample rate {}: {e}", r.sample_rate),
    })?;
    // frontier 以降の未検証 tail は publish しない。
    let frontier = &raw[..want];
    let samples: Vec<f32> = frontier
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect();
    let buffer = AudioBuffer::from_interleaved(
        &samples,
        BufferSpec {
            layout,
            frames,
            sample_rate: rate,
        },
    )
    .map_err(|e| PublishError::CorruptMedia {
        stream_index,
        detail: format!("buffer: {e}"),
    })?;
    let info = WavOutputSpec {
        sample_rate: rate,
        layout,
        sample_format: SampleFormat::F32,
    }
    .stream_info();
    let mut encoder = make_encoder(&info).map_err(|e| PublishError::Encode {
        stream_index,
        detail: e.to_string(),
    })?;
    let pts = MediaTimestamp::carried(MediaTime::new(0, rate.time_base()));
    encoder
        .send_frame(Frame::Audio(AudioFrame::new(buffer, pts)))
        .map_err(|e| PublishError::Encode {
            stream_index,
            detail: e.to_string(),
        })?;
    encoder.drain().map_err(|e| PublishError::Encode {
        stream_index,
        detail: e.to_string(),
    })?;
    let mut muxer =
        open_muxer(Cursor::new(Vec::new()), std::slice::from_ref(&info)).map_err(|e| {
            PublishError::Encode {
                stream_index,
                detail: e.to_string(),
            }
        })?;
    while let Some(packet) = encoder.receive_packet().map_err(|e| PublishError::Encode {
        stream_index,
        detail: e.to_string(),
    })? {
        muxer
            .write_packet(packet)
            .map_err(|e| PublishError::Encode {
                stream_index,
                detail: e.to_string(),
            })?;
    }
    muxer.finish().map_err(|e| PublishError::Encode {
        stream_index,
        detail: e.to_string(),
    })?;
    Ok(muxer.into_sink().into_inner())
}

/// receipt と finalized ref の整合（commit の adopt rule と同一）。
fn receipt_matches(r: &synvelis_recording::FinalizedSourceRef, can: &CanonicalSourceRef) -> bool {
    can.schema_version == CANONICAL_SCHEMA_VERSION
        && can.media_format == CANONICAL_MEDIA_FORMAT
        && can.session_id == r.session_id
        && can.stream_index == r.stream_index
        && can.capture_stream_id == r.capture_stream_id
        && can.lane_id == r.lane_id
        && can.raw_digest == r.digest
        && can.source_frames == r.source_frames
        && can.sample_rate == r.sample_rate
        && can.channels == r.channels
}

fn read_receipt(
    area: &StagingArea,
    stream_index: u16,
) -> Result<Option<CanonicalSourceRef>, PublishError> {
    let path = area.canonical_path(stream_index);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(io_err("read canonical receipt"))?;
    let can = serde_json::from_slice::<CanonicalSourceRef>(&bytes).map_err(|e| {
        PublishError::CorruptMedia {
            stream_index,
            detail: format!("canonical receipt decode: {e}"),
        }
    })?;
    Ok(Some(can))
}

/// digest-addressed blob を確保する。既存 blob は内容 digest を再検証し
/// （存在確認だけで済ませない）、不一致は fail-closed で拒否する。
/// 不在時のみ `make_wav` を呼んで encode→原子公開する。
fn ensure_blob(
    media_root: &Path,
    stream_index: u16,
    digest: &str,
    make_wav: impl FnOnce() -> Result<Vec<u8>, PublishError>,
) -> Result<PathBuf, PublishError> {
    let path = blob_path(media_root, digest);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io_err("create blobs dir"))?;
    }
    if path.exists() {
        let mut file = File::open(&path).map_err(io_err("open blob"))?;
        let actual = content_digest_stream(&mut file).map_err(io_err("digest blob"))?;
        if actual == digest {
            return Ok(path);
        }
        // path 名 = 内容 digest なのに中身が違う = 破損。
        // 別内容で上書きして偽の identity を作らない。
        return Err(PublishError::CorruptMedia {
            stream_index,
            detail: format!("blob {} exists with different content", path.display()),
        });
    }
    let wav = make_wav()?;
    publish_atomic(&path, &wav).map_err(io_err("publish blob"))?;
    Ok(path)
}

/// Finalized (Complete) な録音 media を canonical verified-media へ
/// publish する。`finalize` 後・`commit_session` 前に呼ぶこと。
///
/// 冪等: 整合 receipt が残る stream は reused、receipt が無い/欠落
/// blob のみ再公開する。session スコープの capture-fault marker が
/// 立つ session は全 stream を skipped へ分離する（commit と同じ
/// fail-closed 境界）。
///
/// # Errors
/// [`PublishError`] — I/O・整合破損・identity conflict・encode 失敗。
/// skipped は Err でなく report へ分類される。
pub fn publish_recorded(
    area: &StagingArea,
    media_root: &Path,
) -> Result<PublishReport, PublishError> {
    let fins = load_finalized(area);
    let capture_fault = if area.capture_fault_marker_exists() {
        Some(
            area.capture_fault()
                .unwrap_or_else(|| "unreadable capture-fault marker".to_string()),
        )
    } else {
        None
    };
    let mut report = PublishReport::default();
    for ms in &area.manifest.streams {
        if let Some(detail) = &capture_fault {
            report.skipped.push(SkippedStream {
                stream_index: ms.stream_index,
                reason: format!("capture-fault:{detail}"),
            });
            continue;
        }
        let Some(fin) = fins.iter().find(|f| f.stream_index == ms.stream_index) else {
            continue;
        };
        let r = match (&fin.finalized, &fin.error) {
            (Some(r), _) if r.outcome == FinalizedOutcome::Complete => r,
            (Some(_), _) => {
                report.skipped.push(SkippedStream {
                    stream_index: ms.stream_index,
                    reason: "partial-finalize".to_string(),
                });
                continue;
            }
            (None, Some(e)) => {
                report.skipped.push(SkippedStream {
                    stream_index: ms.stream_index,
                    reason: format!("finalized-unreadable:{e}"),
                });
                continue;
            }
            (None, None) => {
                report.skipped.push(SkippedStream {
                    stream_index: ms.stream_index,
                    reason: "missing-finalized".to_string(),
                });
                continue;
            }
        };
        match publish_stream(area, media_root, ms, r)? {
            StreamAction::Reused(s) => report.reused.push(s),
            StreamAction::Published(s) => report.published.push(s),
        }
    }
    Ok(report)
}

/// 検証済み stream 1 本の publish 判定。
enum StreamAction {
    /// receipt 整合・blob 確保済み（blob のみ再公開を含む）。
    Reused(PublishedStream),
    /// canonical media + receipt を新規公開した。
    Published(PublishedStream),
}

/// 1 stream 分: manifest/ref identity 照合 → receipt 照合 →
/// canonical WAV encode → blob/receipt の atomic publish。
fn publish_stream(
    area: &StagingArea,
    media_root: &Path,
    ms: &synvelis_recording::staging::ManifestStream,
    r: &synvelis_recording::FinalizedSourceRef,
) -> Result<StreamAction, PublishError> {
    // staging manifest と ref の identity 整合（build_payload と
    // 同一 rule — 不一致は fail-closed で全体失敗）。
    if r.capture_stream_id != ms.capture_stream_id
        || r.lane_id != ms.lane_id
        || r.session_id != area.manifest.session_id
        || r.stream_index != ms.stream_index
        || r.channels != ms.channels
        || r.sample_rate != area.manifest.sample_rate
        || r.media != ms.media
    {
        return Err(PublishError::Conflict {
            stream_index: ms.stream_index,
            detail: "finalized ref identity mismatch with staging manifest".into(),
        });
    }
    match read_receipt(area, ms.stream_index)? {
        Some(can) if receipt_matches(r, &can) => {
            // receipt 整合 → blob のみ確保（欠損/未公開なら再公開）。
            let digest = can.digest.clone();
            let blob = ensure_blob(media_root, ms.stream_index, &can.digest, || {
                let raw =
                    fs::read(area.media_path(ms.stream_index)).map_err(io_err("read media"))?;
                encode_canonical_wav(ms.stream_index, r, &raw)
            })?;
            Ok(StreamAction::Reused(PublishedStream {
                stream_index: ms.stream_index,
                digest,
                raw_digest: can.raw_digest,
                blob,
                byte_len: can.byte_len,
                source_frames: can.source_frames,
            }))
        }
        Some(can) => Err(PublishError::Conflict {
            stream_index: ms.stream_index,
            detail: format!(
                "canonical receipt names different media (raw {} vs ref {})",
                can.raw_digest, r.digest
            ),
        }),
        None => {
            let raw = fs::read(area.media_path(ms.stream_index)).map_err(io_err("read media"))?;
            let wav = encode_canonical_wav(ms.stream_index, r, &raw)?;
            let digest = content_digest(&wav);
            let byte_len = wav.len() as u64;
            let blob = ensure_blob(media_root, ms.stream_index, &digest, || Ok(wav))?;
            let can = CanonicalSourceRef {
                schema_version: CANONICAL_SCHEMA_VERSION,
                session_id: r.session_id.clone(),
                stream_index: ms.stream_index,
                capture_stream_id: r.capture_stream_id.clone(),
                lane_id: r.lane_id.clone(),
                raw_digest: r.digest.clone(),
                digest: digest.clone(),
                media_format: CANONICAL_MEDIA_FORMAT.to_string(),
                byte_len,
                source_frames: r.source_frames,
                sample_rate: r.sample_rate,
                channels: r.channels,
            };
            let receipt_bytes =
                serde_json::to_vec_pretty(&can).map_err(|e| PublishError::Encode {
                    stream_index: ms.stream_index,
                    detail: format!("receipt encode: {e}"),
                })?;
            let receipt_path = area.canonical_path(ms.stream_index);
            if receipt_path.exists() {
                fs::remove_file(&receipt_path).map_err(io_err("remove stale receipt"))?;
            }
            publish_atomic(&receipt_path, &receipt_bytes)
                .map_err(io_err("publish canonical receipt"))?;
            // rename を crash で失わないよう session dir も sync する
            // （capabilities 反映は呼び出し側の方針 — ここは best-effort
            // で失敗しない）。
            let _ = area.sync_dir();
            Ok(StreamAction::Published(PublishedStream {
                stream_index: ms.stream_index,
                digest,
                raw_digest: r.digest.clone(),
                blob,
                byte_len,
                source_frames: r.source_frames,
            }))
        }
    }
}
