//! `WebDAV` `PROPFIND` request body and `multistatus` parser.
//!
//! The parser is a small recursive-descent reader over the
//! [`crate::webdav::xml`] event stream producing one [`DavEntry`]
//! per `response` element. It is bounded three ways: at most
//! `entry_cap` entries (the extra response that would exceed it sets
//! `truncated`), at most `body_cap` response bytes (the wrapped
//! reader fails `BudgetExceeded`), and the XML layer's own
//! text/markup bounds.
//!
//! Semantics kept honest:
//!
//! - Only properties inside a `propstat` whose `status` is `200` are
//!   merged — a `404` propstat's props are "not found", not values.
//! - A `response` without an `href` is skipped and counted, never
//!   fabricated.
//! - `resourcetype` containing `collection` marks the entry as a
//!   container; everything else is treated as a file.

use std::io::Read;
use std::time::Instant;

use crate::context::CancellationToken;
use crate::error::{ProviderError, ProviderErrorKind};
use crate::webdav::xml::{XmlEvent, XmlReader};

/// The PROPFIND Depth header value.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Depth {
    /// The resource itself only (`stat`, `pin`).
    Zero,
    /// The resource plus direct children (`browse`).
    One,
}

impl Depth {
    /// The header text.
    #[must_use]
    pub(crate) const fn header(self) -> &'static str {
        match self {
            Self::Zero => "0",
            Self::One => "1",
        }
    }
}

/// The serialized PROPFIND body asking for the props the provider
/// consumes.
#[must_use]
pub(crate) fn propfind_body() -> Vec<u8> {
    b"<?xml version=\"1.0\" encoding=\"utf-8\"?>\
      <d:propfind xmlns:d=\"DAV:\">\
      <d:prop>\
      <d:resourcetype/>\
      <d:getcontentlength/>\
      <d:getlastmodified/>\
      <d:getetag/>\
      <d:displayname/>\
      </d:prop>\
      </d:propfind>"
        .to_vec()
}

/// One `response`'s observed properties. `href` is the raw (still
/// encoded, possibly relative) href text — resolution is the
/// caller's job so origin checks stay in one place.
#[derive(Clone, Debug, Default)]
pub(crate) struct DavEntry {
    /// Raw `D:href` text.
    pub href: Option<String>,
    /// `resourcetype` contained `collection`.
    pub is_collection: bool,
    /// Raw `getetag` entity-tag text (quotes included).
    pub etag: Option<String>,
    /// `getcontentlength` parsed as bytes.
    pub content_length: Option<u64>,
    /// Raw `getlastmodified` HTTP-date text.
    pub last_modified: Option<String>,
    /// `displayname` text when present.
    pub displayname: Option<String>,
}

/// The outcome of a bounded `multistatus` parse.
pub(crate) struct PropfindOutcome {
    /// Parsed entries in document order.
    pub entries: Vec<DavEntry>,
    /// Responses skipped for missing href.
    pub skipped: u32,
    /// `true` when the entry cap stopped the parse early — the
    /// listing is known-incomplete.
    pub truncated: bool,
}

/// Properties buffered from one `propstat`.
#[derive(Default)]
struct PropBuf {
    etag: Option<String>,
    content_length: Option<u64>,
    last_modified: Option<String>,
    displayname: Option<String>,
    is_collection: bool,
}

fn malformed(detail: &str) -> ProviderError {
    ProviderError::new(
        ProviderErrorKind::Internal,
        format!("malformed multistatus: {detail}"),
    )
}

