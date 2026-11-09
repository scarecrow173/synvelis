//! The provider's read-only git mirror — a derived cache, never the
//! project's repository.
//!
//! `ensure` performs at most one `git clone --mirror
//! --filter=blob:none` per mirror directory; `refresh` runs
//! `git fetch --prune` so branch updates and forced pushes become
//! visible only on explicit refresh. All object access goes through
//! `cat-file`/`ls-tree`/`rev-parse` against the mirror — selective
//! fetch happens lazily per object, never as a full checkout, and
//! the provider never writes refs, index, HEAD, or a worktree.
//!
//! When the server cannot honor `--filter` the configured
//! [`UnsupportedFilterPolicy`] decides between an honest
//! `UnsupportedCapability` and an explicit full mirror.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::context::ProviderContext;
use crate::error::{ProviderError, ProviderErrorKind};
use crate::git::objects::{
    GitEntryMode, GitRunner, GitTreeEntry, LS_TREE_MAX_BYTES, LS_TREE_MAX_RECORDS, parse_ls_tree_zl,
};
use crate::git::url::GitRemoteUrl;

/// What the mirror actually holds after clone.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MirrorMode {
    /// `blob:none` filter honored — blobs arrive lazily per object.
    Partial,
    /// Filter unavailable/refused and policy allowed a full mirror.
    Full,
    /// Clone produced no objects to judge (empty remote) — vacuously
    /// consistent with partial.
    Unknown,
}

/// The repository's object id format.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GitObjectFormat {
    /// 40-hex SHA-1 object ids.
    Sha1,
    /// 64-hex SHA-256 object ids.
    Sha256,
}

impl GitObjectFormat {
    /// Expected hex length of an object id.
    #[must_use]
    pub(crate) const fn oid_len(self) -> usize {
        match self {
            Self::Sha1 => 40,
            Self::Sha256 => 64,
        }
    }

    /// Public-facing label for `GitProviderInfo`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        }
    }
}

/// What to do when the remote cannot serve a filtered clone.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UnsupportedFilterPolicy {
    /// Fail with `UnsupportedCapability` — never silently download
    /// the whole repository.
    Reject,
    /// Retry the clone without `--filter` and operate on a full
    /// mirror (still read-only; still no checkout).
    AllowFullMirror,
}

/// Live browse epochs whose cursors remain valid.
const MAX_LIVE_EPOCHS: usize = 64;

struct MirrorState {
    ensured: bool,
    mode: MirrorMode,
    object_format: Option<GitObjectFormat>,
    /// Resolved remote `HEAD` symref target (e.g. `refs/heads/main`)
    /// when the configured reference is `HEAD` — mirror clones can
    /// leave `HEAD` unborn, so we resolve it against the remote.
    head_ref: Option<String>,
    epochs: BTreeSet<u64>,
    epoch_targets: std::collections::BTreeMap<u64, (String, String)>,
}

/// One provider mirror directory + its git runner.
pub(crate) struct GitMirror {
    dir: PathBuf,
    runner: GitRunner,
    reference: String,
    policy: UnsupportedFilterPolicy,
    state: Mutex<MirrorState>,
    next_epoch: AtomicU64,
}

impl GitMirror {
    pub(crate) fn new(
        dir: PathBuf,
        remote: GitRemoteUrl,
        reference: String,
        credential_key: Option<crate::id::CredentialKey>,
        policy: UnsupportedFilterPolicy,
    ) -> Self {
        Self {
            dir: plain_dir(dir),
            runner: GitRunner::new(remote, credential_key),
            reference,
            policy,
            state: Mutex::new(MirrorState {
                ensured: false,
                mode: MirrorMode::Unknown,
                object_format: None,
                head_ref: None,
                epochs: BTreeSet::new(),
                epoch_targets: std::collections::BTreeMap::new(),
            }),
            next_epoch: AtomicU64::new(1),
        }
    }

    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    pub(crate) fn runner(&self) -> &GitRunner {
        &self.runner
    }

