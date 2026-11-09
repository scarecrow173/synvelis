//! A minimal XML pull parser for the `WebDAV` `multistatus` subset.
//!
//! This is deliberately *not* a general XML implementation: it
//! handles elements, attributes, text, CDATA, comments,
//! processing instructions, and `DOCTYPE` internal subsets, decodes
//! the five predefined entities plus numeric character references,
//! and compares element names by their namespace-local part (the
//! prefix before `:` is stripped). Everything else — DTD-driven
//! entity expansion, namespaces beyond prefix stripping, encodings
//! other than UTF-8 — is out of scope and fails closed.
//!
//! The parser is streaming: it reads the source in chunks and emits
//! one [`XmlEvent`] at a time, so a `multistatus` body can be
//! consumed incrementally under an entry/byte budget.

use std::io::Read;

use crate::error::{ProviderError, ProviderErrorKind};

/// Input chunk size for the streaming reader.
const CHUNK: usize = 8192;
/// Bound on a single text run (a `displayname` or prop value can
/// never legitimately approach this).
const TEXT_LIMIT: usize = 1 << 20;
/// Bound on the pending markup buffer (a pathological unclosed tag
/// cannot grow memory without limit).
const MARKUP_LIMIT: usize = 1 << 20;

/// One parser event. `Start` carries `empty = true` for
/// self-closing elements (`<a/>`), which have no matching `End`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum XmlEvent {
    /// `<name ...>` (or `<name .../>` when `empty`).
    Start {
        /// Namespace-local element name.
        name: String,
        /// `true` for self-closing elements.
        empty: bool,
    },
    /// `</name>`.
    End {
        /// Namespace-local element name.
        name: String,
    },
    /// Character data (entity-decoded; CDATA delivered as text).
    Text(String),
}

/// Streaming XML event source over a byte `Read`.
pub(crate) struct XmlReader<R> {
    inner: R,
    /// Bytes not yet consumed by the tokenizer.
    buf: Vec<u8>,
    /// Absolute offset of `buf[0]` in the stream (for errors).
    eof: bool,
}

impl<R: Read> XmlReader<R> {
    /// Wraps `inner`.
    pub(crate) fn new(inner: R) -> Self {
        Self {
            inner,
            buf: Vec::with_capacity(CHUNK * 2),
            eof: false,
        }
    }

    fn fill(&mut self) -> Result<(), ProviderError> {
        if self.eof {
            return Ok(());
        }
        let mut chunk = [0u8; CHUNK];
        match self.inner.read(&mut chunk) {
            Ok(0) => {
                self.eof = true;
                Ok(())
            }
            Ok(n) => {
                self.buf.extend_from_slice(&chunk[..n]);
                Ok(())
            }
            Err(e) => Err(crate::digest::io_provider_error("xml read", &e)),
        }
    }

    /// Ensures at least `n` bytes are buffered or EOF is reached.
    fn need(&mut self, n: usize) -> Result<(), ProviderError> {
        while self.buf.len() < n && !self.eof {
            self.fill()?;
        }
        Ok(())
    }

    fn starts_with(&mut self, pat: &[u8]) -> Result<bool, ProviderError> {
        self.need(pat.len())?;
        Ok(self.buf.len() >= pat.len() && &self.buf[..pat.len()] == pat)
    }

    fn consume(&mut self, n: usize) {
        self.buf.drain(..n);
    }

    /// Finds `pat` in the buffer, refilling as needed. Returns the
    /// index or `None` at EOF.
    fn find(&mut self, pat: &[u8]) -> Result<Option<usize>, ProviderError> {
        loop {
            if let Some(i) = find_subslice(&self.buf, pat) {
                return Ok(Some(i));
            }
            if self.buf.len() > MARKUP_LIMIT {
                return Err(ProviderError::new(
                    ProviderErrorKind::BudgetExceeded,
                    "xml markup run exceeds bound",
                ));
            }
            if self.eof {
                return Ok(None);
            }
            self.fill()?;
        }
    }

