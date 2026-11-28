//! synvelis#176 (SV-GPUI-02) slice S1 — headless real-ABI plugin-dock probe.
//!
//! Three tiers, all honest:
//!
//! - **Tier A** — the upstream `rf631.plugin-dock` `HEADLESS_MATRIX` driven
//!   by `run_all` against the ruviform#630 test-plugin cdylibs. The dev-dep
//!   edge builds `ruviform_plugin_test_vst3` next to this binary, so the
//!   `artifact` case and its dependents really execute; a bogus explicit
//!   path still fails closed to `NotRun`.
//! - **Tier B** — product-level legs: `PluginDock` driven through the
//!   `PluginEditorHost` seam over a live `InProcessHost` with the real
//!   `Vst3Library` factories. `test_gain` ships no editor → a defined
//!   `Unsupported` refusal; `test_editor` ships an editor but the panel's
//!   synthetic lease payload is not a real OS parent → the honest outcome
//!   is recorded, never a fake pass.
//! - **Tier C** — the CLAP format dimension: `ClapLibrary::load` +
//!   `InProcessHost` lifecycle, then the same product-seam open attempt
//!   recorded with whatever verdict the real binary produces.
//!
//! Nothing here claims a real OS window, a child HWND, or a pixel oracle —
//! acceptance rows 1/7 stay `NotRun` (see `evidence/sv176/`).

use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};
use std::env;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ruviform_audio_core::SampleRate;
use ruviform_plugin_clap::host::ClapLibrary;
use ruviform_plugin_core::{PluginDescriptor, PluginFactory, PluginHandle, ProcessConfig};
use ruviform_plugin_dock_test::artifact::resolve_artifact_named;
use ruviform_plugin_dock_test::jsonl::{render_jsonl, repo_sha};
use ruviform_plugin_dock_test::scenario::RUN_ID;
use ruviform_plugin_dock_test::{
    HEADLESS_MATRIX, LaneEnvironment, PLUGIN_PATH_ENV, PLUGIN_PATH_ENV_CLAP, run_all, sha256_hex,
};
use ruviform_plugin_host::{
    AttachMode, EditorOpError, EditorSessionError, EditorSessionEvent, EditorSessionOps,
    EditorSessionState, EditorSize, HostCommand, HostError, HostReply, InProcessEditorOps,
    InProcessHost, LeaseRedeemer, PluginEditorSessions, SessionFailCause,
};
use ruviform_plugin_vst3::host::Vst3Library;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::ipc::Event;
use ruviform_ui::native_slot::{
    LeaseInfo, LogicalRect, NativeSurfaceSlot, SlotCapabilities, SlotError, SlotEvent,
    SlotGeometry, SlotLease, SlotSpec, SlotState, SlotVisibility, SurfaceSlotId,
};
use ruviform_ui::{BackendReport, WindowId};
use ruviform_ui_test::{FakeNativeSlot, ScenarioReport, ScenarioVerdict};
use synvelis_app::plugin_dock::{NativeSlotAuthority, SharedNativeSlotAuthority};
use synvelis_app::{PluginDock, PluginDockError, PluginEditorHost, SynvelisApp};
use synvelis_player::StablePluginInstanceId;

/// Canonical lane artifact — set to a file path to have the `run_all`
/// document written next to the evidence (upstream convention: the
/// binaries emit the same JSONL shape).
const JSONL_OUT_ENV: &str = "SYNVELIS_PLUGIN_DOCK_PROBE_JSONL";
/// Per-leg records — set to a directory to have each product-level leg write
/// one `<case>.jsonl` case line (one file per leg, race-free under the
/// parallel test runner).
const PROBE_DIR_ENV: &str = "SYNVELIS_PLUGIN_DOCK_PROBE_DIR";
/// The stable instance identity every product leg binds.
const INSTANCE: u64 = 7;

fn verdict_of<'a>(reports: &'a [ScenarioReport], case: &str) -> &'a ScenarioReport {
    reports
        .iter()
        .find(|report| report.case.as_ref().is_some_and(|id| id.as_str() == case))
        .unwrap_or_else(|| panic!("missing case report `{case}`"))
}

/// The lane environment a binary would build: the env var override when
/// set; otherwise the dev-dep-built artifact the probe resolution below
/// finds (`plugin_path` is the lane's explicit-path slot — filling it
/// with the discovered cdylib is the honest equivalent of the env var).
fn lane_env() -> LaneEnvironment {
    LaneEnvironment {
        repo_sha: repo_sha(),
        plugin_path: env::var_os(PLUGIN_PATH_ENV)
            .map(PathBuf::from)
            .or_else(|| find_built_fixture("ruviform_plugin_test_vst3")),
    }
}

