//! `ClipPlayer` — the per-clip [`GraphProcessor`]: owns the RT
//! [`PrefetchConsumer`], maps the shared transport clock onto the
//! clip's source window, and accumulates fill telemetry into
//! [`PlaybackStats`]. Underrun, fault and EOF are recorded separately
//! — the fallback is silence, never silently-successful audio.
//!
//! Position protocol (mirrors `ruviform-media-reader`'s own tests):
//! `fill` is always called with `expected_first = consumer.position()`
//! — the consumer's stream position is the resync assertion, not the
//! engine's belief. After a seek the pipeline carries an in-order
//! `Boundary`; the consumer adopts it inside a `fill`, resetting
//! `position()` to the seek target, and a bulk fill spanning the
//! boundary cannot attribute per-frame positions. So while the stream
//! disagrees with the window mapping the player resyncs with
//! single-frame probe fills: each probe's served frame is exactly
//! `position() - 1`, so post-boundary content lands at its true window
//! offset and stale pre-boundary frames are consumed, discarded, and
//! counted as `mismatch`. Once `position()` equals the mapped frame,
//! bulk fills resume — and since source PCM is immutable, same-position
//! content is identical, so a boundary landing on the current position
//! cannot corrupt a bulk fill. Sample-exact seeks: no play-through, no
//! hole, no off-by-one.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_audio::typed::{GraphProcessor, NodeIo};
use ruviform_media_reader::{FillReport, PrefetchConsumer};
use ruviform_rt::RtContext;

use crate::lowering::{ClipWindow, FadeWindow};

/// Bound on probe fills per `process` call — larger than any prefetch
/// ring depth, so resync always converges or stalls on underrun/EOF.
const MAX_PROBES: u64 = 4_096;

/// Observable per-clip playback accounting (RT-written, non-RT read).
#[derive(Debug, Default)]
pub struct PlaybackStats {
    /// Real decoded frames written.
    pub written: AtomicU64,
    /// Frames the pipeline could not supply in time.
    pub underrun: AtomicU64,
    /// Frames covered by fault markers.
    pub faulted: AtomicU64,
    /// Blocks in which EOF was reached.
    pub eof_blocks: AtomicU64,
    /// Stream frames discarded or window frames skipped by resync.
    pub mismatch: AtomicU64,
    /// Frames outside the clip window written as silence.
    pub silenced: AtomicU64,
    /// Number of consumer `fill` calls.
    pub fills: AtomicU64,
}

/// Point-in-time copy of [`PlaybackStats`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatsSnapshot {
    /// See [`PlaybackStats::written`].
    pub written: u64,
    /// See [`PlaybackStats::underrun`].
    pub underrun: u64,
    /// See [`PlaybackStats::faulted`].
    pub faulted: u64,
    /// See [`PlaybackStats::eof_blocks`].
    pub eof_blocks: u64,
    /// See [`PlaybackStats::mismatch`].
    pub mismatch: u64,
    /// See [`PlaybackStats::silenced`].
    pub silenced: u64,
    /// See [`PlaybackStats::fills`].
    pub fills: u64,
}

impl PlaybackStats {
    /// Reads all counters into a snapshot.
    #[must_use]
    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            written: self.written.load(Ordering::Relaxed),
            underrun: self.underrun.load(Ordering::Relaxed),
            faulted: self.faulted.load(Ordering::Relaxed),
            eof_blocks: self.eof_blocks.load(Ordering::Relaxed),
            mismatch: self.mismatch.load(Ordering::Relaxed),
            silenced: self.silenced.load(Ordering::Relaxed),
            fills: self.fills.load(Ordering::Relaxed),
        }
    }
}

/// Outcome of a single-frame probe fill.
enum Probe {
    /// The stream made no progress — underrun or EOF.
    Stalled,
    /// A boundary reset the position without serving a frame.
    Moved,
    /// One frame was served at the returned source position.
    Served(u64),
}

/// Shared driver↔processor signals — non-RT writes, RT reads.
#[derive(Clone)]
pub struct ClipSync {
    /// Transport clock: engine frame at the start of the block.
    pub clock: Arc<AtomicU64>,
    /// Seek nonce: bumped on each `handle.seek` so the processor
    /// knows a `Boundary` may still be in flight.
    pub pending_seek: Arc<AtomicU64>,
    /// Quiesce nonce: bumped before the driver re-anchors so the
    /// processor discards every in-flight item first.
    pub drain: Arc<AtomicU64>,
}

