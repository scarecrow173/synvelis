---
id: "TEST-034YBzC4xwQyNe8RtUb"
type: "test-spec"
status: "implemented"
title: "issue-262-run-file-real-vm — test spec"
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
    - "docs/designs/issue-262-run-file-real-vm.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-03-headless-cli.md"
    - "docs/test-specs/autx-06-mixed-source-run.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-262-run-file-real-vm.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-262-run-file-real-vm.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-262-run-file-real-vm — test spec

## Feature: run --file 実実行

### Rule: workflow `.yaml`/`.yml` 実実行

- **Example**: emit-only workflow（`kind: emit-result`、
  output `out`=42）を `workflow run --file wf.yaml --project
  <saved-bundle>` で実行 → exit 0、`status=completed`、
  `result.executed=true`、`result.state="completed"`、
  `result.outputs.out=42`、`run_id`/`plan_digest`(64hex) 存在、
  `kind="workflow-dsl"`。
- **Example**: `--inputs '{"count":7}'` / `--inputs-file` で
  `$input.count` を emit → outputs が入力値を反映。
- **Example**: 宣言 input 不足 → exit 2 `cli.invalid-input`。
- **Example**: `--inputs`+`--inputs-file` 同時 → exit 2
  `cli.usage`。

### Rule: workflow `.luau` 実実行

- **Example**: `return { schema_version=1, workflow_id=...,
  steps={{call="emit",...}}, outputs={...}, limits={...} }` を
  `workflow run --file wf.luau` で実行 → 実 VM 評価→plan→
  coordinator→`completed`/exit 0、`evaluated=true`、
  `executed=true`、outputs 反映。

### Rule: live lane → Gateway typed 結果

- **Example**: query step（`synvelis.context.query@1.0`）を含む
  workflow → empty policy store で `denied` → run `Failed`、
  exit 3、`error.code="denied"`（gateway envelope verbatim）。
- **Example**: `.luau` action script（`contextQuery` member）→
  実 VM→`ActionRecipe`→wire→`denied`/exit 3。

### Rule: dry-run 非変更

- **Example**: workflow `.yaml` `--dry-run` → exit 0、
  `executed=false`、`dry_run=true`、`definition_valid=true`、
  plan_digest 報告、project/FS 非変更。
- **Example**: action `.luau` `--dry-run` → exit 0、
  `evaluated=true`、`executed=false`、`recipe_kind="single"`、
  `members=1`、`recipe_digest` 報告。
- **Example**: `.rhai` dry-run → classification のみ報告
  （`backend_available=false`）、boot しない。

### Rule: fail-closed 分類行列（維持）

- **Example**: `.rhai`（両 side）→ exit 5 `cli.unsupported`。
- **Example**: `.lua`（両 side）→ exit 5。
- **Example**: `.yaml` under `action run`・未知拡張子・
  no-extension → exit 2 `cli.invalid-input`。
- **Example**: 内容が不正な `.luau`/`.yaml` → 評価/parse 失敗で
  exit 2（分類は parseability を意味しない）。
- **Example**: `script run`・`--script` flag → exit 2
  `cli.usage`。

### Rule: surface 生成器（unit）

- **Example**: `synvelis.context.query`→`contextQuery`、
  `synvelis.project.add-note`→`projectAddNote` の deterministic
  導出。
- **Example**: 異 id 同名導出・同 id 別 version →
  `DuplicateApiName` fail-closed。
- **Example**: Luau identifier 不適合名は emit され model の
  `InvalidName` verdict に委ねる。

## golden fixture 配置

`crates/synvelis-cli/tests/cli_e2e.rs` — 実バイナリ spawn、
exit code + stdout JSON envelope を assert（in-process
shortcut なし）。fixture project は `fixture_project`（saved
bundle, ProjectId=7, revision 1）を使用 — 実実行は saved bundle
を要求するため空 dir は不可。
