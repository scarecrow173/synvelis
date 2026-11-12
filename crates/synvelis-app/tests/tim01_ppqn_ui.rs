//! TIM-01 PPQN selection UI + Snap/Grid policy (synvelis#125 UI
//! slice) — headless evidence for the view-owned select → preview →
//! confirm/cancel gesture ([`PpqnUi`]), the `ppqn_*` app helpers
//! that drive it through the real `syn.project.*` ops, the Project
//! tab's `ViewSpec` surface, and the decided Snap/Grid
//! explicit-tick-preservation policy.
//!
//! Every case drives the same paths the product uses: `UiRuntime::
//! dispatch_op` → `Service::call` → `AppService` → `SynvelisApp` for
//! `syn.*` ops, and `SynvelisApp::ppqn_*` for the view-side gesture
//! helpers. There is no second project model and no test-only op
//! surface.
//!
//! Boundary: this is headless evidence only. Real GUI widget
//! intents (parameterized confirm/cancel through `rvf.ui.command.v1`),
//! real window/OS GUI, real device, Presentation Driver time
//! authority, and product Oracle stay `NotRun` — fake/mock fixtures
//! here never claim those passes.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{Value, json};

use ruviform_media_core::Rounding;
use ruviform_ui::ipc::{ErrorBody, Event};
use ruviform_ui::view::ComponentSpec;
use ruviform_ui::{
    BackendReport, CallOutcome, DataRegistry, OpInfo, Service, ServiceContext, UiRuntime,
};

use synvelis_app::screens::{MainScreenState, PpqnScreenState, main_screen_spec};
use synvelis_app::{AppService, SynvelisApp};
use synvelis_project::{PpqnConversionPolicy, ProjectOp, preview_ppqn};
use synvelis_workspace::snap::{
    DEFAULT_SNAP_QUANTUM_TICKS, SnapConfig, SnapUnit, snap_unit_ppqn_effect,
};
use synvelis_workspace::state::{PpqnPhase, PpqnUi, PpqnUiError, RECOMMENDED_PPQN};
use synvelis_workspace::{GridConfig, ViewState};

// ---------- shared headless harness (same pattern as tim01_ppqn_app) ----------

/// Shares one [`AppService`] between the headless [`UiRuntime`]
/// (real `syn.*` string dispatch) and the test (fixture mutation via
/// `app_mut`). `UiRuntime` never exposes a registered service back,
/// so the service is wrapped in `Rc<RefCell>` and re-registered
/// through a delegating [`Service`] impl.
struct SharedService(Rc<RefCell<AppService>>);

impl Service for SharedService {
    fn ops(&self) -> &'static [OpInfo] {
        self.0.borrow().ops()
    }

    fn call(
        &mut self,
        op: &str,
        params: &Value,
        ctx: &mut ServiceContext<'_>,
    ) -> Result<CallOutcome, ErrorBody> {
        self.0.borrow_mut().call(op, params, ctx)
    }
}

struct Fixture {
    rt: UiRuntime,
    svc: Rc<RefCell<AppService>>,
}

fn fixture() -> Fixture {
    let app = SynvelisApp::new(BackendReport::available("webview2", "test-1.0".into()));
    let svc = Rc::new(RefCell::new(AppService::new(app)));
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(SharedService(svc.clone()))
        .expect("register AppService");
    Fixture { rt, svc }
}

fn call(rt: &mut UiRuntime, op: &str, params: Value) -> (bool, Option<Value>, Option<ErrorBody>) {
    let events = rt.dispatch_op("test-1", op, params);
    let result = events.iter().find_map(|e| match e {
        Event::Result {
            ok, data, error, ..
        } => Some((*ok, data.clone(), error.clone())),
        _ => None,
    });
    result.expect("every intent yields exactly one result event")
}

fn ok(rt: &mut UiRuntime, op: &str, params: Value) -> Value {
    let (ok, data, error) = call(rt, op, params);
    assert!(ok, "{op} failed: {error:?}");
    data.expect("successful result carries data")
}

// ---------- fixture reads/mutations through the same app ----------

fn live_ppqn(f: &Fixture) -> u32 {
    f.svc.borrow().app().workspace().session().project().ppqn.0
}

fn live_revision(f: &Fixture) -> u64 {
    f.svc.borrow().app().workspace().session().revision().0
}

