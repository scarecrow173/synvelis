---
id: "EXP-svscript02luauaction01"
type: "experiment"
status: "completed"
title: "SV-SCRIPT-02 ActionRecipe 分類・batch lower・Gateway 接続の試行"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/272"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/sv-script-02-luau-action-script.md"
  depends-on:
    - "docs/automation/design-script.md"
    - "docs/automation/design-atomic-macro.md"
    - "docs/automation/design-capability-action.md"
    - "docs/impl/ir/sv-script-01-script-api-model.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv-script-02-luau-action-script.md"
    - "evidence/sv272/sv-script-02-luau-action-script-ledger.md"
  refines: []
  refined-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/automation/design-script.md"
    - "docs/automation/design-atomic-macro.md"
    - "crates/synvelis-script/src/action.rs"
    - "crates/synvelis-automation/src/action_recipe.rs"
    - "crates/synvelis-gateway/src/gateway.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV-SCRIPT-02 ActionRecipe 分類・batch lower・Gateway 接続の試行

## 問題設定

Issue #272（SV-SCRIPT-02）の headless slice は、SV-SCRIPT-01
の `ScriptApiModel`/`BindingSet`/digest 契約を消費して
`ScriptActionDefinition` → typed `ActionRecipe` → 実
`Gateway::bind`/`invoke`（`Gateway::handle`）経路への昇格と、
2〜64 `ProjectMutation` の AUTX-04 batch/Composite lower を
実装することが契約（`docs/automation/design-script.md`）。
設計上の未確定点は 4 つあった:

1. recipe の分類規則 — member が 1 件のときと 2 件以上の
   ときで lower 形が変わる（prepare 包含 draft / 直接 draft /
   batch draft）。どの domain・consistency・project 組合せを
   batch に入れ、どれを typed reject / Workflow 誘導にするか。
2. batch member の wire 形 — script 側の解決済み member
   （exact `CapabilityRef`・lowered args・typed target）を
   `synvelis.operation.batch` の `ops[]` member shape
   （`capability`/`target`/`args` の wire JSON）へどう写すか。
   `bindings[]`（create-then-reference）を script recipe が
   生成すべきか、#263 deferred の gateway fallback を昇格
   すべきか。
3. draft → 実経路の接続面 — `Gateway::handle` は
   `parse_wire`→`bind`→`invoke` を内包するが、script 側の
   `RegistrySnapshot::capture` が必要とする
   `SchemaRegistry` の read accessor が存在しなかった。
   Gateway に何を足し、何を足さないか。
4. automation snapshot の出所 — `compile_action_recipe` が
   要求する AUTX-04 `RegistrySnapshot` は live registry の
   投影ではなく serialized mirror。e2e でどの snapshot を
   正本として使うか。

## 試行と結果

### E1: recipe 分類 — domain/consistency/project の batch 適格規則

`ActionRecipe::build` は member 解決（`resolve_call`）の後に
分類を行う。`members.len() == 1` は `Single` — member の
domain に応じ `ProjectMutation` なら prepare 包含 draft、
それ以外は直接 draft。2 件以上は全 member が
`OperationDomain::ProjectMutation`・同一 project・同一
consistency・非 `synvelis.operation.batch` でなければ typed
reject（`UnsupportedMemberDomain`/`CrossProject`/
`ConsistencyMismatch`/`NestedBatch`）。Query/RuntimeControl/
AsyncOperation が混ざる batch は atomicity を保証できない
ため `UnsupportedMemberDomain` で fail-closed — Workflow
Coordinator（AUTX-06）への誘導は型ではなく verdict の
解釈に委ねる。member 数は 1 または 2〜64（空・65 超は
`EmptyRecipe`/`TooManyMembers`）。この規則は Issue の
「65件/Nested/Cross-project/Query/Async を適切に拒否」
をそのまま型に写した。代替案（domain 混在を warning で
通して Workflow へ丸ごと降ろす）は「atomic recipe が
部分的にしか実行されない」経路を生むため棄却。

### E2: batch member の wire 形 — literal-only MacroStep への翻訳

