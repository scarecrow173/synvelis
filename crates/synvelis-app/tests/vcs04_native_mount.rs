//! VCS-04 native mount/dock wiring — headless lane (synvelis#95).
//!
//! Covers what `tests/vcs04_history.rs` cannot: the *mount* half of
//! the slice. A `VcsHistory` view opened through
//! `syn.vcs.visual{action:"open"}` is a registry-adopted dock-domain
//! view — invisible to the static `dock_view_info` table — so these
//! tests pin the contract that carries it:
//!
//! - `syn.view.attach` resolves dock metadata from the workspace
//!   registry descriptor and reports `dock_domain: true`, which the
//!   shell learns for its detach-pool bookkeeping.
//! - `vcs_screen_cell` publishes one [`VcsScreenState`] per registered
//!   `ViewType::VcsHistory` view and [`mount_vcs_history_view`] builds
//!   a real [`ViewHost`] whose spec is the read-only screen
//!   projection.
//! - Detach pools the view (never closes it), `syn.vcs.visual close`
//!   and `syn.view.close` both drop registry/VCS/screen state, and
//!   none of it touches project revision, engine builds, transport,
//!   or the project name.
//! - `NativeShell` + `FakeDriver`: a dynamically opened VCS view
//!   attaches through `attach_view`, survives its window's
//!   `Destroyed` as a pooled dock view (not a legacy close replay),
//!   and completes the new-window dock saga (`dock_prepare_new_
//!   window` → `WindowRequest` → saga `Ready` attach + commit).
//!
//! Real OS window/IME/pixel evidence stays a separate gate (#114);
//! nothing here asserts it.

mod common;

use std::collections::BTreeSet;
use std::time::Duration;

use common::vcs::*;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::ipc::{Event, Intent};
use ruviform_ui::view::ComponentKey;
use ruviform_ui::{DriverEvent, WindowDriver, WindowId, WindowLifecycle, WindowSpec};
use ruviform_ui_test::FakeDriver;
use serde_json::{Value, json};

use synvelis_app::{NativeShell, SynvelisApp, mount_vcs_history_view};
use synvelis_workspace::ViewType;

// ---------------------------------------------------------------------------
// app-level dock domain
// ---------------------------------------------------------------------------

#[test]
fn dynamically_opened_vcs_view_is_a_dock_domain_view() {
    let mut app = fixture_backend();
    let view = open(&mut app);
    assert_eq!(view, "view-3");
    let inst = ViewInstanceId::new(3).expect("nonzero view id");

    // Registry descriptor is the dock metadata: kind/title verbatim.
    let descriptor = app
        .workspace()
        .registry()
        .descriptor(inst)
        .expect("registered");
    assert_eq!(descriptor.kind, ViewType::VcsHistory);
    assert_eq!(descriptor.title, "VCS History");

    app.op_window_adopt(&json!({ "window_id": "w1", "title": "main" }))
        .expect("adopt w1");
    let attached = app
        .op_view_attach(&json!({ "view_id": view, "window_id": "w1" }))
        .expect("attach");
    assert_eq!(attached["attached"], true);
    assert_eq!(attached["dock_domain"], true);
    assert!(attached["generation"].as_u64().unwrap() >= 1);

    let w1 = WindowId::new(1).expect("nonzero window id");
    assert_eq!(app.workspace().dock().owner_of(inst), Some(w1));

    // Detach pools the view — still registered, still VCS state.
    app.op_view_detach(&json!({ "view_id": view, "reason": "unmount" }))
        .expect("detach");
    assert!(app.workspace().dock().is_detached(inst));
    assert!(app.workspace().registry().contains(inst));
    assert!(app.workspace().vcs().contains(inst));

    // Re-attach moves it back onto the window.
    app.op_view_attach(&json!({ "view_id": view, "window_id": "w1" }))
        .expect("reattach");
    assert_eq!(app.workspace().dock().owner_of(inst), Some(w1));

    // `syn.view.close` is the docked close path — registry, dock,
    // and VCS state all drop; `syn.vcs.view` then refuses the id.
    app.op_view_close(&json!({ "view_id": view }))
        .expect("view close");
    assert!(!app.workspace().registry().contains(inst));
    assert!(!app.workspace().vcs().contains(inst));
    assert!(app.op_vcs_view(&json!({ "view_id": view })).is_err());
}

