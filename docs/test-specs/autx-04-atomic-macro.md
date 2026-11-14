---
id: "TSPEC-autx04atomicmacro001"
type: "test-spec"
status: "approved"
title: "AUTX-04 Atomic Macro v1 — versioned recipe・exact capability・binding placeholder invariants"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/263"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/automation/design-atomic-macro.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/autx-04-atomic-macro.md"
    - "docs/impl/exp/autx-04-atomic-macro.md"
    - "evidence/sv263/autx-04-atomic-macro-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/designs/execution-plan-design.md"
    - "docs/specs/command-gateway.md"
  verified-by: []
  references:
    - "evidence/sv263/autx-04-atomic-macro-ledger.md"
    - "docs/impl/ir/autx-04-atomic-macro.md"
  defers: []
  deferred-by: []
---

# AUTX-04 Atomic Macro v1 Test Spec — versioned recipe・exact capability・binding placeholder invariants

## Purpose

Issue #263（AUTX-04）の headless slice が保証すべき不変条件を
固定する。対象は `synvelis-automation` の
`AtomicMacroDefinition`/`MacroCompiler`/`MacroRegistry`/
`Step::AtomicMacro` plan 展開と、`synvelis-gateway` の batch
`bindings` typed reject であり、create-then-reference の
end-to-end commit（scratch rewrite → apply）は
`Composite::validate` 逐次化 seam 依存の Deferred である。
実行・認可・GUI 結合は対象外（#265/#266/#267）。

## Feature

Versioned Atomic Macro Recipe（`{schema_version, macro_id,
version, parameters, steps, output_bindings, policy}`）を
runtime input + Registry Snapshot と共に compile し、
`synvelis.operation.batch` の `ops[]`（+ `bindings`）へ
決定的に lowering する feature。macro 定義・plan は
authority を一切保持しない。

## Rules

- **AUTX4-TS-R01 definition integrity**: `schema_version`・
  `macro_id`・`version`・`parameters`/`steps`/
  `output_bindings`/`policy` の shape は decode 時に検証
  され、unknown field・duplicate・非正値は reject。
  definition に principal/grant/approval/secret/credential
  を表現する field は存在してはならない。
- **AUTX4-TS-R02 exact resolution**: `MacroRegistry` は
  `(macro_id, version)` 完全一致のみで解決し、latest 解決は
  存在しない。registry 不在・ID 不在・version 非対応は
  区別された error（`UndefinedMacro`/`UnknownMacro`/
  `UnsupportedMacroVersion`）で reject。
- **AUTX4-TS-R03 static fail-closed check**: member capability
  は Registry Snapshot 上で exact version・available・
  `ProjectMutation` domain・非 async execution・明示
  target・引数 schema 適合でなければ reject。nested batch
  capability・implicit target・未対応 workflow-only 式は
  reject。member 総数 >64 は policy ceiling で reject。
- **AUTX4-TS-R04 parameter binding**: runtime input は宣言
  parameter schema と照合され、missing/unexpected/schema
  違反は reject。literal/param 値が予約 ID レンジに入る
  場合は `ReservedIdLiteral` で reject — placeholder との
  衝突を型で排除する。
- **AUTX4-TS-R05 deterministic compile**: 同一
  (definition, inputs, registry, pinned revision) は常に
  同一 `ops[]`・`bindings[]`・`definition_digest` を生成
  し、object key 順は digest に影響しない。
- **AUTX4-TS-R06 no guessed IDs**: `$output.<name>` は
  `OUTPUT_BINDING_ID_BASE` 予約レンジの決定的 placeholder
  （`kind-<32hex>` 形式）を emit し、実 project ID を推測・
  ハードコードしてはならない。binding は producing member
  index を指す named `OutputBinding` でなければならない。
- **AUTX4-TS-R07 batch wire contract**: `CompiledMacro::
  batch_args()` は `ops` を常に array で返し、`bindings` は
  非空時のみ含む。`bindings` を持つ batch request は GW-04
  で `PreparationRejected`（typed、Workflow 分解を推奨）
  となり、`bindings` 欠落・空配列の batch は従来通り
  member lowering されなければならない。
- **AUTX4-TS-R08 workflow fail-closed**: workflow definition
  内の macro-only 式（`$param`/`$output`）は
  `MacroOnlyExpr` で reject され、macro step は registry
  解決・call-site 引数 schema 照合・member cost/effect/
  data-access 会計を経て plan bounds に反映されなければ
  ならない。macro-level target は reject。

## Examples

- **EX-01**: create-track → add-lane → bind 出力を持つ
  recipe が compile され、flat `ops[]`（member 順保持）・
  named `OutputBinding`・予約レンジ placeholder・決定的
  digest が得られる（`compile_emits_flat_ops_and_bindings`）。
- **EX-02**: capability version skew・非 mutation domain・
  async execution・implicit target・nested batch・member
  >64・policy ceiling 超過がそれぞれ typed reject
  （`reject_*` 系 test）。
- **EX-03**: `$param` missing/unexpected/schema 違反・
  予約レンジ literal が bind 層で reject
  （`reject_param_binding_failures`・`reserved_literal_
  rejected`）。
- **EX-04**: workflow YAML 内の `$output`/`$param` 参照が
  registry 解決後に `MacroOnlyExpr` で reject され、
  registry なしでは `UndefinedMacro` が先に発火する
  （`plan_rejects_macro_only_exprs_and_bad_args`）。
- **EX-05**: batch request に非空 `bindings` を載せると
  GW-04 prepare が `PreparationRejected` を返し、
  `bindings` なし batch は従来通り lower される
  （`batch_output_bindings_reject_typed_unsupported`・
  `batch_without_bindings_still_lowers`・
  `batch_bindings_shape_is_schema_enforced`）。

## Guarantees

- compile 成功時の `ops[]`/`bindings[]` は pinned revision・
  runtime input・definition digest に対して決定的であり、
  既存 `synvelis.operation.batch` → `ProjectOp::Composite`
  → GW-04/05/06 経路にそのまま載る（第 2 transaction 系
  なし）。
- macro definition・compiled plan・batch args のいずれにも
  principal/grant/approval/secret/credential は表現不能。
- 非空 `bindings` を持つ batch は prepare で typed reject
  され、placeholder ID がそのまま project へ commit される
  経路は存在しない（fail-closed）。
- 全 reject 経路は理由を持つ error で終了し、部分的な
  compile/plan/batch を出力しない。

## Non-goals

- create-then-reference の end-to-end commit（GW-04 scratch
  rewrite・preview/commit ID CAS）— `Composite::validate`
  逐次化 seam（`synvelis-project`）依存の Deferred。
- Macro GUI 統合（#266）、OS 実験/統合（#267）、Action
  Script lowering（#272）、Workflow Script owner（#273）、
  coordinator/runtime owner（#265）、live Gateway への
  実 request 経路結合。
- `synvelis.operation.batch` 以外の lowering 先・macro の
  runtime 実行 engine。
