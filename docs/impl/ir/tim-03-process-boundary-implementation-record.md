---
id: "IMPL-034UyF6gu1UEMJOKr7ZnGg"
type: "impl"
status: "completed"
title: "TIM-03 process boundary implementation record"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/tim03-task1-process-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/tim03-task1-process-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim03-process-boundary.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/tim03-process-boundary.md"
  verified-by: []
  references:
    - "docs/tasks/tim03-task1-process-boundary.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# TIM-03 process boundary implementation record

## Summary

Issue #127 の process boundary slice を `synvelis-player` に実装:
consumer 側 precision/degraded/offline policy oracle
（`process_boundary.rs`）と headless mock 検証 3 層 + RT 証拠。
Ruviform の generic scheduler/`PluginNode` は消費のみで変更なし。

## Context

`docs/tasks/tim03-task1-process-boundary.md` /
`docs/plans/tim03-process-boundary.md` /
`docs/specs/tim03-process-boundary.md` /
`docs/designs/tim03-process-boundary-design.md` に従う。
PR #221（PC-C4）merged の consumer 基盤と整合。

## Implementation

### Changes

- `crates/synvelis-player/src/process_boundary.rs`（新規・約 700 行）:
  - `TimingPrecision`（Issue 語彙 6 variant、`#[non_exhaustive]`）。
  - `ExecutionMode`（`Realtime`/`OfflineExact`）。
  - `ProcessCapability`（`BlockContract` + certified
    `TimingPrecision`、`new`/`unmeasured`/`contract`/`precision`/
    `evaluate`）。
  - `DegradedConsent`（`Default` 非実装、`grant(max_deferral)` のみ）。
  - `BoundaryKind`/`BoundaryPoint`/`ProcessBoundaryRequest`/
    `SegmentPlan`/`BoundaryDegradation`/`ProcessBoundaryPlan`/
    `ProcessBoundaryError`（typed 6 variant）。
  - `evaluate`: offset 正規化（0= head apply、同一 offset coalesce、
    `>window` reject）→ segment 列挙 → `legal_span_len` legality
    （`max_frames==0` は `Unsupported` で事前拒否）→
    exact/degraded/offline/consent/deferral 判定。
  - degraded scan は upstream `find_legal_boundary` を mirror:
    `prev`/`next_bound`（pending non-wrap cut or window end）・
    `min(consent.max_deferral, budget.max_deferral)` bound・
    両隣接 segment legality・`BoundaryDegradation` 記録。
    deferral で requested offset ≤ applied に coalesce。
- `crates/synvelis-player/src/lib.rs`: `pub mod process_boundary`
  + re-export。
- `crates/synvelis-player/tests/tim03_process_boundary.rs`（新規・
  約 1350 行・17 test）: `tim03_pb_*` 8・`tim03_sched_*` 5・
  `tim03_node_*` 4。recording mock plugin（fixed-array `CallLog`）
  + 実 `TransportScheduler`/`PluginNode`/`GraphRuntime`。
- `crates/synvelis-player/tests/tim03_process_boundary_rt.rs`
  （新規・1 test）: `stats_alloc` Region で 64×256f callback
  （wrap boundary + in-loop param event 含む）alloc-free 実測。

### Decisions During Implementation

- **degradation wrap の fail-closed**: upstream degraded は
  illegal wrap を undeliverable span + wrap 継続で処理するが、
  oracle は consented degraded でも合法 boundary が見つから
  なければ `DeferralExceeded` で plan 全体を拒否（undeliverable
  を「consented degraded delivery」として認定しない）。
- **`next_bound` は non-wrap cut のみ**: upstream
  `find_legal_boundary` は loop wrap を scan bound に使わない
  （position-driven）ため oracle も同じ。test
  `tim03_pb_degraded_coalescing_and_wrap_bounds` で固定。
- **plan.precision は delivered 精度**: degraded 適用時は
  `DegradedWithConsent`、exact+`Verified` in-block は
  `ExactWithinVerifiedCapability`、それ以外 exact は
  `FrameAccurateEvents`。capability の certified precision は
  `Unknown` のまま昇格しない。
- **degenerate contract guard**: `BlockContract::new(0,0,_)` は
  `legal_span_len` で除算 panic し得るため `evaluate` が
  `Unsupported` で事前拒否。

## Related Experiments

### Adopted

- なし（experiment log なし）。

### Rejected

- なし。

## Validation

- 補助証拠（canonical = mise）:
  - `cargo test -p synvelis-player --test tim03_process_boundary`
    → 17 passed, 0 failed。
  - `cargo test -p synvelis-player --test tim03_process_boundary_rt`
    → 1 passed（allocations=0, deallocations=0）。
  - `cargo check -p synvelis-player --all-targets` → exit 0。
- canonical（`mise run`）: PR 本文に結果を記録。

## Risks

- oracle は未配線（composition root への反映は後続 slice）のため
  現時点で実害なし。upstream semantics との乖離は test で固定済み
  だが、将来の配線時に再照合が必要。
- 実 CLAP/VST3/OS matrix は NotRun（別 gate）。

## Follow-ups

- `ProcessCapability::evaluate` → `AttachConfig`/`PluginNodeControl`
  への plan 反映（`DeliveryPolicy`/`ScheduleBudget`/consented
  degraded wiring）。
- `TimingPrecision` certified class の認定 provenance。
- boundary offset の Tempo map/Loop occurrence 自動導出。
