---
id: "IMPL-TiM01wIdGeTnPw05"
type: "impl"
status: "in-progress"
title: "TIM-01 PPQN 選択 widget intent — Project tab params 付き Action → `syn.project.ppqn` 実装記録"
created: "2026-11-25"
updated: "2026-11-25"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/tim01_ppqn_widget.rs"
      - type: "file"
        path: "docs/specs/tim-01-ppqn-widget.md"
      - type: "file"
        path: "docs/designs/tim-01-ppqn-widget.md"
      - type: "file"
        path: "docs/tasks/tim-01-ppqn-widget.md"
      - type: "file"
        path: "docs/test-specs/tim-01-ppqn-widget.md"
      - type: "file"
        path: "docs/impl/tim-01-ppqn-widget.md"
      - type: "file"
        path: "docs/plans/tim-01-ppqn-widget.md"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "crates/synvelis-app/src/binding.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/ppqn_ui.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/tim-01-ppqn-widget.md"
    - "docs/designs/tim-01-ppqn-widget.md"
  implemented-by: []
  depends-on:
    - "docs/test-specs/tim-01-ppqn-widget.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim-01-ppqn-widget.md"
    - "docs/tasks/tim-01-ppqn-widget.md"
    - "docs/plans/tim-01-ppqn-widget.md"
    - "evidence/sv125/"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/tim-01-ppqn-widget.md"
  derived-by: []
  verifies: []
  verified-by:
    - "crates/synvelis-app/tests/tim01_ppqn_widget.rs"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  deferred-by: []
---

# TIM-01 PPQN 選択 widget intent — 実装記録（synvelis#125）

## Summary

Project tab の `ppqn.*` 領域に params 付き widget を実装し、
`rvf.ui.command.v1` `Action` → `syn.project.ppqn`（新規 gesture
op）→ `ppqn_*` helper → `syn.project.*` ops → receipt → spec
再投影の鎖を headless で完結させた。新規 `syn.*` op は
`syn.project.ppqn` 1 件のみ、`PpqnUi` 状態機械と既存
`set_ppqn`/`preview_ppqn` wire 契約は不変。

## Context

`ppqn_*` helper は先行 slice で存在したが、seal された
`UiRuntime`/`AppService` の内側にある `SynvelisApp` メソッドで、
登録 op からは到達不能だった。`ruviform#944` で
`ButtonSpec.params` → `Action` intent が開通済みのため、widget
spec が `{new_ppqn, policy, rounding?}` を宣言値として運び、
shell が `view` target の `ppqn.*` action を `syn.project.ppqn`
へ mint する経路を新設した。`service.rs` は Coordinator 承認の
lease 拡張で `OpInfo` 1 件 + dispatch 1 arm の最小差分のみ。

## Implementation

- `crates/synvelis-workspace/src/ppqn_ui.rs` —
  `ppqn_policy_wire_decode(policy, rounding)`:
  `ppqn_policy_wire` の逆関数。`exact`+rounding 拒否・
  `round` に rounding 必須・未知名拒否の strict decode。
- `crates/synvelis-app/src/ui_ops.rs` —
  `op_project_ppqn(params)`: `view_id`（`require_attached_view`
  検証）+ `action` ∈ select|confirm|cancel。select は
  `ppqn_select` + `ppqn_preview_pending` 融合で
  `{view_id, selected, preview}`、confirm は
  `ppqn_confirm`（preview-CAS 付き `set_ppqn`）で
  `{view_id, confirmed, project}`、cancel は
  `{view_id, canceled}`。不正 params は `invalid_params`。
- `crates/synvelis-app/src/service.rs` — `OpInfo` + dispatch
  arm のみ。
- `crates/synvelis-app/src/shell.rs` —
  `answer_view_state_action` の `ppqn.*` prefix 分岐 +
  `dispatch_ppqn_action`: params verbatim 転送 +
  `view_id`/`action` を shell 側 insert（非 object params は
  早期拒否）— `dispatch_library_action` と同型。
- `crates/synvelis-app/src/binding.rs` — `PPQN_ACTION_PREFIX`
  const + `action_params`/admission 文書更新（code change なし
  — `view` target は params 許可済み）。
- `crates/synvelis-app/src/screens.rs` —
  `ppqn_select_button`（`ppqn.select.<v>.<policy>`、params
  宣言）× `RECOMMENDED_PPQN` × {exact, round·nearest_even} +
  `ppqn_gesture_button`（`ppqn.confirm`/`ppqn.cancel`、params
  なし）+ `ppqn.candidates`/`ppqn.gesture` panel を Project
  tab に挿入。`PpqnScreenState` の陳腐化した「action button
  を持たない」doc を実態に更新。

## Validation

- `tests/tim01_ppqn_widget.rs` — 10 test・`common::mounted`
  fixture（実 Button press → `ViewHost::invoke` →
  `rvf.ui.command.v1` → `NativeShell` → `AppService` →
  `Result` → `MainScreenState`/spec 再投影）。spec 形状・
  select+preview 融合・confirm 適用・cancel 非破壊・再 select
  置換・pending 無し confirm/cancel・rejected preview の
  confirm 拒否・forged params 拒否・target 境界・typed op
  経路の任意値。
- `tim01_ppqn_ui.rs` / `tim01_ppqn_app.rs` /
  `tim01_ppqn_preview.rs` — 非回帰 pass。
- 詳細コマンド・exit code は `evidence/sv125/` を参照。

## Risks

- `ppqn_policy_wire_decode` と `app.rs::ppqn_policy` は同じ
  `policy`/`rounding` 語彙を両側で mirror — app.rs は lease
  外のため canonical 統合は統合時課題。
- 推奨値 Button のみ（任意値 1..=32767 入力は ruviform に
  入力系 widget が無く slice 外 — `syn.project.ppqn` typed
  経路で任意値は到達可能、test `op_path_selects_arbitrary_
  value` で固定）。

## Follow-ups

- 実 Window/pixel/実 OS 受入 — `synvelis#114` +
  `ruviform#631` 外部 gate（headless Pass の転用禁止）。
- 任意値入力 widget — ruviform-ui `ComponentSpec` に入力系
  widget が必要（別 Issue 候補）。
- `app.rs::ppqn_policy` ↔ `ppqn_policy_wire_decode` の
  canonical 統合・`state.rs` re-export 追加（lease 外）。
- docs index（`docs/**/README.md`）は Coordinator が統合時に
  整備。
