---
id: "TASK-QDa6_9HNOZ3w0cse6y4ih"
type: "task"
status: "done"
title: "GitVcsProvider rev-parse --quiet 除去による unborn HEAD 誤分類修正 + 回帰 test"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/335"
  changes:
    added:
      - type: "file"
        path: "docs/tasks/vcs-03-unborn-head-classification-task.md"
      - type: "file"
        path: "docs/impl/ir/vcs-03-unborn-head-classification.md"
    modified:
      - type: "file"
        path: "crates/synvelis-library/src/git/vcs.rs"
      - type: "file"
        path: "crates/synvelis-library/tests/vcs03_git_provider.rs"
      - type: "file"
        path: "docs/designs/vcs-03-git-read-provider-design.md"
      - type: "file"
        path: "docs/test-specs/vcs-03-git-provider-fixtures.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/vcs-03-unborn-head-classification-plan.md"
  implemented-by:
    - "docs/impl/ir/vcs-03-unborn-head-classification.md"
  depends-on:
    - "docs/plans/vcs-03-unborn-head-classification-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-03-unborn-head-classification.md"
    - "docs/designs/vcs-03-unborn-head-classification-design.md"
    - "docs/test-specs/vcs-03-unborn-head-classification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-03-unborn-head-classification.md"
    - "docs/designs/vcs-03-unborn-head-classification-design.md"
    - "docs/plans/vcs-03-unborn-head-classification-plan.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-library/src/git/vcs.rs"
    - "crates/synvelis-library/tests/vcs03_git_provider.rs"
    - "crates/synvelis-library/tests/common/vcs_git_fixture.rs"
  defers: []
  deferred-by: []
---

# GitVcsProvider rev-parse --quiet 除去による unborn HEAD 誤分類修正 + 回帰 test

## Work

`vcs.rs` の `resolve_oid`・`head_oid`・`probe_workspace` が使う
`git rev-parse --verify` から `--quiet` を除去し、unborn HEAD・
解決不能 revision の失敗を fatal stderr → `classify_kind` →
`NotFound` 経由で `RevisionGone`/`Ok(None)` へ写像する。
`tests/vcs03_git_provider.rs` に 0-commit repo（`VcsGitFixture::
new()` が unborn HEAD をそのまま提供）の回帰 test を追加し、
VCS-03 design・fixtures test-spec を現状整合する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/vcs-03-unborn-head-classification-plan.md`
- `docs/specs/vcs-03-unborn-head-classification.md`
- `docs/designs/vcs-03-unborn-head-classification-design.md`

## Dependencies

- なし（ブロッカーなし）

## Affected Files

- `crates/synvelis-library/src/git/vcs.rs` — rev-parse argv から
  `"--quiet"` 除去（3 箇所）+ doc comment 更新
- `crates/synvelis-library/tests/vcs03_git_provider.rs` —
  `unborn_head_reports_typed_absence_not_unavailable` 追加
- `docs/designs/vcs-03-git-read-provider-design.md` — resolve 節の
  rev-parse 記述を `--verify`（stderr 駆動分類）へ整合
- `docs/test-specs/vcs-03-git-provider-fixtures.md` — test 数・
  unborn HEAD feature 行を追加
- `docs/specs|designs|plans|tasks|test-specs|impl/ir/vcs-03-
  unborn-head-classification*` — 本タスクの文書チェーン

## Done When

- [x] `rev-parse --verify`（`--quiet` なし）で unborn HEAD が
  `fatal: Needed a single revision` → `NotFound` に分類され、
  `head_oid` → `Ok(None)`・`resolve_oid` → `RevisionGone` になる
- [x] 0-commit repo で `working_status` が `Ok` +
  `head_commit: None`、`resolve_fixed_revision(Head/Expr)`・
  `list_history` が `RevisionGone`、`resolve_staged_snapshot` が
  正常動作することを回帰 test が固定
- [x] `classify_kind`/`GitRunner`/`mirror.rs`/他 crate を変更
  していない（ALIB-04 blast radius ゼロ）
- [x] pre-fix fail（`Unavailable { detail: "TransientIo: git
  command failed" }`）/ post-fix pass を実走で証拠化

## Verification

- [x] `mise run fmt-check` — exit 0
- [x] `mise run check` — exit 0
- [x] `mise run lint` — exit 0
- [x] `mise run test` — exit 0（`git.rs` ALIB-04 系を含む全
  workspace pass、`vcs03_git_provider` 20/20）
- [x] `mise run test-rt` — exit 0
- [x] `mise run check-app-native` — exit 0
- [x] `mise run lint-app-native` — exit 0
- [x] `mise run test-msrv` — exit 0
- [x] `mise run build` — exit 0
- [x] `cargo test -p synvelis-library --test vcs03_git_provider`
  20/20 pass — 補助証拠（canonical = mise）
- [ ] PR CI の verify lane 実測を観測（マージ判断は Coordinator）
