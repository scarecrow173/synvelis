---
id: "TASK-034Qe0k3uLHDv2zcx1jH15"
type: "task"
status: "done"
title: "VS-A task2 synvelis-project data model"
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
    - "docs/tasks/vs-a-task1-workspace-skeleton.md"
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

# VS-A task2 synvelis-project data model

## Work

synvelis-project のデータモデル層: id.rs（全 typed ID + IdGenerator + compile_fail doctest）、time.rs（SessionFrame/SourceFrame/MusicalTick/ProjectTime/MediaTimeValue、checked のみ）、range.rs（Range<F> 系 + MediaSpan）、tempo.rs（TempoMapData/TempoMapRef/DTO/ProjectPpqn/TickTimeMapping）、project.rs + recording_data.rs（全 POD + SessionState/TakeState + SourceState）。serde は deny_unknown_fields。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-a-project-recording-foundation.md
- docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md

## Dependencies

- 直列依存（plan Task Breakdown 参照）

## Affected Files

- crates/synvelis-project/src/{id,time,range,tempo,project,recording_data}.rs
- crates/synvelis-project/tests/{id_roundtrip,time_checked,range,tempo}.rs

## Done When

- [ ] AC2/AC3/AC4/AC10 相当テストが緑

## Verification

- [ ] 対応 test-spec 項目が緑（mise run test）
