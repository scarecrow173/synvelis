//! Non-real-time asynchronous job scheduler (synvelis#518, AC-1).
//!
//! [`JobOrchestrator`] is a single-writer state machine — every job's
//! receipts, gates, and transitions live in one place. This module
//! wraps it in the production shape the headless slice deferred:
//! one dedicated worker thread (a [`ruviform_exec::OfflineJob`])
//! owns the orchestrator and advances jobs one [`DriveOutcome`]
//! quantum at a time, while callers submit typed commands through a
//! bounded [`std::sync::mpsc::sync_channel`]. Commands — trigger
//! reports, intent submits, cancel/retry/resume, grant updates,
//! queries — are drained between demand renders, so control latency
//! is bounded by a single demand rather than whole-job quiescence.
//! The worker never blocks a realtime lane: it is a plain OS thread
//! outside the RT pool, and a full command queue fails typed
//! ([`SchedulerError::QueueFull`]) instead of parking the caller.
//!
//! Executor primitive decision (recorded per the slice contract):
//! `OfflineJob` + consumer-side bounded queue — *not* the ruviform
//! `JobExecutor` (ruviform#744, still open). The orchestrator is the
//! single writer, so per-job executor primitives would duplicate
//! scheduling authority; the bounded channel *is* the admission
//! control pol06's concurrency quota complements.
//!
//! Fail-closed defaults: [`JobScheduler::headless`] wires
//! [`UnavailableExecutor`]/[`UnavailableSettings`]/
//! [`NoDeclaredPolicies`]/[`NoScopeDemands`] — every render reports a
//! typed permanent failure or typed rejection; no success is ever
//! fabricated. Real executor/settings/policy supply is owned by
//! later gates (#296 render execution, settings documents, project
//! declared-policy consumption).

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::time::{Duration, Instant};

use ruviform_exec::OfflineJob;
use serde::Serialize;
use synvelis_project::{Project, ProjectId, ProjectRevision};

use crate::artifact::manifest::RenderArtifactId;
use crate::artifact::store::{ArtifactStore, InMemoryArtifactStore};
use crate::dependency::{ExternalAssetProbe, PluginStateProbe};
use crate::policy::model::{LocalGrants, PolicyPatch, RenderSettingsRef};
use crate::policy::trigger::{
    DemandAuthorizer, JobIntent, PolicyResolver, RoutedTrigger, ScopeDemandResolver,
    TriggerOutcome, TriggerReport,
};
use crate::recipe::RenderRecipeV1;
use crate::render_job::cost::CostCalibration;
use crate::render_job::model::{
    CompletenessVerdict, JobCostEstimate, JobFailureReason, JobPurpose, JobReceipt, RenderDemand,
    RenderJobKey, RenderJobState,
};
use crate::render_job::orchestrator::{
    BackendRequirement, DriveOutcome, JobContext, JobOrchestrator, JobRecord, RejectedAttempt,
    RenderExecution, RenderExecutor, RenderQualityInputs, RenderSettingsProbe, SubmitOutcome,
};

/// Default command-queue depth for [`JobScheduler::headless`].
pub const DEFAULT_QUEUE_CAPACITY: usize = 64;

/// Injected ports the scheduler worker owns — every port is a
/// `Send` trait object so the bundle can move onto the worker
/// thread. `None` selects the orchestrator's fail-closed defaults
/// (unknown plugin states, no external pins) or this module's
/// explicit fail-closed impls where noted.
pub struct SchedulerPorts {
    /// Verified artifact store — the only publish target.
    pub store: Box<dyn ArtifactStore + Send>,
    /// Render backend — [`UnavailableExecutor`] when none is wired.
    pub executor: Box<dyn RenderExecutor + Send>,
    /// Quality-settings document probe.
    pub settings: Box<dyn RenderSettingsProbe + Send>,
    /// Declared + local policy layers.
    pub policies: Box<dyn PolicyResolver + Send>,
    /// Scope → render-demand mapping.
    pub demands: Box<dyn ScopeDemandResolver + Send>,
    /// Plugin-state evidence probe (`None` = all unknown).
    pub plugin_states: Option<Box<dyn PluginStateProbe + Send>>,
    /// External-asset pin probe (`None` = no pins).
    pub external_assets: Option<Box<dyn ExternalAssetProbe + Send>>,
    /// `OnDemand` consent boundary (`None` = every proposal denied).
    pub authorizer: Option<Box<dyn DemandAuthorizer + Send>>,
    /// Measured-cost calibration folded into the estimate floor.
    pub calibration: Option<Box<dyn CostCalibration + Send>>,
}

