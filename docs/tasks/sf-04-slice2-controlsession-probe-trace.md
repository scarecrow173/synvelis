---
id: "TASK-034Uur4AKzlyg8khBcNC6X"
type: "task"
status: "in-progress"
title: "SF-04 slice2 T1 — ControlSession probe observation → ControlTrace 配線"
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
    - "docs/plans/sf-04-slice2-controlsession-probe-trace.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-04-slice2-controlsession-probe-trace.md"
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
    - "docs/test-specs/sf-04-slice2-controlsession-probe-trace.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice2 T1 — ControlSession probe observation → ControlTrace 配線

## Work

1. `synvelis-project`: `probe_trace` module — `ProbeTracePort` trait、
   `ProbeObservationUpdate`、`ProbePortError`。
2. `synvelis-player`: `probe_trace` module — `ProbeTraceBridge` が
   `RoutingEngine` probe API と node 別 `DispatchTraceAdapter` を
   port 契約に写像。`ProbeEvent` → `ProbeObservation` の
   fail-closed translation（generation 照合・clock provenance・
   status 写像）。
3. `synvelis-workspace`: `ControlSession` に `ProbeTraceSession`
   state + probe-trace API（lifecycle・drain・reads）。port は
   per-call `&mut dyn ProbeTracePort`。
4. テスト: workspace fake-port E2E
   （`tests/sf04_s2_probe_trace.rs`）+ player bridge テスト
   （`tests/sf04_s2_probe_trace_bridge.rs`）。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sf-04-slice2-controlsession-probe-trace.md`
- `docs/designs/sf-04-slice2-controlsession-probe-trace-design.md`
- `docs/specs/sf-04-slice2-controlsession-probe-trace.md`
- `docs/test-specs/sf-04-slice2-controlsession-probe-trace.md`

## Dependencies

- slice 1 SF-04 Semantic Probe Adapter（PR #200・main マージ済み）
- PC-C3 ControlTrace projection（PR #206）
- PC-C3-05 `DispatchTraceAdapter`（PR #209）

## Affected Files

- `crates/synvelis-project/src/probe_trace.rs`（新規）
- `crates/synvelis-project/src/lib.rs`
- `crates/synvelis-player/src/probe_trace.rs`（新規）
- `crates/synvelis-player/src/lib.rs`
- `crates/synvelis-workspace/src/probe_trace.rs`（新規）
- `crates/synvelis-workspace/src/control.rs`（field 追加のみ）
- `crates/synvelis-workspace/src/lib.rs`
- `crates/synvelis-workspace/tests/sf04_s2_probe_trace.rs`（新規）
- `crates/synvelis-player/tests/sf04_s2_probe_trace_bridge.rs`（新規）

## Done When

- [x] probe adapter → ControlSession → ControlTrace.observed の配線が
      session API 経由で動作する（lifecycle・drain 単一 owner・
      generation 照合）
- [x] `RebindReceipt`/segment cut が既存経路（publish → on_publish）
      のまま維持される
- [x] headless fixture で observation → drain → trace slot 記録 →
      freshness 分類の E2E が通り、fail-closed・stale reject が
      確認できる

## Verification

- [x] `mise run fmt-check` — exit 0
- [x] `mise run check` — exit 0
- [x] `mise run lint` — exit 0
- [x] `mise run test` — exit 0
- [x] `mise run test-rt`（存在する範囲）— exit 0
- [ ] `mise run check-ruviform-rev` — exit 1（**既存 failure**:
      `ui/vendor/ruviform/REV` `20a24a66` vs `Cargo.lock` `0f5b5737`。
      両ファイルとも本 diff 未変更の pre-existing 状態。`ui/`・
      `synvelis-app` は編集禁止範囲のため未対処・PR に記録）
- [x] `mise run check-headless-deps` — exit 0
