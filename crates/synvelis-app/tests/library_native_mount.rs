//! ALIB-05 native mount wiring — headless lane (synvelis#89).
//!
//! Covers the *mount* half of the ALIB-05 native slice that
//! `tests/library_browser.rs` cannot: an `AssetLibrary` view opened
//! through `syn.library.visual{action:"open"}` mounts the
//! `syn.library.view` wire spec verbatim on a [`ViewHost`]
//! ([`mount_library_view`], the same contract
//! `mount_variant_diff_view` follows), the `NativeShell` +
//! `FakeDriver` lane attaches it as a registry-adopted dock-domain
//! view, and a `syn.library.*` dispatch re-reads the spec through
//! the op itself (the library model lives inside the sealed app's
//! `LibraryViews`, not in a shared screen-state cell).
//!
//! The provider backend is the product's own
//! `LocalFolderProvider` over a real temp-dir fixture — the same
//! provider kind `synvelis-native`'s `SYNVELIS_LIBRARY_DIR` wiring
//! binds — plus the real `ImportStore`/`LocalBlobDir`/
//! `StagedBlobPublisher` import ports.
//!
//! Real OS window/IME/pixel evidence stays a separate gate
//! (synvelis#114); nothing here asserts it.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use common::library::*;
use common::vcs::authority;
use ruviform_ui::BackendReport;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::ipc::{Event, Intent, codes};
use ruviform_ui::view::ViewSpec;
use ruviform_ui::{DriverEvent, WindowDriver, WindowId, WindowLifecycle, WindowSpec};
use ruviform_ui_test::FakeDriver;
use serde_json::{Value, json};

use synvelis_app::screens::{MountSpec, PendingMount, mount_library_view};
use synvelis_app::{MainScreenState, NativeShell, SynvelisApp};

/// `view-{n}` wire id → `ViewInstanceId`, the same parse
/// `synvelis-native::open_library_view` performs.
fn instance_of(view: &str) -> ViewInstanceId {
    view.strip_prefix("view-")
        .and_then(|n| n.parse::<u64>().ok())
        .and_then(ViewInstanceId::new)
        .expect("view id parses")
}

// ---------------------------------------------------------------------------
// mount: wire spec → ViewHost, verbatim
// ---------------------------------------------------------------------------

#[test]
fn library_view_mounts_wire_spec_verbatim() {
    let (mut app, lib_root, _proj) = app_with_local_provider(
        "verbatim",
        &[("a.wav", b"fixture-a"), ("sub/b.wav", b"fixture-b")],
    );
    let view = open_view(&mut app);
    let proj = proj(&mut app, &view);
    let spec: ViewSpec =
        serde_json::from_value(proj["spec"].clone()).expect("wire spec deserializes");

    let host = mount_library_view(
        &app.attach_epoch_allocator(),
        instance_of(&view),
        spec.clone(),
    )
    .expect("mount library view");
    assert_eq!(host.view_id(), instance_of(&view));
    // The mounted host carries exactly what `syn.library.view`
    // returned — the mount path never re-interprets the state into a
    // second spec shape.
    assert_eq!(host.spec(), Some(&spec));

    let keys: BTreeSet<String> = host.mounted_keys().iter().map(|k| k.0.clone()).collect();
    for expected in [
        "library.providers",
        "library.entries",
        "library.browse.refresh",
        "library.detail.selected",
        "library.preview.state",
        "library.import.state",
    ] {
        assert!(keys.contains(expected), "missing component {expected}");
    }
    let _ = std::fs::remove_dir_all(lib_root.parent().unwrap());
}

// ---------------------------------------------------------------------------
// real provider typed projection (browse / pin / preview surfaces)
// ---------------------------------------------------------------------------

