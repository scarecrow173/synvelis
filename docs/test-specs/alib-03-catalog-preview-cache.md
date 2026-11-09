---
id: "TESTSPEC-alib03"
type: "test-spec"
status: "approved"
title: "ALIB-03 Catalog/Preview Cache — テスト仕様"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  verifies:
    - "docs/specs/alib-03-catalog-preview-cache.md"
    - "docs/designs/alib-03-catalog-preview-cache-design.md"
  verified-by:
    - "crates/synvelis-library/tests/cache.rs"
    - "crates/synvelis-library/tests/rt_cache.rs"
---

# ALIB-03 Test Spec

`crates/synvelis-library/tests/cache.rs` が scripted
`AssetProvider` double（in-memory catalog・fault 注入・
open-stream counter・per-principal preview bytes・gated slow
stream）で cache 契約を検証する。`tests/rt_cache.rs` が
`stats_alloc` で `RtEventQueue::emit` の RT 安全性を計測する。
double の結果は fault *分類* の証拠であり実 Provider の証拠
ではない（実負荷・OS 別実測は NotRun/Unsupported）。

## Feature: Cache 状態の正直さ

- Rule: miss/stale/partial/offline/index-incomplete/
  permission-revoked/credential-change を区別し、
  `Coverage`/error kind が正確。部分 index を Complete に
  見せず、権限剥奪を offline cached にしない。
- Example: `browse_*`/`search_*`/`index_status_*` 各 test —
  Complete/Partial snapshot、TTL 期限切れ→refresh、
  provider Offline→OfflineCached、PermissionDenied→
  principal 退役+error 伝播、AuthExpired 同様、
  search Unsupported→UnsupportedCapability 伝播、
  `NeverIndexed`/`Fresh`/`Stale` status。

## Feature: Principal 隔離と退役

- Rule: 同一 object identity でも principal が違えば preview
  entry・disk path が分離。`revoke_principal`/
  `credentials_changed` で当該 principal の entry のみ退役。
- Example: `preview_isolated_per_principal`（A/B が同じ pin で
  異なる bytes を得て provider 呼出が 2 回・disk path が別）、
  `revoke_principal_retires_private_entries`、
  `credentials_changed_invalidates_old_epoch`。

## Feature: Disk cache 削除耐性

- Rule: cache root 削除/clear は Project の保存・blob store・
  Undo に影響しない（cache は derived store）。
- Example: `cache_wipe_does_not_affect_project` —
  project+blob 構築→cache clear+dir 削除→save/reload/undo が
  非影響。

## Feature: Cursor / paging 正確性

- Rule: cache cursor は key fingerprint + index build id +
  offset に結合。query/principal/generation/build 変更で
  `CursorInvalid`。
- Example: `cursor_survives_within_index`、
  `cursor_invalid_after_rebuild`、
  `cursor_invalid_after_generation_change`、
  `paging_serves_from_cache_without_provider`。

## Feature: Bounded / leak 防止

- Rule: `CachePolicy` の disk bytes/entries/lease/preview 上限を
  超えず、rapid cancel で stream/lease/staging が残らない。
- Example: `bounded_disk_and_entries_evict_lru`、
  `leased_entry_survives_eviction`、
  `rapid_cancel_leaves_no_stream_lease_or_staging`、
  `full_object_requires_explicit_consent`、
  `oversized_preview_is_budgeted`、
  `index_walk_is_bounded`（provider call 数・entries 上限）。

## Feature: 非同期通知と RT 安全性

- Rule: event は generation stamp 付きで emit、stale generation
  は suppress、RT callback は queue 投入のみ（alloc/block なし）。
- Example: `events_carry_generation_and_suppress_stale`、
  `rt_event_queue_push_is_alloc_and_drop_free`（rt_cache.rs）。
