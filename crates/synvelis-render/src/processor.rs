//! Render-path processors. `ClipRenderer` serves a clip's decoded
//! canonical PCM inside its engine window (silence outside) — a fresh
//! instance per render, never shared with any live `ClipPlayer`.
//! `GainProc`/`OnePoleProc`/`DelayProc` are the deterministic
//! master-bus DSP inserts; every processor here reports
//! [`OfflineRenderSupport::Verified`] honestly.

use ruviform_audio::typed::{GraphProcessor, NodeIo, OfflineRenderSupport};
use ruviform_rt::RtContext;

use synvelis_player::{ClipWindow, FadeWindow};

/// Offline clip renderer: decoded canonical PCM inside the clip's
/// engine window, silence outside. Position comes from the render's
/// sequential drive — `process` calls are contiguous from
/// `job_start`, so the transport position of call `k`, frame `j` is
/// `play_position + processed + j`; no clock feed or I/O needed. A
/// [`FadeWindow`] (comp transition-side nodes only) shapes the
/// emitted gain per frame.
pub struct ClipRenderer {
    pcm: Vec<Vec<f32>>,
    /// Window-local frame `pcm[c][0]` corresponds to — the decode may
    /// start mid-window when only part of it is processed.
    pcm_base: u64,
    window: ClipWindow,
    fade: Option<FadeWindow>,
    play_position: i64,
    processed: u64,
    support: OfflineRenderSupport,
}

impl ClipRenderer {
    /// `pcm[c][i]` = window-local frame `pcm_base + i`, channel `c`;
    /// `play_position` is the transport position at `job_start`
    /// (`range_start.saturating_sub(preroll)`), `fade` is the node's
    /// crossfade envelope (`None` = unity gain), `support` is the
    /// honestly reported offline capability.
    #[must_use]
    pub fn new(
        pcm: Vec<Vec<f32>>,
        pcm_base: u64,
        window: ClipWindow,
        fade: Option<FadeWindow>,
        play_position: i64,
        support: OfflineRenderSupport,
    ) -> Self {
        Self {
            pcm,
            pcm_base,
            window,
            fade,
            play_position,
            processed: 0,
            support,
        }
    }
}

impl GraphProcessor for ClipRenderer {
    fn offline_support(&self) -> OfflineRenderSupport {
        self.support
    }

    fn process(&mut self, io: &mut NodeIo<'_>, _ctx: &RtContext) {
        let frames = io.frames();
        let window = self.window;
        let first = self.play_position + i64::try_from(self.processed).unwrap_or(i64::MAX);
        if let Some(mut out) = io.output(0) {
            for ch in 0..out.channels() {
                let Some(dst) = out.channel_mut(ch) else {
                    continue;
                };
                let src = self.pcm.get(ch).map_or(&[][..], Vec::as_slice);
                for (j, d) in dst[..frames].iter_mut().enumerate() {
                    let pos = first + i64::try_from(j).unwrap_or(i64::MAX);
                    let local = pos
                        .checked_sub(i64::try_from(window.engine_start).unwrap_or(i64::MAX))
                        .unwrap_or(0);
                    let rel = u64::try_from(local)
                        .ok()
                        .and_then(|l| l.checked_sub(self.pcm_base))
                        .and_then(|r| usize::try_from(r).ok());
                    let sample = rel.and_then(|i| src.get(i)).copied().unwrap_or(0.0);
                    *d = self.fade.map_or(sample, |f| sample * f.gain_at(pos));
                }
            }
        }
        self.processed = self.processed.saturating_add(frames as u64);
    }
}

/// Constant linear gain on every channel.
pub struct GainProc {
    gain: f32,
}

impl GainProc {
    /// Creates a gain processor (`10^(gain_db/20)` linear).
    #[must_use]
    pub fn new(gain_db: f64) -> Self {
        #[expect(clippy::cast_possible_truncation, reason = "audio gain")]
        let gain = 10.0_f64.powf(gain_db / 20.0) as f32;
        Self { gain }
    }
}

impl GraphProcessor for GainProc {
    fn offline_support(&self) -> OfflineRenderSupport {
        OfflineRenderSupport::Verified
    }

    fn process(&mut self, io: &mut NodeIo<'_>, _ctx: &RtContext) {
        let gain = self.gain;
        map_channels(io, |_, _, x| x * gain);
    }
}

