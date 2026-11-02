---
id: "TASK-034Qx10154Li5NXfNP7baP"
type: "task"
status: "done"
title: "WS-A task3 command router・target解決"
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

- command.rs: CommandKind (Application/Project/Transport/View/Editing)、
  CommandContext、InputContext (View/TextInput/Ime/PluginGui/Modal)、
  CommandRouter、ResolveOutcome (Applied/Suppressed/Unresolved/Rejected)。
- target は発行時確定: explicit_target_ids → 発信元 selection_snapshot
  → 両方無し/種別不整合で Unresolved。他 view/window の selection に
  fallback しない。Project/Editing は ProjectController::apply で
  再検証 (StaleRevision 含む)。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/ws-a-workspace-foundation.md
- docs/test-specs/ws-a-e2e-workspace-foundation.md

## Dependencies

- docs/tasks/ws-a-task2-linkgroup-mapping-session.md

## Affected Files

- crates/synvelis-workspace/src/command.rs

## Done When

- [ ] 単体 test: 明示 target・snapshot・unresolved・suppressed・
  stale revision・cross-window fallback 無しが通る。

## Verification

- [ ] `mise run test` (synvelis-workspace) PASS
