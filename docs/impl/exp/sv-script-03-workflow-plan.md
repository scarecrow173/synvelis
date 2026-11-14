---
id: "EXP-svscript03workflow01"
type: "experiment"
status: "completed"
title: "SV-SCRIPT-03 Workflow Script Builder・異種参照 inline 展開の試行"
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
  implemented-by:
    - "docs/impl/ir/sv-script-03-workflow-plan.md"
  depends-on:
    - "docs/automation/design-script.md"
    - "docs/impl/ir/sv-script-01-script-api-model.md"
    - "docs/impl/ir/sv-script-02-luau-action-script.md"
    - "docs/impl/ir/autx-05-workflow-dsl.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv-script-03-workflow-plan.md"
    - "evidence/sv273/sv-script-03-workflow-plan-ledger.md"
  refines: []
  refined-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/automation/design-script.md"
    - "crates/synvelis-script/src/workflow.rs"
    - "crates/synvelis-automation/src/importer.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV-SCRIPT-03 Workflow Script Builder・異種参照 inline 展開の試行

## 問題設定

Issue #273（SV-SCRIPT-03）の headless slice は、YAML/Luau/Rhai
の workflow source が同一の `WorkflowDefinition`→`PlanCompiler`
→`ExecutablePlan` 経路へ収束し、異種 workflow の多段参照が
compile 時に fail-closed で解決されることが契約。設計上の
未確定点は 4 つあった:

1. script 側の中間表現 — script backend（Luau/Rhai VM）が
   生成する「評価記録」をどの型で automation へ渡すか。
   Script 専用 Plan IR・第二 compiler を作らず、既存
   `WorkflowDefinition` wire 形へ写せる構造が必要。
2. `action()` の step kind 決定 — script が `query`/`invoke`/
   `async-submit` を名指すのか、capability contract から導く
   のか。script 側が kind を偽装できると domain/execution
   検査を迂回する。
3. 異種参照の解決方式 — `workflow()` 呼出しを runtime の VM
   間呼出しにするか、compile 時に inline 展開するか。
   後者なら step id 衝突・`$input`/`$steps` 参照の再
   名前空間化・宣言 output の引継ぎ・循環/予算検査をどう
   既存 compiler の規則と整合させるか。
4. 式の射影 — 呼出し引数式（例: `$input.track.field`）を
   参照先の `$input.<name>` へ代入したとき、path をどう合成
   するか。既存 `Expr` には任意 base への field 投影がない。

## 試行と結果

### E1: 中間表現 — import document JSON（canonical
`WorkflowDefinition` wire 形 + `kind:"workflow"` ノード）

script backend の評価記録を `ScriptWorkflowSpec`（call 木 +
typed io + limits）とし、`WorkflowScriptBuilder::build` が
canonical import document JSON を生成する。document は
`schema_version`/`workflow_id`/`version`/`inputs`/`outputs`/
`limits`/`steps` を持ち、steps は既存 kind 名
（`query`/`invoke`/`async-submit`/`if`/`for-each-bounded`/
`await-job`/`atomic-macro`/`emit-result`）に加えて
`kind:"workflow"` を許す。automation の
`import_workflow_document` が `workflow` ノードを除去して
単一 `WorkflowDefinition` を返すため、builder 出力は
「未解決参照を含む definition の中間形」であり Plan IR の
複製ではない。式位置は YAML frontend と同一の ergonomic
`$`-reference 面を verbatim に運び、lower は frontend の
`lower_expr`/`lower_condition`（`pub(crate)` 化して再利用）
が行う — 第二 DSL・第二 lower を作らない。代替案
（script 専用 typed IR を定義して automation が decode する
形）は「同一 Definition/Plan digest」の golden を構造的に
保証しにくく、wire 形の二重管理になるため棄却。

### E2: `action()` kind は capability contract が決める

`WorkflowScriptBuilder::resolve_action` は
`ScriptApiModel::describe` + `BindingSet::get` で name を解決
し、`(binding.domain, binding.execution)` の組から step kind
を導く: `(Query,Sync)→query`、`(ProjectMutation,Sync)→
invoke`、`(AsyncOperation,AsyncAccept)→async-submit`。これ
以外（Proposal・RuntimeControl・sync AsyncOperation・
async-accept Query/Mutation）は `UnsupportedWorkflowStep` で
fail-closed — script が kind を名指す経路は型に存在しない。
capability pin は `id@major.minor` 文字列で、AUTX-05
`VersionedRef` wire 形と一致する。`action()` は呼出しの
記録であり invoke ではない — VM 評価時に Gateway 副作用は
起きず、分岐は `If`/`ForEach`/`StepOutputRef` へ lower される
（Issue の「Script 評価時に Project/外部 Job が実行されない」
を構造で担保）。