fn apply_project(f: &Fixture, op: ProjectOp) {
    let mut svc = f.svc.borrow_mut();
    let session = svc.app_mut().workspace_mut().session_mut();
    let rev = session.revision();
    session.apply(op, rev).expect("fixture op applies");
}

/// The published main-screen state — `main_screen_cell` runs
/// `publish_project_length`, so this is also the gesture re-sync
/// point every service op tail performs.
fn screen_state(f: &Fixture) -> MainScreenState {
    let cell = f.svc.borrow().app().main_screen_cell();
    cell.borrow().clone()
}

// ---------------------------------------------------------------------------
// PpqnUi read model — current/range/preview projection (workspace level)
// ---------------------------------------------------------------------------

/// The read model tracks the live PPQN, exposes the selectable range
/// and recommendations, and projects a real `preview_ppqn` result
/// into a confirm request carrying the preview-time revision CAS.
#[test]
fn read_model_projects_current_range_and_preview() {
    let f = fixture();
    let mut ui = PpqnUi::new(960);
    assert_eq!(ui.current_ppqn(), 960);
    assert_eq!(ui.phase(), PpqnPhase::Idle);
    assert!(ui.pending().is_none());
    assert!(ui.preview_params().is_none());
    assert!(ui.confirm_request().is_none());

    // Selectable range is the SMF 15-bit PPQ bound; recommendations
    // are the conventional grids plus decimal-friendly 1000.
    assert_eq!(RECOMMENDED_PPQN, &[480, 960, 1000, 1920]);
    for edge in [1_u32, 32_767] {
        let mut probe = PpqnUi::new(960);
        probe
            .select(edge, PpqnConversionPolicy::Exact)
            .unwrap_or_else(|e| panic!("ppqn {edge} must be selectable: {e}"));
    }

    ui.select(480, PpqnConversionPolicy::Exact)
        .expect("480 is in range");
    assert_eq!(ui.phase(), PpqnPhase::Selected);
    let params = ui.preview_params().expect("pending preview params");
    assert_eq!(params["new_ppqn"], 480);
    assert_eq!(params["policy"], "exact");
    assert_eq!(params["rounding"], Value::Null);

    // Feed the real project-layer preview back — the boot project
    // has 2 musical-tick sites (default tempo + signature), all exact
    // on the 960 → 480 grid.
    let (preview, revision) = {
        let svc = f.svc.borrow();
        let session = svc.app().workspace().session();
        (
            preview_ppqn(session.project(), 480, PpqnConversionPolicy::Exact)
                .expect("preview computes"),
            session.revision().0,
        )
    };
    ui.record_preview(&preview, revision)
        .expect("matching preview records");
    assert_eq!(ui.phase(), PpqnPhase::Previewed);

    let pending = ui.pending().expect("pending selection");
    let view = pending.preview.as_ref().expect("preview projection");
    assert_eq!(view.old_ppqn, 960);
    assert_eq!(view.new_ppqn, 480);
    assert_eq!(view.policy, "exact");
    assert_eq!(view.site_count, 2);
    assert!(view.applies);
    assert!(view.exact_ok);
    assert!(view.rejection.is_none());
    // Snap/Grid effect: the default 960-tick quantum keeps its tick
    // count; only its beat spacing moves 1.000 → 2.000 beats.
    assert_eq!(view.snap_grid.quantum_ticks, 960);
    assert_eq!(view.snap_grid.describe(), "960 ticks: 1.000 → 2.000 beats");
    assert!(view.smf_export.contains("1..=32767"));

    let confirm = ui.confirm_request().expect("applicable confirm");
    assert_eq!(confirm.old_ppqn, 960);
    assert_eq!(confirm.new_ppqn, 480);
    assert_eq!(confirm.policy, "exact");
    assert_eq!(confirm.expected_revision, Some(revision));
    let wire = confirm.params();
    assert_eq!(wire["old_ppqn"], 960);
    assert_eq!(wire["new_ppqn"], 480);
    assert_eq!(wire["expected_revision"], revision);
}

