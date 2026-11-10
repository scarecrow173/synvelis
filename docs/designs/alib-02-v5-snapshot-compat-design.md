---
id: "DSN-alib02-v5snap"
type: "design"
status: "approved"
title: "ALIB-02 — v5 snapshot asset-digest 互換移行 設計"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/alib-02-v5-snapshot-compat.md"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/migration.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/alib02_digest.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/import_recovery.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/tim_02_smf.rs"
      - type: "file"
        path: "crates/synvelis-library/src/import.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-02-v5-snapshot-compat.md"
  implemented-by: []
  depends-on:
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/designs/alib-02-streaming-verified-import-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/project-asset-persistence-architecture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# ALIB-02 — v5 snapshot asset-digest 互換移行 設計

## Overview

ALIB-02 本体（PR #220）は新規 import の SHA-256 識別子のみを導入し、
既存 snapshot の digest 形式移行を deferred とした。本設計はその残件を、
既存の versioned snapshot + ordered migration ladder の枠組みに沿って
閉じる。新規の v5 専用 reader は作らない。

## 決定

### 1. digest は raw String のまま永続化し、分類は読み出し境界に閉じる

`Asset.digest` / `Source.content_digest` / `AppliedImport.content_hash` の
serde 表現は変更しない。永続化文字列を migration で書き換えると、
旧 blob namespace の実体（`blobs/<flat name>`）との対応が壊れ、かつ
byte-level verify なしの「verified 昇格」になってしまう。

代わりに `synvelis-project/src/project.rs` に型付き分類層を追加する:

- `AssetDigest::parse(&str)` — 空文字列・非 filename-safe な `sha256:`
  形式などを拒否し、`Crc32Fold`（16 lowercase hex）/ `Sha256`
  （`sha256:<64 lowercase hex>`）/ `Opaque`（その他の filename-safe）
  に分類する。
- `Project::validate_references` が全 asset/source/applied-import の
  digest 文字列に `AssetDigest::parse` を適用し、空・不正形式を
  load/commit 境界で拒否する。receipt の `asset_id`/`source_id` が
  live entity を指すことも同時に検証する。

### 2. v10→v11 migration step は分類メタデータのみ

`migration.rs` STEPS に v10→v11 step を追加し `SCHEMA_VERSION` を 11 に
進める。step 自体は digest 文字列を書き換えず、分類が新たに厳格化された
境界（空 digest 拒否など）を `validate_references` に委譲する形で、
既存の v5→v10 ladder と同じく additive・決定的・冪等とする。

- migrated load は live revision を +1 し、save は新 snapshot 名へ書く
  （旧 v5 bytes は immutable・reloadable のまま）。
- 同一入力 bytes の再移行は同一 `Project` を返す（決定性）。

### 3. CommitVerifiedImport の digest ゲート

`synvelis-library/src/import.rs` の `CommitVerifiedImport::validate` に、
`AssetDigest::parse(&content_digest).is_content_addressed()` を要求する
検査を追加する。verified import pipeline は常に SHA-256 を測定するため
通常経路では不変だが、library 外から構築された op が legacy/opaque
digest を verified commit として登録することを構造的に防ぐ。

### 4. Recovery fixture は既存 port の fault seam を使う

新しい production code path は追加しない。fixture は既存の
`ImportStore` manifest / `ProjectStore`（`InMemoryStore` ラッパで save
失敗注入）/ `BlobPublisher` scripted fault / `gc_orphans` /
`rebase_import` を組み合わせ、`Staging→Verify→Published→LiveCommit→
Save/HEAD` 各点の crash-adjacent 挙動を検証する。flat legacy blob と
`blobs/sha256/` namespace の共存・GC 非対象もここで固定する。

## Trade-offs

- digest 文字列を typed newtype にせず raw String + 境界分類に留めたのは、
  既存永続形式・`LocalBlobDir` flat 解決・player 側 legacy consumer との
  互換を最小コストで保つため。分類規則が境界に一箇所集約されるため、
  将来 typed 化する場合も `AssetDigest` が唯一の変更点になる。
- migration step が「何もしない」に近いのは意図的である。v11 の実質的
  な変更は検証規則の厳格化であり、lossless 性は「書き換えないこと」で
  担保される。
