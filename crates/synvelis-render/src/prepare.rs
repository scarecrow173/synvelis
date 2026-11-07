//! Render-time source preparation — non-RT only. Mirrors
//! [`synvelis_player::resolver::prepare`]'s verified-source gate
//! (state ≥ Verified, asset registered, blob present, digest match,
//! PCM codec) but instead of a prefetch pipeline it opens a
//! [`SourceReader`] per clip and decodes the clip's window span into
//! canonical `f32` planar memory. The render's `ClipRenderer` then
//! serves that memory with no I/O inside `process`.

use std::collections::BTreeMap;
use std::fs::File;

use synvelis_player::import::{blob_path, content_digest_stream};
use synvelis_project::{Project, SourceId};

use ruviform_audio_core::AudioFormat;
use ruviform_exec::CancellationToken;
use ruviform_media_core::{Fact, StreamKind};
use ruviform_media_io::{CodecId, DecodeMode, Demuxer};
use ruviform_media_reader::{SourceFrame, SourceReader};

use synvelis_player::error::ResolveError;
use synvelis_player::lowering::ClipWindow;

/// A verified source opened for decoding: canonical format recorded,
/// codec id captured for the preflight codec check.
pub struct OpenedSource {
    /// Independent reader positioned at frame 0.
    pub reader: SourceReader<'static>,
    /// Canonical `f32` planar format of this source.
    pub canonical: AudioFormat,
    /// Codec of the selected audio stream.
    pub codec: CodecId,
    /// Container-declared frame count (`Fact`-classified).
    pub declared_frames: Fact<u64>,
    /// Verified content digest backing this resolution.
    pub digest: String,
}

