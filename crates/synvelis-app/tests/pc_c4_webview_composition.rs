//! PC-C4 webview composition acceptance (synvelis#101): the webview
//! product bin's composition path — [`compose_product_routing`], the
//! shared helper `synvelis.rs` calls — gates on the boot project dir
//! (`SYNVELIS_PROJECT_DIR` resolved at the process boundary and passed
//! as `Option<PathBuf>`), attaches the session routing engine with an
//! empty plugin registry, and the `syn.control.*` op surface answers
//! real evidence instead of the engine-stub `engine:false`/null
//! payload. Project recreate detaches the engine with the session —
//! re-attach for the new media root stays the deferred
//! project-switch wiring slice.
//!
//! Fixture mirrors `pc_c4_drain_composition.rs`'s service harness
//! (`AppService` behind a shared handle, `UiRuntime::dispatch_op`)
//! but composes through [`compose_product_routing`] — the exact entry
//! the webview bin calls — never `compose_routing_engine` or a
//! hand-built `PluginHostProcessors`, and registers no plugin
//! factories (the product ships none; real backends are
//! ruviform#253, `NotRun`). The `SYNVELIS_PROJECT_DIR` env read itself
//! stays in the bin; the gate contract under test is the helper's
//! `Option<PathBuf>` semantics.

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};

use ruviform_ui::ipc::{ErrorBody, Event as UiEvent};
use ruviform_ui::{
    BackendReport, CallOutcome, DataRegistry, OpInfo, Service, ServiceContext, UiRuntime,
};

use synvelis_app::{AppService, OPS, SynvelisApp, compose_product_routing};
use synvelis_project::ProjectOp;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-pcc4wc-{}-{}-{}",
        name,
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// `AppService` behind a shared handle — the fixture reaches
/// `app_mut()`/`app()` through the same object `UiRuntime`
/// dispatches into.
struct SharedService(Rc<RefCell<AppService>>);

impl Service for SharedService {
    fn ops(&self) -> &'static [OpInfo] {
        OPS
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
    svc: Rc<RefCell<AppService>>,
    rt: UiRuntime,
}

/// Fresh app + service + runtime — the untitled boot session the
/// webview bin composes into.
fn fixture() -> Fixture {
    let app = SynvelisApp::new(BackendReport::available("webview", "test-0.1".to_string()));
    let svc = Rc::new(RefCell::new(AppService::new(app)));
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(SharedService(Rc::clone(&svc)))
        .expect("register AppService");
    Fixture { svc, rt }
}

/// Dispatches `op` and extracts the single `result` event payload.
fn call(rt: &mut UiRuntime, op: &str, params: Value) -> (bool, Option<Value>, Option<ErrorBody>) {
    let events = rt.dispatch_op("test-1", op, params);
    let result = events.iter().find_map(|e| match e {
        UiEvent::Result {
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

/// Commits one session revision so the attached engine publishes —
/// the same notification path every product op drives.
fn bump_revision(fx: &Fixture, name: &str) {
    let rev = fx.svc.borrow().app().workspace().session().revision();
    fx.svc
        .borrow_mut()
        .app_mut()
        .workspace_mut()
        .session_mut()
        .apply(ProjectOp::SetProjectName { name: name.into() }, rev)
        .unwrap();
}

/// The gate-closed contract: no boot project dir (`None`, the
/// `SYNVELIS_PROJECT_DIR`-unset path the bin passes) leaves the
/// engine stub — `syn.control.status` reports `engine:false` and
/// null generation/surface evidence, and a `ui_pump` drain pass
/// records nothing.
#[test]
fn project_dir_gate_keeps_engine_stub() {
    let mut fx = fixture();
    compose_product_routing(fx.svc.borrow_mut().app_mut(), None);

    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    assert_eq!(status["engine"], false, "gate closed: engine stub");
    assert!(status["generation"].is_null());
    assert!(status["surface"].is_null());
    assert_eq!(status["drain"]["attached"], false);

    // Even after a session revision the stub stays honest — no
    // engine means no publish, no surface, no drain evidence.
    bump_revision(&fx, "pcwc-gate");
    fx.svc.borrow_mut().app_mut().ui_pump();
    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    assert_eq!(status["engine"], false);
    assert!(status["generation"].is_null());
    assert_eq!(status["drain"]["attached"], false);
    assert_eq!(fx.svc.borrow().app().last_control_drain().drained, 0);
}

/// The gate-open contract: a boot project dir attaches the engine at
/// composition time (`engine:true` before any publish), the first
/// session revision publishes generation 1 and attaches the control
/// surface, and `ui_pump` drives the registered report drain — the
/// `syn.control.*` surface answers real typed evidence, never the
/// stub payload.
#[test]
fn project_dir_attaches_engine_and_control_surface() {
    let root = scratch("attach");
    let mut fx = fixture();
    compose_product_routing(fx.svc.borrow_mut().app_mut(), Some(root.clone()));

    // Attached at composition — `engine:true` with no surface yet
    // (no revision has run, so nothing published).
    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    assert_eq!(status["engine"], true, "engine attached at compose");
    assert!(status["generation"].is_null(), "no publish yet");
    assert!(status["surface"].is_null());

    // First revision → prepare + publish → control surface attach.
    bump_revision(&fx, "pcwc-attach");
    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    assert_eq!(status["engine"], true);
    assert_eq!(status["generation"], 1, "first published generation");
    assert!(
        status["surface"].is_object(),
        "control surface report attached: {status}"
    );
    // Empty registry: no plugin-backed processor → no frame base,
    // honestly null rather than fabricated.
    assert!(status["frame_base"].is_null());
    assert!(status["active_generation"].is_null());

    // The composition-registered drain is live: one `ui_pump` runs
    // one pass — gated fail-closed on the unobserved active
    // generation, zero host calls (no plugin endpoints exist).
    fx.svc.borrow_mut().app_mut().ui_pump();
    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    let drain = &status["drain"];
    assert_eq!(drain["attached"], true);
    assert_eq!(drain["generation"], 1);
    assert_eq!(
        drain["inactive_gated"], true,
        "unobserved active generation gates"
    );
    assert_eq!(drain["drained"], 0, "gated drain never calls the host");
    assert_eq!(drain["events"], 0);
}

/// Session recreate is the composition boundary: `project.new`
/// destroys the session, so the attached engine, its control surface,
/// and the registered drain all reset with it — `syn.control.status`
/// can never pair a dead session's evidence with the new project.
/// Re-attach for the new media root is the deferred engine-wiring
/// slice, not this composition's job.
#[test]
fn project_recreate_detaches_engine() {
    let root = scratch("recreate");
    let mut fx = fixture();
    compose_product_routing(fx.svc.borrow_mut().app_mut(), Some(root));
    bump_revision(&fx, "pcwc-before");
    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    assert_eq!(status["engine"], true);
    assert_eq!(status["generation"], 1);

    let bundle = scratch("next").to_string_lossy().into_owned();
    fx.svc
        .borrow_mut()
        .app_mut()
        .project_new(&bundle, Some("next"))
        .expect("project.new");

    let status = ok(&mut fx.rt, "syn.control.status", json!({}));
    assert_eq!(status["engine"], false, "recreate detached the engine");
    assert!(status["generation"].is_null());
    assert!(status["surface"].is_null());
    assert_eq!(status["drain"]["attached"], false);
    let drain = fx.svc.borrow().app().last_control_drain();
    assert!(!drain.attached);
    assert_eq!(drain.drained, 0);
}
