---
id: "TSPEC-0HnGRMaXEy6WChONldI4n2"
type: "test-spec"
status: "approved"
title: "SF-03 slice7 — render semantic-lowering 消費 test-spec"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/sf-03-slice7-render-consumption.md"
    - "docs/designs/sf-03-slice7-render-consumption-design.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-03-slice7-render-consumption.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-slice7-render-consumption-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-03-slice7-render-consumption.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice7 — render semantic-lowering 消費 test-spec

## Feature: cross-consumer same-Revision fixture

### Rule TS-7-01: same committed revision → same semantic evidence

`crates/synvelis-render/tests/sf03_render.rs::
sf03_render_01_same_revision_across_consumers`

- export（`BuildMode::Offline`、mono output tap）と
  `RoutingEngine::prepare/publish/run`（`BuildMode::Realtime`）が
  同一 `project_revision`・`routing_fingerprint`・
  `capability_generation`・`LOWERING_POLICY_VERSION` を持つ
  `RoutingSourceMap` を生成する。
- 両 map の semantic node/port key が `project.routing` に存在し、
  playback binding の `(content, destination)` 集合が一致する
  （runtime NodeId は consumer/generation scoped で差異許容）。
- engine publish 後 `Active` receipt が同一 revision を指す。

### Rule TS-7-02: deterministic fail-closed parity

`sf03_render_02_fail_closed_same_fault`

- strip 非保有 track の clip: export `PreflightError::
  RoutingLowering{InvalidRouting}`、engine prepare
  `RoutingFault::InvalidRouting` — destination 非作成。
- chain processor host 非供給: 両 consumer `PluginUnavailable`。
- `OfflineRenderSupport::Unknown` instance: export_with_host /
  engine offline prepare とも `OfflineCapabilityMismatch`。
- `Verified` instance は同一 export を完了する（失敗原因が
  topology でなく capability claim であることの対照）。

## Feature: regression — 既存 L2/X/TIM-04

### Rule TS-7-03: PCM 非回帰

- `l2_e2e` 17 tests・`x_e2e` 5 tests・`tim04_plugin_state` 5 tests
  が semantic lowering 経路で全件 PASS（mono/stereo output、
  insert/PDC/tail/comp を含む bit-exact 期待値維持）。
- `synvelis-player` 既存 sf03_l1-l6 fixture 全件 PASS
  （`lower_semantic` realtime 経路不変、`StripProcessor`/
  `GainProcessor` の Verified 報告追加のみ）。
