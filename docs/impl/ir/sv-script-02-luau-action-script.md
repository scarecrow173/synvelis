---
id: "IMPL-034XjAbWQJoR9lTEETMuYS"
type: "impl"
status: "completed"
title: "SV-SCRIPT-02 Luau Action Script定義・Typed Action Recipe・Gateway/Atomic Batch統合"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/272"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-script/src/action.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/action_recipe.rs"
      - type: "file"
        path: "crates/synvelis-script/tests/action.rs"
      - type: "file"
        path: "crates/synvelis-automation/tests/action_recipe.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/script_action.rs"
      - type: "file"
        path: "docs/impl/exp/sv-script-02-luau-action-script.md"
      - type: "file"
        path: "docs/test-specs/sv-script-02-luau-action-script.md"
      - type: "file"
        path: "evidence/sv272/sv-script-02-luau-action-script-ledger.md"
    modified:
      - type: "file"
        path: "crates/synvelis-script/src/binding.rs"
      - type: "file"
        path: "crates/synvelis-script/src/error.rs"
      - type: "file"
        path: "crates/synvelis-script/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-automation/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-automation/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/gateway.rs"
      - type: "file"
        path: "crates/synvelis-gateway/Cargo.toml"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
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
    - "docs/impl/exp/sv-script-02-luau-action-script.md"
    - "docs/test-specs/sv-script-02-luau-action-script.md"
    - "evidence/sv272/sv-script-02-luau-action-script-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/automation/design-script.md"
    - "docs/automation/design-atomic-macro.md"
    - "docs/automation/design-capability-action.md"
    - "docs/impl/ir/sv-script-01-script-api-model.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-script-02-luau-action-script.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/sv-script-02-luau-action-script.md"
    rejected: []
---

# SV-SCRIPT-02 Luau Action Script定義・Typed Action Recipe・Gateway/Atomic Batch統合

## Summary

Issue #272 の headless slice を実装した。`synvelis-script` に
`ScriptActionDefinition`（script id/version・language/backend
要求・source digest・schema pin）と `ActionRecipe`/`RecipeSpec`/
`RecipeMember` を追加し、SV-SCRIPT-01 の `ScriptApiModel`・
`BindingSet`・digest 契約の上で typed recipe を構築する。
`Single` recipe は `synvelis.operation.prepare` 包含の
`InvocationDraft`（`ProjectMutation`）または直接 capability
draft（それ以外）へ、`Batch` recipe（2〜64 の単一
project/consistency の `ProjectMutation` member）は
`synvelis-automation` の新設 `compile_action_recipe` が
AUTX-04 `MacroCompiler` をそのまま通して flat `ops[]` へ
lower し、`ActionRecipe::to_batch_draft` が prepare envelope
へ wrap する。Gateway 側は read-only `schemas()` accessor の
追加と e2e golden のみ — `Gateway::handle`（parse → bind →
invoke）・prepare・proposal・apply・`CommitGate` commit・undo
の既存経路を一切変更していない。非空 `bindings[]`
（create-then-reference）は script recipe が生成しない形の
ため、sv#263 deferred の gateway bindings fallback 昇格は
**不要と判断し Deferred**（意味論は #263 所有のまま）。

## Context

- 対象 Issue: <https://github.com/scarecrow173/synvelis/issues/272>
  （OPEN・PR 本文は `Refs #272`・Close しない）
- Canonical 契約: `docs/automation/design-script.md`
  （Script は authority を持たず独立 run surface を持たない）、
  `docs/automation/design-atomic-macro.md`（AUTX-04 単一
  Batch/Composite 経路）、`docs/automation/design-capability-action.md`
- 消費する先行実装: SV-SCRIPT-01（#271/PR #435 —
  `ScriptApiModel`/`BindingSet`/digest）、AUTX-04（#263 —
  `MacroCompiler`/`CompiledMacro::batch_args`）、AUTX-06
  （#265 — Workflow Coordinator への誘導先）、#433
  `Composite::validate` sequential dry-run
- 実装記録の型式は `docs/impl/ir/sv-script-01-script-api-model.md`
  に倣う。

## Implementation

### Changes

