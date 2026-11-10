---
id: "TASK-v2hQAN1hrbnURHHF2lrf6L"
type: "task"
status: "done"
title: "VCS-03 Task1 Git Read Provider headless slice"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source:
    - "PLAN-D4Oqo3jPnK2GybwKhM5nwh"
  verified_by:
    - "TSPEC-K2QAlGFhpEHJiLP6StV4ta"
  implemented_by: []
  supersedes: []
  superseded_by: []
---

# VCS-03 Task1 Git Read Provider headless slice

- [x] `crates/synvelis-project/src/vcs/types.rs` — provider-neutral 型
- [x] `crates/synvelis-project/src/vcs/mod.rs` — `VcsReadProvider`/
      `VcsError`/`VcsCancel`
- [x] `crates/synvelis-project/src/lib.rs` — `pub mod vcs;`
- [x] `crates/synvelis-library/src/git/vcs.rs` — `GitVcsProvider`/
      `GitVcsConfig`（`VcsReadProvider` + `VcsBytesPort`）
- [x] `crates/synvelis-library/src/git/objects.rs` —
      `"not a git repository"` → `NotFound` 分類
- [x] `crates/synvelis-library/src/context.rs` —
      `CancellationToken::from_shared`
- [x] `crates/synvelis-library/src/git/mod.rs`・`src/lib.rs` — export
- [x] `crates/synvelis-library/tests/common/vcs_git_fixture.rs` —
      local worktree fixture（SHA-1/SHA-256・LFS・conflict・gitlink）
- [x] `crates/synvelis-library/tests/vcs03_git_provider.rs` —
      受入 4 行 fixture test 19 件 + read-only/session oracle
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`build`/
      `test-msrv`
- [x] impl record + docs index 再生成 + PR（`Refs #94`・close しない）+
      worker_done
