//! Two-window mount over `FakeDriver` (SV-GUI-04 slice 1, synvelis#113).
//!
//! The headless half of the real-2-window claim: two `ViewHost`s
//! (`view-1` main screen, `view-2` transport deck) mount through the
//! same composition-root order `synvelis-native` uses — session-owned
//! `AttachEpoch` allocator → `NativeShell` → per-window `Ready` →
//! `attach_view` — and two fake windows attach their own view. Both
//! views share the one `SynvelisApp`/`ProjectSession` behind the
//! shell, keep independent view state (per-view receipts, projections,
//! and host bindings), and `origin_view` commands arriving on a
//! foreign window's channel are rejected by the shell's existing
//! ownership validation — never minted under the live epoch.
//!
//! `deliver_to_host`/`pump_view_intents` mirror `RvfView`'s
//! drain/translate decode verbatim (same wire as `view_mount.rs`),
//! once per window. Nothing here asserts OS/GPU/IME/Dock behavior —
//! the native lane is a separate gate (`SYNVELIS_SECOND_WINDOW` +
//! `run-app-native`), recorded in the task's PR.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use serde_json::{Value, json};

use ruviform_ui::behavior::{
    CommandReceipt, ControlInput, InputSource, ReceiptStatus, Revision, StateProjection, ValueInput,
};
use ruviform_ui::ipc::{Event, Intent};
use ruviform_ui::view::{ComponentKey, ComponentSpec, ProjectionOp, ViewEvent, ViewHost, ViewSpec};
use ruviform_ui::{
    BackendReport, DriverEvent, WindowDriver, WindowId, WindowLifecycle, WindowSpec,
};
use ruviform_ui_test::FakeDriver;

use synvelis_app::{
    MainScreenState, NativeShell, SynvelisApp, main_screen_spec, main_view_id, main_view_targets,
    mount_main_view, mount_secondary_view, secondary_screen_spec, secondary_view_id,
    secondary_view_instance, secondary_view_targets,
};

