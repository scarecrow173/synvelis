---
id: "TSPEC-E_sJp4NvwYKKfYNFbaPxp"
type: "test-spec"
status: "approved"
title: "synvelis#335 unborn HEAD 誤分類修正 — 0-commit repo 回帰 test 仕様"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/335"
  changes:
    added:
      - type: "file"
        path: "docs/test-specs/vcs-03-unborn-head-classification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/tests/vcs03_git_provider.rs"
      - type: "file"
        path: "docs/test-specs/vcs-03-git-provider-fixtures.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/vcs-03-unborn-head-classification.md"
    - "docs/designs/vcs-03-unborn-head-classification-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/vcs-03-git-provider-fixtures.md"
    - "docs/test-specs/alib-07-fault-matrix.md"
    - "docs/plans/vcs-03-unborn-head-classification-plan.md"
    - "docs/tasks/vcs-03-unborn-head-classification-task.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-03-unborn-head-classification.md"
    - "docs/designs/vcs-03-unborn-head-classification-design.md"
  derived-by: []
  verifies:
    - "docs/specs/vcs-03-unborn-head-classification.md"
    - "docs/designs/vcs-03-unborn-head-classification-design.md"
  verified-by:
    - "crates/synvelis-library/tests/vcs03_git_provider.rs"
    - "crates/synvelis-library/tests/common/vcs_git_fixture.rs"
  references:
    - "crates/synvelis-library/tests/vcs03_git_provider.rs"
    - "crates/synvelis-library/tests/common/vcs_git_fixture.rs"
  defers: []
  deferred-by: []
---

# synvelis#335 unborn HEAD 誤分類修正 — 0-commit repo 回帰 test 仕様

本 slice が保証する対象は VCS-03 read provider の **unborn HEAD
（commit 0 件 repo）における typed absence 表現** — 解決不能な
revision が `VcsError::Unavailable`/`TransientIo` に誤分類されず、
`RevisionGone`・`head_commit: None` の設計不変条件どおりに観測
面へ現れること。

## 実行環境（Baseline）

- Synvelis branch: `scarecrow173/sv335-unborn-head`
- Toolchain: rustc 1.99.0（MSRV 1.89）
- OS: Windows（実 `git` CLI を使う local worktree fixture。
  remote/network/git-lfs binary 不要）
- Fixture: `vcs_git_fixture::VcsGitFixture::new()` — `git init
  --initial-branch=main` + config のみで **commit 0 件 = unborn
  HEAD** をそのまま提供（fixture 変更不要・additive 確認済み）
- Status 凡例は `alib-07-fault-matrix` に同じ。

## Feature: UNBORN — commit 0 件 repo の typed absence

- Rule: unborn HEAD の `git rev-parse --verify HEAD^{commit}`
  失敗は `VcsError::Unavailable` ではなく `head_commit: None`
  （`working_status`）・`VcsError::RevisionGone`
  （`resolve_fixed_revision`/`list_history`）として観測される。
- Rule: `RevisionSelector::Expr` の解決不能（unborn branch 名・
  `HEAD` alias・未知 ref）も同じ `RevisionGone` 経路に揃う。
- Rule: unborn repo でも index/staged 系
  （`resolve_staged_snapshot`・`index_fingerprint`・
  `working_status` の status 部）は正常に動作し、staged file が
  `WorkingStatusEntry`/`StagedSnapshot.entries` に現れる。
- Rule: `detect_workspace`/`open` の probe は unborn を
  `head_commit: None` として報告し `NotRepository` にしない。
- Evidence: `tests/vcs03_git_provider.rs::
  unborn_head_reports_typed_absence_not_unavailable`

## 証拠 matrix（Case ID 別 Expected/Observed/Status）

| Case | Expected | Observed | Status |
| --- | --- | --- | --- |
| UNBORN-01 detect/probe | `detect_workspace` が `head_commit: None` で `Ok` | `unborn_head_reports_typed_absence_not_unavailable`（Pass。probe の `.ok()` 経路は修正前から None だったことを確認） | Pass |
| UNBORN-02 working_status | `Ok` + `head_commit: None` + staged file が `index_change: Added` で報告 | 同 test（Pass。修正前は `Unavailable { detail: "TransientIo: git command failed" }` で決定的に失敗 — pre-fix fail 証拠） | Pass |
| UNBORN-03 resolve Head | `RevisionSelector::Head` → `VcsError::RevisionGone` | 同 test（Pass。修正前は `Unavailable`/`TransientIo`） | Pass |
| UNBORN-04 resolve Expr | `"main"`/`"HEAD"`/`"no-such-revision"` → `RevisionGone` | 同 test の expr ループ（Pass） | Pass |
| UNBORN-05 list_history | start 省略・明示 `Head` start の両方で `RevisionGone` | 同 test の query ループ（Pass） | Pass |
| UNBORN-06 staged/index | `resolve_staged_snapshot` が `Ok` で staged entry を含む | 同 test 末尾（Pass。unborn でも `ls-files --stage` は正常） | Pass |
| UNBORN-07 非回帰 | commit 済み repo の全 19 既存 test + `tests/git.rs` ALIB-04 系が green | `cargo test -p synvelis-library --test vcs03_git_provider` 20/20 pass（補助証拠）、`mise run test` で workspace 全 pass | Pass |

## NotRun / Unsupported 記録

- 実 remote fetch・実 OS permission matrix・GUI（VCS History
  表示）・RT 性能: headless fixture の範囲外として NotRun
  （`vcs-03-git-provider-fixtures`・`vcs-06-*` の既存 NotRun 行を
  変更しない）。
- `probe_workspace` が unborn 以外の失敗（一時的 IO 障害等）を
  `head_commit: None` に畳む既存 semantics は本 slice の対象外
  （design Deferred Design Concerns 記録どおり）。
- unborn repo で最初の commit を打った直後の HEAD 遷移
  （unborn → born）は fixture の `commit_all` が既存 test で
  カバー済みのため本 test では扱わない。

## 集計

| Feature | Cases | Pass | Fail | NotRun | Unsupported |
| --- | --- | --- | --- | --- | --- |
| UNBORN | 7 | 7 | 0 | 0 | 0 |
