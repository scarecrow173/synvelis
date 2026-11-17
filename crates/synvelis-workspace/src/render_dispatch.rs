//! Production [`RenderDispatch`] adapter (synvelis#518) — bridges the
//! gateway's host-injected render port to the [`JobScheduler`] owned
//! by [`crate::job_system::JobSystem`].
//!
//! The gateway forwards `synvelis.render.*` intents verbatim; this
//! adapter owns the wire contract:
//!
//! - `synvelis.render.request` / `synvelis.render.plan` — `args` is a
//!   serialized [`TriggerReport`] (the same lowering
//!   `render_request_report` produces on every surface).
//! - `synvelis.render.cancel` / `synvelis.render.retry` /
//!   `synvelis.render.inspect` — `args` is `{"job": <RenderJobKey>}`
//!   (inspect also accepts an absent/empty object to list all jobs).
//! - `synvelis.render.verify` / `attach` / `publish` / `remote` —
//!   typed `Unavailable`: re-verification, manifest attach, the
//!   share gate, and remote backends are separate gates, never
//!   fabricated here.
//!
//! Outcome mapping preserves determined rejections: scheduler
//! admission failures are `Failed`, a missing worker is `Unavailable`,
//! accepted-but-async work is `Pending` with the job key as receipt —
//! a render success is never claimed.

use std::sync::Arc;

use serde_json::Value;
use synvelis_gateway::variant::{
    RenderAction, RenderDispatch, RenderDispatchOutcome, RenderDispatchState, RenderIntent,
};
use synvelis_render::policy::trigger::{TriggerOutcome, TriggerReport};
use synvelis_render::render_job::{
    JobScheduler, RenderJobKey, RenderJobState, SchedulerError, ShareGateVerdict,
};

/// The workspace render backend — holds a scheduler handle and maps
/// gateway intents onto scheduler commands.
#[derive(Clone)]
pub struct WorkspaceRenderDispatch {
    scheduler: JobScheduler,
}

impl WorkspaceRenderDispatch {
    /// Creates the adapter over `scheduler`.
    #[must_use]
    pub fn new(scheduler: JobScheduler) -> Self {
        Self { scheduler }
    }

    /// The `Arc<dyn RenderDispatch + Send + Sync>` form hosts inject
    /// into `GateDeps.render`.
    #[must_use]
    pub fn shared(scheduler: JobScheduler) -> Arc<dyn RenderDispatch + Send + Sync> {
        Arc::new(Self::new(scheduler))
    }

    fn outcome(
        state: RenderDispatchState,
        detail: Option<String>,
        receipt: Option<Value>,
    ) -> RenderDispatchOutcome {
        RenderDispatchOutcome {
            state,
            detail,
            receipt,
        }
    }

    fn scheduler_error(error: &SchedulerError) -> RenderDispatchOutcome {
        match error {
            // Admission control — determined rejection, retryable by
            // the caller submitting again later.
            SchedulerError::QueueFull => Self::outcome(
                RenderDispatchState::Failed {
                    reason: "render scheduler queue full".to_owned(),
                },
                Some("command queue is bounded; retry when capacity frees".to_owned()),
                None,
            ),
            // The backend itself is gone — unavailable, not a
            // command failure.
            SchedulerError::Shutdown | SchedulerError::WorkerGone => Self::outcome(
                RenderDispatchState::Unavailable {
                    reason: "render scheduler worker is not running".to_owned(),
                },
                None,
                None,
            ),
        }
    }

    fn decode_report(args: &Value) -> Result<TriggerReport, RenderDispatchOutcome> {
        serde_json::from_value(args.clone()).map_err(|e| {
            Self::outcome(
                RenderDispatchState::Failed {
                    reason: format!("invalid render request args: {e}"),
                },
                None,
                None,
            )
        })
    }

