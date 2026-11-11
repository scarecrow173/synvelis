//! TIM-01 app-layer PPQN integration (synvelis#125 app slice) —
//! headless `syn.*` op evidence for `syn.project.set_ppqn` and the
//! app-layer precondition gate (transport-stopped / automation-write /
//! merge-commit).
//!
//! Every case drives the same `syn.*` string dispatch the product
//! host uses: `UiRuntime::dispatch_op` → `Service::call` →
//! `AppService` → `SynvelisApp`. Fixture mutation (project ops,
//! session control state) goes through `AppService::app_mut()` on
//! the same composed app — there is no second project model and no
//! test-only op surface.
//!
//! Boundary: this is headless evidence only. PPQN selection UI,
//! Presentation Driver time authority, real window/OS GUI, real
//! device, and product Oracle stay `NotRun` — fake/mock fixtures
//! here never claim those passes.

use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use serde_json::{Value, json};

use ruviform_ui::ipc::{ErrorBody, Event};
use ruviform_ui::{
    BackendReport, CallOutcome, DataRegistry, OpInfo, Service, ServiceContext, UiRuntime,
};

use synvelis_app::{AppService, SynvelisApp};
use synvelis_project::{
    AutomationLaneId, AutomationTimeScope, ControlOwner, ParamMapping, ParameterRole,
    ParameterTargetId, ProjectOp, ValueKind, is_issuable_raw,
};
use synvelis_workspace::{AutomationMode, ControlSession, WriteArm};

// ---------- shared headless harness (same pattern as rcd_04) ----------

/// Shares one [`AppService`] between the headless [`UiRuntime`]
/// (real `syn.*` string dispatch) and the test (fixture mutation via
/// `app_mut`). `UiRuntime` never exposes a registered service back,
/// so the service is wrapped in `Rc<RefCell>` and re-registered
/// through a delegating [`Service`] impl.
struct SharedService(Rc<RefCell<AppService>>);

impl Service for SharedService {
    fn ops(&self) -> &'static [OpInfo] {
        self.0.borrow().ops()
    }

    fn call(
        &mut self,
        op: &str,
        params: &Value,
        ctx: &mut ServiceContext<'_>,
    ) -> Result<CallOutcome, ErrorBody> {
        self.0.borrow_mut().call(op, params, ctx)
    }
}

struct Fixture {
    rt: UiRuntime,
    svc: Rc<RefCell<AppService>>,
}

