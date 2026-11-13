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
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ruviform_media_core::Rounding;
use ruviform_observability::logging::{HealthSnapshot, LoggingHealth, SinkDegraded};
use ruviform_observability::source::{
    CounterDescriptor, CounterSource, MetricSource, ObservationScope, ObservationUnit,
    ReportingHint,
};
use ruviform_observability::{Counter, counter};
use ruviform_ui::BackendReport;
use ruviform_ui::ipc::{ErrorBody, codes};
use ruviform_ui::view::{AttachEpoch, AttachEpochAllocator};
use serde_json::{Value, json};

use synvelis_library::{GitVcsConfig, GitVcsProvider};
use synvelis_project::vcs::{
    ProjectPath, VcsError, VcsHistoryAdapter, VcsHistoryBackend, VcsReadProvider,
};
use synvelis_project::{
    ControlEntityRef, CounterIdGenerator, DirectoryBundleStore, ParameterTargetId,
    PpqnChangePreview, PpqnConversionPolicy, PpqnPreviewSite, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRevision, ProjectStore, RelationDirection, preview_ppqn,
};
use synvelis_workspace::{
    ProjectSession, SignalFlowRuntimeEvidence, SignalFlowView, SignalFlowViewOptions,
    WorkspaceManager, signal_flow_graph,
};

use crate::engine::SessionRoutingEngine;
use crate::error::AppError;
use crate::host::ShellWindowHost;
use crate::logging::InstallState;
use crate::revision::HOST_REVISION;
use crate::screens::{
    MainScreenState, RelationScreenState, SignalFlowScreenState, mixer_strip_targets,
};
use crate::ui_ops::UiOpsState;

/// Crate version baked at build time — the app revision reported to
/// the frontend.
const APP_REVISION: &str = env!("CARGO_PKG_VERSION");

/// Placeholder `ProjectId` for the boot project — replaced on
/// `project.new`/`project.open`; identity inside a loaded project is
/// whatever the store snapshot carries.
const BOOT_PROJECT_ID: u128 = 0;

/// Node/edge bounds for the app's Signal Flow projection — larger
/// than the read model's own defaults so ordinary projects display
/// untruncated; overflow still reports `Truncated` honestly.
pub(crate) const SIGNAL_FLOW_MAX_NODES: usize = 256;
pub(crate) const SIGNAL_FLOW_MAX_EDGES: usize = 512;

/// Media rate of the seconds axis the project length is measured on.
/// Only resolves clip spans to whole frames; the seconds value does not
/// depend on it for musical clips.
const SEEK_MEDIA_RATE: u32 = 48_000;

