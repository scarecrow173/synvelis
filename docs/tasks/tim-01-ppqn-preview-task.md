---
id: "TASK-2pOiUyTrEwQaSdFgHjKl"
type: "task"
status: "done"
title: "TIM-01 — PPQN Preview op + New Project PPQN"
created: "2026-11-12"
updated: "2026-11-12"
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
    - "docs/plans/tim-01-ppqn-preview-plan.md"
  implemented-by:
    - "docs/impl/ir/tim-01-ppqn-preview.md"
  depends-on:
    - "docs/plans/tim-01-ppqn-preview-plan.md"
    - "docs/tasks/tim-01-task2-app-ppqn.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim-01-ppqn-preview-plan.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-preview.md"
  references:
    - "docs/specs/tim-01-ppqn-preview.md"
    - "docs/designs/tim-01-ppqn-preview-design.md"
  defers: []
  deferred-by: []
---

# TIM-01 — PPQN Preview op + New Project PPQN

## Work

Issue #125 headless 残件: `syn.project.preview_ppqn`（非破壊
dry-run op）と `syn.project.new` の optional `ppqn` param を
実装する。project 層は `preview_ppqn(&Project, new_ppqn,
policy) -> PpqnChangePreview` 自由関数を `ppqn.rs` に追加し、
サイト列挙を `reproject_ppqn_with` に共通化。app 層は read-only
preview op と new-project ppqn を既存 wire convention で公開
し、headless fixture で非破壊・inexact/overflow 報告・
new ppqn 既定/選択/拒否を証明する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/tim-01-ppqn-preview.md`
- `docs/designs/tim-01-ppqn-preview-design.md`
- `docs/plans/tim-01-ppqn-preview-plan.md`
- `docs/tasks/tim-01-task2-app-ppqn.md`（`syn.project.set_ppqn`
  app slice・done）
- `docs/tasks/tim-01-configurable-ppqn.md`（headless Command
  slice・done）

## Dependencies

- `ProjectSession::project()`（`&Project` accessor）・
  `ProjectSession::{apply, revision}`（public・変更不要）
- `ProjectOp::SetProjectPpqn`・`PpqnConversionPolicy`・
  `reproject_ppqn`（synvelis-project、merge 済み）
- `ppqn_param`/`ppqn_policy`/`invalid_params`（app.rs 既存
  wire helper）

## Affected Files

- `crates/synvelis-project/src/ppqn.rs` — preview API +
  サイト列挙共通化
- `crates/synvelis-project/src/lib.rs` — re-export
- `crates/synvelis-project/tests/tim_01_ppqn.rs` — 追記のみ
- `crates/synvelis-app/src/app.rs` — preview op +
  `project_new_with_ppqn` + wire helper
- `crates/synvelis-app/src/service.rs` — `OPS`/`dispatch`/
  `optional_u32_param`
- `crates/synvelis-app/src/binding.rs` — `project_new` ppqn
- `crates/synvelis-app/tests/tim01_ppqn_preview.rs` — 新規
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  tim-01-ppqn-preview*` — doc chain 6 件

## Done When

- [x] `preview_ppqn` が `&Project` 非破壊でサイト件数・
      inexact 一覧・overflow・policy 別可否を報告し、
      `reproject_ppqn` とサイト列挙を共有する
- [x] `syn.project.preview_ppqn` が headless dispatch で動作し
      session gate なし・params 不正は `invalid_params`
- [x] `syn.project.new` の `ppqn` が明示選択・省略時 960・
      範囲外は project 層委譲で拒否
- [x] preview が Project/revision/engine notification を
      変更しないことが fixture で証明
- [x] 全検証 lane exit 0

## Verification

- [x] `cargo test -p synvelis-project --test tim_01_ppqn` —
      20 green
- [x] `cargo test -p synvelis-app --test tim01_ppqn_preview` —
      6 green
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
      `test-rt` / `check-app-native` / `lint-app-native` /
      `test-msrv` / `build` — 結果は impl IR に記録
