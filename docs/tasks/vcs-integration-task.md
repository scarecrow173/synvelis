---
id: "TASK-034X9GCGPXdCl5ePe8gJ14"
type: "task"
status: "done"
title: "VCS Integration 統合契約文書チェーン + evidence/sv7 Case 被覆台帳 (synvelis#7)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/7"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/vcs-integration-plan.md"
    - "docs/specs/vcs-integration.md"
  implemented-by:
    - "docs/impl/ir/vcs-integration.md"
  depends-on:
    - "docs/plans/vcs-integration-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/vcs-integration-design.md"
    - "docs/test-specs/vcs-integration.md"
    - "docs/impl/exp/vcs-integration.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-integration.md"
  references:
    - "evidence/sv7/vcs-integration-case-ledger.md"
    - "evidence/sv7/vcs-integration-verification.md"
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
  defers: []
  deferred-by: []
---

# VCS Integration 統合契約文書チェーン + evidence/sv7 Case 被覆台帳 (synvelis#7)

## Work

Issue #7 の統合契約を文書チェーン（spec/design/plan/task/test-spec/
impl-ir/impl-exp）として作成し、`evidence/sv7/` に受入 Case 被覆
台帳（case-ledger）と検証記録（verification）を作る docs-only
スライス。実装コード・CI・本番 crate は変更しない。既存 VCS slice
（vcs-01〜vcs-06）spec を正本として参照し、責務対応・層構成・
Case ID 被覆・verdict 誠実性・8 view 採否・Creative Branch 分離・
sv#95 整合を記録する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/vcs-integration-plan.md`
- `docs/specs/vcs-integration.md`
- `docs/designs/vcs-integration-design.md`
- `docs/test-specs/vcs-integration.md`
- Issue #7 / #97（GitHub）

## Dependencies

- 既存 `docs/specs/vcs-01-*.md`〜`vcs-06-*.md` と
  `evidence/reports/vcs-06-*.md`（read-only 正本）
- 実装 Owner: #92/#93/#94/#95/#96、統合 Gate: #97

## Affected Files

- `docs/specs/vcs-integration.md`（新規）
- `docs/designs/vcs-integration-design.md`（新規）
- `docs/plans/vcs-integration-plan.md`（新規）
- `docs/tasks/vcs-integration-task.md`（本書）
- `docs/test-specs/vcs-integration.md`（新規）
- `docs/impl/ir/vcs-integration.md`（新規）
- `docs/impl/exp/vcs-integration.md`（新規）
- `evidence/sv7/vcs-integration-case-ledger.md`（新規）
- `evidence/sv7/vcs-integration-verification.md`（新規）
- `docs/{specs,designs,plans,tasks,test-specs}/README.md`（index 再生成）
- `docs/impl/{ir,exp}/README.md`（sync_impl_index 再生成）

## Done When

- [x] 7 文書が front matter・relations・status 整合で存在する
- [x] `evidence/sv7/vcs-integration-case-ledger.md` が基底 Case ID 全件 + 拡張行を
  owner/依存/fixture/layer/requirement state/result state/evidence
  path 付きで列挙する
- [x] `evidence/sv7/vcs-integration-verification.md` が実行した検証を記録する
- [x] Issue #7 の 14 受入条件すべてに対応する Case ID・証拠・
  設計記録が文書上で追跡可能
- [x] 8 view 採否・Creative Branch 分離・sv#95 整合が design に記録
- [x] verdict が vcs-06 ledger の実測を転記し NotRun/Unsupported を
  維持する

## Verification

- [x] `mise run fmt-check` exits 0
- [x] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/specs`（他 6 dir も同様）が drift を報告しない
- [x] `node .agents/skills/impl-doc/scripts/sync_impl_index.js --kind ir` / `--kind exp` が in-sync を返す
- [x] `node .agents/skills/doc-status/scripts/doc_status.js lint` が新規文書に error を報告しない
- [x] `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json` / `audit_experiment_log.js --json` が本 slice 起因の error を報告しない（既存ファイルの pre-existing error は範囲外として verification.md に記録）
- [x] `review-protocol` で blocker が残っていない
