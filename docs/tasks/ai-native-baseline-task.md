---
id: "TASK-JOX49mZJEhYDdYV7eCjq12"
type: "task"
status: "in-progress"
title: "AI-Native Hybrid DAW Baseline — Grant/Principal/Scope/Approval/ChangeSet・AI Off 既定・3 Mode 責務設計契約文書化 (synvelis#44)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/44"
  changes:
    added:
      - type: "file"
        path: "docs/specs/ai-native-baseline.md"
      - type: "file"
        path: "docs/designs/ai-native-baseline-design.md"
      - type: "file"
        path: "docs/plans/ai-native-baseline.md"
      - type: "file"
        path: "docs/tasks/ai-native-baseline-task.md"
      - type: "file"
        path: "docs/test-specs/ai-native-baseline.md"
      - type: "file"
        path: "docs/impl/ir/ai-native-baseline.md"
      - type: "file"
        path: "docs/impl/exp/ai-native-baseline.md"
      - type: "file"
        path: "evidence/sv44/ai-native-baseline-case-ledger.md"
      - type: "file"
        path: "evidence/sv44/ai-native-baseline-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements: []
  implemented-by: []
  depends-on:
    - "docs/plans/ai-native-baseline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/ai-native-baseline.md"
  derived-by:
    - "docs/impl/ir/ai-native-baseline.md"
  verifies: []
  verified-by:
    - "docs/test-specs/ai-native-baseline.md"
  references: []
  defers: []
  deferred-by: []
---

# AI-Native Hybrid DAW Baseline — 設計契約文書化タスク (synvelis#44)

## Work

Issue #44 の canonical 設計契約を docs/ 7 文書鎖 + evidence
台帳として作成する。**docs/ 設計契約 + evidence のみ**であり、
Command Gateway の型・実装・GUI・実 device/plugin/OS・実外部
Provider 受入は対象外。

## Classification

`docs/design`（設計契約文書化）。実装タスクではない。

## Upstream

- `docs/specs/ai-native-baseline.md`（AIB-001〜007・AHG-001〜011・
  AIB-AC-01〜08）
- `docs/designs/ai-native-baseline-design.md`（canonical
  concepts・state machines・failure taxonomy・Owner 表・写像）
- `docs/plans/ai-native-baseline.md`（file map・stream・
  verification）
- `docs/designs/gw-01-gateway-contract-design.md`・
  `docs/designs/gw-02-authorization-policy-design.md`（実装側
  先行設計）
- Issue #44 本文（AIB-001〜007・AHG-001〜011・受入 8 件・
  未決定論点）

## Dependencies

- GW-01〜10（`crates/synvelis-gateway`/`crates/mcp-poc`）—
  headless fixture の先行実体（`mise run gw10`）
- #45（Capability/Command Gateway 詳細型契約）— 後続 Owner
- #46（MCP/Permission UX/RT/AI OFF 製品受入）— 後続 Owner
- #114（GUI/OS/Privacy 実機 lane）— 別 Gate
- #17（project-persistence・ProjectController Single Writer）
- #23（実行計画・通常 DAW Gate A/B/C・AI 独立第四軸）
- #1/#2/#5/#7/#8/#10/#11/#18/#19/#20/#21/#38 — 確定済み縦断契約

## Affected Files

| File | 変更 |
|---|---|
| `docs/specs/ai-native-baseline.md` | 新規 |
| `docs/designs/ai-native-baseline-design.md` | 新規 |
| `docs/plans/ai-native-baseline.md` | 新規 |
| `docs/tasks/ai-native-baseline-task.md` | 新規 |
| `docs/test-specs/ai-native-baseline.md` | 新規 |
| `docs/impl/ir/ai-native-baseline.md` | 新規 |
| `docs/impl/exp/ai-native-baseline.md` | 新規 |
| `evidence/sv44/ai-native-baseline-case-ledger.md` | 新規 |
| `evidence/sv44/ai-native-baseline-verification.md` | 新規 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md` | index 再生成のみ |

## Done When

- [ ] 7 文書鎖が canonical front matter・relations・内容を
  持ち、spec→design→plan→task→test-spec→impl の依存が
  一貫している。
- [ ] AIB-001〜007・AHG-001〜011 が spec/design に定義され、
  plan・test-spec・evidence で一貫して参照される。
- [ ] AIB-AC-01〜08 が `evidence/sv44/ai-native-baseline-case-
  ledger.md` に fixture・oracle・Owner・status
  （Pass/Fail/Unsupported/NotRun/Pass(部分)）・証拠付きで
  写像される。
- [ ] Grant/Principal/Scope/Invocation Target/Approval
  Receipt/ChangeSet・3 Mode/Proactive・AI Off の Owner 境界が
  design・plan・evidence で一貫して固定される。
- [ ] AIB/AHG 合意済み事項と未決定事項が区別され、未決定事項
  が受入 Gate 化されない。
- [ ] headless Pass を実機/GUI/OS/外部 Provider/実 MCP Client
  受入に繰り上げない evidence 語彙が一貫している。
- [ ] index が再生成され、docs 検証（lint/audit/route/fmt-
  check）が実行・記録される。
- [ ] review-protocol で blocker が残っていない。
- [ ] commit・push・PR・worker_done が完了する。

## Verification

正規検証コマンド: `mise run fmt-check`（docs/設計スライス）。
補助検証（docs 検証 + headless 回帰入口）:

- `node .agents/skills/doc-status/scripts/doc_status.js lint
  --type {spec,design,plan,task,test-spec} --json`
- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index apply`
- `node .agents/skills/impl-doc/scripts/audit_impl_record.js
  --json`・`audit_experiment_log.js --json`
  ・`sync_impl_index.js --kind ir|exp --apply`
- `node .agents/skills/doc-status/scripts/audit_docs.js
  --type all --json`
- `node .agents/skills/doc-driven-dev-graph/scripts/
  route_graph.js --focus ai-native-baseline --json`
- `mise run gw10`・`mise run gw10-report`・`mise run test
  synvelis-gateway`（headless 回帰入口・実測を evidence に記録）
- `review-protocol`

全て `evidence/sv44/ai-native-baseline-verification.md` に記録。

## Out of Scope

- 実装コード・`crates/`・`Cargo.toml`・`Cargo.lock`・`mise.toml`・
  CI・Ruviform
- GUI 実装・実 device/plugin・実 OS 受入・実外部 MCP
  Client/Provider・ユーザー承認 UI
- 通常 DAW 利用への AI Grant/Provider 接続の強制
- Ruviform 汎用 Gateway/認証 Provider の Synvelis 専用としての
  重複実装
- Command Gateway の型・実装（#45/#46 が Owner）
- 未決定事項の確定（Grant UI・Context・Privacy・Provider・
  Agent 自律性・Budget・MCP Schema/Transport）
