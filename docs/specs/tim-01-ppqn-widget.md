---
id: "SPEC-TiM01wIdGeTnPw01"
type: "spec"
status: "approved"
title: "TIM-01 PPQN 選択 widget intent — Project tab の params 付き Action 配線（synvelis#125 widget slice・headless）"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/tim-01-ppqn-ui.md"
    - "docs/specs/tim-01-ppqn-preview.md"
    - "docs/specs/tim-01-app-ppqn.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/tim-01-ppqn-widget.md"
    - "docs/tasks/tim-01-ppqn-widget.md"
    - "docs/test-specs/tim-01-ppqn-widget.md"
    - "docs/impl/tim-01-ppqn-widget.md"
    - "docs/plans/tim-01-ppqn-widget.md"
    - "docs/specs/tim-01-ppqn-ui.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  derived-by:
    - "docs/designs/tim-01-ppqn-widget.md"
    - "docs/plans/tim-01-ppqn-widget.md"
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-widget.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN 選択 widget intent（synvelis#125 widget slice・headless）

## 背景・目的

Synvelis Issue #125 の widget slice。先行 slice で merge 済みの
もの — `PpqnUi` view 所有 gesture 状態機械・`SynvelisApp::ppqn_*`
helper・Project tab の read-only spec（status/preview/effects/
sites）・`ButtonSpec.params` を `rvf.ui.command.v1` `Action`
intent に載せる ruviform 側経路（`ruviform#944`、view/`autx.*`/
`synvelis.library` target は params 許可済み）。

残件は「実 GUI widget intent」: Project tab の widget から
params 付き Action 経由で `ppqn_select` → `ppqn_preview_pending` →
`ppqn_confirm`/`ppqn_cancel` gesture へ配線すること。現状
`ppqn_*` helper はどの登録 `syn.*` op からも到達不能で、widget
→ intent → gesture → op → receipt → spec 再投影の鎖が headless
でも通っていない。本 slice はその配線を headless fixture で固定
する。

## 要求

- Project tab `ppqn.*` 領域に params 付き選択 widget を追加する。
  推奨値 `RECOMMENDED_PPQN`（480/960/1000/1920）ごとに
  `exact`/`round(nearest_even)` の Button を 1 対ずつ置き、
  `ButtonSpec.params` に `{new_ppqn, policy, rounding?}` を
  宣言する。Confirm/Cancel Button は params なし — view 所有の
  pending 状態が payload。
- 任意値 `1..=32767` の自由入力は ruviform-ui `ComponentSpec`
  に入力系 widget が無いため本 slice の範囲外 — 推奨値のみで
  切る（任意値は typed `syn.project.ppqn` driver 経路で到達
  可能）。
- widget action は `view` target 上の `ppqn.*` prefix 名
  （`ppqn.select`/`ppqn.confirm`/`ppqn.cancel`）とし、shell が
  prefix を剥がして `syn.project.ppqn` へ `view_id`/`action` を
  shell 側で補完して mint する — widget spec が foreign view・
  別 action を名乗ることはできない。
- `syn.project.ppqn` を service に登録する — `params.action`
  ∈ select|confirm|cancel。`select` は `ppqn_select` +
  `ppqn_preview_pending` を 1 receipt に融合、`confirm` は
  preview 時 `expected_revision` CAS 付き `ppqn_confirm`、
  `cancel` は `ppqn_cancel`。wire `policy`/`rounding` 復号は
  `ppqn_policy_wire_decode`（`ppqn_policy_wire` の逆関数、
  `synvelis-workspace` ppqn_ui）が担う。
  （編集範囲記録: `crates/synvelis-app/src/service.rs` は当初の
  lease 対象外だったが、`ppqn_*` gesture に到達する登録 op が
  存在しないことが確定したため Coordinator 承認で占有追加 — 変更は
  `OpInfo` 1 件 + dispatch 1 arm のみ、handler は lease 内の
  `ui_ops.rs` に置く。）
- 不正 params（範囲外 ppqn・未知 policy/rounding・型違反・
  非 object params）は沈黙で丸めず `invalid_params` 拒否。
- headless fixture `crates/synvelis-app/tests/tim01_ppqn_widget.rs`
  が実 composition（mounted `ViewHost` → `rvf.ui.command.v1`
  → `NativeShell` → `AppService` → receipt → `MainScreenState`
  再投影）で widget → intent → gesture → op → receipt → spec
  再投影を証明する。既存 `tim01_ppqn_ui.rs` は非回帰。

## 受入基準（本 slice）

- 推奨値 Button の押下が 1 つの `rvf.ui.command.v1` `Action`
  （target `view`・action `ppqn.select`・params 宣言値 verbatim）
  を mint し、その command `Result` が `syn.project.ppqn` の
  verdict で settle する。
- `select` で pending candidate が記録され preview op が同一
  receipt 内に走り、`ppqn.status`/`ppqn.preview`/`ppqn.effects`/
  `ppqn.sites` が spec 再投影で追従する（live PPQN は不変）。
- `confirm` は `preview.applies` かつ preview 時 revision CAS を
  満たすときのみ `set_ppqn` を適用し、pending を消費する。
  pending 無し・rejected preview（`applies: false`）の confirm は
  implicit commit せず正直に reject する。
- `cancel` は pending を破棄するのみ（`canceled` 報告、pending
  無しでも成功）。
- pending 中の再 select は candidate を置き換える。
- forged/malformed params（範囲外・未知 policy/rounding・型違反・
  exact+rounding・非 object）は全て reject、pending を残さない。
- 実 Window/pixel/実 OS 操作・任意値入力 widget・SMF export
  division 連動・Presentation Driver 時刻正本移動は NotRun/
  Deferred 継続であり Pass 扱いしない。

## 非目標

- 実 GUI/実 OS/pixel 受入（`synvelis#114` + `ruviform#631` の
  外部 gate — headless Pass を実 OS Pass へ転用しない）。
- 任意値 1..=32767 の text/number input widget（ruviform-ui
  `ComponentSpec` に存在しない — 別 Issue 候補）。
- SMF export division の project PPQN 連動（別 slice、
  `app.rs` `project_export_smf`）。
- AC1–3 の再検証・再主張（main で検証済み — 本 slice は回帰
  維持のみ）。
- docs index（`docs/**/README.md`）の編集・再生成 — Coordinator
  が統合時に整備。

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/125
- 前提 slice: `docs/specs/tim-01-ppqn-ui.md`（`PpqnUi` + gesture
  helper + read-only surface）、`docs/specs/tim-01-ppqn-preview.md`
  （preview wire 契約）、`docs/specs/tim-01-app-ppqn.md`
  （`set_ppqn` wire 語彙）
- 前提 ruviform: `ruviform#944`（`ButtonSpec.params` → Action
  intent — merge 済み）
