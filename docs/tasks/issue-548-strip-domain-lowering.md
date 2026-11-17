---
id: "TASK-548StripDomLowr06"
type: "task"
status: "done"
title: "Issue #548 tasks — strip-domain lowering slice"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/548"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-548-strip-domain-lowering.md"
  implemented-by:
    - "docs/impl/ir/issue-548-strip-domain-lowering.md"
  depends-on:
    - "docs/plans/issue-548-strip-domain-lowering.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-548-strip-domain-lowering.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-548-strip-domain-lowering.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #548 tasks — strip-domain lowering slice

- [x] T1: `prepare_engine.rs` — `op_table` に strip-domain
      capability descriptor 5 件 + input schema 登録
- [x] T2: `prepare.rs` — import・target helper・lower fn 5 件・
      `lower_for`/`op_name`/`op_facts` arms 追加
- [x] T3: `cb06_strip_gateway.rs` — module doc 更新・op builder
      追加・positive mask-gated edit 試験 2 件・負側維持
- [x] T4: `tests/prepare.rs` — ordinary prepare 試験 3 件追加
- [x] T5: doc chain（本ファイル群）+ cb-06 doc 3 件追従更新
- [x] T6: `evidence/sv548/` 作成 + `evidence/sv283/` 追記
- [x] T7: `review-protocol` レビュー・blocker 0 確認
- [x] T8: 検証コマンド実行・commit・push・PR・worker_done