fn key(name: &str) -> ComponentKey {
    ComponentKey(name.to_string())
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

/// One window's mounted-view endpoint — the `ViewHost` the driver
/// would own plus the append-only inbox cursor mirroring `RvfView`'s
/// drainable inbox (`tests/view_mount.rs`'s `MountedFixture` fields,
/// once per window).
struct Endpoint {
    window: WindowId,
    host: ViewHost,
    view_id: String,
    /// `true` for the transport deck — state pushes use
    /// [`secondary_screen_spec`], `false` uses [`main_screen_spec`].
    secondary: bool,
    cursor: usize,
}

/// The two-window composition: one shell/app/session, two windows,
/// two mounted views — the `FakeDriver` counterpart of
/// `SYNVELIS_SECOND_WINDOW=1 synvelis-native`.
struct TwoWindow {
    shell: NativeShell,
    driver: FakeDriver,
    /// The app's shared main-screen state cell — taken before the app
    /// sealed, exactly like `synvelis-native` does.
    screen: Rc<RefCell<MainScreenState>>,
    main: Endpoint,
    secondary: Endpoint,
}

/// Composes app → two view hosts → shell → two ready fake windows →
/// `attach_view` per window — the exact composition-root order
/// `synvelis-native` uses with the env gate on.
fn fixture() -> TwoWindow {
    let app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let screen = app.main_screen_cell();
    let attach_epochs = app.attach_epoch_allocator();
    let host1 = mount_main_view(&attach_epochs, &screen.borrow()).expect("main view mounts");
    let host2 =
        mount_secondary_view(&attach_epochs, &screen.borrow()).expect("secondary view mounts");
    let mut shell = NativeShell::new(app, Duration::from_millis(16)).expect("compose shell");
    let mut driver = FakeDriver::new();

    let w1 = driver
        .create_window(&WindowSpec {
            title: "Synvelis".to_string(),
            ..WindowSpec::default()
        })
        .expect("main window");
    inject(
        &mut shell,
        &mut driver,
        DriverEvent::Windowed {
            window: w1,
            event: WindowLifecycle::Ready,
        },
    );
    let w2 = driver
        .create_window(&WindowSpec {
            title: "Synvelis — Secondary".to_string(),
            ..WindowSpec::default()
        })
        .expect("secondary window");
    inject(
        &mut shell,
        &mut driver,
        DriverEvent::Windowed {
            window: w2,
            event: WindowLifecycle::Ready,
        },
    );
    assert_ne!(w1, w2, "two distinct driver windows");

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

    TwoWindow {
        shell,
        driver,
        screen,
        main: Endpoint {
            window: w1,
            host: host1,
            view_id: main_view_id(),
            secondary: false,
            cursor: 0,
        },
        secondary: Endpoint {
            window: w2,
            host: host2,
            view_id: secondary_view_id(),
            secondary: true,
            cursor: 0,
        },
    }
}

/// The spec a state push applies to `ep`'s host — the same
/// per-window choice `synvelis-native`'s run loop makes.
fn spec_for(ep: &Endpoint, state: &MainScreenState) -> ViewSpec {
    if ep.secondary {
        secondary_screen_spec(state)
    } else {
        main_screen_spec(state)
    }
}

/// Drains `ep`'s minted `ViewEvent::Intent`s into `rvf.ui.command.v1`
/// `Action`s on `deliver_window`'s channel — the same envelope
/// `RvfView` mints for `IntentSink::send` (RF387). `deliver_window`
/// defaults to the endpoint's own window; tests pass a foreign one to
/// exercise the stale/foreign-channel rejection.
fn pump_view_intents_to(
    shell: &mut NativeShell,
    driver: &mut FakeDriver,
    ep: &mut Endpoint,
    deliver_window: WindowId,
) {
    for event in ep.host.drain_events() {
        if let ViewEvent::Intent {
            intent,
            command_id,
            expect_receipt,
        } = event
        {
            let params = json!({
                "command_id": command_id,
                "expect_receipt": expect_receipt,
                "intent": intent,
            });
            action(
                shell,
                driver,
                deliver_window,
                Intent::new(command_id, "rvf.ui.command.v1", params),
            );
        }
    }
}

fn pump_view_intents(shell: &mut NativeShell, driver: &mut FakeDriver, ep: &mut Endpoint) {
    let window = ep.window;
    pump_view_intents_to(shell, driver, ep, window);
}

/// Applies every newly delivered window event to `ep`'s host —
/// `Result` → `CommandReceipt`, `Notify` → `StateProjection` —
/// mirroring `RvfView::drain_inbox`'s decode (identical to
/// `view_mount.rs`'s `deliver_to_host`).
fn deliver_to_host(driver: &FakeDriver, ep: &mut Endpoint) {
    let events: Vec<Event> = driver
        .events_for(ep.window)
        .into_iter()
        .skip(ep.cursor)
        .cloned()
        .collect();
    ep.cursor += events.len();
    for event in events {
        match event {
            Event::Result {
                id,
                ok,
                data,
                error,
            } => {
                let revision = data
                    .as_ref()
                    .and_then(|d| d.get("revision"))
                    .and_then(|v| serde_json::from_value::<Revision>(v.clone()).ok());
                let mut detail = error.map(|e| e.message).or_else(|| {
                    data.as_ref()
                        .and_then(|d| d.get("detail"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
                let status = if ok {
                    match data.as_ref().and_then(|d| d.get("status")) {
                        None => ReceiptStatus::Accepted,
                        Some(raw) => serde_json::from_value::<ReceiptStatus>(raw.clone())
                            .unwrap_or_else(|_| {
                                detail = Some(format!("unrecognized receipt status: {raw}"));
                                ReceiptStatus::Rejected
                            }),
                    }
                } else {
                    ReceiptStatus::Rejected
                };
                ep.host.apply_receipt(&CommandReceipt {
                    command_id: id,
                    status,
                    revision,
                    detail,
                });
            }
            Event::Notify { data, .. } => {
                let Some(control_id) = data.get("control_id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(revision) = data
                    .get("revision")
                    .and_then(|v| serde_json::from_value::<Revision>(v.clone()).ok())
                else {
                    continue;
                };
                let provenance = data
                    .get("provenance")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                ep.host.apply_projection(
                    control_id,
                    &StateProjection {
                        revision,
                        value: data.get("value").cloned().unwrap_or(Value::Null),
                        provenance,
                    },
                );
            }
            _ => {}
        }
    }
}

/// One control input end-to-end on `ep`: dispatch into the host,
/// mint intents onto `ep`'s own window channel, apply deliveries back.
fn endpoint_input(
    shell: &mut NativeShell,
    driver: &mut FakeDriver,
    ep: &mut Endpoint,
    k: &str,
    input: ControlInput,
) {
    let generation = ep.host.generation();
    ep.host
        .dispatch_input(generation, &key(k), input)
        .expect("dispatch_input");
    pump_view_intents(shell, driver, ep);
    deliver_to_host(driver, ep);
}

/// Presses the Button `k` on `ep` — same mint/deliver path as
/// `endpoint_input`.
fn endpoint_press(shell: &mut NativeShell, driver: &mut FakeDriver, ep: &mut Endpoint, k: &str) {
    let generation = ep.host.generation();
    ep.host.invoke(generation, &key(k)).expect("button invoke");
    pump_view_intents(shell, driver, ep);
    deliver_to_host(driver, ep);
}

/// Pushes the app's current [`MainScreenState`] into `ep`'s host —
/// the headless mirror of `GpuiDriver::update_view` on the native
/// lane, with the same per-window spec choice the run loop makes.
fn push_screen_state(
    screen: &Rc<RefCell<MainScreenState>>,
    ep: &mut Endpoint,
) -> Vec<ProjectionOp> {
    let state = screen.borrow().clone();
    let generation = ep.host.generation();
    ep.host
        .update(spec_for(ep, &state), generation)
        .expect("screen state update applies")
}

fn begin(id: &str) -> ControlInput {
    ControlInput::Begin {
        gesture_id: id.to_string(),
        source: InputSource::from("pointer"),
        origin_view: None,
        target: None,
        at: None,
        modifiers: None,
        seq: None,
    }
}

fn preview(id: &str, plain: f64) -> ControlInput {
    ControlInput::Preview {
        gesture_id: id.to_string(),
        value: ValueInput::Plain(plain),
        modifiers: None,
        seq: None,
    }
}

fn commit(id: &str) -> ControlInput {
    ControlInput::Commit {
        gesture_id: id.to_string(),
        value: None,
        modifiers: None,
        seq: None,
    }
}

/// `data` payloads of `notify` events delivered to `window` whose
/// `channel` matches — same shape as `tests/shell.rs`'s helper.
fn notifies(driver: &FakeDriver, window: WindowId, channel: &str) -> Vec<Value> {
    driver
        .events_for(window)
        .iter()
        .filter_map(|e| match e {
            Event::Notify { data, .. } if data["channel"] == channel => Some(data["data"].clone()),
            _ => None,
        })
        .collect()
}

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

/// Asserts the command `id` was answered with exactly one
/// `invalid_params` rejection on `window` — the fail-closed shape
/// every origin-resolution refusal must mint (same contract as
/// `tests/shell.rs`'s `assert_rejected`).
fn assert_rejected(driver: &FakeDriver, window: WindowId, id: &str) {
    let results = results_for(driver, window, id);
    assert_eq!(results.len(), 1, "one rejection result: {results:?}");
    assert!(!results[0].0, "command must be rejected: {results:?}");
    let err = driver
        .events_for(window)
        .iter()
        .find_map(|e| match e {
            Event::Result { id: rid, error, .. } if rid == id => error.clone(),
            _ => None,
        })
        .expect("error body");
    assert_eq!(err.code, "invalid_params");
}

/// Asserts no `syn.control.receipt` notify for `id` reached `window`
/// — a rejected command must never mint a domain receipt.
fn assert_no_receipt(driver: &FakeDriver, window: WindowId, id: &str) {
    let receipts = notifies(driver, window, "syn.control.receipt");
    assert!(
        receipts.iter().all(|r| r["receipt"]["commandId"] != id),
        "rejected command must not reach the domain: {receipts:?}"
    );
}

/// Finds a component by key anywhere in the spec tree — Panels recurse
/// through `children`, Tabs through each tab's `child` (same helper
/// as `view_mount.rs`).
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

/// The resolved `text` of the `Label` `name` in `spec`.
fn spec_label_text(spec: &ViewSpec, name: &str) -> String {
    match find_component(&spec.root, name) {
        Some(ComponentSpec::Label(l)) => l.text.clone(),
        other => panic!("label `{name}` missing or wrong kind: {other:?}"),
    }
}

// ---------------------------------------------------------------------
// Two-window mount and shared session
// ---------------------------------------------------------------------

/// Both windows reach `Ready`, each attaches its own view, and the
/// shell's view→window bookkeeping maps `view-1`/`view-2` to distinct
/// windows — the mount half of the slice.
#[test]
fn two_windows_attach_their_own_views() {
    let f = fixture();
    assert_eq!(f.shell.view_window(&f.main.view_id), Some(f.main.window));
    assert_eq!(
        f.shell.view_window(&f.secondary.view_id),
        Some(f.secondary.window)
    );
    assert_ne!(f.main.window, f.secondary.window);
    assert!(f.shell.is_ready(f.main.window));
    assert!(f.shell.is_ready(f.secondary.window));

    // `attach_view` subscribed each view's targets — the subscribe
    // result ids carry the view id, so both windows prove their own.
    let sub1 = results_for(
        &f.driver,
        f.main.window,
        "shell-sub-view-1-transport/position",
    );
    assert_eq!(sub1.len(), 1, "view-1 subscribe result: {sub1:?}");
    assert!(sub1[0].0, "view-1 subscribe ok: {sub1:?}");
    let sub2 = results_for(
        &f.driver,
        f.secondary.window,
        "shell-sub-view-2-transport/position",
    );
    assert_eq!(sub2.len(), 1, "view-2 subscribe result: {sub2:?}");
    assert!(sub2[0].0, "view-2 subscribe ok: {sub2:?}");
}

/// `syn.view.list` reports both views with their declared owner
/// windows, and `syn.project.info` answered on each window reports the
/// same project id and revision — one `SynvelisApp`/`ProjectSession`
/// behind both windows, never two application states.
#[test]
fn both_windows_share_one_project_session() {
    let mut f = fixture();

    action(
        &mut f.shell,
        &mut f.driver,
        f.secondary.window,
        Intent::new("vl", "syn.view.list", json!({})),
    );
    let (ok, data) = results_for(&f.driver, f.secondary.window, "vl")
        .last()
        .cloned()
        .expect("view.list result");
    assert!(ok, "view.list failed: {data:?}");
    let views = data.unwrap()["views"].as_array().unwrap().clone();
    let owner = |view: &str| {
        views
            .iter()
            .find(|v| v["view_id"] == view)
            .and_then(|v| v["window_id"].as_str().map(str::to_string))
    };
    assert_eq!(owner("view-1").as_deref(), Some("w1"));
    assert_eq!(
        owner("view-2").as_deref(),
        Some(f.secondary.window.to_string().as_str())
    );

    for (id, window) in [("pi-1", f.main.window), ("pi-2", f.secondary.window)] {
        action(
            &mut f.shell,
            &mut f.driver,
            window,
            Intent::new(id, "syn.project.info", json!({})),
        );
    }
    let info = |window: WindowId, id: &str| {
        let (ok, data) = results_for(&f.driver, window, id)
            .last()
            .cloned()
            .expect("project.info result");
        assert!(ok, "project.info failed on {window}: {data:?}");
        data.unwrap()
    };
    let i1 = info(f.main.window, "pi-1");
    let i2 = info(f.secondary.window, "pi-2");
    assert_eq!(i1["project_id"], i2["project_id"], "same project");
    assert_eq!(i1["revision"], i2["revision"], "same revision");
}

// ---------------------------------------------------------------------
// Independent view state and consistent display
// ---------------------------------------------------------------------

/// A Seek gesture on the secondary deck mints its commands under
/// `view-2`'s epoch namespace: the `Applied` receipt lands on
/// `view-2`'s own `transport/position` binding, the receipt-as-
/// `Result` reaches the secondary window only, and the main window
/// and the main host's binding for the same target never see it —
/// per-view command ownership, not a shared channel.
#[test]
fn secondary_window_commands_apply_on_their_own_channel() {
    let mut f = fixture();
    deliver_to_host(&f.driver, &mut f.main);
    deliver_to_host(&f.driver, &mut f.secondary);

    endpoint_input(
        &mut f.shell,
        &mut f.driver,
        &mut f.secondary,
        "seek2.position",
        begin("g1"),
    );
    endpoint_input(
        &mut f.shell,
        &mut f.driver,
        &mut f.secondary,
        "seek2.position",
        preview("g1", 30.0),
    );
    endpoint_input(
        &mut f.shell,
        &mut f.driver,
        &mut f.secondary,
        "seek2.position",
        commit("g1"),
    );

    let binding = f
        .secondary
        .host
        .binding("transport/position")
        .expect("view-2 seek binding");
    let receipt = binding
        .state()
        .last_receipt
        .clone()
        .expect("a receipt landed on view-2");
    assert_eq!(receipt.status, ReceiptStatus::Applied, "{receipt:?}");
    let cmd = receipt.command_id.clone();

    // The translated Result reached the owning window only.
    let r2 = results_for(&f.driver, f.secondary.window, &cmd);
    assert_eq!(r2.len(), 1, "one translated Result: {r2:?}");
    assert!(r2[0].0, "receipt applied: {r2:?}");
    assert_eq!(r2[0].1.as_ref().unwrap()["status"], "applied");
    assert!(
        results_for(&f.driver, f.main.window, &cmd).is_empty(),
        "no translated Result on the foreign window"
    );

    // The main view's binding for the same target never adopted the
    // foreign receipt — independent per-view binding state.
    let foreign = f
        .main
        .host
        .binding("transport/position")
        .and_then(|b| b.state().last_receipt.clone());
    assert!(
        foreign.as_ref().is_none_or(|r| r.command_id != cmd),
        "view-1 must not adopt view-2's receipt: {foreign:?}"
    );

    // The raw receipt still broadcast for `ViewBinding` consumers —
    // dual delivery, not either/or (shell contract).
    let receipts = notifies(&f.driver, f.main.window, "syn.control.receipt");
    assert!(
        receipts.iter().any(|r| r["receipt"]["commandId"] == cmd),
        "raw receipt broadcast reached the other window: {receipts:?}"
    );
}

/// Both views subscribe the same transport targets, so one domain
/// truth lands as per-view projection rows on each owner window and
/// each host's controlled display follows it — the "same target,
/// consistent display" half of the acceptance. A Seek gesture on the
/// secondary deck commits through `view-2` and re-projects both.
#[test]
fn both_views_display_the_same_transport_truth() {
    let mut f = fixture();
    deliver_to_host(&f.driver, &mut f.main);
    deliver_to_host(&f.driver, &mut f.secondary);

    // Initial subscription projections reached each host's own seek.
    let s1 = f
        .main
        .host
        .observe(&key("seek.position"))
        .expect("main seek snapshot");
    let s2 = f
        .secondary
        .host
        .observe(&key("seek2.position"))
        .expect("secondary seek snapshot");
    assert!(s1.observed.is_some(), "view-1 projection landed");
    assert!(s2.observed.is_some(), "view-2 projection landed");

    // A commit on the secondary deck's Seek.
    endpoint_input(
        &mut f.shell,
        &mut f.driver,
        &mut f.secondary,
        "seek2.position",
        begin("g1"),
    );
    endpoint_input(
        &mut f.shell,
        &mut f.driver,
        &mut f.secondary,
        "seek2.position",
        preview("g1", 30.0),
    );
    endpoint_input(
        &mut f.shell,
        &mut f.driver,
        &mut f.secondary,
        "seek2.position",
        commit("g1"),
    );

    // Deliver the re-projected rows to BOTH hosts — each window got
    // its own translated Notify for its own (view, target) row.
    deliver_to_host(&f.driver, &mut f.main);
    deliver_to_host(&f.driver, &mut f.secondary);
    let s1 = f
        .main
        .host
        .observe(&key("seek.position"))
        .expect("main seek snapshot after commit");
    let s2 = f
        .secondary
        .host
        .observe(&key("seek2.position"))
        .expect("secondary seek snapshot after commit");
    let d1 = s1.display.expect("main controlled display");
    let d2 = s2.display.expect("secondary controlled display");
    assert!(
        (d1.plain - d2.plain).abs() < 1e-9,
        "same target, same seconds display: {d1:?} vs {d2:?}"
    );

    // The shared screen cell + per-window spec pushes paint the same
    // status on both specs (the native lane's update_view path).
    endpoint_press(
        &mut f.shell,
        &mut f.driver,
        &mut f.secondary,
        "transport2.play",
    );
    assert_eq!(
        f.screen.borrow().transport.status.as_deref(),
        Some("Playing"),
        "shared cell captured the transport truth"
    );
    push_screen_state(&f.screen, &mut f.main);
    push_screen_state(&f.screen, &mut f.secondary);
    let spec1 = f.main.host.spec().expect("main spec mounted");
    let spec2 = f.secondary.host.spec().expect("secondary spec mounted");
    let t1 = spec_label_text(spec1, "transport.status");
    let t2 = spec_label_text(spec2, "transport.status");
    assert!(t1.contains("Playing"), "main status label: {t1}");
    assert!(t2.contains("Playing"), "secondary status label: {t2}");
}

// ---------------------------------------------------------------------
// Cross-window origin_view rejection
// ---------------------------------------------------------------------

/// A command minted by `view-2`'s host but arriving on the main
/// window's channel names a view that window does not own — the
/// shell's existing `origin_view` validation rejects it with
/// `invalid_params` and no domain receipt is minted, in both
/// directions.
#[test]
fn foreign_window_commands_are_rejected() {
    let mut f = fixture();
    deliver_to_host(&f.driver, &mut f.main);
    deliver_to_host(&f.driver, &mut f.secondary);

    // view-2 host mints a Play press; deliver the wire intent on the
    // MAIN window — `origin_view: "view-2"` is foreign there.
    let generation = f.secondary.host.generation();
    f.secondary
        .host
        .invoke(generation, &key("transport2.play"))
        .expect("secondary invoke");
    for event in f.secondary.host.drain_events() {
        if let ViewEvent::Intent {
            intent,
            command_id,
            expect_receipt,
        } = event
        {
            let params = json!({
                "command_id": command_id,
                "expect_receipt": expect_receipt,
                "intent": intent,
            });
            action(
                &mut f.shell,
                &mut f.driver,
                f.main.window,
                Intent::new(command_id.clone(), "rvf.ui.command.v1", params),
            );
            assert_rejected(&f.driver, f.main.window, &command_id);
            assert_no_receipt(&f.driver, f.main.window, &command_id);
            assert_no_receipt(&f.driver, f.secondary.window, &command_id);
        }
    }

    // And the mirror: view-1's Play delivered on the secondary
    // window's channel.
    let generation = f.main.host.generation();
    f.main
        .host
        .invoke(generation, &key("transport.play"))
        .expect("main invoke");
    for event in f.main.host.drain_events() {
        if let ViewEvent::Intent {
            intent,
            command_id,
            expect_receipt,
        } = event
        {
            let params = json!({
                "command_id": command_id,
                "expect_receipt": expect_receipt,
                "intent": intent,
            });
            action(
                &mut f.shell,
                &mut f.driver,
                f.secondary.window,
                Intent::new(command_id.clone(), "rvf.ui.command.v1", params),
            );
            assert_rejected(&f.driver, f.secondary.window, &command_id);
            assert_no_receipt(&f.driver, f.main.window, &command_id);
            assert_no_receipt(&f.driver, f.secondary.window, &command_id);
        }
    }

    // Nothing reached the domain — transport is still stopped.
    assert_eq!(
        f.screen.borrow().transport.status.as_deref(),
        Some("Stopped"),
        "rejected commands never touched the session"
    );
}

/// The owning window still mints normally after the foreign attempts
/// — rejection is per-channel, not a poisoned view.
#[test]
fn owning_window_commands_still_apply_after_foreign_rejection() {
    let mut f = fixture();
    deliver_to_host(&f.driver, &mut f.main);
    deliver_to_host(&f.driver, &mut f.secondary);

    // Foreign attempt first (rejected), then the legitimate gesture
    // on the same secondary channel.
    let generation = f.secondary.host.generation();
    f.secondary
        .host
        .invoke(generation, &key("transport2.play"))
        .expect("secondary invoke");
    for event in f.secondary.host.drain_events() {
        if let ViewEvent::Intent {
            intent,
            command_id,
            expect_receipt,
        } = event
        {
            let params = json!({
                "command_id": command_id,
                "expect_receipt": expect_receipt,
                "intent": intent,
            });
            action(
                &mut f.shell,
                &mut f.driver,
                f.main.window,
                Intent::new(command_id.clone(), "rvf.ui.command.v1", params),
            );
        }
    }
    endpoint_input(
        &mut f.shell,
        &mut f.driver,
        &mut f.secondary,
        "seek2.position",
        begin("g2"),
    );
    endpoint_input(
        &mut f.shell,
        &mut f.driver,
        &mut f.secondary,
        "seek2.position",
        preview("g2", 10.0),
    );
    endpoint_input(
        &mut f.shell,
        &mut f.driver,
        &mut f.secondary,
        "seek2.position",
        commit("g2"),
    );

    let receipt = f
        .secondary
        .host
        .binding("transport/position")
        .expect("view-2 seek binding")
        .state()
        .last_receipt
        .clone()
        .expect("receipt on the owning channel");
    assert_eq!(receipt.status, ReceiptStatus::Applied, "{receipt:?}");
}

// ---------------------------------------------------------------------
// Secondary spec shape
// ---------------------------------------------------------------------

/// `secondary_screen_spec` is a transport deck — status `Label`,
/// Play/Stop `Button`s on the `transport` action target, and the
/// `transport/position` `Seek` under its own component keys — driven
/// by the same [`MainScreenState`] as the main screen, with
/// `control_id == target` on the driven control.
#[test]
fn secondary_spec_is_a_transport_deck() {
    let spec = secondary_screen_spec(&MainScreenState::default());
    for (name, action_name) in [("transport2.play", "play"), ("transport2.stop", "stop")] {
        match find_component(&spec.root, name) {
            Some(ComponentSpec::Button(b)) => {
                assert_eq!(b.target, "transport");
                assert_eq!(b.action, action_name);
            }
            other => panic!("button `{name}` missing or wrong kind: {other:?}"),
        }
    }
    assert!(matches!(
        find_component(&spec.root, "transport.status"),
        Some(ComponentSpec::Label(_))
    ));
    match find_component(&spec.root, "seek2.position") {
        Some(ComponentSpec::Seek(s)) => {
            assert_eq!(s.control_id.as_deref(), Some("transport/position"));
            assert_eq!(s.target.as_deref(), Some("transport/position"));
        }
        other => panic!("seek `seek2.position` missing or wrong kind: {other:?}"),
    }
    // The deck is deliberately not the main screen's tab tree.
    assert!(
        !matches!(
            find_component(&spec.root, "screen.tabs"),
            Some(ComponentSpec::Tabs(_))
        ),
        "secondary spec carries no main-screen tabs"
    );
}

/// `mount_secondary_view` returns a `ViewHost` for `view-2` — the
/// wire identity (`origin_view`/`view_id`) the shell's ownership
/// check keys on. `ViewHost` stamps `origin_view` as
/// `"view-{ViewInstanceId}"` internally, so the instance id is the
/// assertion surface.
#[test]
fn secondary_host_mints_view_2_commands() {
    let app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let screen = app.main_screen_cell();
    let host = mount_secondary_view(&app.attach_epoch_allocator(), &screen.borrow())
        .expect("secondary view mounts");
    assert_eq!(secondary_view_id(), "view-2");
    assert_eq!(host.view_id(), secondary_view_instance());
}
