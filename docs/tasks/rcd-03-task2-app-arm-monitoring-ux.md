---
id: "TASK-034VegaqbLjPWyJoFDrmhQ"
type: "task"
status: "in-progress"
title: "RCD-03 Task2 app Arm/Monitoring UX + monitor session projection"
created: "2026-11-10"
updated: "2026-11-10"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-03-app-arm-monitoring-ux.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-03-app-arm-monitoring-ux.md"
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
    - "docs/test-specs/rcd-03-app-arm-monitoring-ux.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 Task2 app Arm/Monitoring UX + monitor session projection

## Work

`crates/synvelis-app` に `syn.monitoring.set` / `punch` / `transport` /
`session` / `latency` / `state` の 6 op を実装し、session-scope の
`MonitorUxState`（track 宣言・monitor session・punch マーク・
latency 注入）を `SynvelisApp` に追加する。`syn.monitoring.state` は
`decide`/`MonitorProbe`/`LatencyReport::resolve`/`prepare_low_latency`
を消費して route・notices・segments・latency を投影する。
`tests/monitor_arm.rs` で test-spec E1–E7 を検証する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-03-app-arm-monitoring-ux.md`
- `docs/designs/rcd-03-app-arm-monitoring-ux.md`
- `docs/specs/rcd-03-app-arm-monitoring-ux.md`
- `docs/test-specs/rcd-03-app-arm-monitoring-ux.md`

## Dependencies

- crate slice（PR #214）: `synvelis-recording::monitor`/`plan`/`input`、
  `synvelis-player::monitor` の公開 API。
- `docs/tasks/rcd-03-task1-monitor-punch-latency.md`（done）。

## Affected Files

- `crates/synvelis-app/src/monitoring.rs`（新規）
- `crates/synvelis-app/src/ui_ops.rs`
- `crates/synvelis-app/src/service.rs`
- `crates/synvelis-app/src/app.rs`
- `crates/synvelis-app/src/lib.rs`
- `crates/synvelis-app/Cargo.toml`
- `crates/synvelis-app/tests/monitor_arm.rs`（新規）
- `Cargo.lock`（再生成のみ）
- `docs/`（本 chain + index 再生成）

## Done When

- [ ] 6 op が登録・dispatch され、`syn.monitoring.state` が
  decide/MonitorProbe/latency 契約どおり投影する。
- [ ] Hardware Direct Unknown の正直な notice、double_monitoring、
  input_unavailable が投影される。
- [ ] `tests/monitor_arm.rs` が E1–E7 を検証して pass。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`test-msrv` が pass。
- [ ] review-protocol で blocker なし。

## Verification

- [ ] `mise run test`（canonical）+ fixture 実行。
- [ ] `git fetch origin main && git merge origin/main` 後の再検証。
