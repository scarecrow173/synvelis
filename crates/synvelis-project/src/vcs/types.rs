//! Provider-neutral VCS read types: fixed revisions, history nodes,
//! working status, index entries, and capability declarations.
//!
//! These types are the *contract* every [`crate::vcs::VcsReadProvider`]
//! implements. They deliberately stay free of Git-specific concepts
//! where the contract allows it: `object_format` distinguishes SHA-1
//! from SHA-256 object IDs, `commit_oid` is always an *object ID* in
//! the provider's own format — never a [`crate::snapshot::ContentDigest`]
//! (project content SHA-256). The two namespaces must not be compared
//! or interchanged.
//!
//! All paths named `*_root`/`project_path`/`path` in this module are
//! repository-relative, `/`-separated, and never absolute.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::snapshot::IndexGeneration;

/// The object-id format a VCS repository uses for commits/trees/blobs.
///
/// This is the *provider object* namespace (Git SHA-1 vs SHA-256). It is
/// unrelated to the project asset [`crate::snapshot::ContentDigest`],
/// which is always SHA-256 over content bytes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VcsObjectFormat {
    /// 40-hex-digit object IDs.
    Sha1,
    /// 64-hex-digit object IDs.
    Sha256,
}

impl VcsObjectFormat {
    /// The object-id hex length for this format.
    #[must_use]
    pub const fn oid_len(self) -> usize {
        match self {
            Self::Sha1 => 40,
            Self::Sha256 => 64,
        }
    }

    /// The lowercase format label (`"sha1"` / `"sha256"`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        }
    }
}

/// A repository-relative directory path used to scope VCS operations.
///
/// `""` means the repository root. Components reject `.`, `..`, empty
/// segments, `\`, control characters, and over-long names so a scope can
/// never escape the repository grammar.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectScope(String);

impl ProjectScope {
    /// The repository root scope.
    #[must_use]
    pub fn root() -> Self {
        Self(String::new())
    }

    /// Validates and wraps a repository-relative directory path.
    ///
    /// # Errors
    /// [`crate::vcs::VcsError::InvalidProjectPath`] on malformed input.
    pub fn new(path: impl Into<String>) -> Result<Self, crate::vcs::VcsError> {
        let path = path.into();
        if path.is_empty() {
            return Ok(Self(path));
        }
        validate_rel_path(&path, true)?;
        Ok(Self(path))
    }

    /// The raw repository-relative path (`""` = repository root).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A repository-relative path to a single file inside a revision.
///
/// Unlike [`ProjectScope`], the path must be non-empty and names a file
/// (or symlink) entry — never a bare directory.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectRelativePath(String);

impl ProjectRelativePath {
    /// Validates and wraps a repository-relative file path.
    ///
    /// # Errors
    /// [`crate::vcs::VcsError::InvalidProjectPath`] on malformed input.
    pub fn new(path: impl Into<String>) -> Result<Self, crate::vcs::VcsError> {
        let path = path.into();
        validate_rel_path(&path, false)?;
        Ok(Self(path))
    }

    /// The raw repository-relative path.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A filesystem path used to detect a VCS workspace.
///
/// May point at the worktree root or any directory inside it; the
/// provider resolves the containing repository itself.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectPath(String);

impl ProjectPath {
    /// Validates and wraps a filesystem path (absolute or relative to
    /// the process working directory).
    ///
    /// # Errors
    /// [`crate::vcs::VcsError::InvalidProjectPath`] on empty/NUL input.
    pub fn new(path: impl Into<String>) -> Result<Self, crate::vcs::VcsError> {
        let path = path.into();
        if path.is_empty() || path.contains('\0') {
            return Err(crate::vcs::VcsError::InvalidProjectPath {
                detail: "project path is empty or contains NUL".to_owned(),
            });
        }
        Ok(Self(path))
    }

