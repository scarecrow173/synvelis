---
id: "PLAN-034SF01Np0sTu5vW2xYzAbDeF"
type: "plan"
status: "approved"
title: "SF-01 Semantic Routing Model 実装計画"
created: "2026-11-04"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/104"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/designs/sf-01-semantic-routing-model-design.md"
  implemented-by:
    - "docs/tasks/sf-01-task1-routing-model-schema-migration.md"
  depends-on:
    - "docs/plans/pc-b-parameter-dispatch.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-01-semantic-routing-model-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-01-e2e-semantic-routing-model.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-01 実装計画

## Scope Split

単一 scope:`synvelis-project` の model/schema/migration/ops/oracle と
acceptance test。他 crate は `SCHEMA_VERSION` 参照のみで API 変更不要。

## File Map

| File | 変更 |
| --- | --- |
| `crates/synvelis-project/src/id.rs` | 新規 7 Stable ID 型 |
| `crates/synvelis-project/src/routing.rs` | 新規 — RoutingModel/entity/Binding/oracle |
| `crates/synvelis-project/src/migration.rs` | 新規 — ordered step registry + `migrate_v5_routing` |
| `crates/synvelis-project/src/project.rs` | `routing` field・oracle 呼出・watermark fold |
| `crates/synvelis-project/src/store.rs` | `SCHEMA_VERSION = 6` |
| `crates/synvelis-project/src/controller.rs` | `load` の versioned dispatch・migration +1 rev |
| `crates/synvelis-project/src/ops.rs` | 12 routing op variants + validate/apply |
| `crates/synvelis-project/src/lib.rs` | exports |
| `crates/synvelis-project/tests/routing_sf01.rs` | 新規 — SF-MODEL 検証 |
| `crates/synvelis-project/tests/md_a.rs`, `midi_source.rs` | schema version 断言を 6 へ |

## Task Breakdown(順序)

1. **T1 Model+IDs**: `id.rs` 7 型 + `routing.rs` 型定義・serde・
   `RoutingModel::default` bootstrap・`WELL_KNOWN_BASE`。
2. **T2 Oracle+Project**: `RoutingModel::validate`・`Project.routing`
   組込・watermark 拡張。
3. **T3 Migration**: `migration.rs` registry・v5→v6・controller load・
   store version bump。
4. **T4 Ops**: `ProjectOp` 12 variants + `RemoveTrack`/`RemoveParameterTarget`
   拡張。
5. **T5 Tests+verify**: `routing_sf01.rs`・schema 断言修正・`mise run`
   検証・impl record。

## Dependency Graph

T1 → T2 → T3 → T4 → T5(T2 oracle が T3/T4 の最終検証を兼ねるため直列)。

## Risk Register

- R1: reserved ID 方式の watermark fold 解釈違反 → ADR D2 で invariant
  を明文化、test で `id_watermark >= BASE` 拒否を検証。
- R2: v5 snapshot 上書き → migration load で +1 rev、`save` が旧名を
  publish しないことを test 検証。
- R3: op 側と oracle の検査重複 → op validate は存在性のみ、意味
  合法性は oracle 一本化(ADR D4/D5)。
- R4: serde 形の破壊 → deny_unknown_fields + tagged ID 文字列 +
  round-trip test。
- R5: `Composite` 削除経路の退行 → composite `[RemoveConnection,
  RemoveTrack]` 受理 test を追加。

## Rollback

単一 feature 変更のため `git revert` で完全復帰可能。`SCHEMA_VERSION`
を 5 に戻せば v6 snapshot は UnsupportedSchema で読めない(前方互換
なしは仕様)。

## Verification Matrix

| 検証 | 入口 |
| --- | --- |
| fmt | `mise run fmt` + `fmt-check` |
| typecheck | `mise run check` |
| clippy -D warnings | `mise run lint` |
| workspace test | `mise run test` |
| build | `mise run build` |
| SF-MODEL acceptance | `routing_sf01.rs` 各 test |
| merge 後再検証 | `git merge origin/main` → `Cargo.lock` 再生成 → 上記再実行 |
