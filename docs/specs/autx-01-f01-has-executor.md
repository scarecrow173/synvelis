---
id: "SPEC-034XzqwAEPCPIo8wjRqrAP"
type: "spec"
status: "approved"
title: "AUTX-01-F01 CapabilityRegistry::has_executor accessor（No-Handler 区別）"
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
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
    - "docs/impl/ir/autx-01-action-catalog.md"
    - "docs/test-specs/autx-01-action-catalog.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/automation/design-capability-action.md"
    - "docs/automation/design-trust-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/command-gateway.md"
    - "docs/impl/ir/autx-01-action-catalog.md"
  derived-by:
    - "docs/designs/autx-01-f01-has-executor.md"
  verifies: []
  verified-by:
    - "docs/test-specs/autx-01-f01-has-executor.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/272"
  defers: []
  deferred-by: []
---

# AUTX-01-F01 CapabilityRegistry::has_executor accessor（No-Handler 区別）

## What Should Be Built

AUTX-01（synvelis#260, PR #430）で typed `ExecutorState::Unknown` /
Deferred として記録された「登録済み capability に executor が
あるか」の判定を解禁する小 slice（synvelis#446）。

- `synvelis-gateway` の `CapabilityRegistry` に、登録済み
  executor の有無を返す読み取り専用 accessor（`has_executor`
  相当）を追加する。内部状態を変更せず、executor 本体
  （`Registration.handler`）は公開しない。
- `synvelis-workspace` の Action Catalog（#260）が、登録済み
  descriptor を持つ entry について executor 有無を
  `ExecutorState::{Installed, Absent}` として投影し、
  `Unknown`（観測不能）と No-Handler（登録済みだが executor
  未登録）を区別する。
- 消費側は `synvelis-automation` ではなく
  `synvelis-workspace::action_catalog`（#260 が所有する
  read-only projection）。#266 の `automation_ui` view 契約
  （`&[CapabilityDescriptor]` 入力）は変更しない。

## Why Now

- sv#260 は当時 `crates/synvelis-gateway/` が sv#263
  （AUTX-04）の lease 占有中で accessor を追加できず、
  `ExecutorState::Unknown` 単一 variant で統合された
  （`docs/impl/ir/autx-01-action-catalog.md` Decisions、
  `docs/impl/exp/autx-01-action-catalog.md` defers）。
- sv#263（PR #429）・sv#272（PR #438、`Gateway::schemas()`
  read accessor 着地）統合で gateway crate が解放され、
  `has_executor` のみが残件として synvelis#446 に起票された。
- 実 No-Handler 事例が既に存在する: `PrepareEngine::install`
  は mutation-op descriptor を `None` handler で登録し、
  直接 `invoke` は `GatewayError::NoExecutor` を返す
  （`prepare_engine.rs`）。catalog がこの区別を表示できない
  ままだと「Available だが実行経路が prepare 経由のみ」の
  capability が Unknown に埋もれる。

## Users and Value

- Action Catalog / Palette 系の消費者（#266 view、将来の
  CLI surface #262）が、登録済み capability の executor 有無を
  `Unknown` と区別して表示・判断できる。
- `Gateway::invoke` の `NoExecutor` が「唯一の authority」で
  ある構造は維持したまま、read-only discovery 層に観測手段を
  追加する — 認可・実行権限の意味論は変えない。

## Scope

### In Scope

- `CapabilityRegistry::has_executor(&CapabilityRef) -> bool`
  （仮称・命名は既存 API に合わせる）と、その true/false 両
  経路を示す headless テスト。
- `ActionCatalog::project` への executor lookup 供給
  （`schemas: Option<&SchemaRegistry>` と同型の optional
  引数）と `project_gateway` の配線。`ExecutorState` への
  `Installed`/`Absent` variant 追加（exp 文書で予告済みの
  命名）。
- No-Handler 区別を示す headless テスト（実登録経路
  `Gateway::register_capability(desc, None)` を使用）。
