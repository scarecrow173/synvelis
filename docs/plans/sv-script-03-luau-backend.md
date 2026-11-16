---
id: "PLAN-034YAd3CEsjvFfuoNVjFfn"
type: "plan"
status: "completed"
title: "SV-SCRIPT-03 Luau backend — 実装計画"
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
  implements:
    - "docs/specs/sv-script-03-luau-backend.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sv-script-03-luau-backend.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/sv-script-03-workflow-plan.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-script-03-luau-backend.md"
    - "docs/designs/sv-script-03-luau-backend.md"
  derived-by:
    - "docs/tasks/sv-script-03-luau-backend.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv-script-03-luau-backend.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-SCRIPT-03 Luau backend — 実装計画 Implementation Plan

## Goal

headless pipeline に実 Luau backend を結合し、`.luau`
workflow source を `ScriptWorkflowSpec` → `WorkflowDefinition`
の canonical 経路で YAML と同値に評価する（plan compile は
#262/#274 へ defer — Option B）。

## Scope

単一実装 stream。port（`synvelis-script`）→ adapter
（`synvelis-script-luau`）→ 結合（`synvelis-automation`）→
提示（`synvelis-cli`）の依存順で、各層が前層の公開契約のみを
消費する。

## Upstream Documents

- `docs/specs/sv-script-03-luau-backend.md`
- `docs/discovery/sv-script-03-luau-backend.md`

## Design Inputs

- `docs/designs/sv-script-03-luau-backend.md`（approved）

## File Map

- `crates/synvelis-script/src/backend.rs` - 言語中立 backend
  port（ScriptSource/ExecutionBudget/ScriptValue/
  ScriptArtifact/ScriptResult/ScriptError/SessionError/
  ScriptSession/ScriptBackend/ScriptBackendRegistry）
- `crates/synvelis-script/src/workflow.rs` -
  `ScriptWorkflowSpec::from_script_value` decode + 構造検証
- `crates/synvelis-script/src/lib.rs` - backend module export
- `crates/synvelis-script/tests/backend.rs` - port 契約 test
- `crates/synvelis-script-luau/Cargo.toml` - mlua 0.12.2
  luau feature（本 crate のみ）
- `crates/synvelis-script-luau/src/lib.rs` - LuauBackend/
  LuauSession adapter
- `crates/synvelis-script-luau/tests/backend.rs` - 実 VM test
- `crates/synvelis-automation/src/importer.rs` -
  ScriptBuildContext・evaluate_luau_workflow・resolver `.luau`
  seam
- `crates/synvelis-automation/src/error.rs` -
  AutomationError::ScriptBackend(boxed)・SourceDrift Display 修正
- `crates/synvelis-automation/src/lib.rs` - 公開 export
- `crates/synvelis-automation/tests/common/mod.rs` -
  FakeScriptBackend fixture
- `crates/synvelis-automation/tests/importer.rs` - fail-closed
  行列 + 実 VM 解決 test
- `crates/synvelis-cli/src/script.rs` - `.luau` typed 評価
  envelope
- `crates/synvelis-cli/tests/cli_e2e.rs` - CLI E2E
- `Cargo.toml`/`Cargo.lock` - workspace member + mlua 依存木

## Implementation Sequence

- [x] `synvelis-script` backend port + spec decode + test
- [x] `synvelis-script-luau` crate（mlua adapter・実 VM test）
- [x] `synvelis-automation` `.luau` resolver seam + fake/実 VM test
- [x] `synvelis-cli` `.luau` typed 評価 envelope + E2E
- [x] docs（IR/ledger/本 chain）+ review-protocol 修正

## Dependencies and Risks

- mlua 依存追加は coordinator 承認済み（`synvelis-script-luau`
  のみ・`mlua = 0.12.2` + `luau` feature）。
- `AutomationError` Err サイズ 128B 制約 → ScriptBackendError
  は Box 化。
- mlua `configure_luau` の暗黙 global install → strip 必須
  （review で検出・修正済み）。
- registry/surface/plan compile はリース外（#262/#274）。

## Task Breakdown

- `docs/tasks/sv-script-03-luau-backend.md` — 5 commit 構成の
  単一 task（port → adapter → automation → cli → docs）。

## Verification

- [x] `mise run fmt-check` / `check` / `lint` / `test` —
  touched 4 crate 全て green
- [x] `mise run check synvelis-gateway` /
  `synvelis-workspace` — dependent 非回帰
- [x] `.luau` → `WorkflowDefinition`/plan digest が YAML 同値
  と一致（実 VM）
- [x] review-protocol 2 reviewer・blocker 0・P1×3 修正済み
- [x] `evidence/sv273/sv-script-03-luau-backend-ledger.md`

## Open Questions / Gaps

- RF-SCRIPT 正式 contract との整合（ruviform#559 /
  scarecrow173/ruviform#692）— interim adapter の再検証待ち。
- macOS/Linux mlua vendored build — merge 後 CI 証拠。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（完了済み）
- Independent work streams: none
- User approval required before dispatch: yes（完了のため不要）

## Review Handoff

- reviewer focus: mlua sandbox 境界・budget/lift 防御・
  Option B 境界の正直さ・lease 遵守（retro-approve 4 件は
  PR 本文に明記）。
