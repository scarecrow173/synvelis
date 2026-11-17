---
id: "TESTSPEC-alib04-lfs-remote-fetch"
type: "test-spec"
status: "approved"
title: "ALIB-04 LFS Remote Fetch — テスト仕様"
created: "2026-11-10"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/331"
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
  implemented-by:
    - "docs/impl/ir/alib-04-lfs-remote-fetch.md"
  depends-on:
    - "docs/test-specs/alib-04-git-remote-provider.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/alib-06-webdav-provider.md"
    - "docs/test-specs/alib-07-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/tasks/alib-04-lfs-remote-fetch-task.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ALIB-04 LFS Remote Fetch — テスト仕様

## Feature: remote LFS object fetch (http/https)

### Rule: batch → download → verified stream

`http://` remote の `git-lfs:` pin を open すると、
`POST <endpoint>/objects/batch`（`operation=download`、
`objects=[{oid,size}]`、LFS JSON media type）→
`actions.download.href` への `GET`（署名 query verbatim、
action headers 付与）→ 返却 stream が pointer の oid
（=SHA-256）と declared size を `VerifyingReader` で満たす
実バイト列を返す。pointer テキストは決して asset bytes に
ならない。

### Rule: typed errors

- batch endpoint 404 → `NotFound`
- batch 200 + per-object `error.code=404` → `NotFound`
- download GET 401 → `AuthExpired`、403 → `PermissionDenied`
- download body の size 不一致 → `IntegrityMismatch`
- 同 length の corrupt bytes → EOF で `IntegrityMismatch`
  （completion slot に typed failure）
- batch JSON 不正 → `InvalidRequest`（panic しない）
- `https://` remote → `LfsBackendKind::RemoteHttp`（#521
  slice）。TLS 証明書検証失敗（未知 CA・native roots 不信任）
  → `TlsVerification`（`RetryAdvice::Never`）。認証失敗は
  plaintext と同じ `AuthExpired`/`PermissionDenied`、接続拒否は
  `Offline`、TLS handshake 中の deadline は `BudgetExceeded`

### Rule: credential boundary

`credential_key` 設定時、`Authorization` は remote origin
（batch endpoint）にのみ付き、別 origin の署名付き download
href には付かない。action-supplied `authorization` は保持。

### Rule: file:// regression

`file://` remote は `LocalObjects` のまま local
`lfs/objects` を読み、SHA-256/size 検証付きで fetch できる。

## Fixture

- `common::webdav_fixture::MockServer`（in-process
  `TcpListener`、1 request/connection）を再利用。handler は
  `Host` header から href を組み立てる。
- `common::webdav_fixture::MockTlsServer`（#521 slice）— 同一
  accept loop に rustls `ServerConnection` を被せた HTTPS 版。
  `TlsMaterial` が fixture CA + `127.0.0.1`/`localhost` leaf を
  rcgen で生成し、`trust_pem()` が `TlsTrust::from_pem` の入力、
  `foreign_trust_pem()` が検証失敗経路の入力。
- git objects は `git clone --mirror` で seed した local mirror
  から供給し、git network 操作は一切走らせない
  （`reference="main"` で `ls-remote` を回避）。
- 実 remote・SSH・`git://` は **NotRun/Unsupported** —
  外部 gate であり本 test-spec では Pass を主張しない。
  HTTPS は MockTlsServer fixture で Pass を主張する
  （実 HTTPS endpoint・実 remote は NotRun）。

## Examples

- `remote_lfs_batch_download_fetches_and_verifies` — happy path
  + request trace（POST batch→GET signed query）+ receipt.verified
- `remote_lfs_batch_404_is_not_found` / `_object_error_404_…`
- `remote_lfs_download_size_mismatch_is_integrity` /
  `…_corrupt_bytes_fail_sha256_at_eof`
- `remote_lfs_batch_401_is_auth_expired` /
  `remote_lfs_download_403_is_permission_denied`
- `remote_lfs_https_remote_uses_tls_transport` /
  `remote_lfs_malformed_batch_is_typed_not_panic`
- `remote_lfs_credentials_stay_on_remote_origin` /
  `remote_lfs_https_credentials_stay_on_remote_origin`
- `file_remote_lfs_still_reads_local_objects`
- #521 slice 追加: `remote_lfs_https_batch_download_fetches_
  and_verifies` / `…_protocol_relative_href_stays_tls` /
  `…_protocol_relative_redirect_stays_tls` /
  `…_untrusted_ca_is_tls_verification` /
  `…_native_roots_reject_self_signed` / `…_batch_401_is_auth_
  expired` / `…_download_403_is_permission_denied` /
  `…_redirect_is_followed_over_tls` /
  `…_deadline_fails_during_tls_handshake`
