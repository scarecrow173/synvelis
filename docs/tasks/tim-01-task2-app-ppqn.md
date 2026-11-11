---
id: "TASK-034VtBNiTaFYQMA3eOBYXv"
type: "task"
status: "done"
title: "TIM-01 task2 — app 層 PPQN 統合"
created: "2026-11-11"
updated: "2026-11-11"
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
    - "docs/plans/tim-01-app-ppqn-plan.md"
  implemented-by:
    - "docs/impl/ir/tim-01-app-ppqn-slice.md"
  depends-on:
    - "docs/plans/tim-01-app-ppqn-plan.md"
    - "docs/tasks/tim-01-configurable-ppqn.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim-01-app-ppqn-plan.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-app-ppqn.md"
  references:
    - "docs/specs/tim-01-app-ppqn.md"
    - "docs/designs/tim-01-app-ppqn-design.md"
  defers: []
  deferred-by: []
---

# TIM-01 task2 — app 層 PPQN 統合

## Work

Issue #125 app 層 slice: `syn.project.set_ppqn` op を
`synvelis-app` の `syn.*` service 面に追加し、app 層前提条件
（transport-stopped・automation-write・merge-commit）を public
accessor で実装、`project_info` に `ppqn` を投影する。
headless fixture `crates/synvelis-app/tests/tim01_ppqn_app.rs`
で wire CAS/stale・前提条件拒否・undo/redo・save/reload・
engine revision 通知を証明する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/tim-01-app-ppqn.md`
- `docs/designs/tim-01-app-ppqn-design.md`
- `docs/plans/tim-01-app-ppqn-plan.md`
- `docs/tasks/tim-01-configurable-ppqn.md`（headless Command slice・done）

## Dependencies

- `ProjectSession::{transport, control, apply}`・
  `ControlSession::{recording, record_enabled}`（public accessor
  完備・変更不要）
- `ProjectOp::SetProjectPpqn`・`PpqnConversionPolicy`・
  `PPQN_MIN`/`PPQN_MAX`（synvelis-project、merge 済み）

## Affected Files

- `crates/synvelis-app/src/service.rs` — `OPS` + `dispatch`
- `crates/synvelis-app/src/app.rs` — `project_set_ppqn` +
  `project_info.ppqn`
- `crates/synvelis-app/tests/tim01_ppqn_app.rs` — 新規 fixture

## Done When

- [x] `syn.project.set_ppqn` が headless dispatch で動作し、
      二重 CAS/stale 拒否・`1..=32767` 検証が app 層経路で維持
- [x] transport playing・automation record enabled・open
      recording transaction の各前提条件拒否が fixture で検証
- [x] `project_info.ppqn` 投影・undo/redo・save/reload・
      engine revision 通知が fixture で証明
- [x] 全検証 lane exit 0

## Verification

- [x] `cargo test -p synvelis-app --test tim01_ppqn_app` — 11 green
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
      `test-rt` / `test-msrv` / `check-app-native` /
      `lint-app-native` — exit 0
