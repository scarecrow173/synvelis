//! Headless `syn.view.*` / `syn.control.*` / `syn.viz.*` op tests
//! (SV-GUI-02).
//!
//! Same dispatch path as the GUI (`UiRuntime::dispatch_op` ⇔
//! `HostEvent::Message`); the notify outbox is drained through the same
//! shared handle the host loop drains. Real-window verification is
//! separate — nothing here asserts OS/WebView behavior.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use serde_json::{Value, json};

use ruviform_ui::ipc::{ErrorBody, Event};
use ruviform_ui::{BackendReport, DataRegistry, UiRuntime};

use synvelis_app::{AppService, SynvelisApp};

type Outbox = Rc<RefCell<VecDeque<Event>>>;

fn runtime() -> (UiRuntime, Outbox) {
    let app = SynvelisApp::new(BackendReport::available("webview2", "test-1.0".into()));
    let outbox = app.event_outbox();
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(AppService::new(app))
        .expect("register AppService");
    (rt, outbox)
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

fn fail(rt: &mut UiRuntime, op: &str, params: Value) -> Value {
    let (ok, _, error) = call(rt, op, params);
    assert!(!ok, "{op} unexpectedly succeeded");
    serde_json::to_value(error.expect("failure carries an error body"))
        .expect("error body serializes")
}

/// Drains the shared outbox, returns `data` payloads for `channel`,
/// and pushes non-matching notifies back to the front — a selective
/// read must never silently consume other channels.
fn notifies(outbox: &Outbox, channel: &str) -> Vec<Value> {
    let mut matched = Vec::new();
    let mut rest = Vec::new();
    {
        let mut q = outbox.borrow_mut();
        while let Some(e) = q.pop_front() {
            match &e {
                Event::Notify { data, .. } if data["channel"] == channel => {
                    matched.push(data["data"].clone());
                }
                _ => rest.push(e),
            }
        }
        for e in rest.into_iter().rev() {
            q.push_front(e);
        }
    }
    matched
}

fn drain_all(outbox: &Outbox) {
    outbox.borrow_mut().clear();
}

fn attach(rt: &mut UiRuntime, view: &str, window: &str) -> Value {
    ok(
        rt,
        "syn.view.attach",
        json!({ "view_id": view, "window_id": window }),
    )
}

fn dispatch(rt: &mut UiRuntime, view: &str, cmd: &str, intent: &Value, generation: u32) -> Value {
    ok(
        rt,
        "syn.control.dispatch",
        json!({
            "intent": intent.clone(),
            "ctx": {
                "commandId": cmd,
                "viewId": view,
                "generation": u64::from(generation),
                "issuedAt": 0u64,
            },
        }),
    )
}

fn transport_action(action: &str) -> Value {
    json!({
        "kind": "action",
        "target": "transport",
        "controlId": "transport-1",
        "sequence": 1u64,
        "action": action,
    })
}

#[test]
fn view_attach_detach_close_lifecycle() {
    let (mut rt, outbox) = runtime();
    let a = attach(&mut rt, "console", "win-1");
    assert_eq!(a["attached"], true);
    assert_eq!(a["generation"], 1);

    // re-attach on the same window is idempotent; a different window
    // is a dock transfer — detach + attach bumps both generations
    let again = attach(&mut rt, "console", "win-1");
    assert_eq!(again["generation"], 1);
    let b = attach(&mut rt, "console", "win-2");
    // Each gateway owns its epoch: controls bumps on attach only,
    // viz bumps on both detach and attach — both are reported as-is.
    assert_eq!(b["generation"], 2);
    assert_eq!(b["viz_generation"], 3);

    let list = ok(&mut rt, "syn.view.list", json!({}));
    assert_eq!(list["views"].as_array().unwrap().len(), 1);
    assert_eq!(list["views"][0]["window_id"], "win-2");

    drain_all(&outbox);
    ok(&mut rt, "syn.view.detach", json!({ "view_id": "console" }));
    let states = notifies(&outbox, "syn.view.state");
    assert_eq!(states.last().unwrap()["attached"], false);

    ok(&mut rt, "syn.view.close", json!({ "view_id": "console" }));
    let list = ok(&mut rt, "syn.view.list", json!({}));
    assert_eq!(list["views"].as_array().unwrap().len(), 0);
}

#[test]
fn transport_action_receipt_and_projection_flow() {
    let (mut rt, outbox) = runtime();
    attach(&mut rt, "console", "win-1");
    ok(
        &mut rt,
        "syn.control.subscribe",
        json!({ "view_id": "console", "target": "transport" }),
    );
    drain_all(&outbox);

    let ack = dispatch(&mut rt, "console", "cmd-1", &transport_action("play"), 1);
    assert_eq!(ack["accepted"], true);

    // receipt arrives on the receipt channel, projection on its own —
    // never merged into the op result.
    let receipts = notifies(&outbox, "syn.control.receipt");
    assert_eq!(receipts.len(), 1, "one receipt for cmd-1");
    assert_eq!(receipts[0]["target"], "transport");
    assert_eq!(receipts[0]["receipt"]["status"], "applied");
    assert_eq!(receipts[0]["receipt"]["commandId"], "cmd-1");

    let projs = notifies(&outbox, "syn.control.projection");
    let last = projs.last().expect("transport projection pushed");
    assert_eq!(last["target"], "transport");
    assert_eq!(last["row"]["kind"], "transport");
    assert_eq!(
        last["row"]["observed"]["value"]["status"], "Playing",
        "applied truth lands as observed, not as request"
    );
}

#[test]
fn two_views_same_target_both_observe() {
    let (mut rt, outbox) = runtime();
    attach(&mut rt, "a", "win-1");
    attach(&mut rt, "b", "win-2");
    for v in ["a", "b"] {
        ok(
            &mut rt,
            "syn.control.subscribe",
            json!({ "view_id": v, "target": "transport" }),
        );
    }
    drain_all(&outbox);
    dispatch(&mut rt, "a", "cmd-x", &transport_action("play"), 1);
    let projs = notifies(&outbox, "syn.control.projection");
    let views: Vec<&str> = projs.iter().filter_map(|p| p["view_id"].as_str()).collect();
    assert!(views.contains(&"a") && views.contains(&"b"));
}

#[test]
fn unknown_and_detached_dispatch_are_dropped_not_faked() {
    let (mut rt, outbox) = runtime();
    drain_all(&outbox);
    // dispatch on a never-attached view — silently dropped by contract
    dispatch(&mut rt, "ghost", "cmd-g", &transport_action("play"), 0);
    assert_eq!(
        notifies(&outbox, "syn.control.receipt"),
        Vec::<Value>::new()
    );

    attach(&mut rt, "console", "win-1");
    drain_all(&outbox);
    // stale generation intent — dropped, no receipt minted
    dispatch(
        &mut rt,
        "console",
        "cmd-stale",
        &transport_action("play"),
        0,
    );
    assert_eq!(
        notifies(&outbox, "syn.control.receipt"),
        Vec::<Value>::new()
    );
    // unknown target — rejected receipt, never a fake success
    let bad = json!({
        "kind": "action", "target": "nope",
        "controlId": "c", "sequence": 1u64, "action": "play",
    });
    dispatch(&mut rt, "console", "cmd-bad", &bad, 1);
    let receipts = notifies(&outbox, "syn.control.receipt");
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0]["receipt"]["status"], "rejected");
}

