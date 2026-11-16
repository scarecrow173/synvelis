---
id: "IMPL-034Xl5nZaPjrHyr6UPJ5q4"
type: "impl"
status: "completed"
title: "SV-SCRIPT-03 YAML/Luau/Rhai Workflow共通Plan化・異種Workflow参照"
created: "2026-11-14"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/273"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-script/src/workflow.rs"
      - type: "file"
        path: "crates/synvelis-script/tests/workflow.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/importer.rs"
      - type: "file"
        path: "crates/synvelis-automation/tests/importer.rs"
      - type: "file"
        path: "docs/impl/exp/sv-script-03-workflow-plan.md"
      - type: "file"
        path: "docs/test-specs/sv-script-03-workflow-plan.md"
      - type: "file"
        path: "evidence/sv273/sv-script-03-workflow-plan-ledger.md"
      - type: "file"
        path: "crates/synvelis-script/src/backend.rs"
      - type: "file"
        path: "crates/synvelis-script/tests/backend.rs"
      - type: "file"
        path: "crates/synvelis-script-luau/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-script-luau/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-script-luau/tests/backend.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-script/src/action.rs"
      - type: "file"
        path: "crates/synvelis-script/src/error.rs"
      - type: "file"
        path: "crates/synvelis-script/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/compiler.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/error.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/eval.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/expr.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/frontend.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/macros.rs"
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-automation/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-automation/tests/common/mod.rs"
      - type: "file"
        path: "crates/synvelis-cli/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-cli/src/script.rs"
      - type: "file"
        path: "crates/synvelis-cli/tests/cli_e2e.rs"
      - type: "file"
        path: "crates/synvelis-script/src/workflow.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
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
    - "docs/impl/ir/sv-script-01-script-api-model.md"
    - "docs/impl/ir/sv-script-02-luau-action-script.md"
    - "docs/impl/ir/autx-05-workflow-dsl.md"
    - "docs/impl/ir/autx-06-workflow-coordinator.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/sv-script-03-workflow-plan.md"
    - "docs/test-specs/sv-script-03-workflow-plan.md"
    - "evidence/sv273/sv-script-03-workflow-plan-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/automation/design-script.md"
    - "docs/impl/ir/sv-script-01-script-api-model.md"
    - "docs/impl/ir/sv-script-02-luau-action-script.md"
    - "docs/impl/ir/autx-05-workflow-dsl.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-script-03-workflow-plan.md"
  references: []
  defers:
    - "ruviform#559"
    - "synvelis#262"
    - "synvelis#265"
    - "synvelis#267"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/sv-script-03-workflow-plan.md"
    rejected: []
---

# SV-SCRIPT-03 YAML/Luau/Rhai Workflow共通Plan化・異種Workflow参照

## Summary

Issue #273 の headless slice を実装した。script 側
（`synvelis-script`）に `WorkflowScriptDefinition`（versioned・
digest-pinned・language/backend 契約 — authority を型で
表現不能）と `ScriptWorkflowSpec`/`ScriptStepCall`/
`SpecIoDecl`、および `WorkflowScriptBuilder` を追加し、
`action()`/`workflow()`/`if`/`for_each`/`await_job`/
`atomic_macro`/`emit` 呼出しを**即時実行せず step 参照として**
canonical import document JSON へ lower する。automation 側
（`synvelis-automation`）に `importer` module を新設し、
`kind: "workflow"` 参照ノードを `WorkflowResolver` 経由で
compile 時に解決・inline 展開して単一の
`WorkflowDefinition` を生成する。参照は
language/version/source-digest/path で pin され、解決物は
`<call>__<step>` 名前空間・`$input` 引数代入・宣言 output の
virtual output（`$steps.<call>.<output>`）として既存 AST に
還元される — Script 専用 Plan IR・第二 compiler・VM 間
呼出し・runtime import は存在しない。YAML/Luau の同値 spec が
同一 `WorkflowDefinition`・同一 `ExecutablePlan` digest を
生成すること、Luau→Rhai→YAML の多段参照が typed
input/output 引継ぎで compile されること、fail-closed reject
行列を fixture で確認した。

