---
id: "TASK-034WCx98h2mIsDpZJrrBkC"
type: "task"
status: "completed"
title: "RCD-03 monitor engine — implementation slice"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
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
    - "docs/plans/rcd-03-monitor-engine.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-03-monitor-engine.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/rcd-03-monitor-engine.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-03-monitor-engine.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 monitor engine — implementation slice

## Work

`synvelis-player` に `MonitorSession`/`MonitorEngine` を実装し、
headless engine 実経路（`HeadlessDriver`/`RoutingEngine` の
`run`/`cursor`）に monitor bus を載せる。`MonitorProbe` +
`TrackMonitorContext` + `MonitorAdapter` + 入力 PCM FIFO +
`TransportPhase` + segment evidence を合成し、block 単位で
decide → adapter → monitor PCM を生成する。`plan.rs` に
`sample_rate` を追加して engine rate 一致を構築時検証する。
Loop/PunchInLoop gate の probe segment・Raw Byte 独立・
tempo 境界を `monitor_gate.rs` に、engine 実経路の route
結果・block 途中境界・Loop/PunchInLoop pass・fail-closed を
`monitor_engine.rs`（player tests）に追加する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/rcd-03-monitor-engine.md`
- `docs/designs/rcd-03-monitor-engine.md`
- `docs/plans/rcd-03-monitor-engine.md`
- `docs/test-specs/rcd-03-monitor-engine.md`
- `docs/specs/rcd-03-monitoring-punch-latency.md`（前提契約）

## Dependencies

- PR #214（`decide`/`MonitorProbe`/`MonitorAdapter`/
  `LatencyReport` の crate 内契約）— merge 済み。
- `GateSpec::Loop`/`PunchInLoop`・`GateScheduler`/
  `evaluate_gate` — merge 済み。

## Affected Files

- `crates/synvelis-recording/src/plan.rs`
- `crates/synvelis-recording/tests/monitor_gate.rs`
- `crates/synvelis-player/src/monitor_engine.rs`（新規）
- `crates/synvelis-player/src/driver.rs`
- `crates/synvelis-player/src/routing_engine.rs`
- `crates/synvelis-player/src/lib.rs`
- `crates/synvelis-player/tests/monitor_engine.rs`（新規）

## Done When

- [ ] `MonitorEngine` が engine 実経路に monitor bus を載せ、
      headless fixture で Off/Auto/On × Arm × Transport ×
      Punch × Mute/Solo の route 結果が pin される。
- [ ] `GateSpec::Loop`/`PunchInLoop` の block 途中境界で
      Gap/重複 0・Raw Placement 二重補正 0 が fixture で
      検証される。
- [ ] 高 Latency FX・Tempo 区間の組合せが fixture で扱える
      範囲で pin される。
- [ ] `PreparedRecordingPlan.sample_rate` が engine rate と
      の不一致を構築時エラーにする。
- [ ] canonical 9 lane がすべて exit 0。
- [ ] review-protocol で blocker なし。

## Verification

- [ ] `mise run test` — `monitor_gate`/`monitor_engine`
      テストを含む全テスト pass。
- [ ] `mise run test-rt` — RT 関連テスト pass。
- [ ] `mise run fmt-check`/`check`/`lint`/`build`/
      `check-app-native`/`lint-app-native`/`test-msrv` —
      すべて exit 0。
- [ ] `review-protocol` レビュー実施・blocker 解消。