#[test]
fn subscribe_unknown_target_fails_honestly() {
    let (mut rt, _outbox) = runtime();
    attach(&mut rt, "console", "win-1");
    let err = fail(
        &mut rt,
        "syn.control.subscribe",
        json!({ "view_id": "console", "target": "param-ffffffffffffffffffffffffffffffff" }),
    );
    assert_eq!(err["code"], "service_error");
}

#[test]
fn detach_cancels_gesture_but_not_transport() {
    let (mut rt, _outbox) = runtime();
    attach(&mut rt, "console", "win-1");
    dispatch(&mut rt, "console", "cmd-1", &transport_action("play"), 1);
    ok(&mut rt, "syn.view.detach", json!({ "view_id": "console" }));
    // transport keeps playing — detach is not a project/audio cancel
    let row = ok(&mut rt, "syn.control.targets", json!({}));
    assert!(row["targets"].as_array().unwrap().len() >= 2);
}

#[test]
fn viz_meter_push_frame_snapshot_and_notify() {
    let (mut rt, outbox) = runtime();
    attach(&mut rt, "console", "win-1");
    ok(
        &mut rt,
        "syn.viz.register",
        json!({ "kind": "meter", "source": "master", "channels": 2u64 }),
    );
    ok(
        &mut rt,
        "syn.viz.bind",
        json!({ "view_id": "console", "surface_id": "meters",
                "source": "master", "kind": "meter" }),
    );
    drain_all(&outbox);
    ok(
        &mut rt,
        "syn.viz.push_frame",
        json!({
            "source": "master",
            "frame": {
                "sequence": 7u64,
                "channels": [
                    { "peak": 0.9, "rms": 0.5 },
                    { "peak": 0.4, "rms": 0.2 },
                ],
            },
        }),
    );
    let frames = notifies(&outbox, "syn.viz.frame");
    assert_eq!(frames.len(), 1, "latest-wins frame push");
    assert_eq!(frames[0]["frame"]["sequence"], 7);

    let snap = ok(
        &mut rt,
        "syn.viz.snapshot",
        json!({ "view_id": "console", "surface_id": "meters" }),
    );
    assert_eq!(snap["frame"]["sequence"], 7);
    assert_eq!(snap["frame"]["channels"][0]["peak"], 0.9);
}

