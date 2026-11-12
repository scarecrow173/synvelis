//! Git LFS object access — local store for `file://` remotes and the
//! Git LFS batch API over plain `http://` for remote transports.
//!
//! An LFS pointer names an object by SHA-256 oid + declared size.
//! For `file://` remotes the bytes live under
//! `<remote>/lfs/objects/<oid[0..2]>/<oid[2..4]>/<oid>` and are read
//! directly. For `http://` remotes the bytes are fetched through the
//! LFS batch API: `POST <endpoint>/objects/batch` yields a signed
//! `actions.download.href`, and a streaming `GET` returns the object
//! bytes. `https://` stays [`LfsBackendKind::Unsupported`] — the
//! shared HTTP transport has no TLS stack and never downgrades;
//! SSH/`git://` stay unsupported for the same reason (no LFS HTTP
//! transport or auth route exists for them in this slice).
//!
//! Endpoint derivation follows git-lfs server discovery for clone
//! URLs: the remote path already ending in `.git` gains
//! `/info/lfs`, any other path gains `.git/info/lfs`. `lfs.url`,
//! `remote.<name>.lfsurl`, and `.lfsconfig` overrides are *not*
//! consulted — this provider has no repository-config source, so
//! the clone-URL rule is the only honest derivation.
//!
//! The pointer itself is *never* returned as asset bytes: only a
//! stream that passes oid-hex validation, the batch API's oid/size
//! agreement check, and the downstream SHA-256 `VerifyingReader`
//! becomes fetchable content. Download `href`s may be signed,
//! cross-origin URLs — configured credentials are attached only
//! when the target origin equals the git remote's origin, and
//! action-supplied headers are sent verbatim; signed URL secrets
//! never appear in error details.

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use serde::Deserialize;

use crate::context::ProviderContext;
use crate::digest::io_provider_error;
use crate::error::{ProviderError, ProviderErrorKind};
use crate::git::url::{GitRemoteScheme, GitRemoteUrl};
use crate::id::CredentialKey;
use crate::webdav::http::{self, HttpEndpoint, HttpRequest, HttpResponse, MAX_REDIRECTS};
use crate::webdav::locator;
use crate::webdav::url::{WebDavScheme, WebDavUrl};

/// How LFS objects are reached for this remote.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LfsBackendKind {
    /// `<remote>/lfs/objects` on the local filesystem (file
    /// transport only).
    LocalObjects,
    /// Git LFS batch API + HTTP download over `http://` — no TLS.
    RemoteHttp,
    /// No supported transport — `https://` (no TLS stack), SSH, and
    /// `git://` remotes, or transports without an LFS route.
    Unsupported,
}

/// The LFS object backend selected for a remote.
pub(crate) enum LfsBackend {
    /// Local `lfs/objects` directory under the remote path.
    Local { root: PathBuf },
    /// LFS batch API over plain HTTP for an `http://` remote.
    RemoteHttp {
        /// The validated remote the endpoint derives from.
        remote: GitRemoteUrl,
        /// Credential-store key resolved per fetch — attached only
        /// to requests targeting the remote's own origin.
        credential_key: Option<CredentialKey>,
    },
    /// Backend unavailable for this transport.
    Unsupported,
}

/// A resolved plain-HTTP request target: validated origin plus a
/// decoded absolute path and an optional raw (still-encoded) query.
#[derive(Clone)]
struct UrlTarget {
    /// Validated `http(s)` origin — `https` is rejected before any
    /// socket is opened (no TLS stack).
    origin: WebDavUrl,
    /// Decoded absolute path (`/...`), re-encoded at serialization.
    path: String,
    /// Raw query string without `?`, emitted verbatim.
    query: Option<String>,
}

/// Bound on the batch API JSON body — a single-object reply is a
/// few hundred bytes; anything larger is a hostile or broken peer.
const BATCH_BODY_LIMIT: u64 = 1 << 20;

/// LFS batch/download media type (`application/vnd.git-lfs+json`).
const LFS_JSON: &str = "application/vnd.git-lfs+json";

#[derive(Deserialize)]
struct BatchReply {
    /// Response hash algorithm — absent means SHA-256 per spec.
    #[serde(default)]
    hash_algo: Option<String>,
    /// Per-object results, in request order (not relied upon).
    #[serde(default)]
    objects: Vec<BatchObject>,
}

