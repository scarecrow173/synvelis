//! Preflight: every refusal that must fire *before* output begins —
//! range/tail/destination/capability checks, then per-clip source
//! registration, verification state and codec/rate checks over the
//! resolved canonical formats. Nothing here writes a byte; a failure
//! leaves the destination and the staged temp untouched.

use std::collections::BTreeMap;

use synvelis_project::{SourceId, SourceState};

use crate::error::PreflightError;
use crate::prepare::OpenedSource;
use crate::snapshot::RenderSnapshot;
use crate::spec::{ExportRequest, TailPolicy};
use ruviform_audio_core::AudioFormat;

/// Static checks needing no decoder: range geometry, tail
/// classification, destination policy and backend capability.
///
/// # Errors
/// The first [`PreflightError`] encountered, in the contract order.
pub fn preflight_static(
    snapshot: &RenderSnapshot,
    request: &ExportRequest,
) -> Result<(), PreflightError> {
    // Request magnitudes must stay inside what the pipeline can
    // honestly allocate — an unbounded block or delay line aborts the
    // allocator rather than producing a typed failure. `1<<20` frames
    // matches the render compiler's own delay bound.
    const MAX_FRAMES: u64 = 1 << 20;
    if request.engine.sample_rate == 0 || request.range_end <= request.range_start {
        return Err(PreflightError::InvalidRange {
            detail: "range must be non-empty at a non-zero engine rate".into(),
        });
    }
    if request.tail.is_none() {
        return Err(PreflightError::UnknownTail);
    }
    if request.destination.exists() && !request.replace_existing {
        return Err(PreflightError::DestinationConflict {
            path: request.destination.clone(),
        });
    }
    let caps = ruviform_io_wav::wav_capabilities();
    if !caps.sample_formats.contains(&request.wire_format) {
        return Err(PreflightError::InsufficientCapability {
            detail: format!("wire format {:?} is not WAV-encodable", request.wire_format),
        });
    }
    let channels = usize::from(request.engine.channels);
    // The L2 slice ships mono/stereo masters — anything else is an
    // unsupported output shape, not a silent stereo coercion.
    if !(1..=2).contains(&channels) {
        return Err(PreflightError::InsufficientCapability {
            detail: format!("{channels}-channel master unsupported"),
        });
    }
    if request.max_block == 0 || u64::from(request.max_block) > MAX_FRAMES {
        return Err(PreflightError::InvalidRange {
            detail: format!("max_block {} out of 1..={MAX_FRAMES}", request.max_block),
        });
    }
    let tail_carried: u64 = request
        .inserts
        .iter()
        .map(crate::spec::Insert::tail_frames)
        .sum();
    if let Some(ins) = request
        .inserts
        .iter()
        .find(|i| i.tail_frames().max(i.declared_latency()) > MAX_FRAMES)
    {
        return Err(PreflightError::InvalidRange {
            detail: format!("insert {ins:?} exceeds the {MAX_FRAMES}-frame bound"),
        });
    }
    if tail_carried > 0 && request.tail == Some(TailPolicy::None) {
        return Err(PreflightError::InvalidRange {
            detail: format!(
                "insert chain carries a {tail_carried}-frame tail; \
                 TailPolicy::None would drop it — declare Known or Estimated"
            ),
        });
    }
    let frames = request
        .range_end
        .saturating_sub(request.range_start)
        .saturating_add(request.tail.map_or(0, TailPolicy::frames));
    let bytes = frames
        .saturating_mul(channels.max(1) as u64)
        .saturating_mul(u64::from(request.wire_format.bits_per_sample() / 8));
    if bytes > caps.max_data_bytes {
        return Err(PreflightError::InsufficientCapability {
            detail: format!("emitted data would exceed the RIFF limit ({bytes} B)"),
        });
    }
    // Source registration + verification gates — Missing Source and
    // Unverified Source are preflight failures, never silent skips.
    for clip in snapshot.project.clips.values() {
        if clip.muted {
            continue;
        }
        let source =
            snapshot
                .project
                .sources
                .get(&clip.source_id)
                .ok_or(PreflightError::MissingSource {
                    clip_id: clip.id,
                    source_id: clip.source_id,
                })?;
        if source.state < SourceState::Verified {
            return Err(PreflightError::UnverifiedSource {
                source_id: clip.source_id,
                state: format!("{:?}", source.state),
            });
        }
        if !snapshot.project.assets.contains_key(&source.asset_id) {
            return Err(PreflightError::MissingAsset {
                asset_id: source.asset_id,
            });
        }
    }
    Ok(())
}

/// Canonical-format checks: the source rate must equal the engine
/// rate — implicit resampling is forbidden, and a mismatched source is
/// a preflight failure, not a skipped clip. The PCM codec gate runs
/// earlier, inside [`crate::prepare::open_sources`] before the reader
/// is built, and [`crate::export`] maps it to
/// [`PreflightError::UnsupportedCodec`] — non-PCM sources can never
/// reach this pass.
///
/// `opened` maps every source that [`crate::prepare::open_sources`]
/// verified and opened; clips referencing sources outside it were
/// already rejected by [`preflight_static`].
///
/// # Errors
/// [`PreflightError::UnsupportedCodec`] on the first violation.
pub fn preflight_canonical(
    snapshot: &RenderSnapshot,
    request: &ExportRequest,
    opened: &BTreeMap<SourceId, OpenedSource>,
) -> Result<(), PreflightError> {
    for clip in snapshot.project.clips.values() {
        if clip.muted {
            continue;
        }
        let Some(open) = opened.get(&clip.source_id) else {
            continue; // already rejected in preflight_static
        };
        if open.canonical.sample_rate.get() != request.engine.sample_rate {
            return Err(PreflightError::UnsupportedCodec {
                source_id: clip.source_id,
                detail: format!(
                    "source rate {} != engine rate {}",
                    open.canonical.sample_rate.get(),
                    request.engine.sample_rate
                ),
            });
        }
    }
    Ok(())
}

/// The canonical-format lookup `lowering` consumes: `None` only for
/// sources that failed resolution (their clips are already rejected).
#[must_use]
pub fn canonical_of(
    opened: &BTreeMap<SourceId, OpenedSource>,
    source_id: SourceId,
) -> Option<AudioFormat> {
    opened.get(&source_id).map(|o| o.canonical)
}
