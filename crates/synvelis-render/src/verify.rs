//! Readback verification: the sealed staged WAV is read through
//! `SealedFile::open_readback` — a read-only view of the sealed temp,
//! independent of the muxer's write path — then demuxed through an
//! independent reader (symphonia demux + [`SourceReader`] canonical
//! decode) before publish. The hook asserts stream shape, canonical
//! rate/channel geometry and decoded frame count against
//! [`OutputFacts`]; with `expect_pcm` it also compares every decoded
//! sample. Any failure lands as `RenderError::Verify` — the temp is
//! discarded, never published.

use std::io::{Cursor, Read as _};

use ruviform_audio::render::OutputFacts;
use ruviform_io_file::SealedFile;
use ruviform_media_io::{DecodeMode, Demuxer as _, Error as MediaError, ErrorKind};
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

/// Reads `sealed` back through its read-only readback surface and
/// decodes it through the independent reader path.
///
/// The demuxer owns its source (`MediaSource: 'static`), so the
/// borrowed [`ReadbackFile`](ruviform_io_file::ReadbackFile) bytes are
/// first copied into an independent in-memory buffer — the decode
/// cannot observe a file swapped after seal.
///
/// # Errors
/// [`MediaError`] — readback, decode or comparison failure.
pub fn reopen_check(
    sealed: &mut SealedFile,
    facts: &OutputFacts,
    expect_pcm: crate::spec::PcmExpectRef<'_>,
) -> Result<VerifiedOutput, MediaError> {
    const CHUNK: usize = 8192;
    fn invalid(detail: &'static str) -> MediaError {
        MediaError::with_source(
            ErrorKind::InvalidData,
            std::io::Error::new(std::io::ErrorKind::InvalidData, detail),
        )
    }
    let mut readback = sealed
        .open_readback()
        .map_err(|e| MediaError::with_source(ErrorKind::Io, e))?;
    let mut bytes = Vec::new();
    readback
        .read_to_end(&mut bytes)
        .map_err(|e| MediaError::with_source(ErrorKind::Io, e))?;
    let demuxer = ruviform_io_symphonia::open_demuxer(Cursor::new(bytes))?;
    let stream = {
        let streams = demuxer.streams();
        if streams.len() != 1 {
            return Err(invalid("expected exactly one stream"));
        }
        streams[0].clone()
    };
    let mut reader =
        SourceReader::open(Box::new(demuxer), stream.stream.id, DecodeMode::Canonical)?;
    let canonical = reader.metadata().canonical;
    // Layout *variants* may differ across the independent reopen (an
    // unpositioned WAV decodes as `Custom`, not `Mono`/`Stereo`) — the
    // contract being verified is rate + channel count + frames.
    if canonical.sample_rate != facts.sample_rate
        || canonical.layout.channels() != facts.layout.channels()
    {
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
    if frames != facts.frames_emitted {
        return Err(invalid("decoded frame count mismatch"));
    }
    let pcm_checked = if let Some(expect) = expect_pcm {
        if !expect(&planes) {
            return Err(invalid("PCM expectation refused"));
        }
        true
    } else {
        false
    };
    Ok(VerifiedOutput {
        frames,
        channels: planes.len(),
        sample_rate: canonical.sample_rate.get(),
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
