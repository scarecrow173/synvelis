---
id: "TASK-034PCA06Rm1aUv7bDeRfGh"
type: "task"
status: "done"
title: "PC-A task2 control ProjectOp + conflict detection"
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

# PC-A task2 control ProjectOp + conflict detection

## Work

- `error.rs`: `AutomationConflict {target, detail}` / `ControlCycle
  {path}` 追加。
- `control.rs`: `AutomationConflict`/`ConflictPolicy`/`detect_conflicts`。
- `ops.rs`: `AddParameterTarget`/`RemoveParameterTarget`/
  `SetParameterBase`/`AddAutomationLane`/`RemoveAutomationLane`/
  `WriteAutomationRegion`/`EraseAutomationRange`/`AddModulationSource`/
  `RemoveModulationSource`/`SetModulationAssignment`/
  `RemoveModulationAssignment`。検証: target 存在、値域 [0,1]、
  READONLY への base/write 拒否、region 整合、Reject policy で
  conflict → `Error::AutomationConflict`、Replace で明示上書き。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/pc-a-parameter-control.md
- docs/designs/pc-a-parameter-control-design.md

## Dependencies

- docs/tasks/pc-a-task1-control-model-deps.md

## Verification

- `mise run check` + `mise run test` PASS (project 単体 test 含む)。
