//! VCS-04 backend-wiring acceptance tests (synvelis#95) — the
//! production `set_vcs_backend` path shipped by the backend-wiring
//! slice: `project.new`/`open` detect the git workspace containing
//! the bundle, bind a real [`GitVcsProvider`] through
//! `VcsHistoryAdapter`, re-bind per project root, and clear the
//! backend honestly on non-repository bundles.
//!
//! Requires the `git` CLI (same convention as the synvelis-library
//! VCS-03 fixtures): [`Repo::new`] returns `None` when git is
//! unavailable and the tests skip honestly instead of failing.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use ruviform_ui::BackendReport;
use ruviform_ui::dock::ViewInstanceId;
use serde_json::{Value, json};

use synvelis_app::SynvelisApp;
use synvelis_project::ProjectOp;
use synvelis_workspace::ViewType;

const VIEW: u64 = 950;

// ---------------------------------------------------------------------------
// fixture: temp tree + real git CLI repo (mirrors
// synvelis-library/tests/common/{mod,git_fixture,vcs_git_fixture}.rs)
// ---------------------------------------------------------------------------

struct TempTree {
    root: PathBuf,
}

impl TempTree {
    fn new(tag: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let seq = NEXT.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "synvelis-app-vcsw-{tag}-{}-{seq}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp root");
        let root = fs::canonicalize(&root).expect("canonical temp root");
        Self { root }
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// `git` is runnable.
fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Non-verbatim spelling for git CLI path arguments — `TempTree`
/// roots canonicalize to `\\?\C:\…` on Windows, which `git -C`
/// tolerates but git's own path arguments reject.
fn plain_path(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p.to_path_buf()
    }
}

/// Runs `git <args>` in `dir`; panics on failure (fixture setup must
/// be deterministic).
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(plain_path(dir))
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?} spawn failed: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// A plain (non-bare) git worktree built entirely through the CLI.
struct Repo {
    /// Owns every path below.
    _tmp: TempTree,
    /// Worktree root in plain (non-verbatim) spelling.
    root: PathBuf,
}

impl Repo {
    /// `git init` + identity config, or `None` without git.
    fn new(tag: &str) -> Option<Self> {
        if !git_available() {
            return None;
        }
        let tmp = TempTree::new(tag);
        let root = plain_path(&tmp.join("repo"));
        fs::create_dir_all(&root).expect("repo dir");
        git(&root, &["init", "--initial-branch=main"]);
        git(&root, &["config", "user.email", "fixture@test"]);
        git(&root, &["config", "user.name", "fixture"]);
        git(&root, &["config", "commit.gpgsign", "false"]);
        git(&root, &["config", "core.autocrlf", "false"]);
        // Neutralize ambient git-lfs like the library fixture:
        // pointer blobs must commit verbatim.
        git(&root, &["config", "filter.lfs.clean", "cat"]);
        git(&root, &["config", "filter.lfs.smudge", "cat"]);
        git(&root, &["config", "filter.lfs.process", ""]);
        git(&root, &["config", "filter.lfs.required", "false"]);
        let no_hooks = plain_path(&tmp.join("no-hooks"));
        fs::create_dir_all(&no_hooks).expect("no-hooks dir");
        let hooks_path = no_hooks.to_string_lossy().replace('\\', "/");
        git(&root, &["config", "core.hooksPath", hooks_path.as_str()]);
        Some(Self { _tmp: tmp, root })
    }

    fn git(&self, args: &[&str]) -> String {
        git(&self.root, args)
    }

    /// `git add -A` + commit; returns the new HEAD sha.
    fn commit_all(&self, msg: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-m", msg]);
        self.git(&["rev-parse", "HEAD"])
    }
}

impl TempTree {
    fn join(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.root.join(rel.as_ref())
    }
}

// ---------------------------------------------------------------------------
// app helpers (same shape as vcs04_history.rs)
// ---------------------------------------------------------------------------

fn app() -> SynvelisApp {
    SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()))
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

fn rename(app: &mut SynvelisApp, name: &str) {
    apply(app, ProjectOp::SetProjectName { name: name.into() });
}

