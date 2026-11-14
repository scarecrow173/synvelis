---
id: "IMPL-034Xzro5OaEpNBG7MrB3s0"
type: "impl"
status: "completed"
title: "AUTX-01-F01 has_executor accessor（synvelis#446 headless slice）"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
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
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/autx-01-f01-has-executor.md"
  implemented-by: []
  depends-on:
    - "docs/specs/autx-01-f01-has-executor.md"
    - "docs/designs/autx-01-f01-has-executor.md"
    - "docs/plans/autx-01-f01-has-executor.md"
    - "docs/test-specs/autx-01-f01-has-executor.md"
    - "docs/specs/command-gateway.md"
    - "docs/impl/ir/autx-01-action-catalog.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/autx-01-action-catalog.md"
    - "docs/impl/exp/autx-01-action-catalog.md"
    - "docs/test-specs/autx-01-action-catalog.md"
    - "docs/automation/design-capability-action.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/autx-01-f01-has-executor.md"
    - "docs/specs/autx-01-f01-has-executor.md"
  derived-by: []
  verifies:
    - "docs/test-specs/autx-01-f01-has-executor.md"
    - "docs/test-specs/autx-01-action-catalog.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/272"
    - "https://github.com/scarecrow173/synvelis/pull/430"
    - "https://github.com/scarecrow173/synvelis/pull/438"
  defers:
    - "automation_ui::ActionRow.executable への executor verdict 反映（#266 view 契約は別 Owner — catalog 側の executor field を正すまでが本 slice）"
    - "GUI/実 OS/実機 lane・CLI surface 本体（#262）・実 ScriptApiModel 照合（#271）"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/autx-01-f01-has-executor.md"
    rejected: []
---

# AUTX-01-F01 has_executor accessor（synvelis#446 headless slice）

## Summary

Issue #446 の headless slice。`synvelis-gateway` の
`CapabilityRegistry` に読み取り専用 accessor
`has_executor(&CapabilityRef) -> bool` を追加し、
`synvelis-workspace` の Action Catalog（#260）が登録済み
capability の executor 有無を `ExecutorState::Installed` /
`Absent` / `Unknown` の 3 verdict で投影できるようにした。
`Registration.handler` は `pub(crate)` のまま公開せず、
`Gateway::invoke` の `NoExecutor` が引き続き唯一の実行時
authority。#260 で lease 衝突により Deferred だった
No-Handler 区別を解禁する最小配線。

## Context

- 正本契約: `docs/specs/autx-01-f01-has-executor.md`、
  `docs/designs/autx-01-f01-has-executor.md`、
  `docs/plans/autx-01-f01-has-executor.md`、
  `docs/tasks/autx-01-f01-has-executor.md`、
  `docs/test-specs/autx-01-f01-has-executor.md`。
- 出自: #260 / PR #430（AUTX-01 Action Catalog —
  `ExecutorState::Unknown` 単一 variant として Deferred 記録）、
  `docs/impl/ir/autx-01-action-catalog.md` Decisions、
  `docs/impl/exp/autx-01-action-catalog.md` defers。
- 前提: sv#263（PR #429）・sv#272（PR #438、
  `Gateway::schemas()` read accessor）統合済みで
  `crates/synvelis-gateway/` は解放済み。
- 実 No-Handler 事例: `PrepareEngine::install` が mutation-op
  descriptor を `None` handler で登録し、直接 `invoke` は
  `GatewayError::NoExecutor` を返す。
- `doc-driven-dev-graph` route は試行したが、リポジトリ全域の
  pre-existing な artifact-graph / broken-relation findings により
  `blocked`（`focus-required`）を返し task graph を生成しなかった
  （本 slice の変更とは無関係の既存損傷）。route 要求の
  impl/ir・impl/exp・test-specs 文書は lease 指定パスに作成した。

## Implementation

### Changes

