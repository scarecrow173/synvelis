---
id: "IR-alib07-fault-matrix"
type: "impl"
status: "completed"
title: "ALIB-07 Fault Matrix — headless 実装記録（synvelis-library）"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  implements:
    - "docs/plans/alib-07-fault-matrix.md"
    - "docs/designs/alib-07-fault-matrix-design.md"
    - "docs/specs/alib-07-fault-matrix.md"
  verified-by:
    - "docs/test-specs/alib-07-fault-matrix.md"
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
---

# ALIB-07 Implementation Record

## 実装内容

本 slice は製品コードを変更しない。`crates/synvelis-library` の
headless test/fixture のみを拡張し、Issue #91 の Case ID 別
証拠を `docs/test-specs/alib-07-fault-matrix.md` に記録した。

### Fixture 抽出

`crates/synvelis-library/tests/common/webdav_fixture.rs` を新設し、
`tests/webdav.rs` 内の in-process mock HTTP server
（`MockServer`/`MockRequest`/`MockResponse`/`Handler`）と DAV 応答
builder を移動。`MockResponse::raw` を追加して fixture の
head writer を介さない malformed response を注入可能にした。
`tests/common/mod.rs` に `pub mod webdav_fixture` を登録。

### 追加 test

- `tests/git.rs`（3 件）: offline remote typed failure +
  mirror 正直な stale、remote rename + pin/fixed-commit 不変、
  hostile locator/URL fail-closed（argv 構築前拒否・`-` 始まり
  path のデータ解決・`ext::`/`fd::`/userinfo 拒否）。
- `tests/webdav.rs`（8 件 + identity encoding 1 件）: offline
  typed、auth revoke → `AuthExpired`、malformed response head、
  truncated Content-Length、malformed chunked、same-origin
  redirect credential 維持、cross-origin redirect 接続前拒否
  （foreign hit=0）、`accept-encoding: identity` 宣言。
- `tests/import.rs`（2 件）: stage 相 cancel の typed
  `Cancelled` + 同一 ImportId resume、cross-provider 同一 bytes
  の `BlobMaterialization::Reused` による blob 共有。

## 実行済み検証

- `cargo test --test webdav`（library）: 41 passed / 0 failed
  （新規 9 case 含む全件 green）。
- `cargo test --test webdav requests_declare_identity_encoding`:
  1 passed（個別確認）。
- `cargo check --test webdav`（コンパイル確認済み）。
- `mise run fmt-check`: exit 0。
- `mise run check`: exit 0。
- `mise run lint`: exit 0（初回 `duplicated_attributes` を
  fixture の inner attribute 重複で検出し修正後 green）。
- `mise run test`: exit 0（workspace 全件。library git 28 /
  import 17 / webdav 42 / rt_cache 1 を含む）。
- `mise run test-rt`: exit 0。
- `mise run test-msrv`（rust 1.89）: exit 0。
- `doc_maintenance.js apply`: managed index 5 件再生成、
  findings 519→502。残存する alib-07 findings は alib-06 と
  同一 category（ID 形式・Markdown link 未到達・相互 relation）
  のみで新規 category の増加なし。

## NotRun / Unsupported / NotImplemented

- 実 network / 実 WebDAV server / 実 Git remote（SSH・HTTP
  transport・LFS batch API）: NotRun。
- OS credential store 実機: NotRun。
- 実 device / 実 GUI / Linux・macOS 実機: NotRun。
- publish/commit 相 cancel の注入点: NotRun（stage 相のみ実施）。
- v5→SHA-256 移行本体（CRC 昇格）: deferred spec に従い NotRun
  （namespace 共存のみ Pass）。
- SVN/P4/SMB/S3 provider: NotImplemented（matrix に行を設けず
  暗黙 PASS にしない）。

詳細は `docs/test-specs/alib-07-fault-matrix.md` の
Case ID 別 matrix を参照。
