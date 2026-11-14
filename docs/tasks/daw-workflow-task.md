---
id: "TASK-034X9LKtObc3yvqQPs4uwt"
type: "task"
status: "done"
title: "Issue #8 DAW ワークフロー UX 文書チェーンと evidence/sv8 台帳の確定"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/8"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/daw-workflow-plan.md"
  implemented-by:
    - "docs/impl/ir/daw-workflow.md"
  depends-on:
    - "docs/plans/daw-workflow-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/daw-workflow.md"
    - "docs/impl/exp/daw-workflow.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/daw-workflow-plan.md"
    - "docs/specs/daw-workflow.md"
    - "docs/designs/daw-workflow-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/daw-workflow.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #8 DAW ワークフロー UX 文書チェーンと evidence/sv8 台帳の確定

## Work

Issue #8 の UX 仕様を canonical 文書チェーンとして確定する。
対象は `docs/specs/daw-workflow.md`（要件・WF-AC-01〜10）、
`docs/designs/daw-workflow-design.md`（Gate A backbone
GA-01〜08・WF-SCN-MIDI/AUDIO/MIX・Keyboard/Gesture/Tool
契約・共通操作体系・Recording 状態・非依存境界・Decision
Log）、`docs/plans/daw-workflow-plan.md`、本 task、
`docs/test-specs/daw-workflow.md`、`docs/impl/ir/
daw-workflow.md`、`docs/impl/exp/daw-workflow.md`、および
`evidence/sv8/` の受入項目→仕様節→既存 docs/実装 Issue
台帳・検証記録。コード実装・実機/実 GUI 検証はスコープ外。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/daw-workflow-plan.md`
- `docs/specs/daw-workflow.md`
- `docs/designs/daw-workflow-design.md`
- `docs/test-specs/daw-workflow.md`

## Dependencies

- 上位正本 docs（#3/#10/#11/#12/#17/#18/#19/#20/#21）の存在
  — 確認済み。
- `docs/plans/daw-workflow-plan.md` の approved 状態 — 確認済み。

## Affected Files

- `docs/specs/daw-workflow.md`
- `docs/designs/daw-workflow-design.md`
- `docs/plans/daw-workflow-plan.md`
- `docs/tasks/daw-workflow-task.md`
- `docs/test-specs/daw-workflow.md`
- `docs/impl/ir/daw-workflow.md`
- `docs/impl/exp/daw-workflow.md`
- `evidence/sv8/`（台帳・検証記録）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,
  impl/exp}/README.md`（doc-maintenance `index apply` による
  生成 index）

## Done When

- [x] spec/design/plan/task/test-spec/impl(ir/exp) の文書
  チェーンが `docs/` に存在し、front matter relations が
  双方向で整合している。
- [x] Issue #8 の受入条件 7 項目が WF-AC-01〜07 に写像され、
  各 AC に対応する仕様節・Oracle・検証レーンが design に
  存在する。
- [x] 3 代表ワークフロー（WF-SCN-MIDI/AUDIO/MIX）と Gate A
  backbone（GA-01〜08）が工程・操作・Oracle・検証レーン
  つきで定義されている。
- [x] Keyboard/Gesture/Tool UX の最小操作集合と安全契約が
  design §7 に定義されている。
- [x] `evidence/sv8/` に受入項目→仕様節→既存 docs/実装
  Issue の台帳と、OS-L1/GUI-L2/EXT-L3 の結果区分
  （Pass/NotRun/Deferred/Unsupported）が記録されている。
- [x] 実装・実機・実 GUI で未検証の項目が `NotRun`/
  `Deferred`/`Unsupported` として区別され、設計評価を実機
  Pass に換算していない。
- [x] `doc_status lint/audit`・`mise run fmt-check`・
  `git diff --check` が実行され、結果が PR 本文に記録されて
  いる。

## Verification

- `node .agents/skills/doc-status/scripts/doc_status.js lint
  --dir docs/specs` / `--dir docs/designs` / `--dir docs/plans`
  / `--dir docs/tasks` / `--dir docs/test-specs` — 新規文書由来の
  blocking finding がないこと（index 未反映・未作成 doc への
  relation は作成/index 再生成で解消済みのものを除く）。
- `node scripts/doc_maintenance.js index plan --dir docs/specs`
  等 → `index apply` — 触れた dir の index が最新。
- `mise run fmt-check` — exit code 0（docs-only のため cargo
  fmt no-op）。
- `git diff --check` — exit code 0。
- `git status --porcelain` — 変更が許可範囲 + 生成 index のみ。
