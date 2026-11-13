---
id: "TASK-034X8sLyrKt55RXlxUdD4N"
type: "task"
status: "in-progress"
title: "Capability / Command Gateway 設計契約タスク (synvelis#45)"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/command-gateway.md"
  implemented-by: []
  depends-on:
    - "docs/plans/command-gateway.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/command-gateway.md"
  derived-by:
    - "docs/impl/ir/command-gateway.md"
  verifies: []
  verified-by:
    - "docs/test-specs/command-gateway.md"
  references: []
  defers: []
  deferred-by: []
---

# Capability / Command Gateway 設計契約タスク (synvelis#45)

## Work

Issue #45 の canonical 設計契約を docs/ 7 文書鎖 + evidence
台帳として作成する。**docs/ 設計契約 + evidence のみ**であり、
実装コード・GUI・実 device/plugin/OS/Provider 受入は対象外。

## Classification

`docs/design`（設計契約文書化）。実装タスクではない。

## Upstream

- `docs/specs/command-gateway.md`（GW-R-01〜15・GW-AC-01〜12）
- `docs/designs/command-gateway-design.md`（Owner 表・contract・
  state machine・failure taxonomy・写像）
- `docs/plans/command-gateway.md`（file map・stream・verification）
- Issue #45 本文（v0.3〜v0.8・GT/JM/MCP/UX・検証方法・範囲）
- Issue #44（AIB/AHG）・Issue #46（G0〜G6）

## Dependencies

- #53〜#57（GW-01〜05・merge 済み）— contract/policy/context/
  prepare/proposal の先行実体
- #58（GW-06 commit/revoke/idempotency/recovery・merge 済み）
- #59（GW-08 MCP adapter/binding・merge 済み）
- #62（GW-10 E2E/security/concurrency/recovery・merge 済み）
- #83（独立 CI 再現性）— 後続
- #114（GPUI 承認 UI・実外部 MCP/Provider）— 後続・並行
- #259〜#267（AUTX）・ruviform #553/#294 — 参照のみ
- sv#44（ai-native-baseline）— 並行 Worker・参照のみ

## Affected Files

| File | 変更 |
|---|---|
| `docs/specs/command-gateway.md` | 新規 |
| `docs/designs/command-gateway-design.md` | 新規 |
| `docs/plans/command-gateway.md` | 新規 |
| `docs/tasks/command-gateway-task.md` | 新規 |
| `docs/test-specs/command-gateway.md` | 新規 |
| `docs/impl/ir/command-gateway.md` | 新規 |
| `docs/impl/exp/command-gateway.md` | 新規 |
| `evidence/sv45/command-gateway-case-ledger.md` | 新規 |
| `evidence/sv45/command-gateway-verification.md` | 新規 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md` | index 再生成のみ |

## Done When

- [ ] 7 文書鎖が canonical front matter・relations・内容を
  持ち、spec→design→plan→task→test-spec→impl の依存が
  一貫している。
- [ ] GW-R-01〜15・GW-AC-01〜12 が spec に定義され、design・
  plan・test-spec・evidence で一貫して参照される。
- [ ] GT-001〜012・JM-01〜05・MCP-01〜06・UX-01〜06・
  SEC-01〜14・DATA-01〜10・REC-01〜12・JOB-01〜10・
  SYS-01〜10・AC-01〜10・E2E-1/2/3・G0〜G6・GTV-001〜008 が
  `evidence/sv45/command-gateway-case-ledger.md` に fixture・
  oracle・Owner・mode/layer・status・証拠付きで写像される。
- [ ] GW-01〜10 Owner 表・状態機械・Failure Matrix・AUTX/
  CLI/adapter 境界・RT boundary が design・plan・evidence で
  一貫して固定される。
- [ ] headless/mock/PoC real transport の Pass を実機/GUI/
  Provider/第三者 Host 受入に繰り上げない evidence 語彙が
  一貫している。
- [ ] index が再生成され、docs 検証（lint/audit/route/fmt-
  check）と headless 回帰実測（gw10/gw10-report/check/test
  synvelis-gateway）が実行・記録される。
- [ ] review-protocol で blocker が残っていない。
- [ ] commit・push・PR・worker_done が完了する。

## Verification

正規検証コマンド: `mise run fmt-check`（docs/設計スライス）。
headless 回帰入口（補助証拠・canonical = mise）:

- `mise run gw10`・`mise run gw10-report`
- `mise run check synvelis-gateway`・`mise run test
  synvelis-gateway`

補助検証（docs 検証）:

- `node .agents/skills/doc-status/scripts/doc_status.js lint
  --type {spec,design,plan,task,test-spec} --json`
- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index apply`
- `node .agents/skills/impl-doc/scripts/audit_impl_record.js
  --json`・`audit_experiment_log.js --json`・`sync_impl_index.js`
- `node .agents/skills/doc-status/scripts/audit_docs.js
  --type all --json`
- `node .agents/skills/doc-driven-dev-graph/scripts/
  route_graph.js --focus command-gateway --json`
- `review-protocol`

全て `evidence/sv45/command-gateway-verification.md` に記録。

## Out of Scope

- 実装コード・`crates/`・`Cargo.toml`・`Cargo.lock`・`mise.toml`・
  CI・Ruviform
- GUI 実装・実 device/plugin・実 OS/Provider 受入（#114・#83・
  実機 lane）
- 実外部 MCP Host/Provider の Authenticated Session（#46 G4
  拡張・後続試験）
- AUTX 文書編集（#259〜#267）・sv#44 文書編集（並行 Worker・
  参照のみ）
- Caller 自己申告の Trusted Authority 昇格
- 実装・実機・実 Provider の PASS 宣言
