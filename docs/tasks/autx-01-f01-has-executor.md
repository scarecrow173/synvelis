---
id: "TASK-034XzrieoUFNNqBnDVpEVc"
type: "task"
status: "in-progress"
title: "AUTX-01-F01 has_executor accessor 実装タスク（synvelis#446）"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/446"
    - "https://github.com/scarecrow173/synvelis/issues/260"
  changes:
    added:
      - type: "file"
        path: "docs/specs/autx-01-f01-has-executor.md"
      - type: "file"
        path: "docs/designs/autx-01-f01-has-executor.md"
      - type: "file"
        path: "docs/plans/autx-01-f01-has-executor.md"
      - type: "file"
        path: "docs/tasks/autx-01-f01-has-executor.md"
      - type: "file"
        path: "docs/test-specs/autx-01-f01-has-executor.md"
      - type: "file"
        path: "docs/impl/ir/autx-01-f01-has-executor.md"
      - type: "file"
        path: "docs/impl/exp/autx-01-f01-has-executor.md"
      - type: "file"
        path: "evidence/sv446/README.md"
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/registry.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/contract.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/action_catalog.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx01_action_catalog.rs"
      - type: "file"
        path: "docs/test-specs/autx-01-action-catalog.md"
      - type: "file"
        path: "docs/impl/ir/autx-01-action-catalog.md"
      - type: "file"
        path: "docs/impl/exp/autx-01-action-catalog.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/autx-01-f01-has-executor.md"
  implemented-by:
    - "docs/impl/ir/autx-01-f01-has-executor.md"
  depends-on:
    - "docs/plans/autx-01-f01-has-executor.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/autx-01-action-catalog.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/autx-01-f01-has-executor.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/autx-01-f01-has-executor.md"
  references: []
  defers: []
  deferred-by: []
---

# AUTX-01-F01 has_executor accessor 実装タスク（synvelis#446）

## Work

Issue #446 の headless slice。`synvelis-gateway` の
`CapabilityRegistry` に `has_executor` 読み取り専用 accessor
を追加し、`synvelis-workspace` の `ActionCatalog` が登録済み
capability の executor 有無を `ExecutorState::Installed`/
`Absent`/`Unknown` で投影するよう配線する。`Gateway::invoke`
の `NoExecutor` authority・`handler` の `pub(crate)` 境界・
`automation_ui` view 契約は変更しない。

## Classification

`normal-plan-task` — 実装タスク（headless、OS-L1 のみ）。

## Upstream Documents

- `docs/specs/autx-01-f01-has-executor.md`（受入条件）
- `docs/designs/autx-01-f01-has-executor.md`（設計）
- `docs/plans/autx-01-f01-has-executor.md`（file map・sequence）
- `docs/test-specs/autx-01-f01-has-executor.md`（検証規則）
- `docs/impl/ir/autx-01-action-catalog.md`（#260 出自）

## Dependencies

- sv#272（PR #438）`Gateway::schemas()` — `project_gateway`
  の `&Gateway` 化の前提。着地済み。
- sv#263（PR #429）— gateway crate lease 解放の前提。統合済み。

## Affected Files

- `crates/synvelis-gateway/src/registry.rs` — `has_executor` 追加
- `crates/synvelis-gateway/tests/contract.rs` — accessor test
- `crates/synvelis-workspace/src/action_catalog.rs` —
  `ExecutorState` 3 variant・`project`/`project_gateway` 配線
- `crates/synvelis-workspace/tests/autx01_action_catalog.rs` —
  呼出更新 + 区別 test
- `docs/test-specs/autx-01-action-catalog.md` — Rule 改訂
- `docs/impl/{ir,exp}/autx-01-action-catalog.md` — defers 解消
- `docs/impl/{ir,exp}/autx-01-f01-has-executor.md` — 新規
- `evidence/sv446/` — 検証 evidence

## Done When

- [x] `CapabilityRegistry::has_executor` が存在し `&self` のみで
      内部状態を変更しない
- [x] accessor の true/false 両経路の headless test がある
- [x] catalog が No-Handler を `Unknown` と区別し、区別 test が
      ある（消費側は `synvelis-workspace::action_catalog` —
      spec に明示済み）
- [x] 既存 API・挙動の回帰が既存テストで証明される

## Verification

- [x] `mise run fmt-check`
- [x] `mise run check synvelis-gateway` / `lint` / `test`
- [x] `mise run check synvelis-workspace` / `lint` / `test`
- [x] review-protocol によるレビュー（blocker なし）
