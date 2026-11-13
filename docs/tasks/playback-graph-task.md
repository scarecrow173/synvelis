---
id: "TASK-034X6XdaPIa6eFgvfVs5QC"
type: "task"
status: "in-progress"
title: "Playback Graph Task — Issue #18 design contract + EX evidence ledger"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/18"
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
    - "docs/plans/playback-graph.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/playback-graph.md"
  derived-by:
    - "docs/impl/ir/playback-graph.md"
  verifies: []
  verified-by:
    - "docs/test-specs/playback-graph.md"
  references: []
  defers: []
  deferred-by: []
---

# Playback Graph Task — Issue #18 design contract + EX evidence ledger

## Work

Issue #18 の docs/ 設計契約スライス。7 文書チェーン
（spec/design/plan/task/test-spec/impl ir/exp）と
`evidence/sv18/` の case ledger・verification record を作成し、
EX-001〜020 を既存 Synvelis/Ruviform headless 証拠へ写像する。
コード・Cargo・CI・Ruviform は変更しない。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/playback-graph.md`
- `docs/specs/playback-graph.md`
- `docs/designs/playback-graph-design.md`
- `docs/test-specs/playback-graph.md`

## Dependencies

- sv#107（Semantic Probe Adapter・PR#388）統合済み・参照可能
- sv#101（PC-C4 Plugin Binding・PR#393）統合済み・参照可能
- 実装は #104〜109 が Owner、#114 は GPUI 実 GUI、
  ruviform#294 は両 consumer 独立受入

## Affected Files

- `docs/specs/playback-graph.md`
- `docs/designs/playback-graph-design.md`
- `docs/plans/playback-graph.md`
- `docs/tasks/playback-graph-task.md`
- `docs/test-specs/playback-graph.md`
- `docs/impl/ir/playback-graph.md`
- `docs/impl/exp/playback-graph.md`
- `evidence/sv18/`（case ledger・verification）
- 触った dir の生成 index（`docs/*/README.md`）— `index apply` で再生成

## Done When

- [ ] 7 文書が canonical front matter・relation 付きで存在する
- [ ] EX-001〜020 が test-spec case・既存 test・oracle・状態・
      Required/Deferred/NA・Pass/Fail/NotRun/Unsupported に写像される
- [ ] capability 別 gate（実 device/plugin/OS GUI）が
      NotRun/Unsupported として明示される
- [ ] index が `doc_maintenance.js index apply` で再生成される
- [ ] `fmt-check`・`doc_status lint`・`audit_impl_record`・
      `route_graph`・`review-protocol` を実行し結果を記録する
- [ ] headless 証拠を実機/GUI/plugin 受入と混同しない

## Verification

- [ ] `mise run fmt-check`
- [ ] `node .agents/skills/doc-status/scripts/doc_status.js lint --type {spec,design,plan,task,test-spec,impl} --json`
- [ ] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js index apply`
- [ ] `node .agents/skills/impl-doc/scripts/audit_impl_record.js --json`
- [ ] `node .agents/skills/doc-status/scripts/audit_docs.js`
- [ ] `node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --focus playback-graph --json`
- [ ] `review-protocol`（blocker なし）
