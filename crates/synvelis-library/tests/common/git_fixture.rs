//! Offline git fixture builder for the ALIB-04 provider tests.
//!
//! Builds a real work repo + bare `file://` remote entirely through
//! the `git` CLI — no git-sdk, no network. LFS objects are laid down
//! manually under `<remote>/lfs/objects/<fanout>/<oid>` (the same
//! layout `git lfs` produces) so the `git-lfs` binary is not required;
//! the provider reads that store directly.
//!
//! `GitFixture::new` returns `None` when `git` is unavailable so
//! callers can skip honestly instead of failing on machines without
//! git.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

use synvelis_library::GitRemoteUrl;

use super::TempTree;

/// A built fixture: work repo, bare remote, and the remote's
/// `file://` URL.
pub struct GitFixture {
    /// Owns every path below.
    pub tmp: TempTree,
    /// The non-bare working repository.
    pub work: PathBuf,
    /// The bare remote (`remote.git`).
    pub remote: PathBuf,
    /// `file://` URL of the remote — the transport the provider uses.
    pub remote_url: String,
    /// `main`'s commit sha after the fixture commits.
    pub head_sha: String,
    /// Whether the `link.txt` symlink fixture could be created
    /// (platform permission dependent).
    symlink_made: bool,
}

/// `true` when a `git` binary is runnable.
pub fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The plain (non-verbatim) spelling of `p` for git CLI arguments.
/// `TempTree` roots are canonicalized, which on Windows yields
/// `\\?\C:\…` verbatim paths — `git -C` tolerates them via chdir but
/// git's own path arguments (`init --bare <path>`, `remote add`)
/// reject the prefix with "Invalid argument".
pub fn plain_path(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        p.to_path_buf()
    }
}

/// Runs `git <args>` in `dir`; returns stdout on success, panics with
/// stderr on failure (fixture construction must be deterministic).
pub fn git(dir: &Path, args: &[&str]) -> String {
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

/// Runs `git <args>` in `dir` allowing failure; returns the output.
pub fn git_allow_fail(dir: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(plain_path(dir))
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?} spawn failed: {e}"))
}

/// The deterministic LFS payload bytes (shared by pointer creation
/// and object layout).
pub fn lfs_payload() -> Vec<u8> {
    (0..4096u32)
        .map(|i| u8::try_from((i.wrapping_mul(2_654_435_761) >> 11) & 0xff).unwrap())
        .collect()
}

/// SHA-256 hex of `bytes` — names the fixture's LFS object.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

impl GitFixture {
    /// Builds the standard fixture, or `None` without git.
    ///
    /// Layout after build:
    /// - `work/` — repo on branch `main` with `notes.txt`,
    ///   `dir/top.txt`, `dir/sub dir/f ile.txt`,
    ///   `payload.bin` (an LFS pointer), `.gitattributes`,
    ///   `vendor/sub` (gitlink), and `link.txt` (symlink, when the
    ///   platform allows).
    /// - `remote.git/` — bare remote, `HEAD` → `refs/heads/main`,
    ///   `uploadpack.allowFilter` + `allowAnySHA1InWant` enabled,
    ///   `lfs/objects/…` holding the real `payload.bin` bytes.
    pub fn new() -> Option<Self> {
        Self::build(true)
    }

    /// A fixture whose remote refuses `--filter` (no
    /// `uploadpack.allowFilter`) — for the
    /// [`synvelis_library::UnsupportedFilterPolicy`] tests.
    pub fn new_unfiltered() -> Option<Self> {
        Self::build(false)
    }

    fn build(allow_filter: bool) -> Option<Self> {
        if !git_available() {
            return None;
        }
        let tmp = TempTree::new("gitfix");
        let work = tmp.join("work");
        let remote = tmp.join("remote.git");
        fs::create_dir_all(&work).expect("work dir");

        git(&work, &["init", "--initial-branch=main"]);
        git(&work, &["config", "user.email", "fixture@test"]);
        git(&work, &["config", "user.name", "fixture"]);
        git(&work, &["config", "commit.gpgsign", "false"]);
        // Byte-exact content: no CRLF translation, no autocrlf.
        git(&work, &["config", "core.autocrlf", "false"]);

        // Neutralize ambient git-lfs: a global `git lfs install`
        // (template hooks + filter.lfs.* config) would otherwise run
        // `git-lfs clean` on `*.bin` — rewriting our pointer blob —
        // and `git lfs pre-push` would try uploading LFS objects.
        // `cat` passes content through verbatim; an empty hooksPath
        // disables every hook. The provider under test reads the
        // committed pointer text and the remote's lfs/objects store
        // directly — no git-lfs binary involvement anywhere.
        git(&work, &["config", "filter.lfs.clean", "cat"]);
        git(&work, &["config", "filter.lfs.smudge", "cat"]);
        git(&work, &["config", "filter.lfs.process", ""]);
        git(&work, &["config", "filter.lfs.required", "false"]);
        let no_hooks = tmp.join("no-hooks");
        fs::create_dir_all(&no_hooks).expect("no-hooks dir");
        git(
            &work,
            &[
                "config",
                "core.hooksPath",
                &no_hooks.to_string_lossy().replace('\\', "/"),
            ],
        );

        // LFS payload: real bytes live only under
        // remote/lfs/objects; the committed blob is the pointer.
        let (pointer, symlink_made) = seed_worktree(&tmp);

        git(&work, &["add", "-A"]);
        git(&work, &["commit", "-m", "fixture: base"]);
        let base_sha = git(&work, &["rev-parse", "HEAD"]);

        // Gitlink (submodule) entry pointing at the base commit —
        // `update-index --cacheinfo` creates it without a real
        // submodule checkout.
        git(
            &work,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("160000,{base_sha},vendor/sub"),
            ],
        );
        git(&work, &["commit", "-m", "fixture: add gitlink"]);
        let head_sha = git(&work, &["rev-parse", "HEAD"]);

