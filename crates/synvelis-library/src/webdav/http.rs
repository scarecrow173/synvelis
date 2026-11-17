//! Minimal blocking HTTP/1.1 transport shared by the `WebDAV`
//! provider and the Git LFS remote-fetch backend.
//!
//! This is deliberately *not* a general HTTP client: it speaks
//! exactly the request/response subset its callers need — one
//! request per connection (`Connection: close`), `Content-Length`
//! and `chunked` bodies, no keep-alive, no proxy support. `https`
//! endpoints carry a [`TlsSession`] (rustls client configuration +
//! verified server name) on the [`HttpEndpoint`]; the scheme is
//! never downgraded — an HTTPS target always performs a TLS
//! handshake or fails typed.
//!
//! All blocking socket operations — including the TLS handshake and
//! every encrypted record exchange — run under a cooperative guard:
//! socket timeouts poll at [`IO_POLL`], each poll re-checks the
//! caller's [`CancellationToken`] and deadline, and a transfer that
//! makes no progress for [`STALL_TIMEOUT`] fails with
//! [`crate::ProviderErrorKind::TransientIo`]. A stalled or hostile
//! server can therefore never wedge a provider call, and mid-body
//! cancellation surfaces as a typed error inside the stream.
//!
//! TLS verification failures surface as
//! [`crate::ProviderErrorKind::TlsVerification`]; other TLS/record
//! errors stay `TransientIo`. Certificate contents, peer data, and
//! URL secrets never appear in error details.
//!
//! The `WebDAV` provider itself still rejects `https` at its own
//! boundary (`send`) — TLS transport exists for callers that opt in
//! per endpoint (Git LFS); WebDAV-over-HTTPS is a separate decision.

use std::cell::Cell;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::context::CancellationToken;
use crate::digest::io_provider_error;
use crate::error::{ProviderError, ProviderErrorKind};
use crate::tls::TlsSession;
use crate::webdav::locator::encode_path;
use crate::webdav::url::WebDavUrl;

/// Socket poll interval — how often a blocking read/write returns
/// control so cancel/deadline can be re-checked.
const IO_POLL: Duration = Duration::from_millis(250);
/// Connect attempt budget per address.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Response-head size bound — headers beyond this are hostile.
const HEAD_LIMIT: usize = 64 * 1024;
/// Maximum header lines in a response head.
const HEAD_LINE_LIMIT: usize = 100;
/// No-progress bound for any single transfer phase. A body that
/// keeps delivering bytes is never stalled; one that goes silent
/// fails typed instead of hanging the worker.
const STALL_TIMEOUT: Duration = Duration::from_secs(30);
/// Redirect hops followed per request (provider-side policy).
pub(crate) const MAX_REDIRECTS: u32 = 4;

/// One outbound HTTP request. `path` is the *decoded* absolute
/// server path; encoding happens at serialization.
#[derive(Clone, Debug)]
pub(crate) struct HttpRequest {
    /// Method token (`GET`, `PROPFIND`, ...).
    pub method: &'static str,
    /// Decoded absolute path (`/music/a.wav`) — no query/fragment.
    pub path: String,
    /// Optional query string (without `?`), already
    /// percent-encoded — emitted verbatim after the encoded path.
    /// Re-encoding would corrupt signed tokens, so callers pass the
    /// server's own spelling.
    pub query: Option<String>,
    /// Extra header lines as `(name, value)`; names are emitted
    /// verbatim so callers pass canonical casing.
    pub headers: Vec<(String, String)>,
    /// Optional request body.
    pub body: Option<Vec<u8>>,
}

/// A received response: status, headers (lowercase names), and the
/// body as a streaming reader. Dropping the response drops the
/// connection.
pub(crate) struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response headers with lowercased names, in wire order.
    pub headers: Vec<(String, String)>,
    /// The body stream — `Content-Length`/`chunked`/close-delimited
    /// per the response framing. Always `Send`.
    pub body: Box<dyn Read + Send>,
}

