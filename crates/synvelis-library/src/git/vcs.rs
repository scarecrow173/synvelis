//! Git-backed read-only VCS provider (VCS-03, Synvelis Issue `#94`).
//!
//! [`GitVcsProvider`] binds one local worktree and implements both
//! [`synvelis_project::vcs::VcsReadProvider`] (fixed revisions,
//! merge-aware history, working status, index snapshots) and
//! [`synvelis_project::snapshot::VcsBytesPort`] (staged/committed
//! project + asset bytes for the VCS-01 snapshot resolver).
//!
//! Read-only by construction: every git invocation is a fixed-argv
//! plumbing command (`rev-parse`, `ls-tree`, `cat-file`, `ls-files`,
//! `check-attr`, `rev-list`, `status --porcelain`) run with prompts,
//! credential helpers, and optional index locks disabled. Nothing
//! here checks out, stages, commits, moves refs, or writes the index,
//! worktree, or HEAD. LFS pointer text is never served as content —
//! only bytes verified against the project digest (or the LFS
//! object's own SHA-256 identity) count as present.
//!
//! Git object IDs (SHA-1/SHA-256 hex) live in a different namespace
//! from project [`synvelis_project::snapshot::ContentDigest`] values;
//! this provider never compares or interchanges them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use synvelis_project::snapshot::{
    BlobRead, IndexGeneration, VcsBytesPort, VcsFileRead, VcsLocator, VcsPortError,
};
use synvelis_project::vcs::{
    FixedVcsRevision, HistoryCompleteness, HistoryNode, HistoryPage, HistoryQuery, IndexEntry,
    IndexEntryMode, ProjectPath, ProjectRelativePath, ProjectScope, RevisionRelations,
    RevisionSelector, StagedSnapshot, VcsCancel, VcsCapabilities, VcsError, VcsFeature,
    VcsObjectFormat, VcsReadProvider, WorkingChangeKind, WorkingStatus, WorkingStatusEntry,
    WorkspaceInfo,
};

use crate::context::{CancellationToken, ProviderContext};
use crate::error::{ProviderError, ProviderErrorKind};
use crate::git::objects::{
    GitEntryMode, GitRunner, GitTreeEntry, is_pointer_sized, parse_lfs_pointer, parse_ls_tree_zl,
};
use crate::git::url::GitRemoteUrl;
use crate::id::PrincipalId;

/// Default cap for a single file/blob read through the port.
const DEFAULT_MAX_BLOB_BYTES: u64 = 256 * 1024 * 1024;
/// Default history page size.
const DEFAULT_MAX_HISTORY_PAGE: usize = 100;
/// Hard cap on history page size regardless of configuration.
const HARD_MAX_HISTORY_PAGE: usize = 500;
/// Bound on a revision expression's text length.
const MAX_REV_EXPR_LEN: usize = 256;
/// `ls-files --stage` record bound (same spirit as `LS_TREE_MAX_*`).
const LS_FILES_MAX_BYTES: u64 = 64 * 1024 * 1024;
/// DOS device basenames never valid as blob path components on
/// Windows — mirrors `project::DOS_DEVICES`.
const DOS_DEVICES: &[&str] = &[
    "NUL", "CON", "AUX", "PRN", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Connection configuration for [`GitVcsProvider`].
#[derive(Clone, Debug)]
pub struct GitVcsConfig {
    /// Filesystem path inside (or at) the worktree — any directory
    /// git can discover a repository from.
    pub root: PathBuf,
    /// Repository-relative project directory (`""` = repository
    /// root). `None` derives it from `root`'s position under the
    /// worktree root.
    pub project_root: Option<String>,
    /// Repository-relative directory holding content-addressed asset
    /// blobs (the `blobs/` namespace convention: `<blob_root>/sha256/
    /// <64hex>` for `sha256:` digests). `None` defaults to
    /// `<project_root>/blobs` (`blobs` at repository root).
    pub blob_root: Option<String>,
    /// Maximum bytes returned by one file/blob read.
    pub max_blob_bytes: u64,
    /// Maximum history nodes per page (hard-capped).
    pub max_history_page: usize,
}

impl GitVcsConfig {
    /// A config rooted at `root` with default bounds.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            project_root: None,
            blob_root: None,
            max_blob_bytes: DEFAULT_MAX_BLOB_BYTES,
            max_history_page: DEFAULT_MAX_HISTORY_PAGE,
        }
    }
}

/// A read-only Git VCS provider bound to one local worktree.
///
/// Construct via [`GitVcsProvider::open`] (or
/// [`VcsReadProvider::detect_workspace`] for inspection first). The
/// provider is non-RT: calls may block on git subprocess I/O and are
/// bounded by [`GitVcsConfig`] limits plus [`VcsCancel`].
pub struct GitVcsProvider {
    runner: GitRunner,
    worktree: PathBuf,
    git_common: PathBuf,
    repository_id: String,
    object_format: VcsObjectFormat,
    /// Binding context stored on every [`FixedVcsRevision`] — the
    /// canonical common-dir path; foreign revisions fail the check.
    context: String,
    project_root: String,
    blob_root: String,
    principal: PrincipalId,
    max_blob_bytes: u64,
    max_history_page: usize,
}

/// What repository probing resolved.
struct Probe {
    worktree: PathBuf,
    git_dir: PathBuf,
    git_common: PathBuf,
    object_format: VcsObjectFormat,
    head: Option<String>,
    /// `root`'s path relative to the worktree root (`""` = root).
    project_root: String,
}

impl GitVcsProvider {
    /// Detects the repository containing `config.root` and binds a
    /// provider to it.
    ///
    /// # Errors
    /// [`VcsError::NotRepository`], [`VcsError::InvalidProjectPath`],
    /// [`VcsError::Unavailable`].
    pub fn open(config: &GitVcsConfig) -> Result<Self, VcsError> {
        let cx = detect_context()?;
        let root = ProjectPath::new(config.root.to_string_lossy().into_owned())?;
        let probe = probe_workspace(&root, &cx)?;
        let project_root = if let Some(p) = &config.project_root {
            ProjectScope::new(p.clone())?.as_str().to_owned()
        } else {
            // The probed path comes from the filesystem — validate
            // it through the same grammar as explicit config so a
            // hostile directory name cannot smuggle `\` or `..`.
            ProjectScope::new(probe.project_root.clone())?
                .as_str()
                .to_owned()
        };
        let blob_root = match &config.blob_root {
            Some(b) => {
                let scope = ProjectScope::new(b.clone())?;
                scope.as_str().to_owned()
            }
            None => default_blob_root(&project_root),
        };
        let remote = GitRemoteUrl::parse(probe.worktree.to_string_lossy().as_ref())
            .map_err(|e| unavailable(format!("worktree url: {e}")))?;
        let runner = GitRunner::new(remote, None);
        let context = probe.git_common.to_string_lossy().into_owned();
        let repository_id = format!("git:{}", hex_lower(&Sha256::digest(context.as_bytes())));
        Ok(Self {
            runner,
            worktree: probe.worktree,
            git_common: probe.git_common,
            repository_id,
            object_format: probe.object_format,
            context,
            project_root,
            blob_root,
            principal: vcs_principal()?,
            max_blob_bytes: config.max_blob_bytes,
            max_history_page: config.max_history_page.clamp(1, HARD_MAX_HISTORY_PAGE),
        })
    }

