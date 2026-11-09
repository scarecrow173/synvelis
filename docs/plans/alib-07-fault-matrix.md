---
id: "PLAN-alib07-fault-matrix"
type: "plan"
status: "in-progress"
title: "ALIB-07 実装計画 — Asset Library 全経路 Fault Matrix（synvelis-library headless slice）"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/91"
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
    - "docs/designs/alib-07-fault-matrix-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-03-catalog-preview-cache.md"
    - "docs/specs/alib-04-git-remote-provider.md"
    - "docs/specs/alib-06-webdav-provider.md"
    - "docs/specs/alib-02-v5-snapshot-compat.md"
---

# ALIB-07 Plan — Fault Matrix（synvelis-library headless slice）

## スコープ

- `crates/synvelis-library` の test/fixture/docs のみ。
- 製品コード（`src/`）は変更しない。provider 実装 owner
  （#85〜#90）の範囲に触れない。
- 実 network/実 server/実 device/実 GUI/OS credential store は
  `NotRun`/`Unsupported`/`NotImplemented` として記録し、
  Synthetic/Mock 結果を実環境成功と主張しない。

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-library/tests/common/webdav_fixture.rs` | 新規: webdav.rs から mock server + DAV builder を抽出、`MockResponse::raw` 追加 |
| `crates/synvelis-library/tests/common/mod.rs` | `pub mod webdav_fixture;` 追加 |
| `crates/synvelis-library/tests/webdav.rs` | 抽出分を削除して import に置換、ALIB-07 fault case 9 件追加 |
| `crates/synvelis-library/tests/git.rs` | ALIB-07 fault case 3 件追加 |
| `crates/synvelis-library/tests/import.rs` | ALIB-07 fault case 2 件追加（cancel resume・cross-provider share） |
| `docs/specs|designs|plans|tasks|test-specs|impl/ir/alib-07-fault-matrix*.md` | docs チェーン + 証拠 matrix |
| `docs/*/README.md` | managed index 再生成（doc-maintenance apply） |

## リスクと対策

- **fixture 抽出による既存 test 破壊**: `cargo test --test webdav`
  で全件 green を確認してから他 case を追加する。
- **offline git case の error kind 不確定性**: 分類が transport
  依存の箇所は `Offline|TransientIo|NotFound` の typed 集合で
  assert し、確定的なもの（LFS `NotFound`）は exact match とする。
- **doc graph の既存 blocking findings**: repo 全体に pre-existing
  の relation 破損があるため、本 slice の docs は新規 finding を
  増やさないことを `doc_status.js lint/audit` で確認する。
  既存不具合の修復はスコープ外。

## 検証マトリクス

| 検証 | コマンド |
| --- | --- |
| format | `mise run fmt-check` |
| 型 | `mise run check` |
| lint | `mise run lint` |
| 全 test | `mise run test` |
| RT lane | `mise run test-rt` |
| MSRV | `mise run test-msrv` |
| docs | `doc_status.js lint/audit` + `doc_maintenance.js apply`（index 再生成） |
