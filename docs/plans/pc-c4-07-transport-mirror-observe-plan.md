---
id: "PLAN-7c4e9a1b2d3f4e5a6b7c8d9e0f1a2b"
type: "plan"
status: "approved"
title: "PC-C4-07 transport mirror + observe_block plan"
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
    - "docs/designs/pc-c4-07-transport-mirror-observe-design.md"
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
  references:
    - "docs/tasks/pc-c4-07-transport-mirror-observe-task.md"
---

# PC-C4-07 plan

## スコープ

1. `driver.rs`: `TransportMirror` と関連 method を `pub(crate)` 化、
   `note_pause` を追加 (Pause の完全な command coverage)。
2. `routing_control.rs`: `NodeSurface` に mirror + staged/scheduled
   command tracking + `last_span` を追加。`fast_control`/`flush_batches`
   成功時に内部 `Play` を記録、`boundary_stage_window` 成功時に
   commands を記録。`observe_block` + `observe_node_block` +
   `note_transport_command` + `last_observed_span` を実装。
3. `lib.rs`: export 変更なし (全て crate 内または既存 export で足りる)。
4. Tests: `tests/pc_c4_07_transport_observe.rs` (10 件)、
   `tests/pc_c4_07_capability_rebind.rs` (2 件)。
5. Docs: spec/design/plan/task/test-spec/impl-ir。

## 非スコープ

app 層配線・`syn.control.status` `plan_generation` 露出・production
`PluginHost` composition・wire schema L1・real VST3/OS/GUI/AudioLens
(外部 gate = NotRun)。

## 検証

canonical: `mise run fmt-check check lint test test-rt
check-app-native lint-app-native test-msrv build`。
