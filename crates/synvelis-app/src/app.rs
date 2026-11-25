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

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
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
use ruviform_ui::dock::ViewInstanceId;
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
    ProjectId, ProjectOp, ProjectRevision, ProjectStore, RelationDirection, SmfError,
    SmfExportOptions, SmfFormat, SourceId, export_smf, parse_smf, preview_ppqn,
};
use synvelis_workspace::{
    ProjectSession, SignalFlowRuntimeEvidence, SignalFlowView, SignalFlowViewOptions, VcsSession,
    ViewType, WorkspaceManager, signal_flow_graph, vcs_history_view,
};

use crate::engine::SessionRoutingEngine;
use crate::error::AppError;
use crate::host::ShellWindowHost;
use crate::logging::InstallState;
use crate::plugin_dock::{PluginDock, PluginEditorHost};
use crate::revision::HOST_REVISION;
use crate::screens::{
    MainScreenState, RelationScreenState, SignalFlowScreenState, VcsScreenState,
    mixer_strip_targets,
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

/// Recorder session owner — the `syn.recorder.*` lifecycle state
/// (synvelis#114 SV-GUI-05 Option B). Defined in
/// [`crate::recording::RecorderOwner`]; re-exported here under the
/// historical `RecorderSlot` name so the public path
/// `synvelis_app::RecorderSlot` stays stable.
pub use crate::recording::RecorderOwner as RecorderSlot;

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
    /// Recorder session owner (`syn.recorder.*` の lifecycle 状態)。
    /// `pub(crate)` so `ui_ops` can drive the ops disjoint from
    /// `workspace`.
    pub(crate) recorder: crate::recording::RecorderOwner,
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
    /// `pub(crate)` so `ui_ops` can root the Difference Explorer's
    /// variant content store at the live bundle (synvelis#286 CB-09).
    pub(crate) bundle_path: Option<String>,
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
    /// Last published VCS History screen state per adopted
    /// [`ViewType::VcsHistory`] view — the VCS counterpart of
    /// [`Self::main_screen`], refreshed by the same post-op publish
    /// hook (`publish_project_length`, which the service tail runs
    /// after every op) so Live-endpoint freshness — derived from the
    /// project revision — stays honest. The native lane polls it and
    /// pushes `history_screen_spec` rebuilds into mounted VCS view
    /// windows through `update_view`.
    pub(crate) vcs_screens: Rc<RefCell<BTreeMap<ViewInstanceId, VcsScreenState>>>,
    /// RCD-03 Arm/Monitoring UX state (synvelis#117 slice) —
    /// session-scope monitor declarations, the active monitor
    /// session (preflight-frozen plan + transport/punch marks) and
    /// the latency injection point. Never persisted; reset when the
    /// project session is recreated. `pub(crate)` so `ui_ops` can
    /// drive the `syn.monitoring.*` ops.
    pub(crate) monitoring: crate::monitoring::MonitorUxState,
    /// AUTX-07 (synvelis#266) authority-pushed automation catalog —
    /// the opaque [`synvelis_workspace::AutomationCatalog`] the
    /// `syn.autx.catalog` op installs and the palette/macro
    /// projections plus request lowering read. `None` until an
    /// authority pushes one: the surfaces render their honest
    /// empty states and request actions reject typed. Held opaquely
    /// — `synvelis-app` never names `synvelis-gateway` types (AC-05).
    pub(crate) automation_catalog: Option<synvelis_workspace::AutomationCatalog>,
    /// Monotonic mint for `autx.*` lowering request ids — correlation
    /// only; the Gateway assigns the authoritative operation id.
    pub(crate) autx_request_seq: u64,
    /// Last published automation screen state per adopted automation
    /// view (`ViewType::{ActionPalette,MacroBrowser,WorkflowEditor,
    /// RunMonitor}`), keyed by `ViewInstanceId` — the AUTX counterpart
    /// of [`Self::vcs_screens`]. Refreshed by the same post-op publish
    /// hook so view-local `autx.*` mutations and catalog pushes both
    /// reach mounted specs. Entries carry a publish `rev` stamp —
    /// the projection types have no `PartialEq`, so the composition
    /// root's push loop compares stamps.
    pub(crate) automation_screens:
        Rc<RefCell<BTreeMap<ViewInstanceId, crate::automation::AutomationScreenPublish>>>,
    /// Publish counter stamped into every
    /// [`crate::automation::AutomationScreenPublish`] — `Cell` because
    /// [`Self::publish_automation_screens`] runs under `&self`.
    pub(crate) autx_publish_rev: Cell<u64>,
    /// Verified-import backends the `syn.library.*` ops compose into
    /// [`synvelis_workspace::LibraryImportPorts`] (ALIB-UX-H,
    /// synvelis#89): the durable [`synvelis_library::ImportStore`],
    /// the project blob read port, and the blob publisher. `None`
    /// until [`Self::set_library_import`] — import actions then land
    /// the typed `Malformed`/`Unavailable` fault instead of touching
    /// the project. Browse/stat/pin/preview never need this: they run
    /// through the provider registry bound on the workspace's
    /// [`synvelis_workspace::LibraryViews`].
    pub(crate) library_import: Option<LibraryImportServices>,
    /// The provider registry + principal bound via
    /// [`Self::set_library_backend`], retained so
    /// `project.new`/`project.open`/`project.reload` — which replace
    /// the whole [`WorkspaceManager`] — can rebind it onto the fresh
    /// workspace's [`synvelis_workspace::LibraryViews`]. Without this
    /// every library op silently degrades to `unavailable` after any
    /// project switch.
    pub(crate) library_backend: Option<synvelis_workspace::LibraryBackend>,
}