impl HttpResponse {
    /// First value of header `name` (case-insensitive).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        header_value(&self.headers, name)
    }
}

/// Cooperative I/O guard shared by the request and body phases.
/// `last_progress` is a `Cell` because every phase holds the guard
/// immutably while socket reads mutate the stall clock.
struct IoGuard {
    cancel: CancellationToken,
    deadline: Option<Instant>,
    last_progress: Cell<Instant>,
}

impl IoGuard {
    fn new(cancel: &CancellationToken, deadline: Option<Instant>) -> Self {
        Self {
            cancel: cancel.clone(),
            deadline,
            last_progress: Cell::new(Instant::now()),
        }
    }

    /// Cancel/deadline check — the same semantics as
    /// [`crate::ProviderContext::check_interrupts`].
    fn check(&self) -> Result<(), ProviderError> {
        if self.cancel.is_cancelled() {
            return Err(ProviderError::new(
                ProviderErrorKind::Cancelled,
                "operation cancelled",
            ));
        }
        if let Some(deadline) = self.deadline
            && Instant::now() >= deadline
        {
            return Err(ProviderError::new(
                ProviderErrorKind::BudgetExceeded,
                "provider deadline exceeded",
            ));
        }
        Ok(())
    }

    fn note_progress(&self) {
        self.last_progress.set(Instant::now());
    }

    /// The stall check run when a socket poll times out.
    fn check_stall(&self) -> Result<(), ProviderError> {
        if self.last_progress.get().elapsed() >= STALL_TIMEOUT {
            return Err(ProviderError::new(
                ProviderErrorKind::TransientIo,
                "server stopped responding mid-transfer",
            ));
        }
        Ok(())
    }

    /// Socket timeout for the next poll — never longer than the
    /// remaining deadline so a deadline lands promptly.
    fn poll_timeout(&self) -> Duration {
        match self.deadline {
            Some(d) => IO_POLL.min(d.saturating_duration_since(Instant::now())),
            None => IO_POLL,
        }
    }
}

