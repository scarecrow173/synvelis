//! ALIB-04 Git LFS remote-fetch tests (Synvelis #331, HTTPS slice
//! `scarecrow173/synvelis#521`).
//!
//! Covers the `http://` LFS batch API path end-to-end: a headless
//! `MockServer` (the shared `tests/common/webdav_fixture.rs`
//! `TcpListener` fixture) answers the batch `POST` and the download
//! `GET`, while git objects come from a pre-seeded local mirror so
//! no git network operation ever runs. `MockTlsServer` adds the
//! `https://` path: a rustls server with a fixture CA that
//! `TlsTrust::from_pem` pins. SSH, `git://`, real remotes, and real
//! HTTPS endpoints stay NotRun/Unsupported by design — see
//! `docs/test-specs/alib-04-lfs-remote-fetch.md`.

mod common;

use std::io::Read;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::git_fixture::{self, GitFixture};
use common::webdav_fixture::{MockRequest, MockResponse, MockServer, MockTlsServer};
use common::{aref, cx, pid};
use synvelis_library::{
    AssetProvider, CredentialKey, CredentialResolver, GitRemoteConfig, GitRemoteProvider,
    GitRemoteUrl, LfsBackendKind, OpaqueCredential, PinnedRead, ProviderContext, ProviderError,
    ProviderErrorKind, ProviderId, RetryAdvice, TlsTrust, VerificationPolicy,
};

const PROVIDER_RAW: u128 = 0x0331;

fn provider_id() -> ProviderId {
    pid(PROVIDER_RAW)
}

/// The batch API path for remote URL `…/repo.git` — endpoint
/// derivation appends `/info/lfs` to the `.git` path.
const BATCH_PATH: &str = "/repo.git/info/lfs/objects/batch";

/// Seeds `name` under `fix.tmp` as a full local `--mirror` clone of
/// the fixture remote. Every git operation the provider performs
/// (rev-parse/ls-tree/cat-file/check-attr) then stays offline; the
/// configured `http://` remote URL is used *only* by the LFS
/// backend, which is exactly the slice under test.
fn seed_mirror(fix: &GitFixture, name: &str) -> PathBuf {
    let dst = git_fixture::plain_path(&fix.tmp.join(name));
    let src = git_fixture::plain_path(&fix.remote);
    git_fixture::git(
        &fix.work,
        &[
            "clone",
            "--mirror",
            src.to_str().expect("utf8 remote path"),
            dst.to_str().expect("utf8 mirror path"),
        ],
    );
    dst
}

/// A provider whose configured remote is `server`'s `http://` URL
/// (`/repo.git`), with git objects served from the seeded mirror.
/// `reference` is `main` — never `HEAD` — so no `ls-remote` against
/// the HTTP URL can run.
fn http_provider(fix: &GitFixture, remote_url: &str, mirror: &str) -> GitRemoteProvider {
    let mirror_dir = seed_mirror(fix, mirror);
    let url = GitRemoteUrl::parse(remote_url).expect("fixture remote url");
    let mut config = GitRemoteConfig::new(url, mirror_dir);
    "main".clone_into(&mut config.reference);
    GitRemoteProvider::new(provider_id(), config)
}

fn provider_at(fix: &GitFixture, server: &MockServer, mirror: &str) -> GitRemoteProvider {
    http_provider(fix, &format!("{}/repo.git", server.url()), mirror)
}

/// A context trusting `pem` as the only TLS root — the fixture CA
/// for success cases, a foreign CA for verification failures.
fn tls_cx(pem: &[u8]) -> ProviderContext {
    let mut cx = cx();
    cx.tls = Some(TlsTrust::from_pem(pem).expect("fixture trust pem"));
    cx
}

fn pin_payload(provider: &GitRemoteProvider) -> synvelis_library::PinnedAssetRef {
    provider
        .pin(&aref(provider_id(), "main:payload.bin"), &cx())
        .expect("pin payload.bin")
}

fn read_all(pr: &mut PinnedRead) -> Result<Vec<u8>, ProviderError> {
    let mut buf = Vec::new();
    pr.reader
        .read_to_end(&mut buf)
        .map_err(|e| ProviderError::new(ProviderErrorKind::Internal, format!("read: {e}")))?;
    Ok(buf)
}

/// A batch handler that answers `POST BATCH_PATH` with a download
/// action pointing at `href_for(host)`, records every request line
/// as `"METHOD path"`, and serves `GET` bodies from `get_body`.
fn batch_handler(
    oid: &str,
    size: u64,
    href_for: impl Fn(&str) -> String + Send + Sync + 'static,
    get_body: impl Fn() -> MockResponse + Send + Sync + 'static,
    seen: Arc<Mutex<Vec<String>>>,
) -> Arc<dyn Fn(&MockRequest) -> MockResponse + Send + Sync> {
    let oid = oid.to_owned();
    Arc::new(move |req: &MockRequest| {
        seen.lock()
            .unwrap()
            .push(format!("{} {}", req.method, req.path));
        if req.method == "POST" && req.path == BATCH_PATH {
            let host = req.header("host").expect("host header").to_owned();
            let href = href_for(&host);
            let body = format!(
                "{{\"objects\":[{{\"oid\":\"{oid}\",\"size\":{size},\
                 \"actions\":{{\"download\":{{\"href\":\"{href}\"}}}}}}]}}"
            );
            return MockResponse::new(200)
                .header("content-type", "application/vnd.git-lfs+json")
                .body(body.into_bytes());
        }
        if req.method == "GET" {
            return get_body();
        }
        MockResponse::new(404)
    })
}

