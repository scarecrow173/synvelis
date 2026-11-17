---
id: "IMPL-521a1b2c3d4e5f6g7h8i9j0k3"
type: "impl"
status: "completed"
title: "issue-521-lfs-transport — Git LFS remote fetch HTTPS/TLS transport 実装記録"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source: ["https://github.com/scarecrow173/synvelis/issues/521"]
  derives-from:
    - "docs/plans/issue-521-lfs-transport.md"
    - "docs/tasks/issue-521-lfs-transport.md"
  derived-by: ["docs/impl/exp/issue-521-lfs-transport.md"]
  related:
    - "docs/impl/ir/alib-04-lfs-remote-fetch.md"
    - "docs/specs/alib-04-lfs-remote-fetch.md"
    - "docs/designs/alib-04-lfs-remote-fetch-design.md"
    - "docs/test-specs/alib-04-lfs-remote-fetch.md"
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/issue-521-lfs-transport.md"
    rejected: []
---

# IR: issue-521-lfs-transport

## 実装内容

ALIB-04 で `UnsupportedCapability` として残っていた `https://`
Git remote の LFS fetch を、検証必須の TLS transport として
実装した slice（`scarecrow173/synvelis#521`）。silent downgrade
なし、credential は remote origin 一致時のみ、検証の権威は従来
どおり `VerifyingReader`（SHA-256=oid + declared size）。

- `src/tls.rs`（新規）: `TlsTrust`（`native()` =
  `rustls-native-certs` で platform root を process 内 1 回
  lazy 構築、0 件は typed error。`from_pem()` = 明示 CA PEM、
  malformed/usable cert 0 件は typed error）、`TlsSession`
  （server name + `Arc<ClientConfig>`、安全な `Debug` のみ）、
  `session_for(&str, &TlsTrust)`（IP literal →
  `ServerName::IpAddress`、その他 DNS name）、ring provider
  固定の `ClientConfig`（TLS 1.2/1.3、webpki verifier、client
  auth なし、検証無効化経路なし）。
- `src/context.rs`: `ProviderContext.tls: Option<TlsTrust>`
  追加（`None` = native roots、per-call seam）。
- `src/error.rs`: `ProviderErrorKind` を `#[non_exhaustive]` 化し
  `TlsVerification` variant 追加（`RetryAdvice::Never`）。
- `src/lib.rs`: `mod tls` + `pub use TlsTrust`。
- `src/webdav/http.rs`: `HttpEndpoint` に
  `tls: Option<TlsSession>` 追加、`new_tls`/`default_port`
  （https=443/http=80）。`Wire` enum（`Plain(TcpStream)` /
  `Tls(TlsConn)`）で request write・head read・body read を
  共通化。`TlsConn` は rustls `ClientConnection` を自前で駆動
  — handshake は `complete_io` を `IO_POLL` 毎に guard
  （`check`/`check_stall`/`note_progress`）しつつ完了、
  read/write は 1 pass `complete_io` + `reader().read` で
  `WouldBlock` を guard へ伝播（`rustls::Stream` の内部 loop
  は guard bypass のため不採用 — exp 記録）。TLS 分類:
  `InvalidCertificate` → `TlsVerification`、他 → `TransientIo`。
  `send()`（WebDAV 向け）は `https` を従来どおり
  `UnsupportedCapability` で拒否 — TLS は `send_endpoint` の
  opt-in caller のみ。
- `src/git/lfs.rs`: `for_remote` が `GitRemoteScheme::Http |
  Https` を `RemoteHttp` に。`remote_parts` が scheme を保持し、
  `request` が `https` endpoint に `session_for(cx.tls)` +
  `HttpEndpoint::new_tls` を構築。download href・redirect の
  https 受理（scheme は target が運び暗黙変換なし）、
  credential 付与は remote origin 一致時のみ不変。
  protocol-relative（`//host/…`）は href で remote の
  scheme、redirect で現在 target の scheme を継承するよう
  修正 — https 有効化で新たに到達可能になった経路での
  silent HTTPS→HTTP downgrade を塞ぐ（review で検出、
  `remote_lfs_https_protocol_relative_href_stays_tls` /
  `…_redirect_stays_tls` で回帰固定）。同 review で
  `remote_parts` の scheme 大文字小文字正規化、
  `TlsConn::write` の非 timeout `complete_io` エラー即時
  伝播、`HttpEndpoint::from(&WebDavUrl)` の scheme-drop
  footgun 除去も実施。