/// A preview for a different candidate or a stale live PPQN never
/// arms confirm — the mismatch is rejected, not silently recorded.
#[test]
fn read_model_rejects_mismatched_and_stale_previews() {
    let f = fixture();
    let mut ui = PpqnUi::new(960);
    ui.select(480, PpqnConversionPolicy::Exact).unwrap();

    // No pending → NoPending.
    let mut idle = PpqnUi::new(960);
    let (preview, revision) = {
        let svc = f.svc.borrow();
        let session = svc.app().workspace().session();
        (
            preview_ppqn(session.project(), 480, PpqnConversionPolicy::Exact).unwrap(),
            session.revision().0,
        )
    };
    assert!(matches!(
        idle.record_preview(&preview, revision),
        Err(PpqnUiError::NoPending)
    ));

    // Pending targets 480 but the preview ran for 1920 → mismatch.
    let foreign = {
        let svc = f.svc.borrow();
        preview_ppqn(
            svc.app().workspace().session().project(),
            1920,
            PpqnConversionPolicy::Exact,
        )
        .unwrap()
    };
    assert!(matches!(
        ui.record_preview(&foreign, revision),
        Err(PpqnUiError::PreviewMismatch { .. })
    ));
    assert_eq!(ui.phase(), PpqnPhase::Selected);
    assert!(ui.confirm_request().is_none());
}

// ---------------------------------------------------------------------------
// App-level gesture helpers — select → preview → confirm/cancel
// ---------------------------------------------------------------------------

/// `ppqn_select` + `ppqn_preview_pending` run the real
/// `syn.project.preview_ppqn` op and project the result into the
/// mounted spec's `PpqnScreenState` — non-destructive throughout.
#[test]
fn app_gesture_projects_preview_into_screen_state() {
    let f = fixture();
    {
        let mut svc = f.svc.borrow_mut();
        svc.app_mut()
            .ppqn_select(480, PpqnConversionPolicy::Exact)
            .expect("in-range select");
    }
    assert_eq!(live_ppqn(&f), 960);

    let preview_json = {
        let mut svc = f.svc.borrow_mut();
        svc.app_mut()
            .ppqn_preview_pending()
            .expect("pending preview runs the real op")
    };
    assert_eq!(preview_json["new_ppqn"], 480);
    assert_eq!(preview_json["applies"], true);
    assert_eq!(live_ppqn(&f), 960, "preview never mutates");

    {
        let svc = f.svc.borrow();
        let app = svc.app();
        assert_eq!(app.ppqn_ui().phase(), PpqnPhase::Previewed);
    }

    let screen = screen_state(&f);
    assert_eq!(screen.ppqn.current_ppqn, 960);
    assert_eq!(screen.ppqn.pending_new_ppqn, Some(480));
    assert_eq!(screen.ppqn.pending_policy, Some("exact"));
    let preview = screen.ppqn.preview.as_ref().expect("preview projected");
    assert_eq!(preview.new_ppqn, 480);
    assert_eq!(preview.project_revision, live_revision(&f));
    assert_eq!(
        preview.snap_grid.describe(),
        "960 ticks: 1.000 → 2.000 beats"
    );
}

/// `ppqn_confirm` mints `syn.project.set_ppqn` params from the
/// recorded preview — the real op applies the change, the gesture
/// completes, and the screen state follows the new PPQN.
#[test]
fn confirm_applies_set_ppqn_and_clears_gesture() {
    let f = fixture();
    {
        let mut svc = f.svc.borrow_mut();
        let app = svc.app_mut();
        app.ppqn_select(480, PpqnConversionPolicy::Exact).unwrap();
        app.ppqn_preview_pending().expect("preview lands");
    }
    let info = {
        let mut svc = f.svc.borrow_mut();
        svc.app_mut().ppqn_confirm().expect("confirm applies")
    };
    assert_eq!(info["ppqn"], 480);
    assert_eq!(live_ppqn(&f), 480);

    {
        let svc = f.svc.borrow();
        let app = svc.app();
        assert_eq!(app.ppqn_ui().phase(), PpqnPhase::Idle);
        assert!(app.ppqn_ui().pending().is_none());
    }
    let screen = screen_state(&f);
    assert_eq!(screen.ppqn.current_ppqn, 480);
    assert_eq!(screen.ppqn.pending_new_ppqn, None);
    assert!(screen.ppqn.preview.is_none());
}

