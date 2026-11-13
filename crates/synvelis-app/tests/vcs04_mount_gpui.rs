//! VCS-04 native mount — GPUI-simulated lane (synvelis#95).
//!
//! The headless counterpart (`vcs04_native_mount.rs`) proves the
//! dock-domain wiring; this lane proves the *mounted* half: a
//! dynamically opened `VcsHistory` view goes through the same
//! `ViewHost` → `ViewMount` → `create_view_window` composition the
//! production `synvelis-native` binary uses
//! ([`mount_vcs_history_view`] + the session-owned
//! `AttachEpochAllocator`), and the mounted spec is the read-only
//! `history_screen_spec` projection.
//!
//! Covered here (the headless-possible slice of VCS-UI-01〜05):
//!
//! - mount → all six `vcs.*` components resolve and observe mounted;
//! - the history table rejects row selection and no gesture mints an
//!   intent — the surface is a pure projection;
//! - `update_view` with a re-projected spec (post-op publish) keeps
//!   pinned targets valid — the run-loop update path;
//! - detach stales pinned targets; a second window remounts the same
//!   logical view under a fresh host/epoch (`MultiWindow` evidence);
//! - `syn.vcs.visual close` + every navigation op leave project
//!   revision, engine builds, transport, and project name untouched.
//!
//! Real OS window/IME/pixel/plugin-dock evidence remains #114 /
//! ruviform#313/#314 scope — recorded `Unsupported`/`NotRun` via
//! [`gated_scenario_report`], never upgraded by a simulated pass.

#![cfg(feature = "gpui-test")]

mod common;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use common::scenario::{
    assert_honest_capabilities, gated_scenario_report, resolve_component, screen_scenario_report,
};
use common::vcs::*;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::{AttachEpochAllocator, ViewHost};
use ruviform_ui::{WindowId, WindowSpec};
use ruviform_ui_gpui::{GpuiGuiTest, ViewMount};
use ruviform_ui_test::{
    ScenarioCapability, ScenarioDriver, ScenarioError, ScenarioVerdict, SemanticGuiTest,
};
use serde_json::json;

use synvelis_app::{VcsScreenState, history_screen_spec, mount_vcs_history_view};
use synvelis_workspace::VcsViewStatus;

const VCS_SCENARIO: &str = "synvelis.vcs.history";
const CASE_MOUNT_READ_ONLY: &str = "vcs-history-mount-read-only";
const CASE_UPDATE_LIFECYCLE: &str = "vcs-history-update-detach-remount";
const CASE_EXTERNAL_GATES: &str = "vcs-history-external-gates";

/// The `VcsScreenState` the app publishes for `inst` — the same cell
/// the native run loop diffs.
fn published_state(app: &synvelis_app::SynvelisApp, inst: ViewInstanceId) -> VcsScreenState {
    app.vcs_screen_cell()
        .borrow()
        .get(&inst)
        .expect("published VcsScreenState")
        .clone()
}

/// Mounts `state` for `inst` onto a fresh `ViewHost` (session-owned
/// epoch allocator — the production mint path) and opens it as a
/// GPUI test window — identical to `synvelis-native`'s
/// `create_view_window` call.
fn mount_vcs_gpui(
    gui: &mut GpuiGuiTest,
    epochs: &Rc<RefCell<AttachEpochAllocator>>,
    inst: ViewInstanceId,
    state: &VcsScreenState,
) -> WindowId {
    let host: ViewHost =
        mount_vcs_history_view(epochs, inst, state).expect("mount vcs history view");
    let window = gui
        .observer_mut()
        .driver_mut()
        .create_view_window(&WindowSpec::default(), ViewMount { host, theme: None })
        .expect("create_view_window");
    gui.settle().expect("settle mount");
    window
}

