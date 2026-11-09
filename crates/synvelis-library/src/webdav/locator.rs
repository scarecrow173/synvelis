//! `WebDAV` locator grammar and root confinement.
//!
//! A locator is the *decoded* absolute server path (`/music/a.wav`)
//! carried by [`crate::OpaqueLocator`]. Percent-encoding exists only
//! on the wire: hrefs from `multistatus` are decoded once before
//! validation, and request paths are encoded per segment. The
//! grammar is fail-closed — empty/`.`/`..` components, `\`, control
//! characters, NUL, over-long input, and anything outside the
//! configured `root_path` subtree are rejected so a crafted locator
//! or href can never escape the configured collection.

use crate::error::{ProviderError, ProviderErrorKind};
use crate::id::OpaqueLocator;
use crate::webdav::url::WebDavUrl;

/// Maximum locator text length (matches the opaque bound).
const MAX_PATH_LEN: usize = 2048;
/// Maximum single path component length.
const MAX_COMPONENT_LEN: usize = 255;

/// Normalizes and validates an absolute server path. Returns the
/// canonical spelling: exactly one leading `/`, no trailing `/`
/// (except the root `/` itself), no empty/`.`/`..` components.
///
/// # Errors
/// [`ProviderErrorKind::InvalidRequest`] on any grammar violation.
pub(crate) fn normalize_path(text: &str) -> Result<String, ProviderError> {
    let invalid = |detail: &str| {
        ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            format!("invalid dav path: {detail}"),
        )
    };
    if text.len() > MAX_PATH_LEN {
        return Err(invalid("path exceeds length bound"));
    }
    if !text.starts_with('/') {
        return Err(invalid("path must be absolute"));
    }
    if text.contains('\\') || text.contains('\0') {
        return Err(invalid("path contains forbidden character"));
    }
    if text.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return Err(invalid("path contains control character"));
    }
    // Strip trailing slashes (a collection marker) — the root "/"
    // itself is preserved.
    let trimmed = text.trim_end_matches('/');
    let trimmed = if trimmed.is_empty() { "/" } else { trimmed };
    if trimmed == "/" {
        return Ok("/".to_owned());
    }
    // `trimmed` starts with exactly the leading `/`; the remainder
    // must be non-empty components only.
    for comp in trimmed[1..].split('/') {
        if comp.is_empty() {
            return Err(invalid("path contains empty component"));
        }
        if comp == "." || comp == ".." {
            return Err(invalid("path contains dot component"));
        }
        if comp.len() > MAX_COMPONENT_LEN {
            return Err(invalid("path component exceeds length bound"));
        }
    }
    Ok(trimmed.to_owned())
}

/// `true` when normalized `path` is `root` itself or inside its
/// subtree. `root == "/"` allows the whole server namespace.
#[must_use]
pub(crate) fn within_root(path: &str, root: &str) -> bool {
    if root == "/" {
        return true;
    }
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Parses a caller-supplied locator into a normalized absolute path
/// confined to `root`.
///
/// # Errors
/// [`ProviderErrorKind::InvalidRequest`] on grammar or
/// confinement violations.
pub(crate) fn locator_path(locator: &OpaqueLocator, root: &str) -> Result<String, ProviderError> {
    let path = normalize_path(locator.as_str())?;
    if !within_root(&path, root) {
        return Err(ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            format!("locator {locator} escapes configured root {root}"),
        ));
    }
    Ok(path)
}

/// Percent-decodes `text` into raw bytes then UTF-8. Strict: every
/// `%` must be followed by two hex digits.
///
/// # Errors
/// [`ProviderErrorKind::InvalidRequest`] on malformed escapes or
/// non-UTF-8 output.
pub(crate) fn percent_decode(text: &str) -> Result<String, ProviderError> {
    let invalid = || {
        ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            "malformed percent-encoding in dav href",
        )
    };
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return Err(invalid());
            }
            let hi = hex_val(bytes[i + 1]).ok_or_else(invalid)?;
            let lo = hex_val(bytes[i + 2]).ok_or_else(invalid)?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| invalid())
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Percent-encodes a normalized path for the request line: `/`
/// separators stay literal, every other byte outside the unreserved
/// set becomes `%XX` (uppercase hex).
#[must_use]
pub(crate) fn encode_path(path: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(path.len());
    for &b in path.as_bytes() {
        let unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~');
        if unreserved || b == b'/' {
            out.push(b as char);
        } else {
            out.push('%');
            out.push(HEX[usize::from(b >> 4)] as char);
            out.push(HEX[usize::from(b & 0x0f)] as char);
        }
    }
    out
}

