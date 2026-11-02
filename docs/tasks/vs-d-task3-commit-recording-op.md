---
id: "TASK-g9NPYtRA6rCjhXiJ6FRPf1"
type: "task"
status: "done"
title: "VS-D task3 commit recording op"
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
    - "docs/tasks/vs-d-task2-commit-manifest-plan.md"
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

# VS-D task3 commit recording op

## Work

RecordingOp::CommitRecording + driver

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-d-recording-commit-e2e.md
- docs/test-specs/vs-d-e2e-r3-recording-commit.md

## Dependencies

- docs/tasks/vs-d-task2-commit-manifest-plan.md

## Affected Files

- RecordingOp::CommitRecording の validate 全検査（参照/ID衝突/範囲/clock/session Closed/hash照合）・apply 全登録（asset→source Verified→Committed→pass→segment→take→clip・session Finalized・registry・reseed）と driver prepare_manifest/commit_session を実装。

## Done When

- [ ] crates/synvelis-recording/src/ops.rs / commit.rs|commit が単一 mutation で適用され、validate 失敗で project 不変・apply 成功で registry 記録・reseed が watermark を維持

## Verification

- [ ] `mise run check` が通る・関連単体試験緑