    /// The next event, or `None` at end of input.
    ///
    /// # Errors
    /// [`ProviderError`] — malformed markup (`Internal`), oversized
    /// runs (`BudgetExceeded`), or the wrapped read's typed error.
    pub(crate) fn next(&mut self) -> Result<Option<XmlEvent>, ProviderError> {
        loop {
            self.need(1)?;
            if self.buf.is_empty() {
                return Ok(None);
            }
            if self.buf[0] != b'<' {
                return self.next_text().map(Some);
            }
            if self.starts_with(b"<!--")? {
                self.skip_until(b"-->")?;
                continue;
            }
            if self.starts_with(b"<![CDATA[")? {
                return self.next_cdata().map(Some);
            }
            if self.starts_with(b"<?")? {
                self.skip_until(b"?>")?;
                continue;
            }
            if self.starts_with(b"<!")? {
                self.skip_decl()?;
                continue;
            }
            if self.starts_with(b"</")? {
                return self.next_end().map(Some);
            }
            return self.next_start().map(Some);
        }
    }

    /// Reads character data up to the next `<`.
    fn next_text(&mut self) -> Result<XmlEvent, ProviderError> {
        let end = self
            .find(b"<")?
            .ok_or_else(|| Self::unexpected_eof("text"))?;
        let raw = self.buf[..end].to_vec();
        self.consume(end);
        if raw.len() > TEXT_LIMIT {
            return Err(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                "xml text run exceeds bound",
            ));
        }
        Ok(XmlEvent::Text(decode_entities(&raw)?))
    }

    /// Reads `<![CDATA[ ... ]]>` content as a text event.
    fn next_cdata(&mut self) -> Result<XmlEvent, ProviderError> {
        self.consume(b"<![CDATA[".len());
        let end = self
            .find(b"]]>")?
            .ok_or_else(|| Self::unexpected_eof("cdata"))?;
        let raw = self.buf[..end].to_vec();
        self.consume(end + 3);
        if raw.len() > TEXT_LIMIT {
            return Err(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                "xml cdata run exceeds bound",
            ));
        }
        let text = std::str::from_utf8(&raw)
            .map_err(|_| ProviderError::new(ProviderErrorKind::Internal, "non-utf8 xml cdata"))?;
        Ok(XmlEvent::Text(text.to_owned()))
    }

    /// Reads `</name>`.
    fn next_end(&mut self) -> Result<XmlEvent, ProviderError> {
        // Consume through '>' honoring quoted attribute values is
        // unnecessary for end tags — they carry no attributes.
        let end = self
            .find(b">")?
            .ok_or_else(|| Self::unexpected_eof("end tag"))?;
        let inner = String::from_utf8_lossy(&self.buf[2..end]).into_owned();
        self.consume(end + 1);
        let name = local_name(inner.trim());
        if name.is_empty() {
            return Err(Self::malformed("empty end-tag name"));
        }
        Ok(XmlEvent::End { name })
    }

    /// Reads `<name attr="v" ...>` or `<name .../>`.
    fn next_start(&mut self) -> Result<XmlEvent, ProviderError> {
        let end = self
            .find_tag_end()?
            .ok_or_else(|| Self::unexpected_eof("start tag"))?;
        let raw = self.buf[1..end].to_vec();
        self.consume(end + 1);
        let (name, empty) = parse_start_inner(&raw)?;
        Ok(XmlEvent::Start { name, empty })
    }

    /// Finds the `>` terminating a start tag, skipping over quoted
    /// attribute values so a `>` inside quotes does not end it early.
    fn find_tag_end(&mut self) -> Result<Option<usize>, ProviderError> {
        let mut quote: Option<u8> = None;
        let mut i = 0;
        loop {
            self.need(i + 1)?;
            if i >= self.buf.len() {
                if self.eof {
                    return Ok(None);
                }
                if self.buf.len() > MARKUP_LIMIT {
                    return Err(ProviderError::new(
                        ProviderErrorKind::BudgetExceeded,
                        "xml start tag exceeds bound",
                    ));
                }
                self.fill()?;
                continue;
            }
            let b = self.buf[i];
            match quote {
                Some(q) => {
                    if b == q {
                        quote = None;
                    }
                }
                None => match b {
                    b'"' | b'\'' => quote = Some(b),
                    b'>' => return Ok(Some(i)),
                    _ => {}
                },
            }
            i += 1;
        }
    }

    /// Skips `<! ... >` declarations (DOCTYPE and friends). An
    /// internal subset `[...]` may contain `>` so bracket depth is
    /// tracked.
    fn skip_decl(&mut self) -> Result<(), ProviderError> {
        // Consume '<!' then scan for '>' outside brackets/quotes.
        self.consume(2);
        let mut depth = 0i32;
        let mut quote: Option<u8> = None;
        let mut i = 0;
        loop {
            self.need(i + 1)?;
            if i >= self.buf.len() {
                if self.eof {
                    return Err(Self::unexpected_eof("declaration"));
                }
                if self.buf.len() > MARKUP_LIMIT {
                    return Err(ProviderError::new(
                        ProviderErrorKind::BudgetExceeded,
                        "xml declaration exceeds bound",
                    ));
                }
                self.fill()?;
                continue;
            }
            let b = self.buf[i];
            match quote {
                Some(q) => {
                    if b == q {
                        quote = None;
                    }
                }
                None => match b {
                    b'"' | b'\'' => quote = Some(b),
                    b'[' => depth += 1,
                    b']' => depth -= 1,
                    b'>' if depth <= 0 => {
                        self.consume(i + 1);
                        return Ok(());
                    }
                    _ => {}
                },
            }
            i += 1;
        }
    }

    /// Skips through the next `pat` (inclusive).
    fn skip_until(&mut self, pat: &[u8]) -> Result<(), ProviderError> {
        let end = self
            .find(pat)?
            .ok_or_else(|| Self::unexpected_eof("markup"))?;
        self.consume(end + pat.len());
        Ok(())
    }

    fn unexpected_eof(what: &str) -> ProviderError {
        ProviderError::new(
            ProviderErrorKind::Internal,
            format!("unexpected eof inside xml {what}"),
        )
    }

    fn malformed(detail: &str) -> ProviderError {
        ProviderError::new(
            ProviderErrorKind::Internal,
            format!("malformed xml: {detail}"),
        )
    }
}

