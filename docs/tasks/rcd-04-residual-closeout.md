---
id: "TASK-034YpWughtvQy1AKz8xCaW"
type: "task"
status: "done"
title: "RCD-04 residual closeout — Issue #118 AC 照合・canonical lane 実測・証拠固定・簿記 drift 修正"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added:
      - type: "file"
        path: "docs/specs/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/designs/rcd-04-residual-closeout-design.md"
      - type: "file"
        path: "docs/plans/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/tasks/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/test-specs/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/impl/exp/rcd-04-residual-closeout.md"
      - type: "file"
        path: "docs/impl/exp/rcd-04-residual-closeout-exp.jsonl"
      - type: "file"
        path: "docs/discovery/rcd-04-residual-closeout.md"
      - type: "file"
        path: "evidence/sv118/rcd-04-closeout-verification.md"
    modified:
      - type: "file"
        path: "docs/tasks/rcd-04-task-midi-undo-stableid.md"
      - type: "file"
        path: "docs/tasks/rcd-04-task-rt-engine-comp-pcm.md"
      - type: "file"
        path: "docs/tasks/rcd-04-task1-product-e2e.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-midi-undo-stableid-slice.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-product-e2e-slice.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-rt-engine-comp-pcm-slice.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-04-residual-closeout.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-04-residual-closeout.md"
    - "docs/designs/rcd-04-residual-closeout-design.md"
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
    - "docs/test-specs/rcd-04-residual-closeout.md"
  references:
    - "docs/discovery/rcd-04-residual-closeout.md"
    - "evidence/sv118/rcd-04-closeout-verification.md"
  defers: []
  deferred-by: []
---

# RCD-04 residual closeout — Issue #118 AC 照合・canonical lane 実測・証拠固定・簿記 drift 修正

## Work

Issue #118（RCD-04）の簿記クローズアウト。プロダクション
コード・test・fixture は変更しない。受入条件 4 行と実在
test・merge 済み PR 記録・canonical lane 結果を照合し、
doc chain・evidence・IR/exp に固定する。併せて merge 済み
slice の task/impl status・stale checkbox の簿記 drift を
修正する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-04-residual-closeout.md`
- `docs/specs/rcd-04-residual-closeout.md`
- `docs/designs/rcd-04-residual-closeout-design.md`
- `docs/test-specs/rcd-04-residual-closeout.md`

## Dependencies

- canonical lane 実走済みの worktree（環境導入済み toolchain）。
- 編集境界: worker spec の占有リスト（新規 doc chain
  `docs/{specs,designs,plans,tasks,test-specs,impl/ir,
  impl/exp}/rcd-04-residual-closeout*`、`docs/impl/exp/
  rcd-04-residual-closeout-exp.jsonl`、
  `docs/discovery/rcd-04-residual-closeout.md`、
  `evidence/sv118/rcd-04-closeout-verification.md`、drift
  修正対象 6 file）。docs index（`docs/**/README.md`）は
  Coordinator が統合時に再生成するため本 slice では触れない。

## Affected Files

- 新規: `docs/{specs,designs,plans,tasks,test-specs,impl/ir,
  impl/exp}/rcd-04-residual-closeout*.md`（7 file）+
  `docs/impl/exp/rcd-04-residual-closeout-exp.jsonl` +
  `docs/discovery/rcd-04-residual-closeout.md` +
  `evidence/sv118/rcd-04-closeout-verification.md`
- 更新: `docs/tasks/rcd-04-task-{midi-undo-stableid,
  rt-engine-comp-pcm,task1-product-e2e}.md`（status +
  stale checkbox の事実整合）、`docs/impl/ir/rcd-04-{midi-
  undo-stableid,product-e2e,rt-engine-comp-pcm}-slice.md`
  （status + merge PR 注記）

## Done When

- [x] Issue 受入条件 4 行が実在テスト/merge PR/lane 付きで
      [x]/[ ] 記録される（spec Acceptance Criteria + IR +
      `evidence/sv118/rcd-04-closeout-verification.md`）
- [x] canonical lane（`mise run test synvelis-project` /
      `synvelis-recording` / `synvelis-player` /
      `synvelis-render` / `test-rt`）の実走結果が Pass/Fail
      で記録される
- [x] 外部 gate が NotRun/Deferred + 所有 Issue 記名で記録
      される
- [x] merge 済み 3 task file の status が `done`、3 impl/ir
      file の status が `completed` になり、stale checkbox
      が事実に整合する
- [x] `mise run fmt-check` / `prek run --all-files` が Pass
- [x] PR 作成（`Refs #118`。close/merge なし）

## Verification

- [x] `mise run fmt-check` / `mise exec -- prek run
      --all-files`（軽量チェック）
- [x] `mise run test synvelis-project` / `synvelis-recording`
      / `synvelis-player` / `synvelis-render` / `test-rt`
      （結果は IR Validation 参照）
- [x] `mise run test synvelis-app`（opt-in — 実行した場合は
      結果を正直に記録）
- [x] `git diff --stat` / `git status` で占有リスト外の変更
      が無いことを確認
