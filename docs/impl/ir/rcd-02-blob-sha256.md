---
id: "IMPL-034XA69ntDtCS6oYZ7LgEZ"
type: "impl"
status: "completed"
title: "RCD-02 Blob SHA-256 Promotion"
created: "2026-11-14"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/rcd02_blob_digest.rs"
      - type: "file"
        path: "docs/specs/rcd-02-blob-sha256.md"
      - type: "file"
        path: "docs/designs/rcd-02-blob-sha256-design.md"
      - type: "file"
        path: "docs/plans/rcd-02-blob-sha256-plan.md"
      - type: "file"
        path: "docs/tasks/rcd-02-task4-blob-sha256.md"
      - type: "file"
        path: "docs/test-specs/rcd-02-blob-sha256-digest.md"
      - type: "file"
        path: "docs/impl/ir/rcd-02-blob-sha256.md"
      - type: "file"
        path: "docs/impl/exp/rcd-02-blob-sha256.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/publish.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/commit.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/staging.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/tasks/rcd-02-task4-blob-sha256.md"
    - "docs/specs/rcd-02-blob-sha256.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/rcd-02-stagedfile-publish-migration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-02-blob-sha256-design.md"
    - "docs/plans/rcd-02-blob-sha256-plan.md"
    - "docs/test-specs/rcd-02-blob-sha256-digest.md"
    - "docs/impl/exp/rcd-02-blob-sha256.md"
    - "evidence/sv116/rcd-02-blob-sha256-ledger.md"
    - "docs/impl/ir/rcd-02-stagedfile-publish-migration.md"
    - "docs/impl/ir/rcd-02-project-library-adapter-verified-sha-256-mediareferenceprobe-saved-head.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcd-02-blob-sha256-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-02-blob-sha256-digest.md"
  verified-by:
    - "docs/test-specs/rcd-02-blob-sha256-digest.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/84"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/rcd-02-blob-sha256.md"
    rejected: []
---

# RCD-02 Blob SHA-256 Promotion

## Summary

`publish_recorded` の canonical blob publication を、legacy CRC32
fold（hex16・flat `blobs/<digest>`）から SHA-256 content address
（`"sha256:<64hex>"`・`blobs/sha256/<hex>` 名前空間）へ昇格した。
blob publish は synvelis-library の `BlobPublisher` port
（production: `StagedBlobPublisher`）経由で、publish 失敗は
destination の `open_blob_verified` 観測で reconcile する。
receipt は digest scheme ごとに分類し、legacy CRC32 receipt を
sha256 へ書き換えない。新規 receipt のみ sha256 を記録する。

## Context

- Task: `docs/tasks/rcd-02-task4-blob-sha256.md`
- Spec: `docs/specs/rcd-02-blob-sha256.md`
- Design: `docs/designs/rcd-02-blob-sha256-design.md`
- Plan: `docs/plans/rcd-02-blob-sha256-plan.md`
- Test-spec: `docs/test-specs/rcd-02-blob-sha256-digest.md`
- Experiment: `docs/impl/exp/rcd-02-blob-sha256.md`
- Evidence: `evidence/sv116/rcd-02-blob-sha256-ledger.md`
- Issue: <https://github.com/scarecrow173/synvelis/issues/116>
  （#86 ALIB-02 verified blob machinery の recording 側接続 slice）

## Implementation

### Changes

- `crates/synvelis-player/src/publish.rs`:
  - module doc を sha256 名前空間・receipt scheme 分類・opaque
    fail-closed の契約へ更新。
  - `publish_recorded` は `PrincipalId`/`ProviderContext` を構成し
    `StagedBlobPublisher` で `publish_recorded_with` へ委譲。
    `publish_recorded_with`（port 注入版）を新規公開 —
    fault-injecting `BlobPublisher` double で効果分類を検証可能。
  - `sha256_content_digest`: canonical WAV bytes の SHA-256 を
    `ContentDigest`（`DigestAlgorithm::Sha256`）へ写像。
  - `sha256_blob_path`: `blobs/sha256/<hex>` path 解決
    （`LocalBlobDir`/`blob.rs` と同一 mapping）。
  - `receipt_digest_scheme`: receipt `digest` を `AssetDigest::parse`
    で `Sha256`/`Crc32Fold`/`Opaque` に分類。opaque は
    `PublishError::CorruptMedia` で fail-closed。
  - `ensure_sha256_blob`: 既存 blob は `open_blob_verified` で
    byte 再検証して reuse（同一 open handle を demux/検証に使用）。
    不在時のみ `make_wav` → staged source → `BlobPublisher` publish。
  - `publish_sha256_blob`: staged source を session dir の
    `.publish-NNNN.wav` へ書き、`PublishDurability::
    FileAndDirectorySync` で publish。失敗時は effect 分類を
    信用せず destination を `open_blob_verified` で観測
    reconcile — verified bytes が実在すれば採用（Published /
    Indeterminate-landed）、確定的に不在なら 1 回だけ retry、
    再失敗後に再観測してから失敗を返す（fail-closed）。
    staged file は成否に関わらず除去する。
  - `publish_stream` を `encode_stream_wav`/`reuse_stream`/
    `publish_new_stream` へ分割（clippy `too_many_lines` 対応、
    挙動不変）。receipt 一致 stream は `receipt_digest_scheme`
    で scheme を dispatch — sha256 receipt は verified publisher
    経路、legacy CRC32 receipt は従来の flat `ensure_blob` 経路。
    いずれも `StreamAction::Reused` で receipt を書き換えない。
  - 新規 publish の `CanonicalSourceRef.digest` は
    `"sha256:<64hex>"`、`byte_len`/`source_frames`/`sample_rate`/
    `channels` provenance と `raw_digest` を維持。receipt は
    従来どおり `publish_atomic` で atomic publish。
  - `map_resolve_error`/`map_publish_error`: `ResolveError`・
    `BlobPublishError` を `PublishError` 分類へ写像
    （kind/phase/effect/durability を message に保持）。