2026-11-16 の第 2 slice で headless の実 backend 経路を
結合した。`synvelis-script` に言語中立の backend port
（`backend` module — `ScriptSource`/`ExecutionBudget`/
`ScriptValue`/`ScriptArtifact`/`ScriptResult`/
`ScriptError`/`SessionError`/`ScriptSession`/
`ScriptBackend`/`ScriptBackendRegistry`）を追加し、
`ScriptWorkflowSpec::from_script_value` で artifact payload
を typed spec へ decode する。新規 crate
`synvelis-script-luau` が `mlua`（Luau feature、coordinator
承認済み）による実 VM adapter を `ScriptBackend` として
実装し、source digest 検証・session 分離・typed error
写像を行う。`synvelis-automation` の `FileWorkflowResolver`
は注入された `Arc<dyn ScriptBackend>` +
`ScriptBuildContext` 経由で `.luau` file を評価し、file
byte の digest を importer 側で再検証してから
`WorkflowScriptDefinition` → import document →
`WorkflowDefinition` へ lower する。`.rhai` は backend が
注入されても `UnsupportedSource` のまま（PoC/対象外）。
`synvelis-cli` の `workflow run --file x.luau` は実 VM で
評価し typed definition/import 結果を envelope で返すが、
registry/surface 入力が存在しない plan compile は
coordinator 承認の Option B 境界に従い #262 へ defer
する（`host.rs`/`Booted`・canonical `ScriptSurfaceSpec`
は本 slice のリース外）。

## Context

- 対象 Issue: <https://github.com/scarecrow173/synvelis/issues/273>
  （OPEN・PR 本文は `Refs #273`・Close しない — 実 Luau/Rhai
  VM 実行・CLI/GUI 結合・製品 E2E の受入ゲートが残る）
- Canonical 契約: `docs/automation/design-script.md`
  （Script は authority を持たず独立 run surface を持たない）、
  AUTX-05 `WorkflowDefinition`/`PlanCompiler`（#264 — 本 slice
  が lower する唯一の Plan 表現）、AUTX-06 Coordinator（#265 —
  Run/Cancel/Resume/Indeterminate の唯一の処理先）
- 消費する先行実装: SV-SCRIPT-01（#271/PR #435 —
  `ScriptApiModel`/`BindingSet`/snapshot digest）、SV-SCRIPT-02
  （#272/PR #438 — `ScriptActionDefinition`/`LanguageId`/
  `BackendRequirement`/`AvailableBackends` と typed Action
  Recipe 経路）、AUTX-04（#429 — Atomic Macro）、RF-SCRIPT-00
  （ruviform PR #621 — fail-closed 判定表 F-01..F-18・Fault
  Matrix M-01..M-13 の語彙）、RF-SCRIPT-02（ruviform PR #624 —
  analyzer/backend 非対応機能は typed `Unsupported`）
- 本 slice は Start/Integration 分離の Start 側: #264/#265
  実 Plan/Job 結合・ruviform#559 同一バイナリ PoC までは
  本番実行・異種 Backend の Pass を宣言しない。

## Implementation

### Changes

- `crates/synvelis-script/src/workflow.rs`（新規）:
  - `WorkflowScriptDefinition` — `schema_version`/`script_id`/
    `version`/`language_id`/`backend_requirement`/
    `source_digest`。`validate` が version・id/digest 形状・
    language↔backend 整合（Yaml→None/Luau→Luau/Rhai→Rhai）・
    backend 可用性を検査し、`digest()`/`verify_source` が
    drift を typed fail-closed にする。authority field は
    存在しない。
  - `ScriptWorkflowRef` — 異種 `workflow()` 参照の pin:
    `language_id`/`version`/`source_digest`/`path`（canonical
    root 相対、embedded は空）。
  - `ScriptStepCall` — `Action`/`Workflow`/`If`/`ForEach`/
    `AwaitJob`/`AtomicMacro`/`Emit` の呼出し木。式位置は
    YAML frontend と同一の ergonomic `$`-reference 面を
    verbatim に運び、lower は importer 側が行う。
  - `ScriptWorkflowSpec`/`SpecIoDecl` — 評価記録（workflow
    id/version・typed inputs/outputs・call 木・limits）。
  - `WorkflowScriptBuilder::build` — definition 検証 →
    binding↔model drift → spec version → call 木の一意 id・
    node 予算（`MAX_WORKFLOW_NODES`）→ 各 call を import
    document step JSON へ写像。`action()` は
    `ScriptApiModel`/`BindingSet` で name を解決し、
    capability の domain/execution から step kind を**契約側
    で決定**（Query+Sync→`query`、ProjectMutation+Sync→
    `invoke`、AsyncOperation+AsyncAccept→`async-submit`、
    それ以外は `UnsupportedWorkflowStep`）— script が kind を
    名乗る経路はない。capability pin は `id@major.minor`。