#[test]
fn view_shaped_id_without_registration_is_refused_not_legacy_attached() {
    let mut app = fixture_backend();
    app.op_window_adopt(&json!({ "window_id": "w1" }))
        .expect("adopt w1");
    // `view-{n}` that was never adopted has no dock metadata — the
    // attach must fail closed instead of reaching "UI attached but
    // dock unregistered".
    let err = app
        .op_view_attach(&json!({ "view_id": "view-42", "window_id": "w1" }))
        .expect_err("unregistered view-{n} must be refused");
    assert!(err.message.contains("dock metadata"), "got {err:?}");
}

// ---------------------------------------------------------------------------
// screen-state publication + ViewHost mount
// ---------------------------------------------------------------------------

#[test]
fn vcs_screen_cell_publishes_and_mount_vcs_history_view_builds_read_only_host() {
    let mut app = fixture();
    action(&mut app, json!({ "action": "refresh" }));
    action(&mut app, json!({ "action": "load_more" }));
    pin(&mut app, "before", &json!({ "kind": "node", "index": 3 }));
    pin(&mut app, "after", &json!({ "kind": "node", "index": 1 }));
    action(&mut app, json!({ "action": "compare" }));

    let cell = app.vcs_screen_cell();
    let state = {
        let screens = cell.borrow();
        screens
            .get(&history_instance())
            .expect("published VcsScreenState")
            .clone()
    };
    let view = state.view.as_ref().expect("projection");
    assert_eq!(view.rows.len(), 4);
    assert_eq!(view.status, synvelis_workspace::VcsViewStatus::Ready);
    let selection = view.selection.as_ref().expect("A/B selection");
    assert!(selection.diff.is_some(), "compare must materialize a diff");

    // The same helper the native binary calls — fresh host, epoch
    // minted from the session-owned allocator.
    let host = mount_vcs_history_view(&app.attach_epoch_allocator(), history_instance(), &state)
        .expect("mount vcs history view");
    assert_eq!(host.view_id(), history_instance());
    assert!(host.spec().is_some(), "host must carry the mounted spec");

    let keys: BTreeSet<String> = host.mounted_keys().iter().map(|k| k.0.clone()).collect();
    for expected in [
        "vcs.status",
        "vcs.selection",
        "vcs.diff",
        "vcs.history",
        "vcs.changes",
        "vcs.issues",
    ] {
        assert!(keys.contains(expected), "missing component {expected}");
    }

    // Read-only at the host boundary too: the history table mounts
    // with no selection and observing it never selects a row.
    let snapshot = host
        .observe(&ComponentKey::from("vcs.history"))
        .expect("history table mounted");
    assert!(snapshot.mounted);
    assert!(snapshot.selected.is_none());
}

#[test]
fn published_state_tracks_ops_and_drops_closed_views() {
    let mut app = fixture_backend();
    let view = open(&mut app);
    let inst = ViewInstanceId::new(3).expect("nonzero view id");

    // Before any op the cell still publishes the view (status Empty —
    // backend attached, nothing loaded).
    {
        let cell = app.vcs_screen_cell();
        let screens = cell.borrow();
        let state = screens.get(&inst).expect("view published");
        assert_eq!(
            state.view.as_ref().map(|v| &v.status),
            Some(&synvelis_workspace::VcsViewStatus::Empty)
        );
    }

    action_on(&mut app, &view, json!({ "action": "refresh" }));
    action_on(&mut app, &view, json!({ "action": "load_more" }));
    {
        let cell = app.vcs_screen_cell();
        let screens = cell.borrow();
        let state = screens.get(&inst).expect("view published");
        let view_state = state.view.as_ref().expect("projection");
        assert_eq!(view_state.rows.len(), 4);
        assert_eq!(view_state.next_cursor, None);
    }

    // Semantic freshness survives into the published state: pin Live,
    // advance the project, re-publish — the endpoint reports Stale,
    // never Fresh and never collapsed.
    action_on(
        &mut app,
        &view,
        json!({ "action": "pin", "side": "before", "endpoint": { "kind": "live" } }),
    );
    action_on(
        &mut app,
        &view,
        json!({ "action": "pin", "side": "after", "endpoint": { "kind": "live" } }),
    );
    apply(
        &mut app,
        synvelis_project::ProjectOp::SetProjectName {
            name: "vcs-d".into(),
        },
    );
    {
        let cell = app.vcs_screen_cell();
        let screens = cell.borrow();
        let state = screens.get(&inst).expect("view published");
        let selection = state
            .view
            .as_ref()
            .and_then(|v| v.selection.as_ref())
            .expect("selection");
        assert!(
            matches!(
                selection.before.freshness,
                synvelis_workspace::EndpointFreshness::Stale { .. }
            ),
            "live endpoint must report stale after revision advance"
        );
    }

    // Close drops the published entry on the next publish.
    app.op_vcs_visual(&json!({ "action": "close", "view_id": view }))
        .expect("close");
    let cell = app.vcs_screen_cell();
    assert!(!cell.borrow().contains_key(&inst));
}

