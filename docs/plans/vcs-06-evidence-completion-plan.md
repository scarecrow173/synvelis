---
id: "PLAN-4RRd44dWWEduAvHmubiVw"
type: "plan"
status: "approved"
title: "VCS-06 evidence completion — 実装 plan"
created: "2026-11-11"
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
    - "SPEC-034VNNGsOaycnOBjCAH3m1"
  implemented-by: []
  depends-on:
    - "DESIGN-TR6hK0f243aEk3CmFQlfp"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "PLAN-FdgAC8xT08aXjvUlT4GwSz"
    - "SPEC-034V2wnQPWFJdKz68KlwJG"
  refines: []
  refined-by: []
  derives-from:
    - "DESIGN-TR6hK0f243aEk3CmFQlfp"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/reports/vcs-06-compat-evidence.md"
    - "crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs"
  defers: []
  deferred-by: []
---

# VCS-06 evidence completion — 実装 plan

## Scope split

単一 slice。2 系統の evidence 補完（RESTORE-08 fixture /
COMPAT-03/04 CI 記録）+ doc chain で構成し、`src/` 変更・
ledger 編集を含まない。

## File map

| # | ファイル | 操作 | 内容 |
| --- | --- | --- | --- |
| 1 | `crates/synvelis-project/tests/vcs06_plugin_instance_matrix.rs` | add | RESTORE-08 fixture test 1 件 + `plugin_state_source` helper |
| 2 | `evidence/reports/vcs-06-compat-evidence.md` | add | CI run URL/outcome 記録（required/observed 区別） |
| 3 | `docs/specs/vcs-06-evidence-completion.md` | add | spec |
| 4 | `docs/designs/vcs-06-evidence-completion-design.md` | add | design |
| 5 | `docs/plans/vcs-06-evidence-completion-plan.md` | add | plan（本書） |
| 6 | `docs/tasks/vcs-06-evidence-completion-task.md` | add | task |
| 7 | `docs/test-specs/vcs-06-evidence-completion.md` | add | test-spec |
| 8 | `docs/impl/ir/vcs-06-evidence-completion.md` | add | implementation record |

## 依存関係

- RESTORE-08 は `vcs06_monitoring_matrix.rs` の
  `restore_as_copy_preserves_monitoring_verbatim` と
  `vcs06_restore_matrix.rs` の plugin-state helper を先例とし、
  `src/` 非依存の test-only 追加。
- COMPAT 記録は `gh` CLI の実測値に依存。`ci.yml` の
  required/observed 定義は `tools/ci-summary.mjs` が正本。
- doc chain は spec → design → plan → task/test-spec → impl-ir
  の順で relations を張る。

## Risk register

| リスク | 影響 | 対処 |
| --- | --- | --- |
| ledger ファイル占有 | 行追加不可 | 編集せず PR 本文で統合依頼 |
| `too_many_lines` clippy | lint 失敗 | store 構築を helper 化（済） |
| CI run の outcome が failure | evidence の解釈 | required/observed と leg 単位で正直に記録。webdav 失敗は VCS-06 非関連と区別 |
| `docs/*/README.md` 再生成要求 | 範囲外変更 | `--no-index` 相当で index 非更新を維持 |

## Verification matrix

| 検証 | 方法 | 合格条件 |
| --- | --- | --- |
| RESTORE-08 fixture | `cargo test -p synvelis-project --test vcs06_plugin_instance_matrix` | 1 test pass・envelope readback で verbatim pin |
| fmt | `mise run fmt-check` | 差分なし |
| check | `mise run check` | workspace check pass |
| lint | `mise run lint` | clippy `-D warnings` pass |
| test | `mise run test` | workspace test pass（回帰なし） |
| RT | `mise run test-rt` | RT lane pass |
| MSRV | `mise run test-msrv` | MSRV check pass |
| build | `mise run build` | workspace build pass |
| COMPAT 記録 | `gh pr checks` / `gh run list` / `gh run view` の実測転記 | URL・outcome・required/observed が実値と一致 |

## Rollback

全変更が新規ファイルのため、revert は当該 commit の取り消しで
完結する。`src/`・ledger・README への変更は存在しない。

## 未対応の作業（Coordinator 統合依頼）

- `evidence/reports/vcs-06-fixture-matrix-ledger.md` への
  VCS-RESTORE-08 行追加（verdict は本 fixture の実測に基づき
  Coordinator が判定）。
- 同 ledger の VCS-COMPAT-03/04 行更新（`vcs-06-compat-
  evidence.md` の実測を根拠に）。
- `docs/*/README.md` index 再生成（本 slice では対象外）。
