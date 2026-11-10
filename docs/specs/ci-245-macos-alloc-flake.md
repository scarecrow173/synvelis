---
id: "SPEC-034VmfUEU36QwKnlrMNd8G"
type: "spec"
status: "proposed"
title: "Issue #245 verify-macos rt_cache alloc 計測 flake — 計測対象インスタンスの warm-up"
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
        path: "docs/specs/ci-245-macos-alloc-flake.md"
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
  implements: []
  implemented-by:
    - "docs/designs/ci-245-macos-alloc-flake-design.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/alib-03-catalog-preview-cache.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/ci-245-macos-alloc-flake-design.md"
    - "docs/plans/ci-245-macos-alloc-flake-plan.md"
    - "docs/tasks/ci-245-task1-macos-alloc-flake.md"
    - "docs/impl/ir/ci-245-macos-alloc-flake-slice.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-library/tests/rt_cache.rs"
    - "crates/synvelis-library/src/notify.rs"
    - ".github/workflows/ci.yml"
  defers: []
  deferred-by: []
---

# Issue #245 verify-macos rt_cache alloc 計測 flake — 計測対象インスタンスの warm-up

## What Should Be Built

`crates/synvelis-library/tests/rt_cache.rs` の
`rt_event_enqueue_is_alloc_and_dealloc_free` が、計測窓に入る前に
**計測対象と同一インスタンス**を warm-up し、初回タッチ由来の遅延
アロケーションを計測窓から除外する。現行の warm-up は使い捨ての
`RtEventQueue` を暖めるだけで、計測対象の queue / sink は cold の
まま計測窓に入る。リポジトリ内の既存 RT 計測テスト
（`aut_04_rt.rs`・`input_adapter_rt_alloc.rs`・`vs_b_rt_alloc.rs`・
`pc_b_rt.rs`・`trace_c3_05_rt.rs`・`tim02_midi_rt.rs`・
`tim03_process_boundary_rt.rs`）は、いずれも計測対象インスタンス
自身を warm するパターンであり、本修正はそれに揃える。

## Why Now

main `17fc3dec` の Full CI run 42750871026 で `verify-macos` のみが
`rt_event_enqueue_is_alloc_and_dealloc_free`（rt_cache.rs:52）の
`allocations: 1 (64 bytes)`（期待 0）で失敗し、`ci-summary` が
fail-closed で連鎖 failure になった。同一 SHA で Windows / Ubuntu
lane は pass しており、macOS アロケータの初回 lazy-init 系 flake
と判定済み。fail-closed の `ci-summary` が赤いままでは main の
検証信号が得られない。

## Users and Value

- Synvelis 開発者・CI 運用者: main の Full CI が再び緑になり、
  verify-macos の合否が実際の回帰を反映する状態に戻る。
- `synvelis-library` の RT 契約テスト: 「queue 投入経路は warm 後
  alloc/dealloc free」という計測契約が、OS 非依存に安定して
  検証される。

## Scope

### In Scope

- `rt_cache.rs` の warm-up を計測対象インスタンス自身に対して行う
  修正と、その根拠を示すコメント。
- 二つ目の計測窓（`SuppressingSink` 経路）への warm-up 追加。
  warm event が `SuppressingSink` 経由で queue に残り drain できない
  ため空き slot が 7 になり、計測窓で full-queue drop 分岐が追加で
  踏まれる点をコメントで明記する。

### Out of Scope

- 計測契約の緩和（窓内 `allocations == 0`・`deallocations == 0` の
  成功条件は変更しない）。
- `crates/synvelis-workspace/tests/aut_04_rt.rs` と
  `crates/synvelis-player/tests/trace_c3_05_rt.rs` など、同種の
  flake リスクを持つ他テストの修正（別 slice 候補として PR 本文に
  記録する）。
- `synvelis-library` のプロダクトコード（`src/`）の変更、
  `.github/workflows/` の lane 構成変更、`Cargo.toml`/`Cargo.lock`。

## Requirements

- 計測窓 1（`RtEventQueue` 単体）: 計測対象の `queue` を計測前に
  `try_emit`×8 → `drain` → `is_empty` で warm し、初回アロケーション
  を窓の外で吸収する。
- 計測窓 2（`SuppressingSink` 合成経路）: 計測対象の `sink` を計測前に
  構築し、warm event を `emit` して gate の watermark 判定と queue
  投入経路を warm する。`assert_eq!(sink.latest(),
  LibraryGeneration(9))` と alloc/dealloc assert を壊さないこと。
- warm-up は契約外の初期化コストのみを除外し、テストの成功条件・
  計測契約を弱めないこと。
- 新規の外部依存を追加しないこと。

## Acceptance Criteria

- [ ] `rt_event_enqueue_is_alloc_and_dealloc_free` の両計測窓が、
  計測対象と同一インスタンスの warm-up 後に `allocations == 0`・
  `deallocations == 0` を要求する（契約不変）。
- [ ] `cargo test -p synvelis-library --test rt_cache` が Windows
  ローカルで pass する。
- [ ] `mise run test`・`mise run test-rt` が pass する。
- [ ] PR の CI で `verify-macos` を含む Full CI の実測を観測する
  （macOS ローカル検証は NotRun。単一 green は flake 除去の証明に
  ならない点を PR 本文に明記）。

## Deferred Design Concerns

<!-- Intentionally deferred future work. Link the deferred draft doc via relations.defers. -->

- 同種の「別インスタンス warm-up」または warm-up なしの RT 計測
  flake リスクを持つ他テスト（`aut_04_rt.rs`・
  `trace_c3_05_rt.rs` 等）の棚卸しと同パターン修正。再エンゲージ
  条件: verify-macos または他 lane で同型の alloc flake が再発した
  場合、または RT 計測テストの warm-up 規約を文書化する slice が
  起きた場合。無視した場合のリスク: 同型 flake が断続的に CI を
  赤くし、fail-closed gate の信号価値を下げる。

## Sources and Relations

- Issue #245（CI failure tracker、managed コメントは編集しない）。
- 失敗 run: <https://github.com/scarecrow173/synvelis/actions/runs/42750871026>
  — `verify-macos` で `rt_cache.rs:52` が
  `allocations: 1 (64 bytes)`（期待 0）で FAILED、`ci-summary` が
  fail-closed で連鎖 failure。他 lane は同一 SHA で success。
- warm-up パターンの先例: `crates/synvelis-workspace/tests/aut_04_rt.rs`、
  `crates/synvelis-recording/tests/input_adapter_rt_alloc.rs`、
  `crates/synvelis-recording/tests/vs_b_rt_alloc.rs`、
  `crates/synvelis-project/tests/pc_b_rt.rs`、
  `crates/synvelis-player/tests/trace_c3_05_rt.rs`、
  `crates/synvelis-player/tests/tim02_midi_rt.rs`、
  `crates/synvelis-player/tests/tim03_process_boundary_rt.rs`。
