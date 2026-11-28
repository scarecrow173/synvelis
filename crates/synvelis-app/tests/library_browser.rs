//! ALIB-05/ALIB-UX-H acceptance tests (synvelis#89) — the
//! `syn.library.*` op surface over the headless Asset Library
//! browser/import slice.
//!
//! - `syn.library.providers`/`visual`/`view`/`action` register on the
//!   service and project provider catalog, typed view state, and the
//!   deterministic `ViewSpec` verbatim.
//! - Browse pagination, capability-aware search, stat provenance,
//!   pin evidence, preview plan/fetch, drag ghost, and verified
//!   import all stay typed — unsupported/offline/missing/stale are
//!   never folded into empty results or fake successes.
//! - Import commits only through `import_verified` +
//!   `ProjectController` at the intent's expected revision; replay of
//!   the same `ImportId` is idempotent (`reused`).
//!
//! Providers/import ports are in-memory fakes and temp-dir stores —
//! no network, credentials, GPUI, or real data (synvelis#91/#114/#86
//! own those gates).

mod common;

use std::sync::Arc;

use common::library::*;
use ruviform_ui::ipc::{Event, JobState};
use ruviform_ui::{BackendReport, DataRegistry, UiRuntime};
use serde_json::{Value, json};

use synvelis_app::{AppService, SynvelisApp};
use synvelis_library::Availability;
use synvelis_workspace::ViewType;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Selects the fixture provider, then refreshes browse.
fn select_and_browse(app: &mut SynvelisApp, view: &str) {
    action(
        app,
        view,
        json!({ "action": "provider.select", "provider": PROVIDER_TAG }),
    );
    action(app, view, json!({ "action": "browse.refresh" }));
}

fn fault_reason(v: &Value) -> Option<String> {
    v["state"]["fault"]["reason"].as_str().map(str::to_string)
}

// ---------------------------------------------------------------------------
// service surface
// ---------------------------------------------------------------------------

#[test]
fn ops_registered_and_dispatch_through_runtime() {
    let mut app = app();
    let view = open_view(&mut app);
    let service = AppService::new(app);
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(service).expect("register AppService");

    let caps = rt
        .dispatch_op("t", "sys.capabilities", json!({}))
        .into_iter()
        .find_map(|e| match e {
            Event::Result {
                ok: true,
                data: Some(d),
                ..
            } => Some(d),
            _ => None,
        })
        .expect("capabilities result");
    let caps_text = serde_json::to_string(&caps).expect("caps serializes");
    for name in [
        "syn.library.providers",
        "syn.library.visual",
        "syn.library.view",
        "syn.library.action",
        "syn.library.jobs",
    ] {
        assert!(caps_text.contains(name), "capabilities missing {name}");
    }

    // Same dispatch path the host/driver takes.
    let providers = rt
        .dispatch_op("t", "syn.library.providers", json!({}))
        .into_iter()
        .find_map(|e| match e {
            Event::Result {
                ok: true,
                data: Some(d),
                ..
            } => Some(d),
            _ => None,
        })
        .expect("providers result");
    assert_eq!(providers["providers"][0]["id"], PROVIDER_TAG);

    let projection = rt
        .dispatch_op("t", "syn.library.view", json!({ "view_id": view }))
        .into_iter()
        .find_map(|e| match e {
            Event::Result {
                ok: true,
                data: Some(d),
                ..
            } => Some(d),
            _ => None,
        })
        .expect("view result");
    assert_eq!(projection["view_id"], view);
    assert!(projection["spec"]["root"].is_object());
}