/// Per-clip playback processor: prefetched source PCM inside a
/// declared engine window, silence outside it. A [`FadeWindow`]
/// (transition-side nodes only) shapes the emitted gain per frame.
pub struct ClipPlayer {
    consumer: PrefetchConsumer,
    window: ClipWindow,
    fade: Option<FadeWindow>,
    sync: ClipSync,
    stats: Arc<PlaybackStats>,
    scratch: Vec<Vec<f32>>,
    /// One-frame probe buffers for boundary-crossing resync fills.
    probe: Vec<Vec<f32>>,
    /// Consumer generation at construction — the adopted generation
    /// reaches `base_generation + pending_seek` once every issued
    /// seek boundary has been consumed.
    base_generation: u64,
    /// Last quiesce nonce this player has drained.
    drained: u64,
}

impl ClipPlayer {
    /// Creates the processor. `scratch`/`probe` planes are allocated
    /// here — never inside `process` — so the RT path does not allocate.
    /// `fade` is the node's crossfade envelope (`None` = unity gain).
    #[must_use]
    pub fn new(
        consumer: PrefetchConsumer,
        window: ClipWindow,
        fade: Option<FadeWindow>,
        sync: ClipSync,
        stats: Arc<PlaybackStats>,
        block_frames: usize,
        channels: usize,
    ) -> Self {
        let base_generation = consumer.generation();
        Self {
            consumer,
            window,
            fade,
            sync,
            stats,
            scratch: vec![vec![0.0; block_frames]; channels],
            probe: vec![vec![0.0; 1]; channels],
            base_generation,
            drained: 0,
        }
    }

    /// The shared stats handle the driver reports through.
    #[must_use]
    pub fn stats(&self) -> &Arc<PlaybackStats> {
        &self.stats
    }

