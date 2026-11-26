---
id: "DESIGN-n8Vw2Kx6Qm4Rt9LyTzC3"
type: "design"
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
    - "docs/specs/tim-03-transport-application.md"
  implemented-by:
    - "docs/plans/tim-03-transport-application.md"
  depends-on:
    - "docs/designs/tim-03-l1-out-of-process-hosting.md"
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

# TIM-03 transport application minimal vertical — Design

## 責務境界

`control_apply_transport` は transport application owner であり、automation evaluator を
第二正本として再実装しない。beat→media conversion と transport staging を所有し、
automation は同じ瞬間を表す `(BeatPoint, MediaTime)` pair を既存 `control_tick` へ渡す。
`control_tick` は automation / queued fast-control の low-level primitive のまま維持する。
このため UI 経路や independent session clock を移行・複製しない。

## conversion

project の persisted tempo map を compile し、attached control surface の verified
`TimeBase` で `beat_to_media` を呼ぶ。caller は media frame を渡さず、frame base を推測
しない。同一 beat の重複 conversion は call 内 cache に入れ、evidence には role 付きの
input/result を残す。loop end <= start、tempo compile failure、frame 表現不能は typed
error で拒否する。

## window と boundary staging

window head は engine cursor ではなく、直近に処理成功した `ObservedBlock` の
`call_start + call_frames` から導く。loop wrap は command staging 前に同じ window へ
`boundary_loop_wraps` で問い、返された `BoundaryPoint` をそのまま
`boundary_stage_window` の oracle input に渡す。command due はすべて window-relative 0 であり、
`PluginNodeControl` 直接 push、mirror/scheduler 再実装、fake span 捏造はしない。

## composition seam

`compose_routing_engine_with_out_of_process_and_boundaries` は既存 L0/L1 route
登録に processor-keyed `BoundaryDeclaration` table を足す最小 variant である。
宣言は host attach 前に `PluginHostProcessors::register_boundary` へ入れ、
precision を推測・昇格しない。既存 `compose_routing_engine_with_out_of_process` は
空 table で委譲するため既存 contract を変えない。

## negative evidence

surface generation と RT-observed active generation が一致しない場合、
`boundary_stage_window` の typed `InactiveGeneration` を silent success にせず返す。
control surface/frame base/observed window が無い場合も typed error とする。
