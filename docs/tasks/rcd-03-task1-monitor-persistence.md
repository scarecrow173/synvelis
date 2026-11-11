---
id: "TASK-034VpQe0nP5xQ2wU7vB9xM"
type: "task"
status: "done"
title: "RCD-03 Task1 Arm/Monitor/Punch Project 永続投影"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-03-monitor-persistence-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-03-monitor-persistence-plan.md"
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
    - "docs/test-specs/rcd-03-monitor-persistence.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 Task1 Arm/Monitor/Punch Project 永続投影

## Work

`synvelis-project` に `monitoring_data` POD モジュール・
`Project::monitoring` フィールド・schema v12 + v11→v12 migration・
`SetTrackMonitor`/`ClearTrackMonitor`/`SetPunchMarks` op を実装し、
`synvelis-app` の `syn.monitoring.set`/`punch` を write-through 化、
`MonitorUxState` を session 観測値のみに再構成して
`project.monitoring` read-through 投影に切り替える。
`tests/monitoring_persist.rs`（project）と `tests/monitor_arm.rs`
（app）で test-spec E1–E6 を検証する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-03-monitor-persistence-plan.md`
- `docs/designs/rcd-03-monitor-persistence-design.md`
- `docs/specs/rcd-03-monitor-persistence.md`
- `docs/test-specs/rcd-03-monitor-persistence.md`

## Dependencies

- 先行 slice（`rcd-03-app-arm-monitoring-ux`）: `syn.monitoring.*`
  ops・`MonitorUxState`・`decide`/`MonitorProbe`/`preflight` 消費の
  既存実装。
- `synvelis-project` の `ProjectController`/`ProjectOp`/
  `SnapshotEnvelope`/migration STEPS の既存契約。

## Affected Files

- `crates/synvelis-project/src/monitoring_data.rs`（新規）
- `crates/synvelis-project/src/lib.rs`
- `crates/synvelis-project/src/project.rs`
- `crates/synvelis-project/src/store.rs`
- `crates/synvelis-project/src/migration.rs`
- `crates/synvelis-project/src/ops.rs`
- `crates/synvelis-project/tests/monitoring_persist.rs`（新規）
- `crates/synvelis-app/src/monitoring.rs`
- `crates/synvelis-app/src/ui_ops.rs`
- `crates/synvelis-app/src/app.rs`
- `crates/synvelis-app/tests/monitor_arm.rs`
- `docs/`（本 chain + index 再生成）

## Done When

- [x] 宣言・punch が `Project.monitoring` に保存され、save/load・
  v11→v12 migration・undo/redo で復元・整合する。
- [x] `syn.monitoring.set`/`punch` の宣言変更が write-through で
  Project に反映され、`syn.project.undo` で戻る。
- [x] session 観測値が永続化されず、project_new/open で reset
  される。
- [x] `tests/monitoring_persist.rs` と `tests/monitor_arm.rs` が
  test-spec E1–E6 を検証して pass。
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build` が pass。
- [x] review-protocol で blocker なし。

## Verification

- [x] `mise run test`（canonical）+ fixture 実行。
- [x] `git fetch origin main && git merge origin/main` 後の再検証。