fn fixture() -> Fixture {
    let app = SynvelisApp::new(BackendReport::available("webview2", "test-1.0".into()));
    let svc = Rc::new(RefCell::new(AppService::new(app)));
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(SharedService(svc.clone()))
        .expect("register AppService");
    Fixture { rt, svc }
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

// ---------- fixture reads/mutations through the same app ----------

fn live_ppqn(f: &Fixture) -> u32 {
    f.svc.borrow().app().workspace().session().project().ppqn.0
}

fn live_revision(f: &Fixture) -> u64 {
    f.svc.borrow().app().workspace().session().revision().0
}

fn engine_builds(f: &Fixture) -> u64 {
    f.svc
        .borrow()
        .app()
        .workspace()
        .session()
        .engine()
        .graph_builds()
}

fn engine_last_revision(f: &Fixture) -> u64 {
    f.svc
        .borrow()
        .app()
        .workspace()
        .session()
        .engine()
        .last_built_revision()
        .0
}

fn apply_project(f: &Fixture, op: ProjectOp) {
    let mut svc = f.svc.borrow_mut();
    let session = svc.app_mut().workspace_mut().session_mut();
    let rev = session.revision();
    session.apply(op, rev).expect("fixture op applies");
}

fn set_ppqn_params(old_ppqn: u32, new_ppqn: u32) -> Value {
    json!({ "old_ppqn": old_ppqn, "new_ppqn": new_ppqn, "policy": "exact" })
}

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("synvelis-tim01app-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Adds one track + one issuable continuous parameter target owned by
/// that track; returns `(track, target)`.
fn add_track_target(
    f: &Fixture,
    name: &str,
    base: f64,
) -> (synvelis_project::TrackId, ParameterTargetId) {
    apply_project(f, ProjectOp::AddTrack { name: "t".into() });
    let track = {
        let svc = f.svc.borrow();
        *svc.app()
            .workspace()
            .session()
            .project()
            .tracks
            .keys()
            .next_back()
            .unwrap()
    };
    apply_project(
        f,
        ProjectOp::AddParameterTarget {
            name: name.into(),
            owner: ControlOwner::Track(track),
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base,
        },
    );
    let target = {
        let svc = f.svc.borrow();
        *svc.app()
            .workspace()
            .session()
            .project()
            .control
            .targets
            .keys()
            .rfind(|id| is_issuable_raw(id.raw()))
            .unwrap()
    };
    (track, target)
}

fn add_automation_lane(f: &Fixture, target: ParameterTargetId) -> AutomationLaneId {
    apply_project(f, ProjectOp::AddAutomationLane { target });
    let svc = f.svc.borrow();
    *svc.app()
        .workspace()
        .session()
        .project()
        .control
        .lanes
        .keys()
        .next_back()
        .unwrap()
}

/// Arms `target` in `Touch` and opens a recording transaction on a
/// rolling control transport — the merge-commit-in-flight state the
/// app-layer gate must reject. Mirrors `open_recording` in the
/// workspace PC-C1 fixture.
fn open_recording_transaction(f: &Fixture, target: ParameterTargetId, lane: AutomationLaneId) {
    {
        let mut svc = f.svc.borrow_mut();
        let session = svc.app_mut().workspace_mut().session_mut();
        session
            .control_with(|c, p| c.bind_write_lane(p, target, lane))
            .unwrap();
        session
            .control_with(|c, p| c.set_mode(p, target, AutomationMode::Touch))
            .unwrap();
        session
            .control_with(|c, p| c.set_arm(p, target, WriteArm::Armed))
            .unwrap();
        session.control_mut().set_record_enabled(true);
        session.control_with(ControlSession::transport_play);
        let rev = session.revision();
        session.control_with(|c, p| c.record_start(p, rev)).unwrap();
    }
    assert!(
        f.svc
            .borrow()
            .app()
            .workspace()
            .session()
            .control()
            .recording()
            .is_some(),
        "fixture must leave an open recording transaction"
    );
}

// ---------------------------------------------------------------------------
// success path + projection
// ---------------------------------------------------------------------------

/// `syn.project.set_ppqn` applies `SetProjectPpqn` through the real
/// `syn.*` dispatch and `project_info` projects the new PPQN; the
/// engine receives the revision notification via `session.apply`.
#[test]
fn set_ppqn_applies_and_projects_ppqn() {
    let mut f = fixture();
    let info = ok(&mut f.rt, "syn.project.info", json!({}));
    assert_eq!(info["ppqn"], 960);
    let rev_before = info["revision"].as_u64().unwrap();
    let builds_before = engine_builds(&f);

    let res = ok(&mut f.rt, "syn.project.set_ppqn", set_ppqn_params(960, 480));
    assert_eq!(res["ppqn"], 480);
    let rev_after = res["revision"].as_u64().unwrap();
    assert!(rev_after > rev_before);
    assert_eq!(live_ppqn(&f), 480);

    // engine revision notification fired exactly once for the apply.
    assert_eq!(engine_builds(&f), builds_before + 1);
    assert_eq!(engine_last_revision(&f), rev_after);
}

/// `expected_revision` is optional: when present it is a controller
/// CAS — a stale value rejects and leaves the project untouched.
#[test]
fn set_ppqn_rejects_stale_expected_revision() {
    let mut f = fixture();
    let rev = live_revision(&f);

    ok(
        &mut f.rt,
        "syn.project.set_ppqn",
        json!({
            "old_ppqn": 960,
            "new_ppqn": 480,
            "policy": "exact",
            "expected_revision": rev,
        }),
    );
    assert_eq!(live_ppqn(&f), 480);

    // `rev` is now stale — the CAS rejects before any mutation.
    let err = fail(
        &mut f.rt,
        "syn.project.set_ppqn",
        json!({
            "old_ppqn": 480,
            "new_ppqn": 960,
            "policy": "exact",
            "expected_revision": rev,
        }),
    );
    assert_eq!(err["code"], "invalid_params");
    assert_eq!(live_ppqn(&f), 480);
}

/// A stale `old_ppqn` is rejected by the project-layer double CAS —
/// the app layer does not re-implement the check.
#[test]
fn set_ppqn_rejects_stale_old_ppqn() {
    let mut f = fixture();
    let err = fail(&mut f.rt, "syn.project.set_ppqn", set_ppqn_params(100, 480));
    assert_eq!(err["code"], "invalid_params");
    assert_eq!(live_ppqn(&f), 960);
}

/// `new_ppqn` outside `1..=32767` is rejected at the project layer.
#[test]
fn set_ppqn_rejects_out_of_range_ppqn() {
    for bad in [0_u64, 32_768, 4_294_967_295] {
        let mut f = fixture();
        let err = fail(
            &mut f.rt,
            "syn.project.set_ppqn",
            json!({
                "old_ppqn": 960,
                "new_ppqn": bad,
                "policy": "exact",
            }),
        );
        assert_eq!(err["code"], "invalid_params", "new_ppqn {bad}");
        assert_eq!(live_ppqn(&f), 960, "new_ppqn {bad}");
    }
}

// ---------------------------------------------------------------------------
// wire parameter contract
// ---------------------------------------------------------------------------

/// Policy mapping is explicit: `policy` is required, `"round"`
/// requires `rounding`, `"exact"` rejects it, unknown values and
/// missing fields are `invalid_params`.
#[test]
fn set_ppqn_requires_explicit_policy() {
    let mut f = fixture();

    for (label, params) in [
        (
            "missing policy",
            json!({ "old_ppqn": 960, "new_ppqn": 480 }),
        ),
        (
            "unknown policy",
            json!({ "old_ppqn": 960, "new_ppqn": 480, "policy": "bogus" }),
        ),
        (
            "round without rounding",
            json!({ "old_ppqn": 960, "new_ppqn": 480, "policy": "round" }),
        ),
        (
            "exact with rounding",
            json!({ "old_ppqn": 960, "new_ppqn": 480, "policy": "exact", "rounding": "floor" }),
        ),
        (
            "unknown rounding",
            json!({ "old_ppqn": 960, "new_ppqn": 480, "policy": "round", "rounding": "bogus" }),
        ),
        (
            "missing old_ppqn",
            json!({ "new_ppqn": 480, "policy": "exact" }),
        ),
        (
            "missing new_ppqn",
            json!({ "old_ppqn": 960, "policy": "exact" }),
        ),
    ] {
        let err = fail(&mut f.rt, "syn.project.set_ppqn", params);
        assert_eq!(err["code"], "invalid_params", "{label}");
    }
    assert_eq!(live_ppqn(&f), 960);
}

/// `policy:"round"` with a declared rounding succeeds where `exact`
/// rejects positions not representable on the target grid.
#[test]
fn set_ppqn_round_policy_applies_declared_rounding() {
    let mut f = fixture();
    // Seed an inexact musical position: a tempo event at tick 1 is
    // not representable on a 960 → 1000 PPQN grid.
    apply_project(
        &f,
        ProjectOp::AddTempoEvent {
            tick: 1,
            bpm_milli: 120_000,
        },
    );

    let err = fail(
        &mut f.rt,
        "syn.project.set_ppqn",
        json!({ "old_ppqn": 960, "new_ppqn": 1000, "policy": "exact" }),
    );
    assert_eq!(err["code"], "invalid_params");
    assert_eq!(live_ppqn(&f), 960);

    ok(
        &mut f.rt,
        "syn.project.set_ppqn",
        json!({
            "old_ppqn": 960,
            "new_ppqn": 1000,
            "policy": "round",
            "rounding": "floor",
        }),
    );
    assert_eq!(live_ppqn(&f), 1000);
}

// ---------------------------------------------------------------------------
// app-layer preconditions
// ---------------------------------------------------------------------------

/// Transport must be stopped: `syn.transport.set {playing:true}`
/// makes `set_ppqn` fail; stopping again lets it succeed.
#[test]
fn set_ppqn_rejects_while_transport_playing() {
    let mut f = fixture();
    ok(&mut f.rt, "syn.transport.set", json!({ "playing": true }));

    let err = fail(&mut f.rt, "syn.project.set_ppqn", set_ppqn_params(960, 480));
    assert_eq!(err["code"], "invalid_params");
    assert_eq!(live_ppqn(&f), 960);

    ok(&mut f.rt, "syn.transport.set", json!({ "playing": false }));
    ok(&mut f.rt, "syn.project.set_ppqn", set_ppqn_params(960, 480));
    assert_eq!(live_ppqn(&f), 480);
}

/// Automation write enabled (`record_enabled`) rejects the op —
/// fail-closed while the automation-write feature is armed.
#[test]
fn set_ppqn_rejects_while_record_enabled() {
    let mut f = fixture();
    {
        let mut svc = f.svc.borrow_mut();
        svc.app_mut()
            .workspace_mut()
            .session_mut()
            .control_mut()
            .set_record_enabled(true);
    }

    let err = fail(&mut f.rt, "syn.project.set_ppqn", set_ppqn_params(960, 480));
    assert_eq!(err["code"], "invalid_params");
    assert_eq!(live_ppqn(&f), 960);

    // Clearing the flag lifts the gate.
    {
        let mut svc = f.svc.borrow_mut();
        svc.app_mut()
            .workspace_mut()
            .session_mut()
            .control_mut()
            .set_record_enabled(false);
    }
    ok(&mut f.rt, "syn.project.set_ppqn", set_ppqn_params(960, 480));
    assert_eq!(live_ppqn(&f), 480);
}

/// An open recording transaction (merge-commit in flight) rejects
/// the op even though the control transport is the one playing —
/// the gate reads `session.transport()`, not `control.playback`.
#[test]
fn set_ppqn_rejects_while_recording_transaction_open() {
    let mut f = fixture();
    let (_track, target) = add_track_target(&f, "vol", 0.4);
    let lane = add_automation_lane(&f, target);
    open_recording_transaction(&f, target, lane);

    let err = fail(&mut f.rt, "syn.project.set_ppqn", set_ppqn_params(960, 480));
    assert_eq!(err["code"], "invalid_params");
    assert_eq!(live_ppqn(&f), 960);

    // Aborting the transaction and clearing the record flag lifts
    // both gates.
    {
        let mut svc = f.svc.borrow_mut();
        let session = svc.app_mut().workspace_mut().session_mut();
        session.control_record_abort();
        session.control_mut().set_record_enabled(false);
    }
    ok(&mut f.rt, "syn.project.set_ppqn", set_ppqn_params(960, 480));
    assert_eq!(live_ppqn(&f), 480);
}

// ---------------------------------------------------------------------------
// regression surface: undo/redo, save/reload, engine notification
// ---------------------------------------------------------------------------

/// `syn.project.undo` / `syn.project.redo` round-trip the PPQN
/// change through the existing controller snapshot path.
#[test]
fn set_ppqn_undo_redo_round_trip() {
    let mut f = fixture();
    let builds_before = engine_builds(&f);

    ok(&mut f.rt, "syn.project.set_ppqn", set_ppqn_params(960, 480));
    assert_eq!(live_ppqn(&f), 480);

    ok(&mut f.rt, "syn.project.undo", json!({}));
    assert_eq!(live_ppqn(&f), 960);
    ok(&mut f.rt, "syn.project.redo", json!({}));
    assert_eq!(live_ppqn(&f), 480);

    // apply + undo + redo each notified the engine.
    assert!(engine_builds(&f) >= builds_before + 3);
}

/// `syn.project.save` / `syn.project.reload` preserve the changed
/// PPQN through the bundle store.
#[test]
fn set_ppqn_survives_save_reload() {
    let dir = tmp_dir("save-reload");
    let path = dir.to_string_lossy().to_string();
    let mut f = fixture();

    ok(
        &mut f.rt,
        "syn.project.new",
        json!({ "path": path, "name": "ppqn" }),
    );
    ok(&mut f.rt, "syn.project.set_ppqn", set_ppqn_params(960, 480));
    ok(&mut f.rt, "syn.project.save", json!({}));
    ok(&mut f.rt, "syn.project.reload", json!({}));

    let info = ok(&mut f.rt, "syn.project.info", json!({}));
    assert_eq!(info["ppqn"], 480);
    let _ = fs::remove_dir_all(&dir);
}