/// The dev-dep edge lands the fixture cdylib as
/// `deps/<prefix><stem>-<hash><suffix>` — upstream
/// `resolve_artifact_named`'s `current_exe` candidates cover only the
/// un-hashed `<prefix><stem><suffix>` (the `cargo build -p` layout the
/// upstream lane binaries sit next to). This walks the same order —
/// explicit env path verbatim → `deps/deps/<lib>` → `deps/<lib>` — then
/// adds `deps/../<lib>` (profile-root `cargo build -p` output) and the
/// hashed dep-artifact glob so the dev-dep edge resolves too. Every
/// returned path is a real cargo-built artifact; the probe records its
/// sha256 either way.
fn resolve_fixture(explicit: Option<&Path>, lib_stem: &str) -> Result<PathBuf, String> {
    let lib_name = format!(
        "{}{}{}",
        env::consts::DLL_PREFIX,
        lib_stem,
        env::consts::DLL_SUFFIX
    );
    match resolve_artifact_named(explicit, &lib_name) {
        Err(reason) if explicit.is_none() => find_built_fixture(lib_stem).ok_or(reason),
        result => result,
    }
}

/// `deps/../<lib>` (the `cargo build -p` profile-root artifact), then the
/// sorted `deps/<stem>-*<suffix>` hashed dep-edge artifacts.
fn find_built_fixture(lib_stem: &str) -> Option<PathBuf> {
    let dir = env::current_exe().ok()?.parent()?.to_path_buf();
    let lib_name = format!(
        "{}{}{}",
        env::consts::DLL_PREFIX,
        lib_stem,
        env::consts::DLL_SUFFIX
    );
    if let Some(root) = dir.parent() {
        let candidate = root.join(&lib_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let prefix = format!("{}{}-", env::consts::DLL_PREFIX, lib_stem);
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .filter(|name| name.starts_with(&prefix) && name.ends_with(env::consts::DLL_SUFFIX))
        .collect();
    names.sort_unstable();
    names.first().map(|name| dir.join(name))
}

fn view(n: u64) -> ViewInstanceId {
    ViewInstanceId::new(n).expect("view id non-zero")
}

fn win(n: u64) -> WindowId {
    WindowId::new(n).expect("window id non-zero")
}

fn inst() -> StablePluginInstanceId {
    StablePluginInstanceId::new(INSTANCE)
}

fn geom() -> SlotGeometry {
    SlotGeometry {
        rect: LogicalRect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        },
        scale_factor: 1.0,
    }
}

fn size() -> EditorSize {
    EditorSize {
        width: 800.0,
        height: 600.0,
    }
}

/// The `PluginEditorHost` composition root a GUI driver would own: a
/// live `InProcessHost` plus the `StablePluginInstanceId → PluginHandle`
/// map the instantiate path fills. `ops` lends the same
/// `InProcessEditorOps` adapter the lane's upstream scenario uses.
struct RealAbiEditorHost {
    host: InProcessHost,
    handles: HashMap<u64, PluginHandle>,
}

impl RealAbiEditorHost {
    fn new() -> Self {
        Self {
            host: InProcessHost::new(),
            handles: HashMap::new(),
        }
    }

    fn register_factories(&mut self, factories: Vec<impl PluginFactory + 'static>) {
        for factory in factories {
            self.host.register_factory(Box::new(factory));
        }
    }

    fn register_instance(&mut self, handle: PluginHandle) {
        self.handles.insert(INSTANCE, handle);
    }
}

impl PluginEditorHost for RealAbiEditorHost {
    fn resolve(&mut self, instance: StablePluginInstanceId) -> Option<PluginHandle> {
        self.handles.get(&instance.get()).copied()
    }

    fn ops<'a>(
        &'a mut self,
        handle: PluginHandle,
        redeem: Box<LeaseRedeemer<'a>>,
    ) -> Result<Box<dyn EditorSessionOps + 'a>, EditorOpError> {
        Ok(Box::new(InProcessEditorOps::with_redeemer(
            &mut self.host,
            handle,
            redeem,
        )))
    }
}

impl Drop for RealAbiEditorHost {
    fn drop(&mut self) {
        // Product-app probe legs move this host into `SynvelisApp`;
        // dropping the app still owns the honest non-RT teardown of
        // every real ABI instance it activated. Explicit teardown in
        // the older legs makes these calls defined no-ops/errors.
        for handle in self.handles.values().copied() {
            let _ = self.host.execute(HostCommand::Deactivate { handle });
            let _ = self.host.execute(HostCommand::Destroy { handle });
        }
    }
}

/// Selectively drains `syn.plugin_dock.state` notify payloads from an
/// app outbox, leaving unrelated events queued.
fn drain_app_plugin_dock_notifies(app: &SynvelisApp) -> Vec<serde_json::Value> {
    let outbox = app.event_outbox();
    let mut matched = Vec::new();
    let mut rest = Vec::new();
    {
        let mut q = outbox.borrow_mut();
        while let Some(event) = q.pop_front() {
            match &event {
                Event::Notify { data, .. } if data["channel"] == "syn.plugin_dock.state" => {
                    matched.push(data["data"].clone());
                }
                _ => rest.push(event),
            }
        }
        for event in rest.into_iter().rev() {
            q.push_front(event);
        }
    }
    matched
}