/// Parses a `multistatus` body into bounded entries.
///
/// `entry_cap` bounds responses; the `(cap + 1)`th response sets
/// `truncated` and stops the parse. `body_cap` bounds response bytes
/// through the wrapped reader (excess fails `BudgetExceeded`, never
/// silent truncation).
///
/// # Errors
/// [`ProviderError`] — `Internal` on malformed XML/structure,
/// `BudgetExceeded` on oversized input, or the wrapped read's
/// typed error.
pub(crate) fn parse_propfind<R: Read>(
    reader: R,
    entry_cap: u32,
    body_cap: u64,
    cancel: &CancellationToken,
    deadline: Option<Instant>,
) -> Result<PropfindOutcome, ProviderError> {
    let bounded = BoundedReader::new(reader, body_cap, cancel, deadline);
    let mut xml = XmlReader::new(bounded);
    let mut outcome = PropfindOutcome {
        entries: Vec::new(),
        skipped: 0,
        truncated: false,
    };

    // Find the root element — must be `multistatus`.
    loop {
        match xml.next()? {
            Some(XmlEvent::Start { name, empty: false }) if name == "multistatus" => break,
            Some(XmlEvent::Start {
                name: n,
                empty: true,
            }) if n == "multistatus" => {
                // `<multistatus/>` — a valid empty listing.
                return Ok(outcome);
            }
            Some(XmlEvent::Start { name, .. }) => {
                return Err(malformed(&format!(
                    "root element {name:?} is not multistatus"
                )));
            }
            Some(XmlEvent::End { name }) => {
                return Err(malformed(&format!(
                    "unexpected end tag {name:?} before root"
                )));
            }
            Some(XmlEvent::Text(_)) => {}
            None => return Err(malformed("empty document")),
        }
    }

    // Inside multistatus: responses and nothing else matters.
    loop {
        match xml.next()? {
            Some(XmlEvent::Start { name, empty: false }) if name == "response" => {
                if outcome.entries.len() >= usize::try_from(entry_cap).unwrap_or(usize::MAX) {
                    outcome.truncated = true;
                    return Ok(outcome);
                }
                parse_response(&mut xml, &mut outcome)?;
            }
            Some(XmlEvent::Start {
                name: n,
                empty: true,
            }) if n == "response" => {
                // `<response/>` carries no href — count the skip.
                outcome.skipped += 1;
            }
            Some(XmlEvent::Start { empty, .. }) => {
                // Unknown/misc element — skip its subtree (or it is
                // self-closing and needs no skip).
                if !empty {
                    skip_subtree(&mut xml)?;
                }
            }
            Some(XmlEvent::End { name }) if name == "multistatus" => {
                return Ok(outcome);
            }
            Some(XmlEvent::End { name }) => {
                return Err(malformed(&format!("unexpected end tag {name:?}")));
            }
            Some(XmlEvent::Text(_)) => {}
            None => return Err(malformed("multistatus not closed")),
        }
    }
}

/// Parses one `response` element through its end tag, pushing the
/// resulting entry (or counting a skip) into `outcome`.
fn parse_response<R: Read>(
    xml: &mut XmlReader<R>,
    outcome: &mut PropfindOutcome,
) -> Result<(), ProviderError> {
    let mut entry = DavEntry::default();
    loop {
        match xml.next()? {
            Some(XmlEvent::Start { name, empty: false }) if name == "href" => {
                entry.href = non_empty_text(&read_elem_text(xml, "href")?);
            }
            Some(XmlEvent::Start { name, empty: true }) if name == "href" => {}
            Some(XmlEvent::Start { name, empty: false }) if name == "propstat" => {
                merge_propstat(xml, &mut entry)?;
            }
            Some(XmlEvent::Start { empty, .. }) => {
                if !empty {
                    skip_subtree(xml)?;
                }
            }
            Some(XmlEvent::End { name }) if name == "response" => {
                if entry.href.is_none() {
                    outcome.skipped += 1;
                } else {
                    outcome.entries.push(entry);
                }
                return Ok(());
            }
            Some(XmlEvent::End { name }) => {
                return Err(malformed(&format!(
                    "unexpected end tag {name:?} in response"
                )));
            }
            Some(XmlEvent::Text(_)) => {}
            None => return Err(malformed("response not closed")),
        }
    }
}

/// Parses one `propstat`, merging its props into `entry` only when
/// the status code is `200`.
fn merge_propstat<R: Read>(
    xml: &mut XmlReader<R>,
    entry: &mut DavEntry,
) -> Result<(), ProviderError> {
    let mut status: Option<u16> = None;
    let mut buf = PropBuf::default();
    loop {
        match xml.next()? {
            Some(XmlEvent::Start { name, empty: false }) if name == "prop" => {
                parse_prop(xml, &mut buf)?;
            }
            Some(XmlEvent::Start { name, empty: true }) if name == "prop" => {}
            Some(XmlEvent::Start { name, empty: false }) if name == "status" => {
                let text = read_elem_text(xml, "status")?;
                status = text.split_whitespace().find_map(|t| t.parse::<u16>().ok());
            }
            Some(XmlEvent::Start { empty, .. }) => {
                if !empty {
                    skip_subtree(xml)?;
                }
            }
            Some(XmlEvent::End { name }) if name == "propstat" => {
                if status == Some(200) {
                    merge_buf(entry, buf);
                }
                return Ok(());
            }
            Some(XmlEvent::End { name }) => {
                return Err(malformed(&format!(
                    "unexpected end tag {name:?} in propstat"
                )));
            }
            Some(XmlEvent::Text(_)) => {}
            None => return Err(malformed("propstat not closed")),
        }
    }
}

