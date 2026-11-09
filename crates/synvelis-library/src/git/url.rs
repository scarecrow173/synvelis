//! `GitRemoteUrl` — validated, credential-free remote URL.
//!
//! The provider accepts `file`/`http`/`https`/`ssh`/`git` scheme
//! URLs, scp-like `user@host:path` SSH remotes, and plain local
//! paths (canonicalized into `file://` URLs so the transport —
//! and therefore `--filter` partial clone — is always exercised;
//! a plain-path clone would silently hardlink every object).
//!
//! Credentials are never embedded: any userinfo carrying a
//! password (`user:pass@`) is rejected for every scheme, and
//! userinfo at all is rejected for non-SSH schemes. Git's
//! `ext::`/`fd::` transports can execute arbitrary commands and
//! are excluded by the scheme allowlist.

use std::path::{Path, PathBuf};

use crate::error::{ProviderError, ProviderErrorKind};

/// The remote transport family — classification only. Actual
/// network reachability is a runtime property, never claimed here.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum GitRemoteScheme {
    /// `file://` URL or a plain local path (normalized to file).
    File,
    /// `http://`.
    Http,
    /// `https://`.
    Https,
    /// `ssh://` or scp-like `user@host:path`.
    Ssh,
    /// `git://` (git daemon).
    Git,
    /// Anything else that passed validation (currently none —
    /// kept for forward compatibility of the classification).
    Other,
}

/// A validated remote URL plus its resolved local filesystem
/// path when the transport is local (`file://`/path).
#[derive(Clone, Debug)]
pub struct GitRemoteUrl {
    /// The URL text handed to `git` argv verbatim.
    raw: String,
    scheme: GitRemoteScheme,
    /// Local filesystem path of the remote git dir — `Some` only
    /// for `File` scheme. Used to locate `<remote>/lfs/objects`.
    local_path: Option<PathBuf>,
}

impl GitRemoteUrl {
    /// Parses and validates `text` as a git remote.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] on empty input,
    /// unsupported scheme, embedded credentials, malformed
    /// scp-like syntax, or a nonexistent local path.
    pub fn parse(text: &str) -> Result<Self, ProviderError> {
        let text = text.trim();
        if text.is_empty() || text.len() > 4096 || text.contains('\0') {
            return Err(invalid("remote url is empty, over-long, or contains NUL"));
        }
        if text.contains("://") {
            return Self::parse_url_form(text);
        }
        // Windows local paths must be recognized *before* scp-like
        // syntax: `C:\repo` and verbatim `\\?\C:\repo` both contain a
        // colon ahead of any '/', which would otherwise parse as an
        // SSH remote named "C".
        if looks_like_local_path(text) {
            return Self::from_local_path(Path::new(text));
        }
        if looks_like_scp(text) {
            return Self::parse_scp(text);
        }
        Self::from_local_path(Path::new(text))
    }

    fn parse_url_form(text: &str) -> Result<Self, ProviderError> {
        let (scheme, rest) = text.split_once("://").expect("checked above");
        let scheme = scheme.to_ascii_lowercase();
        let scheme = match scheme.as_str() {
            "file" => GitRemoteScheme::File,
            "http" => GitRemoteScheme::Http,
            "https" => GitRemoteScheme::Https,
            "ssh" => GitRemoteScheme::Ssh,
            "git" => GitRemoteScheme::Git,
            other => {
                return Err(invalid(&format!(
                    "unsupported remote scheme {other:?} (allowed: file/http/https/ssh/git)"
                )));
            }
        };
        // Reject any userinfo carrying a password on every scheme,
        // and any userinfo at all off SSH — credentials belong to
        // the app-managed store, never the URL.
        let authority_end = rest.find('/').unwrap_or(rest.len());
        let authority = &rest[..authority_end];
        if authority.contains('@') {
            let userinfo = authority
                .rsplit('@')
                .next()
                .map_or("", |_| authority.split('@').next().unwrap_or(""));
            let _ = userinfo;
            let at = authority.rfind('@').expect("checked");
            let userinfo = &authority[..at];
            if userinfo.contains(':') || scheme != GitRemoteScheme::Ssh {
                return Err(invalid("remote url must not embed credentials (userinfo)"));
            }
        }
        if rest.is_empty() {
            return Err(invalid("remote url has an empty authority/path"));
        }
        let local_path = (scheme == GitRemoteScheme::File)
            .then(|| file_url_path(rest))
            .flatten();
        Ok(Self {
            raw: text.to_owned(),
            scheme,
            local_path,
        })
    }