    /// The raw filesystem path text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Shared repository-relative path validation: `/`-separated components,
/// no absolute forms, traversal, `\`, or control characters.
fn validate_rel_path(path: &str, allow_empty: bool) -> Result<(), crate::vcs::VcsError> {
    let bad = |detail: &str| crate::vcs::VcsError::InvalidProjectPath {
        detail: detail.to_owned(),
    };
    if path.is_empty() {
        return if allow_empty {
            Ok(())
        } else {
            Err(bad("path is empty"))
        };
    }
    if path.len() > 4096 {
        return Err(bad("path exceeds length bound"));
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(bad("path must be repository-relative"));
    }
    if path.ends_with('/') {
        return Err(bad("path must not end with '/'"));
    }
    for comp in path.split('/') {
        if comp.is_empty() || comp == "." || comp == ".." {
            return Err(bad("path has an empty or traversal component"));
        }
        if comp.len() > 255 {
            return Err(bad("path component exceeds length bound"));
        }
        if comp
            .bytes()
            .any(|b| b == b'\\' || b == 0 || b.is_ascii_control())
        {
            return Err(bad("path component has a forbidden character"));
        }
    }
    Ok(())
}

/// A mutable-or-fixed revision selector resolved exactly once.
///
/// [`RevisionSelector::Expr`] accepts any revision expression the
/// provider understands (`main`, `HEAD~2`, `v1.0`, a full object id,
/// ...). The provider resolves it to a concrete commit object id at
/// call time; afterwards only [`FixedVcsRevision`] values — which pin
/// the immutable commit — may be used for reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RevisionSelector {
    /// The repository's current `HEAD`.
    Head,
    /// A provider revision expression resolved once to a commit.
    Expr(String),
}

/// An immutable, fully-resolved VCS revision.
///
/// Every read that must stay pinned to one commit goes through this
/// value. `commit_oid` is an object id in `object_format` — *not* a
/// project [`crate::snapshot::ContentDigest`]. `repository_id` +
/// `context` bind the revision to one object store and one project
/// subtree so revisions from different repositories or project paths
/// can never be confused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixedVcsRevision {
    /// Provider-defined repository identity (stable per object store).
    pub repository_id: String,
    /// The repository's object-id format.
    pub object_format: VcsObjectFormat,
    /// The fixed commit object id (hex, `object_format.oid_len()`).
    pub commit_oid: String,
    /// Repository-relative project directory this revision is scoped
    /// to (`""` = repository root).
    pub project_root: String,
    /// Provider-defined opaque binding context (e.g. the canonical
    /// object-store path for a local Git provider). Used to reject
    /// revisions presented to a different binding.
    pub context: String,
}

impl FixedVcsRevision {
    /// `true` when `oid` has the hex shape of this revision's object
    /// format. Used by providers to reject non-fixed (mutable or
    /// malformed) revision strings before touching the object store.
    #[must_use]
    pub fn oid_matches_format(&self, oid: &str) -> bool {
        oid.len() == self.object_format.oid_len()
            && oid
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
}

/// Parent/child relations of one fixed revision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionRelations {
    /// The revision itself.
    pub revision: FixedVcsRevision,
    /// Every parent commit, in commit order. Merge commits keep all
    /// parents — first-parent-only history is not representable here.
    pub parents: Vec<FixedVcsRevision>,
}

/// Whether a history node's neighborhood was fully enumerated.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HistoryCompleteness {
    /// The node and all of its parents were resolved.
    Complete,
    /// Some parent object could not be confirmed present (e.g. a
    /// shallow or partially-fetched object store). The node itself is
    /// still fixed and readable.
    Partial,
}

/// One commit in provider history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryNode {
    /// The fixed revision for this commit.
    pub revision: FixedVcsRevision,
    /// Every parent revision (all merge parents preserved).
    pub parents: Vec<FixedVcsRevision>,
    /// Committer timestamp, seconds since the Unix epoch.
    pub committed_at_unix: Option<i64>,
    /// Author identity as recorded by the provider (opaque text).
    pub author: Option<String>,
    /// Commit message (full body as recorded by the provider).
    pub message: Option<String>,
    /// Whether the parent neighborhood is fully known.
    pub completeness: HistoryCompleteness,
}

/// A bounded history listing request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryQuery {
    /// Where to start. `None` = the provider's current `HEAD`.
    pub start: Option<RevisionSelector>,
    /// Maximum nodes per page.
    pub limit: usize,
    /// Opaque continuation cursor from a previous [`HistoryPage`].
    /// Bound to the query that minted it; a mismatched cursor is an
    /// error, never silently reinterpreted.
    pub cursor: Option<String>,
    /// Repository-relative path prefix restricting history to commits
    /// touching that subtree. `None` = the bound project root.
    pub path_scope: Option<ProjectScope>,
}

impl Default for HistoryQuery {
    fn default() -> Self {
        Self {
            start: None,
            limit: 50,
            cursor: None,
            path_scope: None,
        }
    }
}

/// One page of history results.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryPage {
    /// Nodes in provider traversal order (newest first).
    pub nodes: Vec<HistoryNode>,
    /// Cursor for the next page, `None` at the end.
    pub next_cursor: Option<String>,
}

/// A per-path change classification from working-status inspection.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkingChangeKind {
    /// Added relative to the compared baseline.
    Added,
    /// Content or mode modified relative to the baseline.
    Modified,
    /// Removed relative to the baseline.
    Deleted,
    /// Unmerged index entry (merge conflict in progress).
    Unmerged,
    /// Not tracked by the index at all.
    Untracked,
}

