---
id: "TESTSPEC-alib04-lfs-remote-fetch"
type: "test-spec"
status: "approved"
title: "ALIB-04 LFS Remote Fetch — テスト仕様"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/331"
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

## Feature: remote LFS object fetch (http)

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
- `https://` remote → `LfsBackendKind::Unsupported` かつ
  open は `UnsupportedCapability`（socket を開かない）

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
- git objects は `git clone --mirror` で seed した local mirror
  から供給し、git network 操作は一切走らせない
  （`reference="main"` で `ls-remote` を回避）。
- 実 remote・HTTPS・SSH・TLS は **NotRun/Unsupported** —
  外部 gate であり本 test-spec では Pass を主張しない。

## Examples

- `remote_lfs_batch_download_fetches_and_verifies` — happy path
  + request trace（POST batch→GET signed query）+ receipt.verified
- `remote_lfs_batch_404_is_not_found` / `_object_error_404_…`
- `remote_lfs_download_size_mismatch_is_integrity` /
  `…_corrupt_bytes_fail_sha256_at_eof`
- `remote_lfs_batch_401_is_auth_expired` /
  `remote_lfs_download_403_is_permission_denied`
- `remote_lfs_https_remote_is_unsupported` /
  `remote_lfs_malformed_batch_is_typed_not_panic`
- `remote_lfs_credentials_stay_on_remote_origin`
- `file_remote_lfs_still_reads_local_objects`
