---
id: "TASK-034Qx11uIekuC4n8fmY3Wh"
type: "task"
status: "todo"
title: "WS-A task6 merge・再生成・最終検証・impl record"
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
    - "docs/tasks/ws-a-task5-acceptance-tests.md"
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

- `git merge origin/main` + 生成物再生成 (Cargo.lock・doc index)。
- `mise run fmt-check/check/lint/test` 最終検証 + review-protocol。
- impl record (docs/impl/ir) に検証表・ruviform rev・未検証項目を記録。
- followup-triage → exit-audit を通す。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/ws-a-workspace-foundation.md
- docs/test-specs/ws-a-e2e-workspace-foundation.md

## Dependencies

- docs/tasks/ws-a-task5-acceptance-tests.md

## Affected Files

- Cargo.lock / docs/impl/ir/ws-a-*.md

## Done When

- [ ] 全検証 PASS・impl record 記録・graph が complete へ到達。

## Verification

- [ ] `mise run fmt-check && mise run check && mise run lint && mise run test`
