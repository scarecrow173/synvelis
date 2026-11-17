//! Shared WebDAV mock server for ALIB-06/ALIB-07 tests.
//!
//! An in-process `TcpListener` mock — a scripted HTTP/1.1 server
//! covering `PROPFIND` multistatus bodies, conditional `GET`s, `Range`
//! responses, redirects, malformed wire input, and typed error
//! statuses. Real network hosts, TLS, OS credential stores, and the
//! GUI are NotRun/Unsupported by design — see
//! `docs/test-specs/alib-06-webdav-provider.md` and
//! `docs/test-specs/alib-07-fault-matrix.md`.
//!
//! Helpers are shared across test targets; each target only uses a
//! subset, so dead-code is allowed module-wide.

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

/// One parsed HTTP request the mock observed.
pub struct MockRequest {
    pub method: String,
    /// Raw (still percent-encoded) request target.
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl MockRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// A scripted response.
pub struct MockResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    /// When set, the response bytes are written verbatim — used for
    /// malformed heads/bodies that the normal builder cannot express.
    raw: Option<Vec<u8>>,
}

impl MockResponse {
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            raw: None,
        }
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }

    /// A verbatim response — head and body exactly as given. The
    /// fixture adds nothing (no auto `Content-Length`, no status
    /// line), so protocol faults reach the client untouched.
    pub fn raw(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 0,
            headers: Vec::new(),
            body: Vec::new(),
            raw: Some(bytes.into()),
        }
    }
}

impl From<u16> for MockResponse {
    fn from(status: u16) -> Self {
        Self::new(status)
    }
}

pub type Handler = Arc<dyn Fn(&MockRequest) -> MockResponse + Send + Sync>;

/// An in-process HTTP/1.1 server: one request per connection,
/// `Connection: close`, sequential accept loop.
pub struct MockServer {
    addr: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MockServer {
    pub fn start(handler: Handler) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("local addr").port();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => handle_conn(stream, &handler),
                    Err(e) if transient_accept_error(&e) => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            addr: format!("127.0.0.1:{port}"),
            stop,
            thread: Some(thread),
        }
    }

    /// `http://127.0.0.1:PORT`
    pub fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // A dummy connect wakes a blocked accept on platforms where
        // nonblocking accept still parks briefly.
        let _ = TcpStream::connect(&self.addr);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        206 => "Partial Content",
        207 => "Multi-Status",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        412 => "Precondition Failed",
        416 => "Range Not Satisfiable",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// Accept-loop errors that describe a dead *pending* connection (a
/// queued peer that reset before accept — surfaced by Winsock, folded
/// away by POSIX) rather than a dead listener. Those are per-connection
/// noise; only listener-level errors may end the loop.
fn transient_accept_error(e: &std::io::Error) -> bool {
    use std::io::ErrorKind as K;
    matches!(
        e.kind(),
        K::WouldBlock | K::ConnectionReset | K::ConnectionAborted | K::TimedOut | K::Interrupted
    )
}

fn handle_conn(mut stream: TcpStream, handler: &Handler) {
    // Winsock propagates the listener's nonblocking flag to the
    // accepted socket; POSIX does not. `read_request`/`write_response`
    // assume a blocking socket with timeouts — normalize the mode here
    // so a `read` that beats the client's first bytes blocks instead
    // of failing `WouldBlock` and RST-ing the connection (os error
    // 10054/10053 on the client; synvelis#245).
    normalize_socket(&mut stream);
    serve(&mut stream, handler);
}

/// Blocking mode + generous timeouts for an accepted socket — the
/// same normalization [`handle_conn`] applies, shared with the TLS
/// path where the raw socket is wrapped before serving.
fn normalize_socket(stream: &mut TcpStream) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));
}

