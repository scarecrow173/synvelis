---
id: "TASK-034PCA08To3cWx9dFgThIj"
type: "task"
status: "done"
title: "PC-A task4 ControlSession + workspace API"
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
    - "docs/tasks/pc-a-task2-control-ops-conflicts.md"
    - "docs/tasks/pc-a-task3-eval-graph-dispatch.md"
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

# PC-A task4 ControlSession + workspace API

## Work

- `control.rs` (新規): `AutomationMode`/`WriteArm`/`PlaybackStatus`/
  `PlaybackState`/`ControlGesture`/`RecordingTransaction`/
  `SealReason`/`ControlObservation`/`ObservationState`/
  `TransportEffect`/`GestureEnd`/`RecordingOutcome`/`ControlBreakdown`/
  `LaneView`/`AssignmentView`/`ControlSession`。
- `session.rs`: `ProjectSession::control` フィールド。
- `workspace.rs`: `control()`/`control_mut()` + set_mode/set_arm/
  record/transport/gesture/stamp API + commit 経路
  (`session.apply` → `distribute_revision`)。
- `error.rs`: `ControlRejected(String)`/`MapRejection` 運搬。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/pc-a-parameter-control.md
- docs/designs/pc-a-parameter-control-design.md

## Dependencies

- docs/tasks/pc-a-task2-control-ops-conflicts.md
- docs/tasks/pc-a-task3-eval-graph-dispatch.md

## Verification

- `mise run check` + `mise run lint` PASS。
- mode 選択が `session.apply` を呼ばないこと (API 構造で保証)。
