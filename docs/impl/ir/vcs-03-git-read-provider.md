---
id: "IMPL-Qt0RuNZjqvP8ojWR34w9p1"
type: "impl"
status: "completed"
title: "VCS-03 Git Read Provider — fixed Commit/Tree・Index capture・History/meta"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/94"
    - "https://github.com/scarecrow173/synvelis/issues/92"
    - "https://github.com/scarecrow173/synvelis/issues/93"
    - "https://github.com/scarecrow173/synvelis/issues/88"
    - "https://github.com/scarecrow173/synvelis/issues/7"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/vcs/types.rs"
      - type: "file"
        path: "crates/synvelis-project/src/vcs/mod.rs"
      - type: "file"
        path: "crates/synvelis-library/src/git/vcs.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/common/vcs_git_fixture.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/vcs03_git_provider.rs"
      - type: "file"
        path: "docs/specs/vcs-03-git-read-provider.md"
      - type: "file"
        path: "docs/designs/vcs-03-git-read-provider-design.md"
      - type: "file"
        path: "docs/plans/vcs-03-git-read-provider.md"
      - type: "file"
        path: "docs/test-specs/vcs-03-git-provider-fixtures.md"
      - type: "file"
        path: "docs/tasks/vcs-03-task1-git-read-provider-headless-slice.md"
      - type: "file"
        path: "docs/impl/ir/vcs-03-git-read-provider.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-library/src/context.rs"
      - type: "file"
        path: "crates/synvelis-library/src/git/mod.rs"
      - type: "file"
        path: "crates/synvelis-library/src/git/objects.rs"
      - type: "file"
        path: "crates/synvelis-library/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/common/mod.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/vcs-03-git-read-provider.md"
    - "docs/designs/vcs-03-git-read-provider-design.md"
    - "docs/specs/vcs-03-git-read-provider.md"
    - "docs/tasks/vcs-03-task1-git-read-provider-headless-slice.md"
  implemented-by: []
  depends-on:
    - "docs/designs/vcs-03-git-read-provider-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/vcs-01-immutable-snapshot-resolver.md"
    - "docs/impl/ir/vcs-02-semantic-diff-engine.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/specs/alib-04-git-remote-provider.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/vcs-03-git-read-provider.md"
  derived-by: []
  verifies:
    - "docs/test-specs/vcs-03-git-provider-fixtures.md"
  verified-by: []
  references: []
  defers:
    - "VCS-04 History/Visual Diff UI（Issue #95）"
    - "VCS-05 Safe Apply/Restore（Issue #96）"
    - "実 OS permission matrix・real remote fetch・auth/credential 実経路・GC/objects 欠損の実 filesystem 条件・GUI・RT 性能保証（Issue #97）"
    - "remote LFS fetch（local LFS object store のみ解決）"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# VCS-03 Git Read Provider — Implementation Record

## What was built

Issue #94 の headless fixture slice として、VCS-01 snapshot resolver と
VCS-02 semantic diff の比較元を Git から供給する read-only provider を
実装した。

- `synvelis-project::vcs`（新規 module）: provider-neutral 契約。
  `FixedVcsRevision`（repository_id/object_format/commit_oid/project_root/
  context）・`RevisionSelector`（Head/Expr、一回だけ immutable commit に
  固定）・merge-aware `HistoryNode`/`HistoryPage`/`HistoryQuery`（bounded
  page・fingerprint-bound cursor）・`WorkingStatus`/`StagedSnapshot`/
  `IndexEntry`（stage 0–3・mode・gitlink）・`VcsCapabilities` optional
  feature 宣言・`VcsError`（missing/degraded/conflict を明示、silent
  Equal 禁止）・`VcsCancel`・`VcsReadProvider` trait。
- `synvelis-library::git::vcs`（新規）: `GitVcsProvider` +
  `GitVcsConfig`。local worktree binding（`rev-parse --show-toplevel/
  --git-dir/--git-common-dir/--show-object-format` probe）、
  `repository_id = git:sha256(git_common)`、`context` = canonical
  common dir で foreign binding を `ForeignBinding` 拒否。
  `VcsReadProvider` 全 method + `VcsBytesPort`（staged/committed
  locator の project file・asset blob read、index generation
  fingerprint）を実装。全 git 呼出は fixed-argv plumbing
  （`rev-parse`/`ls-tree`/`cat-file`/`ls-files`/`check-attr`/`rev-list`/
  `status`）で `GIT_OPTIONAL_LOCKS=0`・prompt/credential helper 無効。
  LFS pointer は local `<git_common>/lfs/objects` のみ解決し pointer
  text を content として返さない。`sha256:` digest は bytes の
  SHA-256 一致時のみ `Present`。
- adapter 小変更: `context.rs` に `CancellationToken::from_shared`
  （`VcsCancel` bridge）、`objects.rs` の `classify_kind` に
  `"not a git repository"` → `NotFound`、両 crate の export。
- tests: `tests/common/vcs_git_fixture.rs`（実 git CLI で temp
  worktree 構築、SHA-1/SHA-256 init・LFS pointer+手置き object・
  conflict/gitlink/partial stage を生成）と
  `tests/vcs03_git_provider.rs` 19 test。

所有境界の遵守: #88 Asset Provider（catalog/preview/import）は再実装
せず、低レベル `GitRunner`/`GitRemoteUrl`/`parse_ls_tree_zl`/
`parse_lfs_pointer`/`is_pointer_sized` のみ再利用。Project History/
Working Status/Asset Catalog は別 Port（`synvelis-project::vcs` vs
既存 browse/import port）。Git object ID（SHA-1/SHA-256 hex）と
project `ContentDigest`（`sha256:` prefix）は別名前空間として型・
test 双方で固定。

## Verification

- `tests/vcs03_git_provider.rs`: 19/19 pass（実 git 2.54 fixture）。
  受入 4 行 — fixed commit が branch/tag force-move 後も同一
  （`branch_and_tag_moves_do_not_change_fixed_reads`・
  `committed_snapshot_is_stable_across_branch_moves`）、merge 全 parent
  保持、rename/delete、partial stage `PartialStage`、untracked/missing
  LFS `Missing`+`Degraded`（silent Equal なし）、index generation
  追跡、repo/project identity 分離、SHA-1/SHA-256 区別、cancel 全
  operation、repo oracle（index hash/status/HEAD/staged 前後一致）と
  session oracle（live revision/tracks 不変）。
- mise lanes: `fmt-check`/`check`/`lint`/`test`/`test-rt`/`build`/
  `test-msrv` を実行（結果は PR 本文に記録）。
- docs index を `doc_maintenance.js` で再生成し `doc_status.js audit`
  で検査。

## NotRun / Deferred

- 実 OS permission matrix・real remote fetch・auth/credential 実経路・
  GC/objects 欠損の実 filesystem 条件・GUI/Visual Diff・RT 性能保証は
  headless fixture で検証不能 → Issue #97 へ `NotRun` として送る。
- remote LFS fetch・submodule 内容取得・rename similarity 検出
  （tree-level delete+add のみ）・first-parent-only history opt-in は
  本 slice 対象外。
