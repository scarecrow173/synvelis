---
id: "PLAN-q3Zx7Vn2Lm8Kw4RyTbP9"
type: "plan"
status: "approved"
title: "TIM-03 transport application minimal vertical"
created: "2026-11-27"
updated: "2026-11-27"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/tim-03-transport-application.md"
  implemented-by:
    - "docs/impl/tim-03-transport-application.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
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

# TIM-03 transport application minimal vertical — Plan

## Phase 1 — 調査

- merged L1 hosting、real ABI binding、process boundary session、loop wrap derivation、
  automation control tick の contract を確認する。
- `test_gain` が transport を消費しない制約を oracle 設計へ反映する。

## Phase 2 — production seam

- `control_apply_transport` と conversion/report/error type を `engine.rs` に追加する。
- boundary declaration を production composition で登録する最小 variant を追加する。
- dedicated test 用に shared L1 helper を `tests/common/tim03_l1.rs` へ抽出する。

## Phase 3 — deterministic L1 scenario

- 非 default tempo map、loop range、seek、automation point を組み合わせる。
- VST3/CLAP `test_gain` を production composition + runner 経由で実走する。
- ledger projection、folded span、PCM block classification、typed negative evidence を
  両 format で記録する。

## Phase 4 — verification/evidence

- `mise run test-tim03-transport` を canonical 入口として追加する。
- JSONL と verification note を `evidence/sv127/` に追加する。
- 正規 lane を実行し、PR 本文に local verification と integration verification の区別を書く。
