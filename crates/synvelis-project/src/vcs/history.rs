//! Read-only history/diff adapter (VCS-04, Issue `#95`).
//!
//! [`VcsHistoryBackend`] is the object-safe read surface the
//! History/Visual-Diff views consume: bounded history pages, one-shot
//! revision pinning, merge-parent relations, working status, index
//! generation, and committed/staged [`ImmutableSnapshot`] resolution
//! through the VCS-01 [`crate::snapshot::SnapshotResolver`].
//!
//! [`VcsHistoryAdapter`] binds one [`VcsReadProvider`] +
//! [`crate::snapshot::VcsBytesPort`] pair to a single
//! (`repository_id`, `project_path`) identity taken from
//! [`WorkspaceInfo`]. Every method is read-only — the trait has no
//! write surface by construction, and `ChangeSet` computation lives
//! in `synvelis-project::diff` / the workspace session cache, not
//! here.

use crate::snapshot::{
    ImmutableSnapshot, IndexGeneration, SnapshotCancel, SnapshotError, SnapshotLocator,
    SnapshotResolver, VcsBytesPort, VcsPortError,
};
use crate::vcs::types::{
    FixedVcsRevision, HistoryPage, HistoryQuery, ProjectRelativePath, ProjectScope,
    RevisionRelations, RevisionSelector, VcsCapabilities, VcsObjectFormat, WorkingStatus,
    WorkspaceInfo,
};
use crate::vcs::{VcsCancel, VcsError, VcsReadProvider};

/// Object-safe read surface the History/Visual-Diff views consume.
///
/// Implementations must be `Send + Sync` and non-real-time: every
/// method may block on backend I/O but must honor the supplied
/// cancellation token. There are deliberately no mutation methods —
/// checkout, stage, index update, ref movement, and worktree writes
/// are outside this contract.
pub trait VcsHistoryBackend: Send + Sync {
    /// Provider-side repository identity this backend is bound to.
    fn repository_id(&self) -> &str;

    /// The repository's object-id format.
    fn object_format(&self) -> VcsObjectFormat;

    /// Repository-relative project file path snapshot reads target.
    fn project_path(&self) -> &str;

    /// The scope history/status queries are restricted to.
    fn scope(&self) -> &ProjectScope;

    /// Declared provider capabilities for the bound workspace.
    fn capabilities(&self) -> VcsCapabilities;

    /// One bounded page of merge-aware history.
    ///
    /// # Errors
    /// [`VcsError::UnsupportedCapability`], [`VcsError::RevisionGone`],
    /// [`VcsError::InvalidRequest`], [`VcsError::Cancelled`],
    /// [`VcsError::BudgetExceeded`], [`VcsError::Unavailable`].
    fn list_history(
        &self,
        query: &HistoryQuery,
        cancel: &VcsCancel,
    ) -> Result<HistoryPage, VcsError>;

    /// Resolves `selector` exactly once to a [`FixedVcsRevision`].
    ///
    /// # Errors
    /// [`VcsError::RevisionGone`], [`VcsError::InvalidRequest`],
    /// [`VcsError::Cancelled`], [`VcsError::Unavailable`].
    fn resolve_fixed_revision(
        &self,
        selector: &RevisionSelector,
        cancel: &VcsCancel,
    ) -> Result<FixedVcsRevision, VcsError>;

    /// Parent relations of `fixed` — every merge parent preserved.
    ///
    /// # Errors
    /// [`VcsError::ForeignBinding`], [`VcsError::RevisionGone`],
    /// [`VcsError::Cancelled`], [`VcsError::Unavailable`].
    fn revision_relations(
        &self,
        fixed: &FixedVcsRevision,
        cancel: &VcsCancel,
    ) -> Result<RevisionRelations, VcsError>;

    /// Working-tree/index status under the bound scope.
    ///
    /// # Errors
    /// [`VcsError::UnsupportedCapability`], [`VcsError::Cancelled`],
    /// [`VcsError::Unavailable`].
    fn working_status(&self, cancel: &VcsCancel) -> Result<WorkingStatus, VcsError>;

