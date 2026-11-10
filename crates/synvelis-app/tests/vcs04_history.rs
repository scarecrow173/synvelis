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

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ruviform_ui::BackendReport;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::ComponentSpec;
use serde_json::{Value, json};

use synvelis_app::{SynvelisApp, VcsScreenState, history_screen_spec};
use synvelis_project::snapshot::{IndexGeneration, VcsBytesPort, VcsPortError};
use synvelis_project::vcs::{
    FixedVcsRevision, HistoryCompleteness, HistoryNode, HistoryPage, HistoryQuery, IndexEntry,
    IndexEntryMode, ProjectPath, ProjectRelativePath, ProjectScope, RevisionRelations,
    RevisionSelector, StagedSnapshot, VcsCancel, VcsCapabilities, VcsError, VcsFeature,
    VcsHistoryAdapter, VcsObjectFormat, VcsReadProvider, WorkingStatus, WorkspaceInfo,
};
use synvelis_project::{
    BlobRead, ProjectController, ProjectOp, SCHEMA_VERSION, SnapshotEnvelope, VcsFileRead,
    VcsLocator,
};
use synvelis_workspace::{VcsSession, ViewType, vcs_history_view};

const REPO: &str = "repo-1";
const PROJECT_PATH: &str = "project/main.json";
const VIEW: u64 = 900;

fn app() -> SynvelisApp {
    SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()))
}

fn view_id() -> String {
    format!("view-{VIEW}")
}

fn adopt_history(app: &mut SynvelisApp) {
    let inst = ViewInstanceId::new(VIEW).expect("nonzero view id");
    app.workspace_mut()
        .adopt_view(inst, ViewType::VcsHistory, "history", None)
        .expect("adopt history view");
}

fn apply(app: &mut SynvelisApp, op: ProjectOp) {
    let rev = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(op, rev)
        .expect("fixture op");
}

fn envelope_bytes(c: &ProjectController) -> Vec<u8> {
    let mut project = c.live().clone();
    project.id_watermark = c.snapshot_id_watermark();
    serde_json::to_vec(&SnapshotEnvelope {
        schema_version: SCHEMA_VERSION,
        project_id: project.project_id,
        revision: c.live_revision(),
        project,
    })
    .expect("envelope serialize")
}

fn oid(ch: char) -> String {
    ch.to_string().repeat(40)
}

fn capabilities() -> VcsCapabilities {
    VcsCapabilities {
        object_formats: BTreeSet::from([VcsObjectFormat::Sha1]),
        history: VcsFeature::Supported,
        working_status: VcsFeature::Supported,
        staged_index: VcsFeature::Supported,
        lfs_objects: VcsFeature::Supported,
        submodule_entries: VcsFeature::Supported,
        cancellation: VcsFeature::Supported,
        resource_bounds: VcsFeature::Supported,
    }
}

fn workspace_info() -> WorkspaceInfo {
    WorkspaceInfo {
        repository_id: REPO.into(),
        object_format: VcsObjectFormat::Sha1,
        worktree_root: "/tmp/wt".into(),
        git_dir: "/tmp/wt/.git".into(),
        git_common_dir: "/tmp/wt/.git".into(),
        project_root: String::new(),
        head_commit: Some(oid('d')),
        capabilities: capabilities(),
    }
}

fn fixed_rev(repository_id: &str, commit_oid: &str) -> FixedVcsRevision {
    FixedVcsRevision {
        repository_id: repository_id.into(),
        object_format: VcsObjectFormat::Sha1,
        commit_oid: commit_oid.into(),
        project_root: String::new(),
        context: "ctx".into(),
    }
}

fn node(rev: &FixedVcsRevision, parents: Vec<FixedVcsRevision>) -> HistoryNode {
    HistoryNode {
        revision: rev.clone(),
        parents,
        committed_at_unix: Some(1_700_000_000),
        author: Some("tester".into()),
        message: Some("commit subject".into()),
        completeness: HistoryCompleteness::Complete,
    }
}