// ---------------------------------------------------------------------------
// read-only authority invariance
// ---------------------------------------------------------------------------

#[test]
fn view_navigation_and_close_leave_project_authority_untouched() {
    let mut app = fixture_backend();
    let before = authority(&app);

    let view = open(&mut app);
    app.op_window_adopt(&json!({ "window_id": "w1" }))
        .expect("adopt w1");
    app.op_view_attach(&json!({ "view_id": view, "window_id": "w1" }))
        .expect("attach");
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
    action_on(
        &mut app,
        &view,
        json!({ "action": "view_mode", "mode": "change_focus" }),
    );
    app.op_view_detach(&json!({ "view_id": view }))
        .expect("detach");
    app.op_view_attach(&json!({ "view_id": view, "window_id": "w1" }))
        .expect("reattach");
    app.op_view_close(&json!({ "view_id": view }))
        .expect("close");

    assert_eq!(authority(&app), before);
}

// ---------------------------------------------------------------------------
// NativeShell + FakeDriver — the shell-side dock-domain contract
// ---------------------------------------------------------------------------

fn shell_harness(app: SynvelisApp) -> (NativeShell, FakeDriver) {
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

fn send_intent(shell: &mut NativeShell, driver: &mut FakeDriver, window: WindowId, intent: Intent) {
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

/// Dispatches one `syn.*` op intent on `window` and returns the last
/// `(ok, data)` result pair.
fn dispatch_op(
    shell: &mut NativeShell,
    driver: &mut FakeDriver,
    window: WindowId,
    id: &str,
    op: &str,
    params: Value,
) -> (bool, Option<Value>) {
    send_intent(shell, driver, window, Intent::new(id, op, params));
    driver
        .events_for(window)
        .iter()
        .filter_map(|e| match e {
            Event::Result {
                id: rid, ok, data, ..
            } if rid == id => Some((*ok, data.clone())),
            _ => None,
        })
        .next_back()
        .unwrap_or_else(|| panic!("no result for {id}"))
}

#[test]
fn native_shell_mounts_pools_and_closes_dynamically_opened_vcs_view() {
    let (mut shell, mut driver) = shell_harness(fixture_backend());
    let w1 = ready_window(&mut shell, &mut driver);
    let w2 = second_window(&mut shell, &mut driver);

    // Open through the real op surface — the shell sees only the
    // wire `view_id`.
    let (ok, data) = dispatch_op(
        &mut shell,
        &mut driver,
        w1,
        "vcs-open",
        "syn.vcs.visual",
        json!({ "action": "open" }),
    );
    assert!(ok, "open failed: {data:?}");
    let view = data.as_ref().unwrap()["view_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(view, "view-3");

    // Mount path: attach_view mints syn.view.attach + subscribes —
    // the result's dock_domain flag is what the shell learns.
    let attach = shell
        .attach_view(&mut driver, w2, &view, &[])
        .expect("attach vcs view");
    assert_eq!(attach["dock_domain"], true);
    assert_eq!(attach["attached"], true);
    assert_eq!(shell.view_window(&view), Some(w2));

    // The view answers its ops while docked.
    let (ok, _) = dispatch_op(
        &mut shell,
        &mut driver,
        w2,
        "vcs-refresh",
        "syn.vcs.visual",
        json!({ "action": "refresh", "view_id": view }),
    );
    assert!(ok, "refresh on docked view must succeed");

    // Window Destroyed: the dock-domain view is pooled by the
    // workspace disposition — NOT closed by the legacy replay.
    inject(
        &mut shell,
        &mut driver,
        DriverEvent::Windowed {
            window: w2,
            event: WindowLifecycle::Destroyed,
        },
    );
    assert_eq!(shell.view_window(&view), None);
    let (ok, data) = dispatch_op(
        &mut shell,
        &mut driver,
        w1,
        "vcs-view-after-destroy",
        "syn.vcs.view",
        json!({ "view_id": view }),
    );
    assert!(
        ok,
        "pooled vcs view must still answer syn.vcs.view: {data:?}"
    );
    assert_eq!(data.as_ref().unwrap()["status"], "ready");

    // And it still closes cleanly through the VCS op.
    let (ok, data) = dispatch_op(
        &mut shell,
        &mut driver,
        w1,
        "vcs-close",
        "syn.vcs.visual",
        json!({ "action": "close", "view_id": view }),
    );
    assert!(ok, "close failed: {data:?}");
    let (ok, _) = dispatch_op(
        &mut shell,
        &mut driver,
        w1,
        "vcs-view-after-close",
        "syn.vcs.view",
        json!({ "view_id": view }),
    );
    assert!(!ok, "closed view must refuse syn.vcs.view");
}

#[test]
fn native_shell_runs_new_window_dock_saga_for_vcs_view() {
    let (mut shell, mut driver) = shell_harness(fixture_backend());
    let w1 = ready_window(&mut shell, &mut driver);

    let (ok, data) = dispatch_op(
        &mut shell,
        &mut driver,
        w1,
        "vcs-open",
        "syn.vcs.visual",
        json!({ "action": "open" }),
    );
    assert!(ok);
    let view = data.as_ref().unwrap()["view_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Dock the view on w1 first — the saga requires a docked source.
    let attach = shell
        .attach_view(&mut driver, w1, &view, &[])
        .expect("attach to w1");
    assert_eq!(attach["dock_domain"], true);
    assert_eq!(shell.view_window(&view), Some(w1));

    // Phase 1: prepare_new_window pushes a WindowRequest; the shell
    // drains it into driver.create_window on the same handle pass.
    let before: BTreeSet<WindowId> = driver.windows().into_iter().collect();
    let (ok, data) = dispatch_op(
        &mut shell,
        &mut driver,
        w1,
        "dock-prepare",
        "syn.window.dock_prepare_new_window",
        json!({ "view_id": view, "title": "VCS History" }),
    );
    assert!(ok, "prepare failed: {data:?}");
    let after: Vec<WindowId> = driver
        .windows()
        .into_iter()
        .filter(|w| !before.contains(w))
        .collect();
    assert_eq!(after.len(), 1, "exactly one saga window must be created");
    let saga_window = after[0];

    // Phase 2: the saga window's Ready attaches the view (the
    // AwaitingWindow exception attach) and commits the dock move.
    for event in [WindowLifecycle::Created, WindowLifecycle::Ready] {
        inject(
            &mut shell,
            &mut driver,
            DriverEvent::Windowed {
                window: saga_window,
                event,
            },
        );
    }
    assert!(shell.is_ready(saga_window));
    assert_eq!(
        shell.view_window(&view),
        Some(saga_window),
        "saga must move the vcs view onto the new window"
    );

    // The moved view still answers ops on its new owner window.
    let (ok, _) = dispatch_op(
        &mut shell,
        &mut driver,
        saga_window,
        "vcs-refresh-2",
        "syn.vcs.visual",
        json!({ "action": "refresh", "view_id": view }),
    );
    assert!(ok, "refresh on saga-moved view must succeed");
}
