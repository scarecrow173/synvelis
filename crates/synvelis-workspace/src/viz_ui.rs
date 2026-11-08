//! SV-48 — Ruviform `ui/viz` + `ui/binding` headless contract adopted
//! into the Synvelis DAW view layer (issue #48).
//!
//! Upstream vocabulary preserved: source ports (`meta`/`query`/
//! `snapshot`), bounded `buckets` queries, `SourceRevision` checks,
//! `WaveformView` payloads, meter/spectrum snapshot frames with
//! `sequence`/`dropped`, `OverlayState`, `MeterScale`, `VizStyleInput`
//! (paint only), the snapshot pump (`pumpNow`/`staleAfter`), the
//! `ViewportQuery`/`QueryReply` binding wire, `ViewLink`
//! attach/detach/close generations, and `rvf-viewport`/`rvf-select`
//! view events.
//!
//! DAW meaning stays on the Synvelis side: logical source ids
//! (`waveform:clip-…`, `meter:track-…`, `spectrum:probe-…`), clip →
//! source-window resolution, timeline items built from `Project`
//! clips, and routing of select/viewport intents into the editing
//! model all live here or in the host — never inside the upstream
//! elements.
//!
//! RT separation: meter/spectrum ports are pull-of-latest-complete-
//! frame; a producer-side bridge pushes frames into the port on a
//! non-RT path and the UI never calls into engine code synchronously.
//!
//! The TS element (`customElements`) and renderer layers are not
//! ported: upstream `ui/viz` reference code stays canonical; this
//! module mirrors the *model + lifecycle* contract headlessly. All
//! queries are synchronous — the upstream `AbortSignal` collapses to
//! generation + query-sequence bookkeeping.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use synvelis_project::{ClipId, SourceId, TrackId};

use ruviform_viz::{
    BoundedCache, LaneItem, SliceWaveform, WaveformSeries, WaveformSource, items_in,
};

use crate::controls_ui::{ViewId, WindowId};
use crate::session::ProjectSession;

// ---------------------------------------------------------------------------
// wire ids
// ---------------------------------------------------------------------------

/// Opaque viz source identifier on the wire — e.g. `waveform:clip-<tag>`,
/// `meter:track-<tag>`, `spectrum:probe-<id>`.
pub type VizSourceId = String;
/// View-local surface slot — a view may host several viz surfaces.
pub type SurfaceId = String;
/// Opaque query identifier (upstream `ViewportQuery.queryId`).
pub type QueryId = String;

/// A source revision token — upstream `number | string`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SourceRevision {
    /// Numeric revision.
    Num(u64),
    /// Textual revision token.
    Text(String),
}

impl SourceRevision {
    /// Numeric value when this is a `Num` revision.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Num(v) => Some(*v),
            Self::Text(_) => None,
        }
    }
}

impl From<u64> for SourceRevision {
    fn from(v: u64) -> Self {
        Self::Num(v)
    }
}
impl From<String> for SourceRevision {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}
impl From<&str> for SourceRevision {
    fn from(v: &str) -> Self {
        Self::Text(v.to_owned())
    }
}

/// Canonical logical target helpers — the DAW-side source naming scheme.
/// Ports are registered under these ids; bindings resolve DAW ids to them.
#[must_use]
pub fn waveform_clip_target(clip: &ClipId) -> VizSourceId {
    format!("waveform:{}", clip.to_tagged())
}

/// Whole-source waveform target (no clip window restriction).
#[must_use]
pub fn waveform_source_target(source: &SourceId) -> VizSourceId {
    format!("waveform:{}", source.to_tagged())
}

/// Track level-meter target.
#[must_use]
pub fn meter_track_target(track: &TrackId) -> VizSourceId {
    format!("meter:{}", track.to_tagged())
}

/// Master/bus meter target.
#[must_use]
pub fn meter_master_target() -> VizSourceId {
    "meter:master".to_owned()
}

/// Probe/inspector spectrum target.
#[must_use]
pub fn spectrum_probe_target(probe: &str) -> VizSourceId {
    format!("spectrum:{probe}")
}

// ---------------------------------------------------------------------------
// waveform contract (upstream ui/viz WaveformDataSource vocabulary)
// ---------------------------------------------------------------------------

/// Axis unit for waveform queries and spans.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WaveformAxis {
    /// Seconds.
    Seconds,
    /// Sample indices.
    Samples,
    /// Frame indices.
    Frames,
}

/// `WaveformDataSource.meta()` — identity + revision + layout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WaveformMeta {
    /// Source identity (`sourceId`).
    pub id: VizSourceId,
    /// Revision token — queries issued against one revision must not
    /// resolve under another.
    pub revision: SourceRevision,
    /// Axis unit `start`/`end`/`extent` are expressed in.
    pub axis: WaveformAxis,
    /// Axis extent (clip/source length in axis units).
    #[serde(with = "serde_f64")]
    pub extent: f64,
    /// Channel count.
    pub channels: usize,
    /// Sample rate when known (needed to translate `seconds` spans).
    pub sample_rate: Option<u32>,
}

/// `WaveformDataSource.query()` — a bounded read. `buckets` is the
/// number of output bins requested (normally the plot pixel width);
/// the source must never read unbounded data to answer it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WaveformQuery {
    /// Axis-unit start (inclusive).
    #[serde(with = "serde_f64")]
    pub start: f64,
    /// Axis-unit end (exclusive).
    #[serde(with = "serde_f64")]
    pub end: f64,
    /// Output bins per channel.
    pub buckets: u32,
    /// Channels to serve; `None` = all.
    pub channels: Option<Vec<usize>>,
}

/// One aggregate bin — upstream `WaveformBucket`. `count == 0`
/// (and `NaN` statistics) marks a gap: data was missing in the range
/// and must never be zero-filled.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveformBucket {
    /// Minimum finite value (`NaN` when empty).
    #[serde(with = "serde_f64")]
    pub min: f64,
    /// Maximum finite value (`NaN` when empty).
    #[serde(with = "serde_f64")]
    pub max: f64,
    /// RMS over finite values (`NaN` when empty).
    #[serde(with = "serde_f64")]
    pub rms: f64,
    /// Finite samples covered (`0` = gap).
    pub count: u64,
}

/// Bounded query payload — upstream `WaveformView` union.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum WaveformView {
    /// Aggregated bins (normal zoomed-out serving path).
    Bins {
        /// Revision the payload was produced under.
        revision: SourceRevision,
        /// Axis position of the first bin edge.
        #[serde(with = "serde_f64")]
        start: f64,
        /// Axis units covered per bin.
        #[serde(with = "serde_f64")]
        units_per_bucket: f64,
        /// `channels[ch][bin]` buckets.
        channels: Vec<Vec<WaveformBucket>>,
    },
    /// Decimated raw samples (deep zoom serving path).
    Samples {
        /// Revision the payload was produced under.
        revision: SourceRevision,
        /// Axis position of the first sample.
        #[serde(with = "serde_f64")]
        start: f64,
        /// Axis units per returned sample.
        #[serde(with = "serde_f64")]
        units_per_sample: f64,
        /// `channels[ch][i]` samples; `NaN` marks missing data.
        #[serde(with = "serde_f64_matrix")]
        channels: Vec<Vec<f64>>,
    },
}

impl WaveformView {
    /// The revision stamped on this payload.
    #[must_use]
    pub fn revision(&self) -> &SourceRevision {
        match self {
            Self::Bins { revision, .. } | Self::Samples { revision, .. } => revision,
        }
    }

    /// Payload byte estimate for `maxBytes` enforcement.
    #[must_use]
    pub fn byte_len(&self) -> usize {
        match self {
            Self::Bins { channels, .. } => channels.iter().map(Vec::len).sum::<usize>() * 4 * 8,
            Self::Samples { channels, .. } => channels.iter().map(Vec::len).sum::<usize>() * 8,
        }
    }
}

// ---------------------------------------------------------------------------
// meter contract (upstream MeterDataSource / MeterFrame)
// ---------------------------------------------------------------------------

/// One meter channel sample — `NaN` marks an unavailable channel.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeterChannel {
    /// Instantaneous peak.
    #[serde(with = "serde_f64")]
    pub peak: f64,
    /// RMS level.
    #[serde(with = "serde_f64")]
    pub rms: f64,
    /// Held peak value (peak-hold). `Some(NaN)` serializes as `null`
    /// and is indistinguishable from `None` on the wire — documented
    /// narrowing of the JSON narrowing.
    pub held_peak: Option<f64>,
    /// Clip flag.
    pub clip: Option<bool>,
}

/// `MeterDataSource.meta()`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeterMeta {
    /// Source identity.
    pub id: VizSourceId,
    /// Revision token.
    pub revision: SourceRevision,
    /// Channel count the source will report.
    pub channels: usize,
}

/// One complete meter snapshot — upstream `MeterFrame`. Complete
/// frames only: partial frames are never delivered.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeterFrame {
    /// Producer sequence number — gaps count toward the dropped
    /// bookkeeping as loss.
    pub sequence: u64,
    /// One entry per channel.
    pub channels: Vec<MeterChannel>,
    /// Producer-reported dropped count since the last frame.
    pub dropped: Option<u64>,
    /// Producer timestamp (ms), opaque to the element.
    pub timestamp: Option<f64>,
}

// ---------------------------------------------------------------------------
// spectrum contract (upstream SpectrumDataSource / SpectrumFrame)
// ---------------------------------------------------------------------------

/// Frequency axis of spectrum bins — upstream `BinAxis`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum SpectrumBinsAxis {
    /// `first_hz + i * step_hz`.
    Uniform {
        /// First bin centre frequency.
        #[serde(with = "serde_f64")]
        first_hz: f64,
        /// Bin spacing.
        #[serde(with = "serde_f64")]
        step_hz: f64,
    },
    /// Explicit per-bin centre frequencies.
    Centers {
        /// Centre frequency per bin.
        #[serde(with = "serde_f64_vec")]
        centers_hz: Vec<f64>,
    },
}

/// `SpectrumDataSource.meta()`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpectrumMeta {
    /// Source identity.
    pub id: VizSourceId,
    /// Revision token.
    pub revision: SourceRevision,
    /// Bin count the source will report.
    pub bins: usize,
    /// Frequency axis of the bins.
    pub axis: SpectrumBinsAxis,
    /// Sample rate when known.
    pub sample_rate: Option<u32>,
}

/// One complete spectrum snapshot — upstream `SpectrumFrame`.
/// `NaN`/non-finite magnitudes mark unavailable bins.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpectrumFrame {
    /// Producer sequence number — gaps count toward loss bookkeeping.
    pub sequence: u64,
    /// Magnitude per bin.
    #[serde(with = "serde_f64_vec")]
    pub magnitudes: Vec<f64>,
    /// Producer timestamp (ms), opaque.
    pub timestamp: Option<f64>,
}

// ---------------------------------------------------------------------------
// query binding wire (upstream ui/binding ViewportQuery/QueryReply)
// ---------------------------------------------------------------------------

/// `ViewportQuery` — the binding-layer wire form of a waveform query.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ViewportQuery {
    /// Caller-correlation id.
    pub query_id: QueryId,
    /// Source the query targets.
    pub source_id: VizSourceId,
    /// Revision the query was issued against.
    pub source_revision: SourceRevision,
    /// Issuing view.
    pub view_id: ViewId,
    /// View generation at issue time — a reply under any other
    /// generation is stale.
    pub generation: u64,
    /// Axis-unit start.
    #[serde(with = "serde_f64")]
    pub start: f64,
    /// Axis-unit end.
    #[serde(with = "serde_f64")]
    pub end: f64,
    /// Requested bins (plot px).
    pub buckets: Option<u32>,
    /// Channel subset.
    pub channels: Option<Vec<usize>>,
    /// Reply byte budget.
    pub max_bytes: Option<usize>,
    /// Payload encoding preference.
    pub encoding: Option<String>,
}

/// Reply payload — upstream `Value | Vec<u8>` narrowed to the viz
/// contract (`WaveformView` is the structured form).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum QueryPayload {
    /// Structured waveform view.
    View(WaveformView),
    /// Encoded bytes.
    Bytes(Vec<u8>),
}

/// `QueryReply` — a successful wire response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryReply {
    /// Correlated query id.
    pub query_id: QueryId,
    /// Generation the reply resolves under.
    pub generation: u64,
    /// Revision the payload was produced under.
    pub source_revision: SourceRevision,
    /// The payload.
    pub payload: QueryPayload,
    /// Encoded byte size when the payload was encoded.
    pub bytes: Option<usize>,
}

/// Upstream `QueryErrorKind` — every query resolves to an outcome,
/// never a thrown exception.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueryErrorKind {
    /// Cancelled (superseded / detached).
    Cancelled,
    /// Revision/generation mismatch — the reply is stale.
    Stale,
    /// Reply exceeded the byte budget.
    OverBudget,
    /// The source cannot serve the request.
    Unsupported,
    /// Any other failure.
    Error,
}

