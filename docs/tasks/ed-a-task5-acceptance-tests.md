---
id: "TASK-034QAu59SzY753wmZUJZ5T"
type: "task"
status: "done"
title: "ED-A task5 受入 test ED-A-01..05"
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
    - "docs/tasks/ed-a-task4-commands-workspace-api.md"
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

# ED-A task5 受入 test ED-A-01..05

## Work

- `tests/ed_a.rs`: test-spec 準拠の受入 test。selection 分離/domain 独立/
  primary/time-selection 非 delete/delete-range、tool 解決/固定/ambiguous、
  snap/grid/domain/priority/hysteresis、preview 分離/atomic/1 undo/
  stale/cancel/undo 配送、edit_binding/link/gesture 非破壊。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/ed-a-editing-model.md
- docs/test-specs/ed-a-e2e-editing-model.md

## Dependencies

- docs/tasks/ed-a-task4-commands-workspace-api.md

## Affected Files

- crates/synvelis-workspace/tests/ed_a.rs

## Done When

- [ ] ED-A-01..05 全 test PASS。

## Verification

- [ ] `mise run test` 全 green
