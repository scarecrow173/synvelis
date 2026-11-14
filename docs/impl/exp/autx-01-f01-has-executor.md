---
id: "EXP-AUTX01F01HasExecutor"
type: "experiment"
status: "completed"
title: "AUTX-01-F01 has_executor accessor 実験ログ（synvelis#446 headless slice）"
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
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/autx-01-f01-has-executor.md"
  implemented-by: []
  depends-on:
    - "docs/automation/design-capability-action.md"
    - "docs/automation/design-trust-boundary.md"
    - "docs/specs/command-gateway.md"
    - "docs/impl/ir/autx-01-action-catalog.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/autx-01-action-catalog.md"
    - "docs/test-specs/autx-01-f01-has-executor.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/autx-01-f01-has-executor.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/272"
    - "https://github.com/scarecrow173/synvelis/pull/430"
    - "https://github.com/scarecrow173/synvelis/pull/438"
  defers:
    - "automation_ui::ActionRow.executable への executor verdict 反映（#266 view 契約は別 Owner の Deferred 判断）"
    - "実 ScriptApiModel（synvelis#271）照合・Script Action（synvelis#272）・CLI surface（synvelis#262）・実 GPUI/OS lane（#114/#267）"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "CapabilityRegistry::has_executor(&CapabilityRef) -> bool — resolve() 再利用の total 判定"
      - "ActionCatalog::project への executors: Option<&CapabilityRegistry> 追加（schemas と同型の optional lookup）"
      - "project_gateway の &mut Gateway → &Gateway 化（Gateway::schemas()/registry() read accessor 経由）"
      - "ExecutorState::{Installed, Absent, Unknown} の 3 verdict — descriptor 無し entry は常に Unknown"
    rejected:
      - "has_executor の Option/Result 戻り値（id 未登録と version 未登録の区別は不要 — bool の total 判定で十分）"
      - "executor handle/列挙 API の公開（Registration.handler は pub(crate) のまま — discovery metadata のみ）"
      - "Absent を ActionRow.executable や authorization failure に結びつける変更（view 契約は #266 の所有物）"
      - "catalog への register/mutate API（第二 Registry 防止は #260 から継続）"
---

# AUTX-01-F01 has_executor accessor experiment log

Issue: https://github.com/scarecrow173/synvelis/issues/446

## 採用した設計判断

### 1. `has_executor` は `resolve()` 再利用の total `bool` 判定

`CapabilityRegistry::has_executor(&self, capability: &CapabilityRef)
-> bool` は crate-private の `resolve()` で exact `(id, version)`
を解決し、`registration.handler.is_some()` を返す。

- 「id 未登録」「version 未登録」「handler 無し登録」を区別なく
  `false` に畳むのは意図的。catalog の用途は「この登録済み ref
  に executor があるか」の一点であり、id/version の欠落理由は
  `describe()`/`versions()` の typed error が既に担う。
  `Option<bool>`/`Result` にすると「観測不能」という第三の状態を
  捏造することになり、`ExecutorState::Unknown` が既に担う役割と
  重複する。
- `&self` のみ・登録 map 非変更・`handler` 非公開を維持。
  `resolve()` は `pub(crate)` のまま — accessor はその唯一の
  公開 read 経路であり、executor 本体への到達経路は増えない。

### 2. `project` は optional lookup、`project_gateway` は常に供給

`ActionCatalog::project` に `executors: Option<&CapabilityRegistry>`
を `schemas: Option<&SchemaRegistry>` と同型に追加した。

- `None` 供給の caller（registry に到達できない投影）は全 entry
  `Unknown` — 「executor が無い」と「この投影では観測できない」
  を `Absent`/`Unknown` で区別する fail-closed 設計は、
  `SchemaConsistency::Unknown` と同じ作法。
- `project_gateway` は sv#272 の `Gateway::schemas()` と既存の
  `Gateway::registry()` を使い、`&Gateway` だけで schema・
  executor 両 lookup を供給する。旧来の `&mut Gateway` +
  `schemas_mut()` sequential borrow は不要になった
  （`&mut` caller は reborrow で source 互換）。

### 3. `ExecutorState` は variant 追加のみ — wire 形は不変

`Installed`/`Absent` を `Unknown` と並ぶ variant として追加した
（#260 exp で「導入時に追加しても wire 形が壊れない」と予告
済みの命名をそのまま採用）。serde `kebab-case` により
`"installed"`/`"absent"`/`"unknown"` — 既存 `"unknown"`
payload との後方互換を保つ。

- descriptor 無し entry（orphan presentation・`describe` 合成
  `UnsupportedVersion`）は registry に登録が無いため `has_executor`
  を呼べず、常に `Unknown` — 観測不能を `Absent` で偽装しない。
- `Absent` は「登録済みだが handler 無し」の display metadata。
  `Gateway::invoke` の `NoExecutor` が引き続き唯一の実行時
  authority であり、policy/bind 経路は一切変更していない
  （test で `Absent` ref の invoke が `NoExecutor` を返すことを
  固定）。

## 試して棄却したもの

- **`has_executor` の `Option`/`Result` 戻り値** — 「未登録」と
  「handler 無し」の区別は catalog の用途に不要で、区別を返すと
  `Unknown` verdict と責務が重複する。`versions()`/`describe()`
  の typed error が既にその区別を担う。
- **executor handle・列挙 API の公開** — `Registration.handler`
  は `pub(crate)` のまま。discovery metadata（有無の bool）のみを
  追加し、executor への到達経路は `bind`/`invoke` のみ。
- **`ActionRow.executable` への executor verdict 組み込み** —
  #266 view 契約は別 Owner。catalog entry の `executor` field を
  正しく埋めるまでが本 slice で、view 側の blocked 表示判断は
  Deferred（spec Deferred Design Concerns）。
- **catalog への register/mutate API** — 第二 Registry 化を型で
  防ぐ #260 の判断を継続。

## 検証証跡

- `mise run test synvelis-gateway`: Pass — `contract.rs` に
  `has_executor_reports_installed_and_absent_paths`・
  `no_handler_invoke_still_returns_no_executor` を追加、全 test
  0 failed。
- `mise run test synvelis-workspace`: Pass —
  `autx01_action_catalog` に
  `executor_state_distinguishes_installed_absent_unknown`・
  `project_without_registry_keeps_executor_unknown`・
  `project_gateway_reports_executor_verdicts` を追加、既存
  suite 全 Pass（回帰なし）。
- `mise run check/lint synvelis-gateway`・
  `mise run check/lint synvelis-workspace`・
  `mise run fmt-check`: 全て Pass（exit 0、OS: Windows、
  toolchain は mise 管理）。
- 実 GPUI/実 OS/live adapter/CLI surface/実 ScriptApiModel 照合は
  NotRun/Deferred（`evidence/sv446/README.md` の Case ID 表参照）。