#[test]
fn remote_lfs_batch_download_fetches_and_verifies() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    let content = GitFixture::lfs_content();
    let size = content.len() as u64;
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let oid_href = oid.clone();
    let server = MockServer::start(batch_handler(
        &oid,
        size,
        move |host| format!("http://{host}/objects/{oid_href}?sig=test-token"),
        {
            let content = content.clone();
            move || MockResponse::new(200).body(content.clone())
        },
        seen.clone(),
    ));
    let provider = provider_at(&fix, &server, "mirror-http");
    assert_eq!(provider.lfs_backend_kind(), LfsBackendKind::RemoteHttp);

    let pin = pin_payload(&provider);
    assert!(pin.object_identity.as_str().starts_with("git-lfs:"));
    assert_eq!(pin.expected_size, Some(size));

    let mut pr = provider.open_pinned(&pin, &cx()).expect("open remote lfs");
    assert!(matches!(
        pr.claims.verification,
        VerificationPolicy::StreamVerified { .. }
    ));
    let bytes = read_all(&mut pr).expect("read remote lfs");
    assert_eq!(bytes, content, "fetched bytes are the real object");
    let receipt = pr.completion.take().expect("finished").expect("receipt");
    assert!(receipt.verified, "sha256 verified at EOF");
    assert_eq!(receipt.bytes_read, size);

    // The wire shape: one batch POST, then one download GET whose
    // request target kept the signed query verbatim.
    let hits = seen.lock().unwrap();
    assert_eq!(
        hits.len(),
        2,
        "expected batch POST + download GET: {hits:?}"
    );
    assert_eq!(hits[0], format!("POST {BATCH_PATH}"));
    assert!(
        hits[1].starts_with("GET /objects/") && hits[1].contains("sig=test-token"),
        "download GET must carry the signed query: {}",
        hits[1]
    );
}

#[test]
fn remote_lfs_batch_404_is_not_found() {
    let Some(fix) = GitFixture::new() else { return };
    let server = MockServer::start(Arc::new(|_req| MockResponse::new(404)));
    let provider = provider_at(&fix, &server, "mirror-http-404");
    let pin = pin_payload(&provider);
    let err = provider.open_pinned(&pin, &cx()).expect_err("batch 404");
    assert_eq!(err.kind(), ProviderErrorKind::NotFound);
}

#[test]
fn remote_lfs_object_error_404_is_not_found() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    // Batch 200 with a per-object error — the spec's missing-object
    // signal inside a successful batch response.
    let server = MockServer::start(Arc::new(move |req: &MockRequest| {
        if req.method == "POST" {
            return MockResponse::new(200)
                .header("content-type", "application/vnd.git-lfs+json")
                .body(
                    format!(
                        "{{\"objects\":[{{\"oid\":\"{oid}\",\"size\":4096,\
                         \"error\":{{\"code\":404,\"message\":\"object missing\"}}}}]}}"
                    )
                    .into_bytes(),
                );
        }
        MockResponse::new(404)
    }));
    let provider = provider_at(&fix, &server, "mirror-http-obj404");
    let pin = pin_payload(&provider);
    let err = provider
        .open_pinned(&pin, &cx())
        .expect_err("per-object 404");
    assert_eq!(err.kind(), ProviderErrorKind::NotFound);
}

#[test]
fn remote_lfs_download_size_mismatch_is_integrity() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    let size = GitFixture::lfs_content().len() as u64;
    // The GET body is shorter than the pointer-declared size; the
    // mock's Content-Length matches the wrong body, so the early
    // consistency check fires before streaming.
    let short = vec![0xABu8; usize::try_from(size - 1).expect("size fits usize")];
    let oid_href = oid.clone();
    let server = MockServer::start(batch_handler(
        &oid,
        size,
        move |host| format!("http://{host}/objects/{oid_href}"),
        move || MockResponse::new(200).body(short.clone()),
        Arc::new(Mutex::new(Vec::new())),
    ));
    let provider = provider_at(&fix, &server, "mirror-http-size");
    let pin = pin_payload(&provider);
    let err = provider
        .open_pinned(&pin, &cx())
        .expect_err("size mismatch");
    assert_eq!(err.kind(), ProviderErrorKind::IntegrityMismatch);
}

