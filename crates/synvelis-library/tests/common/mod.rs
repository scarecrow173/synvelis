//! Shared test helpers: temp trees, contexts, fixture builders.
//!
//! The temp-dir convention mirrors the workspace tests
//! (`std::env::temp_dir()` + pid-tagged name + `Drop` cleanup); no
//! `tempfile` dependency is introduced. The kept root is the
//! *canonical* spelling of the created directory — see
//! [`TempTree::new`].

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use synvelis_library::{
    BrowseRequest, BrowseScope, FilterSpec, FixedClock, OpaqueLocator, PrincipalId,
    ProviderContext, ProviderId, RemoteAssetRef, SortSpec,
};

/// A temporary directory tree removed on drop.
///
/// Helpers are shared across several test targets; each target only
/// uses a subset, so dead-code is allowed module-wide.
#[allow(dead_code)]
pub struct TempTree {
    root: PathBuf,
}

#[allow(dead_code)]
impl TempTree {
    /// A fresh empty directory named for `tag`. Parallel tests share
    /// the pid, so a per-process counter keeps each tree unique.
    pub fn new(tag: &str) -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let seq = NEXT.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("synvelis-lib-{tag}-{}-{seq}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temp root");
        // Keep the canonical spelling: ruviform `ruviform-io-file`
        // rooted capabilities compare absolute link targets
        // component-wise against the canonical root, so a fixture
        // path spelled through a non-canonical alias (macOS `/var`
        // → `/private/var`, Windows 8.3 names, junctions) would make
        // in-root links fail `EscapesRoot`.
        let root = fs::canonicalize(&root).expect("canonical temp root");
        Self { root }
    }

    /// The tree root.
    pub fn path(&self) -> &Path {
        &self.root
    }

    /// A path inside the tree (not checked to exist).
    pub fn join(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.root.join(rel.as_ref())
    }

    /// Creates a file `rel` (with parent dirs) holding `bytes`.
    pub fn write(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let path = self.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dirs");
        }
        fs::write(&path, bytes).expect("write fixture");
        path
    }

    /// Creates a directory `rel` (with parents).
    pub fn mkdir(&self, rel: &str) -> PathBuf {
        let path = self.join(rel);
        fs::create_dir_all(&path).expect("mkdir fixture");
        path
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Offline git fixture builder (ALIB-04 tests). Helpers are shared
/// across test targets; each target uses a subset.
#[allow(dead_code)]
pub mod git_fixture;

/// In-process WebDAV/HTTP mock server (ALIB-06/ALIB-07 tests).
#[allow(dead_code)]
pub mod webdav_fixture;

/// Local-worktree git fixture (VCS-03 tests).
#[allow(dead_code)]
pub mod vcs_git_fixture;

/// A provider id for tests.
#[allow(dead_code)]
pub fn pid(raw: u128) -> ProviderId {
    ProviderId::new(raw)
}

/// An opaque locator for tests.
#[allow(dead_code)]
pub fn loc(text: &str) -> OpaqueLocator {
    OpaqueLocator::new(text).expect("fixture locator")
}

/// An asset ref under `provider`.
#[allow(dead_code)]
pub fn aref(provider: ProviderId, locator: &str) -> RemoteAssetRef {
    RemoteAssetRef {
        provider,
        locator: loc(locator),
    }
}

/// A default context under the `"test-principal"` scope with a fixed
/// clock so `observed_at` is deterministic.
#[allow(dead_code)]
pub fn cx() -> ProviderContext {
    cx_named("test-principal")
}

/// A context under an explicit principal.
#[allow(dead_code)]
pub fn cx_named(principal: &str) -> ProviderContext {
    let mut cx = ProviderContext::new(PrincipalId::new(principal).expect("principal"));
    cx.clock = Arc::new(FixedClock(
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000),
    ));
    cx
}

/// A root-scope browse request with `page_limit`.
#[allow(dead_code)]
pub fn browse_req(page_limit: u32) -> BrowseRequest {
    BrowseRequest {
        scope: BrowseScope::Root,
        page_limit,
        cursor: None,
        sort: SortSpec::default(),
        filter: FilterSpec::default(),
    }
}

/// Attempts to create a symlink; `false` when the platform denies it
/// (e.g. Windows without the developer-mode privilege). Callers skip
/// the test body and report it honestly.
#[cfg(unix)]
#[allow(dead_code)]
pub fn try_symlink(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

/// Windows variant of [`try_symlink`].
#[cfg(windows)]
#[allow(dead_code)]
pub fn try_symlink(target: &Path, link: &Path) -> bool {
    let result = if target.is_dir() {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    };
    result.is_ok()
}
