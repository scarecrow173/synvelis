---
id: "EXP-3fH8jK2mN5pQr7tVw1xY4z"
type: "experiment"
status: "completed"
title: "AUTX-05 Workflow DSL v1 — YAML parser・Gateway mirror・budget 会計の試行"
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
  implemented-by:
    - "docs/impl/ir/autx-05-workflow-dsl.md"
  depends-on:
    - "docs/specs/command-gateway.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-05-workflow-dsl.md"
    - "evidence/sv264/autx-05-workflow-dsl-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/execution-plan-design.md"
    - "docs/specs/command-gateway.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# AUTX-05 Workflow DSL v1 — YAML parser・Gateway mirror・budget 会計の試行

## 問題設定

Issue #264 は「Action/Macro/既存 Job を組み合わせる Versioned
Workflow Definition と非 RT Executable Plan のコンパイラ」を
要求する。設計上の不確実点は (a) YAML frontend をどこまで
自前で書くか（安全サブセットの fail-closed reject が受入条件）、
(b) Gateway 契約（Capability/Schema/Target/Effect）をどう
headless crate へ持ち込むか、(c) 静的 Budget をどう保守的に
会計するか、(d) 決定的 digest をどう canonical 化するか、の
4 点。実行系（FSM/Dry Run/OS/CLI）と Macro 本体・Script
Builder は別 Issue のため、本 slice では compile-time の
契約のみを対象とした。

## 試行と結果

### E1: YAML frontend — 安全サブセット自前 parser vs 一般 YAML crate

候補は (i) workspace 外部依存の追加を伴う一般 YAML parser
（serde_yaml 系）、(ii) 安全サブセット専用の自前 parser。
受入条件は anchor/alias/tag/implicit 型/予算違反を Golden で
reject することであり、一般 parser では「読めるが reject
すべき構文」を別途 AST walk で再検査する必要がある。さらに
workspace 外部依存の追加は task の「`[workspace.dependencies]`
への新規外部依存の追加（必要になったら ask）」に抵触する。
制限付き parser は block mapping/sequence・quoted/plain
scalar・comment のみに絞れば実装量は限定的と判断した。

**結果**: 自前安全サブセット parser（`src/yaml.rs`）を採用。
block style のみ・flow collection/anchor/alias/tag/block
scalar/merge key/multi-document/directive/duplicate key/
非 canonical 数値を reject、1 MiB・depth 64・200k node・
scalar 64 KiB の budget を強制。`tests/yaml_subset.rs` で
accept/reject の両側を固定。実装中に 2 件の bug を検出・修正:
sequence item が pair 行を二重 consume する line-consumption
bug、および document 開始後の `---` を silent skip して
multi-document を merge してしまう bug（後者は「2 個目の
document marker は reject」へ修正）。さらに review-protocol
の adversarial pass で 4 件を検出・修正: single-quoted
scalar の byte→char 誤変換（非 ASCII mojibake）、inline
comment strip が map value のみで sequence item に未適用、
tab indentation 検査の dead code、`...` marker の後続行判定
で行 index を byte offset 誤用。修正後は comment strip を
line scan の一箇所へ集約し、4 件とも regression test で固定。

### E2: Gateway 契約 — 依存 vs serialized snapshot mirror

`PlanCompiler` は Capability の Version/Availability/
Domain/Execution/Target/Effect/Data-Access/Schema を検証
する必要がある。選択肢は (i) `synvelis-gateway` への Cargo
依存（task で明示禁止）、(ii) Gateway 型の serialize 可能な
mirror を本 crate に定義し、fixture/統合時に serialized
snapshot として受け取る方式。

**結果**: (ii) を採用。`src/registry.rs` に
`RegistrySnapshot`（capabilities + schema entries）、
`src/schema.rs` に `SchemaNode`/`StaticType`/
`ResourceKind`/tagged ID 検証を Gateway の
`registry.rs`/`schema.rs`/`snapshot.rs`/`request.rs` と
同じ意味論で mirror（read-only 参照）。`(id, version)` の
完全一致解決のみで latest 解決を持たず、unknown capability
と unsupported version を別 error で区別。live Gateway
Descriptor Read Adapter は統合 slice へ Deferred。