/// Scan → Instantiate → Initialize → Activate(48 kHz, 1..=512) — the
/// defined process configuration upstream's `activate_test_gain` uses.
/// The caller owns Deactivate/Destroy teardown.
fn activate(host: &mut InProcessHost, plugin_id: &str) -> Result<PluginHandle, HostError> {
    match host.execute(HostCommand::Scan)? {
        HostReply::Scanned(descriptors) => {
            if !descriptors.iter().any(|d| d.id == plugin_id) {
                return Err(HostError::UnknownPluginId);
            }
        }
        _ => return Err(HostError::PluginFailed),
    }
    let HostReply::Instantiated { handle } = host.execute(HostCommand::Instantiate {
        plugin_id: plugin_id.to_string(),
    })?
    else {
        return Err(HostError::PluginFailed);
    };
    host.execute(HostCommand::Initialize { handle })?;
    let config = ProcessConfig::new(
        SampleRate::new(48_000).map_err(|_| HostError::InvalidValue)?,
        1,
        512,
    )
    .map_err(|_| HostError::InvalidValue)?;
    host.execute(HostCommand::Activate { handle, config })?;
    Ok(handle)
}

fn descriptor_id(descriptors: &[PluginDescriptor], name: &str) -> Option<&'static str> {
    descriptors.iter().find(|d| d.name == name).map(|d| d.id)
}

fn load_vst3() -> Option<Vst3Library> {
    let explicit = env::var_os(PLUGIN_PATH_ENV).map(PathBuf::from);
    match resolve_fixture(explicit.as_deref(), "ruviform_plugin_test_vst3") {
        Ok(path) => match Vst3Library::load(&path) {
            Ok(library) => Some(library),
            Err(e) => panic!("VST3 artifact resolved but failed to load: {e}"),
        },
        Err(reason) => {
            eprintln!(
                "plugin_dock_probe: VST3 artifact unresolved — product leg skipped: {reason}"
            );
            None
        }
    }
}

fn load_clap() -> Option<(ClapLibrary, PathBuf, String)> {
    let explicit = env::var_os(PLUGIN_PATH_ENV_CLAP).map(PathBuf::from);
    let path = match resolve_fixture(explicit.as_deref(), "ruviform_plugin_test_clap") {
        Ok(path) => path,
        Err(reason) => {
            eprintln!("plugin_dock_probe: CLAP artifact unresolved — NotRun: {reason}");
            return None;
        }
    };
    let sha = sha256_hex(&path).expect("sha256 the CLAP artifact");
    let library = ClapLibrary::load(&path).expect("ClapLibrary::load the resolved fixture");
    Some((library, path, sha))
}

/// One JSONL `case` line per product leg — the same field vocabulary
/// `render_jsonl` emits, so the ledger stays one document shape. Always
/// echoed to stderr; written to `PROBE_DIR_ENV/<case>.jsonl` when set.
fn record_leg(case: &str, driver: &str, verdict: &str, detail: &str, observed: &serde_json::Value) {
    let line = serde_json::json!({
        "type": "case",
        "run_id": RUN_ID,
        "scenario": "sv176-plugin-dock-probe",
        "case": case,
        "driver": driver,
        "verdict": verdict,
        "detail": detail,
        "observed": observed,
        "environment": {
            "os": env::consts::OS,
            "arch": env::consts::ARCH,
            "environment_kind": "test-plugin",
            "repo_sha": repo_sha(),
        },
    });
    eprintln!("{line}");
    if let Some(dir) = env::var_os(PROBE_DIR_ENV).map(PathBuf::from) {
        std::fs::create_dir_all(&dir)
            .and_then(|()| std::fs::write(dir.join(format!("{case}.jsonl")), format!("{line}\n")))
            .unwrap_or_else(|e| panic!("record leg `{case}` under {}: {e}", dir.display()));
    }
}

/// Writes the canonical lane JSONL when `JSONL_OUT_ENV` is set — the
/// identical document the upstream lane binary emits.
fn maybe_write_lane_jsonl(reports: &[ScenarioReport]) {
    if let Some(path) = env::var_os(JSONL_OUT_ENV).map(PathBuf::from) {
        let doc = render_jsonl(RUN_ID, "sv176-s1-headless", repo_sha().as_deref(), reports);
        std::fs::write(&path, doc).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    }
}

// ---------------------------------------------------------------------------
// Tier A — upstream HEADLESS_MATRIX via run_all
// ---------------------------------------------------------------------------

