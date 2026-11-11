//! VCS-06 UI headless matrix tests (synvelis#97) — the `syn.vcs.*`
//! op surface's Unknown/Partial honesty rows that the vcs04
//! happy-path fixtures do not cover.
//!
//! - Committed endpoints from a different `project_id` stay
//!   `unknown` with issue rows — never collapsed to `equal` or a
//!   zero-change diff (the UI-level counterpart of
//!   `different_project_ids_are_unknown_not_diffed`).
//! - A committed endpoint whose asset bytes read `Absent` resolves
//!   degraded: the diff reports `Partial` completeness with
//!   `missing-asset`/`unresolved-source` issue rows, mirrored into
//!   the `vcs.issues` screen-spec table.
//! - A staged pin whose `index_generation` probe fails — or whose
//!   backend is absent at projection time — reports `unknown`
//!   freshness with a reason, never `fresh`.
//!
//! A/B pins, merge parents, live-stale, view lifecycle, and dock
//! state retention are covered by `vcs04_history.rs` and the
//! synvelis-workspace `vcs04_history` tests — not duplicated here.
//! Real GPUI/IME/keyboard/two-window evidence stays `NotRun` (#114);
//! see `evidence/reports/vcs-06-fixture-matrix-ledger.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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
    BlobRead, ProjectController, ProjectId, ProjectOp, SCHEMA_VERSION, SnapshotEnvelope,
    VcsFileRead, VcsLocator,
};
use synvelis_workspace::{EndpointFreshness, EndpointKind, VcsSession, ViewType, vcs_history_view};

const REPO: &str = "repo-1";
const PROJECT_PATH: &str = "project/main.json";
const VIEW: u64 = 900;
const ASSET_DIGEST: &str =
    "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const FOREIGN_PROJECT_ID: u128 = 0x00C0_FFEE;

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

