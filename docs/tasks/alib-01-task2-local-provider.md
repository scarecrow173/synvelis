---
id: "TASK-1XhNow8ckI40ZrcNm0VXFJ"
type: "task"
status: "done"
title: "ALIB-01 task2 — LocalFolderProvider 実装 + 実 FS テスト"
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

# ALIB-01 task2 — LocalFolderProvider

## 内容

- root `canonicalize` 固定。locator resolve: `/` 区切り相対、
  `..`/絶対/NUL/`\\` 拒否、join→canonicalize→root 配下照合。
- bounded iterative DFS browse: 各 dir `file_name`+kind sort、
  Container/Asset 両 entry、symlink は canonical target root 内
  かつ未訪問のみ展開、frontier cursor、Partial/Complete。
- stat: `symlink_metadata`+`metadata` で Observed facts、media は
  `Unknown`。
- pin: bounded sha256 → `ConditionalStrong{ContentHash}`。
- open_pinned: expected_size fstat precondition (`SourceChanged`) →
  EOF sha256 照合 verify reader。
- search: name 部分一致 walk。preview/range: Seek+Take
  （Unverified claims）。cancel/deadline/budget を各所で確認。
- tests/local.rs: 実 tempdir fixture、AC1/AC2 の実測
  （paging・cursor 失効・mutation/rename・traversal・symlink・
  cancel・limit・search・preview）。

## 検証

`cargo test -p synvelis-library` の local tests が GREEN。
正規: `mise run test`。