/// Converts a `multistatus` href into a confined path inside `root`.
///
/// Accepts absolute-path hrefs (`/a/b`), absolute URIs on the same
/// origin (`http://h/a/b`), and scheme-relative hrefs (`//h/a/b`).
/// Relative hrefs resolve against `collection_path`'s directory with
/// RFC 3986 dot-segment removal. The result is percent-decoded,
/// grammar-validated, and confinement-checked; anything else is
/// `None` (the caller counts it unaddressable → `Coverage::Partial`).
#[must_use]
pub(crate) fn href_to_path(
    href: &str,
    base: &WebDavUrl,
    collection_path: &str,
    root: &str,
) -> Option<String> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }
    // Strip query/fragment — DAV resource identity is the path.
    let end = href.find(['?', '#']).unwrap_or(href.len());
    let href = &href[..end];
    let raw_path: String = if let Some((scheme, rest)) = href.split_once("://") {
        // Absolute URI: scheme and authority must match the
        // configured origin, otherwise the entry is foreign.
        if scheme != base.scheme().as_str() {
            return None;
        }
        let auth_end = rest.find('/').unwrap_or(rest.len());
        if !base.authority_matches(&rest[..auth_end]) {
            return None;
        }
        rest[auth_end..].to_owned()
    } else if let Some(rest) = href.strip_prefix("//") {
        // Scheme-relative: authority must match.
        let auth_end = rest.find('/').unwrap_or(rest.len());
        if !base.authority_matches(&rest[..auth_end]) {
            return None;
        }
        rest[auth_end..].to_owned()
    } else if href.starts_with('/') {
        href.to_owned()
    } else {
        // Relative reference: merge with the collection's directory
        // and remove dot segments per RFC 3986 §5.2.
        merge_relative(collection_path, href)
    };
    let raw_path = if raw_path.is_empty() {
        "/".to_owned()
    } else {
        raw_path
    };
    let decoded = percent_decode(&raw_path).ok()?;
    let path = normalize_path(&decoded).ok()?;
    within_root(&path, root).then_some(path)
}

/// Merges a relative reference `r` against the directory of
/// `collection_path` (RFC 3986 §5.2.3) and removes dot segments.
/// The directory of `"/music"` is `"/"`, so `"a.wav"` merges to
/// `"/a.wav"` — never `"//a.wav"`.
pub(crate) fn merge_relative(collection_path: &str, r: &str) -> String {
    let dir = match collection_path.rfind('/') {
        Some(0) | None => "/",
        Some(i) => &collection_path[..i],
    };
    let merged = if dir == "/" {
        format!("/{r}")
    } else {
        format!("{dir}/{r}")
    };
    remove_dot_segments(&merged)
}

/// RFC 3986 §5.2.4 dot-segment removal on an absolute path.
/// `"."` and `".."` resolve structurally; `".."` above the root is
/// dropped. Interior empty segments are preserved — the result still
/// goes through [`normalize_path`], which rejects them.
pub(crate) fn remove_dot_segments(path: &str) -> String {
    let mut p = path.to_owned();
    loop {
        if let Some(i) = p.find("/./") {
            p.replace_range(i..i + 3, "/");
        } else if p.ends_with("/.") {
            // "/a/." → "/a/"
            p.push('/');
        } else if let Some(i) = p.find("/../") {
            // Remove the segment preceding "/../"; at the root the
            // ".." is dropped per RFC.
            let before = &p[..i];
            let seg_start = before.rfind('/').map_or(0, |j| j + 1);
            let prefix = if seg_start == 0 { "/" } else { &p[..seg_start] };
            p = format!("{prefix}{}", &p[i + 4..]);
        } else if p.ends_with("/..") {
            // "/a/b/.." ≡ "/a/b/../" — defer to the "/../" rule.
            p.push('/');
        } else {
            return p;
        }
    }
}

