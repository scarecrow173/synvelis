---
id: "DESIGN-4CsMcB1lWPYHOuIXmJdj4W"
type: "design"
status: "approved"
title: "RCD-02 Blob SHA-256 Promotion Design"
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
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-02-stagedfile-publish-migration-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-02-stagedfile-publish-migration-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-02-blob-sha256.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  defers: []
  deferred-by: []
---

# RCD-02 Blob SHA-256 Promotion Design

## Context

`publish_recorded` は canonical WAV を `import::content_digest`
（CRC32 fold hex16）で digest 化し、`import::blob_path` の flat
`blobs/<digest>` へ `publish_atomic` で publish していた。
`CanonicalSourceRef.digest` も hex16 を運び、`commit.rs` の
`StreamEntry.digest`/`digest_scheme`/`Asset.digest`/
`Source.content_digest` へそのまま伝播する。一方 `blob.rs` の
`open_blob_verified` は既に dual-scheme（`DigestKind::Sha256` →
`blobs/sha256/<hex>` / `Crc32Fold` → flat）を解決し、library の
`import_verified` は `blobs/sha256/` 名前空間へ publish 済み。
録音 canonical blob だけが legacy identity のまま残る不整合を、
所有境界（player publish + recording receipt 契約）で修正する。

## Scope

### In Scope

- `publish.rs` の canonical publish を SHA-256 化。
- `blob.rs`/`import.rs` の digest kind 分類の共有化（最小限）。
- `staging.rs` の `CanonicalSourceRef.digest` 契約 doc 更新。
- 新規 test + recording 側 test の整合。

### Out of Scope

- `import_local`（legacy import）の CRC32 経路。
- library/project の変更（再利用のみ）。
- schema migration・旧 manifest/receipt 書換え。
- 実機 durability・他 OS・consumer 側 test（範囲外/外部 gate）。

## Design

### 1. Digest scheme の分類境界

`AssetDigest::parse`（synvelis-project）が `"sha256:<64hex>"` /
16-hex CRC32 fold / opaque を `DigestKind` で分類する既存契約を
正本とし、publish.rs は独自の digest 判定を持たない。

- **新規 publish（canonical WAV）**: `ContentDigest::sha256_of`
  （synvelis-library）で測定 → `format!("sha256:{}", hex)` を
  `CanonicalSourceRef.digest` へ記録 →
  `BlobPublisher::publish(&content, ContentDigest::Sha256{hex})`。
  `content` は canonical WAV 全体の byte 列。
- **既存 receipt 再利用**: receipt の `digest` を
  `AssetDigest::parse` で分類。
  - `DigestKind::Sha256` → `blobs/sha256/<hex>` を
    `open_blob_verified` で byte 再検証して reuse。
  - `DigestKind::Crc32Fold`（旧 receipt）→ legacy
    `content_digest_stream` + flat `blobs/<digest>` で
    再検証・再利用（旧 session の冪等 publish を維持）。
  - opaque → fail-closed（`PublishError::Integrity`）。

`CanonicalSourceRef.digest` の doc を「hex16（legacy）または
`sha256:<hex>`」へ更新する（schema version は据置き — 値の
形式は additive で旧 reader は opaque 扱いせず parse 可能、
`commit.rs` の `adopt_canonical_digest` は digest をそのまま
採用するため新旧混在でも整合）。

### 2. Blob publish の port 利用

`synvelis_library::BlobPublisher` port を `publish_recorded_with`
内部へ注入可能にする（test で fault seam を差し替え可能にする
ため）。production 経路は `StagedBlobPublisher` を構築して渡す。

```rust
pub fn publish_recorded(area, media_root, project_dir, session_id, cx)
    -> Result<PublishOutcome, PublishError>
{
    publish_recorded_with(area, media_root, project_dir,
        session_id, &mut StagedBlobPublisher::new(media_root), cx)
}
```

- `StagedBlobPublisher::new(media_root)` は
  `LocalBlobDir`（`blobs/sha256/<hex>` mapping）+
  `ruviform StagedFile` pipeline を内包し、publish 成功時に
  readback verify + `PublishDurability`（FileSync + dir sync
  capability 検出）を receipt へ記録する。
- `publish_recorded_with` は blob publish を
  `publisher.publish(content_bytes, ContentDigest::Sha256{hex})`
  で行う。`BlobPublishError` は `PublishError::Io`/`Integrity`
  へ写像し、`PublishEffect::Indeterminate` は観測 reconcile へ。

### 3. Publish 効果の reconcile（indeterminate 対応）

`StagedBlobPublisher` は ruviform `PublishFailure` の
`PublishEffect` を `BlobPublishError::Indeterminate` として
返す。publish.rs は:

