---
id: "TSPEC-034VtALo1YCyrOraJu0URr"
type: "test-spec"
status: "approved"
title: "TIM-01 app 層 PPQN — test spec"
created: "2026-11-11"
updated: "2026-11-11"
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
    - "crates/synvelis-app/tests/tim01_ppqn_app.rs"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/tim-01-app-ppqn-design.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-01-app-ppqn.md"
  verified-by: []
  references:
    - "docs/tasks/tim-01-task2-app-ppqn.md"
  defers: []
  deferred-by: []
---

# TIM-01 app 層 PPQN — test spec

## Purpose

Issue #125 app 層 slice の `syn.project.set_ppqn` op と app 層
前提条件 gate が spec の受入基準を満たすことを、
`crates/synvelis-app/tests/tim01_ppqn_app.rs` で固定する。
headless Command 契約（CAS/範囲/変換拒否）自体の網羅は
`docs/test-specs/tim-01-configurable-ppqn.md` /
`crates/synvelis-project/tests/tim_01_ppqn.rs` が担い、本 spec
は app 層経路での維持と前提条件・投影・revision 通知を検証する。
製品 UI・実 OS/実 GUI・製品 Oracle は対象外のため、本 spec が
退役するのは app 層統合が上位 slice に統合された時点。

## Feature

`syn.project.set_ppqn` op が transport 停止・automation record
無効・merge-commit なしの状態でのみ `SetProjectPpqn` を適用し、
`project_info` に `ppqn` を投影し、undo/redo・save/reload・
engine revision 通知が既存機構で回帰しない。

## Rules

- `syn.project.set_ppqn` は `old_ppqn`/`new_ppqn`/`policy` を
  必須 param とし、`policy == "round"` のとき `rounding` を
  必須とする。`expected_revision` は省略時 live revision、
  指定時は controller CAS が stale を拒否する。
- `policy == "exact"` で `rounding` 指定・未知 policy/rounding・
  param 欠落/型不正は `invalid_params`。
- transport playing 中・`record_enabled` 中・open recording
  transaction 中はいずれも `invalid_params` で拒否し、
  project/revision は無変更。
- `old_ppqn` stale・`expected_revision` stale・`new_ppqn` が
  `1..=32767` 外はいずれも拒否され project は無変更
  （`synvelis-project` の authoritative 検証が app 層経路でも
  維持される）。
- 成功時 `project_info().ppqn` が新 PPQN を示し、revision が
  前進し、engine port に revision 通知が届く。
- `syn.project.undo`/`syn.project.redo` で PPQN が完全往復し、
  `syn.project.save`/`syn.project.reload` で PPQN が保持される。

## Examples

- `set_ppqn_applies_and_projects_ppqn`: Given boot project、
  When `syn.project.set_ppqn {old_ppqn:960,new_ppqn:480,
  policy:"exact"}`、Then ok・`project_info.ppqn == 480`・
  revision 前進・engine `last_built_revision` 更新。
- `set_ppqn_rejects_stale_old_ppqn`: Given live ppqn 960、
  When `old_ppqn:100`、Then 拒否・ppqn 不変。
- `set_ppqn_rejects_stale_expected_revision`: Given live
  revision R、When `expected_revision:R-1`（実在する過去
  revision）、Then 拒否・project 無変更。
- `set_ppqn_rejects_out_of_range_ppqn`: `new_ppqn:0`/`40000`
  は拒否。
- `set_ppqn_rejects_while_transport_playing`: Given
  `syn.transport.set {playing:true}`、When set_ppqn、Then
  拒否。`playing:false` に戻すと成功。
- `set_ppqn_rejects_while_record_enabled`: Given
  `control.set_record_enabled(true)`、When set_ppqn、Then 拒否。
- `set_ppqn_rejects_while_recording_transaction_open`: Given
  armed write-mode target + write lane + record_enabled +
  control transport playing + `record_start` 済み、When
  set_ppqn、Then 拒否（merge-commit in flight）。
- `set_ppqn_undo_redo_round_trip`: 適用 → undo → ppqn 960 →
  redo → ppqn 480。
- `set_ppqn_survives_save_reload`: `syn.project.new` →
  set_ppqn 480 → save → reload/open → `project_info.ppqn == 480`。
- `set_ppqn_round_policy_applies_declared_rounding`: inexact
  位置を持つ project で `policy:"round",rounding:"floor"` が
  成功し、`policy:"exact"` は拒否。
- `set_ppqn_requires_explicit_policy`: `policy` 欠落・未知値・
  `round` で `rounding` 欠落は `invalid_params`。

## Guarantees

- app 層経路でも `SetProjectPpqn` の二重 CAS・範囲・atomic
  validation が維持される。
- 前提条件 3 件（transport-stopped・automation-write・
  merge-commit）が独立に検証され、違反時 project は無変更。
- `project_info.ppqn`・undo/redo・save/reload・engine revision
  通知が既存機構で回帰しない。

## Non-goals

- PPQN 選択 UI widget・Preview/Confirm・Presentation Driver
  時刻正本移動。
- `synvelis-project`/`synvelis-workspace`/`synvelis-gateway`
  側の契約変更（headless slice で検証済み）。
- 実 OS/実 GUI・製品 Oracle。

## Risk

- 前提条件の写像ミス（例: `control.playback` と
  `session.transport()` の混同、record_enabled と open
  transaction の順序）は、拒否漏れ＝in-flight 書込みとの
  競合、または過剰拒否＝正当な PPQN 変更の不能として顕在化
  する。各条件を独立 fixture で固定する。
