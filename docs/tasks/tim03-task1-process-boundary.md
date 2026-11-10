---
id: "TASK-034UyED9PUv2ECvzH9A3RX"
type: "task"
status: "done"
title: "TIM-03 task1 — process boundary/capability/degraded policy 実装"
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
    - "docs/plans/tim03-process-boundary.md"
  implemented-by: []
  depends-on:
    - "docs/plans/tim03-process-boundary.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/designs/tim03-process-boundary-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim03-process-boundary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/tim03-process-boundary.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim03-process-boundary.md"
  references: []
  defers: []
  deferred-by: []
---

# TIM-03 task1 — process boundary/capability/degraded policy 実装

## Work

- `crates/synvelis-player/src/process_boundary.rs` 新設:
  `TimingPrecision`/`ExecutionMode`/`ProcessCapability`/
  `DegradedConsent`/`BoundaryKind`/`BoundaryPoint`/
  `ProcessBoundaryRequest`/`SegmentPlan`/`BoundaryDegradation`/
  `ProcessBoundaryPlan`/`ProcessBoundaryError` と
  `ProcessCapability::evaluate`（exact/degraded/offline/consent/
  deferral/coalescing/degenerate-contract guard）。
- `crates/synvelis-player/src/lib.rs` に module + re-export 追加。
- `crates/synvelis-player/tests/tim03_process_boundary.rs` 新設:
  `tim03_pb_*`（evaluator）/`tim03_sched_*`（実
  `TransportScheduler`）/`tim03_node_*`（実 `PluginNode` +
  `GraphRuntime`）の 3 層 test 17 件。
- `crates/synvelis-player/tests/tim03_process_boundary_rt.rs` 新設:
  `rt_process_boundary_hot_path_alloc_free`（`stats_alloc`、
  boundary split + event delivery を含む 64 callback で
  allocations=deallocations=0）。
- docs chain: spec/design/plan/task/test-spec/impl record。

## Classification

- consumer policy layer（non-RT oracle）+ headless mock 検証。
  RT 配送機構は Ruviform 既存実装を消費のみ。

## Upstream Documents

- `docs/specs/tim03-process-boundary.md`
- `docs/designs/tim03-process-boundary-design.md`
- `docs/plans/tim03-process-boundary.md`
- `docs/test-specs/tim03-process-boundary.md`

## Dependencies

- Ruviform pin `0f5b573`: `BlockContract`/`CallPartition`/
  `DeliveryPolicy`/`ScheduleBudget`/`TransportScheduler`/
  `SpanSink`/`event_engine_frame`/`calls_for_span`/
  `PluginNode`/`attach`/`NodeReport`/`TransportPlanBuilder`。
- PC-C4（#221 merged）の consumer 基盤（本 task は直接 import
  しないが同 crate 内で整合）。

## Affected Files

- `crates/synvelis-player/src/process_boundary.rs`（新規）
- `crates/synvelis-player/src/lib.rs`（export 追加）
- `crates/synvelis-player/tests/tim03_process_boundary.rs`（新規）
- `crates/synvelis-player/tests/tim03_process_boundary_rt.rs`（新規）
- `docs/specs|designs|plans|tasks|test-specs|impl/ir/tim03-*`（新規）

## Done When

- 受入条件 3 項目を test で充足（spec の Acceptance Criteria）。
- `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv`/
  `check-headless-deps` が pass（存在するもの全て）。
- review-protocol self-review で blocker なし。

## Verification

- 補助証拠（canonical = mise）:
  `cargo test -p synvelis-player --test tim03_process_boundary` →
  17 passed; `--test tim03_process_boundary_rt` → 1 passed。
- canonical: `mise run` 系タスク（PR 本文に結果記録）。
