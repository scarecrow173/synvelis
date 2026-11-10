//! VCS-04 acceptance tests (synvelis#95) — headless read-only
//! History/A-B Visual Diff slice.
//!
//! Covers the workspace half of the slice's acceptance:
//!
//! - A/B pinning resolves immutable snapshots and produces a shared
//!   `ChangeSet`; two views comparing the same endpoints share one
//!   `Arc` through the bounded cache.
//! - Dock / two-window moves, detach/reattach, and pool round-trips
//!   preserve per-view history selection, pins, and observed
//!   revision; `ReleaseViews` window close and `remove_view` drop the
//!   state.
//! - Endpoint freshness distinguishes `Fresh`/`Stale`/`Unknown`:
//!   Live pins go stale when the project revision advances, staged
//!   pins go stale when the index generation moves, committed pins
//!   are always fresh.
//! - Merge commits keep all parents; the merge "after" endpoint
//!   records an explicit first-parent choice.
//! - The whole surface is read-only: revision, engine builds,
//!   transport, and project content are unchanged by every op, and a
//!   foreign-repository revision is rejected before any read.
//!
//! The backend under test is the real `VcsHistoryAdapter` over an
//! in-memory fake provider — the same read-only contract the Git
//! provider implements — so snapshot resolution, index-generation
//! bracketing, and foreign-binding rejection are exercised end to end.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use ruviform_ui::dock::{DockTarget, ViewInstanceId, WindowCloseDisposition};
use ruviform_ui::event::HostEvent;
use ruviform_ui::window::MonitorInfo;
use synvelis_project::snapshot::{IndexGeneration, VcsBytesPort, VcsPortError};
use synvelis_project::vcs::{
    FixedVcsRevision, HistoryCompleteness, HistoryNode, HistoryPage, HistoryQuery, IndexEntry,
    IndexEntryMode, ProjectPath, ProjectRelativePath, ProjectScope, RevisionRelations,
    RevisionSelector, StagedSnapshot, VcsCancel, VcsCapabilities, VcsError, VcsFeature,
    VcsHistoryAdapter, VcsHistoryBackend, VcsObjectFormat, VcsReadProvider, WorkingStatus,
    WorkspaceInfo,
};
use synvelis_project::{
    BlobRead, CounterIdGenerator, Project, ProjectController, ProjectId, ProjectOp, SCHEMA_VERSION,
    SnapshotEnvelope, SnapshotError, VcsFileRead, VcsLocator,
};
use synvelis_workspace::{
    EndpointFreshness, EndpointPin, EndpointRequest, FakeWindowHost, PinSide, ProjectSession,
    VcsEquality, VcsSession, VcsUiError, VcsViewMode, VcsViewStatus, ViewType, WorkspaceManager,
    endpoint_freshness, vcs_history_view,
};

// ---------------------------------------------------------------------------
// fixture: workspace + scripted VCS backend
// ---------------------------------------------------------------------------

const REPO: &str = "repo-1";
const PROJECT_PATH: &str = "project/main.json";

type Ws = WorkspaceManager<FakeWindowHost>;

fn oid(ch: char) -> String {
    ch.to_string().repeat(40)
}

fn oid_n(i: usize) -> String {
    format!("{i:040x}")
}

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0x95), "vcs04");
    WorkspaceManager::new(
        FakeWindowHost::with_monitors(vec![MonitorInfo {
            name: Some("primary".to_string()),
            width: 2560,
            height: 1440,
            x: 0,
            y: 0,
            scale_factor: 1.0,
        }]),
        ProjectSession::new(ProjectController::new(
            project,
            Box::new(CounterIdGenerator::new(1)),
        )),
    )
}

fn apply(ws: &mut Ws, op: ProjectOp) {
    let rev = ws.session().revision();
    ws.session_mut().apply(op, rev).expect("fixture op");
}

/// Serialized `SnapshotEnvelope` for the controller's current state —
/// the same bytes a committed/staged VCS read would return.
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
        message: Some("commit subject\n\nbody".into()),
        completeness: HistoryCompleteness::Complete,
    }
}