- `crates/synvelis-workspace/src/library_ui.rs`:
  `TlsVerification` → `"tls-verification"` arm + 非網羅 match
  への wildcard（Coordinator 承認の範囲拡張、option A）。
- `tests/common/webdav_fixture.rs`: `TlsMaterial`（rcgen CA +
  `127.0.0.1`/`localhost` leaf）+ `MockTlsServer`
  （rustls `StreamOwned` accept loop）。request I/O を
  `Read + Write` generic 化し plaintext/TLS で共有。
- `tests/lfs.rs`: https 契約を TLS suite に書換 — trusted CA
  happy path（batch POST + signed GET + verify）、untrusted
  CA → `TlsVerification`、native roots が self-signed を
  reject、batch 401 → `AuthExpired`、download 403 →
  `PermissionDenied`、TLS redirect follow、cross-origin
  credential 非付与、handshake 中 deadline → `BudgetExceeded`。
  plaintext HTTP・file://・SSH/git unsupported は非回帰。
- `Cargo.toml`/`Cargo.lock`: `rustls 0.23`（default-features
  off、`std`/`tls12`/`ring`）+ `rustls-native-certs 0.8`、
  dev-dep `rcgen 0.14`。lock は再生成物（rcgen optional dep
  closure を含む、直接編集なし）。
- docs: alib-04 spec/design/plan/task/test-spec/IR を #521
  slice に整合する形でその場更新 + 本 slug の
  plan/task/IR/exp + `evidence/sv521/`。

## 検証

- `cargo check --all-targets -p synvelis-library`: Pass
  （`--locked` は依存追加のため lock 再生成後に Pass）。
- `cargo test -p synvelis-library --test lfs`: 19/19 Pass
  （in-process `MockServer`/`MockTlsServer`、実 network 不使用）。
- canonical `mise run` 検証（fmt-check/check/lint/test/
  check-headless-deps/prek）: 結果は `evidence/sv521/` ledger と
  PR 本文に記録。
- NotRun/Unsupported（正直記録）: 実 HTTPS endpoint・実 remote
  （GitHub LFS 等）・SSH/`git://`・実 OS credential store・GUI・
  `*-full`/`test-rt`/`check-app-*` lane（Daily Full CI・別 lane
  委譲）。WebDAV-over-HTTPS は本 slice 対象外（#90）。

## Risks

- **guard bypass**: `rustls::Stream` の `prepare_read` 内部
  loop が `IoGuard` を bypass し得るため不採用。`TlsConn` は
  `complete_io` 1 pass ごとに guard を再確認し、`WouldBlock`
  を呼出側 poll へ返す。handshake 中も `note_progress` で
  stall 検知が誤発火しないよう feed 済み。
- **error detail の secret 漏洩**: TLS 失敗の detail は
  `ProviderError` の既存整形に従い、cert bytes・peer data・
  credential・署名 URL query を含めない。
- **`ProviderErrorKind` 追加の波及**: `#[non_exhaustive]` 化で
  downstream exhaustive match を compile error にせず、
  `library_ui.rs`（唯一の同一 workspace exhaustive consumer、
  Coordinator 承認済み）に arm + wildcard を追加。他 consumer
  は wildcard/default のみで非回帰。
- **native roots の環境依存**: root 0 件は typed error で
  fail-closed。fixture は `from_pem` で決定的、native 経路は
  `native_roots_reject_self_signed` で実測。

## Follow-ups

- 実 HTTPS endpoint・実 remote（GitHub LFS 等）での動作確認:
  外部 gate、NotRun（Issue #521 / #331 系の外部検証で扱う）。
- WebDAV-over-HTTPS: #90 の責務として残存。
- SSH/`git://` LFS transport・`.lfsconfig`/`lfs.url` override・
  custom transfer adapter: 本 slice 対象外（alib-04 設計書の
  非対象節を維持）。
