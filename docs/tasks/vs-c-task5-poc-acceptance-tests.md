---
id: "TASK-034QfR0WeQaYFoG1702td0"
type: "task"
status: "todo"
title: "VS-C task5 poc acceptance tests"
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
    - "docs/plans/vs-c-staging-writer-journal-crash-recovery.md"
  implemented-by: []
  depends-on:
    - "docs/plans/vs-c-staging-writer-journal-crash-recovery.md"
    - "docs/tasks/vs-c-task4-recovery-scanner.md"
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
    - "docs/test-specs/vs-c-e2e-r2-staging-writer-journal-crash-recovery.md"
  references: []
  defers: []
  deferred-by: []
---

# VS-C task5 poc acceptance tests

## Work

`recording-poc` に VS-C シナリオ（Normal/2ch/Multi-stream/Punch+Loop + StagingWriter + finalize + recovery デモ）と crash モード（`--vsc-crash-at`）を追加。`tests/vs_c.rs` の全 AC 網羅と `recording-poc/tests/` の Process Kill→reopen 試験・`poc_determinism` 更新。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-c-staging-writer-journal-crash-recovery.md
- docs/test-specs/vs-c-e2e-r2-staging-writer-journal-crash-recovery.md

## Dependencies

- task4（recovery/finalize の全経路）

## Affected Files

- crates/recording-poc/src/main.rs
- crates/recording-poc/tests/{poc_determinism.rs,vsc_crash.rs}
- crates/synvelis-recording/tests/vs_c.rs

## Done When

- [ ] test-spec の AC→test 対応表の全テストが存在し緑
- [ ] 子 process 異常終了→reopen→Last Valid Checkpoint 復旧が実測される
- [ ] `recording-poc` が 2 回同一 stdout

## Verification

- [ ] `mise run test` 全緑・`recording-poc` 2 回同一出力
