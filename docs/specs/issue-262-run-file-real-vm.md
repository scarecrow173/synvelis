---
id: "SPEC-034YBx9Rt4wQzHj2KdN"
type: "spec"
status: "implemented"
title: "issue-262-run-file-real-vm — synvelis-cli run --file 実 VM/実 authority 実行配線"
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
  depends-on:
    - "docs/impl/ir/sv-script-03-luau-backend.md"
    - "docs/impl/ir/autx-06-mixed-source-run.md"
    - "docs/impl/ir/autx-03-headless-cli.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/autx-03-headless-cli.md"
    - "docs/automation/design-script.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-262-run-file-real-vm.md"
  derived-by:
    - "docs/designs/issue-262-run-file-real-vm.md"
    - "docs/plans/issue-262-run-file-real-vm.md"
    - "docs/test-specs/issue-262-run-file-real-vm.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-262-run-file-real-vm.md"
  references: []
  defers: []
  deferred-by: []
---

# issue-262-run-file-real-vm — run --file 実実行配線

## What Should Be Built

`synvelis-cli workflow run --file`（`.yaml`/`.yml`/`.luau`）と
`action run --file`（`.luau`）を、分類のみの deferred 実装から
実 headless 実行へ置き換える。パイプラインは

`source capture/evaluation → typed definition/recipe → plan
compile → WorkflowCoordinator + GatewayStepLane/GatewayJobLane +
MemoryRunJournal → versioned envelope/exit code`

で、単一 Gateway・単一 CLI adapter・単一 commit boundary・
single-source registry capture を全経路で共有する。

## Why Now

sv273（PR #505）で `.luau` の実 VM 評価と typed definition ま
でが merge され、AUTX-06（PR #503）で YAML/Luau 同一
coordinator FSM の配線正本が確定した。残る seam
（`HeadlessSessionHost` 共有 handle・canonical surface・
`Booted` capture）は本 slice の lease であり、Issue #262 の
中核 AC「実バイナリで ScriptKind/Version に対応した
Project/JSON/Exit 結果が既存 Gateway/Plan と一致する」を
Windows headless E2E まで進められる。

## Users and Value

- Synvelis 開発者/CI: workflow/action file を実バイナリで実行し、
  Gateway 権威の typed envelope と exit code を得られる。
- #274（GUI editor/analyzer 結合）: canonical
  `ScriptSurfaceSpec` 生成器を automation に置くことで、同一の
  命名規則を consume できる。

## Scope

### In Scope

- `synvelis-gateway`: `HeadlessSessionHost` 共有 handle seam
  （`gateway_handle`/`cli_handle`/`committer`/`admin` getter。
  既存 `new`/`without_commit_boundary` は非破壊）。
- `synvelis-automation`: canonical `ScriptSurfaceSpec` 生成器
  `surface.rs`（`surface_for_snapshot`/`script_name_for`）、
  `ArtifactKind::ActionRecipe` 版 evaluate helper
  `evaluate_luau_action`、importer/lib export 更新。
- `synvelis-cli`: `Booted` への live registry/surface/model/
  bindings/gateway/cli/commit/admin capture、`script.rs` の
  workflow/action `run --file` 実配線、`--inputs`/`--inputs-file`、
  envelope 正直写像、e2e golden 更新。
- docs chain（slug `issue-262-run-file-real-vm`）と
  `evidence/sv262/` ledger。

### Out of Scope

- `.rhai` backend（PoC-only・UnsupportedSource 維持）、Lua 5.4
  reject、未知拡張子・kind/version/backend mismatch の
  fail-closed は維持するが新規対応はしない。
- `script run`/`--script`（存在しないまま）。
- 実 job step 実行（JobEngine/`synvelis.job.*` cap 不在 —
  typed lane error で fail-closed）、named macro 実解決
  （`MacroRegistry::empty()` — compile 時 typed reject）、
  approval→apply 継続（auto-approve 禁止）、durable run
  journal、`workflow status|cancel|validate|preview` verb。
- 製品 E2E・3OS 実機（#267 gate）・macOS/Linux 実機・実
  2-process lock・OS credential。
- 新規依存追加、`synvelis-script`/`synvelis-script-luau` の変更、
  `crates/synvelis-automation/tests/` への追加、docs index
  （`docs/*/README.md`）の編集・再生成。
- Issue #262 の close・AC checkbox 更新（OPEN 継続、ledger が
  tracking 権威）。

## Requirements

- 単一 Gateway/CLI adapter/commit boundary を composition root
  （`boot_project`）で共有し、live lanes・script surface・
  registry compile が全て同じ権威を見る。
- `.luau` は vendored `synvelis-script-luau` backend のみで評価
  し、surface/model/bindings は boot 時の live capture から
  canonical 生成する（手書き fixture の本番コピー禁止）。
- `WorkflowCoordinator` が run progress/journal/FSM/budget/
  replay/indeterminate を所有し、CLI は終端 state を正直に
  envelope へ写像する（推測で success を捏造しない）。
- mutation は executor 無し登録のため ProposalReady/
  ApprovalRequired で park し、unattended claim の
  approve/apply を自動化しない。
- `--dry-run` は project/FS/job を変更せず、plan compile まで
  進めて `executed:false`+plan_digest を報告する。
- 全失敗は typed（CLI error または Gateway `ErrorEnvelope`）で
  fail-closed。exit code は既存 envelope 写像に従う。

## Acceptance Criteria（本 slice）

- [x] `workflow run --file` `.yaml`/`.yml`/`.luau` が実バイナリで
  `WorkflowCoordinator` + live lanes を駆動し、Completed run が
  outputs/run_id/plan_digest を返す。
- [x] `action run --file` `.luau` が実 VM 評価→`ActionRecipe`→
  `InvocationDraft`→`host::handle` を通り、Gateway の typed
  結果/denial をそのまま返す。
- [x] `--inputs <json>`/`--inputs-file <path>` が動作し、同時指定は
  usage error、宣言 input 不足は typed invalid input。
- [x] `.rhai` PoC Unsupported・`.lua` reject・未知拡張子・
  kind mismatch・`script run`/`--script` 不在が維持される。
- [x] query step が live lane 経由で Gateway に到達し、empty
  policy store の typed `denied`（exit 3）を返す。
- [x] `--dry-run` が project/FS を変更せず `executed:false` を返す。
- [x] Windows headless 実バイナリ E2E green —
  `evidence/sv262/` ledger に記録。

## Deferred Design Concerns

- 実 job run・macro 実解決・approval→apply・durable journal:
  全て外部 gate（#267・#274・AUTX-04 系）— NotRun/Deferred。
- macOS/Linux build・3OS 実機受入: 本環境（Windows）では
  NotRun、#267 gate。
- Issue #262 全体の AC checkbox は未更新のまま — 残 AC は別
  Issue gate が所有。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/262>
- Discovery: `docs/discovery/issue-262-run-file-real-vm.md`
- Design: `docs/designs/issue-262-run-file-real-vm.md`
- Test spec: `docs/test-specs/issue-262-run-file-real-vm.md`
- Evidence: `evidence/sv262/issue-262-run-file-real-vm-ledger.md`