#[derive(Deserialize)]
struct BatchObject {
    oid: String,
    /// Declared size — absent on per-object error replies.
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    actions: Option<BatchActions>,
    #[serde(default)]
    error: Option<BatchError>,
}

#[derive(Deserialize)]
struct BatchActions {
    #[serde(default)]
    download: Option<BatchAction>,
}

#[derive(Deserialize)]
struct BatchAction {
    href: String,
    /// Extra request headers the server requires for this action —
    /// emitted verbatim (string values only).
    #[serde(default)]
    headers: Option<serde_json::Map<String, serde_json::Value>>,
    // `expires_in`/`expires_at` are advisory metadata for the signed
    // URL: the href is used as supplied and an expired URL simply
    // fails as a typed HTTP error on the GET — never retried,
    // refreshed, or rewritten here.
}

#[derive(Deserialize)]
struct BatchError {
    code: u64,
    // `message` is deliberately not modeled: server free text can
    // embed the signed URL or other secrets — it never reaches an
    // error detail.
}

impl LfsBackend {
    /// Selects the backend for `remote`.
    ///
    /// `credential_key` is the configured credential-store key used
    /// by the HTTP backend; `file://` remotes ignore it (local reads
    /// need no auth).
    #[must_use]
    pub(crate) fn for_remote(remote: &GitRemoteUrl, credential_key: Option<CredentialKey>) -> Self {
        if let Some(root) = remote.lfs_objects_root() {
            return Self::Local { root };
        }
        if remote.scheme() == GitRemoteScheme::Http {
            return Self::RemoteHttp {
                remote: remote.clone(),
                credential_key,
            };
        }
        Self::Unsupported
    }

    /// The declared backend kind for `GitProviderInfo`.
    #[must_use]
    pub const fn kind(&self) -> LfsBackendKind {
        match self {
            Self::Local { .. } => LfsBackendKind::LocalObjects,
            Self::RemoteHttp { .. } => LfsBackendKind::RemoteHttp,
            Self::Unsupported => LfsBackendKind::Unsupported,
        }
    }

    /// Opens the LFS object `oid` (64-hex SHA-256) for reading.
    ///
    /// `expected_size` is the pointer's declared size when known: a
    /// mismatch fails *before* streaming — a corrupt or swapped
    /// object never reaches the verifier as plausible bytes. The
    /// remote backend additionally *requires* it, because the batch
    /// API object entry is the `(oid, size)` pair; `None` skips the
    /// local pre-check (the SHA-256 verifier still applies
    /// downstream).
    ///
    /// # Errors
    /// [`ProviderErrorKind::UnsupportedCapability`] for transports
    /// without an LFS route (`https`, SSH, `git`); `NotFound` for a
    /// missing object; `IntegrityMismatch` for a size mismatch;
    /// `AuthExpired`/`PermissionDenied` for auth failures;
    /// `InvalidRequest` for a malformed oid.
    pub(crate) fn open_object(
        &self,
        oid: &str,
        expected_size: Option<u64>,
        cx: &ProviderContext,
    ) -> Result<Box<dyn Read + Send>, ProviderError> {
        cx.check_interrupts()?;
        if let Self::Unsupported = self {
            return Err(ProviderError::new(
                ProviderErrorKind::UnsupportedCapability,
                "git-lfs object download is unsupported for this remote transport",
            ));
        }
        if oid.len() != 64 || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "malformed git-lfs object id",
            ));
        }
        match self {
            Self::Local { root } => {
                open_local(root, oid, expected_size).map(|f| Box::new(f) as Box<dyn Read + Send>)
            }
            Self::RemoteHttp {
                remote,
                credential_key,
            } => open_remote(remote, credential_key.as_ref(), oid, expected_size, cx),
            Self::Unsupported => unreachable!("unsupported checked above"),
        }
    }
}

