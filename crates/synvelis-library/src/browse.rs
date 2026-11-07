//! Browsing and search request/page types plus the page-cursor
//! contract.
//!
//! A page cursor is opaque to callers but self-validating: it binds
//! the provider, the scope/sort/filter fingerprint, the principal, and
//! a provider-owned epoch (browse-session generation for a live
//! filesystem, `ProjectRevision` for the project catalog). Reusing a
//! cursor after the query, principal, provider, or revision changed is
//! rejected with [`ProviderErrorKind::CursorInvalid`].

use serde::{Deserialize, Serialize};

use crate::digest::short_fingerprint;
use crate::error::{ProviderError, ProviderErrorKind};
use crate::id::{OpaqueLocator, ProviderId};
use crate::observation::{AssetObservation, Availability, EntryFacts, ObservedAt};

/// What to list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "kebab-case", deny_unknown_fields)]
pub enum BrowseScope {
    /// The provider's root scope.
    Root,
    /// The subtree/collection under one locator (a container).
    Locator {
        /// The container locator.
        locator: OpaqueLocator,
    },
}

/// Sort field for browse results.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortField {
    /// Display name.
    Name,
    /// Opaque locator (provider order).
    Locator,
    /// Byte length (observed first, unknown last).
    Size,
    /// Modification time (observed first, unknown last).
    Modified,
}

/// Sort direction.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortDirection {
    /// Ascending.
    Asc,
    /// Descending.
    Desc,
}

/// Deterministic sort specification.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SortSpec {
    /// Primary sort field.
    pub field: SortField,
    /// Direction.
    pub direction: SortDirection,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self {
            field: SortField::Locator,
            direction: SortDirection::Asc,
        }
    }
}

/// Optional entry filter.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilterSpec {
    /// Case-insensitive substring on the display name.
    pub name_contains: Option<String>,
    /// Restrict to these entry kinds (`None` = both).
    pub kinds: Option<Vec<EntryKind>>,
    /// Case-insensitive extension filter without the dot (`"wav"`).
    /// Applies to `Asset` entries only.
    pub extensions: Option<Vec<String>>,
}

/// One page of a browse listing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowseRequest {
    /// Scope to list.
    pub scope: BrowseScope,
    /// Maximum entries per page (>= 1).
    pub page_limit: u32,
    /// Continuation cursor from a previous page.
    pub cursor: Option<OpaquePageCursor>,
    /// Ordering.
    pub sort: SortSpec,
    /// Optional filter.
    pub filter: FilterSpec,
}

/// Whether an entry holds fetchable bytes or only groups others.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryKind {
    /// A fetchable object (file, blob).
    Asset,
    /// A browsable container (directory, collection).
    Container,
}

/// One catalog entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowseEntry {
    /// Provider-internal locator of this entry.
    pub locator: OpaqueLocator,
    /// Display name (file name, digest label, ...).
    pub display_name: String,
    /// Asset or container.
    pub kind: EntryKind,
    /// Whether its bytes are reachable.
    pub availability: Availability,
    /// Observed facts.
    pub facts: EntryFacts,
    /// `true` when the entry is reached through a symbolic link.
    pub via_symlink: bool,
}

/// How complete the returned page is relative to the live scope.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Coverage {
    /// The whole scope was listed (zero entries is a legitimate
    /// `Complete` page, not a failure).
    Complete,
    /// The page is real but the scope was truncated (budget, depth,
    /// continuation pending). `next_cursor` continues when present.
    Partial,
    /// Returned from a snapshot that may be behind the live scope.
    Stale,
    /// Served from a cache while the backend is offline.
    OfflineCached,
    /// A search/index capability exists but this scope was never
    /// indexed — distinct from "no results".
    NeverIndexed,
}