/// Serializeable job snapshot for status/inspect surfaces — the
/// adapter-facing view of [`JobRecord`]; receipt streams stay
/// available through the typed query commands.
#[derive(Clone, Debug, Serialize)]
pub struct JobView {
    /// Idempotent job identity.
    pub key: RenderJobKey,
    /// Current state.
    pub state: RenderJobState,
    /// Attempt counter.
    pub attempt: u32,
    /// Environmental blocker while queued.
    pub pending_reason: Option<JobFailureReason>,
    /// Cancel requested but not yet landed.
    pub cancel_requested: bool,
    /// Crash observed — resumable.
    pub crashed: bool,
    /// Merged cost estimate (model floor + caller declaration).
    pub estimate: JobCostEstimate,
    /// Saved revision pin, when any.
    pub saved_revision: Option<ProjectRevision>,
    /// Terminal/interim failure reason, typed.
    pub failure_reason_typed: Option<JobFailureReason>,
    /// Terminal/interim failure detail, when any.
    pub failure_detail: Option<String>,
    /// Completeness verdict, once evaluated.
    pub completeness_verdict: Option<CompletenessVerdict>,
    /// Typed completeness gap count.
    pub completeness_gap_count: usize,
    /// Published/reused artifact identities.
    pub artifacts: Vec<RenderArtifactId>,
    /// Scheduler tick at creation.
    pub created_tick: u64,
    /// Scheduler tick at last update.
    pub updated_tick: u64,
}

impl From<&JobRecord> for JobView {
    fn from(job: &JobRecord) -> Self {
        Self {
            key: job.key.clone(),
            state: job.state,
            attempt: job.attempt,
            pending_reason: job.pending_reason,
            cancel_requested: job.cancel_requested,
            crashed: job.crashed,
            estimate: job.estimate,
            saved_revision: job.saved_revision,
            failure_reason_typed: job.failure.as_ref().map(|f| f.reason),
            failure_detail: job.failure.as_ref().map(|f| f.detail.clone()),
            completeness_verdict: job.completeness.as_ref().map(|c| c.verdict),
            completeness_gap_count: job.completeness.as_ref().map_or(0, |c| c.gaps.len()),
            artifacts: job.artifacts.iter().copied().collect(),
            created_tick: job.created_tick,
            updated_tick: job.updated_tick,
        }
    }
}

/// Read-only routing preview for `synvelis.render.plan` — the
/// [`RoutedTrigger`] decision without submission side effects.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RoutePreview {
    /// A job intent would be submitted.
    Job {
        /// Purpose label.
        purpose: String,
        /// Demand count.
        demands: usize,
        /// Policy resolution digest.
        policy_digest: String,
    },
    /// Event is inert under effective policy.
    Inert {
        /// Typed reason.
        reason: String,
    },
    /// Capability unsupported.
    Unsupported {
        /// Capability name.
        capability: String,
        /// Why.
        reason: String,
    },
    /// Fail-closed denial.
    Denied {
        /// Why.
        reason: String,
    },
    /// Share request routes to the completeness gate.
    ShareGate,
}

