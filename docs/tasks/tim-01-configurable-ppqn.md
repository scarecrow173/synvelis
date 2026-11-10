---
id: "TASK-034UHVQgQGF9WnpLCSCDTB"
type: "task"
status: "done"
title: "TIM-01 configurable PPQN — implementation tasks"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/tim-01-configurable-ppqn.md"
  implemented-by:
    - "docs/impl/ir/tim-01-configurable-ppqn.md"
  depends-on:
    - "docs/plans/tim-01-configurable-ppqn.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim-01-configurable-ppqn.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-configurable-ppqn.md"
  references:
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/designs/tim-01-configurable-ppqn-design.md"
  defers: []
  deferred-by: []
---

# TIM-01 configurable PPQN — implementation tasks

## Work

Issue #125 headless slice: `ProjectOp::SetProjectPpqn` Command と
project aggregate の rational-beat 再投影を実装し、CAS/Undo/
Save/Reload/非破壊 migration を headless fixture で証明する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/tim-01-configurable-ppqn-project-command.md`
- `docs/designs/tim-01-configurable-ppqn-design.md`
- `docs/plans/tim-01-configurable-ppqn.md`

## Dependencies

- Ruviform checked time API（`TickPosition`/`BeatPosition`/`Rounding`）
- 既存 `ProjectController` CAS/Undo/Save/migration 機構

## Affected Files

- `crates/synvelis-project/src/ppqn.rs`（新規）
- `crates/synvelis-project/src/ops.rs`
- `crates/synvelis-project/src/lib.rs`
- `crates/synvelis-project/src/project.rs`
- `crates/synvelis-project/src/tempo.rs`
- `crates/synvelis-gateway/src/prepare.rs`
- `crates/synvelis-project/tests/tim_01_ppqn.rs`（新規）

## Done When

- [x] `SetProjectPpqn` が expected_revision + old_ppqn 二重 CAS、
      `1..=32767` 範囲、open-recording 拒否、atomic validation を持つ
- [x] Exact/Round policy で project 全 musical tick を再投影し、
      inexact/overflow を拒否
- [x] Undo/Redo・Save/Reload・旧 snapshot 非破壊を fixture が証明
- [x] `project.ppqn == tempo_map.ppqn` 不変条件を `validate_references` に追加
- [x] 全検証 lane exit 0

## Verification

- [x] `cargo test -p synvelis-project --test tim_01_ppqn` — 14 passed
- [x] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` / `test-msrv` — exit 0
