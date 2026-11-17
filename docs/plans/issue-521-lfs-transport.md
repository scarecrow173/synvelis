---
id: "PLAN-issue521-lfs-transport"
type: "plan"
status: "approved"
title: "issue-521-lfs-transport — Git LFS remote fetch HTTPS/TLS transport 実装 plan"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/521"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-04-lfs-remote-fetch.md"
    - "docs/designs/alib-04-lfs-remote-fetch-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/alib-04-lfs-remote-fetch.md"
    - "docs/designs/alib-04-lfs-remote-fetch-design.md"
    - "docs/test-specs/alib-04-lfs-remote-fetch.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/alib-04-lfs-remote-fetch-plan.md"
    - "docs/tasks/alib-04-lfs-remote-fetch-task.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/alib-04-lfs-remote-fetch-design.md"
  derived-by:
    - "docs/tasks/issue-521-lfs-transport.md"
    - "docs/test-specs/alib-04-lfs-remote-fetch.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-521-lfs-transport — plan

## file map

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-library/Cargo.toml` | `rustls 0.23`（default-features=false, `std`/`tls12`/`ring`）・`rustls-native-certs 0.8` 追加、dev-dep `rcgen 0.14` |
| `Cargo.lock` | 依存追加に伴う再生成物（rcgen の optional dep closure を含む。直接編集なし） |
| `crates/synvelis-library/src/tls.rs` | 新規 — `TlsTrust`（native/`from_pem`）、`TlsSession`、`session_for`、ring provider 固定 `ClientConfig` |
| `crates/synvelis-library/src/context.rs` | `ProviderContext.tls: Option<TlsTrust>` 追加（`None` = native roots） |
| `crates/synvelis-library/src/error.rs` | `ProviderErrorKind` を `#[non_exhaustive]` 化 + `TlsVerification` variant（`RetryAdvice::Never`） |
| `crates/synvelis-library/src/lib.rs` | `mod tls` + `pub use TlsTrust` |
| `crates/synvelis-library/src/webdav/http.rs` | `HttpEndpoint.tls`/`new_tls`/`default_port`、`Wire` enum（Plain/Tls）、`TlsConn`（guard 付き handshake・1-pass `complete_io` read/write）、`tls_error` 分類 |
| `crates/synvelis-library/src/git/lfs.rs` | `for_remote` が `Http \| Https` を `RemoteHttp` に、`remote_parts` が scheme 保持、`request` が scheme 別 endpoint 構築（https → `session_for` + `new_tls`） |
| `crates/synvelis-workspace/src/library_ui.rs` | `TlsVerification` → `"tls-verification"` arm + `#[non_exhaustive]` 化への wildcard fallback（Coordinator 承認の範囲拡張） |
| `crates/synvelis-library/tests/common/webdav_fixture.rs` | `TlsMaterial`（rcgen CA+leaf）・`MockTlsServer`（rustls `StreamOwned` accept loop）追加、`read_request`/`write_response` を generic 化 |
| `crates/synvelis-library/tests/lfs.rs` | https 契約書換 + TLS suite 8 test 追加 |
| docs | alib-04 chain その場更新 + 本 slug の plan/task/IR/exp + `evidence/sv521/` |

## dependency graph

```
ProviderContext.tls ──→ lfs::request ──→ tls::session_for ──→ HttpEndpoint::new_tls
                                                            └─→ http::send_endpoint
                                                                  └─→ connect → TlsConn::handshake (guard)
                                                                        └─→ Wire::Tls → write_request/read_head/open_body
```

`WebDavUrl::parse`/`GitRemoteUrl`/`download_target`/`resolve_location` は
https を既に受理していたため変更不要 — 変更は transport 層と
LFS の scheme→backend 写像に限定。

## risk register

| リスク | 対応 |
| --- | --- |
| silent HTTP downgrade | `HttpEndpoint.tls` が scheme を運ぶ。`tls: Some` は必ず handshake、port から scheme を推測しない |
| `rustls::Stream` の `prepare_read` が guard を bypass | `TlsConn` は `complete_io` 1 pass + `reader().read` の独自実装。`WouldBlock` を呼出側 guard へ伝播 |
| TLS handshake 中の stall/cancel/deadline 不観測 | handshake loop が `guard.check()`/`check_stall()`/`note_progress` を各 `complete_io` 後に実行 |
| 証明書検証失敗の分類不明瞭 | `rustls::Error::InvalidCertificate` → `TlsVerification`（Never）。他 TLS エラー → `TransientIo`。detail に cert bytes/URL secret を出さない |
| `ProviderErrorKind` 追加で exhaustive match 破壊 | enum を `#[non_exhaustive]` 化 + `library_ui.rs`（Coordinator 承認済み範囲）に arm + wildcard。他 consumer は wildcard/default のみ |
| native roots が環境依存 | `TlsTrust::native` は process 内 1 回 lazy、root 0 件は `Internal`。fixture は `from_pem` で決定的 |
| IPv6/IP literal の server name 誤り | `session_for` が `IpAddr` parse → `ServerName::IpAddress`、それ以外 DNS name |
| lockfile 競合（sv513 未マージ差分） | `Cargo.lock` は再生成物として本 PR に含め、PR 本文に記録 |
| docs index 未同期 | 意図的 — Coordinator が統合時に再生成（PR 本文に明記） |

## verification matrix

| 検証 | コマンド |
| --- | --- |
| fmt | `mise run fmt-check` |
| package check/lint/test | `mise run check synvelis-library` / `lint synvelis-library` / `test synvelis-library` |
| headless deps | `mise run check-headless-deps` |
| hooks | `mise exec -- prek run --all-files` |
| MSRV | `mise run test-msrv`（実施可能なら。重い場合は lane 委譲を PR 本文に記録） |

NotRun（正直記録）: 実 HTTPS endpoint・実 remote（GitHub LFS 等）・
SSH/`git://`・OS credential store・GUI・`*-full`/`test-rt`/
`check-app-*`/`gw10` lane（Daily Full CI・別 lane 委譲）。

## rollback

`lfs.rs` の `for_remote` から `Https` を外し `request` の
`WebDavScheme::Https` arm を `UnsupportedCapability` に戻せば
LFS 側は完全退避可能。transport の `tls` field は `None` で
従来挙動と同一のため、WebDAV provider への影響はない。
