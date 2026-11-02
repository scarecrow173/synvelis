//! Deterministic PCM WAV fixtures for headless acceptance tests.
//!
//! The writer emits a canonical 44-byte RIFF/WAVE header plus PCM16
//! interleaved data — no encoder dependency. The sample pattern is a
//! pure function of `(channel, frame)` so any expected sample is
//! re-derivable and every value is exactly representable in `i16`
//! (and therefore in decoded `f32` as `v / 32768`).

use std::fs::File;
use std::io::{self, Write};
use std::path::Path;

/// Deterministic PCM16 sample for `channel`/`frame`.
///
/// Range `[-32768, 32752]` — exactly representable; decoded canonical
/// `f32` is `fixture_sample(..) as f32 / 32768.0`.
#[must_use]
pub fn fixture_sample(channel: usize, frame: usize) -> i16 {
    let mixed = i32::try_from((frame * 31 + channel * 7) % 4096).unwrap_or_default() - 2048;
    i16::try_from(mixed * 16).unwrap_or(i16::MAX)
}

/// The expected canonical `f32` for `channel`/`frame`.
#[must_use]
pub fn fixture_sample_f32(channel: usize, frame: usize) -> f32 {
    f32::from(fixture_sample(channel, frame)) / 32768.0
}

/// Builds canonical PCM16 WAV bytes (`WAVE`/`fmt `/`data` chunks).
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn wav_pcm16(channels: u16, sample_rate: u32, frames: usize) -> Vec<u8> {
    let block_align = u32::from(channels) * 2;
    let data_len = (frames * usize::from(channels) * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes()); // PCM format tag
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * block_align).to_le_bytes());
    out.extend_from_slice(&(block_align as u16).to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for frame in 0..frames {
        for channel in 0..usize::from(channels) {
            out.extend_from_slice(&fixture_sample(channel, frame).to_le_bytes());
        }
    }
    out
}

/// Writes a PCM16 fixture file and returns the exact bytes written.
///
/// # Errors
/// I/O failures creating or writing `path`.
pub fn write_fixture(
    path: &Path,
    channels: u16,
    sample_rate: u32,
    frames: usize,
) -> io::Result<Vec<u8>> {
    let bytes = wav_pcm16(channels, sample_rate, frames);
    let mut file = File::create(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(bytes)
}
