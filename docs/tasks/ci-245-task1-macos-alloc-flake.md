---
id: "TASK-034VmiXNgZm3ucj1WcbTT9"
type: "task"
status: "done"
title: "rt_cache 計測対象インスタンスの warm-up 修正"
created: "2026-11-10"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/245"
  changes:
    added:
      - type: "file"
        path: "docs/tasks/ci-245-task1-macos-alloc-flake.md"
      - type: "file"
        path: "docs/impl/ir/ci-245-macos-alloc-flake-slice.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/rt_cache.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/ci-245-macos-alloc-flake-plan.md"
  implemented-by:
    - "docs/impl/ir/ci-245-macos-alloc-flake-slice.md"
  depends-on:
    - "docs/plans/ci-245-macos-alloc-flake-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ci-245-macos-alloc-flake.md"
    - "docs/designs/ci-245-macos-alloc-flake-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ci-245-macos-alloc-flake.md"
    - "docs/designs/ci-245-macos-alloc-flake-design.md"
    - "docs/plans/ci-245-macos-alloc-flake-plan.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-library/tests/rt_cache.rs"
    - "crates/synvelis-library/src/notify.rs"
  defers: []
  deferred-by: []
---

# rt_cache 計測対象インスタンスの warm-up 修正

## Work

`crates/synvelis-library/tests/rt_cache.rs` の
`rt_event_enqueue_is_alloc_and_dealloc_free` で、warm-up を計測対象
インスタンス自身に対して行うよう変更する。

- 窓 1: 使い捨て `RtEventQueue` の warm-up を廃止し、計測対象
  `queue` に対して `try_emit(ev(1..=8))` → `drain()` →
  `is_empty()` を `Region` の外で実行する。
- 窓 2: `sink` 構築直後・`Region` の外で `sink.emit(ev(0))` を
  1 回 warm-up として追加する。generation 0 は watermark を
  進めず gate を通過して 1 slot を占有する。warm event は
  `SuppressingSink` 経由では drain できず undrainable なので、
  計測窓の空き slot は 7 になり、gate 通過 9 イベントのうち
  2 件が full-queue drop 分岐を追加で踏む — この 2 点を
  コメントで明記する。
- 計測契約（両窓で allocations/deallocations == 0、
  `sink.latest() == LibraryGeneration(9)`）は変更しない。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/ci-245-macos-alloc-flake-plan.md`
- `docs/specs/ci-245-macos-alloc-flake.md`
- `docs/designs/ci-245-macos-alloc-flake-design.md`

## Dependencies

- なし（ブロッカーなし）

## Affected Files

- `crates/synvelis-library/tests/rt_cache.rs` — warm-up の対象を
  計測対象インスタンスへ変更、窓 2 に warm-up 追加とコメント
- `docs/specs|designs|plans|tasks|impl/ir/ci-245-*` — 本タスクの
  文書チェーン

## Done When

- [x] 両計測窓が計測対象インスタンス自身の warm-up 後に
  `allocations == 0`・`deallocations == 0` を assert する
- [x] `sink.latest() == LibraryGeneration(9)` と
  `queue.len() == 8` の既存 assert が維持される
- [x] undrainable warm event・空き slot 7・full-queue drop 分岐の
  追加がコメントで明記される

## Verification

- [x] `mise run fmt-check` — exit 0
- [x] `mise run check` — exit 0
- [x] `mise run lint` — exit 0
- [x] `mise run test` — exit 0
- [x] `mise run test-rt` — exit 0
- [x] `mise run test-msrv` — exit 0
- [ ] PR CI の verify-macos 実測を観測（マージ判断は Coordinator）
