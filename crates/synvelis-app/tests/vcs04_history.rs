//! VCS-04 acceptance tests (synvelis#95) — the `syn.vcs.*` op surface
//! over the headless History/A-B Visual Diff slice.
//!
//! - `syn.vcs.view` projects status/rows/selection/freshness/diff
//!   verbatim; `syn.vcs.visual` drives open/close/refresh/page/
//!   select/pin/compare/mode/clear — every op read-only toward
//!   project, undo, transport, engine, index, and worktree.
//! - A/B pins resolve immutable snapshots; committed endpoints stay
//!   `fresh` while a `live` pin reports `stale` after the project
//!   revision advances.
//! - Merge parents stay explicit; foreign repositories and missing
//!   backends/stores fail honestly instead of guessing.
//!
//! The backend is the real `VcsHistoryAdapter` over an in-memory
//! fake provider injected through `SynvelisApp::set_vcs_backend` —
//! production provider detection/binding is a later wiring slice.

mod common;

use common::vcs::*;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::ComponentSpec;
use serde_json::{Value, json};

use synvelis_app::{VcsScreenState, history_screen_spec};
use synvelis_project::ProjectOp;
use synvelis_workspace::{VcsSession, ViewType, vcs_history_view};

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[test]
fn history_projection_reports_backend_and_rows() {
    let mut app = fixture();
    let v = history(&app);
    assert_eq!(v["status"], "empty");
    assert_eq!(v["backend_attached"], true);
    assert_eq!(v["repository_id"], REPO);

    action(&mut app, json!({ "action": "refresh" }));
    let v = history(&app);
    assert_eq!(v["status"], "ready");
    assert_eq!(v["rows"].as_array().unwrap().len(), 2);
    assert_eq!(v["rows"][0]["is_merge"], true);
    assert_eq!(v["rows"][0]["parents"].as_array().unwrap().len(), 2);
    assert_eq!(v["next_cursor"], "page-2");

    action(&mut app, json!({ "action": "load_more" }));
    let v = history(&app);
    assert_eq!(v["rows"].as_array().unwrap().len(), 4);
    assert_eq!(v["next_cursor"], Value::Null);
}