#[test]
fn job_actions_run_through_runtime_job_lane() {
    // `syn.library.action`'s job actions return `CallOutcome::Job` —
    // the runtime ack is `{job, label}` (not a plain result), the
    // worker's lifecycle events arrive on `poll`, and the app-owned
    // ledger records the terminal a follow-up op read reconciles.
    let (mut app, dir) = app_with_import("rtjobs");
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    let ledger = shared_job_ledger(&app);
    let service = AppService::new(app);
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(service).expect("register AppService");

    let ack = rt
        .dispatch_op(
            "t",
            "syn.library.action",
            json!({
                "view_id": view,
                "action": "import.submit",
                "import_id": 8,
                "locator": "loc-a",
                "allow_full_object": true,
            }),
        )
        .into_iter()
        .find_map(|e| match e {
            Event::Result {
                ok: true,
                data: Some(d),
                ..
            } => Some(d),
            _ => None,
        })
        .expect("job ack");
    assert_eq!(ack["label"], "syn.library.import.submit");
    let job_id = ack["job"].as_u64().expect("runtime job id");

    // The job parks at commit-pending — the ledger reconcile owns the
    // live commit, so `Finished` carries the parked outcome.
    let saw_finish = bounded_wait(
        "runtime import job Finished event",
        || {
            let mut saw = false;
            for e in rt.poll(Some(JOB_WAIT_INTERVAL)) {
                if let Event::Job {
                    job,
                    state: JobState::Finished,
                    ..
                } = e
                    && job == job_id
                {
                    saw = true;
                }
            }
            saw.then_some(true)
        },
        || {
            format!(
                "runtime_job_id={job_id}; {}",
                raw_job_timeout_diagnostic(&ledger, &import_key(8), "runtime Finished event")
            )
        },
    );
    assert!(saw_finish, "import job never produced a Finished event");

    // The same ledger the direct spawn writes — `syn.library.jobs`
    // read through the runtime reconciles the parked commit.
    let jobs = rt
        .dispatch_op("t", "syn.library.jobs", json!({}))
        .into_iter()
        .find_map(|e| match e {
            Event::Result {
                ok: true,
                data: Some(d),
                ..
            } => Some(d),
            _ => None,
        })
        .expect("jobs result");
    let entry = jobs["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|j| j["job_key"].as_str() == Some(import_key(8).as_str()))
        .cloned()
        .expect("ledger entry");
    assert_eq!(entry["terminal"]["kind"], "committed-live");

    let projection = rt
        .dispatch_op("t", "syn.library.view", json!({ "view_id": view }))
        .into_iter()
        .find_map(|e| match e {
            Event::Result {
                ok: true,
                data: Some(d),
                ..
            } => Some(d),
            _ => None,
        })
        .expect("view result");
    assert_eq!(projection["state"]["import"]["state"], "committed-live");

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// view lifecycle + projection
// ---------------------------------------------------------------------------

#[test]
fn open_view_projects_providers_state_and_spec() {
    let mut app = app();

    // Provider catalog is reachable before any view exists.
    let catalog = providers(&app);
    assert_eq!(catalog["providers"].as_array().unwrap().len(), 1);
    let p = &catalog["providers"][0];
    assert_eq!(p["id"], PROVIDER_TAG);
    assert_eq!(p["display_name"], "fake");
    // Capabilities serialize as the typed ProviderCapabilities object —
    // `Capability` entries keep supported/unsupported{reason}; `pin`
    // is a `PinCapability` tag (immutable-object/conditional-strong/...);
    // `access` is the access-mode string.
    let caps = &p["capabilities"];
    for want in ["browse", "search", "preview"] {
        assert_eq!(
            caps[want]["state"], "supported",
            "missing capability {want}"
        );
    }
    assert!(
        caps["pin"]["state"].is_string() && caps["pin"]["state"] != "unsupported",
        "pin capability must be a supported PinCapability tag"
    );
    assert!(caps["access"].is_string(), "access mode must be typed");

    let view = open_view(&mut app);
    assert!(view.starts_with("view-"));

    let v = proj(&mut app, &view);
    assert_eq!(v["view_id"], view.as_str());
    assert_eq!(v["providers"].as_array().unwrap().len(), 1);
    let state = &v["state"];
    assert_eq!(state["provider"], Value::Null);
    assert_eq!(state["entries"].as_array().unwrap().len(), 0);
    assert_eq!(state["coverage"], Value::Null);
    assert_eq!(state["pin"]["state"], "idle");
    assert_eq!(state["preview"]["state"], "idle");
    assert_eq!(state["import"]["state"], "idle");
    assert_eq!(state["drag"]["state"], "idle");

    // The ViewSpec is deterministic — identical projections serialize
    // identically (stable component keys, pure function of state).
    let spec1 = serde_json::to_string(&v["spec"]).unwrap();
    let v2 = proj(&mut app, &view);
    let spec2 = serde_json::to_string(&v2["spec"]).unwrap();
    assert_eq!(spec1, spec2);
    assert!(spec1.contains("library.providers"));
    assert!(spec1.contains("library.browse"));
}

#[test]
fn view_rejects_unregistered_and_wrong_kind() {
    let mut app = app();
    assert!(
        app.op_library_view(&json!({ "view_id": "view-999" }))
            .is_err()
    );
    assert!(app.op_library_view(&json!({ "view_id": "main" })).is_err());

    // A registered non-library view is refused by kind.
    let inst = ruviform_ui::dock::ViewInstanceId::new(42).unwrap();
    app.workspace_mut()
        .adopt_view(inst, ViewType::VcsHistory, "history", None)
        .unwrap();
    let err = app
        .op_library_view(&json!({ "view_id": "view-42" }))
        .unwrap_err();
    assert_eq!(err.code, "invalid_params");
}

#[test]
fn close_removes_view_and_model() {
    let mut app = app();
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    assert_eq!(
        proj(&mut app, &view)["state"]["entries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    app.op_library_visual(&json!({ "action": "close", "view_id": view }))
        .expect("close");
    assert!(app.op_library_view(&json!({ "view_id": view })).is_err());
    // The model is gone with the view — a fresh open starts clean.
    let view2 = open_view(&mut app);
    let v = proj(&mut app, &view2);
    assert_eq!(v["state"]["provider"], Value::Null);
    assert_eq!(v["state"]["entries"].as_array().unwrap().len(), 0);
}

#[test]
fn unbound_backend_is_typed_not_fabricated() {
    // No set_library_backend — providers honestly empty; view/action
    // ops report `unavailable` rather than inventing a catalog.
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let catalog = providers(&app);
    assert_eq!(catalog["providers"].as_array().unwrap().len(), 0);

    let view = open_view(&mut app);
    let err = app
        .op_library_view(&json!({ "view_id": view }))
        .unwrap_err();
    assert_eq!(err.code, "unavailable");
    let err = app
        .op_library_action(&json!({
            "view_id": view,
            "action": "browse.refresh",
        }))
        .unwrap_err();
    assert_eq!(err.code, "unavailable");
}

// ---------------------------------------------------------------------------
// browse / search / stat / pin / preview / drag
// ---------------------------------------------------------------------------

#[test]
fn browse_pages_and_typed_states() {
    let provider = FakeProvider::new(1, true, true, true)
        .with_page(page(
            vec![entry("a.wav", "loc-a", synvelis_library::EntryKind::Asset)],
            synvelis_library::Coverage::Partial,
            Some("1"),
        ))
        .with_page(page(
            vec![entry("b.wav", "loc-b", synvelis_library::EntryKind::Asset)],
            synvelis_library::Coverage::Complete,
            None,
        ));
    let mut app = app_with(provider);
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);

    let v = proj(&mut app, &view);
    let state = &v["state"];
    assert_eq!(state["provider"], PROVIDER_TAG);
    assert_eq!(state["coverage"], "partial");
    assert_eq!(state["entries"].as_array().unwrap().len(), 1);
    assert_eq!(state["entries"][0]["locator"], "loc-a");
    assert_eq!(state["entries"][0]["display_name"], "a.wav");
    assert!(state["next_cursor"].is_string());

    action(&mut app, &view, json!({ "action": "browse.more" }));
    let v = proj(&mut app, &view);
    let state = &v["state"];
    assert_eq!(state["entries"].as_array().unwrap().len(), 2);
    assert_eq!(state["coverage"], "complete");
    assert_eq!(state["next_cursor"], Value::Null);
}

#[test]
fn search_stat_pin_and_preview_flow() {
    let provider = FakeProvider::new(1, true, true, true)
        .with_search_page(page(
            vec![entry(
                "hit.wav",
                "loc-hit",
                synvelis_library::EntryKind::Asset,
            )],
            synvelis_library::Coverage::Complete,
            None,
        ))
        .with_stat("loc-a", Availability::Available);
    let mut app = app_with(provider);
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);

    // Provider-side search replaces the listing — not an empty result.
    action(
        &mut app,
        &view,
        json!({ "action": "search.set", "text": "hit" }),
    );
    let v = proj(&mut app, &view);
    let state = &v["state"];
    assert_eq!(state["search_text"], "hit");
    assert_eq!(state["entries"].as_array().unwrap().len(), 1);
    assert_eq!(state["entries"][0]["locator"], "loc-hit");

    action(&mut app, &view, json!({ "action": "search.clear" }));
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["search_text"], Value::Null);

    // Select stats the entry — availability is the typed wire value.
    action(
        &mut app,
        &view,
        json!({ "action": "entry.select", "locator": "loc-a" }),
    );
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["selected_locator"], "loc-a");
    assert_eq!(v["state"]["stat"]["availability"]["state"], "available");

    // Pin produces typed evidence — never a project write.
    let rev_before = app.workspace().session().revision();
    action(
        &mut app,
        &view,
        json!({ "action": "entry.pin", "locator": "loc-a" }),
    );
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["pin"]["state"], "pinned");
    assert_eq!(v["state"]["pin"]["proof"], "immutable-object");
    assert_eq!(app.workspace().session().revision(), rev_before);

    // Preview plan → the stat reports unknown size, so the plan is a
    // whole-object fetch behind the consent gate. Fetch without
    // consent stays PlanReady; with `allow_full_object` it reads the
    // bounded stream → Ready. Preview success is NOT a verified
    // import — the import surface stays idle and the project
    // revision never moves.
    action(
        &mut app,
        &view,
        json!({ "action": "preview.plan", "locator": "loc-a" }),
    );
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["preview"]["state"], "plan-ready");
    assert_eq!(v["state"]["preview"]["plan"], "consent-required");
    action(
        &mut app,
        &view,
        json!({ "action": "preview.fetch", "locator": "loc-a" }),
    );
    wait_job(&mut app, &preview_key(&view));
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["preview"]["state"], "plan-ready");
    action(
        &mut app,
        &view,
        json!({
            "action": "preview.fetch",
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    wait_job(&mut app, &preview_key(&view));
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["preview"]["state"], "ready");
    assert_eq!(v["state"]["import"]["state"], "idle");
    assert_eq!(app.workspace().session().revision(), rev_before);
}

#[test]
fn unsupported_capabilities_stay_typed() {
    // No search, no pin, no preview on this provider.
    let mut app = app_with(FakeProvider::new(1, false, false, false));
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);

    action(
        &mut app,
        &view,
        json!({ "action": "search.set", "text": "anything" }),
    );
    let v = proj(&mut app, &view);
    // Typed unsupported fault — never an empty listing.
    assert_eq!(fault_reason(&v).as_deref(), Some("unsupported-capability"));
    assert_eq!(v["state"]["search_text"], Value::Null);

    action(
        &mut app,
        &view,
        json!({ "action": "entry.pin", "locator": "loc-a" }),
    );
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["pin"]["state"], "unsupported");
    assert_eq!(v["state"]["pin"]["reason"], "pin-capability-unsupported");

    action(
        &mut app,
        &view,
        json!({ "action": "preview.plan", "locator": "loc-a" }),
    );
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["preview"]["state"], "failed");
    assert_eq!(v["state"]["preview"]["reason"], "unsupported-capability");
}

