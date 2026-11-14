---
id: "TASK-uivmJkLmNrTyUiOpAsDfGh"
type: "task"
status: "in-progress"
title: "SF-05 ui/ slice タスク分解 — Signal Flow・Probe headless view-model consumer path"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/108"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-05-ui-viewmodel.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-05-ui-viewmodel.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-ui-viewmodel.md"
    - "docs/designs/sf-05-ui-viewmodel.md"
    - "docs/test-specs/sf-05-ui-viewmodel.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-ui-viewmodel.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-05 ui/ slice タスク分解 — headless view-model consumer path

## Work

1. `ui/src/ipc.ts` — `OpName` union に `syn.signal_flow.view`・
   `syn.signal_flow.visual`・`syn.signal_flow.inspection`・
   `syn.signal_flow.routing` を追加。
2. `ui/src/viewmodel/signal-flow-view-model.ts` — view projection
   verbatim 保持 + visual/inspection/routing の typed op surface。
3. `ui/src/viewmodel/probe-view-model.ts` — session/view-scoped
   control 分離 + state/compare projection verbatim。
4. `ui/test/signal-flow-view-model.test.ts`・
   `ui/test/probe-view-model.test.ts` — fake host wire 契約 spec。
5. `ui/test/conformance.test.ts` — validOp op リストへ 7 op 追記。
6. `evidence/sv108/` 本 slice 証拠（verification + coverage）。
7. doc chain 完成 → index 再生成（最後）→ commit/push/PR。

## Classification

headless consumer 実装 + wire 契約 test。GUI mount・OS 受入は
含まない。

## Upstream Documents

- spec: `docs/specs/sf-05-ui-viewmodel.md`
- design: `docs/designs/sf-05-ui-viewmodel.md`
- plan: `docs/plans/sf-05-ui-viewmodel.md`
- test-spec: `docs/test-specs/sf-05-ui-viewmodel.md`

## Dependencies

- service 側 op は main 統合済み（`ui_ops.rs` view/visual/
  inspection/routing・probe control/state/compare）。
- `installFakeHost` harness（既存、変更不要）。

## Affected Files

- `ui/src/ipc.ts`（OpName union）
- `ui/src/viewmodel/signal-flow-view-model.ts`（新規）
- `ui/src/viewmodel/probe-view-model.ts`（新規）
- `ui/test/signal-flow-view-model.test.ts`（新規）
- `ui/test/probe-view-model.test.ts`（新規）
- `ui/test/conformance.test.ts`（op リスト追記）
- `evidence/sv108/`（新規証拠）
- doc chain 7 ファイル + 管理 index 再生成物

## Done When

- `mise run check-ui`/`test-ui`/`fmt-check`/`prek run --all-files`
  が全て Pass。
- 新規 vitest spec が intent 完全一致・verbatim 写像・エラー経路・
  非捏造を検証し、既存 suite が green を維持。
- evidence file が実測値のみを記録し、NotRun 項目を正直に列挙。
- review-protocol 1 cycle で blocker 0。

## Verification

上記 `Done When` のコマンド群。`build-ui`・cargo レーン・msrv・
full・実 GUI/OS・oracle join は NotRun/不要として evidence に記録。

## Sources and Relations

- Issue: `https://github.com/scarecrow173/synvelis/issues/108`
- 関連 slice: `sf-05-slice1`/`sf-05-slice2`/`sf-05-slice3`（service
  側完了済み）、`sf-04-slice4`/`sf-04-slice5`（probe 配線元）。