/// Reads one request, runs the handler, writes one response — the
/// shared tail of every mock connection, plaintext or TLS.
fn serve<S: Read + Write>(stream: &mut S, handler: &Handler) {
    let Ok(req) = read_request(stream) else {
        return;
    };
    let resp = handler(&req);
    let _ = write_response(stream, &resp);
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Reads one request: head through `\r\n\r\n`, then `Content-Length`
/// body bytes. Bounded — a request head over 1 MiB is dropped.
fn read_request<S: Read>(stream: &mut S) -> std::io::Result<MockRequest> {
    let mut buf = Vec::with_capacity(8192);
    let mut chunk = [0u8; 8192];
    let head_end = loop {
        if let Some(i) = find_subslice(&buf, b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > 1 << 20 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request head too large",
            ));
        }
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "eof in request head",
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let (Some(method), Some(path)) = (parts.next(), parts.next()) else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bad request line",
        ));
    };
    let mut headers = Vec::new();
    let mut content_length = 0usize;
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim().to_owned();
        if name == "content-length" {
            content_length = value.parse().unwrap_or(0);
        }
        headers.push((name, value));
    }
    let mut body = buf[head_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    body.truncate(content_length);
    Ok(MockRequest {
        method: method.to_owned(),
        path: path.to_owned(),
        headers,
        body,
    })
}

fn write_response<S: Write>(stream: &mut S, resp: &MockResponse) -> std::io::Result<()> {
    if let Some(raw) = &resp.raw {
        return stream.write_all(raw).and_then(|()| stream.flush());
    }
    let mut out = format!(
        "HTTP/1.1 {} {}\r\n",
        resp.status,
        reason_phrase(resp.status)
    );
    let mut has_length = false;
    for (name, value) in &resp.headers {
        let _ = write!(out, "{name}: {value}\r\n");
        has_length |= name.eq_ignore_ascii_case("content-length");
    }
    if !has_length {
        let _ = write!(out, "content-length: {}\r\n", resp.body.len());
    }
    out.push_str("connection: close\r\n\r\n");
    stream.write_all(out.as_bytes())?;
    stream.write_all(&resp.body)?;
    stream.flush()
}

// ------------------------------------------------------------------
// DAV response builders
// ------------------------------------------------------------------

/// A `multistatus` body with one `response` per `(href, props)` —
/// `props` is raw inner XML for the `prop` element.
pub fn multistatus(responses: &[(&str, &str)]) -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
         <d:multistatus xmlns:d=\"DAV:\">",
    );
    for (href, props) in responses {
        out.push_str("<d:response><d:href>");
        out.push_str(href);
        out.push_str("</d:href><d:propstat><d:prop>");
        out.push_str(props);
        out.push_str("</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>");
    }
    out.push_str("</d:multistatus>");
    out
}

pub fn file_props(size: u64, etag: Option<&str>) -> String {
    let etag = etag.map_or(String::new(), |e| format!("<d:getetag>{e}</d:getetag>"));
    format!(
        "<d:resourcetype/><d:getcontentlength>{size}</d:getcontentlength>\
         <d:getlastmodified>Sun, 06 Nov 1994 08:49:37 GMT</d:getlastmodified>{etag}"
    )
}

pub fn dir_props() -> &'static str {
    "<d:resourcetype><d:collection/></d:resourcetype>"
}

/// A `207 Multi-Status` response. `_path` documents which resource
/// the fixture answers; the body is path-agnostic.
pub fn propfind_response(_path: &str, responses: &[(&str, &str)]) -> MockResponse {
    MockResponse::new(207)
        .header("content-type", "application/xml; charset=utf-8")
        .body(multistatus(responses))
}

// ------------------------------------------------------------------
// TLS fixture — Git LFS HTTPS transport tests (issue #521)
// ------------------------------------------------------------------

/// TLS material for one mock HTTPS endpoint: a fresh self-signed CA
/// plus a leaf certificate for `127.0.0.1`/`localhost`, bound into a
/// rustls `ServerConfig`.
///
/// Every [`TlsMaterial::new`] mints an *independent* CA — pairing a
/// server with another instance's CA exercises the client's
/// verification-failure path (`TlsTrust::from_pem` trusts exactly the
/// CA it was given).
pub struct TlsMaterial {
    ca_pem: Vec<u8>,
    server_config: Arc<rustls::ServerConfig>,
}

