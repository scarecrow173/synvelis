//! SF-04 slice 4 acceptance (synvelis#107): the app/session wiring —
//! `SessionRoutingEngine`'s `ProbeTraceBridge` port supply,
//! `syn.probe.control`/`syn.probe.state` ops, and the `ui_pump`
//! drain hook.
//!
//! - Probe ON/OFF (`request`/`set_mode`/`remove`) works over the
//!   wire; the minted `key` round-trips verbatim.
//! - The same semantic point requested for two targets dedups to
//!   one subscription key (one capture job) while the session
//!   correlation fans drain updates out to both targets.
//! - `ui_pump` is the only drain path — `syn.probe.state` is
//!   read-only and reports the last `ProbeDrainReport`.
//! - Pending resolve states project honestly (`requested` /
//!   `queued-for-binding` / `unresolved`), never as observed-live.
//! - Probe ops leave project revision, undo history, engine graph
//!   builds and the activation receipt untouched.
//! - No attached engine → lifecycle ops fail closed and state ops
//!   report `engine_attached:false` — nothing is fabricated.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_ui::ipc::{Event, codes};
use ruviform_ui::{BackendReport, DataRegistry, UiRuntime};
use serde_json::{Value, json};

use synvelis_app::{AppService, SessionEngineConfig, SessionRoutingEngine, SynvelisApp};
use synvelis_player::driver::PumpPolicy;
use synvelis_project::{
    ProjectOp, SemanticProbePoint, SemanticSignalEntityRef, SignalBoundary, StripId,
};
use synvelis_workspace::EnginePort;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf04-{}-{}-{}",
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

/// App + attached engine + one committed revision so the engine has
/// a published plan to resolve probe points against. The bootstrap
/// project already carries the master bus/strip/fader, so no
/// project mutations are needed — and none must happen for probe
/// ops (they are not project commands).
fn app_with_engine() -> (SynvelisApp, SessionRoutingEngine) {
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let engine = SessionRoutingEngine::new(session_config(scratch("media").join("media")));
    app.attach_routing_engine(engine.clone());
    let rev = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "sf04".into(),
            },
            rev,
        )
        .unwrap();
    (app, engine)
}

/// The bootstrap master strip — always present, always lowered.
fn master_strip(app: &SynvelisApp) -> StripId {
    let project = app.workspace().session().project();
    project.routing.buses[&project.routing.master].strip
}

/// Two distinct bindable `ParameterTargetId`s from the control
/// model — the correlation side of a probe subscription.
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
    serde_json::to_value(SemanticProbePoint {
        entity: SemanticSignalEntityRef::Strip(strip),
        boundary: SignalBoundary::PostFader,
        port: None,
    })
    .unwrap()
}

fn request_params(target: &str, point: &Value) -> Value {
    json!({
        "action": "request",
        "target": target,
        "point": point,
        "kind": "metrics",
        "mode": "monitor",
    })
}

/// Wire-level dispatch — the same path `HostEvent::Message` takes.
fn runtime(app: SynvelisApp) -> UiRuntime {
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(AppService::new(app))
        .expect("register AppService");
    rt
}