fn view_id() -> String {
    format!("view-{VIEW}")
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

fn bundle_str(path: &Path) -> String {
    plain_path(path).to_string_lossy().into_owned()
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

/// `project.new` inside a git worktree binds a real backend; committed
/// and staged endpoints resolve the bundle's `main.json` project file
/// across real commits — the whole chain the headless slice could only
/// fake.
#[test]
fn project_new_binds_real_backend_and_serves_committed_diff() {
    let Some(repo) = Repo::new("bind") else {
        return; // honest skip without git CLI
    };
    let mut app = app();
    let bundle = repo.root.join("proj");
    fs::create_dir_all(&bundle).expect("bundle dir");
    app.project_new(&bundle_str(&bundle), Some("v1"))
        .expect("project.new");
    assert!(app.vcs_backend().is_some(), "backend bound in repo");
    assert!(bundle.join("main.json").is_file(), "main.json published");

    // c1: the initial save's snapshot committed to git.
    let c1 = repo.commit_all("c1");

    rename(&mut app, "v2");
    app.project_save().expect("save v2");
    repo.commit_all("c2");

    // Staged endpoint material: save v3 to the bundle but only `git
    // add` it — the index carries the new main.json, HEAD stays c2.
    rename(&mut app, "v3");
    app.project_save().expect("save v3");
    repo.git(&["add", "proj"]);

    adopt_history(&mut app);
    action(&mut app, json!({ "action": "refresh" }));
    let v = history(&app);
    assert_eq!(v["backend_attached"], true);
    assert_eq!(v["status"], "ready");
    let repository_id = v["repository_id"].as_str().expect("repository_id");
    assert!(repository_id.starts_with("git:"), "{repository_id}");
    assert_eq!(v["rows"].as_array().unwrap().len(), 2);

    // committed c1 → committed HEAD(c2): real envelopes at
    // `proj/main.json` differ by project name.
    pin(
        &mut app,
        "before",
        &json!({ "kind": "committed", "expr": c1 }),
    );
    pin(&mut app, "after", &json!({ "kind": "committed" }));
    let v = history(&app);
    let sel = &v["selection"];
    assert_eq!(sel["before"]["kind"], "committed");
    assert_eq!(sel["after"]["kind"], "committed");
    assert_eq!(sel["diff"]["overall"], "different");
    assert!(
        sel["diff"]["changes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["domain"] == "Project"),
        "expected a Project-domain change: {}",
        sel["diff"]
    );

    // committed HEAD(c2) → staged: the index blob of `proj/main.json`
    // holds the v3 envelope.
    pin(&mut app, "after", &json!({ "kind": "staged" }));
    let v = history(&app);
    assert_eq!(v["selection"]["after"]["kind"], "staged");
    assert_eq!(v["selection"]["diff"]["overall"], "different");
}

/// The backend is bound per project root: opening a different repo
/// re-binds (new `repository_id`), and a non-repository bundle clears
/// it — a stale backend is never kept.
#[test]
fn backend_rebinds_per_root_and_clears_on_non_repo() {
    let (Some(repo_a), Some(repo_b)) = (Repo::new("rebind-a"), Repo::new("rebind-b")) else {
        return; // honest skip without git CLI
    };
    let plain = TempTree::new("plain");
    let mut app = app();

    let refresh_repo_id = |app: &mut SynvelisApp| -> String {
        adopt_history(app);
        action(app, json!({ "action": "refresh" }));
        history(app)["repository_id"]
            .as_str()
            .expect("repository_id")
            .to_owned()
    };

    // Non-repository bundle first: honest NoBackend.
    let plain_bundle = plain.join("proj");
    fs::create_dir_all(&plain_bundle).expect("plain bundle dir");
    app.project_new(&bundle_str(&plain_bundle), Some("plain"))
        .expect("project.new plain");
    assert!(app.vcs_backend().is_none(), "no backend outside git");
    adopt_history(&mut app);
    assert_eq!(history(&app)["status"], "no-backend");

    // Repo A binds; repo B re-binds with a different repository id.
    // Each repo gets an initial commit before refresh: the git
    // provider's unborn-HEAD history path is a known gap (see the
    // backend-wiring spec's known limitations), so fixtures exercise
    // the supported born-HEAD state.
    let bundle_a = repo_a.root.join("proj");
    fs::create_dir_all(&bundle_a).expect("bundle a");
    app.project_new(&bundle_str(&bundle_a), Some("a"))
        .expect("project.new a");
    assert!(app.vcs_backend().is_some());
    repo_a.commit_all("a-c1");
    let id_a = refresh_repo_id(&mut app);

    let bundle_b = repo_b.root.join("proj");
    fs::create_dir_all(&bundle_b).expect("bundle b");
    app.project_new(&bundle_str(&bundle_b), Some("b"))
        .expect("project.new b");
    assert!(app.vcs_backend().is_some());
    repo_b.commit_all("b-c1");
    let id_b = refresh_repo_id(&mut app);
    assert_ne!(id_a, id_b, "rebind must follow the project root");

    // Back to a non-repository root: the repo-B backend is cleared.
    let plain_bundle2 = plain.join("proj2");
    fs::create_dir_all(&plain_bundle2).expect("plain bundle2 dir");
    app.project_new(&bundle_str(&plain_bundle2), Some("plain2"))
        .expect("project.new plain2");
    assert!(app.vcs_backend().is_none(), "stale backend cleared");

    // Re-opening a repo bundle re-binds.
    app.project_open(&bundle_str(&bundle_a))
        .expect("project.open a");
    assert!(app.vcs_backend().is_some(), "rebind on open");
    assert_eq!(refresh_repo_id(&mut app), id_a);
}
