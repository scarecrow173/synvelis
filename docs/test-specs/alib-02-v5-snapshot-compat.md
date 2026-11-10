---
id: "TESTSPEC-alib02-v5snap"
type: "test-spec"
status: "approved"
title: "ALIB-02 v5 asset-digest 互換 + recovery fixture — テスト仕様"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  verifies:
    - "docs/specs/alib-02-v5-snapshot-compat.md"
    - "docs/designs/alib-02-v5-snapshot-compat-design.md"
  verified-by:
    - "crates/synvelis-project/tests/alib02_digest.rs"
    - "crates/synvelis-library/tests/import_recovery.rs"
---

# ALIB-02 v5 asset-digest 互換 + recovery fixture — テスト仕様

## Feature: digest 分類境界

### Rule: persisted digest は lossless、分類は読み出し時

- Example: `digest_kind_classifies_persisted_forms` —
  `sha256:<64hex>`→Sha256、16hex→Crc32Fold、その他 filename-safe→Opaque、
  空/`sha256:` 形式不正→拒否。

### Rule: 旧 CRC は verified identity に昇格しない

- Example: `crc_digest_collision_never_becomes_content_addressed` —
  同一 CRC digest を持つ 2 asset は別 asset のまま。いずれも
  `is_content_addressed()==false`。

### Rule: 空 digest は load/commit 境界で拒否

- Example: `empty_digest_is_rejected_at_commit_and_load`。

## Feature: v5→v11 migration

### Rule: lossless・決定的・immutable source

- Example: `v5_snapshot_with_crc_digests_migrates_losslessly` —
  v5 envelope（routing/control 除去）が v11 へ移行し、digest 文字列・
  asset/source ID・applied-import receipt が保全される。save→reload
  round-trip、同一 bytes の再移行が同一結果、旧 snapshot bytes 不変、
  migrated load で live revision +1（新 snapshot 名で save）。

### Rule: receipt は live entity を指す

- Example: `applied_import_receipt_must_name_live_entities` —
  dangling `asset_id`/`source_id` を持つ receipt は拒否。

## Feature: recovery fixture（library）

- `receipt_survives_save_load_and_replays` — save→load 後の同一
  ImportId replay が registry fast-path で冪等（reused、ID 不変）。
- `save_failure_preserves_commit_and_recovers` — Save/HEAD 障害で
  live commit 温存・HEAD 未保存を正直に報告、retry で回復。
- `undo_after_commit_reimport_converges` — undo 後の再 import は
  `StaleRevision`→明示 `rebase_import`→予約 ID で収束。
- `stale_then_target_gone_rejects_and_orphan_protected` — publish 後の
  stale revision、rebase 後の対象 lane 消失で `CommitRejected`、
  published blob は manifest により orphan 保護。
- `multi_track_placement_lands_on_intended_lane_only` — 2 track 構成で
  clip が意図した lane のみに配置される。
- `legacy_crc_blob_coexists_with_sha256_namespace` — flat `blobs/<crc>`
  が read/GC 双方で温存され、`blobs/sha256/` と衝突しない。

## Non-goals

- 実 FS durability・directory sync の OS 差（NotRun、Issue #91）。
- Mock FS を実 durability PASS と見なさない（fixture は決定性のみ主張）。
