//! [`SynvelisApp`] — the product composition root.
//!
//! One `SynvelisApp` owns the one canonical `WorkspaceManager` →
//! `ProjectSession` → `ProjectController` chain plus the runtime-owner
//! slots (audio runtime, recorder, observability). Windows, frontends,
//! and headless callers never get their own project/session/engine —
//! they all reach this single instance through [`crate::AppService`].
//!
//! Per SV-GUI-01 scope the audio-runtime and recorder slots are
//! *declared* (so the composition shape is fixed and later issues
//! attach real runtimes without restructuring) but report
//! `unavailable` — the shell must surface that honestly instead of
//! implying playback/recording exists.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ruviform_observability::{Counter, counter};
use ruviform_ui::BackendReport;
use serde_json::{Value, json};

use synvelis_project::{
    CounterIdGenerator, DirectoryBundleStore, Project, ProjectController, ProjectId,
};
use synvelis_workspace::{ProjectSession, WorkspaceManager};

use crate::error::AppError;
use crate::host::ShellWindowHost;
use crate::revision::HOST_REVISION;
use crate::ui_ops::UiOpsState;

/// Crate version baked at build time — the app revision reported to
/// the frontend.
const APP_REVISION: &str = env!("CARGO_PKG_VERSION");

/// Placeholder `ProjectId` for the boot project — replaced on
/// `project.new`/`project.open`; identity inside a loaded project is
/// whatever the store snapshot carries.
const BOOT_PROJECT_ID: u128 = 0;

/// Audio-runtime owner slot.
///
/// Declared so the composition root has a single place the engine
/// attaches to; SV-GUI-01 does not wire an engine — `attached()` is
/// always `false` and the capability report says `unavailable`.
#[derive(Debug, Default)]
pub struct AudioRuntimeSlot {
    _private: (),
}

impl AudioRuntimeSlot {
    /// Whether a real engine is attached (never in SV-GUI-01).
    #[must_use]
    pub const fn attached(&self) -> bool {
        false
    }

    /// Status line for the capability report.
    #[must_use]
    pub const fn status(&self) -> &'static str {
        "unavailable"
    }
}

/// Recorder owner slot — declared, not yet wired to a recording
/// session.
#[derive(Debug, Default)]
pub struct RecorderSlot {
    _private: (),
}

impl RecorderSlot {
    /// Whether a recording session is active (never in SV-GUI-01).
    #[must_use]
    pub const fn active(&self) -> bool {
        false
    }

    /// Status line for the capability report.
    #[must_use]
    pub const fn status(&self) -> &'static str {
        "idle"
    }
}

/// Process-wide observability owner — counters the service layer
/// increments; the shell reads them via `syn.app.info`.
#[derive(Debug)]
pub struct ObservabilityOwner {
    intents: Counter,
    intent_errors: Counter,
}

impl ObservabilityOwner {
    /// Zeroed counters.
    #[must_use]
    pub fn new() -> Self {
        let (intents, _) = counter();
        let (intent_errors, _) = counter();
        Self {
            intents,
            intent_errors,
        }
    }

    /// Records one handled `syn.*` intent.
    pub fn record_intent(&self) {
        self.intents.increment();
    }

    /// Records one failed `syn.*` intent.
    pub fn record_intent_error(&self) {
        self.intent_errors.increment();
    }

    /// Snapshot for `syn.app.info` reporting.
    #[must_use]
    pub fn counters(&self) -> (u64, u64) {
        (self.intents.read(), self.intent_errors.read())
    }
}

impl Default for ObservabilityOwner {
    fn default() -> Self {
        Self::new()
    }
}

/// The product application composition root.
///
/// Single-threaded by design — owned by the `synvelis` binary's
/// `run` closure (UI thread) or by a headless `UiRuntime` driver in
/// tests/CLI.
pub struct SynvelisApp {
    /// `pub(crate)` so `ui_ops` can borrow the session disjoint from
    /// the `ui` gateway state (method accessors would borrow all of
    /// `self`).
    pub(crate) workspace: WorkspaceManager<ShellWindowHost>,
    store: Option<DirectoryBundleStore>,
    audio: AudioRuntimeSlot,
    recorder: RecorderSlot,
    observability: ObservabilityOwner,
    backend: BackendReport,
    /// Shared quit flag — `syn.app.quit` sets it from inside the
    /// registered `Service` (which the `UiRuntime` owns and the host
    /// loop cannot reach back into), and the host loop drains it to
    /// call `HostHandle::request_close`.
    quit: Arc<AtomicBool>,
    /// SV-GUI-02 UI op state — `ControlsUi`/`VizUi` gateways, notify
    /// outbox, view/subscription/surface registries, UI clock.
    pub(crate) ui: UiOpsState,
}