/// The namespace-local part of a (possibly prefixed) element name.
fn local_name(name: &str) -> String {
    name.rsplit(':').next().unwrap_or(name).to_owned()
}

/// Parses the inside of a start tag: `name attr="v" ...` possibly
/// ending in `/`. Attributes are validated for well-formedness
/// (quoted values) but their values are not needed by the DAV
/// layer, so they are consumed and dropped.
fn parse_start_inner(raw: &[u8]) -> Result<(String, bool), ProviderError> {
    // raw = "name attr=\"v\" ... /" (leading '<' and trailing '>'
    // already removed; a trailing '/' marks self-closing).
    let mut slice = raw;
    let mut empty = false;
    // Trailing '/' before optional whitespace.
    let trimmed_end = slice.iter().rposition(|b| !b.is_ascii_whitespace());
    if let Some(last) = trimmed_end
        && slice[last] == b'/'
    {
        empty = true;
        slice = &slice[..last];
    }
    // Name = up to first whitespace.
    let name_end = slice
        .iter()
        .position(u8::is_ascii_whitespace)
        .unwrap_or(slice.len());
    if name_end == 0 {
        return Err(ProviderError::new(
            ProviderErrorKind::Internal,
            "malformed xml: empty element name",
        ));
    }
    let name =
        local_name(std::str::from_utf8(&slice[..name_end]).map_err(|_| {
            ProviderError::new(ProviderErrorKind::Internal, "non-utf8 element name")
        })?);
    // Validate the attribute tail is well-formed: name="v" / name='v'
    // / name=v sequences separated by whitespace.
    validate_attrs(&slice[name_end..])?;
    Ok((name, empty))
}