impl From<RoutedTrigger> for RoutePreview {
    fn from(routed: RoutedTrigger) -> Self {
        match routed {
            RoutedTrigger::Job(intent) => Self::Job {
                purpose: format!("{:?}", intent.purpose),
                demands: intent.selection.demands.len(),
                policy_digest: format!("{:?}", intent.policy.resolution_digest),
            },
            RoutedTrigger::Inert { reason } => Self::Inert {
                reason: format!("{reason:?}"),
            },
            RoutedTrigger::Unsupported { capability, reason } => Self::Unsupported {
                capability: capability.to_owned(),
                reason,
            },
            RoutedTrigger::Denied { reason } => Self::Denied { reason },
            RoutedTrigger::ShareGate { .. } => Self::ShareGate,
        }
    }
}

/// Scheduler command — every variant carries a reply channel so the
/// caller gets a typed answer; `Shutdown` ends the worker.
enum SchedulerMsg {
    /// Route + submit a trigger report without driving — the worker
    /// loop owns advancement (share events still evaluate
    /// synchronously inside the orchestrator).
    HandleReport {
        /// The typed trigger report.
        report: Box<TriggerReport>,
        /// Outcome reply.
        reply: std::sync::mpsc::Sender<TriggerOutcome>,
    },
    /// Submit a pre-assembled job intent.
    SubmitIntent {
        /// The intent.
        intent: Box<JobIntent>,
        /// Outcome reply.
        reply: std::sync::mpsc::Sender<SubmitOutcome>,
    },
    /// Route-only preview (no submission).
    Preview {
        /// The typed trigger report.
        report: Box<TriggerReport>,
        /// Preview reply.
        reply: std::sync::mpsc::Sender<RoutePreview>,
    },
    /// Cooperative cancel.
    Cancel {
        /// Job key.
        key: RenderJobKey,
        /// `true` when the job existed and was non-terminal.
        reply: std::sync::mpsc::Sender<bool>,
    },
    /// Retry a retryable failure.
    Retry {
        /// Job key.
        key: RenderJobKey,
        /// `true` when the retry was accepted.
        reply: std::sync::mpsc::Sender<bool>,
    },
    /// Crash-resume a job.
    Resume {
        /// Job key.
        key: RenderJobKey,
        /// Drive outcome reply.
        reply: std::sync::mpsc::Sender<DriveOutcome>,
    },
    /// Replace local grants (environmental state changed) — wakes
    /// every gate-pending job so the recheck runs next quantum.
    SetGrants {
        /// New grants.
        grants: LocalGrants,
        /// Applied-reply.
        reply: std::sync::mpsc::Sender<()>,
    },
    /// One job snapshot.
    Job {
        /// Job key.
        key: RenderJobKey,
        /// View reply.
        reply: std::sync::mpsc::Sender<Option<JobView>>,
    },
    /// All job snapshots in key order.
    Jobs {
        /// Views reply.
        reply: std::sync::mpsc::Sender<Vec<JobView>>,
    },
    /// Job receipt stream.
    Receipts {
        /// Receipts reply.
        reply: std::sync::mpsc::Sender<Vec<JobReceipt>>,
    },
    /// Receipts for one job.
    JobReceipts {
        /// Job key.
        key: RenderJobKey,
        /// Receipts reply.
        reply: std::sync::mpsc::Sender<Vec<JobReceipt>>,
    },
    /// External receipt stream (save/commit/share outcomes).
    ExternalReceipts {
        /// Receipts reply.
        reply: std::sync::mpsc::Sender<Vec<crate::policy::trigger::ExternalReceipt>>,
    },
    /// Pre-issuance rejections.
    Rejected {
        /// Rejections reply.
        reply: std::sync::mpsc::Sender<Vec<RejectedAttempt>>,
    },
    /// End the worker after pending commands drain.
    Shutdown,
}

/// Typed scheduler-side failure — never panics, never blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchedulerError {
    /// Command queue is full — nothing was accepted; the caller may
    /// retry later. Distinct from job-level `Pending`: this is
    /// admission control, not job state.
    QueueFull,
    /// The worker is gone (shutdown requested or thread exited).
    Shutdown,
    /// The worker died before answering — the reply channel closed.
    WorkerGone,
}