/// Merges a 200-propstat's buffer into the entry — first-wins per
/// field so an earlier good propstat is not overwritten.
fn merge_buf(entry: &mut DavEntry, buf: PropBuf) {
    if entry.etag.is_none() {
        entry.etag = buf.etag;
    }
    if entry.content_length.is_none() {
        entry.content_length = buf.content_length;
    }
    if entry.last_modified.is_none() {
        entry.last_modified = buf.last_modified;
    }
    if entry.displayname.is_none() {
        entry.displayname = buf.displayname;
    }
    entry.is_collection |= buf.is_collection;
}

/// Parses one `prop` element's children into `buf`.
fn parse_prop<R: Read>(xml: &mut XmlReader<R>, buf: &mut PropBuf) -> Result<(), ProviderError> {
    loop {
        match xml.next()? {
            Some(XmlEvent::Start { name, empty: false }) if name == "resourcetype" => {
                parse_resourcetype(xml, buf)?;
            }
            Some(XmlEvent::Start { name: n, empty })
                if matches!(
                    n.as_str(),
                    "getetag" | "getcontentlength" | "getlastmodified" | "displayname"
                ) =>
            {
                let text = if empty {
                    String::new()
                } else {
                    read_elem_text(xml, &n)?
                };
                let text = non_empty_text(&text);
                match n.as_str() {
                    "getetag" => {
                        if buf.etag.is_none() {
                            buf.etag = text;
                        }
                    }
                    "getcontentlength" => {
                        if buf.content_length.is_none()
                            && let Some(t) = &text
                            && let Ok(len) = t.parse::<u64>()
                        {
                            buf.content_length = Some(len);
                        }
                    }
                    "getlastmodified" => {
                        if buf.last_modified.is_none() {
                            buf.last_modified = text;
                        }
                    }
                    _ => {
                        if buf.displayname.is_none() {
                            buf.displayname = text;
                        }
                    }
                }
            }
            Some(XmlEvent::Start { empty, .. }) => {
                if !empty {
                    skip_subtree(xml)?;
                }
            }
            Some(XmlEvent::End { name }) if name == "prop" => return Ok(()),
            Some(XmlEvent::End { name }) => {
                return Err(malformed(&format!("unexpected end tag {name:?} in prop")));
            }
            Some(XmlEvent::Text(_)) => {}
            None => return Err(malformed("prop not closed")),
        }
    }
}

/// Parses `resourcetype` — only `collection` matters.
fn parse_resourcetype<R: Read>(
    xml: &mut XmlReader<R>,
    buf: &mut PropBuf,
) -> Result<(), ProviderError> {
    loop {
        match xml.next()? {
            Some(XmlEvent::Start { name, .. }) if name == "collection" => {
                buf.is_collection = true;
            }
            Some(XmlEvent::Start { empty, .. }) => {
                if !empty {
                    skip_subtree(xml)?;
                }
            }
            Some(XmlEvent::End { name }) if name == "resourcetype" => return Ok(()),
            Some(XmlEvent::End { name }) if name == "collection" => {}
            Some(XmlEvent::End { name }) => {
                return Err(malformed(&format!(
                    "unexpected end tag {name:?} in resourcetype"
                )));
            }
            Some(XmlEvent::Text(_)) => {}
            None => return Err(malformed("resourcetype not closed")),
        }
    }
}

/// Reads a leaf element's text through its end tag. `empty` start
/// tags yield an empty string without consuming an end tag.
fn read_elem_text<R: Read>(xml: &mut XmlReader<R>, name: &str) -> Result<String, ProviderError> {
    let mut out = String::new();
    loop {
        match xml.next()? {
            Some(XmlEvent::Text(t)) => out.push_str(&t),
            Some(XmlEvent::Start { empty, .. }) => {
                if !empty {
                    skip_subtree(xml)?;
                }
            }
            Some(XmlEvent::End { name: n }) if n == name => return Ok(out),
            Some(XmlEvent::End { name: n }) => {
                return Err(malformed(&format!("unexpected end tag {n:?} in {name}")));
            }
            None => return Err(malformed(&format!("{name} not closed"))),
        }
    }
}