fn is_timeout(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

/// A connection target — the host/port/IPv6 triple the transport
/// needs, independent of any provider's URL type, plus the TLS
/// session parameters for `https` endpoints. `tls: None` is plain
/// HTTP; `tls: Some` performs a verified TLS handshake — the scheme
/// is never inferred from the port and never downgraded.
#[derive(Clone, Debug)]
pub(crate) struct HttpEndpoint {
    /// Host without brackets for IPv6 literals.
    pub host: String,
    /// Explicit port (the scheme default is applied by callers).
    pub port: u16,
    /// Whether `host` is an IPv6 literal.
    pub ipv6: bool,
    /// TLS session parameters — `Some` for `https` endpoints only.
    pub tls: Option<TlsSession>,
}

impl HttpEndpoint {
    /// Builds an endpoint for plain HTTP on `port`.
    #[must_use]
    pub(crate) fn new(host: impl Into<String>, port: u16, ipv6: bool) -> Self {
        Self {
            host: host.into(),
            port,
            ipv6,
            tls: None,
        }
    }

    /// Builds an `https` endpoint on `port` with the given TLS
    /// session (shared client config + verified server name).
    #[must_use]
    pub(crate) fn new_tls(host: impl Into<String>, port: u16, ipv6: bool, tls: TlsSession) -> Self {
        Self {
            host: host.into(),
            port,
            ipv6,
            tls: Some(tls),
        }
    }

    /// The scheme default port — 443 for TLS endpoints, 80 for plain.
    fn default_port(&self) -> u16 {
        if self.tls.is_some() { 443 } else { 80 }
    }

    /// `host` or `[host]` for IPv6, with `:port` appended unless it
    /// is the scheme default — the `Host` header value.
    fn host_header(&self) -> String {
        let host_part = if self.ipv6 {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if self.port == self.default_port() {
            host_part
        } else {
            format!("{host_part}:{}", self.port)
        }
    }
}

/// Sends `req` to `url` and returns the response head plus an open
/// body stream. One connection per call; `Connection: close` is
/// always sent so close-delimited bodies terminate.
///
/// # Errors
/// [`ProviderError`] — `UnsupportedCapability` for `https` (the
/// `WebDAV` provider does not opt into TLS), `Offline` for
/// connect/resolution failures, `TlsVerification` for certificate
/// failures on TLS endpoints, `TransientIo` for protocol violations
/// and stalls, `Cancelled`/`BudgetExceeded` from the guard.
pub(crate) fn send(
    url: &WebDavUrl,
    req: &HttpRequest,
    cancel: &CancellationToken,
    deadline: Option<Instant>,
) -> Result<HttpResponse, ProviderError> {
    if url.scheme() != crate::webdav::WebDavScheme::Http {
        return Err(ProviderError::unsupported(
            "https transport",
            "webdav provider",
        ));
    }
    // Plain-HTTP endpoint only — `send` never opts into TLS; HTTPS
    // callers build `HttpEndpoint::new_tls` explicitly.
    send_endpoint(
        &HttpEndpoint::new(url.host(), url.port(), url.is_ipv6()),
        req,
        cancel,
        deadline,
    )
}

/// Sends `req` to `endpoint` — the shared transport entry point for
/// non-WebDAV callers (Git LFS). Identical wire behavior to [`send`]:
/// one request per connection, `Connection: close`, and the
/// cooperative cancel/deadline/stall guard. An endpoint carrying
/// [`TlsSession`] parameters performs a verified TLS handshake;
/// `tls: None` is plain HTTP — the scheme is taken from the endpoint
/// construction, never inferred or downgraded.
///
/// # Errors
/// Same contract as [`send`], minus the scheme check — the endpoint
/// carries its own transport parameters.
pub(crate) fn send_endpoint(
    endpoint: &HttpEndpoint,
    req: &HttpRequest,
    cancel: &CancellationToken,
    deadline: Option<Instant>,
) -> Result<HttpResponse, ProviderError> {
    let guard = IoGuard::new(cancel, deadline);
    guard.check()?;
    let addrs = resolve(endpoint)?;
    let mut wire = connect(&addrs, endpoint.tls.as_ref(), &guard)?;
    write_request(&mut wire, endpoint, req, &guard)?;
    let head = read_head(&mut wire, &guard)?;
    let body = open_body(
        wire,
        head.status,
        &head.headers,
        head.pending,
        cancel,
        deadline,
    );
    Ok(HttpResponse {
        status: head.status,
        headers: head.headers,
        body,
    })
}

fn resolve(endpoint: &HttpEndpoint) -> Result<Vec<SocketAddr>, ProviderError> {
    let authority = if endpoint.ipv6 {
        format!("[{}]:{}", endpoint.host, endpoint.port)
    } else {
        format!("{}:{}", endpoint.host, endpoint.port)
    };
    let addrs: Vec<SocketAddr> = authority
        .to_socket_addrs()
        .map_err(|e| {
            ProviderError::new(
                ProviderErrorKind::Offline,
                format!("cannot resolve {}: {e}", endpoint.host),
            )
        })?
        .collect();
    if addrs.is_empty() {
        return Err(ProviderError::new(
            ProviderErrorKind::Offline,
            format!("no addresses for {}", endpoint.host),
        ));
    }
    Ok(addrs)
}

fn connect(
    addrs: &[SocketAddr],
    tls: Option<&TlsSession>,
    guard: &IoGuard,
) -> Result<Wire, ProviderError> {
    let timeout = match guard.deadline {
        Some(d) => CONNECT_TIMEOUT.min(d.saturating_duration_since(Instant::now())),
        None => CONNECT_TIMEOUT,
    };
    let mut last = None;
    for addr in addrs {
        guard.check()?;
        match TcpStream::connect_timeout(addr, timeout.max(Duration::from_millis(1))) {
            Ok(stream) => {
                // Short socket polls let the guard observe
                // cancel/deadline mid-transfer — including inside
                // the TLS handshake and every encrypted exchange.
                let _ = stream.set_nodelay(true);
                let _ = stream.set_read_timeout(Some(IO_POLL));
                let _ = stream.set_write_timeout(Some(IO_POLL));
                return match tls {
                    Some(session) => TlsConn::handshake(stream, session, guard)
                        .map(|conn| Wire::Tls(Box::new(conn))),
                    None => Ok(Wire::Plain(stream)),
                };
            }
            Err(e) => last = Some(e),
        }
    }
    Err(ProviderError::new(
        ProviderErrorKind::Offline,
        format!(
            "cannot connect: {}",
            last.map_or_else(|| "no route".to_owned(), |e| e.to_string())
        ),
    ))
}

/// The connected transport — plaintext TCP or a rustls session over
/// it. Both variants satisfy the guarded contract: socket operations
/// poll at `IO_POLL` so the caller's [`IoGuard`] observes
/// cancel/deadline/stall between calls, and body readers stay
/// `Read + Send`.
enum Wire {
    /// Plain HTTP/1.1 over TCP.
    Plain(TcpStream),
    /// HTTPS — TLS records over TCP.
    Tls(Box<TlsConn>),
}

impl Wire {
    /// Adjusts the underlying socket's read timeout — the body-phase
    /// poll deadline. TLS timeouts live on the same socket.
    fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        match self {
            Self::Plain(stream) => stream.set_read_timeout(timeout),
            Self::Tls(conn) => conn.sock.set_read_timeout(timeout),
        }
    }
}

impl Read for Wire {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.read(buf),
            Self::Tls(conn) => conn.read(buf),
        }
    }
}

