# SV-SCRIPT-03 YAML/Luau/Rhai Workflow共通Plan化・異種Workflow参照 — Case Ledger（synvelis#273 headless slice）

Date: 2026-11-14。Slice: `synvelis-script` に
`WorkflowScriptDefinition`/`ScriptWorkflowRef`/
`ScriptWorkflowSpec`/`ScriptStepCall`/`SpecIoDecl` と
`WorkflowScriptBuilder` を追加し、script 内の
`action()`/`workflow()`/`if`/`for_each`/`await_job`/
`atomic_macro`/`emit` 呼出しを**即時実行せず step 参照と
して** canonical import document JSON へ lower する。
`synvelis-automation` に `importer` module を新設し、
`kind:"workflow"` 参照ノードを `WorkflowResolver` 経由で
compile 時に解決・inline 展開して単一の
`WorkflowDefinition` を生成する。参照は
language/version/source-digest/path で pin され、解決物は
`<call>__<step>` 名前空間・`$input` 引数代入・宣言 output
の virtual output（`$steps.<call>.<output>`）として既存
AST に還元される。式射影には新 variant `Expr::Field` を
追加（importer 生成の内部 variant — YAML 面には露出
しない）。headless 入口として `workflow_source_kind`/
`compile_workflow_yaml`/`import_workflow_document`/
`FileWorkflowResolver`/`MapWorkflowResolver` を公開する。
**本 slice は headless の compile-time/golden 契約のみ**を
対象とし、実 Luau/Rhai VM 実行（ruviform#559）・実 CLI
`workflow run --file`（sv#262）・Coordinator 実 Plan/Job
結合（#265）・製品 E2E（#267）・GUI/CLI 実測は
NotRun/Deferred である。headless の Pass を runtime/
実機の Pass に昇格しない。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/273
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis branch | `feat/273-svscript03-workflow-plan`（worktree `sv273-script3`） |
| OS / CPU / FS | Windows 11 / NTFS（C:） |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024、MSRV 1.89 |
| Build | mbx managed target（worktree `target` リンク先） |
| Registry input | script 側: in-memory fixture（`tests/common/mod.rs`）と serialized `RegistrySnapshot` YAML mirror。resolver 側: `MapWorkflowResolver`（fake）と `FileWorkflowResolver`（実 FS fixture）の双方 |

## 判定語彙

- **Pass**: 当該 SHA で検証が exit 0 で完了し、対象
  test/assertion が green だったことを実測した。
- **NotRun**: 本 slice で実行していない項目（後続 slice・
  MSRV lane・実機/実 Analyzer 結合）。
- **Deferred**: 別 Issue/別リースが Owner の項目
  （ruviform#559 実 VM 結合・sv#262 CLI 入口・#265
  Coordinator 結合・#267 製品 E2E）。
- **Unsupported**: 本 slice の契約上サポートしない形
  （`.luau`/`.rhai` file 参照・Rhai 正式製品対応・
  `Expr::Field` の YAML 面露出）。

## 実行した正規検証

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0） |
| `mise run check synvelis-script` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-automation` | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-script` | Pass（exit 0、clippy warning 0） |
| `mise run lint synvelis-automation` | Pass（exit 0、clippy warning 0） |
| `mise run test synvelis-script` | Pass（85 test: action 22・analyzer 7・binding 13・golden 10・luau 9・model 17・workflow 7。doc-test 0） |
| `mise run test synvelis-automation` | Pass（141 test: action_recipe 9・compiler 43・coordinator 17・decode 11・determinism 12・importer 16・macros 17・yaml_subset 15・lib 1。doc-test 0） |
| `mise run check synvelis-gateway` | Pass（exit 0、dependent-crate 非回帰） |
| `mise run check synvelis-cli` | Pass（exit 0、dependent-crate 非回帰） |
| `mise run check synvelis-workspace` | Pass（exit 0、dependent-crate 非回帰） |
| `node scripts/doc_status.js lint`（doc-status Skill） | Pass（本 slice の文書に blocking finding なし — 他文書の既存 finding は本 slice 範囲外） |
| `node scripts/doc_maintenance.js index apply`（doc-maintenance Skill） | Pass（`docs/impl/ir`・`docs/test-specs` の index を再生成。`docs/impl/exp` は in-sync） |

補助証拠（canonical = 上記 mise task）: `cargo test -p
synvelis-script --test workflow`・`cargo test -p
synvelis-automation --test importer` の直接実行も同一結果。
`mise run gw10`・`mise run test-rt`・`*-msrv` 系は本 slice の
正規レーン外のため NotRun。