/// Stateful one-pole low-pass with an honest declared latency:
/// `y[n] = a·y[n−1] + x[n]`, then `latency` frames of real output
/// delay so the PDC emit shift (`emitted = position − total_latency`)
/// relabels the delayed output onto the positions it came from.
/// State carries across calls — block-partition invariant.
pub struct OnePoleProc {
    a: f32,
    latency: u32,
    state: Vec<f32>,
    /// Per-channel delay line + cursor, empty when `latency == 0`.
    line: Vec<Vec<f32>>,
    cursor: Vec<usize>,
}

impl OnePoleProc {
    /// `a` is the pole coefficient, `latency` the declared *and*
    /// applied output delay in frames (`0` for a plain filter).
    #[must_use]
    pub fn new(a: f32, latency: u32, channels: usize) -> Self {
        let channels = channels.max(1);
        let delay = usize::try_from(latency).unwrap_or(usize::MAX);
        Self {
            a,
            latency,
            state: vec![0.0; channels],
            line: vec![vec![0.0; delay]; channels],
            cursor: vec![0; channels],
        }
    }
}

impl GraphProcessor for OnePoleProc {
    fn latency(&self) -> u32 {
        self.latency
    }

    fn offline_support(&self) -> OfflineRenderSupport {
        OfflineRenderSupport::Verified
    }

    fn process(&mut self, io: &mut NodeIo<'_>, _ctx: &RtContext) {
        let a = self.a;
        let input = io.input(0);
        for o in 0..io.outputs() {
            let Some(mut out) = io.output(o) else {
                continue;
            };
            for ch in 0..out.channels() {
                let Some(plane) = out.channel_mut(ch) else {
                    continue;
                };
                let src = input.as_ref().and_then(|v| v.channel(ch));
                let ci = ch.min(self.state.len() - 1);
                let s = &mut self.state[ci];
                let line = &mut self.line[ci];
                let cursor = &mut self.cursor[ci];
                for (i, d) in plane.iter_mut().enumerate() {
                    *s = *s * a + src.map_or(0.0, |p| p[i]);
                    *d = if line.is_empty() {
                        *s
                    } else {
                        let delayed = line[*cursor];
                        line[*cursor] = *s;
                        *cursor = (*cursor + 1) % line.len();
                        delayed
                    };
                }
            }
        }
    }
}

/// Delay-line effect: `y[n] = x[n−frames]` per channel — `frames = 0`
/// is an honest pass-through, not a 1-sample shift. The delay is the
/// audible content, so `latency() = 0` — an effect, not a latency to
/// PDC-compensate; capturing its tail needs `TailPolicy`.
pub struct DelayProc {
    rings: Vec<Vec<f32>>,
    cursor: Vec<usize>,
}

impl DelayProc {
    /// `frames` of delay per channel (`0` = identity).
    #[must_use]
    pub fn new(frames: u32, channels: usize) -> Self {
        let delay = usize::try_from(frames).unwrap_or(usize::MAX);
        Self {
            rings: vec![vec![0.0; delay]; channels.max(1)],
            cursor: vec![0; channels.max(1)],
        }
    }
}

impl GraphProcessor for DelayProc {
    fn offline_support(&self) -> OfflineRenderSupport {
        OfflineRenderSupport::Verified
    }

    fn process(&mut self, io: &mut NodeIo<'_>, _ctx: &RtContext) {
        let input = io.input(0);
        for o in 0..io.outputs() {
            let Some(mut out) = io.output(o) else {
                continue;
            };
            for ch in 0..out.channels() {
                let Some(plane) = out.channel_mut(ch) else {
                    continue;
                };
                let src = input.as_ref().and_then(|v| v.channel(ch));
                let ci = ch.min(self.rings.len() - 1);
                let ring = &mut self.rings[ci];
                let cursor = &mut self.cursor[ci];
                for (i, d) in plane.iter_mut().enumerate() {
                    *d = if ring.is_empty() {
                        src.map_or(0.0, |p| p[i])
                    } else {
                        let delayed = ring[*cursor];
                        ring[*cursor] = src.map_or(0.0, |p| p[i]);
                        *cursor = (*cursor + 1) % ring.len();
                        delayed
                    };
                }
            }
        }
    }
}

/// Shared in→out map helper for stateless processors.
fn map_channels(io: &mut NodeIo<'_>, f: impl Fn(usize, usize, f32) -> f32) {
    let input = io.input(0);
    for o in 0..io.outputs() {
        let Some(mut out) = io.output(o) else {
            continue;
        };
        for ch in 0..out.channels() {
            let Some(plane) = out.channel_mut(ch) else {
                continue;
            };
            let src = input.as_ref().and_then(|v| v.channel(ch));
            for (i, d) in plane.iter_mut().enumerate() {
                *d = src.map_or(0.0, |p| f(ch, i, p[i]));
            }
        }
    }
}