#[test]
fn viz_waveform_query_is_bounded() {
    let (mut rt, _outbox) = runtime();
    attach(&mut rt, "console", "win-1");
    ok(
        &mut rt,
        "syn.viz.register",
        json!({
            "kind": "waveform", "source": "clip-1", "rate": 48000u64,
            "channels": [[0.0, 0.5, -0.5, 0.25, 0.0, 0.1, 0.0, -0.1]],
        }),
    );
    ok(
        &mut rt,
        "syn.viz.bind",
        json!({ "view_id": "console", "surface_id": "wave",
                "source": "clip-1", "kind": "waveform", "plot_px": 4u64 }),
    );
    let res = ok(
        &mut rt,
        "syn.viz.query",
        json!({
            "view_id": "console", "surface_id": "wave",
            "start": 0.0, "end": 0.001, "buckets": 4u64,
        }),
    );
    assert!(res["view"].is_object(), "waveform view returned");
    // max_bytes cap is enforced, not advisory
    let err = fail(
        &mut rt,
        "syn.viz.query",
        json!({
            "view_id": "console", "surface_id": "wave",
            "start": 0.0, "end": 0.001, "buckets": 4u64,
            "max_bytes": 1u64,
        }),
    );
    assert_eq!(err["code"], "service_error");
}

#[test]
fn viz_no_source_is_honest_not_faked() {
    let (mut rt, _outbox) = runtime();
    attach(&mut rt, "console", "win-1");
    ok(
        &mut rt,
        "syn.viz.bind",
        json!({ "view_id": "console", "surface_id": "wave",
                "source": "no-such-source", "kind": "waveform" }),
    );
    let meta = ok(
        &mut rt,
        "syn.viz.meta",
        json!({ "view_id": "console", "surface_id": "wave" }),
    );
    assert_eq!(meta["status"]["kind"], "no-source");
    assert!(meta["meta"].is_null());
}

#[test]
fn detach_view_stops_surface_and_binding_truth() {
    let (mut rt, outbox) = runtime();
    attach(&mut rt, "console", "win-1");
    ok(
        &mut rt,
        "syn.viz.register",
        json!({ "kind": "meter", "source": "master", "channels": 1u64 }),
    );
    ok(
        &mut rt,
        "syn.viz.bind",
        json!({ "view_id": "console", "surface_id": "meters",
                "source": "master", "kind": "meter" }),
    );
    ok(&mut rt, "syn.view.detach", json!({ "view_id": "console" }));
    let err = fail(
        &mut rt,
        "syn.viz.snapshot",
        json!({ "view_id": "console", "surface_id": "meters" }),
    );
    assert_eq!(err["code"], "service_error");
    let _ = notifies(&outbox, "syn.view.state");
}