1. `Err(BlobPublishError::Indeterminate)` または
   `Published`+応答喪失相当の場合、`open_blob_verified` で
   destination を再検証する。
2. digest 一致 → publish は実質成功として receipt へ進む
   （効果不明を `Ok` に偽装しない — 実測で確認した上で採用）。
3. digest 不一致/不存在 → `CreateNew` の性質上 retry は
   「未公開確定時のみ 1 回」行い、それでも失敗なら
   `PublishError::Io`/`Integrity` を返す。

receipt publish（`canonical.json`）は従来どおり
`publish_atomic`（Replace）を使う — 冪等再公開が必要なため。
blob publish だけが `CreateNew` + content-addressed に変わる。

### 4. WAV 生成と digest の順序

現行は WAV を temp file へ write → `content_digest` で測定 →
publish。新経路では:

1. canonical WAV を `Vec<u8>`（または seekable temp）へ生成。
   WAV は header + PCM payload の決定的 byte 列であり、
   in-memory 生成しても現行 temp file 経路と byte 同一
   （`open_muxer` が決定的 header を書く）。
2. `ContentDigest::sha256_of(&wav_bytes)` で測定。
3. `publisher.publish(&wav_bytes, digest)` で公開。
4. receipt の `digest`/`canonical_digest` に sha256 文字列を記録。

### 5. 変更しないもの

- `blob.rs` の `open_blob_verified` — dual-scheme resolve 済み。
- `import.rs` の `import_local` — legacy CRC32 経路は不変。
- `commit.rs` の manifest/adopted digest 採用ロジック — digest
  文字列をそのまま運ぶため変更不要。
- `writer.rs`/`recovery.rs` の `verified_sha256`（raw provenance）。
- `staging.rs` の `publish_atomic`/`publish_atomic_faultable`
  signature と fault seam。

## File Map

- `crates/synvelis-player/src/publish.rs` — canonical blob の
  SHA-256 publish・`publish_recorded_with`・reconcile・
  legacy dual-scheme 維持
- `crates/synvelis-player/src/blob.rs` — 必要時のみ
  `sha256_blob_path` 等の共有 helper（現状は `open_blob_verified`
  が path を内包するため変更最小限）
- `crates/synvelis-player/src/import.rs` — digest kind 分類の
  共有 helper が必要な場合のみ
- `crates/synvelis-player/Cargo.toml` — `synvelis-library`
  依存確認（既存か確認し、無ければ追加）
- `crates/synvelis-recording/src/staging.rs` —
  `CanonicalSourceRef.digest` doc 更新
- `crates/synvelis-recording/src/commit.rs` — `digest` field doc
  更新（値形式の記述のみ）
- `crates/synvelis-player/tests/rcd02_blob_digest.rs` — 新規 test
- `docs/` チェーン・`evidence/sv116/`

## Risk Register

- **R1 旧 session 再 publish**: 旧 receipt の CRC32 digest を
  sha256 で再 publish しようとすると digest 不一致で fail する。
  → receipt の digest kind を見て legacy 経路へ分岐する設計で
  回避（§1）。
- **R2 synvelis-render `x_e2e` の flat blob 前提**: 既存 test が
  `blobs/<crc16>` layout を assert している場合、新 sha256
  layout で fail する。→ 範囲外のため Coordinator へ確認・
  必要なら範囲拡張依頼。
- **R3 WAV byte 決定性**: in-memory 生成と temp file 生成で
  byte が変わると digest が変わり receipt と不整合。→ 現行
  `open_muxer` の header 決定性を確認し、生成経路を一本化する。
- **R4 `BlobPublisher` port の object safety**: `publish` が
  `&mut self` を取る場合 `&mut dyn BlobPublisher` で注入する。
  `StagedBlobPublisher` が `Send` でない場合 thread 境界に注意。
- **R5 MSRV**: edition 2024 構文は rust-version 1.89 範囲に留める。

## Verification Matrix

- `mise run fmt-check` / `check synvelis-player` /
  `lint synvelis-player` / `test synvelis-player` /
  `check synvelis-recording` / `lint synvelis-recording` /
  `test synvelis-recording` — canonical 検証。
- 新規 `rcd02_blob_digest.rs`: sha256 publish・receipt digest・
  `open_blob_verified` round-trip・tamper fail-closed・legacy
  dual-scheme・indeterminate reconcile・idempotent republish。
- `synvelis-render` の `x_e2e` は範囲外 — 影響があれば
  Coordinator へ報告。
- 実 OS/FS durability・power-loss・他 OS は NotRun。

## Self-Review

- spec の全 requirement が design 判断へ対応する。
- port 再利用・digest 分類の正本・reconcile・legacy 維持・
  fail-closed を明示。
- placeholder なし。
