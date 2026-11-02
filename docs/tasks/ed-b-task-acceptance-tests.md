---
id: "TASK-034QN1YPq2DVRcHKewnTAs"
type: "task"
status: "done"
title: "ED-B task: ed_b acceptance tests"
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
    - "docs/plans/ed-b-media-domain-editing.md"
  implemented-by: []
  depends-on:
    - "docs/plans/ed-b-media-domain-editing.md"
    - "docs/tasks/ed-b-task-media-commit.md"
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
    - "docs/test-specs/ed-b-e2e-media-domain-editing.md"
  references: []
  defers: []
  deferred-by: []
---

# ED-B task: ed_b acceptance tests

## Work

`crates/synvelis-workspace/tests/ed_b.rs` に spec ED-B-01..07 を対応
する headless 受入 test を実装。fixture は ed_a と同じ
`FakeWindowHost` + semantic `HitProbe` 経路に、Absolute×Absolute
media clip と混在 clip (Musical×Absolute / Absolute×Musical)、
mapping 3 種を追加。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/ed-b-media-domain-editing.md`
- `docs/test-specs/ed-b-e2e-media-domain-editing.md`
- `docs/specs/ed-b-media-domain-move-slip-commit-semantics.md`

## Dependencies

- `docs/tasks/ed-b-task-media-commit.md` (commit 経路が先)。

## Affected Files

- `crates/synvelis-workspace/tests/ed_b.rs` (新規)

## Done When

- [x] ED-B-01..07 相当の test が存在し全 PASS。
- [x] 既存 test 全 PASS (非回帰)。

## Verification

- [x] `mise run test` PASS。