#[test]
fn remote_lfs_corrupt_bytes_fail_sha256_at_eof() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    let size = GitFixture::lfs_content().len() as u64;
    // Same length, wrong bytes: passes every size check, fails the
    // SHA-256 verifier at EOF — the completion slot records the
    // typed failure.
    let corrupt: Vec<u8> = (0..size)
        .map(|i| u8::try_from(i % 251).expect("mod bound fits u8"))
        .collect();
    let oid_href = oid.clone();
    let server = MockServer::start(batch_handler(
        &oid,
        size,
        move |host| format!("http://{host}/objects/{oid_href}"),
        move || MockResponse::new(200).body(corrupt.clone()),
        Arc::new(Mutex::new(Vec::new())),
    ));
    let provider = provider_at(&fix, &server, "mirror-http-corrupt");
    let pin = pin_payload(&provider);
    let mut pr = provider.open_pinned(&pin, &cx()).expect("open");
    assert!(read_all(&mut pr).is_err(), "corrupt bytes must fail");
    let outcome = pr.completion.take().expect("finished");
    match outcome {
        Err(e) => assert_eq!(e.kind(), ProviderErrorKind::IntegrityMismatch),
        Ok(r) => panic!("corrupt remote lfs fetch reported success: {r:?}"),
    }
}

#[test]
fn remote_lfs_batch_401_is_auth_expired() {
    let Some(fix) = GitFixture::new() else { return };
    let server = MockServer::start(Arc::new(|_req| MockResponse::new(401)));
    let provider = provider_at(&fix, &server, "mirror-http-401");
    let pin = pin_payload(&provider);
    let err = provider.open_pinned(&pin, &cx()).expect_err("401");
    assert_eq!(err.kind(), ProviderErrorKind::AuthExpired);
}

#[test]
fn remote_lfs_download_403_is_permission_denied() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    let size = GitFixture::lfs_content().len() as u64;
    // Batch succeeds; the download GET is forbidden.
    let oid_href = oid.clone();
    let server = MockServer::start(batch_handler(
        &oid,
        size,
        move |host| format!("http://{host}/objects/{oid_href}"),
        || MockResponse::new(403),
        Arc::new(Mutex::new(Vec::new())),
    ));
    let provider = provider_at(&fix, &server, "mirror-http-403");
    let pin = pin_payload(&provider);
    let err = provider.open_pinned(&pin, &cx()).expect_err("403");
    assert_eq!(err.kind(), ProviderErrorKind::PermissionDenied);
}

#[test]
fn remote_lfs_https_remote_uses_tls_transport() {
    let Some(fix) = GitFixture::new() else { return };
    // No server at all — the backend is `RemoteHttp` now; the fetch
    // fails as a typed transport error (`Offline` on connect
    // refusal), never `UnsupportedCapability` and never a silent
    // plaintext attempt.
    let provider = http_provider(&fix, "https://127.0.0.1:9/repo.git", "mirror-https");
    assert_eq!(provider.lfs_backend_kind(), LfsBackendKind::RemoteHttp);
    let pin = pin_payload(&provider);
    let err = provider
        .open_pinned(&pin, &cx())
        .expect_err("https dead port");
    assert_eq!(err.kind(), ProviderErrorKind::Offline);
}

#[test]
fn remote_lfs_https_batch_download_fetches_and_verifies() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    let content = GitFixture::lfs_content();
    let size = content.len() as u64;
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let oid_href = oid.clone();
    let server = MockTlsServer::start(batch_handler(
        &oid,
        size,
        move |host| format!("https://{host}/objects/{oid_href}?sig=test-token"),
        {
            let content = content.clone();
            move || MockResponse::new(200).body(content.clone())
        },
        seen.clone(),
    ));
    let provider = http_provider(
        &fix,
        &format!("{}/repo.git", server.url()),
        "mirror-https-ok",
    );
    assert_eq!(provider.lfs_backend_kind(), LfsBackendKind::RemoteHttp);

    let cx = tls_cx(&server.trust_pem());
    let pin = provider
        .pin(&aref(provider_id(), "main:payload.bin"), &cx)
        .expect("pin payload.bin");
    let mut pr = provider
        .open_pinned(&pin, &cx)
        .expect("open remote lfs https");
    let bytes = read_all(&mut pr).expect("read remote lfs https");
    assert_eq!(bytes, content, "fetched bytes are the real object");
    let receipt = pr.completion.take().expect("finished").expect("receipt");
    assert!(receipt.verified, "sha256 verified at EOF");

    // Same wire shape as plaintext: batch POST then download GET,
    // signed query verbatim — now inside TLS records.
    let hits = seen.lock().unwrap();
    assert_eq!(
        hits.len(),
        2,
        "expected batch POST + download GET: {hits:?}"
    );
    assert_eq!(hits[0], format!("POST {BATCH_PATH}"));
    assert!(
        hits[1].starts_with("GET /objects/") && hits[1].contains("sig=test-token"),
        "download GET must carry the signed query: {}",
        hits[1]
    );
}

