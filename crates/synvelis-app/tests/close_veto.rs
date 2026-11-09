//! `syn.window.close_guard` + last-window close veto over `FakeDriver`
//! (SV-GUI-04 slice 2, synvelis#113).
//!
//! The headless half of the close-cancel claim: `CloseRequested` on
//! the *last* live window is answered by the app-global guard query —
//! dirty project (`live_revision` past `saved`) or an open recording
//! transaction vetoes the close, a clean project approves it, and a
//! query that cannot produce a well-formed empty `reasons` fails
//! closed to `Veto`. Non-last windows close unconditionally because
//! the shared `ProjectSession` survives in the remaining windows, so
//! `syn.app.quit` over several windows can partially veto.
//!
//! Dirty state is minted before the app seals (`workspace_mut()` +
//! `ProjectSession::apply`) — the same `live_revision`/`saved`
//! contract `syn.project.info` reports — because no `syn.*` op applies
//! a bare project rename. Nothing here asserts real OS window
//! close/focus evidence — that remains the #114 native gate.

use std::time::Duration;

use serde_json::{Value, json};

use ruviform_ui::ipc::{Event, Intent};
use ruviform_ui::{
    BackendReport, DriverEvent, WindowDriver, WindowId, WindowLifecycle, WindowSpec,
};
use ruviform_ui_test::FakeDriver;

use synvelis_app::{NativeShell, SynvelisApp};
use synvelis_project::ProjectOp;

fn app() -> SynvelisApp {
    SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()))
}

/// Dirty variant: one `SetProjectName` past the saved baseline.
/// `saved = true` first runs `syn.project.new`'s composition path
/// (`project_new` saves the initial revision), so the guard sees
/// `live_revision != saved().revision`; `saved = false` leaves the
/// boot project never-saved, so `live_revision > 0` alone is dirty.
fn dirty_app(saved: bool) -> SynvelisApp {
    let mut app = app();
    if saved {
        let dir =
            std::env::temp_dir().join(format!("sv113-close-veto-{}-saved", std::process::id()));
        let path = dir.to_string_lossy().to_string();
        let _ = std::fs::remove_dir_all(&dir);
        app.project_new(&path, Some("guard")).expect("project.new");
    }
    let revision = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "dirty".to_string(),
            },
            revision,
        )
        .expect("rename applies");
    app
}

fn harness_with(app: SynvelisApp) -> (NativeShell, FakeDriver) {
    let shell = NativeShell::new(app, Duration::from_millis(16)).expect("compose shell");
    (shell, FakeDriver::new())
}

fn harness() -> (NativeShell, FakeDriver) {
    harness_with(app())
}

fn pump(shell: &mut NativeShell, driver: &mut FakeDriver) {
    driver.pump(&mut |ev, d| shell.handle(ev, d));
}

fn inject(shell: &mut NativeShell, driver: &mut FakeDriver, ev: DriverEvent) {
    driver.inject(ev);
    pump(shell, driver);
}

fn action(shell: &mut NativeShell, driver: &mut FakeDriver, window: WindowId, intent: Intent) {
    inject(
        shell,
        driver,
        DriverEvent::Windowed {
            window,
            event: WindowLifecycle::Action(intent),
        },
    );
}

fn ready_window(shell: &mut NativeShell, driver: &mut FakeDriver) -> WindowId {
    let window = shell.open_main(driver).expect("main window");
    inject(
        shell,
        driver,
        DriverEvent::Windowed {
            window,
            event: WindowLifecycle::Ready,
        },
    );
    assert!(shell.is_ready(window));
    window
}

fn second_window(shell: &mut NativeShell, driver: &mut FakeDriver) -> WindowId {
    let w = driver
        .create_window(&WindowSpec::default())
        .expect("second window");
    inject(
        shell,
        driver,
        DriverEvent::Windowed {
            window: w,
            event: WindowLifecycle::Ready,
        },
    );
    assert!(shell.is_ready(w));
    w
}

fn close_request(shell: &mut NativeShell, driver: &mut FakeDriver, window: WindowId) {
    inject(
        shell,
        driver,
        DriverEvent::Windowed {
            window,
            event: WindowLifecycle::CloseRequested,
        },
    );
}

/// Result payloads (`data`) delivered to `window` for `id` intents.
fn results_for(driver: &FakeDriver, window: WindowId, id: &str) -> Vec<(bool, Option<Value>)> {
    driver
        .events_for(window)
        .iter()
        .filter_map(|e| match e {
            Event::Result {
                id: rid, ok, data, ..
            } if rid == id => Some((*ok, data.clone())),
            _ => None,
        })
        .collect()
}

fn guard_result(driver: &FakeDriver, window: WindowId) -> (bool, Option<Value>) {
    let id = format!("shell-close-guard-{window}");
    results_for(driver, window, &id)
        .last()
        .cloned()
        .unwrap_or_else(|| panic!("no close-guard result on {window}"))
}