- `crates/synvelis-script/src/action.rs`: `validate_script_id`
  を `pub(crate)` 化（workflow definition が同一 id 規則を
  再利用するため — 重複実装しない）。
- `crates/synvelis-script/src/error.rs`: `DuplicateStepId`・
  `UnsupportedWorkflowStep` variant を追加。
- `crates/synvelis-script/src/lib.rs`: `workflow` module と
  全公開型を export。
- `crates/synvelis-automation/src/expr.rs`: `Expr::Field
  { base, path }` を追加 — YAML 面では表現不能な内部投影。
  workflow import の `$input` 引数代入が構築値（Object/Array
  リテラル等）へ path を継ぎ足す際に使う。
- `crates/synvelis-automation/src/eval.rs`: `Expr::Field` の
  runtime 評価 — base を評価して `resolve_path` で fail-closed
  に投影。
- `crates/synvelis-automation/src/compiler.rs`: `Expr::Field`
  の静型付け — `project_path`（Object/Array/Optional/Json を
  投影、解決不能は `TypeMismatch`）を `pub(crate)` 化して
  compiler/macros/importer で共有。
- `crates/synvelis-automation/src/macros.rs`: `Expr::Field`
  の静型付け（`project_path`）と runtime 投影
  （`project_macro_path` — object key/array index、欠落は
  typed reject）を追加。
- `crates/synvelis-automation/src/frontend.rs`: `lower_expr`・
  `lower_condition` を `pub(crate)` 化 — importer が同一の
  ergonomic `$`-surface lower を再利用するため（第二 DSL
  を作らない）。
- `crates/synvelis-automation/src/error.rs`: `ImportError`
  （`UnknownWorkflow`/`LanguageMismatch`/`VersionMismatch`/
  `SourceDrift`/`ImportCycle`/`ImportDepthExceeded`/
  `TooManySteps`/`WorkflowInputMismatch`/
  `MissingWorkflowOutput`/`UnknownWorkflowOutput`/
  `DuplicateStepId`/`WorkflowDocument`/`UnsupportedSource`/
  `PathEscape`）と `AutomationError::{Import,Script}` variant・
  `From` 変換を追加。