    /// Ensures the mirror exists, honoring the filter policy.
    /// Idempotent; the first call performs the clone.
    pub(crate) fn ensure(&self, cx: &ProviderContext) -> Result<(), ProviderError> {
        {
            let st = self.state.lock().map_err(|_| poisoned())?;
            if st.ensured {
                return Ok(());
            }
        }
        if self.is_cloned() {
            self.finish_ensure(cx)?;
            return Ok(());
        }
        // Fresh clone: always request blob:none first.
        match self.clone_mirror(true, cx) {
            Ok(()) => {}
            Err(e) if e.kind() == ProviderErrorKind::UnsupportedCapability => match self.policy {
                UnsupportedFilterPolicy::Reject => return Err(e),
                UnsupportedFilterPolicy::AllowFullMirror => {
                    self.clone_mirror(false, cx)?;
                }
            },
            Err(e) => return Err(e),
        }
        self.finish_ensure(cx)
    }

    /// `git fetch --prune` — the only way remote updates become
    /// visible. Never implicit. Also re-resolves the remote `HEAD`
    /// symref when the configured reference is `HEAD`.
    pub(crate) fn refresh(&self, cx: &ProviderContext) -> Result<(), ProviderError> {
        self.ensure(cx)?;
        self.runner
            .run(Some(&self.dir), &["fetch", "--prune", "origin"], cx)
            .map(|_| ())?;
        let new_head = if self.reference == "HEAD" {
            self.remote_head_ref(cx).ok().flatten()
        } else {
            None
        };
        if let Ok(mut st) = self.state.lock() {
            st.head_ref = new_head;
        }
        Ok(())
    }

    /// Resolves a revision expression to a commit sha.
    ///
    /// `HEAD` resolves through the remote's symref target when the
    /// mirror's own `HEAD` is unborn (the common `--mirror` case).
    pub(crate) fn resolve_commit(
        &self,
        rev: &str,
        cx: &ProviderContext,
    ) -> Result<String, ProviderError> {
        self.ensure(cx)?;
        let effective = if rev == "HEAD" {
            self.head_ref(cx)?.unwrap_or_else(|| "HEAD".to_owned())
        } else {
            rev.to_owned()
        };
        let out = self.runner.run(
            Some(&self.dir),
            &["rev-parse", "--verify", &format!("{effective}^{{commit}}")],
            cx,
        )?;
        let sha = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        self.validate_oid(&sha)?;
        Ok(sha)
    }

    /// Lists one level of `path`'s tree at commit `sha`.
    /// `path == ""` lists the commit's root tree.
    ///
    /// Returns `(entries, truncated, skipped_non_utf8)`.
    pub(crate) fn list_tree(
        &self,
        sha: &str,
        path: &str,
        cx: &ProviderContext,
    ) -> Result<(Vec<GitTreeEntry>, bool, usize), ProviderError> {
        self.ensure(cx)?;
        let tree_ish = if path.is_empty() {
            sha.to_owned()
        } else {
            format!("{sha}:{path}")
        };
        let max_records = u64::try_from(LS_TREE_MAX_RECORDS).unwrap_or(u64::MAX);
        let (bytes, byte_truncated) = self.runner.run_stdout_bounded(
            Some(&self.dir),
            &["ls-tree", "-z", "-l", &tree_ish],
            LS_TREE_MAX_BYTES,
            cx,
        )?;
        let (entries, skipped) = parse_ls_tree_zl(&bytes);
        let record_truncated = entries.len() as u64 + skipped as u64 > max_records;
        Ok((entries, byte_truncated || record_truncated, skipped))
    }

