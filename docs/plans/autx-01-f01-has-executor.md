---
id: "PLAN-034XzrbQ8Y2M2fvKYrNXr7"
type: "plan"
status: "approved"
title: "AUTX-01-F01 has_executor accessor 実施計画（synvelis#446）"
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
    - "docs/specs/autx-01-f01-has-executor.md"
  implemented-by: []
  depends-on:
    - "docs/designs/autx-01-f01-has-executor.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/autx-01-f01-has-executor.md"
  derived-by:
    - "docs/tasks/autx-01-f01-has-executor.md"
  verifies: []
  verified-by:
    - "docs/test-specs/autx-01-f01-has-executor.md"
  references: []
  defers: []
  deferred-by: []
---

# AUTX-01-F01 has_executor accessor 実施計画（synvelis#446） Implementation Plan

## Goal

`CapabilityRegistry::has_executor` 読み取り専用 accessor を追加し、
AUTX-01 Action Catalog の `ExecutorState` を `Installed`/`Absent`/
`Unknown` の 3 verdict に拡張して No-Handler を `Unknown` と区別する。

## Scope

単一実装ストリーム — 2 crate（`synvelis-gateway` accessor +
`synvelis-workspace` catalog 配線）と対応テスト・文書鎖。
分割しない（変更は相互依存する 1 つの契約）。

## Upstream Documents

- `docs/specs/autx-01-f01-has-executor.md`（受入条件の正本）
- `docs/designs/autx-01-f01-has-executor.md`（設計判断）
- `docs/test-specs/autx-01-f01-has-executor.md`（検証規則）
- `docs/impl/ir/autx-01-action-catalog.md` /
  `docs/test-specs/autx-01-action-catalog.md`（#260 の出自・
  改訂対象規則 AUTX-CAT-EXECUTOR）

## Design Inputs

- `docs/designs/autx-01-f01-has-executor.md`

## File Map

- `crates/synvelis-gateway/src/registry.rs` — `has_executor`
  accessor 追加（`resolve` 再利用・`&self`・bool 返却）
- `crates/synvelis-gateway/tests/contract.rs` — true/false 両経路・
  未登録 id/version・非変更性の headless test
- `crates/synvelis-workspace/src/action_catalog.rs` —
  `ExecutorState` 3 variant 化、`project` に
  `executors: Option<&CapabilityRegistry>` 追加、
  `project_gateway` を `&Gateway` 化して `registry()`/`schemas()`
  を配線、module/field doc 更新
- `crates/synvelis-workspace/tests/autx01_action_catalog.rs` —
  既存 `project` 呼出の更新 + `Installed`/`Absent`/`Unknown`
  区別 test（実 `register_capability(desc, None)` 経路）
- `docs/test-specs/autx-01-action-catalog.md` — Rule
  AUTX-CAT-EXECUTOR の改訂（Unknown 固定 → 3 verdict）
- `docs/impl/ir/autx-01-action-catalog.md` /
  `docs/impl/exp/autx-01-action-catalog.md` — defers の解消記録
- `evidence/sv446/README.md` — 検証 evidence

## Implementation Sequence

- [x] `CapabilityRegistry::has_executor` 実装 + contract test
- [x] `ExecutorState` variant 追加 + `project`/`project_gateway`
      配線 + catalog test 更新・追加
- [x] `mise run fmt-check` / `check` / `lint` / `test`
      （synvelis-gateway, synvelis-workspace）
- [x] impl ir/exp 文書・autx-01-action-catalog 文書改訂・
      evidence/sv446・docs index 再生成

## Dependencies and Risks

- 依存: sv#272 `Gateway::schemas()`（着地済み）—
  `project_gateway` の `&Gateway` 化の前提。
- リスク: `project` シグネチャ変更の caller は repo 内 test のみ
  （`grep` で確認済み）。`ExecutorState` serde 追加 variant は
  kebab-case で後方互換。
- リスク: `has_executor` が `resolve` の 2 段 error を bool に
  潰す — catalog は登録済み ref にのみ呼ぶため影響なし
  （design Risks 参照）。

## Task Breakdown

- `docs/tasks/autx-01-f01-has-executor.md` — 単一 task
  （2 crate の密結合変更のため分割しない）。

## Verification

- [x] `mise run fmt-check` — cargo fmt check
- [x] `mise run check synvelis-gateway` / `lint` / `test`
- [x] `mise run check synvelis-workspace` / `lint` / `test`
- [x] `autx01_action_catalog` test で `Installed`/`Absent`/
      `Unknown` の区別と `NoExecutor` authority 不変を確認
- [x] 既存 test suite の非回帰（全 test binary Pass）

## Open Questions / Gaps

- なし — Issue #446 の受入条件は確定済み。命名は
  `has_executor`/`Installed`/`Absent` で exp 文書の予告どおり。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（本 worker が直接実装）
- Independent work streams: none
- User approval required before dispatch: yes（worker 契約上は
  review-protocol の blocker なしを承認として扱う）

## Review Handoff

- reviewer focus: `has_executor` が executor を露出しないこと、
  `ExecutorState::Unknown` の残存経路（registry 未供給・
  descriptor 無し entry）が意図どおりか、`project_gateway`
  の `&Gateway` 化が borrow を正しく解消しているか。
