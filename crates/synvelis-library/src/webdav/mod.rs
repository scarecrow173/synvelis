//! `WebDAV` file-server provider (ALIB-06, Synvelis #90).
//!
//! A read-only [`AssetProvider`] over plain-HTTP `WebDAV` collections.
//! `browse`/`stat` run `PROPFIND` (metadata-first — never a content
//! GET); `pin` accepts *strong* `ETag`s only and produces
//! [`PinProof::ConditionalStrong`] with
//! [`StrongRevisionToken::StrongValidator`] enforced via `If-Match`;
//! `open_pinned` issues a conditional `GET` that fails `412` →
//! `PreconditionFailed` and re-checks the response validator, while
//! `open_preview` uses `Range` when the server honors it (206) and
//! falls back to a bounded client-side skip when it does not (200) —
//! never pretending the server ranged.
//!
//! Safety invariants:
//!
//! - Locators are decoded absolute server paths confined to the
//!   configured `root_path`; traversal, foreign hrefs, and
//!   cross-origin redirects are refused.
//! - Weak or missing `ETag`s can never pin — mutable objects fail
//!   closed with `UnsupportedCapability`.
//! - Credentials resolve through [`ProviderContext::credentials`]
//!   per call and are sent only to the configured origin; nothing
//!   secret is persisted or logged.
//! - `https` parses at configuration time but transport reports
//!   `UnsupportedCapability` — this slice ships no TLS stack.
//! - All network I/O is blocking worker-side; nothing here is safe
//!   to call from an RT callback.

mod dav;
pub(crate) mod http;
pub(crate) mod locator;
pub(crate) mod url;
mod xml;

pub use url::{WebDavScheme, WebDavUrl};

use std::io::Read;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use dav::{DavEntry, Depth, PropfindOutcome};
use http::{HttpRequest, MAX_REDIRECTS};
use locator::{display_name, href_to_path, locator_path, normalize_path, percent_decode};

use crate::browse::{
    BrowseEntry, BrowsePage, BrowseRequest, BrowseScope, Coverage, CursorPayload, EntryKind,
    FilterSpec, decode_cursor, encode_cursor, principal_fingerprint, query_fingerprint,
    scope_fingerprint, sort_entries,
};
use crate::capability::{
    AccessMode, Capability, PinCapability, ProviderCapabilities, UnsupportedReason,
};
use crate::context::ProviderContext;
use crate::digest::{completion_slot, short_fingerprint};
use crate::error::{ProviderError, ProviderErrorKind};
use crate::id::{CredentialKey, OpaqueLocator, ProviderId};
use crate::observation::{
    AssetKind, AssetObservation, AssetStat, Availability, EntryFacts, MediaFacts,
    ObservationOrigin, ObservedAt, UnavailableReason,
};
use crate::provider::{AssetProvider, ByteRange, FetchClaims, PinnedRead, VerificationPolicy};
use crate::refs::{
    ObjectIdentity, PinProof, PinnedAssetRef, PreconditionKind, RemoteAssetRef, StrongRevisionToken,
};
use crate::registry::ProviderKind;

/// Connection configuration for [`WebDavProvider`].
#[derive(Clone, Debug)]
pub struct WebDavConfig {
    /// The validated origin (`http://host[:port]`).
    pub url: WebDavUrl,
    /// Server path prefix every locator must stay inside
    /// (`"/music"`, `"/"` for the whole namespace). Normalized at
    /// construction — `"."`, `".."`, and trailing slashes are
    /// rejected or canonicalized by [`normalize_path`].
    pub root_path: String,
    /// Credential-store key resolved per call into an
    /// `Authorization` header value. Never embedded in URLs,
    /// locators, or persisted state.
    pub credential_key: Option<CredentialKey>,
}

impl WebDavConfig {
    /// A config for `url` rooted at `/` with no credentials.
    #[must_use]
    pub fn new(url: WebDavUrl) -> Self {
        Self {
            url,
            root_path: "/".to_owned(),
            credential_key: None,
        }
    }
}

/// A read-only `WebDAV` [`AssetProvider`].
pub struct WebDavProvider {
    id: ProviderId,
    url: WebDavUrl,
    /// Normalized `root_path` — every locator and href is confined
    /// to this subtree.
    root: String,
    credential_key: Option<CredentialKey>,
}

impl WebDavProvider {
    /// Builds the provider, validating `root_path` up front.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] when `root_path` is not
    /// a valid absolute server path.
    pub fn new(id: ProviderId, config: WebDavConfig) -> Result<Self, ProviderError> {
        let root = normalize_path(&config.root_path)?;
        Ok(Self {
            id,
            url: config.url,
            root,
            credential_key: config.credential_key,
        })
    }

    /// The configured origin.
    #[must_use]
    pub const fn url(&self) -> &WebDavUrl {
        &self.url
    }

    /// The normalized root path all locators are confined to.
    #[must_use]
    pub fn root_path(&self) -> &str {
        &self.root
    }