- 本 slice の spec→design→plan→task→test-spec→impl 文書鎖と
  `evidence/sv446/`。

### Out of Scope

- `Gateway::invoke` の `NoExecutor` authority の変更、
  executor の公開・呼び出し経路の追加（`handler` は
  `pub(crate)` のまま）。
- `automation_ui::project_action_catalog`/`ActionRow` の
  `executable` 判定への executor 状態の組み込み（view 契約は
  #266 の所有物。本 slice は catalog entry の `executor`
  field を正しく埋めるまで）。
- GUI/実 OS/実機 lane、CLI surface 本体（#262）、実
  ScriptApiModel 照合（#271）。
- `PrepareEngine` の apply path 配線（GW-06 の責務）。

## Requirements

- `has_executor` は `&self` のみを取り、登録 map を変更しない。
  未登録 id・未登録 version・handler 無し登録は区別なく
  `false` を返す（判定不能ではなく「executor 無し」の total
  判定）。
- catalog の `ExecutorState` は、executor lookup が供給された
  場合に限り `Installed`/`Absent` を報告し、未供給
  （`project` に registry を渡せない caller）と descriptor
  無し entry（orphan presentation・合成 describe）では
  `Unknown` を維持する — 観測不能を偽装しない。
- `Gateway::invoke` が返す `NoExecutor` は従来どおり唯一の
  実行時 authority であり、catalog の表示 verdict は実行
  grant を意味しない。
- 既存 API・挙動の回帰を既存テストで証明する
  （`ExecutorState` への variant 追加は serde
  `kebab-case` で後方互換、`project_gateway` の
  `&mut Gateway` → `&Gateway` 化は `Gateway::schemas()`
  （sv#272）着地を利用した source 互換の緩和）。

## Acceptance Criteria

- [ ] `CapabilityRegistry` に登録済み executor の有無を返す
  読み取り専用 accessor（`has_executor` 相当、実際の型・命名は
  既存 API に合わせる）が存在し、内部状態を変更しない
- [ ] accessor の true/false 両経路を示す headless テストを
  追加する
- [ ] #260 の Action Catalog/Presentation が No-Handler
  （executor 未登録の capability）を `Unknown` と区別する
  最小の配線を行い、区別を示すテストを追加する（消費側が
  automation 以外なら Spec で明示 — 本 spec の What Should
  Be Built で `synvelis-workspace::action_catalog` と明示済み）
- [ ] 既存 API・挙動の回帰がないことを既存テストで証明する

## Deferred Design Concerns

- `automation_ui::ActionRow.executable` への executor 状態の
  反映 — view 契約は #266 の所有物であり、本 slice は
  catalog 側の `executor` field を正すのみ。view 側で
  `Absent` を blocked 表示に使うかは #266 系の後続判断。
  | 理由: 責務境界（catalog=投影、view=表示）を跨ぐ変更は
  本 Issue の「最小の配線」を超える | 再検討 trigger:
  palette で No-Handler を blocked 表示する要求が起票された
  とき | 無視時 risk: `Installed`/`Absent` が palette の
  ready/blocked 表示に反映されない（fail-closed 側なので
  誤実行はない）

## Sources and Relations

- Issue #446（本 slice の起票・受入条件）。
- Issue #260 / PR #430（AUTX-01 Action Catalog — `ExecutorState`
  の出自と Deferred 記録）。
- Issue #272 / PR #438（`Gateway::schemas()` read accessor —
  `project_gateway` の `&Gateway` 化を可能にした先行着地）。
- `docs/impl/ir/autx-01-action-catalog.md` /
  `docs/impl/exp/autx-01-action-catalog.md`（`Installed`/
  `Absent` 命名予告と defers の出自）。
- `docs/test-specs/autx-01-action-catalog.md` Rule
  AUTX-CAT-EXECUTOR（本 slice で改訂される規則）。
