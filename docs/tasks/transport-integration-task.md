---
id: "TASK-034X7xdqyxRcd5A5TSwmzT"
type: "task"
status: "in-progress"
title: "Transport Integration Task — Issue #20 design contract + TT evidence ledger"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/20"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/transport-integration.md"
  implemented-by: []
  depends-on:
    - "docs/plans/transport-integration.md"
    - "docs/specs/transport-integration.md"
    - "docs/designs/transport-integration-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/transport-integration.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/transport-integration.md"
    - "https://github.com/scarecrow173/synvelis/issues/20"
  derived-by:
    - "docs/impl/ir/transport-integration.md"
  verifies: []
  verified-by:
    - "docs/test-specs/transport-integration.md"
  references:
    - "evidence/sv20/transport-integration-case-ledger.md"
    - "evidence/sv20/transport-integration-verification.md"
  defers: []
  deferred-by: []
---

# Transport Integration Task — Issue #20 design contract + TT evidence ledger

## Work

Issue #20 の canonical 設計契約を docs 7 文書鎖 + evidence/sv20
台帳として作成する。実装コードは変更しない。

1. `docs/specs/transport-integration.md` — TT-R-01〜15・
   TT-AC-01〜09。
2. `docs/designs/transport-integration-design.md` — Owner 分離・
   receipt/span/offset/capability 契約・TT-001〜030 写像表。
3. `docs/plans/transport-integration.md` — 実装 Owner 写像・
   証拠計画。
4. `docs/tasks/transport-integration-task.md` — 本書。
5. `docs/test-specs/transport-integration.md` — TT-001〜030
   case 写像（Owner/fixture/oracle/状態）。
6. `docs/impl/ir/transport-integration.md` — 実施記録。
7. `docs/impl/exp/transport-integration.md` — 試行・契約照合。
8. `evidence/sv20/transport-integration-case-ledger.md` — TT→
   test/oracle/status 台帳。
9. `evidence/sv20/transport-integration-verification.md` —
   検証実行記録。
10. index 再生成（`doc_maintenance.js index apply`）→ lint/audit
    → review-protocol → commit/push/PR。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/transport-integration.md`（PLAN-034X7x2GLBkoA5kq6w2VxK）
- `docs/specs/transport-integration.md`（SPEC-034X7wGMLTCbfLxfME4Odo）
- `docs/designs/transport-integration-design.md`（DESIGN-034X7wOrHbsoz1senpZA9w）
- `https://github.com/scarecrow173/synvelis/issues/20`

## Dependencies

- TIM-01〜04（#125〜128）headless 実装済み slice の fixture を
  参照（実装変更なし）。
- sv#19（musical-time docs）並行 Worker — `docs/specs/
  musical-time.md` 等は編集・作成しない（参照のみ）。

## Affected Files

- `docs/specs/transport-integration.md`（新規）
- `docs/designs/transport-integration-design.md`（新規）
- `docs/plans/transport-integration.md`（新規）
- `docs/tasks/transport-integration-task.md`（新規）
- `docs/test-specs/transport-integration.md`（新規）
- `docs/impl/ir/transport-integration.md`（新規）
- `docs/impl/exp/transport-integration.md`（新規）
- `evidence/sv20/transport-integration-case-ledger.md`（新規）
- `evidence/sv20/transport-integration-verification.md`（新規）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/
  README.md`（index 再生成のみ・手編集禁止）

## Done When

- 7 文書鎖 + evidence/sv20 台帳が作成され、TT-001〜030 が一意
  ID・Owner・fixture/oracle・状態で写像されている。
- index 再生成・lint・impl audit が実行され、範囲内 finding が
  解消されている。
- review-protocol で blocker が残っていない。
- `mise run fmt-check` が成功。
- commit/push/PR が作成されている。

## Verification

```sh
mise run fmt-check
node .agents/skills/doc-status/scripts/doc_status.js lint docs/specs/transport-integration.md docs/designs/transport-integration-design.md docs/plans/transport-integration.md docs/tasks/transport-integration-task.md docs/test-specs/transport-integration.md docs/impl/ir/transport-integration.md docs/impl/exp/transport-integration.md
node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply
node .agents/skills/impl-doc/scripts/audit_impl_record.js --json
node .agents/skills/doc-status/scripts/audit_docs.js
node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --focus transport-integration --json
```