    /// Current staged-set generation for the bound repository — the
    /// cheap staleness probe for staged pins (no full snapshot read).
    ///
    /// # Errors
    /// [`VcsPortError::NotFound`] for an unknown repository,
    /// [`VcsPortError::Unavailable`] when the generation cannot be read.
    fn index_generation(&self) -> Result<IndexGeneration, VcsPortError>;

    /// Resolves the project bytes committed at `revision` into an
    /// [`ImmutableSnapshot`] (`SnapshotOrigin::VcsCommitted`).
    ///
    /// # Errors
    /// [`SnapshotError::NotFound`] when `revision` belongs to a
    /// different repository, plus every [`crate::snapshot::SnapshotResolver`]
    /// failure (missing file, invalid content, cancellation, budget).
    fn resolve_committed(
        &self,
        revision: &FixedVcsRevision,
        cancel: &SnapshotCancel,
    ) -> Result<ImmutableSnapshot, SnapshotError>;

    /// Resolves the currently staged project bytes into an
    /// [`ImmutableSnapshot`] (`SnapshotOrigin::VcsStaged`), pinning the
    /// index generation observed inside the resolver's capture window.
    ///
    /// # Errors
    /// [`SnapshotError::IndexChanged`], [`SnapshotError::PartialStage`],
    /// [`SnapshotError::Cancelled`], [`SnapshotError::Vcs`], and the
    /// remaining resolver failure set.
    fn resolve_staged(&self, cancel: &SnapshotCancel) -> Result<ImmutableSnapshot, SnapshotError>;
}

/// [`VcsHistoryBackend`] over a [`VcsReadProvider`] +
/// [`VcsBytesPort`] pair bound to one detected workspace.
///
/// `P` is the concrete provider type (e.g. a Git adapter or a test
/// fake); the adapter adds only the snapshot-resolution glue —
/// committed/staged [`SnapshotLocator`] construction, foreign
/// repository rejection, and scope binding — on top of verbatim
/// provider passthroughs.
pub struct VcsHistoryAdapter<P> {
    provider: P,
    resolver: SnapshotResolver,
    repository_id: String,
    object_format: VcsObjectFormat,
    project_path: String,
    scope: ProjectScope,
}

impl<P> VcsHistoryAdapter<P>
where
    P: VcsReadProvider + VcsBytesPort,
{
    /// Binds `provider` to the workspace `info` describes.
    /// `project_path` is the repository-relative project *file* path
    /// snapshot reads target (e.g. `project/main.json`).
    ///
    /// # Errors
    /// [`VcsError::InvalidProjectPath`] on a malformed `project_path`
    /// or `info.project_root`.
    pub fn new(
        provider: P,
        info: &WorkspaceInfo,
        project_path: impl Into<String>,
    ) -> Result<Self, VcsError> {
        let project_path = ProjectRelativePath::new(project_path.into())?;
        Ok(Self {
            provider,
            resolver: SnapshotResolver::new(),
            repository_id: info.repository_id.clone(),
            object_format: info.object_format,
            project_path: project_path.as_str().to_owned(),
            scope: ProjectScope::new(info.project_root.clone())?,
        })
    }
}

