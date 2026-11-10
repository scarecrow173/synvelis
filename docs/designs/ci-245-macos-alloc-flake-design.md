---
id: "DESIGN-034VmgS1sGu9EW9NfKKRPY"
type: "design"
status: "approved"
title: "Issue #245 verify-macos rt_cache alloc 計測 flake — 計測対象インスタンス warm-up 設計"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/245"
    - "https://github.com/scarecrow173/synvelis/actions/runs/42750871026"
  changes:
    added:
      - type: "file"
        path: "docs/designs/ci-245-macos-alloc-flake-design.md"
      - type: "file"
        path: "docs/plans/ci-245-macos-alloc-flake-plan.md"
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
    - "docs/specs/ci-245-macos-alloc-flake.md"
  implemented-by:
    - "docs/plans/ci-245-macos-alloc-flake-plan.md"
  depends-on:
    - "docs/specs/ci-245-macos-alloc-flake.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-03-catalog-preview-cache.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ci-245-macos-alloc-flake.md"
  derived-by:
    - "docs/plans/ci-245-macos-alloc-flake-plan.md"
    - "docs/tasks/ci-245-task1-macos-alloc-flake.md"
    - "docs/impl/ir/ci-245-macos-alloc-flake-slice.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-library/tests/rt_cache.rs"
    - "crates/synvelis-library/src/notify.rs"
  defers: []
  deferred-by: []
---

# Issue #245 verify-macos rt_cache alloc 計測 flake — 計測対象インスタンス warm-up 設計

## Context

`rt_event_enqueue_is_alloc_and_dealloc_free` は `stats_alloc` の
グローバルアロケータで `Region` を開き、その窓内の
`RtEventQueue::try_emit` / `SuppressingSink::emit` が
alloc/dealloc を行わないことを検証する。現行実装の warm-up は
計測対象とは別の使い捨て `RtEventQueue` を暖めるだけで、
計測対象の `queue`（窓 1）と `sink`（窓 2）は cold のまま
`Region` に入る。verify-macos run 42750871026 では窓 1 で
`allocations: 1 (64 bytes)` が観測され、macOS アロケータの
初回 lazy-init 系 flake と判定された。他 OS の同一 SHA は
pass しており、契約違反ではなく計測対象インスタンスの
first-touch コストが窓に混入したものと判断する。

既存の RT 計測テスト群（`aut_04_rt.rs`・
`input_adapter_rt_alloc.rs`・`vs_b_rt_alloc.rs`・`pc_b_rt.rs`・
`trace_c3_05_rt.rs`・`tim02_midi_rt.rs`・
`tim03_process_boundary_rt.rs`）はいずれも計測対象インスタンス
自身を warm しており、本設計はそのパターンへ揃える。

## Scope

### In Scope

- `crates/synvelis-library/tests/rt_cache.rs` の warm-up を
  計測対象インスタンス自身に対して行う変更とコメント。

### Out of Scope

- 計測契約（窓内 allocations/deallocations == 0）の変更。
- 他テストファイル・プロダクトコード・workflow の変更。

## Components and Boundaries

- `rt_cache.rs`（テスト）: 唯一の変更対象。計測窓の構成と
  warm-up の配置を変更する。`synvelis-library` の
  `RtEventQueue` / `SuppressingSink`（`src/notify.rs`）の
  実装には触れない — 契約は「warm 後 alloc/dealloc free」で
  あり、first-touch コストは契約外の初期化として warm-up で
  除外するのが既存パターン。

## Data and Control Flow

窓 1（`RtEventQueue` 単体）:

1. `let queue = RtEventQueue::new(8)` — slot ring の Vec 確保は
   計測窓の外（現行どおり）。
2. warm-up（窓の外）: `try_emit(ev(1..=8))` → `drain()` →
   `is_empty()`。first-touch の mutex/slot ring 経路と lazy
   初期化をこのインスタンスで消化する。
3. `Region::new(ALLOC)` → 計測対象の `events`（`ev(1)..ev(8)`）を
   `try_emit` → `reg.change()` で allocations/deallocations == 0
   を assert → `queue.len() == 8` → `drain()`。

窓 2（`SuppressingSink` 合成経路）:

1. `let sink = SuppressingSink::new(RtEventQueue::new(8),
   LibraryGeneration(0))` — 構築は窓の外（現行どおり）。
2. warm-up（窓の外）: `sink.emit(ev(0))` を 1 回。generation 0 は
   watermark 初期値 0 を進めず gate を通過して queue の 1 slot
   を占有する。`SuppressingSink` は inner queue への drain 経路を
   公開しないため、この warm event は undrainable であり、
   空き slot は 7 になる。
3. `Region::new(ALLOC)` → 計測対象の 10 events（`ev(1)..ev(8)`、
   stale `ev(3)`、`ev(9)`）を `emit` → alloc/dealloc == 0 を
   assert → `sink.latest() == LibraryGeneration(9)`。

watermark への影響: warm event は generation 0 のため
`latest` は 0 のまま。計測窓の `ev(1)..ev(8)` は全て gate を
通過し queue へ向かい、2 個目の `ev(3)` は watermark 8 に対し
stale で gate drop、`ev(9)` で `latest == 9` に到達する —
現行と同一の観測結果。

slot への影響: warm event が 1 slot を占有するため、gate 通過
イベント 9 件のうち先頭 7 件が enqueue 成功、残り 2 件
（`ev(8)`・`ev(9)`）が full-queue drop 分岐を踏む。`try_emit`
の戻り値は `LibraryEventSink::emit` が捨てるため観測契約に
影響せず、full-queue drop 分岐のカバレッジは増える。

## Non-Functional Constraints

- 計測契約は不変: 両窓で `stats.allocations == 0`・
  `stats.deallocations == 0` を assert し続ける。
- warm-up は `Region` の外でのみ実行し、計測対象インスタンス
  と同一であること。
- 新規依存・`#[allow]`・成功条件の緩和を行わない。

## Risks and Trade-offs

- warm event が queue に残ることで計測窓の enqueue 成功数が
  8→7 に減る。対価として full-queue drop 分岐が追加で検証され、
  enqueue 成功経路は 7 件で依然十分にカバーされる。`sink` の
  内部 queue を drain する公開経路がないため、この残留は
  設計上不可避でありコメントで明記する。
- warm-up が first-touch 以外のコストまで隠す可能性はない:
  `try_emit`/`emit` の定常経路は warm 前後で同一コードを通る。
- macOS 効果はローカル（Windows）では検証不能。PR CI の
  verify-macos 実測で観測し、単一 green を flake 除去の証明と
  はみなさない。

## Deferred Design Concerns

- 他の RT 計測テストの warm-up パターン統一（spec の Deferred
  Design Concerns と同件）。本 slice では `rt_cache.rs` のみ。

## Verification Notes

- [x] `cargo test -p synvelis-library --test rt_cache` が
  Windows ローカルで pass（`mise run test` / `test-rt` 経由）。
- [ ] verify-macos の PR CI 実測 — マージ後に run を観測する
  （ローカル macOS 検証は NotRun）。