#[test]
fn remote_lfs_https_protocol_relative_href_stays_tls() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    let content = GitFixture::lfs_content();
    let size = content.len() as u64;
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let oid_href = oid.clone();
    let server = MockTlsServer::start(batch_handler(
        &oid,
        size,
        // Protocol-relative href — must inherit the https remote's
        // scheme, never downgrade to plaintext.
        move |host| format!("//{host}/objects/{oid_href}"),
        {
            let content = content.clone();
            move || MockResponse::new(200).body(content.clone())
        },
        seen.clone(),
    ));
    let provider = http_provider(
        &fix,
        &format!("{}/repo.git", server.url()),
        "mirror-https-pr",
    );
    let cx = tls_cx(&server.trust_pem());
    let pin = provider
        .pin(&aref(provider_id(), "main:payload.bin"), &cx)
        .expect("pin");
    let mut pr = provider.open_pinned(&pin, &cx).expect("open");
    let bytes = read_all(&mut pr).expect("read");
    assert_eq!(bytes, content);
    assert_eq!(seen.lock().unwrap().len(), 2, "batch + download");
}

#[test]
fn remote_lfs_https_untrusted_ca_is_tls_verification() {
    let Some(fix) = GitFixture::new() else { return };
    let server = MockTlsServer::start(Arc::new(|_req| MockResponse::new(200)));
    let provider = http_provider(
        &fix,
        &format!("{}/repo.git", server.url()),
        "mirror-https-badca",
    );
    let pin = pin_payload(&provider);
    // Trust an unrelated CA: the batch POST handshake must fail
    // certificate verification before any request bytes are sent.
    let cx = tls_cx(&MockTlsServer::foreign_trust_pem());
    let err = provider.open_pinned(&pin, &cx).expect_err("untrusted ca");
    assert_eq!(err.kind(), ProviderErrorKind::TlsVerification);
    assert_eq!(
        err.retry(),
        RetryAdvice::Never,
        "trust failure is deterministic"
    );
    // The detail must not leak certificate material or the URL.
    assert!(!err.detail().contains("PRIVATE KEY"));
    assert!(!err.detail().contains(server.url().as_str()));
}

#[test]
fn remote_lfs_https_native_roots_reject_self_signed() {
    let Some(fix) = GitFixture::new() else { return };
    let server = MockTlsServer::start(Arc::new(|_req| MockResponse::new(200)));
    let provider = http_provider(
        &fix,
        &format!("{}/repo.git", server.url()),
        "mirror-https-native",
    );
    let pin = pin_payload(&provider);
    // `cx.tls = None` selects the platform native root store — the
    // fixture CA is not in it, so verification must fail typed.
    let err = provider.open_pinned(&pin, &cx()).expect_err("native roots");
    assert_eq!(err.kind(), ProviderErrorKind::TlsVerification);
}

#[test]
fn remote_lfs_https_batch_401_is_auth_expired() {
    let Some(fix) = GitFixture::new() else { return };
    let server = MockTlsServer::start(Arc::new(|_req| MockResponse::new(401)));
    let provider = http_provider(
        &fix,
        &format!("{}/repo.git", server.url()),
        "mirror-https-401",
    );
    let cx = tls_cx(&server.trust_pem());
    let pin = provider
        .pin(&aref(provider_id(), "main:payload.bin"), &cx)
        .expect("pin");
    let err = provider.open_pinned(&pin, &cx).expect_err("401 over tls");
    assert_eq!(err.kind(), ProviderErrorKind::AuthExpired);
}

#[test]
fn remote_lfs_https_download_403_is_permission_denied() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    let size = GitFixture::lfs_content().len() as u64;
    let oid_href = oid.clone();
    let server = MockTlsServer::start(batch_handler(
        &oid,
        size,
        move |host| format!("https://{host}/objects/{oid_href}"),
        || MockResponse::new(403),
        Arc::new(Mutex::new(Vec::new())),
    ));
    let provider = http_provider(
        &fix,
        &format!("{}/repo.git", server.url()),
        "mirror-https-403",
    );
    let cx = tls_cx(&server.trust_pem());
    let pin = provider
        .pin(&aref(provider_id(), "main:payload.bin"), &cx)
        .expect("pin");
    let err = provider.open_pinned(&pin, &cx).expect_err("403 over tls");
    assert_eq!(err.kind(), ProviderErrorKind::PermissionDenied);
}

