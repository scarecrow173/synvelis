//! RCP-04 — Missing Plugin Subgraph Resolver, Execution Intent,
//! and Safe Plan Reconciliation (synvelis#293), headless slice.
//!
//! This module implements the product-side resolver contract from
//! `docs/specs/render-checkpoint.md` §RCP-04 inside
//! `synvelis-player` only:
//!
//! - [`model`] — the pinned `ResolvedGraphSnapshot` typed-graph
//!   contract plus plugin declarations, capability snapshots and
//!   render candidate indexes (all inputs are explicit pins; no
//!   I/O, no runtime IDs, nothing persisted into `Project`).
//! - [`verify`] — plugin identity/availability/state/port-layout/
//!   host-capability verification into typed [`verify::PluginVerdict`]s.
//! - [`cutset`] — typed cutset validation for verified-render
//!   substitution (boundary ports, domains, latency/PDC, pinned
//!   inputs, dangling/double-feed/control rules).
//! - [`intent`] — the `ExecutionIntent` model (`MainPlayback` /
//!   `EditingPreview` / `BranchAudition` / `OfflineRender` /
//!   `Export`) with explicit non-equivalence opt-in.
//! - [`select`] — [`select::ExecutionSelector`] producing
//!   `ExecutionSelection` + `ReconciliationReceipt`.
//! - [`reconcile`] — edit classification (`NoRebuild`/`ControlOnly`/
//!   `Rebuild`), stale-result rejection, last-good retention, and
//!   the automation/parameter delivery gate.
//!
//! Explicit non-goals of this slice: no workspace semantic→
//! selection integration (deferred residual — another lease owns
//! `synvelis-workspace`), no Ruviform compiler/plugin-host/DSP
//! duplication, no render-job orchestration, no RT-path work, and
//! no real-plugin/OS/PCM acceptance (RCP-GATE / synvelis#296).

pub mod cutset;
pub mod intent;
pub mod model;
pub mod reconcile;
pub mod select;
pub mod verify;

pub use cutset::{CutsetRejection, CutsetReport, validate_cutset};
pub use intent::ExecutionIntent;
pub use model::{
    AccessState, ArtifactEligibility, ArtifactOutput, BoundaryPurpose, BoundaryRequirement,
    DeclaredPort, EdgeKind, NodeClass, PluginCapabilitySnapshot, PluginDeclaration,
    PluginEnvironment, PluginFormat, PluginIdentity, PluginKey, PluginStateDecl, PortEndpoint,
    RcpEdgeId, RcpNodeId, RcpPortId, RenderCandidate, RenderCandidateIndex, ReproducibilityClass,
    ResolvedEdge, ResolvedGraphSnapshot, ResolvedNode, ResolvedPort, StateRestoreSupport,
};
pub use reconcile::{
    CommittedPlan, DeliveryDecision, EditClass, EditKind, PlanContext, PlanReconciler,
    PreparedPlan, ReconcileOutcome, classify_edit,
};
pub use select::{
    AttemptOutcome, CutsetAttempt, ExecutionSelection, ExecutionSelector, LiveRegion,
    ReconciliationReceipt, SelectionVerdict, SkippedNode, Substitution, UnresolvedNode,
    UnresolvedReason,
};
pub use verify::{IdentityField, PluginVerdict, verify_plugin, verify_snapshot};
