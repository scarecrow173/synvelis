---
id: "TASK-alib03-t1"
type: "task"
status: "in-progress"
title: "ALIB-03 Task 1 — cache/catalog/preview/notify modules・tests"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  implements:
    - "docs/plans/alib-03-catalog-preview-cache.md"
  depends-on:
    - "docs/designs/alib-03-catalog-preview-cache-design.md"
  verifies:
    - "docs/test-specs/alib-03-catalog-preview-cache.md"
---

# ALIB-03 Tasks

## Task 1 — modules + facade

- [ ] `src/cache.rs`: `CachePolicy`/`CacheStats`/`CacheLease`/
      `StoreState`(Mutex)/disk layout/atomic payload write/
      LRU+TTL eviction/lease 保護/single-flight/`retire_principal`/
      `credentials_changed`/`clear`/`stats`/`LibraryCache` facade。
- [ ] `src/catalog.rs`: `CatalogGeneration`/`CatalogQuery`/
      `CatalogCacheKey`/`IndexCompleteness`/`CatalogIndex` record/
      `IndexStatus`/cache cursor(encode+decode+key/build 照合)/
      `browse`/`search`/`index_status`（hit/refresh/stale/
      offline-cached/permission-denied retire）。
- [ ] `src/preview.rs`: `PreviewFormat`/`PreviewRequest`/
      `PreviewCacheKey`/`PreviewMeta`/`PreviewLease`(Read+cancel)/
      `preview_plan`/`preview`（pin→key→hit|bounded stream→staging→
      atomic publish→lease）。
- [ ] `src/notify.rs`: `LibraryEvent`/`LibraryEventKind`/
      `RetireReason`/`LibraryEventSink`/`SuppressingSink`/
      `RtEventQueue`(try_lock+bounded+alloc-free)。

## Task 2 — exports + tests

- [ ] `src/lib.rs` module+re-export、`Cargo.toml` dev-dep
      `stats_alloc = "0.1.10"`。
- [ ] `tests/cache.rs`: scripted provider（online/offline/
      permission/auth-expired/search-unsupported/truncated/
      gated slow stream/per-principal preview bytes/open-stream
      counter）で全 acceptance を検証。
- [ ] `tests/rt_cache.rs`: `stats_alloc` Region で
      `RtEventQueue::emit` の warm 後 alloc/dealloc 0・非 blocking・
      bounded drop を検証。

## Notes

- cache は非正本 derived store — project media・verified blob・
  import manifest とは別物。cache 全喪失 = miss→provider 再取得。
- secret/生 token は key・disk・event に出さない。disk path の
  principal 成分は SHA-256 全桁 hex fingerprint のみ。
- `CommittedLive`/`SavedRevision` の区別は ALIB-02 を踏襲 —
  cache は save/HEAD に一切関与しない。