#[test]
fn remote_lfs_https_redirect_is_followed_over_tls() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    let content = GitFixture::lfs_content();
    let size = content.len() as u64;
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen_hits = seen.clone();
    let oid_path = oid.clone();
    let oid_href = oid.clone();
    let body = content.clone();
    // The download href lands on `/redirect`, which answers 302 to a
    // same-origin https URL — the client must re-resolve and follow
    // it over TLS, credentials/origin rules unchanged.
    let server = MockTlsServer::start(Arc::new(move |req: &MockRequest| {
        seen_hits
            .lock()
            .unwrap()
            .push(format!("{} {}", req.method, req.path));
        if req.method == "POST" && req.path == BATCH_PATH {
            let host = req.header("host").expect("host header").to_owned();
            let href = format!("https://{host}/redirect?to=/objects/{oid_href}");
            return MockResponse::new(200)
                .header("content-type", "application/vnd.git-lfs+json")
                .body(
                    format!(
                        "{{\"objects\":[{{\"oid\":\"{oid_path}\",\"size\":{size},\
                         \"actions\":{{\"download\":{{\"href\":\"{href}\"}}}}}}]}}"
                    )
                    .into_bytes(),
                );
        }
        if req.method == "GET" && req.path.starts_with("/redirect") {
            let host = req.header("host").expect("host header").to_owned();
            return MockResponse::new(302).header(
                "location",
                &format!("https://{host}/objects/{oid_path}?sig=redirected"),
            );
        }
        if req.method == "GET" {
            return MockResponse::new(200).body(body.clone());
        }
        MockResponse::new(404)
    }));
    let provider = http_provider(
        &fix,
        &format!("{}/repo.git", server.url()),
        "mirror-https-redir",
    );
    let cx = tls_cx(&server.trust_pem());
    let pin = provider
        .pin(&aref(provider_id(), "main:payload.bin"), &cx)
        .expect("pin");
    let mut pr = provider.open_pinned(&pin, &cx).expect("open");
    let bytes = read_all(&mut pr).expect("read");
    assert_eq!(bytes, content);
    let hits = seen.lock().unwrap();
    assert!(
        hits.iter().any(|h| h.starts_with("GET /redirect")),
        "redirect hop must be requested: {hits:?}"
    );
    assert!(
        hits.iter()
            .any(|h| h.starts_with("GET /objects/") && h.contains("sig=redirected")),
        "redirect target must be fetched over tls: {hits:?}"
    );
}

#[test]
fn remote_lfs_https_protocol_relative_redirect_stays_tls() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    let content = GitFixture::lfs_content();
    let size = content.len() as u64;
    let oid_path = oid.clone();
    let oid_href = oid.clone();
    let body = content.clone();
    // `Location: //host/…` must inherit the https scheme of the
    // redirecting target — never downgrade to plaintext.
    let server = MockTlsServer::start(Arc::new(move |req: &MockRequest| {
        if req.method == "POST" && req.path == BATCH_PATH {
            let host = req.header("host").expect("host header").to_owned();
            let href = format!("https://{host}/redirect");
            return MockResponse::new(200)
                .header("content-type", "application/vnd.git-lfs+json")
                .body(
                    format!(
                        "{{\"objects\":[{{\"oid\":\"{oid_path}\",\"size\":{size},\
                         \"actions\":{{\"download\":{{\"href\":\"{href}\"}}}}}}]}}"
                    )
                    .into_bytes(),
                );
        }
        if req.method == "GET" && req.path.starts_with("/redirect") {
            let host = req.header("host").expect("host header").to_owned();
            return MockResponse::new(302)
                .header("location", &format!("//{host}/objects/{oid_href}"));
        }
        if req.method == "GET" {
            return MockResponse::new(200).body(body.clone());
        }
        MockResponse::new(404)
    }));
    let provider = http_provider(
        &fix,
        &format!("{}/repo.git", server.url()),
        "mirror-https-pr-redir",
    );
    let cx = tls_cx(&server.trust_pem());
    let pin = provider
        .pin(&aref(provider_id(), "main:payload.bin"), &cx)
        .expect("pin");
    let mut pr = provider.open_pinned(&pin, &cx).expect("open");
    let bytes = read_all(&mut pr).expect("read");
    assert_eq!(bytes, content);
}