/// Scripted provider history (newest first):
///   d (merge) <- c + e ; c <- b ; b <- a ; a (root)
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

/// In-memory `VcsReadProvider + VcsBytesPort`. `files` is keyed
/// `committed:{oid}:{path}` / `staged:{path}`; `generation` is shared
/// with the test so staged drift can be simulated after pinning.
struct FakeProvider {
    generation: Arc<Mutex<IndexGeneration>>,
    files: BTreeMap<String, Vec<u8>>,
    first_page: HistoryPage,
    rest_pages: BTreeMap<String, HistoryPage>,
}

impl FakeProvider {
    fn new(generation: Arc<Mutex<IndexGeneration>>) -> Self {
        let nodes = scripted_nodes();
        Self {
            generation,
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

    fn generation(&self) -> IndexGeneration {
        self.generation.lock().expect("generation").clone()
    }

    fn file_key(locator: &VcsLocator, path: &str) -> String {
        match locator {
            VcsLocator::Staged { .. } => format!("staged:{path}"),
            VcsLocator::Committed { fixed_revision, .. } => {
                format!("committed:{fixed_revision}:{path}")
            }
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
            index_generation: self.generation(),
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
            index_generation: self.generation(),
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
        Ok(self.generation())
    }

    fn read_file(&self, locator: &VcsLocator, path: &str) -> Result<VcsFileRead, VcsPortError> {
        let key = Self::file_key(locator, path);
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

/// Builds the fixture: a session whose live project is at "vcs04-c"
/// (rev r2) plus an adapter whose committed revisions map to the
/// envelope bytes captured at each earlier state.
fn fixture() -> (
    Ws,
    VcsHistoryAdapter<FakeProvider>,
    Arc<Mutex<IndexGeneration>>,
) {
    let mut ws = fresh_ws();
    let env_a = envelope_bytes(ws.session().controller());
    apply(
        &mut ws,
        ProjectOp::SetProjectName {
            name: "vcs04-b".into(),
        },
    );
    let env_b = envelope_bytes(ws.session().controller());
    apply(
        &mut ws,
        ProjectOp::SetProjectName {
            name: "vcs04-c".into(),
        },
    );
    let env_c = envelope_bytes(ws.session().controller());

    let generation = Arc::new(Mutex::new(IndexGeneration("gen-1".into())));
    let mut provider = FakeProvider::new(Arc::clone(&generation));
    provider.put_committed(&oid('a'), PROJECT_PATH, env_a);
    provider.put_committed(&oid('b'), PROJECT_PATH, env_b.clone());
    provider.put_committed(&oid('c'), PROJECT_PATH, env_c.clone());
    provider.put_committed(&oid('d'), PROJECT_PATH, env_c.clone());
    provider.put_committed(&oid('e'), PROJECT_PATH, env_b);
    provider.put_staged(PROJECT_PATH, env_c);
    let info = workspace_info();
    let adapter = VcsHistoryAdapter::new(provider, &info, PROJECT_PATH).expect("adapter");
    (ws, adapter, generation)
}

fn spawn_history(ws: &mut Ws, window: ruviform_ui::window::WindowId) -> ViewInstanceId {
    ws.spawn_view(ViewType::VcsHistory, "history", DockTarget::Window(window))
        .expect("spawn vcs history")
}

fn backend(adapter: &VcsHistoryAdapter<FakeProvider>) -> &dyn VcsHistoryBackend {
    adapter
}

fn no_backend() -> Option<&'static dyn VcsHistoryBackend> {
    None
}

fn cancel() -> VcsCancel {
    VcsCancel::new()
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[test]
fn history_pages_and_node_selection() {
    let (mut ws, adapter, _gen) = fixture();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn_history(&mut ws, win);

    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: Some(backend(&adapter)),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        views
            .refresh_history(view, &ctx, None, &cancel())
            .expect("refresh");
        assert_eq!(views.get(view).unwrap().nodes.len(), 2);
        assert_eq!(
            views.get(view).unwrap().next_cursor.as_deref(),
            Some("page-2")
        );
        views
            .load_more_history(view, &ctx, &cancel())
            .expect("load more");
        let state = views.get(view).unwrap();
        assert_eq!(state.nodes.len(), 4);
        assert_eq!(state.next_cursor, None);
        // merge node keeps both parents
        assert_eq!(state.nodes[0].parents.len(), 2);
        views.select_node(view, 0).expect("select");
        assert_eq!(
            views.get(view).unwrap().selected.as_deref(),
            Some(oid('d').as_str())
        );
        assert!(matches!(
            views.select_node(view, 99),
            Err(VcsUiError::InvalidRequest { .. })
        ));
    });

    // read-model projection
    let ctx = VcsSession {
        backend: Some(backend(&adapter)),
        controller: ws.session().controller(),
        store: None,
        blobs: None,
    };
    let projection = vcs_history_view(ws.vcs(), view, &ctx);
    assert_eq!(projection.status, VcsViewStatus::Ready);
    assert_eq!(projection.rows.len(), 4);
    assert!(projection.rows[0].is_merge);
    assert_eq!(projection.rows[0].parents.len(), 2);
    assert_eq!(
        projection.rows[0].message_headline.as_deref(),
        Some("commit subject")
    );
    assert!(projection.rows[0].selected);
    assert_eq!(projection.repository_id.as_deref(), Some(REPO));
    assert!(!projection.new_revision_available);
}

#[test]
fn ab_pin_committed_nodes_produces_shared_changeset() {
    let (mut ws, adapter, _gen) = fixture();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view_a = spawn_history(&mut ws, win);
    let view_b = spawn_history(&mut ws, win);

    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: Some(backend(&adapter)),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        for view in [view_a, view_b] {
            views.refresh_history(view, &ctx, None, &cancel()).unwrap();
            views.load_more_history(view, &ctx, &cancel()).unwrap();
        }
        // before = node 3 ("a", oldest), after = node 1 ("c")
        views
            .pin_endpoint(
                view_a,
                &ctx,
                PinSide::Before,
                &EndpointRequest::Node { index: 3 },
                &cancel(),
            )
            .unwrap();
        views
            .pin_endpoint(
                view_a,
                &ctx,
                PinSide::After,
                &EndpointRequest::Node { index: 1 },
                &cancel(),
            )
            .unwrap();
        // same endpoints on the second view → cache hit, identical Arc
        views
            .pin_endpoint(
                view_b,
                &ctx,
                PinSide::Before,
                &EndpointRequest::Node { index: 3 },
                &cancel(),
            )
            .unwrap();
        views
            .pin_endpoint(
                view_b,
                &ctx,
                PinSide::After,
                &EndpointRequest::Node { index: 1 },
                &cancel(),
            )
            .unwrap();

        let diff_a = views.get(view_a).unwrap().current_diff.clone().unwrap();
        let diff_b = views.get(view_b).unwrap().current_diff.clone().unwrap();
        assert!(
            Arc::ptr_eq(&diff_a, &diff_b),
            "ChangeSet must be shared via cache"
        );
        assert_eq!(
            diff_a.overall,
            synvelis_project::diff::ComparisonEquality::Different
        );
        assert!(diff_a.summary.total > 0);
        // recompute is a cache hit — same Arc again
        views.compare_selection(view_a, &ctx, &cancel()).unwrap();
        let diff_a2 = views.get(view_a).unwrap().current_diff.clone().unwrap();
        assert!(Arc::ptr_eq(&diff_a, &diff_a2));
    });

    let ctx = VcsSession {
        backend: Some(backend(&adapter)),
        controller: ws.session().controller(),
        store: None,
        blobs: None,
    };
    let projection = vcs_history_view(ws.vcs(), view_a, &ctx);
    let selection = projection.selection.expect("selection projection");
    assert_eq!(
        selection.before.kind,
        synvelis_workspace::EndpointKind::Committed
    );
    assert_eq!(
        selection.after.kind,
        synvelis_workspace::EndpointKind::Committed
    );
    assert!(selection.before.label.starts_with("aaaa"));
    let diff = selection.diff.expect("diff projection");
    assert_eq!(diff.overall, VcsEquality::Different);
    assert_ne!(diff.changes.len(), 0);
    assert!(
        diff.changes
            .iter()
            .any(|c| c.domain == synvelis_project::diff::ComparisonDomain::Project)
    );
}

#[test]
fn live_endpoint_goes_stale_when_revision_advances() {
    let (mut ws, adapter, _gen) = fixture();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn_history(&mut ws, win);
    let rev_before = ws.session().revision();

    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: Some(backend(&adapter)),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::Before,
                &EndpointRequest::Live,
                &cancel(),
            )
            .unwrap();
        views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::After,
                &EndpointRequest::Committed {
                    selector: RevisionSelector::Head,
                },
                &cancel(),
            )
            .unwrap();
        let pin = views
            .get(view)
            .unwrap()
            .before
            .as_ref()
            .unwrap()
            .pin
            .clone();
        assert!(matches!(
            endpoint_freshness(&pin, &ctx),
            EndpointFreshness::Fresh
        ));
    });
    apply(
        &mut ws,
        ProjectOp::SetProjectName {
            name: "vcs04-d".into(),
        },
    );
    assert_ne!(ws.session().revision(), rev_before);

    let ctx = VcsSession {
        backend: Some(backend(&adapter)),
        controller: ws.session().controller(),
        store: None,
        blobs: None,
    };
    let state = ws.vcs().get(view).unwrap();
    let pin = state.before.as_ref().unwrap().pin.clone();
    assert!(matches!(
        endpoint_freshness(&pin, &ctx),
        EndpointFreshness::Stale { .. }
    ));
    // the pinned snapshot is immutable: still the "vcs04-c" state
    let proj = state.before.as_ref().unwrap().snapshot.projection.project();
    assert_eq!(proj.name, "vcs04-c");

    let projection = vcs_history_view(ws.vcs(), view, &ctx);
    assert!(projection.new_revision_available);
    let selection = projection.selection.unwrap();
    assert!(matches!(
        selection.before.freshness,
        EndpointFreshness::Stale { .. }
    ));
}

