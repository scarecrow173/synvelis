//! `NativeShell` headless verification over `FakeDriver` (SV-GPUI-01).
//!
//! Exercises the RF386 driver contract end to end without an OS
//! window: creation → `Ready` → typed `Action` dispatch → receipts/
//! projections back through `post_event`, save/reload through the
//! sealed `syn.project.*` ops, close flow with `Destroyed` teardown
//! (`syn.view.close` replay + last-window exit), and `syn.app.quit`
//! → `request_close`. Nothing here asserts real OS/GPU/IME behavior —
//! that evidence remains a separate gate.

use std::time::Duration;

use serde_json::{Value, json};

use ruviform_ui::ipc::{Event, Intent};
use ruviform_ui::{BackendReport, DriverEvent, WindowDriver, WindowId, WindowLifecycle};
use ruviform_ui_test::FakeDriver;

use synvelis_app::{NativeShell, SynvelisApp};

fn harness() -> (NativeShell, FakeDriver) {
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

/// `data` payloads of `notify` events delivered to `window` whose
/// `channel` matches.
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

/// Result payloads (`data`) delivered to `window` for `op` intents —
/// correlated by the intent id the caller minted.
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

fn attach(shell: &mut NativeShell, driver: &mut FakeDriver, window: WindowId, view: &str) -> u64 {
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
    let results = results_for(driver, window, &format!("{view}-attach"));
    let (ok, data) = results.last().expect("attach result delivered");
    assert!(ok, "attach failed: {data:?}");
    data.as_ref().unwrap()["generation"].as_u64().unwrap()
}

fn dispatch(
    shell: &mut NativeShell,
    driver: &mut FakeDriver,
    window: WindowId,
    view: &str,
    cmd: &str,
    intent: &Value,
    generation: u64,
) {
    action(
        shell,
        driver,
        window,
        Intent::new(
            format!("{view}-{cmd}"),
            "syn.control.dispatch",
            json!({
                "intent": intent.clone(),
                "ctx": {
                    "commandId": cmd,
                    "viewId": view,
                    "windowId": window.to_string(),
                    "generation": generation,
                    "issuedAt": 0u64,
                },
            }),
        ),
    );
}

fn gesture(kind: &str, gesture: &str, value: f64) -> Value {
    let mut i = json!({
        "kind": kind,
        "target": "transport/position",
        "controlId": "seekbar",
        "gestureId": gesture,
        "originView": "console",
    });
    i["sequence"] = json!(1u64);
    match kind {
        "begin" | "preview" | "commit" => {
            i["value"] = json!({ "plain": value, "normalized": value / 10.0 });
        }
        "cancel" => {
            i["reason"] = json!("explicit");
        }
        _ => unreachable!(),
    }
    i
}

#[test]
fn window_ready_delivers_boot_app_state() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);
    let app_states: Vec<&Event> = driver
        .events_for(window)
        .into_iter()
        .filter(|e| matches!(e, Event::Notify { data, .. } if data["app"] == "synvelis"))
        .collect();
    assert_eq!(
        app_states.len(),
        1,
        "boot app-state push delivered on Ready"
    );
}

#[test]
fn action_intent_gets_typed_result_at_origin_window() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);
    action(
        &mut shell,
        &mut driver,
        window,
        Intent::new("q1", "syn.app.info", json!({})),
    );
    let results = results_for(&driver, window, "q1");
    let (ok, data) = results.last().expect("result delivered");
    assert!(ok);
    assert_eq!(data.as_ref().unwrap()["name"], "synvelis");
}

