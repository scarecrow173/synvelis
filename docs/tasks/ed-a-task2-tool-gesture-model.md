---
id: "TASK-034QAu1d6DTe49e8cgWMtq"
type: "task"
status: "done"
title: "ED-A task2 tool resolution + gesture fixation/preview 型"
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
    - "docs/tasks/ed-a-task1-selection-model-project-ops.md"
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

# ED-A task2 tool resolution + gesture fixation/preview 型

## Work

- `tool.rs`: `Tool`, `HitZone`/`HandleKind`, `HitProbe` (意味 probe),
  `EditOperation`, `ToolResolution` (Resolved/Ambiguous), `resolve_tool`
  (Smart 優先順 Handle>Body>LaneEmpty>Background・同順位複数→Ambiguous・
  dedicated 固定・MoveClips/EraseTargets の selection 内包判定)。
- `gesture.rs`: `Gesture`/`GestureFixation`/`GesturePreview`/`GesturePhase`/
  `CancelReason`/`GestureOutcome` 型と preview 更新規則 (fixation 不変)。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/ed-a-editing-model.md
- docs/designs/ed-a-editing-model-design.md

## Dependencies

- docs/tasks/ed-a-task1-selection-model-project-ops.md

## Affected Files

- crates/synvelis-workspace/src/{tool,gesture,error,lib}.rs

## Done When

- [ ] 解決表・fixation 型が `mise run check`/`lint` を通す。

## Verification

- [ ] `mise run fmt-check` / `check` / `lint` PASS
