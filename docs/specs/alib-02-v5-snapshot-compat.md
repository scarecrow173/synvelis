---
id: "SPEC-alib02-v5snap"
type: "spec"
status: "approved"
title: "ALIB-02 deferred — v5 snapshot 互換移行（SHA-256/旧 CRC digest 共存）"
created: "2026-11-09"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/86"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/alib-02-v5-snapshot-compat-design.md"
  depends-on:
    - "docs/specs/alib-02-streaming-verified-import.md"
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
    - "docs/test-specs/alib-02-v5-snapshot-compat.md"
  references: []
  defers: []
  deferred-by:
    - "docs/specs/alib-02-streaming-verified-import.md"
---

# ALIB-02 deferred — v5 snapshot 互換移行

ALIB-02（`SPEC-034UviRHTzZZY5QqaoRYZV`）から deferred された残件。
sv86-alib02 slice（Issue #86 残 headless slice）で実装済み。

## What Should Be Built

旧 schema（v5〜v10 系 snapshot・CRC32 fold / opaque digest）で保存された
Project が、SHA-256 名前空間導入後も lossless で読み込めること。
`SCHEMA_VERSION` を v11 へ進め、既存の順序付き migration ladder
（`migration.rs` STEPS）に asset-digest step を追加する。

- `Asset.digest` / `Source.content_digest` / `AppliedImport.content_hash`
  は raw `String` のまま永続化する。移行は digest 文字列を書き換えない
  （lossless）。型付き分類は `project::AssetDigest` / `DigestKind`
  （`Crc32Fold` / `Sha256` / `Opaque`）として読み出し境界に閉じる。
- 空・不正な digest 文字列は load / commit 境界で拒否する。
- 旧 CRC digest は verified SHA-256 content address に昇格しない。
  同一 CRC でも bytes が異なる asset は別 asset のまま保持する。
- `AppliedImport` receipt は live の `asset_id` / `source_id` を指す
  必要がある（dangling receipt は拒否）。
- `CommitVerifiedImport` は `sha256:<64hex>` 形式の content digest のみ
  commit を許可する（library 側 validate 境界）。
- 移行後の save は新 schema version で新しい snapshot 名へ書く。
  旧 snapshot bytes は immutable のまま残り、再読み込み可能。
- 同一 v5 bytes の繰り返し移行は deterministic に同一結果を返す。

## Fault / Recovery fixture

`Staging → Verify → Published → LiveCommit → Save/HEAD` の各障害点を
fixture で検証する（詳細は test-spec 参照）。

- 同一 `ImportId` replay は registry fast-path で冪等。
- 同一 `ImportId`・異なる content は integrity conflict。
- stale revision は `StaleRevision` で明示拒否し、`rebase_import` による
  明示的な回復のみ許可。
- commit 拒否後も published blob は orphan として保護され、明示的な
  `gc_orphans` のみが削除する。flat legacy blob（`blobs/<name>`）は GC
  対象外で、SHA-256 namespace（`blobs/sha256/<64hex>`）と衝突しない。
- `CommittedLive` と `SavedRevision`/`HEAD` は分離する。save 失敗は
  live commit を巻き戻さず、HEAD は正直に未保存のまま報告する。
- undo による live commit 取り消し後の再 import は manifest の予約 ID で
  収束する。

## Non-goals

- 旧 blob bytes の SHA-256 再計算・blob 物理移行（byte-level verify
  なしに verified identity へ昇格しない）。
- player/app/workspace 配線、media decode 検証、Linux/macOS 実 FS
  durability（Issue #91・別 gate）。
- `synvelis-player` の旧 CRC import path の書き換え（legacy consumer
  として温存）。