### E3: 式・条件 AST — 表現力の上限

`Expr` は literal・`$input`・`$steps`・`$item`・array・
object のみ、`Condition` は Eq/Ne/Lt/Lte/Gt/Gte/Exists/
And/Or/Not のみとし、関数呼出・環境変数・eval・任意
script を variant として存在させない設計とした。`$` 参照は
frontend で tagged expr JSON へ lowering し、未知・ malformed
参照を decode/compile の両段で reject。`$item` は
`ForEachBounded` body 内でのみ有効（scope stack で管理）。

**結果**: 採用。`control_step_output_rejected`・
`item_reference_outside_loop_rejected`・
`incomparable_condition_operands_rejected`・
`ordering_condition_requires_numeric` 等で静的照合を固定。
任意コードが AST に存在しないため「静的検証だけで実行権限を
得られない」ことが構造的に保証される
（`forbidden_authority_fields_have_no_representations`・
`plan_has_no_authority_fields`）。

### E4: 静的 budget 会計 — worst-case の保守性

`Cost{steps,iters,depth}` を導入し、sequence は和・depth は
max、`If` branch は両側の max（効果は union で過小申告
しない）、`ForEachBounded` は body cost × bounded
iteration 数で合成。宣言 `WorkflowLimits` を下回る
worst-case は `BudgetUnderestimated`、compiler ceiling
（steps 1e6・depth 64・iters 1e8・runtime 24h・
output 64MiB・memory 1GiB）超過は `BudgetExceeded` で
reject。runtime/output/memory は静的に確定できないため
plan budget として carry forward するのみ。

**結果**: 採用。`loop_workflow_scales_budgets`・
`branch_workflow_unions_effects`・
`iteration_budget_underestimated_rejected`・
`step_budget_underestimated_rejected`・
`depth_budget_underestimated_rejected`・
`declared_budget_above_ceiling_rejected`・
`effect_ceiling_rejected` で固定。

### E5: digest — canonical JSON + SHA-256

`definition_digest`（canonical definition）と
`ExecutablePlan::digest()`（canonical plan）を、object key
を再帰 sort した compact JSON の SHA-256 hex とした。
workspace 既存の `sha2` 依存を利用（新規外部依存なし）。

**結果**: 採用。`definition_digest_stable_across_key_order`・
`basic_workflow_golden_digest`（definition
`1c1a411fb54370886dcdc2307dfcf4110e134178167159898b2f26156e004bae`、
plan `6779664f07bc06672487d6f9603e078ba25af3c214dae22d340ecead16b1492d`
の golden 固定）・`same_inputs_same_plan_across_runs`・
`plan_is_input_value_independent` で決定性を固定。

### E6: test 駆動で検出した実装 bug（記録）

- `OutputTypeMismatch` wrap が schema 系以外の error
  （`UnknownStepId`/`ForwardStepReference`/`ItemOutOfScope`）
  を呑み込み、reject 理由が劣化 → wrap 対象を schema 系のみに限定。
- fixture YAML が flow collection（`{...}`/`[...]`）を使用し
  safe subset parser と非互換 → fixture を block style へ
  書き換え、空 map/array は serde default で表現可能化
  （`SchemaNode::Object` の `fields`/`deny_unknown`、
  `Expr` の `args` 等）。
- `AwaitJob` で output schema 未解決時に opaque job result
  warning を出す経路の match を if-let-else へ整理
  （clippy `single_match_else`）。

## 結論

4 つの不確実点全てで採用案が test で裏付けられ、82 test が
green。`docs/impl/ir/autx-05-workflow-dsl.md` に実装記録を、
`docs/test-specs/autx-05-workflow-dsl.md` に保証仕様を、
`evidence/sv264/autx-05-workflow-dsl-ledger.md` に case 台帳を
残した。Luau/Rhai Builder との同一 compiler 検証・live
Gateway adapter・Macro 展開・runtime FSM は #263/#265/#267/
#273 の後続 slice で NotRun/Deferred。
