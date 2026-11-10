//! `WebDAV` base URL validation.
//!
//! [`WebDavUrl`] is the credential-free origin (`scheme://authority`)
//! a [`crate::WebDavProvider`] connects to. Validation is fail-closed:
//! userinfo, query, fragment, non-ASCII hosts, and malformed ports are
//! rejected so credentials can never hide in the URL and the `Host`
//! header is always well-formed. `https` parses successfully but the
//! transport reports [`crate::UnsupportedCapability`] — this slice
//! ships no TLS stack and never silently downgrades to `http`.

use crate::error::{ProviderError, ProviderErrorKind};

/// The URL scheme of a `WebDAV` base.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum WebDavScheme {
    /// Plain HTTP — the only scheme this slice can transport.
    Http,
    /// HTTPS — accepted at configuration time, rejected at connect
    /// time with [`crate::UnsupportedCapability`] (no TLS stack).
    Https,
}

impl WebDavScheme {
    /// The canonical scheme text (`"http"`/`"https"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }

    /// The default port for the scheme.
    #[must_use]
    pub const fn default_port(self) -> u16 {
        match self {
            Self::Http => 80,
            Self::Https => 443,
        }
    }
}

/// A validated `WebDAV` origin: `scheme://authority` with no path,
/// query, fragment, or userinfo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebDavUrl {
    scheme: WebDavScheme,
    /// Host without brackets for IPv6 literals.
    host: String,
    port: u16,
    /// Whether `host` is an IPv6 literal (`[...]` in authority form).
    ipv6: bool,
}

