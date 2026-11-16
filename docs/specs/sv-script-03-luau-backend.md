---
id: "SPEC-034YAcoXkl0en7soFz7neT"
type: "spec"
status: "implemented"
title: "SV-SCRIPT-03 Luau backend — headless .luau workflow 評価 slice"
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
    - "docs/impl/ir/sv-script-03-workflow-plan.md"
    - "docs/impl/ir/sv-script-01-script-api-model.md"
    - "docs/automation/design-script.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv-script-03-workflow-plan.md"
    - "docs/specs/command-gateway.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv-script-03-luau-backend.md"
  derived-by:
    - "docs/designs/sv-script-03-luau-backend.md"
    - "docs/plans/sv-script-03-luau-backend.md"
    - "docs/test-specs/sv-script-03-luau-backend.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv-script-03-luau-backend.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-SCRIPT-03 Luau backend — headless .luau workflow 評価 slice

## What Should Be Built

Issue #273 SV-SCRIPT-03 の第 2 slice。headless pipeline に
実 Luau backend を結合し、`.luau` workflow source が
`ScriptWorkflowSpec` → import document → `WorkflowDefinition`
の canonical 経路で YAML と同値の結果を生成できること。
CLI `workflow run --file x.luau` は実 VM 評価により typed
definition/import 結果を返す（Option B: plan compile は
#262/#274 へ defer）。

## Why Now

第 1 slice は契約層（builder/importer）のみで実 VM を持たず、
`.luau` file は `UnsupportedSource` だった。SV-SCRIPT-03 の
受入には実 backend 経路の証拠が必要であり、ruviform#559
gate を待たずに interim adapter で先行検証する価値がある。

## Users and Value

- Synvelis 開発者/CI: `.luau` workflow file が YAML と同一の
  `WorkflowDefinition`・plan digest を生成することを
  headless で検証できる。
- SV-SCRIPT consumer（#271–#274）: 言語中立 backend port の
  実契約が確定し、Rhai 等の将来 backend 追加の足場になる。

## Scope

### In Scope

- `synvelis-script`: 言語中立 backend port module
  （`ScriptSource`/`ExecutionBudget`/`ScriptValue`/
  `ScriptArtifact`/`ScriptResult`/`ScriptError`/
  `SessionError`/`ScriptSession`/`ScriptBackend`/
  `ScriptBackendRegistry`）と
  `ScriptWorkflowSpec::from_script_value` decode。
- `synvelis-script-luau`（新規 crate）: mlua Luau adapter —
  session 毎独立 VM・source digest 検証・budget/cancel・
  sandbox global strip・typed error 写像。
- `synvelis-automation`: `FileWorkflowResolver` の `.luau`
  解決 seam（注入 `Arc<dyn ScriptBackend>` +
  `ScriptBuildContext`）、`evaluate_luau_workflow`、
  `AutomationError::ScriptBackend`。
- `synvelis-cli`: `workflow run --file x.luau` の実 VM 評価
  → typed envelope（`executed: false`、plan compile は
  unavailable を正直に報告）。

### Out of Scope

- `.luau` → `ExecutablePlan` の CLI 結合・canonical
  `ScriptSurfaceSpec`・`host.rs`/`Booted` accessor
  （synvelis#262/#274 領域）。
- Rhai 実 backend・RF-SCRIPT 正式 contract 整合
  （ruviform#559 / scarecrow173/ruviform#692）。
- Coordinator/Plan/Job 統合・製品 E2E（#265/#267）。
- script からの capability/authority 呼出し（declarative
  spec record のみ返す契約は不変）。

## Requirements

- backend port は `synvelis-script` に置き、mlua/Luau 固有型を
  他 crate に露出しない（coordinator 承認条件）。
- `.luau` 評価は source digest（SHA-256）を session open で
  検証し、importer 側でも file byte digest を再検証する。
- 全ての失敗は typed error で fail-closed（silent fallback
  ・panic・`"undigested"` fallback なし）。
- VM session は source 毎に独立し、global/state 漏洩がない。
- VM は authority object・capability 呼出し API を持たない。
- `.yaml` 経路・`.rhai` の `UnsupportedSource` 分類は不変。

## Acceptance Criteria

- [x] `.luau` file が backend 注入時に `WorkflowDefinition`
  を生成し、YAML 同値 spec と同一 plan digest になる。
- [x] backend 未注入・digest drift・eval failure・artifact
  契約違反が全て typed error で reject される。
- [x] Luau VM が `require`/`loadstring`/`print`/`debug` を
  持たず、digest pin 外の code exec が不可能。
- [x] budget/cancel 超過・非対応戻り値・DAG 爆発が typed
  error で reject される。
- [x] CLI `.luau` envelope が `executed: false` で typed
  definition/import を返し、plan compile を偽装しない。
- [x] 全検証（fmt/check/lint/test）green —
  `evidence/sv273/sv-script-03-luau-backend-ledger.md`。

## Deferred Design Concerns

- CLI `.luau` → `ExecutablePlan`: registry/surface 入力不在。
  synvelis#262/#274 で canonical `ScriptSurfaceSpec` と
  `host.rs` accessor が確定次第結合。
- RF-SCRIPT 正式 backend contract との整合: interim adapter
  の契約差分は ruviform#559 gate（scarecrow173/ruviform#692）
  で再検証。
- `ExecutionBudget::default()` の無制限: consumer ceiling
  要否は #262/#274 配線時に検討。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/273>
- Discovery: `docs/discovery/sv-script-03-luau-backend.md`
- Design: `docs/designs/sv-script-03-luau-backend.md`
- Test spec: `docs/test-specs/sv-script-03-luau-backend.md`
- Evidence: `evidence/sv273/sv-script-03-luau-backend-ledger.md`