impl Write for Wire {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.write(buf),
            Self::Tls(conn) => conn.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(stream) => stream.flush(),
            Self::Tls(conn) => conn.flush(),
        }
    }
}

/// A rustls client session over a connected `TcpStream`.
///
/// Read/write are deliberately *one bounded IO pass per call* rather
/// than `rustls::Stream`'s `prepare_read` loop: `complete_io` runs at
/// most one record flight, then buffered plaintext is drained. A
/// `WouldBlock` — or plaintext starvation after non-app-data
/// records — propagates to the caller's guard loop, so cancel,
/// deadline, and stall checks always run between calls and a hostile
/// peer streaming endless non-app-data records cannot spin inside a
/// single `read`.
struct TlsConn {
    conn: rustls::ClientConnection,
    /// The raw socket — kept for `set_read_timeout` (timeouts are a
    /// socket property, shared with the TLS session).
    sock: TcpStream,
}

impl TlsConn {
    /// Drives the TLS handshake to completion under the cooperative
    /// guard. Each `WouldBlock` re-checks cancel/deadline/stall;
    /// certificate and protocol failures classify via [`tls_error`].
    fn handshake(
        mut stream: TcpStream,
        session: &TlsSession,
        guard: &IoGuard,
    ) -> Result<Self, ProviderError> {
        let mut conn =
            rustls::ClientConnection::new(session.config.clone(), session.server_name.clone())
                .map_err(|e| {
                    ProviderError::new(
                        ProviderErrorKind::Internal,
                        format!("cannot initialize tls session: {e}"),
                    )
                })?;
        loop {
            guard.check()?;
            match conn.complete_io(&mut stream) {
                // `complete_io` returns `Ok` only once the handshake
                // finished (or when nothing remains to be done); a
                // peer that went silent mid-handshake surfaces as
                // `WouldBlock` → stall check, or `UnexpectedEof`.
                Ok((rd, wr)) => {
                    if rd + wr > 0 {
                        guard.note_progress();
                    }
                    if !conn.is_handshaking() {
                        break;
                    }
                    if rd == 0 && wr == 0 {
                        return Err(ProviderError::new(
                            ProviderErrorKind::TransientIo,
                            "tls handshake ended without completing",
                        ));
                    }
                }
                Err(e) if is_timeout(&e) => {
                    guard.check_stall()?;
                }
                Err(e) => return Err(tls_error(&e)),
            }
        }
        Ok(Self { conn, sock: stream })
    }
}

