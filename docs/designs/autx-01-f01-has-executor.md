---
id: "DESIGN-034XzqUYtBrYIsgDfTvyEz"
type: "design"
status: "approved"
title: "AUTX-01-F01 has_executor accessor 設計（No-Handler 区別の解禁）"
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
    - "docs/specs/autx-01-f01-has-executor.md"
    - "docs/impl/ir/autx-01-action-catalog.md"
    - "docs/test-specs/autx-01-action-catalog.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/command-gateway-design.md"
    - "docs/automation/design-capability-action.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/autx-01-f01-has-executor.md"
  derived-by:
    - "docs/plans/autx-01-f01-has-executor.md"
  verifies: []
  verified-by:
    - "docs/test-specs/autx-01-f01-has-executor.md"
  references: []
  defers: []
  deferred-by: []
---

# AUTX-01-F01 has_executor accessor 設計（No-Handler 区別の解禁）

## Context

AUTX-01（#260）の `ActionCatalog` は、登録済み capability に
executor があるかを registry public API で観測できず、
`ExecutorState::Unknown` 単一 variant で統合された
（`docs/impl/ir/autx-01-action-catalog.md` Decisions）。
本設計は synvelis#446 の 2 変更を固定する:

1. `CapabilityRegistry`（`crates/synvelis-gateway/src/registry.rs`）
   への `has_executor` 読み取り専用 accessor。
2. `ActionCatalog`（`crates/synvelis-workspace/src/action_catalog.rs`）
   の executor verdict 配線 — `ExecutorState` に
   `Installed`/`Absent` を追加し、descriptor を持つ entry に
   限り報告する。

`Registration.handler` は `pub(crate)` のまま — executor 本体を
公開すると `&CapabilityRegistry` の保持者が policy 評価を
迂回して実行できてしまう（`Registration` doc の既存契約）。
`Gateway::invoke` の `NoExecutor` は引き続き唯一の実行時
authority であり、本設計は discovery 層の観測手段のみを追加する。

## Scope

### In Scope

- `CapabilityRegistry::has_executor(&self, &CapabilityRef) -> bool`
- `ActionCatalog::project` への `Option<&CapabilityRegistry>`
  引数追加、`project_gateway` の `&Gateway` 化と配線
- `ExecutorState::{Installed, Absent, Unknown}` の 3 variant 化
- headless テスト（gateway contract・catalog）

### Out of Scope

- `handler` の公開・`resolve` の `pub` 化
- `automation_ui` view 契約（`ActionRow`/`project_action_catalog`）
  の変更
- `PrepareEngine` の apply path・GW-06 commit seam

## Components and Boundaries

- **`CapabilityRegistry`（synvelis-gateway）** — executor 有無の
  authoritative source。`has_executor` は `resolve`（crate 内
  lookup の唯一経路）を再利用し、`(id, version)` 完全一致の
  registration が `handler.is_some()` かを返す。未登録 id・
  未登録 version・handler 無し登録は区別なく `false`
  （`versions()` が未知 id に空 Vec を返すのと同じ total
  判定の作法）。
- **`ActionCatalog`（synvelis-workspace）** — `project` の
  `executors: Option<&CapabilityRegistry>` が `Some` のとき、
  登録 descriptor を持つ entry の `executor` を
  `has_executor` の結果で `Installed`/`Absent` に投影する。
  `None`（registry 未供給の caller）と descriptor 無し entry
  （orphan presentation・`describe` 合成の
  `UnsupportedVersion`）は `Unknown` を維持する — 「登録が
  無い」への `Absent` 報告は「登録はあるが executor が無い」
  という別の事実を偽装するため。
- **`project_gateway`** — sv#272 で着地した `Gateway::schemas()`
  read accessor により `&mut Gateway` 不要となったため、
  `&Gateway` を取り `registry()`/`schemas()` の 2 つの immutable
  borrow で `project` にそのまま渡す。従来の「registry snapshot
  → `schemas_mut()` sequential borrow」構造は解消される
  （`&mut` → `&` は呼び出し側 source 互換）。

## Data and Control Flow

```text
registry.list() → describe() → &[CapabilityDescriptor]
registry.has_executor(&capability_ref) → bool   (per registered entry)
schemas().resolve(...) → SchemaConsistency       (unchanged)
        │
        ▼
ActionEntry { descriptor, presentation, status,
              schema, executor: Installed|Absent|Unknown }
```

- `has_executor` は `BTreeMap::get` 1 回の読み取り — 登録は
  append-only/immutable のため snapshot 後の再評価は不要。
- catalog は依然として build 時 snapshot。registry への後続
  登録は rebuild まで反映されない（AUTX-01 既存契約どおり）。

## Non-Functional Constraints

- **権限境界**: `has_executor` は metadata のみ返し、executor
  の実行・handle 取得・policy 評価迂回の経路を作らない。
  discovery が権限を mint しない AUTX-01 の原則を維持。
- **後方互換**: `ExecutorState` の serde は `kebab-case` tag —
  `"installed"`/`"absent"` の追加は既存 `"unknown"` payload の
  deserialization を壊さない。`ActionEntry` の wire 形は
  variant 追加のみで field 構成は不変。
- **RT**: いずれの変更も non-RT（UI thread / headless）のみ。

## Risks and Trade-offs

- **`has_executor -> bool` は unknown/absent を区別しない**
  — catalog は登録済み ref に対してのみ呼ぶため実害なし。
  「id 自体が未登録」と「version 未登録」を呼び分けたい consumer
  は `versions()`/`describe()` の typed error を使う（役割分担）。
- **`project` シグネチャ変更は呼び出し側の更新を要する** —
  repo 内 caller は test のみ（`autx01_action_catalog.rs`）。
  optional 引数の追加で `None` を渡せば従来挙動
  （全 `Unknown`）を維持できる。
- **`Absent` の意味は「登録時点で handler 無し」** — 後から
  executor が install されても snapshot は変わらない。登録が
  append-only で handler 差し替え経路が無いため、登録時点の
  判定は恒久的に正しい。

## Deferred Design Concerns

- `automation_ui::ActionRow` への executor verdict 反映 —
  spec の Deferred Design Concerns と同じ。view 契約は #266
  所有。

## Verification Notes

- [x] `CapabilityRegistry::has_executor` の true/false 両経路を
  `contract.rs` で検証（登録+handler → true、登録−handler・
  未登録 id・未登録 version → false、内部状態非変更）。
- [x] catalog の `Installed`/`Absent`/`Unknown` 3 状態と
  `project_gateway` 配線を `autx01_action_catalog.rs` で検証
  （実 `register_capability(desc, None)` 経路で No-Handler を
  再現）。
- [x] 既存 suite の非回帰（`ExecutorState::Unknown` 期待値の
  更新を含む全 test Pass）。