impl<P> VcsHistoryBackend for VcsHistoryAdapter<P>
where
    P: VcsReadProvider + VcsBytesPort,
{
    fn repository_id(&self) -> &str {
        &self.repository_id
    }

    fn object_format(&self) -> VcsObjectFormat {
        self.object_format
    }

    fn project_path(&self) -> &str {
        &self.project_path
    }

    fn scope(&self) -> &ProjectScope {
        &self.scope
    }

    fn capabilities(&self) -> VcsCapabilities {
        self.provider.capabilities()
    }

    fn list_history(
        &self,
        query: &HistoryQuery,
        cancel: &VcsCancel,
    ) -> Result<HistoryPage, VcsError> {
        self.provider.list_history(query, cancel)
    }

    fn resolve_fixed_revision(
        &self,
        selector: &RevisionSelector,
        cancel: &VcsCancel,
    ) -> Result<FixedVcsRevision, VcsError> {
        self.provider.resolve_fixed_revision(selector, cancel)
    }

    fn revision_relations(
        &self,
        fixed: &FixedVcsRevision,
        cancel: &VcsCancel,
    ) -> Result<RevisionRelations, VcsError> {
        self.provider.revision_relations(fixed, cancel)
    }

    fn working_status(&self, cancel: &VcsCancel) -> Result<WorkingStatus, VcsError> {
        self.provider.working_status(&self.scope, cancel)
    }

    fn index_generation(&self) -> Result<IndexGeneration, VcsPortError> {
        self.provider.index_generation(&self.repository_id)
    }

    fn resolve_committed(
        &self,
        revision: &FixedVcsRevision,
        cancel: &SnapshotCancel,
    ) -> Result<ImmutableSnapshot, SnapshotError> {
        if revision.repository_id != self.repository_id {
            return Err(SnapshotError::NotFound {
                detail: format!(
                    "fixed revision belongs to repository {}, this adapter is bound to {}",
                    revision.repository_id, self.repository_id
                ),
            });
        }
        self.resolver.resolve_vcs(
            &self.provider,
            &SnapshotLocator::VcsCommitted {
                repository_id: self.repository_id.clone(),
                fixed_revision: revision.commit_oid.clone(),
                project_path: self.project_path.clone(),
            },
            cancel,
        )
    }

    fn resolve_staged(&self, cancel: &SnapshotCancel) -> Result<ImmutableSnapshot, SnapshotError> {
        self.resolver.resolve_vcs(
            &self.provider,
            &SnapshotLocator::VcsStaged {
                repository_id: self.repository_id.clone(),
                project_path: self.project_path.clone(),
                index_generation: None,
            },
            cancel,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::vcs::types::{
        HistoryCompleteness, HistoryNode, IndexEntry, IndexEntryMode, ProjectPath, StagedSnapshot,
        VcsFeature, WorkingStatusEntry,
    };
    use crate::{
        BlobRead, Command, CounterIdGenerator, FixedSnapshotLocator, Project, ProjectController,
        ProjectId, ProjectOp, SCHEMA_VERSION, SnapshotEnvelope, SnapshotOrigin, VcsFileRead,
        VcsLocator,
    };

    // -- fixtures ---------------------------------------------------------

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
            repository_id: "repo-1".into(),
            object_format: VcsObjectFormat::Sha1,
            worktree_root: "/tmp/wt".into(),
            git_dir: "/tmp/wt/.git".into(),
            git_common_dir: "/tmp/wt/.git".into(),
            project_root: String::new(),
            head_commit: Some(oid('a')),
            capabilities: capabilities(),
        }
    }

    fn controller() -> ProjectController {
        ProjectController::new(
            Project::new(ProjectId::new(0), "p"),
            Box::new(CounterIdGenerator::new(1)),
        )
    }

    fn envelope_bytes(c: &ProjectController) -> Vec<u8> {
        let envelope = SnapshotEnvelope {
            schema_version: SCHEMA_VERSION,
            project_id: c.live().project_id,
            revision: c.live_revision(),
            project: c.live().clone(),
        };
        serde_json::to_vec(&envelope).expect("envelope serialize")
    }

    fn oid(ch: char) -> String {
        ch.to_string().repeat(40)
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

    type Journal = Arc<Mutex<Vec<String>>>;

    /// In-memory provider + byte port. `files`/`blobs` are keyed
    /// `"{kind}:{rev_or_staged}:{path|digest}"`; every read is journaled
    /// so tests can assert the read-only call surface.
    struct FakeProvider {
        info: Option<WorkspaceInfo>,
        generation: IndexGeneration,
        files: BTreeMap<String, Vec<u8>>,
        blobs: BTreeMap<String, Vec<u8>>,
        history: HistoryPage,
        journal: Journal,
    }

    impl FakeProvider {
        fn new() -> (Self, Journal) {
            let journal: Journal = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    info: Some(workspace_info()),
                    generation: IndexGeneration("gen-1".into()),
                    files: BTreeMap::new(),
                    blobs: BTreeMap::new(),
                    history: HistoryPage {
                        nodes: Vec::new(),
                        next_cursor: None,
                    },
                    journal: Arc::clone(&journal),
                },
                journal,
            )
        }

        fn file_key(locator: &VcsLocator, path: &str) -> String {
            match locator {
                VcsLocator::Staged { .. } => format!("staged:{path}"),
                VcsLocator::Committed { fixed_revision, .. } => {
                    format!("committed:{fixed_revision}:{path}")
                }
            }
        }

        fn blob_key(locator: &VcsLocator, digest: &str) -> String {
            match locator {
                VcsLocator::Staged { .. } => format!("staged:{digest}"),
                VcsLocator::Committed { fixed_revision, .. } => {
                    format!("committed:{fixed_revision}:{digest}")
                }
            }
        }

        fn put_file(&mut self, locator: &VcsLocator, path: &str, bytes: Vec<u8>) {
            self.files.insert(Self::file_key(locator, path), bytes);
        }

        fn record(&self, call: String) {
            self.journal.lock().expect("journal").push(call);
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
            _cancel: &VcsCancel,
        ) -> Result<WorkingStatus, VcsError> {
            self.record("working_status".into());
            Ok(WorkingStatus {
                entries: vec![WorkingStatusEntry {
                    path: "project/main.json".into(),
                    renamed_from: None,
                    index_change: None,
                    worktree_change: None,
                    untracked: false,
                    conflicted: false,
                }],
                index_generation: self.generation.clone(),
                head_commit: Some(oid('a')),
            })
        }

        fn list_history(
            &self,
            _query: &HistoryQuery,
            _cancel: &VcsCancel,
        ) -> Result<HistoryPage, VcsError> {
            self.record("list_history".into());
            Ok(self.history.clone())
        }

        fn resolve_fixed_revision(
            &self,
            _selector: &RevisionSelector,
            _cancel: &VcsCancel,
        ) -> Result<FixedVcsRevision, VcsError> {
            self.record("resolve_fixed_revision".into());
            Ok(fixed_rev("repo-1", &oid('b')))
        }

        fn revision_relations(
            &self,
            fixed: &FixedVcsRevision,
            _cancel: &VcsCancel,
        ) -> Result<RevisionRelations, VcsError> {
            self.record("revision_relations".into());
            Ok(RevisionRelations {
                revision: fixed.clone(),
                parents: vec![fixed_rev("repo-1", &oid('a'))],
            })
        }

        fn read_revision_file(
            &self,
            _fixed: &FixedVcsRevision,
            _path: &ProjectRelativePath,
            _cancel: &VcsCancel,
        ) -> Result<Vec<u8>, VcsError> {
            self.record("read_revision_file".into());
            Ok(Vec::new())
        }

        fn resolve_staged_snapshot(&self, _cancel: &VcsCancel) -> Result<StagedSnapshot, VcsError> {
            self.record("resolve_staged_snapshot".into());
            Ok(StagedSnapshot {
                index_generation: self.generation.clone(),
                entries: vec![IndexEntry {
                    path: "project/main.json".into(),
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
            self.record(format!("index_generation:{repository_id}"));
            if repository_id != "repo-1" {
                return Err(VcsPortError::NotFound {
                    detail: "unknown repository_id".into(),
                });
            }
            Ok(self.generation.clone())
        }

        fn read_file(&self, locator: &VcsLocator, path: &str) -> Result<VcsFileRead, VcsPortError> {
            let key = Self::file_key(locator, path);
            self.record(format!("read_file:{key}"));
            Ok(self
                .files
                .get(&key)
                .cloned()
                .map_or(VcsFileRead::Absent, VcsFileRead::Present))
        }

        fn read_blob(&self, locator: &VcsLocator, digest: &str) -> Result<BlobRead, VcsPortError> {
            let key = Self::blob_key(locator, digest);
            self.record(format!("read_blob:{key}"));
            Ok(self
                .blobs
                .get(&key)
                .cloned()
                .map_or(BlobRead::Absent, BlobRead::Present))
        }
    }

    fn adapter(provider: FakeProvider) -> (VcsHistoryAdapter<FakeProvider>, Journal) {
        let journal = Arc::clone(&provider.journal);
        let info = provider.info.clone().expect("info");
        (
            VcsHistoryAdapter::new(provider, &info, "project/main.json").expect("adapter"),
            journal,
        )
    }

    fn staged_locator() -> VcsLocator {
        VcsLocator::Staged {
            repository_id: "repo-1".into(),
            project_path: "project/main.json".into(),
            index_generation: IndexGeneration("gen-1".into()),
        }
    }

    fn committed_locator(rev: &str) -> VcsLocator {
        VcsLocator::Committed {
            repository_id: "repo-1".into(),
            fixed_revision: rev.into(),
            project_path: "project/main.json".into(),
        }
    }

    // -- tests ------------------------------------------------------------

    #[test]
    fn committed_resolve_returns_vcs_committed_snapshot() {
        let mut c = controller();
        let bytes_a = envelope_bytes(&c);
        c.apply(Command {
            expected_revision: c.live_revision(),
            op: ProjectOp::SetProjectName {
                name: "renamed".into(),
            },
        })
        .expect("rename");
        let bytes_b = envelope_bytes(&c);

        let (mut provider, _) = FakeProvider::new();
        provider.put_file(&committed_locator(&oid('a')), "project/main.json", bytes_a);
        provider.put_file(&committed_locator(&oid('b')), "project/main.json", bytes_b);
        let (adapter, _) = adapter(provider);

        let snap_a = adapter
            .resolve_committed(&fixed_rev("repo-1", &oid('a')), &SnapshotCancel::new())
            .expect("resolve a");
        assert_eq!(snap_a.origin, SnapshotOrigin::VcsCommitted);
        assert_eq!(
            snap_a.identity.locator,
            FixedSnapshotLocator::VcsCommitted {
                repository_id: "repo-1".into(),
                fixed_revision: oid('a'),
                project_path: "project/main.json".into(),
            }
        );
        assert_eq!(
            snap_a.completeness,
            crate::snapshot::SnapshotCompleteness::Complete
        );

        let snap_b = adapter
            .resolve_committed(&fixed_rev("repo-1", &oid('b')), &SnapshotCancel::new())
            .expect("resolve b");
        assert_ne!(snap_a.identity.digest, snap_b.identity.digest);
        assert_eq!(snap_b.projection.project().name, "renamed");
    }

    #[test]
    fn staged_resolve_pins_observed_index_generation() {
        let c = controller();
        let bytes = envelope_bytes(&c);
        let (mut provider, _) = FakeProvider::new();
        provider.put_file(&staged_locator(), "project/main.json", bytes);
        let (adapter, _) = adapter(provider);

        let snap = adapter
            .resolve_staged(&SnapshotCancel::new())
            .expect("staged resolve");
        assert_eq!(snap.origin, SnapshotOrigin::VcsStaged);
        assert_eq!(
            snap.identity.locator,
            FixedSnapshotLocator::VcsStaged {
                repository_id: "repo-1".into(),
                project_path: "project/main.json".into(),
                index_generation: IndexGeneration("gen-1".into()),
            }
        );
        assert_eq!(
            adapter.index_generation().expect("gen"),
            IndexGeneration("gen-1".into())
        );
    }

    #[test]
    fn foreign_repository_revision_is_rejected() {
        let (mut provider, _) = FakeProvider::new();
        provider.put_file(
            &committed_locator(&oid('a')),
            "project/main.json",
            envelope_bytes(&controller()),
        );
        let (adapter, _) = adapter(provider);
        let err = adapter
            .resolve_committed(&fixed_rev("other-repo", &oid('a')), &SnapshotCancel::new())
            .expect_err("foreign repo must fail");
        assert!(matches!(err, SnapshotError::NotFound { .. }));
    }

    #[test]
    fn history_and_status_pass_through_verbatim() {
        let (mut provider, _) = FakeProvider::new();
        provider.history = HistoryPage {
            nodes: vec![HistoryNode {
                revision: fixed_rev("repo-1", &oid('b')),
                parents: vec![fixed_rev("repo-1", &oid('a'))],
                committed_at_unix: Some(1_700_000_000),
                author: Some("a".into()),
                message: Some("msg".into()),
                completeness: HistoryCompleteness::Complete,
            }],
            next_cursor: Some("cursor-1".into()),
        };
        let (adapter, _) = adapter(provider);

        let page = adapter
            .list_history(&HistoryQuery::default(), &VcsCancel::new())
            .expect("history");
        assert_eq!(page.next_cursor.as_deref(), Some("cursor-1"));
        assert_eq!(page.nodes.len(), 1);
        assert_eq!(page.nodes[0].parents.len(), 1);

        let status = adapter.working_status(&VcsCancel::new()).expect("status");
        assert_eq!(status.head_commit, Some(oid('a')));
        assert_eq!(status.entries.len(), 1);

        let fixed = adapter
            .resolve_fixed_revision(&RevisionSelector::Head, &VcsCancel::new())
            .expect("fixed");
        assert_eq!(fixed.commit_oid, oid('b'));

        let relations = adapter
            .revision_relations(&fixed, &VcsCancel::new())
            .expect("relations");
        assert_eq!(relations.parents.len(), 1);

        assert_eq!(adapter.repository_id(), "repo-1");
        assert_eq!(adapter.project_path(), "project/main.json");
        assert_eq!(adapter.object_format(), VcsObjectFormat::Sha1);
        assert!(adapter.capabilities().history.is_supported());
    }

    #[test]
    fn cancelled_resolve_returns_cancelled() {
        let c = controller();
        let (mut provider, _) = FakeProvider::new();
        provider.put_file(
            &committed_locator(&oid('a')),
            "project/main.json",
            envelope_bytes(&c),
        );
        let (adapter, _) = adapter(provider);
        let cancel = SnapshotCancel::new();
        cancel.cancel();
        let err = adapter
            .resolve_committed(&fixed_rev("repo-1", &oid('a')), &cancel)
            .expect_err("cancelled");
        assert_eq!(err, SnapshotError::Cancelled);
    }

    #[test]
    fn adapter_exposes_only_read_calls() {
        let c = controller();
        let bytes = envelope_bytes(&c);
        let (mut provider, journal) = FakeProvider::new();
        provider.put_file(
            &committed_locator(&oid('a')),
            "project/main.json",
            bytes.clone(),
        );
        provider.put_file(&staged_locator(), "project/main.json", bytes);
        let (adapter, _) = adapter(provider);

        let _ = adapter
            .resolve_committed(&fixed_rev("repo-1", &oid('a')), &SnapshotCancel::new())
            .expect("committed");
        let _ = adapter
            .resolve_staged(&SnapshotCancel::new())
            .expect("staged");
        let _ = adapter.index_generation().expect("gen");
        let _ = adapter.working_status(&VcsCancel::new()).expect("status");
        let _ = adapter
            .list_history(&HistoryQuery::default(), &VcsCancel::new())
            .expect("history");
        let _ = adapter
            .resolve_fixed_revision(&RevisionSelector::Head, &VcsCancel::new())
            .expect("fixed");
        let _ = adapter
            .revision_relations(&fixed_rev("repo-1", &oid('b')), &VcsCancel::new())
            .expect("relations");

        // Every journaled call is a read — the fake has no write
        // methods at all, and the journal names only reads.
        let calls = journal.lock().expect("journal").clone();
        assert_ne!(calls.len(), 0);
        for call in &calls {
            assert!(
                call.starts_with("index_generation")
                    || call.starts_with("read_file")
                    || call.starts_with("read_blob")
                    || [
                        "working_status",
                        "list_history",
                        "resolve_fixed_revision",
                        "revision_relations",
                        "read_revision_file",
                        "resolve_staged_snapshot",
                    ]
                    .contains(&call.as_str()),
                "unexpected call: {call}"
            );
        }
    }
}
