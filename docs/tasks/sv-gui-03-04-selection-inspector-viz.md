---
id: "TASK-Wq3nBs6yLv8pRt5wXe2kFg"
type: "task"
status: "in-progress"
title: "SV-GUI-03 slice4 タスク分解 — Arrangement selection→Inspector + Meter/Waveform viz mount"
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
    - "docs/plans/sv-gui-03-04-selection-inspector-viz-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv-gui-03-04-selection-inspector-viz-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-04-selection-inspector-viz.md"
    - "docs/designs/sv-gui-03-04-selection-inspector-viz-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-03-04-selection-inspector-viz.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice4 タスク

- [x] `screens.rs`: `TableSelectionSpec` mount、viz screen state 3
      型、`waveform_panel`/`meter_panel`、`MainScreenState.viz`、
      module doc 更新
- [x] `binding.rs`: `ViewCommand.action_params` 追加
- [x] `shell.rs`: `TABLE_SELECT_ACTION` → `dispatch_view_select` →
      `syn.view.select` routing
- [x] `service.rs`: `syn.view.select` OpInfo + dispatch 登録
- [x] `ui_ops.rs`: `op_view_select`、viz reconcile/capture、
      inspector entity readout、meter history、`op_view_list`
      fields、detach/close cleanup
- [x] `lib.rs`: 新規 pub 型の export（確認・必要時追加）
- [ ] `tests/view_mount.rs`: selectable spec、select→Inspector、
      syn.view.list、fallback、viz mount/status/data、reject/
      rollback、範囲外拒否
- [ ] `tests/ui_ops.rs` または unit: `op_view_select` param 検証・
      screen state 投影
- [ ] docs index 再生成（必要時）
- [ ] mise 検証一式（fmt-check/check/lint/test/test-rt/test-msrv/
      check-app-native/lint-app-native/build-app-native）
- [ ] origin/main merge → 最終検証 → push → PR（`Refs #112`）
