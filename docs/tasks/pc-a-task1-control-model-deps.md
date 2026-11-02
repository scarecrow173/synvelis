---
id: "TASK-034PCA05Ql0zTu6aCdQeFg"
type: "task"
status: "done"
title: "PC-A task1 ruviform-plugin-core dep + control model + Project::control + schema 2"
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

# PC-A task1 ruviform-plugin-core dep + control model + Project::control + schema 2

## Work

- root `Cargo.toml` + `crates/synvelis-project/Cargo.toml` に
  `ruviform-plugin-core` (同一 git rev) を追加し `Cargo.lock` 更新。
- `id.rs`: `ParameterTargetId`/`AutomationLaneId`/`ModulationSourceId`/
  `ModulationAssignmentId`/`NoteId`。
- `control.rs` (新規): `BeatPoint`/`BeatRange`、`ControlScope`、
  `ValueKind`/`ParamMapping`/`TargetCapability`、`ParameterTarget`、
  `AutomationPoint`/`Interpolation`/`AutomationRegion`/`AutomationLane`、
  `LfoShape`/`SourceKind`/`ModulationSource`、`Polarity`/`Transform`/
  `ModulationAssignment`、`ControlModel`。
- `project.rs`: `Project::control` + `validate_references` 拡張
  (lane→target、assignment→source/target/depth_source、scope 所有参照、
  lane 内 region 重複)。
- `store.rs`: `SCHEMA_VERSION` 1→2。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/pc-a-parameter-control.md
- docs/designs/pc-a-parameter-control-design.md

## Dependencies

- なし (起点)

## Verification

- `mise run check` PASS (model compile)。
- `cargo tree` で ruviform rev 変更なし (crate 追加のみ)。