- `crates/synvelis-script/src/action.rs`（新規）:
  - `ScriptActionDefinition` — `schema_version`/`script_id`/
    `version`/`language_id`/`backend_requirement`/
    `source_digest`/`input_schema`/`output_schema`。
    `validate` が version・id/digest 形状・language↔backend
    整合・backend 可用性・schema 解決を検査し、`digest()` が
    canonical JSON+SHA-256 を返す。`verify_source` が source
    digest 不一致を `SourceDrift` で fail-closed にする。
  - `AvailableBackends`/`LanguageId`/`BackendRequirement` —
    Luau 固定にしない型設計（Rhai は型上受理可能だが本
    slice の検証は Luau backend のみ）。
  - `RecipeSpec`/`RecipeMemberSpec` — member 呼出しの typed
    入力（script API 名・args・明示 target・project・
    consistency）と request 級 field（`request_id`/`task`/
    `grant`/`idempotency_key` は opaque 参照で authority を
    表現不能）。
  - `ActionRecipe::build` — definition 検証 → binding↔model
    drift → member 数 bound（1 または 2〜64）→ member 毎に
    `ScriptApiModel::resolve_call`（assemble_call と同一の
    name/verdict/target/consistency/args 検査）→ batch 分類
    （単一 project・単一 consistency・`ProjectMutation` のみ・
    nested batch 拒否）。失敗は全て typed `ScriptApiError`。
  - `ActionRecipe::{kind,expected_revision,digest,
    verify_definition,to_draft,to_batch_draft}` — `Single` は
    prepare 包含または直接 draft、`Batch` は compile 済み
    `batch_args` を prepare envelope へ wrap。`recipe_digest`
    は definition/binding/model digest・members・project・
    consistency・request field を含む canonical payload の
    SHA-256。
- `crates/synvelis-script/src/binding.rs`: `resolve_call`
  （`ResolvedCall` — capability/domain/schema/target/args/
  consistency の解決済み部品）を `assemble_call` から抽出し
  `pub(crate)` 化、`InvocationDraft::submit`（draft →
  `Gateway::handle` の最小接続）を追加。
- `crates/synvelis-script/src/error.rs`: `ScriptApiError` に
  definition/recipe 系 variant を追加（`Definition`・
  `UnsupportedDefinitionVersion`・`LanguageBackendMismatch`・
  `BackendUnavailable`・`UnresolvedDefinitionSchema`・
  `SourceDrift`・`DefinitionMismatch`・`VersionDrift`・
  `EmptyRecipe`・`TooManyMembers`・`UnsupportedMemberDomain`・
  `NestedBatch`・`CrossProject`・`ConsistencyMismatch`・
  `UnknownApi`・`NotInvocable`・`RecipeKindMismatch` 等）。
- `crates/synvelis-script/src/lib.rs`: `action` module と
  全公開型を export。
- `crates/synvelis-automation/src/action_recipe.rs`（新規）:
  `compile_action_recipe` — batch `ActionRecipe` を
  `AtomicMacroDefinition`（literal-only step `m0..mN`、
  exact `capability@major.minor` pin、literal args/target）へ
  翻訳し、変更なしの `MacroCompiler::compile` を実行する。
  availability・domain/execution・schema・target・effect/
  data-access ceiling・nested batch・64 件 bound の検査は
  全て compiler 側に残る。非 batch recipe は
  `AutomationError::Definition` で拒否。`output_bindings`
  は空 — recipe member は解決済みの具体値のみを持ち、
  create-then-reference binding 意味論は生成しない
  （AUTX-04/GW-04 所有のまま）。
- `crates/synvelis-automation/src/lib.rs`・`Cargo.toml`:
  module export と `synvelis-script` path 依存を追加。
- `crates/synvelis-gateway/src/gateway.rs`: read-only
  `Gateway::schemas()` accessor を追加（`RegistrySnapshot::
  capture` が live registry pair を read-only で投影するため
  に必要。registry 変更面は増やさない）。
- `crates/synvelis-gateway/Cargo.toml`: dev-dependencies に
  `synvelis-script`/`synvelis-automation` を追加
  （dev-dep cycle は Cargo が許容する test-only 結合）。
- `Cargo.lock`: 上記依存辺の再解決（生成物・手編集なし）。

### Decisions During Implementation

- **`resolve_call` の抽出**: `assemble_call`（draft 組立）と
  `ActionRecipe::build`（recipe member 解決）が同一の
  name→verdict→target→consistency→args 検査を必要とするため、
  検査本体を `pub(crate)` の `resolve_call` に集約した。
  policy/validation の authoritative 実装を一箇所に保つ
  （AGENTS「正しい境界で修正する」）。
- **bindings fallback は昇格しない**: Issue の条件付き昇格は
  「script recipe が非空 `bindings[]` を生成する場合に限り」
  必要とされていた。recipe member は常に解決済みの具体
  args/target を持ち、`compile_action_recipe` は
  `output_bindings: Vec::new()` を生成する — create-then-
  reference は script action の表現外。merged 済み経路は
  既に `bindings` key を schema 受理するが非空時の
  `PreparationRejected` は #263 の deferred のまま維持し、
  本 slice では gateway の bindings 意味論を変更しない。
  PR 本文と evidence に Deferred として記録する。
