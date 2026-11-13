---
id: "TASK-mr20261114mediaRenderB"
type: "task"
status: "in-progress"
title: "Media / Import / Export / Offline Render / UI / Diagnostics — Media & Render Lifecycle 設計契約文書化 (synvelis#21)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/21"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/plans/media-render.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/media-render.md"
  derived-by:
    - "docs/impl/ir/media-render.md"
  verifies: []
  verified-by:
    - "docs/test-specs/media-render.md"
  references: []
  defers: []
  deferred-by: []
---

# Media & Render Lifecycle — 設計契約文書化タスク (synvelis#21)

## Work

Issue #21 の canonical 設計契約を docs/ 7 文書鎖 + evidence
台帳として作成する。**docs/ 設計契約 + evidence のみ**であり、
実装コード・GUI・実 device/plugin/OS 受入は対象外。

## Classification

`docs/design`（設計契約文書化）。実装タスクではない。

## Upstream

- `docs/specs/media-render.md`（MR-R-01〜19・MR-AC-01〜10）
- `docs/designs/media-render-design.md`（component・state
  machine・failure taxonomy・MR mapping・Owner 表）
- `docs/plans/media-render.md`（file map・stream・verification）
- `docs/designs/media-import-export-offline-render-ui-diagnostics-
  architecture.md`（v0.6 Approved baseline）
- Issue #21 本文

## Dependencies

- #24/#25/#26（L1/L2/X・closed）— headless fixture の先行実体
- #9/#85〜#91（asset-library）— remote/preview・並行 Worker
- #17（project-persistence・統合済み）
- #18（playback-graph）— graph/probe 設計先例
- #127/#128（tim03/tim04）— plugin precision/state 契約
- #116/#118（vs-c/rcd-02/recording-take-comping）— recording
  durability
- #86（issue-86 verified-import-wiring）— import wiring
- #114（sv-gui-04）— window/job lifetime
- #289〜#297（RCP-00〜F01）— render checkpoint

## Affected Files

| File | 変更 |
|---|---|
| `docs/specs/media-render.md` | 新規 |
| `docs/designs/media-render-design.md` | 新規 |
| `docs/plans/media-render.md` | 新規 |
| `docs/tasks/media-render-task.md` | 新規 |
| `docs/test-specs/media-render.md` | 新規 |
| `docs/impl/ir/media-render.md` | 新規 |
| `docs/impl/exp/media-render.md` | 新規 |
| `evidence/sv21/media-render-case-ledger.md` | 新規 |
| `evidence/sv21/media-render-verification.md` | 新規 |
| `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md` | index 再生成のみ |

## Done When

- [ ] 7 文書鎖が canonical front matter・relations・内容を
  持ち、spec→design→plan→task→test-spec→impl の依存が
  一貫している。
- [ ] MR-R-01〜19・MR-AC-01〜10 が spec に定義され、design・
  plan・test-spec・evidence で一貫して参照される。
- [ ] MR-RCP-01・MR-001〜028 が `evidence/sv21/media-render-
  case-ledger.md` に fixture・oracle・Owner・status
  （Pass/Fail/Unsupported/NotRun/Pass(部分)）・証拠付きで
  写像される。
- [ ] Ruviform generic / Synvelis DAW policy の Owner 境界が
  design・plan・evidence で一貫して固定される。
- [ ] #127/#128・#116/#86・RCP-00〜F01 の横断写像が存在する。
- [ ] headless Pass を実機/GUI/Plugin/OS 受入に繰り上げない
  evidence 語彙が一貫している。
- [ ] index が再生成され、docs 検証（lint/audit/route/fmt-
  check）が実行・記録される。
- [ ] review-protocol で blocker が残っていない。
- [ ] commit・push・PR・worker_done が完了する。

## Verification

正規検証コマンド: `mise run fmt-check`（docs/設計スライス）。
補助検証（docs 検証）:

- `node .agents/skills/doc-status/scripts/doc_status.js lint
  --type {spec,design,plan,task,test-spec} --json`
- `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index apply`
- `node .agents/skills/impl-doc/scripts/audit_impl_record.js
  --json`・`audit_experiment_log.js --json`
- `node .agents/skills/doc-status/scripts/audit_docs.js
  --type all --json`
- `node .agents/skills/doc-driven-dev-graph/scripts/
  route_graph.js --focus media-render --json`
- `review-protocol`

全て `evidence/sv21/media-render-verification.md` に記録。

## Out of Scope

- 実装コード・`crates/`・`Cargo.toml`・`Cargo.lock`・`mise.toml`・
  CI・Ruviform
- GUI 実装・実 device/plugin・実 OS 受入
- asset-library 文書編集（#9 系 Worker と並行・参照のみ）
- 実 PCM・実 Plugin・実機 PASS の宣言
- stem/bounce/freeze/unified orchestrator/RCP の実装
  （後続 Issue が Owner）