    /// The bound worktree root (canonical filesystem path).
    #[must_use]
    pub fn worktree_root(&self) -> &Path {
        &self.worktree
    }

    /// The bound project directory (repository-relative, `""` = root).
    #[must_use]
    pub fn project_root(&self) -> &str {
        &self.project_root
    }

    /// The bound asset blob directory (repository-relative).
    #[must_use]
    pub fn blob_root(&self) -> &str {
        &self.blob_root
    }

    /// Per-call provider context; `cancel` (when given) is bridged
    /// into the runner's cancellation token so in-flight git reads
    /// are interruptible.
    fn cx(&self, cancel: Option<&VcsCancel>) -> ProviderContext {
        let mut cx = ProviderContext::new(self.principal.clone());
        if let Some(c) = cancel {
            cx.cancel = CancellationToken::from_shared(c.shared_flag());
        }
        cx
    }

    /// Resolves a revision expression to a commit oid (fixed at call
    /// time). `expr` must already be validated.
    ///
    /// `--verify` runs *without* `--quiet`: an unresolvable revision
    /// (unborn HEAD, unknown ref, non-commit peel) exits non-zero with
    /// a fatal diagnostic on stderr, which `classify_kind` maps to
    /// `NotFound` — the `RevisionGone` path. `--quiet` would exit 1
    /// silently and misclassify as `TransientIo` (synvelis#335).
    fn resolve_oid(&self, expr: &str, cx: &ProviderContext) -> Result<String, VcsError> {
        let target = format!("{expr}^{{commit}}");
        let out = self
            .runner
            .run(
                Some(&self.worktree),
                &["rev-parse", "--verify", &target],
                cx,
            )
            .map_err(|e| match e.kind() {
                ProviderErrorKind::NotFound => VcsError::RevisionGone {
                    detail: format!("revision does not resolve: {expr}"),
                },
                _ => map_provider_error(&e, None),
            })?;
        let oid = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if !is_hex_oid(&oid, self.object_format) {
            return Err(VcsError::Unavailable {
                detail: format!("rev-parse returned malformed oid for {expr}"),
            });
        }
        Ok(oid)
    }

    /// Resolves the current `HEAD` commit, `None` on unborn HEAD.
    /// Same `--verify` (no `--quiet`) contract as [`Self::resolve_oid`]:
    /// unborn HEAD fails with `fatal: Needed a single revision` →
    /// `NotFound` → `Ok(None)`.
    fn head_oid(&self, cx: &ProviderContext) -> Result<Option<String>, VcsError> {
        match self.runner.run(
            Some(&self.worktree),
            &["rev-parse", "--verify", "HEAD^{commit}"],
            cx,
        ) {
            Ok(out) => {
                let oid = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                Ok((!oid.is_empty()).then_some(oid))
            }
            // Unborn HEAD (and any other unresolvable HEAD) is
            // affirmatively absent, not a backend failure.
            Err(e) if e.kind() == ProviderErrorKind::NotFound => Ok(None),
            Err(e) => Err(map_provider_error(&e, None)),
        }
    }

    /// Verifies `fixed` was minted by this binding (same object
    /// store, format, and project scope).
    fn check_binding(&self, fixed: &FixedVcsRevision) -> Result<(), VcsError> {
        if fixed.repository_id == self.repository_id
            && fixed.context == self.context
            && fixed.object_format == self.object_format
        {
            Ok(())
        } else {
            Err(VcsError::ForeignBinding {
                detail: "fixed revision belongs to a different repository or binding".to_owned(),
            })
        }
    }

