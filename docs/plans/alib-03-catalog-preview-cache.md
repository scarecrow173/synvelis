---
id: "PLAN-alib03-catalog-preview-cache"
type: "plan"
status: "in-progress"
title: "ALIB-03 実装計画 — Catalog/Preview Cache・Principal 隔離・bounded cache（synvelis-library slice）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/87"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-library/src/cache.rs"
      - type: "file"
        path: "crates/synvelis-library/src/catalog.rs"
      - type: "file"
        path: "crates/synvelis-library/src/preview.rs"
      - type: "file"
        path: "crates/synvelis-library/src/notify.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/cache.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/rt_cache.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-library/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/alib-03-catalog-preview-cache.md"
  implemented-by: []
  depends-on:
    - "docs/designs/alib-03-catalog-preview-cache-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-01-asset-provider-contract.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/alib-03-catalog-preview-cache.md"
---

# ALIB-03 Plan

## Task decomposition

- `docs/tasks/alib-03-catalog-preview-cache.md` — 単一 task doc:
  1. `src/cache.rs` — `CachePolicy`/`CacheStats`/`StoreState`/
     `CacheLease`/atomic write/eviction/single-flight/退役/clear/
     `LibraryCache` facade。
  2. `src/catalog.rs` — key/generation/query/index record/status/
     cache cursor/browse・search・index_status。
  3. `src/preview.rs` — format/request/key/meta/lease/preview・
     preview_plan・FullDownloadRequired。
  4. `src/notify.rs` — event/sink/suppress/rt queue。
  5. `src/lib.rs` + `Cargo.toml`（stats_alloc dev-dep）+
     `tests/cache.rs` + `tests/rt_cache.rs`。

## File map

- `crates/synvelis-library/src/{cache,catalog,preview,notify}.rs`
  (new)、`src/lib.rs`（module+export）。
- `crates/synvelis-library/Cargo.toml` — `[dev-dependencies]
  stats_alloc`（rt 計測、他 crate と同版 0.1.10）。
- `crates/synvelis-library/tests/cache.rs`（new）—
  scripted provider fixture + 全 acceptance。
- `crates/synvelis-library/tests/rt_cache.rs`（new）—
  `rt_event_queue_push_is_alloc_and_drop_free`。

## Verification matrix

| Check | Command |
| --- | --- |
| fmt | `mise run fmt-check` |
| typecheck | `mise run check` |
| lint | `mise run lint` |
| tests | `mise run test` |
| build | `mise run build` |
| MSRV | `mise run test-msrv` |
| RT | `mise run test-rt` |
