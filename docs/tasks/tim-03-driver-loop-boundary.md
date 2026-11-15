---
id: "TASK-034XI7BEPPFTBDNJzgkoJh"
type: "task"
status: "done"
title: "TIM-03 driver-side loop-wrap boundary derivation"
created: "2026-11-14"
updated: "2026-11-14"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/plans/tim-03-driver-loop-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim-03-driver-loop-boundary.md"
  derived-by: []
  verifies:
    - "docs/test-specs/tim-03-driver-loop-boundary.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Task — TIM-03 driver-side loop-wrap boundary derivation

- [x] `AttachedDispatch` に `wrap_sim`/`sim_window`/
      `staged_transport`/`last_window_plan` を追加し
      `prepare_attached` で初期化
- [x] `anchor_link` を `AttachedDispatch` method 化 +
      `loop_wrap_boundaries` preview helper
- [x] `stage_node_transport`/`stage_seek`/`anchor_link` の
      `stage_window` 3 箇所へ導出 `LoopWrap` point 供給
- [x] `observe_attached_block` で 1 窓 1 回 wrap_sim advance
      （node cursor ドメイン `sim_window`）
- [x] `attached_window_plan` accessor + import 更新
- [x] `tests/tim03_driver_loop_boundary.rs` 新規（6 test、DB-01..06）
- [x] `mise run test synvelis-player --test tim03_driver_loop_boundary`
      全 pass
- [ ] 全検証 lane（fmt-check/check/lint/test/test-rt/
      check-headless-deps/prek）— evidence に記録
- [ ] review-protocol 1 cycle・blocker 0
