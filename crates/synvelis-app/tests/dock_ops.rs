//! `syn.window.dock_*` op family over `NativeShell` + `FakeDriver`
//! (SV#113 dock transfer slice, synvelis#113).
//!
//! The headless half of the dock-transfer claim: dock-domain views
//! (`view-1`/`view-2`) synchronize `WorkspaceManager`'s dock — the
//! single window/view authority — with the app's UI registry and the
//! shell's owner bookkeeping through the `syn.window.*` ops:
//!
//! - `dock_prepare`/`dock_commit`/`dock_abort` drive the two-phase
//!   `MoveToken` move; every layer agrees on the owner afterwards.
//! - `dock_detach`/`dock_reattach` round-trip through the detached
//!   pool.
//! - `dock_prepare_new_window` runs the async new-window saga: the
//!   shell drains the window request, the destination's `Ready`
//!   attaches + commits, and creation/destruction/attach failures
//!   abort with the source view untouched.
//! - `WindowLifecycle` forwarding adopts host-created windows and
//!   applies `WindowCloseDisposition` on `Destroyed` (`DetachViews`
//!   pools, `ReleaseViews` removes).
//! - Legacy (non-dock) view ids keep the UI-only path — dock ops
//!   refuse them honestly.
//!
//! Nothing here asserts real OS window behavior — that evidence
//! remains the #114 native gate.

use std::time::Duration;

use serde_json::{Value, json};

use ruviform_ui::dock::{WindowCloseDisposition, WindowGeometry};
use ruviform_ui::ipc::{ErrorBody, Event, Intent};
use ruviform_ui::{
    BackendReport, Capability, DriverEvent, WindowCapabilities, WindowDriver, WindowId,
    WindowLifecycle, WindowSpec,
};
use ruviform_ui_test::FakeDriver;

use synvelis_app::{
    NativeShell, SynvelisApp, main_view_id, main_view_targets, secondary_view_id,
    secondary_view_targets,
};

// ---------------------------------------------------------------------------
// Fixture — two ready windows, view-1 on w1, view-2 on w2
// ---------------------------------------------------------------------------

struct Fixture {
    shell: NativeShell,
    driver: FakeDriver,
    w1: WindowId,
    w2: WindowId,
}

impl Fixture {
    /// Local copies of the window ids — `WindowId` is `Copy`, and
    /// borrowing `self` mutably for `op` while reading `self.w1`
    /// does not compile.
    fn ids(&self) -> (WindowId, WindowId) {
        (self.w1, self.w2)
    }
}

fn fixture() -> Fixture {
    fixture_with(SynvelisApp::new(BackendReport::available(
        "gpui",
        "test-0.1".to_string(),
    )))
}

fn fixture_with(app: SynvelisApp) -> Fixture {
    let mut shell = NativeShell::new(app, Duration::from_millis(16)).expect("compose shell");
    let mut driver = FakeDriver::new();

    let w1 = shell.open_main(&mut driver).expect("main window");
    ready(&mut shell, &mut driver, w1);
    let w2 = driver
        .create_window(&WindowSpec {
            title: "Synvelis — Secondary".to_string(),
            ..WindowSpec::default()
        })
        .expect("second window");
    ready(&mut shell, &mut driver, w2);

    shell
        .attach_view(&mut driver, w1, &main_view_id(), &main_view_targets())
        .expect("attach view-1");
    shell
        .attach_view(
            &mut driver,
            w2,
            &secondary_view_id(),
            &secondary_view_targets(),
        )
        .expect("attach view-2");

    Fixture {
        shell,
        driver,
        w1,
        w2,
    }
}

/// `Created` (queued by `create_window`) + explicit `Ready` — the
/// full lifecycle a real driver emits.
fn ready(shell: &mut NativeShell, driver: &mut FakeDriver, window: WindowId) {
    driver.inject(DriverEvent::Windowed {
        window,
        event: WindowLifecycle::Ready,
    });
    pump(shell, driver);
}

