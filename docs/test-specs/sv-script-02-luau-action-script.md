---
id: "TSPEC-034XjK2mQpRsTuVwXyZab1"
type: "test-spec"
status: "approved"
title: "SV-SCRIPT-02 ActionRecipe→Gateway/Atomic Batch 統合 invariants"
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
  implemented-by: []
  depends-on:
    - "docs/automation/design-script.md"
    - "docs/automation/design-atomic-macro.md"
    - "docs/automation/design-capability-action.md"
    - "docs/impl/ir/sv-script-01-script-api-model.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/sv-script-02-luau-action-script.md"
    - "docs/impl/exp/sv-script-02-luau-action-script.md"
    - "evidence/sv272/sv-script-02-luau-action-script-ledger.md"
    - "docs/test-specs/sv-script-01-script-api-model.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/command-gateway.md"
    - "docs/designs/command-gateway-design.md"
  verified-by: []
  references:
    - "docs/automation/design-script.md"
    - "docs/automation/design-atomic-macro.md"
    - "docs/impl/ir/sv-script-02-luau-action-script.md"
    - "crates/synvelis-script/src/action.rs"
    - "crates/synvelis-script/src/binding.rs"
    - "crates/synvelis-automation/src/action_recipe.rs"
    - "crates/synvelis-gateway/tests/script_action.rs"
    - "evidence/sv272/sv-script-02-luau-action-script-ledger.md"
  defers: []
  deferred-by: []
---

# SV-SCRIPT-02 ActionRecipe→Gateway/Atomic Batch 統合 invariants

## Purpose

Issue #272（SV-SCRIPT-02）の headless slice が保証すべき不変条件を
固定する。対象は `ScriptActionDefinition` の検証と source/
version drift の fail-closed、`ActionRecipe::build` の member
解決と batch 分類規則、`Single`/`Batch` recipe の
`InvocationDraft` lower、`compile_action_recipe` の AUTX-04
`MacroCompiler` 経由 lowering、および draft →
`Gateway::handle` → prepare → proposal → apply → commit →
undo の実経路 e2e である。実 Luau runtime 実行・実
Luau.Analysis・CLI `action run --file` 入口（sv#262）・
editor（#274）・consumer E2E（#267）は対象外であり、
headless fixture の Pass を実機の Pass に昇格しない。

## Feature

`ScriptActionDefinition` + `ScriptApiModel`/`BindingSet` から
構築される typed `ActionRecipe` と、その
`Single`（prepare 包含/直接 draft）・`Batch`
（AUTX-04 compile → `synvelis.operation.batch` →
`ProjectOp::Composite`）の Gateway 実経路 lower。

## Rules

- Rule: authority-free recipe — `ScriptActionDefinition`・
  `ActionRecipe`・`RecipeMember`・`InvocationDraft` の型に
  `CallerIdentity`/`Grant`/`Approval`/`Principal`/executor
  の表現が存在しない。`task`/`grant`/`idempotency_key` は
  opaque 参照であり、認可は常に `Gateway::handle` の
  parse→bind→invoke が host 供給の `CallerIdentity` で行う。
  解析・recipe 構築の成功は認可を意味しない。
- Rule: digest 連鎖の fail-closed — definition の
  `source_digest` 不一致は `SourceDrift`、schema version・
  id/digest 形状・language↔backend 不整合・backend 不在・
  未解決 schema は typed `ScriptApiError`。recipe は
  definition/binding/model/recipe の 4 digest を pin し、
  `verify_definition` が id/version/digest の drift を
  `DefinitionMismatch`/`VersionDrift` で拒否する。
  binding↔model の drift は `StaleArtifact` で拒否する。
- Rule: batch 分類の全有か無か — 2〜64 member の batch は
  全 member が単一 project・単一 consistency の
  `ProjectMutation` でなければならない。Query/Proposal/
  `RuntimeControl`/`AsyncOperation` 混在・nested batch・
  cross-project・consistency 不一致・0 件・65 件超は全て
  typed reject であり、部分的な atomic 実行経路は存在しない。
- Rule: Gateway が最終権威 — script/automation 層で受理
  された値でも、wire 境界の schema 再検証・target 検証・
  consistency legality・availability・認可・receipt が
  全て `Gateway` 側で再適用される。script 側の構造検査は
  authority ではない。
- Rule: atomicity は既存経路のみ — batch recipe は
  `synvelis.operation.batch` の `ops[]` を prepare envelope
  経由で `ProjectOp::Composite` へ lower し、GW-04 scratch
  prepare → GW-05 approval → GW-06 commit・1 CAS・1 Undo
  の既存契約に従う。独自 transaction engine・project writer・
  job 正本・独立 executor は存在しない。
