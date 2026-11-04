//! [`AppService`] — the `syn.*` op surface.
//!
//! Registered on a [`ruviform_ui::UiRuntime`] and reached identically
//! from the GUI host (`HostEvent::Message`), headless drivers
//! (`UiRuntime::dispatch_op`), and any future CLI adapter — so every
//! surface exercises the same service contract and the same single
//! project/session source of truth.
//!
//! Op names live under `syn.*` — `sys.*` is reserved to the runtime.
//! Every op returns [`CallOutcome::Result`]; long-running work (none in
//! SV-GUI-01) would use [`CallOutcome::Job`].

use serde_json::{Value, json};

use ruviform_ui::ipc::{ErrorBody, codes};
use ruviform_ui::{CallOutcome, OpInfo, Service, ServiceContext};

use crate::app::SynvelisApp;
use crate::error::AppError;

/// The `syn.*` ops this service registers.
pub const OPS: &[OpInfo] = &[
    OpInfo {
        name: "syn.app.info",
        summary: "application revision, host/IPC revisions, backend probe, counters",
    },
    OpInfo {
        name: "syn.app.capabilities",
        summary: "backend + feature availability report (unavailable areas are listed)",
    },
    OpInfo {
        name: "syn.app.quit",
        summary: "request application shutdown — the host closes the window",
    },
    OpInfo {
        name: "syn.project.new",
        summary: "create a new project bundle at params.path (params.name optional)",
    },
    OpInfo {
        name: "syn.project.open",
        summary: "open an existing project bundle at params.path",
    },
    OpInfo {
        name: "syn.project.save",
        summary: "save the live project to the open bundle",
    },
    OpInfo {
        name: "syn.project.info",
        summary: "project name, id, revision, saved state",
    },
    OpInfo {
        name: "syn.transport.set",
        summary: "set transport rolling — params.playing (bool)",
    },
    OpInfo {
        name: "syn.workspace.mount",
        summary: "workspace mount-point summary (shell window, views, dock)",
    },
];

/// [`Service`] over [`SynvelisApp`] — the GUI/headless op boundary.
pub struct AppService {
    app: SynvelisApp,
}

impl AppService {
    /// Wraps `app`.
    #[must_use]
    pub const fn new(app: SynvelisApp) -> Self {
        Self { app }
    }

    /// The composed application.
    #[must_use]
    pub const fn app(&self) -> &SynvelisApp {
        &self.app
    }

    /// Mutable application access — used by the host loop to drain
    /// the quit flag set by `syn.app.quit` and to push the initial
    /// app-state notify on `Ready`.
    #[must_use]
    pub const fn app_mut(&mut self) -> &mut SynvelisApp {
        &mut self.app
    }

    fn string_param<'a>(params: &'a Value, key: &str) -> Result<&'a str, ErrorBody> {
        params.get(key).and_then(Value::as_str).ok_or_else(|| {
            ErrorBody::new(
                codes::INVALID_PARAMS,
                format!("params.{key} (string) required"),
            )
        })
    }

    fn dispatch(&mut self, op: &str, params: &Value) -> Result<Value, ErrorBody> {
        match op {
            "syn.app.info" => Ok(self.app.app_info()),
            "syn.app.capabilities" => Ok(self.app.capabilities()),
            "syn.app.quit" => {
                self.app.request_quit();
                Ok(json!({ "quit": true }))
            }
            "syn.project.new" => {
                let path = Self::string_param(params, "path")?;
                let name = params.get("name").and_then(Value::as_str);
                self.app
                    .project_new(path, name)
                    .map_err(AppError::into_body)
            }
            "syn.project.open" => {
                let path = Self::string_param(params, "path")?;
                self.app.project_open(path).map_err(AppError::into_body)
            }
            "syn.project.save" => self.app.project_save().map_err(AppError::into_body),
            "syn.project.info" => Ok(self.app.project_info()),
            "syn.transport.set" => {
                let playing = params
                    .get("playing")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| {
                        ErrorBody::new(
                            codes::INVALID_PARAMS,
                            "params.playing (bool) required".to_string(),
                        )
                    })?;
                self.app.transport_set(playing);
                Ok(json!({ "playing": playing }))
            }
            "syn.workspace.mount" => Ok(self.app.workspace_info()),
            _ => Err(ErrorBody::new(
                codes::UNKNOWN_OP,
                format!("unhandled op: {op}"),
            )),
        }
    }
}

impl Service for AppService {
    fn ops(&self) -> &'static [OpInfo] {
        OPS
    }

    fn call(
        &mut self,
        op: &str,
        params: &Value,
        _ctx: &mut ServiceContext<'_>,
    ) -> Result<CallOutcome, ErrorBody> {
        // Non-RT boundary: tracing is allowed here (service ops run on
        // the UI/host thread, never on an audio callback). Event fields
        // carry the op name only — params/bodies may contain paths.
        tracing::debug!(op, "syn op received");
        self.app.observability().record_intent();
        let result = self.dispatch(op, params);
        if let Err(error) = &result {
            self.app.observability().record_intent_error();
            tracing::warn!(op, code = %error.code, "syn op failed");
        }
        result.map(CallOutcome::Result)
    }
}
