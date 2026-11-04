---
id: "PLAN-034CTL01Rz2xAu8bD4eF5gHi"
type: "plan"
status: "approved"
title: "CTL-01 ControlOwner/AutomationTimeScope 実装計画"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/121"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/designs/ctl-01-control-owner-time-scope-design.md"
    - "docs/adr/ctl-01-control-owner-time-scope-schema-v7.md"
  implemented-by:
    - "docs/tasks/ctl-01-task1-owner-time-scope-schema-migration.md"
  depends-on:
    - "docs/plans/sf-01-semantic-routing-model.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/ctl-01-control-owner-time-scope-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ctl-01-e2e-control-owner-time-scope.md"
  references: []
  defers: []
  deferred-by: []
---

# CTL-01 実装計画

## Scope Split

単一 scope:`synvelis-project` の control model/schema v7/migration/
ops/oracle + `synvelis-gateway`/`synvelis-workspace` の新 API 追随 +
acceptance test。`synvelis-player`/`recording` は `ControlScope` 非
依存のため原則変更不要(compile に従い追随のみ)。

## File Map

| File | 変更 |
| --- | --- |
| `crates/synvelis-project/src/control.rs` | `ControlScope` 削除・`ControlOwner`/`AutomationTimeScope`/`ParameterRole` 新規・`resolve_scope_pos` 新 arity・`PreparedTarget`・`validate_control_binding`/`validate_control_owner`・legacy wire 受理 Deserialize |
| `crates/synvelis-project/src/project.rs` | `validate_references` の owner/matrix/uniqueness/gain_target owner mapping 検査 |
| `crates/synvelis-project/src/ops.rs` | `AddParameterTarget`/`AddModulationSource` field 変更・write-time 検査・routing entity 削除 cascade 監査 |
| `crates/synvelis-project/src/migration.rs` | `migrate_v6_control` step |
| `crates/synvelis-project/src/store.rs` | `SCHEMA_VERSION = 7` + 履歴 doc |
| `crates/synvelis-project/src/midi.rs`・`lib.rs` | export/参照追随 |
| `crates/synvelis-gateway/src/snapshot.rs` | owner/time_scope JSON・`ResourceRef`/`ResourceKind` Strip/Send/Processor |
| `crates/synvelis-gateway/src/request.rs` | `ResourceRef`/`ResourceKind` 変種・parse/name |
| `crates/synvelis-workspace/src/control.rs` | `record_points`/`target_scope` 新 API |
| `crates/synvelis-project/tests/ctl_01.rs` | 新規 — CTL-01 検証 |
| `crates/synvelis-project/tests/{controller_store,pc_b_rt,routing_sf01}.rs`・`crates/synvelis-workspace/tests/{pc_a,pc_b,pc_c1,sv47_controls}.rs` | `ControlScope` 参照を新 API へ・schema version 断言を 7 へ |

## Task Breakdown(順序)

1. **T1 Types+resolve**: `control.rs` 新型・legacy Deserialize・
   `resolve_scope_pos`・`validate_control_binding`。
2. **T2 Model+oracle+ops**: `ParameterTarget`/`ModulationSource`
   組替・`validate_references`・`ProjectOp`・eval/record 追随・
   削除 cascade 監査。
3. **T3 Migration**: `migrate_v6_control`・`SCHEMA_VERSION=7`。
4. **T4 Downstream**: gateway snapshot/request・workspace control・
   lib export・全既存テスト追随。
5. **T5 Tests+verify**: `ctl_01.rs`・workspace 全体検証・impl record。

## Dependency Graph

T1 → T2 → T3 → T4 → T5(単一 crate 内の型変更が全工程の起点のため直列)。

## Risk Register

- R1: legacy wire 受理と `deny_unknown_fields` の競合 → `ControlOwner`
  は手書き Deserialize で tag 二形態を厳密受理、Serialize は新形式
  のみ・round-trip test。
- R2: v6 snapshot の `time_scope` 欠落が default に倒れ step 導出と
  不整合 → step は owner からの厳密導出で上書き、`validate_references`
  が組合せ違反を拒否。
- R3: routing entity 削除で Strip/Send/Processor 所有 target が
  dangling → `remove_strip_owned`/`RemoveTrack`/`RemoveBus` の
  cascade を監査し、抜けは削除側で cascade。`validate_references`
  の NotFound が oracle として残る。
- R4: `EvalPlan` snapshot 契約退行 → `PreparedTarget` に owner+
  time_scope を freeze、`is_stale` recompile 比較が両 field を
  含むことを test。
- R5: gateway `ResourceRef` 変種追加で `deny_unknown_fields` 相当の
  wire parse が破綻 → parse/name/enumerate を対称に更新し JSON
  fixture test。
- R6: `ProjectAbsolute` を誤って beats として評価 → 解決は明示
  `ScopeUnavailable`、test で断言。

## Rollback

単一 feature 変更のため `git revert` で復帰可能。v7 snapshot は
`SCHEMA_VERSION` を 6 に戻すと `UnsupportedSchema` で読めない
(前方互換なしは仕様)。

## Verification Matrix

| 検証 | 入口 |
| --- | --- |
| fmt | `mise run fmt` + `fmt-check` |
| typecheck | `mise run check` |
| clippy -D warnings | `mise run lint` |
| workspace test | `mise run test` |
| build | `mise run build` |
| RT 退行 | `mise run test-rt` |
| MSRV | `mise run test-msrv` |
| CTL-01 acceptance | `ctl_01.rs` 各 test |
| merge 後再検証 | `git merge origin/main` → `Cargo.lock` 再生成 → 上記再実行 |