impl Read for TlsConn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Flush pending TLS output, pull at most one flight of
        // records, then hand out whatever plaintext arrived.
        // `WouldBlock` from either step propagates — the caller's
        // guard loop owns the poll cadence.
        self.conn.complete_io(&mut self.sock)?;
        self.conn.reader().read(buf)
    }
}

impl Write for TlsConn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Queue the plaintext, then best-effort push pending TLS
        // records onto the socket. A blocked socket leaves records
        // queued — the next call's `complete_io` drains `wants_write`.
        // Timeout-kind errors are the expected poll cadence and stay
        // masked; a fatal TLS/record error surfaces immediately
        // instead of waiting for the next operation.
        let n = self.conn.writer().write(buf)?;
        if let Err(e) = self.conn.complete_io(&mut self.sock)
            && !is_timeout(&e)
        {
            return Err(e);
        }
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.conn.complete_io(&mut self.sock)?;
        self.sock.flush()
    }
}

/// Maps a TLS-layer failure to a typed provider error.
///
/// rustls wraps certificate-chain, expiry, and name-verification
/// failures as `io::ErrorKind::InvalidData` carrying a
/// `rustls::Error::InvalidCertificate` — those get the dedicated
/// [`ProviderErrorKind::TlsVerification`] (deterministic trust
/// failure, `RetryAdvice::Never`). Protocol violations, alerts, and
/// record errors stay `TransientIo`; plain socket errors keep the
/// transport's existing classification. Details carry only rustls's
/// own sanitized text — never certificate bytes, peer data, or URL
/// secrets.
fn tls_error(e: &io::Error) -> ProviderError {
    if let Some(rustls_err) = e
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<rustls::Error>())
    {
        return match rustls_err {
            rustls::Error::InvalidCertificate(_) => ProviderError::new(
                ProviderErrorKind::TlsVerification,
                format!("tls certificate verification failed: {rustls_err}"),
            ),
            _ => ProviderError::new(
                ProviderErrorKind::TransientIo,
                format!("tls handshake failed: {rustls_err}"),
            ),
        };
    }
    io_provider_error("tls", e)
}

fn write_request(
    stream: &mut Wire,
    endpoint: &HttpEndpoint,
    req: &HttpRequest,
    guard: &IoGuard,
) -> Result<(), ProviderError> {
    let mut head = String::with_capacity(512 + req.headers.len() * 32);
    head.push_str(req.method);
    head.push(' ');
    head.push_str(&encode_path(&req.path));
    if let Some(query) = &req.query {
        // The query is already percent-encoded and emitted
        // verbatim; anything that could break the request line is
        // refused at this last boundary before the wire.
        if query.bytes().any(|b| b <= 0x20 || b == 0x7f || b == b'#') {
            return Err(ProviderError::new(
                ProviderErrorKind::Internal,
                "refusing to emit malformed query",
            ));
        }
        head.push('?');
        head.push_str(query);
    }
    head.push_str(" HTTP/1.1\r\nHost: ");
    head.push_str(&endpoint.host_header());
    head.push_str("\r\nUser-Agent: synvelis-library\r\nConnection: close\r\n");
    for (name, value) in &req.headers {
        // Header values are validated by callers; re-check here so a
        // future caller cannot smuggle CR/LF into the request block.
        // This is the last boundary before the wire.
        if name
            .bytes()
            .any(|b| !(0x21..=0x7e).contains(&b) || b == b':')
            || value.bytes().any(|b| b < 0x20 || b == 0x7f)
        {
            return Err(ProviderError::new(
                ProviderErrorKind::Internal,
                "refusing to emit malformed header",
            ));
        }
        head.push_str(name);
        head.push_str(": ");
        head.push_str(value);
        head.push_str("\r\n");
    }
    if let Some(body) = &req.body {
        head.push_str("Content-Length: ");
        head.push_str(&body.len().to_string());
        head.push_str("\r\n");
    }
    head.push_str("\r\n");
    write_all_guarded(stream, head.as_bytes(), guard)?;
    if let Some(body) = &req.body {
        write_all_guarded(stream, body, guard)?;
    }
    Ok(())
}

