---
id: "TASK-034WTzC0pqxJ29jwHg0KLg"
type: "task"
status: "done"
title: "VCS-06 real-git/FS fault rows slice"
created: "2026-11-13"
updated: "2026-11-13"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/97"
  changes:
    added:
      - type: "file"
        path: "docs/specs/vcs-06-real-git-fs-fault.md"
      - type: "file"
        path: "docs/designs/vcs-06-real-git-fs-fault-design.md"
      - type: "file"
        path: "docs/plans/vcs-06-real-git-fs-fault-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-06-real-git-fs-fault-task.md"
      - type: "file"
        path: "docs/test-specs/vcs-06-real-git-fs-fault.md"
      - type: "file"
        path: "docs/impl/ir/vcs-06-real-git-fs-fault.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/vcs06_git_matrix.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_restore_matrix.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/vcs06_apply_matrix.rs"
      - type: "file"
        path: "evidence/reports/vcs-06-compat-evidence.md"
      - type: "file"
        path: "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/README.md"
  implements:
    - "docs/plans/vcs-06-real-git-fs-fault-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/vcs-06-real-git-fs-fault-plan.md"
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
    - "docs/test-specs/vcs-06-real-git-fs-fault.md"
  references:
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    - "evidence/reports/vcs-06-compat-evidence.md"
  defers: []
  deferred-by: []
---

# VCS-06 real-git/FS fault rows slice

## 概要

synvelis#97 VCS-06 の fixture・fault matrix 未消化行のうち
ローカル検証可能な分を補完する test-only slice。
real-git 4 行（packed/GC・pruned RevisionGone・file:// remote
force-push+fetch・linked worktree index scope）と Windows FS
fault 5 行（restore staging 3・apply journal 2）を追加し、
COMPAT 証拠を最新 covering run で更新、ledger に新規行を記録
する。`src/` 変更・新規 dependency なし。

## 作業項目

- [x] Slice A: `vcs06_git_matrix.rs` に real-git 4 test を追加
  （`gc_pack_refs_and_repack_keep_fixed_revision_reads`・
  `pruned_commit_is_revision_gone_not_silent`・
  `file_remote_force_push_fetch_preserves_pinned_revision`・
  `linked_worktree_index_scope_is_isolated`）。
- [x] Slice B: `vcs06_restore_matrix.rs` に Windows FS fault
  3 test、`vcs06_apply_matrix.rs` に 2 test を `#[cfg(windows)]`
  で追加。
- [x] Slice C: `gh run list/view/api` で covering run を実取得し
  `vcs-06-compat-evidence.md` を更新（run 43428177865 全 leg
  success）。
- [x] `vcs-06-fixture-matrix-ledger.md` に VCS-GIT-REAL-01..04・
  VCS-APPLY-08・VCS-RESTORE-09 行と residual NotRun を記録、
  fixture test 数を更新。
- [x] doc chain（spec/design/plan/task/test-spec/impl-ir）を
  `vcs-06-real-git-fs-fault` 命名で作成し index を再生成。
- [x] canonical 検証を実行し impl-ir に記録。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/vcs-06-real-git-fs-fault-plan.md`
- `docs/specs/vcs-06-real-git-fs-fault.md`

## Dependencies

- `VcsGitFixture` / `common::git_fixture` helper（変更不要で
  流用）。
- synvelis#335 post-fix semantics（`Ok(None)`/`RevisionGone`）。

## Affected Files

- `crates/synvelis-library/tests/vcs06_git_matrix.rs`
- `crates/synvelis-project/tests/vcs06_restore_matrix.rs`
- `crates/synvelis-project/tests/vcs06_apply_matrix.rs`
- `evidence/reports/vcs-06-compat-evidence.md`
- `evidence/reports/vcs-06-fixture-matrix-ledger.md`
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  vcs-06-real-git-fs-fault*.md`

## Done When

- [x] 9 test 全てが Windows で pass（real-git 4 は全 OS で実行、
  FS fault 5 は `#[cfg(windows)]`）。
- [x] COMPAT evidence が最新 covering run の実測で更新される。
- [x] ledger に新規行が Pass/NotRun で記録される。
- [x] `src/`・範囲外ファイルに差分がない。

## Verification

- [x] `mise run fmt-check` — exit 0
- [x] `mise run check synvelis-library` / `synvelis-project` —
  exit 0
- [x] `mise run lint synvelis-library` / `synvelis-project` —
  exit 0
- [x] `mise run test synvelis-library` / `synvelis-project` —
  exit 0（vcs06_git_matrix 7・vcs06_restore_matrix 11・
  vcs06_apply_matrix 10 を含む全 suite green）
- [x] 補助証拠: `cargo test -p synvelis-library --test
  vcs06_git_matrix` 7 passed、`cargo test -p
  synvelis-project --test vcs06_restore_matrix --test
  vcs06_apply_matrix` 21 passed（canonical = mise）。