#[test]
fn staged_endpoint_pins_and_detects_index_drift() {
    let (mut ws, adapter, generation) = fixture();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn_history(&mut ws, win);

    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: Some(backend(&adapter)),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::After,
                &EndpointRequest::Staged,
                &cancel(),
            )
            .unwrap();
        let state = views.get(view).unwrap();
        let after = state.after.as_ref().unwrap();
        match &after.pin {
            EndpointPin::Staged {
                index_generation, ..
            } => {
                assert_eq!(index_generation.0, "gen-1");
            }
            other => panic!("expected staged pin, got {other:?}"),
        }
        assert!(matches!(
            endpoint_freshness(&after.pin, &ctx),
            EndpointFreshness::Fresh
        ));
    });

    // index generation moves → staged pin is stale, never silently fresh
    *generation.lock().unwrap() = IndexGeneration("gen-2".into());
    let ctx = VcsSession {
        backend: Some(backend(&adapter)),
        controller: ws.session().controller(),
        store: None,
        blobs: None,
    };
    let pin = ws
        .vcs()
        .get(view)
        .unwrap()
        .after
        .as_ref()
        .unwrap()
        .pin
        .clone();
    assert!(matches!(
        endpoint_freshness(&pin, &ctx),
        EndpointFreshness::Stale { .. }
    ));
}

