---
id: "TASK-034X6iTAb53abA44E2sAPp"
type: "task"
status: "in-progress"
title: "Project Model Task — Issue #10 design contract + headless verification"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/10"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/project-model.md"
  implemented-by: []
  depends-on:
    - "docs/plans/project-model.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/project-model.md"
    - "docs/specs/project-model.md"
    - "docs/designs/project-model-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/project-model.md"
  references: []
  defers: []
  deferred-by: []
---

# Project Model Task — Issue #10 design contract + headless verification

## Work

1. `docs/specs/project-model.md`・`docs/designs/project-model-design.md`・
   `docs/plans/project-model.md`・`docs/tasks/project-model-task.md`・
   `docs/test-specs/project-model.md` の 5 文書チェーンを確定する。
2. `docs/impl/ir/project-model.md`・`docs/impl/exp/project-model.md` を作成し、
   実施内容・試行・結論を記録する。
3. `evidence/sv10/project-model-case-ledger.md` に PM-AC-01〜13 → PM-T case
   → 既存 `synvelis-project` test/NotRun の写像を台帳化する。
4. `evidence/sv10/project-model-verification.md` に実行した検証
   （`mise run fmt-check`・doc lint・index・audit・review-protocol）と
   baseline finding・NotRun lane を記録する。
5. 触れた docs dir の index を `doc_maintenance.js index apply` で再生成する。
6. review-protocol で文書チェーン全体をレビューし、確認された P0/P1
   blocker を解消する。
7. `git fetch origin main && git merge origin/main` で最新 main を取り込み、
   index を再生成してコミット・push・PR を作成する。

## Classification

`design/docs` — 設計契約の文書化と headless fixture 照合。`crates/` の
実装変更は対象外（別 Issue が Owner）。

## Upstream Documents

- `docs/specs/project-model.md`・`docs/designs/project-model-design.md`・
  `docs/plans/project-model.md`・`docs/test-specs/project-model.md`。
- 依存 spec: VS-A・TIM-01・ED-A/ED-B・MD-A・CTL-01・VCS-01/02/05・
  RCD-02・ALIB-01/02・Issue #86・SYN-IO-02・PC-C4-05・RC-D-04・TIM-02。

## Dependencies

- `crates/synvelis-project` の既存 module/test（読み取りのみ、本 slice は
  変更しない）。
- `doc-driven-dev-graph`・`doc-maintenance`・`doc-status`・`impl-doc`・
  `review-protocol` skill。
- `mise run fmt-check`（canonical 検証）。

## Affected Files

- `docs/specs/project-model.md`（new）
- `docs/designs/project-model-design.md`（new）
- `docs/plans/project-model.md`（new）
- `docs/tasks/project-model-task.md`（new）
- `docs/test-specs/project-model.md`（new）
- `docs/impl/ir/project-model.md`（new）
- `docs/impl/exp/project-model.md`（new）
- `evidence/sv10/project-model-case-ledger.md`（new）
- `evidence/sv10/project-model-verification.md`（new）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/README.md`
  （generated index、`doc_maintenance.js index apply` で再生成。
  `docs/impl/exp/README.md` は `.jsonl` のみ index 対象のため差分なし）

## Done When

- [ ] 7 文書 + evidence/sv10/ 2 件が存在し、PM-AC-01〜13 の全受入条件が
  PM-T case・既存 test・設計レビュー・NotRun/Deferred に漏れなく写像される。
- [ ] relation（implements/depends-on/derives-from/verifies/verified-by/
  references）が既存 docs と整合し、doc-status lint で新規文書起因の
  blocking finding がない。
- [ ] 触れた docs dir の index が `doc_maintenance.js index apply` で
  再生成され、audit で整合が確認される。
- [ ] `mise run fmt-check` が exit 0。
- [ ] review-protocol で spec+design+全体をレビューし、確認された P0/P1
  blocker が残っていない。
- [ ] `git fetch origin main && git merge origin/main` 済みで、index が
  最新 main に対して再生成されている。
- [ ] commit・push・PR（日本語本文）が完了している。

## Verification

- `mise run fmt-check`（canonical）。
- `node .agents/skills/doc-status/scripts/doc_status.js lint --type {spec,design,plan,task,test-spec,impl} --json`。
- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply`。
- `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json`。
- review-protocol（spec+design+chain 全体）。
- PM-T case の oracle は `docs/test-specs/project-model.md`・
  `evidence/sv10/project-model-case-ledger.md`。
