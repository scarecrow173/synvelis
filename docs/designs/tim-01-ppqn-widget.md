---
id: "DESIGN-TiM01wIdGeTnPw02"
type: "design"
status: "approved"
title: "TIM-01 PPQN 選択 widget intent — params 付き Action → `syn.project.ppqn` 設計"
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
  implemented-by: []
  depends-on:
    - "docs/designs/tim-01-ppqn-ui-design.md"
    - "docs/specs/tim-01-ppqn-widget.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-01-ppqn-widget.md"
    - "docs/tasks/tim-01-ppqn-widget.md"
    - "docs/test-specs/tim-01-ppqn-widget.md"
    - "docs/impl/tim-01-ppqn-widget.md"
    - "docs/plans/tim-01-ppqn-widget.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-01-ppqn-widget.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-widget.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN 選択 widget intent — 設計

## Context

`PpqnUi` gesture 状態機械と `ppqn_*` helper は先行 slice で
着地済みだが、登録 `syn.*` op から到達不能だった（helper は
`SynvelisApp` のメソッドで、service は `UiRuntime` に seal 済み
— `syn.project.preview_ppqn`/`set_ppqn` は pending が既に
あるときだけ `PpqnUi` を feed し、pending 確立・preview-time
CAS の mint は gesture でしか起こらない）。widget intent が
params を運べる経路は `ruviform#944` で開通済み（
`ButtonSpec.params` → `ViewHost::invoke` → `rvf.ui.command.v1`
`Action` → `decode_view_command` → `action_params`）。
本設計はその両端を、最小の新規 op 1 件で結ぶ。

## Scope

In:

- `syn.project.ppqn` op 新設（service.rs `OpInfo` + dispatch
  arm、handler `ui_ops.rs::op_project_ppqn`）。
- `ppqn_policy_wire_decode`（`ppqn_ui.rs` — `ppqn_policy_wire`
  の逆関数として wire `policy`/`rounding` →
  `PpqnConversionPolicy`）。
- `answer_view_state_action` に `ppqn.*` 分岐 +
  `dispatch_ppqn_action`（`syn.project.ppqn` mint）。
- `binding.rs` `PPQN_ACTION_PREFIX` const + `action_params`
  文書更新。
- `screens.rs` Project tab へ `ppqn.candidates`（推奨値×policy
  Button 8 個）・`ppqn.gesture`（Confirm/Cancel）を追加。
- `tests/tim01_ppqn_widget.rs` headless fixture。

Out:

- 実 Window/pixel/実 OS（外部 gate）、任意値入力 widget、SMF
  export division 連動、`state.rs` re-export 追加（lease 外 —
  呼び出しは canonical `ppqn_ui::` 経路）、docs index。

## Components and Boundaries

### Op 契約 — `syn.project.ppqn`

| params | 必須 | 意味 |
| --- | --- | --- |
| `view_id` | yes | gesture を所有する view。`require_attached_view` で検証 — shell は widget の declared origin ではなく routing 解決済みの view を書き込むため、spec が foreign view を名乗る余地はない |
| `action` | yes | `select`/`confirm`/`cancel` — closed set、未知は `invalid_params` |
| `new_ppqn` | select のみ | u64 → u32、範囲 `1..=32767` は `PpqnUi::select` が検証 |
| `policy` | select のみ | `"exact"`/`"round"` — `ppqn_policy_wire_decode` |
| `rounding` | select・round のみ必須 | `ppqn_policy_wire` wire 名 5 種。`exact`+rounding は contract 違反 |

結果 shape:

- `select` → `{view_id, selected, preview}` — `preview` は
  `syn.project.preview_ppqn` の結果 verbatim。
- `confirm` → `{view_id, confirmed, project}` — `project` は
  `project_info` payload。
- `cancel` → `{view_id, canceled}`。

`select` = `ppqn_select` + `ppqn_preview_pending` の融合は
Coordinator 承認済みの意味付け — 推奨値 Button は「選ぶと
即座に preview が出る」ため select+preview を分けない（
preview なしの candidate 残置は widget 操作モデルでは存在
しない）。`confirm`/`cancel` は payload を持たず、view 所有
pending 状態を引く — **Confirm Button の押下が project を
変更するのは preview が applicable のときだけ**で、reject は
implicit commit ではなく command `Result` の拒否として settle
する（`ComponentVisualState` に disabled 概念がないため、button
は常時 pressable のまま op が正直に判定する）。

### Wire 境界 — `rvf.ui.command.v1` `Action`