    fn check_owner(&self, asset: &RemoteAssetRef) -> Result<(), ProviderError> {
        if asset.provider != self.id {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!(
                    "asset belongs to provider {}, not {}",
                    asset.provider, self.id
                ),
            ));
        }
        Ok(())
    }

    /// Locator → confined decoded server path.
    fn asset_path(&self, locator: &OpaqueLocator) -> Result<String, ProviderError> {
        locator_path(locator, &self.root)
    }

    /// Browse scope → confined decoded server path.
    fn scope_path(&self, scope: &BrowseScope) -> Result<String, ProviderError> {
        match scope {
            BrowseScope::Root => Ok(self.root.clone()),
            BrowseScope::Locator { locator } => self.asset_path(locator),
        }
    }

    /// Resolves the configured credential into an `Authorization`
    /// header *value* (the app store decides the scheme — `Basic …`,
    /// `Bearer …`). `None` when no key is configured.
    ///
    /// # Errors
    /// `AuthExpired` when a key is configured but no resolver is
    /// present, resolution fails, or the context is not a valid
    /// single-line header value.
    fn auth_value(&self, cx: &ProviderContext) -> Result<Option<String>, ProviderError> {
        let Some(key) = &self.credential_key else {
            return Ok(None);
        };
        let Some(resolver) = &cx.credentials else {
            return Err(ProviderError::new(
                ProviderErrorKind::AuthExpired,
                "webdav requires credentials but no resolver is configured",
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

    /// Issues one HTTP request, following same-origin redirects
    /// (bounded) and mapping `401` to `AuthExpired`. Cross-origin
    /// redirects are refused — credentials never cross origins.
    ///
    /// # Errors
    /// [`ProviderError`] — transport/auth/redirect typed failures.
    fn request(
        &self,
        method: &'static str,
        path: &str,
        headers: &[(String, String)],
        body: Option<&[u8]>,
        cx: &ProviderContext,
    ) -> Result<http::HttpResponse, ProviderError> {
        let auth = self.auth_value(cx)?;
        let mut url = self.url.clone();
        let mut current_path = path.to_owned();
        for hop in 0..=MAX_REDIRECTS {
            cx.check_interrupts()?;
            let mut hs = headers.to_vec();
            // The credential boundary: Authorization only ever goes
            // to the configured origin. Cross-origin redirects are
            // refused in `resolve_redirect`, so this stays true.
            if url.same_origin(&self.url)
                && let Some(value) = &auth
            {
                hs.push(("authorization".to_owned(), value.clone()));
            }
            let resp = http::send(
                &url,
                &HttpRequest {
                    method,
                    path: current_path.clone(),
                    query: None,
                    headers: hs,
                    body: body.map(<[u8]>::to_vec),
                },
                &cx.cancel,
                cx.limits.deadline,
            )?;
            match resp.status {
                401 => {
                    return Err(ProviderError::new(
                        ProviderErrorKind::AuthExpired,
                        if auth.is_some() {
                            "server rejected the configured credentials"
                        } else {
                            "server requires authentication"
                        },
                    ));
                }
                301 | 302 | 307 | 308 => {
                    let Some(location) = resp.header("location").map(str::to_owned) else {
                        return Ok(resp);
                    };
                    if hop == MAX_REDIRECTS {
                        return Err(ProviderError::new(
                            ProviderErrorKind::TransientIo,
                            "too many redirects",
                        )
                        .with_retry(crate::RetryAdvice::Never));
                    }
                    let (new_url, new_path) =
                        self.resolve_redirect(&url, &current_path, &location)?;
                    url = new_url;
                    current_path = new_path;
                }
                // 303 rewrites the method to GET — only meaningful
                // for GET itself here; anything else is refused by
                // the status classifier.
                303 if method == "GET" => {
                    let Some(location) = resp.header("location").map(str::to_owned) else {
                        return Ok(resp);
                    };
                    if hop == MAX_REDIRECTS {
                        return Err(ProviderError::new(
                            ProviderErrorKind::TransientIo,
                            "too many redirects",
                        )
                        .with_retry(crate::RetryAdvice::Never));
                    }
                    let (new_url, new_path) =
                        self.resolve_redirect(&url, &current_path, &location)?;
                    url = new_url;
                    current_path = new_path;
                }
                _ => return Ok(resp),
            }
        }
        Err(
            ProviderError::new(ProviderErrorKind::TransientIo, "too many redirects")
                .with_retry(crate::RetryAdvice::Never),
        )
    }

    /// Resolves a `Location` header against the current request into
    /// `(origin, decoded path)`. Same-origin only — a cross-origin
    /// target is refused so credentials and scope never leak.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] on unparseable,
    /// cross-origin, or out-of-root targets.
    fn resolve_redirect(
        &self,
        current: &WebDavUrl,
        current_path: &str,
        location: &str,
    ) -> Result<(WebDavUrl, String), ProviderError> {
        let invalid = |detail: &str| {
            ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("unsafe redirect: {detail}"),
            )
        };
        let location = location.trim();
        if location.is_empty()
            || location.len() > 4096
            || location.bytes().any(|b| b < 0x20 || b == 0x7f)
        {
            return Err(invalid("malformed location"));
        }
        // Split off query/fragment — DAV identity is the path.
        let end = location.find(['?', '#']).unwrap_or(location.len());
        let location = &location[..end];
        let (new_url, raw_path) = if let Some((scheme, rest)) = location.split_once("://") {
            let auth_end = rest.find('/').unwrap_or(rest.len());
            let origin = format!("{scheme}://{}", &rest[..auth_end]);
            let url = WebDavUrl::parse(&origin).map_err(|_| invalid("bad redirect origin"))?;
            (url, rest[auth_end..].to_owned())
        } else if let Some(rest) = location.strip_prefix("//") {
            let auth_end = rest.find('/').unwrap_or(rest.len());
            let origin = format!("{}://{}", current.scheme().as_str(), &rest[..auth_end]);
            let url = WebDavUrl::parse(&origin).map_err(|_| invalid("bad redirect origin"))?;
            (url, rest[auth_end..].to_owned())
        } else if location.starts_with('/') {
            (current.clone(), location.to_owned())
        } else {
            // Relative reference — resolve against the current
            // path's directory.
            (
                current.clone(),
                locator::merge_relative(current_path, location),
            )
        };
        if !new_url.same_origin(&self.url) {
            return Err(invalid("cross-origin redirect refused"));
        }
        let raw_path = if raw_path.is_empty() {
            "/".to_owned()
        } else {
            raw_path
        };
        let decoded = percent_decode(&raw_path).map_err(|_| invalid("bad redirect path"))?;
        let path = normalize_path(&decoded).map_err(|_| invalid("bad redirect path"))?;
        if !locator::within_root(&path, &self.root) {
            return Err(invalid("redirect escapes configured root"));
        }
        Ok((new_url, path))
    }

    /// `PROPFIND` on `path` at `depth`, returning the bounded parse.
    ///
    /// # Errors
    /// [`ProviderError`] — `UnsupportedCapability` when the server
    /// does not answer `207 Multi-Status` (not a `WebDAV` server),
    /// transport/auth/typed failures otherwise.
    fn propfind(
        &self,
        path: &str,
        depth: Depth,
        cx: &ProviderContext,
    ) -> Result<PropfindOutcome, ProviderError> {
        let resp = self.request(
            "PROPFIND",
            path,
            &[
                ("depth".to_owned(), depth.header().to_owned()),
                (
                    "content-type".to_owned(),
                    "application/xml; charset=utf-8".to_owned(),
                ),
                ("accept-encoding".to_owned(), "identity".to_owned()),
            ],
            Some(dav::propfind_body().as_slice()),
            cx,
        )?;
        match resp.status {
            207 => {}
            200..=299 => {
                return Err(ProviderError::new(
                    ProviderErrorKind::UnsupportedCapability,
                    "server did not return 207 Multi-Status (not a WebDAV server)",
                ));
            }
            s => return Err(status_error(s, "PROPFIND")),
        }
        let entry_cap = cx.limits.max_entries;
        let body_cap = propfind_body_cap(entry_cap);
        dav::parse_propfind(
            resp.body,
            entry_cap,
            body_cap,
            &cx.cancel,
            cx.limits.deadline,
        )
    }

    /// Resolves parsed entries' hrefs to confined paths. Returns
    /// `(path, entry)` pairs; unresolvable hrefs are dropped and
    /// counted by the caller via `skipped`.
    fn resolve_entries(
        &self,
        collection_path: &str,
        outcome: PropfindOutcome,
    ) -> (Vec<(String, DavEntry)>, u32, bool) {
        let mut resolved = Vec::with_capacity(outcome.entries.len());
        let mut unaddressable = outcome.skipped;
        for entry in outcome.entries {
            let Some(href) = &entry.href else {
                unaddressable += 1;
                continue;
            };
            match href_to_path(href, &self.url, collection_path, &self.root) {
                Some(path) => resolved.push((path, entry)),
                None => unaddressable += 1,
            }
        }
        (resolved, unaddressable, outcome.truncated)
    }

    /// Finds the entry describing `path` itself in a `Depth: 0`
    /// result — `None` when absent (→ `NotFound` upstream).
    fn self_entry<'a>(path: &str, resolved: &'a [(String, DavEntry)]) -> Option<&'a DavEntry> {
        resolved.iter().find(|(p, _)| p == path).map(|(_, e)| e)
    }

    /// `Depth: 0` PROPFIND narrowed to the entry for `path`.
    fn stat_remote(&self, path: &str, cx: &ProviderContext) -> Result<DavEntry, ProviderError> {
        let outcome = self.propfind(path, Depth::Zero, cx)?;
        let (resolved, unaddressable, _truncated) = self.resolve_entries(path, outcome);
        if let Some(entry) = Self::self_entry(path, &resolved) {
            return Ok(entry.clone());
        }
        if unaddressable > 0 && resolved.is_empty() {
            // Every response was unusable — the resource state is
            // unknown rather than absent.
            return Err(ProviderError::new(
                ProviderErrorKind::TransientIo,
                "multistatus responses could not be resolved",
            ));
        }
        Err(ProviderError::new(
            ProviderErrorKind::NotFound,
            format!("no such resource {path}"),
        ))
    }

    /// `EntryFacts` for one resolved entry.
    fn entry_facts(entry: &DavEntry, at: ObservedAt) -> EntryFacts {
        let asset_kind = if entry.is_collection {
            AssetKind::Directory
        } else {
            AssetKind::File
        };
        let byte_len = if entry.is_collection {
            AssetObservation::unknown(UnavailableReason::NotApplicable)
        } else {
            entry.content_length.map_or_else(
                || AssetObservation::unknown(UnavailableReason::NotProvided),
                |len| AssetObservation::observed_at(len, ObservationOrigin::Provider, at),
            )
        };
        let modified = match &entry.last_modified {
            Some(raw) => match parse_http_date(raw) {
                Some(t) => {
                    AssetObservation::observed_at(ObservedAt(t), ObservationOrigin::Provider, at)
                }
                None => AssetObservation::unknown(UnavailableReason::Other {
                    detail: format!("unparseable last-modified {raw:?}"),
                }),
            },
            None => AssetObservation::unknown(UnavailableReason::NotProvided),
        };
        EntryFacts {
            byte_len,
            modified,
            kind: AssetObservation::observed_at(asset_kind, ObservationOrigin::Provider, at),
            checksum_hint: AssetObservation::unknown(UnavailableReason::NotProvided),
            media: MediaFacts::all_unknown(),
        }
    }

    /// Display name: `displayname` when sane, else the path leaf.
    fn display_name_of(entry: &DavEntry, path: &str) -> String {
        if let Some(name) = &entry.displayname {
            let sane = !name.is_empty()
                && !name.contains(['/', '\\'])
                && !name.bytes().any(|b| b < 0x20 || b == 0x7f);
            if sane {
                return name.clone();
            }
        }
        display_name(path)
    }

    /// Whether `etag` is a *weak* entity-tag (`W/` prefix).
    fn is_weak_etag(etag: &str) -> bool {
        etag.trim_start().starts_with("W/")
    }

    /// Whether `etag` is a syntactically valid *strong* entity-tag
    /// (`"..."` with etagc content). Used both on server-provided
    /// values and on caller-supplied pin validators — the pin path
    /// is untrusted input, so this is a security check, not hygiene.
    fn is_strong_entity_tag(etag: &str) -> bool {
        let t = etag.trim();
        if Self::is_weak_etag(t) || t.len() < 2 || !t.starts_with('"') || !t.ends_with('"') {
            return false;
        }
        t[1..t.len() - 1]
            .bytes()
            .all(|b| b == 0x21 || (0x23..=0x7e).contains(&b) || b >= 0x80)
    }

    /// Issues a conditional `GET` for `path` with `If-Match:
    /// validator` and returns the validated response — status `200`
    /// with matching validators only; everything else is typed.
    fn conditional_get(
        &self,
        path: &str,
        validator: &str,
        cx: &ProviderContext,
    ) -> Result<http::HttpResponse, ProviderError> {
        let resp = self.request(
            "GET",
            path,
            &[
                ("if-match".to_owned(), validator.to_owned()),
                ("accept-encoding".to_owned(), "identity".to_owned()),
            ],
            None,
            cx,
        )?;
        match resp.status {
            200 => Ok(resp),
            412 => Err(ProviderError::new(
                ProviderErrorKind::PreconditionFailed,
                "pinned ETag no longer matches the resource",
            )),
            404 => Err(ProviderError::new(
                ProviderErrorKind::RevisionGone,
                "pinned resource is gone",
            )),
            s => Err(status_error(s, "GET")),
        }
    }

    /// Wraps a body stream in the verifying reader + completion slot
    /// shared by `open_pinned`/`open_preview`.
    fn wrap_read(
        inner: Box<dyn Read + Send>,
        expected_digest: Option<crate::ContentDigest>,
        expected_size: Option<u64>,
        verification: VerificationPolicy,
        object_identity: ObjectIdentity,
        cx: &ProviderContext,
    ) -> PinnedRead {
        let slot = completion_slot();
        let reader = crate::digest::VerifyingReader::new(
            inner,
            expected_digest,
            expected_size,
            cx.limits.max_bytes,
            cx.cancel.clone(),
            cx.limits.deadline,
            object_identity.clone(),
            slot.clone(),
        );
        PinnedRead {
            reader: Box::new(reader),
            claims: FetchClaims {
                object_identity,
                expected_size,
                verification,
            },
            completion: crate::FetchCompletion::new(slot),
        }
    }

    /// An empty but well-formed `PinnedRead` (zero-length previews).
    fn empty_read(object_identity: ObjectIdentity, cx: &ProviderContext) -> PinnedRead {
        Self::wrap_read(
            Box::new(std::io::empty()),
            None,
            Some(0),
            VerificationPolicy::Unverified,
            object_identity,
            cx,
        )
    }

    /// Maps resolved `(path, entry)` pairs to filtered `BrowseEntry`s.
    /// Returns the entries plus a count of paths that could not form
    /// a locator (counted toward partial coverage).
    fn collect_entries(
        children: Vec<(String, DavEntry)>,
        at: ObservedAt,
        filter: &FilterSpec,
    ) -> (Vec<BrowseEntry>, u32) {
        let mut mapped = Vec::with_capacity(children.len());
        let mut unaddressable = 0;
        for (path, entry) in children {
            let Ok(locator) = OpaqueLocator::new(path.clone()) else {
                unaddressable += 1;
                continue;
            };
            let kind = if entry.is_collection {
                EntryKind::Container
            } else {
                EntryKind::Asset
            };
            let b = BrowseEntry {
                locator,
                display_name: Self::display_name_of(&entry, &path),
                kind,
                availability: Availability::Available,
                facts: Self::entry_facts(&entry, at),
                via_symlink: false,
            };
            if Self::matches_filter(&b.display_name, b.kind, filter) {
                mapped.push(b);
            }
        }
        (mapped, unaddressable)
    }

    fn matches_filter(name: &str, kind: EntryKind, filter: &FilterSpec) -> bool {
        if let Some(kinds) = &filter.kinds
            && !kinds.contains(&kind)
        {
            return false;
        }
        if let Some(text) = &filter.name_contains
            && !name
                .to_ascii_lowercase()
                .contains(&text.to_ascii_lowercase())
        {
            return false;
        }
        if let Some(exts) = &filter.extensions {
            if kind != EntryKind::Asset {
                return false;
            }
            let ext = name
                .rsplit_once('.')
                .map(|(_, e)| e.to_ascii_lowercase())
                .unwrap_or_default();
            if !exts.iter().any(|want| want.eq_ignore_ascii_case(&ext)) {
                return false;
            }
        }
        true
    }
}