impl std::fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull => write!(f, "render scheduler queue full"),
            Self::Shutdown => write!(f, "render scheduler shut down"),
            Self::WorkerGone => write!(f, "render scheduler worker gone"),
        }
    }
}

impl std::error::Error for SchedulerError {}

/// Worker-loop scratch — `resting` holds jobs whose last step
/// returned `Pending` so the loop blocks instead of spinning; any
/// command or any sibling progress wakes the whole set (the gate
/// recheck is cheap and self-correcting).
#[derive(Default)]
struct WorkerState {
    resting: BTreeSet<RenderJobKey>,
    cursor: usize,
}

/// Handle to the scheduler — cloneable, `Send + Sync`. Dropping the
/// last handle closes the command channel; the `OfflineJob` handle
/// inside requests cancellation and detaches, and the worker exits
/// at its next cooperative check.
#[derive(Clone)]
pub struct JobScheduler {
    inner: Arc<JobSchedulerInner>,
}

struct JobSchedulerInner {
    tx: SyncSender<SchedulerMsg>,
    // `OfflineJob` is `Send` but not `Sync` (it owns an event
    // `Receiver`); the mutex keeps the handle `Sync` so the scheduler
    // can back `Arc<dyn RenderDispatch + Send + Sync>` adapters.
    // Held for its Drop side-effect (cancel + detach) and to keep
    // the worker's lifetime tied to this handle.
    _job: std::sync::Mutex<OfflineJob<(), std::io::Error>>,
}

impl JobScheduler {
    /// Spawns the scheduler worker over `ports` with `grants` and a
    /// command queue bounded at `queue_capacity` (must be ≥ 1).
    ///
    /// # Panics
    /// Panics when the OS fails to spawn the worker thread.
    #[must_use]
    pub fn spawn(ports: SchedulerPorts, grants: LocalGrants, queue_capacity: usize) -> Self {
        let capacity = queue_capacity.max(1);
        let (tx, rx) = sync_channel::<SchedulerMsg>(capacity);
        let job = OfflineJob::spawn(move |scope| {
            scheduler_loop(&rx, ports, grants, scope);
            Ok(())
        });
        Self {
            inner: Arc::new(JobSchedulerInner {
                tx,
                _job: std::sync::Mutex::new(job),
            }),
        }
    }

    /// Headless composition: in-memory artifact store, cpu-only
    /// grants, and every port fail-closed — the honest minimum a
    /// host without render backends/settings/policy documents can
    /// wire. Render requests reject or fail typed; nothing is
    /// fabricated.
    #[must_use]
    pub fn headless() -> Self {
        Self::spawn(
            SchedulerPorts {
                store: Box::new(InMemoryArtifactStore::new()),
                executor: Box::new(UnavailableExecutor),
                settings: Box::new(UnavailableSettings),
                policies: Box::new(NoDeclaredPolicies),
                demands: Box::new(NoScopeDemands),
                plugin_states: None,
                external_assets: None,
                authorizer: None,
                calibration: None,
            },
            LocalGrants::cpu_only(),
            DEFAULT_QUEUE_CAPACITY,
        )
    }

    /// Enqueues `msg` and returns its reply receiver without waiting
    /// for the worker — `try_send` keeps callers non-blocking; a
    /// full queue is [`SchedulerError::QueueFull`].
    fn enqueue<T>(
        &self,
        build: impl FnOnce(std::sync::mpsc::Sender<T>) -> SchedulerMsg,
    ) -> Result<Receiver<T>, SchedulerError> {
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        self.inner
            .tx
            .try_send(build(reply_tx))
            .map_err(|e| match e {
                TrySendError::Full(_) => SchedulerError::QueueFull,
                TrySendError::Disconnected(_) => SchedulerError::Shutdown,
            })?;
        Ok(reply_rx)
    }

    /// Sends `msg` and returns its reply — `try_send` keeps callers
    /// non-blocking; a full queue is [`SchedulerError::QueueFull`].
    fn call<T>(
        &self,
        build: impl FnOnce(std::sync::mpsc::Sender<T>) -> SchedulerMsg,
    ) -> Result<T, SchedulerError> {
        self.enqueue(build)?
            .recv()
            .map_err(|_| SchedulerError::WorkerGone)
    }