/// The display name for a path — its last component.
#[must_use]
pub(crate) fn display_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::webdav::url::WebDavUrl;

    fn base() -> WebDavUrl {
        WebDavUrl::parse("http://h:81").expect("url")
    }

    #[test]
    fn normalize_rejects_grammar_violations() {
        for bad in ["rel", "", "/a//b", "/a/./b", "/a/../b", "/a\\b", "/a\u{7}b"] {
            assert!(normalize_path(bad).is_err(), "{bad:?} must fail");
        }
        // '%' is grammar-legal raw — decoding happens once, later.
        assert_eq!(normalize_path("/a%2fb").expect("pct"), "/a%2fb");
        assert_eq!(normalize_path("/a/b/").expect("trail"), "/a/b");
        assert_eq!(normalize_path("//").expect("root"), "/");
    }

    #[test]
    fn within_root_boundary() {
        assert!(within_root("/music", "/music"));
        assert!(within_root("/music/a", "/music"));
        assert!(!within_root("/music2", "/music"));
        assert!(!within_root("/mus", "/music"));
        assert!(within_root("/anything", "/"));
    }

    #[test]
    fn percent_decode_is_strict_and_single() {
        assert_eq!(percent_decode("/a%20b").expect("dec"), "/a b");
        assert_eq!(percent_decode("%25").expect("dec"), "%");
        assert!(percent_decode("%2").is_err());
        assert!(percent_decode("%zz").is_err());
        assert!(percent_decode("%ff").is_err()); // non-UTF-8
        // Decoded exactly once — "%2520" → "%20", not " ".
        assert_eq!(percent_decode("%2520").expect("dec"), "%20");
    }

    #[test]
    fn encode_path_keeps_unreserved_and_slash() {
        assert_eq!(encode_path("/a b/c+d"), "/a%20b/c%2Bd");
        assert_eq!(encode_path("/~x_y-z.9"), "/~x_y-z.9");
    }

    #[test]
    fn dot_segments_and_merge() {
        assert_eq!(remove_dot_segments("/a/b/../c"), "/a/c");
        assert_eq!(remove_dot_segments("/a/./b"), "/a/b");
        assert_eq!(remove_dot_segments("/../a"), "/a");
        assert_eq!(merge_relative("/music", "a.wav"), "/a.wav");
        assert_eq!(merge_relative("/music/sub", "a.wav"), "/music/a.wav");
        assert_eq!(merge_relative("/music/sub", "../a.wav"), "/a.wav");
    }

    #[test]
    fn href_resolution_variants() {
        let b = base();
        // Absolute path.
        assert_eq!(
            href_to_path("/music/a.wav", &b, "/music", "/music").as_deref(),
            Some("/music/a.wav")
        );
        // Same-origin absolute URI (default port 80 vs explicit :81
        // must not match — port differs).
        assert_eq!(
            href_to_path("http://h:81/music/a.wav", &b, "/music", "/music").as_deref(),
            Some("/music/a.wav")
        );
        assert!(href_to_path("http://h/music/a.wav", &b, "/music", "/music").is_none());
        assert!(href_to_path("http://other:81/music/a.wav", &b, "/music", "/music").is_none());
        // Scheme-relative.
        assert_eq!(
            href_to_path("//h:81/music/a.wav", &b, "/music", "/music").as_deref(),
            Some("/music/a.wav")
        );
        // Percent-encoded once.
        assert_eq!(
            href_to_path("/music/a%20b.wav", &b, "/music", "/music").as_deref(),
            Some("/music/a b.wav")
        );
        // Traversal escapes root → refused.
        assert!(href_to_path("/music/../etc", &b, "/music", "/music").is_none());
        assert!(href_to_path("/etc/passwd", &b, "/music", "/music").is_none());
        // Query/fragment stripped.
        assert_eq!(
            href_to_path("/music/a.wav?x=1#f", &b, "/music", "/music").as_deref(),
            Some("/music/a.wav")
        );
    }
}