fn write_all_guarded(
    stream: &mut Wire,
    mut buf: &[u8],
    guard: &IoGuard,
) -> Result<(), ProviderError> {
    while !buf.is_empty() {
        guard.check()?;
        match stream.write(buf) {
            Ok(0) => {
                return Err(ProviderError::new(
                    ProviderErrorKind::TransientIo,
                    "socket write made no progress",
                ));
            }
            Ok(n) => {
                guard.note_progress();
                buf = &buf[n..];
            }
            Err(e) if is_timeout(&e) => {
                guard.check_stall()?;
            }
            Err(e) => return Err(io_provider_error("write", &e)),
        }
    }
    Ok(())
}

/// A parsed response head plus body bytes already read past it.
struct ResponseHead {
    status: u16,
    headers: Vec<(String, String)>,
    pending: Vec<u8>,
}

/// Reads response heads until a non-`1xx` status arrives.
fn read_head(stream: &mut Wire, guard: &IoGuard) -> Result<ResponseHead, ProviderError> {
    let mut acc: Vec<u8> = Vec::with_capacity(4096);
    let mut chunk = [0u8; 8192];
    loop {
        guard.check()?;
        match stream.read(&mut chunk) {
            Ok(0) => {
                return Err(ProviderError::new(
                    ProviderErrorKind::TransientIo,
                    "connection closed before response head",
                ));
            }
            Ok(n) => {
                acc.extend_from_slice(&chunk[..n]);
                if acc.len() > HEAD_LIMIT {
                    return Err(ProviderError::new(
                        ProviderErrorKind::TransientIo,
                        "response head exceeds bound",
                    ));
                }
                if let Some(end) = find_head_end(&acc) {
                    let head = std::str::from_utf8(&acc[..end]).map_err(|_| {
                        ProviderError::new(
                            ProviderErrorKind::TransientIo,
                            "response head is not valid ASCII",
                        )
                    })?;
                    let (status, headers) = parse_head(head)?;
                    let pending = acc.split_off(end + 4);
                    if (100..=199).contains(&status) && status != 101 {
                        // Interim response — keep any bytes that
                        // already belong to the next head.
                        acc = pending;
                        continue;
                    }
                    return Ok(ResponseHead {
                        status,
                        headers,
                        pending,
                    });
                }
            }
            Err(e) if is_timeout(&e) => {
                guard.check_stall()?;
            }
            Err(e) => return Err(io_provider_error("read head", &e)),
        }
    }
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn parse_head(head: &str) -> Result<(u16, Vec<(String, String)>), ProviderError> {
    let malformed = |detail: &str| {
        ProviderError::new(
            ProviderErrorKind::TransientIo,
            format!("malformed response head: {detail}"),
        )
    };
    let mut lines = head.split("\r\n");
    let status_line = lines.next().ok_or_else(|| malformed("empty"))?;
    let mut parts = status_line.split(' ');
    let version = parts.next().unwrap_or_default();
    if !version.starts_with("HTTP/") {
        return Err(malformed("bad status line"));
    }
    let code: u16 = parts
        .next()
        .and_then(|t| t.parse().ok())
        .ok_or_else(|| malformed("bad status code"))?;
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        if headers.len() >= HEAD_LINE_LIMIT {
            return Err(malformed("too many headers"));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| malformed("header without ':'"))?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
        {
            return Err(malformed("bad header name"));
        }
        headers.push((name.to_ascii_lowercase(), value.trim().to_owned()));
    }
    Ok((code, headers))
}