#[test]
fn mounted_view_projects_real_local_folder_state() {
    let (mut app, lib_root, _proj) = app_with_local_provider(
        "typed",
        &[("a.wav", b"aaaa"), ("b.wav", b"bb"), ("sub/c.wav", b"cccc")],
    );
    let view = open_view(&mut app);

    // The product provider kind — a local-folder connection, not a
    // fixture double.
    let catalog = providers(&app);
    let list = catalog["providers"].as_array().expect("provider list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["kind"], "local-folder");
    assert_eq!(list[0]["id"], PROVIDER_TAG);

    action(
        &mut app,
        &view,
        json!({ "action": "provider.select", "provider": PROVIDER_TAG }),
    );
    action(&mut app, &view, json!({ "action": "browse.refresh" }));
    let v = proj(&mut app, &view);
    let locators: Vec<&str> = v["state"]["entries"]
        .as_array()
        .expect("browse entries")
        .iter()
        .map(|e| e["locator"].as_str().unwrap())
        .collect();
    // The real filesystem listing: two files + the `sub` container.
    assert!(locators.contains(&"a.wav"), "entries: {locators:?}");
    assert!(locators.contains(&"b.wav"), "entries: {locators:?}");
    assert!(locators.contains(&"sub"), "entries: {locators:?}");

    action(
        &mut app,
        &view,
        json!({ "action": "entry.select", "locator": "a.wav" }),
    );
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["selected_locator"], "a.wav");
    assert_eq!(v["state"]["stat"]["availability"]["state"], "available");

    action(
        &mut app,
        &view,
        json!({ "action": "entry.pin", "locator": "a.wav" }),
    );
    let v = proj(&mut app, &view);
    // ContentHash pin — the ConditionalStrong proof a real provider
    // reports, never `ImmutableObject` from a scripted fake.
    assert_eq!(v["state"]["pin"]["state"], "pinned");
    assert_eq!(v["state"]["pin"]["proof"], "conditional-strong");
    assert_eq!(v["state"]["pin"]["expected_size"], 4);

    action(
        &mut app,
        &view,
        json!({ "action": "preview.plan", "locator": "a.wav" }),
    );
    action(
        &mut app,
        &view,
        json!({ "action": "preview.fetch", "locator": "a.wav" }),
    );
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["preview"]["state"], "ready");
    assert_eq!(v["state"]["preview"]["byte_len"], 4);

    // The typed state reaches the mounted spec verbatim — the wire
    // spec serialization carries the same pin/proof vocabulary.
    let spec_json = serde_json::to_string(&v["spec"]).unwrap();
    assert!(
        spec_json.contains("conditional-strong"),
        "pin proof must appear in the spec: {spec_json}"
    );
    let _ = std::fs::remove_dir_all(lib_root.parent().unwrap());
}

// ---------------------------------------------------------------------------
// real verified-import ports end-to-end
// ---------------------------------------------------------------------------

#[test]
fn import_submits_through_real_import_ports() {
    let (mut app, lib_root, project_dir) =
        app_with_local_provider("import", &[("a.wav", b"real-asset-bytes")]);
    let view = open_view(&mut app);
    action(
        &mut app,
        &view,
        json!({ "action": "provider.select", "provider": PROVIDER_TAG }),
    );
    action(&mut app, &view, json!({ "action": "browse.refresh" }));
    let rev_before = app.workspace().session().revision();

    action(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 7,
            "locator": "a.wav",
            "allow_full_object": true,
        }),
    );
    let v = proj(&mut app, &view);
    let import = &v["state"]["import"];
    assert_eq!(import["state"], "committed-live", "state: {import}");
    assert_eq!(import["byte_len"], 16);
    assert!(import["asset_id"].is_string());
    assert!(import["source_id"].is_string());
    assert!(import["content_digest"].is_string());
    // CommittedLive advanced the live revision — the only sanctioned
    // project mutation on this surface.
    assert!(app.workspace().session().revision() > rev_before);
    // The verified blob landed in the real project media root.
    assert!(
        project_dir.join("media").join("blobs").exists(),
        "blob store must hold the committed object"
    );
    let _ = std::fs::remove_dir_all(lib_root.parent().unwrap());
}

// ---------------------------------------------------------------------------
// project-authority invariance (mount/attach/browse/pin/preview/close)
// ---------------------------------------------------------------------------

#[test]
fn mount_lifecycle_and_browse_ops_leave_project_authority_untouched() {
    let (mut app, lib_root, _proj) = app_with_local_provider("invariance", &[("a.wav", b"aaaa")]);
    let before = authority(&app);
    let view = open_view(&mut app);
    app.op_window_adopt(&json!({ "window_id": "w1", "title": "main" }))
        .expect("adopt w1");
    app.op_view_attach(&json!({ "view_id": view, "window_id": "w1" }))
        .expect("attach");
    action(
        &mut app,
        &view,
        json!({ "action": "provider.select", "provider": PROVIDER_TAG }),
    );
    action(&mut app, &view, json!({ "action": "browse.refresh" }));
    action(
        &mut app,
        &view,
        json!({ "action": "entry.select", "locator": "a.wav" }),
    );
    action(
        &mut app,
        &view,
        json!({ "action": "entry.pin", "locator": "a.wav" }),
    );
    action(
        &mut app,
        &view,
        json!({ "action": "preview.plan", "locator": "a.wav" }),
    );
    action(
        &mut app,
        &view,
        json!({ "action": "preview.fetch", "locator": "a.wav" }),
    );
    app.op_view_detach(&json!({ "view_id": view }))
        .expect("detach");
    app.op_view_attach(&json!({ "view_id": view, "window_id": "w1" }))
        .expect("reattach");
    app.op_view_close(&json!({ "view_id": view }))
        .expect("close");
    assert_eq!(authority(&app), before);
    let _ = std::fs::remove_dir_all(lib_root.parent().unwrap());
}

