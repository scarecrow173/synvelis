---
id: "TASK-034Qx10ZOfjtNkN8JmDyBR"
type: "task"
status: "done"
title: "WS-A task5 受入test WS-A-01..06"
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
    - "docs/plans/ws-a-workspace-foundation.md"
  implemented-by: []
  depends-on:
    - "docs/plans/ws-a-workspace-foundation.md"
    - "docs/tasks/ws-a-task3-command-router.md"
    - "docs/tasks/ws-a-task4-workspace-manager-dock-host-persist.md"
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
    - "docs/test-specs/ws-a-e2e-workspace-foundation.md"
  references: []
  defers: []
  deferred-by: []
---

## Work

- `tests/ws_a.rs`: WS-A-01..06 を個別 test で検証 (test-spec 準拠):
  複数 view 別 window 共有正本/独立状態、link group 連携/Pinned/独立/
  no-loop、command 解決 (明示 target/suppress/unresolved/no fallback)、
  detach→reattach/rollback/close cancel/復元、dock 操作の履歴・RT
  分離、全 test green。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/ws-a-workspace-foundation.md
- docs/test-specs/ws-a-e2e-workspace-foundation.md

## Dependencies

- docs/tasks/ws-a-task3-command-router.md
- docs/tasks/ws-a-task4-workspace-manager-dock-host-persist.md

## Affected Files

- crates/synvelis-workspace/tests/ws_a.rs

## Done When

- [ ] WS-A-01..06 全 test PASS。

## Verification

- [ ] `mise run test` 全 green
