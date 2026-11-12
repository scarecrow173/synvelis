//! SF-04 slice 10 acceptance (synvelis#107, SF-PROBE-07 wire side):
//! `syn.probe.compare` — the `ProbeComparisonRequest →
//! AcousticObservation` consumer exposed as an app op.
//!
//! - `params.request` is the serde `ProbeComparisonRequest`
//!   verbatim; the `AcousticObservation` maps field-by-field to
//!   JSON (`quality`/`basis`/`gap_mask`/`provenance` — the
//!   contract type carries clock ids and has no serde).
//! - `Aligned`/`Conditional`/`Incomparable` all reach the wire
//!   through `AppService` dispatch on a real attached engine.
//! - Consumer-side fail-closed reasons (`point-unresolved`,
//!   `probe-not-bound`, `no-observation-data`, `coverage-empty`,
//!   `evidence-budget-exceeded`) project verbatim — never
//!   promoted.
//! - The op is read-only like `syn.probe.state`: it never drains,
//!   so evidence is exactly what the last `ui_pump` retained.
//! - No attached engine → `engine_attached:false` +
//!   `observation:null`; a malformed request is `INVALID_PARAMS`
//!   before the engine check.
//!
//! Real OS/GPUI, the #108 UI consumer, and the #109 oracle join
//! stay `NotRun` (#114 gate).

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

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf04s10-{}-{}-{}",
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
/// a published plan to resolve probe points against (same fixture
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
                name: "sf04s10".into(),
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