/// Selects the body reader from the response framing. `pending`
/// carries head-read bytes that already belong to the body.
fn open_body(
    stream: Wire,
    status: u16,
    headers: &[(String, String)],
    pending: Vec<u8>,
    cancel: &CancellationToken,
    deadline: Option<Instant>,
) -> Box<dyn Read + Send> {
    // No-body statuses per RFC 9110 — framing headers are ignored.
    if (100..=199).contains(&status) || status == 204 || status == 304 {
        return Box::new(io::empty());
    }
    if header_value(headers, "transfer-encoding").is_some_and(|v| {
        v.split(',')
            .any(|t| t.trim().eq_ignore_ascii_case("chunked"))
    }) {
        return Box::new(ChunkedBody::new(stream, pending, cancel, deadline));
    }
    if let Some(value) = header_value(headers, "content-length")
        && let Ok(len) = value.trim().parse::<u64>()
    {
        return Box::new(FixedLenBody::new(stream, pending, len, cancel, deadline));
    }
    Box::new(CloseBody::new(stream, pending, cancel, deadline))
}

fn header_value<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// Shared read-with-timeout loop for body phases. Socket reads poll
/// at [`IO_POLL`]; each timeout re-checks cancel/deadline/stall.
/// `pending` drains first — bytes the head reader already consumed.
struct BodyIo {
    stream: Wire,
    pending: Vec<u8>,
    pending_pos: usize,
    guard: IoGuard,
}

impl BodyIo {
    fn new(
        stream: Wire,
        pending: Vec<u8>,
        cancel: &CancellationToken,
        deadline: Option<Instant>,
    ) -> Self {
        Self {
            stream,
            pending,
            pending_pos: 0,
            guard: IoGuard::new(cancel, deadline),
        }
    }

    fn read_inner(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pending_pos < self.pending.len() {
            let n = (self.pending.len() - self.pending_pos).min(buf.len());
            buf[..n].copy_from_slice(&self.pending[self.pending_pos..self.pending_pos + n]);
            self.pending_pos += n;
            self.guard.note_progress();
            return Ok(n);
        }
        loop {
            if let Err(e) = self.guard.check() {
                return Err(io::Error::from(e));
            }
            let _ = self
                .stream
                .set_read_timeout(Some(self.guard.poll_timeout()));
            match self.stream.read(buf) {
                Ok(n) => {
                    if n > 0 {
                        self.guard.note_progress();
                    }
                    return Ok(n);
                }
                Err(e) if is_timeout(&e) => {
                    if let Err(typed) = self.guard.check_stall() {
                        return Err(io::Error::from(typed));
                    }
                }
                Err(e) => return Err(e),
            }
        }
    }
}

/// `Content-Length`-bounded body. Premature EOF is an error, never
/// a clean end — truncated bytes must not verify as complete.
struct FixedLenBody {
    io: BodyIo,
    remaining: u64,
}

impl FixedLenBody {
    fn new(
        stream: Wire,
        pending: Vec<u8>,
        len: u64,
        cancel: &CancellationToken,
        deadline: Option<Instant>,
    ) -> Self {
        Self {
            io: BodyIo::new(stream, pending, cancel, deadline),
            remaining: len,
        }
    }
}

impl Read for FixedLenBody {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Ok(0);
        }
        let want = usize::try_from(self.remaining)
            .unwrap_or(usize::MAX)
            .min(buf.len());
        if want == 0 {
            return Ok(0);
        }
        match self.io.read_inner(&mut buf[..want])? {
            0 => Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                ProviderError::new(
                    ProviderErrorKind::TransientIo,
                    "connection closed mid-body (content-length not reached)",
                ),
            )),
            n => {
                self.remaining -= n as u64;
                Ok(n)
            }
        }
    }
}

/// `chunked` transfer-coded body.
struct ChunkedBody {
    io: BodyIo,
    /// Bytes left in the current chunk data (`u64::MAX` while the
    /// next chunk header must be parsed).
    chunk_remaining: u64,
    /// `true` once the terminal `0` chunk and trailers were consumed.
    done: bool,
}