#[test]
fn view_attach_gesture_and_receipt_projection_flow() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);
    let generation = attach(&mut shell, &mut driver, window, "console");
    assert_eq!(shell.view_window("console"), Some(window));

    action(
        &mut shell,
        &mut driver,
        window,
        Intent::new(
            "sub-1",
            "syn.control.subscribe",
            json!({ "view_id": "console", "target": "transport/position" }),
        ),
    );

    // Scrub gesture: begin → preview → commit lands as receipts +
    // projection rows on the origin window.
    dispatch(
        &mut shell,
        &mut driver,
        window,
        "console",
        "cmd-b",
        &gesture("begin", "g1", 2.0),
        generation,
    );
    dispatch(
        &mut shell,
        &mut driver,
        window,
        "console",
        "cmd-p",
        &gesture("preview", "g1", 4.0),
        generation,
    );
    dispatch(
        &mut shell,
        &mut driver,
        window,
        "console",
        "cmd-c",
        &gesture("commit", "g1", 6.0),
        generation,
    );

    let receipts = notifies(&driver, window, "syn.control.receipt");
    let by_cmd: Vec<&str> = receipts
        .iter()
        .filter_map(|r| r["receipt"]["commandId"].as_str())
        .collect();
    assert!(by_cmd.contains(&"cmd-b"), "begin receipt: {receipts:?}");
    assert!(by_cmd.contains(&"cmd-c"), "commit receipt: {receipts:?}");
    let commit = receipts
        .iter()
        .find(|r| r["receipt"]["commandId"] == "cmd-c")
        .unwrap();
    assert_eq!(commit["receipt"]["status"], "applied");
    // Preview is coalescible — never a receipt.
    assert!(!by_cmd.contains(&"cmd-p"), "preview must not receipt");

    let projections = notifies(&driver, window, "syn.control.projection");
    assert!(
        projections
            .iter()
            .any(|p| p["target"] == "transport/position"),
        "projection row pushed: {projections:?}"
    );

    // Cancel path: begin a second gesture, cancel it.
    dispatch(
        &mut shell,
        &mut driver,
        window,
        "console",
        "cmd-b2",
        &gesture("begin", "g2", 1.0),
        generation,
    );
    dispatch(
        &mut shell,
        &mut driver,
        window,
        "console",
        "cmd-x",
        &gesture("cancel", "g2", 1.0),
        generation,
    );
    let receipts = notifies(&driver, window, "syn.control.receipt");
    assert!(
        receipts
            .iter()
            .any(|r| r["receipt"]["commandId"] == "cmd-x"),
        "cancel reports a receipt: {receipts:?}"
    );
}

#[test]
fn project_new_save_and_reload_roundtrip() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);
    let dir = std::env::temp_dir().join(format!("sv175-shell-{}-{}", std::process::id(), "p1"));
    let path = dir.to_string_lossy().to_string();
    let _ = std::fs::remove_dir_all(&dir);

    action(
        &mut shell,
        &mut driver,
        window,
        Intent::new(
            "pn",
            "syn.project.new",
            json!({ "path": path, "name": "slice" }),
        ),
    );
    let (ok, data) = results_for(&driver, window, "pn")
        .last()
        .cloned()
        .expect("project.new result");
    assert!(ok, "project.new failed: {data:?}");
    assert_eq!(data.unwrap()["name"], "slice");
    assert!(dir.exists(), "bundle written at {path}");

    action(
        &mut shell,
        &mut driver,
        window,
        Intent::new("ps", "syn.project.save", json!({})),
    );
    assert!(results_for(&driver, window, "ps").last().unwrap().0);

    action(
        &mut shell,
        &mut driver,
        window,
        Intent::new("po", "syn.project.open", json!({ "path": path })),
    );
    let (ok, data) = results_for(&driver, window, "po")
        .last()
        .cloned()
        .expect("project.open result");
    assert!(ok, "project.open failed: {data:?}");
    let info = data.unwrap();
    assert_eq!(info["name"], "slice");
    assert_eq!(info["saved"], true, "reloaded project reports saved");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn close_request_approves_and_last_window_exits() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);
    inject(
        &mut shell,
        &mut driver,
        DriverEvent::Windowed {
            window,
            event: WindowLifecycle::CloseRequested,
        },
    );
    // `lifecycle(Closing)` reached the window, the shell approved the
    // close, and `Destroyed` retired the id.
    let saw_closing = driver.events_for(window).iter().any(|e| {
        matches!(e, Event::Lifecycle { state } if matches!(state, ruviform_ui::ipc::LifecycleState::Closing))
    });
    assert!(saw_closing, "closing lifecycle event delivered");
    assert!(driver.destroyed().contains(&window), "window destroyed");
    assert!(driver.exited(), "last-window destroy exits the loop");
}

