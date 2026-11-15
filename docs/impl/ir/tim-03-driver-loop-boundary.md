---
id: "IMPL-034XI9rHssjmyof9bdB7Jc"
type: "impl"
status: "completed"
title: "tim-03-driver-loop-boundary"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/tim03_driver_loop_boundary.rs"
      - type: "file"
        path: "docs/specs/tim-03-driver-loop-boundary.md"
      - type: "file"
        path: "docs/designs/tim-03-driver-loop-boundary.md"
      - type: "file"
        path: "docs/plans/tim-03-driver-loop-boundary.md"
      - type: "file"
        path: "docs/tasks/tim-03-driver-loop-boundary.md"
      - type: "file"
        path: "docs/test-specs/tim-03-driver-loop-boundary.md"
      - type: "file"
        path: "docs/impl/ir/tim-03-driver-loop-boundary.md"
      - type: "file"
        path: "docs/impl/exp/tim-03-driver-loop-boundary.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/driver.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/tasks/tim-03-driver-loop-boundary.md"
    - "docs/plans/tim-03-driver-loop-boundary.md"
    - "docs/designs/tim-03-driver-loop-boundary.md"
    - "docs/specs/tim-03-driver-loop-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/designs/tim-03-driver-loop-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/tim-03-loop-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim-03-driver-loop-boundary.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/tim-03-driver-loop-boundary.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/tim-03-driver-loop-boundary.md"
    rejected: []
---

# IR — TIM-03 driver-side loop-wrap boundary derivation

## 実装概要

`crates/synvelis-player/src/driver.rs` のみ変更
（`process_boundary.rs` は既存 `pub(crate)` `WrapSim` API で足り、
無変更 — routing 側挙動にも影響なし）。

- `AttachedDispatch` に `wrap_sim: WrapSim`・`sim_window: u64`・
  `staged_transport: Vec<TransportCommand>`・
  `last_window_plan: Option<ProcessBoundaryPlan>` を追加。
  `prepare_attached` で `WrapSim::new(dispatch.frame_base())`、
  `sim_window = self.cursor` で初期化。
- `anchor_link` を `HeadlessDriver` の自由関数から
  `AttachedDispatch` method へ移し、`set_clock` 成功後に
  `sim_window` を更新。sessioned 経路は `loop_wrap_boundaries`
  preview → `stage_window(boundaries, …)` → `staged_transport` 記録
  → `last_window_plan` 保存。legacy 経路も queue 成功分のみ記録。
- `loop_wrap_boundaries(window_frames, commands)`:
  `staged_transport` + 新規 command を全て `(sim_window, cmd)` の
  pending にして `wrap_sim` の clone を `walk_window` — read-only
  preview で live sim を進めない。
- `stage_node_transport`/`stage_seek` も同じ preview → `stage_window`
  → 記録の形に統一。`stage_seek` は `set_clock(epoch, frame)` 成功後
  `sim_window = frame`。
- `observe_attached_block` が各 node の wrap_sim を
  `walk_window(sim_window, call_frames, pending)` で 1 窓 1 回だけ
  進め、`sim_window += block_frames`、`staged_transport.clear()`。
  seek quiesce pass もこの経路を通るため node cursor ドメインが
  driver cursor より先行しても正しく追跡される。
- `HeadlessDriver::attached_window_plan(node_id)` accessor を追加
  （`attached_span` と同型の診断証拠）。

## 検証

`tests/tim03_driver_loop_boundary.rs` 6 test 全 pass
（`mise run test synvelis-player --test tim03_driver_loop_boundary`）。
詳細は `docs/impl/exp/tim-03-driver-loop-boundary.md` と
`evidence/sv127/` の検証記録。残り lane は evidence に実測値で記録。