    /// Decodes `{"job": <RenderJobKey>}`; a bare key object is also
    /// accepted for convenience.
    fn decode_key(args: &Value) -> Result<RenderJobKey, RenderDispatchOutcome> {
        let candidate = args.get("job").cloned().unwrap_or_else(|| args.clone());
        serde_json::from_value(candidate).map_err(|e| {
            Self::outcome(
                RenderDispatchState::Failed {
                    reason: format!("invalid job key args: {e}"),
                },
                None,
                None,
            )
        })
    }

    fn handle_report(&self, args: &Value, preview: bool) -> RenderDispatchOutcome {
        let report = match Self::decode_report(args) {
            Ok(r) => r,
            Err(o) => return o,
        };
        if preview {
            return match self.scheduler.preview(report) {
                Ok(plan) => Self::outcome(
                    RenderDispatchState::Dispatched,
                    Some(format!("route preview: {plan:?}")),
                    serde_json::to_value(&plan).ok(),
                ),
                Err(e) => Self::scheduler_error(&e),
            };
        }
        match self.scheduler.handle_report(report) {
            Ok(TriggerOutcome::Job { key, coalesced }) => {
                let receipt = serde_json::to_value(&key).ok();
                if coalesced {
                    // A coalesced submit into a terminal job does no
                    // new work — report the actual job state rather
                    // than claiming "in-flight" (synvelis#518).
                    match self.scheduler.job(&key) {
                        Ok(Some(view)) if view.state == RenderJobState::Published => Self::outcome(
                            RenderDispatchState::Dispatched,
                            Some("coalesced into already-published render job".to_owned()),
                            receipt,
                        ),
                        Ok(Some(view)) if view.state.is_terminal() => Self::outcome(
                            RenderDispatchState::Failed {
                                reason: format!(
                                    "identical render job already reached terminal state {:?}",
                                    view.state
                                ),
                            },
                            None,
                            receipt,
                        ),
                        _ => Self::outcome(
                            RenderDispatchState::Pending {
                                reason: "coalesced into in-flight render job".to_owned(),
                            },
                            None,
                            receipt,
                        ),
                    }
                } else {
                    Self::outcome(
                        RenderDispatchState::Pending {
                            reason: "render job accepted; completion is asynchronous".to_owned(),
                        },
                        None,
                        receipt,
                    )
                }
            }
            Ok(TriggerOutcome::ShareGate { job, verdict }) => {
                let receipt = job.and_then(|k| serde_json::to_value(&k).ok());
                let detail = Some(format!("{verdict:?}"));
                match &verdict {
                    ShareGateVerdict::Allowed { .. } | ShareGateVerdict::AllowedPartial { .. } => {
                        Self::outcome(RenderDispatchState::Dispatched, detail, receipt)
                    }
                    // In-flight work may still close the gaps —
                    // retry the share request later.
                    ShareGateVerdict::Blocked { pending_jobs, .. } if !pending_jobs.is_empty() => {
                        Self::outcome(
                            RenderDispatchState::Pending {
                                reason: "share gate blocked: render job still in flight".to_owned(),
                            },
                            detail,
                            receipt,
                        )
                    }
                    // Determined negative — never an acceptance-class
                    // pending result.
                    ShareGateVerdict::Blocked { .. } => Self::outcome(
                        RenderDispatchState::Failed {
                            reason: "share gate blocked: required artifacts incomplete".to_owned(),
                        },
                        detail,
                        receipt,
                    ),
                }
            }
            Ok(TriggerOutcome::Inert { reason }) => Self::outcome(
                RenderDispatchState::Dispatched,
                Some(format!("inert: {reason:?}")),
                serde_json::to_value(format!("{reason:?}")).ok(),
            ),
            Ok(TriggerOutcome::Denied { reason }) => {
                Self::outcome(RenderDispatchState::Failed { reason }, None, None)
            }
            Ok(TriggerOutcome::Unsupported { capability, reason }) => Self::outcome(
                RenderDispatchState::Unavailable {
                    reason: format!("unsupported {capability}: {reason}"),
                },
                None,
                None,
            ),
            Err(e) => Self::scheduler_error(&e),
        }
    }

