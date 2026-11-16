---
id: "TSPEC-034YAcuRyLWBV83QUKCqPU"
type: "test-spec"
status: "approved"
title: "SV-SCRIPT-03 Luau backend — テスト仕様"
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
  implemented-by: []
  depends-on:
    - "docs/automation/design-script.md"
    - "docs/impl/ir/sv-script-03-workflow-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv-script-03-workflow-plan.md"
    - "docs/impl/ir/sv-script-03-luau-backend.md"
    - "docs/impl/exp/sv-script-03-luau-backend.md"
    - "evidence/sv273/sv-script-03-luau-backend-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-script-03-luau-backend.md"
  derived-by: []
  verifies:
    - "docs/specs/sv-script-03-luau-backend.md"
    - "docs/designs/sv-script-03-luau-backend.md"
  verified-by:
    - "crates/synvelis-script/tests/backend.rs"
    - "crates/synvelis-script-luau/tests/backend.rs"
    - "crates/synvelis-automation/tests/importer.rs"
    - "crates/synvelis-automation/tests/common/mod.rs"
    - "crates/synvelis-cli/tests/cli_e2e.rs"
  references: []
  defers: []
  deferred-by: []
---

# SV-SCRIPT-03 Luau backend — テスト仕様

## Purpose

headless `.luau` workflow 評価経路の保証を固定する。
backend port の fail-closed 解決・VM の sandbox/budget/
session 分離・importer の digest 再検証・CLI envelope の
正直さを、実 VM を含む test で検証する。interim adapter の
RF-SCRIPT 正式 contract 整合は ruviform#559 gate で再検証
されるため、本 spec は現行 adapter の契約のみを対象とする。

## Feature

LuauBackend による `.luau` workflow source の headless 評価
— declarative spec record を返し、canonical
`WorkflowDefinition` へ lower される。

## Rules

- Rule: `ScriptBackendRegistry` は未知 language/backend・
  未対応 contract version・ambiguous candidate を typed
  error で reject する（fail-closed、silent fallback なし）。
- Rule: `LuauSession` open は language=Luau・source digest
  （SHA-256）・artifact kind・source/budget 上限を検証し、
  `max_memory_bytes: Some(0)` は `UnsupportedBudget` で
  reject（0=unlimited 化を防ぐ）。
- Rule: session 毎に独立 `mlua::Lua` VM を生成し、
  `configure_luau` が install する `require`/`loadstring`/
  `print`/`debug` は session 返却前に strip する —
  digest pin 外の code exec・stdout 汚染・debug library
  露出を構造的に排除。
- Rule: entry chunk は `ChunkMode::Text` — bytecode source
  を受理しない。
- Rule: 戻り値は中立 `ScriptValue` へ decode し、
  function/userdata/thread 等の表現不能値・非 UTF-8 文字列・
  malformed/mixed/sparse table・cycle/depth overflow は
  typed reject。ちょうど 1 値（または 0 値）のみ受理。
- Rule: lift は `MAX_LIFT_NODES`（1M node）で共有 subtable
  DAG の指数爆発を Rust 側で fail-closed に止める。
- Rule: wall-clock/cancel は interrupt hook で safepoint 強制、
  `Instant + Duration` overflow・`Lua` state 生成失敗・
  digest 失敗は panic/`"undigested"` ではなく typed error。
- Rule: `evaluate` は single-shot — 2 回目呼出しは
  ContractViolation、成功経路でも interrupt flag を再検査
  （`pcall` に呑まれた interrupt が success を装えない）。
- Rule: `FileWorkflowResolver` は `.luau` file bytes の
  SHA-256 を取り評価結果 `source_digest` と照合 — drift は
  `ImportError::SourceDrift`。`SourceDrift` Display は
  script 制御の非 ASCII digest でも panic しない。
- Rule: artifact は存在・schema_version・kind
  （`ArtifactKind::WorkflowDefinition`）を検査してから
  `ScriptWorkflowSpec::from_script_value` で decode —
  builder と同一の構造規則（script id・digest・language/
  backend 整合・bindings・duplicate id・node budget）で
  fail-closed。
- Rule: CLI `.luau` envelope は `executed: false` で typed
  definition/import を返し、registry/surface 不在の plan
  compile を unavailable として正直に報告する
  （実行・plan を偽装しない）。
- Rule: `.yaml` 経路・`.rhai`/未知拡張子の分類と
  `UnsupportedSource` reject は不変。

## Examples

- Example: `workflow_spec_roundtrip`（synvelis-script）—
  fake backend が返す spec record が
  `WorkflowDefinition` と同値に decode される。
- Example: 実 VM test 群（synvelis-script-luau 29 件）—
  scalar/record 変換・integer/float 保存・workflow artifact
  生成・digest mismatch・parse/runtime error・session 分離・
  budget/cancel・artifact contract 違反・非対応値・sandbox
  global strip（4 global が nil）・DAG lift 予算・
  single-shot evaluate・`pcall_cannot_swallow_wall_clock_budget`・
  zero/overflow budget typed reject。
- Example: `synvelis-automation/tests/importer.rs` —
  fake backend fail-closed 行列（backend 無し・digest drift・
  eval failure・artifact 欠落・kind/schema 違反・inline 成功・
  `.rhai` unsupported）+ 実 VM `.luau` 解決で YAML 同値
  spec と同一 `WorkflowDefinition`・同一 plan digest、
  Luau/Rhai/YAML 多段 inline chain の compile。
- Example: `workflow_luau_file_evaluates_to_typed_definition`
  （synvelis-cli E2E）— 有効な `.luau` file が実 VM 評価で
  typed envelope を返し `executed: false`。

## Guarantees

- `.luau` source は digest pin された byte 列のみ評価され、
  VM から filesystem/host capability への経路が存在しない。
- 全失敗が typed error であり、panic・silent fallback・
  digest 捏造がない。
- YAML/Luau の同値 spec が同一 `WorkflowDefinition`・
  plan digest を生成する（canonical 経路の単一性）。
- CLI は実行していないものを実行済みと報告しない。

## Non-goals

- `.luau` → `ExecutablePlan` compile・registry/surface
  入力（#262/#274）。
- Rhai backend・RF-SCRIPT 正式 contract 整合
  （ruviform#559 / scarecrow173/ruviform#692）。
- 製品 E2E・Coordinator/Job 統合（#265/#267）。
- macOS/Linux vendored build・gw10/test-rt/MSRV lane
  （merge 後 CI・別 lane で担保）。

## Risk

- mlua 更新で `configure_luau` の install 集合が変わると
  strip 対象が陳腐化する — RF-SCRIPT contract gate で
  再検証。
- budget は instruction 近似で hard real-time 保証ではない
  — consumer ceiling は #262/#274 で検討。
