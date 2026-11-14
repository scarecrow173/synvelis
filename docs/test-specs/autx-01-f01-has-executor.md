---
id: "TSPEC-034Xzr6fjlyezcJ8IdtIfp"
type: "test-spec"
status: "approved"
title: "AUTX-01-F01 has_executor headless test spec（synvelis#446）"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/test-specs/autx-01-action-catalog.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-01-action-catalog.md"
    - "docs/impl/ir/autx-01-action-catalog.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/autx-01-f01-has-executor.md"
  derived-by: []
  verifies:
    - "docs/specs/autx-01-f01-has-executor.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/272"
  defers: []
  deferred-by: []
---

# AUTX-01-F01 has_executor headless test spec（synvelis#446）

## Purpose

`crates/synvelis-gateway/tests/contract.rs` と
`crates/synvelis-workspace/tests/autx01_action_catalog.rs` が
固定する AUTX-01-F01 headless 契約を記録する。#260 で
`ExecutorState::Unknown` / Deferred として記録された No-Handler
区別の解禁を、実装方法ではなく観測可能な verdict で固定し、
将来の consumer（CLI surface #262・view 側反映）が同じ区別を
前提にできるようにする。

## Feature

AUTX-01-F01 — `CapabilityRegistry::has_executor` 読み取り専用
accessor と Action Catalog の `ExecutorState` 3 verdict 投影

## Rules

- **Rule AUTX-F01-ACCESSOR** `has_executor(&CapabilityRef)` は
  `&self` のみを取り登録 map を変更しない。完全一致
  `(id, version)` の registration が `handler.is_some()` の
  とき `true`、handler 無し登録・未登録 id・未登録 version は
  区別なく `false` を返す（total 判定 — `versions()` の未知 id
  に対する空 Vec と同じ作法）。executor 本体は公開されず、
  `Gateway::invoke` の `NoExecutor` が引き続き唯一の実行時
  authority（`has_executor_reports_installed_and_absent_paths`）。
- **Rule AUTX-F01-EXECUTOR-STATE** `ActionCatalog::project` に
  `executors: Some(&CapabilityRegistry)` を供給した場合、
  登録 descriptor を持つ entry の `executor` は
  `Installed`（handler 有）/ `Absent`（handler 無し）を報告
  する。`executors: None` と descriptor 無し entry
  （orphan presentation・`describe` 合成 `UnsupportedVersion`）
  は `Unknown` を維持し、観測不能を `Absent` で偽装しない
  （`executor_state_distinguishes_installed_absent_unknown`、
  `project_without_registry_keeps_executor_unknown`）。
- **Rule AUTX-F01-GATEWAY-WIRE** `project_gateway` は
  `Gateway::registry()`/`schemas()` の read accessor 経由で
  executor verdict を配線し、`&Gateway` で呼べる
  （sv#272 の `Gateway::schemas()` 前提）。実登録経路
  `Gateway::register_capability(desc, None)` で登録した
  No-Handler capability は `Absent`、handler 付き登録は
  `Installed` として投影される
  （`project_gateway_reports_executor_verdicts`）。
- **Rule AUTX-F01-NOAUTH** executor verdict は discovery
  metadata であり実行 grant ではない。`Absent` の capability
  でも bind/policy/invoke の経路は不変で、handler の無い
  capability の `invoke` は従来どおり `GatewayError::NoExecutor`
  を返す（`no_handler_invoke_still_returns_no_executor`）。
- **Rule AUTX-F01-REGRESSION** `ExecutorState` への variant
  追加は serde `kebab-case` で既存 `"unknown"` payload と
  後方互換。`ActionEntry` の field 構成・`ActionStatus`・
  `SchemaConsistency`・catalog の join/重複拒否/list/describe
  契約は不変（既存 `autx01_action_catalog` suite 全 Pass）。

## Examples

- **Example AUTX-F01-ACCESSOR-01** fixture install 済み
  Gateway で `registry.has_executor(&cap("synvelis.midi.add-note",
  1)) == true`、`register_capability(desc, None)` した
  capability・`cap("synvelis.unknown.id", 0)`・登録 version
  と異なる minor はすべて `false`。
- **Example AUTX-F01-EXECUTOR-STATE-01** `project` に
  `Some(&registry)` を渡すと handler 付き entry は
  `executor: "installed"`、`None` handler 登録 entry は
  `"absent"`。同じ descriptor 集合を `executors: None` で
  project すると全 entry が `"unknown"`。
- **Example AUTX-F01-GATEWAY-WIRE-01** `project_gateway` で
  投影した catalog は、fixture install の全 capability が
  `Installed`、`register_capability(desc, None)` で追加した
  capability が `Absent`、orphan presentation entry が
  `Unknown` を報告する。

## Guarantees

- `has_executor` は読み取り専用で、登録・policy・invoke の
  いずれの authority も変更・mint しない。
- catalog の `executor` verdict は「登録時点の handler 有無」
  の snapshot であり、`Gateway::invoke` の実行時判定と矛盾
  しない（`Absent` ⇒ invoke は `NoExecutor`、`Installed` ⇒
  handler 到達の可能性あり — policy は別途評価）。
- `Unknown` は「この投影では観測不能」の意味に限定され、
  registry 未供給・descriptor 無し entry でのみ出る。

## Non-goals

- `automation_ui::ActionRow`/`project_action_catalog` への
  executor verdict 反映（#266 view 契約は別 Owner の
  Deferred 判断）。
- executor の列挙・handle 取得・呼び出し API。
- GUI/実 OS/実機 lane、CLI surface 本体（#262）、実
  ScriptApiModel 照合（#271）。
- `PrepareEngine` の apply path 配線（GW-06）。

## Risk

- `has_executor -> bool` は「id 未登録」と「version 未登録」
  を区別しない — catalog は登録済み ref にのみ呼ぶため実害
  なし。区別が必要な consumer は `versions()`/`describe()` の
  typed error を使う（design Risks 参照）。
- `project` のシグネチャ変更は caller 更新を要する — repo 内
  caller は test のみで、`None` 供給で従来挙動を維持できる。
