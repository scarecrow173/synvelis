---
id: "EXP-uivmXcVbNmQwErTyUiOpAsD"
type: "experiment"
status: "completed"
title: "SF-05 ui/ slice 実装時の診断記録 — class member 衝突と helper 設計"
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
  implements: []
  implemented-by:
    - "docs/impl/ir/sf-05-ui-viewmodel.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/sf-05-ui-viewmodel.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-05 ui/ slice 実装診断 — class member 衝突と helper 設計

## 背景

`SignalFlowViewModel` 初版で private method `inspection()` と
public getter `inspection` を同名で定義した。TypeScript/JS の
class semantics では method と accessor は同じ prototype
property namespace を共有するため、後勝ちで getter が method を
shadow し、`vm.inspection` が `SignalFlowInspectionWire` では
なく `[Function inspection]` を返した。vitest 4 件が
`expected [AsyncFunction inspection] to deeply equal …` で失敗。

同型の衝突が `private readonly viewId` field と `get viewId()`
でも発生し、`tsc --noEmit` が `Duplicate identifier 'viewId'`
を 4 件報告した。

## 診断

- 症状: getter が method/field を shadow、または tsc の
  duplicate identifier error。
- 原因: class の field/getter/method は prototype/instance の
  同一名前空間を共有し、同名は共存できない（private でも同じ）。
- 違反された不変条件: VM の public state accessor と private
  IPC helper が別名前空間であるべきところ、命名が衝突した。

## 採用した修正

- private method `inspection()` → `sendInspection()` へ改名
  （`visual`/`routing`/`control`/`params` は対応する public
  member と衝突しないため維持）。
- private field `viewId` → `viewIdValue` へ改名し、public
  `get viewId()` を唯一の accessor とした。
- `private routing<T>`/`private control<T>` を generic 化し、
  public method ごとの `as Promise<…>` cast を排除（wire 型を
  `call<T>` の型引数で直接受ける）。

## 教訓

- VM の private helper 命名は public accessor/method と衝突
  しない prefix/命名（`send*`/`*Value` field）を既定にする。
- `let p` を複数の異なる Promise 型で再代入する test では
  `Promise<unknown>` へ widening しないと `tsc` が落ちる
  （vitest/esbuild は型を剥がすため runtime では検出されない
  — `check-ui` が必須の理由の一例）。
