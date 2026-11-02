---
id: "TASK-034Qx0ZvZ742U0JcMeFDx2"
type: "task"
status: "todo"
title: "WS-A task2 link group・semantic mapping・session配信"
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
    - "docs/tasks/ws-a-task1-crate-skeleton-deps-view-state-focus.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

## Work

- link.rs: LinkGroup/LinkEvent/SelectionMapping/LinkGroupManager。
  Clip→Track 等の semantic mapping (project で lane→track 解決)、
  Follow Inspector 追従・Pinned 保持、Group 間独立、event_id dedup +
  no-reemit。
- session.rs: ProjectSession (ProjectController 包摂) + per-view
  last_notified revision + deliver_updates + TransportState stub +
  EngineStub (revision 変化のみ graph_builds++)。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/ws-a-workspace-foundation.md
- docs/designs/ws-a-workspace-foundation-design.md

## Dependencies

- docs/tasks/ws-a-task1-crate-skeleton-deps-view-state-focus.md

## Affected Files

- crates/synvelis-workspace/src/link.rs
- crates/synvelis-workspace/src/session.rs

## Done When

- [ ] 単体 test: Clip→Track mapping・Pinned 保持・Group 独立・dedup・
  revision 配信が通る。

## Verification

- [ ] `mise run test` (synvelis-workspace) PASS