    /// Stats one path within commit `sha`; `None` = absent.
    /// `path == ""` describes the commit's root tree synthetically.
    pub(crate) fn stat_entry(
        &self,
        sha: &str,
        path: &str,
        cx: &ProviderContext,
    ) -> Result<Option<GitTreeEntry>, ProviderError> {
        self.ensure(cx)?;
        if path.is_empty() {
            // The root "entry" is the commit's root tree — synthesize
            // it so stat/pin on `<rev>:` is well-defined.
            let out = self.runner.run(
                Some(&self.dir),
                &["rev-parse", "--verify", &format!("{sha}^{{tree}}")],
                cx,
            )?;
            let oid = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            self.validate_oid(&oid)?;
            return Ok(Some(GitTreeEntry {
                mode: GitEntryMode::Tree,
                oid,
                size: None,
                name: String::new(),
            }));
        }
        // `:(literal)` pathspec magic + `--` — the path is data, never
        // a pathspec/glob, and never an option. ls-tree echoes the
        // entry's full path in the name field, so an exact match on
        // `path` proves the record is the requested object.
        let pathspec = format!(":(literal){path}");
        let (bytes, truncated) = self.runner.run_stdout_bounded(
            Some(&self.dir),
            &["ls-tree", "-z", "-l", sha, "--", &pathspec],
            LS_TREE_MAX_BYTES,
            cx,
        )?;
        if truncated {
            return Err(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                "ls-tree output exceeded capture bound",
            ));
        }
        let (entries, skipped) = parse_ls_tree_zl(&bytes);
        if skipped > 0 {
            return Err(ProviderError::new(
                ProviderErrorKind::Internal,
                "ls-tree output contained unparseable records",
            ));
        }
        match entries.as_slice() {
            [] => Ok(None),
            [entry] if entry.name == path => Ok(Some(entry.clone())),
            _ => Err(ProviderError::new(
                ProviderErrorKind::Internal,
                "ls-tree literal pathspec did not resolve to exactly one entry",
            )),
        }
    }

    /// Reads up to `head_len` bytes of a blob's head — used at pin
    /// time to detect LFS pointers without streaming whole objects.
    pub(crate) fn blob_head(
        &self,
        oid: &str,
        head_len: u64,
        cx: &ProviderContext,
    ) -> Result<Vec<u8>, ProviderError> {
        self.ensure(cx)?;
        self.validate_oid(oid)?;
        // `cat-file blob <oid>` streams the object; we bound the read.
        let mut stream = self
            .runner
            .spawn_stdout(&self.dir, &["cat-file", "blob", oid], cx)?;
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        let want = usize::try_from(head_len).unwrap_or(usize::MAX);
        while buf.len() < want {
            // The stream watchdog kills the child on cancel/deadline;
            // check here so a truncated head is never mistaken for a
            // complete one.
            cx.check_interrupts()?;
            match std::io::Read::read(&mut stream, &mut chunk) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                Err(e) => {
                    let _ = stream.finish();
                    return Err(crate::digest::io_provider_error("cat-file", &e));
                }
            }
        }
        drop(stream); // kills the child if it is still streaming
        Ok(buf)
    }

    /// `true` when `path` is LFS-tracked (`filter=lfs`) at commit
    /// `sha`, per `git check-attr --source`. On any check failure the
    /// answer fails *safe toward LFS*: a pointer-shaped blob is then
    /// treated as LFS so pointer text can never be served as content.
    pub(crate) fn lfs_tracked(&self, sha: &str, path: &str, cx: &ProviderContext) -> bool {
        match self.runner.run(
            Some(&self.dir),
            &["check-attr", "--source", sha, "filter", "--", path],
            cx,
        ) {
            Ok(out) => String::from_utf8_lossy(&out.stdout)
                .lines()
                .any(|l| l.trim_end().ends_with(": filter: lfs")),
            Err(_) => true,
        }
    }

    /// Spawns a streaming `cat-file blob <oid>` read.
    pub(crate) fn open_blob(
        &self,
        oid: &str,
        cx: &ProviderContext,
    ) -> Result<crate::git::objects::GitObjectStream, ProviderError> {
        self.ensure(cx)?;
        self.validate_oid(oid)?;
        self.runner
            .spawn_stdout(&self.dir, &["cat-file", "blob", oid], cx)
    }

    /// The detected object id format (after ensure).
    pub(crate) fn object_format(&self) -> Option<GitObjectFormat> {
        self.state.lock().ok().and_then(|s| s.object_format)
    }

    /// The detected mirror mode (after ensure).
    pub(crate) fn mirror_mode(&self) -> MirrorMode {
        self.state.lock().map_or(MirrorMode::Unknown, |s| s.mode)
    }

    /// Issues (or reuses for cursor) a browse epoch bound to
    /// `(commit_sha, path)`.
    pub(crate) fn issue_epoch(&self, sha: &str, path: &str) -> u64 {
        let epoch = self.next_epoch.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut st) = self.state.lock() {
            while st.epochs.len() >= MAX_LIVE_EPOCHS {
                if let Some(oldest) = st.epochs.iter().next().copied() {
                    st.epochs.remove(&oldest);
                    st.epoch_targets.remove(&oldest);
                } else {
                    break;
                }
            }
            st.epochs.insert(epoch);
            st.epoch_targets
                .insert(epoch, (sha.to_owned(), path.to_owned()));
        }
        epoch
    }

    /// The `(sha, path)` an epoch pins, when still live.
    pub(crate) fn epoch_target(&self, epoch: u64) -> Option<(String, String)> {
        self.state
            .lock()
            .ok()
            .and_then(|st| st.epoch_targets.get(&epoch).cloned())
    }

    pub(crate) fn epoch_valid(&self, epoch: u64) -> bool {
        self.state.lock().is_ok_and(|st| st.epochs.contains(&epoch))
    }

    /// Validates an object id against the detected format.
    fn validate_oid(&self, oid: &str) -> Result<(), ProviderError> {
        let format = self.object_format().unwrap_or(GitObjectFormat::Sha1);
        let ok = oid.len() == format.oid_len() && oid.bytes().all(|b| b.is_ascii_hexdigit());
        if ok {
            Ok(())
        } else {
            Err(ProviderError::new(
                ProviderErrorKind::Internal,
                format!("git returned a malformed object id {oid:?}"),
            ))
        }
    }

    fn is_cloned(&self) -> bool {
        self.dir.join("HEAD").is_file() && self.dir.join("objects").is_dir()
    }

    /// Post-clone bookkeeping: object format, filter mode, remote
    /// HEAD symref.
    fn finish_ensure(&self, cx: &ProviderContext) -> Result<(), ProviderError> {
        let out = self
            .runner
            .run(Some(&self.dir), &["rev-parse", "--show-object-format"], cx)?;
        let format = match String::from_utf8_lossy(&out.stdout).trim() {
            "sha256" => GitObjectFormat::Sha256,
            "sha1" => GitObjectFormat::Sha1,
            other => {
                return Err(ProviderError::new(
                    ProviderErrorKind::Internal,
                    format!("unknown git object format {other:?}"),
                ));
            }
        };
        // `remote.origin.partialclonefilter` records that the clone
        // *requested* `--filter` — the server may still have ignored
        // it, which `detect_mode` resolves via the object census.
        let filtered = self.clone_requested_filter(cx)?;
        let mode = self.detect_mode(filtered, cx)?;
        // A filtered clone that came back Full means the server
        // cannot serve partial clone. Under `Reject` that is a
        // typed refusal — never a silent full-mirror fallback.
        // Checked before `ensured` is set so every call agrees.
        if filtered && mode == MirrorMode::Full && self.policy == UnsupportedFilterPolicy::Reject {
            return Err(ProviderError::new(
                ProviderErrorKind::UnsupportedCapability,
                "remote does not support partial-clone filters",
            ));
        }
        let head_ref = if self.reference == "HEAD" {
            self.remote_head_ref(cx)?
        } else {
            None
        };
        if let Ok(mut st) = self.state.lock() {
            st.ensured = true;
            st.object_format = Some(format);
            st.mode = mode;
            st.head_ref = head_ref;
        }
        Ok(())
    }

    /// `true` when this mirror's clone requested `--filter`
    /// (`remote.origin.partialclonefilter` is set by the client even
    /// when the server ignores the filter).
    fn clone_requested_filter(&self, cx: &ProviderContext) -> Result<bool, ProviderError> {
        let out = self.runner.run(
            Some(&self.dir),
            &[
                "config",
                "--default",
                "",
                "--get",
                "remote.origin.partialclonefilter",
            ],
            cx,
        )?;
        Ok(!String::from_utf8_lossy(&out.stdout).trim().is_empty())
    }

    /// The remote's HEAD symref target via `ls-remote --symref`.
    fn remote_head_ref(&self, cx: &ProviderContext) -> Result<Option<String>, ProviderError> {
        let out = self.runner.run(
            None,
            &["ls-remote", "--symref", self.runner.remote_url(), "HEAD"],
            cx,
        )?;
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            // "ref: refs/heads/main\tHEAD"
            if let Some(rest) = line.strip_prefix("ref: ")
                && let Some((target, _name)) = rest.split_once('\t')
            {
                return Ok(Some(target.trim().to_owned()));
            }
        }
        Ok(None)
    }

    fn head_ref(&self, cx: &ProviderContext) -> Result<Option<String>, ProviderError> {
        if let Ok(st) = self.state.lock()
            && st.head_ref.is_some()
        {
            return Ok(st.head_ref.clone());
        }
        // Lazily resolve if ensure ran before we cared.
        if let Some(r) = self.remote_head_ref(cx)? {
            if let Ok(mut st) = self.state.lock() {
                st.head_ref = Some(r.clone());
            }
            return Ok(Some(r));
        }
        Ok(None)
    }

    /// `git clone --mirror` — `filtered=true` adds
    /// `--filter=blob:none`.
    fn clone_mirror(&self, filtered: bool, cx: &ProviderContext) -> Result<(), ProviderError> {
        if self.dir.exists() {
            // A failed earlier attempt may have left a partial dir;
            // remove only dirs that look like an aborted clone (no
            // HEAD). Never remove a valid mirror.
            if self.dir.join("HEAD").exists() {
                return Ok(());
            }
            let _ = std::fs::remove_dir_all(&self.dir);
        }
        if let Some(parent) = self.dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ProviderError::new(
                    ProviderErrorKind::TransientIo,
                    format!("cannot create mirror parent {}: {e}", parent.display()),
                )
            })?;
        }
        let url = self.runner.remote_url();
        let dir_s = self.dir.to_string_lossy().into_owned();
        let mut args: Vec<String> = vec![
            "clone".to_owned(),
            "--mirror".to_owned(),
            "--no-checkout".to_owned(),
        ];
        if filtered {
            args.push("--filter=blob:none".to_owned());
        }
        args.push(url.to_owned());
        args.push(dir_s);
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.runner
            .run(None, &arg_refs, cx)
            .map(|_| ())
            .map_err(|e| {
                // A filtered-clone rejection must surface as
                // UnsupportedCapability so the policy can decide.
                if filtered && e.kind() == ProviderErrorKind::UnsupportedCapability {
                    e
                } else if filtered
                    && (e.detail().contains("filter") || e.detail().contains("not recognized"))
                {
                    ProviderError::new(
                        ProviderErrorKind::UnsupportedCapability,
                        "remote does not support partial-clone filters",
                    )
                } else {
                    e
                }
            })
    }

    /// Detects whether the clone is actually partial by counting
    /// missing promisor objects. `filtered` says the clone *requested*
    /// `--filter`; a filtered clone with zero missing objects is
    /// ambiguous — the server may have ignored the filter (all blobs
    /// already local → `Full`) or the repository may simply contain
    /// no blobs (→ `Partial`, vacuously selective). [`Self::repo_has_blobs`]
    /// resolves the ambiguity.
    fn detect_mode(
        &self,
        filtered: bool,
        cx: &ProviderContext,
    ) -> Result<MirrorMode, ProviderError> {
        let out = self.runner.run(
            Some(&self.dir),
            &["rev-list", "--objects", "--all", "--missing=print"],
            cx,
        )?;
        let text = String::from_utf8_lossy(&out.stdout);
        let mut any_objects = false;
        let mut missing = 0u64;
        for line in text.lines() {
            if line.starts_with('?') {
                missing += 1;
            } else if !line.trim().is_empty() {
                any_objects = true;
            }
        }
        if !any_objects {
            return Ok(MirrorMode::Unknown);
        }
        if missing > 0 {
            return Ok(MirrorMode::Partial);
        }
        if !filtered {
            // An unfiltered clone holds everything by definition.
            return Ok(MirrorMode::Full);
        }
        if self.repo_has_blobs(cx)? {
            Ok(MirrorMode::Full)
        } else {
            Ok(MirrorMode::Partial)
        }
    }

    /// `true` when any ref's tree contains at least one blob object.
    /// Inspects refs in order and stops at the first blob found.
    fn repo_has_blobs(&self, cx: &ProviderContext) -> Result<bool, ProviderError> {
        let out = self.runner.run(
            Some(&self.dir),
            &["for-each-ref", "--format=%(objectname)"],
            cx,
        )?;
        for sha in String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
        {
            if self.tree_has_blob(sha, cx)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `true` when `sha`'s tree (peeled via `^{tree}`) lists a blob
    /// record anywhere in its recursive `ls-tree -r` stream. The
    /// child is killed as soon as the first blob record arrives, so
    /// huge trees are never fully materialized.
    fn tree_has_blob(&self, sha: &str, cx: &ProviderContext) -> Result<bool, ProviderError> {
        let tree_ish = format!("{sha}^{{tree}}");
        let mut stream =
            match self
                .runner
                .spawn_stdout(&self.dir, &["ls-tree", "-r", "-z", &tree_ish], cx)
            {
                Ok(s) => s,
                // Interrupts must propagate; anything else (e.g. a ref
                // pointing at a blob, which cannot peel to a tree) just
                // means this ref has no tree to inspect.
                Err(e)
                    if matches!(
                        e.kind(),
                        ProviderErrorKind::Cancelled | ProviderErrorKind::BudgetExceeded
                    ) =>
                {
                    return Err(e);
                }
                Err(_) => return Ok(false),
            };
        let mut pending: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => return Ok(false),
                Ok(n) => {
                    pending.extend_from_slice(&chunk[..n]);
                    let mut start = 0usize;
                    for (i, &b) in pending.iter().enumerate() {
                        if b == 0 {
                            if ls_tree_record_is_blob(&pending[start..=i]) {
                                return Ok(true);
                            }
                            start = i + 1;
                        }
                    }
                    pending.drain(..start);
                }
                Err(e) => {
                    drop(stream);
                    return Err(crate::digest::io_provider_error("ls-tree", &e));
                }
            }
        }
    }
}

/// `true` when one `ls-tree -z -l` record (NUL included) describes a
/// blob object — mode `100644`/`100755`/`120000`, type `blob`.
fn ls_tree_record_is_blob(record: &[u8]) -> bool {
    let Some(tab) = record.iter().position(|&b| b == b'\t') else {
        return false;
    };
    let Ok(meta) = std::str::from_utf8(&record[..tab]) else {
        return false;
    };
    let mut fields = meta.split_ascii_whitespace();
    let _mode = fields.next();
    fields.next() == Some("blob")
}

/// Windows canonicalized paths carry the `\\?\` verbatim prefix,
/// which git's own path arguments (`clone <dir>`) reject with
/// "Invalid argument" — `git -C` tolerates it but path operands do
/// not. Hand git the plain spelling; `\\?\UNC\` maps back to `\\`.
fn plain_dir(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}

fn poisoned() -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::Internal,
        "git mirror state lock poisoned",
    )
}