/// Local `lfs/objects` open — the pre-existing `file://` path,
/// unchanged.
fn open_local(
    root: &std::path::Path,
    oid: &str,
    expected_size: Option<u64>,
) -> Result<File, ProviderError> {
    // LFS layout: objects/<oid[0..2]>/<oid[2..4]>/<oid> — the
    // file name is the full oid inside the two-level fan-out.
    let (a, rest) = oid.split_at(2);
    let (b, _) = rest.split_at(2);
    let path: PathBuf = root.join(a).join(b).join(oid);
    let file = File::open(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ProviderError::new(
            ProviderErrorKind::NotFound,
            format!("git-lfs object {oid} is not present on the remote"),
        ),
        std::io::ErrorKind::PermissionDenied => ProviderError::new(
            ProviderErrorKind::PermissionDenied,
            format!("git-lfs object {oid} is not readable"),
        ),
        _ => ProviderError::new(
            ProviderErrorKind::TransientIo,
            format!("git-lfs object open failed: {e}"),
        ),
    })?;
    let len = file
        .metadata()
        .map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::TransientIo,
                format!("git-lfs object stat failed: {e}"),
            )
        })?
        .len();
    if let Some(expected) = expected_size
        && len != expected
    {
        return Err(ProviderError::new(
            ProviderErrorKind::IntegrityMismatch,
            format!("git-lfs object {oid} size {len} != pointer-declared {expected}"),
        ));
    }
    Ok(file)
}

/// Remote fetch: batch API → signed `href` → streaming GET. The
/// returned reader is wrapped downstream by the SHA-256
/// `VerifyingReader`, which remains the authoritative integrity
/// check — this layer only guarantees the stream is the object the
/// server named.
fn open_remote(
    remote: &GitRemoteUrl,
    credential_key: Option<&CredentialKey>,
    oid: &str,
    expected_size: Option<u64>,
    cx: &ProviderContext,
) -> Result<Box<dyn Read + Send>, ProviderError> {
    let Some(size) = expected_size else {
        return Err(ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            "git-lfs remote fetch requires the pointer-declared size",
        ));
    };
    let (origin, path) = remote_parts(remote)?;
    let auth = resolve_auth(credential_key, cx)?;

    // 1. Batch API: ask for a download action for (oid, size).
    let batch = UrlTarget {
        origin: origin.clone(),
        path: lfs_batch_path(&path)?,
        query: None,
    };
    let body = serde_json::json!({
        "operation": "download",
        "transfers": ["basic"],
        "objects": [{"oid": oid, "size": size}],
    })
    .to_string()
    .into_bytes();
    let headers = vec![
        ("accept".to_owned(), LFS_JSON.to_owned()),
        ("content-type".to_owned(), LFS_JSON.to_owned()),
    ];
    let mut resp = request(
        &origin,
        auth.as_deref(),
        "POST",
        batch,
        &headers,
        Some(body),
        cx,
    )?;
    ensure_status(resp.status, "git-lfs batch request")?;
    let reply = parse_batch(&mut *resp.body, cx)?;
    if let Some(algo) = &reply.hash_algo
        && !algo.eq_ignore_ascii_case("sha256")
    {
        return Err(ProviderError::unsupported(
            "git-lfs hash algorithm other than sha256",
            "git remote provider",
        ));
    }
    let object = reply.objects.iter().find(|o| o.oid == oid).ok_or_else(|| {
        ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            "git-lfs batch response lacks the requested object",
        )
    })?;
    // A per-object error is the server's explicit rejection — it
    // takes precedence over size/action consistency checks.
    if let Some(err) = &object.error {
        return Err(status_error(err.code, &format!("git-lfs object {oid}")));
    }
    if object.size != Some(size) {
        return Err(ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            "git-lfs batch response object size disagrees with the request",
        ));
    }
    let action = object
        .actions
        .as_ref()
        .and_then(|a| a.download.as_ref())
        .ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "git-lfs batch response has no download action",
            )
        })?;

    // 2. Download: GET the action href (absolute or path-relative),
    // carrying the action's own headers. The configured credential
    // is added inside `request` only when the target is the remote
    // origin — signed cross-origin URLs never see it.
    let target = download_target(&action.href, &origin)?;
    let mut headers: Vec<(String, String)> = action
        .headers
        .as_ref()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                .collect()
        })
        .unwrap_or_default();
    // Server-supplied header names/values are re-validated at the
    // wire boundary; drop anything that could not be emitted.
    headers.retain(|(n, v)| {
        !n.is_empty()
            && n.bytes().all(|b| (0x21..=0x7e).contains(&b) && b != b':')
            && v.bytes().all(|b| b >= 0x20 && b != 0x7f)
    });
    let resp = request(&origin, auth.as_deref(), "GET", target, &headers, None, cx)?;
    ensure_status(resp.status, "git-lfs object download")?;
    // Early consistency check when the server declares a length —
    // the streaming verifier remains authoritative at EOF.
    if let Some(len) = resp
        .header("content-length")
        .and_then(|v| v.trim().parse::<u64>().ok())
        && len != size
    {
        return Err(ProviderError::new(
            ProviderErrorKind::IntegrityMismatch,
            format!("git-lfs object {oid} content-length {len} != pointer-declared {size}"),
        ));
    }
    Ok(resp.body)
}

