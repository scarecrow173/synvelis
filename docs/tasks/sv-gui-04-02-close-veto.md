---
id: "TASK-034Uqxo9HW4eRKeVH5w5PU"
type: "task"
status: "done"
title: "SV-GUI-04 slice2 タスク分解 — close-veto/cancel"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sv-gui-04-02-close-veto.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv-gui-04-02-close-veto.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sv-gui-04-02-close-veto.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-04-02-close-veto.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice2 タスク分解 — close-veto/cancel

## Work

SV#113 close-veto/cancel slice。`NativeShell::CloseRequested`
を decision query 経由に置き換え、末尾 live window の
dirty/recording close を `CloseDecision::Veto` で止める。

## Classification

- kind: `fix`（設計済み hook の実装 + fail-closed 強化）
- size: S–M
- risk: 低〜中（close 経路の regression が session 破棄に直結する
  ため、fail-closed と既存テスト非回帰を必須とする）

## Subtasks

- [x] app.rs: `close_veto_reasons()` — revision 比較 +
  `control().recording()` + `recorder.active()`。
- [x] service.rs: `syn.window.close_guard` 登録 + dispatch。
- [x] shell.rs: `close_decision`（last-window 判定 + 同期
  query + fail-closed）。module doc 更新。
- [x] tests/close_veto.rs: test-spec 全 case（EX-6 は op が
  infallible のため NotApplicable — impl record 記載）。
- [x] mise 全検証 + review-protocol + impl record。
- [ ] commit / push `feature/113-close-veto` / PR / worker_done。
