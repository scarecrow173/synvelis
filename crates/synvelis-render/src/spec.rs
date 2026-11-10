//! Frozen render-input specification: the Synvelis-owned half of the
//! render contract. [`ExportRequest`] carries everything the export
//! needs that is *not* inside the project snapshot — range, tail
//! classification, preroll, destination, publish policy, output recipe
//! and the master-bus insert chain.

use std::path::PathBuf;

use ruviform_audio::render::OutputFacts;
use ruviform_audio::typed::OfflineRenderSupport;
use ruviform_audio_core::SampleFormat;
use ruviform_io_file::{PublishFailure, PublishOptions, PublishReceipt, SealedFile, StagedFile};
use ruviform_media_io::{Error as MediaError, MediaSink, Muxer};
use synvelis_player::EngineConfig;
use synvelis_project::{ContentDigest, PluginStateQualitySummary, StateQualityPolicy};

/// Plugin-state quality evidence attached to an offline export
/// recipe (TIM-04, issue #128).
///
/// The recipe carries the *observed* capture quality summary from a
/// [`synvelis_project::PluginStateSession`] save/restore plus the
/// manifest digest it came from. Under [`StateQualityPolicy::Strict`]
/// preflight refuses the export when `evidence.exact_eligible()` is
/// false — a `BestEffort`/`Unsupported`/failed plugin state can
/// never silently pass an exact offline requirement. Under
/// `Standard` the evidence is recorded on the receipt verbatim —
/// a conditional save is never reported as perfect reproduction.
///
/// This slice does not instantiate plugins into the render graph;
/// the recipe is quality evidence + gate only.
#[derive(Clone, Debug)]
pub struct PluginStateRecipe {
    /// Save quality policy the evidence was produced under.
    pub policy: StateQualityPolicy,
    /// Aggregate capture-quality evidence.
    pub evidence: PluginStateQualitySummary,
    /// Content digest of the plugin-state manifest the evidence
    /// came from (`None` when evidence was produced without a
    /// published manifest, e.g. a rejected Strict save's summary).
    pub manifest_digest: Option<ContentDigest>,
}

/// Tail classification handed to the sequential driver.
///
/// `Option<TailPolicy>` on the request, not a variant here: a caller
/// that cannot bound the tail leaves the field `None`, which preflight
/// reports as [`crate::PreflightError::UnknownTail`] — never a silent
/// truncation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TailPolicy {
    /// Provably no tail — emit exactly `[start, end)`. Preflight
    /// refuses this when the insert chain provably *carries* a tail
    /// ([`Insert::tail_frames`]), so `None` can never quietly drop
    /// delayed content.
    None,
    /// Provably bounded tail of `frames`.
    Known(u64),
    /// Estimated tail of `frames` — rendered fully, receipt marks the
    /// evidence as estimated.
    Estimated(u64),
}

impl TailPolicy {
    /// Extra emitted frames past `range_end`.
    #[must_use]
    pub const fn frames(self) -> u64 {
        match self {
            Self::None => 0,
            Self::Known(n) | Self::Estimated(n) => n,
        }
    }
}

/// A master-bus insert for the render path. Every variant maps to a
/// deterministic Synvelis processor reporting
/// [`OfflineRenderSupport::Verified`]; nothing here is a plugin.
#[derive(Clone, Debug)]
pub enum Insert {
    /// Constant linear gain (`10^(db/20)`) applied to every channel.
    Gain {
        /// Gain in decibels.
        gain_db: f64,
    },
    /// Stateful one-pole low-pass: `y[n] = a·y[n−1] + x[n]`.
    OnePole {
        /// Pole coefficient.
        a: f32,
        /// Honestly reported processing latency for the PDC pass —
        /// `0` for a plain filter, `> 0` to exercise PDC realignment.
        latency: u32,
    },
    /// Delay-line effect insert: `y[n] = x[n−frames]` (`0` = identity).
    /// The delay *is* the audible content, so the processor reports
    /// `latency() = 0` — it is an effect, not a latency to compensate.
    Delay {
        /// Delay length in frames.
        frames: u32,
    },
}

impl Insert {
    /// Provable bounded tail this insert leaves past `range_end`: a
    /// `Delay` rings its content out for its full length beyond the
    /// range. `OnePole`'s declared latency is PDC-compensated — its
    /// line drains *inside* the emitted range — and its natural IIR
    /// decay is unbounded rather than provably bounded, so it counts
    /// 0 here (callers wanting that decay declare `Known`/`Estimated`,
    /// and `None` truncates it by explicit choice). `Gain` is
    /// memoryless.
    #[must_use]
    pub fn tail_frames(&self) -> u64 {
        match *self {
            Self::Delay { frames } => u64::from(frames),
            _ => 0,
        }
    }
    /// PDC latency this insert's processor declares to the graph —
    /// `OnePole` only; `Delay` is an effect, `Gain` is memoryless.
    #[must_use]
    pub fn declared_latency(&self) -> u64 {
        match *self {
            Self::OnePole { latency, .. } => u64::from(latency),
            _ => 0,
        }
    }
}

