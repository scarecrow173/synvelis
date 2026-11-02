//! Verified resolver: stable `SourceId` → applied asset → verified
//! blob → `SourceReader` + bounded `Prefetch`. Everything here is
//! non-RT preparation; the returned consumer is the only RT endpoint.
//!
//! Failures are typed: corrupt bytes, hash mismatch, unsupported
//! codec, unresolved source and missing media are distinct variants —
//! a pre-import path or an unverified source can never resolve.

use std::fs::{self, File};
use std::path::Path;

use synvelis_project::{AssetId, Project, SourceId, SourceState};

use ruviform_audio_core::AudioFormat;
use ruviform_media_core::{Fact, StreamKind};
use ruviform_media_io::{CodecId, DecodeMode, Demuxer};
use ruviform_media_reader::{
    Prefetch, PrefetchConfig, PrefetchConsumer, PrefetchHandle, PrefetchProducer, SourceReader,
};

use crate::error::ResolveError;
use crate::import::{blob_path, content_digest};

/// A fully prepared clip playback path: reader inside the producer,
/// consumer ready for the RT side, canonical format recorded.
pub struct PreparedClip {
    /// Non-RT control surface (seek/reopen/telemetry).
    pub handle: PrefetchHandle,
    /// Non-RT decode worker — the driver pumps it deterministically.
    pub producer: PrefetchProducer,
    /// RT fill endpoint — installed inside the clip processor.
    pub consumer: PrefetchConsumer,
    /// Canonical `f32` planar format of this source.
    pub canonical: AudioFormat,
    /// Container-declared frame count (`Fact`-classified).
    pub declared_frames: Fact<u64>,
    /// Verified content digest backing this resolution.
    pub digest: String,
}

/// Resolves a registered source into a prepared reader+prefetch.
///
/// Enforces the state gate: a source below
/// [`SourceState::Verified`] (e.g. a bare `RegisterSource` at
/// `Imported`) is [`ResolveError::UnverifiedSource`] — never playable.
///
/// # Errors
/// [`ResolveError`] — typed per failure class.
pub fn prepare(
    project: &Project,
    source_id: SourceId,
    media_root: &Path,
    config: &PrefetchConfig,
) -> Result<PreparedClip, ResolveError> {
    let source = project
        .sources
        .get(&source_id)
        .ok_or(ResolveError::UnresolvedSource { source_id })?;
    if source.state < SourceState::Verified {
        return Err(ResolveError::UnverifiedSource {
            source_id,
            state: format!("{:?}", source.state),
        });
    }
    let asset_id: AssetId = source.asset_id;
    if !project.assets.contains_key(&asset_id) {
        return Err(ResolveError::MissingAsset { asset_id });
    }
    let digest = source.content_digest.clone();
    let path = blob_path(media_root, &digest);
    let bytes = fs::read(&path).map_err(|e| {
        if path.exists() {
            ResolveError::Io {
                detail: e.to_string(),
            }
        } else {
            ResolveError::MissingMedia {
                digest: digest.clone(),
            }
        }
    })?;
    let actual = content_digest(&bytes);
    if actual != digest {
        return Err(ResolveError::HashMismatch {
            expected: digest,
            actual,
        });
    }
    let demuxer =
        ruviform_io_symphonia::open_demuxer(File::open(&path).map_err(|e| ResolveError::Io {
            detail: e.to_string(),
        })?)
        .map_err(|e| ResolveError::Corrupt {
            detail: e.to_string(),
        })?;
    let stream = demuxer
        .streams()
        .iter()
        .find(|s| s.stream.kind == StreamKind::Audio)
        .ok_or_else(|| ResolveError::UnsupportedCodec {
            detail: "no audio stream".into(),
        })?
        .clone();
    if stream.codec.codec != CodecId::Pcm {
        return Err(ResolveError::UnsupportedCodec {
            detail: format!("codec {:?} is not PCM", stream.codec.codec),
        });
    }
    let reader = SourceReader::open(Box::new(demuxer), stream.stream.id, DecodeMode::Canonical)
        .map_err(|e| ResolveError::Corrupt {
            detail: e.to_string(),
        })?;
    let canonical = reader.metadata().canonical;
    let declared = reader.metadata().frame_count;
    let (handle, producer, consumer) =
        Prefetch::manual(reader, *config).map_err(|e| ResolveError::Io {
            detail: e.to_string(),
        })?;
    Ok(PreparedClip {
        handle,
        producer,
        consumer,
        canonical,
        declared_frames: declared,
        digest,
    })
}
