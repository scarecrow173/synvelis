---
id: "PLAN-v4Kx8Qm2Ln6Rw9TzPyC5"
type: "plan"
status: "approved"
title: "TIM-03 instrument direct L1 musical transport oracle"
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
    - "docs/designs/tim-03-instrument-transport-oracle.md"
  implemented-by:
    - "docs/impl/tim-03-instrument-transport-oracle.md"
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

# TIM-03 instrument direct L1 musical transport oracle — Plan

## Phase 1 — contract 調査

- merged TIM-03 L1 runner、fixture staging、endpoint latency evidence を確認する。
- ruviform#630 `test_instrument` fixture law、direct endpoint transport API、
  upstream `PluginNode` handoff gap を resolved ruviform source で確認する。
- 重複する ruviform Issue がないことを確認し、upstream tracker を起票する。

## Phase 2 — test harness

- `tests/common/tim03_l1.rs` に `test_instrument` descriptor selection と evidence 用
  artifact/revision helper を追加する。既存 `test_gain` helper の意味は変えない。
- `tim03_l1_instrument_transport.rs` を追加し、direct endpoint lifecycle と transport
  phases を実装する。

## Phase 3 — oracle cases

- stopped/paused/missing musical negative control。
- 120 BPM click@100、seek click@500、loop click@250。
- 150 BPM click@200。
- matched/substituted/initial silence の freshness 分類、stereo identical、
  non-click silence を独立計算で判定する。

## Phase 4 — verification/evidence

- `mise run test-tim03-instrument-transport` を追加する。
- `evidence/sv127/instrument-transport-oracle.jsonl` と verification note を作る。
- 指定された canonical command を実行し、Pass/Fail/NotRun/Blocked upstream を分離して記録する。
- docs index は編集しない。