- `crates/synvelis-gateway/src/registry.rs` —
  `CapabilityRegistry::has_executor(&self, capability:
  &CapabilityRef) -> bool` を追加。`resolve()` の exact
  `(id, version)` 解決を再利用し `handler.is_some()` を返す。
  未登録 id・未登録 version・handler 無し登録は区別なく
  `false`（total 判定）。`&self` のみで登録 map を変更せず、
  executor 本体は公開しない。
- `crates/synvelis-gateway/tests/contract.rs` — 2 test 追加:
  - `has_executor_reports_installed_and_absent_paths`: fixture
    install 済み capability で `true`、未知 id・未登録
    version・`register_capability(desc, None)` で `false`、
    その後 `list()`/`describe()` が不変であること（read-only
    性）を確認。
  - `no_handler_invoke_still_returns_no_executor`: handler 無し
    登録が `has_executor == false` でありつつ `bind` 成功・
    `invoke` が `GatewayError::NoExecutor` を返すこと —
    accessor が実行 authority を変えないことを固定。
- `crates/synvelis-workspace/src/action_catalog.rs` —
  - `ExecutorState` に `Installed`/`Absent` variant を追加
    （serde `kebab-case` で `"installed"`/`"absent"`/
    `"unknown"` — 既存 `"unknown"` payload と後方互換）。
    variant 追加のみで field 構成は不変。
  - `ActionCatalog::project` に `executors:
    Option<&CapabilityRegistry>` を追加（`schemas` と同型の
    optional 引数）。登録 descriptor を持つ entry は
    `registry.has_executor(capability)` で `Installed`/`Absent`、
    `None` 供給は `Unknown`。orphan presentation・
    `describe` 合成 `UnsupportedVersion` は descriptor 無しの
    ため常に `Unknown`。
  - `ActionCatalog::project_gateway` を `&mut Gateway` →
    `&Gateway` に変更し、`Gateway::registry()`/`schemas()`
    （sv#272 着地済み）の read accessor 経由で executor・
    schema 両 lookup を供給。`&mut` caller からの呼び出しは
    reborrow で source 互換。
  - module doc の「executor presence is not observable」記述を
    3 verdict の現行契約に更新。
- `crates/synvelis-workspace/tests/autx01_action_catalog.rs` —
  既存 `project` 呼び出しを新シグネチャに追従（`None` 供給で
  従来挙動を維持）+ 3 test 追加:
  - `executor_state_distinguishes_installed_absent_unknown`:
    実 `CapabilityRegistry` に handler 有/無を混在登録し、
    `Installed`/`Absent`/orphan `Unknown` を確認。
  - `project_without_registry_keeps_executor_unknown`:
    `executors: None` で全 entry `Unknown`（観測不能を
    `Absent` で偽装しない）。
  - `project_gateway_reports_executor_verdicts`: 実
    `Gateway::register_capability(desc, None)` で No-Handler
    capability を登録し、catalog が `Absent`・fixture
    capability が `Installed`・orphan が `Unknown` を報告、
    かつ同 ref の `invoke` が `GatewayError::NoExecutor` を
    返すこと（catalog verdict と runtime truth の一致）。
- `docs/test-specs/autx-01-action-catalog.md` — Rule
  AUTX-CAT-EXECUTOR・Guarantees・Risk を 3 verdict の現行
  契約に改訂。
