---
id: "PLAN-034YBz5Lu1tRwVb8NxFc"
type: "plan"
status: "approved"
title: "issue-262-run-file-real-vm — 実装 plan"
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
    - "docs/designs/issue-262-run-file-real-vm.md"
  implemented-by: []
  depends-on:
    - "docs/designs/issue-262-run-file-real-vm.md"
    - "docs/specs/issue-262-run-file-real-vm.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-262-run-file-real-vm.md"
  derived-by:
    - "docs/tasks/issue-262-run-file-real-vm.md"
    - "docs/test-specs/issue-262-run-file-real-vm.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# issue-262-run-file-real-vm — plan

## file map

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-gateway/src/headless.rs` | 共有 handle getter 追加（`gateway_handle`/`cli_handle`/`committer`/`admin`）、field の Arc 化 |
| `crates/synvelis-automation/src/surface.rs` | 新設 — canonical `ScriptSurfaceSpec` 生成器 + unit test |
| `crates/synvelis-automation/src/importer.rs` | `evaluate_luau_action`（`ArtifactKind::ActionRecipe` sibling helper）追加 |
| `crates/synvelis-automation/src/lib.rs` | `surface` module + helper export |
| `crates/synvelis-cli/src/host.rs` | `Booted` に live capture 追加（registry/surface/model/bindings/gateway/cli/commit/admin）、`boot_project`/`boot_discovery` で capture、`live_revision` helper |
| `crates/synvelis-cli/src/args.rs` | `WorkflowRun` に `inputs`/`inputs_file`、`--inputs`/`--inputs-file` flag parse + 相互排他 |
| `crates/synvelis-cli/src/commands.rs` | `WorkflowRun` dispatch 更新、`outcome_result`/`error_result`/`result` の crate 公開 |
| `crates/synvelis-cli/src/script.rs` | 分類+deferred 実装を実実行へ全面書換（compile/drive/action/dry-run/envelope 写像） |
| `crates/synvelis-cli/tests/cli_e2e.rs` | stale 期待値更新 + 実実行 golden 追加 |

## dependency graph

```
surface.rs (automation) ──┐
headless.rs seam ─────────┼─→ host.rs Booted capture ──→ script.rs 実行配線
importer.rs helper ───────┘         │
args.rs flags ──────────────────────┘
```

## risk register

| リスク | 対応 |
| --- | --- |
| registry が同一 id 複数 version を持ち surface 衝突 | fail-closed `DuplicateApiName`（設計どおり）。実 registry は現状 single-version |
| `--file` が project 外の nested `workflow()` を参照 | `FileWorkflowResolver` を canonicalize 済み親 dir に root（parity test と同一境界） |
| mutation step が自動承認される | executor 無し登録 → Approval parked、auto-approve 無し |
| dry-run が durable artifact を作る | readonly boot（`!args.dry_run` で durable 切替）+ e2e `dry_run_leaves_project_untouched` |
| lane error が envelope exit code を失う | `RecordingStepLane` で最後の `ErrorEnvelope` を保持 |
| 空 policy store で全 capability denied | e2e は denied(3) を正直な golden として採用、emit-only で Completed(0) を別途検証 |

## verification matrix

| 検証 | コマンド |
| --- | --- |
| fmt | `mise run fmt-check` |
| cli check/lint/test | `mise run check/lint/test synvelis-cli`（test は real-binary e2e 含む） |
| build-cli | `mise run build-cli`（実バイナリ E2E 前提） |
| automation | `mise run check/lint/test synvelis-automation` |
| gateway | `mise run check/lint/test synvelis-gateway` |
| workspace/script 非回帰 | `mise run check synvelis-workspace` / `check synvelis-script` |
| headless deps | `mise run check-headless-deps` |
| hooks | `mise exec -- prek run --all-files` |

NotRun（正直記録）: macOS/Linux build・3OS 実機（#267）、
実 job run・macro 実解決・approval→apply・durable journal、
`gw10`/`test-rt`/MSRV/`*-full` lane。

## rollback

単一 PR revert で旧 deferred 実装へ戻る。外部状態（project
bundle・journal）は in-memory/readonly で永続副作用を持たない
（durable boot の policy.log/commit.journal は project dir 内の
既存機構どおり）。
