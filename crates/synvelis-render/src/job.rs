//! Job model: progress/cancel plumbing and the typed terminal
//! outcome. `Completed` exists only after reopen verification and a
//! successful publish — a render that dies anywhere earlier is
//! `Failed`/`Cancelled` and carries no receipt.

use std::collections::BTreeMap;

use ruviform_audio::render::{RenderError, RenderReceipt, TailEvidence};
use ruviform_audio::typed::OfflineRenderSupport;
use ruviform_exec::{CancellationToken, Progress};
use ruviform_graph::NodeId;

use crate::error::ExportError;
use crate::snapshot::SnapshotIds;
use crate::spec::PluginStateRecipe;
use crate::verify::VerifiedOutput;

/// Cooperative job controls handed to the driver — `Progress` counts
/// processed engine frames, `CancellationToken` is honored at every
/// physical-call boundary and before drain/finalize/verify/publish.
pub struct ExportJob {
    /// Processed engine frames.
    pub progress: Progress,
    /// Cooperative cancel.
    pub cancel: CancellationToken,
}

impl ExportJob {
    /// A fresh job with zero progress.
    #[must_use]
    pub fn new() -> Self {
        Self {
            progress: Progress::new(),
            cancel: CancellationToken::new(),
        }
    }
}

impl Default for ExportJob {
    fn default() -> Self {
        Self::new()
    }
}

/// The pipeline stage a `Failed` outcome reports — mirrors the
/// [`RenderError`] variant classes plus the pre-render stages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportStage {
    /// Refused before output — capability/range/tail/source/
    /// destination preflight.
    Preflight,
    /// Verified-source resolution/decode.
    Prepare,
    /// Graph validation or compile.
    Compile,
    /// The sequential process loop (process, transport,
    /// segmentation, event channel, unsupported-shape).
    Process,
    /// Encoder send/receive or muxer write.
    Output,
    /// `encoder.drain()`.
    Drain,
    /// Muxer `finish` (trailer/flush — the close/sync class).
    Finalize,
    /// Reopen verification hook.
    Verify,
    /// Publish (sync/close/rename) or destination-exists at publish.
    Publish,
}

/// Backend/capability evidence attached to the receipt — L2-07.
#[derive(Clone, Debug)]
pub struct CapabilityEvidence {
    /// Wire encodings `ruviform-io-wav` accepts (`rf#191` backend
    /// capability proof).
    pub wav_sample_formats: Vec<String>,
    /// Per-node offline support captured at `PreparedRender` compile
    /// (`rf#198` stateful gateway proof — every node must be
    /// `Verified` for the render to have run).
    pub node_support: BTreeMap<NodeId, OfflineRenderSupport>,
    /// `plan.total_latency` the PDC emit math used.
    pub total_latency: u64,
    /// Tail classification actually rendered.
    pub tail_evidence: TailEvidence,
    /// io-wav container facts (single-stream, no RF64/BWF).
    pub wav_rf64: bool,
    /// Whether `ChannelLayout::Custom` output is writable.
    pub wav_custom_layouts: bool,
}

/// Terminal receipt of a completed export — only constructed after
/// reopen verification and a successful publish.
#[derive(Debug)]
pub struct ExportReceipt {
    /// Ruviform render receipt (frames emitted, blocks, plan
    /// generation, tail evidence, stale-skip count).
    pub render: RenderReceipt,
    /// Frozen render-input identities — the proof the render ran on a
    /// snapshot distinct from Live/Saved revisions.
    pub snapshot: SnapshotIds,
    /// Backend/capability evidence.
    pub capability: CapabilityEvidence,
    /// What the independent reopen observed — always present on
    /// `Completed`.
    pub verified: VerifiedOutput,
    /// Plugin-state quality recipe evidence (TIM-04), recorded
    /// verbatim — `Some` only when the request carried a recipe.
    /// A `Standard` recipe's degraded evidence stays visible here;
    /// it is never upgraded to an exact claim.
    pub plugin_state: Option<PluginStateRecipe>,
}

/// Terminal export outcome. `Partial` never reaches `Completed`: a
/// failed render carries the stage-typed error, a cancelled one the
/// phase — only a reopened, published file yields `Completed`.
#[derive(Debug)]
#[non_exhaustive]
pub enum ExportOutcome {
    /// Reopen-verified and published.
    Completed {
        /// The terminal receipt.
        receipt: Box<ExportReceipt>,
    },
    /// A typed stage failure — no receipt, nothing published.
    Failed {
        /// Where it died.
        stage: ExportStage,
        /// The typed failure.
        error: ExportError,
    },
    /// Cooperative cancellation landed at a phase boundary.
    Cancelled {
        /// `prepare`/`process`/`drain`/`finalize`/`verify`/`publish`.
        phase: &'static str,
    },
}

impl ExportOutcome {
    /// `true` when reopen-verified and published.
    #[must_use]
    pub const fn is_completed(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }
}

/// Maps a [`RenderError`] to its pipeline stage.
#[must_use]
pub const fn stage_of(err: &RenderError) -> ExportStage {
    match err {
        RenderError::Preflight(_) => ExportStage::Preflight,
        RenderError::Output { .. } => ExportStage::Output,
        RenderError::Drain { .. } => ExportStage::Drain,
        RenderError::Finalize { .. } => ExportStage::Finalize,
        RenderError::Verify { .. } => ExportStage::Verify,
        RenderError::DestinationExists { .. } | RenderError::Publish { .. } => ExportStage::Publish,
        _ => ExportStage::Process,
    }
}