/// `ppqn_cancel` drops the pending selection — nothing was ever
/// applied, so the project is untouched and a second cancel reports
/// `false`.
#[test]
fn cancel_drops_pending_without_mutating() {
    let f = fixture();
    {
        let mut svc = f.svc.borrow_mut();
        let app = svc.app_mut();
        app.ppqn_select(480, PpqnConversionPolicy::Exact).unwrap();
        app.ppqn_preview_pending().expect("preview lands");
    }
    let revision_before = live_revision(&f);
    {
        let mut svc = f.svc.borrow_mut();
        assert!(svc.app_mut().ppqn_cancel(), "pending discarded");
    }
    assert_eq!(live_ppqn(&f), 960);
    assert_eq!(live_revision(&f), revision_before, "cancel mutates nothing");
    {
        let svc = f.svc.borrow();
        assert_eq!(svc.app().ppqn_ui().phase(), PpqnPhase::Idle);
    }
    let screen = screen_state(&f);
    assert_eq!(screen.ppqn.pending_new_ppqn, None);
    assert!(screen.ppqn.preview.is_none());

    let mut svc = f.svc.borrow_mut();
    assert!(!svc.app_mut().ppqn_cancel(), "nothing left to cancel");
}

/// Confirm is gated on an applicable preview: no pending selection,
/// a selected-but-unpreviewed candidate, a mismatched foreign
/// preview, and a rejected (`applies: false`) preview all fail
/// before any mutation — then an explicit `round` policy applies.
#[test]
fn confirm_requires_applicable_preview() {
    let mut f = fixture();
    {
        let mut svc = f.svc.borrow_mut();
        let app = svc.app_mut();
        // Nothing pending at all.
        let err = app.ppqn_confirm().expect_err("confirm without select");
        assert_eq!(err.code, "invalid_params");
        // Selected but never previewed.
        app.ppqn_select(480, PpqnConversionPolicy::Exact).unwrap();
        let err = app.ppqn_confirm().expect_err("confirm without preview");
        assert_eq!(err.code, "invalid_params");
    }
    assert_eq!(live_ppqn(&f), 960);

    // A foreign preview (different candidate) does not arm the
    // pending 480 selection.
    ok(
        &mut f.rt,
        "syn.project.preview_ppqn",
        json!({ "new_ppqn": 1920, "policy": "exact" }),
    );
    {
        let svc = f.svc.borrow();
        let app = svc.app();
        assert_eq!(app.ppqn_ui().phase(), PpqnPhase::Selected);
    }
    {
        let mut svc = f.svc.borrow_mut();
        let err = svc.app_mut().ppqn_confirm().expect_err("unarmed confirm");
        assert_eq!(err.code, "invalid_params");
    }

    // A rejected preview (`exact` over inexact sites) reports but
    // cannot confirm; switching to `round` with an explicit rounding
    // applies the same candidate.
    apply_project(
        &f,
        ProjectOp::AddTempoEvent {
            tick: 1,
            bpm_milli: 120_000,
        },
    );
    {
        let mut svc = f.svc.borrow_mut();
        let app = svc.app_mut();
        app.ppqn_select(1000, PpqnConversionPolicy::Exact).unwrap();
        let preview = app.ppqn_preview_pending().expect("preview runs");
        assert_eq!(preview["applies"], false);
        assert!(preview["rejection"].is_string());
        let err = app
            .ppqn_confirm()
            .expect_err("rejected preview cannot confirm");
        assert_eq!(err.code, "invalid_params");
        assert_eq!(app.workspace().session().project().ppqn.0, 960);
    }

    {
        let mut svc = f.svc.borrow_mut();
        let app = svc.app_mut();
        app.ppqn_select(1000, PpqnConversionPolicy::Round(Rounding::Floor))
            .unwrap();
        let preview = app.ppqn_preview_pending().expect("round preview runs");
        assert_eq!(preview["applies"], true);
        app.ppqn_confirm().expect("round confirm applies");
    }
    assert_eq!(live_ppqn(&f), 1000);
}