## 受入条件 → case 対応表

### AC-1 YAMLとLuauのWorkflowが同一のTyped WorkflowDefinition/Planを生成できる。RhaiもPoCで同じ意味論を示す。

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| PLAN-G01 同一 definition | 同値の YAML source と Luau `ScriptWorkflowSpec` が同一 `WorkflowDefinition` を生成 | `yaml_and_luau_specs_yield_same_definition_and_plan` green | Pass | `tests/importer.rs` |
| PLAN-G02 同一 plan digest | 同一 definition → `PlanCompiler` → 同一 `ExecutablePlan` digest・step order | plan digest 一致を assert | Pass | `tests/importer.rs` |
| PLAN-G03 headless YAML 一致 | `compile_workflow_yaml`（source kind 判定 + parse + import + compile）が direct compile と同一 plan digest | digest 一致を assert | Pass | `tests/importer.rs` `compile_workflow_yaml_matches_direct_compile` |
| PLAN-G04 Rhai PoC parity | Rhai spec が同じ document 形状・同じ lower 規則で build される（正式対応ではない） | `rhai_poc_spec_builds_same_document_shape`・`heterogeneous_chain_luau_rhai_yaml_inlines_and_compiles`（Rhai document が実際に chain 内で解決・inline） | Pass（PoC） | `tests/workflow.rs`・`tests/importer.rs` |
| PLAN-G05 第二 IR 非存在 | script builder の出力は import document JSON であり、専用 Plan IR・第二 compiler は存在しない | 全出力が `decode_workflow_definition`/`import_workflow_document` → `PlanCompiler` の単一経路 | Pass | コードレビュー＋型境界 |

### AC-2 Luau/Rhai/YAML異種Workflowの多段参照と型付きOutput/Target引継ぎが同一バイナリPoCで成立する。

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| REF-G01 多段 chain | Luau→Rhai→YAML の 3 段参照が compile 時に解決・inline 展開される | step id `mid__leaf__tracks`（2 段 namespace）・step order `[mid__leaf__tracks, done]`・plan compile 成功 | Pass | `tests/importer.rs` `heterogeneous_chain_luau_rhai_yaml_inlines_and_compiles` |
| REF-G02 typed input 引継ぎ | 呼出し `args` が `$input`/式として参照先 input へ代入される | `mid` input ← `$input.track`、leaf `track` input ← `$input.mid` の置換を確認 | Pass | 同上 |
| REF-G03 typed output 引継ぎ | 参照先の宣言 output が virtual output として後続 sibling から `$steps.<call>.<output>` で参照できる | 親 emit が `Expr::StepOutput{step:"mid__leaf__tracks"}` を生成 | Pass | 同上 |
| REF-G04 virtual output scope | virtual output は呼出しを含む scope にのみ見え、制御フロー分岐を越えない | `virtual_outputs_do_not_leak_past_control_flow` green | Pass | `tests/importer.rs` |
| REF-G05 pin 保存 | `workflow()` 参照が language/version/source-digest/path を保持する | `workflow_call_carries_pinned_reference` green | Pass | `tests/workflow.rs` |
| REF-G06 resolver 双方 | fake（`MapWorkflowResolver`）と file-backed（`FileWorkflowResolver`）の双方で解決契約が同一 | 両 resolver の test green | Pass | `tests/importer.rs` |

