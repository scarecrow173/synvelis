---
id: "DSN-alib07-fault-matrix"
type: "design"
status: "approved"
title: "ALIB-07 — Asset Library 全経路 Fault Matrix 設計（synvelis-library headless slice）"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/alib-07-fault-matrix.md"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-library/tests/common/webdav_fixture.rs"
      - type: "file"
        path: "docs/specs/alib-07-fault-matrix.md"
      - type: "file"
        path: "docs/designs/alib-07-fault-matrix-design.md"
      - type: "file"
        path: "docs/plans/alib-07-fault-matrix.md"
      - type: "file"
        path: "docs/tasks/alib-07-fault-matrix.md"
      - type: "file"
        path: "docs/test-specs/alib-07-fault-matrix.md"
      - type: "file"
        path: "docs/impl/ir/alib-07-fault-matrix.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/git.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/import.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/webdav.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-07-fault-matrix.md"
  implemented-by: []
  depends-on:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-03-catalog-preview-cache.md"
    - "docs/specs/alib-04-git-remote-provider.md"
    - "docs/specs/alib-06-webdav-provider.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-02-v5-snapshot-compat.md"
---

# ALIB-07 Design — Fault Matrix（headless slice）

## 方針

本 slice は製品コードを変更しない。既存の injected-fault 基盤
（`ProviderContext` の cancel/deadline/limits/credentials、
`tests/common` の `TempTree`/`GitFixture`/scripted provider・
publisher、`tests/rt_cache.rs` の計測 allocator）へ、新 provider
（git/webdav）の fault case を追加し、Case ID 別の証拠 matrix を
`docs/test-specs/` に記録する。

## 変更点

### 1. `tests/common/webdav_fixture.rs`（新規・抽出）

`tests/webdav.rs` 内に private だった in-process `TcpListener`
mock server（`MockServer`/`MockRequest`/`MockResponse`/
`Handler`）と DAV 応答 builder（`multistatus`/`file_props`/
`dir_props`/`propfind_response`）を `tests/common/` へ移動する。
ALIB-06 test-spec が `verified-by` として宣言済みのパスであり、
抽出により dangling relation を解消する。`MockResponse::raw` を
追加し、fixture 自身の head writer を介さない malformed
response（壊れた status line・header 過多・oversize head・
chunked 異常）を注入可能にする。

### 2. `tests/git.rs` 追加 case

- `offline_remote_typed_failures_mirror_stays_honest`:
  remote 削除後に refresh が typed error、mirror 上の
  browse/stat/取得済み object read は最後の fetched 状態を
  正直に返し、未取得 object の pin は typed failure、LFS object
  store 喪失は `NotFound`（pointer text を内容として返さない）。
- `remote_rename_keeps_pin_and_fixed_commit_identity`:
  remote rename 後、旧 path は `NotFound`・新 path は解決、
  既存 pin と fixed-commit locator は不変 byte を返す。
- `hostile_locator_and_url_inputs_fail_closed`:
  option/reflog/traversal を含む locator が argv 構築前に
  `InvalidRequest`、`-` 始まりの path component はデータとして
  解決、`ext::`/`fd::`/userinfo 付き URL を parse 段階で拒否。

### 3. `tests/webdav.rs` 追加 case

- `offline_server_fails_typed_on_every_op`: listener 喪失後に
  browse/stat/pin が `Offline`。
- `auth_revoked_between_pin_and_conditional_get`: pin 後の
  conditional GET が 401 → `AuthExpired`。
- `malformed_response_head_is_typed_transient`: 壊れた
  status line/ status code/ 非 UTF-8 head/ header 過多/ oversize
  head → `TransientIo`。
- `truncated_body_fails_mid_stream_not_silent` /
  `malformed_chunked_body_fails_typed`: Content-Length 過大・
  chunked 破損が stream 途中で typed error、completion slot に
  記録され、部分 byte を verified content として返さない。
- `requests_declare_identity_encoding`: 全 request が
  `accept-encoding: identity` を宣言し、圧縮 bomb の経路を
  transport で閉じる。
- `same_origin_redirect_keeps_credentials_in_origin` /
  `cross_origin_redirect_never_reaches_other_origin`:
  same-origin redirect では Authorization を維持、
  cross-origin redirect は接続前に拒否され foreign origin に
  request が 1 件も届かない。

### 4. `tests/import.rs` 追加 case

- `cancel_mid_stream_is_typed_and_same_import_resumes`:
  body read 中に cancel token を発火する provider wrapper で
  stage 相の cancel が `ImportError::Provider(Cancelled)`、
  manifest は `Started` に留まり同一 ImportId で resume して
  重複 asset/source を作らない。
- `same_bytes_across_providers_share_one_verified_blob`:
  異なる provider が byte 同一内容を供給した場合、2 件目は
  `BlobMaterialization::Reused` で同一 sha256 blob を共有し、
  asset/source identity は別々に発行される。

## 証拠 matrix

`docs/test-specs/alib-07-fault-matrix.md` に Issue #91 の固定
Case ID（42 class）ごとの Expected/Observed/Status を記録する。
実 network・実 WebDAV server・実 Git remote・OS credential store・
実 device・実 GUI は `NotRun`、未実装 provider（SVN/P4/SMB/S3）は
`NotImplemented`、v5→SHA-256 移行本体は deferred spec に従い
`NotRun` とする。

## 検証

`mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
`test-msrv`。cargo コマンドは mise task 経由を原則とする。
