//! Git process plumbing and output parsing.
//!
//! Every git invocation goes through [`GitRunner`]: fixed argv arrays
//! (never a shell string), an explicit environment that disables
//! interactive prompts and credential helpers, optional
//! `http.<url>.extraHeader` credential injection scoped to the
//! configured remote's authority, and cooperative
//! cancel/deadline checks while the child runs.
//!
//! Parsed shapes: [`GitTreeEntry`] (`ls-tree -z -l` records) and
//! [`LfsPointer`] (the small text pointer that *names* an LFS
//! object — it is never the object's bytes).

use std::io::Read;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Output, Stdio};
use std::thread::JoinHandle;
use std::time::Duration;

use crate::context::ProviderContext;
use crate::error::{ProviderError, ProviderErrorKind};
use crate::git::url::GitRemoteUrl;
use crate::id::CredentialKey;

/// How often a running git child is polled for cancellation.
const WAIT_POLL: Duration = Duration::from_millis(25);

/// `ls-tree` record cap for one browse level — bounds both record
/// count and raw output bytes so a hostile/huge tree cannot exhaust
/// memory. Excess is reported as truncation, never silently dropped.
pub(crate) const LS_TREE_MAX_RECORDS: usize = 100_000;
pub(crate) const LS_TREE_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Git LFS pointer files are tiny; anything larger is not a pointer.
const LFS_POINTER_MAX: usize = 1024;
const LFS_SPEC_URL: &str = "https://git-lfs.github.com/spec/v1";

/// One `ls-tree` entry, classified by git mode+type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GitTreeEntry {
    /// The git mode (`040000`, `100644`, `100755`, `120000`, `160000`).
    pub mode: GitEntryMode,
    /// The object id (hex; length depends on the repo's object format).
    pub oid: String,
    /// Blob byte length when git reports one (`None` for trees and
    /// gitlinks — `-` in `ls-tree -l` output).
    pub size: Option<u64>,
    /// Entry name (single path component, not `/`-joined).
    pub name: String,
}

/// The semantic class of a tree entry.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum GitEntryMode {
    /// `040000 tree` — a browsable subtree.
    Tree,
    /// `100644`/`100755 blob` — fetchable bytes.
    Blob { executable: bool },
    /// `120000 blob` — symlink; the blob content is the target path.
    Symlink,
    /// `160000 commit` — submodule/gitlink; not fetchable here.
    Gitlink,
}

/// A parsed Git LFS pointer: `oid sha256:<hex>` + declared `size`.
/// The pointer is metadata only — it never counts as fetched bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LfsPointer {
    /// The LFS object id (SHA-256 hex, 64 chars).
    pub oid: String,
    /// The declared object size in bytes.
    pub size: u64,
}

/// Stateless git invocation helper bound to one remote URL and an
/// optional credential key.
pub(crate) struct GitRunner {
    remote: GitRemoteUrl,
    credential_key: Option<CredentialKey>,
}

impl GitRunner {
    pub(crate) fn new(remote: GitRemoteUrl, credential_key: Option<CredentialKey>) -> Self {
        Self {
            remote,
            credential_key,
        }
    }

    /// The remote URL (for `ls-remote`-style calls and diagnostics).
    pub(crate) fn remote_url(&self) -> &str {
        self.remote.as_str()
    }