/// Structured query error.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VizQueryError {
    /// Machine kind.
    pub kind: QueryErrorKind,
    /// Human detail.
    pub detail: Option<String>,
}

impl VizQueryError {
    /// Build an error of `kind`.
    #[must_use]
    pub fn new(kind: QueryErrorKind, detail: impl Into<Option<String>>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for VizQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let detail = self.detail.as_deref().unwrap_or("-");
        write!(f, "{:?}: {detail}", self.kind)
    }
}

impl std::error::Error for VizQueryError {}

/// JSON narrowing for `f64` — non-finite values serialize as `null`
/// and `null` deserializes as `NaN`, so gap/missing-data markers
/// survive a JSON round trip instead of hard-failing the payload.
mod serde_f64 {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    // `serialize_with` requires a reference receiver — serde's contract,
    // not a style choice.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
        if v.is_finite() {
            v.serialize(s)
        } else {
            s.serialize_none()
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
        Ok(Option::<f64>::deserialize(d)?.unwrap_or(f64::NAN))
    }
}

/// `Vec<f64>` with the same narrowing element-wise.
mod serde_f64_vec {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(v: &[f64], s: S) -> Result<S::Ok, S::Error> {
        v.iter()
            .map(|x| if x.is_finite() { Some(*x) } else { None })
            .collect::<Vec<Option<f64>>>()
            .serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f64>, D::Error> {
        Ok(Vec::<Option<f64>>::deserialize(d)?
            .into_iter()
            .map(|o| o.unwrap_or(f64::NAN))
            .collect())
    }
}

/// `Vec<Vec<f64>>` with the same narrowing element-wise.
mod serde_f64_matrix {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(v: &[Vec<f64>], s: S) -> Result<S::Ok, S::Error> {
        v.iter()
            .map(|row| {
                row.iter()
                    .map(|x| if x.is_finite() { Some(*x) } else { None })
                    .collect::<Vec<Option<f64>>>()
            })
            .collect::<Vec<Vec<Option<f64>>>>()
            .serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Vec<f64>>, D::Error> {
        Ok(Vec::<Vec<Option<f64>>>::deserialize(d)?
            .into_iter()
            .map(|r| r.into_iter().map(|o| o.unwrap_or(f64::NAN)).collect())
            .collect())
    }
}

/// `QueryOutcome` — ok reply or structured error.
#[derive(Clone, Debug, PartialEq)]
pub enum QueryOutcome {
    /// Successful reply.
    Ok(QueryReply),
    /// Structured failure.
    Err(VizQueryError),
}

// ---------------------------------------------------------------------------
// display model — overlays, scales, paint-only style
// ---------------------------------------------------------------------------

/// A normalized axis-unit range.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeRange {
    /// Earlier endpoint.
    #[serde(with = "serde_f64")]
    pub start: f64,
    /// Later endpoint.
    #[serde(with = "serde_f64")]
    pub end: f64,
}

/// Generic overlay state — upstream `OverlayState`. Pure positions;
/// what a selection or marker *means* is the consumer's business.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OverlayState {
    /// Committed selection (kept by the consumer, not the element).
    pub selection: Option<TimeRange>,
    /// Playhead position.
    pub playhead: Option<f64>,
    /// Point markers.
    pub markers: Vec<OverlayMarker>,
}

/// One overlay marker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayMarker {
    /// Axis position.
    #[serde(with = "serde_f64")]
    pub at: f64,
    /// Optional label.
    pub label: Option<String>,
    /// Paint hint (never semantic).
    pub color: Option<String>,
}

/// Meter dB scale — upstream `MeterScale` (default `-60..+3 dB`,
/// segments at `-18`/`-6`, clip at `0`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeterScale {
    /// Bottom of scale (dB).
    #[serde(with = "serde_f64")]
    pub min_db: f64,
    /// Top of scale (dB).
    #[serde(with = "serde_f64")]
    pub max_db: f64,
    /// Segment thresholds (dB), low → high.
    #[serde(with = "serde_f64_vec")]
    pub segments: Vec<f64>,
    /// Clip threshold (dB).
    #[serde(with = "serde_f64")]
    pub clip_db: f64,
}

impl Default for MeterScale {
    fn default() -> Self {
        Self {
            min_db: -60.0,
            max_db: 3.0,
            segments: vec![-18.0, -6.0],
            clip_db: 0.0,
        }
    }
}

/// Frequency scale selector — upstream `freq.scale`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FreqScale {
    /// Linear frequency axis.
    Linear,
    /// Logarithmic frequency axis.
    Log,
}

/// Spectrum display mapping — upstream `display.freq`/`display.db`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpectrumScale {
    /// Frequency scale.
    pub freq: FreqScale,
    /// Minimum frequency shown (Hz).
    #[serde(with = "serde_f64")]
    pub min_hz: f64,
    /// Maximum frequency shown (Hz).
    #[serde(with = "serde_f64")]
    pub max_hz: f64,
    /// Bottom of the dB range.
    #[serde(with = "serde_f64")]
    pub min_db: f64,
    /// Top of the dB range.
    #[serde(with = "serde_f64")]
    pub max_db: f64,
    /// Magnitude → dB reference (1.0 = raw amplitude).
    #[serde(with = "serde_f64")]
    pub reference: f64,
}

impl Default for SpectrumScale {
    fn default() -> Self {
        Self {
            freq: FreqScale::Log,
            min_hz: 20.0,
            max_hz: 20_000.0,
            min_db: -90.0,
            max_db: 0.0,
            reference: 1.0,
        }
    }
}

/// Paint-only theme/style contract — upstream `VizStyleInput`.
/// Every field is optional; `None` leaves the current paint unchanged.
/// Style never encodes data meaning: missing/stale states are
/// structural cues the element renders; the palette may only restyle
/// them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VizStyleInput {
    /// Theme name.
    pub theme: Option<String>,
    /// Background paint.
    pub background: Option<String>,
    /// Gridline paint.
    pub grid: Option<String>,
    /// Axis/rule paint.
    pub axis: Option<String>,
    /// Label text paint.
    pub text: Option<String>,
    /// Waveform fill/stroke paint.
    pub waveform: Option<String>,
    /// Meter segment palette (low → high).
    pub meter_palette: Option<Vec<String>>,
    /// Spectrum colormap name/palette id.
    pub spectrum_colormap: Option<String>,
    /// Selection overlay paint.
    pub selection: Option<String>,
    /// Playhead overlay paint.
    pub playhead: Option<String>,
    /// Missing-data hatch paint.
    pub state_missing: Option<String>,
    /// Stale-data hatch paint.
    pub state_stale: Option<String>,
}

/// Element status — the textual/non-color state channel (upstream
/// `status` + `statusKind`). Drives "no source" / "awaiting data" /
/// "stale" / "N dropped" / "source error" cues independently of paint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum VizStatus {
    /// Data current.
    Ok,
    /// No source bound.
    NoSource,
    /// Source bound, no data landed yet.
    AwaitingData,
    /// Data on screen is aged/stale.
    Stale,
    /// Source produced an error.
    SourceError {
        /// Producer/adapter detail.
        detail: String,
    },
    /// Capability unsupported (e.g. backend unavailable).
    Unsupported {
        /// Capability detail.
        detail: String,
    },
}

/// What caused a viewport event — upstream `rvf-viewport` `source`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewportTrigger {
    /// Programmatic span set.
    Set,
    /// Drag pan.
    Pan,
    /// Wheel zoom.
    Wheel,
    /// Viewport element resized.
    Resize,
}

/// Outbound view event — the `rvf-viewport`/`rvf-select` equivalents.
/// The host drains these and routes them through the editing model
/// (selection commands, linked zoom); the viz layer never writes
/// project state itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum VizViewEvent {
    /// `rvf-viewport`: span changed.
    Viewport {
        /// Owning view.
        view_id: ViewId,
        /// Emitting surface.
        surface_id: SurfaceId,
        /// New span.
        #[serde(with = "serde_f64")]
        start: f64,
        /// New span end.
        #[serde(with = "serde_f64")]
        end: f64,
        /// Interaction source.
        trigger: ViewportTrigger,
    },
    /// `rvf-select`: a selection gesture completed.
    Select {
        /// Owning view.
        view_id: ViewId,
        /// Emitting surface.
        surface_id: SurfaceId,
        /// Selection start (axis units).
        #[serde(with = "serde_f64")]
        start: f64,
        /// Selection end (axis units).
        #[serde(with = "serde_f64")]
        end: f64,
    },
}

// ---------------------------------------------------------------------------
// timeline contract
// ---------------------------------------------------------------------------

/// Axis unit a timeline surface presents.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimelineAxis {
    /// Project beats (PPQN ticks / ppqn).
    Beats,
    /// Seconds through the tempo map at the surface's `media_rate`.
    Seconds,
}

/// DAW payload carried on a timeline item — opaque to the generic
/// lane/hit-test machinery, meaningful to Synvelis.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimelinePayload {
    /// Owning clip (tagged).
    pub clip: String,
    /// Lane the clip sits on (tagged).
    pub lane: String,
    /// Source the clip reads (tagged).
    pub source: String,
    /// Muted clips render differently but still take a lane slot.
    pub muted: bool,
}

/// One timeline item — a `LaneItem<TimelinePayload>` in the surface's
/// axis unit.
pub type TimelineItem = LaneItem<TimelinePayload>;

// ---------------------------------------------------------------------------
// errors
// ---------------------------------------------------------------------------

/// Host-facing API errors. Stale data is *not* an error — it is
/// surfaced on the row/status contract instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VizError {
    /// The view is unknown to `VizUi`.
    UnknownView(ViewId),
    /// The view exists but is detached.
    DetachedView(ViewId),
    /// The surface is unknown on that view.
    UnknownSurface(ViewId, SurfaceId),
    /// The surface exists but has a different kind.
    KindMismatch {
        /// Expected kind name.
        expected: &'static str,
        /// Owning view.
        view: ViewId,
        /// Surface id.
        surface: SurfaceId,
    },
    /// A span argument is non-finite or `end <= start`.
    InvalidSpan,
    /// A pixel/bucket argument is zero.
    InvalidPixels,
    /// A scale/config argument is invalid.
    InvalidConfig(&'static str),
}

impl std::fmt::Display for VizError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownView(v) => write!(f, "unknown view {v}"),
            Self::DetachedView(v) => write!(f, "view {v} is detached"),
            Self::UnknownSurface(v, s) => write!(f, "unknown surface {s} on view {v}"),
            Self::KindMismatch {
                expected,
                view,
                surface,
            } => write!(f, "surface {surface} on {view} is not a {expected}"),
            Self::InvalidSpan => write!(f, "span must be finite with end > start"),
            Self::InvalidPixels => write!(f, "pixel count must be > 0"),
            Self::InvalidConfig(m) => write!(f, "invalid config: {m}"),
        }
    }
}

impl std::error::Error for VizError {}

// ---------------------------------------------------------------------------
// source ports (upstream WaveformDataSource / MeterDataSource /
// SpectrumDataSource). `subscribe(cb)` collapses into
// `VizUi::notify_source_changed` — the notification channel is the
// gateway, so port implementations stay pull-only and RT-safe.
// ---------------------------------------------------------------------------

/// `WaveformDataSource` — bounded queries over one logical source.
/// Implementations are non-RT: reads may block on decode/IO and are
/// always bounded by `buckets`.
pub trait WaveformPort: Send + Sync {
    /// Identity + revision + layout.
    fn meta(&self) -> WaveformMeta;
    /// Serve a bounded query. Replies must stamp the revision they
    /// were produced under; stale responses are rejected by `VizUi`.
    ///
    /// # Errors
    /// [`VizQueryError`] on any failure — `Unsupported` when the
    /// request cannot be served at all.
    fn query(&self, q: &WaveformQuery) -> Result<WaveformView, VizQueryError>;
}

/// `MeterDataSource` — pull of the latest complete frame. Must be
/// cheap and non-blocking: producers push frames into the port on a
/// non-RT path; the UI never calls into engine state.
pub trait MeterPort: Send + Sync {
    /// Identity + channel layout.
    fn meta(&self) -> MeterMeta;
    /// Latest complete frame, `None` while no frame has arrived.
    ///
    /// # Errors
    /// Propagates a producer/bridge failure — surfaced as
    /// `VizStatus::SourceError`; the pump survives it.
    fn snapshot(&self) -> Result<Option<MeterFrame>, String>;
}

