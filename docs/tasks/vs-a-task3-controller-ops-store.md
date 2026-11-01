---
id: "TASK-034Qe0kHawWuXO1xHGTd8F"
type: "task"
status: "done"
title: "VS-A task3 controller ops store"
created: "2026-11-01"
updated: "2026-11-01"
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
    - "docs/plans/vs-a-project-recording-foundation.md"
  implemented-by: []
  depends-on:
    - "docs/plans/vs-a-project-recording-foundation.md"
    - "docs/tasks/vs-a-task2-synvelis-project-data-model.md"
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
    - "docs/test-specs/vs-a-f0-gate-stable-id-command-save-reload-recording.md"
  references: []
  defers: []
  deferred-by: []
---

# VS-A task3 controller ops store

## Work

controller.rs（Operation/Command<O>/ProjectController<O>/ProjectRevision/SavedState/ExpectedRevision/原子的 apply/Undo/Redo/Save/Load）、ops.rs（ProjectOp）、store.rs（ProjectStore/SnapshotEnvelope strict/DirectoryBundleStore/InMemoryStore）。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-a-project-recording-foundation.md
- docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md

## Dependencies

- 直列依存（plan Task Breakdown 参照）

## Affected Files

- crates/synvelis-project/src/{controller,ops,store}.rs
- crates/synvelis-project/tests/{atomic,undo_redo,schema}.rs

## Done When

- [ ] AC7/AC8/AC9 相当テストが緑

## Verification

- [ ] 対応 test-spec 項目が緑（mise run test）
