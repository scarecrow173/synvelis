---
id: "TSPEC-034Xl5p51hMFBCqjv2UI5O"
type: "test-spec"
status: "approved"
title: "sv-script-03-workflow-plan"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/273"
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
    - "docs/automation/design-script.md"
    - "docs/impl/ir/sv-script-01-script-api-model.md"
    - "docs/impl/ir/sv-script-02-luau-action-script.md"
    - "docs/impl/ir/autx-05-workflow-dsl.md"
    - "docs/impl/ir/autx-06-workflow-coordinator.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/sv-script-03-workflow-plan.md"
    - "docs/impl/exp/sv-script-03-workflow-plan.md"
    - "evidence/sv273/sv-script-03-workflow-plan-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/automation/design-script.md"
  derived-by: []
  verifies:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
  verified-by: []
  references:
    - "docs/automation/spec.md"
  defers: []
  deferred-by: []
---

# sv-script-03-workflow-plan

## Purpose

YAML/Luau/Rhai の workflow source が単一の
`WorkflowDefinition`→`PlanCompiler`→`ExecutablePlan` 経路へ
収束し、異種 workflow 参照が compile 時に fail-closed で
解決されることを保証する test spec。Script は authority を
持たず独立 run surface を持たない（`design-script.md`）という
境界が、型と fixture の両面で維持されることを記録する。
実 Luau/Rhai VM 実行の統合ゲート（ruviform#559）が閉じる
までの契約層検証であり、製品 E2E の受入を代替しない —
Mock/fake の PASS を実行 Pass と読み替えない。

## Feature

Workflow Script の共通 Plan 化と異種 Workflow 参照解決:
`WorkflowScriptBuilder` が `ScriptWorkflowSpec` を canonical
import document へ lower し、`import_workflow_document` が
`kind:"workflow"` 参照を `WorkflowResolver` 経由で解決・
inline 展開して `WorkflowDefinition` を返し、
`PlanCompiler` が唯一の plan 権威として `ExecutablePlan` を
生成する。

## Rules

- Rule: 同一意味の YAML と script spec は同一
  `WorkflowDefinition`・同一 plan digest・同一 step order を
  生成する — 第二 Plan IR・第二 compiler は存在しない。
- Rule: `action()` の step kind は capability の
  domain/execution contract が決定し、script は kind を
  名指せない。解決不能 name・非対応 domain/execution は
  typed reject。
- Rule: `workflow()` 参照は language/version/source-digest/
  path で pin され、解決は compile 時に完結する。実行時の
  VM 間呼出し・runtime import は生成されない。
- Rule: pin 照合は importer が単一実装で行い、resolver は
  実際の language/version/digest を報告するだけで自己
  申告しない。drift は typed fail-closed。
- Rule: 解決済み定義は `<call>__<step>` namespace で inline
  され、`$input` は呼出し引数式へ、宣言 output は virtual
  output として親 scope にのみ見える。制御フロー分岐を
  越えた参照は fail-closed。
- Rule: import cycle・深さ/step 予算超過・typed I/O
  mismatch・未知 output・path escape・未知 source は全て
  typed reject で、推測・fallback・silent skip はない。
- Rule: 未来 Step 参照・schema/type/budget/effect の各 reject
  は既存 `PlanCompiler` の規則がそのまま効き、importer が
  検査を弱めない。
- Rule: Script 定義・spec・binding に authority
  （CallerIdentity/Grant/Approval/executor/VM state）を
  表現する field は存在しない。

## Examples

- Example: `BASIC_WORKFLOW`（YAML）と同値の Luau
  `ScriptWorkflowSpec` が同一 definition・同一 plan digest
  を生成する（`yaml_and_luau_specs_yield_same_definition_
  and_plan`）。
- Example: Luau spec が Rhai document を、Rhai document が
  YAML definition を参照する 3 段 chain が、typed input
  引数と virtual output 引継ぎで `mid__leaf__tracks` へ
  inline 展開され compile される
  （`heterogeneous_chain_luau_rhai_yaml_inlines_and_
  compiles`）。
- Example: `FileWorkflowResolver` が canonical root 内の
  `.yaml` を digest 検証付きで解決し、`../`・絶対 path・
  `.luau` file を typed reject する
  （`file_resolver_*`）。
- Example: pin 不一致（version/source digest/language）・
  A↔B import cycle・深さ 33 段・未知 output・宣言 output
  未 emit・duplicate id がそれぞれ対応する `ImportError`
  variant で reject される。

## Guarantees

- `WorkflowScriptBuilder::build` の出力は import document
  であり、`action()` は invoke ではなく step 参照の記録 —
  VM 評価時に Project/外部 Job は実行されない。
- `import_workflow_document` は任意の script document を
  既存 `WorkflowDefinition` へ還元し、全失敗経路が typed
  error で fail-closed。
- 生成される `WorkflowDefinition`/`ExecutablePlan`/解決物に
  VM stack・coroutine・continuation・authority field は
  存在せず、Run/Cancel/Resume/Indeterminate は既存
  Coordinator/Gateway のみが処理する。
- `.luau`/`.rhai` file 参照・実 VM backend・CLI 配線は
  Unsupported/Deferred として型または記録で明示され、
  silent fallback はない。

## Non-goals

- 実 Luau/Rhai VM の実行・script source の解析
  （ruviform#559 gate）。
- `workflow run --file` の CLI 本体配線（synvelis#262）。
- Coordinator との実 Plan/Job 結合受入・製品 E2E
  （synvelis#265/#267）。
- Rhai の正式製品対応（Issue 対象外 — PoC fixture のみ）。
- `Expr::Field` の YAML 面への露出（内部 variant のまま）。

## Risk

- inline 展開は参照先 step を親 plan へ複製するため、深い
  多段参照や大きな workflow の組合せで `MAX_STATIC_STEPS`
  ・`MAX_IMPORT_DEPTH` の予算が先に効く — 予算は fail-closed
  であり、引き上げは plan 権威（#264）の契約変更を要する。
- virtual output は値式の代入であり、参照先 emit の実行順・
  回数（if/for-each 内 emit）の意味論は compiler の step
  scope 規則に従う — scope を越えた参照は reject されるが、
  「常に実行される emit」の保証は plan 側の責務。
- `Expr::Field` は新しい式 variant であり、将来の expr
  consumer（新しい evaluator/serializer）が網羅 match を
  更新し損ねると compile error で検出される — 型で
  fail-closed だが、consumer 追加時の更新は必要。
