---
id: "TSPEC-7kQm2XvB9pLw4RtY6uN3sD"
type: "test-spec"
status: "approved"
title: "AUTX-05 Workflow DSL v1 — YAML safe subset・typed definition・static plan compile invariants"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/264"
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
    - "docs/specs/command-gateway.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/autx-05-workflow-dsl.md"
    - "docs/impl/exp/autx-05-workflow-dsl.md"
    - "evidence/sv264/autx-05-workflow-dsl-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/designs/command-gateway-design.md"
  verified-by: []
  references:
    - "evidence/sv264/autx-05-workflow-dsl-ledger.md"
    - "docs/impl/ir/autx-05-workflow-dsl.md"
  defers: []
  deferred-by: []
---

# AUTX-05 Workflow DSL v1 Test Spec — YAML safe subset・typed definition・static plan compile invariants

## Purpose

Issue #264（AUTX-05）の headless slice が保証すべき不変条件を
固定する。対象は `synvelis-automation` crate の YAML safe
subset parser・canonical decode・`PlanCompiler`・
`ExecutablePlan` digest であり、runtime 実行・認可・
scheduling・実 Job/OS/CLI 結合は対象外（#265/#267 の別
Gate）。Builder（Luau/Rhai）生成 Definition との同一
compiler 検証は #273 で Deferred — 本 spec は「source 非依存
の `WorkflowDefinition` が同一 compiler で検証される」こと
までを保証する。

## Feature

Versioned YAML Workflow DSL v1 を安全な YAML サブセットで
記述し、型付き `WorkflowDefinition` へ decode し、serialized
Registry Snapshot と供給 input に対して `PlanCompiler` が
静的検証・Budget 会計・決定的 `ExecutablePlan` 生成を行う
feature。

## Rules

- **AUTX-TS-R01 YAML safe subset fail-closed**: parser は
  block mapping/sequence・quoted/plain scalar・制限付き
  implicit 型・comment のみ受理する。anchor/alias/YAML tag/
  block scalar/merge key/flow collection/multi-document/
  directive/duplicate key/非 canonical 数値/深さ・node・
  byte・scalar budget 超過は全て reject されなければならない。
- **AUTX-TS-R02 canonical determinism**: 同一
  Definition+Input+Registry Snapshot からは、実行ごとに同一
  `definition_digest`・plan digest・`step_order`・warnings
  が生成されなければならない。object key 順の違いは digest
  に影響してはならない。
- **AUTX-TS-R03 exact version resolution**: Capability・
  Schema は `(id, version)` 完全一致でのみ解決される。
  latest 解決は存在せず、unknown と unsupported version は
  区別された error で reject されなければならない。
- **AUTX-TS-R04 static fail-closed validation**: 未知
  step/capability/input、前方参照・循環、型不一致、Target
  kind/shape 不一致、暗黙 target、未宣言/重複/nested/missing
  output emit、非 Job 型への await、未定義 macro、schema
  violation は全て compile reject でなければならない。
- **AUTX-TS-R05 conservative static budget**: step/depth/
  iteration の worst-case は branch で過小申告してはならず
  （effect/data-access は可能性集合の union）、bounded loop
  は iteration 数で乗算され、宣言 limit 未満の worst-case・
  compiler ceiling 超過は reject でなければならない。
- **AUTX-TS-R06 no authority in definition or plan**:
  principal/grant/approval/OS credential/shell/eval/任意
  script を表現する field・variant は存在してはならず、
  unknown field は decode で reject されなければならない。
  静的検証のみで実行権限を得る経路は存在してはならない。
- **AUTX-TS-R07 typed step/output contract**: sequential/
  bounded branch/bounded for-each/async submit/job wait/
  capability invoke/emit-result は型付き AST と step output
  schema で静的に照合され、`$steps.<id>.<field>` は先行
  step の宣言 output schema のみを指せなければならない。
- **AUTX-TS-R08 version/schema safety**: `schema_version`
  は version 1 のみ受理し、unsupported version・unknown
  field は安全な error で reject されなければならない。

## Examples

- **EX-01**: block style の workflow YAML（query → invoke →
  emit-result）が parse・decode・compile され、golden
  digest/step order/effect bound が得られる
  （`basic_workflow_golden_digest`）。
- **EX-02**: anchor `&a`/alias `*a`/tag `!x`/flow `{...}`/
  block scalar `|`/2 個目の `---` を含む YAML がそれぞれ
  reject される（`yaml_subset.rs` reject 系 test）。
- **EX-03**: `if` の両 branch が異なる effect を持つ場合、
  plan の `effect_upper_bounds` は両者の union を含み、
  片側のみの申告で compile が通ってはならない
  （`branch_workflow_unions_effects`）。
- **EX-04**: `for-each` の `max_iterations` が 0・未指定・
  budget 超過の場合に reject され、body cost が iteration
  数で乗算される（`loop_workflow_scales_budgets`・
  `iteration_budget_underestimated_rejected`）。
- **EX-05**: `availability: planned`/`retired`/`disabled` の
  capability、未知 capability、未対応 version が
  fail-closed で reject される（`planned_retired_disabled_
  rejected`・`unknown_capability_rejected`・
  `unsupported_capability_version_rejected`）。
- **EX-06**: definition/plan に `principal`/`grant`/`shell`/
  `eval` 等の field を書くと decode reject または variant
  非存在で表現不能である（`forbidden_authority_fields_
  have_no_representations`・`plan_has_no_authority_fields`）。
- **EX-07**: 同一 definition を JSON tagged-expr 経路で
  decode しても YAML 経路と同一 compiler・同一検証結果に
  到達する（`plan_compiler_is_source_agnostic`・
  `json_decode_accepts_tagged_expr_form`）。

## Guarantees

- 同一 (Definition, Inputs, Registry Snapshot) triple は
  常に同一 digest・step order・warnings を生成する。
- compile が成功した plan は、宣言された全 capability が
  snapshot 上で exact version・available・domain/execution
  一致・target/schema 適合・effect/data-access ceiling 内で
  あることを静的に証明する。
- Definition/Plan は authority を一切含まず、実行権限は
  compile 出力からは得られない（runtime 側の再検証が前提）。
- 全 reject 経路は理由を持つ error で fail-closed に終了し、
  部分的な plan を出力しない。

## Non-goals

- runtime 実行・Dry Run の成功保証・実 Job/Recovery/OS/CLI
  結合（#265/#267）。
- Luau/Rhai Script Builder が生成する Definition との同一性
  実証（#273 — 本 spec は source 非依存 compiler への到達
  まで）。
- AtomicMacro 定義・Registry・展開（#263 — 本 spec は未定義
  macro 参照の fail-closed reject まで）。
- live Gateway Descriptor の取得・availability の実時点
  検証（統合 slice）。
- YAML safe subset の拡張（flow collection 等の追加受理は
  別判断）。

## Risk

- `AtomicMacro` の semantics が #263 で確定した際、参照解決
  規則の再検証が必要。
- Registry Snapshot mirror と live Gateway 契約の drift は
  統合 slice の adapter 検証で捕捉する必要がある。
- `AwaitJob` の job validity・output 実値は runtime-owned
  であり、compile 成功は job 成功を保証しない — この境界を
  崩す変更（例: compile 時の job 存在仮定）は本保証を
  無効化する。
- safe subset を超える YAML 記法を受理する変更は
  AUTX-TS-R01 を破壊するため、subset 拡張は spec 改訂を
  必要とする。
