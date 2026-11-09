---
id: "PLAN-034UtpZmsA5LhaaXLS0KRF"
type: "plan"
status: "approved"
title: "RCD-03 Monitor Policy / Punch Gate Probe / Player Monitor-Latency Adapter slice"
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
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
  implemented-by:
    - "docs/tasks/rcd-03-task1-monitor-punch-latency.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
    - "docs/designs/rcd-03-monitoring-punch-latency.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-03-monitor-punch-latency.md"
  verified-by:
    - "docs/test-specs/rcd-03-monitor-punch-latency.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 Monitor Policy / Punch Gate Probe / Player Monitor-Latency Adapter slice Implementation Plan

## Goal

`crates/synvelis-recording` に Monitor Off/Auto/On の可聴 route
判定・Hardware Direct notice・Punch gate 連動 `MonitorProbe` を、
`crates/synvelis-player` に非 RT monitor gain ramp / native
placement / latency resolution / low-latency route adapter を実装し、
Fixed PCM/Frame Fixture で検証する。

## Scope

単一実装 stream。変更は `crates/synvelis-recording/`・
`crates/synvelis-player/`（src + tests）と `docs/` のみ。
`synvelis-app`/`synvelis-workspace`/`synvelis-project` および
他 crate・Ruviform には触れない。

## Upstream Documents

- `docs/specs/rcd-03-monitoring-punch-latency.md`

## Design Inputs

- `docs/designs/rcd-03-monitoring-punch-latency.md`

## File Map

- `crates/synvelis-recording/src/input.rs` - `MonitorState` に
  `Auto` 追加・doc 更新
- `crates/synvelis-recording/src/monitor.rs` - `TransportPhase`/
  `MonitorRoute`/`HardwareDirectState`/`HardwareDirectMonitor`/
  `TrackMonitorContext`/`MonitorNotices`/`MonitorDecision`/
  `decide`/`MonitorProbe`/`MonitorSegment`（新規）
- `crates/synvelis-recording/src/lib.rs` - `monitor` module 公開 +
  export 更新
- `crates/synvelis-recording/tests/monitor_gate.rs` - decide 行列・
  probe・Raw Byte 独立比較・Punch/Loop/Tempo 検証（新規）
- `crates/synvelis-player/src/monitor.rs` - `MonitorAdapter`/
  `MonitorBlock`/`MonitorOutput`/gain ramp・`LatencyQuality`/
  `LatencyBasis`/`LatencyComponent`/`LatencyReport`/
  `LatencyResolution`/`PreparedMonitorRoute`/`LowLatencyMonitor`/
  `prepare_low_latency`（新規）
- `crates/synvelis-player/src/lib.rs` - `monitor` module 公開 +
  export 更新
- `crates/synvelis-player/tests/monitor.rs` - PCM/latency fixture
  検証（新規）

## Implementation Sequence

- [ ] `MonitorState::Auto` 追加（input.rs）
- [ ] `monitor.rs`: 型 + `decide` + `MonitorProbe` 実装
- [ ] `lib.rs` export 更新
- [ ] `tests/monitor_gate.rs` fixture test 実装
- [ ] player `monitor.rs`: adapter + latency + low-latency 実装
- [ ] player `lib.rs` export 更新
- [ ] `tests/monitor.rs` fixture test 実装
- [ ] 既存テスト回帰確認（`input_adapter.rs` 等）

## Dependencies and Risks

- `MonitorProbe` は `plan.scheduler`（`GateScheduler`）を共有
  参照し、`GateSchedule`/`evaluate_gate` を再利用 — gate ロジック
  の第二実装を作らない。
- `MonitorState::Auto` 追加は enum variant のみで既存
  `Off`/`On` 利用・test に影響しない（recording crate 内のみ
  使用を確認済み）。
- player → recording の既存依存で `MonitorRoute` を共有。
  recording → player の逆依存は作らない。
- `MonitorAdapter::process_block` は確保済み scratch のみ使用
  （RT でも alloc しない設計）。出力 `Vec<Vec<f32>>` の複写は
  非 RT 呼出し側への受け渡し用。

## Task Breakdown

- Task 1: `docs/tasks/rcd-03-task1-monitor-punch-latency.md`
  （両 crate の変更を単一 task で実装 — 同一 slice の一貫した
  変更）

## Verification

- [ ] `mise run fmt-check`
- [ ] `mise run check`
- [ ] `mise run lint`
- [ ] `mise run test`
- [ ] `mise run test-rt`
- [ ] `mise run build`
- [ ] 実 OS/Device/GUI 実測 — NotRun（実機 gate・後続 slice）

## Open Questions / Gaps

- なし（spec/design で確定済み）

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（Orca worker dispatch 内）
- Independent work streams: none
- User approval required before dispatch: no（review-protocol で代替）
- If approved: execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- 前提: RCD-01 input adapter・VS-B gate/engine・RCD-02 durability
  は main に実装済み。`MonitorState` への `Auto` 追加が既存
  test の exhaustive match を壊さないことを確認する。
- `decide` の route 規則は spec の製品ポリシー節をそのまま
  写像したもの — 規則表と Issue 本文の対応を review で確認。
