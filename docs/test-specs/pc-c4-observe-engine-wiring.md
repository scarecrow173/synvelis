---
id: "TSPEC-b2e5a8c1f4d90736e5b2a8c2d4e6f7a8b"
type: "test-spec"
status: "approved"
title: "PC-C4 observe engine wiring test spec"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c4-observe-engine-wiring.md"
    - "docs/designs/pc-c4-observe-engine-wiring-design.md"
    - "docs/plans/pc-c4-observe-engine-wiring-plan.md"
    - "docs/tasks/pc-c4-observe-engine-wiring-task.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-observe-engine-wiring.md"
    - "docs/designs/pc-c4-observe-engine-wiring-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-observe-engine-wiring-plan.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c4-observe-engine-wiring.md"
    - "docs/designs/pc-c4-observe-engine-wiring-design.md"
    - "docs/plans/pc-c4-observe-engine-wiring-plan.md"
  verified-by: []
  references:
    - "docs/test-specs/pc-c4-07-transport-mirror-observe.md"
    - "docs/test-specs/pc-c4-drain-composition.md"
    - "docs/test-specs/pc-c4-06-control-op-surface.md"
  defers: []
  deferred-by: []
---

# PC-C4 observe engine wiring test spec

Fixture: `crates/synvelis-app/tests/pc_c4_observe_engine_wiring.rs`（新規）。
pc_c4_04/06/drain-composition と同じ fake plugin・project 構築・
composition root 経路を使い、engine 駆動は
`SessionRoutingEngine::run` / `ui_pump` のみ（fixture が
`observe_block`/`observe_cycle` を直接呼ばないことが engine-driver
配線の検証を兼ねる）。

## Feature: engine 駆動 observe_block

### Rule: run が処理 block ごとに node head を供給する

- **Given** attach 済み surface（generation 1）と publish 済み
  plan（generation 1）
- **When** `engine.run(blocks, policy)` を連続で呼ぶ
- **Then** 最初の block は activation block なので
  `call_start = EngineFrame(ClockEpochId(0), 0)` で観測され、
  以降の block は node cursor が block 長ずつ進んだ head
  （`block_frames` 刻み）で観測される。engine `cursor()` の値とは
  独立（publish 前処理・seek で乖離し得ることを別 Rule で pin）。

### Rule: generation 不一致 window は fail-closed

- **Given** surface generation が ran generation と一致しない期間
  （plan 未 publish・publish 後 RT 未観測など）
- **When** `run` がその期間の block を処理する
- **Then** `observe_block` は `None` を返し buffer に cycle が
  入らない — 観測が捏造されない。

### Rule: seek 後も node head が実体と一致する

- **Given** node cursor が `N` まで進んだ状態で `seek` を実行
- **When** quiesce pass の `rt.process` が成功する
- **Then** その pass 分の node head が追跡され、後続 `run` の
  `call_start` が node 実体の cursor と一致する（engine cursor の
  seek 再 anchor に引きずられない）。

### Rule: staged 制御の配送が正しい window で Dispatched

- **Given** fast-control write が `Play`/param を stage した状態
- **When** engine 駆動の observe が staged due frame を含む
  window を処理する
- **Then** 対象 param の observation が `Dispatched` に進み、
  window 外の frame は `Scheduled` のまま残る。merged cycle の
  receipts/observations が buffer 経由で drain される。

## Feature: app 側 feed と status

### Rule: drain_control_observations が Reported を feed する

- **Given** engine 駆動 observe が cycle を buffer した状態
- **When** `app.drain_control_observations()` を呼ぶ
- **Then** 各 cycle が `feed_control_cycle` を通り、`Reported`
  stage の observation が workspace の plugin-report channel に
  届く（`fed >= 1`）。drain 後の buffer は空。

### Rule: ui_pump が observe drain を周期駆動する

- **Given** composition 済み app + engine 駆動 observe が buffer に
  cycle を残した状態
- **When** `app.ui_pump()` を呼ぶ
- **Then** `drain_control_observations` 相当の feed が実行され、
  plugin-report channel に `Reported` observation が届く。

### Rule: syn.control.status が観測 span を露出する

- **Given** engine 駆動 observe が `last_observed_span` を記録した
  surface
- **When** `syn.control.status` op を実行する
- **Then** `observe.last_observed_span[<processor>]` が最後に投影
  した `TransportSpan`（start/end/status/position/plan_generation
  等）を示し、`observe.plan_generation[<processor>]` がその
  generation を示す。未観測 processor は `null`。