- **`submit` の追加位置**: draft→実経路の接続は script 側の
  `InvocationDraft::submit`（`Gateway::handle` 呼出）に留め、
  Gateway には `schemas()` read accessor のみ追加した。
  `bind`/`invoke`/handle の意味論・認可・receipt は無変更。
- **automation snapshot は serialized mirror**: AUTX-04 の
  `RegistrySnapshot` は live registry からの投影ではなく
  host が供給する宣言的 mirror（`parse_registry_snapshot_yaml`）。
  e2e では gateway op table と同値の YAML mirror を fixture
  として用意し、2 つの snapshot view の不一致が typed reject
  になることを層状 drift 検出として利用した。
- **`ScriptConsistency` → wire**: recipe consistency は
  `ExpectedRevision`/`Latest`/`Unconditional` の script 側
  enum で保持し、draft lowering 時に `to_gateway` → serde
  wire 形へ落とす。batch recipe は全 member が同一
  consistency を共有することを build 時に強制する。

## Related Experiments

### Adopted

- `docs/impl/exp/sv-script-02-luau-action-script.md` — E1
  recipe 分類規則（domain/consistency/project の batch 適格
  全有か無か）、E2 literal-only `MacroStep` 翻訳と
  `bindings[]` 非生成、E3 `InvocationDraft::submit` +
  `Gateway::schemas()` の最小接続、E4 serialized mirror
  snapshot の fixture 利用。4 試行すべて採用。

### Rejected

- domain 混在 batch の Workflow 自動降格（atomicity を
  保証できない呼出を部分実行しないため typed reject）。
- `$output.` placeholder の script 写像（#263 所有の
  binding 意味論の重複実装になるため）。
- Gateway 側の draft 専用 bind/invoke variant（`handle`
  が既に正本経路であり二重経路を生むため）。
- live registry → automation snapshot の投影関数新設
  （AUTX-04 の serialized・source-agnostic 契約を越える
  ため）。

## Validation

`docs/test-specs/sv-script-02-luau-action-script.md` の
Rules/Examples に対応する検証を実施し、全て green:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check/lint/test synvelis-script` — Pass
  （check `--locked --all-targets` exit 0、lint clippy
  warning 0、test 78 件: action 22・analyzer 7・binding 13・
  golden 10・luau 9・model 17）
- `mise run check/lint/test synvelis-automation` — Pass
  （test 125 件: action_recipe 9・compiler 43・coordinator
  17・decode 11・determinism 12・macros 17・yaml_subset
  15・lib 1）
- `mise run check/lint/test synvelis-gateway` — Pass
  （test 27 binaries・437 件・0 failure。新規
  `tests/script_action.rs` 4 件を含む）
- dependent-crate 非回帰: `mise run check
  synvelis-cli`・`mise run check synvelis-workspace` —
  Pass（exit 0）
- 文書: `doc-status` lint・`doc-maintenance` index apply
  （触った dir の index 再生成）

補助証拠（canonical = mise）: `cargo test -p
synvelis-script --test action`・`cargo test -p
synvelis-automation --test action_recipe`・`cargo test -p
synvelis-gateway --test script_action` の直接実行も同一結果。
`mise run gw10`・`mise run test-rt`・`*-msrv` は正規レーン外
のため NotRun。

詳細な case 対応表は
`evidence/sv272/sv-script-02-luau-action-script-ledger.md`。

## Risks

- digest 連鎖（definition/binding/model/recipe）のいずれかが
  崩れると drift 検出が効かず stale recipe が Gateway へ
  流通する — `verify_definition`/`StaleArtifact`/`SourceDrift`
  の fail-closed で担保するが、digest 仕様の変更は生成物
  全体の再検証を要する。
- batch 分類規則は「atomicity を保証できる形」のみを
  Composite へ lower する。将来 script 側に create-
  then-reference を導入する場合、`compile_action_recipe`
  の literal-only 前提と gateway の非空 `bindings`
  `PreparationRejected`（#263 deferred）の両方が再検討
  対象になる。
- automation `RegistrySnapshot` は serialized mirror で
  あり、live registry との同期は host 責務。mirror が
  古いと compile が typed reject する（fail-closed）が、
  投影生成経路は本 slice の範囲外。

## Follow-ups

- sv#262: CLI `action run --file` 入口 — `ActionRecipe`/
  `InvocationDraft`/`submit` の consumer 側配線。
- sv#273: Script Builder/Plan 化 — `ScriptActionDefinition`
  からの compile・Plan 導出。
- sv#274: editor 統合。
- sv#267: 製品 E2E 受入（GUI/CLI/実 OS）。
- ruviform#560: 実 Luau.Analysis/VM 実行ゲート —
  analyzer port の実装差し替え。
- #263 deferred: gateway batch bindings fallback の昇格
  （script recipe が非空 `bindings[]` を生成する形が
  将来導入された場合に再評価）。
