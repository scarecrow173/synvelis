---
id: "TSPEC-7c4e9a1b2d3f4e5a6b7c8d9e0f1a2b3c4d"
type: "test-spec"
status: "approved"
title: "PC-C4-07 transport mirror + observe_block test spec"
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
  implements:
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
    - "docs/designs/pc-c4-07-transport-mirror-observe-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
  verified-by: []
  references:
    - "docs/designs/pc-c4-07-transport-mirror-observe-design.md"
---

# PC-C4-07 test spec

## Feature: routing-path transport mirror + `observe_block`

### Rule MIRROR-01 — span は mirror から導出される

`observe_block` が返す各 node の `TransportSpan` は、node の
commanded transport 状態 (status/position/loop/occurrence/
generation) から導出される。caller は物理 window
(`call_start`,`call_frames`) のみ供給し、transport 証拠を捏造
できない。

- Example: dispatch 前は `Stopped`/position 0/tg 0/pg 0。
  `fast_control` の内部 `Play` が head で apply されると
  `Playing`/anchor 位置/tg 1/pg 1 になる
  (`pc_c4_07_01`)。

### Rule MIRROR-02 — position は node cursor と同期する

`position_ticks` は observed head ごとに scheduler の `pos_at` と
一致して進み、window 内 event は真の apply frame に `Dispatched`
投影される — plugin 側 delivered log とも一致する
(`pc_c4_07_02`)。

### Rule MIRROR-03 — generation gate は fail-closed

surface generation 不一致の `observe_block` は `None`。generation
swap は新 node instance ごとに fresh mirror (初期 `Stopped`) を
mint し、旧 generation の観測を遮断する (`pc_c4_07_03`,
`pc_c4_07_09`)。

### Rule MIRROR-04 — commanded transport 変化へ追従する

`boundary_stage_window` の `Stop`/`Seek`/`Pause`/`SetLoop`/
`ClearLoop` は絶対 apply frame で mirror に fold され、span ctx
が status/position/loop/occurrence/tg を反映する。window 内の
apply は投影を分割し、後続 segment が真の位置線で event を
投影する (`pc_c4_07_04`–`08`)。

### Rule MIRROR-05 — receipts が generation 軸を確定する

merged receipts を `note_receipts` が適用し、次の span ctx の
`transport_generation`/`plan_generation` は最後に drain された
receipt の値と一致する (`pc_c4_07_10`)。

## Feature: capability-drift → republish → rebind

### Rule REBIND-01 — drift は publish を block する

`register_params`/`register_boundary` による registration epoch
bump は `capability_generation` を動かし、prepare→publish 間の
publish は `PluginCapabilityChanged` で拒否され、last-good plan
は生き続ける。`reconcile` は `CapabilityGeneration` mismatch を
報告する (`pc_c4_07_rebind_params_drift_junction`,
`pc_c4_07_rebind_boundary_drift_blocks_publish`)。

### Rule REBIND-02 — republish + sync が新 pin で rebind する

新 `prepare` は新しい host capability generation を pin し、
publish 成功・RT が新 generation を観測・`sync` が
`graph_generation` = 新 plan generation の binding を mint する
(各 prepare が新 plugin instance を接続することを instance log
で確認)。stale-generation の `observe_block`/`fast_control`/
`boundary_stage_window`/`drain_output_reports` は全て gate される。