    fn parse_scp(text: &str) -> Result<Self, ProviderError> {
        // scp-like: [user@]host:path — exactly one ':' separating
        // host from path, no '/' before it, no whitespace/control.
        let colon = text.find(':').expect("checked");
        let (host_part, path) = text.split_at(colon);
        let path = &path[1..];
        // A path starting with ':' means a `scheme::` transport form
        // (`ext::`, `fd::`) smuggled past the `://` check — reject.
        if host_part.is_empty() || path.is_empty() || path.starts_with(':') {
            return Err(invalid("malformed scp-like remote"));
        }
        if host_part.contains('/') || path.contains('\0') {
            return Err(invalid("malformed scp-like remote"));
        }
        if text.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return Err(invalid("scp-like remote contains whitespace/control"));
        }
        Ok(Self {
            raw: text.to_owned(),
            scheme: GitRemoteScheme::Ssh,
            local_path: None,
        })
    }

    /// Local filesystem path remote → `file://` URL.
    fn from_local_path(path: &Path) -> Result<Self, ProviderError> {
        let canonical = path.canonicalize().map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::NotFound,
                format!("remote path {}: {e}", path.display()),
            )
        })?;
        if !canonical.is_dir() {
            return Err(invalid(&format!(
                "remote path {} is not a directory",
                canonical.display()
            )));
        }
        let url = path_to_file_url(&canonical)?;
        Ok(Self {
            raw: url.clone(),
            scheme: GitRemoteScheme::File,
            local_path: Some(canonical),
        })
    }

    /// The URL text for git argv.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// The transport family.
    #[must_use]
    pub const fn scheme(&self) -> GitRemoteScheme {
        self.scheme
    }

    /// `<remote>/lfs/objects` root for local transports.
    #[must_use]
    pub fn lfs_objects_root(&self) -> Option<PathBuf> {
        self.local_path
            .as_ref()
            .map(|p| p.join("lfs").join("objects"))
    }

    /// `true` when LFS objects can be read from the remote's local
    /// object store (file transport only this slice).
    #[must_use]
    pub fn has_local_lfs(&self) -> bool {
        self.lfs_objects_root().is_some()
    }
}

fn invalid(detail: &str) -> ProviderError {
    ProviderError::new(ProviderErrorKind::InvalidRequest, detail.to_owned())
}

