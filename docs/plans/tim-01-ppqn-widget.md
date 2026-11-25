---
id: "PLAN-TiM01wIdGeTnPw06"
type: "plan"
status: "approved"
title: "TIM-01 PPQN 選択 widget intent — 実施計画（synvelis#125 widget slice）"
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
  implemented-by:
    - "docs/tasks/tim-01-ppqn-widget.md"
  depends-on:
    - "docs/specs/tim-01-ppqn-widget.md"
    - "docs/designs/tim-01-ppqn-widget.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/tim-01-ppqn-widget.md"
    - "docs/test-specs/tim-01-ppqn-widget.md"
    - "docs/impl/tim-01-ppqn-widget.md"
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

# TIM-01 PPQN 選択 widget intent — plan

## 実施順

1. **wire decode** — `ppqn_policy_wire_decode` を
   `ppqn_policy_wire` の隣に追加（workspace、逆関数・strict）。
2. **op handler** — `op_project_ppqn` を `ppqn_*` helper の隣に
   追加（ui_ops.rs、in-lease `impl SynvelisApp` block）。
3. **op 登録** — `service.rs` に `OpInfo` + dispatch arm 最小
   差分（lease 拡張は事前に Coordinator 承認取得）。
4. **routing** — `answer_view_state_action` の `ppqn.*` prefix
   分岐 + `dispatch_ppqn_action`（`dispatch_library_action` 先例
   に mirror）+ `PPQN_ACTION_PREFIX` const。
5. **widget spec** — `ppqn_candidates_panel`/`ppqn_gesture_panel`
   を Project tab children に挿入（status → candidates →
   preview → effects → sites → gesture）。
6. **fixture** — `common::mounted` で press → Result →
   `screen.ppqn` → `main_screen_spec` 再投影の検証 test を
   先に書かず動作確認してから断言ケースを固定（forged intent
   系は wire 形状を `RvfActionIntent` に揃える）。
7. **docs + evidence + 検証レーン** — `mise run fmt-check` /
   `check|lint|test synvelis-app` / `check|lint|test
   synvelis-workspace` / `test-rt` / `check-app-native` /
   `lint-app-native` / `test-app-gpui` / `test-msrv`。

## 判断記録

- widget 形状 = 推奨値 Button 群（Table+Confirm ではない）—
  推奨値限定は Coordinator 決定、任意値入力は ruviform に入力系
  widget が無く slice 外。
- `round` Button は `nearest_even` 代表のみ（残り rounding は
  typed op 経路へ）。
- op は新設 1 件 `syn.project.ppqn`（gesture op 先例
  `syn.signal_flow.routing` に mirror）— `set_ppqn`/`preview_ppqn`
  既存 op 契約は不変。
- `service.rs` は最小差分で lease 拡張（msg 承認済み）—
  `app.rs`・`state.rs` は触らない（canonical `ppqn_ui::` 経路）。
