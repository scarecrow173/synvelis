---
id: "TASK-G71A5me0TE1Mc9yHV4G1Qv"
type: "task"
status: "done"
title: "ALIB-01 task3 — ProjectAssetsProvider + catalog/blob port"
created: "2026-11-07"
updated: "2026-11-07"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/85"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/alib-01-asset-provider-baseline.md"
  implemented-by: []
  depends-on:
    - "docs/plans/alib-01-asset-provider-baseline.md"
    - "docs/tasks/alib-01-task1-contract-types.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
---

# ALIB-01 task3 — ProjectAssetsProvider

## 内容

- `ProjectCatalog` port + `Mutex<ProjectController>` 実装 +
  `ProjectCatalogSnapshot` 静的実装（test/headless）。
- `ProjectBlobStore` port + `LocalBlobDir`（filename-safe 検証の
  `<dir>/<digest>` open/stat）。app は `media_root/blobs` を渡す
  （path 規約は複製しない）。
- locator = tagged `AssetId`。browse = catalog snapshot `AssetId`
  sort + revision epoch cursor。stat = source states/import
  provenance/blob facts を Observed、blob missing は
  `Availability::Missing`+`Unknown` byte_len。
- pin = `ImmutableObject{revision:"blob:<digest>"}` + digest_hint=
  sha256 + expected_size 実測。
- open_pinned: identity/size precondition → verify reader。
- search = typed `UnsupportedCapability`。preview = blob range read
  （Unverified）。
- tests/project.rs: 実 `Mutex<ProjectController>` + test-only
  `Operation` と `ProjectCatalogSnapshot` の双方で AC 検証、
  revision 進行後 cursor 失効、blob 改竄 `IntegrityMismatch`、
  missing `Availability::Missing`。

## 検証

`cargo test -p synvelis-library` の project tests が GREEN。
正規: `mise run test`。
