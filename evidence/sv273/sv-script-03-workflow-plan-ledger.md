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

---

# 第 2 slice — 実 Luau backend 結合（2026-11-16）

Date: 2026-11-16。Slice: `synvelis-script` に言語中立の
backend port（`backend` module）と
`ScriptWorkflowSpec::from_script_value` を追加し、新規
crate `synvelis-script-luau`（mlua 0.12.2 Luau feature、
coordinator 承認済み）で実 VM adapter を実装。
`synvelis-automation` の `FileWorkflowResolver` は注入された
`Arc<dyn ScriptBackend>` + `ScriptBuildContext` 経由で
`.luau` file を評価し、file byte digest を importer 側で
再検証してから `WorkflowScriptDefinition` → import
document → `WorkflowDefinition` へ lower する。
`synvelis-cli` の `workflow run --file x.luau` は実 VM で
評価し typed definition/import 結果を envelope で返す。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/273
（OPEN・`Refs` のみ・Close しない）。

## Coordinator 境界決定（記録）

- `mlua` 依存は `crates/synvelis-script-luau` のみに許可。
  他 crate は `synvelis-script` の port trait のみを消費し、
  mlua/Luau 固有型を参照しない。
- Option B: `.luau` は evaluate → `ScriptWorkflowSpec` →
  import document 手前までを配線し、surface/registry 不在は
  typed fail-closed で正直に報告。envelope は definition/
  import 結果を返し、plan compile は synvelis#262 へ defer。
  `host.rs`/`Booted`・canonical `ScriptSurfaceSpec` は本
  slice のリース外。

## 計測環境（第 2 slice）

| 項目 | 値 |
| --- | --- |
| Synvelis branch | `feat/273-luau-workflow-backend`（worktree `sv273-luau-backend`） |
| OS / CPU / FS | Windows 11 / NTFS |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024、MSRV 1.89 |
| Build | mbx managed target |
| VM backend | mlua 0.12.2 `luau` feature（interim adapter — RF-SCRIPT 正式 contract は ruviform#559 で再検証） |

## 実行した正規検証（第 2 slice）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0） |
| `mise run check synvelis-script` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-script-luau` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-automation` | Pass（exit 0、`--locked --all-targets`） |
| `mise run check synvelis-cli` | Pass（exit 0、`--locked --all-targets`） |
| `mise run lint synvelis-script` | Pass（exit 0、clippy warning 0） |
| `mise run lint synvelis-script-luau` | Pass（exit 0、clippy warning 0） |
| `mise run lint synvelis-automation` | Pass（exit 0、clippy warning 0） |
| `mise run lint synvelis-cli` | Pass（exit 0、clippy warning 0） |
| `mise run test synvelis-script` | Pass（97 test: action 22・analyzer 7・backend 12・binding 13・golden 10・luau 9・model 17・workflow 7。doc-test 0） |
| `mise run test synvelis-script-luau` | Pass（29 test、実 mlua Luau VM。doc-test 0） |
| `mise run test synvelis-automation` | Pass（184 test: action_recipe 9・compiler 43・coordinator 17・decode 11・determinism 12・importer 26・live_adapters 15・macros 17・mixed_source_run 6・script_analyzer 12・yaml_subset 15・lib 1。doc-test 0） |
| `mise run test synvelis-cli` | Pass（24 test: lib 5・cli_e2e 19。doc-test 0） |
| `mise run check synvelis-gateway` | Pass（exit 0、dependent-crate 非回帰） |
| `mise run check synvelis-workspace` | Pass（exit 0、dependent-crate 非回帰） |

## 追加検証の要点（第 2 slice）

- `synvelis-script/tests/backend.rs`（7 test）: registry の
  fail-closed 解決（未知 language/backend・contract version
  不一致・ambiguous candidate）・`ScriptValue` codec・
  `ScriptWorkflowSpec::from_script_value` の shape 検証・
  session open/evaluate/close lifecycle。
- `synvelis-script-luau/tests/backend.rs`（29 test、実 VM）:
  scalar/record 変換・integer/float 保存・workflow artifact
  生成・source digest mismatch・parse/runtime error・
  session 間 global/state 分離・budget/cancel・artifact
  contract 違反・非対応値（function/userdata 等）の typed
  reject・sandbox global strip（`require`/`loadstring`/
  `print`/`debug` が nil）・共有 subtable DAG の lift node
  予算・evaluate single-shot・`pcall` が wall-clock budget
  を呑めないこと・zero/overflow budget の typed reject。
- `synvelis-automation/tests/importer.rs`（25 test、前 slice
  16 → 25）: fake backend の fail-closed 行列（backend 無し・
  digest drift・eval failure・artifact 欠落・kind/schema
  違反・inline 成功・`.rhai` unsupported）に加え、実 VM
  backend で `.luau` file が YAML 同値 spec と同一の
  `WorkflowDefinition`・同一 plan digest を生成すること、
  Luau/Rhai/YAML 多段 inline chain が compile されることを
  検証。
- `synvelis-cli/tests/cli_e2e.rs`（19 test）:
  `workflow_luau_file_evaluates_to_typed_definition` — 有効な
  `.luau` workflow file が実 VM 評価で typed envelope を返す。
  分類行列の exit code 期待は維持。

## NotRun / Deferred / Unsupported 台帳（第 2 slice 追記）

