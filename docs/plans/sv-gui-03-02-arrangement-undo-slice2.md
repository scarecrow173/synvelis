---
id: "PLAN-ATb2LKnqsahGgDlMVeHEIX"
type: "plan"
status: "approved"
title: "SV-GUI-03 slice2 実施計画 — Arrangement read-only Table + Undo/Redo"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gui-03-02-arrangement-undo-slice2.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-03-02-arrangement-undo-slice2-design.md"
    - "docs/adr/sv-gui-03-02-arrangement-readmodel-undo-ops.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-03-02-arrangement-undo-slice2.md"
    - "docs/designs/sv-gui-03-02-arrangement-undo-slice2-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-03-02-arrangement-undo-slice2.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice2 実施計画

## 前提確認

- slice 1（PR #199 / `009291d`）マージ済み。
- `Project.tracks/lanes/clips` pub、`timeline_items`、
  `ProjectSession::undo/redo`、`ControlsUi::undo/redo`、
  `project_action_op` project-target 経路は既存。
- `service.rs` 編集の可否は Coordinator 確認中。未承認なら
  Undo/Redo は本 PR から切り分け、PR 本文に明記する。

## 手順

1. **workspace read model**: `arrangement_ui.rs` 新規
   （`ArrangementRow` + `arrangement_rows`）、`viz_ui.rs` の
   `timeline_items` を `pub(crate)` 化、`lib.rs` export、
   workspace tests。
2. **screens**: `MainScreenState.arrangement_rows`、
   Arrangement tab + `Table`（`selection: None`）、toolbar に
   Undo/Redo `Button`（`project` target）。
3. **app**: `publish_project_length` を project 由来 state
   （length + arrangement rows）publish に拡張、`ui_ops` に
   `op_project_undo`/`op_project_redo`、`binding.rs` の
   `project_action_op` に `"undo"`/`"redo"`。
4. **service**（承認時）: `OPS` + dispatch に
   `syn.project.undo`/`syn.project.redo` 追加。
5. **テスト**: `view_mount.rs`（Arrangement mount・revision
   push・Undo/Redo op 経路）、`ui_ops.rs`（op テスト）、
   workspace `arrangement_rows` 単体テスト。
6. **検証**: fmt-check / check / lint / test / build +
   app-native check/lint/build + `run-app-native` 実機確認。
7. **統合**: `git merge origin/main` → 最終検証 → review-protocol
   → commit/push/PR/worker_done。

## 判断ポイント

- `service.rs` 未承認 → 手順 3 の op 関数は残し、binding の
  `"undo"`/`"redo"` マッピングと service 登録のみ外す
  （Button は出すが op 未登録 → dispatch が unknown op を返す。
  その場合は Button 自体も出さない方が誠実 — その時点で判断）。