/// One page of catalog entries.
#[derive(Clone, Debug, PartialEq)]
pub struct BrowsePage {
    /// Page entries in request order.
    pub entries: Vec<BrowseEntry>,
    /// Continuation cursor — `None` when the listing is done.
    pub next_cursor: Option<OpaquePageCursor>,
    /// Coverage of this page relative to the live scope.
    pub coverage: Coverage,
    /// When this page was produced.
    pub observed_at: ObservedAt,
}

/// A provider-side query.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchQuery {
    /// Free-text query. Providers define what it matches (baseline:
    /// display-name substring for local; unsupported for project).
    pub text: String,
    /// Scope to search.
    pub scope: BrowseScope,
    /// Maximum entries per page.
    pub page_limit: u32,
    /// Continuation cursor from a previous search page.
    pub cursor: Option<OpaquePageCursor>,
}

/// An opaque page cursor. Callers store and return it verbatim.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct OpaquePageCursor(String);

impl OpaquePageCursor {
    /// Wraps an opaque token.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] when the token is empty or
    /// exceeds the opaque-identity bound.
    pub fn new(text: impl Into<String>) -> Result<Self, ProviderError> {
        let text = text.into();
        if text.is_empty() || text.len() > crate::id::MAX_OPAQUE_LEN {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "page cursor token is empty or exceeds the opaque bound",
            ));
        }
        Ok(Self(text))
    }

    /// The raw opaque token.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for OpaquePageCursor {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        Self::new(text).map_err(serde::de::Error::custom)
    }
}

impl std::fmt::Display for OpaquePageCursor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Cursor wire payload — versioned JSON, hex-encoded into the opaque
/// token. Internal: providers build it, [`decode_cursor`] validates it.
#[derive(Serialize, Deserialize)]
pub(crate) struct CursorPayload {
    /// Wire version — only `1` is understood.
    pub v: u8,
    /// Tagged provider id the cursor belongs to.
    pub provider: String,
    /// Scope fingerprint.
    pub scope_fp: String,
    /// Sort+filter fingerprint.
    pub query_fp: String,
    /// Principal fingerprint.
    pub principal_fp: String,
    /// Provider epoch (browse generation / project revision).
    pub epoch: u64,
    /// Entries already emitted (the next page starts after it).
    pub offset: u64,
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[usize::from(b >> 4)] as char);
        out.push(HEX[usize::from(b & 0x0f)] as char);
    }
    out
}

fn hex_decode(text: &str) -> Result<Vec<u8>, ProviderError> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return Err(cursor_invalid("odd-length cursor token"));
    }
    let val = |b: u8| -> Result<u8, ProviderError> {
        match b {
            b'0'..=b'9' => Ok(b - b'0'),
            b'a'..=b'f' => Ok(b - b'a' + 10),
            _ => Err(cursor_invalid("non-hex cursor token")),
        }
    };
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.as_chunks::<2>().0 {
        out.push((val(pair[0])? << 4) | val(pair[1])?);
    }
    Ok(out)
}

fn cursor_invalid(detail: impl Into<String>) -> ProviderError {
    ProviderError::new(ProviderErrorKind::CursorInvalid, detail.into())
}

/// Encodes a cursor payload into the opaque token.
pub(crate) fn encode_cursor(payload: &CursorPayload) -> Result<OpaquePageCursor, ProviderError> {
    let bytes = serde_json::to_vec(payload).map_err(|e| {
        ProviderError::new(ProviderErrorKind::Internal, format!("cursor encode: {e}"))
    })?;
    OpaquePageCursor::new(hex_encode(&bytes))
}

