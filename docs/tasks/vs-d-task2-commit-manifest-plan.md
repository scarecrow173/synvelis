---
id: "TASK-89CafxCbPp0k654XLLwJL4"
type: "task"
status: "done"
title: "VS-D task2 commit manifest plan"
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

# VS-D task2 commit manifest plan

## Work

commit.rs manifest/plan/status/marker

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-d-recording-commit-e2e.md
- docs/test-specs/vs-d-e2e-r3-recording-commit.md

## Dependencies

- （先行 task なし）

## Affected Files

- synvelis-recording の新規 commit.rs に RecordingCommitManifest・CommitStatus・commit_status・payload plan（segment merge・take/clip 構成）・marker 読み書きを実装。

## Done When

- [ ] crates/synvelis-recording/src/commit.rs / lib.rs|manifest が原子公開され既存ファイルは照合再利用される・plan が FinalizedSourceRef から segment/take/clip を構成・content_hash 安定

## Verification

- [ ] `mise run check` が通る・関連単体試験緑
