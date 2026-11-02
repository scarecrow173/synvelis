---
id: "TASK-034QAtPmN1oYgkip3TL6wG"
type: "task"
status: "done"
title: "ED-A task1 ProjectOp batch ops + selection model + ViewState 移行"
created: "2026-11-02"
updated: "2026-11-02"
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
    - "docs/plans/ed-a-editing-model.md"
  implemented-by: []
  depends-on:
    - "docs/plans/ed-a-editing-model.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ed-a-e2e-editing-model.md"
  references: []
  defers: []
  deferred-by: []
---

# ED-A task1 ProjectOp batch ops + selection model + ViewState 移行

## Work

- `synvelis-project/src/ops.rs`: `ProjectOp::Composite` (validate-all →
  scratch 逐次 apply, 入れ子禁止, 空拒否), `SplitClip` (musical anchor のみ,
  `clip_b_source_offset` 単調性検証, lane.clip_ids 維持), `RemoveClipsInRange`
  (lane 集合 × musical range overlap 除去)。
- `synvelis-workspace/src/selection.rs`: `SelectionDomain`,
  `ObjectSelections` (active domain + per-domain map, 混在拒否),
  `TimeSelection`, `LaneScope`, `EditTimeDomain`, `EditBinding`。
- `state.rs`: `editing_selection` → `object_selections` 移行 + `time_selection`
  `lane_scope` `active_tool` `grid` `snap` `edit_binding` `gesture`(`serde skip`)。
- `persist.rs`: `WORKSPACE_SCHEMA` = 2。ws_a.rs の selection 書き込みを
  新 API へ追従。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/ed-a-editing-model.md
- docs/designs/ed-a-editing-model-design.md

## Dependencies

- なし

## Affected Files

- crates/synvelis-project/src/ops.rs
- crates/synvelis-workspace/src/{selection,state,persist,error,lib}.rs
- crates/synvelis-workspace/tests/ws_a.rs

## Done When

- [ ] `mise run check` / `mise run test` が green (ws_a 回帰含む)。

## Verification

- [ ] `mise run fmt-check` / `check` / `test` PASS
