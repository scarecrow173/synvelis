---
id: "TASK-TiM01wIdGeTnPw03"
type: "task"
status: "approved"
title: "TIM-01 PPQN 選択 widget intent — 実装作業（synvelis#125 widget slice）"
created: "2026-11-25"
updated: "2026-11-25"
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
    - "docs/specs/tim-01-ppqn-widget.md"
    - "docs/designs/tim-01-ppqn-widget.md"
  implemented-by: []
  depends-on:
    - "docs/specs/tim-01-ppqn-widget.md"
    - "docs/designs/tim-01-ppqn-widget.md"
    - "docs/test-specs/tim-01-ppqn-widget.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/tim-01-ppqn-widget.md"
    - "docs/impl/tim-01-ppqn-widget.md"
    - "docs/tasks/tim-01-ppqn-ui-task.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN 選択 widget intent — task

## 目的

`synvelis#125` 受入行4の headless 残件 — Project tab widget から
`rvf.ui.command.v1` params 付き Action 経由で `ppqn_*` gesture
へ配線し、widget→intent→gesture→op→receipt→spec 再投影を
headless fixture で固定する。

## 作業項目

- [x] `ppqn_policy_wire_decode` を
  `crates/synvelis-workspace/src/ppqn_ui.rs` に追加
  （`ppqn_policy_wire` の逆関数・strict decode）。
- [x] `SynvelisApp::op_project_ppqn` を
  `crates/synvelis-app/src/ui_ops.rs` に追加
  （`params.action` ∈ select|confirm|cancel）。
- [x] `syn.project.ppqn` を `crates/synvelis-app/src/service.rs`
  に登録（`OpInfo` 1 件 + dispatch 1 arm — service.rs は
  Coordinator 承認で lease 拡張済み）。
- [x] `answer_view_state_action` に `ppqn.*` 分岐 +
  `dispatch_ppqn_action` を `crates/synvelis-app/src/shell.rs`
  に追加。
- [x] `PPQN_ACTION_PREFIX` const + `action_params` 文書更新を
  `crates/synvelis-app/src/binding.rs` に追加。
- [x] `ppqn.candidates`（推奨値 4 × {exact, round·nearest_even}
  Button 8 個）+ `ppqn.gesture`（Confirm/Cancel）を
  `crates/synvelis-app/src/screens.rs` Project tab に追加。
  `PpqnScreenState` の「action button を持たない」という
  陳腐化した doc comment を更新。
- [x] `crates/synvelis-app/tests/tim01_ppqn_widget.rs` 新規 —
  `common::mounted` fixture で 10 test。
- [x] doc chain 本ファイル群 +
  `evidence/sv125/` 検証記録。

## 境界

- `tim01_ppqn_ui.rs` 非回帰（変更なし — 追従不要だった）。
- docs index（`docs/**/README.md`）は一切編集しない。
- `app.rs`・`state.rs`・他 crate・lock/Cargo.toml・mise 系は
  不変。

## 依存

- `ruviform#944`（`ButtonSpec.params` → Action intent）—
  merge 済み前提。
- Coordinator 承認: `service.rs` lease 拡張（`syn.project.ppqn`
  登録の最小差分）。
