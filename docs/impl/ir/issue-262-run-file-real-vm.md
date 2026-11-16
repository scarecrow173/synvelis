---
id: "IMPL-034YBzF7hR3tYvNc6WdE"
type: "impl"
status: "completed"
title: "issue-262-run-file-real-vm — run --file 実 VM/実 authority 実行配線"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/262"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-automation/src/surface.rs"
      - type: "file"
        path: "docs/specs/issue-262-run-file-real-vm.md"
      - type: "file"
        path: "docs/designs/issue-262-run-file-real-vm.md"
      - type: "file"
        path: "docs/plans/issue-262-run-file-real-vm.md"
      - type: "file"
        path: "docs/tasks/issue-262-run-file-real-vm.md"
      - type: "file"
        path: "docs/test-specs/issue-262-run-file-real-vm.md"
      - type: "file"
        path: "docs/discovery/issue-262-run-file-real-vm.md"
      - type: "file"
        path: "docs/impl/ir/issue-262-run-file-real-vm.md"
      - type: "file"
        path: "docs/impl/exp/issue-262-run-file-real-vm.md"
      - type: "file"
        path: "docs/impl/exp/issue-262-run-file-real-vm-exp.jsonl"
      - type: "file"
        path: "evidence/sv262/issue-262-run-file-real-vm-ledger.md"
    modified:
      - type: "file"
        path: "crates/synvelis-automation/src/importer.rs"
      - type: "file"
        path: "crates/synvelis-automation/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-cli/src/args.rs"
      - type: "file"
        path: "crates/synvelis-cli/src/commands.rs"
      - type: "file"
        path: "crates/synvelis-cli/src/host.rs"
      - type: "file"
        path: "crates/synvelis-cli/src/script.rs"
      - type: "file"
        path: "crates/synvelis-cli/tests/cli_e2e.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/headless.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/issue-262-run-file-real-vm.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/issue-262-run-file-real-vm.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-262-run-file-real-vm.md"
    - "docs/impl/exp/issue-262-run-file-real-vm-exp.jsonl"
    - "evidence/sv262/issue-262-run-file-real-vm-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-262-run-file-real-vm.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-262-run-file-real-vm.md"
  references: []
  defers: []
  deferred-by: []
---

# issue-262-run-file-real-vm — Implementation Record

## 実装サマリ

`synvelis-cli workflow run --file`（`.yaml`/`.yml`/`.luau`）と
`action run --file`（`.luau`）を、分類+deferred 実装から実
headless 実行へ置き換えた。単一 Gateway/CLI adapter/commit
boundary を composition root で共有し、live lanes・script
surface・registry compile が同一権威を見る。

## 変更点

### `crates/synvelis-gateway/src/headless.rs`

`gateway`/`cli`/`committer`/`admin` を `Arc` 化し、
`gateway_handle()`/`cli_handle()`/`committer()`/`admin()` getter
を追加。`new`/`without_commit_boundary` の signature は不変
（内部で Arc wrap）— 既存呼出は無変更。

### `crates/synvelis-automation/src/surface.rs`（新設）

canonical `ScriptSurfaceSpec` 生成器。命名規則: `synvelis.`
除去→`.`/`-`/`_` 分割→camelCase。衝突（別 id 同名・同 id 別
version）は `DuplicateApiName` fail-closed。identifier 不適合名
は emit して model の `InvalidName` verdict に委ねる。
unit test 6 件を module 内 `#[cfg(test)]` に配置。

### `crates/synvelis-automation/src/importer.rs` / `lib.rs`

`evaluate_luau_action`（`ArtifactKind::ActionRecipe` 固定の
evaluate sibling helper）を追加し export。`surface` module を
公開。

### `crates/synvelis-cli/src/host.rs`

`Booted` に live capture を追加: descriptors、
`RegistrySnapshot`、`surface_for_snapshot`→`ScriptApiModel::
build`→`BindingSet::generate` の script stack、共有
`gateway`/`cli`/`committer`/`admin` handle。`boot_project`/
`boot_discovery` の read-only/durable 両経路で capture
（既存 `snapshot_descriptors` boot-capture パターン踏襲）。
`live_revision` helper を公開。

### `crates/synvelis-cli/src/args.rs`

