---
id: "TESTSPEC-alib02"
type: "test-spec"
status: "approved"
title: "ALIB-02 Streaming Verified Import — テスト仕様"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  verifies:
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/designs/alib-02-streaming-verified-import-design.md"
  verified-by:
    - "crates/synvelis-library/tests/import.rs"
    - "docs/tasks/alib-02-streaming-verified-import.md"
---

# ALIB-02 Test Spec

`crates/synvelis-library/tests/import.rs`（21 tests — PR #220 時点の
15 tests に、後続 slice で cancel/resume・crash subprocess・
cross-provider 収束の 6 tests が追加済み）が仕様の各 Feature/Rule
を検証する。Scripted `BlobPublisher`/`AssetProvider`
double は fault *分類* の証拠であり、実 durability の証拠ではない
（実 publish は `StagedBlobPublisher` = ruviform `StagedFile` 系）。

## Feature: Bounded-memory streaming

- Rule: 任意サイズを全件 Vec 化せず chunk 単位で stage+計測する。
- Example: `bounded_memory_streaming_with_tiny_chunks` — 1 MiB を
  7 byte read で供給し byte-exact に publish/commit。

## Feature: SHA-256 namespace

- Rule: `blobs/sha256/<64hex>`・identity `sha256:<64hex>` は旧
  flat/CRC 名前空間と衝突しない。
- Example: `sha256_namespace_cannot_collide_with_legacy_flat`、
  `import_streams_verifies_publishes_and_commits`（path/identity/
  `LocalBlobDir` round-trip）。

## Feature: Idempotency / integrity

- Rule: 同一 ImportId+同一内容は replay 冪等、異内容は conflict、
  digest/size/budget 不一致は拒否、登録重複なし。
- Example: `replay_same_import_is_idempotent`、
  `same_import_id_different_content_is_conflict`、
  `digest_mismatch_is_integrity_error`、
  `byte_budget_is_enforced_during_staging`。

## Feature: Staging fault recovery

- Rule: manifest が各 stage を durable に記録し、障害後は
  manifest から再開する。publish 失敗・commit 拒否で blob/staging
  は orphan 保護される。
- Example: `publish_failure_resumes_from_manifest`（Verified から
  再開）、`stale_revision_rejected_then_explicit_rebase_converges`。

## Feature: Revision / target safety

- Rule: manifest の `expected_project_revision` のみで commit、
  stale は typed reject、rebase は明示 API、lane/track/clip 契約は
  commit 時に再検証。
- Example: `stale_revision_rejected_then_explicit_rebase_converges`、
  `missing_lane_target_is_rejected_without_clip`、
  `wrong_track_lane_target_is_rejected`。

## Feature: Publish honesty / durability evidence

- Rule: `PublishEffect` 三値を維持し、Indeterminate は stat
  reconcile、DirSync は観測値のみ記録、live commit ≠ saved/HEAD。
- Example: `indeterminate_publish_is_reconciled_not_misreported`、
  `directory_sync_failure_is_recorded_not_hidden`、
  `import_streams_verifies_publishes_and_commits`（`saved()==None`）。

## Feature: Orphan GC

- Rule: reachability（project 参照 + active manifest）のみ保護、
  明示 GC のみが削除、legacy/foreign/未完 job は保持。
- Example: `orphan_gc_protects_reachable_and_deletes_only_orphans`、
  `incomplete_job_and_blob_are_protected_from_gc`。