- `binding.rs` の params 許可 target は `view`・`autx.*`・
  `synvelis.library` のみ（`ruviform#944` 契約）。ppqn は
  `view` target 上の `ppqn.*` prefix action として載せ、
  domain target に params を乗せる wire 拡張は行わない —
  admission（decoder）≠ acceptance（op の strict decode）。
- shell `dispatch_ppqn_action` は `params` を verbatim 転送し、
  `view_id`/`action` を shell 側で上書き insert する —
  `dispatch_library_action`/`dispatch_view_select` の先例どおり。
  非 object params は shell で早期拒否。

### Widget spec — Project tab

- `ppqn.candidates` panel: `RECOMMENDED_PPQN` ×
  {exact, round(nearest_even)} の 8 Button。key
  `ppqn.select.<v>.<policy>`、action `ppqn.select`、params
  `{new_ppqn, policy, rounding?}`。`round` は `nearest_even` を
  宣言 — 残り 4 rounding は typed `syn.project.preview_ppqn`/
  `syn.project.ppqn` driver 経路に残す（button は policy 対の
  「最も意味のある代表」として nearest_even を採用 —
  Coordinator 承認済み）。
- `ppqn.gesture` panel: `ppqn.confirm`/`ppqn.cancel` Button —
  params なし。
- spec 内順序: status → candidates → preview → effects → sites
  → gesture。preview/site readout を見てから Confirm を押す
  流れに並べる。

### 再投影

`AppService::call` の tail が全 op で `publish_project_length`
を走らせるため、`syn.project.ppqn` も追加配線なしで
`PpqnScreenState`（current・pending・revision-guarded preview）
が更新され、`ViewHost::update` 相当の spec 再投影で
`ppqn.*` 領域が追従する。

## Data and Control Flow

```
ButtonSpec.params {new_ppqn, policy, rounding?}
  → ViewHost::invoke → ViewEvent::Intent
  → rvf.ui.command.v1 Action {target:"view", action:"ppqn.select", params}
  → decode_view_command (params admission: view target)
  → answer_view_state_action (ppqn.* prefix)
  → dispatch_ppqn_action (view_id/action insert, params verbatim)
  → syn.project.ppqn → op_project_ppqn
  → ppqn_select → ppqn_preview_pending (select)
      / ppqn_confirm (confirm, preview-CAS)
      / ppqn_cancel (cancel)
  → project_preview_ppqn / project_set_ppqn (real ops)
  → Result → post_routed → command Result (Button receipt)
  → publish_project_length tail → PpqnScreenState → spec reprojection
```

## Non-Functional Constraints

- 新規 `syn.*` op は `syn.project.ppqn` 1 件のみ（既存
  `syn.project.preview_ppqn`/`set_ppqn` wire 契約を変えない）。
- `PpqnUi` 状態機械に新 phase/mutation は追加しない。
- `tim01_ppqn_ui.rs` の全契約は非回帰で維持する。

## Risks and Trade-offs

- `ppqn_policy_wire_decode` と `app.rs::ppqn_policy` の語彙
  二重定義 — app.rs 側は lease 外のため canonical 化は統合
  時課題（decode 表は同じ contract を両側で mirror）。
- select+preview 融合は「select だけして preview しない」状態を
  widget から作れなくする — その状態自体は `PpqnUi` が表現
  可能だが、widget 操作モデルでは使わない（typed driver 経路で
  `ppqn_select` 単独駆動も可能）。
- 推奨値 8 Button は 1 value 1 pair の静的 spec — arbitrary
  value 入力は widget なしで不可（範囲外、別 Issue 候補）。

## Deferred Design Concerns

- 任意値 1..=32767 入力: ruviform-ui `ComponentSpec` に
  text/number input widget が無い — ruviform 側 widget/params
  経路の別 Issue 候補。
- `app.rs::ppqn_policy` と `ppqn_policy_wire_decode` の canonical
  統合（app.rs は本 slice lease 外）。
- `state.rs` の re-export 追加（同、canonical `ppqn_ui::` 経路で
  参照済み）。

## Verification Notes

- `tests/tim01_ppqn_widget.rs`（common::mounted fixture —
  mounted `ViewHost` + `NativeShell` + `FakeDriver`）が
  widget→intent→gesture→op→receipt→spec 再投影を固定。
- `tim01_ppqn_ui.rs`・`tim01_ppqn_app.rs`・`tim01_ppqn_preview.rs`
  非回帰。
- 詳細は `docs/test-specs/tim-01-ppqn-widget.md`。