/// Opens one verified source for canonical decoding — the same
/// verification sequence [`synvelis_player::resolver::prepare`]
/// performs, minus the prefetch endpoints.
///
/// # Errors
/// [`ResolveError`] — typed per failure class.
pub fn open_source(
    project: &Project,
    source_id: SourceId,
    media_root: &std::path::Path,
) -> Result<OpenedSource, ResolveError> {
    let source = project
        .sources
        .get(&source_id)
        .ok_or(ResolveError::UnresolvedSource { source_id })?;
    if source.state < synvelis_project::SourceState::Verified {
        return Err(ResolveError::UnverifiedSource {
            source_id,
            state: format!("{:?}", source.state),
        });
    }
    let asset_id = source.asset_id;
    if !project.assets.contains_key(&asset_id) {
        return Err(ResolveError::MissingAsset { asset_id });
    }
    let digest = source.content_digest.clone();
    let path = blob_path(media_root, &digest);
    // One fd serves digest *and* decode — a path swap between the two
    // cannot put unverified bytes under a verified digest.
    let mut file = File::open(&path).map_err(|e| {
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
    let actual = content_digest_stream(&mut file).map_err(|e| ResolveError::Io {
        detail: e.to_string(),
    })?;
    if actual != digest {
        return Err(ResolveError::HashMismatch {
            expected: digest,
            actual,
        });
    }
    let demuxer = ruviform_io_symphonia::open_demuxer(file).map_err(|e| ResolveError::Corrupt {
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
    let codec = stream.codec.codec;
    // The codec gate runs before the reader is built so every non-PCM
    // source fails identically — never `Corrupt` just because the
    // backend happens to lack a decoder for it.
    if codec != CodecId::Pcm {
        return Err(ResolveError::UnsupportedCodec {
            source_id,
            detail: format!("codec {codec:?} is not PCM"),
        });
    }
    let reader = SourceReader::open(Box::new(demuxer), stream.stream.id, DecodeMode::Canonical)
        .map_err(|e| ResolveError::Corrupt {
            detail: e.to_string(),
        })?;
    let canonical = reader.metadata().canonical;
    let declared = reader.metadata().frame_count;
    Ok(OpenedSource {
        reader,
        canonical,
        codec,
        declared_frames: declared,
        digest,
    })
}

/// Opens every source the project's non-muted clips and active comp
/// bindings reference — one reader per source, shared across nodes on
/// that source. `cancel` is checked per source: prepare is the longest
/// pre-render phase and must not wait for the first physical call to
/// honor it.
///
/// # Errors
/// [`ResolveError`] — `Cancelled` when the token trips, otherwise the
/// first failing source's error.
pub fn open_sources(
    project: &Project,
    media_root: &std::path::Path,
    cancel: &CancellationToken,
) -> Result<BTreeMap<SourceId, OpenedSource>, ResolveError> {
    let mut needed: Vec<SourceId> = project
        .clips
        .values()
        .filter(|clip| !clip.muted)
        .map(|clip| clip.source_id)
        .collect();
    // Comp bindings read the same verified sources through the same
    // digest-verified boundary. A comp that fails to project here also
    // fails at lowering, which reports it — prepare opens nothing for
    // it and the render refuses there.
    for group in project.comp.take_groups.values() {
        let Some(comp_id) = group.active_comp_id else {
            continue;
        };
        let Ok(effective) = synvelis_project::project_comp(project, comp_id) else {
            continue;
        };
        for binding in effective
            .segments
            .iter()
            .filter_map(|s| s.binding.as_ref())
            .chain(
                effective
                    .transitions
                    .iter()
                    .flat_map(|t| [t.left.as_ref(), t.right.as_ref()])
                    .flatten(),
            )
        {
            needed.push(binding.source_id);
        }
    }
    let mut opened = BTreeMap::new();
    for source_id in needed {
        if cancel.is_cancelled() {
            return Err(ResolveError::Cancelled);
        }
        if opened.contains_key(&source_id) {
            continue;
        }
        opened.insert(source_id, open_source(project, source_id, media_root)?);
    }
    Ok(opened)
}

/// Decoded canonical PCM for the part of a clip window the render can
/// actually emit: `planes[c][i]` is source frame
/// `window.source_start + base + i`, channel `c` — equivalently
/// engine-window-local frame `base + i`.
pub struct DecodedWindow {
    /// Planar canonical samples covering `[base, base + planes.len())`
    /// of the window's local span.
    pub planes: Vec<Vec<f32>>,
    /// Window-local frame where `planes[0]` starts.
    pub base: u64,
    /// Frames actually decoded (`< planes.len()` at source EOF — the
    /// tail stays silent).
    pub frames: usize,
}

/// Seeks `reader` to the window span the render actually processes and
/// decodes it into planar memory. `needed = [play_pos, bound_end)` is
/// the span of engine positions the sequential drive will touch —
/// source content outside `needed ∩ window` is never emitted, so it
/// is never decoded: a long clip under a short render range does not
/// inflate memory beyond the rendered span.
///
/// In `Canonical` mode the reader itself discards forward to the
/// requested frame, so a `landed <= target` receipt leaves the
/// position *at* the target — only `landed > target` needs handling:
/// the covered head stays silent (the reader reports the gap rather
/// than pretending), and decoding starts at `landed`. `cancel` is
/// checked every chunk.
///
/// # Errors
/// [`ResolveError`] — `Cancelled` when the token trips, otherwise
/// seek/decode failures.
pub fn decode_window(
    reader: &mut SourceReader<'_>,
    canonical: AudioFormat,
    window: ClipWindow,
    needed: std::ops::Range<u64>,
    cancel: &CancellationToken,
) -> Result<DecodedWindow, ResolveError> {
    let channels = usize::try_from(canonical.layout.channels()).unwrap_or(usize::MAX);
    // Window-local span intersected with the processed span.
    let lo = needed
        .start
        .saturating_sub(window.engine_start)
        .min(window.engine_len);
    let hi = needed
        .end
        .saturating_sub(window.engine_start)
        .min(window.engine_len);
    let want_u64 = hi.saturating_sub(lo);
    let want = usize::try_from(want_u64).unwrap_or(usize::MAX);
    let base = lo;
    if want == 0 {
        return Ok(DecodedWindow {
            planes: vec![Vec::new(); channels],
            base,
            frames: 0,
        });
    }
    let target = window.source_start.saturating_add(lo);
    let landed = reader
        .seek_source_frame(SourceFrame(target))
        .map_err(|e| ResolveError::Corrupt {
            detail: format!("seek failed: {e}"),
        })?
        .landed;
    let mut planes: Vec<Vec<f32>> = vec![vec![0.0; want]; channels];
    let mut decoded = 0usize;
    if landed.0 > target {
        // Landing past the target: the covered head of the window stays
        // silent — decode only the frames still inside it.
        decoded = usize::try_from((landed.0 - target).min(want_u64))
            .unwrap_or(usize::MAX)
            .min(want);
    }
    while decoded < want {
        if cancel.is_cancelled() {
            return Err(ResolveError::Cancelled);
        }
        let take = (want - decoded).min(65_536);
        let mut views: Vec<&mut [f32]> = planes
            .iter_mut()
            .map(|p| &mut p[decoded..decoded + take])
            .collect();
        let outcome = reader
            .read_planar(&mut views, take)
            .map_err(|e| ResolveError::Corrupt {
                detail: format!("decode failed: {e}"),
            })?;
        decoded += outcome.frames;
        if outcome.eof || outcome.frames == 0 {
            break;
        }
    }
    Ok(DecodedWindow {
        planes,
        base,
        frames: decoded,
    })
}