`compile_action_recipe` は各 `RecipeMember` を
`MacroStep { id: "m{index}", capability: VersionedRef,
args: Expr::Literal{value}, target: Expr::Literal }` へ翻訳し、
`AtomicMacroDefinition { parameters: [], output_bindings: [],
policy: MacroPolicy::default() }` として変更なしの
`MacroCompiler::compile` に掛ける。これにより capability
availability・`ProjectMutation`+`Sync` の domain/execution
検査・schema 再検証・target contract・effect/data-access
ceiling・nested batch・64 件 bound の全検査が AUTX-04 の
単一実装に残る。`ScriptTarget` は literal id（resource /
resource-set の id 配列 / create の parent id）へ写し、
kind は automation snapshot の contract が再検証する —
script snapshot と automation snapshot の view が食い違えば
typed reject になる（層状 drift 検出）。`bindings[]` は
`output_bindings: []` で常に空 — script action の member は
実行時にしか存在しない id を名指しできないため、
create-then-reference は script の表現外。代替案
（`$output.` placeholder を script 式へ写像）は #263 が
所有する binding 意味論の重複実装になるため棄却。
その結果、Issue の条件付き昇格条件（script recipe が非空
`bindings[]` を生成する場合）には該当せず、gateway の
batch bindings fallback は **Deferred**（非空 `bindings`
は従来どおり `PreparationRejected`）。

### E3: draft → 実経路 — `InvocationDraft::submit` + `Gateway::schemas()`

`InvocationDraft` は既に `to_wire_value()`（serde wire
JSON）と `verify_wire()`（`WireRequest::parse` 再検証）を
持っていた。実経路接続は `submit(&mut Gateway,
CallerIdentity)` — `gateway.handle(&self.to_wire_value(),
caller)` の 1 呼出に留め、`bind`/`invoke`/handle の
意味論は無変更。`CallerIdentity` は host が引数で供給し、
draft 型に authority field は存在しない（型で表現不能）。
`RegistrySnapshot::capture` が必要とする
`SchemaRegistry` の read accessor として `Gateway::schemas()`
を追加したのみ — `schemas_mut()`/`registry_mut()` の変更面は
増やさない。e2e では `fixture_rig()` の live registry pair
から snapshot を capture し、surface spec で script API 名を
exact `CapabilityRef` に pin して model/bindings を生成、
recipe → compile → draft → `gw.handle` → proposal →
`accept_apply` → `CommitGate::commit` → undo/redo までを
実測した（`tests/script_action.rs`）。代替案（Gateway に
draft 専用の bind/invoke variant を生やす）は handle が
既に正本経路であり二重経路を生むため棄却。

### E4: automation snapshot — serialized mirror を fixture として供給

AUTX-04 の `RegistrySnapshot` は capability descriptor +
`SchemaNode` の serialized 宣言（`parse_registry_snapshot_yaml`）
であり、live `CapabilityRegistry`/`SchemaRegistry` からの
投影関数は存在しない。e2e では gateway `PrepareEngine`
op table と同値の 2 capability（`synvelis.clip.mute`/
`synvelis.clip.move`）+ input/output schema を持つ YAML
mirror を test fixture として定義した。script 側 snapshot
（live capture）と automation 側 snapshot（serialized
mirror）が別 view であること自体が drift 検出の層になり、
`target_kind_mismatch` 等の reject は両 view の不一致を
typed error として観測する。代替案（live registry →
automation snapshot の投影関数を新設）は AUTX-04 の
snapshot 契約（serialized・source-agnostic）を越えるため
棄却 — 投影の生成は host 側の責務として残る。

## 採用結果

E1〜E4 すべて採用。`ScriptActionDefinition`/`ActionRecipe`/
`RecipeSpec` の typed 契約、`compile_action_recipe` の
literal-only macro 翻訳、`InvocationDraft::submit` +
`Gateway::schemas()` の最小接続が実装され、script 22 test・
automation 9 test・gateway e2e 4 test が green。batch
recipe は `synvelis.operation.batch` → `ProjectOp::Composite`
として commit され、1 CAS（expected-revision 1→2）・
1 Undo で member effect が原子的に復元されることを
#263 oracle と同じ観測面で確認した。

棄却した代替: domain 混在 batch の Workflow 誘導（E1）・
`$output.` placeholder の script 写像（E2）・Gateway 側の
draft 専用経路（E3）・live→automation snapshot 投影関数
（E4）。