/// The catalog is the single source `run_all` projects: declared
/// bindings, case ids, order, drivers, and required capabilities must
/// match the produced reports exactly — drift fails here, not in
/// evidence review. (Mirrors the upstream integration test.)
#[test]
fn catalog_matches_run_all_reports() {
    let cells: Vec<_> = HEADLESS_MATRIX.cells().collect();
    assert_eq!(cells.len(), 9, "nine catalog cells");

    // The bogus path keeps the run artifact-free and fast; the
    // catalog↔report correspondence does not depend on verdicts.
    let reports = run_all(&LaneEnvironment {
        repo_sha: None,
        plugin_path: Some(PathBuf::from("definitely/not/a/real/plugin.dll")),
    });
    assert_eq!(reports.len(), cells.len(), "one report per cell");

    for ((entry, binding), report) in cells.iter().zip(reports.iter()) {
        assert_eq!(
            report.case.as_ref().map(ruviform_ui_test::CaseId::as_str),
            Some(entry.case),
            "case id drift"
        );
        assert_eq!(report.scenario.as_str(), entry.scenario);
        assert_eq!(binding.driver, report.driver, "driver drift");
        assert_eq!(
            entry.required.iter().copied().collect::<BTreeSet<_>>(),
            report.required,
            "required-capability drift for `{}`",
            entry.case
        );
    }
}

/// `project()` mirrors `run_scenario`'s pre-flight from declared data
/// only: every headless cell's `required` is covered by its binding's
/// declared capabilities — the lane really executes them.
#[test]
fn catalog_projection_covers_every_declared_requirement() {
    let projections = HEADLESS_MATRIX.project();
    assert_eq!(projections.len(), 9);
    for projection in &projections {
        assert!(
            projection.runnable,
            "case `{}` is not runnable on `{:?}`: missing {:?}",
            projection.entry.case, projection.driver, projection.missing,
        );
        assert_eq!(projection.missing, None);
    }
}

/// A bogus explicit path must fail closed: resolution fails, so the
/// artifact case and every dependent record `NotRun` — the lane output
/// still carries run/case/driver/verdict evidence.
#[test]
fn missing_artifact_fails_closed_as_not_run() {
    let reports = run_all(&LaneEnvironment {
        repo_sha: None,
        plugin_path: Some(PathBuf::from("definitely/not/a/real/plugin.dll")),
    });
    assert_eq!(reports.len(), 9, "nine plugin-dock cases");

    assert_eq!(
        verdict_of(&reports, "view-mount").verdict,
        ScenarioVerdict::Pass,
        "semantic mount does not depend on the artifact"
    );
    assert_eq!(
        verdict_of(&reports, "command-receipt").verdict,
        ScenarioVerdict::Pass,
        "semantic receipt flow does not depend on the artifact"
    );
    for case in [
        "artifact",
        "host-lifecycle",
        "editor-attach",
        "session-lifecycle",
        "host-terminate",
        "open-or-degrade",
        "dock-pixels",
    ] {
        assert_eq!(
            verdict_of(&reports, case).verdict,
            ScenarioVerdict::NotRun,
            "case `{case}` must fail closed as NotRun without the artifact"
        );
    }
    let detail = verdict_of(&reports, "artifact")
        .detail
        .as_deref()
        .unwrap_or_default();
    assert!(detail.contains("not found"), "detail: {detail}");
}

