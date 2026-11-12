---
id: "TASK-TiM01uIxYzAbCdEfGh4"
type: "task"
status: "done"
title: "TIM-01 — PPQN 選択 UI read model + Snap/Grid ポリシー"
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
    - "docs/plans/tim-01-ppqn-ui-plan.md"
  implemented-by:
    - "docs/impl/ir/tim-01-ppqn-ui.md"
  depends-on:
    - "docs/plans/tim-01-ppqn-ui-plan.md"
    - "docs/tasks/tim-01-ppqn-preview-task.md"
    - "docs/tasks/tim-01-task2-app-ppqn.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim-01-ppqn-ui-plan.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-ui.md"
  references:
    - "docs/specs/tim-01-ppqn-ui.md"
    - "docs/designs/tim-01-ppqn-ui-design.md"
  defers: []
  deferred-by: []
---

# TIM-01 — PPQN 選択 UI read model + Snap/Grid ポリシー

## Work

Issue #125 UI slice を headless/ViewSpec レベルで実装する。
workspace に view 所有の `PpqnUi` read model（current PPQN・
`1..=32767` 範囲・推奨値・preview 投影・select→preview→
confirm/cancel lifecycle）を追加し、Snap/Grid
`SnapUnit::Explicit` の PPQN 変更ポリシーを **tick 数
verbatim 維持（beat 間隔のみ変化を報告）** と決定して
`snap_unit_ppqn_effect` で実装。`synvelis-app` に Project
tab（`PpqnScreenState` → status/preview/effects/sites の
read-only 表示）と `ppqn_*` gesture helper（実
`syn.project.*` op 経路）・`ViewBinding` op mint を配線し、
headless fixture 11 件で証明する。実 GUI widget intent・
実機/製品 Oracle は対象外（NotRun）。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/tim-01-ppqn-ui.md`
- `docs/designs/tim-01-ppqn-ui-design.md`
- `docs/plans/tim-01-ppqn-ui-plan.md`
- `docs/tasks/tim-01-ppqn-preview-task.md`（preview op・done）
- `docs/tasks/tim-01-task2-app-ppqn.md`（`syn.project.
  set_ppqn` app slice・done）
- `docs/tasks/tim-01-configurable-ppqn.md`（headless Command
  slice・done）

## Dependencies

- `synvelis-project`: `preview_ppqn`/`PpqnChangePreview`/
  `PpqnPreviewSite`/`ppqn_in_range`/`PPQN_{MIN,MAX}`/
  `PpqnConversionPolicy`/`ProjectPpqn`（merge 済み・
  変更不要）
- `synvelis-workspace`: `SnapUnit`/`GridConfig`/`SnapConfig`/
  `ViewState`（`state.rs`/`snap.rs` lease 内）
- `synvelis-app`: `publish_project_length(&self)`・
  `UiOpsState`・`ViewBinding::push`・既存 `syn.project.*`
  dispatch（変更不要の経路を消費）

## Affected Files

- `crates/synvelis-workspace/src/ppqn_ui.rs` — 新規 read model
- `crates/synvelis-workspace/src/snap.rs` —
  `DEFAULT_SNAP_QUANTUM_TICKS`/`SnapPpqnEffect`/
  `snap_unit_ppqn_effect`
- `crates/synvelis-workspace/src/state.rs` — ポリシー明記 +
  暫定 `#[path]` mount + re-export
- `crates/synvelis-app/src/screens.rs` — `PpqnScreenState` +
  Project tab
- `crates/synvelis-app/src/app.rs` — publish/preview/set/
  new/open の gesture 統合 + preview JSON policy 報告
- `crates/synvelis-app/src/ui_ops.rs` — `UiOpsState.ppqn_ui`
  + `ppqn_*` helper + undo/redo re-sync
- `crates/synvelis-app/src/binding.rs` — op mint 4 件
- `crates/synvelis-app/tests/tim01_ppqn_ui.rs` — 新規
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  tim-01-ppqn-ui*` — doc chain 6 件

## Done When

- [x] `PpqnUi` が select→preview→confirm/cancel を view-local
      状態として完結し、confirm は `preview.applies` +
      preview 時 revision CAS のみ mint、cancel は非破壊
- [x] app `ppqn_*` helper が実 op 経路で gesture を完走し、
      live PPQN・`PpqnScreenState`・mounted spec が追従
- [x] undo/redo・project new/open・外部 session 変化で
      pending gesture が `observe_current`/`committed` で
      再同期される
- [x] Snap/Grid explicit 量子が PPQN 変更で tick 数を維持し、
      beat 間隔変化が preview JSON・screen spec に定量表示
      される（Auto は zoom-derived 報告）
- [x] 範囲外 select（0・32768）が view 側で reject され
      project 不変
- [x] `tim01_ppqn_ui.rs` 11 件 green・全検証 lane exit 0

## Verification

- [x] `cargo check -p synvelis-workspace -p synvelis-app
      --all-targets` — green
- [x] `cargo test -p synvelis-app --test tim01_ppqn_ui` —
      11 green
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
      `test-rt` / `check-app-native` / `lint-app-native` /
      `test-msrv` / `build` — 結果は impl IR に記録