- Rule: 非空 `bindings[]` は生成しない — recipe member は
  解決済みの具体 args/target のみを持ち、create-then-
  reference の `$output.` binding は script action の表現外。
  `compile_action_recipe` は `output_bindings: []` を生成し、
  gateway の非空 bindings `PreparationRejected` は #263 の
  deferred のまま維持される。

## Examples

- Example: definition 検証 — valid definition が validate +
  digest を返し、schema version 不一致・不正 id/digest・
  language↔backend 不整合・backend 不在・未解決 schema・
  source drift がそれぞれ typed error になる
  （`crates/synvelis-script/tests/action.rs`）。
- Example: single lower — `ProjectMutation` member 1 件の
  recipe が `synvelis.operation.prepare` 包含 draft に、
  Query/RuntimeControl member が直接 capability draft に
  lower され、いずれも `verify_wire`（`WireRequest::parse`）
  を通過する（`tests/action.rs`・`tests/script_action.rs`）。
- Example: batch 分類 reject — Query/RuntimeControl 混在・
  cross-project・consistency 不一致・nested batch・0 件・
  65 件超が typed `ScriptApiError` で拒否される
  （`tests/action.rs`）。
- Example: macro compile — batch recipe が literal-only
  `MacroStep` として `MacroCompiler` を通り、unknown
  capability・非 mutation domain・nested batch・schema
  違反・target kind 不一致・65 件が `AutomationError::Compile`
  で拒否される（`crates/synvelis-automation/tests/action_recipe.rs`）。
- Example: e2e golden — `muteClip`+`moveClip` の batch recipe
  が live registry snapshot → model → bindings → recipe →
  compile → batch draft → `Gateway::handle`（agent+grant）→
  proposal → apply → `CommitGate::commit` で revision 1→2、
  `ProjectOp::Composite`・2 commands・mute+anchor 960 の
  両 effect が commit され、1 Undo で原子的に復元・redo で
  再適用される（`crates/synvelis-gateway/tests/script_action.rs`）。
- Example: direct invoke — Query member の recipe が直接
  draft で `synvelis.context.query` を human authority で
  invoke し `Completed` を返す（`tests/script_action.rs`）。
- Example: Gateway 再検証 — script/automation 層の bound を
  超える 65 ops の batch args が wire schema で typed reject
  される（analysis 成功 ≠ authorization の実測）。

## Guarantees

- `ActionRecipe` は definition/binding/model/recipe の digest
  を pin し、`verify_definition` が構築時 definition との
  乖離を検出する。生成物は canonical JSON + SHA-256 で
  決定的である。
- `compile_action_recipe` は `MacroCompiler` を変更せず、
  recipe member を literal-only step として渡す — capability
  availability・domain/execution・schema・target・ceiling・
  member 数の検査は AUTX-04 の単一実装が権威であり続ける。
- `to_draft`/`to_batch_draft` が生成するのは
  `InvocationDraft`（wire-shaped precursor）のみで、
  `submit`/`handle` が `Gateway::bind`→`invoke` の認可・
  schema 再検証・approval・receipt を必ず通す。
- batch の commit は #263 の独立 oracle と同一経路
  （`synvelis.operation.batch` → prepare → `Composite` →
  commit → undo）であり、1 CAS・1 Undo・derived effect の
  観測面が一致する。

## Non-goals

- 実 Luau runtime での script 実行、VM の RT Callback 上での
  実行、実 Luau.Analysis native FFI（ruviform#560 領域）。
- CLI `action run --file` 入口（sv#262）・Script Builder
  （#273）・editor（#274）・GUI/CLI 実測・consumer E2E
  （#267）・Rhahi backend の正式対応。
- `bindings[]`/create-then-reference の script 表現と
  gateway bindings fallback の昇格（#263 deferred のまま）。
- `ScriptActionDefinition` からの script source compile・
  Plan 化（#273）・Workflow への自動降格ロジック。
- 独自 Capability/Permission/Registry・独自認可・独立
  Action executor/run surface・Job 正本の作成。

## Risk

- digest 連鎖のいずれかが崩れると source/version/binding
  drift を検出できず、stale recipe が Gateway へ流通する。
- batch 分類規則の抜けは、atomicity を保証できない呼出を
  Composite commit へ混入させるか、正当な batch を過剰に
  reject する。
- `compile_action_recipe` が literal-only step に限定する
  ため、将来 script 側に `$output.` binding を導入する際は
  本 bridge の再設計が必要になる（現行は #263 deferred と
  整合）。
- automation `RegistrySnapshot` が serialized mirror である
  ため、host が live registry との同期を怠ると compile が
  drift を typed reject する — fail-closed だが運用上の
  mirror 生成経路は本 slice の範囲外。