    /// Routes + submits a trigger report without driving — returns
    /// the orchestrator's typed [`TriggerOutcome`]. Accepted jobs
    /// advance on the worker loop.
    ///
    /// # Errors
    /// [`SchedulerError`] when the queue is full or the worker is
    /// gone — nothing was accepted in that case.
    pub fn handle_report(&self, report: TriggerReport) -> Result<TriggerOutcome, SchedulerError> {
        self.call(|reply| SchedulerMsg::HandleReport {
            report: Box::new(report),
            reply,
        })
    }

    /// Submits a pre-assembled [`JobIntent`].
    ///
    /// # Errors
    /// [`SchedulerError`] — see [`Self::handle_report`].
    pub fn submit_intent(&self, intent: JobIntent) -> Result<SubmitOutcome, SchedulerError> {
        self.enqueue_intent(intent)?
            .recv()
            .map_err(|_| SchedulerError::WorkerGone)
    }

    /// Enqueues a pre-assembled [`JobIntent`] without waiting for
    /// the worker's submit evaluation.
    ///
    /// Ordering contract: the command sits in the worker's queue
    /// when this call returns — ahead of every command or signal
    /// the caller issues afterwards. The worker evaluates it at its
    /// next command drain, and drains process every queued command,
    /// so the submit cannot be overtaken by scheduler-visible work
    /// triggered after this call returns. Callers that must order
    /// a submission against another observable event (for example
    /// releasing a gated executor) get that happens-before from
    /// program order alone — no helper thread whose enqueue timing
    /// they cannot observe (synvelis#567).
    ///
    /// The returned receiver yields the same [`SubmitOutcome`]
    /// [`Self::submit_intent`] returns; `recv` fails only when the
    /// worker died before answering.
    ///
    /// # Errors
    /// [`SchedulerError::QueueFull`]/[`SchedulerError::Shutdown`]
    /// when the command could not be queued — nothing was accepted.
    pub fn enqueue_intent(
        &self,
        intent: JobIntent,
    ) -> Result<Receiver<SubmitOutcome>, SchedulerError> {
        self.enqueue(|reply| SchedulerMsg::SubmitIntent {
            intent: Box::new(intent),
            reply,
        })
    }

    /// Route-only preview — no job is created.
    ///
    /// # Errors
    /// [`SchedulerError`] — see [`Self::handle_report`].
    pub fn preview(&self, report: TriggerReport) -> Result<RoutePreview, SchedulerError> {
        self.call(|reply| SchedulerMsg::Preview {
            report: Box::new(report),
            reply,
        })
    }

    /// Cooperative cancel by job key — idempotent (`false` for
    /// unknown/terminal jobs).
    ///
    /// # Errors
    /// [`SchedulerError`] — see [`Self::handle_report`].
    pub fn cancel(&self, key: &RenderJobKey) -> Result<bool, SchedulerError> {
        self.call(|reply| SchedulerMsg::Cancel {
            key: key.clone(),
            reply,
        })
    }

    /// Retry a retryable failure by job key.
    ///
    /// # Errors
    /// [`SchedulerError`] — see [`Self::handle_report`].
    pub fn retry(&self, key: &RenderJobKey) -> Result<bool, SchedulerError> {
        self.call(|reply| SchedulerMsg::Retry {
            key: key.clone(),
            reply,
        })
    }

    /// Crash-resume a job by key — returns the drive outcome.
    ///
    /// # Errors
    /// [`SchedulerError`] — see [`Self::handle_report`].
    pub fn resume(&self, key: &RenderJobKey) -> Result<DriveOutcome, SchedulerError> {
        self.call(|reply| SchedulerMsg::Resume {
            key: key.clone(),
            reply,
        })
    }

