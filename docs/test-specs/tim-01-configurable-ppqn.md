---
id: "TSPEC-034UHWDhuZ4tU11F5QFAEh"
type: "test-spec"
status: "approved"
title: "TIM-01 configurable PPQN — test spec"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "crates/synvelis-project/tests/tim_01_ppqn.rs"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/tim-01-configurable-ppqn-design.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
  verified-by: []
  references:
    - "docs/tasks/tim-01-configurable-ppqn.md"
  defers: []
  deferred-by: []
---

# TIM-01 configurable PPQN — test spec

## Purpose

Issue #125 headless slice の `SetProjectPpqn` Command と
rational-beat 再投影が spec の受入基準を満たすことを、
`crates/synvelis-project/tests/tim_01_ppqn.rs`（14 tests）で
固定する。製品 UI・実 OS/実 GUI・製品 Oracle は対象外のため、
本 spec が退役するのは headless slice の契約が上位 slice に
統合された時点。

## Feature

Project PPQN を `1..=32767` で変更する Command が、音楽的意味を
変えずに project 所有の全 musical tick を exact rational beat
経由で再投影し、CAS/Undo/Save/Reload/非破壊 migration を
既存 controller 契約で提供する。

## Rules

- PPQN 変更は `expected_revision` CAS と `old_ppqn` ドメイン CAS
  の両方を要求する。stale revision・stale old_ppqn は拒否。
- `new_ppqn` は `1..=32767` 外を拒否。`old_ppqn == new_ppqn` は
  no-op として成功扱い。
- `Exact` policy は target grid に表現不能な位置を一箇所でも
  含むと operation 全体を拒否し、project を無変更に保つ。
- `Round` policy は宣言された `Rounding` を全位置に一貫適用する。
  暗黙の丸め経路は存在しない。
- 再投影は tempo/signature event・musical clip anchor/extent・
  musical comping range・`Project.ppqn`/`TempoMapData.ppqn` を
  含み、tempo map revision を前進させる。
- `TimeAnchor::Absolute`・`ClipExtent::Absolute`・source-native
  `MidiTick`/SMF raw bytes・media frame・recording timestamp・
  plugin frame base は byte/value 同一を保つ。
- `SessionState::Open` の project では PPQN 変更を拒否する。
- 変換 overflow・tick 位置の target grid 非表現可能は拒否する。
- 1 Command = 1 Undo entry。Undo/Redo で PPQN と全 musical tick
  が完全に往復する。
- Save→Reload 後も PPQN と再投影結果が保持され、旧 snapshot
  bytes は load/migration で変更されない。

## Examples

- `ppqn_change_enforces_double_cas`: Given seeded project、
  When stale `expected_revision` または stale `old_ppqn` で
  `SetProjectPpqn` を apply、Then 拒否され project 不変。
- `ppqn_exact_policy_rejects_inexact_positions`: Given
  inexact 位置を含む project、When `Exact` で 960→非約数 PPQN、
  Then 拒否・無変更。
- `ppqn_round_policy_applies_declared_rounding`: Given 同上、
  When `Round` + 明示 `Rounding`、Then 全位置が宣言 policy で
  投影され beat 位置は保存される。
- `ppqn_change_preserves_beat_positions_and_reprojects_ticks`:
  Given tempo/signature/clip/comp を持つ project、When PPQN 変更、
  Then 全 musical beat 位置が保存され tick が正確に再投影される。
- `ppqn_change_preserves_absolute_media_and_source_native_data`:
  Given absolute anchor/extent・MidiTick・SMF raw を持つ project、
  When PPQN 変更、Then それらが完全一致を保つ。
- `ppqn_change_blocked_by_open_recording_session`: Given open
  session、When PPQN 変更、Then 拒否。
- `ppqn_out_of_range_rejected` / `ppqn_change_rejects_tick_overflow`:
  範囲外 PPQN・overflow 位置は拒否。
- `ppqn_change_undo_redo_round_trip`: Undo/Redo で完全往復。
- `ppqn_change_survives_save_reload_and_keeps_old_snapshot`:
  Save/Reload で結果保持、旧 snapshot bytes 非破壊。
- `ppqn_changes_compose_across_steps` / `ppqn_sweep_covers_selectable_range`:
  複数変更の合成と `1..=32767` 全域 sweep。

## Guarantees

- PPQN Command は CAS/Undo/非破壊 migration を既存 controller
  契約で提供する。
- Exact rational beat 変換が musical 意味を保存する。
- 暗黙 quantization・部分適用・stale identity 参照が起きない。

## Non-goals

- 製品 UI の PPQN 選択・app 層前提条件（transport-stopped・
  automation-write・merge-commit）の検証。
- 実 OS/実 GUI・製品 Oracle。
- Ruviform 側 Time API の一般化契約テスト（Ruviform 側責務）。

## Risk

- 変換 policy・CAS・保存対象外データのいずれかの保証が落ちると、
  musical 意味の破壊・silent quantization・部分適用・stale
  TempoMapRef 参照・旧 snapshot 改竄のいずれかが検出不能になる。
