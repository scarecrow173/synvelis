//! Job orchestration — resolve → queue → prepare → render → verify →
//! publish for pinned, non-real-time render jobs (RCP-03,
//! synvelis#292).
//!
//! [`JobOrchestrator`] is the single writer of render-job state. It
//! consumes typed [`TriggerReport`]s, resolves effective policy with
//! provenance, pins immutable snapshots, suppresses duplicate work
//! through [`RenderJobKey`] coalescing, gates every job on
//! budget/grant/consent preflight, drives the [`RenderJobState`]
//! machine with append-only [`JobReceipt`]s, and publishes
//! exclusively through the RCP-02 verified pipeline
//! ([`crate::artifact::publish::publish_artifact`]) — never another
//! renderer, media writer, or VCS writer.
//!
//! Hard boundaries this module keeps:
//!
//! - Jobs write only to their pinned target; a later project
//!   revision can never re-associate a completion (RCP-POL-03).
//! - `OnDemand` is proposal-only until a [`DemandAuthorizer`] grants
//!   it; no unapproved CPU/GPU/network is consumed.
//! - Offline render failure never silently falls back to realtime.
//! - Save/VCS/share outcomes and render-job outcomes are separate
//!   receipt streams ([`ExternalReceipt`] vs [`JobReceipt`]).
//! - Everything here is headless: the executor, stores, probes, and
//!   authorizers are injected ports. Real async scheduling, RT lanes,
//!   OS stores, VCS providers, and gateway projection are later
//!   gates.

use std::collections::{BTreeMap, BTreeSet};

use ruviform_exec::CancellationToken;

use synvelis_project::render_input::{ResolvedRenderProject, resolve_render_project};
use synvelis_project::snapshot::ContentDigest;
use synvelis_project::{ProjectId, ProjectRevision, RENDER_INPUT_SCHEMA_VERSION, SCHEMA_VERSION};

use crate::artifact::lookup::{CacheLookup, IntendedUse, LookupOutcome, TrustScope, lookup};
use crate::artifact::manifest::{
    ArtifactPortSpec, ArtifactProvenance, ArtifactReproducibility, ArtifactTrust, CodecDescriptor,
    RenderArtifactId,
};
use crate::artifact::publish::{ArtifactDraft, PublishError, PublishOutcome, publish_artifact};
use crate::artifact::share::{ExternalManifestRef, attach_external_manifest};
use crate::artifact::store::{ArtifactStore, StoreError};
use crate::dependency::{
    ExternalAssetProbe, NoExternalAssets, NoPluginStates, PluginStateProbe, RecipeEnvironment,
    RecipeRequest, resolve_recipe,
};
use crate::policy::model::{
    CompletionRequirement, EffectivePolicy, LocalGrants, PolicyPatch, RenderSettingsRef,
    RenderTrigger, resolve_effective_policy,
};
use crate::policy::trigger::{
    CommitEvent, DemandAuthorization, DemandAuthorizer, DemandProposal, ExternalReceipt,
    InertReason, JobIntent, PolicyResolver, RoutedTrigger, SaveEvent, ScopeDemandResolver,
    ShareRequest, TriggerEvent, TriggerOutcome, TriggerProvenance, TriggerReport,
};
use crate::recipe::{
    OutputContract, OutputPortContract, RecipeKey, RecipeResolution, RenderBoundary,
    RenderRecipeV1, RendererProfile, ResolutionProvenance, TemporalDomain,
};
use crate::render_job::model::{
    BoundaryResolution, CompletenessGap, CompletenessGapKind, CompletenessReport,
    CompletenessVerdict, EntrySurface, GatewayActionRef, GrantRef, JobCostEstimate, JobFailure,
    JobFailureReason, JobPurpose, JobReceipt, PinnedSnapshot, ReceiptKind, RenderDemand,
    RenderJobKey, RenderJobState, RenderSelection, ShareGateVerdict,
};

// ---------------------------------------------------------------------------
// executor + settings ports
// ---------------------------------------------------------------------------

/// What compute backend a render executor requires — checked against
/// [`LocalGrants`] before any work is issued. A remote backend is
/// never implied: it must be named *and* approved (RCP-POL-06).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackendRequirement {
    /// Local CPU render — requires `cpu_render`.
    Cpu,
    /// Local GPU render — requires `gpu_render`.
    Gpu,
    /// Remote backend by identifier — requires `network` *and* the
    /// id present in `remote_backends`.
    Remote {
        /// Backend identifier.
        backend_id: String,
    },
}

/// Temporal quality fields — the [`TemporalDomain`] fields that come
/// from the render-settings document rather than the per-demand
/// interval. Changing any of these moves every `RecipeKey` the
/// policy produces (RCP-POL-02).
#[derive(Clone, Debug)]
pub struct TemporalQuality {
    /// Pre-roll frames.
    pub preroll_frames: u64,
    /// Warmup frames.
    pub warmup_frames: u64,
    /// Tail frames.
    pub tail_frames: u64,
    /// Declared DSP lookahead.
    pub lookahead_frames: u64,
    /// PDC total.
    pub pdc_frames: u64,
    /// Clock dependency.
    pub clock: crate::recipe::ClockDependency,
    /// Loop region inside the render interval.
    pub loop_region: Option<crate::recipe::FrameInterval>,
    /// Declared seek boundaries.
    pub seek_boundaries: Vec<u64>,
}

/// The quality-affecting render settings resolved from a
/// [`RenderSettingsRef`] — everything the recipe's
/// temporal/renderer/output domains need *except* the per-demand
/// interval and boundary ports, which come from the pinned target.
/// Only changes here may move a `RecipeKey`.
#[derive(Clone, Debug)]
pub struct RenderQualityInputs {
    /// Output sample rate.
    pub sample_rate_hz: u32,
    /// Output channel count (uniform across boundary ports in this
    /// slice — multi-layout ports are a later refinement).
    pub channels: u32,
    /// Output PCM layout.
    pub format: crate::recipe::PcmFormat,
    /// Temporal quality fields.
    pub temporal: TemporalQuality,
    /// Renderer profile (identity, precision, backend, seed,
    /// denormal, compatibility digest).
    pub renderer: RendererProfile,
}

/// Port resolving a [`RenderSettingsRef`] into concrete quality
/// inputs — the settings-document lookup owned by the
/// project/settings layer (read-only consumption here).
pub trait RenderSettingsProbe {
    /// Resolves `reference` into quality inputs, or `None` when the
    /// referenced document is unavailable (fail closed upstream).
    fn resolve(&self, reference: &RenderSettingsRef) -> Option<RenderQualityInputs>;
}

/// What the executor produced for one resolved recipe — bytes plus
/// the declared media facts the orchestrator needs to build the
/// publish draft. The orchestrator owns recipe/target/snapshot/
/// temporal/renderer/provenance fields of the draft so the manifest
/// always matches the resolved recipe exactly.
pub struct RenderedOutput {
    /// Rendered output bytes (container/wire format).
    pub blob: Vec<u8>,
    /// Declared container/codec/lossless class — verified against
    /// independent decode at publish.
    pub codec: CodecDescriptor,
    /// What the blob must decode as.
    pub expect: crate::artifact::publish::PublishExpectation,
    /// Optional decoded-PCM comparison.
    pub expect_pcm: Option<crate::spec::PcmExpect>,
    /// Frames cut from the head of the source interval.
    pub cut_frames: u64,
    /// Effective output latency in frames.
    pub latency_frames: u64,
    /// Reproducibility class.
    pub reproducibility: ArtifactReproducibility,
    /// Shared/remote trust metadata (`None` = local artifact).
    pub trust: Option<ArtifactTrust>,
}

impl std::fmt::Debug for RenderedOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderedOutput")
            .field("blob_len", &self.blob.len())
            .field("codec", &self.codec)
            .field("expect", &self.expect)
            .field("expect_pcm", &self.expect_pcm.as_ref().map(|_| "<fn>"))
            .field("cut_frames", &self.cut_frames)
            .field("latency_frames", &self.latency_frames)
            .field("reproducibility", &self.reproducibility)
            .field("trust", &self.trust)
            .finish()
    }
}

/// Terminal outcome of one render attempt.
#[derive(Debug)]
pub enum RenderExecution {
    /// Output produced — orchestrator runs verified publish.
    Output(Box<RenderedOutput>),
    /// Cooperative cancellation observed by the executor.
    Cancelled,
    /// Worker/process died mid-render — nothing may be assumed
    /// published; the job stays resumable at `Rendering`.
    Crashed,
    /// Typed render failure.
    Failed {
        /// Typed reason.
        reason: JobFailureReason,
        /// Detail.
        detail: String,
    },
}

