---
id: "TASK-034UEtaHWZFrCd1VFAIRfl"
type: "task"
status: "done"
title: "VCS-02 Task1 Semantic Diff Engine headless slice"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source:
    - "PLAN-034UEufn0HEwKaQ2GRVCWB"
  verified_by:
    - "TSPEC-034UEueLhP34NeCWclgoux"
  implemented_by: []
  supersedes: []
  superseded_by: []
---

# VCS-02 Task1 Semantic Diff Engine headless slice

- [x] `src/diff/types.rs` — public 型
- [x] `src/diff/engine.rs` — compare pipeline・matching・field diff・sort・cache
- [x] `src/diff/adapters.rs` — DomainAdapter + per-domain adapter
- [x] `src/diff/mod.rs` + `lib.rs` export
- [x] `tests/vcs02_diff.rs` — VCS-DIFF-01..08 fixture
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv`/app-native lanes
- [x] impl record + merge origin/main + final verify + PR + worker_done