    /// Replaces local grants — wakes every gate-pending job so the
    /// environmental recheck runs at the next quantum.
    ///
    /// # Errors
    /// [`SchedulerError::QueueFull`] when the queue is full — grants
    /// are *not* applied in that case.
    pub fn set_grants(&self, grants: LocalGrants) -> Result<(), SchedulerError> {
        let (reply_tx, reply_rx) = std::sync::mpsc::channel::<()>();
        self.inner
            .tx
            .try_send(SchedulerMsg::SetGrants {
                grants,
                reply: reply_tx,
            })
            .map_err(|e| match e {
                TrySendError::Full(_) => SchedulerError::QueueFull,
                TrySendError::Disconnected(_) => SchedulerError::Shutdown,
            })?;
        reply_rx.recv().map_err(|_| SchedulerError::WorkerGone)
    }

    /// One job snapshot, when the key exists.
    ///
    /// # Errors
    /// [`SchedulerError`] — see [`Self::handle_report`].
    pub fn job(&self, key: &RenderJobKey) -> Result<Option<JobView>, SchedulerError> {
        self.call(|reply| SchedulerMsg::Job {
            key: key.clone(),
            reply,
        })
    }

    /// All job snapshots in key order.
    ///
    /// # Errors
    /// [`SchedulerError`] — see [`Self::handle_report`].
    pub fn jobs(&self) -> Result<Vec<JobView>, SchedulerError> {
        self.call(|reply| SchedulerMsg::Jobs { reply })
    }

    /// The job receipt stream (all jobs, recording order).
    ///
    /// # Errors
    /// [`SchedulerError`] — see [`Self::handle_report`].
    pub fn receipts(&self) -> Result<Vec<JobReceipt>, SchedulerError> {
        self.call(|reply| SchedulerMsg::Receipts { reply })
    }

    /// Receipts for one job.
    ///
    /// # Errors
    /// [`SchedulerError`] — see [`Self::handle_report`].
    pub fn job_receipts(&self, key: &RenderJobKey) -> Result<Vec<JobReceipt>, SchedulerError> {
        self.call(|reply| SchedulerMsg::JobReceipts {
            key: key.clone(),
            reply,
        })
    }

    /// The external receipt stream (save/commit/share outcomes).
    ///
    /// # Errors
    /// [`SchedulerError`] — see [`Self::handle_report`].
    pub fn external_receipts(
        &self,
    ) -> Result<Vec<crate::policy::trigger::ExternalReceipt>, SchedulerError> {
        self.call(|reply| SchedulerMsg::ExternalReceipts { reply })
    }

    /// Pre-issuance rejections.
    ///
    /// # Errors
    /// [`SchedulerError`] — see [`Self::handle_report`].
    pub fn rejected_attempts(&self) -> Result<Vec<RejectedAttempt>, SchedulerError> {
        self.call(|reply| SchedulerMsg::Rejected { reply })
    }