impl SynvelisApp {
    /// Composes the app around a fresh untitled project; `backend` is
    /// the probed `WebView` report the shell surfaces as capability
    /// information (`BackendReport` is backend-agnostic — headless
    /// callers may pass a synthetic one).
    #[must_use]
    pub fn new(backend: BackendReport) -> Self {
        Self {
            workspace: WorkspaceManager::new(
                ShellWindowHost::new(),
                Self::fresh_session("untitled"),
            ),
            store: None,
            audio: AudioRuntimeSlot::default(),
            recorder: RecorderSlot::default(),
            observability: ObservabilityOwner::new(),
            backend,
            quit: Arc::new(AtomicBool::new(false)),
            ui: UiOpsState::new(),
        }
    }

    fn fresh_session(name: &str) -> ProjectSession {
        ProjectSession::new(ProjectController::new(
            Project::new(ProjectId::new(BOOT_PROJECT_ID), name),
            Box::new(CounterIdGenerator::new(1)),
        ))
    }

    /// The workspace manager (project session reachable through it).
    #[must_use]
    pub const fn workspace(&self) -> &WorkspaceManager<ShellWindowHost> {
        &self.workspace
    }

    /// Mutable workspace manager.
    #[must_use]
    pub const fn workspace_mut(&mut self) -> &mut WorkspaceManager<ShellWindowHost> {
        &mut self.workspace
    }

    /// The probed `WebView` backend report.
    #[must_use]
    pub const fn backend(&self) -> &BackendReport {
        &self.backend
    }

    /// The observability owner.
    #[must_use]
    pub const fn observability(&self) -> &ObservabilityOwner {
        &self.observability
    }

    /// Marks quit requested; the host loop observes
    /// [`Self::quit_flag`]/[`Self::take_quit`] and calls
    /// `HostHandle::request_close`.
    pub fn request_quit(&mut self) {
        self.quit.store(true, Ordering::Release);
    }

