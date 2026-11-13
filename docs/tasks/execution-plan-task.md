---
id: "TASK-034X84fclPRNdTgHrtrDka"
type: "task"
status: "in-progress"
title: "Execution Plan Task — Issue #23 横断 DAG・Owner/Gate matrix・sv23 evidence"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/23"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/execution-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/execution-plan.md"
    - "docs/specs/execution-plan.md"
    - "docs/designs/execution-plan-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/execution-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/execution-plan.md"
    - "https://github.com/scarecrow173/synvelis/issues/23"
  derived-by:
    - "docs/impl/ir/execution-plan.md"
  verifies: []
  verified-by:
    - "docs/test-specs/execution-plan.md"
  references:
    - "evidence/sv23/execution-plan-gate-ledger.md"
    - "evidence/sv23/execution-plan-entry-gate-ledger.md"
    - "evidence/sv23/execution-plan-verification.md"
  defers: []
  deferred-by: []
---

# Execution Plan Task — Issue #23 横断 DAG・Owner/Gate matrix・sv23 evidence

## Work

Issue #23 の横断実行計画を docs 7 文書鎖 + evidence/sv23
台帳として作成する。実装コード・CI・Cargo/mise 設定は
変更しない。

1. `docs/specs/execution-plan.md` — EP-R-01〜12・
   EP-AC-01〜09。
2. `docs/designs/execution-plan-design.md` — DAG Model・
   lane/edge 表・Owner/Gate/Acceptance Matrix・OS Level・
   Evidence Schema・正規入口対応・候補 SHA tuple Registry。
3. `docs/plans/execution-plan.md` — 実装 Owner 写像・
   証拠計画・risk register。
4. `docs/tasks/execution-plan-task.md` — 本書。
5. `docs/test-specs/execution-plan.md` — DAG/matrix/OS
   level の不変条件と検証規則。
6. `docs/impl/ir/execution-plan.md` — 実施記録。
7. `docs/impl/exp/execution-plan.md` — 試行・契約照合。
8. `evidence/sv23/execution-plan-gate-ledger.md` —
   workstream × Issue × Owner × Start/Integration/
   Acceptance × OS Level × 状態台帳。
9. `evidence/sv23/execution-plan-entry-gate-ledger.md` —
   正規入口 × OS × Gate/Feature × SHA × 結果状態台帳。
10. `evidence/sv23/execution-plan-verification.md` —
    検証実行記録。
11. index 再生成（`doc_maintenance.js index apply`）→
    lint/audit → review-protocol → commit/push/PR。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/execution-plan.md`（PLAN-034X841J2QRrgpxCPlRiKb）
- `docs/specs/execution-plan.md`（SPEC-034X83uiIsH0n6OuQGcU1T）
- `docs/designs/execution-plan-design.md`（DESIGN-034X83GOEQ84LyUmDRBepu）
- `https://github.com/scarecrow173/synvelis/issues/23`

## Dependencies

- 個別 workstream docs chain（sv#10/#11/#12/#17〜#20・
  GUI 系・VCS/ALIB/OBS 系）を参照（実装変更なし）。
- sv#21（Media/Render）・sv#22（Target Architecture）
  並行 Worker — それらの文書は編集・作成しない（参照
  のみ）。
- `POLICIES.md` の OS-L1/GUI-L2/EXT-L3・Required/Deferred/
  NotApplicable・Pass/Fail/NotRun/Unsupported 規範に従う。

## Affected Files

- `docs/specs/execution-plan.md`（新規）
- `docs/designs/execution-plan-design.md`（新規）
- `docs/plans/execution-plan.md`（新規）
- `docs/tasks/execution-plan-task.md`（新規）
- `docs/test-specs/execution-plan.md`（新規）
- `docs/impl/ir/execution-plan.md`（新規）
- `docs/impl/exp/execution-plan.md`（新規）
- `evidence/sv23/execution-plan-gate-ledger.md`（新規）
- `evidence/sv23/execution-plan-entry-gate-ledger.md`（新規）
- `evidence/sv23/execution-plan-verification.md`（新規）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  README.md`（index 再生成のみ・手編集禁止。
  `docs/impl/exp` は `.jsonl` のみ index 対象の既存規約）

## Done When

- 7 文書鎖 + evidence/sv23 台帳 3 件が作成され、
  RCP-00〜07/F01・F0/E2E-L/R/X・SF/Automation/GUI の
  Start/Integration/Acceptance が非巡回の単一 DAG として
  記録され、共通/Consumer Owner と着手条件が分離されて
  いる。
- OS-L1/GUI-L2/EXT-L3 の matrix・要求状態/実行結果の
  語彙分離・正規入口対応・候補 SHA tuple 追跡が
  evidence/sv23 に記録されている。
- index 再生成・lint・impl audit が実行され、範囲内
  finding が解消されている。
- review-protocol で blocker が残っていない。
- `mise run fmt-check` が成功。
- commit/push/PR が作成されている。

## Verification

```sh
mise run fmt-check
node .agents/skills/doc-status/scripts/doc_status.js lint docs/specs/execution-plan.md docs/designs/execution-plan-design.md docs/plans/execution-plan.md docs/tasks/execution-plan-task.md docs/test-specs/execution-plan.md docs/impl/ir/execution-plan.md docs/impl/exp/execution-plan.md
node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply
node .agents/skills/impl-doc/scripts/audit_impl_record.js --json
node .agents/skills/doc-status/scripts/audit_docs.js
node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --focus execution-plan --json
```
