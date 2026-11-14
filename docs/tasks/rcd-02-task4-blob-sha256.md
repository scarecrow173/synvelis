---
id: "TASK-r2UgyGsdhNOwpsUcfgfvuM"
type: "task"
status: "done"
title: "RCD-02 Task4 Blob SHA-256 Promotion"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-02-blob-sha256-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-02-blob-sha256-plan.md"
    - "docs/tasks/rcd-02-task3-stagedfile-publish-migration.md"
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
    - "docs/test-specs/rcd-02-blob-sha256-digest.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-02 Task4 Blob SHA-256 Promotion

## Work

`publish_recorded` の canonical blob publish を legacy CRC32
fold hex16 + flat `blobs/<digest>` から、SHA-256
（`sha256:<hex>` → `blobs/sha256/<hex>`）へ昇格する。
`synvelis-library` の `BlobPublisher` port（production =
`StagedBlobPublisher`）を再利用し、receipt/commit/playback が
同一の強 digest identity を運ぶようにする。legacy CRC32 receipt
は dual-scheme で維持する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-02-blob-sha256-plan.md`
- `docs/specs/rcd-02-blob-sha256.md`
- `docs/designs/rcd-02-blob-sha256-design.md`

## Dependencies

- `docs/tasks/rcd-02-task3-stagedfile-publish-migration.md`
  （同一 RCD-02 plan 系列の先行 slice）

## Affected Files

- `crates/synvelis-player/src/publish.rs` — canonical blob の
  SHA-256 publish・reconcile・legacy dual-scheme
- `crates/synvelis-player/src/blob.rs` / `import.rs` — digest
  kind 分類の共有 helper（必要最小限）
- `crates/synvelis-player/tests/rcd02_blob_digest.rs` — 新規
- `crates/synvelis-recording/src/staging.rs` — digest doc 更新
- `crates/synvelis-recording/src/commit.rs` — digest doc 更新
- `crates/synvelis-recording/tests/` — 必要更新
- `docs/` チェーン・`evidence/sv116/`・index 再生成

## Done When

- [ ] canonical WAV が `blobs/sha256/<hex>` へ publish され、
      receipt/commit digest が `sha256:<hex>` +
      `DigestScheme::ContentDigest` になる
- [ ] `open_blob_verified` が新 digest を round-trip 解決し、
      tamper を fail-closed にする
- [ ] legacy CRC32 receipt が従来どおり flat blob で再利用できる
- [ ] indeterminate publish を観測 reconcile し偽装しない
- [ ] `raw_digest`/`verified_sha256` provenance が維持される
- [ ] OS-L1 検証（fmt-check/check/lint/test × player/recording）
      Pass、実機 lane は NotRun

## Verification

- [ ] `mise run fmt-check` / `check synvelis-player` /
      `lint synvelis-player` / `test synvelis-player` /
      `check synvelis-recording` / `lint synvelis-recording` /
      `test synvelis-recording`
