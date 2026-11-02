---
id: "TASK-034PCA07Sn2bVw8cEfSgHi"
type: "task"
status: "done"
title: "PC-A task3 eval + control graph + capability/dispatch"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/pc-a-parameter-control.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-a-parameter-control.md"
    - "docs/tasks/pc-a-task1-control-model-deps.md"
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

# PC-A task3 eval + control graph + capability/dispatch

## Work

- `control.rs` eval: `ScopeResolution`/`lane_value` (chase 評価)/
  `evaluate_control` (reference + contributions + clamp + quantize +
  host_resolved + plain)/`ControlEvaluation`/`Contribution`/
  `ReferenceSource`。
- `control.rs` graph: `ControlGraph::compile` (topo 順、`ControlCycle`
  拒否) + `EvalPlan`。
- `control.rs` dispatch: `dispatch_check`/`lower_event`/
  `quantize_value`/`modulation_allowed` (rf#186 `check_param_write`/
  `ParamEvent`/`TimedParamEvent` 消費)。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/pc-a-parameter-control.md
- docs/designs/pc-a-parameter-control-design.md

## Dependencies

- docs/tasks/pc-a-task1-control-model-deps.md

## Verification

- `mise run check` PASS (eval/graph/dispatch compile + project 単体
  test)。