#[test]
fn provider_switch_invalidates_bound_state() {
    let mut app = app();
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    assert_eq!(
        proj(&mut app, &view)["state"]["entries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // Switching to an unregistered provider is a typed fault and
    // leaves the current provider untouched.
    action(
        &mut app,
        &view,
        json!({ "action": "provider.select", "provider": "provider-00000000000000000000000000000009" }),
    );
    let v = proj(&mut app, &view);
    assert_eq!(fault_reason(&v).as_deref(), Some("unknown-provider"));
    assert_eq!(v["state"]["provider"], PROVIDER_TAG);
}

#[test]
fn drag_ghost_never_mutates_project() {
    let mut app = app();
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    let rev_before = app.workspace().session().revision();

    action(
        &mut app,
        &view,
        json!({ "action": "drag.begin", "locator": "loc-a" }),
    );
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["drag"]["state"], "ghost");
    assert_eq!(v["state"]["drag"]["locator"], "loc-a");
    assert_eq!(v["state"]["import"]["state"], "idle");
    assert_eq!(app.workspace().session().revision(), rev_before);

    action(&mut app, &view, json!({ "action": "drag.drop" }));
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["drag"]["state"], "idle");
    assert_eq!(app.workspace().session().revision(), rev_before);
}

#[test]
fn malformed_action_params_are_invalid_params() {
    let mut app = app();
    let view = open_view(&mut app);
    // Unknown action tag → invalid_params at the op boundary.
    let err = app
        .op_library_action(&json!({
            "view_id": view,
            "action": "browse.delete-everything",
        }))
        .unwrap_err();
    assert_eq!(err.code, "invalid_params");
    // Missing action tag entirely.
    let err = app
        .op_library_action(&json!({ "view_id": view }))
        .unwrap_err();
    assert_eq!(err.code, "invalid_params");
}