/// `SpectrumDataSource` — pull of the latest complete frame.
pub trait SpectrumPort: Send + Sync {
    /// Identity + bin layout.
    fn meta(&self) -> SpectrumMeta;
    /// Latest complete frame, `None` while no frame has arrived.
    ///
    /// # Errors
    /// Same contract as [`MeterPort::snapshot`].
    fn snapshot(&self) -> Result<Option<SpectrumFrame>, String>;
}

/// Registered port variants.
enum Port {
    Waveform(Arc<dyn WaveformPort>),
    Meter(Arc<dyn MeterPort>),
    Spectrum(Arc<dyn SpectrumPort>),
}

// ---------------------------------------------------------------------------
// fixtures — upstream MemoryWaveformSource / ManualSnapshotSource
// equivalents. Also usable as real adapters: `MemoryWaveform` bridges
// `ruviform_viz` (LOD pyramid / bounded on-demand reduction) onto the
// ui/viz query contract exactly like the upstream bridge sources.
// ---------------------------------------------------------------------------

/// A `WaveformSource` sub-range — a clip reading only its source
/// window. Generic adapter: offsets queries into `inner`.
pub struct SubrangeWaveform {
    inner: Arc<dyn ruviform_viz::WaveformSource>,
    start: u64,
    len: u64,
}

impl SubrangeWaveform {
    /// Wraps `inner` so only `[start, start+len)` is visible.
    #[must_use]
    pub fn new(inner: Arc<dyn ruviform_viz::WaveformSource>, start: u64, len: u64) -> Self {
        Self { inner, start, len }
    }
}

impl ruviform_viz::WaveformSource for SubrangeWaveform {
    fn info(&self) -> ruviform_viz::WaveformInfo {
        let info = self.inner.info();
        ruviform_viz::WaveformInfo {
            frames: self.len.min(info.frames.saturating_sub(self.start)),
            ..info
        }
    }

    fn read(&self, start: u64, frames: usize) -> ruviform_viz::Result<ruviform_viz::PcmBlock> {
        let avail = self.info().frames.saturating_sub(start);
        let n = usize::try_from(avail).unwrap_or(usize::MAX).min(frames);
        let mut block = self.inner.read(self.start.saturating_add(start), n)?;
        block.start = start;
        Ok(block)
    }
}

/// De-interleaves per-channel PCM into the `SliceWaveform` layout
/// (frame-major); shorter channels pad with `NaN` — missing data, not
/// silence.
fn interleave(channels: &[Vec<f64>]) -> Vec<f64> {
    let frames = channels.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = vec![f64::NAN; frames * channels.len()];
    for (c, ch) in channels.iter().enumerate() {
        for (f, &v) in ch.iter().enumerate() {
            out[f * channels.len() + c] = v;
        }
    }
    out
}

/// In-memory waveform port — the `MemoryWaveformSource` mirror.
/// Serves `Bins` views via `ruviform_viz::WaveformSeries` (bounded
/// LOD/on-demand reduction); `NaN` samples stay gaps.
pub struct MemoryWaveform {
    meta: WaveformMeta,
    series: WaveformSeries,
    rate: u32,
    axis: WaveformAxis,
}

impl MemoryWaveform {
    /// Builds a port over `channels` PCM at `rate` Hz, axis `axis`.
    ///
    /// # Panics
    /// On empty `channels` or `rate == 0` (`SliceWaveform` validates;
    /// unaligned channel lengths are NaN-padded, not a panic).
    #[must_use]
    pub fn new(id: VizSourceId, rate: u32, axis: WaveformAxis, channels: &[Vec<f64>]) -> Self {
        let n_ch = channels.len();
        let source = Arc::new(
            SliceWaveform::new(rate, n_ch, interleave(channels))
                .expect("fixture channels are non-empty and aligned"),
        );
        let info = source.info();
        let series = WaveformSeries::on_demand(source);
        Self {
            meta: WaveformMeta {
                id,
                revision: SourceRevision::Num(0),
                axis,
                extent: extent_in_axis(info.frames, rate, axis),
                channels: info.channels,
                sample_rate: Some(rate),
            },
            series,
            rate,
            axis,
        }
    }

    /// Builds a port restricted to `[start, start+len)` source frames —
    /// the clip-source-window path.
    ///
    /// # Panics
    /// On empty `channels` or `rate == 0` (`SliceWaveform` validates;
    /// unaligned channel lengths are NaN-padded, not a panic).
    #[must_use]
    pub fn window(
        id: VizSourceId,
        rate: u32,
        axis: WaveformAxis,
        channels: &[Vec<f64>],
        start: u64,
        len: u64,
    ) -> Self {
        let n_ch = channels.len();
        let inner: Arc<dyn ruviform_viz::WaveformSource> = Arc::new(
            SliceWaveform::new(rate, n_ch, interleave(channels))
                .expect("fixture channels are non-empty and aligned"),
        );
        let source = Arc::new(SubrangeWaveform::new(inner, start, len));
        let info = source.info();
        let series = WaveformSeries::on_demand(source);
        Self {
            meta: WaveformMeta {
                id,
                revision: SourceRevision::Num(0),
                axis,
                extent: extent_in_axis(info.frames, rate, axis),
                channels: info.channels,
                sample_rate: Some(rate),
            },
            series,
            rate,
            axis,
        }
    }

    /// Builds a port over an arbitrary `WaveformSource` — including a
    /// `WaveformSeries` with a prebuilt LOD pyramid.
    #[must_use]
    pub fn from_series(
        id: VizSourceId,
        revision: SourceRevision,
        rate: u32,
        axis: WaveformAxis,
        series: WaveformSeries,
    ) -> Self {
        let info = series.info();
        Self {
            meta: WaveformMeta {
                id,
                revision,
                axis,
                extent: extent_in_axis(info.frames, rate, axis),
                channels: info.channels,
                sample_rate: Some(rate),
            },
            series,
            rate,
            axis,
        }
    }

    /// Axis position → sample index (clamped to `[0, frames]`).
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // non-finite and negative `at` return 0 above; `.round()` then `.min(frames)` bounds the value
    fn to_sample(&self, at: f64) -> u64 {
        let frames = self.meta_frames();
        match self.axis {
            WaveformAxis::Samples | WaveformAxis::Frames => {
                if !(at.is_finite()) || at <= 0.0 {
                    0
                } else {
                    (at.round() as u64).min(frames)
                }
            }
            WaveformAxis::Seconds => {
                if !(at.is_finite()) || at <= 0.0 {
                    0
                } else {
                    ((at * f64::from(self.rate)).round() as u64).min(frames)
                }
            }
        }
    }

    fn meta_frames(&self) -> u64 {
        self.series.info().frames
    }
}

impl WaveformPort for MemoryWaveform {
    fn meta(&self) -> WaveformMeta {
        self.meta.clone()
    }

    #[allow(clippy::cast_precision_loss)] // bin index/count stay far under 2^53 (bounded by max_bytes)
    fn query(&self, q: &WaveformQuery) -> Result<WaveformView, VizQueryError> {
        if q.buckets == 0 {
            return Err(VizQueryError::new(
                QueryErrorKind::Error,
                Some("buckets must be > 0".into()),
            ));
        }
        if q.buckets > MAX_BUCKETS {
            return Err(VizQueryError::new(
                QueryErrorKind::OverBudget,
                Some(format!("buckets {} over cap {MAX_BUCKETS}", q.buckets)),
            ));
        }
        if !q.start.is_finite() || !q.end.is_finite() || q.end <= q.start {
            return Err(VizQueryError::new(
                QueryErrorKind::Error,
                Some("invalid span".into()),
            ));
        }
        let chans: Vec<usize> = match &q.channels {
            Some(c) => c.clone(),
            None => (0..self.meta.channels).collect(),
        };
        for &c in &chans {
            if c >= self.meta.channels {
                return Err(VizQueryError::new(
                    QueryErrorKind::Unsupported,
                    Some(format!("channel {c}")),
                ));
            }
        }
        // Bin over the *requested* span, not the data-clamped one:
        // bins outside the source extent stay `EMPTY` (pan past the
        // edge shows gaps, never clamps silently). Per-bin reduction
        // through `WaveformSeries` keeps every read bounded and keeps
        // bin edges exactly aligned to the request.
        let buckets = q.buckets as usize;
        let upb = (q.end - q.start) / f64::from(q.buckets);
        let mut out = Vec::with_capacity(chans.len());
        for channel in chans {
            let mut row = Vec::with_capacity(buckets);
            for bin in 0..buckets {
                let bin_start = self.to_sample(q.start + bin as f64 * upb);
                let bin_end = self.to_sample(q.start + (bin as f64 + 1.0) * upb);
                let bucket = if bin_start >= bin_end {
                    WaveformBucket {
                        min: f64::NAN,
                        max: f64::NAN,
                        rms: f64::NAN,
                        count: 0,
                    }
                } else {
                    let bins = self
                        .series
                        .view(channel, bin_start, bin_end, 1)
                        .map_err(|err| {
                            VizQueryError::new(QueryErrorKind::Error, Some(err.to_string()))
                        })?;
                    let lod = bins
                        .first()
                        .map_or(ruviform_viz::LodBucket::EMPTY, |b| b.bucket);
                    WaveformBucket {
                        min: lod.min,
                        max: lod.max,
                        rms: lod.rms,
                        count: lod.count,
                    }
                };
                row.push(bucket);
            }
            out.push(row);
        }
        Ok(WaveformView::Bins {
            revision: self.meta.revision.clone(),
            start: q.start,
            units_per_bucket: (q.end - q.start) / f64::from(q.buckets),
            channels: out,
        })
    }
}

/// Mutable waveform fixture — upstream's swappable test source.
/// `set_data` swaps the PCM and bumps the revision, exercising the
/// stale-response / re-subscribe paths.
pub struct ManualWaveform {
    inner: Mutex<MemoryWaveform>,
}

impl ManualWaveform {
    /// New fixture.
    ///
    /// # Panics
    /// On empty `channels` or `rate == 0` (`SliceWaveform` validates;
    /// unaligned channel lengths are NaN-padded, not a panic).
    #[must_use]
    pub fn new(id: VizSourceId, rate: u32, axis: WaveformAxis, channels: &[Vec<f64>]) -> Self {
        Self {
            inner: Mutex::new(MemoryWaveform::new(id, rate, axis, channels)),
        }
    }

    /// Swaps PCM + bumps the revision. Returns the new revision.
    ///
    /// # Panics
    /// On empty `channels` or `rate == 0` (`SliceWaveform` validates;
    /// unaligned channel lengths are NaN-padded, not a panic).
    pub fn set_data(&self, channels: &[Vec<f64>]) -> SourceRevision {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let rev = guard.meta.revision.as_u64().unwrap_or(0) + 1;
        let id = guard.meta.id.clone();
        let rate = guard.rate;
        let axis = guard.axis;
        let mut next = MemoryWaveform::new(id, rate, axis, channels);
        next.meta.revision = SourceRevision::Num(rev);
        *guard = next;
        SourceRevision::Num(rev)
    }
}

impl WaveformPort for ManualWaveform {
    fn meta(&self) -> WaveformMeta {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .meta()
    }

    fn query(&self, q: &WaveformQuery) -> Result<WaveformView, VizQueryError> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .query(q)
    }
}

/// Manual meter port — upstream `ManualSnapshotSource<MeterFrame>`.
/// The producer pushes complete frames on a non-RT path; `snapshot()`
/// pulls the latest.
pub struct ManualMeter {
    meta: Mutex<MeterMeta>,
    frame: Mutex<Option<MeterFrame>>,
}

impl ManualMeter {
    /// New fixture with `channels` reported channels.
    #[must_use]
    pub fn new(id: VizSourceId, channels: usize) -> Self {
        Self {
            meta: Mutex::new(MeterMeta {
                id,
                revision: SourceRevision::Num(0),
                channels,
            }),
            frame: Mutex::new(None),
        }
    }

    /// Publishes a complete frame (the RT→UI boundary: producer side).
    pub fn push(&self, frame: MeterFrame) {
        *self
            .frame
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(frame);
        let mut meta = self
            .meta
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        meta.revision = SourceRevision::Num(meta.revision.as_u64().unwrap_or(0) + 1);
    }
}

