---
id: "TASK-106SF03Uz2aBc3dEf4gHi5jKl6"
type: "task"
title: "SF-03 slice2 T2 — 受入テスト sf03_l2.rs・検証"
status: "in-progress"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-03-slice2-fault-reconcile.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/sf-03-slice2-fault-contract.md"
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
    - "docs/test-specs/sf-03-slice2-fault-reconcile.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice2 T2 — 受入テスト・検証

- `sf03_l1.rs`: `RoutingFault` 改名追従（`StaleProjectRevision`→
  `StaleProjectSnapshot` 等）。
- `sf03_l2.rs` 新規: test-spec の全 Rule をカバー
  （fault mint・last-good 行列・retry・reconcile drift・offline
  gate・投機的 plan 拒否）。
- `docs/test-specs/sf-03-slice2-fault-reconcile.md` 仕上げ。
- 検証: `git fetch origin main && git merge origin/main` 後に
  `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-headless-deps` を実行し exit code を記録。
