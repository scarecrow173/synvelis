---
id: "TASK-034VpC1mzY9bzJDEjAIDKv"
type: "task"
status: "done"
title: "VCS-06 UI headless matrix"
created: "2026-11-10"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/vcs-06-ui-headless-matrix-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/vcs-06-ui-headless-matrix-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-06-ui-headless-matrix.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-06 UI headless matrix

## Work

`crates/synvelis-app/tests/vcs06_ui_matrix.rs` を新規追加し、
`syn.vcs.view`/`syn.vcs.visual`/`vcs_history_view`/
`history_screen_spec` の外部観測契約で欠損行 3 系統（異
project_id → unknown、asset `Absent` → Partial+issue 行、staged
freshness Unknown 経路）を固定する。`src/` 変更なし。
`evidence/reports/vcs-06-fixture-matrix-ledger.md` の VCS-UI-01..05
行を headless Pass* / 実機 NotRun に分割記録する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/vcs-06-ui-headless-matrix-plan.md`
- `docs/specs/vcs-06-ui-headless-matrix.md`
- `docs/designs/vcs-06-ui-headless-matrix-design.md`
- `docs/test-specs/vcs-06-ui-headless-matrix.md`

## Dependencies

- なし（VCS-01..05 headless 実装は main 統合済み、PR#252/#268）。

## Affected Files

- `crates/synvelis-app/tests/vcs06_ui_matrix.rs`（新規）
- `evidence/reports/vcs-06-fixture-matrix-ledger.md`（更新）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  vcs-06-ui-headless-matrix*.md`（新規）
- `docs/*/README.md` index（doc tooling による再生成）

## Done When

- [x] `vcs06_ui_matrix.rs` 4 tests が外部観測契約を検証し pass
- [x] ledger の VCS-UI-01..05 行が分割記録される
- [x] doc チェーン + index 再生成
- [x] `git fetch origin main && git merge origin/main` 後の
  canonical `mise run` 検証が pass
- [x] push → PR 作成（Refs synvelis#97、Issue は Open のまま）

## Verification

- [x] `cargo test -p synvelis-app --test vcs06_ui_matrix`（補助）
- [x] `cargo clippy -p synvelis-app --all-targets -- -D warnings`
  （補助）
- [x] `cargo fmt --all -- --check`（補助）
- [x] `mise run fmt-check check lint test test-rt test-msrv
  check-app-native lint-app-native`（canonical、全 exit 0）