    /// `fixed`'s commit oid after binding + format validation.
    fn fixed_oid<'a>(&self, fixed: &'a FixedVcsRevision) -> Result<&'a str, VcsError> {
        self.check_binding(fixed)?;
        if !is_hex_oid(&fixed.commit_oid, self.object_format) {
            return Err(VcsError::InvalidRequest {
                detail: "fixed revision commit oid does not match the object format".to_owned(),
            });
        }
        Ok(&fixed.commit_oid)
    }

    /// The repository-relative blob path for `digest`, mirroring the
    /// `blobs/` namespace convention (`sha256:<64hex>` →
    /// `<blob_root>/sha256/<hex>`; legacy filename-safe digests →
    /// `<blob_root>/<digest>`). `None` = unmappable digest.
    fn blob_relpath(&self, digest: &str) -> Option<String> {
        let prefix = if self.blob_root.is_empty() {
            String::new()
        } else {
            format!("{}/", self.blob_root)
        };
        if let Some(hex) = digest.strip_prefix("sha256:") {
            if hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Some(format!("{prefix}sha256/{}", hex.to_ascii_lowercase()));
            }
            return None;
        }
        let name_ok = !digest.is_empty()
            && digest.len() <= 255
            && digest
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            && !DOS_DEVICES.contains(&digest.to_ascii_uppercase().as_str());
        name_ok.then(|| format!("{prefix}{digest}"))
    }

    /// The expected SHA-256 hex for `digest`, when it is
    /// `sha256:`-form (enables content verification).
    fn expected_sha256_hex(digest: &str) -> Option<String> {
        digest
            .strip_prefix("sha256:")
            .filter(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .map(str::to_ascii_lowercase)
    }

    /// Pathspecs scoping index/generation reads to the bound project
    /// + blob directories. Empty result = whole index.
    fn index_scopes(&self) -> Vec<String> {
        let mut scopes: Vec<String> = Vec::new();
        if self.project_root.is_empty() || self.blob_root.is_empty() {
            // Project at repo root, or blobs at repo root: only the
            // whole index safely covers the project file set.
            return scopes;
        }
        scopes.push(self.project_root.clone());
        let blob_under_project = self.blob_root == self.project_root
            || self
                .blob_root
                .strip_prefix(&self.project_root)
                .is_some_and(|rest| rest.starts_with('/'));
        if !blob_under_project {
            scopes.push(self.blob_root.clone());
        }
        scopes
    }

    /// Fingerprint of the staged set under [`Self::index_scopes`].
    fn index_fingerprint(&self, cx: &ProviderContext) -> Result<IndexGeneration, VcsError> {
        let scopes = self.index_scopes();
        let scope_refs: Vec<&str> = scopes.iter().map(String::as_str).collect();
        let mut args: Vec<&str> = vec!["ls-files", "--stage", "-z"];
        if !scope_refs.is_empty() {
            args.push("--");
            args.extend(scope_refs.iter().copied());
        }
        let out = self
            .runner
            .run_stdout_bounded(Some(&self.worktree), &args, LS_FILES_MAX_BYTES, cx)
            .map_err(|e| map_provider_error(&e, None))?;
        if out.1 {
            return Err(VcsError::BudgetExceeded {
                detail: "index listing exceeded capture bound".to_owned(),
            });
        }
        Ok(IndexGeneration(format!(
            "lsfiles-sha256:{}",
            hex_lower(&Sha256::digest(&out.0))
        )))
    }

    /// All `ls-files --stage` records under `pathspecs` (empty =
    /// whole index).
    fn ls_files_stage(
        &self,
        pathspecs: &[String],
        cx: &ProviderContext,
    ) -> Result<(Vec<IndexRecord>, usize), VcsError> {
        let mut args: Vec<&str> = vec!["ls-files", "--stage", "-z"];
        let owned: Vec<&str> = pathspecs.iter().map(String::as_str).collect();
        if !owned.is_empty() {
            args.push("--");
            args.extend(owned.iter().copied());
        }
        let (bytes, truncated) = self
            .runner
            .run_stdout_bounded(Some(&self.worktree), &args, LS_FILES_MAX_BYTES, cx)
            .map_err(|e| map_provider_error(&e, None))?;
        if truncated {
            return Err(VcsError::BudgetExceeded {
                detail: "index listing exceeded capture bound".to_owned(),
            });
        }
        Ok(parse_ls_files_stage(&bytes))
    }

    /// Stage-0 index record for exactly `path`.
    fn index_entry(&self, path: &str, cx: &ProviderContext) -> Result<IndexLookup, VcsError> {
        let pathspec = format!(":(literal){path}");
        let (records, skipped) = self.ls_files_stage(&[pathspec], cx)?;
        if skipped > 0 {
            return Err(VcsError::Unavailable {
                detail: "index listing contained unparseable records".to_owned(),
            });
        }
        let mut stage0 = None;
        let mut conflicted = false;
        for rec in records {
            if rec.path != path {
                continue;
            }
            if rec.stage == 0 {
                stage0 = Some(rec);
            } else {
                conflicted = true;
            }
        }
        if conflicted {
            // Unmerged path: presence of file bytes cannot be
            // determined honestly.
            return Ok(IndexLookup::Conflict);
        }
        Ok(match stage0 {
            Some(rec) => IndexLookup::Record(rec),
            None => IndexLookup::Absent,
        })
    }

    /// `ls-tree` lookup of `path` inside commit `oid`.
    /// `Ok(None)` = affirmatively absent; `Ok(Some(entry))` = the
    /// single matching record.
    fn tree_entry(
        &self,
        oid: &str,
        path: &str,
        cx: &ProviderContext,
    ) -> Result<Option<GitTreeEntry>, VcsError> {
        let pathspec = format!(":(literal){path}");
        let args = ["ls-tree", "-z", "-l", oid, "--", pathspec.as_str()];
        let (bytes, truncated) = self
            .runner
            .run_stdout_bounded(Some(&self.worktree), &args, LS_FILES_MAX_BYTES, cx)
            .map_err(|e| map_provider_error(&e, Some("tree lookup")))?;
        if truncated {
            return Err(VcsError::BudgetExceeded {
                detail: "tree listing exceeded capture bound".to_owned(),
            });
        }
        let (entries, skipped) = parse_ls_tree_zl(&bytes);
        if skipped > 0 {
            return Err(VcsError::Unavailable {
                detail: "tree listing contained unparseable records".to_owned(),
            });
        }
        match entries.as_slice() {
            [] => Ok(None),
            [entry] if entry.name == path => Ok(Some(entry.clone())),
            _ => Err(VcsError::Unavailable {
                detail: "tree pathspec did not resolve to exactly one entry".to_owned(),
            }),
        }
    }

    /// Bounded `cat-file blob <oid>` read.
    fn cat_blob(&self, oid: &str, cx: &ProviderContext) -> Result<(Vec<u8>, bool), VcsError> {
        self.runner
            .run_stdout_bounded(
                Some(&self.worktree),
                &["cat-file", "blob", oid],
                self.max_blob_bytes,
                cx,
            )
            .map_err(|e| map_provider_error(&e, Some("blob read")))
    }

    /// `true` when `path` is LFS-tracked (`filter=lfs`). For committed
    /// reads attributes come from `rev` (`check-attr --source`); for
    /// staged reads the worktree `.gitattributes` applies. On check
    /// failure the answer fails *safe toward LFS* so pointer text can
    /// never be served as content.
    fn lfs_tracked(&self, rev: Option<&str>, path: &str, cx: &ProviderContext) -> bool {
        let args: Vec<String> = match rev {
            Some(sha) => vec!["check-attr", "--source", sha, "filter", "--", path]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            None => vec!["check-attr", "filter", "--", path]
                .into_iter()
                .map(str::to_owned)
                .collect(),
        };
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        match self.runner.run(Some(&self.worktree), &arg_refs, cx) {
            Ok(out) => String::from_utf8_lossy(&out.stdout)
                .lines()
                .any(|l| l.trim_end().ends_with(": filter: lfs")),
            Err(_) => true,
        }
    }

    /// Reads the local LFS object for `oid` (`<common>/lfs/objects/
    /// <a>/<b>/<oid>`). `Ok(None)` = affirmatively absent.
    fn lfs_object(&self, oid: &str, expected_size: u64) -> Result<Option<Vec<u8>>, VcsError> {
        if oid.len() != 64 || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(VcsError::Unavailable {
                detail: "malformed LFS object id".to_owned(),
            });
        }
        let path = self
            .git_common
            .join("lfs")
            .join("objects")
            .join(&oid[..2])
            .join(&oid[2..4])
            .join(oid);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(VcsError::Unavailable {
                    detail: format!("LFS object read failed: {e}"),
                });
            }
        };
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != expected_size {
            return Ok(None);
        }
        Ok(Some(bytes))
    }

    /// Reads blob `oid`'s *content* for `digest`, resolving LFS
    /// pointers against the local object store. `rev` selects the
    /// attribute source (`Some(commit)` committed, `None` staged).
    ///
    /// `Present` is returned only for bytes that verify: `sha256:`
    /// digests must hash-match; legacy digests are presence-verified.
    fn read_blob_content(
        &self,
        oid: &str,
        blob_path: &str,
        digest: &str,
        rev: Option<&str>,
        cx: &ProviderContext,
    ) -> Result<BlobRead, VcsError> {
        let (bytes, truncated) = self.cat_blob(oid, cx)?;
        if truncated {
            return Ok(BlobRead::Unknown);
        }
        // LFS pointer? Only pointer-sized blobs can be pointers, and
        // only when the path is actually LFS-tracked.
        if is_pointer_sized(bytes.len() as u64)
            && let Some(pointer) = parse_lfs_pointer(&bytes)
            && self.lfs_tracked(rev, blob_path, cx)
        {
            // The real content is the LFS object, never the pointer.
            let Some(lfs_bytes) = self.lfs_object(&pointer.oid, pointer.size)? else {
                return Ok(BlobRead::Absent);
            };
            return Ok(verify_digest(digest, lfs_bytes));
        }
        Ok(verify_digest(digest, bytes))
    }

    /// Resolves the object id backing `path` at `locator`'s pinned
    /// position (index stage-0 or committed tree entry).
    fn resolve_entry(
        &self,
        locator: &VcsLocator,
        path: &str,
        cx: &ProviderContext,
    ) -> Result<EntryResolution, VcsError> {
        match locator {
            VcsLocator::Staged { .. } => match self.index_entry(path, cx)? {
                IndexLookup::Absent => Ok(EntryResolution::Absent),
                IndexLookup::Conflict => Ok(EntryResolution::Unknown),
                IndexLookup::Record(rec) => match rec.mode {
                    IndexEntryMode::Regular
                    | IndexEntryMode::Executable
                    | IndexEntryMode::Symlink => Ok(EntryResolution::Blob {
                        oid: rec.object_id,
                        rev: None,
                    }),
                    IndexEntryMode::Gitlink => Ok(EntryResolution::Unknown),
                },
            },
            VcsLocator::Committed { fixed_revision, .. } => {
                if !is_hex_oid(fixed_revision, self.object_format) {
                    return Err(VcsError::Unavailable {
                        detail: "committed locator revision is not a fixed object id".to_owned(),
                    });
                }
                let Some(entry) = self.tree_entry(fixed_revision, path, cx)? else {
                    return Ok(EntryResolution::Absent);
                };
                match entry.mode {
                    GitEntryMode::Blob { .. } | GitEntryMode::Symlink => {
                        Ok(EntryResolution::Blob {
                            oid: entry.oid,
                            rev: Some(fixed_revision.clone()),
                        })
                    }
                    GitEntryMode::Tree | GitEntryMode::Gitlink => Ok(EntryResolution::Unknown),
                }
            }
        }
    }

    /// Parses one `cat-file commit <oid>` object.
    fn cat_commit(&self, oid: &str, cx: &ProviderContext) -> Result<CommitInfo, VcsError> {
        let (bytes, truncated) = self
            .runner
            .run_stdout_bounded(
                Some(&self.worktree),
                &["cat-file", "commit", oid],
                self.max_blob_bytes,
                cx,
            )
            .map_err(|e| map_provider_error(&e, Some("commit read")))?;
        if truncated {
            return Err(VcsError::BudgetExceeded {
                detail: "commit object exceeded read bound".to_owned(),
            });
        }
        parse_commit_object(&bytes).ok_or_else(|| VcsError::Unavailable {
            detail: "malformed commit object".to_owned(),
        })
    }

    /// `true` when commit object `oid` exists in the object store.
    fn commit_exists(&self, oid: &str, cx: &ProviderContext) -> bool {
        self.runner
            .run(Some(&self.worktree), &["cat-file", "-e", oid], cx)
            .is_ok()
    }
}