/// The VCS-facing project file inside a project bundle — a single
/// file holding the canonical [`synvelis_project::SnapshotEnvelope`]
/// bytes the store's HEAD names. The `syn.vcs.*` committed/staged
/// endpoints resolve this repo-relative path through the bound
/// [`VcsHistoryBackend`]; it is refreshed on every save/open so it
/// can never lag the saved HEAD (VCS-04 backend wiring, synvelis#95).
const VCS_PROJECT_FILE: &str = "main.json";

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
    /// `pub(crate)` so `ui_ops` can compose Saved-endpoint reads
    /// without borrowing all of `self`.
    pub(crate) store: Option<DirectoryBundleStore>,
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
    /// Cloneable handle to the attached [`SessionRoutingEngine`] — the
    /// session owns one boxed `EnginePort`; this handle lets the app
    /// compose engine evidence (receipt/source map/probe bindings)
    /// for Signal Flow status without touching the session (SF-05).
    /// `None` until [`Self::attach_routing_engine`]; cleared when the
    /// session is recreated by `project.new`/`project.open`.
    /// `pub(crate)` so `ui_ops` can reach the probe bridge for the
    /// `syn.probe.*` ops and the `ui_pump` drain hook (SF-04 slice 4).
    pub(crate) routing_engine: Option<SessionRoutingEngine>,
    /// Composition-registered plugin→host report drain handle
    /// (PC-C4 drain slice, synvelis#101) — `ui_pump` and
    /// [`Self::drain_control_reports`] drive one
    /// `drain_output_reports` pass per call through it. `None` until
    /// [`Self::set_control_report_drain`]; cleared with
    /// `routing_engine` on project recreate so a stale host cell is
    /// never drained. `pub(crate)` so `ui_ops` can reach it.
    pub(crate) control_report_drain: Option<synvelis_player::PluginReportDrain>,
    /// Last published main-screen display state — shared with the
    /// composition root, refreshed after every service op (project
    /// length + arrangement rows here) and every UI pump
    /// (transport/inspector capture in `ui_ops`). The native lane
    /// polls it and pushes changes into the mounted spec through
    /// `update_view`.
    pub(crate) main_screen: Rc<RefCell<MainScreenState>>,
    /// VCS history backend the `syn.vcs.*` ops compose into
    /// [`synvelis_workspace::VcsSession`] (VCS-04, synvelis#95).
    /// `None` reports `NoBackend` — Live/Saved endpoints still
    /// resolve. Bound per project root by
    /// [`Self::bind_vcs_backend`] on every `project.new`/`open` —
    /// a backend detected for a previous project root is never kept.
    /// `pub(crate)` so `ui_ops` can compose the `VcsSession` context
    /// disjoint from `workspace`.
    pub(crate) vcs_backend: Option<Arc<dyn VcsHistoryBackend>>,
    /// RCD-03 Arm/Monitoring UX state (synvelis#117 slice) —
    /// session-scope monitor declarations, the active monitor
    /// session (preflight-frozen plan + transport/punch marks) and
    /// the latency injection point. Never persisted; reset when the
    /// project session is recreated. `pub(crate)` so `ui_ops` can
    /// drive the `syn.monitoring.*` ops.
    pub(crate) monitoring: crate::monitoring::MonitorUxState,
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
            routing_engine: None,
            control_report_drain: None,
            main_screen: Rc::new(RefCell::new(MainScreenState::default())),
            vcs_backend: None,
            monitoring: crate::monitoring::MonitorUxState::default(),
        }
    }

    /// Injects the VCS history backend `syn.vcs.*` ops use (VCS-04).
    /// Pass an `Arc` so tests/composition roots can keep a handle on
    /// the same provider the app drives.
    pub fn set_vcs_backend(&mut self, backend: Arc<dyn VcsHistoryBackend>) {
        self.vcs_backend = Some(backend);
    }

    /// The injected VCS history backend, when present.
    #[must_use]
    pub fn vcs_backend(&self) -> Option<&dyn VcsHistoryBackend> {
        self.vcs_backend.as_deref()
    }

    /// Rebinds the production VCS history backend for the project
    /// bundle at `bundle_path` (VCS-04 backend wiring, synvelis#95).
    ///
    /// Detects the git workspace containing the bundle, opens a
    /// [`GitVcsProvider`] bound to the bundle directory as
    /// `project_root`, and adapts it into the
    /// [`VcsHistoryBackend`] the `syn.vcs.*` ops consume. The
    /// committed/staged endpoints resolve
    /// `<project_root>/main.json` — the project file
    /// [`Self::refresh_project_file`] keeps in lockstep with the
    /// saved HEAD.
    ///
    /// Every call re-detects: a non-repository bundle, a detection
    /// failure, or a provider/bind error clears `vcs_backend` to
    /// `None` so ops report `NoBackend` honestly — a backend minted
    /// for a different project root is never kept. Binding is
    /// infallible by contract; failures are logged, never propagated
    /// (a missing VCS is a degraded mode, not a project error).
    fn bind_vcs_backend(&mut self, bundle_path: &str) {
        self.vcs_backend = None;
        let detected = (|| -> Result<Arc<dyn VcsHistoryBackend>, VcsError> {
            let root = ProjectPath::new(bundle_path)?;
            let info = GitVcsProvider::detect_workspace(&root)?;
            let provider = GitVcsProvider::open(&GitVcsConfig::new(bundle_path))?;
            let project_file = if info.project_root.is_empty() {
                VCS_PROJECT_FILE.to_owned()
            } else {
                format!("{}/{VCS_PROJECT_FILE}", info.project_root)
            };
            let adapter = VcsHistoryAdapter::new(provider, &info, project_file)?;
            let backend: Arc<dyn VcsHistoryBackend> = Arc::new(adapter);
            Ok(backend)
        })();
        match detected {
            Ok(backend) => self.vcs_backend = Some(backend),
            Err(error) => {
                tracing::warn!(error = %error, "vcs backend bind failed — syn.vcs.* reports no-backend");
            }
        }
    }

    /// Rewrites `<bundle>/main.json` with the exact bytes the store's
    /// HEAD names (VCS-04 backend wiring). Called after every
    /// successful `project.new`/`open`/`save` so the VCS-facing
    /// project file can never lag the saved HEAD — a stale file would
    /// let committed endpoints resolve pre-save content and fake
    /// equality against the Saved endpoint.
    ///
    /// # Errors
    /// [`AppError::Store`] on store read or publish failures.
    fn refresh_project_file(store: &dyn ProjectStore, bundle_path: &str) -> Result<(), AppError> {
        let Some(head) = store.head().map_err(|e| AppError::store(e.to_string()))? else {
            return Ok(());
        };
        let bytes = store
            .read_snapshot(&head)
            .map_err(|e| AppError::store(e.to_string()))?;
        let dir = Path::new(bundle_path);
        let tmp = dir.join(format!("{VCS_PROJECT_FILE}.tmp"));
        let target = dir.join(VCS_PROJECT_FILE);
        fs::write(&tmp, &bytes)
            .map_err(|e| AppError::store(format!("write {}: {e}", tmp.display())))?;
        fs::rename(&tmp, &target)
            .map_err(|e| AppError::store(format!("publish {}: {e}", target.display())))?;
        Ok(())
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

    /// The subscribed parameter's control-relation neighborhood — the
    /// data behind the Inspector's relation table (synvelis#102
    /// UI-C5-01). `None` until the UI pump's inspector capture has
    /// recorded a target (or the tagged id fails to parse) — no
    /// anchor means no query ran, which is not the same as "zero
    /// relations".
    #[must_use]
    pub fn relation_rows(&self) -> Option<synvelis_workspace::RelationView> {
        Self::relation_view(
            self.workspace.session(),
            self.main_screen.borrow().inspector.target.as_deref(),
        )
    }

    /// Resolves the inspector's subscribed parameter target (tagged
    /// id) into a [`ControlEntityRef::Parameter`] anchor and queries
    /// its relation neighborhood in both directions. Shared by
    /// [`Self::relation_rows`] and the `publish_project_length`
    /// refresh so both paths resolve the anchor identically — the
    /// accessor reflects the live project while the published cell
    /// reflects the last refresh point.
    fn relation_view(
        session: &ProjectSession,
        anchor: Option<&str>,
    ) -> Option<synvelis_workspace::RelationView> {
        let anchor = anchor?;
        let entity = ControlEntityRef::Parameter(ParameterTargetId::from_tagged(anchor).ok()?);
        Some(synvelis_workspace::relation_rows(
            session,
            entity,
            RelationDirection::Both,
        ))
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
    /// length (Seek range source), Arrangement rows, and the
    /// subscribed parameter's relation rows — into the shared cell.
    /// Called after every service op (the `AppService` call tail) and
    /// at cell hand-out; the transport/inspector fields of the same
    /// cell are refreshed by the UI pump's screen-state capture
    /// (`ui_ops`), not here. The name predates the arrangement rows;
    /// renaming is deferred to the next `service.rs` touch
    /// (synvelis#112 slice 2).
    pub fn publish_project_length(&self) {
        let mut screen = self.main_screen.borrow_mut();
        screen.project_length_seconds = self.project_length_seconds();
        screen.arrangement_rows = self.arrangement_rows();
        let anchor = screen.inspector.target.clone();
        screen.relations = Self::relation_view(self.workspace.session(), anchor.as_deref())
            .map_or_else(RelationScreenState::default, RelationScreenState::from_view);
        screen.signal_flow = SignalFlowScreenState::from_view(
            self.signal_flow_view(&crate::screens::main_view_id()),
        );
        screen.mixer = mixer_strip_targets(&self.workspace.session().project().control);
        let live_ppqn = self.workspace.session().project().ppqn.0;
        // Re-sync the view-owned PPQN gesture against the live
        // project — the single point that keeps it honest across
        // every mutation path (ops, undo/redo, project new/open,
        // fixture-level session applies). A changed PPQN invalidates
        // any pending selection: its preview ran against the old
        // grid.
        self.ui.ppqn_ui.borrow_mut().observe_current(live_ppqn);
        let revision = self.workspace.session().revision().0;
        let gesture = self.ui.ppqn_ui.borrow();
        let (pending_new_ppqn, pending_policy) = gesture.pending().map_or((None, None), |p| {
            let (policy, _) = synvelis_workspace::state::ppqn_policy_wire(p.policy);
            (Some(p.new_ppqn), Some(policy))
        });
        screen.ppqn = crate::screens::PpqnScreenState {
            current_ppqn: live_ppqn,
            pending_new_ppqn,
            pending_policy,
            // Only a pending gesture's preview renders — the surface
            // exists to arm confirm, and the revision guard drops a
            // report any project mutation has already staled (the
            // confirm's `expected_revision` CAS would reject it
            // honestly anyway).
            preview: gesture
                .pending()
                .and_then(|pending| pending.preview.clone())
                .filter(|p| p.project_revision == revision),
        };
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
    /// new project's media root. An engine swap invalidates the
    /// registered [`PluginReportDrain`](synvelis_player::PluginReportDrain)
    /// pairing and its recorded evidence — the new engine's
    /// processor host may wrap a different `PluginHost` cell, so the
    /// composition root must re-register a paired drain
    /// ([`Self::set_control_report_drain`]; `compose_routing_engine`
    /// does, after attach).
    pub fn attach_routing_engine(&mut self, engine: SessionRoutingEngine) {
        self.routing_engine = Some(engine.clone());
        self.control_report_drain = None;
        self.ui.last_control_drain = synvelis_player::ReportDrainReport::default();
        self.workspace.session_mut().set_engine(Box::new(engine));
    }

    /// Registers the composition root's plugin→host report drain —
    /// `ui_pump` and [`Self::drain_control_reports`] drive one
    /// `drain_output_reports` pass per call through it (PC-C4 drain
    /// slice, synvelis#101). The handle wraps the same shared
    /// `PluginHost` cell the attached engine's processor host vends;
    /// pairing them is the composition root's job
    /// (`compose_routing_engine`). Cleared on project recreate with
    /// `routing_engine`.
    pub fn set_control_report_drain(&mut self, drain: synvelis_player::PluginReportDrain) {
        self.control_report_drain = Some(drain);
    }

    /// The Signal Flow projection for `view` — the per-view persisted
    /// [`synvelis_workspace::SignalFlowVisualState`] inside the
    /// workspace [`ViewState`] (default when the view never ran a
    /// visual op or is not dock-registered) supplies
    /// scope/role/collapse options; engine evidence is composed from
    /// the attached [`SessionRoutingEngine`] when present (SF-05
    /// slice 1, synvelis#108). Read-only: no session/project/engine
    /// state is touched.
    #[must_use]
    pub fn signal_flow_view(&self, view: &str) -> SignalFlowView {
        let options = crate::ui_ops::parse_view_instance(view)
            .and_then(|inst| self.workspace.state(inst).ok())
            .and_then(|state| state.signal_flow.as_ref())
            .map_or_else(SignalFlowViewOptions::default, |sf| {
                sf.visual
                    .view_options(SIGNAL_FLOW_MAX_NODES, SIGNAL_FLOW_MAX_EDGES)
            });
        let evidence = self.signal_flow_runtime_evidence();
        signal_flow_graph(self.workspace.session(), &options, evidence.as_ref())
    }

    /// Engine-side evidence snapshot for one Signal Flow query —
    /// `None` when no [`SessionRoutingEngine`] is attached (the
    /// projection then reports `engine_attached = false` honestly).
    pub(crate) fn signal_flow_runtime_evidence(&self) -> Option<SignalFlowRuntimeEvidence> {
        let engine = self.routing_engine.as_ref()?;
        Some(SignalFlowRuntimeEvidence {
            receipt: engine.receipt(),
            source_map: engine.source_map(),
            probes: engine.probe_bindings(),
        })
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
        self.project_new_with_ppqn(path, name, None)
    }

    /// `syn.project.new` with an optional initial PPQN — the wire
    /// `params.ppqn` (`1..=32767`; `None` keeps the 960 default).
    ///
    /// The value goes through [`ProjectOp::SetProjectPpqn`] before
    /// the initial save so the selectable-range validation and the
    /// `ppqn`/tempo-map invariant stay authoritative in
    /// `synvelis-project` — this method never duplicates them. The
    /// fresh project is empty, so the conversion is always exact;
    /// `Exact` is the strictest policy and needs no rounding param.
    ///
    /// # Errors
    /// [`AppError::Store`] on bundle-creation, PPQN, or save failures.
    pub fn project_new_with_ppqn(
        &mut self,
        path: &str,
        name: Option<&str>,
        ppqn: Option<u32>,
    ) -> Result<Value, AppError> {
        // Non-RT boundary; paths stay out of event fields (sensitive-data
        // rule — the adapter never inspects field content).
        let result = self.project_new_inner(path, name, ppqn);
        match &result {
            Ok(_) => tracing::info!("syn.project.new completed"),
            Err(error) => {
                tracing::warn!(error = %error, "syn.project.new failed");
            }
        }
        result
    }

    fn project_new_inner(
        &mut self,
        path: &str,
        name: Option<&str>,
        ppqn: Option<u32>,
    ) -> Result<Value, AppError> {
        let mut workspace = WorkspaceManager::new(
            ShellWindowHost::new(),
            Self::fresh_session(name.unwrap_or("untitled")),
        );
        // The PPQN is applied before the bundle is created so an
        // out-of-range value fails without leaving a stray directory.
        if let Some(ppqn) = ppqn {
            let session = workspace.session_mut();
            let old_ppqn = session.project().ppqn.0;
            if ppqn != old_ppqn {
                let revision = session.revision();
                session
                    .apply(
                        ProjectOp::SetProjectPpqn {
                            old_ppqn,
                            new_ppqn: ppqn,
                            policy: PpqnConversionPolicy::Exact,
                        },
                        revision,
                    )
                    .map_err(|e| AppError::store(format!("syn.project.new ppqn: {e}")))?;
            }
        }
        let mut store =
            DirectoryBundleStore::create(path).map_err(|e| AppError::store(e.to_string()))?;
        workspace
            .session_mut()
            .controller_mut()
            .save(&mut store)
            .map_err(|e| AppError::store(e.to_string()))?;
        Self::refresh_project_file(&store, path)?;
        self.workspace = workspace;
        self.store = Some(store);
        self.bundle_path = Some(path.to_string());
        self.routing_engine = None;
        self.control_report_drain = None;
        // Drain evidence is session-scoped — a report recorded
        // against the destroyed engine's surface must never surface
        // beside `engine: false` on `syn.control.status`.
        self.ui.last_control_drain = synvelis_player::ReportDrainReport::default();
        // A PPQN preview ran against the previous project — never
        // carry it into the new session. `committed` clears any
        // pending selection even when the new project's PPQN equals
        // the old one (its preview described a different project).
        let fresh_ppqn = self.workspace.session().project().ppqn.0;
        self.ui.ppqn_ui.borrow_mut().committed(fresh_ppqn);
        // authored monitor 宣言・punch は `project.monitoring` を
        // read-through するため新 project の空状態がそのまま
        // 反映される。ここで reset するのは session 観測値のみ。
        self.monitoring = crate::monitoring::MonitorUxState::default();
        self.bind_vcs_backend(path);
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
        Self::refresh_project_file(&store, path)?;
        self.workspace = workspace;
        self.store = Some(store);
        self.bundle_path = Some(path.to_string());
        self.routing_engine = None;
        self.control_report_drain = None;
        // Same session-scoped evidence rule as `project_new_inner`.
        self.ui.last_control_drain = synvelis_player::ReportDrainReport::default();
        // Same stale-gesture rule as `project_new_inner`.
        let fresh_ppqn = self.workspace.session().project().ppqn.0;
        self.ui.ppqn_ui.borrow_mut().committed(fresh_ppqn);
        // authored monitor 宣言・punch は load 済み `project.monitoring`
        // から read-through で再水和される。ここで reset するのは
        // session 観測値（稼働 session・latency 注入・観測 overlay）
        // のみ — これらは永続化対象外。
        self.monitoring = crate::monitoring::MonitorUxState::default();
        self.bind_vcs_backend(path);
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
        let bundle_path = self
            .bundle_path
            .clone()
            .ok_or_else(|| AppError::store("no project store — use syn.project.new/open first"))?;
        Self::refresh_project_file(store, &bundle_path)?;
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
            "ppqn": project.ppqn.0,
            "saved": self.workspace.session().controller().saved().is_some(),
            "store": self.store.is_some(),
        })
    }

    /// `syn.project.set_ppqn` — changes the project PPQN through
    /// [`ProjectOp::SetProjectPpqn`] (TIM-01 app slice, synvelis#125).
    ///
    /// The app layer owns the wire mapping and the session
    /// preconditions the project layer cannot see: transport stopped,
    /// no open recording transaction (merge-commit in flight), and
    /// automation record disabled. The `old_ppqn` CAS, the
    /// `expected_revision` CAS, the `1..=32767` range, and musical
    /// re-projection stay authoritative in `synvelis-project` — this
    /// method never duplicates them.
    ///
    /// # Errors
    /// [`ErrorBody`] with `codes::INVALID_PARAMS` on malformed params,
    /// a violated precondition, or a project-layer rejection — the
    /// message carries the reason either way.
    pub fn project_set_ppqn(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let old_ppqn = ppqn_param(params, "old_ppqn")?;
        let new_ppqn = ppqn_param(params, "new_ppqn")?;
        let policy = ppqn_policy(params)?;
        let expected_revision = match params.get("expected_revision") {
            None | Some(Value::Null) => self.workspace.session().revision(),
            Some(v) => ProjectRevision(
                v.as_u64()
                    .ok_or_else(|| invalid_params("params.expected_revision (u64) required"))?,
            ),
        };
        {
            let session = self.workspace.session();
            if session.transport().playing {
                return Err(invalid_params(
                    "transport is playing — stop transport before changing project ppqn",
                ));
            }
            if session.control().recording().is_some() {
                return Err(invalid_params(
                    "recording transaction open — commit or abort it before changing project ppqn",
                ));
            }
            if session.control().record_enabled() {
                return Err(invalid_params(
                    "automation record is enabled — disable it before changing project ppqn",
                ));
            }
        }
        self.workspace
            .session_mut()
            .apply(
                ProjectOp::SetProjectPpqn {
                    old_ppqn,
                    new_ppqn,
                    policy,
                },
                expected_revision,
            )
            .map_err(|e| invalid_params(format!("syn.project.set_ppqn: {e}")))?;
        // The committed change consumes any pending PPQN gesture —
        // the preview it armed described the pre-change grid, and a
        // gesture for a different candidate is stale by definition.
        let applied = self.workspace.session().project().ppqn.0;
        self.ui.ppqn_ui.borrow_mut().committed(applied);
        Ok(self.project_info())
    }

    /// `syn.project.preview_ppqn` — non-destructive dry-run of a
    /// PPQN change (TIM-01 preview slice, synvelis#125).
    ///
    /// Read-only by contract: no session gate applies — transport,
    /// recording, and automation state cannot be affected by a
    /// preview, so the op answers even while `syn.project.set_ppqn`
    /// would be rejected. The project-layer [`preview_ppqn`] walks
    /// the same site enumeration the mutating re-projection runs,
    /// so the report can never drift from what the command would
    /// do.
    ///
    /// # Errors
    /// [`ErrorBody`] with `codes::INVALID_PARAMS` on malformed params
    /// or an out-of-range `new_ppqn`.
    pub fn project_preview_ppqn(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let new_ppqn = ppqn_param(params, "new_ppqn")?;
        let policy = ppqn_policy(params)?;
        let preview = preview_ppqn(self.workspace.session().project(), new_ppqn, policy)
            .map_err(|e| invalid_params(format!("syn.project.preview_ppqn: {e}")))?;
        // Feed the view-owned gesture: a preview matching the pending
        // candidate arms confirm; a foreign or mismatched one leaves
        // the selection untouched, and a bare preview (no pending
        // selection) records nothing. Re-sync first so a mutation
        // that bypassed the gesture cannot leave `current_ppqn`
        // stale enough to reject an honest preview.
        let revision = self.workspace.session().revision().0;
        let live_ppqn = self.workspace.session().project().ppqn.0;
        self.ui.ppqn_ui.borrow_mut().observe_current(live_ppqn);
        let _ = self
            .ui
            .ppqn_ui
            .borrow_mut()
            .record_preview(&preview, revision);
        Ok(ppqn_preview_json(&preview))
    }

    /// Recording-state projection for `syn.recording.info` — the
    /// persisted `Project.recording` entities plus recorder status
    /// (synvelis#115 RCD-01 headless slice; read-only).
    #[must_use]
    pub fn recording_info(&self) -> Value {
        crate::recording::recording_info(self.workspace.session(), &self.recorder)
    }

    /// Guard reasons for the window-close decision
    /// (`syn.window.close_guard`). App-global by contract: the
    /// project/session is shared by every window, so the reasons
    /// never depend on which window asks — the shell composes them
    /// with window topology and applies them only to the last live
    /// window's close.
    #[must_use]
    pub fn close_veto_reasons(&self) -> Vec<&'static str> {
        let mut reasons = Vec::new();
        let session = self.workspace.session();
        let controller = session.controller();
        let live = controller.live_revision();
        let dirty = match controller.saved() {
            Some(saved) => live != saved.revision,
            // Never saved: only edits past revision 0 are unsaved
            // work — a fresh boot project carries nothing to lose.
            None => live.0 > 0,
        };
        if dirty {
            reasons.push("unsaved_changes");
        }
        if session.control().recording().is_some() {
            reasons.push("recording_active");
        }
        if self.recorder.active() {
            reasons.push("recorder_active");
        }
        reasons
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
            "signal_flow": {
                "state": "available",
                "scope": "headless read model + visual ops (SF-05 slice 1); canvas deferred",
            },
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

// -- `syn.project.set_ppqn` wire helpers -----------------------------------

fn invalid_params(message: impl Into<String>) -> ErrorBody {
    ErrorBody::new(codes::INVALID_PARAMS, message)
}

fn ppqn_param(params: &Value, key: &str) -> Result<u32, ErrorBody> {
    let value = params
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_params(format!("params.{key} (u64) required")))?;
    u32::try_from(value).map_err(|_| invalid_params(format!("params.{key} exceeds u32 range")))
}