fn project_info(
    shell: &mut NativeShell,
    driver: &mut FakeDriver,
    window: WindowId,
    id: &str,
) -> Value {
    action(
        shell,
        driver,
        window,
        Intent::new(id, "syn.project.info", json!({})),
    );
    let (ok, data) = results_for(driver, window, id)
        .last()
        .cloned()
        .expect("project.info result");
    assert!(ok, "project.info failed: {data:?}");
    data.unwrap()
}

fn view_list(
    shell: &mut NativeShell,
    driver: &mut FakeDriver,
    window: WindowId,
    id: &str,
) -> Vec<String> {
    action(
        shell,
        driver,
        window,
        Intent::new(id, "syn.view.list", json!({})),
    );
    let (ok, data) = results_for(driver, window, id)
        .last()
        .cloned()
        .expect("view.list result");
    assert!(ok, "view.list failed: {data:?}");
    data.unwrap()["views"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v["view_id"].as_str().map(str::to_string))
        .collect()
}

fn attach(shell: &mut NativeShell, driver: &mut FakeDriver, window: WindowId, view: &str) {
    action(
        shell,
        driver,
        window,
        Intent::new(
            format!("{view}-attach"),
            "syn.view.attach",
            json!({ "view_id": view, "window_id": window.to_string() }),
        ),
    );
    let id = format!("{view}-attach");
    let results = results_for(driver, window, &id);
    let (ok, data) = results.last().expect("attach result delivered");
    assert!(ok, "attach failed: {data:?}");
}

// ---------------------------------------------------------------------
// syn.window.close_guard — the app-global reason query
// ---------------------------------------------------------------------

/// RULE-8/EX-5: the op reports `reasons` — empty on a clean boot
/// project, `unsaved_changes` once the live revision passes the
/// saved baseline (never-saved and saved-then-dirty alike).
#[test]
fn close_guard_reports_unsaved_changes() {
    for (name, app) in [
        ("clean", app()),
        ("dirty-never-saved", dirty_app(false)),
        ("dirty-after-save", dirty_app(true)),
    ] {
        let (mut shell, mut driver) = harness_with(app);
        let window = ready_window(&mut shell, &mut driver);
        action(
            &mut shell,
            &mut driver,
            window,
            Intent::new("cg", "syn.window.close_guard", json!({})),
        );
        let (ok, data) = results_for(&driver, window, "cg")
            .last()
            .cloned()
            .expect("close_guard result");
        assert!(ok, "{name}: close_guard failed: {data:?}");
        let reasons = data.unwrap()["reasons"].as_array().unwrap().clone();
        let reasons: Vec<&str> = reasons.iter().filter_map(Value::as_str).collect();
        match name {
            "clean" => assert!(reasons.is_empty(), "{name}: {reasons:?}"),
            _ => assert_eq!(reasons, ["unsaved_changes"], "{name}: {reasons:?}"),
        }
    }
}

// ---------------------------------------------------------------------
// Last-window veto / approve
// ---------------------------------------------------------------------

/// EX-2/RULE-2+5: a dirty last window's `CloseRequested` is vetoed —
/// the window, its view, and the shared project/session all survive,
/// and no `exit` is requested.
#[test]
fn dirty_last_window_close_is_vetoed() {
    let (mut shell, mut driver) = harness_with(dirty_app(false));
    let window = ready_window(&mut shell, &mut driver);
    attach(&mut shell, &mut driver, window, "console");

    close_request(&mut shell, &mut driver, window);

    // `lifecycle(Closing)` still reached the window — the veto
    // happens after the notification, never instead of it.
    let saw_closing = driver.events_for(window).iter().any(|e| {
        matches!(e, Event::Lifecycle { state } if matches!(state, ruviform_ui::ipc::LifecycleState::Closing))
    });
    assert!(saw_closing, "closing lifecycle event delivered");

    // The guard query ran and reported the dirty reason.
    let (ok, data) = guard_result(&driver, window);
    assert!(ok, "guard query failed: {data:?}");
    let reasons = data.unwrap()["reasons"].as_array().unwrap().clone();
    assert!(
        reasons.iter().any(|r| r == "unsaved_changes"),
        "guard reports unsaved_changes: {reasons:?}"
    );

    // Veto: the window is alive, nothing was destroyed, no exit.
    assert!(driver.windows().contains(&window), "window survives");
    assert!(!driver.destroyed().contains(&window), "no destroy queued");
    assert!(!driver.exited(), "session keeps the loop alive");
    assert!(!driver.close_pending(window), "veto clears pending close");

    // The shared session is untouched: project identity/revision and
    // the window's view registry are exactly as before the request.
    let info = project_info(&mut shell, &mut driver, window, "pi-after-veto");
    assert_eq!(info["name"], "dirty");
    assert_eq!(info["revision"], 1, "unsaved revision preserved");
    assert_eq!(info["saved"], false);
    let views = view_list(&mut shell, &mut driver, window, "vl-after-veto");
    assert!(
        views.contains(&"console".to_string()),
        "view survives: {views:?}"
    );
}

