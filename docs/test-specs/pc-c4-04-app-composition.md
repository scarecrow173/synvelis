---
id: "TSPEC-PCc4040bs3vN1pQ8wXyZ2aF"
type: "test-spec"
status: "approved"
title: "PC-C4-04 app composition — vend・所有・tick・gate・feed test 保証"
created: "2026-11-11"
updated: "2026-11-11"
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
    - "docs/specs/pc-c4-04-app-composition.md"
    - "docs/designs/pc-c4-04-app-composition-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-04-app-composition.md"
    - "docs/designs/pc-c4-04-app-composition-design.md"
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
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

# PC-C4-04 app composition test-spec

## Feature: catalog vend

- host 経路で `register_params` した `ParamInfo` が connect 時に
  `ProcessorId` keyed catalog へ vend され、production attach の
  bind が手組み catalog 無しで成功する。
- 未登録 plugin は空 table を vend し、bind は typed に失敗する
  （捏造しない）。

## Feature: dispatch 所有・tick 駆動

- publish ごとに `SessionRoutingEngine` が `attach`/`sync` を駆動し、
  `control_report`/`control_generation` が live generation を報告する。
- `ui_pump`（または明示 `control_tick`）が session playback instant
  で tick を駆動し、`TickReport` の gate/delivery counter が
  `last_control_tick` に残る。
- `control_fast_control` が active generation で `Delivered`、
  非 active で `Queued` を返す。

## Feature: generation gate

- generation swap 後、旧 generation を名指す `observe_cycle`/
  `drain_output_reports`/`fast_control` は fail-closed（gate）。
- 新旧 generation の report が混線しない — feed に到達するのは
  新世代 surface の `Reported` observation のみ。

## Feature: observation feed 橋渡し

- `feed_control_cycle` が `Reported` stage の observation のみを
  `ControlSession::feed_plugin_report` へ流し、
  `observation(target).plugin_reported` に値が現れる。
- `Scheduled`/`Dispatched` stage は feed されない（host 解決値を
  plugin report と偽らない）。stamp は `Estimated` provenance。

## NotRun

- 実 plugin instance・実 GUI・L1 wire report verb（pin 未含有）。
- audio thread 上の RT 駆動・`service.rs` op 登録（範囲外）。