/// Undo restores the previous PPQN through the real
/// `syn.project.undo` op and re-syncs the gesture — the pending
/// selection is invalidated rather than left pointing at the old
/// grid.
#[test]
fn undo_restores_previous_ppqn_and_resyncs_gesture() {
    let mut f = fixture();
    {
        let mut svc = f.svc.borrow_mut();
        let app = svc.app_mut();
        app.ppqn_select(480, PpqnConversionPolicy::Exact).unwrap();
        app.ppqn_preview_pending().unwrap();
        app.ppqn_confirm().unwrap();
    }
    assert_eq!(live_ppqn(&f), 480);

    // Start a new pending selection, then undo the PPQN change —
    // the observed PPQN moves, so the gesture must drop its pending
    // state instead of arming a confirm against a stale grid.
    {
        let mut svc = f.svc.borrow_mut();
        svc.app_mut()
            .ppqn_select(1920, PpqnConversionPolicy::Exact)
            .unwrap();
    }
    ok(&mut f.rt, "syn.project.undo", json!({}));
    assert_eq!(live_ppqn(&f), 960);
    {
        let svc = f.svc.borrow();
        let app = svc.app();
        assert_eq!(app.ppqn_ui().phase(), PpqnPhase::Idle);
        assert_eq!(app.ppqn_ui().current_ppqn(), 960);
    }
    let screen = screen_state(&f);
    assert_eq!(screen.ppqn.current_ppqn, 960);
    assert_eq!(screen.ppqn.pending_new_ppqn, None);

    // Redo re-applies 480 through the same op path.
    ok(&mut f.rt, "syn.project.redo", json!({}));
    assert_eq!(live_ppqn(&f), 480);
}

/// Out-of-range candidates are rejected at the gesture boundary
/// before any state changes — `0` and `32_768` fail while the
/// `1..=32767` edges select cleanly.
#[test]
fn select_rejects_out_of_range_values() {
    let f = fixture();
    {
        let mut svc = f.svc.borrow_mut();
        let app = svc.app_mut();
        for bad in [0_u32, 32_768] {
            let err = app
                .ppqn_select(bad, PpqnConversionPolicy::Exact)
                .expect_err("out-of-range select rejected");
            assert!(matches!(err, PpqnUiError::OutOfRange { value } if value == bad));
        }
        assert_eq!(app.ppqn_ui().phase(), PpqnPhase::Idle);
    }
    // Direct read-model check on the boundaries.
    let mut ui = PpqnUi::new(960);
    ui.select(1, PpqnConversionPolicy::Exact).unwrap();
    ui.select(32_767, PpqnConversionPolicy::Exact).unwrap();
    assert!(matches!(
        ui.select(32_768, PpqnConversionPolicy::Exact),
        Err(PpqnUiError::OutOfRange { .. })
    ));
    assert_eq!(live_ppqn(&f), 960);
}

// ---------------------------------------------------------------------------
// Snap/Grid PPQN-change policy — explicit tick preservation
// ---------------------------------------------------------------------------

/// `SnapUnit::Explicit` is a fixed quantum in project ticks —
/// preserved verbatim across a PPQN change; only its musical spacing
/// moves. `Auto` is zoom-derived and reports no fixed quantum.
#[test]
fn snap_grid_policy_preserves_explicit_tick_quanta() {
    assert_eq!(DEFAULT_SNAP_QUANTUM_TICKS, 960);
    // Defaults stay the shipped 960-tick quantum.
    assert_eq!(
        SnapConfig::default().unit,
        SnapUnit::Explicit(DEFAULT_SNAP_QUANTUM_TICKS)
    );
    assert_eq!(
        GridConfig::default().unit,
        SnapUnit::Explicit(DEFAULT_SNAP_QUANTUM_TICKS)
    );
    assert_eq!(
        ViewState::default().snap.unit,
        SnapUnit::Explicit(DEFAULT_SNAP_QUANTUM_TICKS)
    );
    assert_eq!(
        ViewState::default().grid.unit,
        SnapUnit::Explicit(DEFAULT_SNAP_QUANTUM_TICKS)
    );

    // 960 ticks at PPQN 960 → 1.000 beat; at PPQN 480 → 2.000 beats.
    // The tick count itself is unchanged — that is the policy.
    let effect = snap_unit_ppqn_effect(SnapUnit::Explicit(960), 960, 480);
    assert_eq!(effect.quantum_ticks, Some(960));
    assert_eq!(effect.describe(), "960 ticks: 1.000 → 2.000 beats");

    // An arbitrary explicit quantum is preserved the same way —
    // rebinding would be lossy (100 ticks has no exact 480-grid
    // beat spacing), which is why the policy keeps ticks verbatim.
    let odd = snap_unit_ppqn_effect(SnapUnit::Explicit(100), 960, 480);
    assert_eq!(odd.quantum_ticks, Some(100));

    let auto = snap_unit_ppqn_effect(SnapUnit::Auto, 960, 480);
    assert_eq!(auto.quantum_ticks, None);
    assert_eq!(auto.describe(), "zoom-derived (auto)");
}

