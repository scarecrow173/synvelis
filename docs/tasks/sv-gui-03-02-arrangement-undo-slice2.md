---
id: "TASK-cvXxPcl14NogBTKEDTSKrO"
type: "task"
status: "in-progress"
title: "SV-GUI-03 slice2 タスク分解 — Arrangement read-only Table + Undo/Redo"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/plans/sv-gui-03-02-arrangement-undo-slice2.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv-gui-03-02-arrangement-undo-slice2.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-03-02-arrangement-undo-slice2.md"
    - "docs/designs/sv-gui-03-02-arrangement-undo-slice2-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-03-02-arrangement-undo-slice2.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice2 タスク

| # | タスク | 完了条件 |
| --- | --- | --- |
| T1 | `arrangement_ui.rs` + `timeline_items` pub(crate) + lib export + workspace test | `arrangement_rows` が track→lane→clip 順で name/source/start/end/muted を返す単体テスト通過 |
| T2 | `screens.rs`: Arrangement tab Table + Undo/Redo Button + `MainScreenState.arrangement_rows` | spec に Arrangement tab・Table columns・2 button が含まれる |
| T3 | `app.rs`/`ui_ops.rs`/`binding.rs`: publish 拡張 + op 関数 + action マッピング | service op 経由で rows 更新・undo/redo 呼出 |
| T4 | `service.rs` op 登録（承認時のみ） | `syn.project.undo`/`redo` が OPS に存在し dispatch される |
| T5 | `view_mount.rs`/`ui_ops.rs` テスト拡張 | rows mount・revision push・Undo/Redo 経路の headless 検証 |
| T6 | 検証一式 + `run-app-native` 実機確認 | mise タスク全緑 + 実 GPUI window 証拠 |
