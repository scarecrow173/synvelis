---
id: "DESIGN-034YAcuqWR9Ny3LPvqdMWR"
type: "design"
status: "approved"
title: "SV-SCRIPT-03 Luau backend — 設計"
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
    - "docs/specs/sv-script-03-luau-backend.md"
    - "docs/impl/ir/sv-script-03-workflow-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/automation/design-script.md"
    - "docs/impl/ir/sv-script-01-script-api-model.md"
  refines: []
  refined-by:
    - "docs/plans/sv-script-03-luau-backend.md"
  derives-from:
    - "docs/specs/sv-script-03-luau-backend.md"
  derived-by:
    - "docs/plans/sv-script-03-luau-backend.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv-script-03-luau-backend.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-SCRIPT-03 Luau backend — 設計

## Context

第 1 slice（`sv-script-03-workflow-plan`）で確立した canonical
経路 `.luau source → ScriptWorkflowSpec → import document →
WorkflowDefinition → PlanCompiler` のうち、source 評価部分が
未結合だった。本設計は headless slice として、言語中立の
backend port・mlua Luau adapter・resolver/CLI seam を定義する。
製品 runtime 統合・registry/surface/plan compile は対象外
（Option B 境界 — coordinator 承認済み）。

## Scope

### In Scope

- `synvelis-script/src/backend.rs`: backend port mirror。
- `synvelis-script/src/workflow.rs`:
  `ScriptWorkflowSpec::from_script_value`。
- `synvelis-script-luau/`: mlua Luau adapter（新規 crate、
  mlua 依存はここだけ）。
- `synvelis-automation`: `ScriptBuildContext`・
  `evaluate_luau_workflow`・`FileWorkflowResolver` `.luau`
  seam・`AutomationError::ScriptBackend`・`ImportError` 修正。
- `synvelis-cli`: `workflow run --file x.luau` typed 評価
  envelope。

### Out of Scope

- `host.rs`/`Booted`・canonical `ScriptSurfaceSpec`・
  registry/surface 入力（#262/#274 領域）。
- Rhai backend・RF-SCRIPT 正式 contract（ruviform#559 /
  scarecrow173/ruviform#692）。
- Coordinator/Job/製品 E2E 統合（#265/#267）。
- script からの capability/authority 実行。

## Components and Boundaries

- `synvelis-script`（port 層）: `ScriptBackend`/
  `ScriptSession`/`ScriptValue`/`ScriptArtifact`/
  `ExecutionBudget`/`ScriptError`/`SessionError`/
  `ScriptBackendRegistry`。authority 表現を持たない中立値
  モデル。公開 enum は `#[non_exhaustive]`。
- `synvelis-script-luau`（adapter 層）: `LuauBackend`/
  `LuauSession` が port を実装。mlua `Lua` VM を session 毎に
  生成し、`configure_luau` が install する `require`/
  `loadstring`/`print`/`debug` を strip。`ChunkMode::Text`
  で bytecode 拒否。interrupt hook で wall-clock/cancel、
  `set_memory_limit` で memory、`MAX_LIFT_NODES` で lift
  爆発を fail-closed。
- `synvelis-automation`（結合層）: `ScriptBuildContext`
  （AvailableBackends + ScriptApiModel + BindingSet +
  ExecutionBudget）を注入。`evaluate_luau_workflow` が
  session open→evaluate→close し artifact 検査→spec decode→
  `WorkflowScriptBuilder` へ。`FileWorkflowResolver` は
  file byte の SHA-256 を取り評価結果 digest と照合
  （drift は `SourceDrift`）。
- `synvelis-cli`（提示層）: `.luau` file を実 backend で
  評価し typed definition/import を envelope で返す。
  plan compile は unavailable を正直に報告し
  `executed: false`。

## Data and Control Flow

1. resolver が `.luau` file bytes を読み SHA-256 digest を計算。
2. `SessionConfig`（language=Luau・digest・budget・
   `ArtifactKind::WorkflowDefinition`）で session open —
   backend が language/digest/kind/budget を検証。
3. VM が source を text chunk として eval し、戻り値を中立
   `ScriptValue` → `ScriptArtifact`（schema_version・kind・
   payload・provenance）へ。
4. importer が artifact 存在・schema・kind を検査し payload を
   `ScriptWorkflowSpec` へ decode（`from_script_value` が
   builder と同一の構造規則で fail-closed 検証）。
5. `WorkflowScriptDefinition` → `WorkflowScriptBuilder` →
   import document → 既存 importer → `WorkflowDefinition`。
6. CLI は definition/import 結果を envelope で返し、plan
   compile は deferred boundary として報告。

## Non-Functional Constraints

- 宣言的契約: script は spec record のみ返し、authority/
  grant/capability 実行経路を持たない。
- fail-closed: digest drift・budget/cancel・非対応値・
  artifact 契約違反は全て typed error。panic・silent
  fallback・`"undigested"` digest なし。
- 分離: session 毎に独立 VM。Rust 側 lift に node budget
  （`MAX_LIFT_NODES`）。
- 依存隔離: mlua は `synvelis-script-luau` のみ。他 crate は
  port trait のみ消費。
- `AutomationError` の Err サイズ 128B 制約
  （`ScriptBackendError` は Box 化）。

## Risks and Trade-offs

- mlua `configure_luau` の暗黙 global install: strip を
  session 返却前に実施し実 VM test で nil を検証。mlua
  更新時の再検証が必要（RF-SCRIPT contract gate で扱う）。
- interim adapter と RF-SCRIPT 正式 contract の差分:
  ruviform#559/scarecrow173/ruviform#692 で再検証。
- budget は instruction 近似で hard real-time 保証ではない。
- `ExecutionBudget::default()` は無制限 — consumer 責務、
  CLI は 30s/256MiB を設定。

## Deferred Design Concerns

- CLI `.luau` → `ExecutablePlan`: registry/surface 入力不在。
  #262/#274 で `ScriptSurfaceSpec`・`host.rs` accessor 確定後。
- Rhai backend: ruviform#559 gate。
- VM state/continuation・Run/Cancel/Resume: #265 Coordinator
  領域。

## Verification Notes

- [x] 実 VM test 29 件（scalar/record・digest・budget・
  sandbox strip・DAG lift・single-shot・pcall budget）。
- [x] fake backend fail-closed 行列 + 実 VM `.luau` 解決で
  YAML 同値 `WorkflowDefinition`/plan digest 一致。
- [x] CLI E2E: `.luau` typed envelope・分類行列不変。
- [x] fmt/check/lint/test 全 touched crate green
  （`evidence/sv273/sv-script-03-luau-backend-ledger.md`）。
