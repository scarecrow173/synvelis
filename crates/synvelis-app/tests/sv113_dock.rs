//! SV#113 (SV-GUI-04) slice1 precursor — headless multi-window
//! `WindowHandle` + dock fixture verification.
//!
//! Covers the contract layer only, per
//! `docs/test-specs/sv-gui-04-01-headless-window-handle-dock.md`:
//!
//! - `HeadlessWindowHost` issues distinct `WindowId`s that share one
//!   `ProjectSession` through `WorkspaceManager`, and a window
//!   `Destroyed` (default `DetachViews` disposition) leaves the
//!   project, view registry, and view state intact.
//! - View state and focus→active-view resolution stay independent
//!   per window.
//! - `NativeShell` + `FakeDriver` reject a view command whose
//!   `origin_view` names a view attached to a *different* window
//!   (`shell.rs`'s existing validation — unchanged).
//! - `begin_transfer`/`commit_transfer`/`abort_transfer` and
//!   `detach_to_new_window` are reachable through the app host port.
//!
//! Nothing here asserts real OS window, GPUI driver, IME, or DPI
//! behavior — that evidence remains a separate gate (Issue #114).

use std::time::Duration;

use serde_json::{Value, json};

use ruviform_ui::ipc::{Event, Intent};
use ruviform_ui::{
    BackendReport, CloseDecision, DockTarget, DriverEvent, HostEvent, MonitorInfo, UiError,
    WindowConfig, WindowDriver, WindowHandle, WindowId, WindowLifecycle, WindowSpec,
};
use ruviform_ui_test::FakeDriver;

use synvelis_app::{HeadlessWindowHost, NativeShell, ShellWindowHost, SynvelisApp};
use synvelis_project::{CounterIdGenerator, Project, ProjectController, ProjectId, ProjectOp};
use synvelis_workspace::{ProjectSession, Tool, ViewType, WorkspaceManager};

// ---------------------------------------------------------------------------
// WorkspaceManager<HeadlessWindowHost> fixture
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<HeadlessWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0x113), "sv113");
    let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
    WorkspaceManager::new(
        HeadlessWindowHost::default(),
        ProjectSession::new(controller),
    )
}

fn two_windows() -> (Ws, WindowId, WindowId) {
    let mut ws = fresh_ws();
    let w1 = ws.create_window("main", 1280, 800).expect("first window");
    let w2 = ws
        .create_window("second", 1280, 800)
        .expect("second window");
    assert_ne!(w1, w2, "window identities must be distinct");
    (ws, w1, w2)
}

