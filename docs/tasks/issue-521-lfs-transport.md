---
id: "TASK-521a1b2c3d4e5f6g7h8i9j0k1"
type: "task"
status: "completed"
title: "issue-521-lfs-transport — Git LFS remote fetch HTTPS/TLS transport"
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
    - "docs/plans/issue-521-lfs-transport.md"
  implemented-by:
    - "docs/impl/ir/issue-521-lfs-transport.md"
  depends-on:
    - "docs/plans/issue-521-lfs-transport.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/alib-04-lfs-remote-fetch-task.md"
    - "docs/test-specs/alib-04-lfs-remote-fetch.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/test-specs/alib-04-lfs-remote-fetch.md"
---

# Tasks: issue-521-lfs-transport

- [x] T1 `TlsTrust`/`TlsSession`/`session_for`（`src/tls.rs`）+
  `ProviderContext.tls` seam + `ProviderErrorKind::TlsVerification`
  （`#[non_exhaustive]` 化）+ `library_ui.rs` wire tag
- [x] T2 `webdav/http.rs` TLS transport: `HttpEndpoint.tls`/
  `new_tls`/`default_port`、`Wire` enum、`TlsConn`（guard 付き
  handshake・`complete_io` 1-pass read/write・`WouldBlock` 伝播）、
  TLS error 分類（`InvalidCertificate` → `TlsVerification`、他 →
  `TransientIo`）、body reader の `Wire` 化
- [x] T3 `git/lfs.rs`: `for_remote` が `Http | Https` を
  `RemoteHttp` に、`remote_parts` scheme 保持、`request` の
  scheme 別 endpoint 構築（https → `session_for` + `new_tls`）、
  href/redirect/download の https 受理（silent downgrade なし、
  credential は remote origin 一致時のみ）
- [x] T4 fixture: `TlsMaterial`（rcgen CA+leaf）+ `MockTlsServer`
  （rustls `StreamOwned`）、request I/O の generic 化
- [x] T5 `tests/lfs.rs`: https 契約書換 + TLS suite（trusted CA
  happy path・untrusted CA → `TlsVerification`・native roots
  reject・401/403・TLS redirect・cross-origin credential 非付与・
  handshake 中 deadline）
- [x] T6 alib-04 doc chain その場更新（spec/design/plan/task/
  test-spec/IR の https 記述を #521 slice へ整合）+ 本 slug の
  plan/task/IR/exp + `evidence/sv521/`
- [x] T7 canonical verification（fmt-check/check/lint/test/
  check-headless-deps/prek）+ review-protocol

## Verification

- [x] `cargo test -p synvelis-library --test lfs` exits 0（22 tests）
- [x] `cargo check --all-targets -p synvelis-library` exits 0
- [x] `mise run fmt-check` / `mise run lint synvelis-library` /
  `mise run test synvelis-library` / `mise run check-headless-deps`
  （結果は evidence ledger と PR 本文に記録）
- [x] `mise exec -- prek run --all-files` + `git diff --check`