- `crates/synvelis-automation/src/importer.rs`（新規）:
  - `WorkflowResolver` trait — `workflow()` 参照を pinned
    解決する唯一の seam。戻り値 `ImportedWorkflow` は
    実際の language/version/digest を報告し、pin 照合は
    importer が単一実装で行う（resolver は自己申告しない）。
    URI/remote/secret の面は型に存在しない。
  - `MapWorkflowResolver` — in-memory fixture 実装
    （`(language,version,digest)` key）。
  - `FileWorkflowResolver` — canonical root 内の
    `.yaml`/`.yml` のみ解決。relative path・`..`/root/prefix
    成分・canonicalize 後の root 外（symlink escape）を
    `PathEscape`/`UnsupportedSource` で拒否。digest は parse
    した byte 列そのものの SHA-256（TOCTOU は drift として
    検出）。`.luau`/`.rhai` file は script backend 未結合の
    ため `UnsupportedSource`（Deferred）。
  - `import_workflow_document` — import document（canonical
    `WorkflowDefinition` wire 形 + `kind:"workflow"` ノード）
    を再帰的に `WorkflowDefinition` へ lower。参照解決は
    compile 時に完結し、cycle は resolution stack の
    `(language,version,digest)` key で `ImportCycle`、深さは
    `MAX_IMPORT_DEPTH=32`、展開 step 数は
    `MAX_STATIC_STEPS` で fail-closed。解決済み定義は
    `<call>__<step>` 名前空間で inline され、`$input.<name>`
    は呼出し引数式へ（path 射影はリテラル畳み込み・
    参照形 path 連結・`Expr::Field` の使い分け）、`$steps.<id>`
    は `call__` prefix 付きへ再帰書換え。宣言 output の
    `emit-result` は virtual output map として親 scope に
    登録され、`$steps.<call>.<output>` が後続 sibling から
    参照可能 — if/for-each 本体の scope 規則と同じく分岐を
    越えて漏れない。
  - `workflow_source_kind`/`compile_workflow_yaml`/
    `compile_script_workflow` — headless `workflow run
    --file`（synvelis#262）の source kind 判定と、file/spec
    → definition → `PlanCompiler::compile_with_macros` の
    単一 plan 経路。
- `crates/synvelis-automation/src/lib.rs`: `importer` module・
  `ImportError`・公開型を export。

### Changes（第 2 slice: 実 Luau backend 結合）

- `crates/synvelis-script/src/backend.rs`（新規）:
  言語中立の backend port mirror。`ScriptSource`（language・
  source bytes・digest・limits）、`ExecutionBudget`（step/
  instruction/time/cancel flag）、`ScriptValue`（Absent/
  Null/Bool/Integer/Number/String/Bytes/Array/Record/Time/
  ResourceId — authority 表現を持たない中立値）、
  `ScriptArtifact`（kind・schema_version・payload）、
  `ScriptResult`、`ScriptError`/`SessionError`（typed
  kind + position + reason）、`ScriptSession`（evaluate/
  close/session_id）、`ScriptBackend`（language_id・
  capabilities・open_session — source digest・artifact
  kind・budget/isolation を検証）、`ScriptBackendRegistry`
  （AvailableBackends からの fail-closed 解決: 未知
  language/backend・未対応 contract version・ambiguous
  candidate を typed reject）。公開 enum は全て
  `#[non_exhaustive]`。
- `crates/synvelis-script/src/workflow.rs`:
  `ScriptWorkflowSpec::from_script_value` を追加 —
  `ArtifactKind::WorkflowDefinition` の artifact payload
  （`ScriptValue::Record`）を serde 経由で typed spec へ
  decode し、shape 不正は `WorkflowBuildError` で
  fail-closed。
- `crates/synvelis-script/src/lib.rs`: `backend` module と
  全公開型を export。
- `crates/synvelis-script/tests/backend.rs`（新規）: fake
  backend による registry fail-closed・codec・spec decode・
  session lifecycle の契約 test（12 件）。
