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

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ruviform_observability::logging::{HealthSnapshot, LoggingHealth, SinkDegraded};
use ruviform_observability::source::{
    CounterDescriptor, CounterSource, MetricSource, ObservationScope, ObservationUnit,
    ReportingHint,
};
use ruviform_observability::{Counter, counter};
use ruviform_ui::BackendReport;
use ruviform_ui::view::{AttachEpoch, AttachEpochAllocator};
use serde_json::{Value, json};

use synvelis_project::{
    CounterIdGenerator, DirectoryBundleStore, Project, ProjectController, ProjectId,
};
use synvelis_workspace::{ProjectSession, WorkspaceManager};

use crate::engine::SessionRoutingEngine;
use crate::error::AppError;
use crate::host::ShellWindowHost;
use crate::logging::InstallState;
use crate::revision::HOST_REVISION;
use crate::screens::MainScreenState;
use crate::ui_ops::UiOpsState;

/// Crate version baked at build time — the app revision reported to
/// the frontend.
const APP_REVISION: &str = env!("CARGO_PKG_VERSION");

/// Placeholder `ProjectId` for the boot project — replaced on
/// `project.new`/`project.open`; identity inside a loaded project is
/// whatever the store snapshot carries.
const BOOT_PROJECT_ID: u128 = 0;

/// Media rate of the seconds axis the project length is measured on.
/// Only resolves clip spans to whole frames; the seconds value does not
/// depend on it for musical clips.
const SEEK_MEDIA_RATE: u32 = 48_000;

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
/// increments plus the OBS-02 source catalog describing them; the
/// shell reads both via `syn.app.info`.
///
/// The catalog holds canonical `MetricSource` values so the sources
/// can be handed to an `ObservationRegistrar` unchanged once the
/// upstream collector (ruviform#289) lands — Synvelis keeps no
/// registrar of its own.
#[derive(Debug)]
pub struct ObservabilityOwner {
    intents: Counter,
    intent_errors: Counter,
    sources: Vec<MetricSource>,
    logging_state: InstallState,
    logging_health: Option<LoggingHealth>,
    logging_degraded: Vec<SinkDegraded>,
}