    fn command(
        &self,
        dir: Option<&Path>,
        args: &[&str],
        cx: &ProviderContext,
    ) -> Result<Command, ProviderError> {
        cx.check_interrupts()?;
        let mut cmd = Command::new("git");
        if let Some(dir) = dir {
            cmd.arg("-C").arg(dir);
        }
        cmd.args(args)
            // Never block on a prompt: no terminal prompt, no
            // interactive askpass, no credential helpers.
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ASKPASS", "")
            .env("SSH_ASKPASS", "")
            .env("GIT_CONFIG_COUNT", "3")
            .env("GIT_CONFIG_KEY_0", "credential.helper")
            .env("GIT_CONFIG_VALUE_0", "")
            .env("GIT_CONFIG_KEY_1", "http.followRedirects")
            .env("GIT_CONFIG_VALUE_1", "initial")
            .env("GIT_CONFIG_KEY_2", "protocol.version")
            .env("GIT_CONFIG_VALUE_2", "2");
        if let (Some(key), Some(header)) =
            (self.credential_key.as_ref(), self.http_extra_header(cx)?)
        {
            let _ = key;
            cmd.env("GIT_CONFIG_COUNT", "4")
                .env("GIT_CONFIG_KEY_3", &header.key)
                .env("GIT_CONFIG_VALUE_3", &header.value);
        }
        Ok(cmd)
    }

    /// Resolves the credential key into an `http.<url>.extraHeader`
    /// pair scoped to the remote's own authority — the header can
    /// never leak to a different origin.
    fn http_extra_header(&self, cx: &ProviderContext) -> Result<Option<HeaderPair>, ProviderError> {
        let Some(key) = &self.credential_key else {
            return Ok(None);
        };
        if !matches!(
            self.remote.scheme(),
            crate::git::GitRemoteScheme::Http | crate::git::GitRemoteScheme::Https
        ) {
            // Non-HTTP transports authenticate through their own
            // channel (ssh agent, file perms); an HTTP header would
            // be meaningless there.
            return Ok(None);
        }
        let Some(resolver) = &cx.credentials else {
            return Err(ProviderError::new(
                ProviderErrorKind::AuthExpired,
                "remote requires credentials but no resolver is configured",
            ));
        };
        let cred = resolver.resolve(key)?;
        // `context` is the full Authorization header value chosen by
        // the app's credential store (e.g. "Bearer …", "Basic …").
        let context = cred.context();
        if context.is_empty() || context.contains('\n') || context.contains('\r') {
            return Err(ProviderError::new(
                ProviderErrorKind::AuthExpired,
                "credential context is not a valid header value",
            ));
        }
        let authority = http_authority(self.remote.as_str())?;
        Ok(Some(HeaderPair {
            key: format!("http.{authority}/.extraHeader"),
            value: format!("Authorization: {context}"),
        }))
    }

    /// Runs `git <args>` (optionally `-C dir`) to completion with
    /// piped+drained stdout/stderr, enforcing cancel/deadline while
    /// the child runs.
    pub(crate) fn run(
        &self,
        dir: Option<&Path>,
        args: &[&str],
        cx: &ProviderContext,
    ) -> Result<Output, ProviderError> {
        let mut cmd = self.command(dir, args, cx)?;
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| spawn_error("git", &e))?;
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        let out_h = drain(stdout);
        let err_h = drain(stderr);
        wait_bounded(&mut child, cx)?;
        let stdout = join_drain(out_h);
        let stderr = join_drain(err_h);
        let status = child.try_wait().map_err(|e| {
            ProviderError::new(ProviderErrorKind::TransientIo, format!("git wait: {e}"))
        })?;
        let output = Output {
            status: status.expect("child reaped by wait_bounded"),
            stdout,
            stderr,
        };
        if output.status.success() {
            return Ok(output);
        }
        Err(classify_failure(
            String::from_utf8_lossy(&output.stderr).as_ref(),
            output.status.code(),
        ))
    }

    /// Runs `git <args>` returning stdout bytes, killing the child
    /// once `max_bytes` is exceeded. `truncated` reports the cap hit.
    pub(crate) fn run_stdout_bounded(
        &self,
        dir: Option<&Path>,
        args: &[&str],
        max_bytes: u64,
        cx: &ProviderContext,
    ) -> Result<(Vec<u8>, bool), ProviderError> {
        let mut cmd = self.command(dir, args, cx)?;
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| spawn_error("git", &e))?;
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        let err_h = drain(stderr);
        let (buf, truncated) = read_bounded(stdout, max_bytes);
        let _ = join_drain(err_h);
        if truncated {
            let _ = child.kill();
        }
        wait_bounded(&mut child, cx)?;
        if !truncated {
            let status = child.try_wait().map_err(|e| {
                ProviderError::new(ProviderErrorKind::TransientIo, format!("git wait: {e}"))
            })?;
            if let Some(status) = status
                && !status.success()
            {
                return Err(ProviderError::new(
                    ProviderErrorKind::TransientIo,
                    "git listing failed".to_owned(),
                ));
            }
        }
        Ok((buf, truncated))
    }

    /// Spawns `git cat-file blob <oid>` (or any stdout-streaming
    /// command) and returns the live child plus its stdout. The
    /// returned [`GitObjectStream`] kills the child if dropped
    /// mid-read.
    pub(crate) fn spawn_stdout(
        &self,
        dir: &Path,
        args: &[&str],
        cx: &ProviderContext,
    ) -> Result<GitObjectStream, ProviderError> {
        cx.check_interrupts()?;
        let mut cmd = self.command(Some(dir), args, cx)?;
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|e| spawn_error("git", &e))?;
        let stdout = child.stdout.take().expect("piped");
        let stderr = child.stderr.take().expect("piped");
        let err_h = drain(stderr);
        Ok(GitObjectStream {
            child,
            stdout,
            err_h: Some(err_h),
        })
    }
}