/// The dev-dep edge places the real `test_gain` cdylib next to this
/// binary, so the honest-verdict contract must hold with `Pass`
/// actually earned: artifact resolve/hash/load/discover, host
/// lifecycle, session lifecycle, host terminate, open-or-degrade pass;
/// editor attach is `Unsupported` (the fixture ships no editor); dock
/// pixels stay `NotRun`. An unresolvable artifact is a real failure
/// here — the edge is ours — not a soft degrade.
#[test]
fn fixture_run_reports_honest_verdicts() {
    let reports = run_all(&lane_env());
    assert_eq!(reports.len(), 9);
    maybe_write_lane_jsonl(&reports);

    let receipt = verdict_of(&reports, "command-receipt");
    assert_eq!(
        receipt.verdict,
        ScenarioVerdict::Pass,
        "command-receipt must pass on the Fake driver: {:?}",
        receipt.detail
    );
    assert_eq!(
        receipt.receipts.len(),
        2,
        "applied + rejected receipts recorded: {:?}",
        receipt.receipts
    );

    let artifact = verdict_of(&reports, "artifact");
    assert_eq!(
        artifact.verdict,
        ScenarioVerdict::Pass,
        "artifact resolve/hash/load/discover must pass — the dev-dep edge builds it: {}",
        artifact.detail.as_deref().unwrap_or("no detail")
    );
    let environment = artifact.environment.as_ref().expect("environment recorded");
    let plugin = &environment["plugin_artifact"];
    assert!(
        plugin["sha256"].as_str().is_some_and(|s| s.len() == 64),
        "sha256 digest recorded: {plugin}"
    );
    assert_eq!(plugin["name"], "test_gain");
    assert!(plugin["version"].as_str().is_some());
    assert_eq!(
        environment["environment_kind"], "test-plugin",
        "real ABI execution claims the real kind"
    );

    assert_eq!(
        verdict_of(&reports, "host-lifecycle").verdict,
        ScenarioVerdict::Pass,
        "scan/instantiate/initialize/activate ran against the real binary"
    );

    let attach = verdict_of(&reports, "editor-attach");
    assert!(
        matches!(
            attach.verdict,
            ScenarioVerdict::Unsupported | ScenarioVerdict::NotRun
        ),
        "editor attach must never pass without a real editor: {:?}",
        attach.verdict
    );
    if attach.verdict == ScenarioVerdict::Unsupported {
        let detail = attach.detail.as_deref().unwrap_or_default();
        assert!(
            detail.contains("createView") || detail.contains("Unsupported"),
            "detail must name the honest refusal: {detail}"
        );
    }

    for case in ["session-lifecycle", "host-terminate", "open-or-degrade"] {
        let report = verdict_of(&reports, case);
        assert_eq!(
            report.verdict,
            ScenarioVerdict::Pass,
            "case `{case}` must pass on the real binary: {:?}",
            report.detail
        );
        let kind = report
            .environment
            .as_ref()
            .and_then(|env| env["environment_kind"].as_str());
        assert_eq!(
            kind,
            Some("test-plugin"),
            "case `{case}` claims the real ABI kind only because it ran"
        );
    }

    let pixels = verdict_of(&reports, "dock-pixels");
    assert_eq!(
        pixels.verdict,
        ScenarioVerdict::NotRun,
        "dock pixels stay NotRun — no surface, no capture driver"
    );

    for report in &reports {
        if let Some(kind) = report
            .environment
            .as_ref()
            .and_then(|env| env["environment_kind"].as_str())
        {
            assert_ne!(kind, "real-window", "no real window ran: {kind}");
        }
    }
}

// ---------------------------------------------------------------------------
// Shared test slot authority — headless `FakeNativeSlot` fixture
// ---------------------------------------------------------------------------

/// High tag bits of the synthetic parent tokens the test authority
/// mints at redemption. **Test-only vocabulary**: the value is
/// obviously not an OS handle (no real HWND/HWND-like pointer carries
/// this tag) and exists purely so tests can prove which slot a
/// redeemed lease belonged to. Nothing here is evidence of real
/// native docking.
const TEST_PARENT_TOKEN_TAG: u64 = 0xFA17_E000_0000_0000;

/// The synthetic test-only parent token for `slot` — distinct per
/// slot index so a cross-slot redemption mix-up is observable.
fn test_parent_token(seq: u32) -> u64 {
    TEST_PARENT_TOKEN_TAG | u64::from(seq)
}

/// Headless shared-authority fixture: upstream [`FakeNativeSlot`]
/// (the canonical deterministic [`NativeSurfaceSlot`] fake) plus the
/// product seam surface — spec mirroring (the fake does not expose
/// specs) and issuer-side redemption against the *same* ledger. One
/// instance behind one `Rc<RefCell>` is shared by every panel in a
/// test, exactly like the production driver's canonical surface
/// ledger — a second fixture instance is a *different* authority and
/// must reject this ledger's leases as foreign.
///
/// **Headless/test-only** — no OS window, no real native parent.
struct TestSlotAuthority {
    inner: FakeNativeSlot,
    /// Mirror of each live slot's declared spec, updated on
    /// geometry/visibility mutation — the fake validates through its
    /// registry but does not expose specs, and the product seam
    /// requires them.
    specs: HashMap<SurfaceSlotId, SlotSpec>,
}

impl TestSlotAuthority {
    fn new() -> Self {
        Self {
            inner: FakeNativeSlot::new(),
            specs: HashMap::new(),
        }
    }
}

/// mints a shared handle around a fresh fixture authority.
fn test_authority() -> SharedNativeSlotAuthority {
    Rc::new(RefCell::new(TestSlotAuthority::new()))
}

impl NativeSurfaceSlot for TestSlotAuthority {
    fn capabilities(&self, slot: SurfaceSlotId) -> Result<SlotCapabilities, SlotError> {
        self.inner.capabilities(slot)
    }

    fn declare_slot(&mut self, spec: SlotSpec) -> Result<SurfaceSlotId, SlotError> {
        let slot = self.inner.declare_slot(spec)?;
        self.specs.insert(slot, spec);
        Ok(slot)
    }

    fn release_slot(&mut self, slot: SurfaceSlotId) -> Result<(), SlotError> {
        self.inner.release_slot(slot)?;
        self.specs.remove(&slot);
        Ok(())
    }

