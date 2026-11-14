//! Render job orchestration — pinned-target, non-real-time render
//! jobs with idempotent keys, independent receipts, cooperative
//! cancel/retry/resume, and typed completeness verdicts (RCP-03,
//! synvelis#292).
//!
//! [`JobOrchestrator`] consumes [`crate::policy::trigger::TriggerReport`]s,
//! resolves effective policy, pins immutable snapshots, coalesces
//! duplicate work on [`RenderJobKey`], gates jobs on
//! budget/grant/consent preflight, drives the [`RenderJobState`]
//! machine, and publishes exclusively through the RCP-02 verified
//! pipeline. Everything is headless: executor, store, probes, and
//! authorizers are injected ports — real async scheduling, RT lanes,
//! OS stores, VCS providers, and gateway projection are later gates.

pub mod model;
pub mod orchestrator;

pub use model::{
    BoundaryResolution, CompletenessGap, CompletenessGapKind, CompletenessReport,
    CompletenessVerdict, EntrySurface, GatewayActionRef, GrantRef, JobCostEstimate, JobFailure,
    JobFailureReason, JobPurpose, JobReceipt, PinnedSnapshot, ReceiptKind, RenderDemand,
    RenderJobKey, RenderJobState, RenderSelection, ShareGateVerdict,
};
pub use orchestrator::{
    BackendRequirement, BoundaryExec, DriveOutcome, JobContext, JobOrchestrator, JobRecord,
    RejectedAttempt, RenderExecution, RenderExecutor, RenderQualityInputs, RenderSettingsProbe,
    RenderedOutput, SubmitOutcome, TemporalQuality,
};
