---
id: "TASK-UNKbSXncDrwD7F9WA51dDG"
type: "task"
status: "done"
title: "VS-D task1 applied commit registry"
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

# VS-D task1 applied commit registry

## Work

applied_commits registry + draw_raw_id

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-d-recording-commit-e2e.md
- docs/test-specs/vs-d-e2e-r3-recording-commit.md

## Dependencies

- （先行 task なし）

## Affected Files

- synvelis-project の RecordingRegistry に applied_commits（AppliedRecordingCommit・serde default）を追加し、ProjectController に外部 manifest 用 raw ID 発行 draw_raw_id を実装。

## Done When

- [ ] crates/synvelis-project/src/recording_data.rs / controller.rs / lib.rs|applied_commits が serde default で旧 snapshot 読み込み可能・draw_raw_id が generator から発行する

## Verification

- [ ] `mise run check` が通る・関連単体試験緑
