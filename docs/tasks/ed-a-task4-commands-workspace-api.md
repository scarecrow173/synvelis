---
id: "TASK-034QAu4igyxOwf6eNf7xk9"
type: "task"
status: "done"
title: "ED-A task4 command 追加 + workspace editing API + cancel hook"
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
    - "docs/tasks/ed-a-task3-snap-edit-commit.md"
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

# ED-A task4 command 追加 + workspace editing API + cancel hook

## Work

- `command.rs`: `CommandSpec.required_domain`、新 command
  `edit.undo`/`edit.redo`/`clips.delete-range`/`tool.activate`。
- `workspace.rs`: `pointer_down`/`drag_update`/`commit_gesture`/
  `cancel_gesture`/`set_tool`/`set_time_selection`/`set_lane_scope`/
  `open_in_editor` API、undo lane 配送 (View→session+配信、
  Text/Ime→text_undos stub、Modal/PluginGui→Suppressed)、
  remove_view/window Destroyed の gesture cancel hook。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/ed-a-editing-model.md
- docs/designs/ed-a-editing-model-design.md

## Dependencies

- docs/tasks/ed-a-task3-snap-edit-commit.md

## Affected Files

- crates/synvelis-workspace/src/{command,workspace}.rs

## Done When

- [ ] 新 command と API が `check`/`lint` を通す。

## Verification

- [ ] `mise run fmt-check` / `check` / `lint` PASS