- `crates/synvelis-script-luau/`（新規 crate）:
  - `Cargo.toml` — `mlua = { version = "0.12.2",
    default-features = false, features = ["luau"] }` のみ
    （coordinator 承認。mlua は本 crate に隔離し、他 crate
    は port trait のみを消費する）。
  - `src/lib.rs` — `LuauBackend`/`LuauSession` が
    `ScriptBackend`/`ScriptSession` を実装。open_session で
    language（Luau）・source digest（SHA-256 照合）・
    artifact kind・source/budget 上限を検証し、session 毎に
    独立した `mlua::Lua` VM を生成（global/state 漏洩なし）。
    `evaluate` は source を text-mode chunk として
    load・eval し（bytecode 拒否）、戻り値の Luau 値を中立
    `ScriptValue` へ decode（integer/float 保存・
    table→Array/Record・function/userdata/thread 等の
    表現不能値は typed reject）。`evaluate` は single-shot
    で、成功経路でも interrupt flag を再検査する
    （`pcall` に呑まれた interrupt が success を装えない）。
    mlua `configure_luau` が無条件に install する
    `require`/`loadstring`（digest pin 外の filesystem code
    実行）・`print`（host stdout 汚染）・`debug` は session
    返却前に global から strip する。lift は
    `MAX_LIFT_NODES`（1M node）で共有 subtable DAG の
    指数爆発を Rust 側で fail-closed に止める。
    interrupt hook で wall-clock/cancel を safepoint 強制、
    `set_memory_limit` で memory を強制（`Some(0)` は
    unlimited 化を防ぐため open 時に typed reject）。
    `Instant + Duration` の overflow・`Lua` state 生成失敗・
    artifact digest 失敗は panic/"undigested" ではなく
    typed `SessionError`/`ScriptError`。runtime/parse/
    budget/cancel/contract error を `ScriptError` kind へ
    写像。VM 内に authority object・capability 呼出し API
    は存在しない（script は declarative spec record のみ
    返す）。
  - `tests/backend.rs` — 実 VM test 29 件: scalar/record
    変換・int/float 保存・workflow artifact 生成・digest
    mismatch・parse/runtime error・session 分離・budget/
    cancel・artifact contract 違反・非対応値・sandbox
    global strip・DAG lift 予算・single-shot evaluate・
    pcall による budget 回避不可。
- `crates/synvelis-automation/src/importer.rs`:
  - `ScriptBuildContext` — `AvailableBackends` +
    `ScriptApiModel` + `BindingSet` + `ExecutionBudget` を
    まとめて注入する build 入力。
  - `evaluate_luau_workflow(backend, context, text,
    source_digest_hex)` — `SessionConfig`（Luau・digest・
    budget・`ArtifactKind::WorkflowDefinition`）で session
    を open→evaluate→close し、artifact の存在・
    schema_version・kind を検査、payload を
    `ScriptWorkflowSpec` へ decode、`WorkflowScriptDefinition`
    （Luau・Luau backend requirement・digest・spec の
    id/version）を構築して `WorkflowScriptBuilder` へ渡す。
    失敗は `AutomationError::ScriptBackend`（boxed）で
    typed fail-closed。
  - `FileWorkflowResolver` に `luau_backend` +
    `ScriptBuildContext` の注入経路を追加 — `.luau` file
    は bytes の SHA-256 を取り、評価結果の
    `source_digest` と照合（drift は `SourceDrift`）。
    backend 未注入の `.luau` は従来通り
    `UnsupportedSource`。`.rhai` は backend 注入済みでも
    `UnsupportedSource`。
  - `AutomationError::ScriptBackend(Box<ScriptBackendError>)`
    — `ScriptBackendError::{Open,Evaluate,Contract}`。
    variant payload を box 化して Err サイズを 128B 閾値内に
    維持（clippy `result_large_err`）。
  - `ImportError::SourceDrift` の Display を char-safe な
    truncate に修正 — script 制御の `pinned` digest が
    非 ASCII を含んでも panic しない（review で検出した
    byte-slice char-boundary panic）。
- `crates/synvelis-automation/tests/common/mod.rs`:
  再利用可能な `FakeScriptBackend`/`FakeSession` を追加。
- `crates/synvelis-automation/tests/importer.rs`: fake
  backend による fail-closed 行列（backend 無し・digest
  drift・eval failure・artifact 欠落・kind/schema 違反・
  inline 成功・Rhai unsupported）と、実 VM backend による
  `.luau` 解決 test（YAML 同値 spec との definition/plan
  digest 一致・Luau/Rhai/YAML 多段 inline chain）を追加。
- `crates/synvelis-cli/src/script.rs`: `workflow run --file
  x.luau` を `synvelis-script-luau` の実 backend で評価し、
  typed `ScriptWorkflowSpec` + `WorkflowScriptDefinition`
  を envelope で返す経路を追加。registry/surface 入力が
  ない plan compile は unavailable を正直に扱い、envelope
  の `definition`/`import` 結果のみを返す（Option B — plan
  compile は #262 へ defer）。`.yaml`/`.rhai`/`.lua`/未知
  拡張子の分類・reject は不変。
