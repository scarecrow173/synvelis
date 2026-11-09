---
id: "TASK-034UtpBjTeXJ6XSJAMupsv"
type: "task"
status: "in-progress"
title: "RCD-03 Task1 Monitor/Punch/Latency semantics"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/plans/rcd-03-monitoring-punch-latency.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-03-monitoring-punch-latency.md"
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
    - "docs/test-specs/rcd-03-monitor-punch-latency.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 Task1 Monitor/Punch/Latency semantics

## Work

両 crate に RCD-03 slice を実装する。

1. `synvelis-recording/src/input.rs`: `MonitorState` に `Auto`
   variant を追加し doc を更新。
2. `synvelis-recording/src/monitor.rs`（新規）: `TransportPhase`/
   `MonitorRoute`/`HardwareDirectState`/`HardwareDirectMonitor`/
   `TrackMonitorContext`/`MonitorNotices`/`MonitorDecision`/
   `MonitorSegment`/`decide`/`MonitorProbe` を実装。`MonitorProbe`
   は `GateScheduler`/`GateSchedule`/`evaluate_gate` を再利用し
   block 途中境界を session-frame 精度で分割する。
3. `synvelis-recording/src/lib.rs`: `monitor` module 公開 +
   export 更新。
4. `synvelis-recording/tests/monitor_gate.rs`（新規）: decide
   行列・probe segment・Raw Byte 独立比較・Punch 途中境界
   Gap/重複 0・Loop/Tempo 二重補正 0・Hardware Direct notice・
   Mono/Stereo binding。
5. `synvelis-player/src/monitor.rs`（新規）: `MonitorAdapter`/
   `MonitorBlock`/`MonitorOutput`/gain ramp・`LatencyQuality`/
   `LatencyBasis`/`LatencyComponent`/`LatencyReport`/
   `LatencyResolution`/`PreparedMonitorRoute`/`LowLatencyState`/
   `LowLatencyReason`/`LowLatencyMonitor`/`prepare_low_latency`。
6. `synvelis-player/src/lib.rs`: `monitor` module 公開 + export。
7. `synvelis-player/tests/monitor.rs`（新規）: gain ramp・native
   placement・route 別 PCM・input missing fail-closed・latency
   resolve 行列・low latency 状態。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-03-monitoring-punch-latency.md`
- `docs/designs/rcd-03-monitoring-punch-latency.md`
- `docs/specs/rcd-03-monitoring-punch-latency.md`

## Dependencies

- RCD-01 input adapter（main 実装済み）
- VS-B gate/engine/mock fixture（main 実装済み）
- RCD-02 durability/recovery（main 実装済み・本 slice と独立）

## Affected Files

- `crates/synvelis-recording/src/{input,monitor,lib}.rs`
- `crates/synvelis-recording/tests/monitor_gate.rs`（新規）
- `crates/synvelis-player/src/{monitor,lib}.rs`
- `crates/synvelis-player/tests/monitor.rs`（新規）

## Done When

- [ ] `MonitorState::Auto` が追加され、Off/Auto/On × Arm ×
      Transport × Punch × Mute/Solo の route 判定が spec 規則表
      どおりに動く
- [ ] `MonitorProbe` が `evaluate_gate` を再利用して block 途中
      境界を session-frame 精度で分割し、Gap/重複 0
- [ ] Hardware Direct `DeviceControlled`/`UserReported`/
      `Unknown` と `DoubleMonitoring`/`HardwareDirectUnknown`/
      `InputUnavailable` notice が型として区別される
- [ ] Monitor 状態に依らず Raw File Byte が一致し、Arm Off +
      Monitor On で capture が発生しない
- [ ] Loop/Tempo 区間で Raw Placement に二重補正がない
- [ ] player adapter が gain ramp・native placement・route 別
      PCM・input missing fail-closed を fixture で検証する
- [ ] latency が Measured/Reported/Estimated/Unknown・Clock
      Epoch を区別し、round-trip + 個別 reported の二重加算を
      行わない
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`build`
      が全て Pass

## Verification

- [ ] `mise run fmt-check`
- [ ] `mise run check`
- [ ] `mise run lint`
- [ ] `mise run test`
- [ ] `mise run test-rt`
- [ ] `mise run build`
- [ ] 実 OS/Device/GUI — NotRun（実機 gate・後続 slice）
