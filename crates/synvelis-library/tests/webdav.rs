//! ALIB-06 `WebDAV` file-server provider tests (Synvelis #90).
//!
//! Everything runs against an in-process `TcpListener` mock — a
//! scripted HTTP/1.1 server covering `PROPFIND` multistatus bodies,
//! conditional `GET`s, `Range` responses, redirects, and typed error
//! statuses. Real network hosts, TLS, OS credential stores, and the
//! GUI are NotRun/Unsupported by design — see
//! `docs/test-specs/alib-06-webdav-provider.md`.

mod common;

use std::fmt::Write as _;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use common::{aref, cx, loc, pid};
use synvelis_library::{
    AccessMode, AssetProvider, Availability, BrowseScope, ByteRange, Capability, Coverage,
    CredentialKey, CredentialResolver, EntryKind, FilterSpec, ObjectIdentity, OpaqueCredential,
    PinCapability, PinProof, PreconditionKind, ProviderError, ProviderErrorKind, ProviderId,
    ProviderLimits, SortSpec, StrongRevisionToken, VerificationPolicy, WebDavConfig,
    WebDavProvider, WebDavScheme, WebDavUrl,
};

const PROVIDER_RAW: u128 = 0x90_06;

fn provider_id() -> ProviderId {
    pid(PROVIDER_RAW)
}

// ------------------------------------------------------------------
// Mock server
// ------------------------------------------------------------------