// ---------------------------------------------------------------------------
// unbound backend: typed `unavailable`, never a panic or empty spec
// ---------------------------------------------------------------------------

#[test]
fn unbound_backend_ops_answer_typed_unavailable() {
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    // providers is an honest empty list, not an error.
    let catalog = providers(&app);
    assert_eq!(catalog["providers"], json!([]));

    // `open` still adopts the view — it is the *projection* that
    // needs the backend.
    let view = open_view(&mut app);
    let err = app
        .op_library_view(&json!({ "view_id": view }))
        .expect_err("unbound view read must fail");
    assert_eq!(err.code.as_str(), codes::UNAVAILABLE);
    let err = app
        .op_library_action(&json!({ "view_id": view, "action": "browse.refresh" }))
        .expect_err("unbound action must fail");
    assert_eq!(err.code.as_str(), codes::UNAVAILABLE);

    // The shell-side spec re-read path degrades identically — `None`,
    // never a fabricated spec, and the op error still reaches the
    // window as a typed result.
    let (mut shell, mut driver) = shell_harness(app);
    let w = ready_window(&mut shell, &mut driver);
    assert!(shell.library_view_spec(&view, &mut driver).is_none());
    let (ok, _data) = dispatch_op(
        &mut shell,
        &mut driver,
        w,
        "lib-action",
        "syn.library.action",
        json!({ "view_id": view, "action": "browse.refresh" }),
    );
    assert!(!ok, "unbound action must answer a typed failure");
}

// ---------------------------------------------------------------------------
// NativeShell + FakeDriver — mount record, attach, re-read, pool
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
/// `(ok, data)` result pair — the same helper `vcs04_native_mount`
/// uses.
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
fn native_shell_attaches_marks_dirty_and_re_reads_library_spec() {
    let (mut app, lib_root, _proj) = app_with_local_provider("shell", &[("a.wav", b"aaaa")]);
    let view = open_view(&mut app);
    let initial_spec: ViewSpec =
        serde_json::from_value(proj(&mut app, &view)["spec"].clone()).expect("wire spec");
    let (mut shell, mut driver) = shell_harness(app);
    let w1 = ready_window(&mut shell, &mut driver);
    let w2 = second_window(&mut shell, &mut driver);

    // The same mount record `open_windows` builds for the env-gated
    // library window — stored wire spec, no control subscriptions.
    let mut mount = PendingMount {
        window: w2,
        view_id: view.clone(),
        targets: Vec::new(),
        spec_kind: MountSpec::Library(instance_of(&view), Box::new(initial_spec.clone())),
        attached: false,
    };
    let attach = shell
        .attach_view(&mut driver, w2, &view, &[])
        .expect("attach library view");
    mount.attached = true;
    assert_eq!(attach["attached"], true);
    // A registry-adopted AssetLibrary view is dock-domain — the same
    // bookkeeping the VCS mount lane relies on.
    assert_eq!(attach["dock_domain"], true);
    assert_eq!(shell.view_window(&view), Some(w2));

    // `PendingMount::spec` returns the stored wire spec verbatim.
    assert_eq!(
        mount.spec(
            &MainScreenState::default(),
            &BTreeMap::new(),
            &BTreeMap::new()
        ),
        Some(initial_spec.clone())
    );

    // A `syn.library.*` dispatch through the window-intent path sets
    // the dirty mark the run loop polls.
    assert!(!shell.take_library_dirty());
    let (ok, _) = dispatch_op(
        &mut shell,
        &mut driver,
        w1,
        "provider-select",
        "syn.library.action",
        json!({ "view_id": view, "action": "provider.select", "provider": PROVIDER_TAG }),
    );
    assert!(ok);
    assert!(shell.take_library_dirty(), "library op must mark dirty");
    assert!(!shell.take_library_dirty(), "dirty mark consumes");

    // The re-read op returns the wire spec again — and it moved,
    // because provider.select + browse.refresh materialized entries.
    let (ok, _) = dispatch_op(
        &mut shell,
        &mut driver,
        w1,
        "browse-refresh",
        "syn.library.action",
        json!({ "view_id": view, "action": "browse.refresh" }),
    );
    assert!(ok);
    let fresh = shell
        .library_view_spec(&view, &mut driver)
        .expect("library spec re-read");
    assert_ne!(fresh, initial_spec, "spec must refresh after ops");

    // Window destroy pools the dock-domain view — the model survives
    // and the view still answers its projection op.
    inject(
        &mut shell,
        &mut driver,
        DriverEvent::Windowed {
            window: w2,
            event: WindowLifecycle::Destroyed,
        },
    );
    assert_eq!(shell.view_window(&view), None);
    let (ok, _data) = dispatch_op(
        &mut shell,
        &mut driver,
        w1,
        "view-after-destroy",
        "syn.library.view",
        json!({ "view_id": view }),
    );
    assert!(ok, "pooled library view must still answer syn.library.view");

    // And it closes cleanly through the library op.
    let (ok, _) = dispatch_op(
        &mut shell,
        &mut driver,
        w1,
        "lib-close",
        "syn.library.visual",
        json!({ "action": "close", "view_id": view }),
    );
    assert!(ok);
    let (ok, _) = dispatch_op(
        &mut shell,
        &mut driver,
        w1,
        "view-after-close",
        "syn.library.view",
        json!({ "view_id": view }),
    );
    assert!(!ok, "closed view must refuse syn.library.view");
    let _ = std::fs::remove_dir_all(lib_root.parent().unwrap());
}

