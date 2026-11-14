# SV-SCRIPT-02 Luau Action Script → Recipe → Gateway/Atomic Batch — Case Ledger（synvelis#272 headless slice）

Date: 2026-11-14。Slice: `synvelis-script` に
`ScriptActionDefinition`/`ActionRecipe`/`RecipeSpec` を追加し、
SV-SCRIPT-01 の `ScriptApiModel`/`BindingSet`/digest 契約の上で
typed Action Recipe を構築。`Single` recipe は prepare 包含
`InvocationDraft`（`ProjectMutation`）または直接 capability
draft（それ以外）へ lower し、`InvocationDraft::submit` が
`Gateway::handle`（parse→bind→invoke）の実経路へ接続する。
`Batch` recipe（2〜64 の単一 project/consistency の
`ProjectMutation`）は `synvelis-automation` の新設
`compile_action_recipe` が変更なしの AUTX-04 `MacroCompiler`
を通して flat `ops[]` へ lower し、`to_batch_draft` が
`synvelis.operation.prepare` envelope へ wrap する。
Gateway 側の変更は read-only `schemas()` accessor と
dev-dependency のみ — `bind`/`invoke`/handle・prepare・
proposal・apply・`CommitGate`・undo の意味論は無変更。
**本 slice は headless の compile-time/golden 契約のみ**を
対象とし、実 Luau runtime 実行・実 Luau.Analysis・CLI
`action run --file`（sv#262）・editor（#274）・consumer
E2E（#267）・GUI/CLI 実測は NotRun/Deferred である。
headless の Pass を runtime/実機の Pass に昇格しない。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/272
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis branch | `feat/272-svscript02-luau-action-script`（worktree `sv272-script2`） |
| OS / CPU / FS | Windows 11 / Intel Core i9-9900K / NTFS（C:） |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024、MSRV 1.89 |
| Build | mbx managed target（worktree `target` リンク先） |
| Registry input | script 側: in-memory fixture（`tests/common/mod.rs`）と live `fixture_rig()` registry capture の双方。automation 側: serialized `RegistrySnapshot` YAML mirror（AUTX-04 契約どおり host 供給の宣言）。Gateway e2e: `fixture_rig()` live registry |

## 判定語彙

- **Pass**: 当該 SHA で検証が exit 0 で完了し、対象
  test/assertion が green だったことを実測した。
- **NotRun**: 本 slice で実行していない項目（後続 slice・
  MSRV lane・実機/実 Analyzer 結合）。
- **Deferred**: 別 Issue/別リースが Owner の項目
  （sv#262 CLI 入口・#273 Plan 化・#274 editor・#267 consumer
  E2E・ruviform#560 実 Analyzer・#263 deferred bindings
  fallback）。
- **Unsupported**: 本 slice の契約上サポートしない形
  （非空 `bindings[]` の script 生成・Rhai backend の実検証・
  VM の RT Callback 実行）。