    fn job_control(&self, args: &Value, retry: bool) -> RenderDispatchOutcome {
        let key = match Self::decode_key(args) {
            Ok(k) => k,
            Err(o) => return o,
        };
        let result = if retry {
            self.scheduler.retry(&key)
        } else {
            self.scheduler.cancel(&key)
        };
        match result {
            Ok(true) => Self::outcome(
                RenderDispatchState::Dispatched,
                Some(
                    if retry {
                        "retry accepted"
                    } else {
                        "cancel accepted"
                    }
                    .to_owned(),
                ),
                serde_json::to_value(&key).ok(),
            ),
            // Unknown/terminal/ineligible — idempotent no-op, still a
            // determined answer.
            Ok(false) => Self::outcome(
                RenderDispatchState::Dispatched,
                Some("job key unknown, terminal, or control not applicable".to_owned()),
                serde_json::to_value(&key).ok(),
            ),
            Err(e) => Self::scheduler_error(&e),
        }
    }

    fn inspect(&self, args: &Value) -> RenderDispatchOutcome {
        // `{"job": <key>}` → one view; `{}`/absent → all views.
        if args.get("job").is_some() {
            let key = match Self::decode_key(args) {
                Ok(k) => k,
                Err(o) => return o,
            };
            return match self.scheduler.job(&key) {
                Ok(Some(view)) => Self::outcome(
                    RenderDispatchState::Dispatched,
                    None,
                    serde_json::to_value(&view).ok(),
                ),
                Ok(None) => Self::outcome(
                    RenderDispatchState::Failed {
                        reason: "unknown render job key".to_owned(),
                    },
                    None,
                    None,
                ),
                Err(e) => Self::scheduler_error(&e),
            };
        }
        match self.scheduler.jobs() {
            Ok(views) => Self::outcome(
                RenderDispatchState::Dispatched,
                Some(format!("{} render job(s)", views.len())),
                serde_json::to_value(&views).ok(),
            ),
            Err(e) => Self::scheduler_error(&e),
        }
    }
}

impl RenderDispatch for WorkspaceRenderDispatch {
    fn dispatch(&self, intent: &RenderIntent) -> RenderDispatchOutcome {
        match intent.action {
            RenderAction::Request => self.handle_report(&intent.args, false),
            RenderAction::Plan => self.handle_report(&intent.args, true),
            RenderAction::Cancel => self.job_control(&intent.args, false),
            RenderAction::Retry => self.job_control(&intent.args, true),
            RenderAction::Inspect => self.inspect(&intent.args),
            // Re-verify, manifest attach, share/publish gate, and
            // remote backends are separate gates — typed unavailable,
            // never a fabricated success.
            RenderAction::Verify => Self::outcome(
                RenderDispatchState::Unavailable {
                    reason: "artifact re-verification is not wired in this slice".to_owned(),
                },
                None,
                None,
            ),
            RenderAction::Attach => Self::outcome(
                RenderDispatchState::Unavailable {
                    reason: "external manifest attach is not wired in this slice".to_owned(),
                },
                None,
                None,
            ),
            RenderAction::Publish => Self::outcome(
                RenderDispatchState::Unavailable {
                    reason: "share/publish gate is not wired in this slice".to_owned(),
                },
                None,
                None,
            ),
            RenderAction::Remote => Self::outcome(
                RenderDispatchState::Unavailable {
                    reason: "remote render backends are not wired in this slice".to_owned(),
                },
                None,
                None,
            ),
        }
    }
}

// `JobView` is re-exported for host surfaces building inspect
// receipts without a direct synvelis-render dependency.
pub use synvelis_render::render_job::JobView as RenderJobViewSnapshot;