/// Rule H1 + H6: ids are monotonic/non-reused; the shell host still
/// refuses `create_window`.
#[test]
fn window_identity_is_monotonic_and_shell_host_stays_single_window() {
    let (mut ws, w1, w2) = two_windows();
    assert!(ws.host.windows().contains(&w1));
    assert!(ws.host.windows().contains(&w2));
    assert_eq!(ws.host.created.len(), 2);

    // Ids are never reused after destroy.
    ws.host.destroy_window(w2).expect("destroy w2");
    let w3 = ws.create_window("third", 1280, 800).expect("third window");
    assert_ne!(w3, w2, "destroyed id must not be reissued");
    assert!(ws.host.windows().contains(&w3));

    // Delivery-safety contract on the dead window: fallible ops fail
    // with `UiError::Host`; fire-and-forget ops are silently dropped.
    for result in [
        ws.host.resize(w2, 640, 480),
        ws.host.focus(w2, true),
        ws.host.respond_close(w2, CloseDecision::Approve),
        ws.host.destroy_window(w2),
    ] {
        assert!(
            matches!(result, Err(UiError::Host { .. })),
            "dead window must fail with UiError::Host, got {result:?}"
        );
    }
    ws.host.eval(w2, "dead()");
    ws.host.request_close(w2);
    assert!(ws.host.evals.iter().all(|(id, _)| *id != w2));
    assert!(!ws.host.close_requests.contains(&w2));

    // `eval` to a live window is recorded.
    ws.host.eval(w1, "live()");
    assert_eq!(ws.host.evals.last().map(|(id, _)| *id), Some(w1));

    // `fail_create` injection fails honestly — no id issued, no
    // phantom window recorded.
    ws.host.fail_create = true;
    let err = ws
        .host
        .create_window(&WindowConfig::default())
        .expect_err("injected create failure");
    assert!(
        matches!(err, UiError::BackendUnavailable { .. }),
        "expected BackendUnavailable, got {err:?}"
    );
    assert_eq!(ws.host.created.len(), 3);
    ws.host.fail_create = false;

    // `with_monitors` seeds the snapshot `monitors()` reports.
    let monitor = HeadlessWindowHost::with_monitors(vec![MonitorInfo {
        name: Some("test".to_string()),
        width: 1920,
        height: 1080,
        x: 0,
        y: 0,
        scale_factor: 1.0,
    }]);
    assert_eq!(monitor.monitors().len(), 1);

    // Close flow: `request_close` records, `Approve` removes the
    // window as destroyed, `Veto`/`Defer` keep it live.
    let w4 = ws.create_window("closer", 800, 600).expect("closer window");
    ws.host.request_close(w4);
    assert!(ws.host.close_requests.contains(&w4));
    ws.host
        .respond_close(w4, CloseDecision::Approve)
        .expect("approve close");
    assert!(!ws.host.windows().contains(&w4));
    assert!(ws.host.destroys.contains(&w4));

    for (title, decision) in [
        ("vetod", CloseDecision::Veto),
        ("deferd", CloseDecision::Defer),
    ] {
        let w = ws.create_window(title, 800, 600).expect("window");
        ws.host.request_close(w);
        ws.host.respond_close(w, decision).expect("respond close");
        assert!(ws.host.windows().contains(&w), "{decision:?} keeps window");
        assert!(!ws.host.destroys.contains(&w));
    }

    // `ShellWindowHost` keeps its honest single-window contract.
    let mut shell_host = ShellWindowHost::new();
    let err = shell_host
        .create_window(&WindowConfig::default())
        .expect_err("shell host must refuse create_window");
    assert!(
        matches!(err, UiError::BackendUnavailable { .. }),
        "expected BackendUnavailable, got {err:?}"
    );
}

/// Rule H2: two windows share one project; a window `Destroyed` does
/// not stop the project or discard views/states.
#[test]
fn two_windows_share_project_and_survive_window_close() {
    let (mut ws, w1, w2) = two_windows();
    let view_a = ws
        .spawn_view(ViewType::Arrangement, "arr-a", DockTarget::Window(w1))
        .expect("spawn on w1");
    let view_b = ws
        .spawn_view(ViewType::Mixer, "mix-b", DockTarget::Window(w2))
        .expect("spawn on w2");
    assert_eq!(ws.dock().owner_of(view_a), Some(w1));
    assert_eq!(ws.dock().owner_of(view_b), Some(w2));

    let project_id = ws.session().project().project_id;
    let revision = ws.session().revision();
    ws.session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "renamed".to_string(),
            },
            revision,
        )
        .expect("rename applies once for the shared session");
    assert_eq!(ws.session().project().name, "renamed");

    // Host-side destroy + consumer teardown. The default sub-window
    // disposition is `DetachViews`: the view moves to the detached
    // pool, the project/session is untouched.
    ws.host.destroy_window(w2).expect("host destroy w2");
    ws.on_window_event(w2, &HostEvent::Destroyed)
        .expect("window event");

    assert!(!ws.host.windows().contains(&w2));
    assert!(ws.dock().is_detached(view_b), "view_b detached, kept");
    assert!(ws.registry().descriptor(view_b).is_some());
    assert!(ws.states().get(view_b).is_some());
    assert_eq!(ws.session().project().project_id, project_id);
    assert_eq!(ws.session().project().name, "renamed");
    assert!(ws.session().revision() > revision);

    // The surviving window is unaffected, and the detached view
    // reattaches — window lifetime never owned the view.
    assert_eq!(ws.dock().owner_of(view_a), Some(w1));
    ws.reattach(view_b, w1).expect("reattach view_b to w1");
    assert_eq!(ws.dock().owner_of(view_b), Some(w1));
    assert!(!ws.dock().is_detached(view_b));
}