        // Self-check: the committed payload.bin blob must be the
        // pointer text verbatim — ambient LFS filters would silently
        // rewrite it otherwise.
        let committed = git(&work, &["cat-file", "-p", "HEAD:payload.bin"]);
        assert_eq!(
            committed,
            pointer.trim_end(),
            "payload.bin must commit the LFS pointer text verbatim"
        );

        // Bare remote with HEAD on main and filter-enabled upload-pack.
        let init = Command::new("git")
            .args(["init", "--bare", "--initial-branch=main"])
            .arg(plain_path(&remote))
            .output()
            .expect("bare init spawn");
        assert!(
            init.status.success(),
            "bare init failed: {}",
            String::from_utf8_lossy(&init.stderr)
        );
        git(&remote, &["symbolic-ref", "HEAD", "refs/heads/main"]);
        if allow_filter {
            git(&remote, &["config", "uploadpack.allowFilter", "true"]);
        }
        git(
            &remote,
            &["config", "uploadpack.allowAnySHA1InWant", "true"],
        );

        git(
            &work,
            &[
                "remote",
                "add",
                "origin",
                plain_path(&remote).to_str().expect("utf8 remote path"),
            ],
        );
        git(&work, &["push", "origin", "main"]);

        // Lay down the real LFS object under the remote's object
        // store — the layout `git lfs` uses: objects/<a>/<b>/<oid>.
        let lfs_oid = sha256_hex(&lfs_payload());
        let (a, rest) = lfs_oid.split_at(2);
        let (b, _) = rest.split_at(2);
        let lfs_dir = remote.join("lfs").join("objects").join(a).join(b);
        fs::create_dir_all(&lfs_dir).expect("lfs dir");
        fs::write(lfs_dir.join(&lfs_oid), lfs_payload()).expect("lfs object");

        // Derive the transport URL through the provider's own parser
        // — one canonical spelling (handles Windows verbatim paths).
        let remote_url = GitRemoteUrl::parse(remote.to_str().expect("utf8 remote path"))
            .expect("fixture remote url")
            .as_str()
            .to_owned();

        Some(Self {
            tmp,
            work,
            remote,
            remote_url,
            head_sha,
            symlink_made,
        })
    }

    /// `true` when the `link.txt` symlink fixture was created.
    pub fn symlink_made(&self) -> bool {
        self.symlink_made
    }

    /// Commits every current worktree change with `msg`; returns sha.
    pub fn commit_all(&self, msg: &str) -> String {
        git(&self.work, &["add", "-A"]);
        git(&self.work, &["commit", "-m", msg]);
        git(&self.work, &["rev-parse", "HEAD"])
    }

    /// Pushes `main` to the remote (non-force).
    pub fn push(&self) {
        git(&self.work, &["push", "origin", "main"]);
    }

    /// Force-pushes `main` to the remote.
    pub fn push_force(&self) {
        git(&self.work, &["push", "--force", "origin", "main"]);
    }

    /// Deletes `rel` from the worktree (caller commits+pushes).
    pub fn remove(&self, rel: &str) {
        fs::remove_file(self.work.join(rel)).expect("remove fixture file");
    }

    /// The remote's `lfs/objects` root.
    pub fn lfs_objects_root(&self) -> PathBuf {
        self.remote.join("lfs").join("objects")
    }

    /// The LFS oid of `payload.bin`'s real content (sha256 hex).
    pub fn lfs_oid() -> String {
        sha256_hex(&lfs_payload())
    }

    /// The real LFS content bytes.
    pub fn lfs_content() -> Vec<u8> {
        lfs_payload()
    }

    /// Deletes the remote entirely (offline/partition simulation).
    /// Existing mirrors keep their objects; only remote operations
    /// (clone/fetch/lfs-root reads of *new* objects) fail.
    pub fn remove_remote(&self) {
        let _ = fs::remove_dir_all(&self.remote);
    }
}

/// Writes the fixture worktree files; returns the LFS pointer text
/// committed as `payload.bin` and whether `link.txt` could be made.
fn seed_worktree(tmp: &TempTree) -> (String, bool) {
    let work = tmp.join("work");
    let lfs_content = lfs_payload();
    let lfs_oid = sha256_hex(&lfs_content);
    let pointer = format!(
        "version https://git-lfs.github.com/spec/v1\noid sha256:{lfs_oid}\nsize {}\n",
        lfs_content.len()
    );

    tmp.write("work/notes.txt", b"hello git\n");
    tmp.write("work/dir/top.txt", b"top level\n");
    tmp.write("work/dir/sub dir/f ile.txt", b"nested name\n");
    // NB: no `a:b.txt` — ':' is illegal in Windows filenames and
    // git-for-windows silently mangles it to U+F03A. Colon-bearing
    // *locator* grammar stays covered by locator unit tests.
    tmp.write(
        "work/.gitattributes",
        b"*.bin filter=lfs diff=lfs merge=lfs -text\n",
    );
    tmp.write("work/payload.bin", pointer.as_bytes());

    // Symlink when the platform permits.
    let symlink_made = {
        let target = work.join("notes.txt");
        let link = work.join("link.txt");
        super::try_symlink(&target, &link)
    };
    (pointer, symlink_made)
}

/// Absolute path → `file:///` URL, for tests that need a URL for a
/// path the provider parser has not seen (kept in sync with the
/// provider's normalization by going through `GitRemoteUrl` where
/// possible).
pub fn path_to_file_url(path: &Path) -> String {
    GitRemoteUrl::parse(path.to_str().expect("utf8 path"))
        .expect("url")
        .as_str()
        .to_owned()
}
