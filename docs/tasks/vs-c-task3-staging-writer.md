---
id: "TASK-034QfQZsTXhM5gVQP0gh81"
type: "task"
status: "todo"
title: "VS-C task3 staging writer"
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
    - "docs/tasks/vs-c-task2-staging-faultinj.md"
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

# VS-C task3 staging writer

## Work

`writer.rs`（`StagingWriter`・`StreamWriter` 状態機械・`pump` の連続整合検証と決定的書込み・`checkpoint`・`finish`/`finalize`・`FinalizedSourceRef` 生成・held/退役照合・`WriterError`）を実装。`StorageWriter` 経由で faultinj と同一経路を通す。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-c-staging-writer-journal-crash-recovery.md
- docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md
- docs/test-specs/vs-c-e2e-r2-staging-writer-journal-crash-recovery.md

## Dependencies

- task1（journal codec）
- task2（staging 配置・StorageWriter）

## Affected Files

- crates/synvelis-recording/src/{writer,lib}.rs
- crates/synvelis-recording/tests/vs_c.rs（writer 正常系 + fault 試験）

## Done When

- [ ] pump が header/spans の連続整合を検証し決定的順序で media+journal へ追記する
- [ ] checkpoint が media sync→record→journal sync→dir sync（capability 時）の順序で SyncFailed を区別する
- [ ] finalize が byte length/format/frame count/checksum/journal 整合を再検証し tmp→rename で原子公開・`AlreadyFinalized` を返す
- [ ] held/retained/free drain/filled drain の総数照合を維持する

## Verification

- [ ] `mise run check`・`mise run lint`・writer 正常系/fault 試験緑
