---
id: "TASK-Q55t28lMPEU2KfWWnDjN0v"
type: "task"
status: "done"
title: "VS-D task5 acceptance tests"
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
    - "docs/plans/vs-d-recording-commit-e2e.md"
  implemented-by: []
  depends-on:
    - "docs/plans/vs-d-recording-commit-e2e.md"
    - "docs/tasks/vs-d-task4-poc-vsd-scenario.md"
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
    - "docs/test-specs/vs-d-e2e-r3-recording-commit.md"
  references: []
  defers: []
  deferred-by: []
---

# VS-D task5 acceptance tests

## Work

tests/vs_d.rs VS-001〜015 網羅

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-d-recording-commit-e2e.md
- docs/test-specs/vs-d-e2e-r3-recording-commit.md

## Dependencies

- docs/tasks/vs-d-task4-poc-vsd-scenario.md

## Affected Files

- tests/vs_d.rs に Acceptance Matrix 全項目の実測試験（gate シナリオ・fault/queue/epoch 分離・crash retry・undo/redo・save/reload・revision conflict・integrity conflict・PCM 非混入）を実装。PoC 試験 vsd.rs。

## Done When

- [ ] crates/synvelis-recording/tests/vs_d.rs / recording-poc/tests/vsd.rs|test-spec AC→test 対応表の全テストが存在し緑

## Verification

- [ ] `mise run check` が通る・関連単体試験緑
