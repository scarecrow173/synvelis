---
id: "PLAN-034YpUFyOog9NH2jZycwaj"
type: "plan"
status: "completed"
title: "RCD-04 residual closeout — Issue #118 AC 照合と証拠固定の実装計画"
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
    - "docs/specs/rcd-04-residual-closeout.md"
  implemented-by:
    - "docs/tasks/rcd-04-residual-closeout.md"
  depends-on:
    - "docs/designs/rcd-04-residual-closeout-design.md"
  blocks:
    - "docs/tasks/rcd-04-residual-closeout.md"
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcd-04-comp-midi-phrase-fixture.md"
    - "docs/test-specs/rcd-04-midi-undo-stableid.md"
    - "docs/test-specs/rcd-04-product-e2e.md"
    - "docs/test-specs/rcd-04-rt-engine-comp-pcm.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-04-residual-closeout.md"
    - "docs/designs/rcd-04-residual-closeout-design.md"
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

# RCD-04 residual closeout — Issue #118 AC 照合と証拠固定の実装計画

## Goal

Issue #118 の受入条件 4 行を実在 test・merge 済み PR 記録・
canonical lane 実測と 1:1 照合し、doc chain・evidence・IR/exp
に簿記として固定する。プロダクションコード・test・fixture
の変更は含まない。

## Scope

単一 stream の docs/evidence-only 変更。分割不要。
編集境界は worker spec の占有リストに限定する。

## Upstream Documents

- `docs/specs/rcd-04-residual-closeout.md`（本 plan が実装
  する spec）
- `docs/designs/rcd-04-residual-closeout-design.md`（設計
  入力）
- Issue: <https://github.com/scarecrow173/synvelis/issues/118>

## Design Inputs

- `docs/designs/rcd-04-residual-closeout-design.md`

## File Map

| File | 変更 |
| --- | --- |
| `docs/specs/rcd-04-residual-closeout.md` | AC 照合 ledger（新規） |
| `docs/designs/rcd-04-residual-closeout-design.md` | 照合・gate 分類の設計（新規） |
| `docs/plans/rcd-04-residual-closeout.md` | 本計画（新規） |
| `docs/tasks/rcd-04-residual-closeout.md` | task checklist（新規） |
| `docs/test-specs/rcd-04-residual-closeout.md` | 照合・lane・gate 棚卸しの検証仕様（新規） |
| `docs/impl/ir/rcd-04-residual-closeout.md` | 照合結果・lane 結果・残存 gate の IR（新規） |
| `docs/impl/exp/rcd-04-residual-closeout.md` | 照合手法の実験記録（新規） |
| `docs/impl/exp/rcd-04-residual-closeout-exp.jsonl` | experiment run-trace（新規） |
| `docs/discovery/rcd-04-residual-closeout.md` | closeout 対象棚卸しメモ（新規） |
| `evidence/sv118/rcd-04-closeout-verification.md` | AC 照合表・lane 実測・gate 棚卸し（新規） |
| `docs/tasks/rcd-04-task-midi-undo-stableid.md` | status `in-progress`→`done` + stale checkbox 事実整合（PR #374 merge 済み） |
| `docs/tasks/rcd-04-task-rt-engine-comp-pcm.md` | status `in-progress`→`done` + stale checkbox 事実整合（PR #319 merge 済み） |
| `docs/tasks/rcd-04-task1-product-e2e.md` | status `in-progress`→`done` + stale checkbox 事実整合（PR #300 merge 済み） |
| `docs/impl/ir/rcd-04-midi-undo-stableid-slice.md` | status `in-progress`→`completed` + merge PR 注記 |
| `docs/impl/ir/rcd-04-product-e2e-slice.md` | status `in-progress`→`completed` + merge PR 注記 |
| `docs/impl/ir/rcd-04-rt-engine-comp-pcm-slice.md` | status `in-progress`→`completed` + merge PR 注記 |

## Implementation Sequence

- [x] Issue #118 本文と既存 doc chain・実在テストを照合し
      AC 対応表を作成
- [x] canonical lane（`mise run test synvelis-project` /
      `synvelis-recording` / `synvelis-player` /
      `synvelis-render` / `test-rt`）と軽量チェック
      （`fmt-check`・`prek run --all-files`）を実走し結果を
      記録
- [x] spec/design/plan/task/test-spec/IR/exp + discovery の
      新規 doc chain を作成
- [x] merge 済み 3 task file の status を `done` に修正し
      stale checkbox を事実に整合
- [x] merge 済み 3 impl/ir file の status を `completed` に
      修正し merge PR を注記
- [x] `evidence/sv118/rcd-04-closeout-verification.md` を作成
- [x] PR 作成（Issue close/merge はしない）

## Dependencies and Risks

- 並行 Worker が同 repo で稼働中のため、占有リスト外の file
  に触れないことを diff で最終確認する。
- `mise run test-rt` は workspace 全 test を `rt_` filter で
  実行する。toolchain は環境導入済み rust を使う
  （`mise.toml` は Rust を pin しない）。
- `mise run test synvelis-app`（opt-in）は `rcd_04_product_
  e2e` の所在 package の全 test を実行する。実行する場合は
  結果を正直に記録し、実行しない場合は NotRun + merge PR
  #300 の記録を引用する。
- repo-wide baseline の doc finding（legacy invalid-id 系・
  broken-relation 系）は既知・migration 対象。本 slice の
  判定基準は「変更ファイルに新規 finding を増やさないこと」
  だが、docs index regen・`doc_status` audit は Coordinator
  側の責務のため本 slice では実施しない。

## Task Breakdown

- `docs/tasks/rcd-04-residual-closeout.md` — 本 plan の task
  checklist（単一 task、docs+evidence のみ）。

## Verification

- `mise run fmt-check` / `mise exec -- prek run --all-files`
  — 軽量チェック。
- `mise run test synvelis-project` / `synvelis-recording` /
  `synvelis-player` / `synvelis-render` / `test-rt` —
  canonical、結果は IR Validation と evidence に記録。
- `mise run test synvelis-app` — opt-in（任意。実行する場合
  は結果を正直に記録）。
- `git diff --stat` / `git status` で占有リスト外の変更が無
  いことを確認。

## Open Questions / Gaps

- なし（AC は Issue #118 本文で確定済み。外部 gate は owner
  Issue 記名の NotRun/Deferred として記録）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology
      are consistent.

## Implementation Handoff

- Recommended mode: inline execution（docs+evidence の小
  slice、単一 stream）
- Independent work streams: none
- User approval required before dispatch: yes（worker spec
  の review-protocol で代替 — blocker 0 を承認として扱う）
- If approved: discover available implementation and
  delegation capabilities in the current environment, then
  execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- AC-3（Recovery partial+cancel+retry・Group Comp 全体 CAS・
  2Window Stale Gesture/History view read-only の「実
  GUI＋headless」）は実 GUI 分が synvelis#114 gate のため、
  headless 分のみを Pass とし実 GUI 分は Deferred として分離
  した — reviewer はこの分離の妥当性を確認されたい。
- AC-4 の AudioLens native f64 非回帰は consumer gate
  （`scarecrow173/ruviform#294`）側の責務であり、Synvelis
  側は raw PCM/Asset 不変の pin のみを Pass とした — 同様に
  分離の妥当性を確認されたい。
