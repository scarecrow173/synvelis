---
id: "PLAN-yI0tRFYPUF425trgH7V1QP"
type: "plan"
status: "approved"
title: "RCD-02 Blob SHA-256 Promotion Plan"
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
    - "docs/specs/rcd-02-blob-sha256.md"
  implemented-by:
    - "docs/tasks/rcd-02-task4-blob-sha256.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/rcd-02-stagedfile-publish-migration-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-02-blob-sha256.md"
    - "docs/designs/rcd-02-blob-sha256-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-02-blob-sha256-digest.md"
  verified-by:
    - "docs/test-specs/rcd-02-blob-sha256-digest.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  defers: []
  deferred-by: []
---

# RCD-02 Blob SHA-256 Promotion Plan Implementation Plan

## Goal

`publish_recorded` の canonical blob を `blobs/sha256/<hex>`
名前空間へ `sha256:<hex>` digest で publish し、receipt/commit/
playback が同一の強 digest identity を運ぶようにする。legacy
CRC32 receipt は dual-scheme で維持する。

## Scope

単一実装 stream。変更は `crates/synvelis-player`（publish.rs +
必要時 blob.rs/import.rs + 新規 test）と `crates/synvelis-recording`
（digest doc 更新 + 必要 test）と `docs/`・`evidence/` の本 slice
文書のみ。synvelis-library/project/render/app には触れない。

## Upstream Documents

- `docs/specs/rcd-02-blob-sha256.md`

## Design Inputs

- `docs/designs/rcd-02-blob-sha256-design.md`

## File Map

- `crates/synvelis-player/src/publish.rs` — canonical blob の
  SHA-256 publish・`publish_recorded_with`・reconcile・legacy
  dual-scheme
- `crates/synvelis-player/src/blob.rs` / `import.rs` — digest
  kind 分類の共有 helper（必要最小限）
- `crates/synvelis-player/Cargo.toml` — `synvelis-library` 依存
  確認
- `crates/synvelis-player/tests/rcd02_blob_digest.rs` — 新規 test
- `crates/synvelis-recording/src/staging.rs` — `CanonicalSourceRef`
  .digest doc 更新
- `crates/synvelis-recording/src/commit.rs` — digest field doc 更新
- `crates/synvelis-recording/tests/` — 必要 test 更新
- `docs/test-specs/rcd-02-blob-sha256-digest.md` — test-spec
- `docs/impl/ir/rcd-02-blob-sha256.md` — 実装記録
- `docs/impl/exp/rcd-02-blob-sha256.md` — experiment log
- `evidence/sv116/` — 検証記録
- `docs/*/README.md` — doc-maintenance による index 再生成

## Implementation Sequence

- [ ] 現行 `publish.rs` の WAV 生成・digest・publish・receipt
      経路を精読し、WAV byte 決定性を確認
- [ ] `publish_recorded_with` を追加し canonical blob publish を
      `BlobPublisher` port 経由の SHA-256 へ切替え
- [ ] receipt digest kind 分岐（sha256 / crc32 legacy / opaque
      fail-closed）と indeterminate reconcile を実装
- [ ] `staging.rs`/`commit.rs` の digest doc 更新
- [ ] `crates/synvelis-player/tests/rcd02_blob_digest.rs` 作成・
      Pass 確認
- [ ] recording 側 test の整合確認・必要更新
- [ ] `docs/test-specs`/`docs/impl`/`evidence/sv116` 記録・
      index 再生成
- [ ] `git fetch origin main && git merge origin/main` 後に
      最終検証（fmt-check/check/lint/test × player/recording）

## Dependencies and Risks

- `synvelis-render/tests/x_e2e.rs` が legacy flat blob layout を
  assert している場合 fail する — 範囲外のため Coordinator 確認。
- `BlobPublisher` port の object safety/Send 性を確認。
- WAV in-memory 生成の byte 決定性を確認（temp file 経路と同一）。

## Task Breakdown

- Task 4: `docs/tasks/rcd-02-task4-blob-sha256.md`（全ファイルを
  単一 task で実装）

## Verification

- [ ] `mise run fmt-check`
- [ ] `mise run check synvelis-player`
- [ ] `mise run lint synvelis-player`
- [ ] `mise run test synvelis-player`
- [ ] `mise run check synvelis-recording`
- [ ] `mise run lint synvelis-recording`
- [ ] `mise run test synvelis-recording`
- [ ] 新規 `rcd02_blob_digest.rs` Pass
- [ ] 実 OS power-loss・他 OS real-FS・実 device は NotRun

## Open Questions / Gaps

- `synvelis-render/tests/x_e2e.rs` の flat blob assertion が新
  layout で fail する場合の対応（範囲拡張 or 別 slice）—
  実装後に実測して Coordinator へ報告。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（Orca worker dispatch 内）
- Independent work streams: none
- User approval required before dispatch: no（review-protocol で代替）
- If approved: execute task-by-task with review checkpoints.