impl MeterPort for ManualMeter {
    fn meta(&self) -> MeterMeta {
        self.meta
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn snapshot(&self) -> Result<Option<MeterFrame>, String> {
        Ok(self
            .frame
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }
}

/// Manual spectrum port — `ManualSnapshotSource<SpectrumFrame>`.
pub struct ManualSpectrum {
    meta: Mutex<SpectrumMeta>,
    frame: Mutex<Option<SpectrumFrame>>,
}

impl ManualSpectrum {
    /// New fixture with `bins` bins on a uniform axis.
    #[must_use]
    pub fn new(id: VizSourceId, bins: usize, first_hz: f64, step_hz: f64) -> Self {
        Self {
            meta: Mutex::new(SpectrumMeta {
                id,
                revision: SourceRevision::Num(0),
                bins,
                axis: SpectrumBinsAxis::Uniform { first_hz, step_hz },
                sample_rate: None,
            }),
            frame: Mutex::new(None),
        }
    }

    /// Publishes a complete frame.
    pub fn push(&self, frame: SpectrumFrame) {
        *self
            .frame
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(frame);
        let mut meta = self
            .meta
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        meta.revision = SourceRevision::Num(meta.revision.as_u64().unwrap_or(0) + 1);
    }
}

impl SpectrumPort for ManualSpectrum {
    fn meta(&self) -> SpectrumMeta {
        self.meta
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn snapshot(&self) -> Result<Option<SpectrumFrame>, String> {
        Ok(self
            .frame
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }
}

/// Extent conversion helper — frames → axis units.
#[allow(clippy::cast_precision_loss)] // fixture extents stay far under 2^53
fn extent_in_axis(frames: u64, rate: u32, axis: WaveformAxis) -> f64 {
    match axis {
        WaveformAxis::Samples | WaveformAxis::Frames => frames as f64,
        WaveformAxis::Seconds => frames as f64 / f64::from(rate),
    }
}

/// Tick/frame index → f64 axis units — project positions stay under 2^53.
#[allow(clippy::cast_precision_loss)] // timeline positions stay far under 2^53
fn tick_as_f64(v: i64) -> f64 {
    v as f64
}

// ---------------------------------------------------------------------------
// surface state (element mirrors)
// ---------------------------------------------------------------------------

/// Cache key — upstream `viewKey(sourceId+rev, channel, start, end,
/// buckets)` widened to the whole reply (`channels` set included).
/// Style/theme is never part of the key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct WaveformCacheKey {
    source: VizSourceId,
    revision: SourceRevision,
    start: u64,
    end: u64,
    buckets: u32,
    channels: Option<Vec<usize>>,
}

/// Waveform surface — `RvfWaveform` model mirror.
struct WaveformSurface {
    source: VizSourceId,
    meta: Option<WaveformMeta>,
    span: Option<TimeRange>,
    plot_px: u32,
    overlays: OverlayState,
    range: Option<(f64, f64)>,
    channel_labels: Option<Vec<Option<String>>>,
    cache: BoundedCache<WaveformCacheKey, WaveformView>,
    view: Option<WaveformView>,
    view_stale: bool,
    status: VizStatus,
    query_seq: u64,
    last_query: Option<ViewportQuery>,
}

/// Meter surface — `RvfMeter` model mirror.
struct MeterSurface {
    source: VizSourceId,
    meta: Option<MeterMeta>,
    scale: MeterScale,
    frame: Option<MeterFrame>,
    last_seq: Option<u64>,
    last_pull_ms: Option<u64>,
    dropped: u64,
    stale: bool,
    status: VizStatus,
}

/// Spectrum surface — `RvfSpectrum` model mirror.
struct SpectrumSurface {
    source: VizSourceId,
    meta: Option<SpectrumMeta>,
    scale: SpectrumScale,
    frame: Option<SpectrumFrame>,
    last_seq: Option<u64>,
    last_pull_ms: Option<u64>,
    dropped: u64,
    stale: bool,
    status: VizStatus,
}

/// Timeline surface — project-backed items + view-local span/overlays.
struct TimelineSurface {
    axis: TimelineAxis,
    media_rate: u32,
    span: Option<TimeRange>,
    plot_px: u32,
    overlays: OverlayState,
    items: Vec<TimelineItem>,
    unresolved: usize,
    synced_rev: Option<u64>,
    status: VizStatus,
}

/// One bound surface.
enum Surface {
    /// Waveform element.
    Waveform(Box<WaveformSurface>),
    /// Meter element.
    Meter(MeterSurface),
    /// Spectrum element.
    Spectrum(SpectrumSurface),
    /// Timeline element.
    Timeline(TimelineSurface),
}

// ---------------------------------------------------------------------------
// read rows (host-facing state)
// ---------------------------------------------------------------------------

/// Waveform surface row.
#[derive(Clone, Debug)]
pub struct WaveformRow {
    /// Bound source.
    pub source: VizSourceId,
    /// Last known meta (None = source unresolved).
    pub meta: Option<WaveformMeta>,
    /// Current span.
    pub span: Option<TimeRange>,
    /// Plot pixel width (bucket budget).
    pub plot_px: u32,
    /// Current overlays.
    pub overlays: OverlayState,
    /// Landed view (None = no data yet).
    pub view: Option<WaveformView>,
    /// The on-screen view is stale (source moved under it).
    pub view_stale: bool,
    /// Status channel.
    pub status: VizStatus,
    /// Cache entries held.
    pub cache_entries: usize,
    /// Cache bytes held.
    pub cache_bytes: usize,
}

/// Meter surface row.
#[derive(Clone, Debug)]
pub struct MeterRow {
    /// Bound source.
    pub source: VizSourceId,
    /// Last known meta.
    pub meta: Option<MeterMeta>,
    /// Display scale.
    pub scale: MeterScale,
    /// Latest landed frame.
    pub frame: Option<MeterFrame>,
    /// Cumulative loss — producer `dropped` + detected seq gaps.
    pub dropped: u64,
    /// Frame aged past `staleAfterMs`.
    pub stale: bool,
    /// Status channel.
    pub status: VizStatus,
}

/// Spectrum surface row.
#[derive(Clone, Debug)]
pub struct SpectrumRow {
    /// Bound source.
    pub source: VizSourceId,
    /// Last known meta.
    pub meta: Option<SpectrumMeta>,
    /// Display mapping.
    pub scale: SpectrumScale,
    /// Latest landed frame.
    pub frame: Option<SpectrumFrame>,
    /// Cumulative loss.
    pub dropped: u64,
    /// Aged past `staleAfterMs`.
    pub stale: bool,
    /// Status channel.
    pub status: VizStatus,
}

/// Timeline surface row.
#[derive(Clone, Debug)]
pub struct TimelineRow {
    /// Axis unit.
    pub axis: TimelineAxis,
    /// Current span.
    pub span: Option<TimeRange>,
    /// Items visible inside `span` (the bounded read).
    pub items: Vec<TimelineItem>,
    /// Clips skipped as unresolvable at the last sync.
    pub unresolved: usize,
    /// Project revision the items were built under (`None` = never
    /// synced — the stale guard stays up).
    pub synced_rev: Option<u64>,
    /// Current overlays.
    pub overlays: OverlayState,
    /// Status channel.
    pub status: VizStatus,
}

/// Counters for verification + diagnostics.
#[derive(Clone, Debug, Default)]
pub struct VizDiagnostics {
    /// Waveform queries issued to ports.
    pub queries_issued: u64,
    /// Replies landed as current.
    pub queries_resolved: u64,
    /// Replies rejected as stale (incl. retry discards).
    pub queries_stale: u64,
    /// Queries cancelled (detach/close).
    pub queries_cancelled: u64,
    /// Queries failed (port error / over budget).
    pub queries_failed: u64,
    /// Waveform cache hits.
    pub cache_hits: u64,
    /// Waveform cache misses.
    pub cache_misses: u64,
    /// Pump ticks run.
    pub pumps: u64,
    /// Snapshot frames landed.
    pub frames_pulled: u64,
    /// Outbound events dropped at the queue cap.
    pub events_dropped: u64,
    /// Producer-reported dropped accumulated.
    pub producer_dropped: u64,
    /// Sequence-gap loss accumulated.
    pub seq_gaps: u64,
    /// Stale transitions.
    pub stale_transitions: u64,
    /// Theme/overlay/scale repaints (no re-query).
    pub repaints: u64,
    /// View attaches.
    pub attaches: u64,
    /// View detaches.
    pub detaches: u64,
    /// Timeline resyncs.
    pub resyncs: u64,
}

// ---------------------------------------------------------------------------
// VizUi — the gateway
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct LinkState {
    window: Option<WindowId>,
    generation: u64,
    attached: bool,
}

/// The headless viz gateway — owns view link generations, surface
/// bindings, the query/pump lifecycle, source registry, and the
/// outbound event queue. One `VizUi` per host process.
pub struct VizUi {
    views: BTreeMap<ViewId, LinkState>,
    surfaces: BTreeMap<(ViewId, SurfaceId), Surface>,
    ports: BTreeMap<VizSourceId, Port>,
    events: VecDeque<VizViewEvent>,
    themes: BTreeMap<ViewId, (u64, VizStyleInput)>,
    /// Counters.
    diag: VizDiagnostics,
    stale_after_ms: u64,
    max_query_bytes: usize,
    cache_budget: usize,
}

/// Default snapshot staleness window — upstream `staleAfterMs` (1 s).
pub const DEFAULT_STALE_AFTER_MS: u64 = 1000;
/// Default reply byte budget — upstream `maxBytes` (16 MiB).
pub const DEFAULT_MAX_QUERY_BYTES: usize = 16 * 1024 * 1024;
/// Default waveform view-cache budget — upstream LRU budget (4 MiB).
pub const DEFAULT_CACHE_BUDGET: usize = 4 * 1024 * 1024;
/// Hard bin cap on `buckets`/`plot_px` — far beyond any real display
/// width; keeps the bounded-read promise enforced at the boundary
/// (allocation happens before `max_bytes` can see the reply).
pub const MAX_BUCKETS: u32 = 65_536;
/// Outbound event queue cap — oldest events drop with a diagnostic
/// rather than letting an undrained host grow memory without bound.
pub const MAX_EVENTS: usize = 1024;

/// Span default for a freshly-bound/late-arriving waveform source —
/// the source extent, when it is a finite non-empty extent.
fn extent_span(meta: &WaveformMeta) -> Option<TimeRange> {
    (meta.extent.is_finite() && meta.extent > 0.0).then_some(TimeRange {
        start: 0.0,
        end: meta.extent,
    })
}

impl VizUi {
    /// An empty gateway.
    #[must_use]
    pub fn new() -> Self {
        Self {
            views: BTreeMap::new(),
            surfaces: BTreeMap::new(),
            ports: BTreeMap::new(),
            events: VecDeque::new(),
            themes: BTreeMap::new(),
            diag: VizDiagnostics::default(),
            stale_after_ms: DEFAULT_STALE_AFTER_MS,
            max_query_bytes: DEFAULT_MAX_QUERY_BYTES,
            cache_budget: DEFAULT_CACHE_BUDGET,
        }
    }

    /// Overrides the snapshot staleness window (ms).
    #[must_use]
    pub fn with_stale_after_ms(mut self, ms: u64) -> Self {
        self.stale_after_ms = ms;
        self
    }

    /// Overrides the reply byte budget.
    #[must_use]
    pub fn with_max_query_bytes(mut self, bytes: usize) -> Self {
        self.max_query_bytes = bytes;
        self
    }

    /// Diagnostics snapshot.
    #[must_use]
    pub fn diagnostics(&self) -> &VizDiagnostics {
        &self.diag
    }

    /// Drains outbound view events (rvf-viewport / rvf-select).
    pub fn drain_events(&mut self) -> Vec<VizViewEvent> {
        self.events.drain(..).collect()
    }

    /// Enqueues an outbound event at the queue cap — the oldest
    /// drops with a diagnostic rather than growing without bound.
    fn push_event(&mut self, event: VizViewEvent) {
        if self.events.len() >= MAX_EVENTS {
            self.events.pop_front();
            self.diag.events_dropped += 1;
        }
        self.events.push_back(event);
    }

    // -- views (ViewLink mirror) -------------------------------------------

    /// Attaches `view` to `window` — first mount or dock-transfer far
    /// side. Idempotent for the same window; a different window while
    /// attached performs an implicit detach first. The generation
    /// bumps and every bound surface re-syncs against the current
    /// source truth — the upstream re-read-on-attach contract.
    pub fn attach_view(&mut self, view: &str, window: &str) {
        let same = self
            .views
            .get(view)
            .is_some_and(|v| v.attached && v.window.as_deref() == Some(window));
        if same {
            return;
        }
        if self.views.get(view).is_some_and(|v| v.attached) {
            self.detach_view(view);
        }
        let generation = {
            let v = self.views.entry(view.to_owned()).or_insert(LinkState {
                window: None,
                generation: 0,
                attached: false,
            });
            v.attached = true;
            v.window = Some(window.to_owned());
            v.generation += 1;
            v.generation
        };
        self.diag.attaches += 1;
        // Re-read source truth for every surface on this view under
        // the fresh generation — upstream: re-subscribe + re-read.
        let ids: Vec<SurfaceId> = self
            .surfaces
            .range((view.to_owned(), String::new())..)
            .take_while(|((v, _), _)| v == view)
            .map(|((_, s), _)| s.clone())
            .collect();
        for surface in ids {
            self.resync_surface(view, &surface, generation);
        }
    }

    /// Detaches `view` — unmount / dock transfer / host close. Cancels
    /// in-flight query bookkeeping (bumped sequence + generation) and
    /// releases subscriptions. Source/project truth is untouched.
    pub fn detach_view(&mut self, view: &str) {
        let Some(v) = self.views.get_mut(view) else {
            return;
        };
        if !v.attached {
            return;
        }
        v.attached = false;
        v.window = None;
        v.generation += 1;
        self.diag.detaches += 1;
        let ids: Vec<SurfaceId> = self
            .surfaces
            .range((view.to_owned(), String::new())..)
            .take_while(|((v, _), _)| v == view)
            .map(|((_, s), _)| s.clone())
            .collect();
        for surface in ids {
            if let Some(Surface::Waveform(w)) = self.surfaces.get_mut(&(view.to_owned(), surface)) {
                // In-flight queries die under the dying epoch.
                if w.last_query.is_some() {
                    self.diag.queries_cancelled += 1;
                }
                w.last_query = None;
                w.query_seq += 1;
            }
        }
    }

    /// Detaches and drops every surface of `view`. The view entry
    /// stays as a tombstone; a later attach resurrects at a bumped
    /// generation.
    pub fn close_view(&mut self, view: &str) {
        self.detach_view(view);
        let dead: Vec<(ViewId, SurfaceId)> = self
            .surfaces
            .range((view.to_owned(), String::new())..)
            .take_while(|((v, _), _)| v == view)
            .map(|(k, _)| k.clone())
            .collect();
        for key in dead {
            self.surfaces.remove(&key);
        }
        self.themes.remove(view);
    }

    /// Whether `view` is currently attached.
    #[must_use]
    pub fn is_attached(&self, view: &str) -> bool {
        self.views.get(view).is_some_and(|v| v.attached)
    }

    /// The view's current link generation.
    #[must_use]
    pub fn generation(&self, view: &str) -> Option<u64> {
        self.views.get(view).map(|v| v.generation)
    }

    // -- sources -----------------------------------------------------------

    /// Registers a waveform port under `source` (canonical target id).
    /// A swap invalidates bound surfaces — cached truth belongs to the
    /// previous port and must never land under a colliding revision.
    pub fn register_waveform(&mut self, source: &str, port: Arc<dyn WaveformPort>) {
        self.ports.insert(source.to_string(), Port::Waveform(port));
        let budget = self.cache_budget;
        for s in self.surfaces.values_mut() {
            if let Surface::Waveform(w) = s
                && w.source == source
            {
                w.meta = None;
                w.view = None;
                w.view_stale = false;
                w.cache = BoundedCache::new(budget);
            }
        }
        self.notify_source_changed(source);
    }

    /// Registers a meter port. A swap resets the sequence baseline
    /// and drops the landed frame — the old frame is the previous
    /// producer's truth, and the new port is a new producer epoch.
    /// `dropped` stays cumulative: it counts observed loss on the
    /// surface across epochs, like `resync_surface`.
    pub fn register_meter(&mut self, source: &str, port: Arc<dyn MeterPort>) {
        self.ports.insert(source.to_string(), Port::Meter(port));
        for s in self.surfaces.values_mut() {
            if let Surface::Meter(m) = s
                && m.source == source
            {
                m.meta = None;
                m.frame = None;
                m.last_seq = None;
                m.last_pull_ms = None;
                m.stale = false;
            }
        }
        self.notify_source_changed(source);
    }

    /// Registers a spectrum port. A swap resets the sequence baseline
    /// and drops the landed frame. `dropped` stays cumulative across
    /// epochs, like `resync_surface`.
    pub fn register_spectrum(&mut self, source: &str, port: Arc<dyn SpectrumPort>) {
        self.ports.insert(source.to_string(), Port::Spectrum(port));
        for s in self.surfaces.values_mut() {
            if let Surface::Spectrum(sp) = s
                && sp.source == source
            {
                sp.meta = None;
                sp.frame = None;
                sp.last_seq = None;
                sp.last_pull_ms = None;
                sp.stale = false;
            }
        }
        self.notify_source_changed(source);
    }

    /// Drops a registered port. Bound surfaces degrade to `NoSource`;
    /// cached truth and liveness bookkeeping die with the port.
    pub fn unregister_source(&mut self, source: &str) {
        self.ports.remove(source);
        let keys: Vec<(ViewId, SurfaceId)> = self
            .surfaces
            .iter()
            .filter(|(_, s)| surface_source(s) == Some(source))
            .map(|(k, _)| k.clone())
            .collect();
        for (view, surface) in keys {
            match self.surfaces.get_mut(&(view.clone(), surface)) {
                Some(Surface::Waveform(w)) => {
                    w.meta = None;
                    w.view = None;
                    w.view_stale = false;
                    w.cache = BoundedCache::new(self.cache_budget);
                    w.status = VizStatus::NoSource;
                }
                Some(Surface::Meter(m)) => {
                    m.meta = None;
                    m.frame = None;
                    m.last_seq = None;
                    m.last_pull_ms = None;
                    m.stale = false;
                    m.status = VizStatus::NoSource;
                }
                Some(Surface::Spectrum(s)) => {
                    s.meta = None;
                    s.frame = None;
                    s.last_seq = None;
                    s.last_pull_ms = None;
                    s.stale = false;
                    s.status = VizStatus::NoSource;
                }
                _ => {}
            }
        }
    }

    /// The `subscribe` notify channel — a source announces a data/
    /// layout change. Bound waveform surfaces re-read meta and
    /// re-query; snapshot surfaces refresh meta (the pump picks frames
    /// up on its own schedule).
    pub fn notify_source_changed(&mut self, source: &str) {
        let keys: Vec<(ViewId, SurfaceId)> = self
            .surfaces
            .iter()
            .filter(|((v, _), s)| {
                self.views.get(v).is_some_and(|l| l.attached) && surface_source(s) == Some(source)
            })
            .map(|(k, _)| k.clone())
            .collect();
        for (view, surface) in keys {
            match self.surfaces.get_mut(&(view.clone(), surface.clone())) {
                Some(Surface::Waveform(w)) => {
                    w.meta = waveform_meta(&self.ports, &w.source);
                    match &w.meta {
                        // Source vanished under us — same degradation
                        // as unregister (old truth never lingers).
                        None => {
                            w.view = None;
                            w.view_stale = false;
                            w.status = absent_or_kind(&self.ports, &w.source, "waveform");
                        }
                        Some(meta) => {
                            if w.span.is_none() {
                                w.span = extent_span(meta);
                            }
                            w.view_stale = true;
                        }
                    }
                }
                Some(Surface::Meter(m)) => {
                    m.meta = meter_meta(&self.ports, &m.source);
                    if m.meta.is_none() {
                        m.frame = None;
                        m.last_pull_ms = None;
                        m.stale = false;
                        m.status = absent_or_kind(&self.ports, &m.source, "meter");
                    } else {
                        // Meta arrived — re-derive so a swap or
                        // late-registered source never keeps the old
                        // producer's status (same rule as resync).
                        m.status = snapshot_status(m.frame.is_some(), m.stale);
                    }
                }
                Some(Surface::Spectrum(s)) => {
                    s.meta = spectrum_meta(&self.ports, &s.source);
                    if s.meta.is_none() {
                        s.frame = None;
                        s.last_pull_ms = None;
                        s.stale = false;
                        s.status = absent_or_kind(&self.ports, &s.source, "spectrum");
                    } else {
                        s.status = snapshot_status(s.frame.is_some(), s.stale);
                    }
                }
                _ => {}
            }
            self.ensure_waveform(&view, &surface);
        }
    }

    // -- bindings ------------------------------------------------------------

    /// Binds a waveform surface on `view` to `source`. The span
    /// defaults to the source extent; `plot_px` is the bucket budget
    /// (viewport pixel width).
    ///
    /// # Errors
    /// [`VizError::DetachedView`]/[`VizError::UnknownView`] when `view`
    /// is not attached; [`VizError::InvalidPixels`] on `plot_px == 0`.
    pub fn bind_waveform(
        &mut self,
        view: &str,
        surface: &str,
        source: VizSourceId,
        plot_px: u32,
    ) -> Result<(), VizError> {
        self.require_attached(view)?;
        if plot_px == 0 || plot_px > MAX_BUCKETS {
            return Err(VizError::InvalidPixels);
        }
        let meta = waveform_meta(&self.ports, &source);
        let status = if meta.is_some() {
            VizStatus::AwaitingData
        } else {
            VizStatus::NoSource
        };
        let span = meta.as_ref().and_then(extent_span);
        self.surfaces.insert(
            (view.to_owned(), surface.to_owned()),
            Surface::Waveform(Box::new(WaveformSurface {
                source,
                meta,
                span,
                plot_px,
                overlays: OverlayState::default(),
                range: None,
                channel_labels: None,
                cache: BoundedCache::new(self.cache_budget),
                view: None,
                view_stale: false,
                status,
                query_seq: 0,
                last_query: None,
            })),
        );
        self.ensure_waveform(view, surface);
        Ok(())
    }

    /// Binds a meter surface on `view` to `source`.
    ///
    /// # Errors
    /// Same as [`Self::bind_waveform`].
    pub fn bind_meter(
        &mut self,
        view: &str,
        surface: &str,
        source: VizSourceId,
    ) -> Result<(), VizError> {
        self.require_attached(view)?;
        let meta = meter_meta(&self.ports, &source);
        let status = if meta.is_some() {
            VizStatus::AwaitingData
        } else {
            VizStatus::NoSource
        };
        self.surfaces.insert(
            (view.to_owned(), surface.to_owned()),
            Surface::Meter(MeterSurface {
                source,
                meta,
                scale: MeterScale::default(),
                frame: None,
                last_seq: None,
                last_pull_ms: None,
                dropped: 0,
                stale: false,
                status,
            }),
        );
        Ok(())
    }

    /// Binds a spectrum surface on `view` to `source`.
    ///
    /// # Errors
    /// Same as [`Self::bind_waveform`].
    pub fn bind_spectrum(
        &mut self,
        view: &str,
        surface: &str,
        source: VizSourceId,
    ) -> Result<(), VizError> {
        self.require_attached(view)?;
        let meta = spectrum_meta(&self.ports, &source);
        let status = if meta.is_some() {
            VizStatus::AwaitingData
        } else {
            VizStatus::NoSource
        };
        self.surfaces.insert(
            (view.to_owned(), surface.to_owned()),
            Surface::Spectrum(SpectrumSurface {
                source,
                meta,
                scale: SpectrumScale::default(),
                frame: None,
                last_seq: None,
                last_pull_ms: None,
                dropped: 0,
                stale: false,
                status,
            }),
        );
        Ok(())
    }

    /// Binds a project-backed timeline surface on `view`. `media_rate`
    /// is the display rate used when absolute-anchored clips must be
    /// resolved into frames.
    ///
    /// # Errors
    /// Same as [`Self::bind_waveform`].
    pub fn bind_timeline(
        &mut self,
        view: &str,
        surface: &str,
        axis: TimelineAxis,
        media_rate: u32,
    ) -> Result<(), VizError> {
        self.require_attached(view)?;
        if media_rate == 0 {
            return Err(VizError::InvalidConfig("media_rate must be > 0"));
        }
        self.surfaces.insert(
            (view.to_owned(), surface.to_owned()),
            Surface::Timeline(TimelineSurface {
                axis,
                media_rate,
                span: None,
                plot_px: 0,
                overlays: OverlayState::default(),
                items: Vec::new(),
                unresolved: 0,
                synced_rev: None,
                status: VizStatus::AwaitingData,
            }),
        );
        Ok(())
    }

    // -- waveform element ops ------------------------------------------------

    /// Issues the bounded query for a waveform surface — the draw
    /// path. Cache hit → land immediately. Otherwise query the port,
    /// enforce `maxBytes`, double-check the revision at resolve, retry
    /// once on a stale reply, then mark the view stale.
    pub fn ensure_waveform(&mut self, view: &str, surface: &str) {
        if !self.is_attached(view) {
            return;
        }
        let generation = self.views.get(view).map_or(0, |v| v.generation);
        let Some(Surface::Waveform(w)) = self
            .surfaces
            .get_mut(&(view.to_owned(), surface.to_owned()))
        else {
            return;
        };
        // Re-read source truth — the port is the authority, not the
        // surface's cached meta.
        w.meta = waveform_meta(&self.ports, &w.source);
        let Some(meta) = w.meta.clone() else {
            w.view = None;
            w.view_stale = false;
            w.status = absent_or_kind(&self.ports, &w.source, "waveform");
            return;
        };
        // A source that arrived after bind still gets the extent
        // default — binding never leaves a live source unspanned.
        if w.span.is_none() {
            w.span = extent_span(&meta);
        }
        let Some(span) = w.span else {
            // Source present but no usable geometry yet — awaiting a
            // valid extent on the next meta change.
            w.status = VizStatus::AwaitingData;
            return;
        };
        let key = WaveformCacheKey {
            source: w.source.clone(),
            revision: meta.revision.clone(),
            start: span.start.to_bits(),
            end: span.end.to_bits(),
            buckets: w.plot_px,
            channels: None,
        };
        let source_id = w.source.clone();
        let cached = w.cache.get(&key).cloned();
        // A hit still must match the source's *current* revision — a
        // port swap under a colliding revision token must not serve
        // the previous source's truth.
        let hit = cached.filter(|_| {
            waveform_meta(&self.ports, &source_id).is_some_and(|m| m.revision == key.revision)
        });
        if let Some(v) = hit {
            self.diag.cache_hits += 1;
            let Some(w) = waveform_mut(&mut self.surfaces, view, surface) else {
                return;
            };
            w.view = Some(v);
            w.view_stale = false;
            w.status = VizStatus::Ok;
            return;
        }
        self.diag.cache_misses += 1;
        let Some(w) = waveform_mut(&mut self.surfaces, view, surface) else {
            return;
        };
        w.query_seq += 1;
        let query_id = format!("q{}-{}", w.query_seq, generation);
        w.last_query = Some(ViewportQuery {
            query_id: query_id.clone(),
            source_id: w.source.clone(),
            source_revision: meta.revision.clone(),
            view_id: view.to_owned(),
            generation,
            start: span.start,
            end: span.end,
            buckets: Some(w.plot_px),
            channels: None,
            max_bytes: Some(self.max_query_bytes),
            encoding: None,
        });
        let query = WaveformQuery {
            start: span.start,
            end: span.end,
            buckets: w.plot_px,
            channels: None,
        };
        let source_id = w.source.clone();
        self.resolve_waveform_query(view, surface, &source_id, &meta.revision, &key, &query);
    }

    // Upstream stale-response rule: a reply whose revision differs
    // from the issued revision — or from the source's *current*
    // revision at resolve time — is discarded; retry once.
    fn resolve_waveform_query(
        &mut self,
        view: &str,
        surface: &str,
        source_id: &VizSourceId,
        issued_revision: &SourceRevision,
        key: &WaveformCacheKey,
        query: &WaveformQuery,
    ) {
        let mut attempts = 0u8;
        loop {
            attempts += 1;
            self.diag.queries_issued += 1;
            let outcome = if let Some(Port::Waveform(p)) = self.ports.get(source_id) {
                p.query(query)
            } else {
                if let Some(w) = waveform_mut(&mut self.surfaces, view, surface) {
                    w.status = VizStatus::NoSource;
                    w.view = None;
                    w.last_query = None;
                }
                return;
            };
            match outcome {
                Ok(v) => {
                    let current_rev = waveform_meta(&self.ports, source_id)
                        .map_or_else(|| issued_revision.clone(), |m| m.revision);
                    let reply_stale =
                        *v.revision() != *issued_revision || current_rev != *issued_revision;
                    if reply_stale {
                        self.diag.queries_stale += 1;
                        if attempts < 2 {
                            continue;
                        }
                        let Some(w) = waveform_mut(&mut self.surfaces, view, surface) else {
                            return;
                        };
                        w.view_stale = true;
                        w.status = VizStatus::Stale;
                        w.last_query = None;
                        return;
                    }
                    if v.byte_len() > self.max_query_bytes {
                        self.diag.queries_failed += 1;
                        let Some(w) = waveform_mut(&mut self.surfaces, view, surface) else {
                            return;
                        };
                        w.status = VizStatus::SourceError {
                            detail: "reply over byte budget".into(),
                        };
                        w.last_query = None;
                        return;
                    }
                    let bytes = v.byte_len();
                    let Some(w) = waveform_mut(&mut self.surfaces, view, surface) else {
                        return;
                    };
                    w.cache.insert(key.clone(), v.clone(), bytes);
                    w.view = Some(v);
                    w.view_stale = false;
                    w.status = VizStatus::Ok;
                    w.last_query = None;
                    self.diag.queries_resolved += 1;
                    return;
                }
                Err(e) => {
                    self.diag.queries_failed += 1;
                    let Some(w) = waveform_mut(&mut self.surfaces, view, surface) else {
                        return;
                    };
                    w.status = VizStatus::SourceError {
                        detail: e.to_string(),
                    };
                    w.last_query = None;
                    return;
                }
            }
        }
    }

    /// Sets a waveform/timeline surface's span — programmatic viewport
    /// set. Emits `Viewport { Set }` and re-queries (waveform only;
    /// timeline items are already local).
    ///
    /// # Errors
    /// [`VizError::InvalidSpan`] on non-finite/`end <= start`;
    /// kind/detach errors as usual.
    pub fn set_span(
        &mut self,
        view: &str,
        surface: &str,
        start: f64,
        end: f64,
    ) -> Result<(), VizError> {
        if !start.is_finite() || !end.is_finite() || end <= start {
            return Err(VizError::InvalidSpan);
        }
        self.require_attached(view)?;
        let kind = self.set_span_inner(view, surface, start, end)?;
        if kind == SurfaceKind::Waveform {
            self.ensure_waveform(view, surface);
        }
        self.push_event(VizViewEvent::Viewport {
            view_id: view.to_owned(),
            surface_id: surface.to_owned(),
            start,
            end,
            trigger: ViewportTrigger::Set,
        });
        Ok(())
    }

    /// Pans a span by `d` axis units. Emits `Viewport { Pan }`.
    ///
    /// # Errors
    /// As [`Self::set_span`].
    pub fn pan(&mut self, view: &str, surface: &str, d: f64) -> Result<(), VizError> {
        if !d.is_finite() {
            return Err(VizError::InvalidSpan);
        }
        self.require_attached(view)?;
        let Some(span) = self.span_of(view, surface)? else {
            return Err(VizError::InvalidSpan);
        };
        let kind = self.set_span_inner(view, surface, span.start + d, span.end + d)?;
        if kind == SurfaceKind::Waveform {
            self.ensure_waveform(view, surface);
        }
        self.push_event(VizViewEvent::Viewport {
            view_id: view.to_owned(),
            surface_id: surface.to_owned(),
            start: span.start + d,
            end: span.end + d,
            trigger: ViewportTrigger::Pan,
        });
        Ok(())
    }

    /// Zooms a span by `factor` around `anchor` (axis units);
    /// `factor > 1` zooms in. Emits `Viewport { Wheel }`.
    ///
    /// # Errors
    /// As [`Self::set_span`]; `factor <= 0`/non-finite is
    /// [`VizError::InvalidSpan`].
    pub fn zoom(
        &mut self,
        view: &str,
        surface: &str,
        factor: f64,
        anchor: f64,
    ) -> Result<(), VizError> {
        if !factor.is_finite() || factor <= 0.0 || !anchor.is_finite() {
            return Err(VizError::InvalidSpan);
        }
        self.require_attached(view)?;
        let Some(span) = self.span_of(view, surface)? else {
            return Err(VizError::InvalidSpan);
        };
        let len = span.end - span.start;
        let t = (anchor - span.start) / len;
        let new_len = len / factor;
        let (start, end) = (anchor - new_len * t, anchor + new_len * (1.0 - t));
        let kind = self.set_span_inner(view, surface, start, end)?;
        if kind == SurfaceKind::Waveform {
            self.ensure_waveform(view, surface);
        }
        self.push_event(VizViewEvent::Viewport {
            view_id: view.to_owned(),
            surface_id: surface.to_owned(),
            start,
            end,
            trigger: ViewportTrigger::Wheel,
        });
        Ok(())
    }

    /// Resizes the plot width — a `buckets` change re-queries
    /// (cache may still hit for a previously seen width).
    ///
    /// # Errors
    /// [`VizError::InvalidPixels`] on `px == 0`.
    pub fn set_plot_px(&mut self, view: &str, surface: &str, px: u32) -> Result<(), VizError> {
        if px == 0 || px > MAX_BUCKETS {
            return Err(VizError::InvalidPixels);
        }
        self.require_attached(view)?;
        let (kind, span) = match self
            .surfaces
            .get_mut(&(view.to_owned(), surface.to_owned()))
        {
            Some(Surface::Waveform(w)) => {
                w.plot_px = px;
                (SurfaceKind::Waveform, w.span)
            }
            Some(Surface::Timeline(t)) => {
                t.plot_px = px;
                (SurfaceKind::Timeline, t.span)
            }
            Some(_) => return Err(kind_mismatch("waveform/timeline", view, surface)),
            None => {
                return Err(VizError::UnknownSurface(
                    view.to_owned(),
                    surface.to_owned(),
                ));
            }
        };
        if kind == SurfaceKind::Waveform {
            self.ensure_waveform(view, surface);
        }
        if let Some(span) = span {
            self.push_event(VizViewEvent::Viewport {
                view_id: view.to_owned(),
                surface_id: surface.to_owned(),
                start: span.start,
                end: span.end,
                trigger: ViewportTrigger::Resize,
            });
        }
        Ok(())
    }

    /// Replaces a surface's overlays — repaint only, never re-queries.
    /// Authoritative selection/playhead arrive here after the host
    /// routes the gesture through the editing model.
    ///
    /// # Errors
    /// [`VizError::InvalidSpan`] on non-finite or inverted overlay
    /// geometry; kind/detach errors as usual.
    pub fn set_overlays(
        &mut self,
        view: &str,
        surface: &str,
        overlays: OverlayState,
    ) -> Result<(), VizError> {
        // Overlay geometry is axis geometry — same rule as spans:
        // non-finite or inverted positions never land in state.
        let ok = overlays
            .selection
            .is_none_or(|r| r.start.is_finite() && r.end.is_finite() && r.start <= r.end)
            && overlays.playhead.is_none_or(f64::is_finite)
            && overlays.markers.iter().all(|m| m.at.is_finite());
        if !ok {
            return Err(VizError::InvalidSpan);
        }
        self.require_attached(view)?;
        match self
            .surfaces
            .get_mut(&(view.to_owned(), surface.to_owned()))
        {
            Some(Surface::Waveform(w)) => w.overlays = overlays,
            Some(Surface::Timeline(t)) => t.overlays = overlays,
            Some(_) => return Err(kind_mismatch("waveform/timeline", view, surface)),
            None => {
                return Err(VizError::UnknownSurface(
                    view.to_owned(),
                    surface.to_owned(),
                ));
            }
        }
        self.diag.repaints += 1;
        Ok(())
    }

    /// A selection gesture completed — `rvf-select`. The event is
    /// queued for the host to route through the editing model; the
    /// overlay is *not* set here (committed selection stays
    /// consumer-authoritative).
    ///
    /// # Errors
    /// [`VizError::InvalidSpan`] on a non-finite range.
    pub fn select(
        &mut self,
        view: &str,
        surface: &str,
        start: f64,
        end: f64,
    ) -> Result<(), VizError> {
        if !start.is_finite() || !end.is_finite() {
            return Err(VizError::InvalidSpan);
        }
        self.require_attached(view)?;
        match self.surfaces.get(&(view.to_owned(), surface.to_owned())) {
            Some(Surface::Waveform(_) | Surface::Timeline(_)) => {}
            Some(_) => return Err(kind_mismatch("waveform/timeline", view, surface)),
            None => {
                return Err(VizError::UnknownSurface(
                    view.to_owned(),
                    surface.to_owned(),
                ));
            }
        }
        self.push_event(VizViewEvent::Select {
            view_id: view.to_owned(),
            surface_id: surface.to_owned(),
            start: start.min(end),
            end: start.max(end),
        });
        Ok(())
    }

    /// Sets the waveform value-range override (`range.min`/`range.max`)
    /// — repaint only.
    ///
    /// # Errors
    /// [`VizError::InvalidConfig`] when `min >= max`.
    pub fn set_waveform_range(
        &mut self,
        view: &str,
        surface: &str,
        min: f64,
        max: f64,
    ) -> Result<(), VizError> {
        if !min.is_finite() || !max.is_finite() || min >= max {
            return Err(VizError::InvalidConfig("range min must be < max"));
        }
        self.require_attached(view)?;
        match self
            .surfaces
            .get_mut(&(view.to_owned(), surface.to_owned()))
        {
            Some(Surface::Waveform(w)) => {
                w.range = Some((min, max));
                self.diag.repaints += 1;
                Ok(())
            }
            Some(_) => Err(kind_mismatch("waveform", view, surface)),
            None => Err(VizError::UnknownSurface(
                view.to_owned(),
                surface.to_owned(),
            )),
        }
    }

    /// Sets channel labels — repaint only.
    ///
    /// # Errors
    /// Kind/detach errors as usual.
    pub fn set_channel_labels(
        &mut self,
        view: &str,
        surface: &str,
        labels: Option<Vec<Option<String>>>,
    ) -> Result<(), VizError> {
        self.require_attached(view)?;
        match self
            .surfaces
            .get_mut(&(view.to_owned(), surface.to_owned()))
        {
            Some(Surface::Waveform(w)) => {
                w.channel_labels = labels;
                self.diag.repaints += 1;
                Ok(())
            }
            Some(_) => Err(kind_mismatch("waveform", view, surface)),
            None => Err(VizError::UnknownSurface(
                view.to_owned(),
                surface.to_owned(),
            )),
        }
    }

    /// Marks the waveform view stale and re-queries — the upstream
    /// `invalidate()` equivalent (e.g. host knows data changed but no
    /// source notify arrived).
    pub fn invalidate_waveform(&mut self, view: &str, surface: &str) {
        if let Some(Surface::Waveform(w)) = self
            .surfaces
            .get_mut(&(view.to_owned(), surface.to_owned()))
        {
            w.view_stale = true;
        }
        self.ensure_waveform(view, surface);
    }

    /// Clears every waveform view cache — upstream `clearAllCaches`.
    pub fn clear_waveform_caches(&mut self) {
        for s in self.surfaces.values_mut() {
            if let Surface::Waveform(w) = s {
                w.cache.clear();
            }
        }
    }

    // -- snapshot pump ------------------------------------------------------

    /// One pump tick — upstream `pumpNow`. Pulls the latest complete
    /// frame for every attached snapshot surface, accumulates
    /// producer-dropped + sequence-gap loss, ages landed frames past
    /// `staleAfterMs` into the stale state (once). Never calls into RT
    /// — ports return already-published frames.
    pub fn pump(&mut self, now_ms: u64) {
        self.diag.pumps += 1;
        let stale_after = self.stale_after_ms;
        let keys: Vec<(ViewId, SurfaceId)> = self
            .surfaces
            .iter()
            .filter(|((v, _), _)| self.views.get(v).is_some_and(|l| l.attached))
            .filter_map(|(k, s)| match s {
                Surface::Meter(_) | Surface::Spectrum(_) => Some(k.clone()),
                _ => None,
            })
            .collect();
        for (view, surface) in keys {
            let key = (view, surface);
            match self.surfaces.get_mut(&key) {
                Some(Surface::Meter(m)) => {
                    Self::pump_meter(&self.ports, &mut self.diag, m, now_ms, stale_after);
                }
                Some(Surface::Spectrum(s)) => {
                    Self::pump_spectrum(&self.ports, &mut self.diag, s, now_ms, stale_after);
                }
                _ => {}
            }
        }
    }

    fn pump_meter(
        ports: &BTreeMap<VizSourceId, Port>,
        diag: &mut VizDiagnostics,
        m: &mut MeterSurface,
        now_ms: u64,
        stale_after: u64,
    ) {
        if m.meta.is_none() {
            m.meta = meter_meta(ports, &m.source);
        }
        match meter_pull(ports, &m.source) {
            Err(e) => m.status = VizStatus::SourceError { detail: e },
            Ok(None) => Self::age_snapshot(
                &mut m.last_pull_ms,
                now_ms,
                stale_after,
                &mut m.stale,
                &mut m.status,
                diag,
            ),
            Ok(Some(f)) => {
                // Complete frames only — a frame that disagrees with
                // meta channel count is a producer error, not landable
                // data.
                if m.meta
                    .as_ref()
                    .is_some_and(|meta| meta.channels > 0 && f.channels.len() != meta.channels)
                {
                    m.status = VizStatus::SourceError {
                        detail: format!(
                            "partial frame: {}/{} channels",
                            f.channels.len(),
                            m.meta.as_ref().map_or(0, |meta| meta.channels)
                        ),
                    };
                    return;
                }
                // A republished sequence is not new data: bookkeeping,
                // `last_pull` and staleness only move on a genuinely
                // new frame — that is what lets an idle producer go
                // stale while `snapshot()` still serves the last frame.
                if Some(f.sequence) == m.last_seq {
                    Self::age_snapshot(
                        &mut m.last_pull_ms,
                        now_ms,
                        stale_after,
                        &mut m.stale,
                        &mut m.status,
                        diag,
                    );
                } else {
                    Self::land_seq_frame(
                        f.sequence,
                        f.dropped,
                        &mut m.last_seq,
                        &mut m.dropped,
                        diag,
                    );
                    diag.frames_pulled += 1;
                    m.frame = Some(f);
                    m.last_pull_ms = Some(now_ms);
                    m.stale = false;
                    m.status = VizStatus::Ok;
                }
            }
        }
    }

    fn pump_spectrum(
        ports: &BTreeMap<VizSourceId, Port>,
        diag: &mut VizDiagnostics,
        s: &mut SpectrumSurface,
        now_ms: u64,
        stale_after: u64,
    ) {
        if s.meta.is_none() {
            s.meta = spectrum_meta(ports, &s.source);
        }
        match spectrum_pull(ports, &s.source) {
            Err(e) => s.status = VizStatus::SourceError { detail: e },
            Ok(None) => Self::age_snapshot(
                &mut s.last_pull_ms,
                now_ms,
                stale_after,
                &mut s.stale,
                &mut s.status,
                diag,
            ),
            Ok(Some(f)) => {
                if s.meta
                    .as_ref()
                    .is_some_and(|meta| meta.bins > 0 && f.magnitudes.len() != meta.bins)
                {
                    s.status = VizStatus::SourceError {
                        detail: format!(
                            "partial frame: {}/{} bins",
                            f.magnitudes.len(),
                            s.meta.as_ref().map_or(0, |meta| meta.bins)
                        ),
                    };
                    return;
                }
                if Some(f.sequence) == s.last_seq {
                    Self::age_snapshot(
                        &mut s.last_pull_ms,
                        now_ms,
                        stale_after,
                        &mut s.stale,
                        &mut s.status,
                        diag,
                    );
                } else {
                    Self::land_seq_frame(f.sequence, None, &mut s.last_seq, &mut s.dropped, diag);
                    diag.frames_pulled += 1;
                    s.frame = Some(f);
                    s.last_pull_ms = Some(now_ms);
                    s.stale = false;
                    s.status = VizStatus::Ok;
                }
            }
        }
    }

    /// Shared seq-gap + producer-dropped accumulation.
    fn land_seq_frame(
        seq: u64,
        producer_dropped: Option<u64>,
        last_seq: &mut Option<u64>,
        dropped: &mut u64,
        diag: &mut VizDiagnostics,
    ) {
        if let Some(d) = producer_dropped {
            *dropped = dropped.saturating_add(d);
            diag.producer_dropped = diag.producer_dropped.saturating_add(d);
        }
        if let Some(last) = *last_seq
            && seq > last.saturating_add(1)
        {
            let gap = seq - last - 1;
            *dropped = dropped.saturating_add(gap);
            diag.seq_gaps = diag.seq_gaps.saturating_add(gap);
        }
        // A sequence regression is a producer epoch restart: the new
        // frame lands and becomes the gap baseline.
        *last_seq = Some(seq);
    }

    /// Ages a landed frame into stale once past `staleAfter`.
    /// Never-pulled surfaces stay `AwaitingData` — upstream only ages
    /// frames that arrived.
    fn age_snapshot(
        last_pull_ms: &mut Option<u64>,
        now_ms: u64,
        stale_after: u64,
        stale: &mut bool,
        status: &mut VizStatus,
        diag: &mut VizDiagnostics,
    ) {
        if let Some(t) = *last_pull_ms
            && now_ms.saturating_sub(t) > stale_after
            && !*stale
        {
            *stale = true;
            *status = VizStatus::Stale;
            diag.stale_transitions += 1;
        }
    }

    /// Sets the meter display scale — repaint only.
    ///
    /// # Errors
    /// [`VizError::InvalidConfig`] when `min_db >= max_db`.
    pub fn set_meter_scale(
        &mut self,
        view: &str,
        surface: &str,
        scale: MeterScale,
    ) -> Result<(), VizError> {
        let ok = scale.min_db.is_finite()
            && scale.max_db.is_finite()
            && scale.min_db < scale.max_db
            && scale.clip_db.is_finite()
            && scale.segments.iter().all(|s| s.is_finite());
        if !ok {
            return Err(VizError::InvalidConfig(
                "meter scale ranges inverted/non-finite",
            ));
        }
        self.require_attached(view)?;
        match self
            .surfaces
            .get_mut(&(view.to_owned(), surface.to_owned()))
        {
            Some(Surface::Meter(m)) => {
                m.scale = scale;
                self.diag.repaints += 1;
                Ok(())
            }
            Some(_) => Err(kind_mismatch("meter", view, surface)),
            None => Err(VizError::UnknownSurface(
                view.to_owned(),
                surface.to_owned(),
            )),
        }
    }

    /// Sets the spectrum display mapping — repaint only.
    ///
    /// # Errors
    /// [`VizError::InvalidConfig`] on inverted ranges.
    pub fn set_spectrum_scale(
        &mut self,
        view: &str,
        surface: &str,
        scale: SpectrumScale,
    ) -> Result<(), VizError> {
        let ok = scale.min_hz.is_finite()
            && scale.max_hz.is_finite()
            && scale.min_hz < scale.max_hz
            && scale.min_db.is_finite()
            && scale.max_db.is_finite()
            && scale.min_db < scale.max_db
            && scale.reference.is_finite()
            && scale.reference > 0.0;
        if !ok {
            return Err(VizError::InvalidConfig(
                "spectrum scale ranges inverted/non-finite",
            ));
        }
        self.require_attached(view)?;
        match self
            .surfaces
            .get_mut(&(view.to_owned(), surface.to_owned()))
        {
            Some(Surface::Spectrum(s)) => {
                s.scale = scale;
                self.diag.repaints += 1;
                Ok(())
            }
            Some(_) => Err(kind_mismatch("spectrum", view, surface)),
            None => Err(VizError::UnknownSurface(
                view.to_owned(),
                surface.to_owned(),
            )),
        }
    }

    /// Swaps a view's theme — the upstream "theme change is repaint-
    /// only" rule: style is never a cache key and a repaint must not
    /// re-query, rebuild LOD, or touch source state.
    ///
    /// # Errors
    /// Detach errors as usual.
    pub fn set_theme(&mut self, view: &str, style: VizStyleInput) -> Result<(), VizError> {
        self.require_attached(view)?;
        let entry = self
            .themes
            .entry(view.to_owned())
            .or_insert((0, VizStyleInput::default()));
        entry.0 += 1;
        entry.1 = style;
        self.diag.repaints += 1;
        Ok(())
    }

    /// The view's current theme generation + style.
    #[must_use]
    pub fn theme(&self, view: &str) -> Option<&(u64, VizStyleInput)> {
        self.themes.get(view)
    }

    // -- timeline ------------------------------------------------------------

    /// Rebuilds the timeline surface's items from `session` project
    /// truth — the bounded project read. Each clip resolves its
    /// placement span in the surface axis (beats via PPQN/tempo map,
    /// seconds via media frames at `media_rate`); unresolvable clips
    /// are skipped and counted. First sync fits the span to content.
    pub fn sync_timeline(&mut self, session: &ProjectSession, view: &str, surface: &str) {
        let key = (view.to_owned(), surface.to_owned());
        let Some(Surface::Timeline(t)) = self.surfaces.get_mut(&key) else {
            return;
        };
        let (items, unresolved) = timeline_items(session, t.axis, t.media_rate);
        t.items = items;
        t.unresolved = unresolved;
        t.synced_rev = Some(session.revision().0);
        if t.span.is_none() {
            let end = t.items.iter().map(|i| i.end).fold(0.0f64, f64::max);
            t.span = Some(TimeRange {
                start: 0.0,
                end: end.max(match t.axis {
                    TimelineAxis::Beats => 16.0,
                    TimelineAxis::Seconds => 8.0,
                }),
            });
        }
        t.status = VizStatus::Ok;
        self.diag.resyncs += 1;
    }

    /// Whether the timeline items are stale relative to `session`'s
    /// current revision — the project-source revision guard.
    #[must_use]
    pub fn timeline_stale(&self, session: &ProjectSession, view: &str, surface: &str) -> bool {
        match self.surfaces.get(&(view.to_owned(), surface.to_owned())) {
            Some(Surface::Timeline(t)) => t.synced_rev != Some(session.revision().0),
            _ => true,
        }
    }

    // -- rows ----------------------------------------------------------------

    /// Waveform surface row.
    #[must_use]
    pub fn waveform_row(&self, view: &str, surface: &str) -> Option<WaveformRow> {
        match self.surfaces.get(&(view.to_owned(), surface.to_owned())) {
            Some(Surface::Waveform(w)) => Some(WaveformRow {
                source: w.source.clone(),
                meta: w.meta.clone(),
                span: w.span,
                plot_px: w.plot_px,
                overlays: w.overlays.clone(),
                view: w.view.clone(),
                view_stale: w.view_stale,
                status: w.status.clone(),
                cache_entries: w.cache.len(),
                cache_bytes: w.cache.used_bytes(),
            }),
            _ => None,
        }
    }

    /// Meter surface row.
    #[must_use]
    pub fn meter_row(&self, view: &str, surface: &str) -> Option<MeterRow> {
        match self.surfaces.get(&(view.to_owned(), surface.to_owned())) {
            Some(Surface::Meter(m)) => Some(MeterRow {
                source: m.source.clone(),
                meta: m.meta.clone(),
                scale: m.scale.clone(),
                frame: m.frame.clone(),
                dropped: m.dropped,
                stale: m.stale,
                status: m.status.clone(),
            }),
            _ => None,
        }
    }

    /// Spectrum surface row.
    #[must_use]
    pub fn spectrum_row(&self, view: &str, surface: &str) -> Option<SpectrumRow> {
        match self.surfaces.get(&(view.to_owned(), surface.to_owned())) {
            Some(Surface::Spectrum(s)) => Some(SpectrumRow {
                source: s.source.clone(),
                meta: s.meta.clone(),
                scale: s.scale.clone(),
                frame: s.frame.clone(),
                dropped: s.dropped,
                stale: s.stale,
                status: s.status.clone(),
            }),
            _ => None,
        }
    }

    /// Timeline surface row — items inside the current span (the
    /// bounded read: only overlapping items are returned).
    #[must_use]
    pub fn timeline_row(&self, view: &str, surface: &str) -> Option<TimelineRow> {
        match self.surfaces.get(&(view.to_owned(), surface.to_owned())) {
            Some(Surface::Timeline(t)) => {
                let items = match t.span {
                    Some(s) => items_in(&t.items, ruviform_viz::Selection::new(s.start, s.end))
                        .into_iter()
                        .map(|i| t.items[i].clone())
                        .collect(),
                    None => Vec::new(),
                };
                Some(TimelineRow {
                    axis: t.axis,
                    span: t.span,
                    items,
                    unresolved: t.unresolved,
                    synced_rev: t.synced_rev,
                    overlays: t.overlays.clone(),
                    status: t.status.clone(),
                })
            }
            _ => None,
        }
    }

    /// The last wire query issued for a waveform surface (diagnostics/
    /// contract evidence).
    #[must_use]
    pub fn last_query(&self, view: &str, surface: &str) -> Option<ViewportQuery> {
        match self.surfaces.get(&(view.to_owned(), surface.to_owned())) {
            Some(Surface::Waveform(w)) => w.last_query.clone(),
            _ => None,
        }
    }

    // -- internals ------------------------------------------------------------

    fn require_attached(&self, view: &str) -> Result<(), VizError> {
        match self.views.get(view) {
            Some(v) if v.attached => Ok(()),
            Some(_) => Err(VizError::DetachedView(view.to_owned())),
            None => Err(VizError::UnknownView(view.to_owned())),
        }
    }

    /// Re-read-on-attach for one surface under the fresh generation.
    /// Status derives from the truth actually held — a landed frame
    /// or synced items never flap back to `AwaitingData`.
    fn resync_surface(&mut self, view: &str, surface: &str, _generation: u64) {
        match self
            .surfaces
            .get_mut(&(view.to_owned(), surface.to_owned()))
        {
            Some(Surface::Waveform(w)) => {
                w.meta = waveform_meta(&self.ports, &w.source);
                match &w.meta {
                    None => {
                        w.status = absent_or_kind(&self.ports, &w.source, "waveform");
                        w.view = None;
                        w.view_stale = false;
                    }
                    Some(meta) => {
                        if w.span.is_none() {
                            w.span = extent_span(meta);
                        }
                        w.status = if w.view.is_none() {
                            VizStatus::AwaitingData
                        } else if w.view_stale {
                            VizStatus::Stale
                        } else {
                            VizStatus::Ok
                        };
                    }
                }
            }
            Some(Surface::Meter(m)) => {
                m.meta = meter_meta(&self.ports, &m.source);
                m.status = if m.meta.is_none() {
                    absent_or_kind(&self.ports, &m.source, "meter")
                } else {
                    snapshot_status(m.frame.is_some(), m.stale)
                };
            }
            Some(Surface::Spectrum(s)) => {
                s.meta = spectrum_meta(&self.ports, &s.source);
                s.status = if s.meta.is_none() {
                    absent_or_kind(&self.ports, &s.source, "spectrum")
                } else {
                    snapshot_status(s.frame.is_some(), s.stale)
                };
            }
            Some(Surface::Timeline(t)) => {
                t.status = if t.synced_rev.is_some() {
                    VizStatus::Ok
                } else {
                    VizStatus::AwaitingData
                };
            }
            None => return,
        }
        self.ensure_waveform(view, surface);
    }

    /// Shared span setter; returns the surface kind. Computed spans
    /// (pan/zoom) are validated here — non-finite or inverted spans
    /// never land in surface state or the event stream.
    fn set_span_inner(
        &mut self,
        view: &str,
        surface: &str,
        start: f64,
        end: f64,
    ) -> Result<SurfaceKind, VizError> {
        if !start.is_finite() || !end.is_finite() || end <= start {
            return Err(VizError::InvalidSpan);
        }
        match self
            .surfaces
            .get_mut(&(view.to_owned(), surface.to_owned()))
        {
            Some(Surface::Waveform(w)) => {
                w.span = Some(TimeRange { start, end });
                Ok(SurfaceKind::Waveform)
            }
            Some(Surface::Timeline(t)) => {
                t.span = Some(TimeRange { start, end });
                Ok(SurfaceKind::Timeline)
            }
            Some(_) => Err(kind_mismatch("waveform/timeline", view, surface)),
            None => Err(VizError::UnknownSurface(
                view.to_owned(),
                surface.to_owned(),
            )),
        }
    }

    fn span_of(&self, view: &str, surface: &str) -> Result<Option<TimeRange>, VizError> {
        match self.surfaces.get(&(view.to_owned(), surface.to_owned())) {
            Some(Surface::Waveform(w)) => Ok(w.span),
            Some(Surface::Timeline(t)) => Ok(t.span),
            Some(_) => Err(kind_mismatch("waveform/timeline", view, surface)),
            None => Err(VizError::UnknownSurface(
                view.to_owned(),
                surface.to_owned(),
            )),
        }
    }
}

impl Default for VizUi {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum SurfaceKind {
    Waveform,
    Timeline,
}

fn kind_mismatch(expected: &'static str, view: &str, surface: &str) -> VizError {
    VizError::KindMismatch {
        expected,
        view: view.to_owned(),
        surface: surface.to_owned(),
    }
}

fn waveform_mut<'a>(
    surfaces: &'a mut BTreeMap<(ViewId, SurfaceId), Surface>,
    view: &str,
    surface: &str,
) -> Option<&'a mut WaveformSurface> {
    match surfaces.get_mut(&(view.to_owned(), surface.to_owned())) {
        Some(Surface::Waveform(w)) => Some(w),
        _ => None,
    }
}

fn waveform_meta(ports: &BTreeMap<VizSourceId, Port>, source: &str) -> Option<WaveformMeta> {
    match ports.get(source) {
        Some(Port::Waveform(p)) => Some(p.meta()),
        _ => None,
    }
}

fn meter_meta(ports: &BTreeMap<VizSourceId, Port>, source: &str) -> Option<MeterMeta> {
    match ports.get(source) {
        Some(Port::Meter(p)) => Some(p.meta()),
        _ => None,
    }
}

fn spectrum_meta(ports: &BTreeMap<VizSourceId, Port>, source: &str) -> Option<SpectrumMeta> {
    match ports.get(source) {
        Some(Port::Spectrum(p)) => Some(p.meta()),
        _ => None,
    }
}

fn meter_pull(
    ports: &BTreeMap<VizSourceId, Port>,
    source: &str,
) -> Result<Option<MeterFrame>, String> {
    match ports.get(source) {
        Some(Port::Meter(p)) => p.snapshot(),
        Some(_) => Err("source is not a meter port".into()),
        None => Ok(None),
    }
}

fn spectrum_pull(
    ports: &BTreeMap<VizSourceId, Port>,
    source: &str,
) -> Result<Option<SpectrumFrame>, String> {
    match ports.get(source) {
        Some(Port::Spectrum(p)) => p.snapshot(),
        Some(_) => Err("source is not a spectrum port".into()),
        None => Ok(None),
    }
}

/// Status for a bound surface whose meta could not be read — a port
/// of the wrong kind is a producer error, not absence (mirrors the
/// `*_pull` arm that returns `Err` on kind mismatch).
fn absent_or_kind(ports: &BTreeMap<VizSourceId, Port>, source: &str, kind: &str) -> VizStatus {
    match ports.get(source) {
        Some(_) => VizStatus::SourceError {
            detail: format!("source is not a {kind} port"),
        },
        None => VizStatus::NoSource,
    }
}

/// Status derived from landed snapshot truth — shared by resync and
/// notify so a port swap or late-arriving source never leaves a
/// stale status attached to a new producer epoch.
fn snapshot_status(frame_landed: bool, stale: bool) -> VizStatus {
    if !frame_landed {
        VizStatus::AwaitingData
    } else if stale {
        VizStatus::Stale
    } else {
        VizStatus::Ok
    }
}

fn surface_source(s: &Surface) -> Option<&str> {
    match s {
        Surface::Waveform(w) => Some(&w.source),
        Surface::Meter(m) => Some(&m.source),
        Surface::Spectrum(s) => Some(&s.source),
        Surface::Timeline(_) => None,
    }
}

/// Builds `TimelineItem`s from project clips — the DAW-semantic half
/// of the timeline contract. Beat-axis items come straight from the
/// musical anchor/extent (tempo map only needed for absolute anchors);
/// seconds items resolve through `ClipPlacement::resolve_span`.
fn timeline_items(
    session: &ProjectSession,
    axis: TimelineAxis,
    media_rate: u32,
) -> (Vec<TimelineItem>, usize) {
    use synvelis_project::{ClipExtent, TickTimeMapping, TimeAnchor};
    let project = session.project();
    let ppqn = f64::from(project.ppqn.0.max(1));
    let map = TickTimeMapping::new(&project.tempo_map).ok();
    let mut unresolved = 0usize;
    let mut items = Vec::with_capacity(project.clips.len());
    for clip in project.clips.values() {
        let span = match axis {
            TimelineAxis::Beats => {
                // Canonical composition — the anchor's media offset
                // must be included before an absolute extent converts
                // to ticks (`resolve_musical_range` owns this rule).
                let range = match (clip.placement.anchor, clip.placement.extent) {
                    (TimeAnchor::Musical(t), ClipExtent::Musical(d)) => {
                        t.value().checked_add(d.value()).map(|e| (t.value(), e))
                    }
                    _ => map.as_ref().and_then(|m| {
                        clip.placement
                            .resolve_musical_range(m, media_rate)
                            .ok()
                            .map(|r| (r.start().value(), r.end().value()))
                    }),
                };
                range.map(|(s, e)| (tick_as_f64(s) / ppqn, tick_as_f64(e) / ppqn))
            }
            TimelineAxis::Seconds => clip
                .placement
                .resolve_span(map.as_ref(), media_rate)
                .ok()
                .map(|span| {
                    (
                        tick_as_f64(span.start().value()) / f64::from(media_rate),
                        tick_as_f64(span.end().value()) / f64::from(media_rate),
                    )
                }),
        };
        let Some((start, end)) = span else {
            unresolved += 1;
            continue;
        };
        items.push(LaneItem::new(
            start,
            end,
            TimelinePayload {
                clip: clip.id.to_tagged(),
                lane: clip.lane_id.to_tagged(),
                source: clip.source_id.to_tagged(),
                muted: clip.muted,
            },
        ));
    }
    items.sort_by(|a, b| a.start.total_cmp(&b.start));
    (items, unresolved)
}

/// Project length in seconds — the latest clip end on the seconds
/// axis, `0.0` when no clip resolves. Shares [`timeline_items`] with the
/// timeline surface, so the length a consumer reads agrees with the
/// content span the timeline shows.
#[must_use]
pub fn project_length_seconds(session: &ProjectSession, media_rate: u32) -> f64 {
    timeline_items(session, TimelineAxis::Seconds, media_rate)
        .0
        .iter()
        .map(|item| item.end)
        .fold(0.0, f64::max)
}