- `crates/synvelis-cli/tests/cli_e2e.rs`:
  `workflow_luau_file_evaluates_to_typed_definition` を追加
  — 有効な `.luau` workflow file が実 VM 評価で typed
  envelope を返すことを検証。分類行列の exit code 期待は
  維持。
- `Cargo.toml`/`Cargo.lock`: workspace member
  `synvelis-script-luau` 追加と mlua 依存木を記録。

### Non-goals / Deferred

- 実 Luau/Rhai VM 実行は第 2 slice で
  Luau のみ結合済み（`synvelis-script-luau`）。Rhai の実
  backend・ruviform#559 同一バイナリ PoC 結合は依然
  **Deferred** — 本 slice の adapter は interim 実装であり、
  RF-SCRIPT の正式 backend contract への整合は #559 gate
  後に再検証する。
- `.luau` file 解決は注入された backend 経由で結合済み。
  backend 未注入時・`.rhai` file は従来通り
  `UnsupportedSource`（fail-closed）。
- `synvelis-cli` の `workflow run --file` は第 2 slice
  で `.luau` の typed 評価まで配線済み（Option B）。
  registry/surface/plan compile の CLI 結合・canonical
  `ScriptSurfaceSpec`・`host.rs`/`Booted` accessor は
  synvelis#262/#274 領域（リース外）のまま **Deferred**。
- Run/Cancel/Resume/Indeterminate・Grant revoke は既存
  #265 Coordinator/Gateway のみ — Definition/Plan/解決物に
  VM stack・coroutine・continuation は存在しない。
- Rhai の正式製品対応は対象外（Issue「対象外」）— builder
  の意味論適合は PoC fixture で示す。

## Related Experiments

### Adopted

- `docs/impl/exp/sv-script-03-workflow-plan.md` — E1 import
  document JSON を中間表現に採用（script 専用 typed IR を
  作らず既存 decode 経路へ合流）、E2 `action()` の step
  kind は capability の domain/execution contract が決定、
  E3 異種参照は compile 時 inline 展開 + virtual output
  （runtime VM 間呼出し・別 plan 連鎖を棄却）、E4 式射影は
  `Expr::Field` で path を合成（importer 独自式評価器を
  棄却）。4 試行すべて採用。

### Rejected

- script 専用の第二 Plan IR / 独立 run surface（単一
  `WorkflowDefinition`→`PlanCompiler` 経路の契約に反する
  ため）。
- script が step kind を名指す形（capability contract
  との drift を型で排除するため）。
- 実行時の VM 間呼出し・runtime import・VM state 永続化
  （AUTX-00 `design-script.md` の権限境界に反するため）。

## Validation

