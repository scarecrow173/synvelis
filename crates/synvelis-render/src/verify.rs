//! Readback verification: the sealed staged WAV is read through
//! `SealedFile::open_readback` — a read-only view of the sealed temp,
//! independent of the muxer's write path — then demuxed through an
//! independent reader (symphonia demux + [`SourceReader`] canonical
//! decode) before publish. The hook asserts stream shape, canonical
//! rate/channel geometry and decoded frame count against
//! [`OutputFacts`]; with `expect_pcm` it also compares every decoded
//! sample. Any failure lands as `RenderError::Verify` — the temp is
//! discarded, never published.

use std::io::{self, Read, Seek, SeekFrom};
use std::sync::mpsc::{self, Receiver, SyncSender};

use ruviform_audio::render::OutputFacts;
use ruviform_audio_core::SampleRate;
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

/// Upper bound per proxied read request — the channel never carries
/// more than this many bytes at once, so the proxy's footprint is
/// O(chunk) instead of O(file).
const PROXY_CHUNK: usize = 1 << 20;

/// Decode scratch width — frames decoded per `read_planar` call.
const CHUNK: usize = 8192;

/// Request issued by [`ReadProxy`] to the serving thread.
enum ReadbackReq {
    /// Positioned read of `len` bytes at `pos` (short read at EOF).
    Read { pos: u64, len: usize },
    /// Total stream length (for `SeekFrom::End`).
    Len,
}

/// Response handed back from the serving thread.
enum ReadbackResp {
    Bytes(io::Result<Vec<u8>>),
    Len(io::Result<u64>),
}

/// An owned `Read + Seek + Send` endpoint that proxies every byte
/// request through a bounded channel to a producer holding the real
/// reader. It exists because the demuxer requires an owned
/// `MediaSource` (`'static`) while [`SealedFile::open_readback`]
/// deliberately lends only a borrowed, `!Send` `Read + Seek`: the
/// channel endpoint carries no borrow, so it satisfies both bounds
/// and keeps memory at O(chunk) rather than O(staged bytes).
struct ReadProxy {
    pos: u64,
    len: Option<u64>,
    req: SyncSender<ReadbackReq>,
    resp: Receiver<ReadbackResp>,
}

fn disconnected() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "readback proxy producer gone")
}

impl ReadProxy {
    fn stream_len(&mut self) -> io::Result<u64> {
        if let Some(len) = self.len {
            return Ok(len);
        }
        self.req
            .send(ReadbackReq::Len)
            .map_err(|_| disconnected())?;
        match self.resp.recv().map_err(|_| disconnected())? {
            ReadbackResp::Len(result) => {
                let len = result?;
                self.len = Some(len);
                Ok(len)
            }
            ReadbackResp::Bytes(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "readback proxy: unexpected bytes reply",
            )),
        }
    }
}

impl Read for ReadProxy {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let want = buf.len().min(PROXY_CHUNK);
        self.req
            .send(ReadbackReq::Read {
                pos: self.pos,
                len: want,
            })
            .map_err(|_| disconnected())?;
        match self.resp.recv().map_err(|_| disconnected())? {
            ReadbackResp::Bytes(Ok(bytes)) => {
                let n = bytes.len().min(want);
                buf[..n].copy_from_slice(&bytes[..n]);
                self.pos += n as u64;
                Ok(n)
            }
            ReadbackResp::Bytes(Err(e)) => Err(e),
            ReadbackResp::Len(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "readback proxy: unexpected length reply",
            )),
        }
    }
}

impl Seek for ReadProxy {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let resolved = match pos {
            SeekFrom::Start(p) => Some(p),
            SeekFrom::End(off) => self.stream_len()?.checked_add_signed(off),
            SeekFrom::Current(off) => self.pos.checked_add_signed(off),
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "seek out of range"))?;
        self.pos = resolved;
        Ok(resolved)
    }
}

/// Producer side of the proxy — runs on the calling thread so the
/// borrowed `ReadbackFile` never has to cross a thread boundary.
/// Positioned reads only; each request re-seeks, so `Len` probes do
/// not disturb later reads.
fn serve_readback<R: Read + Seek>(
    src: &mut R,
    req: &Receiver<ReadbackReq>,
    resp: &SyncSender<ReadbackResp>,
) {
    while let Ok(request) = req.recv() {
        match request {
            ReadbackReq::Read { pos, len } => {
                let out = positioned_read(src, pos, len);
                if resp.send(ReadbackResp::Bytes(out)).is_err() {
                    return;
                }
            }
            ReadbackReq::Len => {
                let out = src.seek(SeekFrom::End(0));
                if resp.send(ReadbackResp::Len(out)).is_err() {
                    return;
                }
            }
        }
    }
}

fn positioned_read<R: Read + Seek>(src: &mut R, pos: u64, len: usize) -> io::Result<Vec<u8>> {
    src.seek(SeekFrom::Start(pos))?;
    let mut buf = vec![0u8; len];
    let mut n = 0usize;
    while n < len {
        match src.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            // A short read is returned now; the error surfaces on the
            // next call — the standard `Read` contract.
            Err(e) => {
                if n == 0 {
                    return Err(e);
                }
                break;
            }
        }
    }
    buf.truncate(n);
    Ok(buf)
}

fn invalid(detail: &'static str) -> MediaError {
    MediaError::with_source(
        ErrorKind::InvalidData,
        io::Error::new(io::ErrorKind::InvalidData, detail),
    )
}

/// Decoded output returned by the worker thread — canonical facts plus
/// the planes the caller still needs for `expect_pcm`.
struct DecodedPlanes {
    rate: u32,
    channels: usize,
    frames: u64,
    planes: Vec<Vec<f32>>,
}

/// Demux + canonical decode of the proxied bytes — runs on the scoped
/// worker thread; returns what the caller needs to finish evidence
/// and PCM comparison on the calling thread.
fn decode_proxied(
    source: ReadProxy,
    facts_rate: SampleRate,
    facts_channels: u32,
    facts_frames: u64,
) -> Result<DecodedPlanes, MediaError> {
    let demuxer = ruviform_io_symphonia::open_demuxer(source)?;
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
/// on a scoped worker fed by a bounded channel proxy
/// ([`ReadProxy`]/[`serve_readback`]) on this thread. The staged bytes
/// are never buffered whole: peak memory stays at the proxied chunk +
/// decoded planes, matching the pre-staging `File` source profile.
/// The channel keeps the no-path-reopen guarantee — the decode reads
/// the sealed object itself, not a swapped stand-in.
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
    // The readback is `!Send`, so the producer stays on this thread and
    // the demuxer/decode moves to the scoped worker.
    let decoded = std::thread::scope(|scope| {
        let (req_tx, req_rx) = mpsc::sync_channel::<ReadbackReq>(1);
        let (resp_tx, resp_rx) = mpsc::sync_channel::<ReadbackResp>(1);
        let worker = scope.spawn(move || {
            decode_proxied(
                ReadProxy {
                    pos: 0,
                    len: None,
                    req: req_tx,
                    resp: resp_rx,
                },
                facts_rate,
                facts_channels,
                facts_frames,
            )
        });
        serve_readback(&mut readback, &req_rx, &resp_tx);
        worker.join().map_err(|panic| {
            let detail = panic
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("unknown panic");
            MediaError::with_source(
                ErrorKind::Io,
                io::Error::other(format!("verify worker panicked: {detail}")),
            )
        })?
    })?;
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