    /// Polls until `key` reaches a terminal state or `timeout`
    /// elapses — a test/adapter convenience, not a scheduling
    /// primitive. Returns the final [`JobView`] on terminal, `None`
    /// on timeout or unknown key.
    #[must_use]
    pub fn wait_terminal(&self, key: &RenderJobKey, timeout: Duration) -> Option<JobView> {
        let deadline = Instant::now() + timeout;
        loop {
            match self.job(key) {
                Ok(Some(view)) if view.state.is_terminal() => return Some(view),
                // A full queue is transient admission control — keep
                // polling; a gone worker or unknown key is not.
                Ok(Some(_)) | Err(SchedulerError::QueueFull) => {}
                _ => return None,
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Requests worker shutdown — pending commands already queued
    /// are processed first; in-flight job state is simply dropped
    /// (the orchestrator is in-memory by contract).
    pub fn shutdown(&self) {
        // Blocking send: the worker drains between quanta, so this
        // waits at most one demand render; a dead worker errors
        // immediately on the disconnected channel.
        let _ = self.inner.tx.send(SchedulerMsg::Shutdown);
    }
}

// ---------------------------------------------------------------------------
// worker loop
// ---------------------------------------------------------------------------

/// Picks the next job to step: round-robin over non-terminal,
/// non-resting keys. Returns `None` when nothing is runnable — the
/// caller then blocks on the command channel.
fn pick_candidate(orch: &JobOrchestrator<'_>, state: &WorkerState) -> Option<RenderJobKey> {
    let keys = orch.job_keys();
    if keys.is_empty() {
        return None;
    }
    for offset in 0..keys.len() {
        let idx = (state.cursor + offset) % keys.len();
        let key = &keys[idx];
        let runnable = orch
            .job(key)
            .is_some_and(|j| !j.state.is_terminal() && !j.crashed && !state.resting.contains(key));
        if runnable {
            return Some(key.clone());
        }
    }
    None
}

/// Applies one scheduler command. Returns `true` on `Shutdown`.
fn handle_message(
    orch: &mut JobOrchestrator<'_>,
    state: &mut WorkerState,
    msg: SchedulerMsg,
) -> bool {
    match msg {
        SchedulerMsg::HandleReport { report, reply } => {
            let _ = reply.send(orch.handle_async(&report));
        }
        SchedulerMsg::SubmitIntent { intent, reply } => {
            let _ = reply.send(orch.submit(&intent));
        }
        SchedulerMsg::Preview { report, reply } => {
            let routed = orch.route(&report);
            let _ = reply.send(RoutePreview::from(routed));
        }
        SchedulerMsg::Cancel { key, reply } => {
            let _ = reply.send(orch.cancel(&key));
        }
        SchedulerMsg::Retry { key, reply } => {
            let ok = orch.retry(&key);
            if ok {
                state.resting.clear();
            }
            let _ = reply.send(ok);
        }
        SchedulerMsg::Resume { key, reply } => {
            // Clear the crashed flag only — the job re-enters the
            // normal quantum loop instead of being driven to
            // quiescence inside this command handler.
            if orch.resume_crashed(&key) {
                state.resting.clear();
                let _ = reply.send(DriveOutcome::Progress);
            } else {
                // Non-crashed jobs already advance on the quantum
                // loop — a synchronous step here would hold the
                // command reply for a whole demand render.
                let _ = reply.send(DriveOutcome::Pending);
            }
        }
        SchedulerMsg::SetGrants { grants, reply } => {
            orch.set_grants(grants);
            state.resting.clear();
            let _ = reply.send(());
        }
        SchedulerMsg::Job { key, reply } => {
            let _ = reply.send(orch.job(&key).map(JobView::from));
        }
        SchedulerMsg::Jobs { reply } => {
            let views = orch
                .job_keys()
                .iter()
                .filter_map(|k| orch.job(k))
                .map(JobView::from)
                .collect();
            let _ = reply.send(views);
        }
        SchedulerMsg::Receipts { reply } => {
            let _ = reply.send(orch.receipts().to_vec());
        }
        SchedulerMsg::JobReceipts { key, reply } => {
            let _ = reply.send(orch.job_receipts(&key));
        }
        SchedulerMsg::ExternalReceipts { reply } => {
            let _ = reply.send(orch.external_receipts().to_vec());
        }
        SchedulerMsg::Rejected { reply } => {
            let _ = reply.send(orch.rejected_attempts().to_vec());
        }
        SchedulerMsg::Shutdown => return true,
    }
    false
}

/// The worker body — owns the ports and the orchestrator for its
/// whole life. Loop: drain every queued command, step one runnable
/// job by one quantum, repeat; block on the channel only when no
/// job can advance. Exits on `Shutdown`, channel disconnect, or
/// `JobScope` cancellation.
fn scheduler_loop(
    rx: &Receiver<SchedulerMsg>,
    ports: SchedulerPorts,
    grants: LocalGrants,
    scope: &ruviform_exec::JobScope,
) {
    let SchedulerPorts {
        mut store,
        mut executor,
        settings,
        policies,
        demands,
        plugin_states,
        external_assets,
        authorizer,
        calibration,
    } = ports;

    let mut orch = JobOrchestrator::new(
        &mut *store,
        &mut *executor,
        settings.as_ref(),
        policies.as_ref(),
        demands.as_ref(),
        plugin_states
            .as_deref()
            .map(|p| -> &dyn PluginStateProbe { p }),
        external_assets
            .as_deref()
            .map(|p| -> &dyn ExternalAssetProbe { p }),
        authorizer
            .as_deref()
            .map(|a| -> &dyn DemandAuthorizer { a }),
        grants,
    );
    if let Some(cal) = calibration
        .as_deref()
        .map(|c| -> &dyn CostCalibration { c })
    {
        orch = orch.with_cost_calibration(cal);
    }

    let mut state = WorkerState::default();
    loop {
        if scope.is_cancelled() {
            return;
        }

        // Drain every queued command before the next quantum —
        // control latency stays within one demand render.
        let mut commanded = false;
        loop {
            match rx.try_recv() {
                Ok(msg) => {
                    commanded = true;
                    if handle_message(&mut orch, &mut state, msg) {
                        return;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
            }
        }
        // Any command may have changed gates/grants — re-scan rested
        // jobs once.
        if commanded {
            state.resting.clear();
        }

        if let Some(key) = pick_candidate(&orch, &state) {
            let outcome = orch.drive_step(&key);
            match outcome {
                DriveOutcome::Progress => {
                    state.cursor = state.cursor.wrapping_add(1);
                }
                DriveOutcome::Pending => {
                    state.resting.insert(key);
                }
                DriveOutcome::Terminal | DriveOutcome::Crashed => {
                    // Sibling finished — quota/storage gates may have
                    // freed; wake rested jobs for one recheck.
                    state.resting.clear();
                }
                DriveOutcome::Unknown => {}
            }
            continue;
        }

        // Nothing runnable — block for the next command. Channel
        // disconnect (all handles dropped) ends the worker.
        match rx.recv() {
            Ok(msg) => {
                if handle_message(&mut orch, &mut state, msg) {
                    return;
                }
            }
            Err(_) => return,
        }
    }
}

// ---------------------------------------------------------------------------
// fail-closed port defaults (headless composition)
// ---------------------------------------------------------------------------

/// No render backend is wired — every render reports a typed
/// permanent [`JobFailureReason::Unsupported`] failure. The job
/// still flows through the full state machine (preflight, receipts,
/// completeness) so the failure is auditable, never fabricated.
pub struct UnavailableExecutor;

impl RenderExecutor for UnavailableExecutor {
    fn backend_requirement(&self) -> BackendRequirement {
        BackendRequirement::Cpu
    }

    fn render(
        &mut self,
        _job: &JobContext<'_>,
        _recipe: &RenderRecipeV1,
        _cancel: &ruviform_exec::CancellationToken,
    ) -> RenderExecution {
        RenderExecution::Failed {
            reason: JobFailureReason::Unsupported,
            detail: "no render executor is wired for this host".to_owned(),
        }
    }
}

/// No settings document is available — every quality-profile
/// reference resolves to `None`, so `submit` rejects typed
/// (`UnknownDependency`) before any work is issued.
pub struct UnavailableSettings;

impl RenderSettingsProbe for UnavailableSettings {
    fn resolve(&self, _reference: &RenderSettingsRef) -> Option<RenderQualityInputs> {
        None
    }
}

/// No declared project policy and no local override — the effective
/// policy is always the built-in default.
pub struct NoDeclaredPolicies;

impl PolicyResolver for NoDeclaredPolicies {
    fn declared(&self, _project_id: ProjectId) -> Option<PolicyPatch> {
        None
    }

    fn local_override(&self) -> Option<PolicyPatch> {
        None
    }
}

/// No scope→demand mapping — every scope-driven trigger routes
/// `Inert` (typed `NoDemands`), never fabricates demands.
pub struct NoScopeDemands;

impl ScopeDemandResolver for NoScopeDemands {
    fn demands(&self, _project: &Project, _purpose: &JobPurpose) -> Vec<RenderDemand> {
        Vec::new()
    }
}