/// scp-like syntax detection: a `:` before the first `/`, with a
/// non-empty host part and no `://` (handled by caller).
fn looks_like_scp(text: &str) -> bool {
    match (text.find(':'), text.find('/')) {
        (Some(colon), Some(slash)) => colon < slash,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// Local-filesystem-path forms that must never reach scp parsing:
/// Windows drive paths (`C:\…`, `C:/…`), verbatim paths
/// (`\\?\C:\…`), and UNC paths (`\\server\share`, `//server/share`).
fn looks_like_local_path(text: &str) -> bool {
    let b = text.as_bytes();
    if text.starts_with("\\\\?\\")
        || text.starts_with("//?/")
        || text.starts_with("\\\\")
        || text.starts_with("//")
    {
        return true;
    }
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/')
}

/// Converts a `file://` URL's path part into a local `PathBuf`.
/// Handles Windows `file:///C:/...` and percent-encoding.
fn file_url_path(rest: &str) -> Option<PathBuf> {
    // rest = everything after "file://" — may start with '/' (empty
    // authority) or 'C:/' style. Take up to first '?'/'#'.
    let path = rest.split(['?', '#']).next().unwrap_or(rest);
    let decoded = percent_decode(path);
    if decoded.is_empty() {
        return None;
    }
    #[cfg(windows)]
    {
        // "/C:/x" → "C:/x"; "//server/share" stays UNC-ish.
        let d = decoded.as_str();
        if let Some(stripped) = d.strip_prefix('/')
            && stripped.len() >= 3
            && stripped.as_bytes()[1] == b':'
            && matches!(stripped.as_bytes()[2], b'/' | b'\\')
        {
            return Some(PathBuf::from(stripped.replace('/', "\\")));
        }
        Some(PathBuf::from(d.replace('/', "\\")))
    }
    #[cfg(not(windows))]
    {
        Some(PathBuf::from(decoded))
    }
}

/// Minimal percent-decoding for file URL paths (`%20`, `%25`, ...).
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Absolute local path → `file://` URL (forward slashes;
/// Windows drive gets the `/C:/` form git expects).
fn path_to_file_url(path: &Path) -> Result<String, ProviderError> {
    let mut s = path.to_string_lossy().replace('\\', "/");
    // `std::fs::canonicalize` on Windows yields verbatim \\?\…
    // paths; git's file:// transport expects the plain drive or
    // `//server/share` UNC spelling instead.
    if let Some(rest) = s.strip_prefix("//?/UNC/") {
        s = format!("//{rest}");
    } else if let Some(rest) = s.strip_prefix("//?/") {
        s = rest.to_owned();
    }
    if s.starts_with('/') {
        Ok(format!("file://{s}"))
    } else if s.len() >= 3 && s.as_bytes()[1] == b':' {
        // Windows drive path — ensure "/C:/..." under file://.
        Ok(format!("file:///{s}"))
    } else {
        Err(invalid("cannot express local path as file url"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_userinfo_and_bad_scheme() {
        assert!(GitRemoteUrl::parse("https://user:pass@example.com/r.git").is_err());
        assert!(GitRemoteUrl::parse("https://user@example.com/r.git").is_err());
        assert!(GitRemoteUrl::parse("ext::sh -c touch%20pwned").is_err());
        assert!(GitRemoteUrl::parse("fd::17").is_err());
        assert!(GitRemoteUrl::parse("ftp://example.com/r").is_err());
        assert!(GitRemoteUrl::parse("ssh://git@example.com/r.git").is_ok());
        assert!(GitRemoteUrl::parse("git@example.com:org/repo.git").is_ok());
    }

    #[test]
    fn windows_drive_paths_are_local_not_scp() {
        // `C:\repo` carries a colon before any '/' — without the
        // local-path check it would parse as scp host "C".
        for text in ["C:\\repo", "C:/repo", "D:\\"] {
            let url = GitRemoteUrl::parse(text);
            // Either a valid local path (exists) or a NotFound for a
            // missing dir — never an Ssh classification.
            if let Ok(url) = url {
                assert_eq!(url.scheme(), GitRemoteScheme::File, "{text}");
            } else {
                assert_eq!(
                    url.expect_err("must be typed").kind(),
                    ProviderErrorKind::NotFound,
                    "{text}"
                );
            }
        }
    }

    #[test]
    fn file_url_roundtrip() {
        let url = GitRemoteUrl::parse("file:///C:/repo/x.git").unwrap();
        assert_eq!(url.scheme(), GitRemoteScheme::File);
        assert!(url.lfs_objects_root().is_some());
        let url = GitRemoteUrl::parse("file:///tmp/repo/x.git").unwrap();
        assert!(url.lfs_objects_root().is_some());
        let url = GitRemoteUrl::parse("https://example.com/r.git").unwrap();
        assert_eq!(url.scheme(), GitRemoteScheme::Https);
        assert!(url.lfs_objects_root().is_none());
    }
}
