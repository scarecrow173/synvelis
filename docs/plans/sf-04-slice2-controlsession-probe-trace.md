---
id: "PLAN-034Uupw2uQUIw30kQZ9HYX"
type: "plan"
status: "approved"
title: "SF-04 slice2 実装計画 — ControlSession probe observation → ControlTrace 配線"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/designs/sf-04-slice2-controlsession-probe-trace-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice2 実装計画 — ControlSession probe observation → ControlTrace 配線 Implementation Plan

## Goal

probe observation が session（`ControlSession`）→ port → player
bridge → node 別 `DispatchTraceAdapter` → `ControlTrace.observed`
slot に流れる経路を実装し、headless fake-port で E2E を検証する。

## Scope

単一実装ストリーム。synvelis-project に port 契約、synvelis-player
に bridge 参照実装、synvelis-workspace に session 配線の 3 層で、
1 つの slice として不可分（port 契約無しでは consumer が書けず、
consumer 無しでは producer を検証できない）。

## Upstream Documents

- `docs/specs/sf-04-slice2-controlsession-probe-trace.md`

## Design Inputs

- `docs/designs/sf-04-slice2-controlsession-probe-trace-design.md`
- `docs/designs/sf-04-slice1-semantic-probe-adapter-design.md`
- `docs/designs/pc-c3-05-rt-trace-adapter-design.md`

## File Map

- `crates/synvelis-project/src/probe_trace.rs` - `ProbeTracePort`
  trait・`ProbeObservationUpdate`・`ProbePortError`（新規）
- `crates/synvelis-project/src/lib.rs` - `probe_trace` module export
- `crates/synvelis-player/src/probe_trace.rs` - `ProbeTraceBridge`
  port 参照実装・ProbeEvent→ProbeObservation translation（新規）
- `crates/synvelis-player/src/lib.rs` - export 追加
- `crates/synvelis-workspace/src/probe_trace.rs` - `ProbeTraceSession`
  state・`ControlSession` probe-trace API（新規）
- `crates/synvelis-workspace/src/control.rs` - `probe_trace` field
  追加・`ProbeDrainReport` などの re-export で配線（最小）
- `crates/synvelis-workspace/src/lib.rs` - export 追加
- `crates/synvelis-workspace/tests/sf04_s2_probe_trace.rs` - fake
  port E2E（新規）
- `crates/synvelis-player/tests/sf04_s2_probe_trace_bridge.rs` -
  実 engine+adapter bridge テスト（新規）

## Implementation Sequence

- [x] spec → design → test-spec → plan → task 文書チェーン作成
- [ ] synvelis-project `probe_trace.rs`: port 契約 + 型
- [ ] synvelis-player `probe_trace.rs`: `ProbeTraceBridge` + translation
- [ ] synvelis-workspace `probe_trace.rs`: `ProbeTraceSession` + API
- [ ] workspace E2E テスト（fake port）
- [ ] player bridge テスト
- [ ] mise 検証全通過
- [ ] origin/main merge・生成物再生成・commit/push/PR

## Dependencies and Risks

- `ProbeClockMark`/`RtProbeMode`/`ProbeStatusKind` 等の ruviform 型は
  player 内に閉じ、port 境界を越えない — project crate は
  ruviform-audio に依存しない（依存方向確認済み）。
- `DispatchTraceAdapter` は per-`ParamDispatch`。bridge の
  `routes` は composition root が bind 時に登録する明示 mapping —
  推測しない。
- `probe_drain` の generation 照合は `ProbeId → binding` の事前
  snapshot で行う（borrow 衝突回避）。

## Task Breakdown

- T1（本 plan）: 3 層配線 + 2 テストファイル。単一 task で実施
  （依存が直列で分割不能）。

## Verification

- [ ] `mise run fmt-check`
- [ ] `mise run check`
- [ ] `mise run lint`
- [ ] `mise run test`
- [ ] `mise run test-rt`（存在する範囲）
- [ ] `mise run check-ruviform-rev`
- [ ] `mise run check-headless-deps`

## Open Questions / Gaps

- なし（design で決定済み）。app composition root の port 実装は
  別 dispatch の deferred concern。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（worker dispatch）
- Independent work streams: none
- User approval required before dispatch: no（review-protocol で
  blocker 無しを確認済みとして扱う）
- If approved: discover available implementation and delegation
  capabilities in the current environment, then execute task-by-task
  with review checkpoints.
- If declined or unavailable: execute inline using the same dependency
  graph and verification matrix.

## Review Handoff

- reviewer focus: port 契約の境界（ruviform 型の非越境）、
  fail-closed 写像（generation・clock・status kind）、drain 単一
  owner の表現、rebind/segment cut の非回帰。