#[test]
fn destroyed_window_tears_down_its_views() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);
    attach(&mut shell, &mut driver, window, "console");
    action(
        &mut shell,
        &mut driver,
        window,
        Intent::new(
            "sub-1",
            "syn.control.subscribe",
            json!({ "view_id": "console", "target": "transport" }),
        ),
    );
    assert_eq!(shell.view_window("console"), Some(window));

    // A second window stays live so the post-teardown registry is
    // still observable through the op surface.
    let other = {
        let w = driver
            .create_window(&ruviform_ui::WindowSpec::default())
            .unwrap();
        inject(
            &mut shell,
            &mut driver,
            DriverEvent::Windowed {
                window: w,
                event: WindowLifecycle::Ready,
            },
        );
        w
    };

    // Approve-close the view's window — Destroyed replays
    // `syn.view.close` for the orphaned view.
    inject(
        &mut shell,
        &mut driver,
        DriverEvent::Windowed {
            window,
            event: WindowLifecycle::CloseRequested,
        },
    );
    assert!(driver.destroyed().contains(&window));
    assert!(
        shell.view_window("console").is_none(),
        "shell view map cleared"
    );
    assert!(!driver.exited(), "a live window keeps the loop up");

    action(
        &mut shell,
        &mut driver,
        other,
        Intent::new("vl", "syn.view.list", json!({})),
    );
    let (ok, data) = results_for(&driver, other, "vl").last().cloned().unwrap();
    assert!(ok);
    assert_eq!(
        data.unwrap()["views"].as_array().unwrap().len(),
        0,
        "backend view registry torn down — not just the shell map"
    );
}

#[test]
fn app_quit_requests_close_not_silent_destroy() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);
    action(
        &mut shell,
        &mut driver,
        window,
        Intent::new("q", "syn.app.quit", json!({})),
    );
    // The flag drains into a vetoable `request_close` — the close flow
    // then runs through CloseRequested → Approve → Destroyed → exit.
    inject(&mut shell, &mut driver, DriverEvent::Tick);
    assert!(
        driver.destroyed().contains(&window) || driver.close_pending(window),
        "quit requested the close flow: {window:?}"
    );
    pump(&mut shell, &mut driver);
    assert!(driver.destroyed().contains(&window));
    assert!(driver.exited());
}

#[test]
fn unrouted_events_broadcast_to_all_windows() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);
    let other = {
        let w = driver
            .create_window(&ruviform_ui::WindowSpec::default())
            .unwrap();
        inject(
            &mut shell,
            &mut driver,
            DriverEvent::Windowed {
                window: w,
                event: WindowLifecycle::Ready,
            },
        );
        w
    };
    attach(&mut shell, &mut driver, window, "console");
    action(
        &mut shell,
        &mut driver,
        window,
        Intent::new(
            "sub-1",
            "syn.control.subscribe",
            json!({ "view_id": "console", "target": "transport" }),
        ),
    );
    dispatch(
        &mut shell,
        &mut driver,
        window,
        "console",
        "cmd-1",
        &json!({
            "kind": "action", "target": "transport",
            "controlId": "t-1", "sequence": 1u64, "action": "play",
        }),
        1,
    );
    // Outbox notifies are broadcast — `other` receives the same
    // receipt/projection; its own bindings demux by view_id.
    let other_receipts = notifies(&driver, other, "syn.control.receipt");
    assert!(
        other_receipts
            .iter()
            .any(|r| r["receipt"]["commandId"] == "cmd-1"),
        "receipt broadcast reached second window"
    );
}

/// A mounted-view (`rvf.ui.command.v1`) intent — the shape a `ViewHost`
/// binding mints on the window's Action channel (RF387).
fn rvf_command(cmd: &str, intent: &Value) -> Intent {
    Intent::new(
        cmd,
        "rvf.ui.command.v1",
        json!({
            "command_id": cmd,
            "expect_receipt": true,
            "intent": intent,
        }),
    )
}

fn rvf_gesture(kind: &str, gesture: &str, value: f64) -> Value {
    let mut i = json!({
        "kind": kind,
        "target": "transport/position",
        "control_id": "seekbar",
        "gesture_id": gesture,
        "origin_view": "console",
    });
    i["sequence"] = json!(1u64);
    match kind {
        "begin" | "preview" | "commit" => {
            i["value"] = json!({ "plain": value, "normalized": value / 10.0 });
        }
        "cancel" => {
            i["reason"] = json!("explicit");
        }
        _ => unreachable!(),
    }
    i
}

