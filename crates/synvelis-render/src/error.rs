//! Typed export failure classes. Every variant names a distinct stage
//! so a caller can report exactly where an export died; a failure
//! never produces a receipt and never reaches the destination.

use std::fmt;
use std::path::PathBuf;

use synvelis_player::ResolveError;
use synvelis_project::{AssetId, ClipId, CompId, SourceId};

use ruviform_audio::render::RenderError;

/// Refusal before any output begins. Every variant maps to one
/// preflight obligation of the export contract — detectable without
/// touching the destination.
#[derive(Debug)]
#[non_exhaustive]
pub enum PreflightError {
    /// A non-muted clip references a source absent from the project.
    MissingSource {
        /// The clip.
        clip_id: ClipId,
        /// The unresolved source.
        source_id: SourceId,
    },
    /// The backing asset registration is absent.
    MissingAsset {
        /// The missing asset.
        asset_id: AssetId,
    },
    /// The source exists but is below `Verified`.
    UnverifiedSource {
        /// The source.
        source_id: SourceId,
        /// Its current state label.
        state: String,
    },
    /// Non-PCM codec, or a source rate that cannot match the engine
    /// (implicit resampling is forbidden).
    UnsupportedCodec {
        /// The offending source.
        source_id: SourceId,
        /// Underlying diagnostic.
        detail: String,
    },
    /// `tail` was left unclassified — the export cannot bound how far
    /// past `range_end` processing must run.
    UnknownTail,
    /// `end <= start`, a zero engine rate, or an out-of-representation
    /// value.
    InvalidRange {
        /// Human-readable detail.
        detail: String,
    },
    /// The WAV backend does not accept the requested output shape.
    InsufficientCapability {
        /// Human-readable detail.
        detail: String,
    },
    /// Destination exists and `replace_existing` is false — detected
    /// before the staged temp file is even created.
    DestinationConflict {
        /// Occupied destination.
        path: PathBuf,
    },
    /// A non-muted clip could not be lowered into the render graph —
    /// unresolvable anchor, rate mismatch reaching lowering, or a
    /// missing source. Lowering reports skips explicitly and a skipped
    /// clip is a refused render, never silent output.
    UnrenderableClip {
        /// The clip that produced no graph node.
        clip_id: ClipId,
        /// Lowering-reported reason.
        reason: String,
    },
    /// A comp-produced node could not be lowered, or an active comp
    /// could not be projected at all — the same explicit-skip rule:
    /// a failed comp node is a refused render, never silent output.
    UnrenderableComp {
        /// The comp whose node failed.
        comp_id: CompId,
        /// The failing node identity plus lowering-reported reason.
        detail: String,
    },
    /// Lowering produced no clip nodes at all — an all-muted or
    /// clip-free export publishes nothing, so it refuses early rather
    /// than emit a silent file that looks rendered.
    NothingToRender,
    /// The plugin-state recipe demands `Strict` quality but the
    /// supplied evidence is not exact-eligible (`BestEffort`,
    /// `Unsupported` or failed records present) — the offline
    /// export fails closed rather than claim exact reproduction
    /// (TIM-04).
    PluginStateQuality {
        /// Human-readable detail naming the failing evidence.
        detail: String,
    },
}

impl fmt::Display for PreflightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSource { clip_id, source_id } => {
                write!(f, "clip {clip_id} references missing source {source_id}")
            }
            Self::MissingAsset { asset_id } => write!(f, "missing asset {asset_id}"),
            Self::UnverifiedSource { source_id, state } => {
                write!(f, "source {source_id} is {state}, not verified")
            }
            Self::UnsupportedCodec { source_id, detail } => {
                write!(f, "source {source_id}: unsupported codec/format: {detail}")
            }
            Self::UnknownTail => write!(f, "render tail is unclassified"),
            Self::InvalidRange { detail } => write!(f, "invalid render range: {detail}"),
            Self::InsufficientCapability { detail } => {
                write!(f, "insufficient output capability: {detail}")
            }
            Self::DestinationConflict { path } => {
                write!(f, "destination exists: {}", path.display())
            }
            Self::UnrenderableClip { clip_id, reason } => {
                write!(f, "clip {clip_id} could not be rendered: {reason}")
            }
            Self::UnrenderableComp { comp_id, detail } => {
                write!(f, "comp {comp_id} node could not be rendered: {detail}")
            }
            Self::NothingToRender => write!(f, "no clips to render"),
            Self::PluginStateQuality { detail } => {
                write!(f, "plugin state quality insufficient: {detail}")
            }
        }
    }
}

impl std::error::Error for PreflightError {}

/// A typed export failure with its stage evidence attached.
#[derive(Debug)]
#[non_exhaustive]
pub enum ExportError {
    /// Refused before output — capability, range, tail, source
    /// registration/state or destination policy.
    Preflight(PreflightError),
    /// Verified-source resolution/decode failed while preparing the
    /// render inputs.
    Resolve(ResolveError),
    /// Graph validation or compile failed.
    Graph {
        /// Underlying diagnostic.
        detail: String,
    },
    /// The sequential driver failed — one `RenderError` variant per
    /// ruviform pipeline stage.
    Render(RenderError),
    /// Staging or scratch I/O failed before the driver ran.
    Io {
        /// Underlying diagnostic.
        detail: String,
    },
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preflight(e) => write!(f, "{e}"),
            Self::Resolve(e) => write!(f, "source resolution failed: {e}"),
            Self::Graph { detail } => write!(f, "graph validation failed: {detail}"),
            Self::Render(e) => write!(f, "{e}"),
            Self::Io { detail } => write!(f, "I/O failure: {detail}"),
        }
    }
}

impl std::error::Error for ExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Preflight(e) => Some(e),
            Self::Resolve(e) => Some(e),
            Self::Render(e) => Some(e),
            _ => None,
        }
    }
}

impl From<PreflightError> for ExportError {
    fn from(e: PreflightError) -> Self {
        Self::Preflight(e)
    }
}

impl From<ResolveError> for ExportError {
    fn from(e: ResolveError) -> Self {
        Self::Resolve(e)
    }
}

impl From<RenderError> for ExportError {
    fn from(e: RenderError) -> Self {
        Self::Render(e)
    }
}
