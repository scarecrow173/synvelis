//! Deterministic render-job cost model (synvelis#518, AC-3).
//!
//! `JobCostEstimate` on a [`JobIntent`] is a *caller declaration* —
//! historically every routing site passed `JobCostEstimate::default()`
//! (all zeros), so budget and storage-quota gates trusted a stub.
//! [`model_estimate`] derives a conservative floor from what the job
//! actually is: the resolved recipe output contracts (or the demand
//! boundary when resolution failed), the demand frame intervals, and
//! the quality profile's output layout. [`JobOrchestrator::submit`]
//! merges it with the caller declaration element-wise (`max`) — the
//! caller can only *tighten* the estimate, never undercut the model,
//! and [`CostCalibration`] lets a measured-cost probe raise it
//! further when real observations exist.
//!
//! The coefficients are an initial conservative model, not a
//! benchmark: unresolved boundaries assume the quality-profile
//! layout at full boundary port count, `wall_ms` assumes no
//! parallelism, and storage counts staged + published payload plus a
//! per-demand manifest allowance. Every arithmetic step saturates —
//! an overflowing estimate fails closed (larger is always safer for
//! budget gates).

use crate::recipe::{FrameInterval, PcmFormat};
use crate::render_job::model::{
    BoundaryResolution, JobCostEstimate, RenderDemand, RenderSelection,
};
use crate::render_job::orchestrator::RenderQualityInputs;

/// Per-recipe measured-cost seam — a probe that has observed real
/// render costs for a [`crate::recipe::RecipeKey`] reports them here.
/// The orchestrator folds observations into the model floor; absent
/// observations simply leave the coefficient model in place.
pub trait CostCalibration {
    /// Measured cost for `recipe_key`, when this probe has one.
    fn observed(&self, recipe_key: &crate::recipe::RecipeKey) -> Option<JobCostEstimate>;
}

/// Fail-closed default: no measurements — the coefficient model
/// stands alone.
pub struct NoCostCalibration;

impl CostCalibration for NoCostCalibration {
    fn observed(&self, _recipe_key: &crate::recipe::RecipeKey) -> Option<JobCostEstimate> {
        None
    }
}

/// Manifest/journal allowance per demand, on top of payload bytes —
/// covers the publish draft, manifest, and receipt-side bookkeeping
/// the verified pipeline writes per artifact.
const MANIFEST_OVERHEAD_BYTES: u64 = 4096;

/// Frame-channel work units per estimated CPU millisecond. Chosen so
/// a 48 kHz stereo minute (~5.76M frame-channels) estimates ~5.6 s —
/// conservative for simple mixes, and replaceable per-recipe via
/// [`CostCalibration`] once real measurements exist.
const FRAME_CHANNELS_PER_CPU_MS: u64 = 1024;

/// Bytes per PCM sample for `format`.
#[must_use]
pub fn pcm_format_bytes(format: PcmFormat) -> u64 {
    match format {
        PcmFormat::F32 | PcmFormat::I32 => 4,
        PcmFormat::F64 => 8,
        PcmFormat::I16 => 2,
        PcmFormat::I24 => 3,
    }
}

/// Frames covered by `interval` (`end - start`; `FrameInterval`
/// guarantees `end > start`).
#[must_use]
fn interval_frames(interval: FrameInterval) -> u64 {
    interval.end.saturating_sub(interval.start)
}

/// Resolves the per-frame byte and channel cost for one demand:
/// resolved recipes use their declared output contract; anything
/// else falls back to the demand boundary at the quality profile's
/// layout — the conservative same-size assumption.
fn demand_frame_cost(
    demand: &RenderDemand,
    resolution: Option<&crate::recipe::RecipeResolution>,
    quality: &RenderQualityInputs,
) -> (u64, u64) {
    if let Some(crate::recipe::RecipeResolution::Resolved { recipe, .. }) = resolution {
        let mut bytes_per_frame = 0u64;
        let mut channels = 0u64;
        for port in &recipe.output_contract.ports {
            bytes_per_frame = bytes_per_frame.saturating_add(
                u64::from(port.channels).saturating_mul(pcm_format_bytes(port.format)),
            );
            channels = channels.saturating_add(u64::from(port.channels));
        }
        return (bytes_per_frame, channels);
    }
    // Unresolved/unsupported boundary: assume every declared boundary
    // port carries the quality profile's layout.
    let ports = u64::try_from(demand.boundary.ports.len()).unwrap_or(u64::MAX);
    (
        ports
            .saturating_mul(u64::from(quality.channels))
            .saturating_mul(pcm_format_bytes(quality.format)),
        ports.saturating_mul(u64::from(quality.channels)),
    )
}

/// Derives the model cost floor for one selection — see module docs.
/// `boundaries` must be the same per-demand resolutions `submit`
/// computed; `calibration` may be `None`.
#[must_use]
pub fn model_estimate(
    selection: &RenderSelection,
    boundaries: &[BoundaryResolution],
    quality: &RenderQualityInputs,
    calibration: Option<&dyn CostCalibration>,
) -> JobCostEstimate {
    let mut payload_bytes = 0u64;
    let mut frame_channels = 0u64;
    let mut calibrated_cpu = 0u64;
    let mut calibrated_wall = 0u64;
    let mut calibrated_storage = 0u64;

    for (index, demand) in selection.demands.iter().enumerate() {
        let resolution = boundaries.iter().find_map(|b| match b {
            BoundaryResolution::Resolved {
                demand: d,
                resolution,
            } if *d == index => Some(resolution.as_ref()),
            _ => None,
        });
        let (bytes_per_frame, channels_per_frame) = demand_frame_cost(demand, resolution, quality);
        let frames = interval_frames(demand.interval);
        payload_bytes = payload_bytes.saturating_add(frames.saturating_mul(bytes_per_frame));
        frame_channels = frame_channels.saturating_add(frames.saturating_mul(channels_per_frame));

        if let Some(crate::recipe::RecipeResolution::Resolved { key, .. }) = resolution
            && let Some(obs) = calibration.and_then(|c| c.observed(key))
        {
            calibrated_cpu = calibrated_cpu.saturating_add(obs.cpu_ms);
            calibrated_wall = calibrated_wall.saturating_add(obs.wall_ms);
            calibrated_storage = calibrated_storage.saturating_add(obs.storage_bytes);
        }
    }

    // Staged + published payload, plus per-demand manifest allowance.
    let storage_bytes = payload_bytes
        .saturating_mul(2)
        .saturating_add(
            u64::try_from(selection.demands.len())
                .unwrap_or(u64::MAX)
                .saturating_mul(MANIFEST_OVERHEAD_BYTES),
        )
        .max(calibrated_storage);

    let coeff_cpu =
        frame_channels.saturating_add(FRAME_CHANNELS_PER_CPU_MS - 1) / FRAME_CHANNELS_PER_CPU_MS;
    let cpu_ms = coeff_cpu.max(calibrated_cpu);
    // No parallelism assumed — wall tracks CPU conservatively.
    let wall_ms = cpu_ms.max(calibrated_wall);

    JobCostEstimate {
        cpu_ms,
        wall_ms,
        storage_bytes,
    }
}

/// Merges the caller declaration with the model floor element-wise —
/// the caller may tighten but never undercut the model (AC-3).
#[must_use]
pub fn merge_estimate(caller: JobCostEstimate, model: JobCostEstimate) -> JobCostEstimate {
    JobCostEstimate {
        cpu_ms: caller.cpu_ms.max(model.cpu_ms),
        wall_ms: caller.wall_ms.max(model.wall_ms),
        storage_bytes: caller.storage_bytes.max(model.storage_bytes),
    }
}