/// Rule H3: per-window view state and focus→active-view resolution
/// are independent.
#[test]
fn view_state_and_focus_are_independent_per_window() {
    let (mut ws, w1, w2) = two_windows();
    let view_a = ws
        .spawn_view(ViewType::Arrangement, "arr-a", DockTarget::Window(w1))
        .expect("spawn on w1");
    let view_b = ws
        .spawn_view(ViewType::Inspector, "insp-b", DockTarget::Window(w2))
        .expect("spawn on w2");

    ws.set_tool(view_a, Tool::Trim).expect("set tool on view_a");
    {
        let state_a = ws.state_mut(view_a).expect("state_a");
        state_a.viewport.scroll_x = 42.0;
        state_a.viewport.scroll_y = 7.0;
    }

    let state_b = ws.state(view_b).expect("state_b");
    assert_eq!(state_b.active_tool, Tool::Smart);
    assert_eq!(state_b.viewport.scroll_x, 0.0);
    assert_eq!(state_b.viewport.scroll_y, 0.0);
    assert_eq!(state_b.viewport.zoom_x, 1.0);

    // Focus resolves the active view from the focused window's own
    // dock tree — w1 focus picks view_a, w2 focus picks view_b.
    ws.on_window_event(w1, &HostEvent::Focused(true))
        .expect("focus w1");
    assert_eq!(ws.focus().native_window, Some(w1));
    assert_eq!(ws.focus().active_view, Some(view_a));

    ws.on_window_event(w2, &HostEvent::Focused(true))
        .expect("focus w2");
    assert_eq!(ws.focus().native_window, Some(w2));
    assert_eq!(ws.focus().active_view, Some(view_b));

    // Losing w1's focus report while w2 is native-focused changes
    // nothing — focus state is per-window, not global-last-writer.
    ws.on_window_event(w1, &HostEvent::Focused(false))
        .expect("unfocus w1");
    assert_eq!(ws.focus().native_window, Some(w2));
    assert_eq!(ws.focus().active_view, Some(view_b));
}

/// Rule H5: begin/commit/abort transfers and `detach_to_new_window`
/// are reachable through the app host contract.
#[test]
fn dock_transfers_and_detach_reach_through_app_host() {
    let (mut ws, w1, w2) = two_windows();
    let mover = ws
        .spawn_view(ViewType::Inspector, "insp", DockTarget::Window(w1))
        .expect("spawn mover on w1");
    let stayer = ws
        .spawn_view(ViewType::Mixer, "mix", DockTarget::Window(w1))
        .expect("spawn stayer on w1");

    // begin → commit moves ownership w1 → w2.
    let token = ws
        .begin_transfer(mover, DockTarget::Window(w2))
        .expect("begin transfer");
    let report = ws.commit_transfer(token).expect("commit transfer");
    assert_eq!(report.view, mover);
    assert_eq!(report.from, Some(w1));
    assert_eq!(report.to, w2);
    assert_eq!(ws.dock().owner_of(mover), Some(w2));

    // begin → abort keeps the view at its source window.
    let token = ws
        .begin_transfer(stayer, DockTarget::Window(w2))
        .expect("begin second transfer");
    ws.abort_transfer(token).expect("abort transfer");
    assert_eq!(ws.dock().owner_of(stayer), Some(w1));
    assert!(!ws.dock().is_transferring(stayer));

    // `detach_to_new_window` goes through `WindowHandle::create_window`
    // — the path `ShellWindowHost` refuses today.
    let w3 = ws
        .detach_to_new_window(stayer, "floater")
        .expect("detach to new window");
    assert!(ws.host.windows().contains(&w3));
    assert_eq!(ws.dock().owner_of(stayer), Some(w3));
    assert_eq!(ws.windows().len(), 3);
    assert_eq!(
        ws.host.created.last().map(|(_, c)| c.title.as_str()),
        Some("floater")
    );
}

// ---------------------------------------------------------------------------
// NativeShell + FakeDriver fixture — cross-window origin_view refusal
// ---------------------------------------------------------------------------

fn shell_harness() -> (NativeShell, FakeDriver) {
    let app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let shell = NativeShell::new(app, Duration::from_millis(16)).expect("compose shell");
    (shell, FakeDriver::new())
}

fn pump(shell: &mut NativeShell, driver: &mut FakeDriver) {
    driver.pump(&mut |ev, d| shell.handle(ev, d));
}

fn inject(shell: &mut NativeShell, driver: &mut FakeDriver, ev: DriverEvent) {
    driver.inject(ev);
    pump(shell, driver);
}