/// A staged-sink wrapper for fault injection (bounded-write,
/// failing-flush sinks) — wraps the `&mut StagedFile` borrow the
/// muxer writes through.
pub type WrapSink = Box<dyn for<'a> FnOnce(&'a mut StagedFile) -> Box<dyn MediaSink + 'a> + Send>;
/// A verify hook over the sealed staged file + emitted facts; reads
/// through `SealedFile::open_readback`, never a raw file handle.
pub type DynVerify =
    Box<dyn FnOnce(&mut SealedFile, &OutputFacts) -> Result<(), MediaError> + Send>;
/// A publish step replacing [`SealedFile::publish`].
pub type DynPublish =
    Box<dyn FnOnce(SealedFile, PublishOptions) -> Result<PublishReceipt, PublishFailure> + Send>;
/// A PCM comparison run inside the default verify hook — receives the
/// reopened decoded planes (`planar f32`, one vec per channel);
/// returning `false` fails verification.
pub type PcmExpect = Box<dyn Fn(&[Vec<f32>]) -> bool + Send>;
/// The borrowed form [`crate::verify::reopen_check`] consumes.
pub type PcmExpectRef<'a> = Option<&'a (dyn Fn(&[Vec<f32>]) -> bool + Send + 'a)>;

/// How a caller replaces the output plumbing for fault injection —
/// the same seams [`ruviform_audio::render::RenderOutput`] exposes.
/// The default (all `None`) is the real io-wav path: `open_muxer` over
/// the borrowed `StagedFile`, io-wav encoder, sealed readback verify
/// and [`SealedFile::publish`].
#[derive(Default)]
pub struct OutputHooks {
    /// Wraps the borrowed staged file sink before the muxer is built —
    /// e.g. a bounded-write or failing-flush sink. `None` uses the
    /// `&mut StagedFile` directly.
    pub wrap_sink: Option<WrapSink>,
    /// Replaces the default readback/verify hook entirely. `None` runs
    /// [`crate::verify::reopen_check`] (independent reader: rate,
    /// layout, frame count, plus `expect_pcm` when set).
    pub verify: Option<DynVerify>,
    /// Replaces [`SealedFile::publish`] — e.g. an injected publish
    /// failure. `None` delegates to `SealedFile::publish`.
    pub publish: Option<DynPublish>,
    /// Extra PCM comparison run inside the default verify hook.
    /// Ignored when `verify` is set.
    pub expect_pcm: Option<PcmExpect>,
}

/// Everything an export needs beyond the frozen snapshot.
///
/// - `range` is half-open `[range_start, range_end)` in sample frames
///   at `engine.sample_rate`.
/// - `tail` must be `Some(..)` — `None` fails preflight as
///   [`crate::PreflightError::UnknownTail`].
/// - `preroll` is the number of transport positions before
///   `range_start` the engine processes as warm-up (stateful DSP is
///   always driven continuously from `range_start - preroll`, clamped
///   to `0` — never chunked or parallelized).
/// - `inserts` install deterministic master-bus DSP in signal order.
/// - `clip_support` is the support level clip renderers report to the
///   ruviform capability gate — `Verified` in production; tests set
///   `Declared`/`Unknown` to exercise the preflight rejection.
#[derive(Debug)]
pub struct ExportRequest {
    /// Headless engine geometry (rate/channels) — `sample_rate` sizes
    /// both the semantic graph and the output; `channels` is the
    /// *output tap* channel count (1 = mono, otherwise stereo). The
    /// semantic graph itself runs at the project's declared routing
    /// port channel count, and a differing output count is bridged
    /// only by the explicit channel map on the device-sink edge —
    /// never an implicit downmix inside the graph. `engine.block_frames`
    /// is unused on the render path: the drive is bounded by
    /// `max_block`.
    pub engine: EngineConfig,
    /// First emitted position (inclusive).
    pub range_start: u64,
    /// Last emitted position (exclusive).
    pub range_end: u64,
    /// Tail classification — `None` is a preflight failure.
    pub tail: Option<TailPolicy>,
    /// Warm-up positions processed before `range_start` (clamped so
    /// the play position never goes below `0`).
    pub preroll: u64,
    /// Destination path — a `StagedFile` temp sibling is written
    /// first and only published on success.
    pub destination: PathBuf,
    /// Publish policy: allow replacing an existing destination.
    pub replace_existing: bool,
    /// Wire sample encoding (must be in `wav_capabilities().sample_formats`).
    pub wire_format: SampleFormat,
    /// Master-bus insert chain in signal order.
    pub inserts: Vec<Insert>,
    /// Physical-call bound for the render (`BlockContract::max_frames`
    /// and the scheduler callback chunk).
    pub max_block: u32,
    /// Offline support level clip renderers report.
    pub clip_support: OfflineRenderSupport,
    /// Plugin-state quality recipe (TIM-04). `None` = the export
    /// makes no plugin-state claim; `Some` under `Strict` is a
    /// fail-closed preflight gate on the evidence's
    /// `exact_eligible()`.
    pub plugin_state: Option<PluginStateRecipe>,
}

/// A custom muxer constructor over the borrowed staged file.
pub(crate) type DynMuxerCtor =
    Box<dyn for<'a> FnOnce(&'a mut StagedFile) -> Result<Box<dyn Muxer + 'a>, MediaError> + Send>;