struct HeaderPair {
    key: String,
    value: String,
}

/// Extracts `scheme://authority` from an http(s) URL for the
/// `http.<url>.extraHeader` urlmatch key.
fn http_authority(url: &str) -> Result<String, ProviderError> {
    let rest = url
        .split("://")
        .nth(1)
        .ok_or_else(|| ProviderError::new(ProviderErrorKind::InvalidRequest, "bad remote url"))?;
    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty() {
        return Err(ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            "remote url has no authority",
        ));
    }
    Ok(format!(
        "{}://{}",
        url.split("://").next().unwrap_or(""),
        authority
    ))
}

/// A spawned git child streaming object bytes on stdout.
pub(crate) struct GitObjectStream {
    child: Child,
    stdout: ChildStdout,
    err_h: Option<JoinHandle<Vec<u8>>>,
}

impl GitObjectStream {
    /// Waits for exit and returns stderr text — used by callers that
    /// consume stdout to EOF first.
    pub(crate) fn finish(mut self) -> Result<String, ProviderError> {
        // stdout already consumed by the reader; reap the child.
        let status = self.child.wait().map_err(|e| {
            ProviderError::new(ProviderErrorKind::TransientIo, format!("git wait: {e}"))
        })?;
        let stderr = self.err_h.take().map(join_drain).unwrap_or_default();
        if status.success() {
            Ok(String::from_utf8_lossy(&stderr).into_owned())
        } else {
            Err(classify_failure(
                String::from_utf8_lossy(&stderr).as_ref(),
                status.code(),
            ))
        }
    }
}

impl Read for GitObjectStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.stdout.read(buf)
    }
}

impl Drop for GitObjectStream {
    fn drop(&mut self) {
        // Abandoned mid-read: kill so a lazy-fetch child cannot
        // linger, then reap.
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(h) = self.err_h.take() {
            let _ = h.join();
        }
    }
}

/// Drains a pipe on a thread so the parent can poll for
/// cancellation without deadlocking on a full pipe buffer.
fn drain(mut pipe: impl Read + Send + 'static) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        }
        buf
    })
}

fn join_drain(h: JoinHandle<Vec<u8>>) -> Vec<u8> {
    h.join().unwrap_or_default()
}

/// Polls the child until exit, killing it on cancel/deadline.
fn wait_bounded(child: &mut Child, cx: &ProviderContext) -> Result<(), ProviderError> {
    loop {
        if let Some(_status) = child.try_wait().map_err(|e| {
            ProviderError::new(ProviderErrorKind::TransientIo, format!("git wait: {e}"))
        })? {
            return Ok(());
        }
        if let Err(err) = cx.check_interrupts() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(err);
        }
        std::thread::sleep(WAIT_POLL);
    }
}

