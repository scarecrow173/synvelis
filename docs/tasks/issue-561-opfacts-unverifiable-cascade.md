---
id: "TASK-561OpFactsUnverifiable05"
type: "task"
status: "done"
title: "Issue #561 tasks — op_facts unverifiable cascade 修正 slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/561"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-561-opfacts-unverifiable-cascade.md"
  implemented-by:
    - "docs/impl/ir/issue-561-opfacts-unverifiable-cascade.md"
  depends-on:
    - "docs/plans/issue-561-opfacts-unverifiable-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-557-adopt-facts.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-561-opfacts-unverifiable-cascade.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #561 tasks — op_facts unverifiable cascade 修正 slice

| # | タスク | 状態 | 証拠 |
| --- | --- | --- | --- |
| 1 | `op_facts` 3 arm + helper 実装（`prepare.rs`） | done | `docs/impl/ir/issue-561-opfacts-unverifiable-cascade.md` |
| 2 | wire pin test 6 件（`tests/prepare.rs`、red→green） | done | `evidence/sv561/reproduction.md`・`verification.md` |
| 3 | domain pin test 4 件（`sv561_unobservable_cascade.rs`） | done | `docs/test-specs/issue-561-opfacts-unverifiable-cascade.md` |
| 4 | review 対応: `monitoring.tracks` decl 検査追加 | done | `prepare.rs` RemoveTrack arm・`sv561_track_remove_monitor_decl_denied` |
| 5 | review 対応: Composite arm の逐次 scratch 評価 | done | `prepare.rs` Composite arm・`sv561_batch_seal_release_unobservable_denied` |
| 6 | review 対応: depth_source/assignment-leg/batch pin test 追加 | done | `tests/prepare.rs` 4 件追加・domain pin 3 件追加 |
| 7 | doc chain + evidence 整備 | done | `docs/`・`evidence/sv561/` |
| 8 | mise 検証一式（check/lint/test/fmt/headless-deps/prek） | done | `evidence/sv561/verification.md` |
| 9 | review-protocol レビュー・commit・push・PR | done | PR 本文 |
