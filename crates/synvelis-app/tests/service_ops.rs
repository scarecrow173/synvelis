//! Headless service-contract tests for `synvelis-app` (SV-GUI-01).
//!
//! These exercise the exact `syn.*` surface the GUI consumes —
//! `UiRuntime::dispatch_op` is the same dispatch path the host's
//! `HostEvent::Message` takes, so a pass here covers the GUI service
//! semantics. Real-window verification is separate (see the test spec).

use std::fs;
use std::sync::atomic::Ordering;

use serde_json::{Value, json};

use ruviform_ui::ipc::{ErrorBody, Event};
use ruviform_ui::{BackendReport, DataRegistry, UiRuntime};

use synvelis_app::{AppService, SynvelisApp};

fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("synvelis-app-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn backend() -> BackendReport {
    BackendReport::available("webview2", "test-1.0".to_string())
}

fn runtime() -> UiRuntime {
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(AppService::new(SynvelisApp::new(backend())))
        .expect("register AppService");
    rt
}

/// Dispatches `op` and extracts the single `result` event payload.
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

#[test]
fn app_info_reports_revisions_backend_and_counters() {
    let mut rt = runtime();
    let info = ok(&mut rt, "syn.app.info", json!({}));
    assert_eq!(info["name"], "synvelis");
    assert!(info["version"].is_string());
    assert_eq!(info["host_revision"], synvelis_app::HOST_REVISION);
    assert_eq!(info["ipc"], 1);
    assert_eq!(info["backend"]["name"], "webview2");
    assert_eq!(info["backend"]["state"], "Available");
    // this call itself is counted
    assert!(info["counters"]["intents"].as_u64().unwrap() >= 1);
}

#[test]
fn app_info_reports_logging_state_without_install() {
    // SYN-OBS-01: the logging section is always present — `off` when the
    // process entry point never attached an install outcome (tests,
    // headless callers), with empty sink/degraded lists.
    let mut rt = runtime();
    let info = ok(&mut rt, "syn.app.info", json!({}));
    assert_eq!(info["logging"]["state"], "off");
    assert_eq!(info["logging"]["sinks"], json!([]));
    assert_eq!(info["logging"]["degraded"], json!([]));
}

#[test]
fn capabilities_lists_unavailable_areas_honestly() {
    let mut rt = runtime();
    let caps = ok(&mut rt, "syn.app.capabilities", json!({}));
    for area in ["audio_runtime", "recorder", "multi_window", "plugins", "ai"] {
        assert_eq!(caps[area]["state"], "unavailable", "{area}");
    }
    assert_eq!(caps["project"]["state"], "available");
    assert_eq!(caps["workspace"]["state"], "available");
    assert_eq!(caps["views"]["state"], "available");
    // SF-05 slice 1: headless read model + visual ops are available;
    // the canvas stays deferred (synvelis#108).
    assert_eq!(caps["signal_flow"]["state"], "available");
}

#[test]
fn project_new_open_save_info_roundtrip() {
    let dir = temp_dir("project");
    let path = dir.to_string_lossy().to_string();

    let mut rt = runtime();
    let created = ok(
        &mut rt,
        "syn.project.new",
        json!({ "path": path, "name": "demo" }),
    );
    assert_eq!(created["name"], "demo");
    assert_eq!(created["saved"], true);
    assert_eq!(created["store"], true);
    let project_id = created["project_id"].clone();
    assert!(dir.join("HEAD").exists(), "bundle HEAD written");

    // Reopen through a fresh app — the same snapshot must load.
    let mut rt2 = runtime();
    let opened = ok(&mut rt2, "syn.project.open", json!({ "path": path }));
    assert_eq!(opened["name"], "demo");
    assert_eq!(opened["project_id"], project_id);
    assert_eq!(opened["saved"], true);

    let saved = ok(&mut rt2, "syn.project.save", json!({}));
    assert_eq!(saved["saved"], true);

    let info = ok(&mut rt2, "syn.project.info", json!({}));
    assert_eq!(info["project_id"], project_id);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn project_save_without_store_fails() {
    let mut rt = runtime();
    let err = fail(&mut rt, "syn.project.save", json!({}));
    assert_eq!(err["code"], "service_error");
}

#[test]
fn project_new_requires_path() {
    let mut rt = runtime();
    let err = fail(&mut rt, "syn.project.new", json!({}));
    assert_eq!(err["code"], "invalid_params");
}

#[test]
fn project_open_missing_bundle_fails() {
    let dir = temp_dir("missing");
    let mut rt = runtime();
    let err = fail(
        &mut rt,
        "syn.project.open",
        json!({ "path": dir.to_string_lossy() }),
    );
    assert_eq!(err["code"], "service_error");
}

#[test]
fn transport_set_toggles_session_state() {
    let mut rt = runtime();
    ok(&mut rt, "syn.transport.set", json!({ "playing": true }));
    let err = fail(&mut rt, "syn.transport.set", json!({}));
    assert_eq!(err["code"], "invalid_params");
}

#[test]
fn workspace_mount_reports_shell_window() {
    let mut rt = runtime();
    let mount = ok(&mut rt, "syn.workspace.mount", json!({}));
    assert_eq!(mount["shell_window"], 1);
    assert_eq!(mount["windows"], json!([]));
    assert_eq!(mount["mounted"], true);
}

#[test]
fn quit_sets_shared_flag() {
    let app = SynvelisApp::new(backend());
    let flag = app.quit_flag();
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(AppService::new(app))
        .expect("register AppService");
    let res = ok(&mut rt, "syn.app.quit", json!({}));
    assert_eq!(res["quit"], true);
    assert!(
        flag.load(Ordering::Acquire),
        "quit flag observable by host loop"
    );
}

#[test]
fn unknown_op_is_an_error_event_not_a_panic() {
    let mut rt = runtime();
    let err = fail(&mut rt, "syn.bogus", json!({}));
    assert_eq!(err["code"], "unknown_op");
}

#[test]
fn sys_capabilities_lists_syn_ops() {
    let mut rt = runtime();
    let caps = ok(&mut rt, "sys.capabilities", json!({}));
    let ops = serde_json::to_string(&caps).expect("caps serializes");
    for op in [
        "syn.app.info",
        "syn.app.quit",
        "syn.project.new",
        "syn.project.open",
        "syn.project.save",
        "syn.transport.set",
        "syn.workspace.mount",
    ] {
        assert!(ops.contains(op), "capabilities missing {op}");
    }
}

#[test]
fn headless_and_gui_dispatch_agree_on_contract() {
    // The GUI path feeds `HostEvent::Message` into `UiRuntime::handle`;
    // this drives the same service dispatch as `dispatch_op`, so the
    // wire-level intent → result correlation must be identical.
    let mut rt = runtime();
    let intent = serde_json::json!({
        "v": 1, "kind": "intent", "id": "wire-1",
        "op": "syn.project.info", "params": {},
    });
    let events = rt.handle(ruviform_ui::HostEvent::Message(intent.to_string()));
    let (ok, data) = events
        .iter()
        .find_map(|e| match e {
            Event::Result { id, ok, data, .. } => {
                assert_eq!(id, "wire-1");
                Some((*ok, data.clone()))
            }
            _ => None,
        })
        .expect("message intent yields a correlated result");
    assert!(ok);
    assert!(data.expect("result data")["project_id"].is_string());
}