#[test]
fn rvf_command_translates_and_receipt_lands_as_result() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);
    attach(&mut shell, &mut driver, window, "console");
    action(
        &mut shell,
        &mut driver,
        window,
        Intent::new(
            "sub-1",
            "syn.control.subscribe",
            json!({ "view_id": "console", "target": "transport/position" }),
        ),
    );

    // A gesture minted on the mounted view's Action channel rides
    // typed through `syn.control.dispatch` — the attach-adopted
    // generation is what `DispatchContext` carries (a stale epoch
    // would reject/stale the receipt, which `applied` disproves).
    // begin → preview → commit: the full lifecycle, as a real control.
    for (cmd, kind, value) in [
        ("cmd-b", "begin", 2.0),
        ("cmd-p", "preview", 4.0),
        ("cmd-r1", "commit", 6.0),
    ] {
        action(
            &mut shell,
            &mut driver,
            window,
            rvf_command(cmd, &rvf_gesture(kind, "g1", value)),
        );
    }

    // The ok-ack is suppressed; the authoritative receipt arrives as
    // the translated Result — exactly one `ok` Result for `cmd-r1`,
    // carrying the exact status string.
    let results = results_for(&driver, window, "cmd-r1");
    assert_eq!(
        results.len(),
        1,
        "ack suppressed, receipt only: {results:?}"
    );
    let (ok, data) = &results[0];
    assert!(ok, "receipt must be applied: {results:?}");
    assert_eq!(data.as_ref().unwrap()["status"], "applied");
    // `revision` is absent for a transport scrub — present only when
    // the receipt reports a domain revision (never null on the wire).
    if let Some(d) = data.as_ref() {
        assert!(d.get("revision").is_none_or(Value::is_u64));
    }

    // The original `syn.control.receipt` notify still broadcasts for
    // `ViewBinding` consumers — dual delivery, not either/or.
    let receipts = notifies(&driver, window, "syn.control.receipt");
    assert!(
        receipts
            .iter()
            .any(|r| r["receipt"]["commandId"] == "cmd-r1"),
        "raw receipt notify still delivered: {receipts:?}"
    );
}

#[test]
fn rvf_command_malformed_gets_typed_error_not_silent_default() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);
    action(
        &mut shell,
        &mut driver,
        window,
        rvf_command("bad-1", &json!({ "kind": "bogus" })),
    );
    let results = results_for(&driver, window, "bad-1");
    assert_eq!(results.len(), 1);
    let (ok, _) = &results[0];
    assert!(!ok, "unknown kind must fail: {results:?}");
    let err = driver
        .events_for(window)
        .iter()
        .find_map(|e| match e {
            Event::Result { id, error, .. } if id == "bad-1" => error.clone(),
            _ => None,
        })
        .expect("error body");
    assert_eq!(err.code, "invalid_params");
}

