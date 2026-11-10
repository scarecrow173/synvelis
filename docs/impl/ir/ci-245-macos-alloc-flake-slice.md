---
id: "IMPL-034Vn0xIlQz4EDxKYyEsSD"
type: "impl"
status: "completed"
title: "Issue #245 rt_cache alloc 計測 flake — 計測対象インスタンス warm-up slice"
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
        path: "docs/impl/ir/ci-245-macos-alloc-flake-slice.md"
      - type: "file"
        path: "docs/specs/ci-245-macos-alloc-flake.md"
      - type: "file"
        path: "docs/designs/ci-245-macos-alloc-flake-design.md"
      - type: "file"
        path: "docs/plans/ci-245-macos-alloc-flake-plan.md"
      - type: "file"
        path: "docs/tasks/ci-245-task1-macos-alloc-flake.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/rt_cache.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/ci-245-task1-macos-alloc-flake.md"
    - "docs/plans/ci-245-macos-alloc-flake-plan.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/ci-245-task1-macos-alloc-flake.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ci-245-macos-alloc-flake.md"
    - "docs/designs/ci-245-macos-alloc-flake-design.md"
    - "docs/plans/ci-245-macos-alloc-flake-plan.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/ci-245-task1-macos-alloc-flake.md"
    - "crates/synvelis-library/tests/rt_cache.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# Issue #245 rt_cache alloc 計測 flake — 計測対象インスタンス warm-up slice

## Summary

`rt_event_enqueue_is_alloc_and_dealloc_free` の warm-up を、計測対象
とは別の使い捨て `RtEventQueue` ではなく計測対象インスタンス自身に
対して行うよう修正した。窓 1 は計測対象 `queue` を
`try_emit`×8 → `drain` → `is_empty` で warm し、窓 2 は計測対象
`sink` 構築直後に `sink.emit(ev(0))` を 1 回 warm-up として追加した。
計測契約（両窓で allocations/deallocations == 0、
`sink.latest() == LibraryGeneration(9)`）は不変。

## Context

`docs/tasks/ci-245-task1-macos-alloc-flake.md` /
`docs/plans/ci-245-macos-alloc-flake-plan.md` /
`docs/specs/ci-245-macos-alloc-flake.md` /
`docs/designs/ci-245-macos-alloc-flake-design.md` に従う。
verify-macos run 42750871026 で `rt_cache.rs` の alloc 計測が
`allocations: 1 (64 bytes)`（期待 0）で flake した件の是正。
既存 RT 計測テスト（`aut_04_rt.rs`・`input_adapter_rt_alloc.rs`・
`vs_b_rt_alloc.rs`・`pc_b_rt.rs`・`trace_c3_05_rt.rs`・
`tim02_midi_rt.rs`・`tim03_process_boundary_rt.rs`）が計測対象
インスタンス自身を warm するパターンに揃えた。

## Implementation

### Changes

- `crates/synvelis-library/tests/rt_cache.rs`
  - 使い捨て queue の warm-up ブロックを廃止し、計測対象 `queue`
    に対して warm-up（`try_emit(ev(1..=8))` → `drain()` →
    `is_empty()`）を `Region` の外で実行。`drain().len() == 8` と
    `is_empty()` の assert を追加。
  - 窓 2: `sink` 構築直後・`Region` の外で `sink.emit(ev(0))` を
    warm-up として追加。generation 0 は watermark を進めず gate を
    通過して 1 slot を占有する。warm event は `SuppressingSink`
    経由では drain できず undrainable のため空き slot は 7 になり、
    gate 通過 9 イベントのうち `ev(8)`・`ev(9)` が full-queue drop
    分岐を追加で踏む — この 2 点をコメントで明記。
- 文書チェーン: `docs/specs|designs|plans|tasks|impl/ir/ci-245-*`。

### Decisions During Implementation

- warm event の generation は 0 を選択。`SuppressingSink` の
  watermark 初期値が `LibraryGeneration(0)` のため、observed=0 は
  `observed < prev` に該当せず gate を通過しつつ watermark を
  進めない。generation ≥1 の warm event だと計測窓の
  `ev(1)..` が stale-drop され queue 投入経路の計測が弱まるため、
  0 が最小の warm emit。
- warm event を 1 件に限定。`SuppressingSink` は inner queue への
  drain 経路を公開しないため残留は不可避であり、1 件で
  gate 通過 + queue 投入の first-touch をカバーできる。
  full-queue drop 分岐は計測窓内で追加カバーされる（仕様どおり
  コメント明記）。

## Related Experiments

### Adopted

- なし

### Rejected

- なし

## Validation

ホスト: Windows（ローカル）。cargo 系は `mise run` 経由、
`cargo test -p synvelis-library --test rt_cache` の直接実行は
補助証拠（canonical = mise）。

- `mise run fmt-check` — exit 0（cargo fmt --all -- --check）。
- `mise run check` — exit 0（cargo check --workspace --all-targets）。
- `mise run lint` — exit 0（cargo clippy --workspace --all-targets
  -- -D warnings）。
- `mise run test` — exit 0（cargo test --workspace --locked、
  `rt_event_enqueue_is_alloc_and_dealloc_free` 含む全件 pass）。
- `mise run test-rt` — exit 0（`rt_` filter、同テスト pass）。
- `mise run test-msrv` — exit 0（rust 1.89.0 toolchain で
  `cargo test --workspace --locked`、同テスト pass）。
- 補助証拠: `mise exec -- cargo test -p synvelis-library --test
  rt_cache --locked` — 1 passed（dev profile）。

未実行: macOS ローカル検証（NotRun — 本ホストは Windows）。
macOS での効果は PR CI の verify-macos 実測で観測する。単一の
green は flake 除去の証明にならない。

## Risks

- warm event 残留により窓 2 の enqueue 成功数が 8→7 に減るが、
  emit 経路は呼び出し毎に同一コードを通り、full-queue drop 分岐の
  カバレッジは増える。観測契約への影響なし。
- macOS アロケータの 64 bytes lazy-init の具体的発生源は未同定。
  同一インスタンス warm-up は既存パターンに沿った契約内の対処で
  あり、flaky 再発時は発生源の同定を別途行う。

## Follow-ups

- `crates/synvelis-workspace/tests/aut_04_rt.rs`・
  `crates/synvelis-player/tests/trace_c3_05_rt.rs` など、同種の
  warm-up リスクを持つ他 RT 計測テストの棚卸し（本 slice の編集
  範囲外。PR 本文「未対応の作業」に記録）。
- PR CI の verify-macos 実測を観測し、flake 再発の有無を確認
  （マージ判断は Coordinator）。