`docs/test-specs/sv-script-03-workflow-plan.md` の
Rules/Examples に対応する検証を実施し、全て green:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check synvelis-script` — Pass（exit 0、
  `--locked --all-targets`）
- `mise run check synvelis-automation` — Pass（exit 0、
  `--locked --all-targets`）
- `mise run lint synvelis-script` — Pass（exit 0、clippy
  warning 0）
- `mise run lint synvelis-automation` — Pass（exit 0、
  clippy warning 0）
- `mise run test synvelis-script` — Pass（85 test:
  action 22・analyzer 7・binding 13・golden 10・luau 9・
  model 17・workflow 7。doc-test 0）
- `mise run test synvelis-automation` — Pass（141 test:
  action_recipe 9・compiler 43・coordinator 17・decode 11・
  determinism 12・importer 16・macros 17・yaml_subset 15・
  lib 1。doc-test 0）
- dependent-crate 非回帰: `mise run check
  synvelis-gateway`・`mise run check synvelis-cli`・
  `mise run check synvelis-workspace` — Pass（exit 0）
- 文書: `doc-status` lint・`doc-maintenance` index apply
  （触った dir の index 再生成）

第 2 slice（2026-11-16、実 Luau backend 結合）:

- `mise run fmt-check` — Pass（exit 0）
- `mise run check synvelis-script` — Pass（exit 0、
  `--locked --all-targets`）
- `mise run check synvelis-script-luau` — Pass（exit 0、
  `--locked --all-targets`）
- `mise run check synvelis-automation` — Pass（exit 0、
  `--locked --all-targets`）
- `mise run check synvelis-cli` — Pass（exit 0、
  `--locked --all-targets`）
- `mise run lint synvelis-script` — Pass（exit 0、clippy
  warning 0）
- `mise run lint synvelis-script-luau` — Pass（exit 0、
  clippy warning 0）
- `mise run lint synvelis-automation` — Pass（exit 0、
  clippy warning 0）
- `mise run lint synvelis-cli` — Pass（exit 0、clippy
  warning 0）
- `mise run test synvelis-script` — Pass（97 test:
  action 22・analyzer 7・backend 12・binding 13・golden 10・
  luau 9・model 17・workflow 7。doc-test 0）
- `mise run test synvelis-script-luau` — Pass（29 test、
  実 mlua Luau VM。doc-test 0）
- `mise run test synvelis-automation` — Pass（184 test:
  action_recipe 9・compiler 43・coordinator 17・decode 11・
  determinism 12・importer 26・live_adapters 15・macros 17・
  mixed_source_run 6・script_analyzer 12・yaml_subset 15・
  lib 1。doc-test 0）
- `mise run test synvelis-cli` — Pass（24 test: lib 5・
  cli_e2e 19。doc-test 0）
- dependent-crate 非回帰: `mise run check
  synvelis-gateway`・`mise run check synvelis-workspace` —
  Pass（exit 0）

補助証拠（canonical = mise）: `cargo test -p
synvelis-script --test workflow`・`cargo test -p
synvelis-automation --test importer` の直接実行も同一結果。
`mise run gw10`・`mise run test-rt`・`*-msrv` は正規レーン外
のため NotRun。

詳細な case 対応表は
`evidence/sv273/sv-script-03-workflow-plan-ledger.md`。

## Risks

- inline 展開は参照先 step を親 plan へ複製するため、深い
  多段参照や大きな workflow の組合せで `MAX_STATIC_STEPS`
  ・`MAX_IMPORT_DEPTH` の予算が先に効く — 予算は
  fail-closed であり、引き上げは plan 権威（#264）の契約
  変更を要する。
- virtual output は値式の代入であり、参照先 emit の実行
  順・回数（if/for-each 内 emit）の意味論は compiler の
  step scope 規則に従う — scope を越えた参照は reject
  されるが、「常に実行される emit」の保証は plan 側の
  責務。
- `Expr::Field` は新しい式 variant であり、将来の expr
  consumer（新しい evaluator/serializer）が網羅 match を
  更新し損ねると compile error で検出される — 型で
  fail-closed だが、consumer 追加時の更新は必要。
- `synvelis-script-luau` の adapter は interim 実装で
  あり、RF-SCRIPT 正式 backend contract（ruviform#559）
  との整合は同 gate で再検証が必要。session 分離は VM 単位
  で保証するが、budget は instruction 数カウントの近似で
  あり hard real-time 保証ではない。
- `AutomationError::ScriptBackend` は `Box` 化により Err
  サイズを維持しているが、今後 variant を増やす際は同じ
  制約（128B）に注意が必要。

## Follow-ups

- ruviform#559: Luau/Rhai 同一バイナリ PoC 結合 — interim
  `synvelis-script-luau` adapter の正式 contract 整合・
  Rhai backend・VM state/continuation 設計はこの gate で
  再検証。
- synvelis#262: CLI `workflow run --file` 入口 —
  `workflow_source_kind`/`compile_workflow_yaml`/
  `import_workflow_document` の consumer 側配線（リース外）。
- synvelis#265: Coordinator との実 Plan/Job 結合受入 —
  import 解決済み `WorkflowDefinition` の実行側検証。
- synvelis#267: 製品 E2E 受入（GUI/CLI/実 OS）。
- `Expr::Field` を YAML 面へ露出するかの判断 — 現状は
  importer 生成の内部 variant に限定。
