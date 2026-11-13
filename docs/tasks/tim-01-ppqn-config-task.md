---
id: "TASK-ibJUI1kRs2FdkuegWR0Ln7"
type: "task"
status: "in-progress"
title: "TIM-01 PPQN config — punch marks 再投影と受入残件検証タスク"
created: "2026-11-14"
updated: "2026-11-14"
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
    - "docs/plans/tim-01-ppqn-config-plan.md"
  implemented-by:
    - "docs/impl/ir/tim-01-ppqn-config.md"
  depends-on:
    - "docs/plans/tim-01-ppqn-config-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim-01-ppqn-config-plan.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-config.md"
  references:
    - "docs/specs/tim-01-ppqn-config.md"
    - "docs/designs/tim-01-ppqn-config-design.md"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN config — punch marks 再投影と受入残件検証タスク

## Work

Issue #125 残件の `synvelis-project` slice。
`reproject_ppqn_with` の site 列挙に `monitoring.punch` の
`punch_in`/`punch_out` を追加し、Issue 受入条件の headless
検証残件を `tim_01_ppqn.rs` fixture で固定する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/tim-01-ppqn-config.md`
- `docs/designs/tim-01-ppqn-config-design.md`
- `docs/plans/tim-01-ppqn-config-plan.md`
- `docs/test-specs/tim-01-ppqn-config.md`

## Dependencies

- 既存 `reproject_ppqn_with`/`reproject_tick`/`preview_ppqn`
  （`crates/synvelis-project/src/ppqn.rs`）
- 既存 `ProjectController` CAS/Undo/Save/migration 機構
- 実 v5 snapshot fixture（`tests/fixtures/alib02-v5-legacy.json`）

## Affected Files

- `crates/synvelis-project/src/ppqn.rs`
- `crates/synvelis-project/src/ops.rs`（doc のみ）
- `crates/synvelis-project/tests/tim_01_ppqn.rs`
- `docs/impl/ir/tim-01-ppqn-config.md`

## Done When

- [ ] `reproject_ppqn_with` が `monitoring.punch` の
      `punch_in`/`punch_out` を site 列挙に含み、preview と
      command が同一 site を報告する
- [ ] `tim_01_ppqn.rs` が 6 PPQN 行列 Save/Reload/Undo/Redo・
      実 v5 snapshot 非破壊 migration・分母上限・同 tick site・
      保存対象外安全性・異 Division SMF import を検証し green
- [ ] `mise run fmt-check` / `check synvelis-project` /
      `lint synvelis-project` / `test synvelis-project` が exit 0
- [ ] docs index 再生成・IR 記録済み

## Verification

- `cargo test -p synvelis-project --test tim_01_ppqn`（補助証拠;
  canonical は `mise run test synvelis-project`）
- `mise run fmt-check` / `mise run check synvelis-project` /
  `mise run lint synvelis-project` / `mise run test synvelis-project`