/// Reads stdout up to `max_bytes`; returns `(bytes, truncated)`.
fn read_bounded(mut stdout: ChildStdout, max_bytes: u64) -> (Vec<u8>, bool) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match stdout.read(&mut chunk) {
            Ok(0) | Err(_) => return (buf, false),
            Ok(n) => {
                let room = usize::try_from(max_bytes.saturating_sub(buf.len() as u64))
                    .unwrap_or(usize::MAX);
                if n > room {
                    buf.extend_from_slice(&chunk[..room.min(n)]);
                    return (buf, true);
                }
                buf.extend_from_slice(&chunk[..n]);
            }
        }
    }
}

/// Parses `git ls-tree -z -l` output into entries.
///
/// Record layout: `<mode> SP <type> SP <oid> SP <size|-> TAB <name> NUL`.
/// Non-UTF-8 names cannot become locators/display names — they are
/// skipped and reported via the returned `skipped` count so callers
/// can mark coverage partial rather than silently dropping entries.
pub(crate) fn parse_ls_tree_zl(bytes: &[u8]) -> (Vec<GitTreeEntry>, usize) {
    let mut entries = Vec::new();
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
        let name_bytes = &record[tab + 1..];
        let Ok(name) = std::str::from_utf8(name_bytes) else {
            skipped += 1;
            continue;
        };
        // `ls-tree -l` right-pads the size column with spaces, so the
        // meta fields are separated by *runs* of whitespace — split
        // on whitespace, never on a single space.
        let Ok(meta) = std::str::from_utf8(meta) else {
            skipped += 1;
            continue;
        };
        let mut parts = meta.split_ascii_whitespace();
        let (Some(mode_s), Some(type_s), Some(oid), Some(size_s)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            skipped += 1;
            continue;
        };
        if parts.next().is_some() {
            skipped += 1;
            continue;
        }
        let mode = match (mode_s, type_s) {
            ("040000", "tree") => GitEntryMode::Tree,
            ("160000", "commit") => GitEntryMode::Gitlink,
            ("120000", "blob") => GitEntryMode::Symlink,
            ("100644", "blob") => GitEntryMode::Blob { executable: false },
            ("100755", "blob") => GitEntryMode::Blob { executable: true },
            _ => {
                skipped += 1;
                continue;
            }
        };
        let size = if size_s == "-" {
            None
        } else if let Ok(v) = size_s.parse::<u64>() {
            Some(v)
        } else {
            skipped += 1;
            continue;
        };
        entries.push(GitTreeEntry {
            mode,
            oid: oid.to_owned(),
            size,
            name: name.to_owned(),
        });
    }
    (entries, skipped)
}