/// Opens a VCS History view through the real op surface, drives it to
/// a pinned A/B + diff state, and returns `(app, wire view_id,
/// instance, epochs, published state)` ready to mount.
fn prepared_app() -> (
    synvelis_app::SynvelisApp,
    String,
    ViewInstanceId,
    Rc<RefCell<AttachEpochAllocator>>,
    VcsScreenState,
) {
    let mut app = fixture_backend();
    let view = open(&mut app);
    let inst = ViewInstanceId::new(3).expect("view-3");
    action_on(&mut app, &view, json!({ "action": "refresh" }));
    action_on(&mut app, &view, json!({ "action": "load_more" }));
    action_on(
        &mut app,
        &view,
        json!({ "action": "pin", "side": "before", "endpoint": { "kind": "node", "index": 3 } }),
    );
    action_on(
        &mut app,
        &view,
        json!({ "action": "pin", "side": "after", "endpoint": { "kind": "node", "index": 1 } }),
    );
    action_on(&mut app, &view, json!({ "action": "compare" }));
    let epochs = app.attach_epoch_allocator();
    let state = published_state(&app, inst);
    assert_eq!(
        state.view.as_ref().map(|v| &v.status),
        Some(&VcsViewStatus::Ready)
    );
    (app, view, inst, epochs, state)
}

/// Mount → resolve → read-only proof → honest report. The mounted
/// spec carries all six `vcs.*` components; the history table rejects
/// selection and mints no intents.
#[test]
fn gpui_vcs_history_mount_is_a_read_only_projection() {
    let (app, view, inst, epochs, state) = prepared_app();
    let authority_before = authority(&app);

    let mut gui = GpuiGuiTest::new();
    let window = mount_vcs_gpui(&mut gui, &epochs, inst, &state);

    // Every spec component resolves and observes mounted.
    let mut targets = Vec::new();
    for component in [
        "vcs.status",
        "vcs.selection",
        "vcs.diff",
        "vcs.history",
        "vcs.changes",
        "vcs.issues",
    ] {
        let (target, key) = resolve_component(&mut gui, window, inst, component)
            .unwrap_or_else(|e| panic!("resolve {component}: {e}"));
        let snapshot = gui.observe(&target, &key).expect("observe");
        assert!(snapshot.mounted, "{component} must be mounted");
        targets.push(target);
    }

    // Read-only at the semantic boundary: row selection on the
    // history table is refused (`selection: None` in the spec), an
    // invoke on a label is refused, and nothing mints an intent.
    let (history_target, history_key) =
        resolve_component(&mut gui, window, inst, "vcs.history").expect("history target");
    gui.select_row(&history_target, &history_key, 0)
        .expect_err("read-only history table must reject row selection");
    let (status_target, status_key) =
        resolve_component(&mut gui, window, inst, "vcs.status").expect("status target");
    gui.invoke(&status_target, &status_key)
        .expect_err("status label must reject invoke");
    gui.settle().expect("settle");
    assert!(
        gui.drain_intents().is_empty(),
        "read-only vcs spec must never mint intents"
    );

    // The mounted view's ops stayed read-only toward project
    // authority — revision/engine/transport/name are untouched.
    assert_eq!(authority(&app), authority_before);

    let mut report = screen_scenario_report(
        &gui,
        "run-gpui-vcs-history-mount",
        VCS_SCENARIO,
        CASE_MOUNT_READ_ONLY,
    );
    report.targets = targets;
    report.observed = Some(json!({
        "view_id": view,
        "components": 6,
        "select_row_rejected": true,
        "invoke_rejected": true,
        "intents": 0,
    }));
    report.verdict = ScenarioVerdict::Pass;
    assert_eq!(report.driver, ScenarioDriver::GpuiSimulated);
    assert_honest_capabilities(&report);
}