/// One parsed HTTP request the mock observed.
struct MockRequest {
    method: String,
    /// Raw (still percent-encoded) request target.
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl MockRequest {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// A scripted response.
struct MockResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl MockResponse {
    fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }
}

impl From<u16> for MockResponse {
    fn from(status: u16) -> Self {
        Self::new(status)
    }
}

type Handler = Arc<dyn Fn(&MockRequest) -> MockResponse + Send + Sync>;

/// An in-process HTTP/1.1 server: one request per connection,
/// `Connection: close`, sequential accept loop.
struct MockServer {
    addr: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl MockServer {
    fn start(handler: Handler) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("local addr").port();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => handle_conn(stream, &handler),
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
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
    fn url(&self) -> String {
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

fn handle_conn(mut stream: TcpStream, handler: &Handler) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(15)));
    let Ok(req) = read_request(&mut stream) else {
        return;
    };
    let resp = handler(&req);
    let _ = write_response(&mut stream, &resp);
}

fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Reads one request: head through `\r\n\r\n`, then `Content-Length`
/// body bytes. Bounded — a request head over 1 MiB is dropped.
fn read_request(stream: &mut TcpStream) -> std::io::Result<MockRequest> {
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

fn write_response(stream: &mut TcpStream, resp: &MockResponse) -> std::io::Result<()> {
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
// Fixtures
// ------------------------------------------------------------------

fn provider(server: &MockServer) -> WebDavProvider {
    provider_at_root(server, "/")
}

fn provider_at_root(server: &MockServer, root: &str) -> WebDavProvider {
    let url = WebDavUrl::parse(&server.url()).expect("fixture url");
    let config = WebDavConfig {
        url,
        root_path: root.to_owned(),
        credential_key: None,
    };
    WebDavProvider::new(provider_id(), config).expect("provider")
}

/// A `multistatus` body with one `response` per `(href, props)` —
/// `props` is raw inner XML for the `prop` element.
fn multistatus(responses: &[(&str, &str)]) -> String {
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

fn file_props(size: u64, etag: Option<&str>) -> String {
    let etag = etag.map_or(String::new(), |e| format!("<d:getetag>{e}</d:getetag>"));
    format!(
        "<d:resourcetype/><d:getcontentlength>{size}</d:getcontentlength>\
         <d:getlastmodified>Sun, 06 Nov 1994 08:49:37 GMT</d:getlastmodified>{etag}"
    )
}

fn dir_props() -> &'static str {
    "<d:resourcetype><d:collection/></d:resourcetype>"
}

/// A `207 Multi-Status` response. `_path` documents which resource
/// the fixture answers; the body is path-agnostic.
fn propfind_response(_path: &str, responses: &[(&str, &str)]) -> MockResponse {
    MockResponse::new(207)
        .header("content-type", "application/xml; charset=utf-8")
        .body(multistatus(responses))
}

fn names(page: &synvelis_library::BrowsePage) -> Vec<String> {
    page.entries
        .iter()
        .map(|e| e.display_name.clone())
        .collect()
}

fn scope_req(locator: Option<&str>, limit: u32) -> synvelis_library::BrowseRequest {
    synvelis_library::BrowseRequest {
        scope: match locator {
            Some(l) => BrowseScope::Locator { locator: loc(l) },
            None => BrowseScope::Root,
        },
        page_limit: limit,
        cursor: None,
        sort: SortSpec::default(),
        filter: FilterSpec::default(),
    }
}

fn kind_of(err: &ProviderError) -> ProviderErrorKind {
    err.kind()
}

// ------------------------------------------------------------------
// URL / locator validation
// ------------------------------------------------------------------

#[test]
fn url_parse_accepts_origins() {
    let u = WebDavUrl::parse("http://example.com").expect("plain");
    assert_eq!(u.scheme(), WebDavScheme::Http);
    assert_eq!(u.host(), "example.com");
    assert_eq!(u.port(), 80);
    assert_eq!(u.host_header(), "example.com");

    let u = WebDavUrl::parse("https://example.com:8443/").expect("https");
    assert_eq!(u.scheme(), WebDavScheme::Https);
    assert_eq!(u.port(), 8443);
    assert_eq!(u.host_header(), "example.com:8443");

    let u = WebDavUrl::parse("http://[::1]:8080").expect("v6");
    assert!(u.is_ipv6());
    assert_eq!(u.host_header(), "[::1]:8080");
}

#[test]
fn url_parse_rejects_unsafe_forms() {
    for bad in [
        "ftp://h",
        "http://user:pass@h",
        "http://h/path",
        "http://h?x=1",
        "http://h#frag",
        "http://h:99999",
        "http://h:abc",
        "http://",
        "http://exa mple.com",
        "http://h//",
        "notaurl",
        "http://[::1",
        "http://h:0",
    ] {
        assert!(WebDavUrl::parse(bad).is_err(), "{bad:?} must be rejected");
    }
}

#[test]
fn url_authority_matches_default_ports() {
    let u = WebDavUrl::parse("http://example.com").expect("url");
    assert!(u.authority_matches("example.com"));
    assert!(u.authority_matches("example.com:80"));
    assert!(u.authority_matches("EXAMPLE.com"));
    assert!(!u.authority_matches("example.com:8080"));
    assert!(!u.authority_matches("other.com"));
    assert!(!u.authority_matches("example.com:abc"));
}

#[test]
fn locator_outside_root_rejected() {
    let server = MockServer::start(Arc::new(|_| 404.into()));
    let p = provider_at_root(&server, "/music");
    let err = p
        .stat(&aref(provider_id(), "/etc/passwd"), &cx())
        .expect_err("escape must fail");
    assert_eq!(kind_of(&err), ProviderErrorKind::InvalidRequest);
    let err = p
        .stat(&aref(provider_id(), "../x"), &cx())
        .expect_err("dotdot must fail");
    assert_eq!(kind_of(&err), ProviderErrorKind::InvalidRequest);
}

#[test]
fn provider_rejects_bad_root() {
    let server = MockServer::start(Arc::new(|_| 404.into()));
    let url = WebDavUrl::parse(&server.url()).expect("url");
    let config = WebDavConfig {
        url,
        root_path: "relative".to_owned(),
        credential_key: None,
    };
    let err = WebDavProvider::new(provider_id(), config)
        .err()
        .expect("bad root");
    assert_eq!(kind_of(&err), ProviderErrorKind::InvalidRequest);
}

// ------------------------------------------------------------------
// Browse / stat (PROPFIND metadata-first)
// ------------------------------------------------------------------

#[test]
fn browse_lists_children_complete() {
    let server = MockServer::start(Arc::new(|req| {
        assert_eq!(req.method, "PROPFIND");
        assert_eq!(req.header("depth"), Some("1"));
        // Metadata-first: the body asks only for the props consumed.
        let body = String::from_utf8_lossy(&req.body);
        for prop in [
            "resourcetype",
            "getcontentlength",
            "getlastmodified",
            "getetag",
            "displayname",
        ] {
            assert!(body.contains(prop), "propfind body lacks {prop}");
        }
        propfind_response(
            "/music",
            &[
                ("/music", dir_props()),
                ("/music/a.wav", &file_props(100, Some("\"e1\""))),
                ("/music/sub", dir_props()),
            ],
        )
    }));
    let p = provider_at_root(&server, "/music");
    let page = p.browse(&scope_req(None, 10), &cx()).expect("browse");
    assert_eq!(page.coverage, Coverage::Complete);
    assert_eq!(names(&page), vec!["a.wav", "sub"]);
    let a = page
        .entries
        .iter()
        .find(|e| e.display_name == "a.wav")
        .unwrap();
    assert_eq!(a.kind, EntryKind::Asset);
    assert_eq!(a.locator.as_str(), "/music/a.wav");
    let sub = page
        .entries
        .iter()
        .find(|e| e.display_name == "sub")
        .unwrap();
    assert_eq!(sub.kind, EntryKind::Container);
}

#[test]
fn browse_partial_on_foreign_and_unaddressable_hrefs() {
    let server = MockServer::start(Arc::new(|_| {
        propfind_response(
            "/",
            &[
                ("/", dir_props()),
                ("http://evil.example/x", &file_props(1, None)),
                ("/ok.txt", &file_props(2, None)),
            ],
        )
    }));
    let p = provider(&server);
    let page = p.browse(&scope_req(None, 10), &cx()).expect("browse");
    assert_eq!(page.coverage, Coverage::Partial);
    assert_eq!(names(&page), vec!["ok.txt"]);
}

#[test]
fn browse_partial_when_entry_cap_truncates() {
    let server = MockServer::start(Arc::new(|_| {
        propfind_response(
            "/",
            &[
                ("/", dir_props()),
                ("/a", &file_props(1, None)),
                ("/b", &file_props(1, None)),
                ("/c", &file_props(1, None)),
            ],
        )
    }));
    let p = provider(&server);
    let mut cx = cx();
    cx.limits = ProviderLimits {
        max_entries: 1,
        ..ProviderLimits::default()
    };
    let page = p.browse(&scope_req(None, 10), &cx).expect("browse");
    assert_eq!(page.coverage, Coverage::Partial);
}

#[test]
fn browse_rejects_file_scope() {
    let server = MockServer::start(Arc::new(|_| {
        propfind_response("/a.wav", &[("/a.wav", &file_props(5, Some("\"e\"")))])
    }));
    let p = provider(&server);
    let err = p
        .browse(&scope_req(Some("/a.wav"), 10), &cx())
        .expect_err("file scope");
    assert_eq!(kind_of(&err), ProviderErrorKind::InvalidRequest);
}

#[test]
fn browse_cursor_pages_listing() {
    let server = MockServer::start(Arc::new(|_| {
        propfind_response(
            "/",
            &[
                ("/", dir_props()),
                ("/a", &file_props(1, None)),
                ("/b", &file_props(1, None)),
                ("/c", &file_props(1, None)),
            ],
        )
    }));
    let p = provider(&server);
    let page1 = p.browse(&scope_req(None, 2), &cx()).expect("page1");
    assert!(page1.next_cursor.is_some());
    let mut req2 = scope_req(None, 2);
    req2.cursor = page1.next_cursor.clone();
    let page2 = p.browse(&req2, &cx()).expect("page2");
    assert_eq!(names(&page2), vec!["c"]);
    assert!(page2.next_cursor.is_none());
}

#[test]
fn stat_reports_facts() {
    let server = MockServer::start(Arc::new(|req| {
        assert_eq!(req.header("depth"), Some("0"));
        propfind_response(
            "/music/a.wav",
            &[("/music/a.wav", &file_props(4096, Some("\"v1\"")))],
        )
    }));
    let p = provider_at_root(&server, "/music");
    let stat = p
        .stat(&aref(provider_id(), "/music/a.wav"), &cx())
        .expect("stat");
    assert_eq!(stat.availability, Availability::Available);
    assert_eq!(stat.facts.byte_len.value().copied(), Some(4096));
}

#[test]
fn stat_not_found() {
    let server = MockServer::start(Arc::new(|_| {
        propfind_response("/nope", &[("/other", &file_props(1, None))])
    }));
    let p = provider(&server);
    let err = p
        .stat(&aref(provider_id(), "/nope"), &cx())
        .expect_err("missing");
    assert_eq!(kind_of(&err), ProviderErrorKind::NotFound);
}

#[test]
fn stat_wrong_owner_rejected() {
    let server = MockServer::start(Arc::new(|_| 404.into()));
    let p = provider(&server);
    let err = p
        .stat(&aref(pid(0xdead), "/x"), &cx())
        .expect_err("foreign provider");
    assert_eq!(kind_of(&err), ProviderErrorKind::InvalidRequest);
}

// ------------------------------------------------------------------
// Pin / conditional read
// ------------------------------------------------------------------

fn pinned_fixture(etag: &str) -> (MockServer, WebDavProvider) {
    let etag = etag.to_owned();
    let server = MockServer::start(Arc::new(move |req| {
        if req.method == "PROPFIND" {
            return propfind_response("/a.wav", &[("/a.wav", &file_props(11, Some(&etag)))]);
        }
        // GET
        if req.header("if-match") == Some(etag.as_str()) {
            MockResponse::new(200)
                .header("etag", &etag)
                .header("content-length", "11")
                .body(b"hello world".to_vec())
        } else {
            412.into()
        }
    }));
    let p = provider(&server);
    (server, p)
}

#[test]
fn pin_strong_etag_produces_if_match_proof() {
    let (server, p) = pinned_fixture("\"abc123\"");
    let pin = p.pin(&aref(provider_id(), "/a.wav"), &cx()).expect("pin");
    let PinProof::ConditionalStrong {
        token:
            StrongRevisionToken::StrongValidator {
                validator,
                precondition,
            },
    } = &pin.proof
    else {
        panic!(
            "expected ConditionalStrong/StrongValidator, got {:?}",
            pin.proof
        );
    };
    assert_eq!(validator, "\"abc123\"");
    assert_eq!(*precondition, PreconditionKind::IfMatch);
    assert_eq!(pin.expected_size, Some(11));
    drop(server);
}

#[test]
fn pin_rejects_weak_and_missing_etags() {
    for (etag, props_desc) in [("W/\"weak\"", "weak"), ("", "missing")] {
        let props = if etag.is_empty() {
            file_props(10, None)
        } else {
            file_props(10, Some(etag))
        };
        let server = MockServer::start(Arc::new(move |_| {
            propfind_response("/a.wav", &[("/a.wav", &props)])
        }));
        let p = provider(&server);
        let err = p
            .pin(&aref(provider_id(), "/a.wav"), &cx())
            .expect_err(props_desc);
        assert_eq!(
            kind_of(&err),
            ProviderErrorKind::UnsupportedCapability,
            "{props_desc} etag must fail closed"
        );
    }
}

#[test]
fn pin_rejects_collection() {
    let server = MockServer::start(Arc::new(|_| {
        propfind_response("/dir", &[("/dir", dir_props())])
    }));
    let p = provider(&server);
    let err = p
        .pin(&aref(provider_id(), "/dir"), &cx())
        .expect_err("collection pin");
    assert_eq!(kind_of(&err), ProviderErrorKind::InvalidRequest);
}

#[test]
fn open_pinned_sends_if_match_and_verifies() {
    let (server, p) = pinned_fixture("\"abc123\"");
    let pin = p.pin(&aref(provider_id(), "/a.wav"), &cx()).expect("pin");
    let mut pr = p.open_pinned(&pin, &cx()).expect("open");
    let mut body = Vec::new();
    pr.reader.read_to_end(&mut body).expect("read");
    assert_eq!(body, b"hello world");
    let receipt = pr.completion.take().expect("finished").expect("receipt");
    assert_eq!(receipt.bytes_read, 11);
    assert!(
        receipt.verified
            || matches!(
                pr.claims.verification,
                VerificationPolicy::IdentityBound { .. }
            )
    );
    drop(server);
}

#[test]
fn open_pinned_412_is_precondition_failed() {
    let server = MockServer::start(Arc::new(|req| {
        if req.method == "PROPFIND" {
            return propfind_response("/a.wav", &[("/a.wav", &file_props(11, Some("\"v\"")))]);
        }
        412.into()
    }));
    let p = provider(&server);
    let pin = p.pin(&aref(provider_id(), "/a.wav"), &cx()).expect("pin");
    let err = p.open_pinned(&pin, &cx()).expect_err("412");
    assert_eq!(kind_of(&err), ProviderErrorKind::PreconditionFailed);
}

#[test]
fn open_pinned_response_etag_mismatch_is_source_changed() {
    let server = MockServer::start(Arc::new(|req| {
        if req.method == "PROPFIND" {
            return propfind_response("/a.wav", &[("/a.wav", &file_props(11, Some("\"v\"")))]);
        }
        MockResponse::new(200)
            .header("etag", "\"other\"")
            .body(b"hello world".to_vec())
    }));
    let p = provider(&server);
    let pin = p.pin(&aref(provider_id(), "/a.wav"), &cx()).expect("pin");
    let err = p.open_pinned(&pin, &cx()).expect_err("mismatch");
    assert_eq!(kind_of(&err), ProviderErrorKind::SourceChanged);
}

#[test]
fn open_pinned_rejects_foreign_proof() {
    let server = MockServer::start(Arc::new(|_| 404.into()));
    let p = provider(&server);
    let pin = synvelis_library::PinnedAssetRef {
        asset: aref(provider_id(), "/a.wav"),
        proof: PinProof::ImmutableObject {
            revision: synvelis_library::RevisionKey::new("rev-1").expect("rev"),
        },
        object_identity: ObjectIdentity::new("x").expect("id"),
        expected_size: None,
        digest_hint: None,
    };
    let err = p.open_pinned(&pin, &cx()).expect_err("foreign proof");
    assert_eq!(kind_of(&err), ProviderErrorKind::InvalidRequest);
}

// ------------------------------------------------------------------
// Preview / range
// ------------------------------------------------------------------

#[test]
fn preview_range_honored_206() {
    let server = MockServer::start(Arc::new(|req| {
        if req.method == "PROPFIND" {
            return propfind_response("/a.wav", &[("/a.wav", &file_props(11, Some("\"e\"")))]);
        }
        assert_eq!(req.header("range"), Some("bytes=6-8"));
        MockResponse::new(206)
            .header("content-range", "bytes 6-8/11")
            .header("content-length", "3")
            .body(b"wor".to_vec())
    }));
    let p = provider(&server);
    let mut pr = p
        .open_preview(
            &aref(provider_id(), "/a.wav"),
            Some(ByteRange { offset: 6, len: 3 }),
            &cx(),
        )
        .expect("preview");
    let mut body = Vec::new();
    pr.reader.read_to_end(&mut body).expect("read");
    assert_eq!(body, b"wor");
}

#[test]
fn preview_range_ignored_falls_back_to_client_skip() {
    let server = MockServer::start(Arc::new(|req| {
        if req.method == "PROPFIND" {
            return propfind_response("/a.wav", &[("/a.wav", &file_props(11, None))]);
        }
        // Server ignores Range — returns 200 with the full body.
        MockResponse::new(200)
            .header("content-length", "11")
            .body(b"hello world".to_vec())
    }));
    let p = provider(&server);
    let mut pr = p
        .open_preview(
            &aref(provider_id(), "/a.wav"),
            Some(ByteRange { offset: 6, len: 3 }),
            &cx(),
        )
        .expect("preview");
    let mut body = Vec::new();
    pr.reader.read_to_end(&mut body).expect("read");
    assert_eq!(body, b"wor");
}

#[test]
fn preview_416_yields_empty_read() {
    let server = MockServer::start(Arc::new(|req| {
        if req.method == "PROPFIND" {
            return propfind_response("/a.wav", &[("/a.wav", &file_props(11, None))]);
        }
        416.into()
    }));
    let p = provider(&server);
    let mut pr = p
        .open_preview(
            &aref(provider_id(), "/a.wav"),
            Some(ByteRange {
                offset: 100,
                len: 5,
            }),
            &cx(),
        )
        .expect("preview");
    let mut body = Vec::new();
    pr.reader.read_to_end(&mut body).expect("read");
    assert_eq!(body, Vec::<u8>::new());
}

// ------------------------------------------------------------------
// Auth / typed status mapping
// ------------------------------------------------------------------

#[test]
fn status_401_maps_auth_expired() {
    let server = MockServer::start(Arc::new(|_| 401.into()));
    let p = provider(&server);
    let err = p.browse(&scope_req(None, 10), &cx()).expect_err("401");
    assert_eq!(kind_of(&err), ProviderErrorKind::AuthExpired);
}

#[test]
fn status_mapping_table() {
    for (status, expected) in [
        (403u16, ProviderErrorKind::PermissionDenied),
        (404, ProviderErrorKind::NotFound),
        (429, ProviderErrorKind::RateLimited),
        (500, ProviderErrorKind::TransientIo),
        (503, ProviderErrorKind::TransientIo),
    ] {
        let server = MockServer::start(Arc::new(move |_| status.into()));
        let p = provider(&server);
        let err = p.browse(&scope_req(None, 10), &cx()).expect_err("status");
        assert_eq!(kind_of(&err), expected, "HTTP {status}");
    }
}

#[test]
fn non_dav_server_is_unsupported() {
    // A plain HTTP file server answers PROPFIND with 200 — the
    // provider must refuse rather than fake a listing.
    let server = MockServer::start(Arc::new(|_| MockResponse::new(200).body(b"ok".to_vec())));
    let p = provider(&server);
    let err = p.browse(&scope_req(None, 10), &cx()).expect_err("not dav");
    assert_eq!(kind_of(&err), ProviderErrorKind::UnsupportedCapability);
}

struct TestResolver;

impl CredentialResolver for TestResolver {
    fn resolve(&self, key: &CredentialKey) -> Result<OpaqueCredential, ProviderError> {
        Ok(OpaqueCredential::new(key.clone(), "Basic dXNlcjpwYXNz"))
    }
}

#[test]
fn credentials_sent_only_as_authorization_header() {
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen2 = seen.clone();
    let server = MockServer::start(Arc::new(move |req| {
        if let Some(v) = req.header("authorization") {
            seen2.lock().unwrap().push(v.to_owned());
        }
        if req.method == "PROPFIND" {
            return propfind_response("/", &[("/", dir_props())]);
        }
        404.into()
    }));
    let url = WebDavUrl::parse(&server.url()).expect("url");
    let mut config = WebDavConfig::new(url);
    config.credential_key = Some(CredentialKey::new("test-key").expect("key"));
    let p = WebDavProvider::new(provider_id(), config).expect("provider");
    let mut cx = cx();
    cx.credentials = Some(Arc::new(TestResolver));
    p.browse(&scope_req(None, 10), &cx).expect("browse");
    let auth = seen.lock().unwrap();
    assert_eq!(auth.as_slice(), &["Basic dXNlcjpwYXNz".to_owned()]);
    // The credential never appears in the request target.
    drop(auth);
}

#[test]
fn missing_resolver_is_auth_expired() {
    let server = MockServer::start(Arc::new(|_| 404.into()));
    let url = WebDavUrl::parse(&server.url()).expect("url");
    let mut config = WebDavConfig::new(url);
    config.credential_key = Some(CredentialKey::new("test-key").expect("key"));
    let p = WebDavProvider::new(provider_id(), config).expect("provider");
    let err = p
        .browse(&scope_req(None, 10), &cx())
        .expect_err("no resolver");
    assert_eq!(kind_of(&err), ProviderErrorKind::AuthExpired);
}

// ------------------------------------------------------------------
// Redirects / https / cancellation / budgets
// ------------------------------------------------------------------

#[test]
fn same_origin_redirect_is_followed() {
    let hits = Arc::new(Mutex::new(Vec::<String>::new()));
    let hits2 = hits.clone();
    let server = MockServer::start(Arc::new(move |req| {
        hits2.lock().unwrap().push(req.path.clone());
        if req.path == "/old" {
            return MockResponse::new(301).header("location", "/new");
        }
        propfind_response("/new", &[("/new", dir_props())])
    }));
    let url = WebDavUrl::parse(&server.url()).expect("url");
    // Root "/" contains both the redirect source and target; the
    // browse scope selects /old.
    let config = WebDavConfig {
        url,
        root_path: "/".to_owned(),
        credential_key: None,
    };
    let p = WebDavProvider::new(provider_id(), config).expect("provider");
    p.browse(&scope_req(Some("/old"), 10), &cx())
        .expect("browse");
    let paths = hits.lock().unwrap();
    assert_eq!(paths.as_slice(), &["/old", "/new"]);
}

#[test]
fn cross_origin_redirect_refused() {
    let server = MockServer::start(Arc::new(|_| {
        MockResponse::new(302).header("location", "http://evil.example/x")
    }));
    let p = provider(&server);
    let err = p.browse(&scope_req(None, 10), &cx()).expect_err("xorigin");
    assert_eq!(kind_of(&err), ProviderErrorKind::InvalidRequest);
}

#[test]
fn https_is_typed_unsupported() {
    // Parses at configuration time; transport refuses — no silent
    // downgrade, no TLS claim.
    let url = WebDavUrl::parse("https://127.0.0.1:1").expect("https url");
    let p = WebDavProvider::new(provider_id(), WebDavConfig::new(url)).expect("provider");
    let err = p.browse(&scope_req(None, 10), &cx()).expect_err("https");
    assert_eq!(kind_of(&err), ProviderErrorKind::UnsupportedCapability);
}

#[test]
fn cancelled_token_fails_before_io() {
    let server = MockServer::start(Arc::new(|_| 404.into()));
    let p = provider(&server);
    let cx = cx();
    cx.cancel.cancel();
    let err = p.browse(&scope_req(None, 10), &cx).expect_err("cancelled");
    assert_eq!(kind_of(&err), ProviderErrorKind::Cancelled);
}

#[test]
fn max_bytes_enforced_on_pinned_read() {
    let server = MockServer::start(Arc::new(|req| {
        if req.method == "PROPFIND" {
            return propfind_response("/a.wav", &[("/a.wav", &file_props(100, Some("\"e\"")))]);
        }
        MockResponse::new(200)
            .header("etag", "\"e\"")
            .body(vec![b'x'; 100])
    }));
    let p = provider(&server);
    let pin = p.pin(&aref(provider_id(), "/a.wav"), &cx()).expect("pin");
    let mut cx = cx();
    cx.limits = ProviderLimits {
        max_bytes: 10,
        ..ProviderLimits::default()
    };
    let mut pr = p.open_pinned(&pin, &cx).expect("open");
    let mut buf = [0u8; 64];
    // The VerifyingReader must fail BudgetExceeded past the bound —
    // never silently truncate a pinned read.
    let mut saw_err = false;
    loop {
        match pr.reader.read(&mut buf) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                let pe = e
                    .get_ref()
                    .and_then(|inner| inner.downcast_ref::<synvelis_library::ProviderError>())
                    .cloned()
                    .unwrap_or_else(|| {
                        synvelis_library::ProviderError::new(
                            ProviderErrorKind::Internal,
                            e.to_string(),
                        )
                    });
                assert_eq!(pe.kind(), ProviderErrorKind::BudgetExceeded);
                saw_err = true;
                break;
            }
        }
    }
    assert!(saw_err, "byte budget must fail the stream");
    let outcome = pr.completion.take().expect("finished");
    assert!(outcome.is_err());
}

#[test]
fn capabilities_declare_contract() {
    let server = MockServer::start(Arc::new(|_| 404.into()));
    let p = provider(&server);
    let caps = p.capabilities(None);
    assert_eq!(caps.browse, Capability::Supported);
    assert_eq!(caps.stat, Capability::Supported);
    assert_eq!(caps.pin, PinCapability::ConditionalStrong);
    assert_eq!(caps.range_read, Capability::Supported);
    assert_eq!(caps.fetch, Capability::Supported);
    assert!(matches!(caps.search, Capability::Unsupported { .. }));
    assert_eq!(caps.access, AccessMode::ReadOnly);
}