/// One path's working-tree/index state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingStatusEntry {
    /// Repository-relative path.
    pub path: String,
    /// For rename/copy records, the original path. Always `None` for
    /// providers that do not detect renames.
    pub renamed_from: Option<String>,
    /// Index-side (staged) change, `None` when unchanged there.
    pub index_change: Option<WorkingChangeKind>,
    /// Worktree-side change relative to the index, `None` when
    /// unchanged there.
    pub worktree_change: Option<WorkingChangeKind>,
    /// `true` for untracked paths (`??` records).
    pub untracked: bool,
    /// `true` when the index holds unmerged stages for this path.
    pub conflicted: bool,
}

/// Working-tree status for a scoped subtree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkingStatus {
    /// Changed paths under the requested scope, sorted by path.
    pub entries: Vec<WorkingStatusEntry>,
    /// Fingerprint of the staged set at observation time (same
    /// semantics as [`crate::snapshot::IndexGeneration`]).
    pub index_generation: IndexGeneration,
    /// The resolved `HEAD` commit oid at observation time, if any.
    pub head_commit: Option<String>,
}

/// The recorded mode of an index entry.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IndexEntryMode {
    /// Regular non-executable file (`100644`).
    Regular,
    /// Executable file (`100755`).
    Executable,
    /// Symbolic link (`120000`).
    Symlink,
    /// Submodule/gitlink (`160000`).
    Gitlink,
}

/// One index (staging area) record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    /// Repository-relative path.
    pub path: String,
    /// Index stage (0 = merged/normal, 1–3 = unmerged conflict stages).
    pub stage: u32,
    /// Recorded object mode.
    pub mode: IndexEntryMode,
    /// Staged object id in the repository's [`VcsObjectFormat`].
    pub object_id: String,
}

/// A fixed view of the index (staging area) under a scope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedSnapshot {
    /// Fingerprint of the staged set this snapshot describes.
    pub index_generation: IndexGeneration,
    /// All index records under the scope, sorted by `(path, stage)`.
    pub entries: Vec<IndexEntry>,
    /// Paths holding unmerged (conflict) stages.
    pub conflicted_paths: Vec<String>,
    /// `false` when the index could not be fully enumerated (unmerged
    /// entries, undecodable records). Consumers must treat `false` as
    /// degraded, never as "clean".
    pub complete: bool,
}

/// Whether an optional provider capability is available.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum VcsFeature {
    /// The capability is implemented and honest.
    Supported,
    /// The capability is not implemented; `reason` explains why.
    /// Callers must not fake the behavior.
    Unsupported {
        /// Human-readable reason.
        reason: String,
    },
}

impl VcsFeature {
    /// `true` for [`VcsFeature::Supported`].
    #[must_use]
    pub const fn is_supported(&self) -> bool {
        matches!(self, Self::Supported)
    }
}

/// Declared capabilities of a [`crate::vcs::VcsReadProvider`].
///
/// Git-specific features (index, merge DAG) appear here as *optional*
/// capabilities — providers that lack them report
/// [`VcsFeature::Unsupported`] and their trait methods return
/// [`crate::vcs::VcsError::UnsupportedCapability`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VcsCapabilities {
    /// Object-id formats this repository can read.
    pub object_formats: BTreeSet<VcsObjectFormat>,
    /// Merge-aware commit history listing.
    pub history: VcsFeature,
    /// Working-tree/index status inspection.
    pub working_status: VcsFeature,
    /// Fixed index (staging area) snapshots.
    pub staged_index: VcsFeature,
    /// Git-LFS pointer resolution against the *local* object store.
    /// Remote LFS fetch is a separate concern.
    pub lfs_objects: VcsFeature,
    /// Submodule/gitlink entries are detected and reported but their
    /// contents are never fetched.
    pub submodule_entries: VcsFeature,
    /// Cooperative cancellation is honored at operation boundaries.
    pub cancellation: VcsFeature,
    /// Hard resource bounds exist (page sizes, blob bytes, output
    /// caps). Unbounded providers must not claim this.
    pub resource_bounds: VcsFeature,
}

/// What [`crate::vcs::VcsReadProvider::detect_workspace`] found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    /// Stable repository identity for this object store.
    pub repository_id: String,
    /// The repository's object-id format.
    pub object_format: VcsObjectFormat,
    /// Canonical worktree root (filesystem path).
    pub worktree_root: String,
    /// Filesystem path of the git directory (per-worktree metadata).
    pub git_dir: String,
    /// Filesystem path of the shared git common directory (object
    /// store, refs, LFS objects).
    pub git_common_dir: String,
    /// Repository-relative project directory bound to this workspace
    /// (`""` = repository root).
    pub project_root: String,
    /// Resolved `HEAD` commit oid at detection time, if any.
    pub head_commit: Option<String>,
    /// Declared capabilities for this workspace.
    pub capabilities: VcsCapabilities,
}