/// EX-1/RULE-2+6: a clean last window's close approves — the guard
/// query runs, reports no reasons, and the normal `Destroyed`
/// teardown + last-window `exit` follows.
#[test]
fn clean_last_window_close_approves_and_exits() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);

    close_request(&mut shell, &mut driver, window);

    // The guard was consulted and answered clean.
    let (ok, data) = guard_result(&driver, window);
    assert!(ok, "guard query failed: {data:?}");
    assert_eq!(
        data.unwrap()["reasons"].as_array().unwrap().len(),
        0,
        "clean project reports no reasons"
    );

    assert!(driver.destroyed().contains(&window), "window destroyed");
    assert!(driver.exited(), "last-window destroy exits the loop");
    assert!(!driver.windows().contains(&window));
}

/// EX-3/RULE-4: with a second window live, closing the first is not
/// the last-window case — it approves without consulting the guard,
/// and the dirty project survives untouched on the remaining window.
#[test]
fn dirty_non_last_window_close_approves_without_guard_query() {
    let (mut shell, mut driver) = harness_with(dirty_app(false));
    let w1 = ready_window(&mut shell, &mut driver);
    let w2 = second_window(&mut shell, &mut driver);

    close_request(&mut shell, &mut driver, w1);

    assert!(driver.destroyed().contains(&w1), "w1 destroyed");
    assert!(driver.windows().contains(&w2), "w2 survives");
    assert!(!driver.exited(), "w2 keeps the loop alive");
    // Non-last close never issued the guard query — no
    // `shell-close-guard-*` result was posted to w1.
    let guard_id = format!("shell-close-guard-{w1}");
    assert!(
        results_for(&driver, w1, &guard_id).is_empty(),
        "non-last close skips the guard query"
    );

    // w2 is now the last window: its close *is* guarded and vetoed.
    close_request(&mut shell, &mut driver, w2);
    let (ok, data) = guard_result(&driver, w2);
    assert!(ok, "guard query failed: {data:?}");
    assert!(driver.windows().contains(&w2), "dirty w2 vetoed");
    assert!(!driver.destroyed().contains(&w2));
    assert!(!driver.exited());

    let info = project_info(&mut shell, &mut driver, w2, "pi-w2");
    assert_eq!(info["name"], "dirty", "project survived both closes");
}

// ---------------------------------------------------------------------
// syn.app.quit — per-window vetoable close requests
// ---------------------------------------------------------------------

/// EX-4/RULE-7: quit over two windows with a dirty project requests
/// close on each; the first-processed window approves and is
/// destroyed (its view tears down contractually), the last-processed
/// window is vetoed and keeps the session alive — a partial veto,
/// never an implicit project/session stop.
#[test]
fn quit_with_dirty_project_partially_vetoes() {
    let (mut shell, mut driver) = harness_with(dirty_app(false));
    let w1 = ready_window(&mut shell, &mut driver);
    let w2 = second_window(&mut shell, &mut driver);
    attach(&mut shell, &mut driver, w1, "view-1");
    attach(&mut shell, &mut driver, w2, "view-2");

    action(
        &mut shell,
        &mut driver,
        w1,
        Intent::new("quit", "syn.app.quit", json!({})),
    );
    pump(&mut shell, &mut driver);

    // w1 (lower id, requested first) was not last → approved →
    // destroyed with its view; w2 was last → dirty → vetoed.
    assert!(
        driver.destroyed().contains(&w1),
        "w1 destroyed: {:?}",
        driver.destroyed()
    );
    assert!(driver.windows().contains(&w2), "vetoed w2 survives");
    assert!(!driver.destroyed().contains(&w2), "w2 not destroyed");
    assert!(!driver.exited(), "vetoed window keeps the loop alive");

    // w2's guard query ran and reported the dirty reason.
    let (ok, data) = guard_result(&driver, w2);
    assert!(ok, "guard query failed: {data:?}");
    assert!(
        data.unwrap()["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r == "unsaved_changes"),
        "w2 guard reports unsaved_changes"
    );

    // The shared project/session is intact on the surviving window:
    // same dirty revision, w2's view still attached, w1's view
    // contractually torn down with its window.
    let info = project_info(&mut shell, &mut driver, w2, "pi-survivor");
    assert_eq!(info["name"], "dirty");
    assert_eq!(info["revision"], 1);
    let views = view_list(&mut shell, &mut driver, w2, "vl-survivor");
    assert!(
        views.contains(&"view-2".to_string()),
        "w2 view survives: {views:?}"
    );
    assert!(
        !views.contains(&"view-1".to_string()),
        "w1 view torn down: {views:?}"
    );
}

/// Clean counterpart: quit over two clean windows approves both —
/// the guard runs only for the last one and the loop exits.
#[test]
fn quit_with_clean_project_closes_all_windows() {
    let (mut shell, mut driver) = harness();
    let w1 = ready_window(&mut shell, &mut driver);
    let w2 = second_window(&mut shell, &mut driver);

    action(
        &mut shell,
        &mut driver,
        w1,
        Intent::new("quit", "syn.app.quit", json!({})),
    );
    pump(&mut shell, &mut driver);

    assert!(driver.destroyed().contains(&w1), "w1 destroyed");
    assert!(driver.destroyed().contains(&w2), "w2 destroyed");
    assert!(driver.windows().is_empty(), "no live windows remain");
    assert!(driver.exited(), "last-window destroy exits the loop");
}
