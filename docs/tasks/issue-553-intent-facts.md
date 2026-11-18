---
id: "TASK-553IntentFacts06"
type: "task"
status: "done"
title: "Issue #553 tasks — intent_facts 伝播 slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/553"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-553-intent-facts.md"
  implemented-by:
    - "docs/impl/ir/issue-553-intent-facts.md"
  depends-on:
    - "docs/plans/issue-553-intent-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-550-strip-routing-ops.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-553-intent-facts.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #553 tasks — intent_facts 伝播 slice

- [x] T1: `cb06_strip_gateway.rs` — modulated send fixture
      の `seed_strip_send_modulated_store`/
      `boot_strip_send_modulated` helper と
      `cb06_strip_553_variant_edit_send_remove_
      unobservable_cascade_denied` test を追加
      （修正前 red を実測確認）
- [x] T2: `variant.rs` — `declared_facts` に
      preconditions/unsupported accumulator を追加し、
      Edit arm の `BranchEditOp::Project` inner facts を
      伝播（doc comment 更新）
- [x] T3: doc chain（本ファイル群）+ cb-06 doc 3 件
      追補 + `evidence/sv553/` 新設
- [x] T4: `review-protocol` レビュー・blocker 0 確認
- [x] T5: 検証コマンド実行・commit・push・PR・
      worker_done
