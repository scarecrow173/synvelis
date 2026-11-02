---
id: "TASK-94KUOSi6E5hLp0z924tx0Z"
type: "task"
status: "done"
title: "VS-D task4 poc vsd scenario"
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
    - "docs/tasks/vs-d-task3-commit-recording-op.md"
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

# VS-D task4 poc vsd scenario

## Work

recording-poc vsd シナリオ

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-d-recording-commit-e2e.md
- docs/test-specs/vs-d-e2e-r3-recording-commit.md

## Dependencies

- docs/tasks/vs-d-task3-commit-recording-op.md

## Affected Files

- recording-poc に vsd サブコマンドを追加し record→finalize→commit→save→reload→undo/redo→retry を決定的に実演。結果比較とレポート出力（既存出力不変）。

## Done When

- [ ] crates/recording-poc/src/main.rs|vsd シナリオが決定的 report を出力し既存 vsc 出力を壊さない

## Verification

- [ ] `mise run check` が通る・関連単体試験緑