/// Maps an HTTP status to a typed provider error for non-redirect,
/// non-auth responses.
fn status_error(status: u16, what: &str) -> ProviderError {
    let detail = format!("{what} failed with HTTP {status}");
    match status {
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

/// `PROPFIND` body byte bound derived from the entry cap — each
/// entry costs roughly hundreds of XML bytes; the bound is a
/// backstop against pathological payloads, clamped to sane limits.
fn propfind_body_cap(max_entries: u32) -> u64 {
    (u64::from(max_entries) * 4096 + 65_536).clamp(1 << 20, 1 << 28)
}

/// Parses an HTTP-date (`IMF-fixdate`, `RFC 850`, `asctime`) into a
/// `SystemTime`. Returns `None` on any deviation — callers mark the
/// fact unknown rather than guess.
fn parse_http_date(text: &str) -> Option<SystemTime> {
    let toks: Vec<&str> = text.split_whitespace().collect();
    let (year, month, day, hms) = match toks.as_slice() {
        // IMF-fixdate `Sun, 06 Nov 1994 08:49:37 GMT` or asctime
        // `Sun Nov  6 08:49:37 1994` — bindings name the semantic
        // role in each alternative.
        [_, day, mon, year, hms, "GMT"] | [_, mon, day, hms, year] => {
            (parse_year4(year)?, parse_month(mon)?, parse_day(day)?, *hms)
        }
        // RFC 850: `Sunday, 06-Nov-94 08:49:37 GMT`
        [_, date, hms, "GMT"] if date.contains('-') => {
            let mut p = date.split('-');
            let (d, m, y) = (p.next()?, p.next()?, p.next()?);
            if p.next().is_some() {
                return None;
            }
            (parse_year2(y)?, parse_month(m)?, parse_day(d)?, *hms)
        }
        _ => return None,
    };
    let (h, mi, s) = parse_hms(hms)?;
    let days = days_from_civil(i64::from(year), month, day);
    let secs = days * 86_400 + i64::from(h) * 3600 + i64::from(mi) * 60 + i64::from(s);
    if secs >= 0 {
        u64::try_from(secs)
            .ok()
            .and_then(|s| UNIX_EPOCH.checked_add(Duration::from_secs(s)))
    } else {
        UNIX_EPOCH.checked_sub(Duration::from_secs(secs.unsigned_abs()))
    }
}

fn parse_year4(text: &str) -> Option<i32> {
    let y: i32 = text.parse().ok()?;
    (1900..=9999).contains(&y).then_some(y)
}

fn parse_year2(text: &str) -> Option<i32> {
    let y: i32 = text.parse().ok()?;
    if !(0..=99).contains(&y) {
        return None;
    }
    // RFC 850 obs-year: 70-99 → 1900s, 00-69 → 2000s.
    Some(if y >= 70 { 1900 + y } else { 2000 + y })
}

fn parse_month(text: &str) -> Option<u32> {
    Some(match text {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    })
}

fn parse_day(text: &str) -> Option<u32> {
    let d: u32 = text.parse().ok()?;
    (1..=31).contains(&d).then_some(d)
}

fn parse_hms(text: &str) -> Option<(u32, u32, u32)> {
    let mut p = text.split(':');
    let (h, mi, s) = (p.next()?, p.next()?, p.next()?);
    if p.next().is_some() {
        return None;
    }
    let (h, mi, s): (u32, u32, u32) = (h.parse().ok()?, mi.parse().ok()?, s.parse().ok()?);
    if h > 23 || mi > 59 || s > 60 {
        return None;
    }
    Some((h, mi, s))
}

/// Days since 1970-01-01 for a proleptic Gregorian date
/// (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

impl AssetProvider for WebDavProvider {
    fn id(&self) -> ProviderId {
        self.id
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::WebDav
    }

    fn capabilities(&self, _asset: Option<&RemoteAssetRef>) -> ProviderCapabilities {
        ProviderCapabilities {
            browse: Capability::Supported,
            stat: Capability::Supported,
            // No provider-side index exists — search must not fake
            // empty results.
            search: Capability::Unsupported {
                reason: UnsupportedReason::RequiresIndex,
            },
            pin: PinCapability::ConditionalStrong,
            range_read: Capability::Supported,
            preview: Capability::Supported,
            fetch: Capability::Supported,
            // ETag is a validator, not a content digest — no strong
            // digest hint is ever carried on pins.
            checksum_hint: Capability::Unsupported {
                reason: UnsupportedReason::NotSupportedByBackend,
            },
            access: AccessMode::ReadOnly,
        }
    }

    fn browse(
        &self,
        req: &BrowseRequest,
        cx: &ProviderContext,
    ) -> Result<BrowsePage, ProviderError> {
        if req.page_limit == 0 {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "page_limit must be >= 1",
            ));
        }
        cx.check_interrupts()?;
        let scope_fp = scope_fingerprint(&req.scope);
        let query_fp = query_fingerprint(req.sort, &req.filter);
        let principal_fp = principal_fingerprint(&cx.principal);
        let (cursor_offset, cursor_epoch) = if let Some(cursor) = &req.cursor {
            let payload =
                decode_cursor(cursor, self.id, &scope_fp, &query_fp, &principal_fp, |_| {
                    true
                })?;
            (payload.offset, Some(payload.epoch))
        } else {
            (0, None)
        };

        let scope_path = self.scope_path(&req.scope)?;
        let outcome = self.propfind(&scope_path, Depth::One, cx)?;
        let (resolved, mut unaddressable, truncated) = self.resolve_entries(&scope_path, outcome);

        // The scope's own entry must be a collection — browsing a
        // file is a caller error, not an empty listing.
        if let Some(self_e) = Self::self_entry(&scope_path, &resolved)
            && !self_e.is_collection
        {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "browse scope is not a collection",
            ));
        }

        // Children = everything except the scope's self-entry.
        let children: Vec<(String, DavEntry)> = resolved
            .into_iter()
            .filter(|(p, _)| *p != scope_path)
            .collect();

        // Epoch = fingerprint of the sorted child-path membership —
        // a cursor continuation re-lists and compares, so paging is
        // snapshot-consistent without server-side pagination.
        let mut fp_paths: Vec<String> = children
            .iter()
            .map(|(p, e)| format!("{}{}", p, if e.is_collection { 'd' } else { 'f' }))
            .collect();
        fp_paths.sort();
        let epoch = u64::from_str_radix(&short_fingerprint(fp_paths.join("\u{1f}").as_bytes()), 16)
            .unwrap_or(0);
        if let Some(expected) = cursor_epoch
            && expected != epoch
        {
            return Err(ProviderError::new(
                ProviderErrorKind::CursorInvalid,
                "collection membership changed during browse",
            ));
        }

        let at = cx.now();
        let (mut mapped, extra_skipped) = Self::collect_entries(children, at, &req.filter);
        unaddressable += extra_skipped;
        sort_entries(&mut mapped, req.sort);
        let total = mapped.len() as u64;
        let start = usize::try_from(cursor_offset)
            .unwrap_or(usize::MAX)
            .min(mapped.len());
        let end = start
            .saturating_add(usize::try_from(req.page_limit).unwrap_or(usize::MAX))
            .min(mapped.len());
        let page_entries: Vec<BrowseEntry> = mapped[start..end].to_vec();
        let emitted = end as u64;

        let next_cursor = if emitted < total {
            Some(encode_cursor(&CursorPayload {
                v: 1,
                provider: self.id.to_tagged(),
                scope_fp,
                query_fp,
                principal_fp,
                epoch,
                offset: emitted,
            })?)
        } else {
            None
        };
        let coverage = if next_cursor.is_none() && !truncated && unaddressable == 0 {
            Coverage::Complete
        } else {
            Coverage::Partial
        };
        Ok(BrowsePage {
            entries: page_entries,
            next_cursor,
            coverage,
            observed_at: at,
        })
    }

    fn stat(
        &self,
        asset: &RemoteAssetRef,
        cx: &ProviderContext,
    ) -> Result<AssetStat, ProviderError> {
        self.check_owner(asset)?;
        cx.check_interrupts()?;
        let path = self.asset_path(&asset.locator)?;
        let entry = self.stat_remote(&path, cx)?;
        let at = cx.now();
        Ok(AssetStat {
            asset: asset.clone(),
            availability: Availability::Available,
            facts: Self::entry_facts(&entry, at),
            provenance: Vec::new(),
        })
    }

    fn pin(
        &self,
        asset: &RemoteAssetRef,
        cx: &ProviderContext,
    ) -> Result<PinnedAssetRef, ProviderError> {
        self.check_owner(asset)?;
        cx.check_interrupts()?;
        let path = self.asset_path(&asset.locator)?;
        let entry = self.stat_remote(&path, cx)?;
        if entry.is_collection {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "cannot pin a collection (container)",
            ));
        }
        // Fail closed: only a strong ETag is pin evidence. Weak
        // (`W/`) and missing validators can never produce a pin.
        let Some(etag) = entry.etag else {
            return Err(ProviderError::new(
                ProviderErrorKind::UnsupportedCapability,
                format!(
                    "{} has no ETag; cannot pin without a strong validator",
                    asset.locator
                ),
            ));
        };
        if Self::is_weak_etag(&etag) {
            return Err(ProviderError::new(
                ProviderErrorKind::UnsupportedCapability,
                format!(
                    "{} has only a weak ETag; weak validators cannot pin",
                    asset.locator
                ),
            ));
        }
        if !Self::is_strong_entity_tag(&etag) {
            return Err(ProviderError::new(
                ProviderErrorKind::Internal,
                format!("server returned malformed ETag {etag:?}"),
            ));
        }
        let validator = etag.trim().to_owned();
        if let Some(size) = entry.content_length
            && size > cx.limits.max_bytes
        {
            return Err(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                format!(
                    "{} exceeds byte budget {}",
                    asset.locator, cx.limits.max_bytes
                ),
            ));
        }
        Ok(PinnedAssetRef {
            asset: asset.clone(),
            proof: PinProof::ConditionalStrong {
                token: StrongRevisionToken::StrongValidator {
                    validator: validator.clone(),
                    precondition: PreconditionKind::IfMatch,
                },
            },
            object_identity: ObjectIdentity::new(format!("webdav-etag:{validator}"))?,
            expected_size: entry.content_length,
            digest_hint: None,
        })
    }

    fn open_pinned(
        &self,
        pin: &PinnedAssetRef,
        cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        self.check_owner(&pin.asset)?;
        cx.check_interrupts()?;
        let (validator, precondition) = match &pin.proof {
            PinProof::ConditionalStrong {
                token:
                    StrongRevisionToken::StrongValidator {
                        validator,
                        precondition,
                    },
            } => (validator, *precondition),
            _ => {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "webdav provider only opens ConditionalStrong{StrongValidator} pins",
                ));
            }
        };
        if precondition != PreconditionKind::IfMatch {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "webdav provider only supports IfMatch preconditions",
            ));
        }
        // The pin is caller input — the validator becomes a header
        // value, so its entity-tag grammar is a security check.
        if !Self::is_strong_entity_tag(validator) {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "pin validator is not a strong entity-tag",
            ));
        }
        let path = self.asset_path(&pin.asset.locator)?;
        let resp = self.conditional_get(&path, validator.trim(), cx)?;
        // The response validator must agree with the pin: a strong
        // ETag that differs means the entity moved between pin and
        // fetch. A weak or absent response ETag cannot disprove the
        // If-Match evaluation — the precondition already held.
        if let Some(resp_etag) = resp.header("etag") {
            let t = resp_etag.trim();
            if Self::is_weak_etag(t) || t != validator.trim() {
                return Err(ProviderError::new(
                    ProviderErrorKind::SourceChanged,
                    "response ETag differs from the pinned validator",
                ));
            }
        }
        // Content-Length is a precondition hint, not proof — a
        // mismatch means the entity is not the pinned one.
        if let (Some(cl), Some(expected)) = (
            resp.header("content-length")
                .and_then(|v| v.trim().parse::<u64>().ok()),
            pin.expected_size,
        ) && cl != expected
        {
            return Err(ProviderError::new(
                ProviderErrorKind::SourceChanged,
                format!("content-length {cl} != pinned expected_size {expected}"),
            ));
        }
        Ok(Self::wrap_read(
            resp.body,
            None,
            pin.expected_size,
            VerificationPolicy::IdentityBound {
                scheme: "webdav-etag-if-match".to_owned(),
            },
            pin.object_identity.clone(),
            cx,
        ))
    }

    fn open_preview(
        &self,
        asset: &RemoteAssetRef,
        range: Option<ByteRange>,
        cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        self.check_owner(asset)?;
        cx.check_interrupts()?;
        let path = self.asset_path(&asset.locator)?;
        let entry = self.stat_remote(&path, cx)?;
        if entry.is_collection {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "cannot preview a collection (container)",
            ));
        }
        let object_identity = ObjectIdentity::new(asset.locator.as_str())?;
        if let Some(r) = range {
            if r.offset.checked_add(r.len).is_none() {
                return Err(ProviderError::new(
                    ProviderErrorKind::InvalidRequest,
                    "preview range overflows",
                ));
            }
            if r.len == 0 {
                return Ok(Self::empty_read(object_identity, cx));
            }
            if r.offset > cx.limits.max_bytes {
                return Err(ProviderError::new(
                    ProviderErrorKind::BudgetExceeded,
                    "preview offset exceeds byte budget",
                ));
            }
        }
        let mut headers = vec![("accept-encoding".to_owned(), "identity".to_owned())];
        if let Some(r) = range {
            let end = r.offset + r.len - 1;
            headers.push(("range".to_owned(), format!("bytes={}-{end}", r.offset)));
        }
        let resp = self.request("GET", &path, &headers, None, cx)?;
        match resp.status {
            206 => {
                // Server honored Range — verify Content-Range names
                // the requested start before trusting the body.
                let cr = resp.header("content-range").ok_or_else(|| {
                    ProviderError::new(ProviderErrorKind::Internal, "206 without content-range")
                })?;
                let (start, len) = parse_content_range(cr).ok_or_else(|| {
                    ProviderError::new(
                        ProviderErrorKind::Internal,
                        format!("malformed content-range {cr:?}"),
                    )
                })?;
                if let Some(r) = range
                    && start != r.offset
                {
                    return Err(ProviderError::new(
                        ProviderErrorKind::SourceChanged,
                        format!("content-range start {start} != requested {}", r.offset),
                    ));
                }
                // Deliver at most the requested length, never more
                // than the content-range promises.
                let deliver = range.map_or(len, |r| r.len.min(len));
                let inner: Box<dyn Read + Send> = Box::new(
                    resp.body
                        .take(deliver.min(cx.limits.max_bytes.saturating_add(1))),
                );
                Ok(Self::wrap_read(
                    inner,
                    None,
                    Some(deliver),
                    VerificationPolicy::Unverified,
                    object_identity,
                    cx,
                ))
            }
            200 => {
                // Server ignored Range (or none was requested): fall
                // back to a bounded client-side skip — honest bytes,
                // no pretense that the server ranged.
                let mut body = resp.body;
                let cap = match range {
                    Some(r) => {
                        discard(&mut *body, r.offset, cx)?;
                        r.len
                    }
                    None => u64::MAX,
                };
                let inner: Box<dyn Read + Send> =
                    Box::new(body.take(cap.min(cx.limits.max_bytes.saturating_add(1))));
                Ok(Self::wrap_read(
                    inner,
                    None,
                    None,
                    VerificationPolicy::Unverified,
                    object_identity,
                    cx,
                ))
            }
            416 => {
                // Range unsatisfiable — the honest preview is empty.
                Ok(Self::empty_read(object_identity, cx))
            }
            s => Err(status_error(s, "GET")),
        }
    }
}

