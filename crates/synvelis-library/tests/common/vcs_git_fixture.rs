//! Local-worktree git fixture for the VCS-03 read-provider tests.
//!
//! Builds a plain (non-bare) repository entirely through the `git`
//! CLI — no remote, no network, no git-lfs binary. LFS objects are
//! laid down manually under `<repo>/.git/lfs/objects/<fanout>/<oid>`,
//! the same layout `git lfs` produces, so the provider's local LFS
//! read path is exercised without the tool.
//!
//! The fixture *may* mutate the repo (commits, branch moves, staging)
//! — those are test setup. The provider under test must never mutate;
//! the read-only oracle in `vcs03_git_provider.rs` proves it.

use std::fs;
use std::path::{Path, PathBuf};

use synvelis_library::{GitVcsConfig, GitVcsProvider};
use synvelis_project::vcs::{ProjectPath, VcsReadProvider};

use super::TempTree;
use super::git_fixture::{git, git_allow_fail, git_available, plain_path};

/// A built local git worktree fixture.
pub struct VcsGitFixture {
    /// Owns every path below.
    pub tmp: TempTree,
    /// The worktree root (repository toplevel).
    pub repo: PathBuf,
}

impl VcsGitFixture {
    /// A SHA-1-object-format repo, or `None` without git.
    pub fn new() -> Option<Self> {
        Self::build(&[])
    }

    /// A SHA-256-object-format repo (`git init --object-format=sha256`).
    pub fn new_sha256() -> Option<Self> {
        Self::build(&["--object-format=sha256"])
    }

    fn build(extra_init: &[&str]) -> Option<Self> {
        if !git_available() {
            return None;
        }
        let tmp = TempTree::new("vcsgit");
        let repo = tmp.join("repo");
        fs::create_dir_all(&repo).expect("repo dir");

        let mut init = vec!["init", "--initial-branch=main"];
        init.extend_from_slice(extra_init);
        git(&repo, &init);
        git(&repo, &["config", "user.email", "fixture@test"]);
        git(&repo, &["config", "user.name", "fixture"]);
        git(&repo, &["config", "commit.gpgsign", "false"]);
        git(&repo, &["config", "core.autocrlf", "false"]);
        // Neutralize ambient git-lfs exactly like the ALIB-04
        // fixture: pointer blobs must commit verbatim and no
        // git-lfs binary may be required.
        git(&repo, &["config", "filter.lfs.clean", "cat"]);
        git(&repo, &["config", "filter.lfs.smudge", "cat"]);
        git(&repo, &["config", "filter.lfs.process", ""]);
        git(&repo, &["config", "filter.lfs.required", "false"]);
        let no_hooks = tmp.join("no-hooks");
        fs::create_dir_all(&no_hooks).expect("no-hooks dir");
        let hooks_path = no_hooks.to_string_lossy().replace('\\', "/");
        git(&repo, &["config", "core.hooksPath", hooks_path.as_str()]);

        Some(Self { tmp, repo })
    }

    /// The worktree root.
    pub fn path(&self) -> &Path {
        &self.repo
    }

    /// Writes `rel` (with parent dirs) holding `bytes`.
    pub fn write(&self, rel: impl AsRef<Path>, bytes: &[u8]) {
        let path = self.repo.join(rel.as_ref());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dirs");
        }
        fs::write(&path, bytes).expect("fixture write");
    }

    /// Removes `rel` from the worktree.
    pub fn remove(&self, rel: impl AsRef<Path>) {
        fs::remove_file(self.repo.join(rel.as_ref())).expect("fixture remove");
    }

    /// Runs `git <args>` in the worktree; panics on failure.
    pub fn git(&self, args: &[&str]) -> String {
        git(&self.repo, args)
    }

    /// Runs `git <args>` allowing failure.
    pub fn git_allow_fail(&self, args: &[&str]) -> std::process::Output {
        git_allow_fail(&self.repo, args)
    }

    /// `git rev-parse HEAD`.
    pub fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"])
    }

    /// Stages `rel` (`git add -- <rel>`).
    pub fn stage(&self, rel: &str) {
        self.git(&["add", "--", rel]);
    }

    /// `git add -A` + commit; returns the new HEAD sha.
    pub fn commit_all(&self, msg: &str) -> String {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-m", msg]);
        self.head()
    }

    /// The repo's git common dir (`.git` for a normal worktree).
    pub fn git_common(&self) -> PathBuf {
        self.repo.join(".git")
    }

    /// Lays down a local LFS object for `oid` (64-hex sha256) with
    /// `content` — `<git-common>/lfs/objects/<a>/<b>/<oid>`.
    pub fn write_lfs_object(&self, oid: &str, content: &[u8]) {
        assert_eq!(oid.len(), 64, "lfs oid must be sha256 hex");
        let dir = self
            .git_common()
            .join("lfs")
            .join("objects")
            .join(&oid[..2])
            .join(&oid[2..4]);
        fs::create_dir_all(&dir).expect("lfs dir");
        fs::write(dir.join(oid), content).expect("lfs object");
    }

    /// An LFS pointer text naming `oid`/`size`.
    pub fn lfs_pointer(oid: &str, size: usize) -> String {
        format!("version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {size}\n")
    }

    /// A provider bound to `project_root` (default `"proj"`).
    pub fn provider(&self) -> GitVcsProvider {
        self.provider_at("proj")
    }

    /// A provider bound to an explicit repository-relative
    /// `project_root`.
    pub fn provider_at(&self, project_root: &str) -> GitVcsProvider {
        GitVcsProvider::open(&GitVcsConfig {
            root: plain_path(&self.repo),
            project_root: Some(project_root.to_owned()),
            blob_root: None,
            max_blob_bytes: 64 * 1024 * 1024,
            max_history_page: 100,
        })
        .expect("fixture provider")
    }

    /// The `repository_id` a provider bound here reports.
    pub fn repository_id(&self) -> String {
        let info = <GitVcsProvider as VcsReadProvider>::detect_workspace(
            &ProjectPath::new(plain_path(&self.repo).to_string_lossy().into_owned()).expect("path"),
        )
        .expect("detect");
        info.repository_id
    }
}
