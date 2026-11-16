---
id: "IMPL-034YpXqODquHNxLVIPcrN8"
type: "impl"
status: "completed"
title: "RCD-04 residual closeout — Issue #118 AC 照合・canonical lane 実測・残存 gate 棚卸し・簿記 drift 修正"
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
    - "docs/tasks/rcd-04-residual-closeout.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-04-residual-closeout.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcd-04-comp-transition-midi-phrase-crate-fixture-slice.md"
    - "docs/impl/ir/rcd-04-product-e2e-slice.md"
    - "docs/impl/ir/rcd-04-rt-engine-comp-pcm-slice.md"
    - "docs/impl/ir/rcd-04-midi-undo-stableid-slice.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/test-specs/rcd-04-residual-closeout.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-04-residual-closeout.md"
  verified-by: []
  references:
    - "docs/discovery/rcd-04-residual-closeout.md"
    - "docs/impl/exp/rcd-04-residual-closeout.md"
    - "evidence/sv118/rcd-04-closeout-verification.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/rcd-04-residual-closeout.md"
    rejected: []
---

# RCD-04 residual closeout — Issue #118 AC 照合・canonical lane 実測・残存 gate 棚卸し・簿記 drift 修正

## Summary

Issue #118（RCD-04 tracker）の residual closeout を docs +
evidence のみで実施した。受入条件 4 行を merge 済み実装
（PR #219/#300/#319/#374）の実在 test 名・merge 記録・
canonical lane 実測に 1:1 で対応付け、外部 gate（実 GUI =
synvelis#114・AudioLens native f64 = `scarecrow173/
ruviform#294`・MIDI 永続化 = synvelis#126・実 device/実 RT
= 実機試験・comp kind apply whitelist 拡張 = 別 slice
Deferred）を NotRun/Deferred のまま正直に棚卸しした。併せ
て merge 済み slice の task/impl frontmatter status と stale
checkbox の簿記 drift を修正した。プロダクションコード・
test・fixture・共有設定・docs index は一切変更していない。

## Context

`docs/tasks/rcd-04-residual-closeout.md` /
`docs/plans/rcd-04-residual-closeout.md` /
`docs/designs/rcd-04-residual-closeout-design.md` /
`docs/test-specs/rcd-04-residual-closeout.md` に従う。
構造の正本は `docs/{specs,designs,plans,tasks,test-specs,
impl/ir,impl/exp}/rcd-02-residual-closeout*`（sv#116）と
`alib-02-residual-closeout*`（sv#86）。本 slice は docs +
evidence only であり、RCD-04 機能そのものの再検証・新規
test/fixture 追加は行わない。

## Implementation

### Changes

- 新規 doc chain 7 file: `docs/specs/rcd-04-residual-closeout.
  md`・`docs/designs/rcd-04-residual-closeout-design.md`・
  `docs/plans/rcd-04-residual-closeout.md`・
  `docs/tasks/rcd-04-residual-closeout.md`・
  `docs/test-specs/rcd-04-residual-closeout.md`・
  `docs/impl/ir/rcd-04-residual-closeout.md`（本書）・
  `docs/impl/exp/rcd-04-residual-closeout.md`
- `docs/impl/exp/rcd-04-residual-closeout-exp.jsonl`
  （experiment run-trace）・`docs/discovery/
  rcd-04-residual-closeout.md`（棚卸しメモ）・
  `evidence/sv118/rcd-04-closeout-verification.md`
  （AC 照合・lane 実測・gate 棚卸しの正本 evidence）
- 簿記 drift 修正 6 file: merge 済み 3 task file の
  frontmatter `status` を `in-progress`→`done`、3 impl/ir
  file の `status` を `in-progress`→`completed` に修正し、
  Done When/Verification の stale `[ ]` を事実（merge PR
  番号・本 closeout evidence 参照）に整合。当時の作業記録
  本文は書き換えていない。

### Decisions During Implementation

- AC 照合の root 証拠は既存 test 名・merge 済み PR 記録・
  実測 lane に限定し、headless/fake/mock の PASS を実
  GUI・実 device・実 RT・consumer の証拠に昇格しない
  （POLICIES.md の検証レベル規範どおり）。
- `docs/**/README.md` 管理 index は編集・再生成・`index
  apply` を一切行わない（Coordinator が統合時に再生成。
  index 未同期は許容済み）。
- `*-msrv`・`*-full` 系 lane は Daily Full CI 委譲のため
  実行せず NotRun として記録した。

## Related Experiments

### Adopted

- `docs/impl/exp/rcd-04-residual-closeout.md` /
  `docs/impl/exp/rcd-04-residual-closeout-exp.jsonl`:
  canonical lane 実測と AC 照合の run-trace。

### Rejected

- なし（新規実験・代替案の比較は発生していない）。

## Validation

正本の lane 実測結果・AC 照合表・gate 棚卸しは
`evidence/sv118/rcd-04-closeout-verification.md` を参照。
本 slice で実行した検証:

- `mise run fmt-check`: Pass（exit 0）
- `mise exec -- prek run --all-files`: Pass（exit 0）
- `mise run test synvelis-project`: Pass（exit 0）
- `mise run test synvelis-recording`: Pass（exit 0）
- `mise run test synvelis-player`: Pass（exit 0）
- `mise run test synvelis-render`: Pass（exit 0）
- `mise run test-rt`: Pass（exit 0）
- `mise run test synvelis-app`（opt-in）: Pass（exit 0、
  355 tests・`rcd_04_product_e2e` 5 tests 全 pass）
- `git status` / `git diff --stat`: 占有リスト外の変更なし
  を確認

## Risks

- docs index が本 slice の新規 file を未反映のまま残る
  （Coordinator が統合時に再生成する前提の許容済み状態）。
- Issue #118 は外部 gate が残るため open のまま維持する
  （auto-close しない。merge/Close は Coordinator 判断）。

## Follow-ups

- 実 GUI 検証: synvelis#114。
- AudioLens native f64 consumer 検証: `scarecrow173/
  ruviform#294`。
- MIDI comp selection 永続化: synvelis#126。
- comp kind VCS-05 apply whitelist 拡張: 別 slice
  （fail-closed 契約は `apply_selected_fails_closed_on_
  comp_transition` で pin 済み。対象 Issue の起票・依存
  登録は Coordinator 側対応）。
- 統合 CI 証拠: merge 後 main SHA の CI を Coordinator が
  確認。
- docs index 再生成・`doc_status` audit: Coordinator 側。