fn pump(shell: &mut NativeShell, driver: &mut FakeDriver) {
    driver.pump(&mut |ev, d| shell.handle(ev, d));
}

fn inject(shell: &mut NativeShell, driver: &mut FakeDriver, ev: DriverEvent) {
    driver.inject(ev);
    pump(shell, driver);
}

/// Dispatches a `syn.*` op as an `Action` intent on `window` and
/// returns its `(ok, data, error)` result.
fn op(
    f: &mut Fixture,
    window: WindowId,
    id: &str,
    name: &str,
    params: Value,
) -> (bool, Option<Value>, Option<ErrorBody>) {
    inject(
        &mut f.shell,
        &mut f.driver,
        DriverEvent::Windowed {
            window,
            event: WindowLifecycle::Action(Intent::new(id, name, params)),
        },
    );
    f.driver
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
        .next_back()
        .unwrap_or_else(|| panic!("no result for {name} on {window}"))
}

fn view_row(f: &mut Fixture, window: WindowId, view: &str) -> Value {
    let (ok, data, err) = op(f, window, "vl", "syn.view.list", json!({}));
    assert!(ok, "view.list failed: {err:?}");
    let data = data.unwrap();
    data["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["view_id"] == view)
        .unwrap_or_else(|| panic!("{view} listed"))
        .clone()
}

fn window_list(f: &mut Fixture, window: WindowId) -> Value {
    let (ok, data, err) = op(f, window, "wl", "syn.window.list", json!({}));
    assert!(ok, "window.list failed: {err:?}");
    data.unwrap()
}

fn docked_view(f: &mut Fixture, window: WindowId, view: &str) -> Value {
    let list = window_list(f, window);
    list["docked_views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["view_id"] == view)
        .unwrap_or_else(|| panic!("{view} docked"))
        .clone()
}

fn pending_moves(f: &mut Fixture, window: WindowId) -> usize {
    window_list(f, window)["pending_moves"]
        .as_array()
        .unwrap()
        .len()
}

fn detached_pool(f: &mut Fixture, window: WindowId) -> Vec<String> {
    window_list(f, window)["detached_views"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

// ---------------------------------------------------------------------------
// Two-phase move on an existing window
// ---------------------------------------------------------------------------

/// prepare → commit moves the view w1 → w2 and every authority agrees:
/// the workspace dock, the shell's owner map, and `syn.view.list`.
#[test]
fn dock_prepare_commit_moves_view_everywhere() {
    let mut f = fixture();
    let (w1, w2) = f.ids();

    let (ok, data, err) = op(
        &mut f,
        w1,
        "dp",
        "syn.window.dock_prepare",
        json!({ "view_id": "view-1", "window_id": w2.to_string() }),
    );
    assert!(ok, "dock_prepare failed: {err:?}");
    assert_eq!(data.unwrap()["prepared"], true);

    // While the move is pending the view is busy: attach/detach and
    // conflicting dock ops refuse.
    for (id, name, params) in [
        (
            "busy-attach",
            "syn.view.attach",
            json!({ "view_id": "view-1", "window_id": w1.to_string() }),
        ),
        (
            "busy-detach",
            "syn.view.detach",
            json!({ "view_id": "view-1" }),
        ),
        (
            "busy-dock",
            "syn.window.dock_reattach",
            json!({ "view_id": "view-1", "window_id": w2.to_string() }),
        ),
    ] {
        let (ok, _, err) = op(&mut f, w1, id, name, params);
        assert!(!ok, "{name} must refuse a busy view");
        assert_eq!(err.unwrap().code, "invalid_params");
    }

    let (ok, data, err) = op(
        &mut f,
        w1,
        "dc",
        "syn.window.dock_commit",
        json!({ "view_id": "view-1" }),
    );
    assert!(ok, "dock_commit failed: {err:?}");
    let data = data.unwrap();
    assert_eq!(data["committed"], true);
    assert_eq!(data["to"], w2.to_string());

    // One authority, three projections.
    assert_eq!(f.shell.view_window("view-1"), Some(w2));
    let row = view_row(&mut f, w2, "view-1");
    assert_eq!(row["window_id"], w2.to_string());
    assert_eq!(row["attached"], true);
    assert_eq!(
        docked_view(&mut f, w2, "view-1")["window_id"],
        w2.to_string()
    );
    assert_eq!(
        pending_moves(&mut f, w2),
        0,
        "no pending moves after commit"
    );
}

/// prepare → abort keeps the view at its source on every layer.
#[test]
fn dock_abort_preserves_source_ownership() {
    let mut f = fixture();
    let (w1, w2) = f.ids();

    op(
        &mut f,
        w1,
        "dp",
        "syn.window.dock_prepare",
        json!({ "view_id": "view-1", "window_id": w2.to_string() }),
    );
    let (ok, data, err) = op(
        &mut f,
        w1,
        "da",
        "syn.window.dock_abort",
        json!({ "view_id": "view-1" }),
    );
    assert!(ok, "dock_abort failed: {err:?}");
    let data = data.unwrap();
    assert_eq!(data["aborted"], true);
    assert_eq!(data["owner"], w1.to_string());

    assert_eq!(f.shell.view_window("view-1"), Some(w1));
    let row = view_row(&mut f, w1, "view-1");
    assert_eq!(row["window_id"], w1.to_string());
    assert_eq!(row["attached"], true);
    assert_eq!(
        docked_view(&mut f, w1, "view-1")["window_id"],
        w1.to_string()
    );
}

/// A prepared move whose destination window dies is released: the
/// commit then fails and the view stays at its source.
#[test]
fn dock_commit_after_destination_destroy_fails_closed() {
    let mut f = fixture();
    let (w1, w2) = f.ids();

    op(
        &mut f,
        w1,
        "dp",
        "syn.window.dock_prepare",
        json!({ "view_id": "view-1", "window_id": w2.to_string() }),
    );

    // w2 dies mid-move — `Destroyed` releases the pending transfer.
    f.driver.destroy_window(w2).expect("destroy w2");
    pump(&mut f.shell, &mut f.driver);

    let (ok, _, err) = op(
        &mut f,
        w1,
        "dc",
        "syn.window.dock_commit",
        json!({ "view_id": "view-1" }),
    );
    assert!(!ok, "commit against a dead destination must fail");
    assert!(err.is_some());

    assert_eq!(f.shell.view_window("view-1"), Some(w1));
    let row = view_row(&mut f, w1, "view-1");
    assert_eq!(row["window_id"], w1.to_string());
    // w2's own view-2 was pooled by `DetachViews`, not closed.
    let pooled = view_row(&mut f, w1, "view-2");
    assert_eq!(pooled["attached"], false);
    assert!(pooled["window_id"].is_null());
}

/// `dock_detach` pools the view; `dock_reattach` brings it back —
/// through the workspace pool, not a UI-only hack.
#[test]
fn dock_detach_reattach_round_trips_through_pool() {
    let mut f = fixture();
    let (w1, w2) = f.ids();

    let (ok, _, err) = op(
        &mut f,
        w1,
        "dd",
        "syn.window.dock_detach",
        json!({ "view_id": "view-1" }),
    );
    assert!(ok, "dock_detach failed: {err:?}");
    assert_eq!(f.shell.view_window("view-1"), None);
    let row = view_row(&mut f, w1, "view-1");
    assert_eq!(row["attached"], false);
    assert!(row["window_id"].is_null());
    assert!(
        detached_pool(&mut f, w1).iter().any(|v| v == "view-1"),
        "view-1 listed in the detached pool"
    );

    let (ok, data, err) = op(
        &mut f,
        w1,
        "dr",
        "syn.window.dock_reattach",
        json!({ "view_id": "view-1", "window_id": w2.to_string() }),
    );
    assert!(ok, "dock_reattach failed: {err:?}");
    assert_eq!(data.unwrap()["to"], w2.to_string());
    assert_eq!(f.shell.view_window("view-1"), Some(w2));
    let row = view_row(&mut f, w2, "view-1");
    assert_eq!(row["window_id"], w2.to_string());
    assert_eq!(row["attached"], true);
}

/// `syn.view.detach` on a dock-domain view pools it too; a plain
/// `syn.view.attach` then moves it back through the dock.
#[test]
fn view_detach_pools_and_view_attach_reattaches() {
    let mut f = fixture();
    let (w1, w2) = f.ids();

    let (ok, _, err) = op(
        &mut f,
        w1,
        "vd",
        "syn.view.detach",
        json!({ "view_id": "view-1" }),
    );
    assert!(ok, "view.detach failed: {err:?}");
    assert_eq!(f.shell.view_window("view-1"), None);
    assert!(
        detached_pool(&mut f, w1).iter().any(|v| v == "view-1"),
        "syn.view.detach pooled the dock view"
    );

    let (ok, data, err) = op(
        &mut f,
        w1,
        "va",
        "syn.view.attach",
        json!({ "view_id": "view-1", "window_id": w2.to_string() }),
    );
    assert!(ok, "view.attach failed: {err:?}");
    assert_eq!(data.unwrap()["attached"], true);
    assert_eq!(f.shell.view_window("view-1"), Some(w2));
    assert_eq!(
        docked_view(&mut f, w2, "view-1")["window_id"],
        w2.to_string()
    );
}

// ---------------------------------------------------------------------------
// New-window saga
// ---------------------------------------------------------------------------

/// The window id `dock_prepare_new_window` produces — the driver
/// window that was not there before the request drained.
fn saga_window(f: &Fixture, before: &[WindowId]) -> WindowId {
    f.driver
        .windows()
        .into_iter()
        .find(|w| !before.contains(w))
        .expect("saga created a new driver window")
}

/// `dock_prepare_new_window` → shell `create_window` → `Ready` attaches the
/// view (the saga's one permitted attach) → `dock_commit` lands the
/// move. Every layer ends on the new window.
#[test]
fn new_window_saga_commits_on_destination_ready() {
    let mut f = fixture();
    let (w1, w2) = f.ids();
    let before = f.driver.windows();

    let (ok, data, err) = op(
        &mut f,
        w1,
        "dpn",
        "syn.window.dock_prepare_new_window",
        json!({ "view_id": "view-1", "title": "View One" }),
    );
    assert!(ok, "dock_prepare_new_window failed: {err:?}");
    assert_eq!(data.unwrap()["pending"], true);

    // The request drains into `create_window`; `Created` is queued.
    pump(&mut f.shell, &mut f.driver);
    let w3 = saga_window(&f, &before);
    assert!(f.driver.spec_of(w3).is_some_and(|s| s.title == "View One"));

    // Busy until the destination commits: a stray attach elsewhere
    // is refused; the destination attach itself is the exception.
    let (ok, _, err) = op(
        &mut f,
        w1,
        "stray",
        "syn.view.attach",
        json!({ "view_id": "view-1", "window_id": w2.to_string() }),
    );
    assert!(!ok, "stray attach during saga must fail: {err:?}");

    // `Ready` drives the saga: attach + subscribe + commit.
    ready(&mut f.shell, &mut f.driver, w3);

    assert_eq!(f.shell.view_window("view-1"), Some(w3));
    assert_eq!(
        docked_view(&mut f, w3, "view-1")["window_id"],
        w3.to_string()
    );
    assert_eq!(
        pending_moves(&mut f, w3),
        0,
        "saga committed — no pending move left"
    );
    // The other window keeps its own view.
    assert_eq!(f.shell.view_window("view-2"), Some(w2));
}

/// `multi_window` unsupported → `create_window` fails → the pending
/// move aborts and the view never leaves its source.
#[test]
fn new_window_creation_failure_aborts_and_keeps_source() {
    let mut f = fixture();
    let w1 = f.w1;
    f.driver.set_capabilities(WindowCapabilities {
        resize: Capability::Supported,
        focus: Capability::Supported,
        scale_factor: Capability::Supported,
        multi_window: Capability::Unsupported,
        ime: Capability::Supported,
        native_surface_slot: Capability::Supported,
    });

    let before = f.driver.windows();
    let (ok, data, err) = op(
        &mut f,
        w1,
        "dpn",
        "syn.window.dock_prepare_new_window",
        json!({ "view_id": "view-1" }),
    );
    assert!(ok, "prepare accepted: {err:?}");
    assert_eq!(data.unwrap()["pending"], true);

    pump(&mut f.shell, &mut f.driver);
    assert_eq!(f.driver.windows(), before, "no window was created");

    // The failed creation aborted the move — source untouched.
    assert_eq!(f.shell.view_window("view-1"), Some(w1));
    let row = view_row(&mut f, w1, "view-1");
    assert_eq!(row["window_id"], w1.to_string());
    assert_eq!(row["attached"], true);
    assert_eq!(
        pending_moves(&mut f, w1),
        0,
        "failed saga leaves no pending move"
    );
}

/// The destination dying before `Ready` aborts the saga: the orphan
/// window is already gone, the source view keeps ownership and its
/// UI binding is never moved.
#[test]
fn saga_destination_destroyed_before_ready_aborts() {
    let mut f = fixture();
    let w1 = f.w1;
    let before = f.driver.windows();

    op(
        &mut f,
        w1,
        "dpn",
        "syn.window.dock_prepare_new_window",
        json!({ "view_id": "view-1" }),
    );
    pump(&mut f.shell, &mut f.driver);
    let w3 = saga_window(&f, &before);

    // Destroy before Ready — the attach/commit never ran.
    f.driver.destroy_window(w3).expect("destroy w3");
    pump(&mut f.shell, &mut f.driver);

    assert!(f.driver.destroyed().contains(&w3));
    assert_eq!(f.shell.view_window("view-1"), Some(w1));
    let row = view_row(&mut f, w1, "view-1");
    assert_eq!(row["window_id"], w1.to_string());
    assert_eq!(row["attached"], true);
    assert_eq!(
        pending_moves(&mut f, w1),
        0,
        "destroyed destination leaves no pending move"
    );
}

// ---------------------------------------------------------------------------
// Window adoption + lifecycle + disposition
// ---------------------------------------------------------------------------

/// `syn.window.adopt` is idempotent for consistent params and refuses
/// contradictory ones; `syn.window.list` exposes the dock surface.
#[test]
fn window_adopt_is_idempotent_and_list_reports_dock() {
    let mut f = fixture();
    let (w1, w2) = f.ids();

    // Re-adopting w1 with the same title is a no-op ack.
    let (ok, data, err) = op(
        &mut f,
        w1,
        "adopt1",
        "syn.window.adopt",
        json!({ "window_id": w1.to_string(), "title": "Synvelis" }),
    );
    assert!(ok, "re-adopt failed: {err:?}");
    assert_eq!(data.unwrap()["adopted"], true);

    // A contradictory title is an error, never a silent overwrite.
    let (ok, _, _err) = op(
        &mut f,
        w1,
        "adopt2",
        "syn.window.adopt",
        json!({ "window_id": w1.to_string(), "title": "Other" }),
    );
    assert!(!ok, "contradictory adopt must fail");

    let list = window_list(&mut f, w1);
    let windows = list["windows"].as_array().unwrap();
    assert!(windows.iter().any(|w| w["window_id"] == w1.to_string()));
    assert!(windows.iter().any(|w| w["window_id"] == w2.to_string()));
    assert_eq!(
        docked_view(&mut f, w1, "view-1")["window_id"],
        w1.to_string()
    );
    assert_eq!(
        docked_view(&mut f, w2, "view-2")["window_id"],
        w2.to_string()
    );
}

/// `ReleaseViews` disposition: `Destroyed` removes the docked views
/// entirely — `syn.view.list` no longer carries them.
#[test]
fn destroyed_with_release_views_closes_docked_views() {
    // The disposition is workspace state — set it before the app
    // seals; the shell's `Created`/`Ready` adoption then keeps the
    // record (adopt is a no-op for a known window).
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let w1 = WindowId::new(1).expect("w1 id"); // FakeDriver's first window
    app.workspace_mut()
        .adopt_window(
            w1,
            "main",
            WindowGeometry {
                x: 0,
                y: 0,
                width: 1280,
                height: 800,
                monitor: None,
            },
        )
        .expect("adopt w1");
    app.workspace_mut()
        .set_close_disposition(w1, WindowCloseDisposition::ReleaseViews);
    let mut f = fixture_with(app);
    let (w1, w2) = f.ids();

    f.driver.destroy_window(w1).expect("destroy w1");
    pump(&mut f.shell, &mut f.driver);

    let (ok, data, err) = op(&mut f, w2, "vl", "syn.view.list", json!({}));
    assert!(ok, "view.list failed: {err:?}");
    let data = data.unwrap();
    let views = data["views"].as_array().unwrap();
    assert!(
        views.iter().all(|v| v["view_id"] != "view-1"),
        "ReleaseViews removed view-1: {views:?}"
    );
    assert!(
        views.iter().any(|v| v["view_id"] == "view-2"),
        "view-2 on the surviving window stays: {views:?}"
    );
}

/// A dock move never touches project/session truth: the project
/// revision and transport state are identical before and after.
#[test]
fn dock_transfer_preserves_project_and_transport_state() {
    let mut f = fixture();
    let (w1, w2) = f.ids();

    let (ok, before, err) = op(&mut f, w1, "pi0", "syn.project.info", json!({}));
    assert!(ok, "project.info failed: {err:?}");

    op(
        &mut f,
        w1,
        "dp",
        "syn.window.dock_prepare",
        json!({ "view_id": "view-1", "window_id": w2.to_string() }),
    );
    op(
        &mut f,
        w1,
        "dc",
        "syn.window.dock_commit",
        json!({ "view_id": "view-1" }),
    );

    let (ok, after, err) = op(&mut f, w2, "pi1", "syn.project.info", json!({}));
    assert!(ok, "project.info failed: {err:?}");
    assert_eq!(before.unwrap(), after.unwrap(), "project truth unchanged");
}

/// Legacy view ids stay on the UI-only path: dock ops refuse them,
/// `syn.view.*` keeps working without workspace adoption.
#[test]
fn legacy_view_ids_stay_ui_only() {
    let mut f = fixture();
    let (w1, w2) = f.ids();

    // "console" is not `view-{n}` — it never enters the dock domain.
    let (ok, _, err) = op(
        &mut f,
        w1,
        "legacy-attach",
        "syn.view.attach",
        json!({ "view_id": "console", "window_id": w1.to_string() }),
    );
    assert!(ok, "legacy attach failed: {err:?}");

    for (id, name, params) in [
        (
            "legacy-dd",
            "syn.window.dock_detach",
            json!({ "view_id": "console" }),
        ),
        (
            "legacy-dp",
            "syn.window.dock_prepare",
            json!({ "view_id": "console", "window_id": w2.to_string() }),
        ),
        (
            "legacy-dpn",
            "syn.window.dock_prepare_new_window",
            json!({ "view_id": "console" }),
        ),
        (
            "legacy-dr",
            "syn.window.dock_reattach",
            json!({ "view_id": "console", "window_id": w2.to_string() }),
        ),
        (
            "legacy-dc",
            "syn.window.dock_commit",
            json!({ "view_id": "console" }),
        ),
        (
            "legacy-da",
            "syn.window.dock_abort",
            json!({ "view_id": "console" }),
        ),
    ] {
        let (ok, _, _err) = op(&mut f, w1, id, name, params);
        assert!(!ok, "{name} must refuse a non-dock view");
    }

    // The legacy view still detaches/closes on the UI-only path.
    let (ok, _, err) = op(
        &mut f,
        w1,
        "legacy-detach",
        "syn.view.detach",
        json!({ "view_id": "console" }),
    );
    assert!(ok, "legacy detach failed: {err:?}");
}