/// Validates a cursor against the live query context and returns the
/// verified payload.
///
/// # Errors
/// [`ProviderErrorKind::CursorInvalid`] on undecodable tokens, foreign
/// providers, changed scope/query/principal, or an epoch the provider
/// no longer honors.
pub(crate) fn decode_cursor(
    cursor: &OpaquePageCursor,
    provider: ProviderId,
    scope_fp: &str,
    query_fp: &str,
    principal_fp: &str,
    epoch_valid: impl Fn(u64) -> bool,
) -> Result<CursorPayload, ProviderError> {
    let bytes = hex_decode(cursor.as_str())?;
    let payload: CursorPayload =
        serde_json::from_slice(&bytes).map_err(|_| cursor_invalid("undecodable cursor payload"))?;
    if payload.v != 1 {
        return Err(cursor_invalid("unsupported cursor version"));
    }
    if payload.provider != provider.to_tagged() {
        return Err(cursor_invalid("cursor belongs to another provider"));
    }
    if payload.scope_fp != scope_fp
        || payload.query_fp != query_fp
        || payload.principal_fp != principal_fp
    {
        return Err(cursor_invalid(
            "cursor scope/query/principal no longer matches",
        ));
    }
    if !epoch_valid(payload.epoch) {
        return Err(cursor_invalid("cursor epoch is no longer valid"));
    }
    Ok(payload)
}

/// Fingerprint helpers for cursor binding. The fingerprint is a short
/// SHA-256 prefix over canonical part text — it never carries the
/// values themselves.
#[must_use]
pub(crate) fn fingerprint(parts: &[&str]) -> String {
    let joined = parts.join("");
    short_fingerprint(joined.as_bytes())
}

/// Fingerprint of a [`BrowseScope`].
#[must_use]
pub(crate) fn scope_fingerprint(scope: &BrowseScope) -> String {
    match scope {
        BrowseScope::Root => fingerprint(&["root"]),
        BrowseScope::Locator { locator } => fingerprint(&["locator", locator.as_str()]),
    }
}

/// Fingerprint of sort+filter (the "query" a cursor is bound to).
#[must_use]
pub(crate) fn query_fingerprint(sort: SortSpec, filter: &FilterSpec) -> String {
    // JSON keeps the fingerprint stable without exposing values.
    let filter_text = serde_json::to_string(filter).unwrap_or_default();
    fingerprint(&[
        match sort.field {
            SortField::Name => "name",
            SortField::Locator => "locator",
            SortField::Size => "size",
            SortField::Modified => "modified",
        },
        match sort.direction {
            SortDirection::Asc => "asc",
            SortDirection::Desc => "desc",
        },
        &filter_text,
    ])
}

/// Fingerprint of the caller principal.
#[must_use]
pub(crate) fn principal_fingerprint(principal: &crate::PrincipalId) -> String {
    fingerprint(&["principal", principal.as_str()])
}

/// Deterministically sorts a collected entry vec for paging.
/// `Unknown` values sort after observed ones in *both* directions —
/// reversing the direction must not pull unknowns to the front.
/// Locator is always the final tie-break so the order is total.
pub(crate) fn sort_entries(entries: &mut [BrowseEntry], sort: SortSpec) {
    let desc = matches!(sort.direction, SortDirection::Desc);
    entries.sort_by(|a, b| {
        let ord = match sort.field {
            SortField::Name => reverse_if(a.display_name.cmp(&b.display_name), desc),
            SortField::Locator => reverse_if(a.locator.cmp(&b.locator), desc),
            SortField::Size => {
                observation_cmp(&a.facts.byte_len, &b.facts.byte_len, desc, Ord::cmp)
            }
            SortField::Modified => {
                observation_cmp(&a.facts.modified, &b.facts.modified, desc, Ord::cmp)
            }
        };
        ord.then_with(|| a.locator.cmp(&b.locator))
    });
}

fn reverse_if(ord: std::cmp::Ordering, desc: bool) -> std::cmp::Ordering {
    if desc { ord.reverse() } else { ord }
}

fn observation_cmp<T>(
    a: &AssetObservation<T>,
    b: &AssetObservation<T>,
    desc: bool,
    cmp: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a.value(), b.value()) {
        (Some(x), Some(y)) => reverse_if(cmp(x, y), desc),
        // Unknown always sorts last regardless of direction.
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}
