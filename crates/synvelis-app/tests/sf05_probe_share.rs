//! SF-05 slice3 acceptance (synvelis#108, SF-UX-06 headless side):
//! the app/UI wiring of the session-layer probe subscriber lease /
//! capture ownership proven in `tests/sf04_probe_wiring.rs` and the
//! workspace `probe_trace` suite.
//!
//! - `syn.probe.control` view-scoped actions (`subscribe`/
//!   `unsubscribe`/`capture_for`/`abort_capture`) resolve
//!   `view_id` → `ViewInstanceId` and reach the session APIs.
//! - Two views subscribing the same `(session, point, kind)` share
//!   one provider subscription — one binding, one capture job.
//! - `syn.view.close` releases only the closing view's lease; the
//!   provider subscription is removed only when the last lease
//!   leaves. A view-owned capture retreats to session authority
//!   instead of stopping work remaining subscribers need.
//! - `syn.probe.state` projects lease holders, capture owner, and
//!   the view→keys subscriber map without collapsing requested vs
//!   RT-confirmed state.
//! - The Signal Flow tab shows the main view's probe subscriptions
//!   as a read-only table driven by `MainScreenState`, never
//!   mutating project/DSP state.
//!
//! Real OS/GPUI, real two-window proof, and the #109 oracle join
//! stay `NotRun` (#114 gate).

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_ui::BackendReport;
use ruviform_ui::ipc::codes;
use ruviform_ui::view::{CellValue, ComponentSpec, TableSpec};
use serde_json::{Value, json};

use synvelis_app::{SessionEngineConfig, SessionRoutingEngine, SynvelisApp, main_screen_spec};
use synvelis_project::{
    ProjectOp, SemanticProbePoint, SemanticSignalEntityRef, SignalBoundary, StripId,
};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf05s3-{}-{}-{}",
        name,
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn engine_config() -> synvelis_player::lowering::EngineConfig {
    synvelis_player::lowering::EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 256,
    }
}

fn session_config(media_root: PathBuf) -> SessionEngineConfig {
    SessionEngineConfig {
        media_root,
        engine: engine_config(),
        prefetch: ruviform_media_reader::PrefetchConfig {
            block_frames: 256,
            block_count: 8,
            channels: 2,
            sample_rate: 48_000,
        },
        budget: synvelis_project::BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
        mode: synvelis_project::BuildMode::Realtime,
        runtime_instance: synvelis_project::RuntimeInstanceId(1),
        device_clock_epoch: synvelis_project::RuntimeClockEpochRef(1),
        engine_config_generation: 1,
    }
}

/// App + attached engine + one committed revision (same fixture
/// shape as `sf04_probe_wiring.rs`).
fn app_with_engine() -> (SynvelisApp, SessionRoutingEngine) {
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let engine = SessionRoutingEngine::new(session_config(scratch("media").join("media")));
    app.attach_routing_engine(engine.clone());
    let rev = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "sf05s3".into(),
            },
            rev,
        )
        .unwrap();
    (app, engine)
}

fn master_strip(app: &SynvelisApp) -> StripId {
    let project = app.workspace().session().project();
    project.routing.buses[&project.routing.master].strip
}

fn probe_targets(app: &SynvelisApp) -> Vec<String> {
    app.workspace()
        .session()
        .project()
        .control
        .targets
        .keys()
        .take(2)
        .map(|t| t.to_tagged())
        .collect()
}

fn probe_point(strip: StripId) -> Value {
    probe_point_at(strip, SignalBoundary::PostFader)
}

fn probe_point_at(strip: StripId, boundary: SignalBoundary) -> Value {
    serde_json::to_value(SemanticProbePoint {
        entity: SemanticSignalEntityRef::Strip(strip),
        boundary,
        port: None,
    })
    .unwrap()
}

fn subscribe_params(view: &str, target: &str, point: &Value) -> Value {
    json!({
        "action": "subscribe",
        "view_id": view,
        "target": target,
        "point": point,
        "kind": "metrics",
        "mode": "monitor",
    })
}

fn view_action(action: &str, view: &str, key: &Value) -> Value {
    json!({ "action": action, "view_id": view, "key": key })
}

fn capture_for_params(view: &str, key: &Value) -> Value {
    json!({
        "action": "capture_for",
        "view_id": view,
        "key": key,
        "max_frames": 64,
    })
}