/// The preview op's `effects.snap_grid` reports the decided policy
/// concretely, and the mounted spec's `ppqn.effects` label carries
/// the same quantification.
#[test]
fn preview_and_spec_report_snap_grid_policy() {
    let mut f = fixture();
    let res = ok(
        &mut f.rt,
        "syn.project.preview_ppqn",
        json!({ "new_ppqn": 480, "policy": "exact" }),
    );
    let snap_grid = res["effects"]["snap_grid"].as_str().unwrap();
    assert!(snap_grid.contains("preserved"), "{snap_grid}");
    assert!(
        snap_grid.contains("960 ticks: 1.000 → 2.000 beats"),
        "{snap_grid}"
    );
    assert!(
        res["effects"]["smf_export"]
            .as_str()
            .unwrap()
            .contains("32767")
    );

    // Drive the gesture so the spec carries a live preview, then
    // read the rendered labels/tables back out of the spec.
    {
        let mut svc = f.svc.borrow_mut();
        let app = svc.app_mut();
        app.ppqn_select(480, PpqnConversionPolicy::Exact).unwrap();
        app.ppqn_preview_pending().unwrap();
    }
    let screen = screen_state(&f);
    let spec = main_screen_spec(&screen);
    let status = label_text(&spec.root, "ppqn.status");
    assert!(status.contains("PPQN 960"), "{status}");
    assert!(status.contains("1..=32767"), "{status}");
    assert!(status.contains("480, 960, 1000, 1920"), "{status}");
    let preview = label_text(&spec.root, "ppqn.preview");
    assert!(preview.contains("960 → 480"), "{preview}");
    let effects = label_text(&spec.root, "ppqn.effects");
    assert!(
        effects.contains("960 ticks: 1.000 → 2.000 beats"),
        "{effects}"
    );
    assert!(effects.contains("SMF export"), "{effects}");
    let sites = table_spec(&spec.root, "ppqn.sites");
    assert!(sites.selection.is_none(), "preview table is read-only");
    assert_eq!(
        sites.columns,
        ["Flag", "Kind", "Instance", "Part", "Tick", "Projected"]
    );
}

/// The Project tab exists in the mounted spec and renders the
/// no-preview state honestly — current PPQN, selectable range,
/// recommendations, and an empty flagged-site table.
#[test]
fn project_tab_renders_ppqn_surface() {
    let spec = main_screen_spec(&MainScreenState {
        ppqn: PpqnScreenState::default(),
        ..MainScreenState::default()
    });
    assert!(matches!(
        find_component(&spec.root, "tab.project.panel"),
        Some(ComponentSpec::Panel(_))
    ));
    assert_eq!(
        label_text(&spec.root, "ppqn.status"),
        "PPQN 960 — selectable 1..=32767 — recommended 480, 960, 1000, 1920"
    );
    assert_eq!(
        label_text(&spec.root, "ppqn.preview"),
        "PPQN — no pending selection"
    );
    let sites = table_spec(&spec.root, "ppqn.sites");
    assert_eq!(sites.rows.len(), 0);
    assert!(sites.selection.is_none());
}

// ---------- spec-tree helpers (same walk as view_mount.rs) ----------

/// Finds a component by key anywhere in the spec tree — Panels
/// recurse through `children`, Tabs through each tab's `child`.
fn find_component<'a>(spec: &'a ComponentSpec, name: &str) -> Option<&'a ComponentSpec> {
    if spec.key().0 == name {
        return Some(spec);
    }
    match spec {
        ComponentSpec::Panel(p) => p.children.iter().find_map(|c| find_component(c, name)),
        ComponentSpec::Tabs(t) => t
            .tabs
            .iter()
            .find_map(|tab| find_component(&tab.child, name)),
        _ => None,
    }
}

fn label_text(spec: &ComponentSpec, name: &str) -> String {
    match find_component(spec, name) {
        Some(ComponentSpec::Label(l)) => l.text.clone(),
        other => panic!("label `{name}` missing or wrong kind: {other:?}"),
    }
}

fn table_spec<'a>(spec: &'a ComponentSpec, name: &str) -> &'a ruviform_ui::view::TableSpec {
    match find_component(spec, name) {
        Some(ComponentSpec::Table(t)) => t,
        other => panic!("table `{name}` missing or wrong kind: {other:?}"),
    }
}
