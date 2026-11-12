//! TIM-01 PPQN preview + new-project PPQN (synvelis#125 preview
//! slice) — headless `syn.*` op evidence for
//! `syn.project.preview_ppqn` (read-only dry-run) and the optional
//! `ppqn` param on `syn.project.new`.
//!
//! Every case drives the same `syn.*` string dispatch the product
//! host uses: `UiRuntime::dispatch_op` → `Service::call` →
//! `AppService` → `SynvelisApp`. Fixture mutation (project ops,
//! session control state) goes through `AppService::app_mut()` on
//! the same composed app — there is no second project model and no
//! test-only op surface.
//!
//! Boundary: this is headless evidence only. PPQN selection UI,
//! Snap/Grid re-binding policy, Presentation Driver time authority,
//! real window/OS GUI, real device, and product Oracle stay `NotRun`
//! — fake/mock fixtures here never claim those passes.

use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use serde_json::{Value, json};

use ruviform_ui::ipc::{ErrorBody, Event};
use ruviform_ui::{
    BackendReport, CallOutcome, DataRegistry, OpInfo, Service, ServiceContext, UiRuntime,
};

use synvelis_app::{AppService, SynvelisApp};
use synvelis_project::ProjectOp;

// ---------- shared headless harness (same pattern as tim01_ppqn_app) ----------

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

