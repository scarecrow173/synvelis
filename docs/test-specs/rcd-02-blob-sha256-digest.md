---
id: "TSPEC-nb7F26elY2sGZvVr3qyB2r"
type: "test-spec"
status: "approved"
title: "RCD-02 Blob SHA-256 Digest test-spec"
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
    - "docs/specs/rcd-02-blob-sha256.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-02-blob-sha256.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-02-blob-sha256.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-02 Blob SHA-256 Digest test-spec

## Purpose

canonical 録音 blob の SHA-256 昇格が、publish・receipt・commit・
playback 解決・legacy 互換・indeterminate reconcile の全契約を
headless で保証することを検証する test-spec。fake/injected
fault の成功を実 FS durability の証明にしない。

## Feature

`publish_recorded` の canonical blob publish と digest identity
（`sha256:` / legacy CRC32 fold の dual-scheme）。

## Rules

- **R1 SHA-256 正式 identity**: 新規 publish の
  `CanonicalSourceRef.digest` は `"sha256:<64hex lowercase>"`、
  blob path は `<media_root>/blobs/sha256/<hex>`。digest は
  canonical WAV 全 byte の SHA-256 と一致しなければならない。
- **R2 Verified boundary round-trip**: `open_blob_verified` が
  sha256 digest を `blobs/sha256/<hex>` で解決し、file bytes を
  SHA-256 で再検証してから返す。bytes 改竄・欠損・不存在は
  それぞれ `HashMismatch`/`Io`/`NotFound` で fail-closed。
- **R3 Legacy dual-scheme**: CRC32 hex16 digest の receipt は
  `blobs/<digest>` flat で再利用・再 publish でき、
  `open_blob_verified` が CRC32 fold で検証する。新旧混在の
  session で digest kind を誤判定しない。
- **R4 Indeterminate reconcile**: publish が
  `PublishEffect::Indeterminate`/`Published`+応答喪失相当を
  返した場合、destination を再検証して実測で採用可否を決める。
  効果不明を `Ok`/`Err` に偽装せず、未公開確定時のみ 1 回
  retry する。
- **R5 Content-addressed no-overwrite**: `PublishMode::CreateNew`
  により既存 blob を上書きしない。同一 content の再 publish は
  冪等（既存 blob を検証して reuse）。
- **R6 Provenance 維持**: `raw_digest`（raw media fold）と
  `verified_sha256`（verified prefix provenance）は変更せず、
  commit manifest の `StreamEntry` が両者を保持する。
- **R7 Opaque fail-closed**: `AssetDigest::parse` が opaque と
  分類する digest は publish/playback で fail-closed
  （`UnsupportedDigest`/`Integrity`）。

## Examples

- **E1 新規 publish**: finalized Complete stream を持つ session
  を `publish_recorded` → receipt digest が `sha256:<hex>`、
  blob が `blobs/sha256/<hex>` に存在、`open_blob_verified` で
  byte 一致を確認。
- **E2 tamper**: `blobs/sha256/<hex>` の 1 byte を改竄 →
  `open_blob_verified` が `HashMismatch`。
- **E3 legacy receipt**: CRC32 hex16 digest を持つ receipt を
  持つ session で publish 再実行 → flat `blobs/<digest>` を
  再利用し sha256 へ書き換えない。
- **E4 idempotent republish**: 同一 session で `publish_recorded`
  を 2 回呼ぶ → blob/receipt が冪等、digest 不変。
- **E5 indeterminate**: `BlobPublisher` stub が
  `BlobPublishError::Indeterminate` を返しつつ実際には publish
  済み → reconcile が destination を再検証して成功扱い。
- **E6 provenance**: commit manifest の `StreamEntry` が
  `raw_digest`/`verified_sha256`/`digest_scheme=ContentDigest`/
  `digest=sha256:<hex>` を保持。

## Guarantees

- sha256 digest と blob path・byte 内容が常に一致する。
- legacy/opaque digest を sha256 へ誤昇格・誤解決しない。
- publish 効果不明を成功/失敗に偽装しない。
- headless lane の `Pass` は実 FS durability・power-loss・
  他 OS・実 device を意味しない（NotRun 維持）。

## Non-goals

- 真の power-loss・dir-sync 非対応 FS・他 OS real-FS・実 device。
- `import_local`（legacy import）の CRC32 経路変更。
- consumer 側（synvelis-render 等）の test 更新。
- schema migration・旧 manifest/receipt 書換え。

## Risk

- `BlobPublisher` port の stub が本物と異なる効果分類を返すと
  reconcile の検証が歪む — stub は `BlobPublishError` variant と
  実 destination 状態を独立に制御できること。
- WAV in-memory 生成で byte が変わると digest 不整合 — 生成
  経路を一本化して決定性を保つ。
