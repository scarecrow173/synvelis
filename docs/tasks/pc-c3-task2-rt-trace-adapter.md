---
id: "TASK-P1ub1PJgLQhinQuEDIDy8S"
type: "task"
status: "in-progress"
title: "PC-C3 task2 — synvelis-player RT trace adapter 実装（TRACE-C3-05）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/100"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
    - "docs/plans/pc-c3-05-rt-trace-adapter.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c3-05-rt-trace-adapter.md"
    - "docs/test-specs/pc-c3-05-rt-trace-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c3-05-rt-trace-adapter.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c3-05-rt-trace-adapter.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C3 task2 — synvelis-player RT trace adapter 実装（TRACE-C3-05）

## Context

Issue #100 TRACE-C3-05 slice。`ParamDispatch`/`ParamDispatchLedger`/`NodeReport`/`DispatchStage` の実イベントを `synvelis_project::trace::ControlTrace` に写像する player 層 adapter。project 層は read-only 消費。

## Steps

- [x] 1. `src/dispatch.rs`: `DispatchCycle` + `observe_cycle` + `record_reported`/`mark_reporting_unavailable` passthrough（`observe` は wrapper として維持）
- [ ] 2. `src/trace.rs`: `DispatchTraceAdapter`/`TraceAdapterConfig`/`TraceContext`/`TraceMetrics`/`TraceEvent`/`TraceEventKind` + 写像・bounded history・metrics
- [ ] 3. `src/lib.rs` export + `Cargo.toml` dev-dep `stats_alloc`
- [ ] 4. `tests/trace_c3_05.rs`（TRC-T1〜T10）+ `tests/trace_c3_05_rt.rs`（TRC-T11）
- [ ] 5. `mise run fmt-check`/`check`/`lint`/`test`/`test-rt` + review-protocol
- [ ] 6. docs/impl/ir 記録 + index 再生成

## Acceptance

- spec TRC-C3-05-R1〜R6・test-spec TRC-T1〜T11 を満たす。
- `mise run test-rt` で RT hot path no-alloc を実測。
