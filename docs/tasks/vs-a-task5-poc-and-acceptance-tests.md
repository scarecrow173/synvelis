---
id: "TASK-034Qe0lVcKRcJl9zeQENb4"
type: "task"
status: "done"
title: "VS-A task5 poc and acceptance tests"
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
    - "docs/tasks/vs-a-task4-recording-ops-validation.md"
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

# VS-A task5 poc and acceptance tests

## Work

recording-poc 決定的シナリオ（DirectoryBundleStore・固定 path）、AC 全網羅テスト（source_state/poc_determinism を含む）、test-spec の対応表との一致確認。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-a-project-recording-foundation.md
- docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md

## Dependencies

- 直列依存（plan Task Breakdown 参照）

## Affected Files

- crates/recording-poc/src/main.rs
- crates/*/tests/{source_state,poc_determinism}.rs

## Done When

- [ ] AC1-AC13 全ての対応テストが緑、poc 2 回実行同一出力

## Verification

- [ ] 対応 test-spec 項目が緑（mise run test）