impl ObservabilityOwner {
    /// Zeroed counters with their source descriptors registered in the
    /// process-scope catalog.
    #[must_use]
    pub fn new() -> Self {
        let (intents, intents_reader) = counter();
        let (intent_errors, intent_errors_reader) = counter();
        let sources = [
            ("synvelis.app.intents", intents_reader),
            ("synvelis.app.intent_errors", intent_errors_reader),
        ]
        .into_iter()
        .map(|(key, reader)| {
            MetricSource::from(CounterSource {
                descriptor: CounterDescriptor {
                    key,
                    unit: ObservationUnit::Count,
                    reporting: ReportingHint::SnapshotOnly,
                },
                scope: ObservationScope::Process,
                reader,
            })
        })
        .collect();
        Self {
            intents,
            intent_errors,
            sources,
            logging_state: InstallState::Off,
            logging_health: None,
            logging_degraded: Vec::new(),
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

    /// The OBS-02 source catalog — descriptors + readers ready for
    /// `ObservationRegistrar` registration (ruviform#289).
    #[must_use]
    pub fn sources(&self) -> &[MetricSource] {
        &self.sources
    }

    /// Connects the process-level logging install outcome so
    /// `syn.app.info` can report it. `health` is `None` when logging is
    /// off or when the install call lost an already-installed race.
    pub fn attach_logging(
        &mut self,
        state: InstallState,
        health: Option<LoggingHealth>,
        degraded: Vec<SinkDegraded>,
    ) {
        self.logging_state = state;
        self.logging_health = health;
        self.logging_degraded = degraded;
    }

    /// Non-recursive snapshot of the installed sinks — `None` when no
    /// logging health handle is attached.
    #[must_use]
    pub fn logging_snapshot(&self) -> Option<HealthSnapshot> {
        self.logging_health.as_ref().map(LoggingHealth::snapshot)
    }

    /// The reported install state for `syn.app.info`.
    #[must_use]
    pub const fn logging_state(&self) -> InstallState {
        self.logging_state
    }

    /// Best-effort sinks that failed at build time.
    #[must_use]
    pub fn logging_degraded(&self) -> &[SinkDegraded] {
        &self.logging_degraded
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
    /// Session-owned `AttachEpoch` allocator (RF387) behind a shared
    /// cell — the same pattern as `quit`/`outbox`/`ui_clock`: the
    /// composition root takes a handle *before* the app seals into
    /// `AppService`/`UiRuntime`, so a `ViewHost` recreated mid-session
    /// still mints from the one allocator (a second allocator would
    /// restart the sequence and re-open the stale-receipt collision).
    /// Minted epochs are runtime guards only — the persisted identity
    /// is the `ViewInstanceId`, never the epoch.
    attach_epochs: Rc<RefCell<AttachEpochAllocator>>,
    /// Bundle the session was created or opened from — `reload` reopens it.
    bundle_path: Option<String>,
    /// Last published main-screen display state — shared with the
    /// composition root, refreshed after every service op (project
    /// length + arrangement rows here) and every UI pump
    /// (transport/inspector capture in `ui_ops`). The native lane
    /// polls it and pushes changes into the mounted spec through
    /// `update_view`.
    pub(crate) main_screen: Rc<RefCell<MainScreenState>>,
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
            attach_epochs: Rc::new(RefCell::new(AttachEpochAllocator::new())),
            bundle_path: None,
            main_screen: Rc::new(RefCell::new(MainScreenState::default())),
        }
    }

    /// The shared session `AttachEpoch` allocator — take a handle
    /// before the app seals (same extraction pattern as
    /// [`Self::quit_flag`]/[`crate::ui_ops::UiOpsState::event_outbox`]).
    /// Exactly one allocator exists per session, so every `ViewHost`
    /// — the first mount *and* any mid-session recreation — mints
    /// disjoint command-id namespaces.
    #[must_use]
    pub fn attach_epoch_allocator(&self) -> Rc<RefCell<AttachEpochAllocator>> {
        Rc::clone(&self.attach_epochs)
    }

    /// Mints the `AttachEpoch` for a `ViewHost` this session is
    /// creating (RF387). Convenience wrapper over the shared
    /// allocator — the sequence is monotone, so a host recreated for
    /// the same logical view mints commands in a disjoint namespace
    /// and stale receipts addressed to a dead host can never land on
    /// the live one.
    #[must_use]
    pub fn mint_attach_epoch(&self) -> AttachEpoch {
        self.attach_epochs.borrow_mut().mint()
    }

    /// The live project length in seconds — the Seek range source.
    #[must_use]
    pub fn project_length_seconds(&self) -> f64 {
        synvelis_workspace::viz_ui::project_length_seconds(
            self.workspace.session(),
            SEEK_MEDIA_RATE,
        )
    }

    /// The Arrangement table rows for the live project — the
    /// track/lane/clip read model behind the Arrangement tab
    /// (synvelis#112 slice 2).
    #[must_use]
    pub fn arrangement_rows(&self) -> Vec<synvelis_workspace::ArrangementRow> {
        synvelis_workspace::arrangement_rows(self.workspace.session(), SEEK_MEDIA_RATE)
    }

    /// Shared main-screen state cell. Taken before the app seals; the
    /// service refreshes the project-derived fields (length +
    /// arrangement rows) after every op and the UI pump refreshes the
    /// transport/inspector fields.
    #[must_use]
    pub fn main_screen_cell(&self) -> Rc<RefCell<MainScreenState>> {
        self.publish_project_length();
        Rc::clone(&self.main_screen)
    }

    /// Publishes the live project-derived screen state — project
    /// length (Seek range source) and Arrangement rows — into the
    /// shared cell. Called after every service op (the `AppService`
    /// call tail) and at cell hand-out; the transport/inspector
    /// fields of the same cell are refreshed by the UI pump's
    /// screen-state capture (`ui_ops`), not here. The name predates
    /// the arrangement rows; renaming is deferred to the next
    /// `service.rs` touch (synvelis#112 slice 2).
    pub fn publish_project_length(&self) {
        let mut screen = self.main_screen.borrow_mut();
        screen.project_length_seconds = self.project_length_seconds();
        screen.arrangement_rows = self.arrangement_rows();
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

    /// Attaches a [`SessionRoutingEngine`] to the current project
    /// session's [`EnginePort`](synvelis_workspace::EnginePort) —
    /// the headless session→routing connection (SF-03 slice 5).
    /// `project_new`/`project_open` recreate the session and detach
    /// the engine; re-attach afterwards with a config matching the
    /// new project's media root.
    pub fn attach_routing_engine(&mut self, engine: SessionRoutingEngine) {
        self.workspace.session_mut().set_engine(Box::new(engine));
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

    /// Connects the process-level logging install outcome (SYN-OBS-01)
    /// so `syn.app.info` reports it. Called once by the process entry
    /// point after `install` — never installs a subscriber itself.
    pub fn attach_logging(
        &mut self,
        state: InstallState,
        health: Option<LoggingHealth>,
        degraded: Vec<SinkDegraded>,
    ) {
        self.observability.attach_logging(state, health, degraded);
    }

    /// Marks quit requested; the host loop observes
    /// [`Self::quit_flag`]/[`Self::take_quit`] and calls
    /// `HostHandle::request_close`.
    pub fn request_quit(&mut self) {
        tracing::info!("syn.app.quit requested");
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
        // Non-RT boundary; paths stay out of event fields (sensitive-data
        // rule — the adapter never inspects field content).
        let result = self.project_new_inner(path, name);
        match &result {
            Ok(_) => tracing::info!("syn.project.new completed"),
            Err(error) => {
                tracing::warn!(error = %error, "syn.project.new failed");
            }
        }
        result
    }

    fn project_new_inner(&mut self, path: &str, name: Option<&str>) -> Result<Value, AppError> {
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
        self.bundle_path = Some(path.to_string());
        Ok(self.project_info())
    }

    /// Opens the project bundle at `path` — loads `HEAD` and rebuilds
    /// the workspace/session around the loaded project.
    ///
    /// # Errors
    /// [`AppError::Store`] when the bundle is missing, empty, or fails
    /// to load.
    pub fn project_open(&mut self, path: &str) -> Result<Value, AppError> {
        let result = self.project_open_inner(path);
        match &result {
            Ok(_) => tracing::info!("syn.project.open completed"),
            Err(error) => {
                tracing::warn!(error = %error, "syn.project.open failed");
            }
        }
        result
    }

    fn project_open_inner(&mut self, path: &str) -> Result<Value, AppError> {
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
        self.bundle_path = Some(path.to_string());
        Ok(self.project_info())
    }

    /// Reopens the bundle this session was created or opened from,
    /// discarding unsaved edits.
    ///
    /// # Errors
    /// [`AppError::Store`] when no bundle was opened yet or the reopen
    /// fails.
    pub fn project_reload(&mut self) -> Result<Value, AppError> {
        let path = self
            .bundle_path
            .clone()
            .ok_or_else(|| AppError::store("no project store — use syn.project.new/open first"))?;
        self.project_open(&path)
    }

    /// Saves the live project to the currently open bundle.
    ///
    /// # Errors
    /// [`AppError::Store`] when no project was opened/created yet or
    /// the write fails.
    pub fn project_save(&mut self) -> Result<Value, AppError> {
        let result = self.project_save_inner();
        match &result {
            Ok(_) => tracing::info!("syn.project.save completed"),
            Err(error) => {
                tracing::warn!(error = %error, "syn.project.save failed");
            }
        }
        result
    }

    fn project_save_inner(&mut self) -> Result<Value, AppError> {
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
            "logging": self.logging_info(),
        })
    }

    /// Logging section for `syn.app.info` — the process-level install
    /// state plus a bounded point-in-time sink snapshot. Never reaches
    /// the logging pipeline itself (the health handle is non-recursive).
    fn logging_info(&self) -> Value {
        let sinks: Vec<Value> = self
            .observability
            .logging_snapshot()
            .map(|snapshot| {
                snapshot
                    .sinks
                    .iter()
                    .map(|sink| {
                        json!({
                            "sink_index": sink.sink_index,
                            "kind": sink.kind.to_string(),
                            "state": sink.state.to_string(),
                            "events_written": sink.events_written,
                            "dropped_queue_full": sink.dropped_queue_full,
                            "partial_writes": sink.partial_writes,
                            "write_failures": sink.write_failures,
                            "flush_failures": sink.flush_failures,
                            "channel_disconnected": sink.channel_disconnected,
                            "shutdown_abandoned": sink.shutdown_abandoned,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        let degraded: Vec<Value> = self
            .observability
            .logging_degraded()
            .iter()
            .map(|entry| {
                json!({
                    "sink_index": entry.sink_index,
                    "kind": entry.kind.to_string(),
                    "reason": entry.reason,
                })
            })
            .collect();
        json!({
            "state": self.observability.logging_state().as_str(),
            "sinks": sinks,
            "degraded": degraded,
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