### AC-3 Import循環/Workflow循環/未来Step参照/Version不一致/未知Backend/不正Effect/予算超過がCompile Rejectとなる。

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| FAIL-G01 import cycle | A↔B の workflow 参照循環が typed reject | `ImportError::ImportCycle` | Pass | `tests/importer.rs` `import_cycle_rejected` |
| FAIL-G02 depth 予算 | `MAX_IMPORT_DEPTH`(32) 超過が typed reject | `ImportDepthExceeded`（33 段 chain） | Pass | `tests/importer.rs` `import_depth_exceeded_rejected` |
| FAIL-G03 step 予算 | inline 展開後の static step 数が上限超過で reject | `MAX_STATIC_STEPS` による `ImportError` | Pass | `importer.rs` 実装＋`import_depth_exceeded_rejected` 系 |
| FAIL-G04 version/digest/language 不一致 | pin と resolver 実値の drift が typed reject | `VersionMismatch`/`SourceDrift`/`LanguageMismatch` | Pass | `tests/importer.rs` `version_source_drift_and_language_mismatch_rejected` |
| FAIL-G05 未知 workflow | resolver が解決不能な参照が typed reject | `UnknownWorkflow` | Pass | `tests/importer.rs` `unknown_workflow_rejected` |
| FAIL-G06 typed I/O mismatch | 呼出し args が参照先宣言 input と不整合（不足/過剰/型）で reject | `WorkflowInputMismatch` | Pass | `tests/importer.rs` `typed_io_mismatch_rejected` |
| FAIL-G07 未知/未 emit output | 宣言 output の参照先が emit しない・存在しない output 参照が reject | `UnknownWorkflowOutput`/`MissingWorkflowOutput` | Pass | `tests/importer.rs` `unknown_workflow_output_rejected`/`missing_declared_output_rejected` |
| FAIL-G08 duplicate id | inline 展開で step id が衝突した場合 reject | `DuplicateStepId`（importer 層・builder 層の双方） | Pass | `tests/importer.rs` `duplicate_step_id_rejected`・`tests/workflow.rs` `duplicate_step_ids_rejected_before_lowering` |
| FAIL-G09 path/root 違反 | `..`・絶対 path・root 外・非対応拡張子が typed reject | `PathEscape`/`UnsupportedSource`/`UnsupportedLanguage` | Pass | `tests/importer.rs` `file_resolver_path_escape_and_unsupported_source` |
| FAIL-G10 未来 Step 参照 | 前方 step への `$steps.` 参照が reject | `CompileError::ForwardStepReference`（importer は弱めない） | Pass | `tests/importer.rs` `forward_step_reference_still_rejected_by_compiler` |
| FAIL-G11 schema/type/budget/effect | capability schema・型・budget・effect ceiling の reject が `PlanCompiler` 層でそのまま効く | compiler 既存 43 test 全 green・importer 経由 plan も同一検査 | Pass | `tests/compiler.rs` 全件・`tests/importer.rs` |
| FAIL-G12 backend/language 未知 | `AvailableBackends` に無い backend・未知 `LanguageId` が typed reject | `backend_and_language_fail_closed`（builder 層） | Pass | `tests/workflow.rs` |
| FAIL-G13 未知/非対応 action | model/binding で解決不能な `action()`・非対応 domain/execution が typed reject | `UnknownApi`/`UnsupportedWorkflowStep` | Pass | `tests/workflow.rs` `unknown_and_unmapped_actions_rejected` |

### AC-4 Workflow結果依存分岐はAST Stepで表し、Script評価時にProject/外部Jobが実行されない。

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| EXEC-G01 AST lower | `if`/`for_each`/`await_job`/`atomic_macro`/`emit` が既存 `Step` variant（`If`/`ForEachBounded`/`AwaitJob`/`AtomicMacro`/`EmitResult`）へ lower される | `control_calls_lower_to_restricted_ast_kinds` green・`Expr::Field` は importer 内部のみ | Pass | `tests/workflow.rs` |
| EXEC-G02 実行なし | `action()`/`workflow()` は invoke ではなく step 参照の記録 — builder 出力に副作用・executor 呼出が存在しない | builder は document JSON のみ生成・`synvelis-script` に Gateway/executor 依存なし | Pass | コードレビュー＋依存 DAG |
| EXEC-G03 分岐意味論 | 結果依存分岐は `If`/`ForEachBounded`/`StepOutputRef` で表され、script 評価時に外部 Job が走らない | `AwaitJob` は job 式参照のみ・`coordinator` 17 test 非回帰 | Pass | `tests/workflow.rs`・`tests/coordinator.rs` |

### AC-5 Run/Cancel/Resume/Indeterminate/Grant revokeは既存Coordinator/Gatewayでのみ処理しVM stackを永続化しない。

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| BOUND-G01 単一権威 | Definition/Plan/解決物に VM stack・coroutine・continuation・authority field が存在しない | `WorkflowScriptDefinition`/`ScriptWorkflowSpec`/`ScriptWorkflowRef`/`ImportedWorkflow` の全 field が version/digest/language/path/spec のみ — `deny_unknown_fields` | Pass | 型定義＋`tests/decode.rs` `forbidden_authority_fields_have_no_representations` 非回帰 |
| BOUND-G02 Coordinator 非回帰 | Run/Cancel/Resume/Indeterminate/Grant revoke の処理先は既存 #265 Coordinator/Gateway のみ | `tests/coordinator.rs` 17 test 全 green・`synvelis-automation` に第二 run path なし | Pass | `mise run test synvelis-automation` |
| BOUND-G03 VM 非永続化 | script source・解決物に VM state の保存経路がない | importer/builder に VM handle・stack・session を保持する型が存在しない | Pass | コードレビュー＋型境界 |