fn ready_window(shell: &mut NativeShell, driver: &mut FakeDriver) -> WindowId {
    let window = shell.open_main(driver).expect("main window");
    for event in [WindowLifecycle::Created, WindowLifecycle::Ready] {
        inject(shell, driver, DriverEvent::Windowed { window, event });
    }
    assert!(shell.is_ready(window));
    window
}

fn second_window(shell: &mut NativeShell, driver: &mut FakeDriver) -> WindowId {
    let window = driver
        .create_window(&WindowSpec::default())
        .expect("second driver window");
    for event in [WindowLifecycle::Created, WindowLifecycle::Ready] {
        inject(shell, driver, DriverEvent::Windowed { window, event });
    }
    assert!(shell.is_ready(window));
    window
}

/// Result payloads delivered to `window` for intent id `id`.
fn results_for(
    driver: &FakeDriver,
    window: WindowId,
    id: &str,
) -> Vec<(bool, Option<Value>, Option<ruviform_ui::ipc::ErrorBody>)> {
    driver
        .events_for(window)
        .iter()
        .filter_map(|e| match e {
            Event::Result {
                id: rid,
                ok,
                data,
                error,
            } if rid == id => Some((*ok, data.clone(), error.clone())),
            _ => None,
        })
        .collect()
}

/// A `rvf.ui.command.v1` action intent (`tab-select` on the view-local
/// target) carrying `origin_view`.
fn view_command(id: &str, origin_view: &str) -> Intent {
    Intent::new(
        id,
        "rvf.ui.command.v1",
        json!({
            "command_id": id,
            "expect_receipt": false,
            "intent": {
                "kind": "action",
                "target": "view",
                "control_id": "tabs",
                "sequence": 1u64,
                "origin_view": origin_view,
                "action": "tab-select",
            },
        }),
    )
}

fn send_command(
    shell: &mut NativeShell,
    driver: &mut FakeDriver,
    window: WindowId,
    intent: Intent,
) {
    inject(
        shell,
        driver,
        DriverEvent::Windowed {
            window,
            event: WindowLifecycle::Action(intent),
        },
    );
}

/// Rule H4: a view command whose `origin_view` names a view attached
/// to a different window is refused with `invalid_params`; the same
/// command naming this window's own view is accepted.
#[test]
fn cross_window_origin_view_is_rejected() {
    let (mut shell, mut driver) = shell_harness();
    let w1 = ready_window(&mut shell, &mut driver);
    let w2 = second_window(&mut shell, &mut driver);

    shell
        .attach_view(&mut driver, w1, "view-a", &[])
        .expect("attach view-a to w1");
    shell
        .attach_view(&mut driver, w2, "view-b", &[])
        .expect("attach view-b to w2");
    assert_eq!(shell.view_window("view-a"), Some(w1));
    assert_eq!(shell.view_window("view-b"), Some(w2));

    // view-b lives on w2 — commanding it from w1 must fail closed.
    send_command(
        &mut shell,
        &mut driver,
        w1,
        view_command("cmd-cross", "view-b"),
    );
    let results = results_for(&driver, w1, "cmd-cross");
    let (ok, _, error) = results.last().expect("rejection result delivered to w1");
    assert!(!ok, "cross-window origin_view must be rejected");
    let error = error.as_ref().expect("typed error body");
    assert_eq!(error.code, "invalid_params");
    assert!(
        error.message.contains("view-b") && error.message.contains("not this one"),
        "rejection names the foreign origin: {}",
        error.message
    );

    // Symmetric: view-a commanded from w2 is refused too.
    send_command(
        &mut shell,
        &mut driver,
        w2,
        view_command("cmd-cross2", "view-a"),
    );
    let results = results_for(&driver, w2, "cmd-cross2");
    assert!(
        results.last().is_some_and(|(ok, _, _)| !*ok),
        "w2 must not mint commands for w1's view"
    );

    // Same-window origin_view is accepted — the view-local
    // `tab-select` action resolves to an ok Result.
    send_command(
        &mut shell,
        &mut driver,
        w1,
        view_command("cmd-same", "view-a"),
    );
    let results = results_for(&driver, w1, "cmd-same");
    let (ok, _, _) = results.last().expect("acceptance result delivered");
    assert!(ok, "same-window origin_view must be accepted");
}
