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
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::webdav_fixture::{MockResponse, MockServer, dir_props, file_props, propfind_response};
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

// ------------------------------------------------------------------
// ALIB-07 fault matrix: offline / revoke / malformed wire / redirect
// ------------------------------------------------------------------

#[test]
fn offline_server_fails_typed_on_every_op() {
    let server = MockServer::start(Arc::new(|_| propfind_response("/", &[("/", dir_props())])));
    let p = provider(&server);
    p.browse(&scope_req(None, 10), &cx()).expect("browse");
    drop(server); // listener gone → connect refused
    for (name, result) in [
        ("browse", p.browse(&scope_req(None, 10), &cx()).map(|_| ())),
        (
            "stat",
            p.stat(&aref(provider_id(), "/x"), &cx()).map(|_| ()),
        ),
        ("pin", p.pin(&aref(provider_id(), "/x"), &cx()).map(|_| ())),
    ] {
        let err = result.expect_err(name);
        assert_eq!(kind_of(&err), ProviderErrorKind::Offline, "{name}");
    }
}

#[test]
fn auth_revoked_between_pin_and_conditional_get() {
    // Credentials still resolve, but the server revoked them between
    // the pin's PROPFIND and the conditional GET.
    let server = MockServer::start(Arc::new(|req| {
        if req.method == "PROPFIND" {
            return propfind_response("/a.wav", &[("/a.wav", &file_props(11, Some("\"v1\"")))]);
        }
        401.into()
    }));
    let p = provider(&server);
    let pin = p.pin(&aref(provider_id(), "/a.wav"), &cx()).expect("pin");
    let err = p.open_pinned(&pin, &cx()).expect_err("revoked");
    assert_eq!(kind_of(&err), ProviderErrorKind::AuthExpired);
}

#[test]
fn malformed_response_head_is_typed_transient() {
    // `MockResponse::raw` bypasses the fixture's own head writer so
    // each protocol fault reaches the client untouched.
    let mut too_many = String::from("HTTP/1.1 207 Multi-Status\r\n");
    for i in 0..150 {
        let _ = write!(too_many, "x-pad-{i}: v\r\n");
    }
    too_many.push_str("\r\n");
    let mut oversized = String::from("HTTP/1.1 207 Multi-Status\r\n");
    for _ in 0..700 {
        oversized.push_str("x-pad: ");
        oversized.push_str(&"a".repeat(96));
        oversized.push_str("\r\n");
    }
    oversized.push_str("\r\n");
    for raw in [
        b"this is not http\r\n\r\n".to_vec(),
        b"HTTP/1.1 ABC Nope\r\n\r\n".to_vec(),
        b"HTTP/1.1 207 Multi-Status\r\nx-pad: \xff\xfe bad\r\n\r\n".to_vec(),
        too_many.into_bytes(),
        oversized.into_bytes(),
    ] {
        let server = MockServer::start(Arc::new(move |_| MockResponse::raw(raw.clone())));
        let p = provider(&server);
        let err = p
            .browse(&scope_req(None, 10), &cx())
            .expect_err("malformed head");
        assert_eq!(kind_of(&err), ProviderErrorKind::TransientIo);
    }
}

#[test]
fn truncated_body_fails_mid_stream_not_silent() {
    let server = MockServer::start(Arc::new(|req| {
        if req.method == "PROPFIND" {
            return propfind_response("/a.wav", &[("/a.wav", &file_props(11, Some("\"v\"")))]);
        }
        // Content-Length 11 but only 5 bytes arrive before close.
        MockResponse::new(200)
            .header("etag", "\"v\"")
            .header("content-length", "11")
            .body(b"hello".to_vec())
    }));
    let p = provider(&server);
    let pin = p.pin(&aref(provider_id(), "/a.wav"), &cx()).expect("pin");
    let mut pr = p.open_pinned(&pin, &cx()).expect("open");
    let mut body = Vec::new();
    pr.reader
        .read_to_end(&mut body)
        .expect_err("truncated body must fail");
    assert_eq!(body, b"hello");
    let recorded = pr
        .completion
        .take()
        .expect("finished")
        .expect_err("completion must be Err");
    assert_eq!(recorded.kind(), ProviderErrorKind::TransientIo);
}

#[test]
fn malformed_chunked_body_fails_typed() {
    let raw = b"HTTP/1.1 200 OK\r\netag: \"v\"\r\ntransfer-encoding: chunked\r\n\r\nzz\r\nhello\r\n0\r\n\r\n".to_vec();
    let server = MockServer::start(Arc::new(move |req| {
        if req.method == "PROPFIND" {
            return propfind_response("/a.wav", &[("/a.wav", &file_props(11, Some("\"v\"")))]);
        }
        MockResponse::raw(raw.clone())
    }));
    let p = provider(&server);
    let pin = p.pin(&aref(provider_id(), "/a.wav"), &cx()).expect("pin");
    let mut pr = p.open_pinned(&pin, &cx()).expect("open");
    let mut body = Vec::new();
    pr.reader
        .read_to_end(&mut body)
        .expect_err("malformed chunk must fail");
    let recorded = pr
        .completion
        .take()
        .expect("finished")
        .expect_err("completion must be Err");
    assert_eq!(recorded.kind(), ProviderErrorKind::TransientIo);
}