/// How a path resolves at a pinned locator position.
enum EntryResolution {
    /// Affirmatively absent (no index/tree entry).
    Absent,
    /// A blob/symlink object id; `rev` = `Some(commit)` for committed
    /// reads (attribute source), `None` for staged reads.
    Blob { oid: String, rev: Option<String> },
    /// Present but not readable as file bytes (gitlink, tree, or
    /// unmerged index state).
    Unknown,
}

/// Index lookup result for one path.
enum IndexLookup {
    /// No index record for the path.
    Absent,
    /// Unmerged (conflict) stages exist — presence undetermined.
    Conflict,
    /// The stage-0 record.
    Record(IndexRecord),
}

/// One parsed `ls-files --stage` record.
struct IndexRecord {
    path: String,
    stage: u32,
    mode: IndexEntryMode,
    object_id: String,
}

/// Parsed commit header fields needed for history nodes.
struct CommitInfo {
    parents: Vec<String>,
    author: Option<String>,
    committed_at_unix: Option<i64>,
    message: Option<String>,
}

impl VcsReadProvider for GitVcsProvider {
    fn detect_workspace(root: &ProjectPath) -> Result<WorkspaceInfo, VcsError> {
        let cx = detect_context()?;
        let probe = probe_workspace(root, &cx)?;
        let context = probe.git_common.to_string_lossy().into_owned();
        let repository_id = format!("git:{}", hex_lower(&Sha256::digest(context.as_bytes())));
        Ok(WorkspaceInfo {
            repository_id,
            object_format: probe.object_format,
            worktree_root: probe.worktree.to_string_lossy().into_owned(),
            git_dir: probe.git_dir.to_string_lossy().into_owned(),
            git_common_dir: probe.git_common.to_string_lossy().into_owned(),
            project_root: probe.project_root,
            head_commit: probe.head,
            capabilities: capabilities_for(probe.object_format),
        })
    }

    fn capabilities(&self) -> VcsCapabilities {
        capabilities_for(self.object_format)
    }

