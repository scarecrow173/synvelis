---
id: "SPEC-uMtmtxYDthnvcLOk19wZH4"
type: "spec"
status: "approved"
title: "RCD-02 Blob SHA-256 Promotion — synvelis#116 slice"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-02-stagedfile-publish-migration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/specs/rcd-02-stagedfile-publish-migration.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/84"
  defers: []
  deferred-by: []
---

# RCD-02 Blob SHA-256 Promotion — synvelis#116 slice

## What Should Be Built

`publish_recorded`（`crates/synvelis-player/src/publish.rs`）が公開する
canonical 録音 blob の内容識別を、legacy CRC32 fold hex16 から
ALIB-02（synvelis#86）の SHA-256 content address
（`"sha256:<64hex>"` → `<media_root>/blobs/sha256/<hex>`）へ昇格する。

- canonical WAV の digest を SHA-256 で計測し、blob を
  `blobs/sha256/<hex>` 名前空間へ publish する。publish は
  synvelis-library の `BlobPublisher` port（production 実装
  `StagedBlobPublisher` = ruviform `StagedFile`/`SealedFile`
  pipeline、`PublishMode::CreateNew` + integrity 再検証 +
  readback verify）を再利用する。
- `CanonicalSourceRef.digest`（staging receipt）と、それを採用する
  `StreamEntry.digest` → `Asset.digest`/`Source.content_digest`
  （`synvelis-recording/src/commit.rs`）が `sha256:<hex>` を運ぶ。
  `digest_scheme` は canonical 採用時 `DigestScheme::ContentDigest`
  のまま。raw media provenance（`raw_digest`・`verified_sha256`）は
  変更しない。
- 既存の legacy receipt（CRC32 hex16 `digest`）は再利用経路で
  そのまま扱う — `open_blob_verified` の dual-scheme resolve
  （`DigestKind::Crc32Fold` → `blobs/<digest>` flat）により旧
  blob・旧 commit manifest・旧 Snapshot/HEAD は schema migration
  なしで解決可能なまま保護される。
- publish の indeterminate/Published effect は観測ベースで
  reconcile する（destination を `open_blob_verified` で再検証し、
  未公開確定時のみ 1 回 retry。効果不明を `Ok`/失敗に偽装しない）。

## Why Now

Issue #116 の契約「正式 Asset/Blob の内容識別には ALIB-02 #86 の
SHA-256 を使用」に対し、録音 canonical blob だけが legacy CRC32
fold のまま残っている。`open_blob_verified`（`blob.rs`）は
`sha256:` → `blobs/sha256/<hex>` の dual-scheme resolve を実装
済みで、library の `import_verified` pipeline は既に同名前空間へ
publish している。録音側を同一契約へ揃えることで、録音由来
Source も verified import 由来 Source と同一の強 digest identity
・同一 blob store・同一検証境界を持つ。

## Users and Value

- playback/resolver: `Source.content_digest` が `sha256:` form の
  場合も既存の verified blob 境界でそのまま解決される
  （sha256 は CRC32 より衝突耐性が高い正式 identity）。
- library/GC: `blobs/sha256/` 名前空間に録音 blob が統合され、
  orphan GC・catalog・将来の consumer 照合が単一名前空間で扱える。
- 保守者: blob publish の atomicity/integrity/durability 分類を
  `StagedBlobPublisher` へ一元化し、publish.rs 独自の publish
  実装を減らす。

## Scope

### In Scope

- `crates/synvelis-player/src/publish.rs`: canonical blob の
  SHA-256 digest 化・`blobs/sha256/<hex>` publish
  （`BlobPublisher` port 経由）・indeterminate reconcile・
  legacy receipt の dual-scheme 維持。
- `crates/synvelis-player/src/blob.rs` / `import.rs`: digest kind
  分類・path 解決の共有 helper（必要最小限）。
- `crates/synvelis-recording/`: `CanonicalSourceRef.digest` の
  doc/契約コメント更新（値形式 `sha256:<hex>` or legacy hex16）。
