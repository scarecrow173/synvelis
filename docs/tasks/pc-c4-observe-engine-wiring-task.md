---
id: "TASK-b2e5a8c1f4d90736e5b2a8c2d4e6f7a"
type: "task"
status: "completed"
title: "PC-C4 observe engine wiring task1 — observe_block の engine-driver 配線"
created: "2026-11-15"
updated: "2026-11-15"
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
    - "docs/plans/pc-c4-observe-engine-wiring-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-observe-engine-wiring-plan.md"
    - "docs/specs/pc-c4-observe-engine-wiring.md"
    - "docs/designs/pc-c4-observe-engine-wiring-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c4-observe-engine-wiring-plan.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-observe-engine-wiring.md"
  verified-by:
    - "docs/test-specs/pc-c4-observe-engine-wiring.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C4 observe engine wiring task1 — observe_block の engine-driver 配線

## 作業項目

- [x] player: `ObservedBlock` + `observe_gen`/`observe_head` 追跡 +
  `note_processed_block` + `run_observed`/`seek_observed`
  （`run`/`seek` は no-op delegate で不変）
- [x] app engine: `SessionRoutingEngine::run` の observer 配線、
  `pending_observe_cycles` buffer、`control_drain_observations`、
  publish 時の stale cycle clear
- [x] app: `SynvelisApp::drain_control_observations`（pub 1 pass）+
  `ui_pump` 周期配線
- [x] app: `syn.control.status` の `observe` 節
  （`last_observed_span`/`plan_generation`）+ deferred 記述更新、
  receipt drain-once 意味の doc 明記
- [x] fixture `pc_c4_observe_engine_wiring.rs`: node head 正確性・
  generation 切替・stale gate・feed 伝播・status 露出
- [x] canonical 検証 lane 全 pass
- [x] `pc_c4_06_control_op_surface` の receipt assert を
  drain-once 意味へ更新（実装時に判明した意味変化、exp EXP-2）