    fn working_status(
        &self,
        scope: &ProjectScope,
        cancel: &VcsCancel,
    ) -> Result<WorkingStatus, VcsError> {
        let cx = self.cx(Some(cancel));
        cx.check_interrupts()
            .map_err(|e| map_provider_error(&e, None))?;
        let mut args: Vec<String> = vec![
            "status",
            "--porcelain=v1",
            "-z",
            "--no-renames",
            "--untracked-files=all",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        if !scope.as_str().is_empty() {
            args.push("--".to_owned());
            args.push(format!(":(literal){}", scope.as_str()));
        }
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = self
            .runner
            .run(Some(&self.worktree), &arg_refs, &cx)
            .map_err(|e| map_provider_error(&e, None))?;
        cx.check_interrupts()
            .map_err(|e| map_provider_error(&e, None))?;
        let entries = parse_status_z(&out.stdout);
        let index_generation = self.index_fingerprint(&cx)?;
        let head_commit = self.head_oid(&cx)?;
        Ok(WorkingStatus {
            entries,
            index_generation,
            head_commit,
        })
    }

    fn list_history(
        &self,
        query: &HistoryQuery,
        cancel: &VcsCancel,
    ) -> Result<HistoryPage, VcsError> {
        let cx = self.cx(Some(cancel));
        if query.limit == 0 {
            return Err(VcsError::InvalidRequest {
                detail: "history limit must be non-zero".to_owned(),
            });
        }
        let limit = query.limit.min(self.max_history_page);
        // Resolve the start once — history from a fixed commit is
        // immutable, so an offset cursor stays valid for that start.
        let start_oid = match &query.start {
            Some(sel) => self.selector_oid(sel, &cx)?,
            None => self.head_oid(&cx)?.ok_or_else(|| VcsError::RevisionGone {
                detail: "HEAD is unborn; no history to list".to_owned(),
            })?,
        };
        let scope = query
            .path_scope
            .clone()
            .unwrap_or_else(|| ProjectScope::new(self.project_root.clone()).expect("bound scope"));
        let fingerprint = hex_lower(&Sha256::digest(format!(
            "{}|{start_oid}|{}",
            self.repository_id,
            scope.as_str()
        )))[..16]
            .to_owned();
        let offset = match &query.cursor {
            None => 0usize,
            Some(cursor) => decode_cursor(cursor, &fingerprint)?,
        };
        let mut args: Vec<String> = vec![
            "rev-list".to_owned(),
            "--topo-order".to_owned(),
            format!("--skip={offset}"),
            format!("--max-count={}", limit + 1),
            start_oid.clone(),
        ];
        if !scope.as_str().is_empty() {
            args.push("--".to_owned());
            args.push(format!(":(literal){}", scope.as_str()));
        }
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = self
            .runner
            .run(Some(&self.worktree), &arg_refs, &cx)
            .map_err(|e| map_provider_error(&e, Some("history walk")))?;
        cx.check_interrupts()
            .map_err(|e| map_provider_error(&e, None))?;
        let text = String::from_utf8_lossy(&out.stdout);
        let oids: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
        let has_more = oids.len() > limit;
        let page_oids = &oids[..oids.len().min(limit)];
        let mut nodes = Vec::with_capacity(page_oids.len());
        for oid in page_oids {
            cx.check_interrupts()
                .map_err(|e| map_provider_error(&e, None))?;
            nodes.push(self.history_node(oid, &cx)?);
        }
        let next_cursor =
            has_more.then(|| format!("v1:{}:{fingerprint}", offset + page_oids.len()));
        Ok(HistoryPage { nodes, next_cursor })
    }

    fn resolve_fixed_revision(
        &self,
        selector: &RevisionSelector,
        cancel: &VcsCancel,
    ) -> Result<FixedVcsRevision, VcsError> {
        let cx = self.cx(Some(cancel));
        let oid = self.selector_oid(selector, &cx)?;
        Ok(self.fixed(oid))
    }

    fn revision_relations(
        &self,
        fixed: &FixedVcsRevision,
        cancel: &VcsCancel,
    ) -> Result<RevisionRelations, VcsError> {
        let cx = self.cx(Some(cancel));
        let oid = self.fixed_oid(fixed)?.to_owned();
        let info = self.cat_commit(&oid, &cx).map_err(|e| match e {
            VcsError::Unavailable { .. } => VcsError::RevisionGone {
                detail: format!("commit {oid} does not resolve"),
            },
            other => other,
        })?;
        let parents = info.parents.iter().map(|p| self.fixed(p.clone())).collect();
        Ok(RevisionRelations {
            revision: fixed.clone(),
            parents,
        })
    }

    fn read_revision_file(
        &self,
        fixed: &FixedVcsRevision,
        path: &ProjectRelativePath,
        cancel: &VcsCancel,
    ) -> Result<Vec<u8>, VcsError> {
        let cx = self.cx(Some(cancel));
        let oid = self.fixed_oid(fixed)?.to_owned();
        let resolution = self.resolve_entry_committed(&oid, path.as_str(), &cx)?;
        match resolution {
            EntryResolution::Absent => Err(VcsError::MissingProjectFile {
                detail: format!("{} not present at {}", path.as_str(), oid),
            }),
            EntryResolution::Unknown => Err(VcsError::Unavailable {
                detail: format!("{} is not a readable file at {}", path.as_str(), oid),
            }),
            EntryResolution::Blob { oid: blob, .. } => {
                let (bytes, truncated) = self.cat_blob(&blob, &cx)?;
                if truncated {
                    return Err(VcsError::BudgetExceeded {
                        detail: "file read exceeded bound".to_owned(),
                    });
                }
                Ok(bytes)
            }
        }
    }

    fn resolve_staged_snapshot(&self, cancel: &VcsCancel) -> Result<StagedSnapshot, VcsError> {
        let cx = self.cx(Some(cancel));
        let scopes = self.index_scopes();
        let (records, skipped) = self.ls_files_stage(&scopes, &cx)?;
        let mut entries: Vec<IndexEntry> = Vec::with_capacity(records.len());
        let mut conflicted_set = std::collections::BTreeSet::new();
        for rec in records {
            if rec.stage != 0 {
                conflicted_set.insert(rec.path.clone());
            }
            entries.push(IndexEntry {
                path: rec.path,
                stage: rec.stage,
                mode: rec.mode,
                object_id: rec.object_id,
            });
        }
        let conflicted: Vec<String> = conflicted_set.into_iter().collect();
        entries.sort_by(|a, b| a.path.cmp(&b.path).then(a.stage.cmp(&b.stage)));
        let index_generation = self.index_fingerprint(&cx)?;
        let complete = skipped == 0 && conflicted.is_empty();
        Ok(StagedSnapshot {
            index_generation,
            entries,
            conflicted_paths: conflicted,
            complete,
        })
    }
}

impl GitVcsProvider {
    /// `resolve_entry` restricted to committed locators — used by
    /// `read_revision_file` where the locator is a [`FixedVcsRevision`]
    /// rather than a [`VcsLocator`].
    fn resolve_entry_committed(
        &self,
        commit_oid: &str,
        path: &str,
        cx: &ProviderContext,
    ) -> Result<EntryResolution, VcsError> {
        let Some(entry) = self.tree_entry(commit_oid, path, cx)? else {
            return Ok(EntryResolution::Absent);
        };
        match entry.mode {
            GitEntryMode::Blob { .. } | GitEntryMode::Symlink => Ok(EntryResolution::Blob {
                oid: entry.oid,
                rev: Some(commit_oid.to_owned()),
            }),
            GitEntryMode::Tree | GitEntryMode::Gitlink => Ok(EntryResolution::Unknown),
        }
    }