- `crates/synvelis-player/tests/rcd02_blob_digest.rs`（新規）と
  `crates/synvelis-recording/` 配下の必要 test 更新。
- `docs/` チェーンと `evidence/sv116/` の検証記録。

### Out of Scope

- `import_local`（legacy import 経路）の CRC32 digest — legacy
  v5 互換のため変更しない。
- `synvelis-library`/`synvelis-project` の変更（再利用のみ）。
- schema migration・旧 receipt/commit manifest の書換え — 旧形式は
  dual-scheme resolve で読めるため不要。
- 真の power-loss・Linux/macOS real-FS・実 device・consumer 照合 —
  外部 gate（NotRun）。
- `crates/synvelis-render/tests/x_e2e.rs` 等、編集範囲外の test
  更新 — 必要なら Coordinator へ範囲拡張を依頼する。

## Requirements

- 新規 publish の `CanonicalSourceRef.digest` は
  `"sha256:<64hex lowercase>"`、blob path は
  `<media_root>/blobs/sha256/<hex>`。
- `Asset.digest`/`Source.content_digest`/`StreamEntry.digest` は
  receipt の digest をそのまま採用し、canonical 採用時は
  `DigestScheme::ContentDigest`。
- 既存 blob は名前ではなく byte 再検証で reuse 判定する
  （SHA-256 は `open_blob_verified`、CRC32 は既存
  `content_digest_stream`）。
- `PublishMode::CreateNew` により既存 blob を上書きしない。
  publish 効果が indeterminate/published でも destination を
  再検証して reconcile し、未公開確定時のみ retry する。
- `raw_digest`（raw media fold）と `verified_sha256`（verified
  prefix provenance）は従来どおり記録する。
- legacy CRC32 receipt は `blobs/<digest>` flat で再利用・再公開
  できる（旧 session の冪等 publish を壊さない）。

## Acceptance Criteria

- [ ] `publish_recorded` が canonical WAV を
      `blobs/sha256/<sha256>` へ publish し、receipt/commit の
      digest が `sha256:<hex>` + `DigestScheme::ContentDigest`
      になる。
- [ ] `open_blob_verified` が新 digest を `blobs/sha256/<hex>` で
      解決し、bytes 改竄を `HashMismatch` で fail-closed にする。
- [ ] legacy CRC32 receipt を持つ session が従来どおり
      `blobs/<digest>` flat で publish 再利用できる。
- [ ] indeterminate/published publish effect を観測 reconcile し、
      未公開確定時のみ retry・偽装しない。
- [ ] `raw_digest`/`verified_sha256` provenance が維持される。
- [ ] OS-L1: `mise run fmt-check`/`check`/`lint`/`test`
      （synvelis-player・synvelis-recording）が Pass。
      真の power-loss・他 OS real-FS・実 device は NotRun。

## Deferred Design Concerns

- 真の power-loss・dir-sync 非対応 FS での durability 実測 |
  実機/他 OS gate | 実機環境整備後 | fake/injected fault を実
  durability と誤認するリスク（NotRun 明記で抑止）
- consumer 側（synvelis-render `x_e2e` 等）の legacy layout 前提
  assertion 更新 | 編集範囲外 | Coordinator 判断 | merge 前に
  対応が必要

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/116>（RCD-02）
- 前提 spec: `docs/specs/rcd-02-stagedfile-publish-migration.md`
  （StagedFile publish 移行済み）、
  `docs/specs/rcd-02-durability-evidence-recovery-review.md`
- 関連 Issue: #86（ALIB-02 SHA-256 verified blob — 実装済み）、
  #84（StagedFile/HEAD CAS — 実装済み）
- upstream API: `synvelis_library::{StagedBlobPublisher,
  BlobPublisher, BlobPublishError, PublishDurability,
  ContentDigest, DigestAlgorithm, ProviderContext, PrincipalId}`、
  `ruviform_io_file`（Cargo.lock rev は `Cargo.lock` 参照）