#[test]
fn remote_lfs_https_credentials_stay_on_remote_origin() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    let content = GitFixture::lfs_content();
    let size = content.len() as u64;

    // Download href points at a *different* origin — a plaintext
    // `MockServer` on another port: the credential must reach the
    // https batch endpoint but never the signed URL host, and the
    // plaintext hop needs no TLS trust.
    let dl_auth = Arc::new(Mutex::new(Vec::<String>::new()));
    let dl_seen = dl_auth.clone();
    let downloader = MockServer::start(Arc::new(move |req: &MockRequest| {
        if let Some(v) = req.header("authorization") {
            dl_seen.lock().unwrap().push(v.to_owned());
        }
        MockResponse::new(200).body(content.clone())
    }));

    let batch_auth = Arc::new(Mutex::new(Vec::<String>::new()));
    let batch_seen = batch_auth.clone();
    let oid_href = oid.clone();
    let api = MockTlsServer::start(Arc::new(move |req: &MockRequest| {
        if let Some(v) = req.header("authorization") {
            batch_seen.lock().unwrap().push(v.to_owned());
        }
        if req.method == "POST" {
            return MockResponse::new(200)
                .header("content-type", "application/vnd.git-lfs+json")
                .body(
                    format!(
                        "{{\"objects\":[{{\"oid\":\"{oid_href}\",\"size\":{size},\
                         \"actions\":{{\"download\":{{\"href\":\"{}/objects/{oid_href}\"}}}}}}]}}",
                        downloader.url()
                    )
                    .into_bytes(),
                );
        }
        MockResponse::new(404)
    }));

    let url = GitRemoteUrl::parse(&format!("{}/repo.git", api.url())).expect("url");
    let mut config = GitRemoteConfig::new(url, seed_mirror(&fix, "mirror-https-auth"));
    "main".clone_into(&mut config.reference);
    config.credential_key = Some(CredentialKey::new("test-key").expect("key"));
    let provider = GitRemoteProvider::new(provider_id(), config);

    let mut cx = tls_cx(&api.trust_pem());
    cx.credentials = Some(Arc::new(TestResolver));
    let pin = provider
        .pin(&aref(provider_id(), "main:payload.bin"), &cx)
        .expect("pin");
    let mut pr = provider.open_pinned(&pin, &cx).expect("open");
    let bytes = read_all(&mut pr).expect("read");
    assert_eq!(bytes, GitFixture::lfs_content());

    assert_eq!(
        batch_auth.lock().unwrap().as_slice(),
        &["Basic dXNlcjpwYXNz".to_owned()],
        "credential must reach the https remote-origin batch endpoint"
    );
    assert!(
        dl_auth.lock().unwrap().is_empty(),
        "credential must never cross to the signed download origin"
    );
}

#[test]
fn remote_lfs_redirect_strips_sensitive_action_headers_cross_origin() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    let content = GitFixture::lfs_content();
    let size = content.len() as u64;

    // Third-party download target (plaintext is fine — the href
    // chain legitimately crosses origins) that records what it saw.
    let seen = Arc::new(Mutex::new(Vec::<(Option<String>, Option<String>)>::new()));
    let seen_dl = seen.clone();
    let dl_content = content.clone();
    let downloader = MockServer::start(Arc::new(move |req: &MockRequest| {
        seen_dl.lock().unwrap().push((
            req.header("authorization").map(str::to_owned),
            req.header("x-custom").map(str::to_owned),
        ));
        MockResponse::new(200).body(dl_content.clone())
    }));
    let dl_url = downloader.url();

    let oid_path = oid.clone();
    let api = MockTlsServer::start(Arc::new(move |req: &MockRequest| {
        if req.method == "POST" && req.path == BATCH_PATH {
            let host = req.header("host").expect("host header").to_owned();
            let href = format!("https://{host}/redirect");
            return MockResponse::new(200)
                .header("content-type", "application/vnd.git-lfs+json")
                .body(
                    format!(
                        "{{\"objects\":[{{\"oid\":\"{oid_path}\",\"size\":{size},\
                         \"actions\":{{\"download\":{{\"href\":\"{href}\",\
                         \"headers\":{{\"authorization\":\"Bearer cdn-tok\",\
                         \"x-custom\":\"keep-me\"}}}}}}}}]}}"
                    )
                    .into_bytes(),
                );
        }
        if req.method == "GET" && req.path.starts_with("/redirect") {
            return MockResponse::new(302).header("location", &format!("{dl_url}/dl"));
        }
        MockResponse::new(404)
    }));

    let provider = http_provider(
        &fix,
        &format!("{}/repo.git", api.url()),
        "mirror-redir-strip",
    );
    let cx = tls_cx(&api.trust_pem());
    let pin = provider
        .pin(&aref(provider_id(), "main:payload.bin"), &cx)
        .expect("pin");
    let mut pr = provider.open_pinned(&pin, &cx).expect("open");
    let bytes = read_all(&mut pr).expect("read");
    assert_eq!(bytes, content);

    let hits = seen.lock().unwrap();
    assert_eq!(hits.len(), 1, "one download GET at third party");
    let (authz, custom) = &hits[0];
    assert!(
        authz.is_none(),
        "sensitive action header must not cross origins: {authz:?}"
    );
    assert_eq!(
        custom.as_deref(),
        Some("keep-me"),
        "non-sensitive action header still forwarded"
    );
}

#[test]
fn remote_lfs_https_deadline_fails_during_tls_handshake() {
    let Some(fix) = GitFixture::new() else { return };
    // A bare TCP listener that accepts and never answers: the TLS
    // handshake stalls and the context deadline must land as
    // `BudgetExceeded` — the same cooperative guard as plaintext.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind silent");
    let port = listener.local_addr().expect("local addr").port();
    // Blocking accept; held sockets keep the TCP connection open
    // without ever answering the TLS ClientHello.
    std::thread::spawn(move || {
        let mut held = Vec::new();
        while let Ok((s, _)) = listener.accept() {
            held.push(s);
        }
    });

    let provider = http_provider(
        &fix,
        &format!("https://127.0.0.1:{port}/repo.git"),
        "mirror-https-dl",
    );
    let pin = pin_payload(&provider);
    let mut cx = cx();
    cx.limits.deadline = Some(Instant::now() + Duration::from_millis(300));
    let err = provider.open_pinned(&pin, &cx).expect_err("deadline");
    assert_eq!(err.kind(), ProviderErrorKind::BudgetExceeded);
}

