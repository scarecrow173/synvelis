---
id: "TASK-034QfQY88NrDeir6YJ5IjM"
type: "task"
status: "todo"
title: "VS-C task1 crc32 journal codec"
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

# VS-C task1 crc32 journal codec

## Work

`crc32.rs`（IEEE CRC32 table + incremental）と `journal.rs`（Record encode/decode・`JournalWriter` append・`JournalReplay` prefix 走査・`StopReason`・`MAX_PAYLOAD`）を実装。payload schema（Chunk/Checkpoint/Finalize）を design §Journal Record 形式どおりに固定。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-c-staging-writer-journal-crash-recovery.md
- docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md
- docs/adr/vs-c-journal-checkpoint-sync-fault-finalized.md

## Dependencies

- なし（最初の実装 slice）

## Affected Files

- crates/synvelis-recording/src/{crc32,journal,lib}.rs
- crates/synvelis-recording/tests/vs_c.rs（codec 単体試験を先行）

## Done When

- [ ] encode→decode roundtrip が全 record kind で一致
- [ ] torn tail/BadChecksum/SequenceGap/BadMagic/BadLength が replay で検出され intact prefix と停止理由を返す
- [ ] 破損以降の byte を変更・削除しない

## Verification

- [ ] `mise run check`・`mise run lint`・codec 単体試験緑