    /// Builds a [`FixedVcsRevision`] for `oid` under this binding.
    fn fixed(&self, oid: String) -> FixedVcsRevision {
        FixedVcsRevision {
            repository_id: self.repository_id.clone(),
            object_format: self.object_format,
            commit_oid: oid,
            project_root: self.project_root.clone(),
            context: self.context.clone(),
        }
    }

    /// Resolves a [`RevisionSelector`] to a commit oid.
    fn selector_oid(
        &self,
        selector: &RevisionSelector,
        cx: &ProviderContext,
    ) -> Result<String, VcsError> {
        match selector {
            RevisionSelector::Head => self.head_oid(cx)?.ok_or_else(|| VcsError::RevisionGone {
                detail: "HEAD is unborn".to_owned(),
            }),
            RevisionSelector::Expr(expr) => {
                validate_rev_expr(expr)?;
                self.resolve_oid(expr, cx)
            }
        }
    }

    /// Builds one [`HistoryNode`] for commit `oid`, verifying parent
    /// object presence for the completeness flag.
    fn history_node(&self, oid: &str, cx: &ProviderContext) -> Result<HistoryNode, VcsError> {
        let info = self.cat_commit(oid, cx)?;
        let mut parents = Vec::with_capacity(info.parents.len());
        let mut complete = true;
        for p in &info.parents {
            if !is_hex_oid(p, self.object_format) {
                complete = false;
                continue;
            }
            if !self.commit_exists(p, cx) {
                complete = false;
            }
            parents.push(self.fixed(p.clone()));
        }
        Ok(HistoryNode {
            revision: self.fixed(oid.to_owned()),
            parents,
            committed_at_unix: info.committed_at_unix,
            author: info.author,
            message: info.message,
            completeness: if complete {
                HistoryCompleteness::Complete
            } else {
                HistoryCompleteness::Partial
            },
        })
    }
}

impl VcsBytesPort for GitVcsProvider {
    fn index_generation(&self, repository_id: &str) -> Result<IndexGeneration, VcsPortError> {
        self.check_port_repo(repository_id)?;
        let cx = self.cx(None);
        self.index_fingerprint(&cx).map_err(port_error)
    }

    fn read_file(&self, locator: &VcsLocator, path: &str) -> Result<VcsFileRead, VcsPortError> {
        self.check_port_repo(locator_repository_id(locator))?;
        let rel = ProjectRelativePath::new(path).map_err(|e| VcsPortError::Unavailable {
            detail: e.to_string(),
        })?;
        let cx = self.cx(None);
        match self.resolve_entry(locator, rel.as_str(), &cx) {
            Ok(EntryResolution::Absent) => Ok(VcsFileRead::Absent),
            Ok(EntryResolution::Unknown) => Ok(VcsFileRead::Unknown),
            Ok(EntryResolution::Blob { oid, .. }) => {
                let (bytes, truncated) = self.cat_blob(&oid, &cx).map_err(port_error)?;
                if truncated {
                    return Err(VcsPortError::Unavailable {
                        detail: "file read exceeded bound".to_owned(),
                    });
                }
                Ok(VcsFileRead::Present(bytes))
            }
            Err(e) => Err(port_error(e)),
        }
    }