impl TlsMaterial {
    /// Generates a fresh CA and a `127.0.0.1`/`localhost` leaf — IP
    /// and DNS SANs so both literal and name spellings verify.
    ///
    /// # Panics
    /// Fixture generation failure (keygen, signing, config build) —
    /// a broken fixture is a test bug, not an environmental
    /// condition.
    pub fn new() -> Self {
        use rcgen::{BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair};
        use rustls::pki_types::pem::PemObject;

        let ca_key = KeyPair::generate().expect("fixture ca key");
        let mut ca_params =
            CertificateParams::new(Vec::<String>::new()).expect("fixture ca params");
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let ca_cert = ca_params.self_signed(&ca_key).expect("fixture ca cert");
        let issuer = Issuer::from_params(&ca_params, ca_key);

        let server_key = KeyPair::generate().expect("fixture server key");
        let server_params =
            CertificateParams::new(vec!["127.0.0.1".to_owned(), "localhost".to_owned()])
                .expect("fixture server params");
        let server_cert = server_params
            .signed_by(&server_key, &issuer)
            .expect("signed fixture server cert");

        let key_der =
            rustls::pki_types::PrivateKeyDer::from_pem_slice(server_key.serialize_pem().as_bytes())
                .expect("fixture server key pem");
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("fixture protocol versions")
            .with_no_client_auth()
            .with_single_cert(vec![server_cert.der().clone()], key_der)
            .expect("fixture server config");

        Self {
            ca_pem: ca_cert.pem().into_bytes(),
            server_config: Arc::new(config),
        }
    }

    /// The CA certificate PEM — the trust input for
    /// `TlsTrust::from_pem` that makes this material's leaf verify.
    pub fn ca_pem(&self) -> &[u8] {
        &self.ca_pem
    }

    /// A fresh *unrelated* CA's PEM — trusting this while talking to
    /// a server built from any [`TlsMaterial`] must fail verification.
    pub fn foreign_ca_pem() -> Vec<u8> {
        Self::new().ca_pem
    }

    fn server_config(&self) -> Arc<rustls::ServerConfig> {
        Arc::clone(&self.server_config)
    }
}

/// An in-process HTTPS server: [`MockServer`]'s accept loop with a
/// rustls `ServerConnection` wrapped around each accepted socket.
/// One request per TLS connection, `Connection: close`, sequential —
/// identical wire semantics to the plaintext mock.
pub struct MockTlsServer {
    addr: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    material: TlsMaterial,
}

impl MockTlsServer {
    /// Starts a TLS listener with a fresh [`TlsMaterial`].
    pub fn start(handler: Handler) -> Self {
        let material = TlsMaterial::new();
        let config = material.server_config();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind tls mock");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("local addr").port();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => handle_tls_conn(stream, &config, &handler),
                    Err(e) if transient_accept_error(&e) => {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            addr: format!("127.0.0.1:{port}"),
            stop,
            thread: Some(thread),
            material,
        }
    }

    /// `https://127.0.0.1:PORT`
    pub fn url(&self) -> String {
        format!("https://{}", self.addr)
    }

    /// The fixture CA PEM — the `TlsTrust::from_pem` input that
    /// makes this server's certificate verify.
    pub fn trust_pem(&self) -> Vec<u8> {
        self.material.ca_pem.clone()
    }

    /// An unrelated CA's PEM — trusting this while talking to any
    /// [`MockTlsServer`] must fail certificate verification.
    pub fn foreign_trust_pem() -> Vec<u8> {
        TlsMaterial::foreign_ca_pem()
    }
}

impl Drop for MockTlsServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.addr);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Accepts one TLS connection and serves it like [`handle_conn`].
/// The handshake runs lazily inside the first `read`/`write` — a
/// client that fails verification simply never produces a request,
/// and the connection is dropped.
fn handle_tls_conn(mut stream: TcpStream, config: &Arc<rustls::ServerConfig>, handler: &Handler) {
    normalize_socket(&mut stream);
    let Ok(conn) = rustls::ServerConnection::new(Arc::clone(config)) else {
        return;
    };
    let mut tls = rustls::StreamOwned::new(conn, stream);
    serve(&mut tls, handler);
}