#[test]
fn merge_after_endpoint_records_parent_choice() {
    let (mut ws, adapter, _gen) = fixture();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn_history(&mut ws, win);

    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: Some(backend(&adapter)),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        views.refresh_history(view, &ctx, None, &cancel()).unwrap();
        views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::Before,
                &EndpointRequest::Node { index: 1 },
                &cancel(),
            )
            .unwrap();
        views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::After,
                &EndpointRequest::Node { index: 0 },
                &cancel(),
            )
            .unwrap();
        let state = views.get(view).unwrap();
        let chosen = state.chosen_parent.as_ref().expect("merge parent recorded");
        assert_eq!(chosen.commit_oid, oid('c'), "first parent must be explicit");
        // second parent is preserved on the node, not collapsed
        assert_eq!(state.nodes[0].parents.len(), 2);
    });

    let ctx = VcsSession {
        backend: Some(backend(&adapter)),
        controller: ws.session().controller(),
        store: None,
        blobs: None,
    };
    let projection = vcs_history_view(ws.vcs(), view, &ctx);
    let selection = projection.selection.unwrap();
    assert_eq!(selection.chosen_parent.as_deref(), Some("cccccccccccc"));
}

#[test]
fn committed_endpoint_selector_resolves_once_and_rejects_foreign() {
    let (mut ws, adapter, _gen) = fixture();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn_history(&mut ws, win);

    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: Some(backend(&adapter)),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        // Head resolves to the merge node "d" and pins its snapshot
        views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::After,
                &EndpointRequest::Committed {
                    selector: RevisionSelector::Head,
                },
                &cancel(),
            )
            .unwrap();
        let after = views.get(view).unwrap().after.as_ref().unwrap();
        match &after.pin {
            EndpointPin::Committed { revision } => {
                assert_eq!(revision.commit_oid, oid('d'));
                assert_eq!(revision.repository_id, REPO);
            }
            other => panic!("expected committed pin, got {other:?}"),
        }
        // foreign repository binding is rejected before any snapshot read
        let err = views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::Before,
                &EndpointRequest::Committed {
                    selector: RevisionSelector::Expr("foreign".into()),
                },
                &cancel(),
            )
            .unwrap_err();
        assert!(matches!(
            err,
            VcsUiError::Snapshot(SnapshotError::NotFound { .. })
        ));
        // saved endpoint without a store reports NoStore, not a guess
        let err = views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::Before,
                &EndpointRequest::Saved { head: None },
                &cancel(),
            )
            .unwrap_err();
        assert_eq!(err, VcsUiError::NoStore);
    });
}

