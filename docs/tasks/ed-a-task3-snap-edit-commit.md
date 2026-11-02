---
id: "TASK-034QAu2bJQYSQOBkB7eOoz"
type: "task"
status: "done"
title: "ED-A task3 snap engine + edit op 構築 + commit/cancel 経路"
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
    - "docs/tasks/ed-a-task2-tool-gesture-model.md"
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

# ED-A task3 snap engine + edit op 構築 + commit/cancel 経路

## Work

- `snap.rs`: `GridConfig`/`SnapConfig`/`SnapUnit`(Explicit|Auto)/
  `SnapTarget`/`SnapMode`(Absolute|Relative)/`SnapEnvironment`/`SnapWinner`/
  `resolve_snap` (domain 絞り・threshold・priority・hysteresis・label)。
- `edit.rs`: time-domain 導出 (anchor→EditTimeDomain)、MoveClips/Trim/
  Split/Erase/Draw/Marquee の `ProjectOp` 構築、mixed-domain 拒否、
  primary-only snap delta 適用。
- gesture commit: build ops → 0 件=selection 更新 / 1 件=直接 apply /
  複数=Composite 1 apply、cancel で preview 破棄。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/ed-a-editing-model.md
- docs/designs/ed-a-editing-model-design.md

## Dependencies

- docs/tasks/ed-a-task2-tool-gesture-model.md

## Affected Files

- crates/synvelis-workspace/src/{snap,edit,gesture}.rs

## Done When

- [ ] snap 解決・commit/cancel が `check`/`lint` を通す。

## Verification

- [ ] `mise run fmt-check` / `check` / `lint` PASS
