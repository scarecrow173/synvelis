---
id: "IMPL-034WZR7KP4DOMwnoStw98I"
type: "impl"
status: "in-progress"
title: "TIM-01 PPQN config — punch marks re-projection and acceptance residual verification"
created: "2026-11-13"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/tests/tim_01_ppqn.rs"
      - type: "file"
        path: "docs/specs/tim-01-ppqn-config.md"
      - type: "file"
        path: "docs/designs/tim-01-ppqn-config-design.md"
      - type: "file"
        path: "docs/plans/tim-01-ppqn-config-plan.md"
      - type: "file"
        path: "docs/tasks/tim-01-ppqn-config-task.md"
      - type: "file"
        path: "docs/test-specs/tim-01-ppqn-config.md"
      - type: "file"
        path: "docs/impl/ir/tim-01-ppqn-config.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/ppqn.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
  implements:
    - "docs/tasks/tim-01-ppqn-config-task.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/tim-01-ppqn-config-task.md"
    - "docs/impl/ir/tim-01-configurable-ppqn.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/tim-02-ppqn-smf-cross-golden.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/tim-01-ppqn-config-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/tim-01-ppqn-config.md"
  verified-by: []
  references:
    - "docs/specs/tim-01-ppqn-config.md"
    - "docs/designs/tim-01-ppqn-config-design.md"
    - "docs/plans/tim-01-ppqn-config-plan.md"
    - "docs/test-specs/tim-01-ppqn-config.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/tim-01-ppqn-config.md"
    rejected: []
---

# TIM-01 PPQN config — punch marks re-projection and acceptance residual verification

## Summary

Issue #125 の `synvelis-project` 残件 slice を実装した。
先行 slice の `reproject_ppqn_with`（`crates/synvelis-project/src/ppqn.rs`）
が `Project::monitoring.punch` の `punch_in`/`punch_out`
（永続化される project musical tick）を site 列挙から漏らして
いた silent retime を修復し、`preview_ppqn` の dry-run も同一
walk を共有するため両者が同じ site を報告する。併せて Issue
受入条件の headless 検証残件 — 6 PPQN
（1/480/960/1000/1920/32767）の Save/Reload/Undo/Redo 行列、
実 v5 snapshot の非破壊 migration、分母上限、同 tick 複数
site、保存対象外 subtree（automation `BeatPoint`・plugin
instance・SMF document/異 Division・recording）の完全一致 —
を `tests/tim_01_ppqn.rs` で固定した。

## Context

`docs/tasks/tim-01-ppqn-config-task.md` /
`docs/plans/tim-01-ppqn-config-plan.md` /
`docs/designs/tim-01-ppqn-config-design.md` /
`docs/specs/tim-01-ppqn-config.md` /
`docs/test-specs/tim-01-ppqn-config.md` の文書チェーンに従う。
先行実装（`docs/impl/ir/tim-01-configurable-ppqn.md`）が
`SetProjectPpqn` コマンド・CAS・`PpqnConversionPolicy`・
`preview_ppqn`・atomic scratch-clone 検証を提供済みであり、
本 slice はその site 列挙完全性の修復と受入残件の検証被覆に
限定される。GUI/Workspace/app 層の配線と実 OS 受入は別 slice。

## Implementation

### Changes

- `crates/synvelis-project/src/ppqn.rs`
  - `reproject_ppqn_with` に punch marks の再投影を追加:
    `project.monitoring.punch.punch_in`/`punch_out` の各
    `Some(MusicalTick)` を `PpqnSite::single("punch_in"|
    "punch_out", "project")` で識別し、既存の `convert`
    コールバック（`TickPosition::to_beat` → `from_beat`、
    `Exact`/`Round` policy、checked conversion）を通す。
    `{None, None}` は site を生やさない。
  - `PpqnSite`/`PpqnPreviewSite` の kind 語彙、`Display` の
    表示名（`punch-in mark`/`punch-out mark`）、module doc・
    `PpqnChangePreview.site_count` doc を更新。
- `crates/synvelis-project/src/ops.rs`
  - `ProjectOp::SetProjectPpqn` の doc コメントの再投影対象
    列挙に punch marks を追記（doc のみ、挙動変更なし）。