impl ChunkedBody {
    fn new(
        stream: Wire,
        pending: Vec<u8>,
        cancel: &CancellationToken,
        deadline: Option<Instant>,
    ) -> Self {
        Self {
            io: BodyIo::new(stream, pending, cancel, deadline),
            chunk_remaining: u64::MAX,
            done: false,
        }
    }

    fn malformed(detail: &str) -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidData,
            ProviderError::new(
                ProviderErrorKind::TransientIo,
                format!("malformed chunked body: {detail}"),
            ),
        )
    }

    /// Reads one CRLF-terminated line. Bounded — chunk headers and
    /// trailers stay small; pathological lines fail typed.
    fn read_line(&mut self) -> io::Result<Vec<u8>> {
        let mut line = Vec::with_capacity(256);
        let mut byte = [0u8; 1];
        loop {
            if line.len() > 8192 {
                return Err(Self::malformed("line exceeds bound"));
            }
            match self.io.read_inner(&mut byte)? {
                0 => return Err(Self::malformed("eof inside chunk header")),
                1 => {
                    if byte[0] == b'\n' {
                        if line.last() == Some(&b'\r') {
                            line.pop();
                        }
                        return Ok(line);
                    }
                    line.push(byte[0]);
                }
                _ => unreachable!(),
            }
        }
    }

    /// Consumes the CRLF terminating a chunk's data.
    fn expect_crlf(&mut self) -> io::Result<()> {
        let mut buf = [0u8; 2];
        let mut got = 0;
        while got < 2 {
            match self.io.read_inner(&mut buf[got..])? {
                0 => return Err(Self::malformed("eof inside chunk data")),
                n => got += n,
            }
        }
        if &buf != b"\r\n" {
            return Err(Self::malformed("chunk data not CRLF-terminated"));
        }
        Ok(())
    }

    /// Parses the next chunk-size line; on the terminal `0` chunk,
    /// consumes trailers through the empty line and reports `false`.
    fn next_chunk(&mut self) -> io::Result<bool> {
        let line = self.read_line()?;
        let size_text = line.split(|b| *b == b';').next().unwrap_or_default();
        let size_text = std::str::from_utf8(size_text)
            .map_err(|_| Self::malformed("non-utf8 chunk size"))?
            .trim();
        let size =
            u64::from_str_radix(size_text, 16).map_err(|_| Self::malformed("bad chunk size"))?;
        if size == 0 {
            loop {
                if self.read_line()?.is_empty() {
                    break;
                }
            }
            self.done = true;
            return Ok(false);
        }
        self.chunk_remaining = size;
        Ok(true)
    }
}

impl Read for ChunkedBody {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            if self.done {
                return Ok(0);
            }
            if self.chunk_remaining == u64::MAX && !self.next_chunk()? {
                return Ok(0);
            }
            if self.chunk_remaining == 0 {
                self.expect_crlf()?;
                self.chunk_remaining = u64::MAX;
                continue;
            }
            let want = usize::try_from(self.chunk_remaining)
                .unwrap_or(usize::MAX)
                .min(buf.len());
            let n = self.io.read_inner(&mut buf[..want])?;
            if n == 0 {
                return Err(Self::malformed("eof inside chunk data"));
            }
            self.chunk_remaining -= n as u64;
            return Ok(n);
        }
    }
}

/// Close-delimited body (`HTTP/1.0` style or missing framing).
struct CloseBody {
    io: BodyIo,
}

impl CloseBody {
    fn new(
        stream: Wire,
        pending: Vec<u8>,
        cancel: &CancellationToken,
        deadline: Option<Instant>,
    ) -> Self {
        Self {
            io: BodyIo::new(stream, pending, cancel, deadline),
        }
    }
}

impl Read for CloseBody {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.io.read_inner(buf)
    }
}