fn scripted_nodes() -> Vec<HistoryNode> {
    vec![
        node(
            &fixed_rev(REPO, &oid('d')),
            vec![fixed_rev(REPO, &oid('c')), fixed_rev(REPO, &oid('e'))],
        ),
        node(
            &fixed_rev(REPO, &oid('c')),
            vec![fixed_rev(REPO, &oid('b'))],
        ),
        node(
            &fixed_rev(REPO, &oid('b')),
            vec![fixed_rev(REPO, &oid('a'))],
        ),
        node(&fixed_rev(REPO, &oid('a')), vec![]),
    ]
}

/// In-memory `VcsReadProvider + VcsBytesPort` — same contract the Git
/// provider implements, scripted for the fixture.
struct FakeProvider {
    files: BTreeMap<String, Vec<u8>>,
    first_page: HistoryPage,
    rest_pages: BTreeMap<String, HistoryPage>,
}

impl FakeProvider {
    fn new() -> Self {
        let nodes = scripted_nodes();
        Self {
            files: BTreeMap::new(),
            first_page: HistoryPage {
                nodes: nodes[..2].to_vec(),
                next_cursor: Some("page-2".into()),
            },
            rest_pages: BTreeMap::from([(
                "page-2".into(),
                HistoryPage {
                    nodes: nodes[2..].to_vec(),
                    next_cursor: None,
                },
            )]),
        }
    }

    fn put_committed(&mut self, commit_oid: &str, path: &str, bytes: Vec<u8>) {
        self.files
            .insert(format!("committed:{commit_oid}:{path}"), bytes);
    }

    fn put_staged(&mut self, path: &str, bytes: Vec<u8>) {
        self.files.insert(format!("staged:{path}"), bytes);
    }
}

impl VcsReadProvider for FakeProvider {
    fn detect_workspace(_root: &ProjectPath) -> Result<WorkspaceInfo, VcsError> {
        Ok(workspace_info())
    }

    fn capabilities(&self) -> VcsCapabilities {
        capabilities()
    }

    fn working_status(
        &self,
        _scope: &ProjectScope,
        cancel: &VcsCancel,
    ) -> Result<WorkingStatus, VcsError> {
        if cancel.is_cancelled() {
            return Err(VcsError::Cancelled);
        }
        Ok(WorkingStatus {
            entries: Vec::new(),
            index_generation: IndexGeneration("gen-1".into()),
            head_commit: Some(oid('d')),
        })
    }

    fn list_history(
        &self,
        query: &HistoryQuery,
        cancel: &VcsCancel,
    ) -> Result<HistoryPage, VcsError> {
        if cancel.is_cancelled() {
            return Err(VcsError::Cancelled);
        }
        match &query.cursor {
            None => Ok(self.first_page.clone()),
            Some(cursor) => {
                self.rest_pages
                    .get(cursor)
                    .cloned()
                    .ok_or_else(|| VcsError::InvalidRequest {
                        detail: format!("unknown cursor {cursor}"),
                    })
            }
        }
    }

    fn resolve_fixed_revision(
        &self,
        selector: &RevisionSelector,
        cancel: &VcsCancel,
    ) -> Result<FixedVcsRevision, VcsError> {
        if cancel.is_cancelled() {
            return Err(VcsError::Cancelled);
        }
        match selector {
            RevisionSelector::Head => Ok(fixed_rev(REPO, &oid('d'))),
            RevisionSelector::Expr(expr) if expr == "foreign" => {
                Ok(fixed_rev("other-repo", &oid('a')))
            }
            RevisionSelector::Expr(expr) => Err(VcsError::InvalidRequest {
                detail: format!("unknown revision expression {expr}"),
            }),
        }
    }

    fn revision_relations(
        &self,
        fixed: &FixedVcsRevision,
        cancel: &VcsCancel,
    ) -> Result<RevisionRelations, VcsError> {
        if cancel.is_cancelled() {
            return Err(VcsError::Cancelled);
        }
        let parents = scripted_nodes()
            .into_iter()
            .find(|n| n.revision == *fixed)
            .map(|n| n.parents)
            .unwrap_or_default();
        Ok(RevisionRelations {
            revision: fixed.clone(),
            parents,
        })
    }