fn call(rt: &mut UiRuntime, op: &str, params: Value) -> (bool, Option<Value>, Option<Value>) {
    let events = rt.dispatch_op("test-1", op, params);
    let result = events.iter().find_map(|e| match e {
        Event::Result {
            ok, data, error, ..
        } => Some((
            *ok,
            data.clone(),
            error.clone().map(|e| serde_json::to_value(e).unwrap()),
        )),
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
    error.expect("failure carries an error body")
}

// ---------------------------------------------------------------------------
// Fail-closed without an engine
// ---------------------------------------------------------------------------

#[test]
fn probe_ops_fail_closed_without_engine() {
    let mut rt = runtime(SynvelisApp::new(BackendReport::available(
        "gpui",
        "test-0.1".to_string(),
    )));

    let err = fail(
        &mut rt,
        "syn.probe.control",
        json!({"action": "remove", "key": {"session_id": 1, "point": {}, "kind": "metrics"}}),
    );
    assert_eq!(err["code"], codes::SERVICE_ERROR);

    // Unknown action is a param fault even before the engine check
    // matters — action parsing runs first.
    let err = fail(&mut rt, "syn.probe.control", json!({"action": "bogus"}));
    assert_eq!(err["code"], codes::INVALID_PARAMS);

    // State stays honest: unattached, empty — never fabricated.
    let state = ok(&mut rt, "syn.probe.state", json!({}));
    assert_eq!(state["engine_attached"], false);
    assert_eq!(state["bindings"], json!([]));
    assert_eq!(state["segments"], json!([]));
    assert_eq!(state["traces"], json!([]));
    assert_eq!(state["drain"]["updates"], 0);
}

// ---------------------------------------------------------------------------
// Wire lifecycle: request → state → set_mode → remove
// ---------------------------------------------------------------------------

#[test]
fn probe_lifecycle_over_the_wire() {
    let (app, engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    assert!(targets.len() >= 2, "fixture needs two param targets");
    let point = probe_point(strip);
    let mut rt = runtime(app);

    // request — ON. Returns the minted key + session-correlated targets.
    let result = ok(
        &mut rt,
        "syn.probe.control",
        request_params(&targets[0], &point),
    );
    let key = result["key"].clone();
    assert!(key["session_id"].is_u64());
    assert_eq!(result["targets"], json!([targets[0]]));

    // state — the binding exists with requested (pending) state,
    // never promoted to observed-live.
    let state = ok(&mut rt, "syn.probe.state", json!({}));
    assert_eq!(state["engine_attached"], true);
    let bindings = state["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0]["key"], key);
    assert_eq!(bindings[0]["requested_mode"], "monitor");
    assert_eq!(bindings[0]["confirmed_mode"], Value::Null);
    let resolve = bindings[0]["state"].as_str().unwrap();
    assert!(
        matches!(
            resolve,
            "requested"
                | "queued-for-binding"
                | "pending-rt-install"
                | "bound-active"
                | "unresolved"
                | "unavailable"
                | "unsupported"
        ),
        "unexpected resolve state {resolve}"
    );
    // Pending states must not read as RT-confirmed.
    if resolve != "bound-active" {
        assert_ne!(bindings[0]["confirmed_mode"], "monitor");
    }

    // Drive RT so install/status evidence lands, then pump via a
    // lifecycle op (the drain hook runs at every ui op tail).
    engine.run(8, PumpPolicy::Every(1)).unwrap();
    ok(
        &mut rt,
        "syn.probe.control",
        json!({"action": "set_mode", "key": key, "mode": "monitor"}),
    );

    let state = ok(&mut rt, "syn.probe.state", json!({}));
    let drain = &state["drain"];
    assert!(
        drain["updates"].as_u64().unwrap() > 0,
        "expected drained probe updates, got {drain}"
    );
    assert_eq!(drain["uncorrelated"], 0);
    // No DispatchTraceAdapter is routed in this fixture — every
    // correlated record_probe is an honest unrouted drop.
    assert_eq!(drain["recorded"], 0);
    assert!(drain["unrouted"].as_u64().unwrap() >= 1);

    // OFF via set_mode, then remove — the binding ledger keeps a
    // stopped record rather than vanishing.
    ok(
        &mut rt,
        "syn.probe.control",
        json!({"action": "set_mode", "key": key, "mode": "off"}),
    );
    ok(
        &mut rt,
        "syn.probe.control",
        json!({"action": "remove", "key": key}),
    );
    let state = ok(&mut rt, "syn.probe.state", json!({}));
    assert_eq!(state["bindings"][0]["state"], "stopped");

    // set_mode on the removed key fails closed — UnknownKey.
    let err = fail(
        &mut rt,
        "syn.probe.control",
        json!({"action": "set_mode", "key": key, "mode": "monitor"}),
    );
    assert_eq!(err["code"], codes::INVALID_PARAMS);
}

// ---------------------------------------------------------------------------
// Dedup: one semantic point → one key, two correlated targets,
// one capture job; focus changes cannot duplicate it.
// ---------------------------------------------------------------------------

#[test]
fn probe_dedup_same_point_two_targets_and_single_capture() {
    let (mut app, engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    assert_eq!(targets.len(), 2);
    let point = probe_point(strip);

    // Same (point, kind) for two targets → identical key: the
    // subscription is deduped at the key, not per target.
    let r1 = app
        .op_probe_control(&request_params(&targets[0], &point))
        .unwrap();
    let r2 = app
        .op_probe_control(&request_params(&targets[1], &point))
        .unwrap();
    let key = r1["key"].clone();
    assert_eq!(r2["key"], key, "same point+kind must dedup to one key");
    assert_eq!(
        r2["targets"].as_array().unwrap().len(),
        2,
        "both targets correlate to the one key"
    );

    // While monitoring, metric records drain as contract updates —
    // each fans out to *both* correlated targets, and with no
    // DispatchTraceAdapter routed in this fixture every record is an
    // honest unrouted drop (never silently recorded).
    engine.run(8, PumpPolicy::Every(1)).unwrap();
    app.ui_pump();
    let state = app.op_probe_state();
    let updates = state["drain"]["updates"].as_u64().unwrap();
    assert!(updates >= 1, "expected drained probe updates, got {state}");
    assert_eq!(state["drain"]["uncorrelated"], 0);
    assert_eq!(state["drain"]["recorded"], 0);
    assert_eq!(
        state["drain"]["unrouted"].as_u64().unwrap(),
        updates * 2,
        "one update must fan out to both correlated targets"
    );

    // Bind then start one bounded capture.
    app.op_probe_control(&json!({
        "action": "capture",
        "key": key,
        "max_frames": 8_192,
    }))
    .unwrap();

    // A second capture request on the same key is refused — there is
    // exactly one open capture job.
    let err = app
        .op_probe_control(&json!({
            "action": "capture",
            "key": key,
            "max_frames": 8_192,
        }))
        .unwrap_err();
    assert_eq!(err.code, codes::INVALID_PARAMS);

    // Change Focus (Signal Flow inspection mode) is pure view state —
    // it must not mint, drop or duplicate any probe/capture state.
    let state_before = app.op_probe_state();
    app.op_signal_flow_inspection(&json!({
        "action": "set_mode",
        "mode": "explore",
    }))
    .unwrap();
    let state_after = app.op_probe_state();
    assert_eq!(state_before["bindings"], state_after["bindings"]);
    assert_eq!(state_before["segments"], state_after["segments"]);

    // Stop the capture and drain the RT CaptureStopped evidence —
    // the single capture job closes exactly one segment.
    app.op_probe_control(&json!({"action": "stop_capture", "key": key}))
        .unwrap();
    engine.run(8, PumpPolicy::Every(1)).unwrap();
    app.ui_pump();
    let state = app.op_probe_state();
    assert_eq!(state["drain"]["uncorrelated"], 0);
    assert_eq!(
        state["segments"].as_array().unwrap().len(),
        1,
        "one capture job must close exactly one segment, got {state}"
    );

    app.op_probe_control(&json!({"action": "remove", "key": key}))
        .unwrap();
}

// ---------------------------------------------------------------------------
// Drain hook: ui_pump is the only path into observed slots
// ---------------------------------------------------------------------------

#[test]
fn ui_pump_drains_probe_updates_into_session_report() {
    let (mut app, engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    let point = probe_point(strip);

    let result = app
        .op_probe_control(&request_params(&targets[0], &point))
        .unwrap();
    let key = result["key"].clone();

    // The request op's own ui_pump already drained (nothing pending
    // yet). After RT blocks, an explicit pump is the only drain.
    engine.run(8, PumpPolicy::Every(1)).unwrap();
    let before = app.op_probe_state();
    assert_eq!(before["drain"]["updates"], 0, "state op must not drain");

    app.ui_pump();
    let after = app.op_probe_state();
    assert!(after["drain"]["updates"].as_u64().unwrap() > 0);
    assert_eq!(after["drain"]["uncorrelated"], 0);

    // Install evidence alone reaches BoundActive; `confirmed_mode`
    // stays empty until RT status for a real mode transition arrives
    // — requested and confirmed never blur on the wire.
    let binding = &after["bindings"][0];
    assert_eq!(binding["state"], "bound-active");
    assert_eq!(binding["confirmed_mode"], Value::Null);

    // A capture cycle produces CaptureStarted/CaptureStopped status
    // evidence: confirmed mode tracks RT, not the request.
    app.op_probe_control(&json!({
        "action": "capture", "key": key, "max_frames": 4_096,
    }))
    .unwrap();
    engine.run(8, PumpPolicy::Every(1)).unwrap();
    app.ui_pump();
    assert_eq!(
        app.op_probe_state()["bindings"][0]["confirmed_mode"],
        "capture"
    );

    app.op_probe_control(&json!({"action": "stop_capture", "key": key}))
        .unwrap();
    engine.run(8, PumpPolicy::Every(1)).unwrap();
    app.ui_pump();
    let settled = app.op_probe_state();
    assert_eq!(settled["bindings"][0]["confirmed_mode"], "monitor");
    assert_eq!(settled["segments"].as_array().unwrap().len(), 1);

    app.op_probe_control(&json!({"action": "remove", "key": key}))
        .unwrap();
}

// ---------------------------------------------------------------------------
// Invariants: probe ops never touch project/revision/engine truth
// ---------------------------------------------------------------------------

#[test]
fn probe_ops_leave_project_revision_undo_and_engine_untouched() {
    let (mut app, engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    let point = probe_point(strip);

    let (rev_before, project_before, builds_before, receipt_before) = {
        let session = app.workspace().session();
        (
            session.revision(),
            serde_json::to_value(session.project()).unwrap(),
            engine.graph_builds(),
            format!("{:?}", engine.receipt()),
        )
    };

    let result = app
        .op_probe_control(&request_params(&targets[0], &point))
        .unwrap();
    let key = result["key"].clone();
    app.op_probe_control(&json!({
        "action": "capture", "key": key, "max_frames": 4_096,
    }))
    .unwrap();
    let _ = app.op_probe_state();
    app.ui_pump();
    app.op_probe_control(&json!({"action": "stop_capture", "key": key}))
        .unwrap();
    app.op_probe_control(&json!({"action": "remove", "key": key}))
        .unwrap();
    let _ = app.op_probe_state();

    let session = app.workspace().session();
    assert_eq!(session.revision(), rev_before);
    assert_eq!(
        serde_json::to_value(session.project()).unwrap(),
        project_before
    );
    assert_eq!(engine.graph_builds(), builds_before);
    assert_eq!(format!("{:?}", engine.receipt()), receipt_before);
}

// ---------------------------------------------------------------------------
// Honest unresolvable states — never upgraded to observed-live
// ---------------------------------------------------------------------------

#[test]
fn unresolvable_probe_point_reports_honest_state() {
    let (mut app, _engine) = app_with_engine();
    let targets = probe_targets(&app);

    // A strip id that does not exist in the project.
    let bogus = StripId::new(u128::MAX);
    let point = probe_point(bogus);
    let result = app
        .op_probe_control(&request_params(&targets[0], &point))
        .unwrap();
    let key = result["key"].clone();

    let state = app.op_probe_state();
    let binding = &state["bindings"][0];
    assert_eq!(binding["key"], key);
    // Unresolved/Unavailable/Unsupported project verbatim — the
    // wire never shows them as bound or observed.
    assert!(matches!(
        binding["state"].as_str().unwrap(),
        "unresolved" | "unavailable" | "unsupported"
    ));
    assert_eq!(binding["confirmed_mode"], Value::Null);
    assert_eq!(binding["runtime_probe_id"], Value::Null);

    // Lifecycle ops on an unbound key fail closed (NotBound).
    let err = app
        .op_probe_control(&json!({
            "action": "capture", "key": key, "max_frames": 1_024,
        }))
        .unwrap_err();
    assert_eq!(err.code, codes::INVALID_PARAMS);
}