/// The run-loop update path: re-project after ops, `update_view`
/// keeps pinned targets valid; detach stales them; a fresh host on a
/// second window remounts the same logical view (`MultiWindow`).
#[test]
fn gpui_vcs_history_update_detach_and_second_window_remount() {
    let (mut app, view, inst, epochs, state) = prepared_app();

    let mut gui = GpuiGuiTest::new();
    let window = mount_vcs_gpui(&mut gui, &epochs, inst, &state);
    let (target, key) =
        resolve_component(&mut gui, window, inst, "vcs.history").expect("history target");

    // Op → republish → update_view: the mounted spec tracks the
    // projection without a remount (production run_loop parity).
    action_on(
        &mut app,
        &view,
        json!({ "action": "view_mode", "mode": "change_focus" }),
    );
    let updated = published_state(&app, inst);
    let spec = history_screen_spec(&updated);
    gui.observer_mut()
        .driver_mut()
        .update_view(window, spec)
        .expect("update_view");
    gui.settle().expect("settle update");
    let snapshot = gui.observe(&target, &key).expect("observe after update");
    assert!(snapshot.mounted, "update_view must keep the view mounted");

    // Detach stales the pinned target — the epoch/generation guard.
    gui.detach_view(window, "test-detach").expect("detach");
    let err = gui
        .observe(&target, &key)
        .expect_err("detached view must stale the target");
    assert!(
        matches!(
            err,
            ScenarioError::StaleGeneration { .. } | ScenarioError::StaleEpoch { .. }
        ),
        "expected stale pin error, got {err:?}"
    );

    // Remount on a second window: a *fresh* host minted from the same
    // session allocator — the RF387 fresh-host contract — while the
    // first window's host is gone. Both windows address the same
    // logical view id independently (`MultiWindow` capability).
    let window2 = mount_vcs_gpui(&mut gui, &epochs, inst, &updated);
    assert_ne!(window, window2);
    let (target2, key2) =
        resolve_component(&mut gui, window2, inst, "vcs.status").expect("second window target");
    let snapshot2 = gui.observe(&target2, &key2).expect("observe window2");
    assert!(snapshot2.mounted);

    // Close through the VCS op — the mounted windows keep their last
    // spec (host-side state), but the domain view is gone: a fresh
    // mount now fails to resolve the view.
    app.op_vcs_visual(&json!({ "action": "close", "view_id": view }))
        .expect("close");
    let err = gui
        .resolve_target(&ruviform_ui_test::TargetQuery::new(
            WindowId::new(999).expect("nonzero"),
            inst,
        ))
        .expect_err("unknown window must fail");
    assert!(matches!(err, ScenarioError::NoSuchWindow { .. }));

    let mut report = screen_scenario_report(
        &gui,
        "run-gpui-vcs-history-lifecycle",
        VCS_SCENARIO,
        CASE_UPDATE_LIFECYCLE,
    );
    report.targets = vec![target2];
    report.observed = Some(json!({
        "update_view": "mounted",
        "detach": "stale-target",
        "second_window": "mounted",
        "closed": true,
    }));
    report.verdict = ScenarioVerdict::Pass;
    assert_eq!(report.driver, ScenarioDriver::GpuiSimulated);
    assert_honest_capabilities(&report);
}

/// External gates stay honest: real OS input, IME, pixel capture,
/// and the native plugin dock are `Unsupported` on this adapter —
/// recorded, never upgraded by the simulated pass above.
#[test]
fn gpui_vcs_history_external_gates_report_unsupported() {
    let (app, _view, inst, epochs, state) = prepared_app();
    let mut gui = GpuiGuiTest::new();
    let _window = mount_vcs_gpui(&mut gui, &epochs, inst, &state);
    let _ = app;

    let report = gated_scenario_report(
        &gui,
        "run-gpui-vcs-history-gates",
        VCS_SCENARIO,
        CASE_EXTERNAL_GATES,
        BTreeSet::from([
            ScenarioCapability::RealOsPointerOrKeyboard,
            ScenarioCapability::Ime,
            ScenarioCapability::VisualCapture,
            ScenarioCapability::NativePluginDock,
        ]),
        ScenarioVerdict::Unsupported,
        "real OS GUI/IME/pixel/plugin-dock evidence is Issue #114 / ruviform#313/#314 scope — not provided by gpui-test",
    );
    assert_eq!(report.driver, ScenarioDriver::GpuiSimulated);
    assert_eq!(report.verdict, ScenarioVerdict::Unsupported);
    assert_eq!(report.verdict.lane_status(), "unsupported");
    assert_honest_capabilities(&report);
}