fn live_project_json(f: &Fixture) -> Value {
    serde_json::to_value(f.svc.borrow().app().workspace().session().project())
        .expect("project serializes")
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

fn apply_project(f: &Fixture, op: ProjectOp) {
    let mut svc = f.svc.borrow_mut();
    let session = svc.app_mut().workspace_mut().session_mut();
    let rev = session.revision();
    session.apply(op, rev).expect("fixture op applies");
}

fn preview_params(new_ppqn: u64, policy: &str) -> Value {
    json!({ "new_ppqn": new_ppqn, "policy": policy })
}

fn tmp_dir(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("synvelis-tim01prev-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

// ---------------------------------------------------------------------------
// preview_ppqn — report shape + read-only contract
// ---------------------------------------------------------------------------

/// `syn.project.preview_ppqn` returns the full dry-run report and
/// leaves the live project — content, PPQN, revision, engine
/// notifications — completely untouched.
#[test]
fn preview_ppqn_reports_without_mutating() {
    let mut f = fixture();
    let project_before = live_project_json(&f);
    let rev_before = live_revision(&f);
    let builds_before = engine_builds(&f);

    let res = ok(
        &mut f.rt,
        "syn.project.preview_ppqn",
        preview_params(480, "exact"),
    );

    assert_eq!(res["old_ppqn"], 960);
    assert_eq!(res["new_ppqn"], 480);
    assert_eq!(res["policy"], "exact");
    assert_eq!(res["rounding"], Value::Null);
    // Boot project: default tempo + signature events = 2 sites.
    assert_eq!(res["site_count"], 2);
    assert_eq!(res["inexact"], json!([]));
    assert_eq!(res["overflow_sites"], json!([]));
    assert_eq!(res["exact_ok"], true);
    assert_eq!(res["round_ok"], true);
    assert_eq!(res["applies"], true);
    assert_eq!(res["rejection"], Value::Null);
    assert!(res["effects"]["snap_grid"].is_string());
    assert!(res["effects"]["smf_export"].is_string());

    // Read-only: project content, revision, and engine notification
    // count are all unchanged.
    assert_eq!(live_project_json(&f), project_before);
    assert_eq!(live_revision(&f), rev_before);
    assert_eq!(live_ppqn(&f), 960);
    assert_eq!(engine_builds(&f), builds_before);
}

/// The preview is read-only by contract — the session gates that
/// reject `syn.project.set_ppqn` (transport playing, automation
/// record, open recording transaction) do not apply.
#[test]
fn preview_ppqn_ignores_session_gates() {
    let mut f = fixture();
    ok(&mut f.rt, "syn.transport.set", json!({ "playing": true }));
    {
        let mut svc = f.svc.borrow_mut();
        svc.app_mut()
            .workspace_mut()
            .session_mut()
            .control_mut()
            .set_record_enabled(true);
    }

    // `set_ppqn` is gated shut in this state…
    let err = fail(
        &mut f.rt,
        "syn.project.set_ppqn",
        json!({ "old_ppqn": 960, "new_ppqn": 480, "policy": "exact" }),
    );
    assert_eq!(err["code"], "invalid_params");

    // …while the preview still answers.
    let res = ok(
        &mut f.rt,
        "syn.project.preview_ppqn",
        preview_params(480, "exact"),
    );
    assert_eq!(res["applies"], true);
    assert_eq!(live_ppqn(&f), 960);
}

/// Inexact positions are reported with site identity and the
/// requested policy's feasibility — `exact` rejects where `round`
/// quantizes explicitly.
#[test]
fn preview_ppqn_reports_inexact_sites_and_policy_feasibility() {
    let mut f = fixture();
    // Seed an inexact musical position: a tempo event at tick 1 is
    // not representable on the 960 → 1000 grid.
    apply_project(
        &f,
        ProjectOp::AddTempoEvent {
            tick: 1,
            bpm_milli: 120_000,
        },
    );

    let res = ok(
        &mut f.rt,
        "syn.project.preview_ppqn",
        preview_params(1000, "exact"),
    );
    // default tempo(1) + default signature(1) + added tempo(1) = 3.
    assert_eq!(res["site_count"], 3);
    assert_eq!(res["exact_ok"], false);
    assert_eq!(res["round_ok"], true);
    assert_eq!(res["applies"], false);
    assert!(res["rejection"].is_string());
    let inexact = res["inexact"].as_array().unwrap();
    assert_eq!(inexact.len(), 1);
    assert_eq!(inexact[0]["kind"], "tempo_event");
    assert_eq!(inexact[0]["instance"], "1");
    assert_eq!(inexact[0]["part"], Value::Null);
    assert_eq!(inexact[0]["tick"], 1);
    assert_eq!(inexact[0]["projected"], Value::Null);

    let res = ok(
        &mut f.rt,
        "syn.project.preview_ppqn",
        json!({ "new_ppqn": 1000, "policy": "round", "rounding": "floor" }),
    );
    assert_eq!(res["policy"], "round");
    assert_eq!(res["rounding"], "floor");
    assert_eq!(res["applies"], true);
    // 1/960 beat → 1000/960 = 25/24 ticks — Floor projects to 1.
    assert_eq!(res["inexact"][0]["projected"], 1);

    assert_eq!(live_ppqn(&f), 960);
}

/// Wire contract mirrors `set_ppqn`: `new_ppqn`/`policy` are
/// required, `round` needs `rounding`, `exact` rejects it, and an
/// out-of-range `new_ppqn` is `invalid_params`.
#[test]
fn preview_ppqn_rejects_malformed_params() {
    let mut f = fixture();
    for (label, params) in [
        ("missing new_ppqn", json!({ "policy": "exact" })),
        ("missing policy", json!({ "new_ppqn": 480 })),
        (
            "unknown policy",
            json!({ "new_ppqn": 480, "policy": "bogus" }),
        ),
        (
            "round without rounding",
            json!({ "new_ppqn": 480, "policy": "round" }),
        ),
        (
            "exact with rounding",
            json!({ "new_ppqn": 480, "policy": "exact", "rounding": "floor" }),
        ),
        ("new_ppqn zero", json!({ "new_ppqn": 0, "policy": "exact" })),
        (
            "new_ppqn above max",
            json!({ "new_ppqn": 32_768, "policy": "exact" }),
        ),
        (
            "new_ppqn not a number",
            json!({ "new_ppqn": "480", "policy": "exact" }),
        ),
    ] {
        let err = fail(&mut f.rt, "syn.project.preview_ppqn", params);
        assert_eq!(err["code"], "invalid_params", "{label}");
    }
    assert_eq!(live_ppqn(&f), 960);
}

// ---------------------------------------------------------------------------
// syn.project.new — optional initial PPQN
// ---------------------------------------------------------------------------

/// `params.ppqn` selects the initial project PPQN; omitting it keeps
/// the 960 default, and an explicit 960 does not churn the revision.
#[test]
fn project_new_ppqn_defaults_and_explicit_selection() {
    let dir_default = tmp_dir("new-default");
    let dir_960 = tmp_dir("new-960");
    let dir_480 = tmp_dir("new-480");
    let mut f = fixture();

    let res_default = ok(
        &mut f.rt,
        "syn.project.new",
        json!({
            "path": dir_default.to_string_lossy(),
            "name": "default",
        }),
    );
    assert_eq!(res_default["ppqn"], 960);

    let res_960 = ok(
        &mut f.rt,
        "syn.project.new",
        json!({
            "path": dir_960.to_string_lossy(),
            "name": "explicit-960",
            "ppqn": 960,
        }),
    );
    assert_eq!(res_960["ppqn"], 960);
    // Explicit 960 is the identity — no revision churn vs. default.
    assert_eq!(res_960["revision"], res_default["revision"]);

    let res_480 = ok(
        &mut f.rt,
        "syn.project.new",
        json!({
            "path": dir_480.to_string_lossy(),
            "name": "explicit-480",
            "ppqn": 480,
        }),
    );
    assert_eq!(res_480["ppqn"], 480);
    assert_eq!(live_ppqn(&f), 480);

    // The selected PPQN persists through save → reload.
    ok(&mut f.rt, "syn.project.save", json!({}));
    ok(&mut f.rt, "syn.project.reload", json!({}));
    let info = ok(&mut f.rt, "syn.project.info", json!({}));
    assert_eq!(info["ppqn"], 480);

    let _ = fs::remove_dir_all(&dir_default);
    let _ = fs::remove_dir_all(&dir_960);
    let _ = fs::remove_dir_all(&dir_480);
}

/// Out-of-range `ppqn` is delegated to the project layer — the wire
/// only checks it is a `u64` fitting `u32`. A rejected create must
/// not leave a stray bundle directory behind.
#[test]
fn project_new_rejects_bad_ppqn() {
    // Wire-shape failures → invalid_params.
    for (label, ppqn) in [
        ("string", json!("480")),
        ("above u32", json!(4_294_967_296_u64)),
    ] {
        let mut f = fixture();
        let bundle = tmp_dir(&format!("bad-{label}")).join("bundle");
        let err = fail(
            &mut f.rt,
            "syn.project.new",
            json!({ "path": bundle.to_string_lossy(), "ppqn": ppqn }),
        );
        assert_eq!(err["code"], "invalid_params", "{label}");
        assert!(!bundle.exists(), "{label}: stray bundle dir left behind");
    }

    // Domain-range failures (0, 32768) are rejected by the project
    // layer's `SetProjectPpqn` validation → service_error — applied
    // before `DirectoryBundleStore::create`, so no stray dir either.
    for (label, ppqn) in [("zero", 0_u64), ("above max", 32_768_u64)] {
        let mut f = fixture();
        let bundle = tmp_dir(&format!("range-{label}")).join("bundle");
        let err = fail(
            &mut f.rt,
            "syn.project.new",
            json!({ "path": bundle.to_string_lossy(), "ppqn": ppqn }),
        );
        assert_eq!(err["code"], "service_error", "{label}");
        assert!(
            err["message"].as_str().unwrap().contains("ppqn"),
            "{label}: {err}"
        );
        assert!(!bundle.exists(), "{label}: stray bundle dir left behind");
        // The rejected create must not leave a usable project behind.
        assert_eq!(live_ppqn(&f), 960);
    }
}
