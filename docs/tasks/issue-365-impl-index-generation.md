---
id: "TASK-034WsBdOQFzQABNIgCwFPl"
type: "task"
status: "done"
title: "Issue #365 impl index 生成可能化 — front matter 是正と index 再生成"
created: "2026-11-12"
updated: "2026-11-13"
owners: []
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
    - "docs/plans/issue-365-impl-index-generation-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-365-impl-index-generation-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-365-impl-index-generation.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #365 impl index 生成可能化 — front matter 是正と index 再生成

## Work

`docs/impl/ir` の schema 違反 44 件の front matter を形式修正し、
`docs/impl/ir/README.md`・`docs/impl/exp/README.md` に generated-index
marker を追加したうえで、`doc-maintenance` の `index apply` で両 index
を再生成して `in-sync` にする。既存文書の本文・ファイル名・id は
変更しない。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/issue-365-impl-index-generation-plan.md`
- `docs/specs/issue-365-impl-index-generation.md`
- `docs/designs/issue-365-impl-index-generation-design.md`
- `docs/test-specs/issue-365-impl-index-generation.md`

## Dependencies

- なし（plan 承認済み）

## Affected Files

- `docs/impl/ir/*.md`（schema 違反 44 件の front matter）
- `docs/impl/ir/README.md`、`docs/impl/exp/README.md`（marker 追加 + 再生成）
- `docs/*/README.md`（新規 doc-chain 文書を含む dir の生成 index）
- `docs/impl/ir/<本タスクの ir>.md`（Implementation Record）

## Done When

- [x] `index plan --dir docs/impl/ir` が `rejected` でなく、
  `index apply` 後に `in-sync`
- [x] `docs/impl/exp` も同様に `in-sync`
- [x] 引数なし `index apply` が exit 0
- [x] 既存文書の本文・ファイル名・id が不変
- [x] audit error 数がベースライン（ir 331 / exp 52）を超えない

## Verification

- [x] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/impl/ir` が `rejected` でない
- [x] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply` が exit 0
- [x] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/impl/exp` が `in-sync`
- [x] `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` の error ≤ 331
- [x] `node .agents/skills/impl-doc/scripts/audit_experiment_log.js --json` の error ≤ 52
- [x] `git diff` で既存 ir/exp 文書の本文・ファイル名・id に差分がない