#[test]
fn ab_pin_produces_diff_and_merge_parent_choice() {
    let mut app = fixture();
    action(&mut app, json!({ "action": "refresh" }));
    action(&mut app, json!({ "action": "load_more" }));

    pin(&mut app, "before", &json!({ "kind": "node", "index": 3 }));
    pin(&mut app, "after", &json!({ "kind": "node", "index": 1 }));
    let v = history(&app);
    let sel = &v["selection"];
    assert_eq!(sel["before"]["kind"], "committed");
    assert_eq!(sel["after"]["kind"], "committed");
    assert_eq!(sel["before"]["freshness"], "fresh");
    let diff = &sel["diff"];
    assert_eq!(diff["overall"], "different");
    assert!(diff["summary"]["total"].as_u64().unwrap() > 0);
    assert!(
        diff["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["domain"] == "Project")
    );

    // merge "after" endpoint records the explicit first parent
    pin(&mut app, "after", &json!({ "kind": "node", "index": 0 }));
    let v = history(&app);
    assert_eq!(v["selection"]["chosen_parent"], "cccccccccccc");
}

#[test]
fn live_pin_reports_stale_after_revision_advances() {
    let mut app = fixture();
    action(&mut app, json!({ "action": "refresh" }));
    pin(&mut app, "before", &json!({ "kind": "live" }));
    pin(
        &mut app,
        "after",
        &json!({ "kind": "committed" }), // HEAD → merge "d" = env_c = live state
    );
    let v = history(&app);
    assert_eq!(v["selection"]["before"]["freshness"], "fresh");
    assert_eq!(v["selection"]["diff"]["overall"], "equal");

    apply(
        &mut app,
        ProjectOp::SetProjectName {
            name: "vcs-d".into(),
        },
    );
    let v = history(&app);
    assert_eq!(v["new_revision_available"], true);
    assert_eq!(v["selection"]["before"]["freshness"], "stale");
    assert_eq!(v["selection"]["after"]["freshness"], "fresh");
}

#[test]
fn staged_and_saved_endpoints_report_honestly() {
    let mut app = fixture();
    pin(&mut app, "before", &json!({ "kind": "staged" }));
    pin(&mut app, "after", &json!({ "kind": "live" }));
    let v = history(&app);
    assert_eq!(v["selection"]["before"]["kind"], "staged");
    assert_eq!(v["selection"]["before"]["freshness"], "fresh");

    // no project store is open — Saved fails, never guesses
    let err = app
        .op_vcs_visual(&json!({
            "view_id": view_id(),
            "action": "pin",
            "side": "after",
            "endpoint": { "kind": "saved" },
        }))
        .expect_err("saved without store must fail");
    assert!(err.message.contains("store"), "got {err:?}");
}

#[test]
fn foreign_repository_and_bad_params_fail_honestly() {
    let mut app = fixture();
    action(&mut app, json!({ "action": "refresh" }));

    let err = app
        .op_vcs_visual(&json!({
            "view_id": view_id(),
            "action": "pin",
            "side": "before",
            "endpoint": { "kind": "committed", "expr": "foreign" },
        }))
        .expect_err("foreign revision must fail");
    assert!(err.message.contains("repo-1") || err.message.contains("repository"));

    // the failure is recorded verbatim for the projection
    let v = history(&app);
    assert_eq!(v["status"], "failed");
    assert!(v["last_error"].as_str().unwrap().contains("repo"));

    // unregistered / malformed view ids are caller errors
    assert!(app.op_vcs_view(&json!({ "view_id": "view-999" })).is_err());
    assert!(app.op_vcs_view(&json!({ "view_id": "main" })).is_err());
    assert!(
        app.op_vcs_visual(&json!({
            "view_id": view_id(),
            "action": "bogus",
        }))
        .is_err()
    );
}

#[test]
fn no_backend_reports_status_and_refuses_history() {
    let mut app = app();
    adopt_history(&mut app);
    let v = app
        .op_vcs_view(&json!({ "view_id": view_id() }))
        .expect("history projection");
    assert_eq!(v["status"], "no-backend");
    assert_eq!(v["backend_attached"], false);

    let err = app
        .op_vcs_visual(&json!({
            "view_id": view_id(),
            "action": "refresh",
        }))
        .expect_err("refresh without backend must fail");
    assert!(err.message.contains("backend"));

    // Live endpoints still resolve without a backend; once a
    // selection exists the status reports `ready` (`no-backend` only
    // describes an empty view).
    app.op_vcs_visual(&json!({
        "view_id": view_id(),
        "action": "pin",
        "side": "before",
        "endpoint": { "kind": "live" },
    }))
    .expect("live pin needs no backend");
    app.op_vcs_visual(&json!({
        "view_id": view_id(),
        "action": "pin",
        "side": "after",
        "endpoint": { "kind": "live" },
    }))
    .expect("live pin needs no backend");
    let v = history(&app);
    assert_eq!(v["status"], "ready");
    assert_eq!(v["selection"]["before"]["kind"], "live");
}

#[test]
fn view_close_drops_vcs_state_and_ops_stay_read_only() {
    let mut app = fixture();
    action(&mut app, json!({ "action": "refresh" }));
    pin(&mut app, "before", &json!({ "kind": "live" }));
    pin(&mut app, "after", &json!({ "kind": "staged" }));

    let inst = ViewInstanceId::new(VIEW).unwrap();
    assert!(app.workspace().vcs().contains(inst));

    let rev0 = app.workspace().session().revision();
    let builds0 = app.workspace().session().engine().graph_builds();
    let name0 = app.workspace().session().project().name.clone();
    let transport0 = (
        app.workspace().session().transport().playing,
        app.workspace().session().transport().position_ticks,
    );

    // dock moves / window-close lifecycle are covered exhaustively in
    // synvelis-workspace tests; here the app-level contract is that
    // removing the view drops its VCS state.
    app.workspace_mut().remove_view(inst).unwrap();
    assert!(!app.workspace().vcs().contains(inst));

    // nothing above touched project/engine/transport — and the ops
    // themselves are read-only by construction
    assert_eq!(app.workspace().session().revision(), rev0);
    assert_eq!(app.workspace().session().engine().graph_builds(), builds0);
    assert_eq!(app.workspace().session().project().name, name0);
    assert_eq!(
        (
            app.workspace().session().transport().playing,
            app.workspace().session().transport().position_ticks
        ),
        transport0
    );
}

#[test]
fn open_close_lifecycle_is_kind_scoped() {
    let mut app = app();
    // No views registered yet — `open` mints above the composition
    // root's reserved view-1/view-2 so a later adopt of either can
    // never collide.
    let opened = app
        .op_vcs_visual(&json!({ "action": "open" }))
        .expect("open");
    assert_eq!(opened["view_id"], "view-3");
    let inst = ViewInstanceId::new(3).unwrap();
    assert!(app.workspace().vcs().contains(inst));
    assert_eq!(
        app.workspace().registry().descriptor(inst).unwrap().kind,
        ViewType::VcsHistory
    );

    // Composition-root ids still adopt cleanly afterwards.
    app.workspace_mut()
        .adopt_view(
            ViewInstanceId::new(1).unwrap(),
            ViewType::Arrangement,
            "Main",
            None,
        )
        .expect("view-1 adopt after open");

    // A second open gets the next free id.
    let opened2 = app
        .op_vcs_visual(&json!({ "action": "open" }))
        .expect("open2");
    assert_eq!(opened2["view_id"], "view-4");

    // `close` refuses non-VCS kinds — it can never close view-1.
    let err = app
        .op_vcs_visual(&json!({ "action": "close", "view_id": "view-1" }))
        .expect_err("close view-1 must fail");
    assert!(err.message.contains("vcs-history"), "got {err:?}");

    // `close` drops the view, its registry entry, and its VCS state.
    let res = app
        .op_vcs_visual(&json!({ "action": "close", "view_id": "view-3" }))
        .expect("close view-3");
    assert_eq!(res["closed"], true);
    assert!(!app.workspace().vcs().contains(inst));
    assert!(!app.workspace().registry().contains(inst));
    assert!(app.op_vcs_view(&json!({ "view_id": "view-3" })).is_err());
    // view-1 survives untouched.
    assert!(
        app.workspace()
            .registry()
            .contains(ViewInstanceId::new(1).unwrap())
    );

    // Reopening never recycles a removed view's id — the workspace
    // high-water mark keeps minting upward.
    let opened3 = app
        .op_vcs_visual(&json!({ "action": "open" }))
        .expect("open3");
    assert_eq!(opened3["view_id"], "view-5");
}

#[test]
fn screen_spec_is_a_read_only_projection() {
    let mut app = fixture();
    action(&mut app, json!({ "action": "refresh" }));
    action(&mut app, json!({ "action": "load_more" }));
    pin(&mut app, "before", &json!({ "kind": "node", "index": 3 }));
    pin(&mut app, "after", &json!({ "kind": "node", "index": 1 }));

    let inst = ViewInstanceId::new(VIEW).unwrap();
    let ctx = VcsSession {
        backend: app.vcs_backend(),
        controller: app.workspace().session().controller(),
        store: None,
        blobs: None,
    };
    let view = vcs_history_view(app.workspace().vcs(), inst, &ctx);
    let spec = history_screen_spec(&VcsScreenState::from_view(view));

    let ComponentSpec::Panel(root) = &spec.root else {
        panic!("history spec root must be a panel");
    };
    assert_eq!(root.children.len(), 6);
    // status/selection/diff labels + history/changes/issues tables —
    // every table is read-only (`selection: None`, no intents).
    let mut tables = 0;
    for child in &root.children {
        if let ComponentSpec::Table(t) = child {
            tables += 1;
            assert!(t.selection.is_none(), "vcs table must be read-only");
        }
    }
    assert_eq!(tables, 3);
}
