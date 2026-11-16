---
id: "DES-034YBy2Ks8xNvTc4QwE"
type: "design"
status: "approved"
title: "issue-262-run-file-real-vm — run --file 実実行の設計"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/262"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-262-run-file-real-vm.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-262-run-file-real-vm.md"
    - "docs/impl/ir/autx-06-mixed-source-run.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/autx-03-headless-cli.md"
    - "docs/automation/design-script.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-262-run-file-real-vm.md"
  derived-by:
    - "docs/plans/issue-262-run-file-real-vm.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-262-run-file-real-vm — 設計

## 全体構成

```
--file source
  → classify (拡張子×subcommand 行列、fail-closed)
  → read_source (canonicalize + UTF-8)
  → host::boot_project (live capture: descriptors/registry/
     surface→model→bindings/gateway/cli/commit/admin)
  ├─ workflow .yaml/.yml
  │    → parse_workflow_yaml → PlanCompiler::compile_with_macros
  │      (MacroRegistry::empty — named macro は compile 時 reject)
  ├─ workflow .luau
  │    → evaluate_luau_spec (vendored LuauBackend + budget)
  │    → WorkflowScriptBuilder::build → import document
  │    → FileWorkflowResolver::with_script_backend
  │      (canonicalize 済み親 dir root)
  │    → import_workflow_document → PlanCompiler
  └─ action .luau
       → evaluate_luau_action → RecipeSpec
       → ScriptActionDefinition (file metadata pin)
       → ActionRecipe::build → Single=to_draft /
         Batch=compile_action_recipe→to_batch_draft
       → host::handle (claim→attest→gateway)

workflow compiled → RunContext/RunMeta → MemoryRunJournal +
  RecordingStepLane(GatewayStepLane) + GatewayJobLane →
  WorkflowCoordinator::start_run → drive → state→envelope 写像
```

## 主要な設計判断

### 1. `HeadlessSessionHost` 共有 handle seam（composition-time 注入）

`HeadlessSessionHost` の `gateway`/`cli`/`committer`/`admin` を
`Arc` 化し、`gateway_handle()`/`cli_handle()`/`committer()`/
`admin()` getter で clone を返す。`new`/
`without_commit_boundary` の signature は不変（内部で Arc wrap）
— in-repo 呼出 3 箇所は無変更で compile できる。

cli `boot_project`/`boot_discovery` が boot 時に
`live_registry::capture(&gateway)` →
`synvelis_script::RegistrySnapshot::capture` →
`surface_for_snapshot` → `ScriptApiModel::build` →
`BindingSet::generate` を実行し `Booted` に保持する
（既存 `snapshot_descriptors` boot-capture パターン踏襲）。
これにより script surface・plan compile・live lanes が全て
同一の live registry 権威を見る。

### 2. canonical `ScriptSurfaceSpec` 生成器（automation `surface.rs`）

命名規則（deterministic・固定）:

1. capability id から先頭 `synvelis.` を除去（foreign prefix は保持）
2. `.`/`-`/`_` で分割し空 segment を除去
3. camelCase（先頭 lower、以降 capitalize）
   - `synvelis.context.query` → `contextQuery`
   - `synvelis.project.add-note` → `projectAddNote`

衝突規則: 同名を導出する 2 件目（別 id、または同 id の別
version — version 違いも script 名を shadow するため曖昧）は
`ScriptApiError::DuplicateApiName` で fail-closed。Luau
identifier 不適合名は entry を emit したまま model の
`InvalidName` verdict に委ねる（1 件の不正名で registry 全体を
隠さない）。

配置は `synvelis-automation` 固定 — `synvelis-script` は命名
無関知の汎用 crate、cli は #274 と lease を共有できない。

### 3. workflow 実実行

- `.yaml`/`.yml`: `parse_workflow_yaml` →
  `PlanCompiler::compile_with_macros(&definition, inputs,
  &booted.registry, &MacroRegistry::empty())`。
