---
id: "TASK-034QfR0eLqVxiQlwHq30L3"
type: "task"
status: "todo"
title: "VS-C task4 recovery scanner"
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
    - "docs/tasks/vs-c-task3-staging-writer.md"
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

# VS-C task4 recovery scanner

## Work

`recovery.rs`（`RecoveryScanner::scan` の staging root 列挙・journal replay・media 照合・Last Valid Checkpoint frontier・`RecoveryReport` 分類 Healthy/Partial/Corrupted/Unjournaled・`recover_finalize`）を実装。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-c-staging-writer-journal-crash-recovery.md
- docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md

## Dependencies

- task3（writer/journal/frontier 意味論）

## Affected Files

- crates/synvelis-recording/src/{recovery,lib}.rs
- crates/synvelis-recording/tests/vs_c.rs（recovery 試験）

## Done When

- [ ] scan が Last Valid Checkpoint を frontier とし unverified/validated tail を区別報告する
- [ ] corrupt journal が最後の intact checkpoint で停止する
- [ ] recover_finalize が verified prefix を `partial` として公開し既存 finalized では `AlreadyFinalized` を返す

## Verification

- [ ] `mise run check`・`mise run lint`・recovery 試験緑