/// Parses `Content-Range: bytes S-E/T` (or `bytes S-E/*`) into
/// `(start, length)`. `None` on any deviation.
fn parse_content_range(value: &str) -> Option<(u64, u64)> {
    let spec = value.trim().strip_prefix("bytes ")?;
    let (range, _total) = spec.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start: u64 = start.trim().parse().ok()?;
    let end: u64 = end.trim().parse().ok()?;
    if end < start {
        return None;
    }
    Some((start, end - start + 1))
}

/// Discards `count` bytes from `reader` with per-chunk interrupt
/// checks — the client-side half of an ignored `Range`.
fn discard(
    reader: &mut (dyn Read + Send),
    count: u64,
    cx: &ProviderContext,
) -> Result<(), ProviderError> {
    let mut remaining = count;
    let mut buf = [0u8; 8192];
    while remaining > 0 {
        cx.check_interrupts()?;
        let want = usize::try_from(remaining.min(8192)).unwrap_or(8192);
        match reader.read(&mut buf[..want]) {
            Ok(0) => return Ok(()), // EOF — the file is shorter; the
            // remaining take() yields empty.
            Ok(n) => remaining -= n as u64,
            Err(e) => {
                return Err(crate::digest::io_provider_error("preview skip", &e));
            }
        }
    }
    Ok(())
}
