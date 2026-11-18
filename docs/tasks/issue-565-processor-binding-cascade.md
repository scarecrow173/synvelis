---
id: "TASK-565ProcessorBindingCascade05"
type: "task"
status: "done"
title: "Issue #565 tasks — 退役 cascade 監査 slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/565"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-565-processor-binding-cascade.md"
  implemented-by:
    - "docs/impl/ir/issue-565-processor-binding-cascade.md"
  depends-on:
    - "docs/plans/issue-565-processor-binding-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-561-opfacts-unverifiable-cascade.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-565-processor-binding-cascade.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #565 tasks — 退役 cascade 監査 slice

| # | タスク | 状態 | 証拠 |
| --- | --- | --- | --- |
| 1 | 退役経路の監査（不変条件 3 軸照合） | done | `docs/discovery/issue-565-processor-binding-cascade.md`・`evidence/sv565/audit.md` |
| 2 | 契約明記コメント（`prepare.rs`・`prepare_engine.rs`・`ops.rs`） | done | 各ファイルのコメント差分 |
| 3 | op 経路 pin 3 件（`plugin_instance_persist.rs`） | done | `docs/test-specs/issue-565-processor-binding-cascade.md` |
| 4 | wire pin（`tests/prepare.rs` — Write detail + candidate store） | done | 同上 |
| 5 | `PluginStateProbe` staleness の defer 判定記録 | done | `docs/impl/ir/issue-565-processor-binding-cascade.md` 残件節 |
| 6 | doc chain + evidence 整備 | done | `docs/`・`evidence/sv565/` |
| 7 | mise 検証一式（check/lint/test/fmt/headless-deps/prek） | done | `evidence/sv565/verification.md` |
| 8 | review-protocol レビュー・commit・push・PR | done | PR 本文 |
