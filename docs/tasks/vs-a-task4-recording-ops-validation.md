---
id: "TASK-034Qe0ljzKKQaZoE9tnw0X"
type: "task"
status: "done"
title: "VS-A task4 recording ops validation"
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
    - "docs/tasks/vs-a-task3-controller-ops-store.md"
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

# VS-A task4 recording ops validation

## Work

synvelis-recording: RecordingOp + SynvelisOp + impl Operation、SessionState 遷移表、validate.rs の全参照検証。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-a-project-recording-foundation.md
- docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md

## Dependencies

- 直列依存（plan Task Breakdown 参照）

## Affected Files

- crates/synvelis-recording/src/{lib,ops,session,validate}.rs
- crates/synvelis-recording/tests/{session_state,recording_refs}.rs

## Done When

- [ ] AC5/AC6 相当テストが緑

## Verification

- [ ] 対応 test-spec 項目が緑（mise run test）