/// A second ready window — the multi-window counterpart of
/// `ready_window` for ownership/reattach cases.
fn second_window(shell: &mut NativeShell, driver: &mut FakeDriver) -> WindowId {
    let w = driver
        .create_window(&ruviform_ui::WindowSpec::default())
        .unwrap();
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

/// Asserts the command `id` was answered with exactly one
/// `invalid_params` rejection on `window` — the fail-closed shape
/// every origin-resolution refusal must mint.
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

/// Asserts no `syn.control.receipt` notify for `id` reached `window` —
/// a rejected command must never mint a domain receipt.
fn assert_no_receipt(driver: &FakeDriver, window: WindowId, id: &str) {
    let receipts = notifies(driver, window, "syn.control.receipt");
    assert!(
        receipts.iter().all(|r| r["receipt"]["commandId"] != id),
        "rejected command must not reach the domain: {receipts:?}"
    );
}

#[test]
fn rvf_command_on_stale_window_after_reattach_is_rejected() {
    let (mut shell, mut driver) = harness();
    let a = ready_window(&mut shell, &mut driver);
    let b = second_window(&mut shell, &mut driver);
    let gen_a = attach(&mut shell, &mut driver, a, "console");

    // Dock transfer: the same view re-attaches on B — ownership
    // moves and the generation bumps.
    let gen_b = attach(&mut shell, &mut driver, b, "console");
    assert_eq!(shell.view_window("console"), Some(b));
    assert!(gen_b > gen_a, "reattach bumps the epoch");

    // A leftover command on the old channel names the view that no
    // longer lives here — it must never mint an op under the live
    // epoch (the runtime forwards ctx.window_id unchecked).
    action(
        &mut shell,
        &mut driver,
        a,
        rvf_command("stale-1", &rvf_gesture("begin", "g-stale", 3.0)),
    );
    assert_rejected(&driver, a, "stale-1");
    assert_no_receipt(&driver, a, "stale-1");
    assert_no_receipt(&driver, b, "stale-1");
    assert!(
        results_for(&driver, b, "stale-1").is_empty(),
        "nothing is routed to the new owner for a stale command"
    );
}

#[test]
fn rvf_command_follows_reattach_to_the_new_owner_window() {
    let (mut shell, mut driver) = harness();
    let a = ready_window(&mut shell, &mut driver);
    let b = second_window(&mut shell, &mut driver);
    attach(&mut shell, &mut driver, a, "console");
    attach(&mut shell, &mut driver, b, "console");
    action(
        &mut shell,
        &mut driver,
        b,
        Intent::new(
            "sub-b",
            "syn.control.subscribe",
            json!({ "view_id": "console", "target": "transport/position" }),
        ),
    );

    // The new owner window keeps minting commands — its
    // begin→commit lands translated as `applied`.
    action(
        &mut shell,
        &mut driver,
        b,
        rvf_command("cmd-b0", &rvf_gesture("begin", "g-b", 2.0)),
    );
    action(
        &mut shell,
        &mut driver,
        b,
        rvf_command("cmd-b1", &rvf_gesture("commit", "g-b", 5.0)),
    );
    let results = results_for(&driver, b, "cmd-b1");
    assert_eq!(results.len(), 1, "receipt-as-result: {results:?}");
    let (ok, data) = &results[0];
    assert!(ok, "new-owner command applies: {results:?}");
    assert_eq!(data.as_ref().unwrap()["status"], "applied");
}

#[test]
fn rvf_command_for_a_view_on_another_window_is_rejected() {
    let (mut shell, mut driver) = harness();
    let a = ready_window(&mut shell, &mut driver);
    let b = second_window(&mut shell, &mut driver);
    attach(&mut shell, &mut driver, a, "console");

    // B never owned the view — a command naming it on B's channel is
    // not B's to mint.
    action(
        &mut shell,
        &mut driver,
        b,
        rvf_command("foreign-1", &rvf_gesture("begin", "g-f", 1.0)),
    );
    assert_rejected(&driver, b, "foreign-1");
    assert_no_receipt(&driver, a, "foreign-1");
    assert_no_receipt(&driver, b, "foreign-1");
}

#[test]
fn rvf_command_naming_an_unattached_view_is_rejected() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);
    attach(&mut shell, &mut driver, window, "console");

    // `origin_view` names a view the shell never saw attach — it is
    // not adopted into a fresh op with a fabricated epoch.
    let mut g = rvf_gesture("begin", "g-ghost", 1.0);
    g["origin_view"] = json!("ghost");
    action(&mut shell, &mut driver, window, rvf_command("ghost-1", &g));
    assert_rejected(&driver, window, "ghost-1");
    assert_no_receipt(&driver, window, "ghost-1");
}

#[test]
fn rvf_command_without_origin_view_resolves_the_bound_view() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);
    attach(&mut shell, &mut driver, window, "console");
    action(
        &mut shell,
        &mut driver,
        window,
        Intent::new(
            "sub-1",
            "syn.control.subscribe",
            json!({ "view_id": "console", "target": "transport/position" }),
        ),
    );

    // No `origin_view` — the window's single bound view applies.
    let mut b = rvf_gesture("begin", "g-no", 2.0);
    b.as_object_mut().unwrap().remove("origin_view");
    action(&mut shell, &mut driver, window, rvf_command("cmd-n0", &b));
    let mut g = rvf_gesture("commit", "g-no", 4.0);
    g.as_object_mut().unwrap().remove("origin_view");
    action(&mut shell, &mut driver, window, rvf_command("cmd-no", &g));
    let results = results_for(&driver, window, "cmd-no");
    assert_eq!(results.len(), 1, "receipt-as-result: {results:?}");
    let (ok, data) = &results[0];
    assert!(ok, "unique bound view resolves: {results:?}");
    assert_eq!(data.as_ref().unwrap()["status"], "applied");
}

