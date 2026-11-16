---
id: "EXP-034YAdCE2j4WvKqPzXnTb"
type: "experiment"
status: "completed"
title: "SV-SCRIPT-03 Luau backend slice の試行 — mlua adapter・Option B 境界・review cycle"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/273"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/sv-script-03-luau-backend.md"
  depends-on:
    - "docs/tasks/sv-script-03-luau-backend.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/sv-script-03-luau-backend-exp.jsonl"
    - "docs/impl/exp/sv-script-03-workflow-plan.md"
    - "docs/discovery/sv-script-03-luau-backend.md"
    - "evidence/sv273/sv-script-03-luau-backend-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv-script-03-luau-backend.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-script-luau/src/lib.rs"
    - "crates/synvelis-automation/src/importer.rs"
    - "crates/synvelis-cli/src/script.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV-SCRIPT-03 Luau backend slice の試行

Event log: `docs/impl/exp/sv-script-03-luau-backend-exp.jsonl`
（`append_experiment_event` で追記した構造化記録）。

## 試行の概要

第 1 slice（`sv-script-03-workflow-plan`）が残した「実 VM
backend 未結合」の gap を埋めるため、(a) mlua Luau adapter
の実装可能性、(b) `.luau` → `ExecutablePlan` の CLI 結合
境界、(c) interim adapter と RF-SCRIPT 正式 contract の
距離、を試行した。

## 結果

- **mlua adapter**: `mlua = 0.12.2` + `luau` feature で実
  VM 評価が成立。`configure_luau` が無条件 install する
  `require`/`loadstring`/`print`/`debug` は digest pin を
  迂回するため strip が必須と判明（review P1 → 修正済み）。
- **Option B 境界**: `.luau` → `ExecutablePlan` は
  registry/surface/`host.rs` accessor が全てリース外・
  未存在のため不可能。coordinator 承認の Option B
  （typed definition/import まで、plan compile は
  #262/#274 へ defer）で確定。
- **review cycle**: 2 reviewer 並行 review で P1×3
  （sandbox escape・lift DAG 爆発・SourceDrift panic）を
  検出・修正。blocker 0 件で完了。

## 残る不確実性

- interim adapter と RF-SCRIPT 正式 contract の差分は
  ruviform#559 gate（`scarecrow173/ruviform#692`）で再検証。
- macOS/Linux vendored build は未検証（Windows のみ）。