fn probe_point_at(strip: StripId, boundary: SignalBoundary) -> Value {
    serde_json::to_value(SemanticProbePoint {
        entity: SemanticSignalEntityRef::Strip(strip),
        boundary,
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

/// `syn.probe.compare` params — `params.request` is the serde
/// `ProbeComparisonRequest` verbatim.
fn compare_params(before: &Value, after: &Value) -> Value {
    json!({
        "request": {
            "before": before,
            "after": after,
            "metric_set": ["peak-rms"],
            "max_bytes": 1 << 20,
        }
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

/// Requests monitor-mode subscriptions for `before`/`after` through
/// the wire and returns their minted keys — the two sides a
/// comparison needs.
fn subscribe_pair(rt: &mut UiRuntime, targets: &[String], before: &Value, after: &Value) -> Value {
    let ra = ok(rt, "syn.probe.control", request_params(&targets[0], before));
    let rb = ok(rt, "syn.probe.control", request_params(&targets[1], after));
    json!({ "before": ra["key"].clone(), "after": rb["key"].clone() })
}

/// Runs RT blocks then pumps through a mutating op — the only drain
/// path (`ui_pump` tail) that feeds retained comparison coverage.
fn run_and_pump(rt: &mut UiRuntime, engine: &SessionRoutingEngine, blocks: usize) {
    engine.run(blocks, PumpPolicy::Every(1)).unwrap();
    // `set_mode` to the already-requested mode is the lightest
    // pump-driving op; its tail runs `ui_pump` → `drain_probe`.
    let state = ok(rt, "syn.probe.state", json!({}));
    let key = state["bindings"][0]["key"].clone();
    ok(
        rt,
        "syn.probe.control",
        json!({ "action": "set_mode", "key": key, "mode": "monitor" }),
    );
}

// ---------------------------------------------------------------------------
// Fail-closed without an engine / malformed params
// ---------------------------------------------------------------------------

#[test]
fn compare_fails_closed_without_engine() {
    let mut rt = runtime(SynvelisApp::new(BackendReport::available(
        "gpui",
        "test-0.1".to_string(),
    )));

    let strip = StripId::new(1);
    let before = probe_point_at(strip, SignalBoundary::PostFader);
    let after = probe_point_at(strip, SignalBoundary::PreFader);

    // Valid request, no engine → honest unavailable state, never an
    // error and never a fabricated observation.
    let result = ok(
        &mut rt,
        "syn.probe.compare",
        compare_params(&before, &after),
    );
    assert_eq!(result["engine_attached"], false);
    assert_eq!(result["observation"], Value::Null);

    // A malformed request is a param fault even without an engine —
    // params validate before the attachment check.
    let err = fail(&mut rt, "syn.probe.compare", json!({}));
    assert_eq!(err["code"], codes::INVALID_PARAMS);
    let err = fail(
        &mut rt,
        "syn.probe.compare",
        json!({ "request": { "before": {}, "after": {} } }),
    );
    assert_eq!(err["code"], codes::INVALID_PARAMS);
}

// ---------------------------------------------------------------------------
// Aligned — two monitored points on one cursor series
// ---------------------------------------------------------------------------

#[test]
fn compare_aligned_two_monitored_points() {
    let (app, engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    assert!(targets.len() >= 2, "fixture needs two param targets");
    // PostFader (main-out) and PreFader (fader pre-out tap) are
    // distinct physical observation points on the same cursor
    // series — a real two-probe comparison, not a self-compare.
    let before = probe_point_at(strip, SignalBoundary::PostFader);
    let after = probe_point_at(strip, SignalBoundary::PreFader);
    let mut rt = runtime(app);

    subscribe_pair(&mut rt, &targets, &before, &after);
    run_and_pump(&mut rt, &engine, 8);

    let result = ok(
        &mut rt,
        "syn.probe.compare",
        compare_params(&before, &after),
    );
    assert_eq!(result["engine_attached"], true);
    let obs = &result["observation"];

    // The request echoes through verbatim.
    assert_eq!(obs["before"], before);
    assert_eq!(obs["after"], after);

    let quality = &obs["quality"];
    assert_eq!(quality["kind"], "aligned", "got {quality}");
    assert_eq!(quality["basis"]["kind"], "shared-cursor");
    assert!(quality["basis"]["clock"]["domain"].is_u64());
    assert!(quality["basis"]["clock"]["epoch"].is_u64());
    assert!(quality["offset_frames"].is_i64());
    assert!(quality["error_budget_frames"].is_u64());

    // The compared window and provenance are explicit evidence.
    assert!(obs["interval"]["start"].is_u64());
    assert!(obs["interval"]["end"].is_u64());
    assert_eq!(obs["gap_mask"], json!([]));
    let provenance = &obs["provenance"];
    assert_eq!(
        provenance["before_clock"]["domain"], provenance["after_clock"]["domain"],
        "one cursor series — one clock domain"
    );
    assert_eq!(provenance["sample_rate"], 48_000);
    assert!(provenance["runtime_instance"].is_u64());
    assert!(provenance["before_generations"]["first"].is_u64());
    assert!(provenance["after_generations"]["last"].is_u64());
}

// ---------------------------------------------------------------------------
// Conditional — uncovered request regions are gaps, never Aligned
// ---------------------------------------------------------------------------

#[test]
fn compare_conditional_partial_coverage() {
    let (app, engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    let before = probe_point_at(strip, SignalBoundary::PostFader);
    let after = probe_point_at(strip, SignalBoundary::PreFader);
    let mut rt = runtime(app);

    subscribe_pair(&mut rt, &targets, &before, &after);
    run_and_pump(&mut rt, &engine, 8);

    // Learn the observed extent, then ask for a window running past
    // it — the uncovered tail is a gap, not silent validity.
    let extent = ok(
        &mut rt,
        "syn.probe.compare",
        compare_params(&before, &after),
    );
    let interval = extent["observation"]["interval"].clone();
    assert!(interval["start"].is_u64() && interval["end"].is_u64());

    let mut params = compare_params(&before, &after);
    params["request"]["observed_range"] = json!({
        "start": interval["start"],
        "end": interval["end"].as_u64().unwrap() + 4_096,
    });
    let result = ok(&mut rt, "syn.probe.compare", params);
    let obs = &result["observation"];

    let quality = &obs["quality"];
    assert_eq!(quality["kind"], "conditional", "got {quality}");
    let conditions = quality["conditions"].as_array().unwrap();
    assert!(
        conditions.iter().any(|c| c["kind"] == "partial-coverage"),
        "uncovered request tail must demote to conditional: {conditions:?}"
    );
    assert!(
        !quality["excluded_intervals"].as_array().unwrap().is_empty(),
        "excluded intervals name the uncovered overlap"
    );
    for side in ["before", "after"] {
        assert!(
            obs["gap_mask"]
                .as_array()
                .unwrap()
                .iter()
                .any(|g| g["side"] == side && g["kind"] == "unknown"),
            "unobserved {side} window tail must surface as an unknown gap"
        );
    }
}

// ---------------------------------------------------------------------------
// Incomparable — consumer-side resolution failures project verbatim
// ---------------------------------------------------------------------------

#[test]
fn compare_incomparable_resolution_failures() {
    let (app, engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    let before = probe_point_at(strip, SignalBoundary::PostFader);
    let after = probe_point_at(strip, SignalBoundary::PreFader);
    let mut rt = runtime(app);

    subscribe_pair(&mut rt, &targets, &before, &after);
    run_and_pump(&mut rt, &engine, 4);

    // `before` names a point nobody subscribed — no binding exists.
    let ghost_point = probe_point_at(StripId::new(0xDEAD_BEEF), SignalBoundary::Output);
    let result = ok(
        &mut rt,
        "syn.probe.compare",
        compare_params(&ghost_point, &after),
    );
    let reason = &result["observation"]["quality"]["reason"];
    assert_eq!(result["observation"]["quality"]["kind"], "incomparable");
    assert_eq!(reason["kind"], "point-unresolved");
    assert_eq!(reason["side"], "before");
    assert_eq!(result["observation"]["interval"], Value::Null);

    // A binding whose point cannot resolve never gets a runtime
    // probe — `probe-not-bound`, distinct from `point-unresolved`.
    let unresolvable = probe_point_at(StripId::new(u128::MAX), SignalBoundary::Output);
    ok(
        &mut rt,
        "syn.probe.control",
        request_params(&targets[0], &unresolvable),
    );
    let result = ok(
        &mut rt,
        "syn.probe.compare",
        compare_params(&unresolvable, &after),
    );
    let reason = &result["observation"]["quality"]["reason"];
    assert_eq!(reason["kind"], "probe-not-bound");
    assert_eq!(reason["side"], "before");
}

#[test]
fn compare_incomparable_window_and_budget_failures() {
    let (app, engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    let before = probe_point_at(strip, SignalBoundary::PostFader);
    let after = probe_point_at(strip, SignalBoundary::PreFader);
    let mut rt = runtime(app);

    subscribe_pair(&mut rt, &targets, &before, &after);
    // No run, no pump — bindings exist but no coverage evidence was
    // ever drained.
    let result = ok(
        &mut rt,
        "syn.probe.compare",
        compare_params(&before, &after),
    );
    let reason = &result["observation"]["quality"]["reason"];
    assert_eq!(reason["kind"], "no-observation-data");
    assert_eq!(reason["side"], "before");

    run_and_pump(&mut rt, &engine, 4);

    // A degenerate requested window fails closed before evidence
    // work — `coverage-empty`, alignment never attempted.
    let mut params = compare_params(&before, &after);
    params["request"]["observed_range"] = json!({ "start": 8, "end": 8 });
    let result = ok(&mut rt, "syn.probe.compare", params);
    assert_eq!(
        result["observation"]["quality"]["reason"]["kind"],
        "coverage-empty"
    );
    assert_eq!(result["observation"]["interval"], Value::Null);

    // The evidence budget bounds retained coverage fed to
    // alignment — `max_bytes: 1` fails closed after resolution.
    let mut params = compare_params(&before, &after);
    params["request"]["max_bytes"] = json!(1);
    let result = ok(&mut rt, "syn.probe.compare", params);
    let reason = &result["observation"]["quality"]["reason"];
    assert_eq!(reason["kind"], "evidence-budget-exceeded");
    assert!(
        result["observation"]["interval"].is_object(),
        "resolved sides still report their window"
    );
}

// ---------------------------------------------------------------------------
// Read-only: compare never drains and never mutates projections
// ---------------------------------------------------------------------------

#[test]
fn compare_is_read_only_and_never_drains() {
    let (app, engine) = app_with_engine();
    let strip = master_strip(&app);
    let targets = probe_targets(&app);
    let before = probe_point_at(strip, SignalBoundary::PostFader);
    let after = probe_point_at(strip, SignalBoundary::PreFader);
    let mut rt = runtime(app);

    subscribe_pair(&mut rt, &targets, &before, &after);
    run_and_pump(&mut rt, &engine, 4);

    let state_before = ok(&mut rt, "syn.probe.state", json!({}));
    let drain_before = state_before["drain"].clone();
    let bindings_before = state_before["bindings"].clone();

    // Compare twice — identical evidence in, identical evidence
    // state out: the op never drains, never mutates the ledger.
    let first = ok(
        &mut rt,
        "syn.probe.compare",
        compare_params(&before, &after),
    );
    let second = ok(
        &mut rt,
        "syn.probe.compare",
        compare_params(&before, &after),
    );
    assert_eq!(first, second, "same retained evidence → same observation");

    let state_after = ok(&mut rt, "syn.probe.state", json!({}));
    assert_eq!(state_after["drain"], drain_before);
    assert_eq!(state_after["bindings"], bindings_before);

    // Project truth is untouched — compare is not a project command.
    let project_before = ok(&mut rt, "syn.project.info", json!({}));
    let _ = ok(
        &mut rt,
        "syn.probe.compare",
        compare_params(&before, &after),
    );
    let project_after = ok(&mut rt, "syn.project.info", json!({}));
    assert_eq!(project_before, project_after);
}