/// Maps the wire `policy`/`rounding` strings onto
/// [`PpqnConversionPolicy`]. The policy is required command input —
/// a PPQN change can never silently quantize a stored musical
/// position — and `round` requires an explicit ruviform rounding.
fn ppqn_policy(params: &Value) -> Result<PpqnConversionPolicy, ErrorBody> {
    let policy = params
        .get("policy")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_params("params.policy (string) required"))?;
    let rounding = match params.get("rounding") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            v.as_str()
                .ok_or_else(|| invalid_params("params.rounding (string) required"))?,
        ),
    };
    match (policy, rounding) {
        ("exact", None) => Ok(PpqnConversionPolicy::Exact),
        ("exact", Some(_)) => Err(invalid_params(
            "params.rounding is only valid with policy \"round\"",
        )),
        ("round", Some(r)) => {
            let rounding = match r {
                "floor" => Rounding::Floor,
                "ceil" => Rounding::Ceil,
                "toward_zero" => Rounding::TowardZero,
                "away_from_zero" => Rounding::AwayFromZero,
                "nearest_even" => Rounding::NearestEven,
                _ => {
                    return Err(invalid_params(format!(
                        "params.rounding: unknown rounding {r:?}"
                    )));
                }
            };
            Ok(PpqnConversionPolicy::Round(rounding))
        }
        ("round", None) => Err(invalid_params(
            "params.rounding required when policy is \"round\"",
        )),
        (other, _) => Err(invalid_params(format!(
            "params.policy: unknown policy {other:?}"
        ))),
    }
}

