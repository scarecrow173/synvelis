---
id: "PLAN-Jh8rTy5xKw9nQp3vLc7zMe"
type: "plan"
status: "approved"
title: "SV-GUI-03 slice4 実施計画 — Arrangement selection→Inspector + Meter/Waveform viz mount"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gui-03-04-selection-inspector-viz.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-03-04-selection-inspector-viz-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-03-04-selection-inspector-viz.md"
    - "docs/designs/sv-gui-03-04-selection-inspector-viz-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-03-04-selection-inspector-viz.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice4 実施計画

## 前提確認

- slice1/2/3 は main 統合済み（slice3 は PR #240）。本 slice は
  Arrangement selection→Inspector 接続と Meter/Waveform mount の
  みで、既存 Transport/Inspector/Arrangement/Mixer 要素の契約を
  変更しない。
- `WorkspaceManager::select`/`inspect`、`ObjectSelections`、
  `InspectionFocus`、`SemanticTarget`、`EditingSelection::single`、
  `waveform_clip_target`/`meter_master_target`、`syn.viz.*` op、
  `ViewHost::select_row`/`TableSpec.selection` は全て pinned rev・
  workspace に既存。新規 domain op・新規 crate 変更なし。
- `doc-driven-dev-graph` router は main の既存 broken-relation で
  blocked を返すため、slice1-3 と同じ手動 doc chain で進める
  （本書がその記録）。

## 手順

1. `screens.rs`: `TableSelectionSpec` mount、`WaveformScreenState`/
   `MeterScreenState`/`VizScreenState`、`waveform_panel`/
   `meter_panel`、`MainScreenState.viz`、module doc 更新。
2. `binding.rs`: `ViewCommand.action_params` 追加（view-local
   target のみ params 許容の既存規則を維持）。
3. `shell.rs`: `TABLE_SELECT_ACTION` → `dispatch_view_select` →
   `syn.view.select` op routing。
4. `service.rs`: `syn.view.select` OpInfo + dispatch 登録。
5. `ui_ops.rs`: `op_view_select`、`reconcile_viz_surfaces`/
   `desired_waveform_source`、`capture_viz_state`/
   `waveform_screen_state`/`meter_screen_state`、
   `semantic_target_tag`/`semantic_inspector_rows`/
   `viz_status_text`、`capture_screen_state` の inspector
   rebinding + `screen.viz`、`push_surface_frames` の meter
   history、`op_view_list` の selection/inspection、detach/close
   の `meter_levels` cleanup。
6. `lib.rs`: 新規 pub 型 export。
7. `tests/view_mount.rs`: selectable spec、select→ObjectSelections
   →Inspector、syn.view.list、fallback、viz mount/status/data、
   reject/rollback、範囲外拒否の各ケース。
8. docs index 再生成（doc-maintenance rebuild-index 相当、必要時）。
9. mise canonical 検証一式を実行し、失敗はスコープ内で修正。
10. `git fetch origin main && git merge origin/main` 後に最終検証、
    Conventional Commits で論理分割して push、PR 作成
    （本文日本語・`Refs #112`）。

## 判断ポイント

- selection を `syn.view.select` op 経由にした理由: app は shell
  内で `AppService` に seal され、view action から workspace
  `select`/`inspect` を直接呼ぶ経路が無い。view-local params を
  許すのは `VIEW_STATE_TARGET` のみという decoder 規則が既に
  あり、そこに `syn.view.select` を足すのが最小の整合変更。
- Inspector を `inspection_focus` 駆動にし `object_selections` と
  分離したままにした理由: 両者は workspace で意図的に別 state
  （select=編集選択、inspect=観測 focus）。表示だけ inspection に
  追随させ、link propagation・編集 semantics を壊さない。
- waveform desired source を「選択 clip → 先頭 clip → unbound」
  にした理由: 空 project で phantom source を bind せず、
  selection 前でも決定的な fallback を表示できる。unbound は
  port 無し id で `NoSource` を正直に出す。
- meter level history を app 内部 `VecDeque` にした理由:
  `Meter` spec は resolved bins のみを運び、query 契約を持たない。
  landed `MeterFrame` の bounded 蓄積が唯一の正直な供給源で、
  project state ではなく表示キャッシュとして detach/close で
  破棄する。