#[test]
fn remote_lfs_malformed_batch_is_typed_not_panic() {
    let Some(fix) = GitFixture::new() else { return };
    let server = MockServer::start(Arc::new(|req: &MockRequest| {
        if req.method == "POST" {
            return MockResponse::new(200).body(b"not json".to_vec());
        }
        MockResponse::new(404)
    }));
    let provider = provider_at(&fix, &server, "mirror-http-badjson");
    let pin = pin_payload(&provider);
    let err = provider
        .open_pinned(&pin, &cx())
        .expect_err("malformed batch");
    assert_eq!(err.kind(), ProviderErrorKind::InvalidRequest);
}

struct TestResolver;

impl CredentialResolver for TestResolver {
    fn resolve(&self, key: &CredentialKey) -> Result<OpaqueCredential, ProviderError> {
        Ok(OpaqueCredential::new(key.clone(), "Basic dXNlcjpwYXNz"))
    }
}

#[test]
fn remote_lfs_credentials_stay_on_remote_origin() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    let content = GitFixture::lfs_content();
    let size = content.len() as u64;

    // The download href points at a *different* origin (second
    // listener, different port): the configured credential must go
    // to the batch endpoint but never to the signed URL host.
    let dl_auth = Arc::new(Mutex::new(Vec::<String>::new()));
    let dl_seen = dl_auth.clone();
    let downloader = MockServer::start(Arc::new(move |req: &MockRequest| {
        if let Some(v) = req.header("authorization") {
            dl_seen.lock().unwrap().push(v.to_owned());
        }
        MockResponse::new(200).body(content.clone())
    }));

    let batch_auth = Arc::new(Mutex::new(Vec::<String>::new()));
    let batch_seen = batch_auth.clone();
    let api = MockServer::start(Arc::new(move |req: &MockRequest| {
        if let Some(v) = req.header("authorization") {
            batch_seen.lock().unwrap().push(v.to_owned());
        }
        if req.method == "POST" {
            return MockResponse::new(200)
                .header("content-type", "application/vnd.git-lfs+json")
                .body(
                    format!(
                        "{{\"objects\":[{{\"oid\":\"{oid}\",\"size\":{size},\
                         \"actions\":{{\"download\":{{\"href\":\"{}/objects/{oid}\"}}}}}}]}}",
                        downloader.url()
                    )
                    .into_bytes(),
                );
        }
        MockResponse::new(404)
    }));

    // Attach the credential to the provider config — the remote is
    // the batch API server; the mirror is seeded once, locally.
    let url = GitRemoteUrl::parse(&format!("{}/repo.git", api.url())).expect("url");
    let mut config = GitRemoteConfig::new(url, seed_mirror(&fix, "mirror-http-auth"));
    "main".clone_into(&mut config.reference);
    config.credential_key = Some(CredentialKey::new("test-key").expect("key"));
    let provider = GitRemoteProvider::new(provider_id(), config);

    let mut cx = cx();
    cx.credentials = Some(Arc::new(TestResolver));
    let pin = provider
        .pin(&aref(provider_id(), "main:payload.bin"), &cx)
        .expect("pin");
    let mut pr = provider.open_pinned(&pin, &cx).expect("open");
    let bytes = read_all(&mut pr).expect("read");
    assert_eq!(bytes, GitFixture::lfs_content());

    assert_eq!(
        batch_auth.lock().unwrap().as_slice(),
        &["Basic dXNlcjpwYXNz".to_owned()],
        "credential must reach the remote-origin batch endpoint"
    );
    assert!(
        dl_auth.lock().unwrap().is_empty(),
        "credential must never cross to the signed download origin"
    );
}

/// A resolver that hands out a sentinel secret — LIB-SEC-04 for the
/// remote-LFS transports: the value rides the wire as `Authorization`
/// but a typed failure must never echo it in the error's Display,
/// detail, or Debug rendering.
struct SecretResolver;

impl CredentialResolver for SecretResolver {
    fn resolve(&self, key: &CredentialKey) -> Result<OpaqueCredential, ProviderError> {
        Ok(OpaqueCredential::new(key.clone(), LFS_SENTINEL))
    }
}

const LFS_SENTINEL: &str = "Bearer SECRET-LFS-7f3a";

/// Every rendered surface a log could print — Display, the `detail`
/// accessor, and derived `Debug`.
fn assert_secret_absent(err: &ProviderError, secret: &str) {
    let rendered = format!("{err} | {} | {err:?}", err.detail());
    assert!(
        !rendered.contains(secret),
        "secret material leaked into error: {rendered}"
    );
}