    fn set_geometry(
        &mut self,
        slot: SurfaceSlotId,
        geometry: SlotGeometry,
    ) -> Result<(), SlotError> {
        self.inner.set_geometry(slot, geometry)?;
        if let Some(spec) = self.specs.get_mut(&slot) {
            spec.geometry = geometry;
        }
        Ok(())
    }

    fn set_visibility(
        &mut self,
        slot: SurfaceSlotId,
        visibility: SlotVisibility,
    ) -> Result<(), SlotError> {
        self.inner.set_visibility(slot, visibility)?;
        if let Some(spec) = self.specs.get_mut(&slot) {
            spec.visibility = visibility;
        }
        Ok(())
    }

    fn set_clip(
        &mut self,
        slot: SurfaceSlotId,
        clip: Option<LogicalRect>,
    ) -> Result<(), SlotError> {
        self.inner.set_clip(slot, clip)
    }

    fn set_zorder(&mut self, slot: SurfaceSlotId, z: i32) -> Result<(), SlotError> {
        self.inner.set_zorder(slot, z)
    }

    fn request_focus(&mut self, slot: SurfaceSlotId) -> Result<(), SlotError> {
        self.inner.request_focus(slot)
    }

    fn lease_parent(&mut self, slot: SurfaceSlotId) -> Result<SlotLease, SlotError> {
        self.inner.lease_parent(slot)
    }

    fn release_lease(&mut self, lease: SlotLease) -> Result<(), SlotError> {
        self.inner.release_lease(lease)
    }

    fn validate_lease(&self, lease: SlotLease) -> Result<LeaseInfo, SlotError> {
        self.inner.validate_lease(lease)
    }

    fn drain_events(&mut self) -> Vec<SlotEvent> {
        self.inner.drain_events()
    }
}

impl NativeSlotAuthority for TestSlotAuthority {
    fn state(&self, slot: SurfaceSlotId) -> Result<Option<SlotState>, SlotError> {
        Ok(self.inner.state(slot))
    }

    fn spec(&self, slot: SurfaceSlotId) -> Result<Option<SlotSpec>, SlotError> {
        Ok(self.specs.get(&slot).copied())
    }

    fn mark_busy(&mut self, slot: SurfaceSlotId) -> Result<(), SlotError> {
        self.inner.mark_busy(slot)
    }

    fn clear_busy(&mut self, slot: SurfaceSlotId) -> Result<(), SlotError> {
        self.inner.clear_busy(slot)
    }

    fn redeem_parent(&self, payload: u64) -> Result<u64, SlotError> {
        // Issuer-side validation against *this* ledger — a stale,
        // revoked, or foreign (other-authority) payload never yields
        // a token. The token itself is the clearly synthetic
        // test-only value `test_parent_token`, never an OS handle.
        let lease = SlotLease::new(payload);
        self.inner.validate_lease(lease)?;
        Ok(test_parent_token(lease.seq()))
    }
}

// ---------------------------------------------------------------------------
// Tier B — product PluginDock over the real VST3 ABI path
// ---------------------------------------------------------------------------

/// `test_gain` ships no editor: `PluginDock::open` must surface the
/// defined `Unsupported` refusal — the same contract the mock-adapter
/// fixture asserts, now through `InProcessHost` + a real cdylib.
/// Also verified: four-id mint, lease revocation, `Failed` state kept
/// observable, `SessionBusy` on the pinned instance, event drain, close
/// convergence, and DSP non-interference (the real instance still
/// deactivates/destroys).
#[test]
fn product_dock_vst3_test_gain_open_refused_is_honest() {
    let Some(library) = load_vst3() else { return };
    let gain_id =
        descriptor_id(library.descriptors(), "test_gain").expect("test_gain descriptor present");

    let mut host = RealAbiEditorHost::new();
    host.register_factories(library.factories());
    let handle = activate(&mut host.host, gain_id).expect("activate test_gain on the real ABI");
    host.register_instance(handle);

    let mut sessions = PluginEditorSessions::new();
    let mut dock =
        PluginDock::declare(view(11), win(1), geom(), test_authority()).expect("declare panel");

    let Err(err) = dock.open(&mut sessions, &mut host, inst(), size(), AttachMode::Docked) else {
        panic!("test_gain ships no editor — open must refuse honestly")
    };
    assert!(
        matches!(
            err,
            PluginDockError::Session(EditorSessionError::Ops(EditorOpError::Unsupported))
        ),
        "defined Unsupported refusal expected, got {err:?}"
    );

    let binding = dock.binding().expect("failed binding kept observable");
    assert_eq!(binding.instance.get(), INSTANCE);
    assert_eq!(binding.handle, handle);
    assert!(binding.lease().is_none(), "lease revoked on refused open");
    assert_eq!(binding.slot, dock.slot().expect("slot live"));
    assert_eq!(binding.view, view(11));

    assert!(matches!(
        dock.session_state(&sessions),
        Some(EditorSessionState::Failed {
            cause: SessionFailCause::Unsupported
        })
    ));
    assert!(
        matches!(
            sessions.begin_session(handle),
            Err(EditorSessionError::SessionBusy)
        ),
        "the pinned refused session still owns the instance"
    );

    let events = dock.drain_session_events(&mut sessions);
    assert_eq!(
        events,
        vec![EditorSessionEvent::Failed {
            cause: SessionFailCause::Unsupported
        }],
        "a refused open emits exactly the Failed event"
    );

    dock.close(&mut sessions, Some(&mut host))
        .expect("close converges a refused-open session");
    assert!(dock.binding().is_none());

    // DSP/RT non-interference oracle: the refused editor session never
    // touched the real instance — Deactivate/Destroy still complete.
    host.host
        .execute(HostCommand::Deactivate { handle })
        .expect("deactivate survives the refused session");
    host.host
        .execute(HostCommand::Destroy { handle })
        .expect("destroy survives the refused session");

    record_leg(
        "product-dock-test-gain-open",
        "plugin-dock+in-process-host/vst3",
        "unsupported",
        "test_gain ships no editor — open refused with the defined Unsupported; refused session stayed observable, closed cleanly, instance survived",
        &serde_json::json!({
            "error": format!("{err:?}"),
            "session_state": "failed{unsupported}",
        }),
    );
}