    /// Adds a [`FillReport`]'s telemetry to the shared stats.
    fn account(&self, report: &FillReport) {
        self.stats.fills.fetch_add(1, Ordering::Relaxed);
        self.stats
            .written
            .fetch_add(report.written as u64, Ordering::Relaxed);
        self.stats
            .underrun
            .fetch_add(report.underrun as u64, Ordering::Relaxed);
        self.stats
            .faulted
            .fetch_add(report.faulted as u64, Ordering::Relaxed);
        if report.eof {
            self.stats.eof_blocks.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// The consumer generation reached once every seek issued through
    /// `pending_seek` has been adopted (`+1` generation per `seek`).
    fn target_generation(&self) -> u64 {
        self.base_generation + self.sync.pending_seek.load(Ordering::Relaxed)
    }

    /// `true` while a seek `Boundary` may still be in flight or the
    /// stream disagrees with the engine mapping — bulk fills are only
    /// safe in the aligned steady state, since a bulk fill spanning a
    /// boundary cannot attribute per-frame positions.
    fn needs_resync(&self, want: u64) -> bool {
        self.consumer.generation() < self.target_generation() || self.consumer.position().0 != want
    }

    /// One single-frame probe fill — the only fill whose served
    /// position is attributable across a boundary adoption.
    fn probe_one(&mut self, ctx: &RtContext) -> Probe {
        let before = self.consumer.position();
        let mut views: Vec<&mut [f32]> = self.probe.iter_mut().map(Vec::as_mut_slice).collect();
        let report = self.consumer.fill(&mut views, before, ctx);
        self.account(&report);
        let after = self.consumer.position();
        if after == before {
            return Probe::Stalled;
        }
        // `position()` moved by the served span — `before + span` in
        // steady state, or the seek target plus the post-boundary span
        // when a boundary was adopted inside this fill. Either way,
        // `after - span` is the first served frame's position.
        let span = (report.written + report.faulted) as u64;
        if span == 0 {
            return Probe::Moved;
        }
        Probe::Served(after.0.saturating_sub(span))
    }

    /// Copies the last probe's served frame (when it held real data)
    /// into `scratch` at `dst` for every channel.
    fn commit_probe(&mut self, dst: usize) {
        for (channel, probe) in self.scratch.iter_mut().zip(self.probe.iter()) {
            channel[dst] = probe[0];
        }
    }

    /// Quiesce drain: every in-flight item is consumed into the
    /// scratch planes and discarded, so the next seek `Boundary`
    /// reaches the consumer without stale play-through in front of
    /// it. Drained frames are real PCM but belong to the abandoned
    /// generation — counted as `mismatch`, never played.
    fn drain_inflight(&mut self, ctx: &RtContext) {
        loop {
            let position = self.consumer.position();
            let mut views: Vec<&mut [f32]> =
                self.scratch.iter_mut().map(Vec::as_mut_slice).collect();
            let report = self.consumer.fill(&mut views, position, ctx);
            self.account(&report);
            let consumed = report.written + report.faulted;
            self.stats
                .mismatch
                .fetch_add(consumed as u64, Ordering::Relaxed);
            if consumed == 0 {
                // Underrun or end-of-stream — the ring is empty.
                break;
            }
        }
    }

    /// Fills `scratch[..][done..done + take]` with the window span
    /// starting at source frame `source_start + local`: bulk fills
    /// while aligned, single-frame probes while a boundary may be in
    /// flight or positions disagree. Unfillable tails stay silent and
    /// are counted as `mismatch`.
    fn fill_window(&mut self, done: usize, take: usize, local: u64, ctx: &RtContext) {
        let window = self.window;
        let mut filled = 0usize;
        let mut probes = 0_u64;
        while filled < take {
            let want = window.source_start + local + filled as u64;
            if !self.needs_resync(want) {
                // Aligned steady state: bulk-serve the rest of the
                // window span — no boundary can be in flight.
                let mut views: Vec<&mut [f32]> = self
                    .scratch
                    .iter_mut()
                    .map(|channel| &mut channel[done + filled..done + take])
                    .collect();
                let position = self.consumer.position();
                let report = self.consumer.fill(&mut views, position, ctx);
                self.account(&report);
                let served = report.written + report.faulted;
                if served == 0 {
                    break;
                }
                filled += served;
                continue;
            }
            // Misaligned or a boundary may be in flight:
            // single-frame probe — the only fill whose served
            // position is attributable across the adoption.
            match self.probe_one(ctx) {
                Probe::Served(served) if served == want => {
                    self.commit_probe(done + filled);
                    filled += 1;
                }
                Probe::Served(_) => {
                    // Stale or skipped-ahead stream frame: consumed
                    // and discarded so the stream can converge.
                    self.stats.mismatch.fetch_add(1, Ordering::Relaxed);
                }
                Probe::Moved => {}
                Probe::Stalled => break,
            }
            probes += 1;
            if probes >= MAX_PROBES {
                break;
            }
        }
        if filled < take {
            // Resync could not cover the span — honest silence and
            // an explicit mismatch count for the skipped frames.
            for channel in &mut self.scratch {
                channel[done + filled..done + take].fill(0.0);
            }
            self.stats
                .mismatch
                .fetch_add((take - filled) as u64, Ordering::Relaxed);
        }
    }
}

impl GraphProcessor for ClipPlayer {
    /// Fills the output port: pre-window and post-window frames are
    /// silence; inside the window the consumer supplies the mapped
    /// source frames. Stream/engine disagreement is resynced with
    /// position-true probes — never silently misaligned.
    fn process(&mut self, io: &mut NodeIo<'_>, ctx: &RtContext) {
        let frames = io
            .output(0)
            .and_then(|mut out| out.channel_mut(0).map(|c| c.len()))
            .unwrap_or(0);
        if frames == 0 {
            return;
        }
        let window = self.window;
        let base = self.sync.clock.load(Ordering::Relaxed);
        let quiesce = self.sync.drain.load(Ordering::Relaxed);
        if quiesce != self.drained {
            self.drain_inflight(ctx);
            self.drained = quiesce;
        }
        let mut done = 0usize;
        while done < frames {
            let engine = base + done as u64;
            let remaining = frames - done;
            if engine < window.engine_start {
                let take = remaining
                    .min(usize::try_from(window.engine_start - engine).unwrap_or(usize::MAX));
                for channel in &mut self.scratch {
                    channel[done..done + take].fill(0.0);
                }
                self.stats
                    .silenced
                    .fetch_add(take as u64, Ordering::Relaxed);
                done += take;
                continue;
            }
            if engine >= window.engine_start + window.engine_len {
                for channel in &mut self.scratch {
                    channel[done..frames].fill(0.0);
                }
                self.stats
                    .silenced
                    .fetch_add(remaining as u64, Ordering::Relaxed);
                break;
            }
            let local = engine - window.engine_start;
            let take =
                remaining.min(usize::try_from(window.engine_len - local).unwrap_or(usize::MAX));
            self.fill_window(done, take, local, ctx);
            done += take;
        }
        if let Some(mut out) = io.output(0) {
            for (index, channel) in self.scratch.iter().enumerate() {
                if let Some(dst) = out.channel_mut(index) {
                    if let Some(fade) = self.fade {
                        // The fade parameter is the engine frame — the
                        // envelope is evaluated in the graph's own
                        // position domain.
                        let block = i64::try_from(base).unwrap_or(i64::MAX);
                        for (i, d) in dst[..frames].iter_mut().enumerate() {
                            let at = block.saturating_add(i64::try_from(i).unwrap_or(0));
                            *d = channel[i] * fade.gain_at(at);
                        }
                    } else {
                        dst[..frames].copy_from_slice(&channel[..frames]);
                    }
                }
            }
        }
    }
}
