---
id: "TASK-034X3SQ8cOyUYvh20KEekf"
type: "task"
status: "in-progress"
title: "Workspace UX 操作仕様の正本化（Issue #3 文書チェーン実施）"
created: "2026-11-13"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/3"
  changes:
    added:
      - type: "file"
        path: "docs/specs/workspace-ux-scenarios.md"
      - type: "file"
        path: "docs/designs/workspace-ux-scenarios-design.md"
      - type: "file"
        path: "docs/plans/workspace-ux-scenarios.md"
      - type: "file"
        path: "docs/tasks/workspace-ux-scenarios-task.md"
      - type: "file"
        path: "docs/test-specs/workspace-ux-scenarios.md"
      - type: "file"
        path: "docs/impl/ir/workspace-ux-scenarios.md"
      - type: "file"
        path: "docs/impl/exp/workspace-ux-scenarios.md"
      - type: "file"
        path: "evidence/sv3/workspace-ux-scenarios-traceability.md"
      - type: "file"
        path: "evidence/sv3/workspace-ux-scenarios-verification.md"
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
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/plans/workspace-ux-scenarios.md"
  implemented-by: []
  depends-on:
    - "docs/plans/workspace-ux-scenarios.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/workspace-ux-scenarios.md"
    - "docs/designs/workspace-ux-scenarios-design.md"
    - "docs/impl/ir/workspace-ux-scenarios.md"
    - "docs/impl/exp/workspace-ux-scenarios.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/workspace-ux-scenarios.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/workspace-ux-scenarios.md"
  references: []
  defers: []
  deferred-by: []
---

# Workspace UX 操作仕様の正本化（Issue #3 文書チェーン実施）

## Work

Issue #3 の UX 仕様正本化タスクを文書チェーンとして実施する。
`docs/specs/`・`docs/designs/`・`docs/plans/`・`docs/tasks/`・
`docs/test-specs/`・`docs/impl/{ir,exp}/` に canonical 文書を
作成し、`evidence/sv3/` に受入項目→シナリオ→仕様節の追跡表と
検証記録を残す。製品コード（`crates/`）・Ruviform・`mise.toml`・
CI・`docs/mockups/` は変更しない。実装・実 OS/実 Plugin GUI 検証は
行わず、NotRun/Deferred を明示する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/workspace-ux-scenarios.md`（status: in-progress）
- `docs/specs/workspace-ux-scenarios.md`（status: approved）
- `docs/designs/workspace-ux-scenarios-design.md`（status:
  approved）

## Dependencies

- 統合設計 `docs/designs/workspace-docking-state-architecture.md`
- headless 固定済み契約: WS-A spec、ED-A spec
- SV-GPUI-03 preset 契約、sv-gui-04 slice2 dock op 経路
- 実機 lane は #114/#176、Inspector 詳細は #5（いずれも後続）

## Affected Files

- `docs/specs/workspace-ux-scenarios.md`（新規）
- `docs/designs/workspace-ux-scenarios-design.md`（新規）
- `docs/plans/workspace-ux-scenarios.md`（新規）
- `docs/tasks/workspace-ux-scenarios-task.md`（本書）
- `docs/test-specs/workspace-ux-scenarios.md`（新規）
- `docs/impl/ir/workspace-ux-scenarios.md`（新規）
- `docs/impl/exp/workspace-ux-scenarios.md`（新規・JSONL）
- `evidence/sv3/workspace-ux-scenarios-traceability.md`（新規）
- `evidence/sv3/workspace-ux-scenarios-verification.md`（新規）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/README.md`
  （`doc-maintenance index apply` による再生成のみ）

## Done When

- [ ] spec/design/plan/task/test-spec/impl ir/exp の 7 文書が
  存在し、relations が相互に解決する
- [ ] `evidence/sv3/` に WUX-AC-* → WUX-SCN-* → 設計節 →
  Issue § の追跡表と検証記録がある
- [ ] review-protocol を spec+design に実施し CONFIRMED P0/P1
  が残っていない
- [ ] `doc_status lint` で新規文書に index/orphan 以外の
  blocking finding がない
- [ ] `mise run fmt-check` が exit 0
- [ ] 変更が `docs/`・`evidence/sv3/`・生成 index の範囲内

## Verification

- [ ] `node .agents/skills/doc-status/scripts/doc_status.js lint
  --type spec --json` で `workspace-ux-scenarios` の blocking
  finding が index/orphan 系のみ（exit 0、JSON 出力を目視確認）
- [ ] `node .agents/skills/doc-status/scripts/doc_status.js lint
  --type design --json` 同上
- [ ] `node .agents/skills/doc-status/scripts/doc_status.js lint
  --type plan --json` / `--type task --json` /
  `--type test-spec --json` で同ファイルの blocking finding が
  index/orphan 系のみ
- [ ] `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js
  index apply` が成功し、再 lint で index/orphan finding が
  解消される
- [ ] `mise run fmt-check` が exit 0
- [ ] `git status --porcelain` の変更ファイルが
  `docs/`・`evidence/sv3/`・`docs/*/README.md` index のみ