#[test]
fn dock_moves_and_two_window_detach_preserve_vcs_state() {
    let (mut ws, adapter, _gen) = fixture();
    let win_a = ws.create_window("A", 1280, 800).unwrap();
    let win_b = ws.create_window("B", 1280, 800).unwrap();
    let view = spawn_history(&mut ws, win_a);

    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: Some(backend(&adapter)),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        views.refresh_history(view, &ctx, None, &cancel()).unwrap();
        views.load_more_history(view, &ctx, &cancel()).unwrap();
        views.select_node(view, 1).unwrap();
        views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::Before,
                &EndpointRequest::Node { index: 3 },
                &cancel(),
            )
            .unwrap();
        views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::After,
                &EndpointRequest::Live,
                &cancel(),
            )
            .unwrap();
    });

    let snapshot_state = |ws: &Ws| {
        let s = ws.vcs().get(view).unwrap();
        (
            s.nodes.len(),
            s.selected.clone(),
            s.before.as_ref().unwrap().pin.clone(),
            s.after.as_ref().unwrap().pin.clone(),
            s.current_diff.is_some(),
        )
    };
    let before = snapshot_state(&ws);
    assert!(before.4, "diff computed before move");

    // dock move A → B (two-window)
    ws.move_view(view, DockTarget::Window(win_b)).unwrap();
    assert_eq!(snapshot_state(&ws), before);
    // focus follows the dock: window B's active view is the history view
    ws.on_window_event(win_b, &HostEvent::Focused(true))
        .unwrap();
    assert_eq!(ws.active_view_in(win_b), Some(view));

    // detach to pool and reattach — identity and state survive
    ws.detach_to_pool(view).unwrap();
    assert_eq!(snapshot_state(&ws), before);
    ws.reattach(view, win_a).unwrap();
    assert_eq!(snapshot_state(&ws), before);

    // detach into a brand-new window (second OS window)
    let win_c = ws.detach_to_new_window(view, "history-detached").unwrap();
    assert_eq!(snapshot_state(&ws), before);
    assert_eq!(ws.dock().owner_of(view), Some(win_c));
}