### AC-6 CLI workflow run --file がYAMLとLuauを解釈し、Headless同一Plan経由で実行できる。

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| CLI-G01 source kind 判定 | `.yaml`/`.yml`/`.luau` の source kind 判定が公開 API として存在する | `workflow_source_kind` + `WorkflowSourceKind`（`.luau`/`.rhai` file payload は現状 Unsupported — fail-closed） | Pass（API のみ） | `tests/importer.rs` `workflow_source_kind_classifies` |
| CLI-G02 headless plan 入口 | file → source 種別 → 共通 Definition → `PlanCompiler` → coordinator 駆動の seam が公開されている | `compile_workflow_yaml`/`import_workflow_document` が `WorkflowDefinition`/`PlanCompiler` へ合流 | Pass（API のみ） | `tests/importer.rs` |
| CLI-G03 実 CLI 実行 | `synvelis-cli` の `workflow run --file` が YAML/Luau を解釈し実行する | **実 CLI 配線は sv#262 領域（リース外）— 未実施** | Deferred | — |

## NotRun / Deferred / Unsupported 台帳

| 項目 | 判定 | 理由・後続検証 |
| --- | --- | --- |
| 実 Luau/Rhai VM の実行・script source の解析 | NotRun | ruviform#559 同一バイナリ結合 Gate。builder/importer は script source を実行・解析しない契約層のみ |
| 実 CLI `workflow run --file` の YAML/Luau 解釈・実行 | Deferred | synvelis#262 領域（リース外）。本 slice は source kind 判定と共通 plan 入口の公開 API のみ |
| Coordinator との実 Plan/Job 結合受入 | Deferred | synvelis#265。import 解決済み Definition の実行側検証は後続 |
| 製品 E2E（GUI/CLI/実 OS/RT） | Deferred | synvelis#267 |
| `.luau`/`.rhai` file の resolver 解決 | Unsupported | `FileWorkflowResolver` は `.yaml`/`.yml` のみ — script file parse は VM/backend 結合後。typed reject（fail-closed・silent fallback なし） |
| Rhai の正式製品対応 | Unsupported | Issue「対象外」— builder の意味論適合は PoC fixture のみ |
| `Expr::Field` の YAML 面への露出 | Unsupported | importer 生成の内部 variant に限定 — YAML parser/frontend は受理しない |
| `mise run gw10`・`mise run test-rt` | NotRun | ハーネス非回帰の任意確認。本 slice の正規レーン外 |
| `*-msrv` レーン | NotRun | ユーザー指定により実行しない |
| 実 OS・実 GPUI・実 provider lane | NotRun | 本 slice は headless 契約層のみ |

## Fail-closed verdict 台帳（RF-SCRIPT-00 語彙対応）

| Case | verdict/型 | 判定 |
| --- | --- | --- |
| 未知 workflow 参照 | `ImportError::UnknownWorkflow` | Pass |
| import cycle | `ImportError::ImportCycle` | Pass |
| import 深さ超過 | `ImportError::ImportDepthExceeded` | Pass |
| static step 数超過 | `ImportError`（`MAX_STATIC_STEPS`） | Pass |
| version 不一致 | `ImportError::VersionMismatch` | Pass |
| source digest drift | `ImportError::SourceDrift` | Pass |
| language 不一致 | `ImportError::LanguageMismatch` | Pass |
| 呼出し input 不足/過剰/型不一致 | `ImportError::WorkflowInputMismatch` | Pass |
| 未知 workflow output 参照 | `ImportError::UnknownWorkflowOutput` | Pass |
| 宣言 output 未 emit | `ImportError::MissingWorkflowOutput` | Pass |
| duplicate step id | `ImportError::DuplicateStepId`・builder `DuplicateStepId` | Pass |
| path escape/絶対 path/root 外 | `ImportError::PathEscape` | Pass |
| 未知 source 拡張子/非対応言語 file | `ImportError::UnsupportedSource`/`UnsupportedLanguage` | Pass |
| 未来 Step 参照 | `CompileError::ForwardStepReference`（compiler 層・importer 非弱化） | Pass |
| 未知 backend/language（builder） | `WorkflowBuildError` typed variant | Pass |
| 未知/非対応 action | `UnknownApi`/`UnsupportedWorkflowStep` | Pass |
| schema/type/budget/effect 違反 | `CompileError` 各 variant（`PlanCompiler` 単一権威） | Pass |
| authority field の script 表現 | 型に存在しない（`deny_unknown_fields`） | Pass |
