//! Verified resolver: stable `SourceId` → applied asset → verified
//! blob → `SourceReader` + bounded `Prefetch`. Everything here is
//! non-RT preparation; the returned consumer is the only RT endpoint.
//!
//! Failures are typed: corrupt bytes, hash mismatch, unsupported
//! codec, unresolved source and missing media are distinct variants —
//! a pre-import path or an unverified source can never resolve.

use std::path::Path;

use synvelis_project::{AssetId, Project, SourceId, SourceState};

use ruviform_audio_core::AudioFormat;
use ruviform_media_core::{Fact, StreamKind};
use ruviform_media_io::{CodecId, DecodeMode, Demuxer};
use ruviform_media_reader::{
    Prefetch, PrefetchConfig, PrefetchConsumer, PrefetchHandle, PrefetchProducer, SourceReader,
};

use crate::blob::open_blob_verified;
use crate::error::ResolveError;

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

/// Runs the full verified-open chain for `source_id` — state gate,
/// asset check, blob read + digest verify, demuxer open, audio-stream
/// and PCM-codec checks, and `SourceReader` open in canonical decode
/// mode — returning the reader plus its canonical format, declared
/// frame count, and verified digest. Shared by [`prepare`] and
/// [`canonical_format`] so both observe identical verification.
fn open_verified(
    project: &Project,
    source_id: SourceId,
    media_root: &Path,
) -> Result<(SourceReader<'static>, AudioFormat, Fact<u64>, String), ResolveError> {
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
    let blob = open_blob_verified(media_root, &digest)?;
    let demuxer =
        ruviform_io_symphonia::open_demuxer(blob.file).map_err(|e| ResolveError::Corrupt {
            detail: e.to_string(),
        })?;
    let stream = demuxer
        .streams()
        .iter()
        .find(|s| s.stream.kind == StreamKind::Audio)
        .ok_or_else(|| ResolveError::UnsupportedCodec {
            source_id,
            detail: "no audio stream".into(),
        })?
        .clone();
    if stream.codec.codec != CodecId::Pcm {
        return Err(ResolveError::UnsupportedCodec {
            source_id,
            detail: format!("codec {:?} is not PCM", stream.codec.codec),
        });
    }
    let reader = SourceReader::open(Box::new(demuxer), stream.stream.id, DecodeMode::Canonical)
        .map_err(|e| ResolveError::Corrupt {
            detail: e.to_string(),
        })?;
    let canonical = reader.metadata().canonical;
    let declared = reader.metadata().frame_count;
    Ok((reader, canonical, declared, digest))
}

/// Probes the canonical `f32` planar [`AudioFormat`] of a registered
/// source through the same verified-open chain as [`prepare`] —
/// without acquiring prefetch resources. Used by session-layer
/// routing adapters to supply `canonical_of` to playback planning.
///
/// # Errors
/// [`ResolveError`] — identical failure classes to [`prepare`].
pub fn canonical_format(
    project: &Project,
    source_id: SourceId,
    media_root: &Path,
) -> Result<AudioFormat, ResolveError> {
    let (_, canonical, _, _) = open_verified(project, source_id, media_root)?;
    Ok(canonical)
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
    let (reader, canonical, declared, digest) = open_verified(project, source_id, media_root)?;
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