#[test]
fn remove_view_and_release_views_clean_vcs_state() {
    let (mut ws, adapter, _gen) = fixture();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn_history(&mut ws, win);

    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: Some(backend(&adapter)),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        views.refresh_history(view, &ctx, None, &cancel()).unwrap();
    });
    assert!(ws.vcs().contains(view));

    // DetachViews (default): state survives window close into the pool
    ws.request_close(win);
    ws.on_window_event(win, &HostEvent::CloseRequested).unwrap();
    ws.on_window_event(win, &HostEvent::Destroyed).unwrap();
    assert!(ws.vcs().contains(view), "detach keeps vcs state");

    let win_b2 = ws.create_window("B", 1280, 800).unwrap();
    ws.reattach(view, win_b2).unwrap();
    ws.remove_view(view).unwrap();
    assert!(!ws.vcs().contains(view), "remove_view drops vcs state");

    // ReleaseViews: closing the window drops the view's vcs state
    let win2 = ws.create_window("C", 1280, 800).unwrap();
    let view2 = spawn_history(&mut ws, win2);
    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: Some(backend(&adapter)),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        views.refresh_history(view2, &ctx, None, &cancel()).unwrap();
    });
    assert!(ws.vcs().contains(view2));
    ws.set_close_disposition(win2, WindowCloseDisposition::ReleaseViews);
    ws.request_close(win2);
    ws.on_window_event(win2, &HostEvent::CloseRequested)
        .unwrap();
    ws.on_window_event(win2, &HostEvent::Destroyed).unwrap();
    assert!(!ws.vcs().contains(view2), "ReleaseViews drops vcs state");
}

#[test]
fn no_backend_reports_status_but_live_still_pins() {
    let mut ws = fresh_ws();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn_history(&mut ws, win);

    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: no_backend(),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        let err = views
            .refresh_history(view, &ctx, None, &cancel())
            .unwrap_err();
        assert_eq!(err, VcsUiError::NoBackend);
        // Live endpoint does not need the backend
        views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::Before,
                &EndpointRequest::Live,
                &cancel(),
            )
            .unwrap();
        assert!(views.get(view).unwrap().before.is_some());
    });

    let ctx = VcsSession {
        backend: no_backend(),
        controller: ws.session().controller(),
        store: None,
        blobs: None,
    };
    let projection = vcs_history_view(ws.vcs(), view, &ctx);
    assert_eq!(projection.status, VcsViewStatus::NoBackend);
    assert!(!projection.backend_attached);
}

#[test]
fn cancelled_ops_surface_cancelled_not_success() {
    let (mut ws, adapter, _gen) = fixture();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn_history(&mut ws, win);
    let cancelled = VcsCancel::new();
    cancelled.cancel();

    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: Some(backend(&adapter)),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        let err = views
            .refresh_history(view, &ctx, None, &cancelled)
            .unwrap_err();
        assert!(err.is_cancelled(), "expected cancelled, got {err:?}");
        let err = views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::Before,
                &EndpointRequest::Live,
                &cancelled,
            )
            .unwrap_err();
        assert!(err.is_cancelled());
        assert!(views.get(view).unwrap().before.is_none());
    });

    let ctx = VcsSession {
        backend: Some(backend(&adapter)),
        controller: ws.session().controller(),
        store: None,
        blobs: None,
    };
    let projection = vcs_history_view(ws.vcs(), view, &ctx);
    assert_eq!(projection.status, VcsViewStatus::Cancelled);
}

