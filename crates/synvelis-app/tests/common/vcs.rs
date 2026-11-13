//! VCS-04 shared fixture (synvelis#95) — the in-memory
//! `VcsReadProvider + VcsBytesPort` fake and the app composition the
//! `vcs04_history`/`vcs04_native_mount`/`vcs04_mount_gpui` lanes
//! share. Adapter-agnostic: no GPUI types appear here.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use ruviform_ui::BackendReport;
use ruviform_ui::dock::ViewInstanceId;
use serde_json::{Value, json};

use synvelis_app::SynvelisApp;
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
use synvelis_workspace::{ViewType, vcs_history_view};

pub const REPO: &str = "repo-1";
pub const PROJECT_PATH: &str = "project/main.json";
pub const VIEW: u64 = 900;

pub fn app() -> SynvelisApp {
    SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()))
}

pub fn view_id() -> String {
    format!("view-{VIEW}")
}

/// Adopts `view-900` as a detached [`ViewType::VcsHistory`] view —
/// the registry entry `syn.vcs.*` ops and dock resolution key on.
pub fn adopt_history(app: &mut SynvelisApp) {
    let inst = ViewInstanceId::new(VIEW).expect("nonzero view id");
    app.workspace_mut()
        .adopt_view(inst, ViewType::VcsHistory, "history", None)
        .expect("adopt history view");
}

pub fn apply(app: &mut SynvelisApp, op: ProjectOp) {
    let rev = app.workspace().session().revision();
    app.workspace_mut()
        .session_mut()
        .apply(op, rev)
        .expect("fixture op");
}

pub fn envelope_bytes(c: &ProjectController) -> Vec<u8> {
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

pub fn oid(ch: char) -> String {
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
pub struct FakeProvider {
    files: BTreeMap<String, Vec<u8>>,
    first_page: HistoryPage,
    rest_pages: BTreeMap<String, HistoryPage>,
}

impl FakeProvider {
    pub fn new() -> Self {
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

    pub fn put_committed(&mut self, commit_oid: &str, path: &str, bytes: Vec<u8>) {
        self.files
            .insert(format!("committed:{commit_oid}:{path}"), bytes);
    }

    pub fn put_staged(&mut self, path: &str, bytes: Vec<u8>) {
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

/// App + scripted backend with *no* adopted view — the state
/// `syn.vcs.visual{action:"open"}` mints `view-3` from.
pub fn fixture_backend() -> SynvelisApp {
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
    app
}

/// App whose live project is at "vcs-c" with committed envelopes for
/// "a"/"b"/"c"(+merge "d", second parent "e") and a staged copy of
/// "c" — the same states the workspace tests use.
pub fn fixture() -> SynvelisApp {
    let mut app = fixture_backend();
    adopt_history(&mut app);
    app
}

/// `syn.vcs.view` projection for the fixture's adopted view.
pub fn history(app: &SynvelisApp) -> Value {
    view_json(app, &view_id())
}

/// `syn.vcs.view` for an arbitrary wire view id.
pub fn view_json(app: &SynvelisApp, view: &str) -> Value {
    app.op_vcs_view(&json!({ "view_id": view }))
        .expect("syn.vcs.view")
}

/// `syn.vcs.visual{action:"open"}` — returns the minted wire id.
pub fn open(app: &mut SynvelisApp) -> String {
    app.op_vcs_visual(&json!({ "action": "open" }))
        .expect("syn.vcs.visual open")["view_id"]
        .as_str()
        .expect("view_id")
        .to_string()
}

/// `syn.vcs.visual` with an explicit wire view id.
pub fn action_on(app: &mut SynvelisApp, view: &str, params: Value) {
    let mut p = params;
    p["view_id"] = json!(view);
    app.op_vcs_visual(&p).expect("syn.vcs.visual");
}

/// `syn.vcs.visual` with `view_id` filled in for the fixture view.
pub fn action(app: &mut SynvelisApp, params: Value) {
    action_on(app, &view_id(), params);
}

/// `pin` shortcut for the fixture view.
pub fn pin(app: &mut SynvelisApp, side: &str, endpoint: &Value) {
    action(
        app,
        json!({ "action": "pin", "side": side, "endpoint": endpoint }),
    );
}

/// The fixture view's `ViewInstanceId`.
pub fn history_instance() -> ViewInstanceId {
    ViewInstanceId::new(VIEW).expect("nonzero view id")
}

/// `(revision, engine graph builds, project name, playing,
/// position_ticks)` — the project-authority snapshot VCS view
/// navigation and close must never move.
#[must_use]
pub fn authority(app: &SynvelisApp) -> (u64, u64, String, bool, i64) {
    let session = app.workspace().session();
    (
        session.revision().0,
        session.engine().graph_builds(),
        session.project().name.clone(),
        session.transport().playing,
        session.transport().position_ticks,
    )
}

/// Projects the fixture view into the wire JSON the ops return —
/// convenience for tests asserting on the read model.
#[allow(dead_code)]
pub fn projected_view(app: &SynvelisApp) -> synvelis_workspace::VcsHistoryView {
    let ctx = synvelis_workspace::VcsSession {
        backend: app.vcs_backend(),
        controller: app.workspace().session().controller(),
        store: None,
        blobs: None,
    };
    vcs_history_view(app.workspace().vcs(), history_instance(), &ctx)
}