impl WebDavUrl {
    /// Parses and validates a base URL.
    ///
    /// Accepted forms: `http://host[:port]` and `https://host[:port]`
    /// (optionally with a single trailing `/`). Everything else —
    /// userinfo, path beyond `/`, query, fragment, empty or
    /// non-ASCII host, out-of-range port — is rejected.
    ///
    /// # Errors
    /// [`ProviderErrorKind::InvalidRequest`] on any malformed input.
    pub fn parse(text: &str) -> Result<Self, ProviderError> {
        let invalid = |detail: &str| {
            ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                format!("invalid webdav url: {detail}"),
            )
        };
        let (scheme, rest) = text
            .split_once("://")
            .ok_or_else(|| invalid("missing scheme"))?;
        let scheme = match scheme {
            "http" => WebDavScheme::Http,
            "https" => WebDavScheme::Https,
            _ => return Err(invalid("scheme must be http or https")),
        };
        // Only an optional single trailing slash may follow the
        // authority — no path, query, or fragment.
        let authority = match rest.find(['/', '?', '#']) {
            None => rest,
            Some(i) => {
                if &rest[i..] != "/" {
                    return Err(invalid(
                        "base url must be an origin (no path/query/fragment)",
                    ));
                }
                &rest[..i]
            }
        };
        if authority.is_empty() {
            return Err(invalid("missing authority"));
        }
        let (host, port, ipv6) = parse_authority(authority).map_err(invalid)?;
        Ok(Self {
            scheme,
            host,
            port: if port == 0 {
                scheme.default_port()
            } else {
                port
            },
            ipv6,
        })
    }

    /// The URL scheme.
    #[must_use]
    pub const fn scheme(&self) -> WebDavScheme {
        self.scheme
    }

    /// The host (no brackets for IPv6 literals).
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// `true` when the host is an IPv6 literal.
    #[must_use]
    pub const fn is_ipv6(&self) -> bool {
        self.ipv6
    }

    /// The explicit or default port.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// `host` or `[host]` for IPv6, with `:port` appended when it is
    /// not the scheme default — the `Host` header value.
    #[must_use]
    pub fn host_header(&self) -> String {
        let host_part = if self.ipv6 {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if self.port == self.scheme.default_port() {
            host_part
        } else {
            format!("{host_part}:{}", self.port)
        }
    }

    /// The origin key used for redirect/auth decisions:
    /// `scheme://authority` with the port always explicit, so
    /// `http://h` and `http://h:80` compare equal.
    #[must_use]
    pub fn origin_key(&self) -> String {
        format!("{}://{}", self.scheme.as_str(), self.authority_explicit())
    }

    /// `host:port` with the port always explicit (bracketed IPv6).
    fn authority_explicit(&self) -> String {
        let host_part = if self.ipv6 {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        format!("{host_part}:{}", self.port)
    }

    /// `true` when `other` names the same origin (scheme + host +
    /// port) — the boundary inside which credentials may be sent.
    #[must_use]
    pub fn same_origin(&self, other: &Self) -> bool {
        self.scheme == other.scheme && self.host == other.host && self.port == other.port
    }

    /// `true` when `authority` — the `host[:port]` (or
    /// `[v6][:port]`) text carried by a server href — names this
    /// origin. Host compares case-insensitively; an absent port
    /// means the scheme default. Malformed authorities never match.
    #[must_use]
    pub fn authority_matches(&self, authority: &str) -> bool {
        let Ok((host, port, _)) = parse_authority(authority) else {
            return false;
        };
        let port = if port == 0 {
            self.scheme.default_port()
        } else {
            port
        };
        host.eq_ignore_ascii_case(&self.host) && port == self.port
    }
}

/// Splits `authority` into `(host, port, ipv6)`. Host grammar is the
/// visible-ASCII reg-name/IP-literal subset: letters, digits, `-`,
/// `.`, and `:`-free — percent-encoding, userinfo, and non-ASCII
/// (IDN) are rejected rather than normalized.
fn parse_authority(authority: &str) -> Result<(String, u16, bool), &'static str> {
    if authority.contains('@') {
        return Err("userinfo in url is not allowed");
    }
    let (host, port, ipv6) = if let Some(rest) = authority.strip_prefix('[') {
        // IPv6 literal: `[::1]` or `[::1]:8080`.
        let (addr, tail) = rest.split_once(']').ok_or("unterminated ipv6 literal")?;
        if addr.is_empty() || !addr.bytes().all(|b| b.is_ascii_hexdigit() || b == b':') {
            return Err("malformed ipv6 literal");
        }
        let port = match tail.strip_prefix(':') {
            Some(p) => parse_port(p)?,
            None if tail.is_empty() => 0,
            None => return Err("malformed ipv6 literal"),
        };
        (addr.to_owned(), port, true)
    } else {
        match authority.rsplit_once(':') {
            Some((h, p))
                if !h.is_empty() && !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) =>
            {
                (h.to_owned(), parse_port(p)?, false)
            }
            Some(_) => return Err("malformed port"),
            None => (authority.to_owned(), 0, false),
        }
    };
    if host.is_empty() || host.len() > 253 {
        return Err("host is empty or over-long");
    }
    if !ipv6
        && !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.'))
    {
        return Err("host contains characters outside the reg-name subset");
    }
    // Hostnames are case-insensitive — canonicalize so `same_origin`
    // and `origin_key` compare equal spellings.
    Ok((host.to_ascii_lowercase(), port, ipv6))
}

fn parse_port(text: &str) -> Result<u16, &'static str> {
    if text.is_empty() || text.len() > 5 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err("malformed port");
    }
    let port: u16 = text.parse().map_err(|_| "port out of range")?;
    if port == 0 {
        return Err("port 0 is not allowed");
    }
    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_key_canonicalizes_default_port() {
        let a = WebDavUrl::parse("http://Example.COM").expect("a");
        let b = WebDavUrl::parse("http://example.com:80").expect("b");
        assert_eq!(a.origin_key(), b.origin_key());
        assert!(a.same_origin(&b));
        let c = WebDavUrl::parse("https://example.com").expect("c");
        assert!(!a.same_origin(&c));
    }

    #[test]
    fn authority_matches_ipv6() {
        let u = WebDavUrl::parse("http://[::1]:8080").expect("u");
        assert!(u.authority_matches("[::1]:8080"));
        assert!(!u.authority_matches("::1"));
        assert!(!u.authority_matches("[::1]:80"));
    }
}
