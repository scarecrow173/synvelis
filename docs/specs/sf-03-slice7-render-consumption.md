---
id: "SPEC-0ETTGUPUGPQE6z2dyPRs0b"
type: "spec"
status: "approved"
title: "SF-03 slice7 — synvelis-render semantic-lowering 消費 + cross-consumer same-Revision fixture"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  derived-by:
    - "docs/designs/sf-03-slice7-render-consumption-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice7-render-consumption.md"
  references:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/sf-03-slice6-act06-control-surface.md"
  defers: []
  deferred-by: []
---

# SF-03 slice7 — synvelis-render semantic-lowering 消費 + cross-consumer same-Revision fixture

## What Should Be Built

`synvelis-render` の export が legacy `synvelis_player::lowering::
ClipWindow`/`lower_with_inserts` 経路をやめ、realtime routing engine
と同一の `lower_semantic` lowering を `BuildMode::Offline` で消費する。
semantic graph は project の port channel geometry で構築し、
`request.engine.channels` は device-sink の明示 output tap channel
数としてのみ扱う — sink edge の明示 channel map だけが幾何差を橋渡し
し、semantic graph 内部での暗黙 downmix/リサンプルは行わない
（Issue #106 の implicit conversion 禁止に適合する「明示 output 宣言」）。

- `lower_semantic_offline`（`SemanticOutput { insert_count, format }`）:
  master strip post-fader 出力と device sink の間に output insert
  slot を signal order で挿入し、sink format が engine format と
  channel 数だけ異なる場合に sink edge へ明示 channel map を付す。
  realtime `lower_semantic` は `SemanticOutput::default()` で不変。
- `SemanticLowered::inserts` は semantic 意味を持たない output-stage
  binding で、`node_roles`/`RoutingSourceMap` には入らない。
- `StripProcessor`/`GainProcessor` は deterministic なので
  `OfflineRenderSupport::Verified` を正直に報告する。
- `synvelis-render` は `RoutingBuildKey`（`BuildMode::Offline`、
  snapshot の `project_revision`/`routing_fingerprint`/
  `capability_generation`/`LOWERING_POLICY_VERSION`）と generation
  scoped `RoutingSourceMap` を構築し、`ExportReceipt::source_map`
  として証拠を残す。chain processor は `export_with_host` 経由で
  realtime engine と同一の `ProcessorHost` 境界を通り、host 非供給・
  `OfflineRenderSupport` 非 Verified は fail-closed。
- lowering/provisioning 失敗は `PreflightError::RoutingLowering
  { fault: RoutingFault, detail }` として型付き分類を保持する。

## Acceptance Criteria

- [ ] `synvelis-render` が `lower_semantic` 系経路のみを使い、
      legacy `lower_with_inserts` への依存が残らない。
- [ ] SF-ACT-02: strip fader/mute/solo/send/bus/processor 順序を
      含む semantic lowering が Realtime/Offline で同一 pipeline
      を通り、既存 L2/X/TIM-04 fixture の PCM 期待値が維持される
      （mono output tap・stereo graph の明示 channel map 経路を含む）。
- [ ] §8.2-1 cross-consumer same-Revision fixture:
      `crates/synvelis-render/tests/sf03_render.rs` が同一 committed
      revision に対し export receipt・engine `source_map`・
      `project.routing` semantic model の revision/fingerprint/
      semantic ID 解決が一致することを証明する。
- [ ] SF-ACT-07 deterministic fail-closed 半分: Missing Strip /
      unprovisioned chain processor / uncertified offline instance が
      両 consumer で同一 `RoutingFault` class に落ち、destination
      非作成・fake silence なしを確認する。

## Out of Scope（Issue 残件として記録）

- SF-ACT-05 実 device、SF-ACT-07 policy 未定部分、SF-ACT-08
  （upstream/AudioLens/Ruviform#294）、SF-ACT-09 perf baseline、
  全 GUI gate — NotRun。
- `synvelis-app`/`synvelis-workspace` 側 Revision fixture — 別
  dispatch（sv108）占有のため NotRun。