    fn read_revision_file(
        &self,
        _fixed: &FixedVcsRevision,
        _path: &ProjectRelativePath,
        _cancel: &VcsCancel,
    ) -> Result<Vec<u8>, VcsError> {
        Ok(Vec::new())
    }

    fn resolve_staged_snapshot(&self, cancel: &VcsCancel) -> Result<StagedSnapshot, VcsError> {
        if cancel.is_cancelled() {
            return Err(VcsError::Cancelled);
        }
        Ok(StagedSnapshot {
            index_generation: IndexGeneration("gen-1".into()),
            entries: vec![IndexEntry {
                path: PROJECT_PATH.into(),
                stage: 0,
                mode: IndexEntryMode::Regular,
                object_id: oid('c'),
            }],
            conflicted_paths: Vec::new(),
            complete: true,
        })
    }
}

impl VcsBytesPort for FakeProvider {
    fn index_generation(&self, repository_id: &str) -> Result<IndexGeneration, VcsPortError> {
        if repository_id != REPO {
            return Err(VcsPortError::NotFound {
                detail: "unknown repository_id".into(),
            });
        }
        Ok(IndexGeneration("gen-1".into()))
    }

    fn read_file(&self, locator: &VcsLocator, path: &str) -> Result<VcsFileRead, VcsPortError> {
        let key = match locator {
            VcsLocator::Staged { .. } => format!("staged:{path}"),
            VcsLocator::Committed { fixed_revision, .. } => {
                format!("committed:{fixed_revision}:{path}")
            }
        };
        Ok(match self.files.get(&key) {
            Some(bytes) => VcsFileRead::Present(bytes.clone()),
            None => VcsFileRead::Absent,
        })
    }

    fn read_blob(&self, locator: &VcsLocator, digest: &str) -> Result<BlobRead, VcsPortError> {
        let key = match locator {
            VcsLocator::Staged { .. } => format!("staged:{digest}"),
            VcsLocator::Committed { fixed_revision, .. } => {
                format!("committed:{fixed_revision}:{digest}")
            }
        };
        Ok(match self.files.get(&key) {
            Some(bytes) => BlobRead::Present(bytes.clone()),
            None => BlobRead::Absent,
        })
    }
}

/// App whose live project is at "vcs-c" with committed envelopes for
/// "a"/"b"/"c"(+merge "d", second parent "e") and a staged copy of
/// "c" — the same states the workspace tests use.
fn fixture() -> SynvelisApp {
    let mut app = app();
    let env_a = envelope_bytes(app.workspace().session().controller());
    apply(
        &mut app,
        ProjectOp::SetProjectName {
            name: "vcs-b".into(),
        },
    );
    let env_b = envelope_bytes(app.workspace().session().controller());
    apply(
        &mut app,
        ProjectOp::SetProjectName {
            name: "vcs-c".into(),
        },
    );
    let env_c = envelope_bytes(app.workspace().session().controller());

    let mut provider = FakeProvider::new();
    provider.put_committed(&oid('a'), PROJECT_PATH, env_a);
    provider.put_committed(&oid('b'), PROJECT_PATH, env_b.clone());
    provider.put_committed(&oid('c'), PROJECT_PATH, env_c.clone());
    provider.put_committed(&oid('d'), PROJECT_PATH, env_c.clone());
    provider.put_committed(&oid('e'), PROJECT_PATH, env_b);
    provider.put_staged(PROJECT_PATH, env_c);
    let info = workspace_info();
    let adapter = VcsHistoryAdapter::new(provider, &info, PROJECT_PATH).expect("adapter");
    app.set_vcs_backend(Arc::new(adapter));
    adopt_history(&mut app);
    app
}

fn history(app: &SynvelisApp) -> Value {
    app.op_vcs_view(&json!({ "view_id": view_id() }))
        .expect("syn.vcs.view")
}

fn action(app: &mut SynvelisApp, params: Value) {
    let mut p = params;
    p["view_id"] = json!(view_id());
    app.op_vcs_visual(&p).expect("syn.vcs.visual");
}

fn pin(app: &mut SynvelisApp, side: &str, endpoint: &Value) {
    action(
        app,
        json!({ "action": "pin", "side": side, "endpoint": endpoint }),
    );
}

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
