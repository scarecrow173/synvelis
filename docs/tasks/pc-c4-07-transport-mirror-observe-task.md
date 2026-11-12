---
id: "TASK-7c4e9a1b2d3f4e5a6b7c8d9e0f1a2b3c"
type: "task"
status: "done"
title: "PC-C4-07 transport mirror + observe_block + capability rebind"
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
    - "docs/plans/pc-c4-07-transport-mirror-observe-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-07-transport-mirror-observe-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-07-transport-mirror-observe.md"
  verified-by: []
  references: []
---

# PC-C4-07 task

- [x] `TransportMirror` を `pub(crate)` 化 + `note_pause` 追加
- [x] `NodeSurface` に mirror/staged/scheduled/last_span 追加
- [x] `fast_control`/`flush_batches`/`boundary_stage_window` で
      command 記録
- [x] `observe_block` + per-node segment observer +
      `note_transport_command` + `last_observed_span` 実装
- [x] `tests/pc_c4_07_transport_observe.rs` 10 件
- [x] `tests/pc_c4_07_capability_rebind.rs` 2 件
- [x] canonical `mise` 検証