- `docs/impl/ir/autx-01-action-catalog.md` /
  `docs/impl/exp/autx-01-action-catalog.md` — `ExecutorState`
  3 verdict 化・`project`/`project_gateway` シグネチャ・
  defers 解消を反映（#260 当時の経緯は Decisions/本文に保持）。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,
  impl/exp}/README.md` — doc-maintenance で index 再生成。

### Decisions During Implementation

- **accessor は `has_executor -> bool` の total 判定**。
  `resolve()` を再利用し `handler.is_some()` を返す。
  「id 未登録」「version 未登録」「handler 無し」を区別しない
  のは意図的 — catalog は登録済み ref にのみ呼び、区別が
  必要な consumer は `versions()`/`describe()` の typed error
  を使う（design Risks）。`Option`/`Result` ではなく `bool`
  は「executor が無い」全ケースを同じ答えに畳む discovery
  metadata の semantics に合致。
- **`project` は optional lookup、`project_gateway` は常に供給**。
  `schemas: Option<&SchemaRegistry>` と同型に `executors:
  Option<&CapabilityRegistry>` を足し、registry に到達
  できない caller（descriptor slice のみを持つ投影）は
  `Unknown` を維持する — 観測不能を `Absent` で偽装しない
  fail-closed 設計を schema verdict と揃えた。
- **`project_gateway` の `&mut Gateway` → `&Gateway` 化**。
  sv#272 で `Gateway::schemas()` read accessor が着地済みの
  ため、`schemas_mut()` の mutable borrow が不要になった。
  executor lookup も `registry()`（既存 read accessor）で
  取れるため、projection 全体が immutable `&Gateway` で
  完結する。`&mut` caller は reborrow でそのまま動く
  （source 互換の緩和であり破壊的変更ではない）。
- **`Absent` は display metadata であり authority ではない**。
  `ActionRow.executable`（#266 view 契約）への反映は本 slice
  の範囲外 — catalog entry の `executor` field を正しく埋める
  までとし、view 側で blocked 表示に使うかは別 Owner の
  判断（spec Deferred Design Concerns）。

## Related Experiments

### Adopted

- `docs/impl/exp/autx-01-f01-has-executor.md` — 本 slice の
  設計判断と棄却案の記録。

### Rejected

- なし（exp 文書側に詳細）。

## Validation

- `mise run fmt` / `mise run fmt-check`: Pass
  （`cargo fmt --all -- --check`、exit 0）。
- `mise run check synvelis-gateway`: Pass
  （`cargo check --locked --all-targets -p synvelis-gateway`、
  exit 0）。
- `mise run check synvelis-workspace`: Pass（同上
  `-p synvelis-workspace`、exit 0）。
- `mise run lint synvelis-gateway`: Pass（clippy、exit 0）。
- `mise run lint synvelis-workspace`: Pass（clippy、exit 0）。
- `mise run test synvelis-gateway`: Pass — `contract.rs` の
  `has_executor_reports_installed_and_absent_paths`・
  `no_handler_invoke_still_returns_no_executor` を含む全 test
  0 failed。
- `mise run test synvelis-workspace`: Pass —
  `autx01_action_catalog` の新規 3 test
  （`executor_state_distinguishes_installed_absent_unknown`・
  `project_without_registry_keeps_executor_unknown`・
  `project_gateway_reports_executor_verdicts`）を含む全 suite
  0 failed（既存 test の非破壊 = 受入条件の回帰証明）。
- OS: Windows（worktree ホスト）。toolchain は `mise` 管理。
- 詳細な Case 台帳: `evidence/sv446/README.md`。

## Risks

- `has_executor -> bool` は「id 未登録」「version 未登録」
  「handler 無し登録」を区別しない — catalog は登録済み ref
  にのみ呼ぶため実害なし。区別が必要な consumer は
  `versions()`/`describe()` の typed error を使う。
- `ActionCatalog` は build 時 snapshot — registry への後続
  登録・handler 変更は rebuild まで反映されない（登録は
  append-only/immutable のため descriptor は変化しないが、
  handler の有無も登録時点の snapshot）。
- `ExecutorState::Absent` を view が blocked 表示に使うかは
  未決 — `executable` への組み込みは #266 系の後続判断で、
  誤って authorization failure と解釈されないよう display
  metadata のまま維持した。

## Follow-ups

- `automation_ui::ActionRow.executable`/palette 表示への
  executor verdict 反映 — #266 view 契約の Owner 判断
  （spec Deferred Design Concerns の再検討 trigger 参照）。
- #262 CLI surface が `ActionCatalog` を消費する際の結合検証。
- 実 GPUI/実 OS lane は #114/#267 の別 Gate（NotRun 記録済み）。