/// `test_editor` ships a real `IPlugView`, but redemption yields the
/// fixture authority's synthetic tagged test token — not a real OS parent
/// HWND. The docked attach must
/// therefore refuse honestly (or, on a platform where the fixture
/// accepts it, attach — either is recorded verbatim; a fabricated pass
/// is impossible at this seam).
#[test]
fn product_dock_vst3_test_editor_attach_observation() {
    let Some(library) = load_vst3() else { return };
    let Some(editor_id) = descriptor_id(library.descriptors(), "test_editor") else {
        eprintln!("plugin_dock_probe: no test_editor descriptor — leg NotRun");
        return;
    };

    let mut host = RealAbiEditorHost::new();
    host.register_factories(library.factories());
    let handle = activate(&mut host.host, editor_id).expect("activate test_editor on the real ABI");
    host.register_instance(handle);

    let mut sessions = PluginEditorSessions::new();
    let mut dock =
        PluginDock::declare(view(12), win(1), geom(), test_authority()).expect("declare panel");

    let outcome = dock.open(&mut sessions, &mut host, inst(), size(), AttachMode::Docked);
    let state = dock.session_state(&sessions);
    let (verdict, detail) = match &outcome {
        Ok(report) => {
            assert!(
                matches!(state, Some(EditorSessionState::Attached { .. })),
                "Ok implies Attached"
            );
            ("pass", format!("attached mode={:?}", report.mode))
        }
        Err(e @ PluginDockError::Ops(_)) => {
            assert!(
                dock.binding().is_none(),
                "pre-open ops refusal drops the binding: {e:?}"
            );
            ("fail", format!("pre-open adapter refusal {e:?}"))
        }
        Err(e) => {
            assert!(
                matches!(state, Some(EditorSessionState::Failed { .. })),
                "a session refusal lands Failed: {e:?}"
            );
            (
                "unsupported",
                format!("docked attach refused {e:?} state={state:?}"),
            )
        }
    };
    record_leg(
        "product-dock-test-editor-attach",
        "plugin-dock+in-process-host/vst3",
        verdict,
        &detail,
        &serde_json::json!({
            "outcome": format!("{outcome:?}"),
            "session_state": format!("{state:?}"),
        }),
    );

    if dock.binding().is_some() {
        dock.close(&mut sessions, Some(&mut host))
            .expect("close converges whatever open produced");
    }
    host.host
        .execute(HostCommand::Deactivate { handle })
        .expect("deactivate");
    host.host
        .execute(HostCommand::Destroy { handle })
        .expect("destroy");
}

// ---------------------------------------------------------------------------
// Tier C — CLAP format dimension
// ---------------------------------------------------------------------------

