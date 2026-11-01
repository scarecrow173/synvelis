---
id: "TASK-034QfQYNLu0fCICEAgIdjS"
type: "task"
status: "todo"
title: "VS-C task2 staging faultinj"
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
    - "docs/tasks/vs-c-task1-crc32-journal-codec.md"
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

# VS-C task2 staging faultinj

## Work

`staging.rs`（`StagingArea::create/open`・session dir 配置・`manifest.json` 書込み・stream 別 pcm/journal/finalized path・`atomic_rename`/`file_sync`/`directory_sync` capability 検出・tmp→sync→rename publish）と `faultinj.rs`（`StorageWriter` trait・`FaultWriter` の FailWrite/ShortWrite/FailSync/TruncateAfter・JournalCorrupt 補助）を実装。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/vs-c-staging-writer-journal-crash-recovery.md
- docs/designs/vs-c-staging-writer-journal-crash-recovery-verified-media-range.md

## Dependencies

- task1（journal codec の append/replay を staging 配置へ適用）

## Affected Files

- crates/synvelis-recording/src/{staging,faultinj,lib}.rs
- crates/synvelis-recording/Cargo.toml（serde/serde_json 依存確認）

## Done When

- [ ] `create` が manifest + 空 stream files を atomic に配置し `open` が復元する
- [ ] capability が検出・manifest に記録される
- [ ] FaultWriter が FailWrite/ShortWrite/FailSync/TruncateAfter を決定的に注入する

## Verification

- [ ] `mise run check`・`mise run lint`・staging/faultinj 単体試験緑