/// The import-side backends `syn.library.*` import actions borrow for
/// one call — injected once by the composition root or a test fixture
/// (real `ImportStore`/`LocalBlobDir`/`StagedBlobPublisher` over a
/// temp root, or fault-injecting doubles).
#[derive(Clone)]
pub struct LibraryImportServices {
    /// Durable import manifest/staging store.
    pub store: synvelis_library::ImportStore,
    /// Project blob store read port.
    pub blobs: Arc<dyn synvelis_library::ProjectBlobStore>,
    /// Blob publisher write port.
    pub publisher: Arc<dyn synvelis_library::BlobPublisher>,
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
            recorder: crate::recording::RecorderOwner::default(),
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
            vcs_screens: Rc::new(RefCell::new(BTreeMap::new())),
            monitoring: crate::monitoring::MonitorUxState::default(),
            automation_catalog: None,
            autx_request_seq: 0,
            automation_screens: Rc::new(RefCell::new(BTreeMap::new())),
            autx_publish_rev: Cell::new(0),
            library_import: None,
            library_backend: None,
        }
    }

    /// Binds the provider registry + principal every lazily-created
    /// Asset Library view model shares (ALIB-UX-H, synvelis#89). Pass
    /// an [`Arc`](std::sync::Arc) registry so tests/composition roots
    /// keep a handle on the same providers the app drives — the
    /// `synvelis#91` real-provider wiring lands here. The binding is
    /// retained and re-applied whenever the workspace is rebuilt by
    /// `project.new`/`open`/`reload`.
    pub fn set_library_backend(&mut self, backend: synvelis_workspace::LibraryBackend) {
        self.workspace.library_mut().set_backend(backend.clone());
        self.library_backend = Some(backend);
    }

    /// Rebinds the retained [`Self::library_backend`] onto the current
    /// workspace — called after `project.new`/`open` replace the
    /// [`WorkspaceManager`], mirroring [`Self::bind_vcs_backend`].
    fn bind_library_backend(&mut self) {
        if let Some(backend) = &self.library_backend {
            self.workspace.library_mut().set_backend(backend.clone());
        }
    }

    /// Injects the verified-import backends `syn.library.*` import
    /// actions use — the durable [`synvelis_library::ImportStore`],
    /// blob read port, and blob publisher. Until bound, import actions
    /// land the typed fault and never touch the project.
    pub fn set_library_import(&mut self, services: LibraryImportServices) {
        self.library_import = Some(services);
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
        self.publish_vcs_screens();
        self.publish_automation_screens();
    }

    /// Shared automation screen-state cell map — one
    /// [`crate::automation::AutomationScreenState`] per registered
    /// automation view, keyed by its adopted [`ViewInstanceId`]. Same
    /// hand-out contract as [`Self::vcs_screen_cell`]: publishes
    /// first, then returns the shared handle the composition root
    /// keeps across the app seal.
    #[must_use]
    pub fn automation_screen_cell(
        &self,
    ) -> Rc<RefCell<BTreeMap<ViewInstanceId, crate::automation::AutomationScreenPublish>>> {
        self.publish_automation_screens();
        Rc::clone(&self.automation_screens)
    }

    /// Projects every registered automation view into its
    /// [`crate::automation::AutomationScreenState`] and stores it in
    /// [`Self::automation_screens`] — closed views drop out,
    /// non-automation registry entries are ignored. Runs inside
    /// [`Self::publish_project_length`] so `autx.*` view-local
    /// mutations, catalog pushes, and project replacements that
    /// re-render all refresh the mounted specs. The pushed catalog
    /// supplies the palette/macro projections; workflow plans and run
    /// monitors have no producer yet (the live adapter is #264), so
    /// those surfaces publish their honest empty states.
    fn publish_automation_screens(&self) {
        use crate::automation::AutomationScreenState as S;
        use synvelis_workspace::ViewType as V;
        let autx_ids: Vec<ViewInstanceId> = self
            .workspace
            .registry()
            .ids()
            .into_iter()
            .filter(|inst| {
                self.workspace
                    .registry()
                    .descriptor(*inst)
                    .is_some_and(|d| {
                        matches!(
                            d.kind,
                            V::ActionPalette | V::MacroBrowser | V::WorkflowEditor | V::RunMonitor
                        )
                    })
            })
            .collect();
        let rev = self.autx_publish_rev.get().wrapping_add(1);
        self.autx_publish_rev.set(rev);
        let mut screens = self.automation_screens.borrow_mut();
        screens.retain(|inst, _| autx_ids.contains(inst));
        for inst in autx_ids {
            let kind = self.workspace.registry().descriptor(inst).map(|d| d.kind);
            let view_state = self
                .workspace
                .state(inst)
                .ok()
                .and_then(|s| s.automation.clone())
                .unwrap_or_default();
            let screen = match kind {
                Some(V::ActionPalette) => S::Palette(crate::automation::ActionPaletteScreenState {
                    catalog: self
                        .automation_catalog
                        .as_ref()
                        .map(|c| c.action_catalog(&view_state.palette_query)),
                    view: view_state,
                }),
                Some(V::MacroBrowser) => S::Macro(crate::automation::MacroBrowserScreenState {
                    catalog: self
                        .automation_catalog
                        .as_ref()
                        .map(synvelis_workspace::AutomationCatalog::macro_browser),
                    view: view_state,
                }),
                Some(V::WorkflowEditor) => {
                    S::Workflow(crate::automation::WorkflowEditorScreenState {
                        plan: None,
                        view: view_state,
                    })
                }
                Some(V::RunMonitor) => S::Run(crate::automation::RunMonitorScreenState {
                    monitor: None,
                    view: view_state,
                }),
                _ => continue,
            };
            screens.insert(
                inst,
                crate::automation::AutomationScreenPublish { rev, state: screen },
            );
        }
    }

    /// Shared VCS History screen-state cell map — one
    /// [`VcsScreenState`] per registered [`ViewType::VcsHistory`]
    /// view, keyed by its adopted [`ViewInstanceId`]. Same hand-out
    /// contract as [`Self::main_screen_cell`]: publishes first, then
    /// returns the shared handle the composition root keeps across
    /// the app seal.
    #[must_use]
    pub fn vcs_screen_cell(&self) -> Rc<RefCell<BTreeMap<ViewInstanceId, VcsScreenState>>> {
        self.publish_vcs_screens();
        Rc::clone(&self.vcs_screens)
    }

    /// Projects every registered [`ViewType::VcsHistory`] view into
    /// [`VcsScreenState`] and stores it in [`Self::vcs_screens`] —
    /// closed views drop out, non-VCS registry entries are ignored.
    /// Runs inside [`Self::publish_project_length`] (the post-op
    /// publish hook the service tail calls after every op) so VCS
    /// ops *and* project mutations that stale Live endpoints both
    /// refresh the mounted specs. Pure read: `vcs_history_view`'s
    /// freshness probes are the only backend touches.
    fn publish_vcs_screens(&self) {
        let vcs_ids: Vec<ViewInstanceId> = self
            .workspace
            .registry()
            .ids()
            .into_iter()
            .filter(|inst| {
                self.workspace
                    .registry()
                    .descriptor(*inst)
                    .is_some_and(|d| d.kind == ViewType::VcsHistory)
            })
            .collect();
        let ctx = VcsSession {
            backend: self.vcs_backend.as_deref(),
            controller: self.workspace.session().controller(),
            store: self.store.as_ref().map(|s| -> &dyn ProjectStore { s }),
            blobs: None,
        };
        let mut screens = self.vcs_screens.borrow_mut();
        screens.retain(|inst, _| vcs_ids.contains(inst));
        for inst in vcs_ids {
            let view = vcs_history_view(self.workspace.vcs(), inst, &ctx);
            screens.insert(inst, VcsScreenState::from_view(view));
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

    /// Installs the [`PluginEditorHost`] seam the `syn.plugin_dock.*`
    /// ops borrow per command (SV-GPUI-02 headless slice,
    /// synvelis#176): the adapter resolves stable instance identities
    /// to live host handles and lends an `EditorSessionOps`
    /// implementation per call, redeeming opaque parent leases through
    /// the panel's own ledger. `None` (the default) reports
    /// `unavailable` honestly — no adapter means no editor, never a
    /// faked one.
    pub fn set_plugin_editor_host(&mut self, host: Box<dyn PluginEditorHost>) {
        self.ui.plugin_editor_host = Some(host);
    }

    /// The `PluginDock` panel record for a wire `view_id`, when the
    /// view hosts one — slot/session/instance wiring and the upstream
    /// state machine for tests and the debug console.
    #[must_use]
    pub fn plugin_dock_panel(&self, view_id: &str) -> Option<&PluginDock> {
        crate::ui_ops::parse_view_instance(view_id).and_then(|inst| self.ui.plugin_docks.get(&inst))
    }

    /// The `syn.plugin_dock.state` read model for `view_id`, when the
    /// view hosts a `PluginDock` panel — tests and the debug console
    /// read the same projection the op returns.
    #[must_use]
    pub fn plugin_dock_state(&self, view_id: &str) -> Option<serde_json::Value> {
        crate::ui_ops::parse_view_instance(view_id)
            .and_then(|inst| self.ui.plugin_docks.get(&inst))
            .map(|dock| crate::ui_ops::plugin_dock_status_json(dock, &self.ui.plugin_sessions))
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
        // The ledger sidecar is written before the workspace swap so a
        // reused bundle dir never keeps a foreign project's job rows —
        // the fresh workspace's (empty) ledger is the honest record.
        Self::persist_library_jobs(&workspace, &mut store)?;
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
        // 稼働 recorder session は旧 project の session/stream ID を
        // 参照するため破棄する（注入 device は host resource として
        // 保持される）。
        self.recorder.reset();
        // Difference Explorer の engine/session/publish 観測は
        // 旧 project の pin・receipt に紐づくため破棄する —
        // view id 空間も新 workspace で再採番される。
        self.ui.reset_variant_diff();
        self.bind_vcs_backend(path);
        self.bind_library_backend();
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
        // `project_new_inner` と同じく稼働 recorder session を破棄
        // する（注入 device は保持）。
        self.recorder.reset();
        // `project_new_inner` と同じく variant-diff の engine/
        // session/publish 観測を破棄する。
        self.ui.reset_variant_diff();
        self.bind_vcs_backend(path);
        self.bind_library_backend();
        // Restore the app-owned job ledger last — the sidecar read is
        // honest about failure (corrupt/unknown-format errors open
        // instead of silently dropping the receipts).
        self.restore_library_jobs()?;
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
        // The ledger sidecar rides the same save: HEAD and
        // `library-jobs.json` describe one point in time, and an
        // in-flight job is persisted as in-flight (recovery assigns
        // the typed terminal at open).
        Self::persist_library_jobs(&self.workspace, store)?;
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
        let expected_revision = expected_revision_param(self.workspace.session(), params)?;
        project_mutation_gate(self.workspace.session(), "changing project ppqn")?;
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

    /// `syn.project.import_smf` — imports a Standard MIDI File as a
    /// new MIDI source through [`ProjectOp::ImportMidiSmf`] (TIM-02
    /// app slice, synvelis#126).
    ///
    /// The app layer owns the wire mapping (`params.path` → file
    /// bytes → [`parse_smf`] → session apply) and the session
    /// preconditions shared with the other mutating project ops.
    /// Document validation and source/note identity minting stay in
    /// `synvelis-project`; SMF boundary failures (malformed /
    /// unsupported / overflow) and file IO failures arrive as typed
    /// [`ErrorBody`]s — the kind rides in `details`, never a silent
    /// partial import.
    ///
    /// # Errors
    /// [`ErrorBody`] with `codes::INVALID_PARAMS` on malformed params,
    /// a violated precondition, an SMF parse rejection, or a
    /// project-layer rejection; `codes::SERVICE_ERROR` when the file
    /// cannot be read.
    ///
    /// # Panics
    /// When a successful `ImportMidiSmf` apply mints no new source —
    /// an op-contract violation, unreachable by contract.
    pub fn project_import_smf(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let path = string_param(params, "path")?;
        let expected_revision = expected_revision_param(self.workspace.session(), params)?;
        project_mutation_gate(self.workspace.session(), "importing an SMF file")?;
        let bytes = fs::read(path).map_err(|e| smf_io_body("read", path, &e))?;
        let document = parse_smf(&bytes).map_err(|e| smf_error_body(&e))?;
        let summary = json!({
            "format": document.format,
            "division": document.division,
            "tracks": document.tracks.len(),
            "controllers": document.controller_events().len(),
            "bytes": bytes.len(),
        });
        let existing: std::collections::BTreeSet<SourceId> = self
            .workspace
            .session()
            .project()
            .midi_sources
            .keys()
            .copied()
            .collect();
        let revision = self
            .workspace
            .session_mut()
            .apply(ProjectOp::ImportMidiSmf { document }, expected_revision)
            .map_err(|e| invalid_params(format!("syn.project.import_smf: {e}")))?;
        let project = self.workspace.session().project();
        let source_id = project
            .midi_sources
            .keys()
            .find(|id| !existing.contains(id))
            .copied()
            .expect("ImportMidiSmf mints exactly one source");
        let notes = project
            .midi_sources
            .get(&source_id)
            .map_or(0, |source| source.notes.len());
        Ok(json!({
            "source_id": source_id.to_tagged(),
            "path": path,
            "notes": notes,
            "revision": revision.0,
            "import": summary,
        }))
    }

    /// `syn.project.export_smf` — exports one MIDI source to an SMF
    /// file through [`export_smf`] (TIM-02 app slice, synvelis#126).
    ///
    /// Read-only on the project: no mutation gate applies — the
    /// source/document is only read and the SMF bytes are written
    /// at this headless boundary. `params.format` (`"format0"`/
    /// `"format1"`) and `params.division` (PPQN `1..=32767`) are
    /// optional; omitted values default to the source's provenance
    /// document (or the source's own division / Format 0 when none
    /// exists). The reply carries the full
    /// [`synvelis_project::SmfConversionReport`] so rounding error
    /// is never silently dropped.
    ///
    /// # Errors
    /// [`ErrorBody`] with `codes::INVALID_PARAMS` on malformed params
    /// or an [`SmfError`] rejection (`NotFound`/`Unsupported`/
    /// `Malformed`/`Overflow` — the kind rides in `details`);
    /// `codes::SERVICE_ERROR` when the output file cannot be written.
    pub fn project_export_smf(&mut self, params: &Value) -> Result<Value, ErrorBody> {
        let source_id = SourceId::from_tagged(string_param(params, "source_id")?)
            .map_err(|e| invalid_params(format!("params.source_id: {e}")))?;
        let path = string_param(params, "path")?;
        let project = self.workspace.session().project();
        // Unknown source is `NotFound` — checked before the default
        // resolution so a missing source never degrades to a
        // misleading division-0 rejection.
        if !project.midi_sources.contains_key(&source_id) {
            return Err(smf_error_body(&SmfError::NotFound(format!(
                "midi source {source_id}"
            ))));
        }
        let (default_division, default_format) = project.smf_documents.get(&source_id).map_or_else(
            || {
                (
                    project
                        .midi_sources
                        .get(&source_id)
                        .map_or(0, |source| source.division.ticks_per_quarter()),
                    SmfFormat::Format0,
                )
            },
            |doc| (doc.division, doc.format),
        );
        let division = optional_u32_param(params, "division")?.unwrap_or(default_division);
        let format = match params.get("format") {
            None | Some(Value::Null) => default_format,
            Some(v) => match v
                .as_str()
                .ok_or_else(|| invalid_params("params.format (string) required"))?
            {
                "format0" => SmfFormat::Format0,
                "format1" => SmfFormat::Format1,
                other => {
                    return Err(invalid_params(format!(
                        "params.format: unknown format {other:?} (format0|format1)"
                    )));
                }
            },
        };
        let file = export_smf(project, &source_id, &SmfExportOptions { format, division })
            .map_err(|e| smf_error_body(&e))?;
        fs::write(path, &file.bytes).map_err(|e| smf_io_body("write", path, &e))?;
        let report = &file.report;
        Ok(json!({
            "source_id": source_id.to_tagged(),
            "path": path,
            "format": format,
            "division": division,
            "bytes": file.bytes.len(),
            "report": {
                "event_count": report.event_count,
                "exact_count": report.exact_count,
                "rounded_count": report.rounded_count,
                "max_error": { "num": report.max_error.num, "den": report.max_error.den },
                "unsupported_events": report.unsupported_events,
            },
        }))
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

    /// Injects the input devices `syn.recorder.*` binds at
    /// `syn.recorder.start` — the device-enumeration seam
    /// (synvelis#114 SV-GUI-05 Option B). Real `DeviceEnumerator`
    /// wiring is a later slice; headless callers inject fake
    /// `Device`s here. Devices are host resources, not project
    /// state — they survive `project.new`/`open` session resets.
    pub fn set_recorder_devices(
        &mut self,
        devices: Vec<Box<dyn ruviform_audio_core::device::Device>>,
    ) {
        self.recorder.set_devices(devices);
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

    /// `syn.app.capabilities` の `recorder` 節 — command 面は
    /// `syn.recorder.*` として利用可能だが、実際に録音を開始できるのは
    /// 入力 device が注入されている場合のみ。device 未注入は
    /// `unavailable` + reason で正直に報告し、session owner の
    /// 実態（`status`/`active`/`devices`）をそのまま投影する。
    fn recorder_capabilities(&self) -> Value {
        let devices = self.recorder.device_count();
        if devices == 0 {
            json!({
                "state": "unavailable",
                "reason": "no recorder input device injected",
                "status": self.recorder.status(),
                "active": self.recorder.active(),
                "devices": 0,
            })
        } else {
            json!({
                "state": "available",
                "status": self.recorder.status(),
                "active": self.recorder.active(),
                "devices": devices,
            })
        }
    }

    /// `syn.app.capabilities` の `midi` 節 (TIM-02, synvelis#126) —
    /// headless の `syn.project.import_smf`/`export_smf` op 面は
    /// `available`。OS backend は ruviform `ruviform-midi-device`
    /// (ruviform#253) の OS 別状態を正直に投影する: Windows は
    /// `WinMM` backend 実装済みだが実 port 検証は外部 gate
    /// (`not_run`)、macOS/Linux は backend 未実装で
    /// `unsupported`。SMPTE division と Format 2 は synvelis#129、
    /// MIDI 2.0 は synvelis#130/ruviform#317 で deferred。
    /// clip→MIDI は clip→plan→wire event の compile まで headless;
    /// instrument node への配送 seam は別 gate。
    fn midi_capabilities() -> Value {
        json!({
            "smf": {
                "state": "available",
                "ops": ["syn.project.import_smf", "syn.project.export_smf"],
                "formats": ["format0", "format1"],
                "division": "ppqn 1..=32767",
            },
            "backend": {
                "windows": {
                    "state": "implemented",
                    "backend": "winmm",
                    "real_port": "not_run",
                    "reason": "ruviform-midi-device (ruviform#253); real-port verification is an external gate",
                },
                "macos": {
                    "state": "unsupported",
                    "reason": "no OS backend implemented (ruviform#253)",
                },
                "linux": {
                    "state": "unsupported",
                    "reason": "no OS backend implemented (ruviform#253)",
                },
            },
            "unsupported": [
                { "feature": "smpte_division", "state": "unsupported", "reason": "deferred to synvelis#129" },
                { "feature": "smf_format2", "state": "unsupported", "reason": "deferred to synvelis#129" },
                { "feature": "midi2", "state": "unsupported", "reason": "future — synvelis#130 / ruviform#317" },
            ],
            "player_clip": {
                "state": "partial",
                "note": "midi-source clips compile to engine-frame MIDI event plans and stage onto the track strip's chain-head instrument node headless; real-plugin playback acceptance is an external gate",
            },
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
            "recorder": self.recorder_capabilities(),
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
            "midi": Self::midi_capabilities(),
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

/// Required string param — same wire contract the service-level
/// `string_param` enforces for ops dispatched inline there.
fn string_param<'a>(params: &'a Value, key: &str) -> Result<&'a str, ErrorBody> {
    params
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_params(format!("params.{key} (string) required")))
}

/// Optional `u64 → u32` param — absent or `null` means `None`;
/// domain-range validation stays with the owning layer.
fn optional_u32_param(params: &Value, key: &str) -> Result<Option<u32>, ErrorBody> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .map(Some)
            .ok_or_else(|| invalid_params(format!("params.{key} must be a u64 fitting u32"))),
    }
}

/// The `expected_revision` CAS mutating ops share — absent/`null`
/// means "current session revision" (no pin).
fn expected_revision_param(
    session: &ProjectSession,
    params: &Value,
) -> Result<ProjectRevision, ErrorBody> {
    match params.get("expected_revision") {
        None | Some(Value::Null) => Ok(session.revision()),
        Some(v) => v
            .as_u64()
            .map(ProjectRevision)
            .ok_or_else(|| invalid_params("params.expected_revision (u64) required")),
    }
}

/// The app-layer mutation gate every project-mutating op shares —
/// transport stopped, no open recording transaction, automation
/// record disabled. The project layer never sees session state, so
/// the precondition lives here; `what` names the attempted change
/// in the rejection message.
fn project_mutation_gate(session: &ProjectSession, what: &str) -> Result<(), ErrorBody> {
    if session.transport().playing {
        return Err(invalid_params(format!(
            "transport is playing — stop transport before {what}"
        )));
    }
    if session.control().recording().is_some() {
        return Err(invalid_params(format!(
            "recording transaction open — commit or abort it before {what}"
        )));
    }
    if session.control().record_enabled() {
        return Err(invalid_params(format!(
            "automation record is enabled — disable it before {what}"
        )));
    }
    Ok(())
}

/// Maps an [`SmfError`] onto a wire [`ErrorBody`] — the typed kind
/// rides in `details.smf_error` so headless callers branch on
/// structure, not on message text.
fn smf_error_body(error: &SmfError) -> ErrorBody {
    let (kind, offset, detail) = match error {
        SmfError::Malformed { offset, detail } => ("malformed", Some(*offset), detail.clone()),
        SmfError::Unsupported { detail } => ("unsupported", None, detail.clone()),
        SmfError::Overflow { detail } => ("overflow", None, detail.clone()),
        SmfError::NotFound(what) => ("not_found", None, what.clone()),
    };
    let mut body = invalid_params(error.to_string());
    body.details = Some(json!({
        "smf_error": { "kind": kind, "offset": offset, "detail": detail },
    }));
    body
}

/// Maps an SMF file IO failure onto a wire [`ErrorBody`] — file
/// boundary failures are `service_error` with the io `kind` in
/// `details`, distinct from SMF-domain rejections.
fn smf_io_body(action: &str, path: &str, error: &std::io::Error) -> ErrorBody {
    let mut body = ErrorBody::new(
        codes::SERVICE_ERROR,
        format!("smf {action} {path:?} failed: {error}"),
    );
    body.details = Some(json!({
        "io_error": {
            "action": action,
            "path": path,
            "kind": format!("{:?}", error.kind()),
        },
    }));
    body
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