| 項目 | 判定 | 理由・後続検証 |
| --- | --- | --- |
| `.luau` file の resolver 解決（backend 注入時） | Pass | 実 VM backend 経由で `WorkflowDefinition`/plan digest が YAML 同値と一致 |
| `.luau` file の resolver 解決（backend 未注入） | Unsupported→Pass | `UnsupportedSource` の typed reject を維持（fail-closed） |
| `.rhai` file の resolver 解決 | Unsupported | backend 注入済みでも `UnsupportedSource` — Rhai 正式対応は対象外 |
| CLI `.luau` → `ExecutablePlan` compile | Deferred | registry/surface 入力不在（Option B）。synvelis#262/#274 領域 |
| Rhai 実 backend | Deferred | ruviform#559 gate |
| `synvelis-script-luau` の RF-SCRIPT 正式 contract 整合 | Deferred | interim adapter — ruviform#559 で再検証 |
| `mise run gw10`・`mise run test-rt`・`*-msrv` | NotRun | 正規レーン外 |
| 実 OS・実 GPUI・実 provider lane | NotRun | headless 契約層のみ |

## Fail-closed verdict 台帳（第 2 slice 追記）

| Case | verdict/型 | 判定 |
| --- | --- | --- |
| backend 未登録 language | `ScriptError`/`SessionError` typed | Pass |
| source digest mismatch（session open） | `SessionError` typed | Pass |
| file byte digest drift（importer 再検証） | `ImportError::SourceDrift` | Pass |
| artifact 欠落 | `AutomationError::ScriptBackend(Contract)` | Pass |
| artifact kind/schema_version 違反 | `AutomationError::ScriptBackend(Contract)` | Pass |
| Luau parse/runtime error | `ScriptError` typed kind | Pass |
| budget/cancel 超過 | `ScriptError` typed kind（interrupt hook） | Pass |
| 表現不能な戻り値（function/userdata 等） | `ScriptError` typed kind | Pass |
| `.rhai` file（backend 注入済み） | `ImportError::UnsupportedSource` | Pass |
| session 間 state/global 漏洩 | 独立 `mlua::Lua` VM で構造的に排除 | Pass |
| authority object の VM 内露出 | 型に存在しない（script は declarative spec のみ返す） | Pass |

## Review cycle 記録（2026-11-16）

review-protocol に従い cooperative（criteria）/ adversarial
（defect hunt）の 2 reviewer を並行 dispatch し、candidate を
検証した。CONFIRMED として修正したもの:

| 指摘 | severity | 対応 |
| --- | --- | --- |
| mlua `configure_luau` が `require`/`loadstring`（filesystem code exec・digest pin 外）・`print`（stdout 汚染）・`debug` を無条件 install — script source が digest pin を逸脱可能 | P1 | session 返却前に 4 global を strip（`STRIPPED_GLOBALS`）。実 VM test で nil を検証 |
| `lift_table` が共有 subtable DAG で 2^n path を列挙 — Rust 側処理のため全 VM budget を迂回 | P1 | `MAX_LIFT_NODES`（1M）node budget を lift に追加。DAG test で ContractViolation を検証 |
| `ImportError::SourceDrift` Display が script 制御の非 ASCII `pinned` を byte-slice → char-boundary panic | P1 | char-safe truncate に修正。非 ASCII digest の Display test を追加 |
| `Instant::now() + Duration` が巨大 budget で panic | P2 | `checked_add` → `UnsupportedBudget` typed reject |
| `evaluate_luau_spec` が builder 未経由で spec 構造未検証 — CLI が不正 spec を valid と報告し得た | P2 | `from_script_value` に `validate_script_id` + `check_calls`（builder と同一規則）を集約 |
| `max_memory_bytes: Some(0)` が mlua の 0=unlimited 仕様で fail-open | P3 | open 時に `UnsupportedBudget` で reject |
| entry chunk が bytecode mode を受理し得た | P3 | `ChunkMode::Text` を明示 |
| `Lua::new()` の内部 expect panic・`"undigested"` digest fallback | P3 | `new_with` Result + `catch_unwind` → typed `SessionError`、digest 失敗は typed `ScriptError` |
| `evaluate` が複数回呼べ stale `hit` で誤分類 | P3 | single-shot guard（2 回目は ContractViolation）+ 成功経路でも interrupt flag を再検査 |
| CLI error が canonical 絶対 path を露出・stale unsupported message | P3 | user 提示 path を使用・message 更新 |

検証して棄却/非該当としたもの:

- Reviewer A の「`pcall` が interrupt を呑み false success/無限 hang」:
  Luau の safepoint は CALL/JUMPBACK/FORNLOOP/RETURN に存在し、
  interrupt error は最も近い `pcall` へ unwind するが、catch 後の
  次 safepoint で再 throw されるため `return` に到達不能。
  `pcall_cannot_swallow_wall_clock_budget` test が実測で
  `BudgetExceeded` を確認（Reviewer B の safepoint 解析が正しい）。
  成功経路の `hit` 再検査は防御として残置。
- `synvelis-automation` の新規 public enum に `#[non_exhaustive]`
  なし: crate 既存規約（`AutomationError`/`ImportError` 等全て
  非付与）に一致。port 層（`synvelis-script/backend.rs`）は
  全 enum に付与済み。crate 全体の policy 是正は別件。
- `ExecutionBudget::default()` の無制限: port 契約上 consumer
  責務。CLI は 30s/256MiB を設定済み。#262/#274 配線時に
  最小 ceiling を検討。
