---
id: "TASK-034Wv2LV1SZLdyPnE8fyZD"
type: "task"
status: "done"
title: "Issue #368 docs/impl/ir missing-section 本文補完"
created: "2026-11-13"
updated: "2026-11-13"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/368"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-368-ir-missing-sections-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-368-ir-missing-sections-plan.md"
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
    - "docs/test-specs/issue-368-ir-missing-sections.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #368 docs/impl/ir missing-section 本文補完

## Work

`docs/impl/ir/` の missing-section 保持 60 文書について、audit が
要求する必須 `##` セクション（Summary / Context / Implementation /
Related Experiments / Validation / Risks / Follow-ups）を本文に
補完する。既存本文が正規名以外の見出しにある場合は内容を保持した
まま正規セクションへ再配置・改題し、根拠の無い記述は上流文書で
事実確認してから書く。front matter とファイル名は変更しない。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/issue-368-ir-missing-sections-plan.md`
- `docs/specs/issue-368-ir-missing-sections.md`
- `docs/designs/issue-368-ir-missing-sections-design.md`
- `docs/test-specs/issue-368-ir-missing-sections.md`

## Dependencies

- なし（plan 承認済み。Issue #365 / PR #366 は統合済み）

## Affected Files

- `docs/impl/ir/*.md`（missing-section 保持の 60 文書。本文のみ）
- `docs/specs|designs|plans|tasks|test-specs/issue-368-*.md`（新規）
- `docs/impl/ir/issue-368-*.md`（本タスクの Implementation Record）
- `docs/*/README.md`（新規文書を含む dir の生成 index）

## Done When

- [x] `audit_impl_record.js --json` の `missing-section` が 0 件
- [x] 追加した全セクションに実装・検証内容に基づく具体的な本文がある
- [x] 60 文書の front matter・ファイル名が不変
- [x] `missing-experiment-link` ≤ 18・`broken-relation-link` ≤ 36
- [x] `index plan --dir docs/impl/ir` が `in-sync`

## Verification

- [x] `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json`
- [x] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index plan --dir docs/impl/ir`
- [x] `git diff` で front matter・ファイル名に差分がないことを確認