/// Sorted string list — holder/subscriber order is an impl detail;
/// the assertions care about membership.
fn sorted_strings(value: &Value) -> Vec<String> {
    let mut v: Vec<String> = value
        .as_array()
        .map(|a| {
            a.iter()
                .map(|x| x.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Collects every `ComponentSpec` in a `ViewSpec` tree by key —
/// owned clones so the spec can drop.
fn spec_components(state: &synvelis_app::MainScreenState) -> HashMap<String, ComponentSpec> {
    fn walk(spec: &ComponentSpec, out: &mut HashMap<String, ComponentSpec>) {
        out.insert(spec.key().0.clone(), spec.clone());
        match spec {
            ComponentSpec::Panel(panel) => {
                for child in &panel.children {
                    walk(child, out);
                }
            }
            ComponentSpec::Tabs(tabs) => {
                for tab in &tabs.tabs {
                    walk(&tab.child, out);
                }
            }
            _ => {}
        }
    }
    let spec = main_screen_spec(state);
    let mut map = HashMap::new();
    walk(&spec.root, &mut map);
    map
}

fn probe_table(state: &synvelis_app::MainScreenState) -> TableSpec {
    match spec_components(state).get("signal_flow.probes") {
        Some(ComponentSpec::Table(table)) => table.clone(),
        other => panic!("signal_flow.probes table missing: {other:?}"),
    }
}

fn cell_text(cell: Option<&CellValue>) -> String {
    match cell {
        Some(CellValue::Text(t)) => t.clone(),
        other => format!("{other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Dedup: two views, one semantic point → one key, one binding,
// one capture job.
// ---------------------------------------------------------------------------

#[test]
fn two_views_share_one_probe_subscription() {
    let (mut app, _engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    assert_eq!(targets.len(), 2);
    let point = probe_point(strip);

    // view-1 subscribes first — the first live lease triggers the
    // single provider request.
    let r1 = app
        .op_probe_control(&subscribe_params("view-1", &targets[0], &point))
        .unwrap();
    let key = r1["key"].clone();
    assert_eq!(r1["view_id"], "view-1");
    assert_eq!(r1["targets"], json!([targets[0]]));

    // view-2 subscribes the same (point, kind) for a different
    // correlation target — same key, deduped provider-side.
    let r2 = app
        .op_probe_control(&subscribe_params("view-2", &targets[1], &point))
        .unwrap();
    assert_eq!(r2["key"], key, "same point+kind must dedup to one key");
    assert_eq!(
        sorted_strings(&r2["targets"]),
        {
            let mut v = targets.clone();
            v.sort();
            v
        },
        "both targets correlate to the shared key"
    );

    // Idempotent re-subscribe: view-1 again adds no lease and no
    // provider call — the lease set and binding count stay put.
    let r3 = app
        .op_probe_control(&subscribe_params("view-1", &targets[0], &point))
        .unwrap();
    assert_eq!(r3["key"], key);

    let state = app.op_probe_state();
    assert_eq!(state["engine_attached"], true);
    let bindings = state["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 1, "one shared provider binding");
    assert_eq!(bindings[0]["key"], key);
    assert_eq!(
        sorted_strings(&bindings[0]["holders"]),
        vec!["view-1".to_string(), "view-2".to_string()]
    );
    // Both views appear in the subscriber projection with the key.
    let subscribers = state["subscribers"].as_array().unwrap();
    assert_eq!(subscribers.len(), 2);
    for entry in subscribers {
        assert_eq!(entry["keys"], json!([key]));
    }
    let view_ids: Vec<&str> = subscribers
        .iter()
        .map(|s| s["view_id"].as_str().unwrap())
        .collect();
    assert!(view_ids.contains(&"view-1") && view_ids.contains(&"view-2"));

    // One capture job: view-1 captures; view-2's capture on the
    // same key is rejected port-side (AlreadyCapturing) — the
    // shared binding cannot mint a second job.
    app.op_probe_control(&capture_for_params("view-1", &key))
        .unwrap();
    let err = app
        .op_probe_control(&capture_for_params("view-2", &key))
        .unwrap_err();
    assert_eq!(err.code, codes::INVALID_PARAMS);
    let state = app.op_probe_state();
    assert_eq!(state["bindings"][0]["capture_owner"], "view-1");
}

// ---------------------------------------------------------------------------
// View close releases only that view's lease; last lease removes.
// ---------------------------------------------------------------------------

#[test]
fn view_close_releases_only_own_lease() {
    let (mut app, _engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    let point = probe_point(strip);

    app.op_probe_control(&subscribe_params("view-1", &targets[0], &point))
        .unwrap();
    app.op_probe_control(&subscribe_params("view-2", &targets[1], &point))
        .unwrap();

    // Close view-2 — only its lease leaves; view-1 keeps the
    // shared subscription alive.
    app.op_view_close(&json!({ "view_id": "view-2" })).unwrap();
    let state = app.op_probe_state();
    assert_eq!(state["bindings"].as_array().unwrap().len(), 1);
    assert_ne!(
        state["bindings"][0]["state"], "stopped",
        "shared binding must survive one view's close"
    );
    assert_eq!(
        sorted_strings(&state["bindings"][0]["holders"]),
        vec!["view-1".to_string()]
    );
    let subscribers = state["subscribers"].as_array().unwrap();
    assert_eq!(subscribers.len(), 1);
    assert_eq!(subscribers[0]["view_id"], "view-1");

    // Close view-1 — the last lease triggers provider removal
    // (probe_remove semantics: the binding ledger records stopped).
    app.op_view_close(&json!({ "view_id": "view-1" })).unwrap();
    let state = app.op_probe_state();
    assert_eq!(state["bindings"][0]["state"], "stopped");
    assert_eq!(state["subscribers"], json!([]));
}

// ---------------------------------------------------------------------------
// Owner-only abort + capture retreat on owner close.
// ---------------------------------------------------------------------------

#[test]
fn capture_is_owner_only_and_retreats_to_session_on_close() {
    let (mut app, _engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);

    // Key A — owner-only abort authorization on a shared key.
    let point_a = probe_point(strip);
    app.op_probe_control(&subscribe_params("view-1", &targets[0], &point_a))
        .unwrap();
    let key_a = app
        .op_probe_control(&subscribe_params("view-2", &targets[1], &point_a))
        .unwrap()["key"]
        .clone();

    // view-1 owns the capture; view-2 holds a lease but is not the
    // owner — abort is refused.
    app.op_probe_control(&capture_for_params("view-1", &key_a))
        .unwrap();
    let state = app.op_probe_state();
    assert_eq!(state["bindings"][0]["capture_owner"], "view-1");
    let err = app
        .op_probe_control(&view_action("abort_capture", "view-2", &key_a))
        .unwrap_err();
    assert_eq!(err.code, codes::INVALID_PARAMS);
    assert!(
        err.message.contains("does not own") || err.message.contains("owner"),
        "unexpected error: {err:?}"
    );

    // A view with no lease at all is refused before owner checks.
    let err = app
        .op_probe_control(&view_action("abort_capture", "view-3", &key_a))
        .unwrap_err();
    assert_eq!(err.code, codes::INVALID_PARAMS);

    // The owner aborts successfully; attribution clears.
    app.op_probe_control(&view_action("abort_capture", "view-1", &key_a))
        .unwrap();
    let state = app.op_probe_state();
    assert_eq!(state["bindings"][0]["capture_owner"], Value::Null);

    // Key B — capture retreat when the owner view closes while a
    // capture is in flight and another holder remains.
    let point_b = probe_point_at(strip, SignalBoundary::PreFader);
    app.op_probe_control(&subscribe_params("view-1", &targets[0], &point_b))
        .unwrap();
    let key_b = app
        .op_probe_control(&subscribe_params("view-2", &targets[1], &point_b))
        .unwrap()["key"]
        .clone();
    app.op_probe_control(&capture_for_params("view-1", &key_b))
        .unwrap();

    app.op_view_close(&json!({ "view_id": "view-1" })).unwrap();
    let state = app.op_probe_state();
    let binding_b = state["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["key"] == key_b)
        .expect("key B binding");
    assert_eq!(binding_b["capture_owner"], "session");
    assert_eq!(
        sorted_strings(&binding_b["holders"]),
        vec!["view-2".to_string()]
    );
    // Session authority remains distinct: view-2 still cannot abort
    // a session-owned capture; session stop_capture can.
    let err = app
        .op_probe_control(&view_action("abort_capture", "view-2", &key_b))
        .unwrap_err();
    assert_eq!(err.code, codes::INVALID_PARAMS);
    app.op_probe_control(&json!({
        "action": "stop_capture",
        "key": key_b,
    }))
    .unwrap();
    let state = app.op_probe_state();
    let binding_b = state["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["key"] == key_b)
        .expect("key B binding");
    assert_eq!(binding_b["capture_owner"], Value::Null);
}

// ---------------------------------------------------------------------------
// unsubscribe: explicit view release; non-holder fails closed.
// ---------------------------------------------------------------------------

#[test]
fn unsubscribe_releases_view_lease_and_fail_closed() {
    let (mut app, _engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    let point = probe_point(strip);

    app.op_probe_control(&subscribe_params("view-1", &targets[0], &point))
        .unwrap();
    let key = app
        .op_probe_control(&subscribe_params("view-2", &targets[1], &point))
        .unwrap()["key"]
        .clone();

    // view-3 never subscribed — every subscriber action fails
    // closed with INVALID_PARAMS (NotSubscriber), not a silent ok.
    for action in ["unsubscribe", "capture_for", "abort_capture"] {
        let params = if action == "capture_for" {
            capture_for_params("view-3", &key)
        } else {
            view_action(action, "view-3", &key)
        };
        let err = app.op_probe_control(&params).unwrap_err();
        assert_eq!(err.code, codes::INVALID_PARAMS, "{action}: {err:?}");
    }

    // Explicit unsubscribe of view-2 keeps view-1's lease.
    let released = app
        .op_probe_control(&view_action("unsubscribe", "view-2", &key))
        .unwrap();
    assert_eq!(released["view_id"], "view-2");
    assert!(released["released"].as_array().unwrap().contains(&key));
    assert_eq!(released["removed"], json!([]));
    let state = app.op_probe_state();
    assert_eq!(state["bindings"].as_array().unwrap().len(), 1);
    assert_eq!(
        sorted_strings(&state["bindings"][0]["holders"]),
        vec!["view-1".to_string()]
    );

    // Unsubscribing a view twice fails closed the second time.
    let err = app
        .op_probe_control(&view_action("unsubscribe", "view-2", &key))
        .unwrap_err();
    assert_eq!(err.code, codes::INVALID_PARAMS);

    // Malformed view_id is a param fault before any session call.
    let err = app
        .op_probe_control(&subscribe_params("not-a-view", &targets[0], &point))
        .unwrap_err();
    assert_eq!(err.code, codes::INVALID_PARAMS);
}

// ---------------------------------------------------------------------------
// Fail-closed without an engine.
// ---------------------------------------------------------------------------

#[test]
fn subscriber_actions_fail_closed_without_engine() {
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    let point = probe_point(strip);

    for params in [
        subscribe_params("view-1", &targets[0], &point),
        view_action("unsubscribe", "view-1", &json!({})),
        capture_for_params("view-1", &json!({})),
        view_action("abort_capture", "view-1", &json!({})),
    ] {
        let err = app.op_probe_control(&params).unwrap_err();
        assert_eq!(err.code, codes::SERVICE_ERROR, "{params}: {err:?}");
    }

    let state = app.op_probe_state();
    assert_eq!(state["engine_attached"], false);
    assert_eq!(state["subscribers"], json!([]));
}

// ---------------------------------------------------------------------------
// Signal Flow screen: probe rows projected into spec, read-only,
// on/off follows subscribe/unsubscribe; project state untouched.
// ---------------------------------------------------------------------------

#[test]
fn signal_flow_screen_shows_probe_subscriptions() {
    let (mut app, _engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    let point = probe_point(strip);

    let rev_before = app.workspace().session().revision();
    let project_before = serde_json::to_value(app.workspace().session().project()).unwrap();

    app.op_probe_control(&subscribe_params("view-1", &targets[0], &point))
        .unwrap();

    // The shared screen cell carries the main view's probe rows —
    // populated by the ui_pump tail's screen-state capture.
    let screen = app.main_screen_cell();
    let rows = screen.borrow().signal_flow_probes.clone();
    assert_eq!(rows.len(), 1, "one probe subscription row: {rows:?}");
    let row = &rows[0];
    assert!(row.point.contains("PostFader") || row.point.contains("post"));
    assert_eq!(row.kind, "Metrics");
    assert_eq!(row.mode, "Monitor");
    assert!(row.holders.contains("view-1"));

    // The spec renders them as a read-only table.
    let state = screen.borrow().clone();
    let table = probe_table(&state);
    assert_eq!(table.selection, None, "probe table stays read-only");
    assert_eq!(table.rows.len(), 1);
    let rendered: Vec<String> = table
        .rows
        .first()
        .unwrap()
        .iter()
        .map(|cell| cell_text(cell.as_ref()))
        .collect();
    assert!(rendered.iter().any(|c| c.contains("view-1")));

    // A second view's subscription does not add a row to the main
    // view's table — rows are per-view (the main view's leases).
    app.op_probe_control(&subscribe_params("view-2", &targets[1], &point))
        .unwrap();
    let screen = app.main_screen_cell();
    let rows = screen.borrow().signal_flow_probes.clone();
    assert_eq!(rows.len(), 1, "view-2's lease is not the main view's");

    // OFF: unsubscribe removes the row — the toggle state is
    // honest screen truth, not a stale label.
    let key = app.op_probe_state()["bindings"][0]["key"].clone();
    app.op_probe_control(&view_action("unsubscribe", "view-1", &key))
        .unwrap();
    let screen = app.main_screen_cell();
    assert!(
        screen.borrow().signal_flow_probes.is_empty(),
        "unsubscribe must clear the probe row"
    );

    // Probe lifecycle never touched project revision or the
    // project document itself.
    assert_eq!(app.workspace().session().revision(), rev_before);
    assert_eq!(
        serde_json::to_value(app.workspace().session().project()).unwrap(),
        project_before
    );
}