/// The CLAP half of the real-ABI claim: resolve/hash/`ClapLibrary::load`
/// the fixture, run `InProcessHost` Scan → Instantiate → Initialize →
/// Activate → Deactivate → Destroy, then record the product-seam open
/// outcome for `test_gain` (no `clap.ext.gui` — expected refusal,
/// recorded verbatim rather than asserted to one cause).
#[test]
fn clap_artifact_and_host_lifecycle_probe() {
    let Some((library, path, sha)) = load_clap() else {
        return;
    };
    let gain_id =
        descriptor_id(library.descriptors(), "test_gain").expect("clap test_gain descriptor");
    record_leg(
        "clap-artifact",
        "clap-library/in-process",
        "pass",
        "resolve + sha256 + ClapLibrary::load + descriptor discovery",
        &serde_json::json!({
            "path": path,
            "sha256": sha,
            "plugin_id": gain_id,
            "format": "clap",
        }),
    );

    let mut host = RealAbiEditorHost::new();
    host.register_factories(library.factories());
    let handle = match activate(&mut host.host, gain_id) {
        Ok(handle) => handle,
        Err(e) => {
            record_leg(
                "clap-host-lifecycle",
                "in-process-host/clap",
                "fail",
                &format!("activate refused: {e:?}"),
                &serde_json::json!({ "error": format!("{e:?}") }),
            );
            panic!("clap test_gain activate: {e:?}");
        }
    };
    record_leg(
        "clap-host-lifecycle",
        "in-process-host/clap",
        "pass",
        "Scan/Instantiate/Initialize/Activate on the real clap binary",
        &serde_json::json!({ "handle": format!("{handle:?}") }),
    );

    let mut sessions = PluginEditorSessions::new();
    let mut dock =
        PluginDock::declare(view(13), win(1), geom(), test_authority()).expect("declare panel");
    host.register_instance(handle);
    let outcome = dock.open(&mut sessions, &mut host, inst(), size(), AttachMode::Docked);
    let state = dock.session_state(&sessions);
    let verdict = match &outcome {
        Ok(_) => "pass",
        Err(PluginDockError::Session(EditorSessionError::Ops(EditorOpError::Unsupported))) => {
            "unsupported"
        }
        Err(_) => "fail",
    };
    record_leg(
        "product-dock-clap-test-gain-open",
        "plugin-dock+in-process-host/clap",
        verdict,
        &format!("clap test_gain docked open: {outcome:?} state={state:?}"),
        &serde_json::json!({ "outcome": format!("{outcome:?}") }),
    );

    if dock.binding().is_some() {
        dock.close(&mut sessions, Some(&mut host))
            .expect("close converges whatever open produced");
    }
    host.host
        .execute(HostCommand::Deactivate { handle })
        .expect("deactivate");
    host.host
        .execute(HostCommand::Destroy { handle })
        .expect("destroy");
}

/// Product-app pump leg over the real VST3 ABI: `test_gain` refuses
/// docked open with `Unsupported`, the refusal stays queued until the
/// authoritative `ui_pump` seam drains it, and the notify/read-model
/// projection reports the failure exactly once. This is headless ABI
/// evidence only — the synthetic parent token is not an OS parent.
#[test]
fn product_app_pump_projects_vst3_test_gain_refusal_once() {
    let Some(library) = load_vst3() else {
        record_leg(
            "product-app-pump-test-gain-open",
            "synvelis-app+in-process-host/vst3",
            "notrun",
            "VST3 fixture artifact unresolved",
            &serde_json::json!({}),
        );
        return;
    };
    let gain_id =
        descriptor_id(library.descriptors(), "test_gain").expect("test_gain descriptor present");

    let mut host = RealAbiEditorHost::new();
    host.register_factories(library.factories());
    let handle = activate(&mut host.host, gain_id).expect("activate test_gain on the real ABI");
    host.register_instance(handle);

    let mut app = SynvelisApp::new(BackendReport::available("gpui", "probe-0.1".to_string()));
    app.set_plugin_editor_host(Box::new(host));
    app.set_native_slot_authority(test_authority());
    app.op_window_lifecycle(&serde_json::json!({
        "window_id": "w1",
        "event": "created",
        "title": "Synvelis probe",
    }))
    .expect("adopt probe window");

    let err = app
        .op_plugin_dock_open(&serde_json::json!({
            "view_id": "view-21",
            "window_id": "w1",
            "instance": INSTANCE,
        }))
        .expect_err("test_gain ships no editor");
    assert_eq!(err.code.as_str(), "unavailable");

    app.op_window_lifecycle(&serde_json::json!({ "event": "tick" }))
        .expect("first authoritative pump");
    let state = app.plugin_dock_state("view-21").expect("panel read model");
    assert_eq!(state["session_state"], "failed");
    app.op_window_lifecycle(&serde_json::json!({ "event": "tick" }))
        .expect("second authoritative pump");

    let failed: Vec<_> = drain_app_plugin_dock_notifies(&app)
        .iter()
        .filter(|row| row["view_id"] == "view-21")
        .flat_map(|row| row["events"].as_array().into_iter().flatten())
        .filter(|event| event["event"] == "failed")
        .cloned()
        .collect();
    assert_eq!(
        failed.len(),
        1,
        "the real refusal is projected exactly once"
    );
    assert_eq!(failed[0]["cause"], "Unsupported");

    record_leg(
        "product-app-pump-test-gain-open",
        "synvelis-app+in-process-host/vst3",
        "unsupported",
        "real ABI test_gain open refused Unsupported; authoritative ui_pump projected Failed exactly once",
        &serde_json::json!({
            "error": format!("{err:?}"),
            "failed_events": failed,
        }),
    );
}