// ---------------------------------------------------------------------------
// uitest adapter — the run loop's mount/push semantics on the seam
// ---------------------------------------------------------------------------

/// `PendingMount::Library` + the dirty-mark → `syn.library.view`
/// re-read → `SpecPush` path, exercised through the real
/// [`SynvelisTestApp`] adapter — the same semantics `synvelis-native`'s
/// run loop applies (the seam's whole point). Gated on `uitest`
/// because the adapter is.
#[cfg(feature = "uitest")]
#[test]
fn uitest_adapter_pushes_refreshed_library_spec() {
    use std::cell::RefCell;
    use std::rc::Rc;

    use ruviform_ui_test::TestApp;
    use synvelis_app::uitest::SynvelisTestApp;

    let (mut app, lib_root, _proj) = app_with_local_provider("uitest", &[("a.wav", b"aaaa")]);
    let view = open_view(&mut app);
    let initial_spec: ViewSpec =
        serde_json::from_value(proj(&mut app, &view)["spec"].clone()).expect("wire spec");
    let screen_state = app.main_screen_cell();
    let vcs_screens = app.vcs_screen_cell();
    let autx_screens = app.automation_screen_cell();
    let shell = NativeShell::new(app, Duration::from_millis(16)).expect("compose shell");
    let mut driver = FakeDriver::new();
    let lib_window = driver
        .create_window(&WindowSpec {
            title: "Synvelis — Asset Library".to_string(),
            ..WindowSpec::default()
        })
        .expect("library window");

    let pushes: Rc<RefCell<Vec<(WindowId, ViewSpec)>>> = Rc::new(RefCell::new(Vec::new()));
    let recorder = Rc::clone(&pushes);
    let mounts = vec![PendingMount {
        window: lib_window,
        view_id: view.clone(),
        targets: Vec::new(),
        spec_kind: MountSpec::Library(instance_of(&view), Box::new(initial_spec.clone())),
        attached: false,
    }];
    let mut test_app = SynvelisTestApp::new(
        shell,
        mounts,
        screen_state,
        vcs_screens,
        autx_screens,
        Box::new(move |window, spec| {
            recorder.borrow_mut().push((window, spec));
            Ok(())
        }),
    );

    // Created + Ready → attach + post-attach push of the stored spec.
    for event in [WindowLifecycle::Created, WindowLifecycle::Ready] {
        test_app.on_event(
            DriverEvent::Windowed {
                window: lib_window,
                event,
            },
            &mut driver,
        );
    }
    assert_eq!(pushes.borrow().len(), 1, "post-attach push expected");
    assert_eq!(pushes.borrow()[0], (lib_window, initial_spec.clone()));

    // A `syn.library.*` op over the seam's normal Action channel
    // marks dirty → the adapter re-reads and pushes the fresh spec.
    for params in [
        json!({ "view_id": view, "action": "provider.select", "provider": PROVIDER_TAG }),
        json!({ "view_id": view, "action": "browse.refresh" }),
    ] {
        test_app.on_event(
            DriverEvent::Windowed {
                window: lib_window,
                event: WindowLifecycle::Action(Intent::new("lib-op", "syn.library.action", params)),
            },
            &mut driver,
        );
    }
    let pushes = pushes.borrow();
    let (_w, refreshed) = pushes.last().expect("a pushed refreshed spec");
    assert_ne!(refreshed, &initial_spec);
    // The refreshed spec is the wire projection again — entries the
    // real `LocalFolderProvider` listed now appear in it.
    let spec_json = serde_json::to_string(refreshed).unwrap();
    assert!(
        spec_json.contains("a.wav"),
        "browse entries must reach the pushed spec: {spec_json}"
    );
    drop(pushes);
    let _ = std::fs::remove_dir_all(lib_root.parent().unwrap());
}