- `crates/synvelis-player/src/lib.rs`: `publish_recorded_with`
  を export。
- `crates/synvelis-recording/src/commit.rs`: `DigestScheme`・
  `StreamEntry.digest` の doc を dual-scheme（新規 sha256 /
  旧 receipt legacy CRC32）の実態へ更新（code 不変）。
- `crates/synvelis-recording/src/staging.rs`: `CanonicalSourceRef`
  の doc を digest scheme・名前空間の実態へ更新（code 不変）。
- `crates/synvelis-player/tests/rcd02_blob_digest.rs`（新規）:
  sha256 publish・idempotent reuse・missing blob republish・
  tamper fail-closed・opaque receipt fail-closed・legacy CRC32
  receipt の flat 維持・Published/Indeterminate reconcile・
  unlanded retry fail-closed・provenance fields の 10 test。
- `crates/synvelis-render/tests/x_e2e.rs`（coordinator 承認の
  scope 拡張）: X-01/X-04 の flat layout 前提 assertion を
  `open_blob_verified` + `blobs/sha256/` namespace assert へ
  追従。
- `evidence/sv116/rcd-02-blob-sha256-ledger.md`（新規）: 本 slice
  の headless fixture 証跡 ledger。

### Decisions During Implementation

- blob publish は library 境界（`BlobPublisher` port +
  `StagedBlobPublisher` production 実装）を再利用し、player 側で
  hash/publish/readback を再実装しない — #86 の verified blob
  契約（staged source の stream hash・length/digest 検証・
  readback verify・`PublishMode::CreateNew`）をそのまま得る。
- publish 失敗の reconcile は effect 分類ではなく観測で決める:
  `open_blob_verified` が verified bytes を返せば採用、
  `MissingMedia` のときだけ 1 回 retry、最終失敗は
  `PublishError` へ写像。`Indeterminate` を `Ok` にも一律 `Err`
  にも偽装しない（test-spec RCD02-BS-05/06）。
- staged source は session dir の `.publish-NNNN.wav` —
  `BlobPublisher::publish` が file path を要求するため bytes を
  一時 file 化する。recovery `scan`/`classify_review` は manifest
  記載 path のみを読み未知 file を走査しないため、crash 残留は
  recovery 分類に影響しない（コードコメントに明記）。
- `publish_recorded_with` の port 注入は test 専用 seam —
  production は `publish_recorded` が `StagedBlobPublisher` を
  束縛する。public API 追加は `lib.rs` export のみ。
- `ensure_blob`（legacy flat 経路）は legacy CRC32 receipt の
  reuse のために残置 — 新規 publish では呼ばない。

## Related Experiments

### Adopted

- `docs/impl/exp/rcd-02-blob-sha256.md` — publish 失敗の
  observation-reconcile 設計と port 注入 seam の検証。

### Rejected

<!-- なし -->

## Validation

canonical = mise（補助証拠として cargo 直接実行も記録）:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check synvelis-player` — Pass（exit 0）
- `mise run check synvelis-recording` — Pass（exit 0）
- `mise run lint synvelis-player` — Pass（exit 0）
- `mise run lint synvelis-recording` — Pass（exit 0）
- `mise run test synvelis-player` — Pass（exit 0、全 test binary
  緑。`rcd02_blob_digest` 10 tests を含む）
- `mise run test synvelis-recording` — Pass（exit 0、全 test
  binary 緑）
- `mise run check synvelis-render` — Pass（exit 0）
- `mise run lint synvelis-render` — Pass（exit 0）
- `mise run test synvelis-render` — Pass（exit 0、全 test binary
  緑。`x_e2e` 5 tests を含む）
- 補助: `cargo test -p synvelis-player --test rcd02_blob_digest`
  — 10 passed / 0 failed

OS: Windows (win32)。toolchain: mise pin（`mise.ci.toml` 1.99.0
系、実測 rustc/cargo 1.99.1・PR 本文に記録）。`origin/main`
（f12aa66）merge 後に再検証済み。

NotRun: 実 OS power-loss・Linux/macOS real-FS・実 device 録音
（#115）・製品 GUI E2E（#118）・consumer 照合・synvelis-project
HEAD CAS・`test-rt`/`test-msrv`（canonical 検証コマンド非含有）。

## Risks

- `x_e2e.rs` X-01/X-04 が `media_root/blobs` 直下の flat digest
  file と CRC32 fold 再計算を前提としていたため、namespaced
  layout で fail した。coordinator 承認の scope 拡張で同 test
  file のみ更新済み（`open_blob_verified` による verified
  resolve + sha256 namespace assert へ追従、render src 無変更）。
  設計判断: 新規 publish は全て sha256 canonical のため assert
  は新 layout のみを対象とし、legacy flat の読取互換は
  `blob.rs`/`verified_import` の既存 test
  （`crc32_fold_source_still_resolves` 等）が担保する。
- staged source `.publish-NNNN.wav` は crash 時に session dir に
  残留し得る（recovery 分類への影響なし・次回 publish で上書き
  後除去）。
- `PublishDurability::FileAndDirectorySync` を要求するが、
  `StagedBlobPublisher` は unsupported 時に publish 前の
  `FileSync` へ正直に fallback し receipt に evidence を残す
  （library 側契約）。

## Follow-ups

- 実 OS/FS durability・power-loss 実測（実機 gate）。
- #115 実 device・#118 製品 GUI E2E・#86 consumer 照合。