/// Read-only context handed to the executor for one render call.
pub struct JobContext<'a> {
    /// The job's immutable pinned target.
    pub snapshot: &'a PinnedSnapshot,
    /// The resolved recipe being rendered.
    pub recipe: &'a RenderRecipeV1,
    /// The boundary/demand interval being rendered.
    pub demand: &'a RenderDemand,
}

/// The injected render backend — the only place actual audio
/// generation happens. Implementations must be deterministic given
/// the pinned snapshot + recipe and must honor `cancel`
/// cooperatively; they must never publish (the orchestrator owns the
/// verified publish path) and never fall back to realtime.
pub trait RenderExecutor {
    /// Backend requirement — preflighted against [`LocalGrants`]
    /// before the job is admitted.
    fn backend_requirement(&self) -> BackendRequirement;

    /// Renders `recipe` (already resolved against `job`'s pinned
    /// snapshot).
    fn render(
        &mut self,
        job: &JobContext<'_>,
        recipe: &RenderRecipeV1,
        cancel: &CancellationToken,
    ) -> RenderExecution;
}

// ---------------------------------------------------------------------------
// job record + orchestrator state
// ---------------------------------------------------------------------------

/// Per-demand execution outcome — the job record's audit trail for
/// which boundaries produced (or failed to produce) artifacts.
#[derive(Clone, Debug)]
pub enum BoundaryExec {
    /// Artifact published or reused for this demand.
    Done {
        /// Demand index.
        demand: usize,
        /// Recipe key.
        key: RecipeKey,
        /// Artifact identity.
        artifact: RenderArtifactId,
        /// `true` when an existing verified artifact was reused
        /// instead of rendering.
        reused: bool,
    },
    /// Render/publish failed for this demand.
    Failed {
        /// Demand index.
        demand: usize,
        /// Recipe key when resolution succeeded.
        key: Option<RecipeKey>,
        /// Typed failure.
        failure: JobFailure,
    },
}

/// One tracked render job.
#[derive(Debug)]
pub struct JobRecord {
    /// Idempotent job identity.
    pub key: RenderJobKey,
    /// Current state.
    pub state: RenderJobState,
    /// Attempt counter (incremented by `retry`).
    pub attempt: u32,
    /// Environmental blocker while in `Queue` (`None` = runnable).
    pub pending_reason: Option<JobFailureReason>,
    /// Cooperative cancellation token.
    pub cancel: CancellationToken,
    /// Cancel requested but not yet landed.
    pub cancel_requested: bool,
    /// Crash observed — job resumable at `Rendering`.
    pub crashed: bool,
    /// Resolved effective policy (audit + gate input).
    pub policy: EffectivePolicy,
    /// Renderer profile resolved from the quality settings at
    /// submit — the recipe stores only its digest, but publish
    /// drafts and lookup fitness checks need the profile itself.
    pub renderer: RendererProfile,
    /// Pinned target — immutable for the job's life.
    pub selection: RenderSelection,
    /// Per-demand recipe resolutions (fixed at submit).
    pub boundaries: Vec<BoundaryResolution>,
    /// Per-demand execution outcomes.
    pub exec: Vec<BoundaryExec>,
    /// Published/reused artifact identities.
    pub artifacts: BTreeSet<RenderArtifactId>,
    /// External manifest attach seeds (commit locators).
    pub external_manifests: Vec<String>,
    /// Trigger provenance contributors.
    pub triggers: Vec<TriggerProvenance>,
    /// Entry surface (receipt provenance).
    pub entry: EntrySurface,
    /// Gateway action.
    pub action: GatewayActionRef,
    /// Grant, when present.
    pub grant: Option<GrantRef>,
    /// Saved revision when pinned via durable save.
    pub saved_revision: Option<ProjectRevision>,
    /// Cost estimate used at preflight.
    pub estimate: JobCostEstimate,
    /// Terminal failure, when `state == Failed`.
    pub failure: Option<JobFailure>,
    /// Completeness report from the verify stage.
    pub completeness: Option<CompletenessReport>,
    /// Created tick.
    pub created_tick: u64,
    /// Last update tick.
    pub updated_tick: u64,
}

/// What `submit` produced.
#[derive(Clone, Debug)]
pub enum SubmitOutcome {
    /// Job accepted — `coalesced` when an existing job absorbed it.
    Accepted {
        /// Job key.
        key: RenderJobKey,
        /// Whether this coalesced into existing work.
        coalesced: bool,
    },
    /// Rejected at pre-issuance validation — typed reason, no job
    /// created, no resources consumed.
    Rejected {
        /// Typed reason.
        reason: JobFailureReason,
        /// Detail.
        detail: String,
    },
}

/// What `drive`/`run` produced for one job.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DriveOutcome {
    /// Job reached a terminal state.
    Terminal,
    /// Job remains queued pending an environmental gate.
    Pending,
    /// Crash observed — resumable via `resume`.
    Crashed,
    /// Job key unknown.
    Unknown,
}

/// A pre-issuance rejection kept for audit — typed evidence that a
/// request was refused before any work existed.
#[derive(Clone, Debug)]
pub struct RejectedAttempt {
    /// Job key material that was rejected.
    pub detail: String,
    /// Typed reason.
    pub reason: JobFailureReason,
    /// Detail message.
    pub detail_msg: String,
}

/// Fail-closed default probes for orchestrators that do not inject
/// plugin-state or external-asset evidence.
static NO_PLUGIN_STATES: NoPluginStates = NoPluginStates;
static NO_EXTERNAL_ASSETS: NoExternalAssets = NoExternalAssets;

/// What the publish step produced for one render attempt.
enum PublishStep {
    Published(RenderArtifactId),
    Failed(JobFailure),
    Cancelled,
}

/// The render-job orchestrator — owns job state, receipts, and the
/// verified publish path for every trigger-driven render.
pub struct JobOrchestrator<'a> {
    store: &'a mut dyn ArtifactStore,
    executor: &'a mut dyn RenderExecutor,
    settings: &'a dyn RenderSettingsProbe,
    policies: &'a dyn PolicyResolver,
    demands: &'a dyn ScopeDemandResolver,
    plugin_states: &'a dyn PluginStateProbe,
    external_assets: &'a dyn ExternalAssetProbe,
    authorizer: Option<&'a dyn DemandAuthorizer>,
    grants: LocalGrants,
    jobs: BTreeMap<RenderJobKey, JobRecord>,
    receipts: Vec<JobReceipt>,
    external: Vec<ExternalReceipt>,
    rejected: Vec<RejectedAttempt>,
    tick: u64,
}