#[test]
fn history_bound_refuses_oversized_page() {
    let (mut ws, _adapter, _gen) = fixture();
    // Rebuild a provider whose pages exceed MAX_HISTORY_NODES.
    let generation = Arc::new(Mutex::new(IndexGeneration("gen-1".into())));
    let mut provider = FakeProvider::new(Arc::clone(&generation));
    let big: Vec<HistoryNode> = (0..600)
        .map(|i| node(&fixed_rev(REPO, &oid_n(i)), vec![]))
        .collect();
    provider.first_page = HistoryPage {
        nodes: big[..500].to_vec(),
        next_cursor: Some("rest".into()),
    };
    provider.rest_pages.insert(
        "rest".into(),
        HistoryPage {
            nodes: big[500..].to_vec(),
            next_cursor: None,
        },
    );
    let info = workspace_info();
    let adapter = VcsHistoryAdapter::new(provider, &info, PROJECT_PATH).expect("adapter");

    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn_history(&mut ws, win);
    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: Some(backend(&adapter)),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        views.refresh_history(view, &ctx, None, &cancel()).unwrap();
        assert_eq!(views.get(view).unwrap().nodes.len(), 500);
        let err = views.load_more_history(view, &ctx, &cancel()).unwrap_err();
        assert!(matches!(err, VcsUiError::LimitExceeded { .. }));
        let state = views.get(view).unwrap();
        assert_eq!(state.nodes.len(), 500, "no silent drop of oldest nodes");
        assert_eq!(state.next_cursor.as_deref(), Some("rest"));
        assert!(state.last_error.is_some());
    });
}

#[test]
fn view_mode_and_clear_selection_are_display_only() {
    let (mut ws, adapter, _gen) = fixture();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn_history(&mut ws, win);

    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: Some(backend(&adapter)),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        views.refresh_history(view, &ctx, None, &cancel()).unwrap();
        views.load_more_history(view, &ctx, &cancel()).unwrap();
        views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::Before,
                &EndpointRequest::Node { index: 3 },
                &cancel(),
            )
            .unwrap();
        views
            .pin_endpoint(
                view,
                &ctx,
                PinSide::After,
                &EndpointRequest::Node { index: 2 },
                &cancel(),
            )
            .unwrap();
        views.set_view_mode(view, VcsViewMode::ChangeFocus);
        assert_eq!(views.get(view).unwrap().view_mode, VcsViewMode::ChangeFocus);

        views.clear_selection(view);
        let state = views.get(view).unwrap();
        assert!(state.before.is_none() && state.after.is_none() && state.current_diff.is_none());
        assert_eq!(state.nodes.len(), 4, "history nodes preserved");
        // compare without both pins is an explicit request error
        assert!(matches!(
            views.compare_selection(view, &ctx, &cancel()),
            Err(VcsUiError::InvalidRequest { .. })
        ));
    });
}

#[test]
fn vcs_ops_never_touch_project_engine_or_transport() {
    let (mut ws, adapter, _gen) = fixture();
    let win = ws.create_window("A", 1280, 800).unwrap();
    let view = spawn_history(&mut ws, win);

    let rev0 = ws.session().revision();
    let builds0 = ws.session().engine().graph_builds();
    let name0 = ws.session().project().name.clone();
    let transport0 = (
        ws.session().transport().playing,
        ws.session().transport().position_ticks,
    );

    ws.with_vcs(|views, session| {
        let ctx = VcsSession {
            backend: Some(backend(&adapter)),
            controller: session.controller(),
            store: None,
            blobs: None,
        };
        views.refresh_history(view, &ctx, None, &cancel()).unwrap();
        views.load_more_history(view, &ctx, &cancel()).unwrap();
        views.select_node(view, 0).unwrap();
        for (side, req) in [
            (PinSide::Before, &EndpointRequest::Node { index: 3 }),
            (PinSide::After, &EndpointRequest::Live),
        ] {
            views
                .pin_endpoint(view, &ctx, side, req, &cancel())
                .unwrap();
        }
        views.compare_selection(view, &ctx, &cancel()).unwrap();
        views.set_view_mode(view, VcsViewMode::SideBySide);
        let _ = endpoint_freshness(&views.get(view).unwrap().before.as_ref().unwrap().pin, &ctx);
        let _ = vcs_history_view(views, view, &ctx);
    });

    assert_eq!(ws.session().revision(), rev0);
    assert_eq!(ws.session().engine().graph_builds(), builds0);
    assert_eq!(ws.session().project().name, name0);
    assert_eq!(
        (
            ws.session().transport().playing,
            ws.session().transport().position_ticks
        ),
        transport0
    );
}