/// Skips events through the end tag matching the most recently
/// consumed start tag (depth-counted).
fn skip_subtree<R: Read>(xml: &mut XmlReader<R>) -> Result<(), ProviderError> {
    let mut depth = 1u32;
    loop {
        match xml.next()? {
            Some(XmlEvent::Start { empty, .. }) => {
                if !empty {
                    depth += 1;
                }
            }
            Some(XmlEvent::End { .. }) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            Some(XmlEvent::Text(_)) => {}
            None => return Err(malformed("subtree not closed")),
        }
    }
}

fn non_empty_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

/// Wraps a reader and fails `BudgetExceeded` once `cap` bytes have
/// been pulled — an oversized `multistatus` is a budget violation,
/// never a silently truncated listing.
struct BoundedReader<R> {
    inner: R,
    remaining: u64,
    cancel: CancellationToken,
    deadline: Option<Instant>,
}

impl<R: Read> BoundedReader<R> {
    fn new(inner: R, cap: u64, cancel: &CancellationToken, deadline: Option<Instant>) -> Self {
        Self {
            inner,
            remaining: cap,
            cancel: cancel.clone(),
            deadline,
        }
    }
}

impl<R: Read> Read for BoundedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(std::io::Error::from(ProviderError::new(
                ProviderErrorKind::Cancelled,
                "operation cancelled",
            )));
        }
        if let Some(d) = self.deadline
            && Instant::now() >= d
        {
            return Err(std::io::Error::from(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                "provider deadline exceeded",
            )));
        }
        if self.remaining == 0 {
            // At the bound: probe one byte so a body of *exactly*
            // `cap` bytes still reads as clean EOF while `cap + 1`
            // is caught as an overrun, never silently truncated.
            let mut probe = [0u8; 1];
            return match self.inner.read(&mut probe) {
                Ok(0) => Ok(0),
                Ok(_) => Err(std::io::Error::from(ProviderError::new(
                    ProviderErrorKind::BudgetExceeded,
                    "multistatus body exceeds byte bound",
                ))),
                Err(e) => Err(e),
            };
        }
        let want = usize::try_from(self.remaining)
            .unwrap_or(usize::MAX)
            .min(buf.len());
        let n = self.inner.read(&mut buf[..want])?;
        self.remaining = self.remaining.saturating_sub(n as u64);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn parse(xml: &str, cap: u32) -> Result<PropfindOutcome, ProviderError> {
        let cancel = CancellationToken::new();
        parse_propfind(
            Cursor::new(xml.as_bytes().to_vec()),
            cap,
            1 << 20,
            &cancel,
            None,
        )
    }

    const BODY: &str = "<?xml version=\"1.0\"?>\
        <d:multistatus xmlns:d=\"DAV:\">\
        <d:response>\
          <d:href>/a</d:href>\
          <d:propstat><d:prop>\
            <d:resourcetype><d:collection/></d:resourcetype>\
            <d:getetag>\"e\"</d:getetag>\
            <d:getcontentlength>42</d:getcontentlength>\
          </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat>\
        </d:response>\
        <d:response>\
          <d:href>/b</d:href>\
          <d:propstat><d:prop><d:getetag>\"f\"</d:getetag></d:prop>\
          <d:status>HTTP/1.1 404 Not Found</d:status></d:propstat>\
        </d:response>\
        <d:response><d:prop><d:getetag>\"x\"</d:getetag></d:prop></d:response>\
        </d:multistatus>";

    #[test]
    fn parses_entries_and_skips() {
        let out = parse(BODY, 10).expect("parse");
        assert_eq!(out.entries.len(), 2);
        assert_eq!(out.skipped, 1); // no-href response
        let a = &out.entries[0];
        assert_eq!(a.href.as_deref(), Some("/a"));
        assert!(a.is_collection);
        assert_eq!(a.etag.as_deref(), Some("\"e\""));
        assert_eq!(a.content_length, Some(42));
        // 404 propstat props are "not found" — never merged.
        let b = &out.entries[1];
        assert!(!b.is_collection);
        assert_eq!(b.etag, None);
    }

    #[test]
    fn entry_cap_truncates() {
        let out = parse(BODY, 1).expect("parse");
        assert!(out.truncated);
        assert_eq!(out.entries.len(), 1);
    }

    #[test]
    fn wrong_root_fails() {
        let err = parse("<html></html>", 10).err().expect("root");
        assert_eq!(err.kind(), ProviderErrorKind::Internal);
    }

    #[test]
    fn empty_multistatus_is_valid() {
        let out = parse("<d:multistatus xmlns:d=\"DAV:\"/>", 10).expect("parse");
        assert!(out.entries.is_empty());
    }
}