    /// A clone of the shared quit flag — handed to the host loop
    /// before the service moves into `UiRuntime`.
    #[must_use]
    pub fn quit_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.quit)
    }

    /// Returns and clears the quit flag.
    pub fn take_quit(&mut self) -> bool {
        self.quit.swap(false, Ordering::AcqRel)
    }

    /// Creates a new project at `path` (directory bundle), saves the
    /// initial revision, and resets the workspace/session around it.
    ///
    /// # Errors
    /// [`AppError::Store`] on bundle-creation or save failures.
    pub fn project_new(&mut self, path: &str, name: Option<&str>) -> Result<Value, AppError> {
        let mut store =
            DirectoryBundleStore::create(path).map_err(|e| AppError::store(e.to_string()))?;
        let mut workspace = WorkspaceManager::new(
            ShellWindowHost::new(),
            Self::fresh_session(name.unwrap_or("untitled")),
        );
        workspace
            .session_mut()
            .controller_mut()
            .save(&mut store)
            .map_err(|e| AppError::store(e.to_string()))?;
        self.workspace = workspace;
        self.store = Some(store);
        Ok(self.project_info())
    }

    /// Opens the project bundle at `path` — loads `HEAD` and rebuilds
    /// the workspace/session around the loaded project.
    ///
    /// # Errors
    /// [`AppError::Store`] when the bundle is missing, empty, or fails
    /// to load.
    pub fn project_open(&mut self, path: &str) -> Result<Value, AppError> {
        let store =
            DirectoryBundleStore::create(path).map_err(|e| AppError::store(e.to_string()))?;
        let session = Self::fresh_session("loading");
        let mut workspace = WorkspaceManager::new(ShellWindowHost::new(), session);
        workspace
            .session_mut()
            .controller_mut()
            .load(&store)
            .map_err(|e| AppError::store(e.to_string()))?;
        self.workspace = workspace;
        self.store = Some(store);
        Ok(self.project_info())
    }

    /// Saves the live project to the currently open bundle.
    ///
    /// # Errors
    /// [`AppError::Store`] when no project was opened/created yet or
    /// the write fails.
    pub fn project_save(&mut self) -> Result<Value, AppError> {
        let store = self
            .store
            .as_mut()
            .ok_or_else(|| AppError::store("no project store — use syn.project.new/open first"))?;
        self.workspace
            .session_mut()
            .controller_mut()
            .save(store)
            .map_err(|e| AppError::store(e.to_string()))?;
        Ok(self.project_info())
    }

    /// Current project summary for `syn.project.info` / save-open
    /// replies.
    #[must_use]
    pub fn project_info(&self) -> Value {
        let project = self.workspace.session().controller().live();
        json!({
            "name": project.name,
            "project_id": project.project_id.to_tagged(),
            "revision": self.workspace.session().controller().live_revision().0,
            "saved": self.workspace.session().controller().saved().is_some(),
            "store": self.store.is_some(),
        })
    }

    /// Sets transport playing/stopped through the session.
    pub fn transport_set(&mut self, playing: bool) {
        self.workspace.session_mut().set_playing(playing);
    }

    /// Workspace mount-point summary for `syn.workspace.mount`.
    ///
    /// The shell window is host-owned (created from `HostConfig`, not
    /// through `WorkspaceManager`), so it is reported separately from
    /// the manager's tracked windows — which are honestly `[]` until
    /// views mount in SV-GUI-02.
    #[must_use]
    pub fn workspace_info(&self) -> Value {
        let windows: Vec<Value> = self
            .workspace
            .windows()
            .iter()
            .map(|w| json!({ "id": w.id.get(), "title": w.title }))
            .collect();
        json!({
            "shell_window": self.workspace.host.window().get(),
            "windows": windows,
            "views": 0,
            "mounted": true,
        })
    }

    /// Application info for `syn.app.info`.
    #[must_use]
    pub fn app_info(&self) -> Value {
        let (intents, intent_errors) = self.observability.counters();
        json!({
            "name": "synvelis",
            "version": APP_REVISION,
            "host_revision": HOST_REVISION,
            "ipc": ruviform_ui::ipc::SCHEMA_VERSION,
            "backend": {
                "name": self.backend.name,
                "version": self.backend.version,
                "state": format!("{:?}", self.backend.state),
                "notes": self.backend.notes,
            },
            "counters": { "intents": intents, "intent_errors": intent_errors },
        })
    }

    /// Capability report for `syn.app.capabilities` — every
    /// not-yet-wired area is listed `unavailable` with a reason, never
    /// silently omitted.
    #[must_use]
    pub fn capabilities(&self) -> Value {
        json!({
            "webview": {
                "state": format!("{:?}", self.backend.state),
                "backend": self.backend.name,
                "version": self.backend.version,
                "notes": self.backend.notes,
            },
            "audio_runtime": { "state": "unavailable", "attached": self.audio.attached() },
            "recorder": { "state": "unavailable", "active": self.recorder.active() },
            "workspace": { "state": "available", "mounted": true },
            "project": { "state": "available" },
            "multi_window": { "state": "unavailable", "reason": "deferred to SV-GUI-03" },
            "views": { "state": "available" },
            "signal_flow": { "state": "unavailable", "reason": "issue #108" },
            "plugins": { "state": "unavailable", "reason": "not wired in SV-GUI-01" },
            "ai": { "state": "unavailable", "reason": "out of scope" },
        })
    }

    /// Latest-wins app-state push payload for the `Ready` event.
    pub fn app_state_notify(&mut self) -> ruviform_ui::ipc::Event {
        self.ui.notify_seq += 1;
        ruviform_ui::ipc::Event::Notify {
            seq: self.ui.notify_seq,
            data: json!({
                "app": "synvelis",
                "project": self.project_info(),
                "capabilities": self.capabilities(),
            }),
        }
    }
}
