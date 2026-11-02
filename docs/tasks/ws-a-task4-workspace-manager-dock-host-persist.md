---
id: "TASK-034Qx10viwoP7xnqZBuC1p"
type: "task"
status: "todo"
title: "WS-A task4 workspace manager・dock lifecycle・fake host・persist"
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
    - "docs/tasks/ws-a-task2-linkgroup-mapping-session.md"
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

- workspace.rs: WorkspaceManager。DockWorkspace + Window 台帳
  (WindowKey/geometry) + registries + focus + link + ProjectSession。
  detach_to_new_window (begin→create→register→commit・失敗 rollback)、
  move_view、on_window_event (Focused/CloseRequested veto or
  Approve+DetachViews/Destroyed)、view edit lock (ViewBusy)。
- host.rs: FakeWindowHost (WindowHandle 実装・create 失敗注入・
  close/eval 記録)。
- persist.rs: WorkspaceSnapshot envelope (schema version・window keys
  順序対応・LayoutSnapshot・view descriptors+state・link groups) +
  restore_or_default 経由の安全復元。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/ws-a-workspace-foundation.md
- docs/test-specs/ws-a-e2e-workspace-foundation.md

## Dependencies

- docs/tasks/ws-a-task2-linkgroup-mapping-session.md

## Affected Files

- crates/synvelis-workspace/src/workspace.rs
- crates/synvelis-workspace/src/host.rs
- crates/synvelis-workspace/src/persist.rs

## Done When

- [ ] 単体 test: detach→reattach・move rollback・close veto/approve・
  snapshot 保存/復元/破損復帰が通る。

## Verification

- [ ] `mise run test` (synvelis-workspace) PASS
