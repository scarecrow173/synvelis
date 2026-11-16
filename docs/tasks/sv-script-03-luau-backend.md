---
id: "TASK-034YAd4cqogfxblwadtcmc"
type: "task"
status: "done"
title: "SV-SCRIPT-03 Luau backend — 実装タスク"
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
    - "docs/plans/sv-script-03-luau-backend.md"
  implemented-by:
    - "docs/impl/ir/sv-script-03-luau-backend.md"
  depends-on:
    - "docs/plans/sv-script-03-luau-backend.md"
    - "docs/designs/sv-script-03-luau-backend.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv-script-03-luau-backend.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-script-03-luau-backend.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-SCRIPT-03 Luau backend — 実装タスク

## Work

Issue #273 第 2 slice: headless pipeline への実 Luau backend
結合。5 commit 構成:

1. `synvelis-script` backend port + spec decode + test
2. `synvelis-script-luau` crate（mlua adapter・実 VM test）
3. `synvelis-automation` `.luau` resolver seam + test
4. `synvelis-cli` `.luau` typed 評価 envelope + E2E
5. docs（IR/ledger/chain）+ review-protocol 修正

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sv-script-03-luau-backend.md`
- `docs/specs/sv-script-03-luau-backend.md`
- `docs/designs/sv-script-03-luau-backend.md`

## Dependencies

- mlua 0.12.2 `luau` feature — coordinator 承認済み
  （`synvelis-script-luau` のみ）。
- Option B 境界 — coordinator 承認済み（plan compile は
  #262/#274 へ defer）。

## Affected Files

- `crates/synvelis-script/src/{backend,workflow,lib}.rs` +
  `tests/backend.rs`
- `crates/synvelis-script-luau/`（新規 crate）
- `crates/synvelis-automation/src/{importer,error,lib}.rs` +
  `tests/{common/mod,importer}.rs`
- `crates/synvelis-cli/src/script.rs` +
  `tests/cli_e2e.rs`
- `Cargo.toml`/`Cargo.lock`
- `docs/`・`evidence/sv273/`（本 chain + ledger）

## Done When

- [x] `.luau` file が実 VM backend で `WorkflowDefinition` を
  生成し YAML 同値と plan digest が一致する。
- [x] fail-closed 行列（digest drift・budget・artifact 契約・
  sandbox・DAG lift）が typed error で reject される。
- [x] CLI `.luau` envelope が `executed: false` で typed
  definition/import を返す。
- [x] 全検証 green（`evidence/sv273/sv-script-03-luau-backend-ledger.md`）。

## Verification

- [x] `mise run fmt-check` / `check` / `lint` / `test` —
  touched 4 crate green
- [x] `mise run check synvelis-gateway` / `synvelis-workspace`
- [x] review-protocol 2 reviewer・blocker 0
