---
id: "IMPL-034UHWGxnC2sjDoLd5P01u"
type: "impl"
status: "completed"
title: "TIM-01 Configurable PPQN Project Command・非破壊 Migration（headless slice）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/ppqn.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/tim_01_ppqn.rs"
      - type: "file"
        path: "docs/specs/tim-01-configurable-ppqn-project-command.md"
      - type: "file"
        path: "docs/designs/tim-01-configurable-ppqn-design.md"
      - type: "file"
        path: "docs/plans/tim-01-configurable-ppqn.md"
      - type: "file"
        path: "docs/tasks/tim-01-configurable-ppqn.md"
      - type: "file"
        path: "docs/test-specs/tim-01-configurable-ppqn.md"
      - type: "file"
        path: "docs/impl/ir/tim-01-configurable-ppqn.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/tempo.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
  implements:
    - "docs/tasks/tim-01-configurable-ppqn.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/tim-01-configurable-ppqn.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/designs/tim-01-configurable-ppqn-design.md"
    - "docs/test-specs/tim-01-configurable-ppqn.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# TIM-01 Configurable PPQN Project Command・非破壊 Migration — implementation record

## Summary

Issue #125 の headless slice を実装した。固定 960 前提の
`ProjectOp::SetPpqn` stub を `SetProjectPpqn { old_ppqn,
new_ppqn, policy }` に置換し、project 所有の全 musical tick を
Ruviform checked API 経由の exact rational beat で再投影する
`ppqn` module を新設した。`1..=32767` 範囲・二重 CAS・
atomic validation・open-recording 拒否・`project.ppqn ==
tempo_map.ppqn` 不変条件を備え、14 件の headless fixture で
Undo/Redo・Save/Reload・旧 snapshot 非破壊・保存対象外データ
の完全一致を証明した。

## Context

`docs/specs/tim-01-configurable-ppqn-project-command.md` /
`docs/designs/tim-01-configurable-ppqn-design.md` /
`docs/plans/tim-01-configurable-ppqn.md` /
`docs/tasks/tim-01-configurable-ppqn.md` に従う。
TIM-02/03/04（#126/#127/#128 merged）の既存 Command・schema・
migration 規約を踏襲。

## Implementation

### Changes

- `crates/synvelis-project/src/ppqn.rs`（新規）: `PPQN_MIN/MAX`、
  `ppqn_in_range`、`PpqnConversionPolicy`（Exact/Round）、
  musical tick の rational beat 再投影ヘルパ、project aggregate
  一括変換 `reproject_ppqn`。
- `crates/synvelis-project/src/ops.rs`: `SetPpqn` を
  `SetProjectPpqn` に置換。validate で expected_revision・
  old_ppqn・範囲・open-recording・scratch clone 再投影を検証し、
  apply で `reproject_ppqn` を実行。
- `crates/synvelis-project/src/project.rs`: `validate_references`
  に PPQN coherence（`project.ppqn == tempo_map.ppqn`、範囲内）を追加。
- `crates/synvelis-project/src/lib.rs`: `ppqn` module 登録・export。
- `crates/synvelis-project/src/tempo.rs`: configurable PPQN 向け
  doc 更新。
- `crates/synvelis-gateway/src/prepare.rs`: op 名
  `set-ppqn` → `set-project-ppqn`。
- `crates/synvelis-project/tests/tim_01_ppqn.rs`（新規）: 14 tests。

### Decisions During Implementation

- 変換は `project.ppqn` 代入ではなく、全 musical tick を
  `TickPosition::to_beat` → `from_beat`/`rescale`（checked API）
  で再投影。Ruviform の一般化 Time API は重複実装しない。
- `Exact` policy は inexact を即拒否、`Round` policy は呼び出し側
  宣言の `Rounding` を全位置に一貫適用。暗黙丸めなし。
- `SCHEMA_VERSION`（現行 9）bump なし。PPQN は既存 serialized
  field の値域拡張であり、旧 snapshot はそのまま load 可能。
  migration は read-only projection・save は新 snapshot 発行の
  既存契約を維持。
- transport-stopped・automation-write・merge-commit 前提条件は
  app 層責務として `synvelis-project` に重複実装しない。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

- `cargo test -p synvelis-project --test tim_01_ppqn` — 14 passed
- `cargo test --workspace --locked` — 全 workspace green
- `cargo test --workspace --locked rt_` — RT lane green
- `mise exec rust@1.89.0 -- cargo test --workspace --locked` — MSRV green
- `cargo fmt --all -- --check` / `cargo clippy --workspace
  --all-targets -- -D warnings` / `cargo check --workspace
  --all-targets` — 全て exit 0
- NotRun: 実 OS/実 GUI・製品 Oracle（headless slice 対象外）

## Risks

- app 層の PPQN 選択 UI・前提条件（transport/automation/merge）は
  別 slice。headless Command 契約のみが本 slice の成果物。
- `1..=32767` は SMF 指向 UI slice と同一範囲。将来の範囲拡張は
  spec 改訂を要する。

## Follow-ups

- 製品 UI の PPQN 設定・app 層前提条件の統合（別 Issue/slice）。
- 実機・製品 Oracle 検証（別 slice）。