/// Issues one HTTP request, following bounded redirects. The
/// configured `auth` header value is attached only when the current
/// target origin equals the git remote's origin — download hrefs on
/// other origins (signed CDN URLs) never receive it, and an
/// `authorization` header already supplied by the batch action is
/// never overridden.
///
/// # Errors
/// [`ProviderError`] — transport failures, `UnsupportedCapability`
/// for `https` redirect targets, `Cancelled`/`BudgetExceeded`.
fn request(
    remote_origin: &WebDavUrl,
    auth: Option<&str>,
    method: &'static str,
    target: UrlTarget,
    headers: &[(String, String)],
    body: Option<Vec<u8>>,
    cx: &ProviderContext,
) -> Result<HttpResponse, ProviderError> {
    let mut target = target;
    let mut method = method;
    let mut body = body;
    for hop in 0..=MAX_REDIRECTS {
        cx.check_interrupts()?;
        if target.origin.scheme() == WebDavScheme::Https {
            return Err(ProviderError::unsupported(
                "https transport",
                "git-lfs remote fetch",
            ));
        }
        let mut hs = headers.to_vec();
        if target.origin.same_origin(remote_origin)
            && let Some(value) = auth
            && !hs
                .iter()
                .any(|(n, _)| n.eq_ignore_ascii_case("authorization"))
        {
            hs.push(("authorization".to_owned(), value.to_owned()));
        }
        let endpoint = HttpEndpoint::new(
            target.origin.host(),
            target.origin.port(),
            target.origin.is_ipv6(),
        );
        let resp = http::send_endpoint(
            &endpoint,
            &HttpRequest {
                method,
                path: target.path.clone(),
                query: target.query.clone(),
                headers: hs,
                body: body.clone(),
            },
            &cx.cancel,
            cx.limits.deadline,
        )?;
        let redirect = matches!(resp.status, 301 | 302 | 303 | 307 | 308);
        if !redirect {
            return Ok(resp);
        }
        let Some(location) = resp.header("location").map(str::to_owned) else {
            return Ok(resp);
        };
        if hop == MAX_REDIRECTS {
            return Err(
                ProviderError::new(ProviderErrorKind::TransientIo, "too many redirects")
                    .with_retry(crate::RetryAdvice::Never),
            );
        }
        // 303 always rewrites to GET; 301/302 do for POST per
        // long-standing client behavior; 307/308 preserve.
        if resp.status == 303 || (matches!(resp.status, 301 | 302) && method == "POST") {
            method = "GET";
            body = None;
        }
        target = resolve_location(&target, &location)?;
    }
    Err(
        ProviderError::new(ProviderErrorKind::TransientIo, "too many redirects")
            .with_retry(crate::RetryAdvice::Never),
    )
}

/// Resolves a `Location` header against the current target into a
/// new [`UrlTarget`]. Absolute URLs, protocol-relative URLs,
/// absolute paths, and relative references are accepted; the
/// fragment is dropped (never sent). `https` targets parse fine but
/// are rejected as `UnsupportedCapability` by [`request`].
///
/// # Errors
/// [`ProviderErrorKind::InvalidRequest`] on unusable locations.
fn resolve_location(current: &UrlTarget, location: &str) -> Result<UrlTarget, ProviderError> {
    let invalid = |detail: &str| {
        ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            format!("unsafe git-lfs redirect: {detail}"),
        )
    };
    let location = location.trim();
    if location.is_empty()
        || location.len() > 8192
        || location.bytes().any(|b| b < 0x20 || b == 0x7f)
    {
        return Err(invalid("malformed location"));
    }
    let end = location.find('#').unwrap_or(location.len());
    let location = &location[..end];
    let (origin, raw_path) = if let Some((scheme, rest)) = location.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err(invalid("non-http redirect target"));
        }
        let auth_end = rest.find('/').unwrap_or(rest.len());
        let origin = WebDavUrl::parse(&format!("{scheme}://{}", &rest[..auth_end]))
            .map_err(|_| invalid("bad redirect origin"))?;
        (origin, rest[auth_end..].to_owned())
    } else if let Some(rest) = location.strip_prefix("//") {
        let auth_end = rest.find('/').unwrap_or(rest.len());
        let origin = WebDavUrl::parse(&format!("http://{}", &rest[..auth_end]))
            .map_err(|_| invalid("bad redirect origin"))?;
        (origin, rest[auth_end..].to_owned())
    } else if location.starts_with('/') {
        (current.origin.clone(), location.to_owned())
    } else {
        (
            current.origin.clone(),
            locator::merge_relative(&current.path, location),
        )
    };
    let (raw_path, query) = match raw_path.split_once('?') {
        Some((p, q)) => (p, Some(q.to_owned())),
        None => (raw_path.as_str(), None),
    };
    let raw_path = if raw_path.is_empty() { "/" } else { raw_path };
    let path = locator::percent_decode(raw_path).map_err(|_| invalid("bad redirect path"))?;
    Ok(UrlTarget {
        origin,
        path,
        query,
    })
}