#[test]
fn requests_declare_identity_encoding() {
    // Every request forbids content codings — a hostile or buggy
    // server cannot answer with gzip/brotli and turn a bounded read
    // into a compression bomb.
    let encodings = Arc::new(Mutex::new(Vec::<Option<String>>::new()));
    let encodings2 = encodings.clone();
    let server = MockServer::start(Arc::new(move |req| {
        encodings2
            .lock()
            .unwrap()
            .push(req.header("accept-encoding").map(str::to_owned));
        if req.method == "PROPFIND" {
            return propfind_response("/a.wav", &[("/a.wav", &file_props(11, Some("\"e\"")))]);
        }
        MockResponse::new(200)
            .header("etag", "\"e\"")
            .body(b"hello world".to_vec())
    }));
    let p = provider(&server);
    let pin = p.pin(&aref(provider_id(), "/a.wav"), &cx()).expect("pin");
    let mut pr = p.open_pinned(&pin, &cx()).expect("open");
    let mut body = Vec::new();
    pr.reader.read_to_end(&mut body).expect("read");
    let encodings = encodings.lock().unwrap();
    assert!(
        encodings.len() >= 2,
        "expected PROPFIND + GET, got {encodings:?}"
    );
    assert!(
        encodings.iter().all(|e| e.as_deref() == Some("identity")),
        "every request must declare identity encoding: {encodings:?}"
    );
}

#[test]
fn same_origin_redirect_keeps_credentials_in_origin() {
    let seen = Arc::new(Mutex::new(Vec::<(String, Option<String>)>::new()));
    let seen2 = seen.clone();
    let server = MockServer::start(Arc::new(move |req| {
        seen2.lock().unwrap().push((
            req.path.clone(),
            req.header("authorization").map(str::to_owned),
        ));
        if req.path == "/old" {
            return MockResponse::new(301).header("location", "/new");
        }
        propfind_response("/new", &[("/new", dir_props())])
    }));
    let url = WebDavUrl::parse(&server.url()).expect("url");
    let mut config = WebDavConfig::new(url);
    config.credential_key = Some(CredentialKey::new("test-key").expect("key"));
    let p = WebDavProvider::new(provider_id(), config).expect("provider");
    let mut cx = cx();
    cx.credentials = Some(Arc::new(TestResolver));
    p.browse(&scope_req(Some("/old"), 10), &cx).expect("browse");
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert!(
        seen.iter()
            .all(|(_, a)| a.as_deref() == Some("Basic dXNlcjpwYXNz")),
        "same-origin hops must keep Authorization: {seen:?}"
    );
}

#[test]
fn cross_origin_redirect_never_reaches_other_origin() {
    // A second mock stands in for the foreign origin: it must see
    // zero requests — the redirect is refused before any connect, so
    // neither bytes nor credentials can leak across origins.
    let b_hits = Arc::new(Mutex::new(0usize));
    let b_hits2 = b_hits.clone();
    let server_b = MockServer::start(Arc::new(move |_| {
        *b_hits2.lock().unwrap() += 1;
        propfind_response("/", &[("/", dir_props())])
    }));
    let b_url = server_b.url();
    let a = MockServer::start(Arc::new(move |_| {
        let location = format!("{b_url}/steal");
        MockResponse::new(302).header("location", &location)
    }));
    let url = WebDavUrl::parse(&a.url()).expect("url");
    let mut config = WebDavConfig::new(url);
    config.credential_key = Some(CredentialKey::new("test-key").expect("key"));
    let p = WebDavProvider::new(provider_id(), config).expect("provider");
    let mut cx = cx();
    cx.credentials = Some(Arc::new(TestResolver));
    let err = p
        .browse(&scope_req(None, 10), &cx)
        .expect_err("cross-origin redirect");
    assert_eq!(kind_of(&err), ProviderErrorKind::InvalidRequest);
    assert_eq!(*b_hits.lock().unwrap(), 0, "foreign origin was contacted");
}

// ------------------------------------------------------------------
// Fixture self-check: accepted-connection socket mode
// ------------------------------------------------------------------

/// Regression for synvelis#245: the listener runs nonblocking, and on
/// Windows `accept()` propagates that flag to the accepted socket —
/// POSIX does not. `handle_conn` assumed a blocking socket, so a
/// `read` that beat the client's first bytes failed `WouldBlock`,
/// dropped the connection, and surfaced as os error 10054/10053 on
/// the client. Holding the request past the 2 ms accept poll forces
/// that race window deterministically.
#[test]
fn fixture_serves_request_written_after_accept() {
    let server = MockServer::start(Arc::new(|_| MockResponse::new(204)));
    let mut stream =
        TcpStream::connect(server.url().trim_start_matches("http://")).expect("connect");
    // Bound the read: a wedged server must fail the test fast, never
    // hang the suite.
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("read timeout");
    std::thread::sleep(Duration::from_millis(150));
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: x\r\n\r\n")
        .expect("write");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).expect("read");
    let head = String::from_utf8_lossy(&buf);
    assert!(head.starts_with("HTTP/1.1 204"), "bad head: {head:?}");
}