- `crates/synvelis-project/tests/tim_01_ppqn.rs`
  - `seeded()` fixture を拡張: `SetPunchMarks`（whole-beat 値）・
    automation lane/region（`BeatPoint` 4/3 beat の非格子位置を
    含む）・resolved processor（`plugin_instances` 払い出し）・
    division 480 の `SmfDocument` import を追加し、canonical
    fixture が全 site kind と保存対象外 subtree を含むように
    した。
  - `musical_positions` に punch entries を追加。
  - preview `site_count` assertion を 11→13 に更新。
  - 既存 test 拡張: `ppqn_change_preserves_beat_positions_
    and_reprojects_ticks` に punch spot assertion、
    `ppqn_change_preserves_absolute_media_and_source_native_data`
    に `smf_documents`/`control`/`routing`/`plugin_instances`/
    `midi_sources` 全 map の完全一致 assertion を追加。
  - 新規 test 5 件:
    `ppqn_sweep_save_reload_undo_redo_matrix`（6 PPQN 全値で
    apply→save→load→undo→redo + beat 位置 + Beat→Frame span
    一致）、
    `ppqn_change_on_v5_snapshot_preserves_migrated_data`
    （実 v5 fixture migrate→960→480→save→reload、v5 bytes
    byte-identical）、
    `ppqn_denominator_boundary_is_explicit`（1/960 beat →
    32767 で Exact 拒否・Round Floor 34/Ceil 35）、
    `ppqn_change_handles_same_tick_sites`（tick 960 を共有する
    tempo/signature/clip/punch_in が同一 tick へ再投影）、
    `preview_ppqn_reports_inexact_punch_marks`（inexact punch
    が `kind == "punch_in"` で報告される）。

### Decisions During Implementation

- punch marks の site 識別は `PpqnSite::single(kind, "project")`
  — project-wide singleton で entity id を持たず、range endpoint
  でもない独立 2 tick のため `part` は使わない（design 通り）。
- `preview_ppqn`/`reproject_ppqn` は `reproject_ppqn_with` の
  単一 walk を共有するため、site 追加だけで dry-run と
  mutating command の site 一致が自動維持される — 別経路の
  二重列挙は導入しない。
- automation・plugin instance・SMF document の保存対象外証明は
  専用 test を増やさず、既存 preservation test の fixture を
  拡張して subtree 完全一致で検証する方針を採った（design の
  「保存対象外の安全性」と「異 Division SMF import」を 1 test
  に集約。export cross-golden は `tim_02_ppqn_smf.rs` が既被覆）。
- 分母上限 test は 960→32767 方向（1/960 beat → 32767/960
  ticks）を採用 — 選択上限 PPQN を target として直接行使し、
  Floor 34/Ceil 35 の宣言 rounding を観測可能にするため。

## Related Experiments

### Adopted

- `docs/impl/exp/tim-01-ppqn-config.md` — site 列挙完全性監査。
  `Project` aggregate の persisted field を型で走査し、
  `reproject_ppqn_with` の walk との突合で `monitoring.punch`
  の 2 site 欠落を特定。`BeatPoint` 系・source-native・
  frame domain・非永続 compile 出力が対象外であることも
  同監査で確認した。

### Rejected

<!-- なし。 -->

## Validation

- `mise run fmt-check` — exit 0（`cargo fmt --all -- --check`）。
- `mise run check synvelis-project` — exit 0（`cargo check
  --locked --all-targets -p synvelis-project`）。
- `mise run lint synvelis-project` — exit 0（`cargo clippy
  --locked --all-targets -p synvelis-project -- -D warnings`
  相当; 初回は `tim_01_ppqn.rs` の `doc_markdown` /
  `if-let` lint 2 件で fail → 修正後再実行で green）。
- `mise run test synvelis-project` — exit 0（`cargo test
  --locked -p synvelis-project`、全 target 全件 pass;
  `tim_01_ppqn` 25 tests・`tim_02_ppqn_smf`・他全 suite
  green）。
- `cargo test --locked -p synvelis-project --test tim_01_ppqn`
  （補助証拠; canonical は `mise run test synvelis-project`）
  — 25 tests all pass（新規 5 件 + 拡張 2 件を含む）。

## Risks

- punch marks 以外の未監査 site が残る可能性 — `Project`
  aggregate の persisted field を型（`MusicalTick`/
  `MusicalDuration`/`i64` tick）で再走査済み。`BeatPoint` 系は
  rational beat 正本で PPQN 非依存、`MidiPlanEvent`/
  `MidiPlaybackPlan` は非永続（compile 出力）。
- `site_count` が app 層の表示・test に影響する可能性 —
  `kind` は `&'static str` wire 透過で exhaustive match 無しを
  確認済み。punch 無し fixture の site 数は不変。

## Follow-ups

- GUI PPQN 選択・Preview UI・Workspace/実 OS 受入 — 別 slice
  （NotRun）。
- app 層の transport/automation-write/merge-commit 前提条件 —
  別 Worker 範囲。
- plugin `frame_base` は runtime 状態で project 非保持 —
  project 層では `plugin_instances`/`routing` の不変で安全性を
  検証し、frame_base 自体の検証は player/app 層の受入に委譲。