/// Splits an `http://` remote URL into its validated origin and
/// decoded path. The remote was already validated credential-free
/// by [`GitRemoteUrl`]; the origin is re-parsed through
/// [`WebDavUrl`] so host/port/IPv6 grammar stays single-sourced.
fn remote_parts(remote: &GitRemoteUrl) -> Result<(WebDavUrl, String), ProviderError> {
    let invalid = |detail: &str| {
        ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            format!("git-lfs remote url is unusable: {detail}"),
        )
    };
    let raw = remote.as_str();
    let Some((_, rest)) = raw.split_once("://") else {
        return Err(invalid("missing scheme"));
    };
    let auth_end = rest.find('/').unwrap_or(rest.len());
    let origin = WebDavUrl::parse(&format!("http://{}", &rest[..auth_end]))
        .map_err(|e| invalid(e.detail()))?;
    let path_raw = &rest[auth_end..];
    let path_raw = path_raw.split(['?', '#']).next().unwrap_or(path_raw);
    let path_raw = if path_raw.is_empty() { "/" } else { path_raw };
    let path = locator::percent_decode(path_raw).map_err(|_| invalid("bad path"))?;
    Ok((origin, path))
}

/// `<endpoint>/objects/batch` — the batch API target derived from
/// the remote path per git-lfs server discovery.
fn lfs_batch_path(remote_path: &str) -> Result<String, ProviderError> {
    let trimmed = remote_path.trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            "git-lfs endpoint cannot be derived from a path-less remote url",
        ));
    }
    let endpoint = if trimmed.strip_suffix(".git").is_some() {
        format!("{trimmed}/info/lfs")
    } else {
        format!("{trimmed}.git/info/lfs")
    };
    Ok(format!("{endpoint}/objects/batch"))
}

/// Resolves a download `href` into a request target. Accepts
/// absolute `http(s)://` URLs, protocol-relative `//host/…`,
/// absolute paths (resolved against the remote origin), and
/// relative references (merged against the batch endpoint path).
/// `https` and non-HTTP schemes surface as
/// [`ProviderErrorKind::UnsupportedCapability`] — a genuinely
/// missing capability, not a parse failure.
///
/// # Errors
/// [`ProviderError`] — `UnsupportedCapability` for non-plain-HTTP
/// hrefs, `InvalidRequest` for malformed ones.
fn download_target(href: &str, remote_origin: &WebDavUrl) -> Result<UrlTarget, ProviderError> {
    let invalid = |detail: &str| {
        ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            format!("git-lfs download href is unusable: {detail}"),
        )
    };
    let href = href.trim();
    if href.is_empty() || href.len() > 8192 || href.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return Err(invalid("malformed href"));
    }
    let end = href.find('#').unwrap_or(href.len());
    let href = &href[..end];
    let (target, query) = match href.split_once('?') {
        Some((t, q)) => (t, Some(q.to_owned())),
        None => (href, None),
    };
    let (origin, raw_path) = if let Some((scheme, rest)) = target.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        match scheme.as_str() {
            "http" | "https" => {
                let auth_end = rest.find('/').unwrap_or(rest.len());
                let origin = WebDavUrl::parse(&format!("{scheme}://{}", &rest[..auth_end]))
                    .map_err(|_| invalid("bad href origin"))?;
                (origin, rest[auth_end..].to_owned())
            }
            _ => {
                return Err(ProviderError::unsupported(
                    "non-http download href",
                    "git-lfs remote fetch",
                ));
            }
        }
    } else if let Some(rest) = target.strip_prefix("//") {
        let auth_end = rest.find('/').unwrap_or(rest.len());
        let origin = WebDavUrl::parse(&format!("http://{}", &rest[..auth_end]))
            .map_err(|_| invalid("bad href origin"))?;
        (origin, rest[auth_end..].to_owned())
    } else if target.starts_with('/') {
        (remote_origin.clone(), target.to_owned())
    } else {
        // Relative reference — merge against the remote root "/"
        // since the batch path is an API endpoint, not a resource
        // namespace; a bare relative href resolves at the origin
        // root, matching how servers commonly issue them.
        (remote_origin.clone(), format!("/{target}"))
    };
    let raw_path = if raw_path.is_empty() {
        "/"
    } else {
        raw_path.as_str()
    };
    let path = locator::percent_decode(raw_path).map_err(|_| invalid("bad href path"))?;
    Ok(UrlTarget {
        origin,
        path,
        query,
    })
}