// ---------------------------------------------------------------------------
// verified import
// ---------------------------------------------------------------------------

#[test]
fn import_commits_live_and_replays_idempotently() {
    let (mut app, dir) = app_with_import("commit");
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    let rev_before = app.workspace().session().revision();

    action(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 7,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    wait_job(&mut app, &import_key(7));
    let v = proj(&mut app, &view);
    let import = &v["state"]["import"];
    assert_eq!(import["state"], "committed-live", "state: {import}");
    assert_eq!(
        import["import_id"],
        "import-00000000000000000000000000000007"
    );
    assert!(import["asset_id"].is_string());
    assert!(import["source_id"].is_string());
    assert!(import["content_digest"].is_string());
    assert_eq!(import["byte_len"], ASSET_BYTES.len() as u64);
    assert_eq!(import["reused"], false);
    // CommittedLive advanced the live revision — the only sanctioned
    // project mutation on this surface.
    assert!(app.workspace().session().revision() > rev_before);

    // Same ImportId + same intent replays idempotently — no second
    // commit, `reused` marks the terminal.
    action(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 7,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    wait_job(&mut app, &import_key(7));
    let v = proj(&mut app, &view);
    let import = &v["state"]["import"];
    assert_eq!(import["state"], "committed-live");
    assert_eq!(import["reused"], true);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn import_without_backend_or_fetch_is_typed() {
    // No import services bound → Malformed fault, project untouched.
    let mut app = app();
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    let rev_before = app.workspace().session().revision();
    action(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 1,
            "locator": "loc-a",
        }),
    );
    let v = proj(&mut app, &view);
    assert_eq!(fault_reason(&v).as_deref(), Some("malformed"));
    assert_eq!(app.workspace().session().revision(), rev_before);

    // Import services bound but the provider cannot open the pin →
    // typed Failed, never a fabricated CommittedLive.
    let (mut app2, dir) = app_with_import("nofetch");
    // Rebind a provider whose open_pinned is unsupported.
    app2.set_library_backend(synvelis_workspace::LibraryBackend {
        registry: common::library::registry_with(
            FakeProvider::new(1, true, true, true).with_pinned_bytes(false),
        ),
        principal: synvelis_library::PrincipalId::new("test").unwrap(),
    });
    let view2 = open_view(&mut app2);
    select_and_browse(&mut app2, &view2);
    action(
        &mut app2,
        &view2,
        json!({
            "action": "import.submit",
            "import_id": 3,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    wait_job(&mut app2, &import_key(3));
    let v2 = proj(&mut app2, &view2);
    // The provider's typed open_pinned failure surfaces as a fault —
    // the import surface never claims a commit it did not earn.
    assert_eq!(fault_reason(&v2).as_deref(), Some("unsupported-capability"));
    assert_ne!(v2["state"]["import"]["state"], "committed-live");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn import_stale_revision_rebase_retry_recovers() {
    let (mut app, dir) = app_with_import("stale");
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);

    // First import commits — the live revision advances under any
    // intent pinned to the old revision.
    let rev0 = app.workspace().session().revision().0;
    let builds0 = app.workspace().session().engine().graph_builds();
    action(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 10,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    wait_job(&mut app, &import_key(10));
    assert_eq!(
        proj(&mut app, &view)["state"]["import"]["state"],
        "committed-live"
    );
    let rev1 = app.workspace().session().revision().0;
    assert!(rev1 > rev0);

    // Second import pinned to the pre-commit revision → the CAS
    // rejects it as StaleRevision carrying the manifest's true
    // expected/actual, and the engine is NOT notified (no commit).
    action(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 11,
            "locator": "loc-a",
            "expected_project_revision": rev0,
            "allow_full_object": true,
        }),
    );
    wait_job(&mut app, &import_key(11));
    let v = proj(&mut app, &view);
    let import = &v["state"]["import"];
    assert_eq!(import["state"], "stale-revision", "state: {import}");
    assert_eq!(import["expected"], rev0);
    assert_eq!(import["actual"], rev1);
    assert_eq!(
        app.workspace().session().engine().graph_builds(),
        builds0 + 1
    );

    // Rebase against a different import id is refused — the pending
    // intent is the only rebase target this surface drives.
    action(
        &mut app,
        &view,
        json!({
            "action": "import.rebase",
            "import_id": 12,
            "expected_project_revision": rev1,
        }),
    );
    assert_eq!(
        fault_reason(&proj(&mut app, &view)).as_deref(),
        Some("malformed")
    );

    // Rebase the pending intent to live, then retry → commits, and
    // the session pipeline observes exactly one more revision advance
    // (engine rebuild notification + control seal).
    action(
        &mut app,
        &view,
        json!({
            "action": "import.rebase",
            "import_id": 11,
            "expected_project_revision": rev1,
        }),
    );
    action(&mut app, &view, json!({ "action": "import.retry" }));
    wait_job(&mut app, &import_key(11));
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["import"]["state"], "committed-live");
    assert_eq!(app.workspace().session().revision().0, rev1 + 1);
    assert_eq!(
        app.workspace().session().engine().graph_builds(),
        builds0 + 2
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn provider_switch_scopes_import_surface() {
    const PROVIDER2_TAG: &str = "provider-00000000000000000000000000000002";
    let (mut app, dir) = app_with_import_registry(
        "pswitch",
        vec![
            FakeProvider::new(1, true, true, true),
            FakeProvider::new(2, true, true, true),
        ],
    );
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);

    // A stale pending intent dies with the provider switch — its pin
    // evidence belongs to provider 1 and retry under provider 2 could
    // only fail typed.
    let rev0 = app.workspace().session().revision().0;
    action(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 20,
            "locator": "loc-a",
            "expected_project_revision": rev0 + 5,
            "allow_full_object": true,
        }),
    );
    wait_job(&mut app, &import_key(20));
    assert_eq!(
        proj(&mut app, &view)["state"]["import"]["state"],
        "stale-revision"
    );
    action(
        &mut app,
        &view,
        json!({ "action": "provider.select", "provider": PROVIDER2_TAG }),
    );
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["provider"], PROVIDER2_TAG);
    assert_eq!(v["state"]["import"]["state"], "idle");
    // No pending intent survives — retry is a typed fault, not a
    // cross-provider replay.
    action(&mut app, &view, json!({ "action": "import.retry" }));
    assert_eq!(
        fault_reason(&proj(&mut app, &view)).as_deref(),
        Some("malformed")
    );

    // A committed receipt is project truth — it survives the switch.
    action(
        &mut app,
        &view,
        json!({ "action": "provider.select", "provider": PROVIDER_TAG }),
    );
    select_and_browse(&mut app, &view);
    action(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 21,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    wait_job(&mut app, &import_key(21));
    assert_eq!(
        proj(&mut app, &view)["state"]["import"]["state"],
        "committed-live"
    );
    action(
        &mut app,
        &view,
        json!({ "action": "provider.select", "provider": PROVIDER2_TAG }),
    );
    assert_eq!(
        proj(&mut app, &view)["state"]["import"]["state"],
        "committed-live"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn library_backend_survives_project_new() {
    // project.new replaces the whole WorkspaceManager — the bound
    // provider backend must be re-applied or every syn.library.* op
    // silently degrades to `unavailable`.
    let mut app = app();
    let bundle = std::env::temp_dir().join(format!(
        "sv89-libapp-projnew-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.project_new(&bundle.to_string_lossy(), None)
        .expect("project.new");

    assert_eq!(providers(&app)["providers"].as_array().unwrap().len(), 1);
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    assert_eq!(
        proj(&mut app, &view)["state"]["entries"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let _ = std::fs::remove_dir_all(&bundle);
}

// ---------------------------------------------------------------------------
// job lane (ALIB-05): ledger, cancel, dedup, recovery
// ---------------------------------------------------------------------------

#[test]
fn job_ack_and_ledger_wire_shape() {
    // A job action acks `queued` + its ledger key; `syn.library.jobs`
    // projects the entry the runtime events correlate on — the
    // cross-view truth a non-spawning view reads.
    let (mut app, dir) = app_with_import("jobwire");
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    let view2 = open_view(&mut app);

    let ack = action_json(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 31,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    assert_eq!(ack["ok"], true);
    assert_eq!(ack["queued"], true);
    assert_eq!(ack["job_key"], import_key(31));

    let j = wait_job(&mut app, &import_key(31));
    assert_eq!(j["kind"], "import-submit");
    assert_eq!(j["state"], "done");
    assert_eq!(j["job_key"], import_key(31));
    assert_eq!(j["import_id"], "import-0000000000000000000000000000001f");
    assert!(j["epoch"].is_number() && j["cancel_requested"].is_boolean());
    assert_eq!(j["terminal"]["kind"], "committed-live");
    assert!(j["view"].is_string());

    // The ledger is app-owned — a second view reads the same entry
    // without materializing the terminal onto its own surfaces.
    assert_eq!(
        job(&mut app, &import_key(31)).unwrap()["terminal"]["kind"],
        "committed-live"
    );
    let v2 = proj(&mut app, &view2);
    assert_eq!(v2["state"]["import"]["state"], "idle");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn import_submit_dedups_in_flight() {
    // A gated publisher parks the job mid-publish; resubmitting the
    // same ImportId answers `in-flight` — one ImportId is never two
    // jobs.
    let gate = Gate::new();
    let (mut app, dir) = app_with_import_gate(
        "dedup",
        gate.clone(),
        vec![FakeProvider::new(1, true, true, true)],
    );
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);

    let ack = action_json(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 32,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    assert_eq!(ack["queued"], true);
    wait_stage(&mut app, &import_key(32), "materializing");

    let dup = action_json(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 32,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    assert_eq!(dup["status"], "in-flight");
    assert_eq!(dup["job_key"], import_key(32));

    gate.open();
    let j = wait_job(&mut app, &import_key(32));
    assert_eq!(j["terminal"]["kind"], "committed-live");
    assert_eq!(
        proj(&mut app, &view)["state"]["import"]["state"],
        "committed-live"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn import_cancel_mid_flight_then_retry_recovers() {
    // The gate parks the job inside `publish` — `import.cancel` raises
    // the job's cooperative token, the publisher's `check_interrupts`
    // honors it, and the durable manifest (verified) makes the cancel
    // resumable: `import.retry` converges once the gate opens.
    let gate = Gate::new();
    let (mut app, dir) = app_with_import_gate(
        "cancel",
        gate.clone(),
        vec![FakeProvider::new(1, true, true, true)],
    );
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    let rev0 = app.workspace().session().revision();

    action_json(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 33,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    wait_stage(&mut app, &import_key(33), "materializing");

    let ack = action_json(
        &mut app,
        &view,
        json!({ "action": "import.cancel", "import_id": 33 }),
    );
    assert_eq!(ack["status"], "cancel-requested");
    assert_eq!(ack["job_key"], import_key(33));

    let j = wait_job(&mut app, &import_key(33));
    assert_eq!(j["cancel_requested"], true);
    assert_eq!(j["terminal"]["kind"], "cancelled");
    assert_eq!(j["terminal"]["resumable"], true);
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["import"]["state"], "cancelled");
    // Cancel is not a commit — the live revision never moved.
    assert_eq!(app.workspace().session().revision(), rev0);

    // The verified manifest survives — the gate opens and the retry
    // publishes + commits without re-streaming.
    gate.open();
    action(&mut app, &view, json!({ "action": "import.retry" }));
    let j = wait_job(&mut app, &import_key(33));
    assert_eq!(j["kind"], "import-retry");
    assert_eq!(j["terminal"]["kind"], "committed-live");
    assert_eq!(
        proj(&mut app, &view)["state"]["import"]["state"],
        "committed-live"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn import_cancel_is_cross_view() {
    // `import.cancel` resolves the job by `import_id` (or `job_key`)
    // against the app-owned ledger — a non-spawning view cancels the
    // same job and the spawning view still reads the terminal.
    let gate = Gate::new();
    let (mut app, dir) = app_with_import_gate(
        "xcancel",
        gate.clone(),
        vec![FakeProvider::new(1, true, true, true)],
    );
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    let view2 = open_view(&mut app);

    action_json(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 34,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    wait_stage(&mut app, &import_key(34), "materializing");

    let ack = action_json(
        &mut app,
        &view2,
        json!({ "action": "import.cancel", "job_key": import_key(34) }),
    );
    assert_eq!(ack["status"], "cancel-requested");

    let j = wait_job(&mut app, &import_key(34));
    assert_eq!(j["cancel_requested"], true);
    assert_eq!(j["terminal"]["kind"], "cancelled");
    assert_eq!(
        proj(&mut app, &view)["state"]["import"]["state"],
        "cancelled"
    );

    // A terminal job answers `terminal`, not a second request.
    let ack = action_json(
        &mut app,
        &view2,
        json!({ "action": "import.cancel", "job_key": import_key(34) }),
    );
    assert_eq!(ack["status"], "terminal");
    assert_eq!(ack["terminal"]["kind"], "cancelled");
    // And an unknown key is typed, never an error or a fake cancel.
    let ack = action_json(
        &mut app,
        &view2,
        json!({ "action": "import.cancel", "job_key": "import:nope" }),
    );
    assert_eq!(ack["status"], "unknown-job");

    gate.open();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn import_publish_failure_is_recoverable_and_resumes() {
    // The first publish dies unclassified-I/O — the verified manifest
    // keeps the job resumable, so the terminal is typed `recoverable`
    // (never `failed`), and `import.retry` converges on the live
    // commit.
    let (mut app, dir) = app_with_publisher(
        "recover",
        Arc::new(FailOncePublisher::new()),
        vec![FakeProvider::new(1, true, true, true)],
    );
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);

    action_json(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 35,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    let j = wait_job(&mut app, &import_key(35));
    assert_eq!(j["terminal"]["kind"], "recoverable");
    let v = proj(&mut app, &view);
    assert_eq!(v["state"]["import"]["state"], "recoverable");
    assert_eq!(v["state"]["import"]["reason"], "io");

    action(&mut app, &view, json!({ "action": "import.retry" }));
    let j = wait_job(&mut app, &import_key(35));
    assert_eq!(j["terminal"]["kind"], "committed-live");
    assert_eq!(
        proj(&mut app, &view)["state"]["import"]["state"],
        "committed-live"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn preview_fetch_job_cancels_cooperatively() {
    // `preview.cancel` raises the job's token — the gated reader
    // observes it mid-stream and the terminal lands as a typed
    // `cancelled` on both the ledger and the preview surface.
    let gate = Gate::new();
    let provider = FakeProvider::new(1, true, true, true)
        .with_page(page(
            vec![entry("a.wav", "loc-a", synvelis_library::EntryKind::Asset)],
            synvelis_library::Coverage::Complete,
            None,
        ))
        .with_preview_gate(gate.clone());
    let mut app = app_with(provider);
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);

    action_json(
        &mut app,
        &view,
        json!({
            "action": "preview.fetch",
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    let key = preview_key(&view);
    wait_job_registered(&mut app, &key);

    action(&mut app, &view, json!({ "action": "preview.cancel" }));
    let j = wait_job(&mut app, &key);
    assert_eq!(j["kind"], "preview-fetch");
    assert_eq!(j["terminal"]["kind"], "cancelled");
    assert_eq!(
        proj(&mut app, &view)["state"]["preview"]["state"],
        "cancelled"
    );
}

#[test]
fn view_close_leaves_job_in_ledger() {
    // The ledger is app-owned — closing the spawning view never drops
    // the entry, and a sibling view still observes the terminal.
    let (mut app, dir) = app_with_import("closejob");
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);

    action_json(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 36,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    wait_job(&mut app, &import_key(36));

    app.op_library_visual(&json!({ "action": "close", "view_id": view }))
        .expect("close");
    let j = job(&mut app, &import_key(36)).expect("entry survives view close");
    assert_eq!(j["terminal"]["kind"], "committed-live");

    let _ = std::fs::remove_dir_all(&dir);
}
