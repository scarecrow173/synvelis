---
id: "TASK-rVJcKAC3jQHu6ZFC0SvSV9"
type: "task"
status: "done"
title: "Issue #514 tasks — 判定記録 slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/514"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-514-session-engine-selection.md"
  implemented-by:
    - "docs/impl/ir/issue-514-session-engine-selection.md"
  depends-on:
    - "docs/plans/issue-514-session-engine-selection.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-514-session-engine-selection.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-514-session-engine-selection.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #514 tasks

- [x] T1: root-cause 調査（rebuild 経路・`SelectionInputs` 供給可能性・
      pin 所在・consumer 確認）→ discovery 記録
- [x] T2: 判定確定（分岐 B: 実装不要）→ ADR 記録
- [x] T3: doc chain 作成（discovery/spec/design/plan/task/test-spec/
      adr/impl-ir/impl-exp）
- [x] T4: `engine.rs` module doc に selection 非消費が意図的である旨を
      明記
- [x] T5: AC4 — stateless `PluginStateDecl` の帰属確定（player 契約変更
      が必要 → 別 Issue 起票 + evidence 記録）
- [x] T6: `mise` 必須 lane 実行（fmt-check・check/lint/test
      synvelis-app・check/lint/test synvelis-workspace）
- [x] T7: `evidence/sv514/` ledger + verification 記録
- [x] T8: review-protocol 1 cycle・blocker 0
- [x] T9: commit・push・PR 作成（`Refs #514`）・worker_done