/// Resolves the configured credential key into an
/// `Authorization` header *value* — identical contract to the
/// `WebDAV` provider: `None` when no key is configured,
/// `AuthExpired` when a key exists but resolution is impossible or
/// the context is not a valid header value.
fn resolve_auth(
    credential_key: Option<&CredentialKey>,
    cx: &ProviderContext,
) -> Result<Option<String>, ProviderError> {
    let Some(key) = credential_key else {
        return Ok(None);
    };
    let Some(resolver) = &cx.credentials else {
        return Err(ProviderError::new(
            ProviderErrorKind::AuthExpired,
            "git-lfs remote fetch requires credentials but no resolver is configured",
        ));
    };
    let cred = resolver.resolve(key)?;
    let context = cred.context();
    if context.is_empty() || context.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return Err(ProviderError::new(
            ProviderErrorKind::AuthExpired,
            "credential context is not a valid header value",
        ));
    }
    Ok(Some(context.to_owned()))
}

/// Reads the batch response body to EOF under a byte cap. Typed
/// errors wrapped in `io::Error` by the transport body readers are
/// unwrapped back to their [`ProviderError`].
fn parse_batch(
    body: &mut (dyn Read + Send),
    cx: &ProviderContext,
) -> Result<BatchReply, ProviderError> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 8192];
    loop {
        cx.check_interrupts()?;
        match body.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                if buf.len() as u64 + n as u64 > BATCH_BODY_LIMIT {
                    return Err(ProviderError::new(
                        ProviderErrorKind::InvalidRequest,
                        "git-lfs batch response exceeds the size bound",
                    ));
                }
                buf.extend_from_slice(&chunk[..n]);
            }
            Err(e) => {
                if let Some(typed) = e
                    .get_ref()
                    .and_then(|inner| inner.downcast_ref::<ProviderError>())
                {
                    return Err(typed.clone());
                }
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(io_provider_error("git-lfs batch read", &e));
            }
        }
    }
    serde_json::from_slice::<BatchReply>(&buf).map_err(|_| {
        ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            "git-lfs batch response is not valid JSON",
        )
    })
}

/// `true` for 2xx; otherwise the typed error for the status —
/// shared by the batch and download legs.
fn ensure_status(status: u16, what: &str) -> Result<(), ProviderError> {
    if (200..=299).contains(&status) {
        Ok(())
    } else {
        Err(status_error(u64::from(status), what))
    }
}

/// Maps an HTTP status (or a batch per-object `error.code`, which
/// uses the same numbering) to a typed provider error.
fn status_error(status: u64, what: &str) -> ProviderError {
    let detail = format!("{what} failed with status {status}");
    match status {
        401 => ProviderError::new(ProviderErrorKind::AuthExpired, detail),
        403 => ProviderError::new(ProviderErrorKind::PermissionDenied, detail),
        404 => ProviderError::new(ProviderErrorKind::NotFound, detail),
        405 => ProviderError::new(
            ProviderErrorKind::UnsupportedCapability,
            format!("{detail} (method not allowed)"),
        ),
        409 | 500..=599 => ProviderError::new(ProviderErrorKind::TransientIo, detail),
        412 | 416 => ProviderError::new(ProviderErrorKind::PreconditionFailed, detail),
        429 => ProviderError::new(ProviderErrorKind::RateLimited, detail),
        _ => ProviderError::new(ProviderErrorKind::InvalidRequest, detail),
    }
}