/// Validates (without decoding) the attribute tail of a start tag.
fn validate_attrs(tail: &[u8]) -> Result<(), ProviderError> {
    let malformed = || ProviderError::new(ProviderErrorKind::Internal, "malformed xml attributes");
    let mut i = 0;
    while i < tail.len() {
        while i < tail.len() && tail[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= tail.len() {
            return Ok(());
        }
        // Attribute name: up to '=', whitespace, or end.
        let name_start = i;
        while i < tail.len() && !matches!(tail[i], b'=' | b'/') && !tail[i].is_ascii_whitespace() {
            i += 1;
        }
        if i == name_start {
            return Err(malformed());
        }
        while i < tail.len() && tail[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= tail.len() || tail[i] != b'=' {
            return Err(malformed());
        }
        i += 1;
        while i < tail.len() && tail[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= tail.len() {
            return Err(malformed());
        }
        match tail[i] {
            b'"' | b'\'' => {
                let q = tail[i];
                i += 1;
                let close = tail[i..]
                    .iter()
                    .position(|b| *b == q)
                    .ok_or_else(malformed)?;
                i += close + 1;
            }
            _ => {
                // Unquoted value — legal XML forbids it, but be
                // lenient: consume to whitespace.
                while i < tail.len() && !tail[i].is_ascii_whitespace() {
                    i += 1;
                }
            }
        }
    }
    Ok(())
}

/// Decodes the five predefined XML entities and numeric character
/// references in a text run.
fn decode_entities(raw: &[u8]) -> Result<String, ProviderError> {
    let text = std::str::from_utf8(raw)
        .map_err(|_| ProviderError::new(ProviderErrorKind::Internal, "non-utf8 xml text"))?;
    if !text.contains('&') {
        return Ok(text.to_owned());
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let (decoded, consumed) = decode_entity(after)?;
        out.push_str(&decoded);
        rest = &after[consumed..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Decodes one entity reference at the start of `s` (which begins
/// just after `&`). Returns `(decoded text, bytes consumed)`.
fn decode_entity(s: &str) -> Result<(String, usize), ProviderError> {
    let malformed = || ProviderError::new(ProviderErrorKind::Internal, "malformed xml entity");
    for (name, val) in [
        ("amp;", "&"),
        ("lt;", "<"),
        ("gt;", ">"),
        ("quot;", "\""),
        ("apos;", "'"),
    ] {
        if s.starts_with(name) {
            return Ok((val.to_owned(), name.len()));
        }
    }
    if let Some(rest) = s.strip_prefix("#x") {
        let digits = rest.chars().take_while(char::is_ascii_hexdigit).count();
        if digits == 0 {
            return Err(malformed());
        }
        let (num, tail) = rest.split_at(digits);
        if !tail.starts_with(';') {
            return Err(malformed());
        }
        let cp = u32::from_str_radix(num, 16).map_err(|_| malformed())?;
        let ch = char::from_u32(cp).ok_or_else(malformed)?;
        return Ok((ch.to_string(), 2 + digits + 1));
    }
    if let Some(rest) = s.strip_prefix('#') {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 {
            return Err(malformed());
        }
        let (num, tail) = rest.split_at(digits);
        if !tail.starts_with(';') {
            return Err(malformed());
        }
        let cp: u32 = num.parse().map_err(|_| malformed())?;
        let ch = char::from_u32(cp).ok_or_else(malformed)?;
        return Ok((ch.to_string(), 1 + digits + 1));
    }
    Err(malformed())
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn events(xml: &str) -> Vec<XmlEvent> {
        let mut r = XmlReader::new(xml.as_bytes());
        let mut out = Vec::new();
        while let Some(e) = r.next().expect("event") {
            out.push(e);
        }
        out
    }

    #[test]
    fn entities_decode() {
        let ev = events("<a>x &amp; &lt;tag&gt; &#65; &#x42; &quot;q&quot;</a>");
        assert!(ev.contains(&XmlEvent::Text("x & <tag> A B \"q\"".to_owned())));
    }

    #[test]
    fn cdata_delivered_as_text() {
        let ev = events("<a><![CDATA[<raw>&]]></a>");
        assert!(ev.contains(&XmlEvent::Text("<raw>&".to_owned())));
    }

    #[test]
    fn comments_pi_and_doctype_skipped() {
        let ev = events(
            "<?xml version=\"1.0\"?><!-- c -->\
             <!DOCTYPE d:multistatus [<!ENTITY x \"y\">]>\
             <a><?pi data?><b/></a>",
        );
        assert_eq!(
            ev,
            vec![
                XmlEvent::Start {
                    name: "a".to_owned(),
                    empty: false
                },
                XmlEvent::Start {
                    name: "b".to_owned(),
                    empty: true
                },
                XmlEvent::End {
                    name: "a".to_owned()
                },
            ]
        );
    }

    #[test]
    fn malformed_fails_closed() {
        // Unknown entity: the text event errors.
        let mut r = XmlReader::new("<a>&bogus;</a>".as_bytes());
        assert!(matches!(
            r.next().expect("start"),
            Some(XmlEvent::Start { .. })
        ));
        assert!(r.next().is_err());
        // Bad numeric reference: same.
        let mut r = XmlReader::new("<a>&#xZZ;</a>".as_bytes());
        assert!(r.next().is_ok());
        assert!(r.next().is_err());
        // Truncated tag at EOF.
        let mut r = XmlReader::new("<a".as_bytes());
        assert!(r.next().is_err());
        // Unterminated text run at EOF.
        let mut r = XmlReader::new("<a>text".as_bytes());
        assert!(r.next().is_ok());
        assert!(r.next().is_err());
    }

    #[test]
    fn namespace_prefix_stripped() {
        let ev = events("<d:multistatus xmlns:d=\"DAV:\"><d:response/></d:multistatus>");
        assert_eq!(
            ev[0],
            XmlEvent::Start {
                name: "multistatus".to_owned(),
                empty: false
            }
        );
    }
}