/// Re-identifies an envelope as a different project — same content,
/// foreign `project_id` on both the envelope and the embedded
/// project, exactly what a cross-project VCS read would surface.
fn foreign_project_envelope(bytes: &[u8]) -> Vec<u8> {
    let mut envelope: SnapshotEnvelope = serde_json::from_slice(bytes).expect("envelope parse");
    envelope.project_id = ProjectId::new(FOREIGN_PROJECT_ID);
    envelope.project.project_id = ProjectId::new(FOREIGN_PROJECT_ID);
    serde_json::to_vec(&envelope).expect("envelope serialize")
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
/// provider implements, scripted for the fixture. `index_probe_fail`
/// is shared with the test so the staged-freshness probe can be
/// failed *after* the pin resolved (the adapter owns the provider).
struct FakeProvider {
    files: BTreeMap<String, Vec<u8>>,
    first_page: HistoryPage,
    rest_pages: BTreeMap<String, HistoryPage>,
    index_probe_fail: Arc<AtomicBool>,
}

impl FakeProvider {
    fn new(index_probe_fail: Arc<AtomicBool>) -> Self {
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
            index_probe_fail,
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
            // "cross-project" serves the foreign-project envelope at
            // 'f'; "degraded" serves the asset-bearing envelope at
            // 'g' whose blob reads stay `Absent`.
            RevisionSelector::Expr(expr) if expr == "cross-project" => {
                Ok(fixed_rev(REPO, &oid('f')))
            }
            RevisionSelector::Expr(expr) if expr == "degraded" => Ok(fixed_rev(REPO, &oid('g'))),
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
        if self.index_probe_fail.load(Ordering::Relaxed) {
            return Err(VcsPortError::Unavailable {
                detail: "index generation probe offline".into(),
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

/// App whose live project is at "vcs-c" plus one registered asset.
/// Committed endpoints cover "a"/"b"/"c"(+merge "d", second parent
/// "e"), the foreign-project envelope at "f", and the asset-bearing
/// envelope at "g"; the staged set is the pre-asset "c" state. The
/// returned flag fails the provider's `index_generation` probe on
/// demand — staged pins resolve before it is set.
fn fixture() -> (SynvelisApp, Arc<AtomicBool>) {
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
    apply(
        &mut app,
        ProjectOp::RegisterAsset {
            digest: ASSET_DIGEST.into(),
        },
    );
    let env_g = envelope_bytes(app.workspace().session().controller());
    let env_f = foreign_project_envelope(&env_c);

    let index_probe_fail = Arc::new(AtomicBool::new(false));
    let mut provider = FakeProvider::new(Arc::clone(&index_probe_fail));
    provider.put_committed(&oid('a'), PROJECT_PATH, env_a);
    provider.put_committed(&oid('b'), PROJECT_PATH, env_b.clone());
    provider.put_committed(&oid('c'), PROJECT_PATH, env_c.clone());
    provider.put_committed(&oid('d'), PROJECT_PATH, env_c.clone());
    provider.put_committed(&oid('e'), PROJECT_PATH, env_b);
    provider.put_committed(&oid('f'), PROJECT_PATH, env_f);
    provider.put_committed(&oid('g'), PROJECT_PATH, env_g);
    provider.put_staged(PROJECT_PATH, env_c);
    let info = workspace_info();
    let adapter = VcsHistoryAdapter::new(provider, &info, PROJECT_PATH).expect("adapter");
    app.set_vcs_backend(Arc::new(adapter));
    adopt_history(&mut app);
    (app, index_probe_fail)
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

fn screen_spec_issues_table_rows(app: &SynvelisApp) -> usize {
    let inst = ViewInstanceId::new(VIEW).expect("nonzero view id");
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
    let table = root
        .children
        .iter()
        .find_map(|child| match child {
            ComponentSpec::Table(t) if t.key.0 == "vcs.issues" => Some(t),
            _ => None,
        })
        .expect("vcs.issues table");
    assert!(table.selection.is_none(), "vcs table must be read-only");
    table.rows.len()
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[test]
fn cross_project_committed_endpoints_report_unknown_not_equal() {
    let (mut app, _probe) = fixture();
    // HEAD resolves to the merge commit 'd' serving env_c (project_id
    // 0); "cross-project" serves env_f — identical content re-identified
    // as project_id 0xC0FFEE.
    pin(&mut app, "before", &json!({ "kind": "committed" }));
    pin(
        &mut app,
        "after",
        &json!({ "kind": "committed", "expr": "cross-project" }),
    );

    let v = history(&app);
    let sel = &v["selection"];
    assert_eq!(sel["before"]["kind"], "committed");
    assert_eq!(sel["after"]["kind"], "committed");
    assert_eq!(sel["before"]["freshness"], "fresh");
    assert_eq!(sel["after"]["freshness"], "fresh");

    let diff = &sel["diff"];
    // Unknown stays distinct from Equal: the cross-project identity
    // collision is an unresolved issue, never a zero-change diff.
    assert_eq!(diff["overall"], "unknown");
    assert_eq!(diff["completeness"], "Partial");
    assert_eq!(diff["summary"]["total"], 0);
    assert!(
        diff["summary"]["unresolved"].as_u64().unwrap() > 0,
        "unresolved issues must keep the diff out of Equal"
    );
    assert_eq!(diff["changes"], json!([]));
    let issues = diff["issues"].as_array().unwrap();
    assert_ne!(diff["issues"], json!([]));
    assert!(
        issues.iter().any(|i| i["kind"] == "ambiguous-identity"),
        "got {issues:?}"
    );
}

#[test]
fn absent_asset_bytes_mark_degraded_partial_with_issue_rows() {
    let (mut app, _probe) = fixture();
    // 'g' carries the registered asset but every blob read reports
    // `Absent` → `ContentStatus::Missing`; the live pin probes with
    // `blobs: None` → `ContentStatus::Unknown`. Identical project
    // content on both sides — equality must still fail closed.
    pin(
        &mut app,
        "before",
        &json!({ "kind": "committed", "expr": "degraded" }),
    );
    pin(&mut app, "after", &json!({ "kind": "live" }));

    let v = history(&app);
    let diff = &v["selection"]["diff"];
    assert_eq!(diff["overall"], "unknown");
    assert_eq!(diff["completeness"], "Partial");
    assert_eq!(diff["summary"]["total"], 0);
    assert_eq!(diff["changes"], json!([]));
    let unresolved = diff["summary"]["unresolved"].as_u64().unwrap();
    assert!(unresolved >= 2, "got {diff:?}");

    let issues = diff["issues"].as_array().unwrap();
    let kinds: Vec<&str> = issues
        .iter()
        .map(|i| i["kind"].as_str().expect("issue kind"))
        .collect();
    assert!(kinds.contains(&"missing-asset"), "got {kinds:?}");
    assert!(kinds.contains(&"unresolved-source"), "got {kinds:?}");

    // The screen spec mirrors the same issue rows into the read-only
    // `vcs.issues` table — an issue never reads as an empty diff.
    assert_eq!(
        screen_spec_issues_table_rows(&app),
        usize::try_from(unresolved).expect("unresolved fits usize")
    );
}

#[test]
fn staged_pin_freshness_is_unknown_when_index_probe_fails() {
    let (mut app, index_probe_fail) = fixture();
    pin(&mut app, "before", &json!({ "kind": "staged" }));
    pin(&mut app, "after", &json!({ "kind": "committed" }));

    // Baseline: the pinned index generation still matches.
    let v = history(&app);
    assert_eq!(v["selection"]["before"]["kind"], "staged");
    assert_eq!(v["selection"]["before"]["freshness"], "fresh");
    assert_eq!(v["selection"]["before"]["freshness_reason"], Value::Null);

    // The probe fails after the pin resolved — freshness degrades to
    // `unknown` with the verbatim reason, never silently `fresh`.
    index_probe_fail.store(true, Ordering::Relaxed);
    let v = history(&app);
    let before = &v["selection"]["before"];
    assert_eq!(before["freshness"], "unknown");
    let reason = before["freshness_reason"]
        .as_str()
        .expect("unknown freshness carries a reason");
    assert!(
        reason.contains("index generation probe failed"),
        "got {reason}"
    );
    // Committed pins are immutable by construction — unaffected.
    assert_eq!(v["selection"]["after"]["freshness"], "fresh");
}

#[test]
fn staged_pin_freshness_is_unknown_without_backend() {
    let (mut app, _probe) = fixture();
    pin(&mut app, "before", &json!({ "kind": "staged" }));
    pin(&mut app, "after", &json!({ "kind": "committed" }));

    // `op_vcs_view` always carries the attached backend, so the
    // no-backend arm of `endpoint_freshness` is observed at the same
    // `vcs_history_view` projection boundary with `backend: None`.
    let inst = ViewInstanceId::new(VIEW).expect("nonzero view id");
    let ctx = VcsSession {
        backend: None,
        controller: app.workspace().session().controller(),
        store: None,
        blobs: None,
    };
    let view = vcs_history_view(app.workspace().vcs(), inst, &ctx);
    let sel = view.selection.as_ref().expect("selection");
    assert_eq!(sel.before.kind, EndpointKind::Staged);
    let EndpointFreshness::Unknown { reason } = &sel.before.freshness else {
        panic!(
            "staged pin without backend must be unknown, got {:?}",
            sel.before.freshness
        );
    };
    assert!(reason.contains("no VCS backend"), "got {reason}");
    assert_eq!(sel.after.freshness, EndpointFreshness::Fresh);
}