- `.luau`: `evaluate_luau_spec` → `WorkflowScriptBuilder::build`
  （live model/bindings + `AvailableBackends::luau()`）→
  import document → `FileWorkflowResolver::with_script_backend`
  （`--file` canonicalize 済み親 dir root、同一 backend/budget/
  build context）→ `import_workflow_document` → 同上 compile。
- `RunMeta`: principal=Agent（`args.principal`、claim 由来）、
  project_id=booted binding の tagged id、
  expected_revision=`host::live_revision`、idempotency_key/run_id
  は process-unique nonce で自動生成、policy_epoch=None
  （既存 oracle 経路に委譲）、task/grant=None。
- lane: `RecordingStepLane`（`GatewayStepLane` を wrap し最後の
  typed `ErrorEnvelope` を保持 — journal は verdict のみで
  envelope の exit code 写像に authoritative error が必要）、
  `GatewayJobLane`（JobEngine 不在のため job step は typed
  lane error で fail-closed）、`MemoryRunJournal`。
- `WorkflowCoordinator::start_run` → `drive` で単一 invocation
  run-to-completion。

### 4. action 実実行

`evaluate_luau_action`（新設 sibling helper、
`ArtifactKind::ActionRecipe` 固定）→ payload を `RecipeSpec`
decode → file metadata 由来の `ScriptActionDefinition`
（script_id=file stem、version=1、language/backend=Luau、
source_digest=sha256、schema refs=先頭 member の解決済み
binding entry）→ `AvailableBackends::luau()` + live snapshot で
`validate` → `ActionRecipe::build` → project pin 検証
（script 宣言 project ≠ `--project` binding → invalid input）
→ Single=`to_draft`/`verify_wire`/`to_wire_value`、
Batch=`compile_action_recipe`→`to_batch_draft`→同経路 →
`host::handle`（他の全 CLI invocation と同一 claim path）。

### 5. envelope 正直写像

| run.state | envelope | exit |
| --- | --- | --- |
| `Completed` | `completed` + outputs/run_id/plan_digest/state | 0 |
| `Approval` | `approval-required`/`proposal-ready` + references{run_id,proposals,approvals,resume:"not-supported"} | 0 |
| `Failed` + lane error | `failed` + gateway `ErrorEnvelope` verbatim | envelope_exit（denied→3 等） |
| `Failed`（run-level） | `failed` + `cli.run-failed` + checkpoint detail | 6 |
| その他（Waiting/Indeterminate/CancelRequested 等） | `accepted` + real state + pending_jobs | 7 |
| `drive` error | `failed` + `cli.run-failed`（Eval/Budget→6、他→10） | 6/10 |

`references.resume` は in-memory proposal store のため
cross-invocation resume を約束しないことを明示。

### 6. `--inputs`/`--inputs-file`

`workflow run` のみ受理（`action run` は inputs を取らない）。
両 flag 同時指定は usage error。JSON object 必須、file は
UTF-8 JSON として読む。declared input は全て必須
（`InputDecl` に default なし）— 不足・型不一致は
compile/coordinator の typed error → invalid input。

### 7. dry-run

readonly boot（durable policy/journal/commit を作らない）で
compile まで進め、`executed:false`/`dry_run:true`/
plan_digest/plan_steps/warnings を報告。`.rhai` dry-run は
boot せず classification を正直に報告。

## エラー写像

- `AutomationError::Script(ScriptApiError::Gateway)` →
  `ErrorEnvelope` を保持し `commands::error_result` で描画
  （authoritative exit code）。
- `ScriptApiError::Internal` → `CliError::Internal`(10)。
- その他 script/automation error → `CliError::InvalidInput`(2)。
- `AutomationError::Registry` → `CliError::Internal`(10)。
- classify/read_source/usage/project 不在 → 既存 cli.* code。

## 非回帰

- `.rhai`: PoC-only `Unsupported`(5) 維持（dry-run のみ分類報告）。
- `.lua`: `Unsupported`(5)、未知拡張子・kind mismatch:
  `InvalidInput`(2)、`script run`/`--script`: `Usage`(2)。
- `action list/describe/invoke/prepare`、`project info` 等の
  既存経路は無変更。