#[test]
fn remote_lfs_credentials_never_echo_in_typed_errors() {
    let Some(fix) = GitFixture::new() else { return };

    // Plain http: the batch endpoint rejects the credential — the
    // typed `AuthExpired` must carry no secret material.
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen_req = seen.clone();
    let server = MockServer::start(Arc::new(move |req: &MockRequest| {
        if let Some(v) = req.header("authorization") {
            seen_req.lock().unwrap().push(v.to_owned());
        }
        MockResponse::new(401)
    }));
    let url = GitRemoteUrl::parse(&format!("{}/repo.git", server.url())).expect("url");
    let mut config = GitRemoteConfig::new(url, seed_mirror(&fix, "mirror-secret-http"));
    "main".clone_into(&mut config.reference);
    config.credential_key = Some(CredentialKey::new("test-key").expect("key"));
    let provider = GitRemoteProvider::new(provider_id(), config);
    let mut cx = cx();
    cx.credentials = Some(Arc::new(SecretResolver));
    let pin = provider
        .pin(&aref(provider_id(), "main:payload.bin"), &cx)
        .expect("pin");
    let err = provider.open_pinned(&pin, &cx).expect_err("401");
    assert_eq!(err.kind(), ProviderErrorKind::AuthExpired);
    // Honest precondition: the sentinel really reached the wire —
    // otherwise "not echoed" is vacuous.
    assert_eq!(seen.lock().unwrap().as_slice(), &[LFS_SENTINEL.to_owned()]);
    assert_secret_absent(&err, "SECRET-LFS-7f3a");

    // The same contract over TLS — a batch 401 inside records.
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen_req = seen.clone();
    let server = MockTlsServer::start(Arc::new(move |req: &MockRequest| {
        if let Some(v) = req.header("authorization") {
            seen_req.lock().unwrap().push(v.to_owned());
        }
        MockResponse::new(401)
    }));
    let url = GitRemoteUrl::parse(&format!("{}/repo.git", server.url())).expect("url");
    let mut config = GitRemoteConfig::new(url, seed_mirror(&fix, "mirror-secret-https"));
    "main".clone_into(&mut config.reference);
    config.credential_key = Some(CredentialKey::new("test-key").expect("key"));
    let provider = GitRemoteProvider::new(provider_id(), config);
    let mut cx = tls_cx(&server.trust_pem());
    cx.credentials = Some(Arc::new(SecretResolver));
    let pin = provider
        .pin(&aref(provider_id(), "main:payload.bin"), &cx)
        .expect("pin");
    let err = provider.open_pinned(&pin, &cx).expect_err("401 over tls");
    assert_eq!(err.kind(), ProviderErrorKind::AuthExpired);
    assert_eq!(seen.lock().unwrap().as_slice(), &[LFS_SENTINEL.to_owned()]);
    assert_secret_absent(&err, "SECRET-LFS-7f3a");
}

#[test]
fn remote_lfs_signed_url_tokens_never_echo_in_errors() {
    let Some(fix) = GitFixture::new() else { return };
    let oid = GitFixture::lfs_oid();
    let size = GitFixture::lfs_content().len() as u64;
    let oid_href = oid.clone();
    // The batch action's signed href carries a bearer query token;
    // the forbidden download must surface `PermissionDenied` typed —
    // the token rides the wire query but never echoes in the error
    // or the pin.
    let server = MockServer::start(batch_handler(
        &oid,
        size,
        move |host| format!("http://{host}/objects/{oid_href}?sig=SIGNED-TOKEN-q1w2"),
        || MockResponse::new(403),
        Arc::new(Mutex::new(Vec::new())),
    ));
    let provider = provider_at(&fix, &server, "mirror-http-sig");
    let pin = pin_payload(&provider);
    let err = provider.open_pinned(&pin, &cx()).expect_err("403");
    assert_eq!(err.kind(), ProviderErrorKind::PermissionDenied);
    assert_secret_absent(&err, "SIGNED-TOKEN-q1w2");
    assert!(
        !format!("{pin:?}").contains("SIGNED-TOKEN-q1w2"),
        "signed url material leaked into pin: {pin:?}"
    );
}

#[test]
fn file_remote_lfs_still_reads_local_objects() {
    let Some(fix) = GitFixture::new() else { return };
    // Regression: the pre-existing file:// path is untouched — the
    // provider clones the file remote itself and reads
    // `<remote>/lfs/objects` directly.
    let url = GitRemoteUrl::parse(&fix.remote_url).expect("fixture remote url");
    let provider = GitRemoteProvider::new(
        provider_id(),
        GitRemoteConfig::new(url, fix.tmp.join("mirror-file")),
    );
    assert_eq!(provider.lfs_backend_kind(), LfsBackendKind::LocalObjects);

    let pin = provider
        .pin(&aref(provider_id(), "HEAD:payload.bin"), &cx())
        .expect("pin");
    let mut pr = provider.open_pinned(&pin, &cx()).expect("open file lfs");
    let bytes = read_all(&mut pr).expect("read file lfs");
    assert_eq!(bytes, GitFixture::lfs_content());
    let receipt = pr.completion.take().expect("finished").expect("receipt");
    assert!(receipt.verified);
}