#[test]
fn rvf_command_without_origin_view_and_no_bound_view_is_rejected() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);

    let mut g = rvf_gesture("begin", "g-none", 1.0);
    g.as_object_mut().unwrap().remove("origin_view");
    action(&mut shell, &mut driver, window, rvf_command("none-1", &g));
    assert_rejected(&driver, window, "none-1");
}

#[test]
fn rvf_command_without_origin_view_ambiguous_binding_is_rejected() {
    let (mut shell, mut driver) = harness();
    let window = ready_window(&mut shell, &mut driver);
    attach(&mut shell, &mut driver, window, "console");
    attach(&mut shell, &mut driver, window, "mixer");

    // Two views bound to one window — `origin_view` is the only
    // unambiguous key; a silent pick would mint for the wrong view.
    let mut g = rvf_gesture("begin", "g-amb", 1.0);
    g.as_object_mut().unwrap().remove("origin_view");
    action(&mut shell, &mut driver, window, rvf_command("amb-1", &g));
    assert_rejected(&driver, window, "amb-1");
    assert_no_receipt(&driver, window, "amb-1");
}

/// `rvf_gesture` with an explicit `origin_view` — the declared
/// producer view on the wire.
fn rvf_gesture_on(view: &str, kind: &str, gesture: &str, value: f64) -> Value {
    let mut i = rvf_gesture(kind, gesture, value);
    i["origin_view"] = json!(view);
    i
}

#[test]
fn attach_owner_is_the_declared_window_not_the_arrival_channel() {
    let (mut shell, mut driver) = harness();
    let a = ready_window(&mut shell, &mut driver);
    let b = second_window(&mut shell, &mut driver);

    // The attach rides A's channel but declares B — the domain
    // attaches the view to B, and shell bookkeeping mirrors that:
    // A's channel never owned the view.
    action(
        &mut shell,
        &mut driver,
        a,
        Intent::new(
            "v-attach",
            "syn.view.attach",
            json!({ "view_id": "v", "window_id": b.to_string() }),
        ),
    );
    let (ok, _) = results_for(&driver, a, "v-attach")
        .last()
        .cloned()
        .expect("attach result");
    assert!(ok, "attach succeeded");
    assert_eq!(shell.view_window("v"), Some(b));

    action(
        &mut shell,
        &mut driver,
        a,
        rvf_command("v-stale", &rvf_gesture_on("v", "begin", "g-v", 1.0)),
    );
    assert_rejected(&driver, a, "v-stale");
    assert_no_receipt(&driver, a, "v-stale");

    // B — the declared owner — mints commands normally.
    action(
        &mut shell,
        &mut driver,
        b,
        rvf_command("v-b0", &rvf_gesture_on("v", "begin", "g-v", 1.0)),
    );
    action(
        &mut shell,
        &mut driver,
        b,
        rvf_command("v-b1", &rvf_gesture_on("v", "commit", "g-v", 2.0)),
    );
    let results = results_for(&driver, b, "v-b1");
    let (ok, data) = results.last().expect("receipt-as-result");
    assert!(ok, "owner-window command applies: {results:?}");
    assert_eq!(data.as_ref().unwrap()["status"], "applied");
}

#[test]
fn attach_declaring_a_dead_window_leaves_the_view_ownerless() {
    let (mut shell, mut driver) = harness();
    let a = ready_window(&mut shell, &mut driver);

    // `window_id` names no live window — the domain accepts the
    // string, but no channel gets to claim the view.
    action(
        &mut shell,
        &mut driver,
        a,
        Intent::new(
            "v-attach",
            "syn.view.attach",
            json!({ "view_id": "v", "window_id": "w999" }),
        ),
    );
    let (ok, _) = results_for(&driver, a, "v-attach")
        .last()
        .cloned()
        .expect("attach result");
    assert!(ok, "attach op accepted");
    assert!(
        shell.view_window("v").is_none(),
        "declared dead window — ownerless"
    );

    action(
        &mut shell,
        &mut driver,
        a,
        rvf_command("v-dead", &rvf_gesture_on("v", "begin", "g-d", 1.0)),
    );
    assert_rejected(&driver, a, "v-dead");
    assert_no_receipt(&driver, a, "v-dead");
}