## 実行した正規検証

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0） |
| `mise run check synvelis-script` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-automation` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-gateway` | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-script` | Pass（exit 0、clippy warning 0） |
| `mise run lint synvelis-automation` | Pass（exit 0、clippy warning 0） |
| `mise run lint synvelis-gateway` | Pass（exit 0、clippy warning 0） |
| `mise run test synvelis-script` | Pass（78 test: action 22・analyzer 7・binding 13・golden 10・luau 9・model 17。doc-test 0） |
| `mise run test synvelis-automation` | Pass（125 test: action_recipe 9・compiler 43・coordinator 17・decode 11・determinism 12・macros 17・yaml_subset 15・lib 1。doc-test 0） |
| `mise run test synvelis-gateway` | Pass（27 test binaries・437 test・0 failure。script_action 4 を含む。doc-test 0） |
| `mise run check synvelis-cli` | Pass（exit 0、dependent-crate 非回帰） |
| `mise run check synvelis-workspace` | Pass（exit 0、dependent-crate 非回帰） |
| `node scripts/doc_status.js lint`（doc-status Skill） | Pass（blocking findings 0 — index 適用後に再計測） |
| `node scripts/doc_maintenance.js index apply`（doc-maintenance Skill） | Pass（触った dir の index を再生成） |

補助証拠（canonical = 上記 mise task）: `cargo test -p
synvelis-script --test action`・`cargo test -p
synvelis-automation --test action_recipe`・`cargo test -p
synvelis-gateway --test script_action` の直接実行も同一結果。
`mise run gw10`・`mise run test-rt`・`*-msrv` 系は本 slice の
正規レーン外のため NotRun。

## 受入条件 → case 対応表

### AC-1 Luau Action Script → CapabilityRef/Version 紐付 Recipe → Gateway Policy/Receipt/Commit

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| ACT-G01 definition 検証 | valid definition が validate+digest、version/id/digest/language/backend/schema 不備が typed reject | 22 test 全 green | Pass | `tests/action.rs` definition 系 |
| ACT-G02 source/version drift | `verify_source`/`verify_definition` が digest・version drift を typed reject | `SourceDrift`/`VersionDrift`/`DefinitionMismatch` | Pass | `tests/action.rs` |
| ACT-G03 single mutation | `ProjectMutation` 1 件が prepare 包含 draft へ lower・`verify_wire` 通過・args が schema 検証済み | prepare capability pin・operation triple・i64 decode | Pass | `tests/action.rs` `single_mutation_lowers_to_prepare_draft` |
| ACT-G04 single query/runtime | 非 mutation member が直接 capability draft へ lower | `synvelis.query.*`/`synvelis.transport.*` pin・domain 別 consistency | Pass | `tests/action.rs` |
| ACT-G05 実経路 e2e | draft → `Gateway::handle` → prepare → proposal → apply → commit → undo | single/batch とも commit revision 1→2・undo/redo | Pass | `tests/script_action.rs` |

### AC-2 2〜64 ProjectMutation の Batch/Composite lower・65件/Nested/Cross-project/Query/Async 拒否

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| BAT-G01 batch 分類 | 2〜64 の単一 project/consistency `ProjectMutation` member が `Batch` kind | kind=Batch・4 digest pin・決定的 recipe_digest | Pass | `tests/action.rs` `batch_recipe_collects_project_mutation_members` |
| BAT-G02 batch lower | `compile_action_recipe` が literal-only `MacroStep` として `MacroCompiler` を通り flat `ops[]` を生成 | ops 2 件・bindings 空・expected_revision echo | Pass | `tests/action_recipe.rs`・`tests/script_action.rs` |
| BAT-G03 Composite commit | batch draft → prepare → `ProjectOp::Composite`・2 commands・1 CAS・1 Undo | mute+move が原子的に commit/復元 | Pass | `tests/script_action.rs` `batch_recipe_commits_as_one_composite_and_undoes_once` |
| BAT-G04 65 件拒否 | 65 member recipe・65 ops batch_args が typed reject | `TooManyMembers`（recipe）・schema violation（gateway 再検証） | Pass | `tests/action.rs` `oversized_recipe_rejects`・`tests/script_action.rs` `gateway_revalidation_rejects_what_analysis_accepted` |
| BAT-G05 nested 拒否 | `synvelis.operation.batch` を member に持つ recipe/compile が typed reject | `NestedBatch`（script）・`CompileError::NestedBatch`（automation） | Pass | `tests/action.rs` `nested_batch_member_rejects`・`tests/action_recipe.rs` `nested_batch_capability_fails_closed` |
| BAT-G06 cross-project 拒否 | 異 project member 混在が typed reject | `CrossProject` | Pass | `tests/action.rs` `cross_project_members_reject` |
| BAT-G07 Query/Async 拒否 | 非 `ProjectMutation` domain 混在が typed reject・Workflow 誘導の typed verdict | `UnsupportedMemberDomain`・`DomainMismatch`/`ExecutionMismatch` | Pass | `tests/action.rs` `non_mutation_member_rejects_batch`・`tests/action_recipe.rs` `non_mutation_domain_member_fails_closed` |
| BAT-G08 consistency 不一致 | member 間 consistency 相違が typed reject | `ConsistencyMismatch` | Pass | `tests/action.rs` `inconsistent_consistency_rejects` |

### AC-3 Preview/Commit の ID/DerivedEffect/Revision/1 Undo の #263 独立 Oracle 一致

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| ORA-G01 Composite oracle | retained record が 2 commands・`Composite` op・base revision 1・順序どおりの effect | commands `[set-clip-muted, move-clip]`・`ProjectOp::Composite`・effects on clip11 | Pass | `tests/script_action.rs`（`record_for`/`effect` は #263 と同一 rig helper） |
| ORA-G02 commit revision | expected-revision 1 → commit revision 2・receipt base/revision | receipt `base=1`・`revision=2` | Pass | `tests/script_action.rs` |
| ORA-G03 effect 着地 | mute+anchor が live project に反映 | `muted==true`・`TimeAnchor::Musical(960)` | Pass | `tests/script_action.rs` |
| ORA-G04 1 Undo | 1 回の undo で composite 全体が原子的に復元・redo で再適用 | undo → muted=false・anchor 復元・redo → muted=true | Pass | `tests/script_action.rs` |

### AC-4 Source/Version drift・Invalid Target・Revoke・Indeterminate・Unknown Schema の typed fail-closed

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| FAIL-G01 drift 系 | source/version/definition/binding/model drift が typed reject | `SourceDrift`/`VersionDrift`/`DefinitionMismatch`/`StaleArtifact` | Pass | `tests/action.rs` |
| FAIL-G02 invalid target | target 欠落・kind 不一致・tagged id 不正が typed reject | `GatewayError`/`TargetKindMismatch` | Pass | `tests/action.rs`・`tests/action_recipe.rs` |
| FAIL-G03 unknown/hidden/disabled | 未知名・hidden・非 invocable entry が recipe member にならない | `UnknownApi`/`NotInvocable` | Pass | `tests/action.rs` `member_resolution_failures_stay_typed` |
| FAIL-G04 schema 違反 | args の schema 違反が typed reject（script 層 + gateway 再検証の双方） | `Gateway(SchemaViolation)`・`CompileError::SchemaViolation`・gateway 65 ops reject | Pass | `tests/action.rs`・`tests/action_recipe.rs`・`tests/script_action.rs` |
| FAIL-G05 revoke/indeterminate | grant 取消・indeterminate verdict が commit を止める既存契約の非回帰 | gateway 全 suite（proposal/commit_gate/authorization）green | Pass | `mise run test synvelis-gateway` 437 test |

### AC-5 Script 保存・型解析・CLI フラグで Human 権限/Grant を獲得しない・VM を RT Callback 上で実行しない

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| SEC-G01 authority 非含有 | recipe/draft/definition の型に authority field が存在しない・`task`/`grant` は opaque 参照 | serde `deny_unknown_fields`・型に principal/grant 実体なし | Pass | `action.rs`/`binding.rs` 型定義・`tests/action.rs` |
| SEC-G02 認可は Gateway のみ | agent+grant 経路は grant による capability 認可を要求し、batch capability 自体も grant 対象 | grant 無し/範囲外は `Denied{PrepareProposal}`（実測: batch capability 追加で解消） | Pass | `tests/script_action.rs`（初回 denied → grant に `synvelis.operation.batch` 追加で commit） |
| SEC-G03 human/agent 分離 | human authority は grant 無しで query を許可、agent は task+grant 必須 | `query_member_lowers_to_direct_draft_and_invokes` が human() で Completed | Pass | `tests/script_action.rs` |
| SEC-G04 VM 非 RT 実行 | 本 slice に VM 実行経路が存在しない（型境界のみ） | `synvelis-script`/`synvelis-automation` に executor/VM 呼出なし | Pass | コードレビュー＋依存 DAG（script→gateway、automation→gateway+script のみ） |

## NotRun / Deferred / Unsupported 台帳

| 項目 | 判定 | 理由・後続検証 |
| --- | --- | --- |
| 実 Luau runtime での Action Script 実行 | NotRun | rf#560 bridge は PoC 級・製品接続は統合 Gate。後続: 実 VM 実行ゲート |
| 実 Luau.Analysis native FFI/実測 | Deferred | ruviform#560 領域。analyzer port は SV-SCRIPT-01 の fake 契約のみ |
| CLI `action run --file` 入口・GUI/CLI 実測 | Deferred | sv#262（CLI）・#267（consumer E2E）。本 slice は headless のみ |
| Script Builder（#273）・editor（#274） | Deferred | 後続 Issue |
| Rhai backend の正式対応 | Unsupported | 型設計は `LanguageId`/`BackendRequirement` を Luau 固定にしていないが、本 slice の検証は Luau backend のみ |
| 非空 `bindings[]`（create-then-reference）の script 生成 | Unsupported | recipe member は解決済み具体値のみ。#263 deferred の gateway fallback 昇格条件に非該当 → **Deferred**（意味論は #263 所有） |
| `mise run gw10`・`mise run test-rt` | NotRun | ハーネス非回帰の任意確認。本 slice の正規レーン外 |
| `*-msrv` レーン | NotRun | ユーザー指定により実行しない |
| live registry → automation `RegistrySnapshot` 投影 | Deferred | AUTX-04 snapshot は serialized mirror。host 側の投影生成経路は本 slice 範囲外。e2e は同値 YAML mirror fixture で代替 |

## Fail-closed verdict 台帳（RF-SCRIPT-00 語彙対応）

| Case | verdict/型 | 判定 |
| --- | --- | --- |
| 未知 script API 名 | `UnknownApi` | Pass |
| hidden/disabled/planned/retired/unsupported-version/未解決 schema の member | `NotInvocable`（model verdict 経由） | Pass |
| definition version 不一致 | `UnsupportedDefinitionVersion` | Pass |
| source digest 不一致 | `SourceDrift` | Pass |
| definition/binding/model drift | `DefinitionMismatch`/`VersionDrift`/`StaleArtifact` | Pass |
| Query/Async/RuntimeControl の batch 混在 | `UnsupportedMemberDomain` → Workflow 誘導（F-09 相当の typed reject） | Pass |
| nested batch | `NestedBatch` | Pass |
| cross-project | `CrossProject` | Pass |
| consistency 不一致 | `ConsistencyMismatch` | Pass |
| member 数 0/65 超 | `EmptyRecipe`/`TooManyMembers` | Pass |
| 認可なし agent 呼出 | `GatewayError::Denied{PrepareProposal}` | Pass |
| schema 違反（script 層/automation 層/gateway 層） | `Gateway(SchemaViolation)`/`CompileError::SchemaViolation`/wire schema reject | Pass |
| 非空 `bindings[]` の script 生成 | 表現不能（`output_bindings: []` 固定） | Pass（Unsupported として記録） |