### E3: 異種参照は compile 時 inline 展開 + virtual output

`workflow()` 参照は `ScriptWorkflowRef`（language/version/
source_digest/path の pin）として記録され、importer が
`WorkflowResolver` 経由で compile 時に解決する。resolver は
`ImportedWorkflow { language_id, version, source_digest,
payload }` を返し、pin 照合（language/version/digest）は
importer が単一実装で行う — resolver は実際の値を報告する
だけで自己申告しない。payload は完成済み
`WorkflowDefinition`（YAML file・投影済み source）または
未解決 document（script source）で、後者は再帰的に
`import_document` へ掛ける。解決済み定義は call site id
`<call>` を prefix として全 step id を `<call>__<step>` へ
namespace 化し、`$input.<name>[.<rest>]` を呼出し引数式へ
`project_expr` で代入、`$steps.<id>` を `call__` prefix へ
再帰書換えする。宣言 output の `emit-result` は step として
残さず virtual output map（`output name → 値式`）として親
scope に登録し、`$steps.<call>.<output>[.<rest>]` を後続
sibling が参照できる — scope は compiler の step scope と
同じく if/for-each 本体で push/pop され、分岐を越えた参照
は `UnknownStepId` で fail-closed。代替案（参照先を別 plan
として保持し coordinator が跨ぐ形）は第二 run surface・VM
間継続を生むため棄却 — inline 展開なら実行主体は常に単一
`PlanCompiler`/`WorkflowCoordinator`。

### E4: 式射影は `Expr::Field` で path を合成

`$input` 引数がリテラル Object/Array の場合は path を具体値
へ畳み込み（欠落 segment は typed reject）、`$steps`/`$input`/
`$item`/`Expr::Field` の参照形は path を連結し、それ以外の
構築値（Object/Array 式・条件結果等）には新設
`Expr::Field { base, path }` を被せる。`Expr::Field` は
YAML 面では表現不能な内部 variant で、compiler は共有
`project_path` で静型付け（解決不能は `TypeMismatch`）、
eval/macros は fail-closed に path 解決する。macro 式面でも
同 variant を受理するため `macros.rs` の 2 箇所の網羅 match
を更新した。代替案（importer 側で独自の式評価器を持つ）は
第二意味論になるため棄却。

## 採用結果

E1〜E4 すべて採用。`WorkflowScriptDefinition`/`ScriptWorkflowRef`/
`ScriptStepCall`/`ScriptWorkflowSpec`/`WorkflowScriptBuilder` と
`import_workflow_document`/`WorkflowResolver`/
`MapWorkflowResolver`/`FileWorkflowResolver`/
`compile_workflow_yaml`/`compile_script_workflow`/
`workflow_source_kind` が実装され、script 7 test・automation
16 test が green。golden として YAML `BASIC_WORKFLOW` と
同値の Luau spec が**同一 `WorkflowDefinition`・同一
`ExecutablePlan` digest・同一 step order** を生成することを
確認し、Luau→Rhai→YAML の 3 段参照が `mid__leaf__tracks`
の二重 namespace step と virtual output 引継ぎで compile
されることを確認した。fail-closed 行列（cycle・version/
digest/language drift・未知/不足/過剰 input・未知/
未 emit output・duplicate id・depth/step 予算・path escape・
未知 source・未来 Step 参照）は全て typed reject。

棄却した代替: script 専用 typed IR（E1）・script 名指しの
step kind（E2）・runtime VM 間呼出し/別 plan 連鎖（E3）・
importer 独自式評価器（E4）。

## Deferred（本 slice が証明しないもの）

- 実 Luau/Rhai VM 実行・script source → `ScriptWorkflowSpec`
  の評価経路 — ruviform#559 同一バイナリ結合 Gate。
- `.luau`/`.rhai` の file-backed 参照解決 — backend 未結合の
  ため `FileWorkflowResolver` は `UnsupportedSource`。
- `workflow run --file` の CLI 本体配線 — synvelis#262。
- Coordinator との実 Plan/Job 結合受入 — synvelis#265/#267。
