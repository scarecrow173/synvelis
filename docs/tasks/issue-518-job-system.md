---
id: "TASK-sv518eF6gH8iJ0kL4mN6"
type: "task"
status: "done"
title: "Issue #518 tasks — production job system 配線"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/plans/issue-518-job-system.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-518-job-system.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #518 tasks — production job system 配線

## タスク一覧

- [x] T1: render serde — `model.rs`/`trigger.rs` wire 契約型 serde
- [x] T2: orchestrator — `drive_step` 追加・`drive()`/`run()` 再構成・cost model 統合・`inspect_share` pub(crate)
- [x] T3: `cost.rs` — `CostCalibration`・`model_estimate`・fail-closed 既定
- [x] T4: `scheduler.rs` — `SchedulerPorts`/`JobScheduler`/`JobView`/bounded queue/OfflineJob worker
- [x] T5: project — `render_policy` field + migration v15 + store + #[cfg(test)]
- [x] T6: workspace — `job_system.rs` + `render_dispatch.rs` + lib.rs mod 宣言
- [x] T7: host — cli/mcp-poc path dep + `GateDeps.render` adapter 注入
- [x] T8: `tests/rcp03_render_policy.rs` 内 async pin suite 9 件（pol01 coalesce・pol06 失敗行列・bounded queue・headless fail-closed）
- [x] T9: docs impl chain + `evidence/sv518`
- [x] T10: verify（mise fmt-check/check/lint/test/headless-deps）+ review-protocol + PR