/// Wire name for a [`PpqnConversionPolicy`] — delegates to the
/// workspace read model so op JSON and the UI projection share one
/// vocabulary.
fn ppqn_policy_name(policy: PpqnConversionPolicy) -> &'static str {
    synvelis_workspace::state::ppqn_policy_name(policy)
}

/// Wire name for a ruviform [`Rounding`] — delegates to the
/// workspace read model (see [`ppqn_policy_name`]).
fn rounding_name(rounding: Rounding) -> &'static str {
    synvelis_workspace::state::ppqn_rounding_name(rounding)
}

fn ppqn_site_json(site: &PpqnPreviewSite) -> Value {
    json!({
        "kind": site.kind,
        "instance": site.instance,
        "part": site.part,
        "tick": site.tick,
        "projected": site.projected,
    })
}

/// JSON shape of [`synvelis_project::PpqnChangePreview`] — the
/// read-only report `syn.project.preview_ppqn` returns. `effects`
/// carries the informational surfaces: the decided Snap/Grid policy
/// (explicit quanta are fixed project ticks, preserved across the
/// change — only musical spacing moves, quantified for the shipped
/// default quantum) and the SMF-export division note.
fn ppqn_preview_json(preview: &PpqnChangePreview) -> Value {
    let snap_effect = synvelis_workspace::snap::snap_unit_ppqn_effect(
        synvelis_workspace::SnapUnit::Explicit(
            synvelis_workspace::snap::DEFAULT_SNAP_QUANTUM_TICKS,
        ),
        preview.old_ppqn,
        preview.new_ppqn,
    );
    json!({
        "old_ppqn": preview.old_ppqn,
        "new_ppqn": preview.new_ppqn,
        "policy": ppqn_policy_name(preview.policy),
        "rounding": match preview.policy {
            PpqnConversionPolicy::Exact => Value::Null,
            PpqnConversionPolicy::Round(r) => json!(rounding_name(r)),
        },
        "site_count": preview.site_count,
        "inexact": preview.inexact.iter().map(ppqn_site_json).collect::<Vec<_>>(),
        "overflow_sites": preview.overflow_sites.iter().map(ppqn_site_json).collect::<Vec<_>>(),
        "exact_ok": preview.exact_ok,
        "round_ok": preview.round_ok,
        "applies": preview.applies,
        "rejection": preview.rejection,
        "effects": {
            "snap_grid": format!(
                "explicit snap/grid quanta are fixed project ticks and are preserved across the ppqn change; only musical spacing moves — default {}",
                snap_effect.describe(),
            ),
            "smf_export": "export division stays an explicit option inside the same 1..=32767 SMF PPQ range",
        },
    })
}
