---
id: "TASK-ldDyN5a7IH0RlM2Ys4UN"
type: "task"
status: "done"
title: "Issue #530 tasks — stateless PluginStateDecl 契約 slice"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/530"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-530-plugin-state-decl.md"
  implemented-by:
    - "docs/impl/ir/issue-530-plugin-state-decl.md"
  depends-on:
    - "docs/plans/issue-530-plugin-state-decl.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-530-plugin-state-decl.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-530-plugin-state-decl.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #530 tasks

- [x] T1: root-cause 調査（`PluginStateDecl`/`StateRestoreSupport`/
      `verify_plugin`/`rcp_selection.rs`/既存 oracle）→ discovery 記録
- [x] T2: 契約表現の設計確定（既存語彙再利用: `stateless()`
      predicate + `Unsupported` dual-use、variant/field 追加なし）
      → design + ADR 記録
- [x] T3: doc chain 作成（discovery/spec/design/plan/task/
      test-spec/adr/impl-ir/impl-exp + exp.jsonl）
- [x] T4: `model.rs` — `PluginStateDecl::stateless()` 追加 +
      `PluginStateDecl`/`StateRestoreSupport::Unsupported` doc 更新
- [x] T5: `verify.rs` — `Unsupported` arm に stateless→Live 経路
      追加 + verdict doc 更新
- [x] T6: `rcp_selection.rs` — `ProcessorPluginEvidence.state` doc
      追従
- [x] T7: oracle 追加 — `rcp_04.rs`（profile 単体・verdict matrix・
      selection Live/Blocked）+ `rcp_selection_integration.rs`
      （stateless evidence 経路・未 capture fail-closed）
- [x] T8: `mise` 必須 lane 実行（fmt-check・check/lint/test
      synvelis-player・check/lint/test synvelis-workspace）
- [x] T9: `evidence/sv530/` ledger + verification 記録
- [x] T10: review-protocol self-review 1 cycle・blocker 0
- [x] T11: `origin/main` merge・commit・push・PR 作成
      （`Refs #530`）・worker_done
