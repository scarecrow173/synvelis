---
id: "SPEC-7c4e9a1b2d3f4e5a6b7c8d9e0f"
type: "spec"
status: "approved"
title: "PC-C4-07 transport mirror + observe_block + capability rebind tests"
created: "2026-11-19"
updated: "2026-11-19"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-06-control-op-surface.md"
    - "docs/specs/pc-c4-03-param-observation.md"
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-03-session-driver-wiring.md"
  refines:
    - "docs/specs/pc-c4-06-control-op-surface.md"
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/pc-c4-06-control-op-surface-design.md"
    - "docs/test-specs/pc-c4-07-transport-mirror-observe.md"
---

# PC-C4-07 transport mirror + `observe_block` + capability rebind

Issue `#101` (PLUG-C4-04 残件) の slice 7。routing-path の
`RoutingControlDispatch::observe_cycle` は caller 供給の
`TransportSpan` + call window を要求し、`syn.control.observe`
(`ui_ops.rs`) が呼出ごとに虚構 span を生成していた。driver path は
`TransportMirror` (`driver.rs`) で commanded transport →
`SpanContext` 投影を解決済み。本 slice は routing path に同じ
mirror を持たせ、span を runtime 所有の transport 状態から内部導出
する。

## 要件

### A. routing-path transport mirror + `observe_block`

- `driver.rs` の `TransportMirror` を crate 内で再利用可能にする
  (`pub(crate)`)。第二の非互換 transport モデルを作らない。
- `RoutingControlDispatch` の surface node ごとに mirror を保持。
- command ingress は `node.dispatch.control()`
  (`boundary_stage_window` の stage 経路) と
  `ParamDispatch::dispatch` 内部の `Play` (due 0) — `driver.rs`
  と同じ receipt 補正モデルで投影する。
- `observe_block(generation, call_start, call_frames)` を追加 —
  transport span は内部導出、物理 call window は caller 供給のまま
  (run loop 内、node cursor domain)。
- staged `due_frames` は staging 後に観測される最初の `call_start`
  (= node が pop する次 callback head) に対して絶対 frame へ解決し、
  window 内の apply frame で投影を分割する。merged cycle は後の
  segment が同一 target で勝つ。
-  generation 不一致は fail-closed で `None`。診断用
  `last_observed_span(processor)` を公開する。

### B. capability-drift → republish → rebind 結合テスト (test-only)

`register_params`/`register_boundary` drift → blocked
`PluginCapabilityChanged` 発行 → republish → `sync` が新 capability
pin で rebind・stale-generation write が gate される一連を headless
で証明する。src 変更は原則不要。

## 非対象 (外部 gate = NotRun 委譲)

- app 層の `drain_output_reports` 周期配線、engine-driver 配線、
  `syn.control.status` の `plan_generation` 露出 (後続 slice)。
- production composition root `PluginHost`、wire schema L1、
  real VST3/OS/GUI/AudioLens 統合 — いずれも外部 gate であり
  本 slice では NotRun として報告する。
