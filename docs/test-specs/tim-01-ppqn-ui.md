---
id: "TSPEC-TiM01uIxYzAbCdEfGh5"
type: "test-spec"
status: "approved"
title: "TIM-01 PPQN 選択 UI + Snap/Grid ポリシー — test spec"
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
  implements: []
  implemented-by:
    - "crates/synvelis-app/tests/tim01_ppqn_ui.rs"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim-01-ppqn-preview.md"
    - "docs/test-specs/tim-01-app-ppqn.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/tim-01-ppqn-ui-design.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-01-ppqn-ui.md"
  verified-by: []
  references:
    - "docs/tasks/tim-01-ppqn-ui-task.md"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN 選択 UI + Snap/Grid ポリシー — test spec

## Purpose

Issue #125 UI slice の `PpqnUi` read model・app `ppqn_*`
gesture helper・Project tab ViewSpec・Snap/Grid
explicit-tick-preservation ポリシーが spec の受入基準を
満たすことを `crates/synvelis-app/tests/tim01_ppqn_ui.rs`
（headless `UiRuntime::dispatch_op` + `SynvelisApp::ppqn_*`
経路）で固定する。preview/set の wire 契約網羅は
`docs/test-specs/tim-01-ppqn-preview.md` /
`docs/test-specs/tim-01-app-ppqn.md` が担う。実 GUI・
実 OS/実機・製品 Oracle は対象外のため、本 spec が退役
するのは UI slice が上位 slice に統合された時点。

## Feature

view 所有の `PpqnUi` が select→preview→confirm/cancel を
状態機械として完結し、app helper が実 `syn.project.*`
op でそれを駆動、mounted spec の Project tab が
current/pending/preview/effects/sites を投影し、
Snap/Grid explicit 量子が PPQN 変更で tick 数を維持
する（beat 間隔のみ変化を報告）。

## Rules

- `PpqnUi::select` は `1..=32767` 外を view 側で reject
  し、pending 確立前に状態を変えない。
- `record_preview` は pending candidate の `new_ppqn` と
  live `old_ppqn` が一致する preview のみ記録 —
  不一致は `PreviewMismatch` で confirm を arm しない。
- `confirm_request` は `preview.applies` のときのみ
  `expected_revision`（preview 時 revision）を同梱した
  `syn.project.set_ppqn` params を返す。
- `cancel` は pending を破棄するのみで project/revision
  を変更しない。
- `observe_current`/`committed` は live PPQN の外部変化
  （undo/redo/open/new/fixture apply）で pending を
  invalidate/消費する。
- `snap_unit_ppqn_effect` は `SnapUnit::Explicit` の
  tick 数を維持したまま beat 間隔 before/after を報告し、
  `SnapUnit::Auto` は固定量子なし（zoom-derived）を報告。
- mounted spec の PPQN 表示は read-only（Label/Table、
  `selection: None`、action なし）— gesture 駆動は
  `SynvelisApp::ppqn_*` helper と `ViewBinding` op mint。

## Examples

- `read_model_projects_current_range_and_preview`:
  Given `PpqnUi::new(960)`、When select(480, Exact) →
  実 `preview_ppqn` を `record_preview`、Then phase が
  Idle→Selected→Previewed と遷移し、`PpqnPreviewView` が
  old/new・site_count 2・applies・snap_grid
  "960 ticks: 1.000 → 2.000 beats"・`confirm_request` が
  `expected_revision` 同梱 params を返す。
- `read_model_rejects_mismatched_and_stale_previews`:
  pending 無しの `record_preview` は `NoPending`、
  別 candidate（480 pending に 1920 preview）は
  `PreviewMismatch` で phase Selected のまま confirm
  不可。
- `app_gesture_projects_preview_into_screen_state`:
  `ppqn_select` + `ppqn_preview_pending` で実 op が走り、
  `ppqn_ui().phase() == Previewed`、`screen.ppqn` が
  current 960・pending 480/exact・revision 一致 preview
  を持つ。live PPQN は 960 のまま（非破壊）。
- `confirm_applies_set_ppqn_and_clears_gesture`:
  preview 後の `ppqn_confirm` が `syn.project.set_ppqn`
  を適用し live PPQN 480・gesture Idle・screen state が
  新 PPQN に追従。
- `cancel_drops_pending_without_mutating`: preview 済み
  pending を `ppqn_cancel` で破棄 — live PPQN/revision
  不変・phase Idle・2 回目の cancel は false。
- `confirm_requires_applicable_preview`: pending 無し・
  未 preview・foreign preview・`applies: false`（exact
  over inexact sites）の全てで confirm が
  `invalid_params` reject・project 不変。`Round(Floor)`
  で同 candidate が applies → confirm で 1000 適用。
- `undo_restores_previous_ppqn_and_resyncs_gesture`:
  confirm 480 → 新規 pending 1920 select → `syn.project.
  undo` で 960 復帰・gesture Idle 再同期・redo で 480。
- `select_rejects_out_of_range_values`: select(0)/
  select(32768) が `OutOfRange` で reject・phase 不変、
  境界 1/32767 は select 可能。
- `snap_grid_policy_preserves_explicit_tick_quanta`:
  default `SnapConfig`/`GridConfig`/`ViewState` が
  `Explicit(960)` を保持、`snap_unit_ppqn_effect
  (Explicit(960), 960, 480)` が tick 960 維持・
  "960 ticks: 1.000 → 2.000 beats"、Auto は
  "zoom-derived (auto)"。
- `preview_and_spec_report_snap_grid_policy`:
  `syn.project.preview_ppqn` の `effects.snap_grid` が
  "preserved" + beat 量子化を含み、gesture 後の spec が
  `ppqn.status`（PPQN 960・1..=32767・推奨値）/
  `ppqn.preview`（960 → 480）/`ppqn.effects`（snap/grid
  + SMF export）/`ppqn.sites`（read-only table）を描画。
- `project_tab_renders_ppqn_surface`: default state の
  spec が `tab.project.panel` と全 PPQN component を持ち、
  status label が "PPQN 960 — selectable 1..=32767 —
  recommended 480, 960, 1000, 1920"、preview label が
  "no pending selection"、sites table が空・
  `selection: None`。

## Guarantees

- gesture の全 mutation は `syn.project.set_ppqn` の
  double CAS + session gate を経由し、preview 無し・
  stale preview・rejected preview の confirm は
  view 側で arm されない。
- cancel・select・preview は project/revision/engine
  notification を変更しない（非破壊）。
- Snap/Grid explicit 量子の tick 数は PPQN 変更を通じて
  verbatim 維持され、変化は beat 間隔の報告のみ —
  保存値の書き換えは存在しない。
- spec の PPQN 表示は `publish_project_length` の
  revision guard で stale preview を落とし、gesture
  authority を持たない。

## Non-goals

- 実 GUI widget intent・parameterized Action・実
  OS/実機・製品 Oracle・Presentation Driver 時刻正本
  移動。
- SMF export 実装変更・`synvelis-project` PPQN コマンド
  意味変更。
- `synvelis-workspace/src/lib.rs`・`synvelis-app/src/
  lib.rs` の export 整理（coordinator 統合）。

## Risk

- `PpqnUi` の `current_ppqn` が外部 mutation で drift
  する経路（fixture `session.apply` 等）— `publish_
  project_length` の `observe_current` が唯一の re-sync
  点として全経路を網羅する設計で担保し、fixture で
  undo/new/open 経路を検証。
- preview の `project_revision` と confirm の
  `expected_revision` が乖離する境界 — 二重 guard
  （screen 表示 + CAS）で stale confirm は正直に
  reject されることを fixture で確認。
