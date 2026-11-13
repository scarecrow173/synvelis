---
id: "TSPEC-3ppuFy4wUxgYHFtuGGZHG6"
type: "test-spec"
status: "approved"
title: "TIM-01 PPQN config — punch marks 再投影と受入残件 test spec"
created: "2026-11-14"
updated: "2026-11-14"
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
  related:
    - "docs/test-specs/tim-01-configurable-ppqn.md"
    - "docs/test-specs/tim-02-ppqn-smf-cross-golden.md"
  refines:
    - "docs/test-specs/tim-01-configurable-ppqn.md"
  refined-by: []
  derives-from:
    - "docs/designs/tim-01-ppqn-config-design.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-01-ppqn-config.md"
  verified-by: []
  references:
    - "docs/tasks/tim-01-ppqn-config-task.md"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN config — punch marks 再投影と受入残件 test spec

## Purpose

Issue #125 残件 slice の契約を
`crates/synvelis-project/tests/tim_01_ppqn.rs` で固定する。
punch marks の再投影 site 追加と、Issue 受入条件の
`synvelis-project` 範囲の残件（6 PPQN 行列・v5 snapshot・
分母上限・同 tick site・保存対象外安全性・異 Division import）
を検証する。既存 test が被覆する契約（CAS・Exact/Round・
overflow・open-recording 拒否・identity no-op・preview 報告・
SMF export cross-golden）は重複追加せず本表に記録する。

## Feature

`SetProjectPpqn` の再投影 walk が `Project` aggregate の全
persisted musical tick（punch marks を含む）を exact rational
beat 経由で再投影し、保存対象外データを一切変更しないこと。

## Rules

- `monitoring.punch.punch_in`/`punch_out` は他の musical site
  と同一規則（`Exact`/`Round` policy、atomic validation、
  overflow 拒否）で再投影され、`preview_ppqn` が同 site を
  `site_count`/`inexact`/`overflow_sites` に報告する。
- PPQN `1/480/960/1000/1920/32767` の各 target で、Save →
  Reload → Undo → Redo が ppqn・全 musical beat 位置・
  TempoMap→Frame 解決を保存する。
- 実 v5 snapshot（schema v5 fixture）の migrate → PPQN 変更 →
  save → reload で、v5 bytes が byte-identical のまま残り、
  source-native MIDI data（`midi_sources`/`notes`/`smf_documents`）
  が完全一致を保つ。
- beat の既約分母が target grid に非表現の位置（分母上限）は
  `Exact` で拒否、`Round` で宣言 rounding のみ適用される。
- 同一 tick を共有する複数 site は再投影後も一致し続ける。
- 異 Division の SMF import source は PPQN 変更後も
  `smf_documents`・`MidiSource.division`・notes が完全一致。
- automation `BeatPoint`・`control`/`plugin_instances`/`routing`/
  `notes`/`recording`/`applied_imports` subtree は PPQN 変更で
  完全一致（frame_base 相当の runtime 状態は project 非保持）。

## Examples

- `ppqn_change_preserves_beat_positions_and_reprojects_ticks`
  （punch spot assertion + `musical_positions` 包含）:
  Given punch marks を持つ project、When PPQN 変更、Then 両
  mark が beat 保存で再投影される。
- `preview_ppqn_reports_inexact_punch_marks`: Given inexact な
  punch_in を持つ project、When `preview_ppqn`、Then
  `kind == "punch_in"` の site が報告され feasibility が
  command と一致する。
- `ppqn_sweep_save_reload_undo_redo_matrix`: Given seeded
  project、When 各 target PPQN（1/480/960/1000/1920/32767）で
  apply→save→load→undo→redo、Then 全段階で beat 位置・ppqn が
  一致し reload が live と等しい。
- `ppqn_change_on_v5_snapshot_preserves_migrated_data`: Given
  実 v5 snapshot（`fixtures/alib02-v5-legacy.json`）、When
  migrate→960→480 Exact→save→reload、Then v5 bytes 不変・
  source-native MIDI data 完全一致・再投影結果保持。
- `ppqn_denominator_boundary_is_explicit`: Given `1/960` beat
  の位置（tick 1 @960）、When 32767 への Exact/Round、
  Then Exact 拒否・Round(Floor/Ceil) が 34/35 を宣言適用。
- `ppqn_change_handles_same_tick_sites`: Given tick 960 を
  共有する tempo/signature/clip anchor/extent/punch_in、
  When PPQN 変更、Then 全 site が同一 tick へ再投影される。
- `ppqn_change_preserves_absolute_media_and_source_native_data`
  （拡張）: Given automation region（4/3 beat の非格子
  BeatPoint）・plugin instance・division 480 の SMF import を
  持つ project、When PPQN 変更、Then `control`/`routing`/
  `plugin_instances`/`smf_documents`/`midi_sources`/`recording`
  が完全一致。

## 既存被覆（重複追加しない）

| 受入要素 | 被覆 test（`tim_01_ppqn.rs` 既存） |
| --- | --- |
| 二重 CAS・範囲外拒否・open-recording 拒否・identity no-op | `ppqn_change_enforces_double_cas`・`ppqn_out_of_range_rejected`・`ppqn_change_blocked_by_open_recording_session`・`ppqn_identity_change_is_noop` |
| Exact inexact 拒否・Round 宣言適用 | `ppqn_exact_policy_rejects_inexact_positions`・`ppqn_round_policy_applies_declared_rounding` |
| overflow・長大曲 | `ppqn_change_rejects_tick_overflow`・`ppqn_change_handles_long_arrangements` |
| 複数変更合成・sweep | `ppqn_changes_compose_across_steps`・`ppqn_sweep_covers_selectable_range` |
| preview 報告 | `preview_ppqn_*` 4 tests |
| Beat→Frame RT 丸め再現（960→480） | `rt_ppqn_beat_to_frame_rounding_reproduces` |
| Save/Reload・旧 snapshot 非破壊（960→480） | `ppqn_change_survives_save_reload_and_keeps_old_snapshot` |
| 異 Division SMF import→export cross-golden | `tim_02_ppqn_smf.rs`（TIM-02 側 fixture） |

## Guarantees

- punch marks を含む全 persisted musical tick が同一規則で
  再投影され、preview と command が同一 site を報告する。
- 6 PPQN 値で Save/Reload/Undo/Redo・Beat→Frame 解決が保存
  される。
- 実 v5 snapshot・source-native MIDI・automation・plugin/
  routing・recording data が PPQN 変更で改竄されない。
- 分母上限・同 tick site・可変テンポ下で silent retime が
  起きない（拒否または宣言済み quantization のみ）。

## Non-goals

- GUI PPQN 選択・Preview 表示・Workspace/実 OS 受入。
- app 層前提条件（transport/automation-write/merge-commit）。
- Ruviform Time API の契約テスト（Ruviform 側責務）。
- Project Schema（#17）/Workspace 別 schema の版管理。

## Risk

- punch marks 追加漏れが再発した場合、monitoring intent が
  silent retime される — `musical_positions` 包含と spot
  assertion で検出する。
- v5 fixture の前提（whole-beat 位置・session 無し）が崩れた
  場合 test が fail し、fixture 側の前提変更として顕在化する。