`Command::WorkflowRun` に `inputs: Option<Value>` /
`inputs_file: Option<PathBuf>` を追加。`--inputs <json>` を
parse 時に JSON decode、`--inputs-file <path>` を path として
保持、両者同時指定は usage error。`action run` は inputs を
取らない。

### `crates/synvelis-cli/src/commands.rs`

`WorkflowRun` dispatch を新 variant に更新し
`script::run_file_command` へ inputs を受け渡す。
`result`/`outcome_result`/`error_result` を crate 公開
（script.rs の envelope 描画用）。未使用 import/helper を除去。

### `crates/synvelis-cli/src/script.rs`（全面書換）

- 分類行列・`read_source`・poc 判定は維持。
- `compile_workflow`: `.yaml`→`parse_workflow_yaml`、
  `.luau`→`evaluate_luau_spec`→`WorkflowScriptBuilder::build`→
  `FileWorkflowResolver::with_script_backend`（canonicalize
  済み親 dir root）→`import_workflow_document`。共に
  `PlanCompiler::compile_with_macros`+`MacroRegistry::empty()`。
- `drive_workflow`: `RunContext`/`RunMeta`（Agent principal・
  tagged project・live revision pin・process-unique
  run/idempotency）→`MemoryRunJournal`+`RecordingStepLane`
  （`GatewayStepLane` wrap、最後の `ErrorEnvelope` 保持）+
  `GatewayJobLane`→`WorkflowCoordinator::start_run`→`drive`。
- `run_state_result`: Completed→completed/0+outputs、
  Approval→approval-required/proposal-ready/0+references
  （resume 非対応を明示）、Failed→lane error verbatim または
  `cli.run-failed`/6、非終端→accepted/7。
- `run_action`: `evaluate_luau_action`→`RecipeSpec`→
  `ScriptActionDefinition`（file metadata pin）→validate→
  `ActionRecipe::build`→project pin 検証→Single=`to_draft`/
  Batch=`compile_action_recipe`→`to_batch_draft`→
  `verify_wire`→`host::handle`。dry-run は recipe metadata のみ。
- `resolve_inputs`: `--inputs`/`--inputs-file` を JSON object
  に解決、相互排他・非 object を fail-closed。
- `FileFailure`: CLI error と typed `ErrorEnvelope` の 2 経路を
  分離し、boot 後に `commands::error_result` で描画。

### `crates/synvelis-cli/tests/cli_e2e.rs`

- `file_classification_matrix`: fixture を saved bundle
  （`project_dir`）に変更し、実実行後の期待 exit を更新
  （`.luau`/`.yaml` の内容不正 → 2、`.rhai`/`.lua` → 5 維持）。
- `workflow_luau_file_executes_to_completion`: 旧
  `evaluated:false` 期待を実実行（`executed:true`/
  `state=completed`/outputs）へ更新。
- dry-run 2 件: project fixture 化（readonly boot が saved
  bundle を要求するため）。
- 新規 golden: `workflow_run_yaml_executes_to_completion`、
  `workflow_run_inputs_inline_and_file`（--inputs/
  --inputs-file/不足/相互排他）、
  `workflow_run_query_step_surfaces_typed_denial`（live lane→
  Gateway `denied`/exit 3）、
  `action_run_luau_reaches_gateway_and_surfaces_denial`、
  `action_run_luau_dry_run_reports_recipe_without_executing`。

## 設計上の注意

- `HeadlessSessionHost` の getter 追加は composition-time 注入
  の共有を明示する seam であり、authority の複製ではない —
  lanes・script stack・compile が全て同一 `Gateway`/`LocalCliAdapter`
  を参照する。
- `MacroRegistry::empty()` は named macro を compile 時 typed
  reject に留める意図的な fail-closed（macro load 経路は
  AUTX-04 後続）。
- `policy_epoch=None`・`task=None`・`grant=None` — 既存 oracle
  経路・unattended claim の範囲内で authority を mint しない。
- in-memory proposal store のため Approval references は
  cross-invocation resume を約束しない（envelope に明記）。

## 検証

`evidence/sv262/issue-262-run-file-real-vm-ledger.md` に全
コマンド・exit code・結果を記録。Windows headless 実バイナリ
E2E 24/24 green。
