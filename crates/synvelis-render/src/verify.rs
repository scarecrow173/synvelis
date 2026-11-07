//! Readback verification: the sealed staged WAV is read through
//! `SealedFile::open_readback` — a read-only view of the sealed temp,
//! independent of the muxer's write path — then demuxed through an
//! independent reader (symphonia demux + [`SourceReader`] canonical
//! decode) before publish. The hook asserts stream shape, canonical
//! rate/channel geometry and decoded frame count against
//! [`OutputFacts`]; with `expect_pcm` it also compares every decoded
//! sample. Any failure lands as `RenderError::Verify` — the temp is
//! discarded, never published.

use std::io;

use ruviform_audio::render::OutputFacts;
use ruviform_audio_core::SampleRate;
use ruviform_io_file::SealedFile;
use ruviform_media_io::{DecodeMode, Demuxer, Error as MediaError, ErrorKind};
use ruviform_media_reader::SourceReader;

/// What the verify stage actually established — recorded on the
/// receipt so `Completed` always carries verified output evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct VerifiedOutput {
    /// Decoded frames read back.
    pub frames: u64,
    /// Decoded channel count.
    pub channels: usize,
    /// Declared sample rate.
    pub sample_rate: u32,
    /// The staged file was reopened through an independent reader —
    /// `false` when a caller-supplied verify hook replaced the
    /// built-in one (facts-derived evidence, honesty preserved).
    pub reopened: bool,
    /// Whether caller PCM comparison ran and passed.
    pub pcm_checked: bool,
}

/// Decode scratch width — frames decoded per `read_planar` call.
const CHUNK: usize = 8192;

fn invalid(detail: &'static str) -> MediaError {
    MediaError::with_source(
        ErrorKind::InvalidData,
        io::Error::new(io::ErrorKind::InvalidData, detail),
    )
}

/// A bridge-worker panic mapped to the same typed verify failure the
/// pipeline already classifies — the staged temp is discarded, never
/// published.
fn worker_panic(payload: &(dyn std::any::Any + Send)) -> MediaError {
    let detail = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic");
    MediaError::with_source(
        ErrorKind::Io,
        io::Error::other(format!("verify worker panicked: {detail}")),
    )
}

/// Decoded output returned by the bridge worker — canonical facts plus
/// the planes the caller still needs for `expect_pcm`. `Send` by
/// construction so it can leave the worker-thread callback.
struct DecodedPlanes {
    rate: u32,
    channels: usize,
    frames: u64,
    planes: Vec<Vec<f32>>,
}

/// Demux + canonical decode through the borrowed demuxer — runs on the
/// bridge's worker thread; returns what the caller needs to finish
/// evidence and PCM comparison on the calling thread.
fn decode_borrowed(
    demuxer: &mut dyn Demuxer,
    facts_rate: SampleRate,
    facts_channels: u32,
    facts_frames: u64,
) -> Result<DecodedPlanes, MediaError> {
    let stream = {
        let streams = demuxer.streams();
        if streams.len() != 1 {
            return Err(invalid("expected exactly one stream"));
        }
        streams[0].clone()
    };
    // The reader holds a `Box<dyn Demuxer + '_>` built from the
    // callback's `&mut dyn` reborrow (`impl Demuxer for &mut D`) — the
    // borrow stays inside this scope and never reaches the return.
    let mut reader =
        SourceReader::open(Box::new(demuxer), stream.stream.id, DecodeMode::Canonical)?;
    let canonical = reader.metadata().canonical;
    // Layout *variants* may differ across the independent reopen (an
    // unpositioned WAV decodes as `Custom`, not `Mono`/`Stereo`) — the
    // contract being verified is rate + channel count + frames.
    if canonical.sample_rate != facts_rate || canonical.layout.channels() != facts_channels {
        return Err(invalid("rate or channel count mismatch"));
    }
    let mut planes = vec![Vec::new(); canonical.layout.channels() as usize];
    let mut scratch = vec![vec![0.0f32; CHUNK]; planes.len()];
    let mut frames = 0u64;
    loop {
        let mut views: Vec<&mut [f32]> = scratch.iter_mut().map(Vec::as_mut_slice).collect();
        let outcome = reader.read_planar(&mut views, CHUNK)?;
        for (plane, src) in planes.iter_mut().zip(&scratch) {
            plane.extend_from_slice(&src[..outcome.frames]);
        }
        frames += outcome.frames as u64;
        if outcome.eof || outcome.frames == 0 {
            break;
        }
    }
    if frames != facts_frames {
        return Err(invalid("decoded frame count mismatch"));
    }
    Ok(DecodedPlanes {
        rate: canonical.sample_rate.get(),
        channels: planes.len(),
        frames,
        planes,
    })
}

/// Reads `sealed` back through its read-only readback surface and
/// decodes it through the independent reader path.
///
/// The demuxer owns its source (`MediaSource: 'static`) while the
/// readback is a borrowed, `!Send` `Read + Seek` — so the decode runs
/// through [`ruviform_io_symphonia::with_borrowed_demuxer`]: this
/// thread serves the borrowed readback while the bridge's worker owns
/// the demuxer. In-flight encoded bytes stay bounded by the bridge's
/// per-chunk cap rather than the staged size — the same bounded
/// profile as the pre-staging `File` source. The bridge keeps the
/// no-path-reopen guarantee — the decode reads the sealed object
/// itself, not a swapped stand-in.
///
/// # Errors
/// [`MediaError`] — readback, decode or comparison failure.
pub fn reopen_check(
    sealed: &mut SealedFile,
    facts: &OutputFacts,
    expect_pcm: crate::spec::PcmExpectRef<'_>,
) -> Result<VerifiedOutput, MediaError> {
    let mut readback = sealed
        .open_readback()
        .map_err(|e| MediaError::with_source(ErrorKind::Io, e))?;
    let facts_rate = facts.sample_rate;
    let facts_channels = facts.layout.channels();
    let facts_frames = facts.frames_emitted;
    // A worker panic crosses the bridge as `resume_unwind`; catch it
    // here so it stays the typed verify failure the pipeline
    // classifies, not an unwinding `render()` caller.
    let decoded = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        ruviform_io_symphonia::with_borrowed_demuxer(&mut readback, move |demuxer| {
            decode_borrowed(demuxer, facts_rate, facts_channels, facts_frames)
        })
    })) {
        Ok(result) => result?,
        Err(payload) => return Err(worker_panic(&*payload)),
    };
    let pcm_checked = if let Some(expect) = expect_pcm {
        if !expect(&decoded.planes) {
            return Err(invalid("PCM expectation refused"));
        }
        true
    } else {
        false
    };
    Ok(VerifiedOutput {
        frames: decoded.frames,
        channels: decoded.channels,
        sample_rate: decoded.rate,
        reopened: true,
        pcm_checked,
    })
}

/// Synthesizes verify evidence from [`OutputFacts`] when a
/// caller-supplied hook replaced the built-in reopen — the receipt
/// stays honest: `reopened`/`pcm_checked` are `false`.
#[must_use]
pub fn facts_evidence(facts_channels: usize, facts_rate: u32, facts_frames: u64) -> VerifiedOutput {
    VerifiedOutput {
        frames: facts_frames,
        channels: facts_channels,
        sample_rate: facts_rate,
        reopened: false,
        pcm_checked: false,
    }
}
