---
id: "DISC-034YBx7Qm2vNpLkR9sTd"
type: "discovery"
status: "completed"
title: "issue-262-run-file-real-vm — run --file 実実行の探索結果"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/sv-script-03-luau-backend.md"
    - "docs/impl/ir/autx-06-mixed-source-run.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-262-run-file-real-vm.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-262-run-file-real-vm — 探索結果

## 課題の framing

synvelis#262 の `synvelis-cli action run --file` / `workflow run
--file` は、sv273 slice（PR #505）まで「分類 + `.luau` 評価 +
typed definition 報告」までで、plan compile 以降は defer されて
いた（`executed: false` / `Unsupported`）。本 slice は
capture → plan compile → `WorkflowCoordinator` + live lanes →
envelope の実配線を行う。

## 探索で確定した事実

- **seam blocker（解決済み方針）**: `HeadlessSessionHost` は
  `gateway: Mutex<Gateway>` + `cli: LocalCliAdapter` を非共有保持
  しており、live lanes が要求する `Arc<Mutex<Gateway>>` /
  `Arc<LocalCliAdapter>` を満たせない。composition-time 注入
  （cli `boot_project` が Arc を組み立て host 構築）が正しい境界。
  in-repo 呼出は cli host.rs・gateway tests/cli_adapter.rs・
  workspace tests/autx08_e2e.rs の 3 箇所のみ。
- **canonical surface 不在**: `synvelis-script` は命名無関知の
  汎用 crate で、`ScriptSurfaceSpec` を consume するが生成しない。
  test fixture の手書き surface を本番へコピーするわけにはいかない。
  命名規則 authority は `synvelis-automation` に置く
  （#274 GUI/analyzer 結合が consume できる境界）。
- **parity 正本**: `crates/synvelis-automation/tests/mixed_source_run.rs`
  （AUTX-06 / PR #503）が YAML/Luau 同一 coordinator FSM の配線
  正本。`RunContext.definition` は `WorkflowScriptBuilder::build`
  → `import_workflow_document` の single-source 再構成、
  `FileWorkflowResolver::with_script_backend` を `--file` の
  canonicalize 済み親 dir に root する。
- **ActionRecipe 契約**: `ArtifactKind::ActionRecipe` 版の
  evaluate helper が `synvelis-automation` に必要
  （`evaluate_luau_spec` は `WorkflowDefinition` 固定）。
  payload→`RecipeSpec` decode→`ScriptActionDefinition`+
  `ActionRecipe::build`→Single=`to_draft`/Batch=
  `compile_action_recipe`→`to_batch_draft`→`host::handle`。
- **権限モデル**: unattended claim は delegated `Agent` caller。
  TTY/`--yes`/env/script source/JSON field は Human authority を
  mint しない。mutation step は executor 無し登録のため
  ProposalReady/ApprovalRequired 経路で park し、auto-approve
  しない。
- **既存 e2e 期待値**: `cli_e2e.rs` の file_classification_matrix・
  luau workflow・dry-run 系は旧 deferred 挙動を期待しており、
  実実行化に伴い fixture project（saved bundle）と期待 exit
  code の更新が必要。

## gap analysis

| 項目 | 状態 |
| --- | --- |
| `.luau` evaluate（workflow/action） | sv273 で workflow のみ済 → action 版 helper を追加 |
| canonical `ScriptSurfaceSpec` 生成 | 未存在 → automation `surface.rs` を新設 |
| `HeadlessSessionHost` 共有 handle | 未存在 → composition-time 注入 seam を追加 |
| `Booted` registry/surface/model/bindings capture | 未存在 → boot-capture 追加 |
| workflow `.yaml`/`.luau` 実実行 | 未配線 → `PlanCompiler` + `WorkflowCoordinator` + lanes |
| action `.luau` 実実行 | 未配線 → recipe→draft→`host::handle` |
| `--inputs`/`--inputs-file` | 未存在 → args + 入力解決を追加 |
| envelope 正直写像 | 未定義 → run state → status/exit 写像を新設 |

## 代替案の比較

- **host getter 追加 vs composition-time 注入**: getter は共有を
  暗黙化する。composition-time 注入は「誰が Arc を持つか」を
  composition root で明示でき、既存 `new`/`without_commit_boundary`
  を非破壊に保てる → 採用。
- **surface 生成を cli に置く案**: cli は #274 と lease を共有
  できず、命名規則が cli 固有に閉じる → automation に配置。
- **`synvelis-script` に生成器を置く案**: 同 crate は命名無関知
  が契約 → 不採用。

## 残る不確実性

- 実 job step（await-job/async-submit）・named macro 実解決・
  approval→apply 継続・durable journal は外部 gate
  （#267/#274/AUTX-04 系）— 本 slice では typed fail-closed と
  NotRun 記録に留める。
