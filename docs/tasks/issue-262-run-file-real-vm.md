---
id: "TASK-034YBz8Pv6wNxKd2SgH"
type: "task"
status: "completed"
title: "issue-262-run-file-real-vm — task 分解"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/262"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-262-run-file-real-vm.md"
  implemented-by: []
  depends-on:
    - "docs/plans/issue-262-run-file-real-vm.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-262-run-file-real-vm.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-262-run-file-real-vm.md"
  references: []
  defers: []
  deferred-by: []
---

# issue-262-run-file-real-vm — tasks

| # | task | 状態 |
| --- | --- | --- |
| T1 | `HeadlessSessionHost` 共有 handle seam（Arc 化 + getter、非破壊） | done |
| T2 | automation `surface.rs` canonical `ScriptSurfaceSpec` 生成器 + unit test | done |
| T3 | `evaluate_luau_action`（ActionRecipe sibling helper）+ lib export | done |
| T4 | cli `host.rs` Booted live capture（registry/surface/model/bindings/gateway/cli/commit/admin） | done |
| T5 | args `--inputs`/`--inputs-file` + 相互排他 | done |
| T6 | `script.rs` workflow `.yaml`/`.luau` 実実行（compile→coordinator→envelope） | done |
| T7 | `script.rs` action `.luau` 実実行（recipe→draft→host::handle） | done |
| T8 | envelope 正直写像（Completed/Approval/Failed/非終端/dry-run） | done |
| T9 | `cli_e2e.rs` stale 期待値更新 + 実実行 golden 追加 | done |
| T10 | 検証（fmt/check/lint/test/build/prek）+ evidence ledger + docs chain | done |

## 検証済み事項（Windows headless）

- `workflow run --file` `.yaml`: emit-only → `completed`/exit 0、
  outputs/run_id/plan_digest を返す。
- `workflow run --file` `.luau`: 実 VM 評価→plan→coordinator→
  `completed`/exit 0。
- query step: live lane→Gateway→empty policy `denied`/exit 3。
- `action run --file` `.luau`: 実 VM→recipe→wire→`denied`/exit 3、
  dry-run は `executed:false`+recipe metadata/exit 0。
- `--inputs`/`--inputs-file`/相互排他/宣言 input 不足の fail-closed。
- 分類行列（`.rhai` PoC・`.lua`・未知拡張子・kind mismatch）維持。

## NotRun / Deferred（外部 gate）

- 実 job step 実行・named macro 実解決・approval→apply 継続・
  durable run journal — JobEngine/macro load 経路/durable store
  の所有 gate（AUTX-04 系・Issue 対象外）。
- 製品 E2E・3OS 実機・macOS/Linux build — #267 gate。
- `gw10`/`test-rt`/MSRV/`*-full` lane — Daily Full CI 委譲。
