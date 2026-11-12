//! Minimal blocking HTTP/1.1 transport for the `WebDAV` provider.
//!
//! This is deliberately *not* a general HTTP client: it speaks
//! exactly the request/response subset the provider needs — one
//! request per connection (`Connection: close`), `Content-Length`
//! and `chunked` bodies, no keep-alive, no TLS, no proxy support.
//! TLS (`https`) is rejected with
//! [`crate::ProviderErrorKind::UnsupportedCapability`] — this slice
//! ships no TLS stack and never silently downgrades.
//!
//! All blocking socket operations run under a cooperative guard:
//! socket timeouts poll at [`IO_POLL`], each poll re-checks the
//! caller's [`CancellationToken`] and deadline, and a transfer that
//! makes no progress for [`STALL_TIMEOUT`] fails with
//! [`crate::ProviderErrorKind::TransientIo`]. A stalled or hostile
//! server can therefore never wedge a provider call, and mid-body
//! cancellation surfaces as a typed error inside the stream.

use std::cell::Cell;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

use crate::context::CancellationToken;
use crate::digest::io_provider_error;
use crate::error::{ProviderError, ProviderErrorKind};
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

/// A plain-HTTP connection target — the host/port/IPv6 triple the
/// transport needs, independent of any provider's URL type. The
/// scheme is always `http`: this transport has no TLS stack, so
/// `https` endpoints are rejected by callers before construction.
#[derive(Clone, Debug)]
pub(crate) struct HttpEndpoint {
    /// Host without brackets for IPv6 literals.
    pub host: String,
    /// Explicit port (the scheme default is applied by callers).
    pub port: u16,
    /// Whether `host` is an IPv6 literal.
    pub ipv6: bool,
}

impl HttpEndpoint {
    /// Builds an endpoint for plain HTTP on `port`.
    #[must_use]
    pub(crate) fn new(host: impl Into<String>, port: u16, ipv6: bool) -> Self {
        Self {
            host: host.into(),
            port,
            ipv6,
        }
    }

    /// `host` or `[host]` for IPv6, with `:port` appended unless it
    /// is the HTTP default (80) — the `Host` header value.
    fn host_header(&self) -> String {
        let host_part = if self.ipv6 {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if self.port == 80 {
            host_part
        } else {
            format!("{host_part}:{}", self.port)
        }
    }
}

impl From<&WebDavUrl> for HttpEndpoint {
    fn from(url: &WebDavUrl) -> Self {
        Self::new(url.host(), url.port(), url.is_ipv6())
    }
}

/// Sends `req` to `url` and returns the response head plus an open
/// body stream. One connection per call; `Connection: close` is
/// always sent so close-delimited bodies terminate.
///
/// # Errors
/// [`ProviderError`] — `UnsupportedCapability` for `https`,
/// `Offline` for connect/resolution failures, `TransientIo` for
/// protocol violations and stalls, `Cancelled`/`BudgetExceeded`
/// from the guard.
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
    send_endpoint(&HttpEndpoint::from(url), req, cancel, deadline)
}

/// Sends `req` to a plain-HTTP `endpoint` — the shared transport
/// entry point for non-WebDAV callers (Git LFS). Identical wire
/// behavior to [`send`]: one request per connection,
/// `Connection: close`, and the cooperative cancel/deadline/stall
/// guard. Never TLS.
///
/// # Errors
/// Same contract as [`send`], minus the scheme check — the
/// endpoint is HTTP by construction.
pub(crate) fn send_endpoint(
    endpoint: &HttpEndpoint,
    req: &HttpRequest,
    cancel: &CancellationToken,
    deadline: Option<Instant>,
) -> Result<HttpResponse, ProviderError> {
    let guard = IoGuard::new(cancel, deadline);
    guard.check()?;
    let addrs = resolve(endpoint)?;
    let mut stream = connect(&addrs, &guard)?;
    write_request(&mut stream, endpoint, req, &guard)?;
    let head = read_head(&mut stream, &guard)?;
    let body = open_body(
        stream,
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

fn connect(addrs: &[SocketAddr], guard: &IoGuard) -> Result<TcpStream, ProviderError> {
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
                // cancel/deadline mid-transfer.
                let _ = stream.set_nodelay(true);
                let _ = stream.set_read_timeout(Some(IO_POLL));
                let _ = stream.set_write_timeout(Some(IO_POLL));
                return Ok(stream);
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

fn write_request(
    stream: &mut TcpStream,
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
    stream: &mut TcpStream,
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
fn read_head(stream: &mut TcpStream, guard: &IoGuard) -> Result<ResponseHead, ProviderError> {
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
    stream: TcpStream,
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
    stream: TcpStream,
    pending: Vec<u8>,
    pending_pos: usize,
    guard: IoGuard,
}

impl BodyIo {
    fn new(
        stream: TcpStream,
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
        stream: TcpStream,
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
        stream: TcpStream,
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
        stream: TcpStream,
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