    fn read_blob(&self, locator: &VcsLocator, digest: &str) -> Result<BlobRead, VcsPortError> {
        self.check_port_repo(locator_repository_id(locator))?;
        let cx = self.cx(None);
        let Some(blob_path) = self.blob_relpath(digest) else {
            return Ok(BlobRead::Unknown);
        };
        match self.resolve_entry(locator, &blob_path, &cx) {
            Ok(EntryResolution::Absent) => Ok(BlobRead::Absent),
            Ok(EntryResolution::Unknown) => Ok(BlobRead::Unknown),
            Ok(EntryResolution::Blob { oid, rev }) => self
                .read_blob_content(&oid, &blob_path, digest, rev.as_deref(), &cx)
                .map_err(port_error),
            Err(e) => Err(port_error(e)),
        }
    }
}

/// `detect_workspace`/`open` context — no caller cancellation.
fn detect_context() -> Result<ProviderContext, VcsError> {
    Ok(ProviderContext::new(vcs_principal()?))
}

/// The provider's principal identity (static, always valid).
fn vcs_principal() -> Result<PrincipalId, VcsError> {
    PrincipalId::new("synvelis-vcs").map_err(|e| VcsError::Unavailable {
        detail: format!("principal init: {}", e.detail()),
    })
}

/// Default blob directory for a project root.
fn default_blob_root(project_root: &str) -> String {
    if project_root.is_empty() {
        "blobs".to_owned()
    } else {
        format!("{project_root}/blobs")
    }
}

/// `sha256` hex of `data`, lowercase.
fn hex_lower(digest: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(digest.len() * 2);
    for b in digest {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// `true` when `oid` is lowercase-hex of the format's length.
fn is_hex_oid(oid: &str, format: VcsObjectFormat) -> bool {
    oid.len() == format.oid_len()
        && oid
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Validates a caller-supplied revision expression: bounded, no
/// option-looking or control characters. Passed as a single argv
/// element, never through a shell.
fn validate_rev_expr(expr: &str) -> Result<(), VcsError> {
    let bad = |detail: &str| VcsError::InvalidRequest {
        detail: detail.to_owned(),
    };
    if expr.is_empty() || expr.len() > MAX_REV_EXPR_LEN {
        return Err(bad("revision expression is empty or over-long"));
    }
    if expr.starts_with('-')
        || expr
            .bytes()
            .any(|b| b == 0 || b.is_ascii_control() || b.is_ascii_whitespace())
        || expr.contains("..")
        || expr.contains("@{")
    {
        return Err(bad("revision expression is not allowed"));
    }
    Ok(())
}

/// Maps a [`ProviderError`] into [`VcsError`]; `context` tags the
/// operation for diagnostics. Details are already sanitized by the
/// runner's failure classification.
fn map_provider_error(e: &ProviderError, context: Option<&str>) -> VcsError {
    let detail = match context {
        Some(c) => format!("{c}: {:?}: {}", e.kind(), e.detail()),
        None => format!("{:?}: {}", e.kind(), e.detail()),
    };
    match e.kind() {
        ProviderErrorKind::Cancelled => VcsError::Cancelled,
        ProviderErrorKind::BudgetExceeded => VcsError::BudgetExceeded { detail },
        ProviderErrorKind::UnsupportedCapability => VcsError::UnsupportedCapability { detail },
        ProviderErrorKind::AuthExpired => VcsError::AuthExpired { detail },
        ProviderErrorKind::PermissionDenied => VcsError::PermissionDenied { detail },
        ProviderErrorKind::RateLimited => VcsError::RateLimited { detail },
        ProviderErrorKind::Offline => VcsError::Offline { detail },
        ProviderErrorKind::InvalidRequest | ProviderErrorKind::CursorInvalid => {
            VcsError::InvalidRequest { detail }
        }
        _ => VcsError::Unavailable { detail },
    }
}

/// `repository_id` carried by either locator variant.
fn locator_repository_id(locator: &VcsLocator) -> &str {
    match locator {
        VcsLocator::Staged { repository_id, .. } | VcsLocator::Committed { repository_id, .. } => {
            repository_id
        }
    }
}

/// [`VcsError`] → [`VcsPortError`] for the byte port (the resolver
/// only distinguishes not-found vs unavailable).
fn port_error(e: VcsError) -> VcsPortError {
    match e {
        VcsError::Unavailable { detail } | VcsError::InvalidRequest { detail } => {
            VcsPortError::Unavailable { detail }
        }
        other => VcsPortError::Unavailable {
            detail: other.to_string(),
        },
    }
}

impl GitVcsProvider {
    /// Shared repository-id check for [`VcsBytesPort`] calls.
    fn check_port_repo(&self, repository_id: &str) -> Result<(), VcsPortError> {
        if repository_id == self.repository_id {
            Ok(())
        } else {
            Err(VcsPortError::NotFound {
                detail: "unknown repository_id".to_owned(),
            })
        }
    }
}

/// `Present` only when `bytes` verify against `digest`: `sha256:`-
/// form digests must hash-match exactly; other digests are
/// presence-verified (the port contract cannot interpret them).
fn verify_digest(digest: &str, bytes: Vec<u8>) -> BlobRead {
    match GitVcsProvider::expected_sha256_hex(digest) {
        Some(expected) => {
            let actual = hex_lower(&Sha256::digest(&bytes));
            if actual == expected {
                BlobRead::Present(bytes)
            } else {
                BlobRead::Absent
            }
        }
        None => BlobRead::Present(bytes),
    }
}

/// Parses `git ls-files --stage -z` output.
/// Record: `<mode-octal> SP <oid-hex> SP <stage> TAB <path> NUL`.
fn parse_ls_files_stage(bytes: &[u8]) -> (Vec<IndexRecord>, usize) {
    let mut records = Vec::new();
    let mut skipped = 0usize;
    for record in bytes.split(|&b| b == 0) {
        if record.is_empty() {
            continue;
        }
        let Some(tab) = record.iter().position(|&b| b == b'\t') else {
            skipped += 1;
            continue;
        };
        let meta = &record[..tab];
        let path_bytes = &record[tab + 1..];
        let Ok(path) = std::str::from_utf8(path_bytes) else {
            skipped += 1;
            continue;
        };
        let Ok(meta) = std::str::from_utf8(meta) else {
            skipped += 1;
            continue;
        };
        let mut parts = meta.split_ascii_whitespace();
        let (Some(mode_s), Some(oid), Some(stage_s)) = (parts.next(), parts.next(), parts.next())
        else {
            skipped += 1;
            continue;
        };
        if parts.next().is_some() {
            skipped += 1;
            continue;
        }
        let mode = match mode_s {
            "100644" => IndexEntryMode::Regular,
            "100755" => IndexEntryMode::Executable,
            "120000" => IndexEntryMode::Symlink,
            "160000" => IndexEntryMode::Gitlink,
            _ => {
                skipped += 1;
                continue;
            }
        };
        let Ok(stage) = stage_s.parse::<u32>() else {
            skipped += 1;
            continue;
        };
        records.push(IndexRecord {
            path: path.to_owned(),
            stage,
            mode,
            object_id: oid.to_owned(),
        });
    }
    (records, skipped)
}

/// Parses `git status --porcelain=v1 -z --no-renames` output.
/// Record: `XY SP <path> NUL`; `??` = untracked.
fn parse_status_z(bytes: &[u8]) -> Vec<WorkingStatusEntry> {
    let mut entries = Vec::new();
    for record in bytes.split(|&b| b == 0) {
        if record.len() < 4 {
            continue;
        }
        let x = record[0];
        let y = record[1];
        if record[2] != b' ' {
            continue;
        }
        let Ok(path) = std::str::from_utf8(&record[3..]) else {
            continue;
        };
        if path.is_empty() {
            continue;
        }
        let untracked = x == b'?' && y == b'?';
        let conflicted = matches!((x, y), (b'U', _) | (_, b'U') | (b'A', b'A') | (b'D', b'D'));
        let index_change = status_kind(x);
        let worktree_change = if untracked { None } else { status_kind(y) };
        entries.push(WorkingStatusEntry {
            path: path.to_owned(),
            renamed_from: None,
            index_change,
            worktree_change,
            untracked,
            conflicted,
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

/// Maps one porcelain XY column to a change kind; `' '` and unknown
/// columns are both `None` (unchanged/ignored).
fn status_kind(c: u8) -> Option<WorkingChangeKind> {
    match c {
        b'M' | b'T' | b'R' | b'C' => Some(WorkingChangeKind::Modified),
        b'A' => Some(WorkingChangeKind::Added),
        b'D' => Some(WorkingChangeKind::Deleted),
        b'U' => Some(WorkingChangeKind::Unmerged),
        b'?' | b'!' => Some(WorkingChangeKind::Untracked),
        _ => None,
    }
}

/// Parses a `cat-file commit` object: headers to the first blank
/// line, message = the rest.
fn parse_commit_object(bytes: &[u8]) -> Option<CommitInfo> {
    let split = find_header_end(bytes)?;
    let header = &bytes[..split];
    let message = &bytes[split + 2..];
    let mut parents = Vec::new();
    let mut author = None;
    let mut committed_at_unix = None;
    for line in header.split(|&b| b == b'\n') {
        if let Some(rest) = line.strip_prefix(b"parent ") {
            if let Ok(p) = std::str::from_utf8(rest) {
                parents.push(p.to_owned());
            }
        } else if let Some(rest) = line.strip_prefix(b"author ") {
            author = std::str::from_utf8(rest).ok().map(ident_part);
        } else if let Some(rest) = line.strip_prefix(b"committer ") {
            committed_at_unix = std::str::from_utf8(rest)
                .ok()
                .and_then(|l| l.rsplit(' ').nth(1))
                .and_then(|ts| ts.parse::<i64>().ok());
        }
    }
    let message = if message.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(message).trim_end().to_owned())
    };
    Some(CommitInfo {
        parents,
        author,
        committed_at_unix,
        message,
    })
}

/// Byte offset just past the `\n\n` header/message separator.
fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(2).position(|w| w == b"\n\n")
}

/// `Name <email> ts tz` → `Name <email>`.
fn ident_part(ident: &str) -> String {
    // Strip the trailing ` <ts> <tz>` — ident itself may contain
    // spaces, so peel from the right twice.
    let mut parts = ident.rsplitn(3, ' ');
    let _tz = parts.next();
    let _ts = parts.next();
    parts.next().unwrap_or(ident).trim().to_owned()
}

/// Capability set for a detected object format.
fn capabilities_for(format: VcsObjectFormat) -> VcsCapabilities {
    let mut formats = BTreeSet::new();
    formats.insert(format);
    VcsCapabilities {
        object_formats: formats,
        history: VcsFeature::Supported,
        working_status: VcsFeature::Supported,
        staged_index: VcsFeature::Supported,
        lfs_objects: VcsFeature::Supported,
        submodule_entries: VcsFeature::Supported,
        cancellation: VcsFeature::Supported,
        resource_bounds: VcsFeature::Supported,
    }
}

/// Cursor codec: `v1:<offset>:<fingerprint16>`.
fn decode_cursor(cursor: &str, fingerprint: &str) -> Result<usize, VcsError> {
    let bad = || VcsError::InvalidRequest {
        detail: "history cursor is invalid for this query".to_owned(),
    };
    let mut parts = cursor.split(':');
    if parts.next() != Some("v1") {
        return Err(bad());
    }
    let offset = parts
        .next()
        .ok_or_else(bad)?
        .parse::<usize>()
        .map_err(|_| bad())?;
    if parts.next() != Some(fingerprint) || parts.next().is_some() {
        return Err(bad());
    }
    Ok(offset)
}

/// Probes the repository containing `root`: toplevel, git dirs,
/// object format, HEAD, and the suggested project root.
fn probe_workspace(root: &ProjectPath, cx: &ProviderContext) -> Result<Probe, VcsError> {
    let dir = Path::new(root.as_str());
    if !dir.is_dir() {
        return Err(VcsError::NotRepository {
            detail: format!("{} is not a directory", root.as_str()),
        });
    }
    // The runner's remote URL is unused for local object reads; the
    // worktree path only needs to be an existing directory for
    // GitRemoteUrl validation.
    let remote = GitRemoteUrl::parse(dir.to_string_lossy().as_ref())
        .map_err(|e| unavailable(format!("workspace path: {e}")))?;
    let runner = GitRunner::new(remote, None);
    let out = runner
        .run(
            Some(dir),
            &[
                "rev-parse",
                "--path-format=absolute",
                "--show-toplevel",
                "--git-dir",
                "--git-common-dir",
                "--show-object-format",
            ],
            cx,
        )
        .map_err(|e| match e.kind() {
            ProviderErrorKind::NotFound | ProviderErrorKind::InvalidRequest => {
                VcsError::NotRepository {
                    detail: format!("{} is not inside a git worktree", root.as_str()),
                }
            }
            _ => map_provider_error(&e, Some("workspace probe")),
        })?;
    let text = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() != 4 {
        return Err(VcsError::Unavailable {
            detail: "rev-parse probe returned unexpected output".to_owned(),
        });
    }
    let worktree = canonical(lines[0])?;
    let git_dir = canonical(lines[1])?;
    let git_common = canonical(lines[2])?;
    let object_format = match lines[3] {
        "sha1" => VcsObjectFormat::Sha1,
        "sha256" => VcsObjectFormat::Sha256,
        other => {
            return Err(VcsError::Unavailable {
                detail: format!("unsupported git object format {other:?}"),
            });
        }
    };
    let root_canon = dir.canonicalize().map_err(|e| VcsError::Unavailable {
        detail: format!("cannot canonicalize {}: {e}", dir.display()),
    })?;
    let project_root = rel_repo_path(&worktree, &root_canon)?;
    // HEAD is informational at probe time; unborn HEAD is not an
    // error (repos may have no commits yet). Every failure collapses
    // to `None` here — `--quiet` is irrelevant since the error is
    // discarded either way; `--verify` alone keeps the rev-parse
    // idiom uniform with `resolve_oid`/`head_oid`.
    let head = runner
        .run(Some(dir), &["rev-parse", "--verify", "HEAD^{commit}"], cx)
        .ok()
        .and_then(|o| {
            let oid = String::from_utf8_lossy(&o.stdout).trim().to_owned();
            (!oid.is_empty()).then_some(oid)
        });
    Ok(Probe {
        worktree,
        git_dir,
        git_common,
        object_format,
        head,
        project_root,
    })
}

/// Canonicalizes a probed path (already absolute).
fn canonical(path: &str) -> Result<PathBuf, VcsError> {
    Path::new(path)
        .canonicalize()
        .map_err(|e| VcsError::Unavailable {
            detail: format!("cannot canonicalize {path}: {e}"),
        })
}

/// `root`'s path relative to `worktree`, `/`-separated (`""` = same).
fn rel_repo_path(worktree: &Path, root: &Path) -> Result<String, VcsError> {
    let rel = root
        .strip_prefix(worktree)
        .map_err(|_| VcsError::NotRepository {
            detail: "project path escapes the worktree root".to_owned(),
        })?;
    if rel.as_os_str().is_empty() {
        return Ok(String::new());
    }
    let mut parts = Vec::new();
    for comp in rel.components() {
        match comp {
            std::path::Component::Normal(s) => parts.push(s.to_string_lossy().into_owned()),
            _ => {
                return Err(VcsError::Unavailable {
                    detail: "project path contains non-normal components".to_owned(),
                });
            }
        }
    }
    Ok(parts.join("/"))
}

/// [`VcsError::Unavailable`] shorthand.
fn unavailable(detail: String) -> VcsError {
    VcsError::Unavailable { detail }
}