/// Parses a Git LFS pointer. Strict three-line format:
///
/// ```text
/// version https://git-lfs.github.com/spec/v1
/// oid sha256:<64 lowercase hex>
/// size <decimal>
/// ```
///
/// Returns `None` for anything else — a regular blob that merely
/// *resembles* a pointer is still a regular blob only when it fails
/// this exact grammar, and callers additionally require the blob to
/// be small enough to be a pointer.
pub(crate) fn parse_lfs_pointer(bytes: &[u8]) -> Option<LfsPointer> {
    if bytes.len() > LFS_POINTER_MAX {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut lines = text.lines();
    if lines.next()? != format!("version {LFS_SPEC_URL}") {
        return None;
    }
    let oid_line = lines.next()?;
    let oid = oid_line.strip_prefix("oid sha256:")?;
    if oid.len() != 64 || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let size_line = lines.next()?;
    let size_text = size_line.strip_prefix("size ")?;
    if size_text.is_empty() || !size_text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let size = size_text.parse::<u64>().ok()?;
    // Exactly three content lines; a trailing newline is fine but no
    // fourth line may exist.
    if lines.next().is_some() {
        return None;
    }
    Some(LfsPointer {
        oid: oid.to_ascii_lowercase(),
        size,
    })
}

/// `true` when a blob is small enough to be an LFS pointer.
pub(crate) fn is_pointer_sized(size: u64) -> bool {
    size <= LFS_POINTER_MAX as u64
}

/// Classifies git's stderr + exit status into a typed error kind.
/// Pure function — the unit-testable heart of failure mapping.
/// Credential-bearing stderr is never echoed into `detail` beyond a
/// short sanitized classification tag.
pub(crate) fn classify_failure(stderr: &str, code: Option<i32>) -> ProviderError {
    let kind = classify_kind(stderr, code);
    ProviderError::new(kind, sanitize_detail(stderr))
}

fn classify_kind(stderr: &str, code: Option<i32>) -> ProviderErrorKind {
    let s = stderr.to_ascii_lowercase();
    // Authentication first — git reports auth failures through many
    // transports (http 401/403, ssh, credential helper).
    if s.contains("authentication failed")
        || s.contains("could not read username")
        || s.contains("could not read password")
        || s.contains("returned error: 401")
        || s.contains("returned error: 403")
        || s.contains("access denied")
        || s.contains("permission denied")
        || s.contains("authentication required")
        || s.contains("terminal prompts disabled")
    {
        return ProviderErrorKind::AuthExpired;
    }
    if s.contains("429")
        || s.contains("too many requests")
        || s.contains("rate limit")
        || s.contains("secondary rate limit")
    {
        return ProviderErrorKind::RateLimited;
    }
    if s.contains("could not resolve host")
        || s.contains("failed to connect")
        || s.contains("connection refused")
        || s.contains("network is unreachable")
        || s.contains("no route to host")
        || s.contains("name or service not known")
        || s.contains("temporary failure in name resolution")
    {
        return ProviderErrorKind::Offline;
    }
    if s.contains("filtering not recognized")
        || s.contains("uploadpack.allowfilter")
        || s.contains("filter not recognized")
        || s.contains("server does not support --filter")
        || s.contains("filtering is not recognized")
    {
        return ProviderErrorKind::UnsupportedCapability;
    }
    if s.contains("not found")
        || s.contains("does not exist")
        || s.contains("unknown revision")
        || s.contains("needed a single revision")
        || s.contains("not a valid object name")
        || s.contains("bad revision")
        || s.contains("ambiguous argument")
        || s.contains("not our ref")
        || s.contains("could not get object info")
    {
        return ProviderErrorKind::NotFound;
    }
    if s.contains("not a tree object") {
        // ls-tree/stat against a non-tree path — a caller error, not
        // a backend failure.
        return ProviderErrorKind::InvalidRequest;
    }
    if s.contains("timed out") || s.contains("timeout") {
        return ProviderErrorKind::TransientIo;
    }
    match code {
        Some(128) => ProviderErrorKind::TransientIo,
        Some(c) if c < 0 => ProviderErrorKind::Cancelled,
        _ => ProviderErrorKind::TransientIo,
    }
}

/// Reduces stderr to a bounded, credential-safe diagnostic tag.
/// Git can echo URLs (and historically tokens in them) into stderr;
/// when a URL is present we keep only `scheme://host/` — userinfo
/// (which may carry secrets) and any path/query are dropped.
fn sanitize_detail(stderr: &str) -> String {
    let first = stderr
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("git command failed");
    let mut out: String = first.chars().take(160).collect();
    if let Some(idx) = out.find("://") {
        let after = &out[idx + 3..];
        let auth_end = after.find(['/', '?', '#']).unwrap_or(after.len());
        let authority = &after[..auth_end];
        // Drop any userinfo: keep only the host part after the last '@'.
        let host = authority.rsplit('@').next().unwrap_or(authority);
        out = format!("{}://{host}/", &out[..idx]);
    }
    out
}

fn spawn_error(op: &'static str, e: &std::io::Error) -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::Internal,
        format!("failed to spawn {op}: {e}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_failures() {
        assert_eq!(
            classify_kind(
                "fatal: Authentication failed for 'https://x/r.git/'",
                Some(128)
            ),
            ProviderErrorKind::AuthExpired
        );
        assert_eq!(
            classify_kind(
                "fatal: unable to access 'https://x/r.git/': The requested URL returned error: 403",
                Some(128)
            ),
            ProviderErrorKind::AuthExpired
        );
        assert_eq!(
            classify_kind("error: RPC failed; HTTP 429 curl 22", Some(128)),
            ProviderErrorKind::RateLimited
        );
        assert_eq!(
            classify_kind(
                "fatal: unable to access 'https://x/r.git/': Could not resolve host: x",
                Some(128)
            ),
            ProviderErrorKind::Offline
        );
        assert_eq!(
            classify_kind(
                "fatal: remote error: uploadpack: not our ref filtering not recognized",
                Some(128)
            ),
            ProviderErrorKind::UnsupportedCapability
        );
        assert_eq!(
            classify_kind("fatal: Not a valid object name abc123", Some(128)),
            ProviderErrorKind::NotFound
        );
        assert_eq!(
            classify_kind(
                "fatal: ambiguous argument 'main': unknown revision",
                Some(128)
            ),
            ProviderErrorKind::NotFound
        );
    }

    #[test]
    fn parses_ls_tree_records() {
        let blob = b"100644 blob 19c10ccecaa1815078cbfa4dc1113eb06038c813 12\ttop.txt\0";
        let tree = b"040000 tree a55e768a920c0422fbfab79cee663536f980551a -\tsub dir\0";
        let link = b"120000 blob 975fbec8256d3e8a3797e7a3611380f27c49f4ac 9\tlink\0";
        let gitlink = b"160000 commit 587be6b4c3f93f93c489c0111bba5596147a26cb -\tsub\0";
        let exe = b"100755 blob b68025345d5301abad4d9ec9166f455243a0d746 3\trun.sh\0";
        let mut all = Vec::new();
        all.extend_from_slice(blob);
        all.extend_from_slice(tree);
        all.extend_from_slice(link);
        all.extend_from_slice(gitlink);
        all.extend_from_slice(exe);
        let (entries, skipped) = parse_ls_tree_zl(&all);
        assert_eq!(skipped, 0);
        assert_eq!(entries.len(), 5);
        assert_eq!(entries[0].mode, GitEntryMode::Blob { executable: false });
        assert_eq!(entries[0].size, Some(12));
        assert_eq!(entries[1].mode, GitEntryMode::Tree);
        assert_eq!(entries[1].size, None);
        assert_eq!(entries[2].mode, GitEntryMode::Symlink);
        assert_eq!(entries[3].mode, GitEntryMode::Gitlink);
        assert_eq!(entries[4].mode, GitEntryMode::Blob { executable: true });
        assert!(matches!(entries[3].mode, GitEntryMode::Gitlink));
        assert!(matches!(entries[2].mode, GitEntryMode::Symlink));
    }

    #[test]
    fn parses_lfs_pointer_strictly() {
        let oid = "a".repeat(64);
        let good =
            format!("version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize 12345\n")
                .into_bytes();
        let p = parse_lfs_pointer(&good).unwrap();
        assert_eq!(p.oid, oid);
        assert_eq!(p.size, 12345);
        // wrong version, bad oid, extra line, oversized input rejected
        assert!(parse_lfs_pointer(
            b"version https://example.com/spec/v1\noid sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nsize 1\n"
        )
        .is_none());
        assert!(
            parse_lfs_pointer(
                b"version https://git-lfs.github.com/spec/v1\noid sha256:aaaa\nsize 1\n"
            )
            .is_none()
        );
        assert!(parse_lfs_pointer(
            b"version https://git-lfs.github.com/spec/v1\noid sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\nsize 1\nextra\n"
        )
        .is_none());
        assert!(parse_lfs_pointer(&vec![b'x'; 2000]).is_none());
        assert!(parse_lfs_pointer(b"just some text\n").is_none());
    }
}
