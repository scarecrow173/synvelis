---
id: "PLAN-034VmidubXq53HjWVDgDeV"
type: "plan"
status: "approved"
title: "Issue #245 verify-macos rt_cache alloc 計測 flake — warm-up 実装計画"
created: "2026-11-10"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/245"
    - "https://github.com/scarecrow173/synvelis/actions/runs/42750871026"
  changes:
    added:
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
    - "docs/designs/ci-245-macos-alloc-flake-design.md"
  implemented-by:
    - "docs/tasks/ci-245-task1-macos-alloc-flake.md"
    - "docs/impl/ir/ci-245-macos-alloc-flake-slice.md"
  depends-on:
    - "docs/designs/ci-245-macos-alloc-flake-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ci-245-macos-alloc-flake.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ci-245-macos-alloc-flake.md"
    - "docs/designs/ci-245-macos-alloc-flake-design.md"
  derived-by:
    - "docs/tasks/ci-245-task1-macos-alloc-flake.md"
    - "docs/impl/ir/ci-245-macos-alloc-flake-slice.md"
  verifies: []
  verified-by: []
  test-spec-skip: "既存 RT 契約テスト rt_event_enqueue_is_alloc_and_dealloc_free 自体が検証対象であり、本 slice の編集範囲に test-spec 文書を含まない。検証は plan 本文の Verification 節（mise run fmt-check/check/lint/test/test-rt/test-msrv）と impl record の Validation 節に記録済み"
  references:
    - "crates/synvelis-library/tests/rt_cache.rs"
    - "crates/synvelis-library/src/notify.rs"
  defers: []
  deferred-by: []
---

# Issue #245 verify-macos rt_cache alloc 計測 flake — warm-up 実装計画 Implementation Plan

## Goal

`rt_event_enqueue_is_alloc_and_dealloc_free` の両計測窓が、計測対象
インスタンス自身の warm-up 後に alloc/dealloc == 0 を要求するよう、
`rt_cache.rs` の warm-up を同一インスタンス方式へ修正する。

## Scope

単一の実装ストリーム。変更はテストファイル 1 件と文書チェーンに
限定し、プロダクトコード・workflow・他クレートのテストには触れない。

## Upstream Documents

- `docs/specs/ci-245-macos-alloc-flake.md`

## Design Inputs

- `docs/designs/ci-245-macos-alloc-flake-design.md`（approved）

## File Map

- `crates/synvelis-library/tests/rt_cache.rs` - warm-up を計測対象
  `queue` / `sink` 自身に対して行うよう変更し、undrainable warm
  event による空き slot 7・full-queue drop 分岐の追加カバレッジを
  コメントで明記。
- `docs/specs|designs|plans|tasks|impl/ir/ci-245-*` - 本タスクの
  文書チェーン。

## Implementation Sequence

- [x] `rt_cache.rs` の窓 1: 使い捨て queue の warm-up を廃止し、
  計測対象 `queue` に対して `try_emit`×8 → `drain` → `is_empty`
  を `Region` の外で実行する。
- [x] `rt_cache.rs` の窓 2: `sink` 構築直後・`Region` の外で
  `sink.emit(ev(0))` を 1 回 warm-up として追加し、undrainable
  event・空き slot 7・full-queue drop 分岐の追加をコメントで
  明記する。
- [x] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` で検証する。

## Dependencies and Risks

- リスク: warm event の残留で計測窓の enqueue 成功数が減るが、
  full-queue drop 分岐のカバレッジが増え観測契約は不変
  （design の Data and Control Flow 参照）。
- リスク: macOS での効果はローカル検証不能。PR CI の
  verify-macos 実測で観測する。

## Task Breakdown

- `docs/tasks/ci-245-task1-macos-alloc-flake.md` — 唯一の task。
  依存なし、並列分解なし。

## Verification

- [x] `mise run fmt-check` — exit 0。
- [x] `mise run check` — exit 0。
- [x] `mise run lint` — exit 0。
- [x] `mise run test` — exit 0（`rt_cache` を含む全テスト pass）。
- [x] `mise run test-rt` — exit 0。
- [x] `mise run test-msrv` — exit 0。
- [ ] PR CI の verify-macos 実測を観測（マージ判断は Coordinator）。

## Open Questions / Gaps

- macOS アロケータの具体的な lazy-init 機構（64 bytes の発生源）は
  ローカルで同定不能。同一インスタンス warm-up は既存パターンに
  沿った対症的かつ契約内の対処であり、flaky 再発時は発生源の
  同定を別途行う。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一ファイルの小修正）。
- Independent work streams: none。
- User approval required before dispatch: no（受入条件確定済みの
  是正タスク。代わりに review-protocol レビューで blocker なしを
  確認する）。
- If declined or unavailable: execute inline using the same dependency
  graph and verification matrix.

## Review Handoff

- warm event の generation 0 選択（watermark を進めず gate を
  通過する最小の warm emit）と、undrainable event による
  full-queue drop 分岐の追加が観測契約を壊さないことの確認。