impl<'a> JobOrchestrator<'a> {
    /// Creates an orchestrator over the injected ports. `None` probes
    /// select the fail-closed defaults (every plugin state unknown,
    /// no external pins).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        store: &'a mut dyn ArtifactStore,
        executor: &'a mut dyn RenderExecutor,
        settings: &'a dyn RenderSettingsProbe,
        policies: &'a dyn PolicyResolver,
        demands: &'a dyn ScopeDemandResolver,
        plugin_states: Option<&'a dyn PluginStateProbe>,
        external_assets: Option<&'a dyn ExternalAssetProbe>,
        authorizer: Option<&'a dyn DemandAuthorizer>,
        grants: LocalGrants,
    ) -> Self {
        Self {
            store,
            executor,
            settings,
            policies,
            demands,
            plugin_states: plugin_states.unwrap_or(&NO_PLUGIN_STATES),
            external_assets: external_assets.unwrap_or(&NO_EXTERNAL_ASSETS),
            authorizer,
            grants,
            jobs: BTreeMap::new(),
            receipts: Vec::new(),
            external: Vec::new(),
            rejected: Vec::new(),
            tick: 0,
        }
    }

    /// The independent external receipt stream (save/commit/share
    /// outcomes) — disjoint from [`Self::receipts`].
    #[must_use]
    pub fn external_receipts(&self) -> &[ExternalReceipt] {
        &self.external
    }

    /// All render-job receipts in recording order — the independent
    /// job receipt stream.
    #[must_use]
    pub fn receipts(&self) -> &[JobReceipt] {
        &self.receipts
    }

    /// Receipts belonging to one job, in sequence order.
    #[must_use]
    pub fn job_receipts(&self, key: &RenderJobKey) -> Vec<JobReceipt> {
        self.receipts
            .iter()
            .filter(|r| &r.job == key)
            .cloned()
            .collect()
    }

    /// Pre-issuance rejections (audit evidence).
    #[must_use]
    pub fn rejected_attempts(&self) -> &[RejectedAttempt] {
        &self.rejected
    }

    /// A job record by key.
    #[must_use]
    pub fn job(&self, key: &RenderJobKey) -> Option<&JobRecord> {
        self.jobs.get(key)
    }

    /// All job keys in deterministic order.
    #[must_use]
    pub fn job_keys(&self) -> Vec<RenderJobKey> {
        self.jobs.keys().cloned().collect()
    }

    /// Updates the local grants (environment state can change —
    /// e.g. storage freed, consent granted).
    pub fn set_grants(&mut self, grants: LocalGrants) {
        self.grants = grants;
    }

    // ------------------------------------------------------------------
    // trigger handling
    // ------------------------------------------------------------------

    /// Handles one typed trigger report end-to-end: policy
    /// resolution, routing, submission/coalescing, driving to
    /// quiescence, and (for share) the completeness gate.
    pub fn handle(&mut self, report: &TriggerReport) -> TriggerOutcome {
        self.tick += 1;
        match self.route(report) {
            RoutedTrigger::Job(intent) => match self.submit(&intent) {
                SubmitOutcome::Accepted { key, coalesced } => {
                    if !coalesced {
                        // Headless slice drives synchronously; the
                        // state machine and receipts are identical
                        // to an async driver.
                        let _ = self.drive(&key);
                    }
                    TriggerOutcome::Job { key, coalesced }
                }
                SubmitOutcome::Rejected { reason, detail } => TriggerOutcome::Denied {
                    reason: format!("{reason:?}: {detail}"),
                },
            },
            RoutedTrigger::ShareGate {
                request,
                entry,
                action,
                grant,
            } => self.inspect_share(&request, entry, action, grant),
            RoutedTrigger::Inert { reason } => TriggerOutcome::Inert { reason },
            RoutedTrigger::Unsupported { capability, reason } => {
                TriggerOutcome::Unsupported { capability, reason }
            }
            RoutedTrigger::Denied { reason } => TriggerOutcome::Denied { reason },
        }
    }

    /// Routes a report without submitting — exposed for contract
    /// tests that need the routing decision itself.
    pub fn route(&mut self, report: &TriggerReport) -> RoutedTrigger {
        let project_id = report_project_id(&report.event);
        let declared = project_id.and_then(|id| self.policies.declared(id));
        let local = self.policies.local_override();

        match &report.event {
            TriggerEvent::Manual {
                selection,
                purpose,
                policy_intent,
            } => {
                let policy = resolve_effective_policy(
                    declared.as_ref(),
                    local.as_ref(),
                    policy_intent.as_ref(),
                );
                RoutedTrigger::Job(JobIntent {
                    selection: selection.clone(),
                    purpose: purpose.clone(),
                    policy,
                    triggers: vec![TriggerProvenance {
                        trigger: RenderTrigger::Manual,
                        detail: "manual".to_owned(),
                    }],
                    external_manifests: Vec::new(),
                    estimate: JobCostEstimate::default(),
                    saved_revision: None,
                    entry: report.entry,
                    action: report.action.clone(),
                    grant: report.grant.clone(),
                })
            }
            TriggerEvent::Save(event) => {
                self.route_save(event, declared.as_ref(), local.as_ref(), report)
            }
            TriggerEvent::Commit(event) => {
                self.route_commit(event, declared.as_ref(), local.as_ref(), report)
            }
            TriggerEvent::Share(request) => {
                let policy = resolve_effective_policy(
                    declared.as_ref(),
                    local.as_ref(),
                    request.policy_intent.as_ref(),
                );
                if !policy.trigger_enabled(RenderTrigger::OnShare) {
                    return RoutedTrigger::Inert {
                        reason: InertReason::TriggerDisabled,
                    };
                }
                RoutedTrigger::ShareGate {
                    request: request.clone(),
                    entry: report.entry,
                    action: report.action.clone(),
                    grant: report.grant.clone(),
                }
            }
            TriggerEvent::Demand(proposal) => {
                self.route_demand(proposal, declared.as_ref(), local.as_ref(), report)
            }
        }
    }

    fn route_save(
        &mut self,
        event: &SaveEvent,
        declared: Option<&PolicyPatch>,
        local: Option<&PolicyPatch>,
        report: &TriggerReport,
    ) -> RoutedTrigger {
        match event {
            SaveEvent::Failed {
                project_id,
                revision,
                reason,
            } => {
                // Save failure never launches OnSave — recorded on
                // the external stream only.
                self.external.push(ExternalReceipt::SaveFailed {
                    project_id: *project_id,
                    revision: *revision,
                    reason: reason.clone(),
                });
                RoutedTrigger::Inert {
                    reason: InertReason::ExternalFailed,
                }
            }
            SaveEvent::Completed(receipt) => {
                let policy = resolve_effective_policy(declared, local, None);
                if !policy.trigger_enabled(RenderTrigger::OnSave) {
                    return RoutedTrigger::Inert {
                        reason: InertReason::TriggerDisabled,
                    };
                }
                let purpose = JobPurpose::Scoped {
                    scope: policy.policy.scope,
                };
                let demands = self.demands.demands(&receipt.snapshot.project, &purpose);
                if demands.is_empty() {
                    return RoutedTrigger::Inert {
                        reason: InertReason::NoDemands,
                    };
                }
                RoutedTrigger::Job(JobIntent {
                    selection: RenderSelection {
                        snapshot: receipt.snapshot.clone(),
                        demands,
                    },
                    purpose,
                    policy,
                    triggers: vec![TriggerProvenance {
                        trigger: RenderTrigger::OnSave,
                        detail: format!("save head {}", receipt.head),
                    }],
                    external_manifests: Vec::new(),
                    estimate: JobCostEstimate::default(),
                    saved_revision: Some(receipt.saved_revision),
                    entry: report.entry,
                    action: report.action.clone(),
                    grant: report.grant.clone(),
                })
            }
        }
    }

    fn route_commit(
        &mut self,
        event: &CommitEvent,
        declared: Option<&PolicyPatch>,
        local: Option<&PolicyPatch>,
        report: &TriggerReport,
    ) -> RoutedTrigger {
        match event {
            CommitEvent::Unsupported { reason } => {
                self.external.push(ExternalReceipt::CommitUnsupported {
                    reason: reason.clone(),
                });
                RoutedTrigger::Unsupported {
                    capability: "vcs-commit",
                    reason: reason.clone(),
                }
            }
            CommitEvent::ProviderFailed { detail } => {
                self.external.push(ExternalReceipt::CommitProviderFailed {
                    detail: detail.clone(),
                });
                RoutedTrigger::Inert {
                    reason: InertReason::ExternalFailed,
                }
            }
            CommitEvent::PushFailed {
                commit_locator,
                detail,
            } => {
                self.external.push(ExternalReceipt::CommitPushFailed {
                    commit_locator: commit_locator.clone(),
                    detail: detail.clone(),
                });
                RoutedTrigger::Inert {
                    reason: InertReason::ExternalFailed,
                }
            }
            CommitEvent::Committed(receipt) => {
                let policy = resolve_effective_policy(declared, local, None);
                if !policy.trigger_enabled(RenderTrigger::OnCommit) {
                    return RoutedTrigger::Inert {
                        reason: InertReason::TriggerDisabled,
                    };
                }
                let purpose = JobPurpose::Scoped {
                    scope: policy.policy.scope,
                };
                let demands = self.demands.demands(&receipt.snapshot.project, &purpose);
                if demands.is_empty() {
                    return RoutedTrigger::Inert {
                        reason: InertReason::NoDemands,
                    };
                }
                RoutedTrigger::Job(JobIntent {
                    selection: RenderSelection {
                        snapshot: receipt.snapshot.clone(),
                        demands,
                    },
                    purpose,
                    policy,
                    triggers: vec![TriggerProvenance {
                        trigger: RenderTrigger::OnCommit,
                        detail: format!("commit {}", receipt.commit_locator),
                    }],
                    external_manifests: vec![receipt.commit_locator.clone()],
                    estimate: JobCostEstimate::default(),
                    saved_revision: None,
                    entry: report.entry,
                    action: report.action.clone(),
                    grant: report.grant.clone(),
                })
            }
        }
    }

    fn route_demand(
        &mut self,
        proposal: &DemandProposal,
        declared: Option<&PolicyPatch>,
        local: Option<&PolicyPatch>,
        report: &TriggerReport,
    ) -> RoutedTrigger {
        let policy = resolve_effective_policy(declared, local, proposal.policy_intent.as_ref());
        if !policy.trigger_enabled(RenderTrigger::OnDemand) {
            return RoutedTrigger::Inert {
                reason: InertReason::TriggerDisabled,
            };
        }
        let Some(authorizer) = self.authorizer else {
            return RoutedTrigger::Denied {
                reason: "no demand authorizer configured".to_owned(),
            };
        };
        match authorizer.authorize(proposal) {
            DemandAuthorization::Granted { grant } => RoutedTrigger::Job(JobIntent {
                selection: proposal.selection.clone(),
                purpose: proposal.purpose.clone(),
                policy,
                triggers: vec![TriggerProvenance {
                    trigger: RenderTrigger::OnDemand,
                    detail: format!("proposal {}", proposal.proposal_id),
                }],
                external_manifests: Vec::new(),
                estimate: JobCostEstimate::default(),
                saved_revision: None,
                entry: report.entry,
                action: report.action.clone(),
                grant: Some(grant),
            }),
            DemandAuthorization::Denied { reason } => RoutedTrigger::Denied { reason },
        }
    }

    // ------------------------------------------------------------------
    // submission — resolve, key, coalesce, preflight
    // ------------------------------------------------------------------

    /// Submits a routed job intent: resolves recipes against the
    /// pinned snapshot, computes the idempotent key, coalesces or
    /// preflights, and records the job.
    pub fn submit(&mut self, intent: &JobIntent) -> SubmitOutcome {
        // 1. Resolve the pinned project view.
        let resolved_project =
            match resolve_render_project(&intent.selection.snapshot.project, None) {
                Ok(r) => r,
                Err(e) => {
                    return self.reject(
                        intent,
                        JobFailureReason::UnknownDependency,
                        format!("render input resolution failed: {e}"),
                    );
                }
            };

        // 2. Resolve quality settings — fail closed when the
        //    referenced document is unavailable.
        let Some(quality) = self.settings.resolve(&intent.policy.policy.quality_profile) else {
            return self.reject(
                intent,
                JobFailureReason::UnknownDependency,
                "quality profile reference unresolvable".to_owned(),
            );
        };

        // 3. Per-demand recipe resolution.
        let (boundaries, mut recipe_keys) =
            self.resolve_boundaries(intent, &quality, &resolved_project);
        recipe_keys.sort();
        recipe_keys.dedup();

        // 4. Idempotent key.
        let key = RenderJobKey {
            snapshot_locator: intent.selection.snapshot.locator_key(),
            snapshot_digest: intent.selection.snapshot.identity.digest,
            recipe_keys,
            purpose: intent.purpose.clone(),
            policy_resolution: intent.policy.resolution_digest,
            action: intent.action.0.clone(),
            grant: intent.grant.as_ref().map(|g| g.0.clone()),
        };

        // 5. Coalesce — merge trigger/pin provenance and record the
        //    duplicate intent.
        if self.jobs.contains_key(&key) {
            return self.coalesce(&key, intent);
        }

        // 6. Pre-issuance validation — before the job exists.
        if let Some((reason, detail)) = self.preflight_reject(intent) {
            return self.reject(intent, reason, detail);
        }

        // 7. Create the record.
        let pending = self.pending_gate(intent);
        let record = JobRecord {
            key: key.clone(),
            state: RenderJobState::Resolve,
            attempt: 1,
            pending_reason: pending.as_ref().map(|(r, _)| *r),
            cancel: CancellationToken::new(),
            cancel_requested: false,
            crashed: false,
            policy: intent.policy.clone(),
            renderer: quality.renderer.clone(),
            selection: intent.selection.clone(),
            boundaries,
            exec: Vec::new(),
            artifacts: BTreeSet::new(),
            external_manifests: intent.external_manifests.clone(),
            triggers: intent.triggers.clone(),
            entry: intent.entry,
            action: intent.action.clone(),
            grant: intent.grant.clone(),
            saved_revision: intent.saved_revision,
            estimate: intent.estimate,
            failure: None,
            completeness: None,
            created_tick: self.tick,
            updated_tick: self.tick,
        };
        self.jobs.insert(key.clone(), record);

        self.push_receipt(
            &key,
            ReceiptKind::Submitted {
                entry: intent.entry,
                triggers: intent.triggers.clone(),
                target_locator: intent.selection.snapshot.locator_key(),
                snapshot_digest: intent.selection.snapshot.identity.digest,
                saved_revision: intent.saved_revision,
            },
        );
        self.transition(&key, RenderJobState::Queue);
        if let Some((reason, detail)) = pending {
            self.push_receipt(&key, ReceiptKind::Pending { reason, detail });
        }

        SubmitOutcome::Accepted {
            key,
            coalesced: false,
        }
    }

    /// Resolves every demand of `intent` against the pinned snapshot
    /// into a [`BoundaryResolution`] plus the sorted-deduped recipe
    /// keys feeding [`RenderJobKey`]. Unknown/Unsupported outcomes
    /// are preserved as typed boundaries — they become completeness
    /// gaps at verify, never renders.
    fn resolve_boundaries(
        &self,
        intent: &JobIntent,
        quality: &RenderQualityInputs,
        resolved_project: &ResolvedRenderProject,
    ) -> (Vec<BoundaryResolution>, Vec<RecipeKey>) {
        let environment = RecipeEnvironment {
            plugin_states: self.plugin_states,
            external_assets: self.external_assets,
        };
        let mut boundaries = Vec::with_capacity(intent.selection.demands.len());
        let mut recipe_keys = Vec::new();
        for (demand_idx, demand) in intent.selection.demands.iter().enumerate() {
            let temporal = TemporalDomain {
                sample_rate_hz: quality.sample_rate_hz,
                channels: quality.channels,
                format: quality.format,
                interval: demand.interval,
                preroll_frames: quality.temporal.preroll_frames,
                warmup_frames: quality.temporal.warmup_frames,
                tail_frames: quality.temporal.tail_frames,
                lookahead_frames: quality.temporal.lookahead_frames,
                pdc_frames: quality.temporal.pdc_frames,
                clock: quality.temporal.clock.clone(),
                loop_region: quality.temporal.loop_region,
                seek_boundaries: quality.temporal.seek_boundaries.clone(),
            };
            let output = OutputContract {
                ports: demand
                    .boundary
                    .ports
                    .iter()
                    .map(|port| OutputPortContract {
                        port: *port,
                        channels: quality.channels,
                        format: quality.format,
                    })
                    .collect(),
            };
            let branch_pin =
                resolved_project
                    .branch
                    .as_ref()
                    .map(|b| crate::recipe::RecipeBranchPin {
                        scope_digest: b.scope_digest,
                        shared_context_digest: b.shared_context_digest,
                    });
            let mut provenance = ResolutionProvenance {
                snapshot: Some(intent.selection.snapshot.identity.clone()),
                ..ResolutionProvenance::default()
            };
            if let Some(b) = resolved_project.branch.as_ref() {
                provenance.variant_set = Some(b.variant_set);
                provenance.branch = Some(b.branch);
                provenance.branch_revision = Some(b.branch_revision.0);
                provenance.payload_digest = b.payload_digest;
            }
            let request = RecipeRequest {
                project: &resolved_project.project,
                boundary: demand.boundary.clone(),
                temporal,
                renderer: quality.renderer.clone(),
                output,
                environment: &environment,
                branch: branch_pin,
                provenance,
                project_schema_version: SCHEMA_VERSION,
                render_input_schema_version: RENDER_INPUT_SCHEMA_VERSION,
            };
            let resolution = resolve_recipe(&request);
            if let RecipeResolution::Resolved { key, .. } = &resolution {
                recipe_keys.push(*key);
            }
            boundaries.push(match resolution {
                r @ RecipeResolution::Resolved { .. } => BoundaryResolution::Resolved {
                    demand: demand_idx,
                    resolution: Box::new(r),
                },
                RecipeResolution::Unknown { dependencies } => BoundaryResolution::Unknown {
                    demand: demand_idx,
                    dependencies,
                },
                RecipeResolution::Unsupported { reason } => BoundaryResolution::Unsupported {
                    demand: demand_idx,
                    reason,
                },
            });
        }
        (boundaries, recipe_keys)
    }

    /// Merges a duplicate intent into the existing job — trigger and
    /// manifest provenance are unioned (never duplicated), the first
    /// saved-revision pin wins, and the coalesced event is recorded
    /// on the receipt stream. Terminal jobs are an idempotent no-op
    /// for *work* (computed artifacts are reused through lookup,
    /// never re-published) but the intent stays receipt-visible so
    /// no trigger provenance is lost (RCP-POL-01).
    fn coalesce(&mut self, key: &RenderJobKey, intent: &JobIntent) -> SubmitOutcome {
        {
            let record = self.jobs.get_mut(key).expect("job present");
            for t in &intent.triggers {
                if !record.triggers.contains(t) {
                    record.triggers.push(t.clone());
                }
            }
            for m in &intent.external_manifests {
                if !record.external_manifests.contains(m) {
                    record.external_manifests.push(m.clone());
                }
            }
            if record.saved_revision.is_none() {
                record.saved_revision = intent.saved_revision;
            }
        }
        self.push_receipt(
            key,
            ReceiptKind::Coalesced {
                entry: intent.entry,
                triggers: intent.triggers.clone(),
            },
        );
        SubmitOutcome::Accepted {
            key: key.clone(),
            coalesced: true,
        }
    }

    /// Hard pre-issuance rejections: budget caps and grant/consent
    /// denials. Environmental blockers (quota/storage) are *pending*,
    /// not rejections — handled by [`Self::pending_gate`].
    fn preflight_reject(&self, intent: &JobIntent) -> Option<(JobFailureReason, String)> {
        let budget = &intent.policy.policy.resource_budget;
        let e = &intent.estimate;
        if e.cpu_ms > budget.max_cpu_ms
            || e.wall_ms > budget.max_wall_ms
            || e.storage_bytes > budget.max_storage_bytes
        {
            return Some((
                JobFailureReason::BudgetExceeded,
                format!(
                    "estimate cpu={}ms wall={}ms storage={}B exceeds budget cpu={}ms wall={}ms storage={}B",
                    e.cpu_ms,
                    e.wall_ms,
                    e.storage_bytes,
                    budget.max_cpu_ms,
                    budget.max_wall_ms,
                    budget.max_storage_bytes,
                ),
            ));
        }
        match self.executor.backend_requirement() {
            BackendRequirement::Cpu if !self.grants.cpu_render => Some((
                JobFailureReason::AuthDenied,
                "local CPU render not granted".to_owned(),
            )),
            BackendRequirement::Gpu if !self.grants.gpu_render => Some((
                JobFailureReason::AuthDenied,
                "local GPU render not granted".to_owned(),
            )),
            BackendRequirement::Remote { backend_id } => {
                if !self.grants.network || !self.grants.remote_backends.contains(&backend_id) {
                    Some((
                        JobFailureReason::RemoteConsentDenied,
                        format!("remote backend {backend_id} not consented"),
                    ))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Environmental pending gate evaluated for a *new* intent —
    /// concurrency quota and local storage headroom. Returns
    /// `(reason, detail)` when the job must wait.
    fn pending_gate(&self, intent: &JobIntent) -> Option<(JobFailureReason, String)> {
        let active = self
            .jobs
            .values()
            .filter(|j| !j.state.is_terminal() && j.pending_reason.is_none())
            .count();
        let cap = intent.policy.policy.resource_budget.max_concurrent_jobs;
        if active >= usize::try_from(cap).unwrap_or(usize::MAX) {
            return Some((
                JobFailureReason::QuotaExceeded,
                format!("{active} jobs active, cap {cap}"),
            ));
        }
        if intent.estimate.storage_bytes > self.grants.storage_quota_bytes {
            return Some((
                JobFailureReason::InsufficientStorage,
                format!(
                    "estimate {}B exceeds storage quota {}B",
                    intent.estimate.storage_bytes, self.grants.storage_quota_bytes
                ),
            ));
        }
        None
    }

    /// Same gate re-evaluated for an existing queued job.
    fn pending_gate_for(&self, key: &RenderJobKey) -> Option<(JobFailureReason, String)> {
        let job = self.jobs.get(key)?;
        let active = self
            .jobs
            .values()
            .filter(|j| !j.state.is_terminal() && j.pending_reason.is_none() && j.key != *key)
            .count();
        let cap = job.policy.policy.resource_budget.max_concurrent_jobs;
        if active >= usize::try_from(cap).unwrap_or(usize::MAX) {
            return Some((
                JobFailureReason::QuotaExceeded,
                format!("{active} jobs active, cap {cap}"),
            ));
        }
        if job.estimate.storage_bytes > self.grants.storage_quota_bytes {
            return Some((
                JobFailureReason::InsufficientStorage,
                format!(
                    "estimate {}B exceeds storage quota {}B",
                    job.estimate.storage_bytes, self.grants.storage_quota_bytes
                ),
            ));
        }
        None
    }

    fn reject(
        &mut self,
        intent: &JobIntent,
        reason: JobFailureReason,
        detail: String,
    ) -> SubmitOutcome {
        self.rejected.push(RejectedAttempt {
            detail: format!(
                "locator={} purpose={:?} policy={:?}",
                intent.selection.snapshot.locator_key(),
                intent.purpose,
                intent.policy.resolution_digest,
            ),
            reason,
            detail_msg: detail.clone(),
        });
        SubmitOutcome::Rejected { reason, detail }
    }

    // ------------------------------------------------------------------
    // driving the state machine
    // ------------------------------------------------------------------

    /// Drives one job toward quiescence: pending-gate recheck,
    /// prepare, render+publish each unresolved demand, verify
    /// completeness, terminate.
    pub fn drive(&mut self, key: &RenderJobKey) -> DriveOutcome {
        if !self.jobs.contains_key(key) {
            return DriveOutcome::Unknown;
        }

        // Environmental gate recheck for queued jobs — grants and
        // sibling completions may have changed since submit.
        let pending_reason = self
            .jobs
            .get(key)
            .filter(|j| j.state == RenderJobState::Queue)
            .and_then(|j| j.pending_reason);
        if let Some(reason) = pending_reason {
            if reason.gate_managed() {
                if let Some((r, d)) = self.pending_gate_for(key) {
                    if r != reason {
                        if let Some(j) = self.jobs.get_mut(key) {
                            j.pending_reason = Some(r);
                        }
                        self.push_receipt(
                            key,
                            ReceiptKind::Pending {
                                reason: r,
                                detail: d,
                            },
                        );
                    }
                    return DriveOutcome::Pending;
                }
                if let Some(j) = self.jobs.get_mut(key) {
                    j.pending_reason = None;
                }
                self.push_receipt(key, ReceiptKind::Resumed);
            } else {
                return DriveOutcome::Pending;
            }
        }

        // Cancel requested while queued → Cancelled without work.
        if self
            .jobs
            .get(key)
            .is_some_and(|j| j.cancel_requested || j.cancel.is_cancelled())
        {
            return self.terminate(key, RenderJobState::Cancelled, None);
        }

        match self.jobs.get(key).map(|j| j.state) {
            Some(RenderJobState::Queue) => {
                if self
                    .jobs
                    .get(key)
                    .is_some_and(|j| j.pending_reason.is_some())
                {
                    return DriveOutcome::Pending;
                }
                self.transition(key, RenderJobState::Preparing);
                self.transition(key, RenderJobState::Rendering);
                self.drive_render(key)
            }
            Some(RenderJobState::Rendering) => self.drive_render(key),
            Some(RenderJobState::Verifying) => self.drive_verify(key),
            _ => DriveOutcome::Terminal,
        }
    }

    /// Drives every runnable job to quiescence — the headless
    /// equivalent of an async scheduler tick.
    pub fn run(&mut self) -> Vec<(RenderJobKey, DriveOutcome)> {
        let keys = self.job_keys();
        keys.into_iter()
            .map(|k| {
                let out = self.drive(&k);
                (k, out)
            })
            .collect()
    }

    /// Rendering stage: per resolved demand — reuse a verified
    /// artifact via lookup, else executor render + verified publish.
    fn drive_render(&mut self, key: &RenderJobKey) -> DriveOutcome {
        let resolutions: Vec<BoundaryResolution> = self
            .jobs
            .get(key)
            .map(|j| j.boundaries.clone())
            .unwrap_or_default();
        let profile = self.jobs.get(key).map(|j| j.renderer.clone());
        let Some(profile) = profile else {
            return DriveOutcome::Unknown;
        };
        let cancel = self
            .jobs
            .get(key)
            .map(|j| j.cancel.clone())
            .unwrap_or_default();

        for bres in &resolutions {
            let (demand_idx, recipe_key, recipe) = match bres {
                BoundaryResolution::Resolved { demand, resolution } => match resolution.as_ref() {
                    RecipeResolution::Resolved {
                        key: k,
                        recipe: rec,
                        ..
                    } => (*demand, *k, rec.as_ref()),
                    _ => unreachable!("Resolved variant holds Resolved resolution"),
                },
                // Unknown/Unsupported boundaries become completeness
                // gaps at verify — no render work is lawful.
                _ => continue,
            };

            // Already executed (resume path)?
            if self.jobs.get(key).is_some_and(|j| {
                j.exec.iter().any(
                    |e| matches!(e, BoundaryExec::Done { demand, .. } if *demand == demand_idx),
                )
            }) {
                continue;
            }

            // Cancel boundary check.
            if self
                .jobs
                .get(key)
                .is_some_and(|j| j.cancel_requested || j.cancel.is_cancelled())
            {
                return self.terminate(key, RenderJobState::Cancelled, None);
            }

            // Reuse: an Exact-eligible verified artifact already
            // satisfies this recipe — never re-render or re-publish.
            if let Some(artifact) = self.find_reusable(recipe_key, recipe, &profile) {
                self.record_done(key, demand_idx, recipe_key, artifact, true);
                continue;
            }

            if let Some(outcome) =
                self.drive_demand(key, demand_idx, recipe_key, recipe, &profile, &cancel)
            {
                return outcome;
            }
        }

        // All demands processed → verify completeness.
        self.transition(key, RenderJobState::Verifying);
        self.drive_verify(key)
    }

    /// Executes one resolved demand through the injected executor and
    /// runs the verified publish. Returns `Some(outcome)` when the
    /// job must stop (pending/cancel/crash/failure), `None` to
    /// continue with the next demand.
    fn drive_demand(
        &mut self,
        key: &RenderJobKey,
        demand_idx: usize,
        recipe_key: RecipeKey,
        recipe: &RenderRecipeV1,
        profile: &RendererProfile,
        cancel: &CancellationToken,
    ) -> Option<DriveOutcome> {
        let demand = self
            .jobs
            .get(key)
            .and_then(|j| j.selection.demands.get(demand_idx).cloned())?;
        let snapshot = self.jobs.get(key).map(|j| j.selection.snapshot.clone())?;
        let ctx = JobContext {
            snapshot: &snapshot,
            recipe,
            demand: &demand,
        };
        let execution = self.executor.render(&ctx, recipe, cancel);

        match execution {
            RenderExecution::Output(output) => {
                match self.publish_rendered(key, recipe_key, recipe, *output, cancel, profile) {
                    PublishStep::Published(artifact) => {
                        self.record_done(key, demand_idx, recipe_key, artifact, false);
                        None
                    }
                    PublishStep::Failed(failure) => {
                        Some(self.fail_demand(key, demand_idx, Some(recipe_key), failure))
                    }
                    PublishStep::Cancelled => {
                        Some(self.terminate(key, RenderJobState::Cancelled, None))
                    }
                }
            }
            RenderExecution::Cancelled => {
                Some(self.terminate(key, RenderJobState::Cancelled, None))
            }
            RenderExecution::Crashed => {
                if let Some(j) = self.jobs.get_mut(key) {
                    j.crashed = true;
                }
                self.push_receipt(key, ReceiptKind::CrashObserved);
                Some(DriveOutcome::Crashed)
            }
            RenderExecution::Failed { reason, detail } => {
                let failure = JobFailure { reason, detail };
                Some(self.fail_demand(key, demand_idx, Some(recipe_key), failure))
            }
        }
    }

    /// Records a boundary failure and maps it to the job outcome:
    /// retryable reasons pend the job (resumable via `retry` or the
    /// environmental gate recheck), permanent reasons terminate it
    /// `Failed`.
    fn fail_demand(
        &mut self,
        key: &RenderJobKey,
        demand_idx: usize,
        recipe_key: Option<RecipeKey>,
        failure: JobFailure,
    ) -> DriveOutcome {
        self.record_failed(key, demand_idx, recipe_key, failure.clone());
        if failure.retryable() {
            if let Some(j) = self.jobs.get_mut(key) {
                j.pending_reason = Some(failure.reason);
            }
            self.transition(key, RenderJobState::Queue);
            self.push_receipt(
                key,
                ReceiptKind::Pending {
                    reason: failure.reason,
                    detail: failure.detail.clone(),
                },
            );
            return DriveOutcome::Pending;
        }
        self.terminate_with_failure(key, failure)
    }

    /// Verifying stage: completeness evaluation → Published or
    /// Partial. Gaps are typed, never hidden.
    fn drive_verify(&mut self, key: &RenderJobKey) -> DriveOutcome {
        let report = self.completeness_report(key);
        let has_gaps = report.verdict == CompletenessVerdict::Incomplete;
        if let Some(j) = self.jobs.get_mut(key) {
            j.completeness = Some(report);
        }
        let state = if has_gaps {
            RenderJobState::Partial
        } else {
            RenderJobState::Published
        };
        self.terminate(key, state, None);
        DriveOutcome::Terminal
    }

    // ------------------------------------------------------------------
    // publish + completeness + share gate
    // ------------------------------------------------------------------

    /// Builds the publish draft from the resolved recipe (the
    /// orchestrator owns recipe/target/snapshot/temporal/renderer/
    /// provenance fields) plus the executor's declared media facts,
    /// then runs the RCP-02 verified publish.
    fn publish_rendered(
        &mut self,
        key: &RenderJobKey,
        recipe_key: RecipeKey,
        recipe: &RenderRecipeV1,
        output: RenderedOutput,
        cancel: &CancellationToken,
        profile: &RendererProfile,
    ) -> PublishStep {
        let RenderedOutput {
            blob,
            codec,
            expect,
            expect_pcm,
            cut_frames,
            latency_frames,
            reproducibility,
            trust,
        } = output;

        // Per-port channel offsets tile the decoded stream.
        let mut offset = 0u32;
        let ports: Vec<ArtifactPortSpec> = recipe
            .output_contract
            .ports
            .iter()
            .map(|p| {
                let spec = ArtifactPortSpec {
                    port: p.port,
                    channels: p.channels,
                    channel_offset: offset,
                    format: p.format,
                };
                offset += p.channels;
                spec
            })
            .collect();

        let snapshot = self
            .jobs
            .get(key)
            .map(|j| j.selection.snapshot.identity.clone());

        let draft = ArtifactDraft {
            recipe_key,
            target: recipe.target.clone(),
            snapshot,
            temporal: recipe.temporal.clone(),
            cut_frames,
            latency_frames,
            ports,
            codec,
            renderer: profile.clone(),
            reproducibility,
            provenance: ArtifactProvenance {
                produced_by: "synvelis-render/render-job".to_owned(),
                job_id: Some(format!("{:?}", key.purpose)),
            },
            trust,
        };

        match publish_artifact(
            self.store,
            &draft,
            &blob,
            &expect,
            expect_pcm.as_deref(),
            Some(cancel),
            None,
        ) {
            PublishOutcome::Published { artifact, .. } => PublishStep::Published(artifact),
            PublishOutcome::Cancelled { .. } => PublishStep::Cancelled,
            PublishOutcome::Failed { error, .. } => PublishStep::Failed(map_publish_error(error)),
        }
    }

    /// Exact-eligible artifact reuse — returns the artifact id when a
    /// verified artifact already satisfies this recipe for the
    /// pinned span/ports/profile.
    fn find_reusable(
        &self,
        recipe_key: RecipeKey,
        recipe: &RenderRecipeV1,
        profile: &RendererProfile,
    ) -> Option<RenderArtifactId> {
        let query = CacheLookup {
            recipe_key,
            intended_use: IntendedUse::Exact,
            requested_span: recipe.temporal.interval,
            required_ports: recipe.output_contract.ports.clone(),
            trust_scope: TrustScope::Local,
            compatibility_profile: profile.compatibility_profile,
        };
        let current = RecipeResolution::Resolved {
            recipe: Box::new(recipe.clone()),
            key: recipe_key,
            provenance: Box::new(ResolutionProvenance::default()),
        };
        let result = lookup(self.store, &query, &current).ok()?;
        match result.outcome {
            LookupOutcome::EligibleExact { artifact } => Some(artifact),
            _ => None,
        }
    }

    /// Completeness evaluation over the job's resolved boundaries —
    /// independent of job state; gaps are typed.
    fn completeness_report(&self, key: &RenderJobKey) -> CompletenessReport {
        let mut gaps = Vec::new();
        let boundaries = self
            .jobs
            .get(key)
            .map(|j| j.boundaries.clone())
            .unwrap_or_default();
        let compatibility_profile = self.jobs.get(key).map_or_else(
            || ContentDigest::sha256(b"missing-renderer"),
            |j| j.renderer.compatibility_profile,
        );
        for bres in &boundaries {
            match bres {
                BoundaryResolution::Resolved { resolution, .. } => {
                    let RecipeResolution::Resolved {
                        key: recipe_key,
                        recipe,
                        ..
                    } = resolution.as_ref()
                    else {
                        continue;
                    };
                    let query = CacheLookup {
                        recipe_key: *recipe_key,
                        intended_use: IntendedUse::Exact,
                        requested_span: recipe.temporal.interval,
                        required_ports: recipe.output_contract.ports.clone(),
                        trust_scope: TrustScope::Local,
                        compatibility_profile,
                    };
                    match lookup(self.store, &query, resolution) {
                        Ok(result) => {
                            if !matches!(result.outcome, LookupOutcome::EligibleExact { .. }) {
                                gaps.push(CompletenessGap {
                                    recipe_key: Some(*recipe_key),
                                    boundary: recipe.target.boundary.clone(),
                                    kind: gap_kind_of(&result.outcome),
                                    detail: format!("{:?}", result.outcome),
                                });
                            }
                        }
                        Err(e) => gaps.push(CompletenessGap {
                            recipe_key: Some(*recipe_key),
                            boundary: recipe.target.boundary.clone(),
                            kind: CompletenessGapKind::UnverifiedArtifact,
                            detail: format!("lookup failed: {e}"),
                        }),
                    }
                }
                BoundaryResolution::Unknown {
                    demand,
                    dependencies,
                } => {
                    let boundary = self
                        .jobs
                        .get(key)
                        .and_then(|j| j.selection.demands.get(*demand))
                        .map_or_else(|| RenderBoundary::new(Vec::new()), |d| d.boundary.clone());
                    for dep in dependencies {
                        gaps.push(CompletenessGap {
                            recipe_key: None,
                            boundary: boundary.clone(),
                            kind: gap_kind_of_unknown(&dep.cause),
                            detail: format!("{:?}", dep.cause),
                        });
                    }
                }
                BoundaryResolution::Unsupported { demand, reason } => {
                    let boundary = self
                        .jobs
                        .get(key)
                        .and_then(|j| j.selection.demands.get(*demand))
                        .map_or_else(|| RenderBoundary::new(Vec::new()), |d| d.boundary.clone());
                    gaps.push(CompletenessGap {
                        recipe_key: None,
                        boundary,
                        kind: CompletenessGapKind::UnsupportedCapability,
                        detail: format!("{reason:?}"),
                    });
                }
            }
        }
        CompletenessReport {
            verdict: if gaps.is_empty() {
                CompletenessVerdict::Complete
            } else {
                CompletenessVerdict::Incomplete
            },
            gaps,
        }
    }

    /// Share gate: submit the fill job (idempotent), drive it, then
    /// apply the completion policy to the completeness verdict.
    fn inspect_share(
        &mut self,
        request: &ShareRequest,
        entry: EntrySurface,
        action: GatewayActionRef,
        grant: Option<GrantRef>,
    ) -> TriggerOutcome {
        let project_id = request.snapshot.identity.project_id;
        let declared = self.policies.declared(project_id);
        let local = self.policies.local_override();
        let policy = resolve_effective_policy(
            declared.as_ref(),
            local.as_ref(),
            request.policy_intent.as_ref(),
        );
        let completion = policy.policy.completion;
        let intent = JobIntent {
            selection: RenderSelection {
                snapshot: request.snapshot.clone(),
                demands: request.demands.clone(),
            },
            purpose: JobPurpose::Share {
                package_id: request.package_id.clone(),
            },
            policy,
            triggers: vec![TriggerProvenance {
                trigger: RenderTrigger::OnShare,
                detail: format!("share {}", request.package_id),
            }],
            external_manifests: Vec::new(),
            estimate: JobCostEstimate::default(),
            saved_revision: None,
            entry,
            action,
            grant,
        };
        let job_key = match self.submit(&intent) {
            SubmitOutcome::Accepted { key, .. } => key,
            SubmitOutcome::Rejected { reason, detail } => {
                self.external.push(ExternalReceipt::ShareGate {
                    package_id: request.package_id.clone(),
                    verdict: format!("rejected {reason:?}: {detail}"),
                });
                return TriggerOutcome::Denied {
                    reason: format!("share job rejected: {reason:?}: {detail}"),
                };
            }
        };
        let _ = self.drive(&job_key);

        // Pending work makes completeness undecidable — the gate
        // blocks with the in-flight job named for retry.
        if self
            .jobs
            .get(&job_key)
            .is_some_and(|j| !j.state.is_terminal())
        {
            let gaps = self.completeness_report(&job_key).gaps;
            let verdict = ShareGateVerdict::Blocked {
                gaps,
                pending_jobs: vec![job_key.clone()],
            };
            self.external.push(ExternalReceipt::ShareGate {
                package_id: request.package_id.clone(),
                verdict: verdict_name(&verdict).to_owned(),
            });
            return TriggerOutcome::ShareGate {
                job: Some(job_key),
                verdict,
            };
        }

        let report = self.completeness_report(&job_key);
        let artifacts = self
            .jobs
            .get(&job_key)
            .map(|j| j.artifacts.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default();

        let verdict = match (report.verdict, completion) {
            (CompletenessVerdict::Complete, _) => ShareGateVerdict::Allowed { artifacts },
            (CompletenessVerdict::Incomplete, CompletionRequirement::BestEffort) => {
                ShareGateVerdict::AllowedPartial {
                    gaps: report.gaps,
                    artifacts,
                }
            }
            (CompletenessVerdict::Incomplete, CompletionRequirement::RequireComplete) => {
                ShareGateVerdict::Blocked {
                    gaps: report.gaps,
                    pending_jobs: Vec::new(),
                }
            }
        };
        self.external.push(ExternalReceipt::ShareGate {
            package_id: request.package_id.clone(),
            verdict: verdict_name(&verdict).to_owned(),
        });
        TriggerOutcome::ShareGate {
            job: Some(job_key),
            verdict,
        }
    }

    // ------------------------------------------------------------------
    // cancel / retry / resume
    // ------------------------------------------------------------------

    /// Cooperative cancel — takes effect at the next stage boundary
    /// or via the executor's token. Terminal jobs are unaffected.
    pub fn cancel(&mut self, key: &RenderJobKey) -> bool {
        let queued = {
            let Some(job) = self.jobs.get_mut(key) else {
                return false;
            };
            if job.state.is_terminal() {
                return false;
            }
            job.cancel_requested = true;
            job.cancel.cancel();
            job.state == RenderJobState::Queue
        };
        self.push_receipt(key, ReceiptKind::CancelRequested);
        // Queued jobs cancel immediately — no work was issued.
        if queued {
            let _ = self.terminate(key, RenderJobState::Cancelled, None);
        }
        true
    }

    /// Idempotent retry of a failed or gate-pending job — only
    /// retryable failure classes requeue; permanent failures stay
    /// terminal, and gate-managed pendings (quota/storage) resume
    /// only through `drive`'s headroom recheck, never by bypass.
    pub fn retry(&mut self, key: &RenderJobKey) -> bool {
        let retryable = self.jobs.get(key).is_some_and(|j| {
            j.failure.as_ref().is_some_and(JobFailure::retryable)
                || j.pending_reason
                    .is_some_and(|r| r.retryable() && !r.gate_managed())
        });
        if !retryable {
            return false;
        }
        let attempt = {
            let Some(job) = self.jobs.get_mut(key) else {
                return false;
            };
            job.attempt += 1;
            job.state = RenderJobState::Queue;
            job.pending_reason = None;
            job.failure = None;
            job.completeness = None;
            job.crashed = false;
            job.cancel_requested = false;
            job.cancel = CancellationToken::new();
            job.attempt
        };
        self.push_receipt(key, ReceiptKind::Retrying { attempt });
        self.transition(key, RenderJobState::Queue);
        true
    }

    /// Resumes a non-terminal job — crashed renders re-drive from
    /// `Rendering`; lookup reuse makes re-entry idempotent.
    pub fn resume(&mut self, key: &RenderJobKey) -> DriveOutcome {
        if self.jobs.get(key).is_some_and(|j| j.crashed) {
            self.push_receipt(key, ReceiptKind::Resumed);
            if let Some(j) = self.jobs.get_mut(key) {
                j.crashed = false;
            }
        }
        self.drive(key)
    }

    // ------------------------------------------------------------------
    // internals
    // ------------------------------------------------------------------

    fn push_receipt(&mut self, key: &RenderJobKey, kind: ReceiptKind) {
        let seq = self.receipts.iter().filter(|r| &r.job == key).count() as u64;
        self.receipts.push(JobReceipt {
            job: key.clone(),
            seq,
            tick: self.tick,
            kind,
        });
    }

    fn transition(&mut self, key: &RenderJobKey, to: RenderJobState) {
        let from = self.jobs.get(key).map_or(to, |j| j.state);
        if let Some(j) = self.jobs.get_mut(key) {
            j.state = to;
            j.updated_tick = self.tick;
        }
        self.push_receipt(key, ReceiptKind::Transition { from, to });
    }

    fn terminate(
        &mut self,
        key: &RenderJobKey,
        state: RenderJobState,
        verdict: Option<CompletenessVerdict>,
    ) -> DriveOutcome {
        let artifacts: Vec<RenderArtifactId> = self
            .jobs
            .get(key)
            .map(|j| j.artifacts.iter().copied().collect())
            .unwrap_or_default();
        let failure = self.jobs.get(key).and_then(|j| j.failure.clone());
        let completeness = verdict.or_else(|| {
            self.jobs
                .get(key)
                .and_then(|j| j.completeness.as_ref().map(|r| r.verdict))
        });
        let from = self.jobs.get(key).map(|j| j.state);
        if let Some(j) = self.jobs.get_mut(key) {
            j.state = state;
            j.updated_tick = self.tick;
        }
        if let Some(from) = from.filter(|f| *f != state) {
            self.push_receipt(key, ReceiptKind::Transition { from, to: state });
        }
        self.push_receipt(
            key,
            ReceiptKind::Terminal {
                state,
                artifacts,
                completeness,
                failure,
            },
        );
        // Post-publish external manifest attach — the commit locator
        // stays immutable; only the registry entry is added
        // (RCP-POL-04).
        self.attach_external_manifests(key);
        DriveOutcome::Terminal
    }

    fn terminate_with_failure(&mut self, key: &RenderJobKey, failure: JobFailure) -> DriveOutcome {
        if let Some(j) = self.jobs.get_mut(key) {
            j.failure = Some(failure);
        }
        self.terminate(key, RenderJobState::Failed, None)
    }

    /// Attaches every published artifact to each recorded external
    /// manifest locator — attach failures are recorded evidence,
    /// never fatal to the render job's own receipt and never a
    /// commit rewrite.
    fn attach_external_manifests(&mut self, key: &RenderJobKey) {
        let (locators, snapshot_digest, artifacts) = {
            let Some(j) = self.jobs.get(key) else {
                return;
            };
            (
                j.external_manifests.clone(),
                j.selection.snapshot.identity.digest,
                j.artifacts.iter().copied().collect::<Vec<_>>(),
            )
        };
        for locator in locators {
            for artifact in &artifacts {
                let reference = ExternalManifestRef {
                    commit_locator: locator.clone(),
                    snapshot_digest,
                    artifact: *artifact,
                };
                if let Err(e) = attach_external_manifest(self.store, &reference) {
                    self.external
                        .push(ExternalReceipt::ExternalManifestAttachFailed {
                            commit_locator: locator.clone(),
                            artifact: *artifact,
                            detail: e.to_string(),
                        });
                }
            }
        }
    }

    fn record_done(
        &mut self,
        key: &RenderJobKey,
        demand: usize,
        recipe_key: RecipeKey,
        artifact: RenderArtifactId,
        reused: bool,
    ) {
        if let Some(j) = self.jobs.get_mut(key) {
            j.exec.push(BoundaryExec::Done {
                demand,
                key: recipe_key,
                artifact,
                reused,
            });
            j.artifacts.insert(artifact);
            j.updated_tick = self.tick;
        }
        self.push_receipt(
            key,
            ReceiptKind::ArtifactReady {
                demand,
                recipe_key,
                artifact,
                reused,
            },
        );
    }

    fn record_failed(
        &mut self,
        key: &RenderJobKey,
        demand: usize,
        recipe_key: Option<RecipeKey>,
        failure: JobFailure,
    ) {
        if let Some(j) = self.jobs.get_mut(key) {
            j.exec.push(BoundaryExec::Failed {
                demand,
                key: recipe_key,
                failure: failure.clone(),
            });
        }
        self.push_receipt(
            key,
            ReceiptKind::BoundaryFailed {
                demand,
                recipe_key,
                failure,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// free helpers
// ---------------------------------------------------------------------------

fn report_project_id(event: &TriggerEvent) -> Option<ProjectId> {
    match event {
        TriggerEvent::Manual { selection, .. } => Some(selection.snapshot.identity.project_id),
        TriggerEvent::Save(SaveEvent::Completed(r)) => Some(r.snapshot.identity.project_id),
        TriggerEvent::Save(SaveEvent::Failed { project_id, .. }) => Some(*project_id),
        TriggerEvent::Commit(CommitEvent::Committed(r)) => Some(r.snapshot.identity.project_id),
        TriggerEvent::Share(req) => Some(req.snapshot.identity.project_id),
        TriggerEvent::Demand(p) => Some(p.selection.snapshot.identity.project_id),
        _ => None,
    }
}

fn gap_kind_of(outcome: &LookupOutcome) -> CompletenessGapKind {
    match outcome {
        LookupOutcome::Missing => CompletenessGapKind::MissingRender,
        LookupOutcome::Corrupt => CompletenessGapKind::InvalidArtifact,
        LookupOutcome::Stale => CompletenessGapKind::StaleArtifact,
        LookupOutcome::Unverified => CompletenessGapKind::UnverifiedArtifact,
        LookupOutcome::Denied { .. } => CompletenessGapKind::Denied,
        LookupOutcome::Unknown { .. } => CompletenessGapKind::UnresolvedDependency,
        LookupOutcome::Unsupported { .. } => CompletenessGapKind::UnsupportedCapability,
        LookupOutcome::EligibleExact { .. } | LookupOutcome::EligiblePreview { .. } => {
            CompletenessGapKind::IncompleteContent
        }
    }
}

fn gap_kind_of_unknown(cause: &crate::recipe::UnknownCause) -> CompletenessGapKind {
    match cause {
        crate::recipe::UnknownCause::PluginStateUnknown { .. }
        | crate::recipe::UnknownCause::UnresolvedProcessor { .. }
        | crate::recipe::UnknownCause::PluginStateDegraded { .. } => {
            CompletenessGapKind::PluginMissing
        }
        _ => CompletenessGapKind::UnresolvedDependency,
    }
}

fn verdict_name(verdict: &ShareGateVerdict) -> &'static str {
    match verdict {
        ShareGateVerdict::Allowed { .. } => "allowed",
        ShareGateVerdict::AllowedPartial { .. } => "allowed-partial",
        ShareGateVerdict::Blocked { .. } => "blocked",
    }
}

fn map_publish_error(error: PublishError) -> JobFailure {
    match error {
        PublishError::Store(StoreError::DiskFull) => JobFailure {
            reason: JobFailureReason::InsufficientStorage,
            detail: "store disk full".to_owned(),
        },
        PublishError::Store(StoreError::Denied { reason }) => JobFailure {
            reason: JobFailureReason::AuthDenied,
            detail: reason,
        },
        PublishError::Store(StoreError::Cancelled) => JobFailure {
            reason: JobFailureReason::Cancelled,
            detail: "publish cancelled".to_owned(),
        },
        PublishError::Store(e) => JobFailure {
            reason: JobFailureReason::WorkerFailure,
            detail: e.to_string(),
        },
        PublishError::Verify { stage, detail } => JobFailure {
            reason: JobFailureReason::WorkerFailure,
            detail: format!("verify failed at {stage:?}: {detail}"),
        },
    }
}
